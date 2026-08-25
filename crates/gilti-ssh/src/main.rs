// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Restricted OpenSSH forced command for Git services.
//!
//! Every authenticated key is fully trusted. Repository directories, their
//! configuration, and server-side hooks are trusted administrator state.

const GIT_HOME: &str = "/var/lib/gilti/git";
const REPOSITORIES: &str = "/var/lib/gilti/git/repositories";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GitService {
    ReceivePack,
    UploadArchive,
    UploadPack,
}

impl GitService {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "git-receive-pack" => Some(Self::ReceivePack),
            "git-upload-archive" => Some(Self::UploadArchive),
            "git-upload-pack" => Some(Self::UploadPack),
            _ => None,
        }
    }

    fn program(self) -> &'static str {
        match self {
            Self::ReceivePack => "/usr/bin/git-receive-pack",
            Self::UploadArchive => "/usr/bin/git-upload-archive",
            Self::UploadPack => "/usr/bin/git-upload-pack",
        }
    }
}

fn main() -> std::process::ExitCode {
    // SAFETY: setting the process umask has no memory-safety implications.
    unsafe {
        libc::umask(0o077);
    }

    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("gilti-ssh: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--check")) {
        return check_installation();
    }

    let remote = std::env::var("SSH_CONNECTION")
        .ok()
        .and_then(|connection| connection.split_whitespace().next().map(str::to_owned))
        .ok_or_else(|| "SSH_CONNECTION is missing".to_owned())?;
    let command = match std::env::var("SSH_ORIGINAL_COMMAND") {
        Ok(command) if !command.is_empty() => command,
        _ => {
            eprintln!("gilti-ssh: authenticated connection from {remote}");
            println!("Gilti: authenticated. Shell access is disabled.");
            return Ok(());
        }
    };
    if command.contains(['\n', '\r']) {
        return Err("newlines are not allowed in SSH_ORIGINAL_COMMAND".to_owned());
    }

    let (service, repository) = parse_command(&command)?;
    eprintln!("gilti-ssh: {service:?} {repository} from {remote}");
    let root = std::path::Path::new(REPOSITORIES);
    let root_metadata = std::fs::symlink_metadata(root)
        .map_err(|error| format!("cannot inspect {REPOSITORIES}: {error}"))?;
    if !root_metadata.file_type().is_dir() {
        return Err(format!("{REPOSITORIES} is not a real directory"));
    }
    let path = root.join(format!("{repository}.git"));

    if service == GitService::ReceivePack && !path.exists() {
        create_repository(root, &path)?;
    }
    let metadata = std::fs::symlink_metadata(&path)
        .map_err(|_| format!("repository '{repository}' does not exist"))?;
    if !metadata.file_type().is_dir() {
        return Err(format!("repository '{repository}' is not a real directory"));
    }
    verify_repository_path(root, &path)?;

    let mut command = git_command(service.program());
    if let Some(protocol) = std::env::var_os("GIT_PROTOCOL") {
        command.env("GIT_PROTOCOL", protocol);
    }
    let error = std::os::unix::process::CommandExt::exec(command.arg(path));
    Err(format!("cannot execute {}: {error}", service.program()))
}

fn check_installation() -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(REPOSITORIES)
        .map_err(|error| format!("cannot inspect {REPOSITORIES}: {error}"))?;
    if !metadata.file_type().is_dir() {
        return Err(format!("{REPOSITORIES} is not a real directory"));
    }
    for service in [
        GitService::ReceivePack,
        GitService::UploadArchive,
        GitService::UploadPack,
    ] {
        let metadata = std::fs::metadata(service.program())
            .map_err(|error| format!("cannot inspect {}: {error}", service.program()))?;
        if !metadata.is_file()
            || std::os::unix::fs::PermissionsExt::mode(&metadata.permissions()) & 0o111 == 0
        {
            return Err(format!("{} is not executable", service.program()));
        }
    }
    Ok(())
}

fn parse_command(command: &str) -> Result<(GitService, String), String> {
    let (program, argument) = command
        .split_once(' ')
        .ok_or_else(|| "only Git protocol commands are allowed".to_owned())?;
    let service = GitService::parse(program)
        .ok_or_else(|| "only Git protocol commands are allowed".to_owned())?;
    let repository = parse_repository(argument)?;
    Ok((service, repository))
}

fn parse_repository(argument: &str) -> Result<String, String> {
    let argument = argument
        .strip_prefix('\'')
        .and_then(|value| value.strip_suffix('\''));
    let argument = argument.ok_or_else(|| "repository must be single-quoted".to_owned())?;
    let argument = argument.strip_prefix('/').unwrap_or(argument);
    let argument = argument.strip_suffix(".git").unwrap_or(argument);

    if argument.is_empty()
        || argument.len() > 1024
        || !argument.as_bytes()[0].is_ascii_alphanumeric()
        || argument.contains("..")
        || argument.contains(".git/")
    {
        return Err("invalid repository name".to_owned());
    }
    for component in argument.split('/') {
        if component.is_empty()
            || component == "."
            || component == ".."
            || !component
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
        {
            return Err("invalid repository name".to_owned());
        }
    }
    Ok(argument.to_owned())
}

fn verify_repository_path(root: &std::path::Path, path: &std::path::Path) -> Result<(), String> {
    let root = std::fs::canonicalize(root)
        .map_err(|error| format!("cannot resolve {}: {error}", root.display()))?;
    let path = std::fs::canonicalize(path)
        .map_err(|error| format!("cannot resolve {}: {error}", path.display()))?;
    if !path.starts_with(root) {
        return Err("repository escapes the repository directory".to_owned());
    }
    Ok(())
}

fn verify_creation_parent(root: &std::path::Path, path: &std::path::Path) -> Result<(), String> {
    let mut ancestor = path
        .parent()
        .ok_or_else(|| "repository has no parent directory".to_owned())?;
    loop {
        match std::fs::symlink_metadata(ancestor) {
            Ok(metadata) => {
                if !metadata.is_dir() {
                    return Err(format!("{} is not a directory", ancestor.display()));
                }
                return verify_repository_path(root, ancestor);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && ancestor != root => {
                ancestor = ancestor
                    .parent()
                    .ok_or_else(|| "repository escapes the repository directory".to_owned())?;
            }
            Err(error) => {
                return Err(format!("cannot inspect {}: {error}", ancestor.display()));
            }
        }
    }
}

fn git_command(program: &str) -> std::process::Command {
    let mut command = std::process::Command::new(program);
    command
        .env_clear()
        .env("HOME", GIT_HOME)
        .env("USER", "git")
        .env("LOGNAME", "git")
        .env("PATH", "/usr/bin:/bin")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0");
    command
}

fn create_repository(root: &std::path::Path, path: &std::path::Path) -> Result<(), String> {
    verify_creation_parent(root, path)?;
    let parent = path
        .parent()
        .ok_or_else(|| "repository has no parent directory".to_owned())?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    verify_repository_path(root, parent)?;

    let status = git_command("/usr/bin/git")
        .args(["init", "--quiet", "--bare", "--initial-branch=main", "--"])
        .arg(path)
        .status()
        .map_err(|error| format!("cannot initialize {}: {error}", path.display()))?;
    if !status.success() {
        return Err(format!(
            "cannot initialize {}: git exited with {status}",
            path.display()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_git_commands() {
        assert_eq!(
            super::parse_command("git-upload-pack 'group/project.git'").unwrap(),
            (super::GitService::UploadPack, "group/project".to_owned())
        );
        assert_eq!(
            super::parse_command("git-receive-pack '/project'").unwrap(),
            (super::GitService::ReceivePack, "project".to_owned())
        );
    }

    #[test]
    fn rejects_other_commands_and_unsafe_names() {
        for command in [
            "sh -c true",
            "git-upload-pack '../../etc/passwd'",
            "git-upload-pack 'repo..backup'",
            "git-upload-pack 'outer.git/inner'",
            "git-upload-pack '.hidden'",
            "git-upload-pack 'repo name'",
            "git-upload-pack 'repo' trailing",
            "git-upload-pack repo",
            "git-upload-pack 'repo'\nwhoami",
        ] {
            assert!(super::parse_command(command).is_err(), "accepted {command}");
        }
    }

    #[test]
    fn refuses_to_create_through_a_symlinked_parent() {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

        let base = std::env::temp_dir().join(format!(
            "gilti-ssh-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let root = base.join("repositories");
        let outside = base.join("outside");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("group")).unwrap();

        let repository = root.join("group/project.git");
        assert!(super::create_repository(&root, &repository).is_err());
        assert!(!outside.join("project.git").exists());

        std::fs::remove_dir_all(base).unwrap();
    }
}
