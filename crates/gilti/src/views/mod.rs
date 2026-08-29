// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

pub mod about;
pub mod archive_signature;
pub mod blame;
pub mod object;
pub mod overview;
pub mod refs;
pub mod repositories;
pub mod shared;
pub mod tag;
pub mod tree;

fn method_not_allowed() -> axum::response::Response {
    axum::response::Response::builder()
        .status(axum::http::StatusCode::METHOD_NOT_ALLOWED)
        .header(axum::http::header::CONTENT_TYPE, "text/plain")
        .body(axum::body::Body::from("method not allowed\n"))
        .expect("static response is valid")
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
