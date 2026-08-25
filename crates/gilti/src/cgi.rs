// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

//! HTTP-to-CGI Tower service.

#[derive(Clone, Copy)]
pub struct RemoteAddr(pub std::net::SocketAddr);

#[derive(Clone)]
pub struct Cgi {
    program: std::path::PathBuf,
    current_dir: std::path::PathBuf,
    environment: Vec<(std::ffi::OsString, std::ffi::OsString)>,
    server_addr: std::net::SocketAddr,
}

impl Cgi {
    pub fn new(
        program: impl Into<std::path::PathBuf>,
        current_dir: impl Into<std::path::PathBuf>,
        server_addr: std::net::SocketAddr,
    ) -> Self {
        Self {
            program: program.into(),
            current_dir: current_dir.into(),
            environment: Vec::new(),
            server_addr,
        }
    }

    pub fn env(
        mut self,
        name: impl Into<std::ffi::OsString>,
        value: impl Into<std::ffi::OsString>,
    ) -> Self {
        self.environment.push((name.into(), value.into()));
        self
    }

    async fn execute(
        self,
        request: axum::http::Request<axum::body::Body>,
    ) -> std::io::Result<axum::http::Response<axum::body::Body>> {
        let remote = request.extensions().get::<RemoteAddr>().map_or_else(
            || std::net::SocketAddr::from(([0, 0, 0, 0], 0)),
            |value| value.0,
        );
        let (parts, body) = request.into_parts();
        let body = axum::body::to_bytes(body, 1024 * 1024)
            .await
            .map_err(std::io::Error::other)?;
        let path = percent_encoding::percent_decode_str(parts.uri.path()).collect::<Vec<_>>();
        if path.contains(&0) {
            return Err(invalid("request path contains a null byte"));
        }
        let path = <std::ffi::OsString as std::os::unix::ffi::OsStringExt>::from_vec(path);
        let host = parts
            .headers
            .get(axum::http::header::HOST)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("localhost");
        let content_type = parts
            .headers
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("");
        let request_uri = parts
            .uri
            .path_and_query()
            .map_or(parts.uri.path(), |value| value.as_str());

        let mut command = tokio::process::Command::new(&self.program);
        command
            .env_clear()
            .envs(self.environment)
            .env("GATEWAY_INTERFACE", "CGI/1.1")
            .env("SERVER_PROTOCOL", "HTTP/1.1")
            .env("REQUEST_METHOD", parts.method.as_str())
            .env("SCRIPT_FILENAME", &self.program)
            .env("SCRIPT_NAME", "")
            .env("PATH_INFO", path)
            .env("QUERY_STRING", parts.uri.query().unwrap_or(""))
            .env("REQUEST_URI", request_uri)
            .env("REMOTE_ADDR", remote.ip().to_string())
            .env("REMOTE_PORT", remote.port().to_string())
            .env("SERVER_ADDR", self.server_addr.ip().to_string())
            .env("SERVER_PORT", self.server_addr.port().to_string())
            .env("SERVER_NAME", host)
            .env("CONTENT_LENGTH", body.len().to_string())
            .env("CONTENT_TYPE", content_type)
            .env("HTTP_HOST", host)
            .current_dir(self.current_dir)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit());

        for (name, value) in &parts.headers {
            if name == axum::http::header::HOST
                || name == axum::http::header::CONTENT_LENGTH
                || name == axum::http::header::CONTENT_TYPE
                || name == "proxy"
            {
                continue;
            }
            if let Ok(value) = value.to_str() {
                command.env(
                    format!(
                        "HTTP_{}",
                        name.as_str().to_ascii_uppercase().replace('-', "_")
                    ),
                    value,
                );
            }
        }

        let mut child = command.spawn()?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| std::io::Error::other("CGI stdin is unavailable"))?;
        let write = async move {
            tokio::io::AsyncWriteExt::write_all(&mut stdin, &body).await?;
            tokio::io::AsyncWriteExt::shutdown(&mut stdin).await
        };
        let (write, output) = tokio::join!(write, child.wait_with_output());
        write?;
        let output = output?;
        if !output.status.success() {
            return Err(std::io::Error::other(format!(
                "CGI program exited with {}",
                output.status
            )));
        }

        let (status, headers, body) = parse_response(&output.stdout)?;
        let mut response = axum::http::Response::builder()
            .status(status)
            .body(axum::body::Body::from(body.to_vec()))
            .map_err(std::io::Error::other)?;
        *response.headers_mut() = headers;
        Ok(response)
    }
}

impl tower::Service<axum::http::Request<axum::body::Body>> for Cgi {
    type Response = axum::http::Response<axum::body::Body>;
    type Error = std::io::Error;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = std::io::Result<Self::Response>> + Send>,
    >;

    fn poll_ready(
        &mut self,
        _context: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: axum::http::Request<axum::body::Body>) -> Self::Future {
        let service = self.clone();
        Box::pin(async move { service.execute(request).await })
    }
}

fn parse_response(
    output: &[u8],
) -> std::io::Result<(axum::http::StatusCode, axum::http::HeaderMap, &[u8])> {
    let (head, body) = if let Some(offset) = output.windows(4).position(|part| part == b"\r\n\r\n")
    {
        (&output[..offset], &output[offset + 4..])
    } else if let Some(offset) = output.windows(2).position(|part| part == b"\n\n") {
        (&output[..offset], &output[offset + 2..])
    } else {
        return Err(invalid("CGI response has no header separator"));
    };

    let mut status = axum::http::StatusCode::OK;
    let mut headers = axum::http::HeaderMap::new();
    for line in head.split(|byte| *byte == b'\n') {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        let colon = line
            .iter()
            .position(|byte| *byte == b':')
            .ok_or_else(|| invalid("malformed CGI header"))?;
        let (name, mut value) = (&line[..colon], &line[colon + 1..]);
        while value.first().is_some_and(u8::is_ascii_whitespace) {
            value = &value[1..];
        }
        if name.eq_ignore_ascii_case(b"Status") {
            status = axum::http::StatusCode::from_bytes(
                value.split(|byte| *byte == b' ').next().unwrap_or(value),
            )
            .map_err(|_| invalid("invalid CGI status"))?;
        } else {
            headers.append(
                axum::http::HeaderName::from_bytes(name)
                    .map_err(|_| invalid("invalid CGI header"))?,
                axum::http::HeaderValue::from_bytes(value)
                    .map_err(|_| invalid("invalid CGI header"))?,
            );
        }
    }
    if status == axum::http::StatusCode::OK && headers.contains_key("location") {
        status = axum::http::StatusCode::FOUND;
    }
    Ok((status, headers, body))
}

fn invalid(message: &'static str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_cgi_response() {
        let (status, headers, body) =
            super::parse_response(b"Status: 201 Created\r\nContent-Type: text/plain\r\n\r\nhello")
                .unwrap();
        assert_eq!(status, axum::http::StatusCode::CREATED);
        assert_eq!(headers[axum::http::header::CONTENT_TYPE], "text/plain");
        assert_eq!(body, b"hello");
    }

    #[test]
    fn is_a_tower_service() {
        fn assert_service<T: tower::Service<axum::http::Request<axum::body::Body>>>() {}
        assert_service::<super::Cgi>();
    }
}
