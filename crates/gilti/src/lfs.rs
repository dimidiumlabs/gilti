// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

const MAX_OBJECT_SIZE: usize = 1024 * 1024 * 1024;

#[derive(serde::Deserialize)]
struct BatchRequest {
    operation: String,
    objects: Vec<ObjectRequest>,
}

#[derive(serde::Deserialize)]
struct ObjectRequest {
    oid: String,
    size: u64,
}

#[derive(serde::Deserialize)]
struct VerifyRequest {
    oid: String,
    size: u64,
}

#[derive(serde::Serialize)]
struct BatchResponse {
    transfer: &'static str,
    objects: Vec<ObjectResponse>,
}

#[derive(serde::Serialize)]
struct ObjectResponse {
    oid: String,
    size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    actions: Option<Actions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<ObjectError>,
}

#[derive(serde::Serialize)]
struct Actions {
    #[serde(skip_serializing_if = "Option::is_none")]
    download: Option<Action>,
    #[serde(skip_serializing_if = "Option::is_none")]
    upload: Option<Action>,
    #[serde(skip_serializing_if = "Option::is_none")]
    verify: Option<Action>,
}

#[derive(serde::Serialize)]
struct Action {
    href: String,
}

#[derive(serde::Serialize)]
struct ObjectError {
    code: u16,
    message: &'static str,
}

pub async fn serve(
    repositories: &std::path::Path,
    repo: &str,
    path: &str,
    write_enabled: bool,
    request: axum::extract::Request,
) -> axum::response::Response {
    let Some(repository) = repository(repositories, repo) else {
        return plain(axum::http::StatusCode::NOT_FOUND, "repository not found\n");
    };
    let objects = repository.join("lfs/objects");
    let method = request.method().clone();

    if path == "objects/batch" && method == axum::http::Method::POST {
        return batch(objects, write_enabled, request).await;
    }
    if method == axum::http::Method::POST {
        if let Some(oid) = path
            .strip_prefix("objects/")
            .and_then(|value| value.strip_suffix("/verify"))
        {
            return verify(&objects, oid, request).await;
        }
    }
    if let Some(oid) = path.strip_prefix("objects/") {
        if !valid_oid(oid) {
            return plain(axum::http::StatusCode::BAD_REQUEST, "invalid object id\n");
        }
        return match method {
            axum::http::Method::GET | axum::http::Method::HEAD => {
                download(&objects, oid, method == axum::http::Method::HEAD).await
            }
            axum::http::Method::PUT if write_enabled => upload(&objects, oid, request).await,
            axum::http::Method::PUT => plain(
                axum::http::StatusCode::FORBIDDEN,
                "LFS uploads are disabled\n",
            ),
            _ => plain(
                axum::http::StatusCode::METHOD_NOT_ALLOWED,
                "method not allowed\n",
            ),
        };
    }
    plain(axum::http::StatusCode::NOT_FOUND, "not found\n")
}

async fn batch(
    objects: std::path::PathBuf,
    write_enabled: bool,
    request: axum::extract::Request,
) -> axum::response::Response {
    let base = request
        .uri()
        .path()
        .strip_suffix("/objects/batch")
        .unwrap_or(request.uri().path());
    let scheme = request
        .headers()
        .get("x-forwarded-proto")
        .and_then(|value| value.to_str().ok())
        .filter(|value| matches!(*value, "http" | "https"))
        .unwrap_or("http");
    let host = request
        .headers()
        .get(axum::http::header::HOST)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("localhost");
    let prefix = format!("{scheme}://{host}{base}/objects");
    let body = match axum::body::to_bytes(request.into_body(), 1024 * 1024).await {
        Ok(body) => body,
        Err(_) => {
            return plain(
                axum::http::StatusCode::PAYLOAD_TOO_LARGE,
                "request too large\n",
            );
        }
    };
    let request: BatchRequest = match serde_json::from_slice(&body) {
        Ok(request) => request,
        Err(_) => return plain(axum::http::StatusCode::BAD_REQUEST, "invalid LFS request\n"),
    };
    if !matches!(request.operation.as_str(), "download" | "upload") {
        return plain(
            axum::http::StatusCode::BAD_REQUEST,
            "invalid LFS operation\n",
        );
    }

    let objects = request
        .objects
        .into_iter()
        .map(|object| {
            if !valid_oid(&object.oid) {
                return error(object, 422, "invalid object id");
            }
            let stored = object_path(&objects, &object.oid);
            let present = std::fs::metadata(&stored)
                .is_ok_and(|metadata| metadata.is_file() && metadata.len() == object.size);
            match request.operation.as_str() {
                "download" if present => response(
                    object,
                    Actions {
                        download: Some(Action {
                            href: format!(
                                "{prefix}/{}",
                                stored.file_name().unwrap().to_string_lossy()
                            ),
                        }),
                        upload: None,
                        verify: None,
                    },
                ),
                "download" => error(object, 404, "object not found"),
                "upload" if present => response(
                    object,
                    Actions {
                        download: Some(Action {
                            href: format!(
                                "{prefix}/{}",
                                stored.file_name().unwrap().to_string_lossy()
                            ),
                        }),
                        upload: None,
                        verify: None,
                    },
                ),
                "upload" if !write_enabled => error(object, 403, "LFS uploads are disabled"),
                "upload" => response(
                    object,
                    Actions {
                        download: None,
                        upload: Some(Action {
                            href: format!(
                                "{prefix}/{}",
                                stored.file_name().unwrap().to_string_lossy()
                            ),
                        }),
                        verify: Some(Action {
                            href: format!(
                                "{prefix}/{}/verify",
                                stored.file_name().unwrap().to_string_lossy()
                            ),
                        }),
                    },
                ),
                _ => unreachable!(),
            }
        })
        .collect();
    json(
        axum::http::StatusCode::OK,
        &BatchResponse {
            transfer: "basic",
            objects,
        },
    )
}

async fn download(objects: &std::path::Path, oid: &str, head: bool) -> axum::response::Response {
    let path = object_path(objects, oid);
    let metadata = match tokio::fs::metadata(&path).await {
        Ok(metadata) if metadata.is_file() => metadata,
        _ => return plain(axum::http::StatusCode::NOT_FOUND, "object not found\n"),
    };
    let body = if head {
        axum::body::Body::empty()
    } else {
        match tokio::fs::read(path).await {
            Ok(body) => axum::body::Body::from(body),
            Err(_) => {
                return plain(
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    "read failed\n",
                );
            }
        }
    };
    axum::http::Response::builder()
        .status(axum::http::StatusCode::OK)
        .header(axum::http::header::CONTENT_TYPE, "application/octet-stream")
        .header(axum::http::header::CONTENT_LENGTH, metadata.len())
        .body(body)
        .expect("valid LFS response")
}

async fn upload(
    objects: &std::path::Path,
    oid: &str,
    request: axum::extract::Request,
) -> axum::response::Response {
    use sha2::Digest;

    let body = match axum::body::to_bytes(request.into_body(), MAX_OBJECT_SIZE).await {
        Ok(body) => body,
        Err(_) => {
            return plain(
                axum::http::StatusCode::PAYLOAD_TOO_LARGE,
                "object too large\n",
            );
        }
    };
    let actual = format!("{:x}", sha2::Sha256::digest(&body));
    if actual != oid {
        return plain(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "object hash mismatch\n",
        );
    }
    let path = object_path(objects, oid);
    let Some(parent) = path.parent() else {
        return plain(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "invalid storage path\n",
        );
    };
    if tokio::fs::create_dir_all(parent).await.is_err() {
        return plain(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "storage failed\n",
        );
    }
    let temporary = parent.join(format!(".{oid}.{}.tmp", std::process::id()));
    if tokio::fs::write(&temporary, &body).await.is_err()
        || tokio::fs::rename(&temporary, &path).await.is_err()
    {
        let _ = tokio::fs::remove_file(temporary).await;
        return plain(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "storage failed\n",
        );
    }
    plain(axum::http::StatusCode::OK, "")
}

async fn verify(
    objects: &std::path::Path,
    oid: &str,
    request: axum::extract::Request,
) -> axum::response::Response {
    if !valid_oid(oid) {
        return plain(axum::http::StatusCode::BAD_REQUEST, "invalid object id\n");
    }
    let body = match axum::body::to_bytes(request.into_body(), 64 * 1024).await {
        Ok(body) => body,
        Err(_) => {
            return plain(
                axum::http::StatusCode::PAYLOAD_TOO_LARGE,
                "request too large\n",
            );
        }
    };
    let request: VerifyRequest = match serde_json::from_slice(&body) {
        Ok(request) => request,
        Err(_) => return plain(axum::http::StatusCode::BAD_REQUEST, "invalid LFS request\n"),
    };
    let valid = request.oid == oid
        && std::fs::metadata(object_path(objects, oid))
            .is_ok_and(|metadata| metadata.is_file() && metadata.len() == request.size);
    if valid {
        plain(axum::http::StatusCode::OK, "")
    } else {
        plain(axum::http::StatusCode::NOT_FOUND, "object not found\n")
    }
}

fn repository(root: &std::path::Path, repo: &str) -> Option<std::path::PathBuf> {
    let root = std::fs::canonicalize(root).ok()?;
    let repository = std::fs::canonicalize(root.join(format!("{repo}.git"))).ok()?;
    repository
        .is_dir()
        .then_some(repository)
        .filter(|repository| repository.starts_with(root))
}

fn object_path(root: &std::path::Path, oid: &str) -> std::path::PathBuf {
    root.join(&oid[..2]).join(&oid[2..4]).join(oid)
}

fn valid_oid(oid: &str) -> bool {
    oid.len() == 64
        && oid
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn response(object: ObjectRequest, actions: Actions) -> ObjectResponse {
    ObjectResponse {
        oid: object.oid,
        size: object.size,
        actions: Some(actions),
        error: None,
    }
}

fn error(object: ObjectRequest, code: u16, message: &'static str) -> ObjectResponse {
    ObjectResponse {
        oid: object.oid,
        size: object.size,
        actions: None,
        error: Some(ObjectError { code, message }),
    }
}

fn json<T: serde::Serialize>(
    status: axum::http::StatusCode,
    value: &T,
) -> axum::response::Response {
    match serde_json::to_vec(value) {
        Ok(body) => axum::http::Response::builder()
            .status(status)
            .header(
                axum::http::header::CONTENT_TYPE,
                "application/vnd.git-lfs+json",
            )
            .body(axum::body::Body::from(body))
            .expect("valid LFS response"),
        Err(_) => plain(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "encoding failed\n",
        ),
    }
}

fn plain(status: axum::http::StatusCode, message: &'static str) -> axum::response::Response {
    axum::http::Response::builder()
        .status(status)
        .header(axum::http::header::CONTENT_TYPE, "text/plain")
        .body(axum::body::Body::from(message))
        .expect("valid LFS response")
}
