// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

pub mod about;
pub mod archive;
pub mod archive_signature;
pub mod blame;
pub mod diff;
pub mod object;
pub mod overview;
pub mod patch;
pub mod refs;
pub mod repositories;
pub mod revision;
pub mod shared;
pub mod stats;
pub mod tag;
pub mod tree;

fn method_not_allowed() -> axum::response::Response {
    axum::response::Response::builder()
        .status(axum::http::StatusCode::METHOD_NOT_ALLOWED)
        .header(axum::http::header::CONTENT_TYPE, "text/plain")
        .body(axum::body::Body::from("method not allowed\n"))
        .expect("static response is valid")
}

pub(crate) fn bad_request(message: &'static str) -> axum::response::Response {
    axum::response::Response::builder()
        .status(axum::http::StatusCode::BAD_REQUEST)
        .header(axum::http::header::CONTENT_TYPE, "text/plain")
        .body(axum::body::Body::from(message))
        .expect("static response is valid")
}

fn bytes_response(
    content_type: &'static str,
    content_disposition: Option<String>,
    etag: Option<String>,
    bytes: Vec<u8>,
    method: &axum::http::Method,
) -> axum::response::Response {
    let length = bytes.len();
    let body = if method == axum::http::Method::HEAD {
        axum::body::Body::empty()
    } else {
        axum::body::Body::from(bytes)
    };
    let mut response = axum::response::Response::builder()
        .status(axum::http::StatusCode::OK)
        .header(axum::http::header::CONTENT_TYPE, content_type)
        .header(axum::http::header::CONTENT_LENGTH, length);
    if let Some(value) = content_disposition {
        response = response.header(axum::http::header::CONTENT_DISPOSITION, value);
    }
    if let Some(value) = etag {
        response = response.header(axum::http::header::ETAG, value);
    }
    response.body(body).expect("byte response is valid")
}

fn error(error: crate::models::Error) -> axum::response::Response {
    let (status, message) = match error {
        crate::models::Error::NotFound => (axum::http::StatusCode::NOT_FOUND, "not found\n"),
        crate::models::Error::Internal(message) => {
            eprintln!("gilti: repository view failed: {message}");
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "internal server error\n",
            )
        }
    };
    axum::response::Response::builder()
        .status(status)
        .header(axum::http::header::CONTENT_TYPE, "text/plain")
        .body(axum::body::Body::from(message))
        .expect("static response is valid")
}
