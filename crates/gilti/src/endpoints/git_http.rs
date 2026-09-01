// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later
//! Axum adaptation for the streaming framework-neutral git HTTP backend.
use futures_util::TryStreamExt;
use tokio_util::io::{ReaderStream, StreamReader};
pub async fn serve(
    backend: &gilti_git::backend::HttpBackend,
    request: axum::extract::Request,
    environment: Vec<(std::ffi::OsString, std::ffi::OsString)>,
) -> Result<axum::response::Response, gilti_git::backend::BackendError> {
    let remote = request
        .extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        .map(|v| v.0);
    let (parts, body) = request.into_parts();
    let reader = StreamReader::new(body.into_data_stream().map_err(std::io::Error::other));
    let response = backend
        .execute(gilti_git::backend::BackendRequest {
            method: parts.method.to_string(),
            path: parts.uri.path().into(),
            query: parts.uri.query().map(str::to_owned),
            headers: parts
                .headers
                .iter()
                .filter_map(|(n, v)| v.to_str().ok().map(|v| (n.to_string(), v.into())))
                .collect(),
            body: Box::pin(reader),
            remote_addr: remote,
            environment,
        })
        .await?;
    let mut builder = axum::http::Response::builder().status(response.status);
    for (name, value) in response.headers {
        builder = builder.header(name, value)
    }
    builder
        .body(axum::body::Body::from_stream(ReaderStream::new(
            response.body,
        )))
        .map_err(|e| gilti_git::backend::BackendError::Io(e.to_string()))
}
