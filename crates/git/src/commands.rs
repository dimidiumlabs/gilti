// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Git command execution used by representations that libgit2 cannot produce.

use crate::{Error, GIT};

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
    repository: &std::path::Path,
    old: Option<&str>,
    new: &str,
    path: Option<&str>,
    context: u32,
    ignore_whitespace: bool,
) -> Result<Vec<u8>, Error> {
    let mut command = tokio::process::Command::new(GIT);
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
    repository: &std::path::Path,
    old: &str,
    new: &str,
    path: Option<&str>,
) -> Result<Vec<u8>, Error> {
    let mut command = tokio::process::Command::new(GIT);
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
        .arg(format!("{old}..{new}"))
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1");
    if let Some(path) = path {
        command.arg("--").arg(format!(":(literal){path}"));
    }
    run(command, "git format-patch").await
}

pub async fn archive(
    repository: &std::path::Path,
    oid: &str,
    prefix: &str,
    format: &str,
    path: Option<&str>,
    compressor: Option<(&str, &[&str])>,
) -> Result<Vec<u8>, Error> {
    let mut command = tokio::process::Command::new(GIT);
    command
        .arg("--git-dir")
        .arg(repository)
        .arg("archive")
        .arg(format!("--format={format}"))
        .arg(format!("--prefix={prefix}/"))
        .arg(oid)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1");
    if let Some(path) = path {
        command.arg("--").arg(format!(":(literal){path}"));
    }
    let bytes = run(command, "git archive").await?;
    let Some((program, args)) = compressor else {
        return Ok(bytes);
    };
    let mut command = tokio::process::Command::new(program);
    command
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|e| Error::Internal(e.to_string()))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| Error::Internal("compressor stdin unavailable".into()))?;
    let write = async move {
        tokio::io::AsyncWriteExt::write_all(&mut stdin, &bytes)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;
        tokio::io::AsyncWriteExt::shutdown(&mut stdin)
            .await
            .map_err(|e| Error::Internal(e.to_string()))
    };
    let (write, output) = tokio::join!(write, child.wait_with_output());
    write?;
    let output = output.map_err(|e| Error::Internal(e.to_string()))?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(Error::Internal(format!(
            "archive compressor failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )))
    }
}
