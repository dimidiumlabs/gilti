// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Git command execution used by representations that libgit2 cannot produce.

use crate::Error;

#[derive(Clone, Debug)]
pub struct GitCommand {
    program: std::path::PathBuf,
    home: std::path::PathBuf,
    executable_path: String,
    global_config: std::path::PathBuf,
}

impl GitCommand {
    pub fn new(
        program: std::path::PathBuf,
        home: std::path::PathBuf,
        executable_path: String,
        global_config: std::path::PathBuf,
    ) -> Self {
        Self {
            program,
            home,
            executable_path,
            global_config,
        }
    }

    pub(crate) fn std(&self) -> std::process::Command {
        let mut command = std::process::Command::new(&self.program);
        self.configure(&mut command);
        command
    }

    fn tokio(&self) -> tokio::process::Command {
        let mut command = tokio::process::Command::new(&self.program);
        self.configure(command.as_std_mut());
        command
    }

    fn configure(&self, command: &mut std::process::Command) {
        command
            .env_clear()
            .env("HOME", &self.home)
            .env("USER", "git")
            .env("LOGNAME", "git")
            .env("PATH", &self.executable_path)
            .env("LC_ALL", "C")
            .env("GIT_CONFIG_GLOBAL", &self.global_config)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_ATTR_NOSYSTEM", "1")
            .env("GIT_TERMINAL_PROMPT", "0");
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ArchiveCompression {
    pub stream_buffer_bytes: usize,
    pub gzip_level: u8,
    pub bzip2_level: u8,
    pub xz_level: u8,
    pub zstd_level: u8,
}

pub type ArchiveStream = std::pin::Pin<
    Box<dyn futures_util::Stream<Item = Result<bytes::Bytes, Error>> + Send + 'static>,
>;

async fn run(mut command: tokio::process::Command, label: &'static str) -> Result<Vec<u8>, Error> {
    let output = command
        .output()
        .await
        .map_err(|e| Error::Internal(e.to_string()))?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(Error::Internal(format!(
            "{label} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )))
    }
}

pub async fn raw_diff(
    git: &GitCommand,
    repository: &std::path::Path,
    old: Option<&str>,
    new: &str,
    path: Option<&str>,
    context: u32,
    ignore_whitespace: bool,
) -> Result<Vec<u8>, Error> {
    let mut command = git.tokio();
    command
        .arg("--git-dir")
        .arg(repository)
        .arg("-c")
        .arg("color.ui=false")
        .arg("diff")
        .arg("--no-ext-diff")
        .arg("--no-textconv")
        .arg("--no-renames")
        .arg(format!("--unified={context}"));
    if ignore_whitespace {
        command.arg("--ignore-all-space");
    }
    command.arg(old.unwrap_or("")).arg(new);
    if let Some(path) = path {
        command.arg("--").arg(path);
    }
    run(command, "git diff").await
}

pub async fn format_patch(
    git: &GitCommand,
    repository: &std::path::Path,
    old: &str,
    new: &str,
    path: Option<&str>,
) -> Result<Vec<u8>, Error> {
    let mut command = git.tokio();
    command
        .arg("--git-dir")
        .arg(repository)
        .arg("-c")
        .arg("color.ui=false")
        .arg("format-patch")
        .arg("--stdout")
        .arg("--keep-subject")
        .arg("--no-renames")
        .arg("--signature=Gilti")
        .arg(format!("{old}..{new}"));
    if let Some(path) = path {
        command.arg("--").arg(format!(":(literal){path}"));
    }
    run(command, "git format-patch").await
}

pub async fn archive(
    git: &GitCommand,
    repository: &std::path::Path,
    oid: &str,
    prefix: &str,
    format: crate::archive::Format,
    path: Option<&str>,
    compression: ArchiveCompression,
) -> Result<ArchiveStream, Error> {
    let mut command = git.tokio();
    command
        .arg("--git-dir")
        .arg(repository)
        .arg("archive")
        .arg(format!("--format={}", format.git_format()))
        .arg(format!("--prefix={prefix}/"))
        .arg(oid)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    if let Some(path) = path {
        command.arg("--").arg(format!(":(literal){path}"));
    }
    let mut child = command
        .spawn()
        .map_err(|error| Error::Internal(error.to_string()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| Error::Internal("git archive stdout unavailable".into()))?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| Error::Internal("git archive stderr unavailable".into()))?;
    let stderr_task = tokio::spawn(async move {
        let mut bytes = Vec::new();
        tokio::io::AsyncReadExt::read_to_end(&mut stderr, &mut bytes)
            .await
            .map(|_| bytes)
    });
    let mut reader = archive_reader(stdout, format, compression);
    let stream = async_stream::try_stream! {
        let mut buffer = vec![0_u8; compression.stream_buffer_bytes];
        loop {
            let count = tokio::io::AsyncReadExt::read(&mut reader, &mut buffer)
                .await
                .map_err(|error| Error::Internal(error.to_string()))?;
            if count == 0 {
                break;
            }
            yield bytes::Bytes::copy_from_slice(&buffer[..count]);
        }
        let status = child
            .wait()
            .await
            .map_err(|error| Error::Internal(error.to_string()))?;
        let stderr = stderr_task
            .await
            .map_err(|error| Error::Internal(error.to_string()))?
            .map_err(|error| Error::Internal(error.to_string()))?;
        if !status.success() {
            Err(Error::Internal(format!(
                "git archive failed: {}",
                String::from_utf8_lossy(&stderr)
            )))?;
        }
    };
    Ok(Box::pin(stream))
}

fn archive_reader(
    stdout: tokio::process::ChildStdout,
    format: crate::archive::Format,
    compression: ArchiveCompression,
) -> std::pin::Pin<Box<dyn tokio::io::AsyncRead + Send>> {
    let reader = tokio::io::BufReader::new(stdout);
    match format {
        crate::archive::Format::Tar | crate::archive::Format::Zip => Box::pin(reader),
        crate::archive::Format::TarGzip => Box::pin(
            async_compression::tokio::bufread::GzipEncoder::with_quality(
                reader,
                async_compression::Level::Precise(i32::from(compression.gzip_level)),
            ),
        ),
        crate::archive::Format::TarBzip2 => {
            Box::pin(async_compression::tokio::bufread::BzEncoder::with_quality(
                reader,
                async_compression::Level::Precise(i32::from(compression.bzip2_level)),
            ))
        }
        crate::archive::Format::TarXz => {
            Box::pin(async_compression::tokio::bufread::XzEncoder::with_quality(
                reader,
                async_compression::Level::Precise(i32::from(compression.xz_level)),
            ))
        }
        crate::archive::Format::TarZstd => Box::pin(
            async_compression::tokio::bufread::ZstdEncoder::with_quality(
                reader,
                async_compression::Level::Precise(i32::from(compression.zstd_level)),
            ),
        ),
    }
}
