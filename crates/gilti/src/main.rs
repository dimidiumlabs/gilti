// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

mod cgi;
mod ui;

const DEFAULT_LISTEN_ADDR: &str = "0.0.0.0:8080";
const DEFAULT_CACHE_SECONDS: &str = "5";
const DEFAULT_ROOT_TITLE: &str = "Gilti";
const DEFAULT_ROOT_DESCRIPTION: &str = "A tiny Git server";
const MAX_CACHE_SECONDS: u64 = 3600;

const CGIT: &str = "/usr/local/bin/gilti-cgit";
const GIT_HOME: &str = "/var/lib/gilti/git";

const CGIT_CSS: &str = "/usr/share/webapps/cgit/cgit.css";
const CGIT_JS: &str = "/usr/share/webapps/cgit/cgit.js";
const CGIT_LOGO: &str = "/usr/share/webapps/cgit/cgit.png";
const CGIT_FAVICON: &str = "/usr/share/webapps/cgit/favicon.ico";

const CGIT_ENVIRONMENT: &[(&str, &str)] = &[
    ("CGIT_AGEFILE", "info/web/last-modified"),
    ("CGIT_BRANCH_SORT", "0"),
    ("CGIT_CASE_SENSITIVE_SORT", "1"),
    ("CGIT_CLONE_URL", ""),
    ("CGIT_COMMIT_SORT", "0"),
    ("CGIT_CSS", "/cgit.css"),
    ("CGIT_DIFFTYPE", "0"),
    ("CGIT_EMBEDDED", "0"),
    ("CGIT_ENABLE_BLAME", "0"),
    ("CGIT_ENABLE_COMMIT_GRAPH", "1"),
    ("CGIT_ENABLE_FOLLOW_LINKS", "0"),
    ("CGIT_ENABLE_HTML_SERVING", "0"),
    ("CGIT_ENABLE_HTTP_CLONE", "0"),
    ("CGIT_ENABLE_INDEX_LINKS", "1"),
    ("CGIT_ENABLE_INDEX_OWNER", "1"),
    ("CGIT_ENABLE_LOG_FILECOUNT", "1"),
    ("CGIT_ENABLE_LOG_LINECOUNT", "1"),
    ("CGIT_ENABLE_REMOTE_BRANCHES", "0"),
    ("CGIT_ENABLE_SUBJECT_LINKS", "0"),
    ("CGIT_ENABLE_TREE_LINENUMBERS", "1"),
    ("CGIT_FAVICON", "/favicon.ico"),
    ("CGIT_FOOTER", ""),
    ("CGIT_HEADER", ""),
    ("CGIT_HEAD_INCLUDE", ""),
    ("CGIT_JS", ""),
    ("CGIT_LOCAL_TIME", "0"),
    ("CGIT_LOGO", "/cgit.png"),
    ("CGIT_LOGO_LINK", ""),
    ("CGIT_MAX_ATOM_ITEMS", "10"),
    ("CGIT_MAX_BLOB_SIZE", "0"),
    ("CGIT_MAX_COMMIT_COUNT", "50"),
    ("CGIT_MAX_MESSAGE_LENGTH", "80"),
    ("CGIT_MAX_REPO_COUNT", "50"),
    ("CGIT_MAX_REPODESC_LENGTH", "80"),
    ("CGIT_MAX_STATS", "0"),
    ("CGIT_MIMETYPE_FILE", ""),
    ("CGIT_MODULE_LINK", ""),
    ("CGIT_NOHEADER", "0"),
    ("CGIT_NOPLAINEMAIL", "0"),
    ("CGIT_README_0", ":README.md"),
    ("CGIT_README_1", ":README"),
    ("CGIT_REMOVE_SUFFIX", "1"),
    ("CGIT_REPO_DEFAULT_DESC", "[no description]"),
    ("CGIT_RENAMELIMIT", "-1"),
    ("CGIT_REPOSITORY_SORT", "name"),
    ("CGIT_ROBOTS", "index, nofollow"),
    ("CGIT_ROOT_README", ""),
    ("CGIT_SCAN_HIDDEN_PATH", "0"),
    ("CGIT_SCAN_PATH", "/var/lib/gilti/git/repositories"),
    ("CGIT_SECTION", ""),
    ("CGIT_SECTION_FROM_PATH", "0"),
    ("CGIT_SECTION_SORT", "1"),
    ("CGIT_SNAPSHOTS", "0"),
    ("CGIT_STRICT_EXPORT", ""),
    ("CGIT_SUMMARY_BRANCHES", "10"),
    ("CGIT_SUMMARY_LOG", "10"),
    ("CGIT_SUMMARY_TAGS", "10"),
    ("CGIT_VIRTUAL_ROOT", "/"),
    ("GIT_ATTR_NOSYSTEM", "1"),
    ("GIT_CONFIG_NOSYSTEM", "1"),
];

#[derive(Clone)]
struct AppState {
    cgit: cgi::Cgi,
}

struct Config {
    listen_addr: std::net::SocketAddr,
    cache: std::time::Duration,
    root_title: String,
    root_description: String,
    clone_prefix: String,
}

impl Config {
    fn from_environment() -> std::io::Result<Self> {
        let listen_addr = environment("GILTI_HTTP_ADDR", DEFAULT_LISTEN_ADDR)?
            .parse()
            .map_err(|_| invalid_config("GILTI_HTTP_ADDR must be a socket address"))?;
        let cache = parse_cache(&environment("GILTI_CGIT_CACHE", DEFAULT_CACHE_SECONDS)?)?;
        Ok(Self {
            listen_addr,
            cache,
            root_title: environment("GILTI_CGIT_ROOT_TITLE", DEFAULT_ROOT_TITLE)?,
            root_description: environment("GILTI_CGIT_ROOT_DESCRIPTION", DEFAULT_ROOT_DESCRIPTION)?,
            clone_prefix: environment("GILTI_CGIT_CLONE_PREFIX", "")?,
        })
    }
}

fn environment(name: &str, default: &str) -> std::io::Result<String> {
    match std::env::var(name) {
        Ok(value) => Ok(value),
        Err(std::env::VarError::NotPresent) => Ok(default.to_owned()),
        Err(std::env::VarError::NotUnicode(_)) => {
            Err(invalid_config(format!("{name} must be valid UTF-8")))
        }
    }
}

fn parse_cache(value: &str) -> std::io::Result<std::time::Duration> {
    let seconds = value
        .parse::<u64>()
        .map_err(|_| invalid_config("GILTI_CGIT_CACHE must be an integer number of seconds"))?;
    if seconds > MAX_CACHE_SECONDS {
        return Err(invalid_config(format!(
            "GILTI_CGIT_CACHE must not exceed {MAX_CACHE_SECONDS} seconds"
        )));
    }
    Ok(std::time::Duration::from_secs(seconds))
}

fn invalid_config(message: impl Into<String>) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message.into())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_environment()?;
    let listen_addr = config.listen_addr;
    check_files()?;

    if std::env::args().nth(1).as_deref() == Some("--check") {
        return Ok(());
    }

    let mut cgit = cgi::Cgi::new(CGIT, GIT_HOME, config.listen_addr)
        .cache(config.cache)
        .env("CGIT_ROOT_TITLE", config.root_title)
        .env("CGIT_ROOT_DESC", config.root_description)
        .env("CGIT_CLONE_PREFIX", config.clone_prefix)
        .env("PATH", "/usr/bin:/bin");
    for (name, value) in CGIT_ENVIRONMENT {
        cgit = cgit.env(*name, *value);
    }
    let state = AppState { cgit };
    let app = axum::Router::new()
        .route(
            "/healthz",
            axum::routing::get(async || plain_response(axum::http::StatusCode::OK, "ok\n")),
        )
        .route(
            "/cgit.css",
            axum::routing::get(async || static_file(CGIT_CSS, "text/css")),
        )
        .route(
            "/cgit.js",
            axum::routing::get(async || static_file(CGIT_JS, "text/javascript")),
        )
        .route(
            "/cgit.png",
            axum::routing::get(async || static_file(CGIT_LOGO, "image/png")),
        )
        .route(
            "/favicon.ico",
            axum::routing::get(async || static_file(CGIT_FAVICON, "image/x-icon")),
        )
        .fallback(proxy_to_cgit)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(listen_addr).await?;
    eprintln!("gilti: listening on {listen_addr}");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;

    Ok(())
}

fn check_files() -> std::io::Result<()> {
    let metadata = std::fs::metadata(CGIT)?;
    if !metadata.is_file()
        || std::os::unix::fs::PermissionsExt::mode(&metadata.permissions()) & 0o111 == 0
    {
        return Err(std::io::Error::other(format!("{CGIT} is not executable")));
    }
    for path in [CGIT_CSS, CGIT_JS, CGIT_LOGO, CGIT_FAVICON] {
        if !std::fs::metadata(path)?.is_file() {
            return Err(std::io::Error::other(format!(
                "{path} is not a regular file"
            )));
        }
    }
    Ok(())
}

async fn shutdown_signal() {
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("install SIGTERM handler");
    let mut interrupt = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
        .expect("install SIGINT handler");
    tokio::select! {
        _ = terminate.recv() => {}
        _ = interrupt.recv() => {}
    }
}

fn static_file(path: &str, content_type: &'static str) -> axum::response::Response {
    match std::fs::read(path) {
        Ok(bytes) => response(axum::http::StatusCode::OK, content_type, bytes),
        Err(error) => {
            eprintln!("gilti: cannot read {path}: {error}");
            plain_response(axum::http::StatusCode::NOT_FOUND, "not found\n")
        }
    }
}

async fn proxy_to_cgit(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    mut request: axum::extract::Request,
) -> axum::response::Response {
    if request.method() != axum::http::Method::GET && request.method() != axum::http::Method::HEAD {
        return plain_response(axum::http::StatusCode::FORBIDDEN, "forbidden\n");
    }
    request.extensions_mut().insert(cgi::RemoteAddr(remote));

    let method = request.method().clone();
    match tower::ServiceExt::oneshot(state.cgit.clone(), request).await {
        Ok(response) => render_private_page(method, response).await,
        Err(error) => {
            eprintln!("gilti: cgit request failed: {error}");
            plain_response(axum::http::StatusCode::BAD_GATEWAY, "bad gateway\n")
        }
    }
}

async fn render_private_page(
    method: axum::http::Method,
    response: axum::response::Response,
) -> axum::response::Response {
    let private_page = response
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .is_some_and(|value| value.as_bytes() == ui::PRIVATE_CONTENT_TYPE.as_bytes());
    if !private_page {
        return response;
    }

    let (mut parts, body) = response.into_parts();
    parts.headers.insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("text/html; charset=UTF-8"),
    );
    parts.headers.remove(axum::http::header::CONTENT_LENGTH);
    if method == axum::http::Method::HEAD {
        return axum::http::response::Response::from_parts(parts, axum::body::Body::empty());
    }
    let body = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(body) => body,
        Err(error) => {
            eprintln!("gilti: cannot read private page response: {error}");
            return plain_response(axum::http::StatusCode::BAD_GATEWAY, "bad gateway\n");
        }
    };
    match ui::render(&body) {
        Ok(markup) => axum::http::response::Response::from_parts(
            parts,
            axum::body::Body::from(markup.into_string()),
        ),
        Err(error) => {
            eprintln!("gilti: invalid private page response: {error}");
            plain_response(axum::http::StatusCode::BAD_GATEWAY, "bad gateway\n")
        }
    }
}

fn response(
    status: axum::http::StatusCode,
    content_type: &'static str,
    body: Vec<u8>,
) -> axum::response::Response {
    axum::response::Response::builder()
        .status(status)
        .header(axum::http::header::CONTENT_TYPE, content_type)
        .body(axum::body::Body::from(body))
        .expect("static response is valid")
}

fn plain_response(
    status: axum::http::StatusCode,
    message: &'static str,
) -> axum::response::Response {
    response(status, "text/plain", message.as_bytes().to_vec())
}

#[cfg(test)]
mod tests {
    #[test]
    fn cache_is_bounded() {
        assert_eq!(
            super::parse_cache("5").unwrap(),
            std::time::Duration::from_secs(5)
        );
        assert!(super::parse_cache("0").unwrap().is_zero());
        assert!(super::parse_cache("3601").is_err());
        assert!(super::parse_cache("forever").is_err());
    }

    #[tokio::test]
    async fn private_pages_become_html_and_head_keeps_headers() {
        let model = br#"{"page":"repolist","title":"Gilti","root_desc":"","root_url":"/","about_url":"/?p=about","noheader":true,"search":"","current_url":"/","root_readme":false,"owner_enabled":false,"links_enabled":false,"section_grouping":false,"shell":{"embedded":false,"robots":"","css":[],"js":[],"favicon":"","head_include":null,"header":null,"footer_configured":false,"footer":null,"logo":"","logo_link":"","cgit_version":"v1","git_version":"2","generated_at":"now"},"sort_urls":{"name":"/?s=name","desc":"/?s=desc","owner":"/?s=owner","idle":"/?s=idle"},"rows":[],"pager":[]}"#;
        let response = axum::http::Response::builder()
            .status(axum::http::StatusCode::OK)
            .header(
                axum::http::header::CONTENT_TYPE,
                super::ui::PRIVATE_CONTENT_TYPE,
            )
            .header(axum::http::header::CACHE_CONTROL, "max-age=60")
            .header(axum::http::header::CONTENT_LENGTH, model.len())
            .body(axum::body::Body::from(model.as_slice()))
            .unwrap();
        let response = super::render_private_page(axum::http::Method::GET, response).await;
        assert_eq!(
            response.headers()[axum::http::header::CONTENT_TYPE],
            "text/html; charset=UTF-8"
        );
        assert_eq!(
            response.headers()[axum::http::header::CACHE_CONTROL],
            "max-age=60"
        );
        assert!(
            !response
                .headers()
                .contains_key(axum::http::header::CONTENT_LENGTH)
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert!(
            std::str::from_utf8(&body)
                .unwrap()
                .contains("repository list")
        );

        let response = axum::http::Response::builder()
            .header(
                axum::http::header::CONTENT_TYPE,
                super::ui::PRIVATE_CONTENT_TYPE,
            )
            .header(axum::http::header::CACHE_CONTROL, "max-age=60")
            .body(axum::body::Body::empty())
            .unwrap();
        let response = super::render_private_page(axum::http::Method::HEAD, response).await;
        assert_eq!(
            response.headers()[axum::http::header::CONTENT_TYPE],
            "text/html; charset=UTF-8"
        );
        assert!(
            axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn malformed_private_page_fails_closed() {
        let response = axum::http::Response::builder()
            .header(
                axum::http::header::CONTENT_TYPE,
                super::ui::PRIVATE_CONTENT_TYPE,
            )
            .body(axum::body::Body::from("not json"))
            .unwrap();
        assert_eq!(
            super::render_private_page(axum::http::Method::GET, response)
                .await
                .status(),
            axum::http::StatusCode::BAD_GATEWAY
        );
    }
}
