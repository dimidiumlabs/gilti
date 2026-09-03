// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

mod components;
mod endpoints;
pub mod router;
mod styles;
mod urls;

mod licenses {
    /// Deterministic dependency and bundled-resource license report.
    pub const JSON: &str = include_str!(concat!(env!("OUT_DIR"), "/licenses.json"));
}

use dimidiumlabs_server::{
    HtmlCompressionPredicate, assets_router,
    service::{
        AdmissionLayer, ClientIpLayer, DrainLayer, ForwardedHeader, HtmlLayer, PeerAddr,
        TrustedProxies,
        compression::{CompressionLayer, CompressionLevel},
    },
    transport::{HttpTransport, TransportPolicyError},
};
use dimidiumlabs_ui::{AssetsCatalog, FOUNDATION};
use hyper_util::{rt::TokioIo, service::TowerToHyperService};

const DEFAULT_LISTEN_ADDR: &str = "0.0.0.0:8080";
const DEFAULT_ROOT_TITLE: &str = "Gilti";
const DEFAULT_ROOT_DESCRIPTION: &str = "A tiny Git server";
const HTTP_HEADER_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const HTTP1_MAX_BUFFER_BYTES: usize = 32 * 1024;
const HTTP2_MAX_CONCURRENT_STREAMS: u32 = 64;
const HTTP2_MAX_HEADER_LIST_BYTES: u32 = 16 * 1024;
const REQUEST_BODY_IDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);
const DYNAMIC_COMPRESSION_MIN_BYTES: u16 = 128;
const DYNAMIC_COMPRESSION_LEVEL: CompressionLevel = CompressionLevel::Precise(5);
const MAX_CONCURRENT_REQUESTS: usize = 64;
const MAX_QUEUED_REQUESTS: usize = 128;
const ADMISSION_WAIT: std::time::Duration = std::time::Duration::from_secs(1);
const SHUTDOWN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(25);
const GIT_HTTP_BACKENDS: &[&str] = &[
    "/usr/libexec/git-core/git-http-backend",
    "/usr/lib/git-core/git-http-backend",
];
const GIT_HOME: &str = "/var/lib/gilti/git";
const REPOSITORIES: &str = "/var/lib/gilti/git/repositories";
const ARCHIVE_COMPRESSORS: &[&str] = &[
    "/usr/bin/bzip2",
    "/usr/bin/lzip",
    "/usr/bin/xz",
    "/usr/bin/zstd",
];

static ASSETS: std::sync::LazyLock<std::sync::Arc<AssetsCatalog>> =
    std::sync::LazyLock::new(|| {
        std::sync::Arc::new(
            AssetsCatalog::new()
                .with(FOUNDATION)
                .expect("foundation assets are valid")
                .with(styles::APPLICATION)
                .expect("Gilti assets are valid and unique"),
        )
    });

#[derive(Clone)]
struct RepositoryService {
    git: gilti_git::backend::HttpBackend,
    context: endpoints::shared::Context,
    write_enabled: bool,
}

struct Config {
    listen_addr: std::net::SocketAddr,
    root_title: String,
    root_description: String,
    clone_prefix: String,
    http_write: bool,
}

impl Config {
    fn from_environment() -> std::io::Result<Self> {
        let listen_addr = environment("GILTI_HTTP_ADDR", DEFAULT_LISTEN_ADDR)?
            .parse()
            .map_err(|_| invalid_config("GILTI_HTTP_ADDR must be a socket address"))?;
        Ok(Self {
            listen_addr,
            root_title: environment("GILTI_ROOT_TITLE", DEFAULT_ROOT_TITLE)?,
            root_description: environment("GILTI_ROOT_DESCRIPTION", DEFAULT_ROOT_DESCRIPTION)?,
            clone_prefix: environment("GILTI_CLONE_PREFIX", "")?,
            http_write: parse_bool("GILTI_HTTP_WRITE", &environment("GILTI_HTTP_WRITE", "0")?)?,
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

fn parse_bool(name: &str, value: &str) -> std::io::Result<bool> {
    match value {
        "0" => Ok(false),
        "1" => Ok(true),
        _ => Err(invalid_config(format!("{name} must be 0 or 1"))),
    }
}

fn invalid_config(message: impl Into<String>) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message.into())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_environment()?;
    let listen_addr = config.listen_addr;
    let git_http_backend = git_http_backend()?;
    check_files(&git_http_backend)?;

    if std::env::args().nth(1).as_deref() == Some("--check") {
        return Ok(());
    }

    let context = endpoints::shared::Context {
        repositories: REPOSITORIES,
        root_title: std::sync::Arc::from(config.root_title.clone()),
        root_description: std::sync::Arc::from(config.root_description.clone()),
        clone_prefix: std::sync::Arc::from(config.clone_prefix.clone()),
    };
    let git = gilti_git::backend::HttpBackend::new(git_http_backend, GIT_HOME, config.listen_addr)
        .env("GIT_PROJECT_ROOT", REPOSITORIES)
        .env("GIT_HTTP_EXPORT_ALL", "1")
        .env("HOME", GIT_HOME)
        .env("USER", "git")
        .env("LOGNAME", "git")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("PATH", "/usr/bin:/bin");
    let repositories = RepositoryService {
        git,
        context,
        write_enabled: config.http_write,
    };
    let ui = axum::Router::new()
        .merge(assets_router::<()>(std::sync::Arc::clone(&ASSETS)))
        .route(
            "/-/licenses.json",
            axum::routing::get(async || {
                response(
                    axum::http::StatusCode::OK,
                    "application/json; charset=utf-8",
                    licenses::JSON.as_bytes().to_vec(),
                )
            }),
        );
    let browser = axum::Router::new()
        .route(
            "/-/about",
            axum::routing::get(async || {
                plain_response(axum::http::StatusCode::OK, "Gilti Git server\n")
            }),
        )
        .route(
            "/-/terms",
            axum::routing::get(async || {
                plain_response(axum::http::StatusCode::OK, "No additional terms of use.\n")
            }),
        )
        .fallback_service(repositories)
        .layer(HtmlLayer::new(&ASSETS).with_negotiated_compression())
        .layer(
            CompressionLayer::new()
                .quality(DYNAMIC_COMPRESSION_LEVEL)
                .compress_when(HtmlCompressionPredicate::new(DYNAMIC_COMPRESSION_MIN_BYTES)),
        );
    let app = browser.merge(ui);

    let (app, drain_handle, transport) = harden(app)?;
    // Keep the shared liveness/readiness endpoint independent from application
    // admission so overload does not cause Kubernetes restart loops.
    let app = app.route(
        "/-/health",
        axum::routing::get(async || {
            response(
                axum::http::StatusCode::OK,
                "application/json",
                b"{\"status\":\"ok\"}\n".to_vec(),
            )
        }),
    );
    let listener = tokio::net::TcpListener::bind(listen_addr).await?;
    let shutdown = tokio_util::sync::CancellationToken::new();
    eprintln!("gilti: listening on {listen_addr}");

    let server = serve(listener, app, transport, shutdown.clone());
    tokio::pin!(server);
    tokio::select! {
        result = &mut server => result?,
        () = shutdown_signal() => {
            let _ = drain_handle.begin();
            shutdown.cancel();
            let drained = tokio::time::timeout(SHUTDOWN_TIMEOUT, async {
                server.await?;
                drain_handle.wait().await;
                std::io::Result::Ok(())
            })
            .await;
            match drained {
                Ok(result) => result?,
                Err(_) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "HTTP shutdown exceeded its deadline",
                    ).into());
                }
            }
        }
    }

    Ok(())
}

fn git_http_backend() -> std::io::Result<std::path::PathBuf> {
    GIT_HTTP_BACKENDS
        .iter()
        .map(std::path::PathBuf::from)
        .find(|path| path.is_file())
        .ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "git-http-backend not found")
        })
}

fn harden(
    app: axum::Router,
) -> Result<
    (
        axum::Router,
        dimidiumlabs_server::service::DrainHandle,
        HttpTransport,
    ),
    TransportPolicyError,
> {
    let (drain_layer, drain_handle) = DrainLayer::new();
    let app = app
        .layer(
            dimidiumlabs_server::service::body::RequestBodyLimitLayer::new(
                usize::try_from(gilti_git::backend::MAX_REQUEST_SIZE)
                    .expect("request body limit fits usize"),
            ),
        )
        .layer(
            dimidiumlabs_server::service::timeout::RequestBodyTimeoutLayer::new(
                REQUEST_BODY_IDLE_TIMEOUT,
            ),
        )
        .layer(
            AdmissionLayer::new(
                std::num::NonZeroUsize::new(MAX_CONCURRENT_REQUESTS)
                    .expect("concurrency limit is non-zero"),
            )
            .with_wait(
                ADMISSION_WAIT,
                std::num::NonZeroUsize::new(MAX_QUEUED_REQUESTS).expect("queue limit is non-zero"),
            ),
        )
        .layer(ClientIpLayer::new(TrustedProxies::new(
            [],
            ForwardedHeader::XForwardedFor,
        )))
        .layer(drain_layer);
    let transport = HttpTransport::new(
        HTTP_HEADER_READ_TIMEOUT,
        HTTP1_MAX_BUFFER_BYTES,
        std::num::NonZeroU32::new(HTTP2_MAX_CONCURRENT_STREAMS)
            .expect("HTTP/2 stream limit is non-zero"),
        std::num::NonZeroU32::new(HTTP2_MAX_HEADER_LIST_BYTES)
            .expect("HTTP/2 header limit is non-zero"),
    )?;
    Ok((app, drain_handle, transport))
}

async fn serve(
    listener: tokio::net::TcpListener,
    app: axum::Router,
    transport: HttpTransport,
    shutdown: tokio_util::sync::CancellationToken,
) -> std::io::Result<()> {
    let mut connections = tokio::task::JoinSet::new();

    loop {
        tokio::select! {
            () = shutdown.cancelled() => break,
            accepted = listener.accept() => {
                let (stream, peer) = accepted?;
                let app = app
                    .clone()
                    .layer(axum::Extension(axum::extract::ConnectInfo(peer)))
                    .layer(axum::Extension(PeerAddr(peer)));
                let transport = transport.clone();
                let shutdown = shutdown.clone();
                connections.spawn(async move {
                    let builder = transport.builder();
                    let connection = builder.serve_connection_with_upgrades(
                        TokioIo::new(stream),
                        TowerToHyperService::new(app),
                    );
                    tokio::pin!(connection);
                    let result = tokio::select! {
                        result = &mut connection => result,
                        () = shutdown.cancelled() => {
                            connection.as_mut().graceful_shutdown();
                            connection.await
                        }
                    };
                    if let Err(error) = result {
                        eprintln!("gilti: HTTP connection failed: {error}");
                    }
                });
            }
            Some(result) = connections.join_next(), if !connections.is_empty() => {
                if let Err(error) = result {
                    eprintln!("gilti: HTTP connection task failed: {error}");
                }
            }
        }
    }

    while let Some(result) = connections.join_next().await {
        if let Err(error) = result {
            eprintln!("gilti: HTTP connection task failed: {error}");
        }
    }
    Ok(())
}

fn check_files(git_http_backend: &std::path::Path) -> std::io::Result<()> {
    for path in std::iter::once(git_http_backend)
        .chain(std::iter::once(std::path::Path::new(gilti_git::GIT)))
        .chain(ARCHIVE_COMPRESSORS.iter().map(std::path::Path::new))
    {
        let metadata = std::fs::metadata(path)?;
        if !metadata.is_file()
            || std::os::unix::fs::PermissionsExt::mode(&metadata.permissions()) & 0o111 == 0
        {
            return Err(std::io::Error::other(format!(
                "{} is not executable",
                path.display()
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

impl tower::Service<axum::extract::Request> for RepositoryService {
    type Response = axum::response::Response;
    type Error = std::convert::Infallible;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;

    fn poll_ready(
        &mut self,
        _context: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: axum::extract::Request) -> Self::Future {
        let service = self.clone();
        Box::pin(async move { Ok(service.execute(request).await) })
    }
}

impl RepositoryService {
    async fn execute(&self, request: axum::extract::Request) -> axum::response::Response {
        let route = match router::parse(request.uri().path()) {
            Ok(route) => route,
            Err(_) => return plain_response(axum::http::StatusCode::NOT_FOUND, "not found\n"),
        };
        let browser_view = matches!(
            &route,
            router::Route::Repositories
                | router::Route::Overview(_)
                | router::Route::About(_)
                | router::Route::Stats(_)
                | router::Route::Object(_)
                | router::Route::Refs(_)
                | router::Route::Revision(_)
                | router::Route::Tree(_)
                | router::Route::Blame(_)
                | router::Route::Archive(_)
                | router::Route::ArchiveSignature(_)
                | router::Route::Diff(_)
                | router::Route::Patch(_)
                | router::Route::Log(_)
                | router::Route::AtomFeed(_)
        );
        let query = if browser_view {
            let query = match request_query(request.uri().query()) {
                Ok(query) => query,
                Err(()) => {
                    return plain_response(axum::http::StatusCode::BAD_REQUEST, "bad query\n");
                }
            };
            if !valid_format(&route, query.format.as_deref()) {
                return plain_response(axum::http::StatusCode::NOT_FOUND, "not found\n");
            }
            Some(query)
        } else {
            None
        };
        let format = query.as_ref().and_then(|query| query.format.as_deref());

        match route {
            router::Route::Repositories => {
                endpoints::repositories::serve(
                    &self.context,
                    endpoints::repositories::Query::from_request(
                        query.as_ref().expect("query parsed"),
                    ),
                    request.method().clone(),
                )
                .await
            }
            router::Route::Summary(route) => redirect(&route.repo),
            router::Route::GitClone(route) => redirect(&route.repo),
            router::Route::Overview(route) => {
                endpoints::overview::serve(
                    &self.context,
                    route,
                    request.headers(),
                    request.method().clone(),
                )
                .await
            }
            router::Route::About(route) => {
                endpoints::about::serve(&self.context, route, request.method().clone()).await
            }
            router::Route::Stats(route) => {
                let query = match endpoints::stats::Query::from_request(
                    query.as_ref().expect("query parsed"),
                ) {
                    Ok(query) => query,
                    Err(endpoints::stats::QueryError::BadRequest) => {
                        return endpoints::bad_request("bad query\n");
                    }
                    Err(endpoints::stats::QueryError::NotFound) => {
                        return plain_response(axum::http::StatusCode::NOT_FOUND, "not found\n");
                    }
                };
                endpoints::stats::serve(&self.context, route, query, request.method().clone()).await
            }
            router::Route::Object(route) => {
                endpoints::object::serve(REPOSITORIES, route, request.method().clone()).await
            }
            router::Route::Refs(route) => {
                endpoints::refs::serve(&self.context, route, request.method().clone()).await
            }
            router::Route::Tree(route) => {
                endpoints::tree::serve(&self.context, route, format, request.method().clone()).await
            }
            router::Route::Blame(route) => {
                endpoints::blame::serve(&self.context, route, request.method().clone()).await
            }
            router::Route::Archive(route) => {
                let Some(format) = endpoints::archive::Format::parse(format) else {
                    return plain_response(axum::http::StatusCode::NOT_FOUND, "not found\n");
                };
                endpoints::archive::serve(&self.context, route, format, request.method().clone())
                    .await
            }
            router::Route::ArchiveSignature(route) => {
                endpoints::archive_signature::serve(
                    &self.context,
                    route,
                    format,
                    request.method().clone(),
                )
                .await
            }
            router::Route::Revision(route)
                if matches!(
                    &route.params,
                    router::Revision::Ref(reference) if reference.starts_with("refs/tags/")
                ) =>
            {
                endpoints::tag::serve(&self.context, route, request.method().clone()).await
            }
            router::Route::Revision(route) => {
                let query = match endpoints::diff::Query::from_request(
                    query.as_ref().expect("query parsed"),
                ) {
                    Ok(query) => query,
                    Err(()) => return endpoints::bad_request("bad query\n"),
                };
                endpoints::revision::serve(&self.context, route, query, request.method().clone())
                    .await
            }
            router::Route::Diff(route) => {
                let query = match endpoints::diff::Query::from_request(
                    query.as_ref().expect("query parsed"),
                ) {
                    Ok(query) => query,
                    Err(()) => return endpoints::bad_request("bad query\n"),
                };
                endpoints::diff::serve(
                    &self.context,
                    route,
                    query,
                    format == Some("raw"),
                    request.method().clone(),
                )
                .await
            }
            router::Route::Patch(route) => {
                endpoints::patch::serve(&self.context, route, request.method().clone()).await
            }
            router::Route::Log(route) => {
                let query = match endpoints::log::Query::from_request(
                    query.as_ref().expect("query parsed"),
                ) {
                    Ok(query) => query,
                    Err(()) => return endpoints::bad_request("bad query\n"),
                };
                endpoints::log::serve(&self.context, route, query, request.method().clone()).await
            }
            router::Route::AtomFeed(route) => {
                endpoints::atom::serve(
                    &self.context,
                    route,
                    request.headers().get(axum::http::header::HOST),
                    request.method().clone(),
                )
                .await
            }
            router::Route::GitLfs(route) => {
                endpoints::lfs::serve(
                    std::path::Path::new(REPOSITORIES),
                    &route.repo,
                    &route.params,
                    self.write_enabled,
                    request,
                )
                .await
            }
            router::Route::GitInfoRefs(route) => {
                self.git(request, route.repo, "info/refs", self.write_enabled)
                    .await
            }
            router::Route::GitUploadPack(route) => {
                self.git(request, route.repo, "git-upload-pack", false)
                    .await
            }
            router::Route::GitReceivePack(route) if self.write_enabled => {
                self.git(request, route.repo, "git-receive-pack", true)
                    .await
            }
            router::Route::GitReceivePack(_) => {
                plain_response(axum::http::StatusCode::FORBIDDEN, "HTTP push is disabled\n")
            }
            router::Route::GitHead(route) => self.git(request, route.repo, "HEAD", false).await,
            router::Route::GitObjects(route) => {
                self.git(
                    request,
                    route.repo,
                    &format!("objects/{}", route.params),
                    false,
                )
                .await
            }
        }
    }

    async fn git(
        &self,
        request: axum::extract::Request,
        repo: String,
        endpoint: &str,
        authenticated: bool,
    ) -> axum::response::Response {
        if !safe_repository(&repo) {
            return plain_response(axum::http::StatusCode::NOT_FOUND, "not found\n");
        }
        let mut environment = vec![
            ("PATH_INFO".into(), format!("/{repo}.git/{endpoint}").into()),
            ("GIT_PROJECT_ROOT".into(), REPOSITORIES.into()),
        ];
        if authenticated {
            environment.push(("REMOTE_USER".into(), "gilti".into()));
        }
        match endpoints::git_http::serve(&self.git, request, environment).await {
            Ok(response) => response,
            Err(error) => internal_error("git-http-backend", std::io::Error::other(error)),
        }
    }
}

#[derive(Default)]
struct RequestQuery {
    format: Option<String>,
    environment: Vec<(std::ffi::OsString, std::ffi::OsString)>,
}

impl RequestQuery {
    fn value(&self, name: &str) -> Option<&str> {
        self.environment
            .iter()
            .find(|(key, _)| key == std::ffi::OsStr::new(name))
            .and_then(|(_, value)| value.to_str())
    }
}

fn request_query(query: Option<&str>) -> Result<RequestQuery, ()> {
    let mut result = RequestQuery::default();
    for pair in query
        .unwrap_or("")
        .split('&')
        .filter(|pair| !pair.is_empty())
    {
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        let name = decode_query(name)?;
        let value = decode_query(value)?;
        if name == "format" {
            if result.format.replace(value).is_some() {
                return Err(());
            }
            continue;
        }
        let environment = match name.as_str() {
            "q" => "GILTI_QUERY_SEARCH",
            "qt" => "GILTI_QUERY_GREP",
            "ofs" => "GILTI_QUERY_OFFSET",
            "s" => "GILTI_QUERY_SORT",
            "showmsg" => "GILTI_QUERY_SHOWMSG",
            "period" => "GILTI_QUERY_PERIOD",
            "dt" => "GILTI_QUERY_DIFFTYPE",
            "context" => "GILTI_QUERY_CONTEXT",
            "ignorews" => "GILTI_QUERY_IGNOREWS",
            "follow" => "GILTI_QUERY_FOLLOW",
            "view" => "GILTI_QUERY_VIEW",
            _ => return Err(()),
        };
        if result
            .environment
            .iter()
            .any(|(existing, _)| existing == std::ffi::OsStr::new(environment))
        {
            return Err(());
        }
        result.environment.push((environment.into(), value.into()));
    }
    Ok(result)
}

fn decode_query(value: &str) -> Result<String, ()> {
    let value = value.replace('+', " ");
    let value = percent_encoding::percent_decode_str(&value)
        .decode_utf8()
        .map_err(|_| ())?
        .into_owned();
    (!value.contains('\0')).then_some(value).ok_or(())
}

fn valid_format(route: &router::Route, format: Option<&str>) -> bool {
    let Some(format) = format else {
        return true;
    };
    match route {
        router::Route::Tree(_) | router::Route::Diff(_) => matches!(format, "html" | "raw"),
        router::Route::Object(_) => format == "raw",
        router::Route::Patch(_) => matches!(format, "patch" | "raw"),
        router::Route::Archive(_) | router::Route::ArchiveSignature(_) => matches!(
            format,
            "tar" | "tar.gz" | "tar.bz2" | "tar.lz" | "tar.xz" | "tar.zst" | "zip"
        ),
        router::Route::AtomFeed(_) => format == "atom",
        _ => format == "html",
    }
}

fn safe_repository(repo: &str) -> bool {
    !repo.is_empty()
        && !repo.starts_with('/')
        && !repo.chars().any(char::is_control)
        && !repo.contains('\\')
        && repo
            .split('/')
            .all(|component| !component.is_empty() && !matches!(component, "." | ".."))
}

fn redirect(repo: &str) -> axum::response::Response {
    axum::http::Response::builder()
        .status(axum::http::StatusCode::PERMANENT_REDIRECT)
        .header(
            axum::http::header::LOCATION,
            format!("/{}", encode_path(repo)),
        )
        .body(axum::body::Body::empty())
        .expect("valid redirect")
}

fn encode_path(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || byte == b'/' || byte == b'_' {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write;
            write!(encoded, "%{byte:02X}").expect("writing to String cannot fail");
        }
    }
    encoded
}

fn internal_error(context: &str, error: std::io::Error) -> axum::response::Response {
    eprintln!("gilti: {context} request failed: {error}");
    plain_response(
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        "internal server error\n",
    )
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
    use tower::ServiceExt;

    #[tokio::test]
    async fn shared_transport_serves_with_peer_extensions() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let app =
            axum::Router::new().route(
                "/peer",
                axum::routing::get(
                    |axum::Extension(peer): axum::Extension<
                        dimidiumlabs_server::service::PeerAddr,
                    >| async move { peer.0.ip().to_string() },
                ),
            );
        let (app, _drain, transport) = super::harden(app).unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let shutdown = tokio_util::sync::CancellationToken::new();
        let server = tokio::spawn(super::serve(listener, app, transport, shutdown.clone()));

        let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
        stream
            .write_all(b"GET /peer HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).await.unwrap();
        let response = String::from_utf8(response).unwrap();
        assert!(response.starts_with("HTTP/1.1 200"));
        assert!(response.ends_with("127.0.0.1"));

        shutdown.cancel();
        server.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn shared_stack_limits_bodies_and_rejects_during_drain() {
        let app = axum::Router::new().fallback(|| async { "ok" });
        let (app, drain, _transport) = super::harden(app).unwrap();
        let app = app.route("/-/health", axum::routing::get(|| async { "healthy" }));
        let oversized = axum::http::Request::builder()
            .method(axum::http::Method::POST)
            .header(
                axum::http::header::CONTENT_LENGTH,
                gilti_git::backend::MAX_REQUEST_SIZE + 1,
            )
            .body(axum::body::Body::empty())
            .unwrap();
        assert_eq!(
            app.clone().oneshot(oversized).await.unwrap().status(),
            axum::http::StatusCode::PAYLOAD_TOO_LARGE
        );

        assert!(drain.begin());
        let rejected = app
            .clone()
            .oneshot(axum::http::Request::new(axum::body::Body::empty()))
            .await
            .unwrap();
        assert_eq!(
            rejected.status(),
            axum::http::StatusCode::SERVICE_UNAVAILABLE
        );
        let health = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/-/health")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(health.status(), axum::http::StatusCode::OK);
    }

    #[test]
    fn application_assets_use_one_composed_generated_catalog() {
        let stylesheet = super::ASSETS
            .lookup("application.css")
            .expect("application stylesheet")
            .asset();
        assert!(
            std::str::from_utf8(stylesheet.bytes())
                .expect("UTF-8 stylesheet")
                .contains("padding:4px")
        );
        assert_eq!(stylesheet.kind(), dimidiumlabs_ui::AssetKind::Stylesheet);
        assert_ne!(stylesheet.name(), stylesheet.fingerprinted_name());
        assert_eq!(
            super::ASSETS
                .lookup("application.js")
                .unwrap()
                .asset()
                .kind(),
            dimidiumlabs_ui::AssetKind::Script
        );
        assert!(super::ASSETS.lookup("foundation.css").is_some());
        for name in [
            "favicon.ico",
            "favicon.svg",
            "apple-touch-icon.png",
            "icon-192.png",
            "icon-512.png",
            "manifest.webmanifest",
            "robots.txt",
        ] {
            let asset = super::ASSETS
                .lookup(name)
                .expect("registered static asset")
                .asset();
            assert!(asset.integrity().starts_with("sha384-"));
        }
        let manifest = super::ASSETS
            .lookup("manifest.webmanifest")
            .expect("web manifest")
            .asset();
        let manifest = std::str::from_utf8(manifest.bytes()).expect("UTF-8 manifest");
        assert!(manifest.contains("/-/assets/icon-192.png"));
        assert!(manifest.contains("/-/assets/icon-512.png"));
    }

    #[test]
    fn structural_query_parameters_are_rejected() {
        assert!(super::request_query(Some("id=HEAD")).is_err());
        assert!(super::request_query(Some("path=README.md")).is_err());
        assert!(super::request_query(Some("format=raw&format=html")).is_err());
    }

    #[test]
    fn generated_license_bundle_includes_embedded_plex_fonts() {
        assert!(super::licenses::JSON.contains("\"id\": \"OFL-1.1\""));
        assert!(super::licenses::JSON.contains("\"name\": \"dimidiumlabs-ui\""));
        assert!(super::licenses::JSON.contains("SIL OPEN FONT LICENSE Version 1.1"));
    }

    #[test]
    fn embedded_relative_time_script_is_minified_and_keeps_dom_contract() {
        let script = super::ASSETS.lookup("application.js").unwrap().asset();
        let script = std::str::from_utf8(script.bytes()).unwrap();
        assert!(!script.is_empty());
        assert!(script.len() < 1_500);
        assert!(script.contains("data-relative-time"));
        assert!(script.contains("dataset.timestamp"));
        assert!(script.contains("dataset.unit"));
    }
}
