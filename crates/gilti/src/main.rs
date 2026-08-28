// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

mod cgi;
mod lfs;
pub mod router;
mod ui;

const DEFAULT_LISTEN_ADDR: &str = "0.0.0.0:8080";
const DEFAULT_CACHE_SECONDS: &str = "5";
const DEFAULT_ROOT_TITLE: &str = "Gilti";
const DEFAULT_ROOT_DESCRIPTION: &str = "A tiny Git server";
const MAX_CACHE_SECONDS: u64 = 3600;

const CGIT: &str = "/usr/local/bin/gilti-cgit";
const GIT_HTTP_BACKENDS: &[&str] = &[
    "/usr/libexec/git-core/git-http-backend",
    "/usr/lib/git-core/git-http-backend",
];
const GIT_HOME: &str = "/var/lib/gilti/git";
const REPOSITORIES: &str = "/var/lib/gilti/git/repositories";

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
    ("CGIT_CSS", "/-/assets/cgit.css"),
    ("CGIT_DIFFTYPE", "0"),
    ("CGIT_EMBEDDED", "0"),
    ("CGIT_ENABLE_BLAME", "1"),
    ("CGIT_ENABLE_COMMIT_GRAPH", "1"),
    ("CGIT_ENABLE_FOLLOW_LINKS", "0"),
    ("CGIT_ENABLE_HTML_SERVING", "1"),
    ("CGIT_ENABLE_HTTP_CLONE", "1"),
    ("CGIT_ENABLE_INDEX_LINKS", "1"),
    ("CGIT_ENABLE_INDEX_OWNER", "1"),
    ("CGIT_ENABLE_LOG_FILECOUNT", "1"),
    ("CGIT_ENABLE_LOG_LINECOUNT", "1"),
    ("CGIT_ENABLE_REMOTE_BRANCHES", "0"),
    ("CGIT_ENABLE_SUBJECT_LINKS", "0"),
    ("CGIT_ENABLE_TREE_LINENUMBERS", "1"),
    ("CGIT_FAVICON", "/-/assets/favicon.ico"),
    ("CGIT_FOOTER", ""),
    ("CGIT_HEADER", ""),
    ("CGIT_HEAD_INCLUDE", ""),
    ("CGIT_JS", "/-/assets/cgit.js"),
    ("CGIT_LOCAL_TIME", "0"),
    ("CGIT_LOGO", "/-/assets/cgit.png"),
    ("CGIT_LOGO_LINK", ""),
    ("CGIT_MAX_ATOM_ITEMS", "10"),
    ("CGIT_MAX_BLOB_SIZE", "0"),
    ("CGIT_MAX_COMMIT_COUNT", "50"),
    ("CGIT_MAX_MESSAGE_LENGTH", "80"),
    ("CGIT_MAX_REPO_COUNT", "50"),
    ("CGIT_MAX_REPODESC_LENGTH", "80"),
    ("CGIT_MAX_STATS", "4"),
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
    ("CGIT_SNAPSHOTS", "2147483647"),
    ("CGIT_STRICT_EXPORT", ""),
    ("CGIT_SUMMARY_BRANCHES", "10"),
    ("CGIT_SUMMARY_LOG", "10"),
    ("CGIT_SUMMARY_TAGS", "10"),
    ("CGIT_VIRTUAL_ROOT", "/"),
    ("GIT_ATTR_NOSYSTEM", "1"),
    ("GIT_CONFIG_NOSYSTEM", "1"),
];

#[derive(Clone)]
struct RepositoryService {
    cgit: cgi::Cgi,
    git: cgi::Cgi,
    write_enabled: bool,
}

struct Config {
    listen_addr: std::net::SocketAddr,
    cache: std::time::Duration,
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
        let cache = parse_cache(&environment("GILTI_CGIT_CACHE", DEFAULT_CACHE_SECONDS)?)?;
        Ok(Self {
            listen_addr,
            cache,
            root_title: environment("GILTI_CGIT_ROOT_TITLE", DEFAULT_ROOT_TITLE)?,
            root_description: environment("GILTI_CGIT_ROOT_DESCRIPTION", DEFAULT_ROOT_DESCRIPTION)?,
            clone_prefix: environment("GILTI_CGIT_CLONE_PREFIX", "")?,
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
    let git_http_backend = git_http_backend()?;
    check_files(&git_http_backend)?;

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
    let git = cgi::Cgi::new(git_http_backend, GIT_HOME, config.listen_addr)
        .env("GIT_PROJECT_ROOT", REPOSITORIES)
        .env("GIT_HTTP_EXPORT_ALL", "1")
        .env("HOME", GIT_HOME)
        .env("USER", "git")
        .env("LOGNAME", "git")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("PATH", "/usr/bin:/bin");
    let repositories = RepositoryService {
        cgit,
        git,
        write_enabled: config.http_write,
    };
    let app = axum::Router::new()
        .route(
            "/-/health",
            axum::routing::get(async || {
                response(
                    axum::http::StatusCode::OK,
                    "application/json",
                    b"{\"status\":\"ok\"}\n".to_vec(),
                )
            }),
        )
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
        .route(
            "/-/assets/cgit.css",
            axum::routing::get(async || static_file(CGIT_CSS, "text/css")),
        )
        .route(
            "/-/assets/cgit.js",
            axum::routing::get(async || static_file(CGIT_JS, "text/javascript")),
        )
        .route(
            "/-/assets/cgit.png",
            axum::routing::get(async || static_file(CGIT_LOGO, "image/png")),
        )
        .route(
            "/-/assets/favicon.ico",
            axum::routing::get(async || static_file(CGIT_FAVICON, "image/x-icon")),
        )
        .fallback_service(repositories);

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

fn git_http_backend() -> std::io::Result<std::path::PathBuf> {
    GIT_HTTP_BACKENDS
        .iter()
        .map(std::path::PathBuf::from)
        .find(|path| path.is_file())
        .ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "git-http-backend not found")
        })
}

fn check_files(git_http_backend: &std::path::Path) -> std::io::Result<()> {
    for path in [std::path::Path::new(CGIT), git_http_backend] {
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
    async fn execute(&self, mut request: axum::extract::Request) -> axum::response::Response {
        let route = match router::parse(request.uri().path()) {
            Ok(route) => route,
            Err(_) => return plain_response(axum::http::StatusCode::NOT_FOUND, "not found\n"),
        };
        if let Some(axum::extract::ConnectInfo(remote)) = request
            .extensions()
            .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
            .copied()
        {
            request.extensions_mut().insert(cgi::RemoteAddr(remote));
        }

        match route {
            router::Route::Summary(route) => redirect(&route.repo),
            router::Route::GitClone(route) => redirect(&route.repo),
            router::Route::GitLfs(route) => {
                lfs::serve(
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
            route => self.cgit(request, route).await,
        }
    }

    async fn git(
        &self,
        mut request: axum::extract::Request,
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
        request
            .extensions_mut()
            .insert(cgi::Environment(environment));
        request.extensions_mut().insert(cgi::NoCache);
        match tower::ServiceExt::oneshot(self.git.clone(), request).await {
            Ok(response) => response,
            Err(error) => internal_error("git-http-backend", error),
        }
    }

    async fn cgit(
        &self,
        mut request: axum::extract::Request,
        route: router::Route,
    ) -> axum::response::Response {
        if request.method() != axum::http::Method::GET
            && request.method() != axum::http::Method::HEAD
        {
            return plain_response(
                axum::http::StatusCode::METHOD_NOT_ALLOWED,
                "method not allowed\n",
            );
        }
        let query = match request_query(request.uri().query()) {
            Ok(query) => query,
            Err(()) => return plain_response(axum::http::StatusCode::BAD_REQUEST, "bad query\n"),
        };
        if !valid_format(&route, query.format.as_deref()) {
            return plain_response(axum::http::StatusCode::NOT_FOUND, "not found\n");
        }
        let environment = match cgit_environment(route, query, request.uri().path()) {
            Ok(environment) => environment,
            Err(()) => return plain_response(axum::http::StatusCode::NOT_FOUND, "not found\n"),
        };
        request
            .extensions_mut()
            .insert(cgi::Environment(environment));
        let method = request.method().clone();
        match tower::ServiceExt::oneshot(self.cgit.clone(), request).await {
            Ok(response) => render_private_page(method, response).await,
            Err(error) => internal_error("cgit", error),
        }
    }
}

#[derive(Default)]
struct RequestQuery {
    format: Option<String>,
    environment: Vec<(std::ffi::OsString, std::ffi::OsString)>,
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

fn cgit_environment(
    route: router::Route,
    query: RequestQuery,
    current_url: &str,
) -> Result<Vec<(std::ffi::OsString, std::ffi::OsString)>, ()> {
    let mut environment = query.environment;
    let format = query.format.as_deref();
    let mut set = |name: &str, value: String| environment.push((name.into(), value.into()));
    set("GILTI_CURRENT_URL", current_url.to_owned());

    match route {
        router::Route::Repositories => set("GILTI_PAGE", "repolist".to_owned()),
        router::Route::Overview(route) => {
            set("GILTI_REPOSITORY", route.repo);
            set("GILTI_PAGE", "summary".to_owned());
        }
        router::Route::About(route) => {
            set("GILTI_REPOSITORY", route.repo);
            set("GILTI_PAGE", "about".to_owned());
        }
        router::Route::Stats(route) => {
            set("GILTI_REPOSITORY", route.repo);
            set("GILTI_PAGE", "stats".to_owned());
        }
        router::Route::Object(route) => {
            set("GILTI_REPOSITORY", route.repo);
            set("GILTI_PAGE", "blob".to_owned());
            set("GILTI_REVISION", route.params);
        }
        router::Route::Refs(route) => {
            set("GILTI_REPOSITORY", route.repo);
            set("GILTI_PAGE", "refs".to_owned());
        }
        router::Route::Revision(route) => {
            set("GILTI_REPOSITORY", route.repo);
            set("GILTI_PAGE", "revision".to_owned());
            set("GILTI_REVISION", revision(route.params));
        }
        router::Route::Log(route) => {
            set("GILTI_REPOSITORY", route.repo);
            set("GILTI_PAGE", "log".to_owned());
            set("GILTI_REVISION", revision(route.params.rev));
            if let Some(path) = route.params.path {
                set("GILTI_PATH", path);
            }
        }
        router::Route::Tree(route) => {
            set("GILTI_REPOSITORY", route.repo);
            set(
                "GILTI_PAGE",
                if format == Some("raw") {
                    "plain"
                } else {
                    "tree"
                }
                .to_owned(),
            );
            set("GILTI_REVISION", revision(route.params.rev));
            if let Some(path) = route.params.path {
                set("GILTI_PATH", path);
            }
        }
        router::Route::Blame(route) => {
            set("GILTI_REPOSITORY", route.repo);
            set("GILTI_PAGE", "blame".to_owned());
            set("GILTI_REVISION", revision(route.params.rev));
            set("GILTI_PATH", route.params.path);
        }
        router::Route::Archive(route) => {
            set("GILTI_REPOSITORY", route.repo);
            set("GILTI_PAGE", "snapshot".to_owned());
            set("GILTI_REVISION", revision(route.params.rev));
            set("GILTI_FORMAT", format.unwrap_or("tar.gz").to_owned());
            if let Some(path) = route.params.path {
                set("GILTI_PATH", path);
            }
        }
        router::Route::ArchiveSignature(route) => {
            set("GILTI_REPOSITORY", route.repo);
            set("GILTI_PAGE", "snapshot".to_owned());
            set("GILTI_REVISION", revision(route.params));
            set("GILTI_FORMAT", format.unwrap_or("tar.gz").to_owned());
            set("GILTI_SIGNATURE", "1".to_owned());
        }
        router::Route::AtomFeed(route) => {
            set("GILTI_REPOSITORY", route.repo);
            set("GILTI_PAGE", "atom".to_owned());
            set("GILTI_REVISION", route.params.reference);
            if let Some(path) = route.params.path {
                set("GILTI_PATH", path);
            }
        }
        router::Route::Diff(route) => {
            return comparison_environment(environment, route, format, false);
        }
        router::Route::Patch(route) => {
            return comparison_environment(environment, route, format, true);
        }
        _ => return Err(()),
    }
    Ok(environment)
}

fn comparison_environment(
    mut environment: Vec<(std::ffi::OsString, std::ffi::OsString)>,
    route: router::RepoRoute<router::Comparison>,
    format: Option<&str>,
    patch: bool,
) -> Result<Vec<(std::ffi::OsString, std::ffi::OsString)>, ()> {
    environment.push(("GILTI_REPOSITORY".into(), route.repo.into()));
    environment.push((
        "GILTI_PAGE".into(),
        if patch {
            "patch"
        } else if format == Some("raw") {
            "rawdiff"
        } else {
            "diff"
        }
        .into(),
    ));
    environment.push((
        "GILTI_OLD_REVISION".into(),
        revision(route.params.old_rev).into(),
    ));
    environment.push((
        "GILTI_REVISION".into(),
        revision(route.params.new_rev).into(),
    ));
    if let Some(path) = route.params.path {
        environment.push(("GILTI_PATH".into(), path.into()));
    }
    Ok(environment)
}

fn revision(revision: router::Revision) -> String {
    match revision {
        router::Revision::Head => "HEAD".to_owned(),
        router::Revision::Ref(reference) | router::Revision::Commit(reference) => reference,
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
            return plain_response(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "internal server error\n",
            );
        }
    };
    match ui::render(&body) {
        Ok(markup) => axum::http::response::Response::from_parts(
            parts,
            axum::body::Body::from(markup.into_string()),
        ),
        Err(error) => {
            eprintln!("gilti: invalid private page response: {error}");
            plain_response(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "internal server error\n",
            )
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
    fn environment<'a>(
        values: &'a [(std::ffi::OsString, std::ffi::OsString)],
        name: &str,
    ) -> &'a str {
        values
            .iter()
            .find(|(key, _)| key == std::ffi::OsStr::new(name))
            .and_then(|(_, value)| value.to_str())
            .unwrap()
    }

    #[test]
    fn route_parameters_become_trusted_cgit_environment() {
        let route = super::router::parse(
            "/group/repo/+/diff/refs/heads/main..0123456789abcdef0123456789abcdef01234567/+/src/lib.rs",
        )
        .unwrap();
        let query = super::request_query(Some("format=raw&context=5")).unwrap();
        let values = super::cgit_environment(route, query, "/group/repo").unwrap();
        assert_eq!(environment(&values, "GILTI_REPOSITORY"), "group/repo");
        assert_eq!(environment(&values, "GILTI_PAGE"), "rawdiff");
        assert_eq!(
            environment(&values, "GILTI_OLD_REVISION"),
            "refs/heads/main"
        );
        assert_eq!(
            environment(&values, "GILTI_REVISION"),
            "0123456789abcdef0123456789abcdef01234567"
        );
        assert_eq!(environment(&values, "GILTI_PATH"), "src/lib.rs");
        assert_eq!(environment(&values, "GILTI_QUERY_CONTEXT"), "5");
    }

    #[test]
    fn structural_query_parameters_are_rejected() {
        assert!(super::request_query(Some("id=HEAD")).is_err());
        assert!(super::request_query(Some("path=README.md")).is_err());
        assert!(super::request_query(Some("format=raw&format=html")).is_err());
    }

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
        let model = br#"{"page":"repolist","title":"Gilti","root_desc":"","root_url":"/","about_url":"/-/about","noheader":true,"search":"","current_url":"/","root_readme":false,"owner_enabled":false,"links_enabled":false,"section_grouping":false,"shell":{"embedded":false,"robots":"","css":[],"js":[],"favicon":"","head_include":null,"header":null,"footer_configured":false,"footer":null,"logo":"","logo_link":"","cgit_version":"v1","git_version":"2","generated_at":"now"},"sort_urls":{"name":"/?s=name","desc":"/?s=desc","owner":"/?s=owner","idle":"/?s=idle"},"rows":[],"pager":[]}"#;
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
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        );
    }
}
