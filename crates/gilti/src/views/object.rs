// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

pub async fn serve(
    repositories: &'static str,
    route: crate::router::RepoRoute<String>,
    method: axum::http::Method,
) -> axum::response::Response {
    if method != axum::http::Method::GET && method != axum::http::Method::HEAD {
        return axum::response::Response::builder()
            .status(axum::http::StatusCode::METHOD_NOT_ALLOWED)
            .header(axum::http::header::CONTENT_TYPE, "text/plain")
            .body(axum::body::Body::from("method not allowed\n"))
            .expect("static response is valid");
    }
    let model = tokio::task::spawn_blocking(move || {
        crate::models::object::RawObject::load(
            std::path::Path::new(repositories),
            &route.repo,
            &route.params,
        )
    })
    .await;
    let model = match model {
        Ok(Ok(model)) => model,
        Ok(Err(error)) => return super::error(error),
        Err(error) => {
            return super::error(crate::models::Error::Internal(error.to_string()));
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
            if model.binary {
                "application/octet-stream; charset=UTF-8"
            } else {
                "text/plain; charset=UTF-8"
            },
        )
        .header(axum::http::header::CONTENT_LENGTH, length)
        .header("x-content-type-options", "nosniff")
        .header("content-security-policy", "default-src 'none'")
        .body(body)
        .expect("object response is valid")
}
