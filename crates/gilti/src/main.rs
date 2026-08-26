// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

mod cgi;

const DEFAULT_LISTEN_ADDR: &str = "0.0.0.0:8080";

const CGIT: &str = "/usr/local/bin/gilti-cgit";
const CGIT_CONFIG: &str = "/etc/cgitrc";
const GIT_HOME: &str = "/var/lib/gilti/git";
const RUN_DIR: &str = "/run/gilti/http";

const CGIT_CSS: &str = "/usr/share/webapps/cgit/cgit.css";
const CGIT_JS: &str = "/usr/share/webapps/cgit/cgit.js";
const CGIT_LOGO: &str = "/usr/share/webapps/cgit/cgit.png";
const CGIT_FAVICON: &str = "/usr/share/webapps/cgit/favicon.ico";

#[derive(Clone)]
struct AppState {
    cgit: cgi::Cgi,
}

struct CgitConfig {
    path: std::path::PathBuf,
}

impl CgitConfig {
    fn create() -> std::io::Result<Self> {
        let contents = std::fs::read(CGIT_CONFIG)?;
        let path = std::path::PathBuf::from(format!("{RUN_DIR}/cgitrc.{}", std::process::id()));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        let mut file = options.open(&path)?;
        let config = Self { path };
        std::io::Write::write_all(&mut file, &contents)?;
        Ok(config)
    }
}

impl Drop for CgitConfig {
    fn drop(&mut self) {
        match std::fs::remove_file(&self.path) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                eprintln!("gilti: cannot remove {}: {error}", self.path.display());
            }
            _ => {}
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listen_addr = std::env::var("GILTI_HTTP_ADDR")
        .unwrap_or_else(|_| DEFAULT_LISTEN_ADDR.to_owned())
        .parse::<std::net::SocketAddr>()?;
    check_files()?;

    if std::env::args().nth(1).as_deref() == Some("--check") {
        return Ok(());
    }

    let cgit_config = CgitConfig::create()?;
    let state = AppState {
        cgit: cgi::Cgi::new(CGIT, GIT_HOME, listen_addr)
            .env("CGIT_CONFIG", cgit_config.path.as_os_str())
            .env("HOME", GIT_HOME)
            .env("PATH", "/usr/bin:/bin"),
    };
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
    for path in [CGIT_CONFIG, CGIT_CSS, CGIT_JS, CGIT_LOGO, CGIT_FAVICON] {
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

    match tower::ServiceExt::oneshot(state.cgit.clone(), request).await {
        Ok(response) => response,
        Err(error) => {
            eprintln!("gilti: cgit request failed: {error}");
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
