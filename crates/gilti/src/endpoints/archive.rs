// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

pub use gilti_git::archive::Format;

pub async fn serve(
    context: &super::shared::Context,
    route: crate::router::RepoRoute<crate::router::RevisionPath>,
    format: Format,
    method: axum::http::Method,
) -> axum::response::Response {
    if method != axum::http::Method::GET && method != axum::http::Method::HEAD {
        return super::method_not_allowed();
    }
    let repositories = std::sync::Arc::clone(&context.repositories);
    let name = route.repo.clone();
    let revision = route.params.rev.clone();
    let path = route.params.path.clone();
    let model = tokio::task::spawn_blocking(move || {
        gilti_git::archive::Archive::load(repositories.as_path(), &name, &revision, path.as_deref())
    })
    .await;
    let model = match model {
        Ok(Ok(model)) => model,
        Ok(Err(error)) => return super::error(error),
        Err(error) => return super::error(gilti_git::Error::Internal(error.to_string())),
    };
    let filename = format!("{}.{}", model.prefix, format.as_str());
    let body = if method == axum::http::Method::HEAD {
        axum::body::Body::empty()
    } else {
        let stream = match generate(
            &context.git,
            &model,
            route.params.path.as_deref(),
            format,
            context.archive_compression,
        )
        .await
        {
            Ok(stream) => stream,
            Err(error) => return super::error(error),
        };
        axum::body::Body::from_stream(stream)
    };
    axum::response::Response::builder()
        .status(axum::http::StatusCode::OK)
        .header(axum::http::header::CONTENT_TYPE, content_type(format))
        .header(
            axum::http::header::CONTENT_DISPOSITION,
            content_disposition(&filename),
        )
        .header(axum::http::header::ETAG, format!("\"{}\"", model.oid))
        .body(body)
        .expect("archive response is valid")
}

async fn generate(
    git: &gilti_git::commands::GitCommand,
    model: &gilti_git::archive::Archive,
    path: Option<&str>,
    format: Format,
    compression: gilti_git::commands::ArchiveCompression,
) -> Result<gilti_git::commands::ArchiveStream, gilti_git::Error> {
    gilti_git::commands::archive(
        git,
        &model.repository_path,
        &model.oid,
        &model.prefix,
        format,
        path,
        compression,
    )
    .await
}

fn content_type(format: Format) -> &'static str {
    match format {
        Format::Tar => "application/x-tar; charset=UTF-8",
        Format::TarGzip => "application/gzip",
        Format::TarBzip2 => "application/x-bzip2",
        Format::TarXz => "application/x-xz",
        Format::TarZstd => "application/zstd",
        Format::Zip => "application/zip",
    }
}

fn content_disposition(filename: &str) -> String {
    let fallback = filename
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    let encoded =
        percent_encoding::utf8_percent_encode(filename, percent_encoding::NON_ALPHANUMERIC);
    format!("inline; filename=\"{fallback}\"; filename*=UTF-8''{encoded}")
}
