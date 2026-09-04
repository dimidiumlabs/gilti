// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use garde::Validate as _;
use serde::Deserialize;

/// Public name and description rendered by the repository browser.
#[derive(Clone, Debug, Deserialize, garde::Validate)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct ConfigInstance {
    /// Title shown in the repository browser.
    #[garde(custom(single_line))]
    pub root_title: String,

    /// Description shown in the repository browser.
    #[garde(custom(single_line))]
    pub root_description: String,

    /// Prefix prepended to repository names when rendering clone URLs.
    #[garde(custom(single_line))]
    pub clone_prefix: String,
}

impl ConfigInstance {
    const DEFAULT_ROOT_TITLE: &str = "Gilti";
    const DEFAULT_ROOT_DESCRIPTION: &str = "A small, fast, independent Git server";
    const DEFAULT_CLONE_PREFIX: &str = "";
}

impl Default for ConfigInstance {
    fn default() -> Self {
        Self {
            root_title: Self::DEFAULT_ROOT_TITLE.to_owned(),
            root_description: Self::DEFAULT_ROOT_DESCRIPTION.to_owned(),
            clone_prefix: Self::DEFAULT_CLONE_PREFIX.to_owned(),
        }
    }
}

/// HTTP listener and server policy.
#[derive(Clone, Debug, Deserialize, garde::Validate)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct ConfigServer {
    /// Address on which the HTTP daemon accepts connections.
    #[garde(skip)]
    pub addr: std::net::SocketAddr,

    /// HTTP authorities accepted by the listener. An empty list disables host filtering.
    #[serde(deserialize_with = "deserialize_authorities")]
    #[garde(skip)]
    pub hostnames: Vec<axum::http::uri::Authority>,

    /// Maximum time allowed for reading HTTP request headers.
    #[serde(with = "humantime_serde")]
    #[garde(custom(nonzero_duration))]
    pub header_read_timeout: std::time::Duration,

    /// Maximum amount of memory used by an HTTP/1 connection buffer.
    #[garde(custom(nonzero_byte_size), custom(byte_size_fits_usize))]
    pub http1_max_buffer_bytes: bytesize::ByteSize,

    /// Maximum number of concurrent streams accepted by one HTTP/2 connection.
    #[garde(range(min = 1))]
    pub http2_max_concurrent_streams: u32,

    /// Maximum HTTP/2 header-list size.
    #[garde(custom(nonzero_byte_size), custom(byte_size_fits_u32))]
    pub http2_max_header_list_bytes: bytesize::ByteSize,

    /// Maximum idle period while receiving a request body.
    #[serde(with = "humantime_serde")]
    #[garde(custom(nonzero_duration))]
    pub request_body_idle_timeout: std::time::Duration,

    /// Maximum accepted HTTP request-body size.
    #[garde(custom(nonzero_byte_size), custom(byte_size_fits_usize))]
    pub request_body_max_bytes: bytesize::ByteSize,

    /// Reverse-proxy networks allowed to provide `X-Forwarded-For`.
    #[garde(skip)]
    pub trusted_proxies: Vec<ipnet::IpNet>,

    /// Smallest dynamic response eligible for compression.
    #[garde(custom(nonzero_byte_size), custom(byte_size_fits_u16))]
    pub compression_min_bytes: bytesize::ByteSize,

    /// Compression quality passed to the HTTP compression backend.
    #[garde(range(min = 0, max = 22))]
    pub compression_level: u8,

    /// Maximum number of requests executing concurrently.
    #[garde(range(min = 1))]
    pub max_concurrent_requests: usize,

    /// Maximum number of requests waiting for admission.
    #[garde(range(min = 1))]
    pub max_queued_requests: usize,

    /// Maximum time a request may wait for admission.
    #[serde(with = "humantime_serde")]
    #[garde(custom(nonzero_duration))]
    pub admission_wait: std::time::Duration,

    /// Maximum graceful-shutdown period.
    #[serde(with = "humantime_serde")]
    #[garde(custom(nonzero_duration))]
    pub shutdown_timeout: std::time::Duration,
}

impl ConfigServer {
    const DEFAULT_ADDR: &str = "0.0.0.0:8080";
    const DEFAULT_HEADER_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
    const DEFAULT_HTTP1_MAX_BUFFER_BYTES: bytesize::ByteSize = bytesize::ByteSize::kib(32);
    const DEFAULT_HTTP2_MAX_CONCURRENT_STREAMS: u32 = 64;
    const DEFAULT_HTTP2_MAX_HEADER_LIST_BYTES: bytesize::ByteSize = bytesize::ByteSize::kib(16);
    const DEFAULT_REQUEST_BODY_IDLE_TIMEOUT: std::time::Duration =
        std::time::Duration::from_secs(60);
    const DEFAULT_REQUEST_BODY_MAX_BYTES: bytesize::ByteSize = bytesize::ByteSize::gib(1);
    const DEFAULT_COMPRESSION_MIN_BYTES: bytesize::ByteSize = bytesize::ByteSize::b(128);
    const DEFAULT_COMPRESSION_LEVEL: u8 = 5;
    const DEFAULT_MAX_CONCURRENT_REQUESTS: usize = 64;
    const DEFAULT_MAX_QUEUED_REQUESTS: usize = 128;
    const DEFAULT_ADMISSION_WAIT: std::time::Duration = std::time::Duration::from_secs(1);
    const DEFAULT_SHUTDOWN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(25);
}

impl Default for ConfigServer {
    fn default() -> Self {
        Self {
            addr: Self::DEFAULT_ADDR.parse().expect("valid default address"),
            hostnames: Vec::new(),
            header_read_timeout: Self::DEFAULT_HEADER_READ_TIMEOUT,
            http1_max_buffer_bytes: Self::DEFAULT_HTTP1_MAX_BUFFER_BYTES,
            http2_max_concurrent_streams: Self::DEFAULT_HTTP2_MAX_CONCURRENT_STREAMS,
            http2_max_header_list_bytes: Self::DEFAULT_HTTP2_MAX_HEADER_LIST_BYTES,
            request_body_idle_timeout: Self::DEFAULT_REQUEST_BODY_IDLE_TIMEOUT,
            request_body_max_bytes: Self::DEFAULT_REQUEST_BODY_MAX_BYTES,
            trusted_proxies: Vec::new(),
            compression_min_bytes: Self::DEFAULT_COMPRESSION_MIN_BYTES,
            compression_level: Self::DEFAULT_COMPRESSION_LEVEL,
            max_concurrent_requests: Self::DEFAULT_MAX_CONCURRENT_REQUESTS,
            max_queued_requests: Self::DEFAULT_MAX_QUEUED_REQUESTS,
            admission_wait: Self::DEFAULT_ADMISSION_WAIT,
            shutdown_timeout: Self::DEFAULT_SHUTDOWN_TIMEOUT,
        }
    }
}

/// Paths that define the Git repository storage layout.
#[derive(Clone, Debug, garde::Validate)]
#[garde(custom(valid_git_storage))]
pub(crate) struct ConfigGitStorage {
    /// Home directory used by Git subprocesses.
    #[garde(custom(absolute_normal_path))]
    pub home: std::path::PathBuf,

    /// Root directory containing bare repositories.
    #[garde(custom(absolute_normal_path))]
    pub repositories: std::path::PathBuf,
}

impl ConfigGitStorage {
    const DEFAULT_HOME: &str = "/var/lib/gilti/git";
    const DEFAULT_REPOSITORIES_DIRECTORY: &str = "repositories";
}

impl Default for ConfigGitStorage {
    fn default() -> Self {
        let home = std::path::PathBuf::from(Self::DEFAULT_HOME);
        let repositories = home.join(Self::DEFAULT_REPOSITORIES_DIRECTORY);
        Self { home, repositories }
    }
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ConfigGitStorageRepresentation {
    home: std::path::PathBuf,
    repositories: Option<std::path::PathBuf>,
}

impl Default for ConfigGitStorageRepresentation {
    fn default() -> Self {
        Self {
            home: ConfigGitStorage::DEFAULT_HOME.into(),
            repositories: None,
        }
    }
}

impl<'de> Deserialize<'de> for ConfigGitStorage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let representation = ConfigGitStorageRepresentation::deserialize(deserializer)?;
        let repositories = representation.repositories.unwrap_or_else(|| {
            representation
                .home
                .join(Self::DEFAULT_REPOSITORIES_DIRECTORY)
        });
        Ok(Self {
            home: representation.home,
            repositories,
        })
    }
}

/// Git executables and subprocess environment.
#[derive(Clone, Debug, Deserialize, garde::Validate)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct ConfigGit {
    /// Git executable used by repository readers and repository creation.
    #[garde(custom(absolute_normal_path))]
    pub executable: std::path::PathBuf,

    /// Candidate locations for Git's smart-HTTP CGI backend.
    #[garde(custom(nonempty_paths))]
    pub http_backends: Vec<std::path::PathBuf>,

    /// Git receive-pack executable allowed by the restricted shell.
    #[garde(custom(absolute_normal_path))]
    pub receive_pack: std::path::PathBuf,

    /// Git upload-archive executable allowed by the restricted shell.
    #[garde(custom(absolute_normal_path))]
    pub upload_archive: std::path::PathBuf,

    /// Git upload-pack executable allowed by the restricted shell.
    #[garde(custom(absolute_normal_path))]
    pub upload_pack: std::path::PathBuf,

    /// Search path exposed to Git subprocesses.
    #[garde(custom(single_line), custom(nonempty_string))]
    pub executable_path: String,

    /// Git configuration file used in place of a user-controlled global config.
    #[garde(custom(absolute_normal_path))]
    pub global_config: std::path::PathBuf,

    /// Maximum UTF-8 byte length accepted for a repository name over SSH.
    #[garde(custom(nonzero_byte_size), custom(byte_size_fits_usize))]
    pub max_repository_name_bytes: bytesize::ByteSize,

    /// Maximum response-header size accepted from the smart-HTTP CGI backend.
    #[garde(custom(nonzero_byte_size), custom(byte_size_fits_usize))]
    pub http_response_header_max_bytes: bytesize::ByteSize,
}

impl ConfigGit {
    const DEFAULT_EXECUTABLE: &str = "/usr/bin/git";
    const DEFAULT_HTTP_BACKENDS: [&str; 2] = [
        "/usr/libexec/git-core/git-http-backend",
        "/usr/lib/git-core/git-http-backend",
    ];
    const DEFAULT_RECEIVE_PACK: &str = "/usr/bin/git-receive-pack";
    const DEFAULT_UPLOAD_ARCHIVE: &str = "/usr/bin/git-upload-archive";
    const DEFAULT_UPLOAD_PACK: &str = "/usr/bin/git-upload-pack";
    const DEFAULT_EXECUTABLE_PATH: &str = "/usr/bin:/bin";
    const DEFAULT_GLOBAL_CONFIG: &str = "/dev/null";
    const DEFAULT_MAX_REPOSITORY_NAME_BYTES: bytesize::ByteSize = bytesize::ByteSize::kib(1);
    const DEFAULT_HTTP_RESPONSE_HEADER_MAX_BYTES: bytesize::ByteSize = bytesize::ByteSize::kib(64);
}

impl Default for ConfigGit {
    fn default() -> Self {
        Self {
            executable: Self::DEFAULT_EXECUTABLE.into(),
            http_backends: Self::DEFAULT_HTTP_BACKENDS
                .into_iter()
                .map(std::path::PathBuf::from)
                .collect(),
            receive_pack: Self::DEFAULT_RECEIVE_PACK.into(),
            upload_archive: Self::DEFAULT_UPLOAD_ARCHIVE.into(),
            upload_pack: Self::DEFAULT_UPLOAD_PACK.into(),
            executable_path: Self::DEFAULT_EXECUTABLE_PATH.to_owned(),
            global_config: Self::DEFAULT_GLOBAL_CONFIG.into(),
            max_repository_name_bytes: Self::DEFAULT_MAX_REPOSITORY_NAME_BYTES,
            http_response_header_max_bytes: Self::DEFAULT_HTTP_RESPONSE_HEADER_MAX_BYTES,
        }
    }
}

/// Git LFS request and object limits.
#[derive(Clone, Debug, Deserialize, garde::Validate)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct ConfigLfs {
    /// Maximum accepted LFS object size.
    #[garde(custom(nonzero_byte_size))]
    pub max_object_bytes: bytesize::ByteSize,

    /// Maximum LFS batch API request size.
    #[garde(custom(nonzero_byte_size), custom(byte_size_fits_usize))]
    pub batch_request_max_bytes: bytesize::ByteSize,

    /// Maximum LFS verification request size.
    #[garde(custom(nonzero_byte_size), custom(byte_size_fits_usize))]
    pub verify_request_max_bytes: bytesize::ByteSize,
}

impl ConfigLfs {
    const DEFAULT_MAX_OBJECT_BYTES: bytesize::ByteSize = bytesize::ByteSize::gib(1);
    const DEFAULT_BATCH_REQUEST_MAX_BYTES: bytesize::ByteSize = bytesize::ByteSize::mib(1);
    const DEFAULT_VERIFY_REQUEST_MAX_BYTES: bytesize::ByteSize = bytesize::ByteSize::kib(64);
}

impl Default for ConfigLfs {
    fn default() -> Self {
        Self {
            max_object_bytes: Self::DEFAULT_MAX_OBJECT_BYTES,
            batch_request_max_bytes: Self::DEFAULT_BATCH_REQUEST_MAX_BYTES,
            verify_request_max_bytes: Self::DEFAULT_VERIFY_REQUEST_MAX_BYTES,
        }
    }
}

/// Repository browser pagination and presentation policy.
#[derive(Clone, Debug, Deserialize, garde::Validate)]
#[garde(custom(valid_browser))]
#[serde(default, deny_unknown_fields)]
pub(crate) struct ConfigBrowser {
    /// Repositories shown on one index page.
    #[garde(range(min = 1))]
    pub repositories_per_page: usize,
    /// Commits shown on one history page.
    #[garde(range(min = 1))]
    pub log_commits_per_page: usize,
    /// Commits included in an Atom feed.
    #[garde(range(min = 1))]
    pub feed_commits: usize,
    /// Branches and tags shown in each overview section.
    #[garde(range(min = 1))]
    pub summary_refs: usize,
    /// Recent commits shown on a repository overview.
    #[garde(range(min = 1))]
    pub summary_commits: usize,
    /// Maximum repository-description length shown on the index.
    #[garde(range(min = 1))]
    pub description_chars: usize,
    /// Default number of context lines around a diff.
    #[garde(range(min = 1))]
    pub diff_default_context: u32,
    /// Largest diff context accepted from a query.
    #[garde(range(min = 1))]
    pub diff_max_context: u32,
    /// Default number of authors shown on a statistics page.
    #[garde(range(min = 1))]
    pub stats_default_authors: usize,
    /// Author-count choices rendered by the statistics page.
    #[garde(custom(nonzero_unique_values))]
    pub stats_author_options: Vec<usize>,
    /// Prefix inspected for a NUL byte when classifying a blob as binary.
    #[garde(custom(nonzero_byte_size), custom(byte_size_fits_usize))]
    pub binary_detection_bytes: bytesize::ByteSize,
    /// Characters shown in abbreviated object identifiers.
    #[garde(range(min = 1))]
    pub abbreviated_oid_chars: usize,
}

impl ConfigBrowser {
    const DEFAULT_REPOSITORIES_PER_PAGE: usize = 50;
    const DEFAULT_LOG_COMMITS_PER_PAGE: usize = 50;
    const DEFAULT_FEED_COMMITS: usize = 10;
    const DEFAULT_SUMMARY_REFS: usize = 10;
    const DEFAULT_SUMMARY_COMMITS: usize = 10;
    const DEFAULT_DESCRIPTION_CHARS: usize = 80;
    const DEFAULT_DIFF_CONTEXT: u32 = 3;
    const DEFAULT_DIFF_MAX_CONTEXT: u32 = 40;
    const DEFAULT_STATS_AUTHORS: usize = 10;
    const DEFAULT_STATS_AUTHOR_OPTIONS: [usize; 4] = [10, 25, 50, 100];
    const DEFAULT_BINARY_DETECTION_BYTES: bytesize::ByteSize = bytesize::ByteSize::b(8_000);
    const DEFAULT_ABBREVIATED_OID_CHARS: usize = 7;
}

impl Default for ConfigBrowser {
    fn default() -> Self {
        Self {
            repositories_per_page: Self::DEFAULT_REPOSITORIES_PER_PAGE,
            log_commits_per_page: Self::DEFAULT_LOG_COMMITS_PER_PAGE,
            feed_commits: Self::DEFAULT_FEED_COMMITS,
            summary_refs: Self::DEFAULT_SUMMARY_REFS,
            summary_commits: Self::DEFAULT_SUMMARY_COMMITS,
            description_chars: Self::DEFAULT_DESCRIPTION_CHARS,
            diff_default_context: Self::DEFAULT_DIFF_CONTEXT,
            diff_max_context: Self::DEFAULT_DIFF_MAX_CONTEXT,
            stats_default_authors: Self::DEFAULT_STATS_AUTHORS,
            stats_author_options: Self::DEFAULT_STATS_AUTHOR_OPTIONS.to_vec(),
            binary_detection_bytes: Self::DEFAULT_BINARY_DETECTION_BYTES,
            abbreviated_oid_chars: Self::DEFAULT_ABBREVIATED_OID_CHARS,
        }
    }
}

/// Repository access policy.
#[derive(Clone, Debug, Deserialize, garde::Validate)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct ConfigAccess {
    /// Whether authenticated Git access over SSH is enabled.
    #[garde(skip)]
    pub ssh: bool,

    /// Whether unauthenticated Git pushes and LFS uploads over HTTP are enabled.
    #[garde(skip)]
    pub http_write: bool,
}

impl ConfigAccess {
    const DEFAULT_SSH: bool = true;
    const DEFAULT_HTTP_WRITE: bool = false;
}

impl Default for ConfigAccess {
    fn default() -> Self {
        Self {
            ssh: Self::DEFAULT_SSH,
            http_write: Self::DEFAULT_HTTP_WRITE,
        }
    }
}

/// Repository archive policy.
#[derive(Clone, Debug, Deserialize, garde::Validate)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct ConfigArchive {
    /// Archive formats exposed by the browser and download endpoints.
    #[serde(deserialize_with = "deserialize_archive_formats")]
    #[garde(custom(unique_archive_formats))]
    pub formats: Vec<gilti_git::archive::Format>,

    /// Streaming buffer used while producing archives.
    #[garde(custom(nonzero_byte_size), custom(byte_size_fits_usize))]
    pub stream_buffer_bytes: bytesize::ByteSize,

    /// Gzip compression level.
    #[garde(range(min = 1, max = 9))]
    pub gzip_level: u8,
    /// Bzip2 compression level.
    #[garde(range(min = 1, max = 9))]
    pub bzip2_level: u8,
    /// XZ compression level.
    #[garde(range(min = 1, max = 9))]
    pub xz_level: u8,
    /// Zstandard compression level.
    #[garde(range(min = 1, max = 22))]
    pub zstd_level: u8,
}

impl ConfigArchive {
    const DEFAULT_FORMATS: [gilti_git::archive::Format; 6] = [
        gilti_git::archive::Format::Tar,
        gilti_git::archive::Format::TarGzip,
        gilti_git::archive::Format::TarBzip2,
        gilti_git::archive::Format::TarXz,
        gilti_git::archive::Format::TarZstd,
        gilti_git::archive::Format::Zip,
    ];
    const DEFAULT_STREAM_BUFFER_BYTES: bytesize::ByteSize = bytesize::ByteSize::kib(64);
    const DEFAULT_GZIP_LEVEL: u8 = 6;
    const DEFAULT_BZIP2_LEVEL: u8 = 9;
    const DEFAULT_XZ_LEVEL: u8 = 6;
    const DEFAULT_ZSTD_LEVEL: u8 = 3;
}

impl Default for ConfigArchive {
    fn default() -> Self {
        Self {
            formats: Self::DEFAULT_FORMATS.to_vec(),
            stream_buffer_bytes: Self::DEFAULT_STREAM_BUFFER_BYTES,
            gzip_level: Self::DEFAULT_GZIP_LEVEL,
            bzip2_level: Self::DEFAULT_BZIP2_LEVEL,
            xz_level: Self::DEFAULT_XZ_LEVEL,
            zstd_level: Self::DEFAULT_ZSTD_LEVEL,
        }
    }
}

/// Complete runtime configuration for the Gilti daemon and restricted Git shell.
#[derive(Debug, Default, Deserialize, garde::Validate)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Config {
    /// HTTP listener and server policy.
    #[garde(dive)]
    pub server: ConfigServer,

    /// Public instance identity.
    #[garde(dive)]
    pub instance: ConfigInstance,

    /// Git repository storage layout.
    #[garde(dive)]
    pub git_storage: ConfigGitStorage,

    /// Git executables and subprocess environment.
    #[garde(dive)]
    pub git: ConfigGit,

    /// Git LFS limits.
    #[garde(dive)]
    pub lfs: ConfigLfs,

    /// Repository browser policy.
    #[garde(dive)]
    pub browser: ConfigBrowser,

    /// Repository access policy.
    #[garde(dive)]
    pub access: ConfigAccess,

    /// Repository archive policy.
    #[garde(dive)]
    pub archive: ConfigArchive,
}

impl Config {
    /// Loads and validates a JSON, TOML, or YAML configuration file.
    pub fn load(path: &std::path::Path) -> std::io::Result<Self> {
        let source = std::fs::read_to_string(path).map_err(|error| {
            std::io::Error::new(
                error.kind(),
                format!("cannot read configuration {}: {error}", path.display()),
            )
        })?;
        Self::parse(path, &source)
    }

    fn parse(path: &std::path::Path, source: &str) -> std::io::Result<Self> {
        let extension = path
            .extension()
            .and_then(std::ffi::OsStr::to_str)
            .map(str::to_ascii_lowercase)
            .ok_or_else(|| unsupported_format(path))?;
        let config: Self = match extension.as_str() {
            "json" => serde_json::from_str(source).map_err(|error| parse_error(path, error))?,
            "toml" => toml::from_str(source).map_err(|error| parse_error(path, error))?,
            "yaml" | "yml" => {
                serde_yaml_ng::from_str(source).map_err(|error| parse_error(path, error))?
            }
            _ => return Err(unsupported_format(path)),
        };
        config
            .validate()
            .map_err(|error| invalid(format!("configuration validation failed: {error}")))?;
        Ok(config)
    }
}

fn deserialize_archive_formats<'de, D>(
    deserializer: D,
) -> Result<Vec<gilti_git::archive::Format>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Vec::<String>::deserialize(deserializer)?
        .into_iter()
        .map(|format| {
            gilti_git::archive::Format::parse(Some(&format)).ok_or_else(|| {
                serde::de::Error::custom(format!("unsupported archive format '{format}'"))
            })
        })
        .collect()
}

fn deserialize_authorities<'de, D>(
    deserializer: D,
) -> Result<Vec<axum::http::uri::Authority>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Vec::<String>::deserialize(deserializer)?
        .into_iter()
        .map(|authority| {
            authority.parse().map_err(|_| {
                serde::de::Error::custom(format!("invalid HTTP authority '{authority}'"))
            })
        })
        .collect()
}

fn nonempty_paths(value: &[std::path::PathBuf], context: &()) -> Result<(), garde::Error> {
    if value.is_empty() {
        return Err(garde::Error::new("must contain at least one path"));
    }
    for path in value {
        absolute_normal_path(path, context)?;
    }
    Ok(())
}

fn nonempty_string(value: &str, _context: &()) -> Result<(), garde::Error> {
    if value.is_empty() {
        Err(garde::Error::new("must not be empty"))
    } else {
        Ok(())
    }
}

fn nonzero_unique_values(value: &[usize], _context: &()) -> Result<(), garde::Error> {
    if value.is_empty() || value.contains(&0) {
        return Err(garde::Error::new(
            "must contain at least one non-zero value",
        ));
    }
    let unique = value
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    if unique.len() == value.len() {
        Ok(())
    } else {
        Err(garde::Error::new("must not contain duplicate values"))
    }
}

fn valid_browser(value: &ConfigBrowser, _context: &()) -> Result<(), garde::Error> {
    if value.diff_default_context > value.diff_max_context {
        return Err(garde::Error::new(
            "diff_default_context must not exceed diff_max_context",
        ));
    }
    if !value
        .stats_author_options
        .contains(&value.stats_default_authors)
    {
        return Err(garde::Error::new(
            "stats_author_options must contain stats_default_authors",
        ));
    }
    Ok(())
}

fn unique_archive_formats(
    value: &[gilti_git::archive::Format],
    _context: &(),
) -> Result<(), garde::Error> {
    let unique = value
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    if unique.len() == value.len() {
        Ok(())
    } else {
        Err(garde::Error::new("must not contain duplicate formats"))
    }
}

fn single_line(value: &str, _context: &()) -> Result<(), garde::Error> {
    if value.contains(['\r', '\n']) {
        Err(garde::Error::new("must be a single line"))
    } else {
        Ok(())
    }
}

fn nonzero_duration(value: &std::time::Duration, _context: &()) -> Result<(), garde::Error> {
    if value.is_zero() {
        Err(garde::Error::new("must be greater than zero"))
    } else {
        Ok(())
    }
}

fn nonzero_byte_size(value: &bytesize::ByteSize, _context: &()) -> Result<(), garde::Error> {
    if value.as_u64() == 0 {
        Err(garde::Error::new("must be greater than zero"))
    } else {
        Ok(())
    }
}

fn byte_size_fits_usize(value: &bytesize::ByteSize, _context: &()) -> Result<(), garde::Error> {
    usize::try_from(value.as_u64())
        .map(|_| ())
        .map_err(|_| garde::Error::new("must fit into usize"))
}

fn byte_size_fits_u32(value: &bytesize::ByteSize, _context: &()) -> Result<(), garde::Error> {
    u32::try_from(value.as_u64())
        .map(|_| ())
        .map_err(|_| garde::Error::new("must fit into u32"))
}

fn byte_size_fits_u16(value: &bytesize::ByteSize, _context: &()) -> Result<(), garde::Error> {
    u16::try_from(value.as_u64())
        .map(|_| ())
        .map_err(|_| garde::Error::new("must fit into u16"))
}

fn absolute_normal_path(value: &std::path::Path, _context: &()) -> Result<(), garde::Error> {
    let Some(value_text) = value.to_str() else {
        return Err(garde::Error::new("must be valid UTF-8"));
    };
    if value_text.chars().any(char::is_control) {
        return Err(garde::Error::new("must not contain control characters"));
    }
    if !value.is_absolute() {
        return Err(garde::Error::new("must be an absolute path"));
    }
    if value
        .components()
        .any(|component| component == std::path::Component::ParentDir)
    {
        return Err(garde::Error::new(
            "must not contain parent-directory components",
        ));
    }
    Ok(())
}

fn valid_git_storage(value: &ConfigGitStorage, _context: &()) -> Result<(), garde::Error> {
    if value.repositories == value.home || !value.repositories.starts_with(&value.home) {
        return Err(garde::Error::new(
            "repositories must be located below the Git home directory",
        ));
    }
    Ok(())
}

fn unsupported_format(path: &std::path::Path) -> std::io::Error {
    invalid(format!(
        "configuration {} must have a .json, .toml, .yaml, or .yml extension",
        path.display()
    ))
}

fn parse_error(path: &std::path::Path, error: impl std::fmt::Display) -> std::io::Error {
    invalid(format!(
        "cannot parse configuration {}: {error}",
        path.display()
    ))
}

fn invalid(message: impl Into<String>) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message.into())
}

#[cfg(test)]
mod tests {
    use super::{Config, ConfigAccess, ConfigInstance, ConfigServer};

    fn parse(extension: &str, source: &str) -> std::io::Result<Config> {
        Config::parse(std::path::Path::new(&format!("config.{extension}")), source)
    }

    #[test]
    fn loads_defaults_through_serde() {
        let config = parse("toml", "").unwrap();
        assert_eq!(config.server.addr.to_string(), ConfigServer::DEFAULT_ADDR);
        assert!(config.server.hostnames.is_empty());
        assert_eq!(
            config.server.header_read_timeout,
            ConfigServer::DEFAULT_HEADER_READ_TIMEOUT
        );
        assert_eq!(
            config.server.http1_max_buffer_bytes,
            ConfigServer::DEFAULT_HTTP1_MAX_BUFFER_BYTES
        );
        assert_eq!(
            config.instance.root_title,
            ConfigInstance::DEFAULT_ROOT_TITLE
        );
        assert_eq!(
            config.instance.root_description,
            ConfigInstance::DEFAULT_ROOT_DESCRIPTION
        );
        assert_eq!(
            config.instance.clone_prefix,
            ConfigInstance::DEFAULT_CLONE_PREFIX
        );
        assert_eq!(
            config.git_storage.home,
            std::path::Path::new("/var/lib/gilti/git")
        );
        assert_eq!(
            config.git_storage.repositories,
            std::path::Path::new("/var/lib/gilti/git/repositories")
        );
        assert_eq!(config.access.ssh, ConfigAccess::DEFAULT_SSH);
        assert_eq!(config.access.http_write, ConfigAccess::DEFAULT_HTTP_WRITE);
        assert_eq!(
            config.archive.formats,
            super::ConfigArchive::DEFAULT_FORMATS
        );
    }

    #[test]
    fn loads_distributed_toml_configuration() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config/gilti.toml");
        Config::load(&path).unwrap();
    }

    #[test]
    fn loads_toml_representation() {
        let config = parse(
            "toml",
            r#"
[server]
addr = "127.0.0.1:9000"
hostnames = ["git.example.test", "git.example.test:8443"]
header_read_timeout = "15s"
http1_max_buffer_bytes = "64KiB"
request_body_max_bytes = "2GiB"
trusted_proxies = ["10.0.0.0/8"]
compression_level = 7

[instance]
root_title = "Repositories"
root_description = "Hosted here"
clone_prefix = "ssh://git@example.test/"

[git_storage]
home = "/srv/git"
repositories = "/srv/git/repos"

[lfs]
max_object_bytes = "512MiB"

[browser]
log_commits_per_page = 25

[access]
ssh = false
http_write = true

[archive]
formats = ["tar", "zip"]
gzip_level = 8
"#,
        )
        .unwrap();
        assert_eq!(config.server.addr.to_string(), "127.0.0.1:9000");
        assert_eq!(
            config
                .server
                .hostnames
                .iter()
                .map(axum::http::uri::Authority::as_str)
                .collect::<Vec<_>>(),
            ["git.example.test", "git.example.test:8443"]
        );
        assert_eq!(
            config.server.header_read_timeout,
            std::time::Duration::from_secs(15)
        );
        assert_eq!(
            config.server.http1_max_buffer_bytes,
            bytesize::ByteSize::kib(64)
        );
        assert_eq!(config.server.compression_level, 7);
        assert_eq!(
            config.server.request_body_max_bytes,
            bytesize::ByteSize::gib(2)
        );
        assert_eq!(config.server.trusted_proxies[0].to_string(), "10.0.0.0/8");
        assert_eq!(config.instance.root_title, "Repositories");
        assert_eq!(config.instance.root_description, "Hosted here");
        assert_eq!(config.instance.clone_prefix, "ssh://git@example.test/");
        assert_eq!(config.git_storage.home, std::path::Path::new("/srv/git"));
        assert_eq!(
            config.git_storage.repositories,
            std::path::Path::new("/srv/git/repos")
        );
        assert_eq!(config.lfs.max_object_bytes, bytesize::ByteSize::mib(512));
        assert_eq!(config.browser.log_commits_per_page, 25);
        assert!(!config.access.ssh);
        assert!(config.access.http_write);
        assert_eq!(config.archive.gzip_level, 8);
        assert_eq!(
            config.archive.formats,
            [
                gilti_git::archive::Format::Tar,
                gilti_git::archive::Format::Zip
            ]
        );
    }

    #[test]
    fn loads_json_and_yaml_representations() {
        let json = parse(
            "json",
            r#"{"instance":{"root_title":"JSON"},"access":{"ssh":false}}"#,
        )
        .unwrap();
        assert_eq!(json.instance.root_title, "JSON");
        assert!(!json.access.ssh);

        let yaml = parse(
            "yaml",
            "server:\n  shutdown_timeout: 40s\ninstance:\n  root_title: YAML\n",
        )
        .unwrap();
        assert_eq!(yaml.instance.root_title, "YAML");
        assert_eq!(
            yaml.server.shutdown_timeout,
            std::time::Duration::from_secs(40)
        );
    }

    #[test]
    fn derives_repository_directory_from_overridden_git_home() {
        let config = parse("toml", "[git_storage]\nhome = '/srv/git'\n").unwrap();
        assert_eq!(
            config.git_storage.repositories,
            std::path::Path::new("/srv/git/repositories")
        );
    }

    #[test]
    fn rejects_invalid_representation_and_values() {
        let unknown = parse("toml", "[server]\nunknown = true\n").unwrap_err();
        assert!(unknown.to_string().contains("unknown field"));

        let bad_hostname = parse("json", r#"{"server":{"hostnames":["bad host"]}}"#).unwrap_err();
        assert!(bad_hostname.to_string().contains("invalid HTTP authority"));

        let bad_archive = parse("toml", "[archive]\nformats = ['tar.lz']\n").unwrap_err();
        assert!(
            bad_archive
                .to_string()
                .contains("unsupported archive format")
        );
        let duplicate_archive = parse("toml", "[archive]\nformats = ['tar', 'tar']\n").unwrap_err();
        assert!(duplicate_archive.to_string().contains("archive.formats"));

        let bad_storage = parse(
            "yaml",
            "git_storage:\n  home: /srv/git\n  repositories: /srv/outside\n",
        )
        .unwrap_err();
        assert!(bad_storage.to_string().contains("git_storage"));

        let invalid_values = parse(
            "toml",
            r#"
[server]
shutdown_timeout = "0s"
compression_min_bytes = "1MiB"
compression_level = 23

[instance]
root_title = "first\nsecond"

[git]
http_backends = []

[browser]
diff_default_context = 41
diff_max_context = 40
stats_default_authors = 10
stats_author_options = [25, 25]

[archive]
gzip_level = 0
"#,
        )
        .unwrap_err()
        .to_string();
        for field in [
            "server.shutdown_timeout",
            "server.compression_min_bytes",
            "server.compression_level",
            "instance.root_title",
            "git.http_backends",
            "browser",
            "browser.stats_author_options",
            "archive.gzip_level",
        ] {
            assert!(invalid_values.contains(field), "{invalid_values}");
        }

        let unsupported = parse("ini", "").unwrap_err();
        assert!(unsupported.to_string().contains(".json"));
    }
}
