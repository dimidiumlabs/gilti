// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

pub async fn serve(
    context: &super::shared::Context,
    route: crate::router::RepoRoute<crate::router::Revision>,
    format: gilti_git::archive::Format,
    method: axum::http::Method,
) -> axum::response::Response {
    if method != axum::http::Method::GET && method != axum::http::Method::HEAD {
        return super::method_not_allowed();
    }
    let repositories = std::sync::Arc::clone(&context.repositories);
    let format = format.as_str().to_owned();
    let model = tokio::task::spawn_blocking(move || {
        gilti_git::archive_signature::ArchiveSignature::load(
            repositories.as_path(),
            &route.repo,
            route.params,
            &format,
        )
    })
    .await;
    let model = match model {
        Ok(Ok(model)) => model,
        Ok(Err(error)) => return super::error(error),
        Err(error) => {
            return super::error(gilti_git::Error::Internal(error.to_string()));
        }
    };
    let length = model.bytes.len();
    let body = if method == axum::http::Method::HEAD {
        axum::body::Body::empty()
    } else {
        axum::body::Body::from(model.bytes)
    };
    axum::response::Response::builder()
        .status(axum::http::StatusCode::OK)
        .header(
            axum::http::header::CONTENT_TYPE,
            "application/pgp-signature; charset=UTF-8",
        )
        .header(axum::http::header::CONTENT_LENGTH, length)
        .header(
            axum::http::header::CONTENT_DISPOSITION,
            format!("inline; filename=\"{}\"", model.filename),
        )
        .header(axum::http::header::ETAG, format!("\"{}\"", model.oid))
        .header("x-content-type-options", "nosniff")
        .header("content-security-policy", "default-src 'none'")
        .body(body)
        .expect("archive signature response is valid")
}
