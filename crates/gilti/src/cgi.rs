// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

//! HTTP-to-CGI Tower service.

const CACHE_MAX_ENTRIES: usize = 64;
const CACHE_MAX_BYTES: usize = 4 * 1024 * 1024;

fn has_cache_directive(headers: &axum::http::HeaderMap, expected: &[&str]) -> bool {
    headers
        .get_all(axum::http::header::CACHE_CONTROL)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .any(|value| {
            let name = value.split_once('=').map_or(value, |(name, _)| name);
            expected
                .iter()
                .any(|expected| name.eq_ignore_ascii_case(expected))
        })
}

#[derive(Clone, Eq, Hash, PartialEq)]
struct CacheKey {
    uri: String,
    host: Option<Vec<u8>>,
    cookie: Option<Vec<u8>>,
    referer: Option<Vec<u8>>,
}

impl CacheKey {
    fn from_request(parts: &axum::http::request::Parts, body: &[u8]) -> Option<Self> {
        if parts.method != axum::http::Method::GET
            || !body.is_empty()
            || parts
                .headers
                .contains_key(axum::http::header::AUTHORIZATION)
            || parts.headers.contains_key(axum::http::header::RANGE)
            || has_cache_directive(&parts.headers, &["no-cache", "no-store"])
            || parts
                .headers
                .get(axum::http::header::PRAGMA)
                .and_then(|value| value.to_str().ok())
                .is_some_and(|value| value.eq_ignore_ascii_case("no-cache"))
        {
            return None;
        }
        let header = |name| {
            parts
                .headers
                .get(name)
                .map(|value| value.as_bytes().to_vec())
        };
        Some(Self {
            uri: parts.uri.to_string(),
            host: header(axum::http::header::HOST),
            cookie: header(axum::http::header::COOKIE),
            referer: header(axum::http::header::REFERER),
        })
    }
}

#[derive(Clone)]
struct CachedResponse {
    status: axum::http::StatusCode,
    headers: axum::http::HeaderMap,
    body: Vec<u8>,
}

impl CachedResponse {
    fn is_cacheable(&self) -> bool {
        if self.status != axum::http::StatusCode::OK
            || self.headers.contains_key(axum::http::header::SET_COOKIE)
            || self.headers.contains_key(axum::http::header::VARY)
        {
            return false;
        }
        !has_cache_directive(&self.headers, &["no-cache", "no-store", "private"])
    }

    fn into_response(self) -> std::io::Result<axum::http::Response<axum::body::Body>> {
        let mut response = axum::http::Response::builder()
            .status(self.status)
            .body(axum::body::Body::from(self.body))
            .map_err(std::io::Error::other)?;
        *response.headers_mut() = self.headers;
        Ok(response)
    }
}

struct CacheEntry {
    response: CachedResponse,
    expires: std::time::Instant,
}

#[derive(Default)]
struct CacheState {
    entries: std::collections::VecDeque<(CacheKey, CacheEntry)>,
    bytes: usize,
}

impl CacheState {
    fn prune_expired(&mut self, now: std::time::Instant) {
        let mut bytes = 0;
        self.entries.retain(|(_, entry)| {
            if entry.expires <= now {
                false
            } else {
                bytes += entry.response.body.len();
                true
            }
        });
        self.bytes = bytes;
    }
}

#[derive(Clone)]
struct ResponseCache {
    ttl: std::time::Duration,
    state: std::sync::Arc<std::sync::Mutex<CacheState>>,
}

impl ResponseCache {
    fn new(ttl: std::time::Duration) -> Self {
        Self {
            ttl,
            state: std::sync::Arc::new(std::sync::Mutex::new(CacheState::default())),
        }
    }

    fn get(&self, key: &CacheKey) -> Option<CachedResponse> {
        self.get_at(key, std::time::Instant::now())
    }

    fn get_at(&self, key: &CacheKey, now: std::time::Instant) -> Option<CachedResponse> {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state.prune_expired(now);
        let index = state
            .entries
            .iter()
            .position(|(entry_key, _)| entry_key == key)?;
        let entry = state.entries.remove(index)?;
        let response = entry.1.response.clone();
        state.entries.push_back(entry);
        Some(response)
    }

    fn insert(&self, key: CacheKey, response: CachedResponse) {
        self.insert_at(key, response, std::time::Instant::now());
    }

    fn insert_at(&self, key: CacheKey, response: CachedResponse, now: std::time::Instant) {
        let size = response.body.len();
        if !response.is_cacheable() || size > CACHE_MAX_BYTES {
            return;
        }

        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state.prune_expired(now);
        let previous = state
            .entries
            .iter()
            .position(|(entry_key, _)| entry_key == &key)
            .and_then(|index| state.entries.remove(index));
        if let Some(previous) = previous {
            state.bytes -= previous.1.response.body.len();
        }
        while state.entries.len() >= CACHE_MAX_ENTRIES
            || state.bytes.saturating_add(size) > CACHE_MAX_BYTES
        {
            let Some(previous) = state.entries.pop_front() else {
                break;
            };
            state.bytes -= previous.1.response.body.len();
        }
        state.bytes += size;
        state.entries.push_back((
            key,
            CacheEntry {
                response,
                expires: now + self.ttl,
            },
        ));
    }
}

#[derive(Clone, Default)]
pub struct Environment(pub Vec<(std::ffi::OsString, std::ffi::OsString)>);

#[derive(Clone, Copy)]
pub struct NoCache;

#[derive(Clone, Copy)]
pub struct RemoteAddr(pub std::net::SocketAddr);

#[derive(Clone)]
pub struct Cgi {
    program: std::path::PathBuf,
    current_dir: std::path::PathBuf,
    environment: Vec<(std::ffi::OsString, std::ffi::OsString)>,
    server_addr: std::net::SocketAddr,
    cache: Option<ResponseCache>,
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
            cache: None,
        }
    }

    pub fn cache(mut self, ttl: std::time::Duration) -> Self {
        if !ttl.is_zero() {
            self.cache = Some(ResponseCache::new(ttl));
        }
        self
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
        let body = axum::body::to_bytes(body, 1024 * 1024 * 1024)
            .await
            .map_err(std::io::Error::other)?;
        let cache_key = parts
            .extensions
            .get::<NoCache>()
            .is_none()
            .then(|| CacheKey::from_request(&parts, &body))
            .flatten();
        if let Some(response) = self
            .cache
            .as_ref()
            .zip(cache_key.as_ref())
            .and_then(|(cache, key)| cache.get(key))
        {
            return response.into_response();
        }
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

        if let Some(environment) = parts.extensions.get::<Environment>() {
            command.envs(environment.0.iter().cloned());
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
        let response = CachedResponse {
            status,
            headers,
            body: body.to_vec(),
        };
        if let (Some(cache), Some(key)) = (&self.cache, cache_key) {
            cache.insert(key, response.clone());
        }
        response.into_response()
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
    fn cache_key(uri: impl Into<String>) -> super::CacheKey {
        super::CacheKey {
            uri: uri.into(),
            host: None,
            cookie: None,
            referer: None,
        }
    }

    fn cached_response(body: impl Into<Vec<u8>>) -> super::CachedResponse {
        super::CachedResponse {
            status: axum::http::StatusCode::OK,
            headers: axum::http::HeaderMap::new(),
            body: body.into(),
        }
    }

    #[test]
    fn memory_cache_expires_entries() {
        let cache = super::ResponseCache::new(std::time::Duration::from_secs(5));
        let now = std::time::Instant::now();
        cache.insert_at(cache_key("/repo/"), cached_response(b"first".to_vec()), now);
        assert_eq!(
            cache.get_at(&cache_key("/repo/"), now).unwrap().body,
            b"first"
        );
        assert!(
            cache
                .get_at(
                    &cache_key("/repo/"),
                    now + std::time::Duration::from_secs(5)
                )
                .is_none()
        );
    }

    #[test]
    fn memory_cache_is_bounded_and_honors_response_directives() {
        let cache = super::ResponseCache::new(std::time::Duration::from_secs(5));
        let now = std::time::Instant::now();
        for index in 0..super::CACHE_MAX_ENTRIES {
            cache.insert_at(
                cache_key(format!("/{index}")),
                cached_response(vec![index as u8]),
                now,
            );
        }
        assert!(cache.get_at(&cache_key("/0"), now).is_some());
        cache.insert_at(cache_key("/new"), cached_response(b"new".to_vec()), now);
        assert!(cache.get_at(&cache_key("/1"), now).is_none());
        assert!(cache.get_at(&cache_key("/0"), now).is_some());

        let mut private = cached_response(b"private".to_vec());
        private.headers.insert(
            axum::http::header::CACHE_CONTROL,
            axum::http::HeaderValue::from_static("private"),
        );
        cache.insert_at(cache_key("/private"), private, now);
        assert!(cache.get_at(&cache_key("/private"), now).is_none());
    }

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
