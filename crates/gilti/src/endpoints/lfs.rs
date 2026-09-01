// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Git LFS HTTP protocol adaptation; storage is implemented by `gilti_git::lfs`.
use futures_util::TryStreamExt;
use tokio_util::io::{ReaderStream, StreamReader};

#[derive(Clone, serde::Deserialize)]
struct BatchRequest {
    operation: String,
    objects: Vec<ObjectRequest>,
}
#[derive(Clone, serde::Deserialize)]
struct ObjectRequest {
    oid: String,
    size: u64,
}
#[derive(Clone, serde::Deserialize)]
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
    root: &std::path::Path,
    repo: &str,
    path: &str,
    write_enabled: bool,
    request: axum::extract::Request,
) -> axum::response::Response {
    let store = match gilti_git::lfs::LfsStore::open(root, repo) {
        Ok(store) => store,
        Err(_) => return plain(axum::http::StatusCode::NOT_FOUND, "repository not found\n"),
    };
    let method = request.method().clone();
    if path == "objects/batch" && method == axum::http::Method::POST {
        return batch(store, write_enabled, request).await;
    }
    if method == axum::http::Method::POST
        && let Some(oid) = path
            .strip_prefix("objects/")
            .and_then(|value| value.strip_suffix("/verify"))
    {
        return verify(store, oid, request).await;
    }
    let Some(oid) = path.strip_prefix("objects/") else {
        return plain(axum::http::StatusCode::NOT_FOUND, "not found\n");
    };
    if !gilti_git::lfs::valid_oid(oid) {
        return plain(axum::http::StatusCode::BAD_REQUEST, "invalid object id\n");
    }
    match method {
        axum::http::Method::GET | axum::http::Method::HEAD => {
            download(store, oid, method == axum::http::Method::HEAD).await
        }
        axum::http::Method::PUT if write_enabled => upload(store, oid, request).await,
        axum::http::Method::PUT => plain(
            axum::http::StatusCode::FORBIDDEN,
            "LFS uploads are disabled\n",
        ),
        _ => plain(
            axum::http::StatusCode::METHOD_NOT_ALLOWED,
            "method not allowed\n",
        ),
    }
}
async fn batch(
    store: gilti_git::lfs::LfsStore,
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
        .and_then(|v| v.to_str().ok())
        .filter(|v| matches!(*v, "http" | "https"))
        .unwrap_or("http");
    let host = request
        .headers()
        .get(axum::http::header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("localhost");
    let prefix = format!("{scheme}://{host}{base}/objects");
    let body = match axum::body::to_bytes(request.into_body(), 1024 * 1024).await {
        Ok(v) => v,
        Err(_) => {
            return plain(
                axum::http::StatusCode::PAYLOAD_TOO_LARGE,
                "request too large\n",
            );
        }
    };
    let request: BatchRequest = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(_) => return plain(axum::http::StatusCode::BAD_REQUEST, "invalid LFS request\n"),
    };
    if !matches!(request.operation.as_str(), "download" | "upload") {
        return plain(
            axum::http::StatusCode::BAD_REQUEST,
            "invalid LFS operation\n",
        );
    };
    let objects = request
        .objects
        .into_iter()
        .map(|object| {
            if !gilti_git::lfs::valid_oid(&object.oid) {
                return error(object, 422, "invalid object id");
            };
            let present = store.present(&object.oid, object.size).unwrap_or(false);
            match request.operation.as_str() {
                "download" if present => response(
                    object.clone(),
                    Actions {
                        download: Some(Action {
                            href: format!("{prefix}/{}", object.oid),
                        }),
                        upload: None,
                        verify: None,
                    },
                ),
                "download" => error(object, 404, "object not found"),
                "upload" if present => response(
                    object.clone(),
                    Actions {
                        download: Some(Action {
                            href: format!("{prefix}/{}", object.oid),
                        }),
                        upload: None,
                        verify: None,
                    },
                ),
                "upload" if !write_enabled => error(object, 403, "LFS uploads are disabled"),
                "upload" => response(
                    object.clone(),
                    Actions {
                        download: None,
                        upload: Some(Action {
                            href: format!("{prefix}/{}", object.oid),
                        }),
                        verify: Some(Action {
                            href: format!("{prefix}/{}/verify", object.oid),
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
async fn download(
    store: gilti_git::lfs::LfsStore,
    oid: &str,
    head: bool,
) -> axum::response::Response {
    let result = store.open_stream(oid).await;
    let (length, file) = match result {
        Ok(value) => value,
        Err(_) => return plain(axum::http::StatusCode::NOT_FOUND, "object not found\n"),
    };
    let body = if head {
        axum::body::Body::empty()
    } else {
        axum::body::Body::from_stream(ReaderStream::new(file))
    };
    axum::http::Response::builder()
        .status(axum::http::StatusCode::OK)
        .header(axum::http::header::CONTENT_TYPE, "application/octet-stream")
        .header(axum::http::header::CONTENT_LENGTH, length)
        .body(body)
        .expect("valid LFS response")
}
async fn upload(
    store: gilti_git::lfs::LfsStore,
    oid: &str,
    request: axum::extract::Request,
) -> axum::response::Response {
    let reader = StreamReader::new(
        request
            .into_body()
            .into_data_stream()
            .map_err(std::io::Error::other),
    );
    let mut reader = reader;
    match store.write_stream(oid, &mut reader).await {
        Ok(()) => plain(axum::http::StatusCode::OK, ""),
        Err(gilti_git::lfs::StoreError::HashMismatch) => plain(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "object hash mismatch\n",
        ),
        Err(gilti_git::lfs::StoreError::InvalidOid) => {
            plain(axum::http::StatusCode::BAD_REQUEST, "invalid object id\n")
        }
        Err(gilti_git::lfs::StoreError::Storage(message)) if message == "object too large" => {
            plain(
                axum::http::StatusCode::PAYLOAD_TOO_LARGE,
                "object too large\n",
            )
        }
        Err(_) => plain(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "storage failed\n",
        ),
    }
}
async fn verify(
    store: gilti_git::lfs::LfsStore,
    oid: &str,
    request: axum::extract::Request,
) -> axum::response::Response {
    if !gilti_git::lfs::valid_oid(oid) {
        return plain(axum::http::StatusCode::BAD_REQUEST, "invalid object id\n");
    };
    let body = match axum::body::to_bytes(request.into_body(), 64 * 1024).await {
        Ok(v) => v,
        Err(_) => {
            return plain(
                axum::http::StatusCode::PAYLOAD_TOO_LARGE,
                "request too large\n",
            );
        }
    };
    let request: VerifyRequest = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(_) => return plain(axum::http::StatusCode::BAD_REQUEST, "invalid LFS request\n"),
    };
    if request.oid == oid && store.verify(oid, request.size).unwrap_or(false) {
        plain(axum::http::StatusCode::OK, "")
    } else {
        plain(axum::http::StatusCode::NOT_FOUND, "object not found\n")
    }
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
            .expect("valid JSON response"),
        Err(_) => plain(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "JSON serialization failed\n",
        ),
    }
}
fn plain(status: axum::http::StatusCode, message: &str) -> axum::response::Response {
    axum::http::Response::builder()
        .status(status)
        .header(axum::http::header::CONTENT_TYPE, "text/plain")
        .body(axum::body::Body::from(message.to_owned()))
        .expect("valid LFS response")
}
