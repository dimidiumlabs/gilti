// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

#![allow(clippy::type_complexity)]
//! Framework-neutral, streaming `git-http-backend` process runner.
use std::{
    ffi::OsString,
    future::Future,
    io,
    net::SocketAddr,
    path::PathBuf,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt, ReadBuf};

#[derive(Clone)]
pub struct HttpBackend {
    program: PathBuf,
    current_dir: PathBuf,
    environment: Vec<(OsString, OsString)>,
    server_addr: SocketAddr,
    max_request_bytes: u64,
    max_response_header_bytes: usize,
}

pub struct BackendRequest {
    pub method: String,
    pub path: String,
    pub query: Option<String>,
    pub headers: Vec<(String, String)>,
    pub body: Pin<Box<dyn AsyncRead + Send>>,
    pub remote_addr: Option<SocketAddr>,
    pub environment: Vec<(OsString, OsString)>,
}

pub struct BackendResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: BackendBody,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BackendError {
    InvalidResponse,
    InputTooLarge,
    Io(String),
    Failed(String),
}

impl std::fmt::Display for BackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for BackendError {}

pub struct BackendBody {
    prefix: io::Cursor<Vec<u8>>,
    stdout: tokio::process::ChildStdout,
    child: Option<tokio::process::Child>,
    exit: Option<tokio::sync::oneshot::Receiver<io::Result<std::process::ExitStatus>>>,
}

impl BackendBody {
    fn begin_wait(&mut self) {
        if self.exit.is_none()
            && let Some(mut child) = self.child.take()
        {
            let (sender, receiver) = tokio::sync::oneshot::channel();
            tokio::spawn(async move {
                let _ = sender.send(child.wait().await);
            });
            self.exit = Some(receiver);
        }
    }
}

impl Drop for BackendBody {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            tokio::spawn(async move {
                let _ = child.kill().await;
                let _ = child.wait().await;
            });
        }
    }
}

impl AsyncRead for BackendBody {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if (self.prefix.position() as usize) < self.prefix.get_ref().len() {
            return Pin::new(&mut self.prefix).poll_read(cx, buf);
        }
        if let Some(exit) = self.exit.as_mut() {
            return match Pin::new(exit).poll(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(Ok(Ok(status))) if status.success() => Poll::Ready(Ok(())),
                Poll::Ready(Ok(Ok(status))) => Poll::Ready(Err(io::Error::other(format!(
                    "git-http-backend exited with {status}"
                )))),
                Poll::Ready(Ok(Err(error))) => Poll::Ready(Err(error)),
                Poll::Ready(Err(_)) => Poll::Ready(Err(io::Error::other(
                    "git-http-backend wait task cancelled",
                ))),
            };
        }
        let before = buf.filled().len();
        match Pin::new(&mut self.stdout).poll_read(cx, buf) {
            Poll::Ready(Ok(())) if buf.filled().len() == before => {
                self.begin_wait();
                cx.waker().wake_by_ref();
                Poll::Pending
            }
            result => result,
        }
    }
}

impl HttpBackend {
    pub fn new(
        program: impl Into<PathBuf>,
        current_dir: impl Into<PathBuf>,
        server_addr: SocketAddr,
        max_request_bytes: u64,
        max_response_header_bytes: usize,
    ) -> Self {
        Self {
            program: program.into(),
            current_dir: current_dir.into(),
            environment: vec![],
            server_addr,
            max_request_bytes,
            max_response_header_bytes,
        }
    }

    pub fn env(mut self, name: impl Into<OsString>, value: impl Into<OsString>) -> Self {
        self.environment.push((name.into(), value.into()));
        self
    }

    pub async fn execute(
        &self,
        mut request: BackendRequest,
    ) -> Result<BackendResponse, BackendError> {
        let path = percent_encoding::percent_decode_str(&request.path).collect::<Vec<_>>();
        if path.contains(&0) {
            return Err(BackendError::InvalidResponse);
        }
        let path = <OsString as std::os::unix::ffi::OsStringExt>::from_vec(path);
        let remote = request
            .remote_addr
            .unwrap_or_else(|| SocketAddr::from(([0, 0, 0, 0], 0)));
        let header = |name: &str| {
            request
                .headers
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(name))
                .map(|(_, v)| v.as_str())
        };
        let host = header("host").unwrap_or("localhost");
        let content_type = header("content-type").unwrap_or("");
        let content_length = header("content-length").unwrap_or("");
        let uri = request
            .query
            .as_ref()
            .map_or_else(|| request.path.clone(), |q| format!("{}?{q}", request.path));
        let mut cmd = tokio::process::Command::new(&self.program);
        cmd.kill_on_drop(true);
        cmd.env_clear()
            .envs(self.environment.iter().cloned())
            .env("GATEWAY_INTERFACE", "CGI/1.1")
            .env("SERVER_PROTOCOL", "HTTP/1.1")
            .env("REQUEST_METHOD", &request.method)
            .env("SCRIPT_FILENAME", &self.program)
            .env("SCRIPT_NAME", "")
            .env("PATH_INFO", path)
            .env("QUERY_STRING", request.query.unwrap_or_default())
            .env("REQUEST_URI", uri)
            .env("REMOTE_ADDR", remote.ip().to_string())
            .env("REMOTE_PORT", remote.port().to_string())
            .env("SERVER_ADDR", self.server_addr.ip().to_string())
            .env("SERVER_PORT", self.server_addr.port().to_string())
            .env("SERVER_NAME", host)
            .env("CONTENT_LENGTH", content_length)
            .env("CONTENT_TYPE", content_type)
            .env("HTTP_HOST", host)
            .current_dir(&self.current_dir)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit());
        for (n, v) in &request.headers {
            if !["host", "content-length", "content-type", "proxy"]
                .iter()
                .any(|x| n.eq_ignore_ascii_case(x))
            {
                cmd.env(
                    format!("HTTP_{}", n.to_ascii_uppercase().replace('-', "_")),
                    v,
                );
            }
        }
        cmd.envs(request.environment);
        let mut child = cmd.spawn().map_err(|e| BackendError::Io(e.to_string()))?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| BackendError::Io("stdin unavailable".into()))?;
        let mut total = 0;
        let mut chunk = [0; 8192];
        loop {
            let read = match request.body.read(&mut chunk).await {
                Ok(read) => read,
                Err(error) => {
                    reap(&mut child).await;
                    return Err(BackendError::Io(error.to_string()));
                }
            };
            if read == 0 {
                break;
            }
            total += read as u64;
            if total > self.max_request_bytes {
                reap(&mut child).await;
                return Err(BackendError::InputTooLarge);
            }
            if let Err(error) = stdin.write_all(&chunk[..read]).await {
                reap(&mut child).await;
                return Err(BackendError::Io(error.to_string()));
            }
        }
        if let Err(error) = stdin.shutdown().await {
            reap(&mut child).await;
            return Err(BackendError::Io(error.to_string()));
        }
        drop(stdin);
        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| BackendError::Io("stdout unavailable".into()))?;
        let (status, headers, prefix) =
            match parse_headers(&mut stdout, self.max_response_header_bytes).await {
                Ok(value) => value,
                Err(error) => {
                    drop(stdout);
                    reap(&mut child).await;
                    return Err(error);
                }
            };
        Ok(BackendResponse {
            status,
            headers,
            body: BackendBody {
                prefix: io::Cursor::new(prefix),
                stdout,
                child: Some(child),
                exit: None,
            },
        })
    }
}

async fn reap(child: &mut tokio::process::Child) {
    let _ = child.kill().await;
    let _ = child.wait().await;
}

pub async fn parse_headers(
    reader: &mut (impl AsyncRead + Unpin),
    max_bytes: usize,
) -> Result<(u16, Vec<(String, String)>, Vec<u8>), BackendError> {
    let mut bytes = Vec::new();
    let mut part = [0; 1024];
    loop {
        let n = reader
            .read(&mut part)
            .await
            .map_err(|e| BackendError::Io(e.to_string()))?;
        if n == 0 {
            return Err(BackendError::InvalidResponse);
        }
        bytes.extend_from_slice(&part[..n]);
        if let Some(i) = bytes.windows(4).position(|x| x == b"\r\n\r\n") {
            return parse_head(&bytes[..i], bytes[i + 4..].to_vec());
        }
        if let Some(i) = bytes.windows(2).position(|x| x == b"\n\n") {
            return parse_head(&bytes[..i], bytes[i + 2..].to_vec());
        }
        if bytes.len() > max_bytes {
            return Err(BackendError::InvalidResponse);
        }
    }
}

fn parse_head(
    head: &[u8],
    body: Vec<u8>,
) -> Result<(u16, Vec<(String, String)>, Vec<u8>), BackendError> {
    let mut status = 200;
    let mut headers: Vec<(String, String)> = vec![];
    for line in head.split(|b| *b == b'\n') {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        let Some(i) = line.iter().position(|b| *b == b':') else {
            return Err(BackendError::InvalidResponse);
        };
        let name = std::str::from_utf8(&line[..i]).map_err(|_| BackendError::InvalidResponse)?;
        let value = std::str::from_utf8(&line[i + 1..])
            .map_err(|_| BackendError::InvalidResponse)?
            .trim_start();
        if name.eq_ignore_ascii_case("status") {
            status = value
                .split(' ')
                .next()
                .and_then(|x| x.parse::<u16>().ok())
                .filter(|status| (100..600).contains(status))
                .ok_or(BackendError::InvalidResponse)?
        } else {
            headers.push((name.into(), value.into()))
        }
    }
    if status == 200
        && headers
            .iter()
            .any(|(n, _)| n.eq_ignore_ascii_case("location"))
    {
        status = 302
    }
    Ok((status, headers, body))
}

#[cfg(test)]
mod tests {
    fn request() -> super::BackendRequest {
        super::BackendRequest {
            method: "GET".into(),
            path: "/x".into(),
            query: None,
            headers: vec![],
            body: Box::pin(std::io::Cursor::new(Vec::new())),
            remote_addr: None,
            environment: vec![],
        }
    }

    fn shell_request(script: &str) -> super::BackendRequest {
        super::BackendRequest {
            body: Box::pin(std::io::Cursor::new(script.as_bytes().to_vec())),
            ..request()
        }
    }

    #[tokio::test]
    async fn streams_body_and_waits_for_success() {
        use tokio::io::AsyncReadExt;
        let backend = super::HttpBackend::new(
            "/bin/sh",
            "/",
            std::net::SocketAddr::from(([127, 0, 0, 1], 80)),
            1024 * 1024,
            64 * 1024,
        );
        let script = "printf 'Content-Type: text/plain\\r\\n\\r\\nhello'";
        let mut response = backend.execute(shell_request(script)).await.unwrap();
        let mut body = Vec::new();
        response.body.read_to_end(&mut body).await.unwrap();
        assert_eq!(body, b"hello");
    }

    #[tokio::test]
    async fn nonzero_exit_is_stream_error() {
        use tokio::io::AsyncReadExt;
        let backend = super::HttpBackend::new(
            "/bin/sh",
            "/",
            std::net::SocketAddr::from(([127, 0, 0, 1], 80)),
            1024 * 1024,
            64 * 1024,
        );
        let script = "printf 'Content-Type: text/plain\\r\\n\\r\\nhello'\nexit 3";
        let mut response = backend.execute(shell_request(script)).await.unwrap();
        let mut body = Vec::new();
        assert!(response.body.read_to_end(&mut body).await.is_err());
    }

    #[tokio::test]
    async fn enforces_configured_request_limit() {
        let backend = super::HttpBackend::new(
            "/bin/sh",
            "/",
            std::net::SocketAddr::from(([127, 0, 0, 1], 80)),
            1,
            64 * 1024,
        );
        assert!(matches!(
            backend.execute(shell_request("xx")).await,
            Err(super::BackendError::InputTooLarge)
        ));
    }

    #[tokio::test]
    async fn parses_incremental_headers() {
        let mut input = &b"Status: 201 Created\r\nX-A: b\r\n\r\nbody"[..];
        let (s, h, b) = super::parse_headers(&mut input, 64 * 1024).await.unwrap();
        assert_eq!(s, 201);
        assert_eq!(h[0], ("X-A".into(), "b".into()));
        assert_eq!(b, b"body");
    }
}
