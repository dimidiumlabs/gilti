// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use tokio::io::AsyncWriteExt;

#[derive(Clone, Copy)]
pub enum Format {
    Tar,
    TarGzip,
    TarBzip2,
    TarLzip,
    TarXz,
    TarZstd,
    Zip,
}

impl Format {
    pub fn parse(value: Option<&str>) -> Option<Self> {
        match value.unwrap_or("tar.gz") {
            "tar" => Some(Self::Tar),
            "tar.gz" => Some(Self::TarGzip),
            "tar.bz2" => Some(Self::TarBzip2),
            "tar.lz" => Some(Self::TarLzip),
            "tar.xz" => Some(Self::TarXz),
            "tar.zst" => Some(Self::TarZstd),
            "zip" => Some(Self::Zip),
            _ => None,
        }
    }

    fn extension(self) -> &'static str {
        match self {
            Self::Tar => "tar",
            Self::TarGzip => "tar.gz",
            Self::TarBzip2 => "tar.bz2",
            Self::TarLzip => "tar.lz",
            Self::TarXz => "tar.xz",
            Self::TarZstd => "tar.zst",
            Self::Zip => "zip",
        }
    }

    fn content_type(self) -> &'static str {
        match self {
            Self::Tar => "application/x-tar; charset=UTF-8",
            Self::TarGzip => "application/x-gzip; charset=UTF-8",
            Self::TarBzip2 => "application/x-bzip2; charset=UTF-8",
            Self::TarLzip => "application/x-lzip; charset=UTF-8",
            Self::TarXz => "application/x-xz; charset=UTF-8",
            Self::TarZstd => "application/x-zstd; charset=UTF-8",
            Self::Zip => "application/x-zip; charset=UTF-8",
        }
    }

    fn git_format(self) -> &'static str {
        match self {
            Self::TarGzip => "tar.gz",
            Self::Zip => "zip",
            _ => "tar",
        }
    }

    fn compressor(self) -> Option<(&'static str, &'static [&'static str])> {
        match self {
            Self::TarBzip2 => Some(("/usr/bin/bzip2", &[])),
            Self::TarLzip => Some(("/usr/bin/lzip", &[])),
            Self::TarXz => Some(("/usr/bin/xz", &[])),
            Self::TarZstd => Some(("/usr/bin/zstd", &["-T0"])),
            _ => None,
        }
    }
}

pub async fn serve(
    context: &super::shared::Context,
    route: crate::router::RepoRoute<crate::router::RevisionPath>,
    format: Format,
    method: axum::http::Method,
) -> axum::response::Response {
    if method != axum::http::Method::GET && method != axum::http::Method::HEAD {
        return super::method_not_allowed();
    }
    let repositories = context.repositories;
    let name = route.repo.clone();
    let revision = route.params.rev.clone();
    let path = route.params.path.clone();
    let model = tokio::task::spawn_blocking(move || {
        crate::models::archive::Archive::load(
            std::path::Path::new(repositories),
            &name,
            &revision,
            path.as_deref(),
        )
    })
    .await;
    let model = match model {
        Ok(Ok(model)) => model,
        Ok(Err(error)) => return super::error(error),
        Err(error) => return super::error(crate::models::Error::Internal(error.to_string())),
    };
    let bytes = match generate(&model, route.params.path.as_deref(), format).await {
        Ok(bytes) => bytes,
        Err(error) => return super::error(crate::models::Error::Internal(error)),
    };
    let filename = format!("{}.{}", model.prefix, format.extension());
    super::bytes_response(
        format.content_type(),
        Some(content_disposition(&filename)),
        Some(format!("\"{}\"", model.oid)),
        bytes,
        &method,
    )
}

async fn generate(
    model: &crate::models::archive::Archive,
    path: Option<&str>,
    format: Format,
) -> Result<Vec<u8>, String> {
    let mut command = tokio::process::Command::new(crate::GIT);
    command
        .arg("--git-dir")
        .arg(&model.repository_path)
        .arg("archive")
        .arg(format!("--format={}", format.git_format()))
        .arg(format!("--prefix={}/", model.prefix))
        .arg(&model.oid)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1");
    if let Some(path) = path {
        command.arg("--").arg(format!(":(literal){path}"));
    }
    let output = command.output().await.map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "git archive failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let Some((program, arguments)) = format.compressor() else {
        return Ok(output.stdout);
    };
    let mut child = tokio::process::Command::new(program)
        .args(arguments)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|error| error.to_string())?;
    let mut input = child.stdin.take().expect("compressor stdin is piped");
    let bytes = output.stdout;
    let writer = tokio::spawn(async move {
        input.write_all(&bytes).await?;
        input.shutdown().await
    });
    let output = child
        .wait_with_output()
        .await
        .map_err(|error| error.to_string())?;
    writer
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(format!(
            "archive compressor failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
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
