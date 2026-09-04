// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

mod components;
mod config;
mod daemon;
mod endpoints;
pub mod router;
mod shell;
mod styles;
mod urls;

const USAGE: &str = "Usage: gilti --config <PATH> [--check] [shell [--is-enabled]]";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Command {
    Daemon,
    Shell,
}

#[derive(Debug, Eq, PartialEq)]
struct Arguments {
    config: std::path::PathBuf,
    command: Command,
    check: bool,
    is_enabled: bool,
}

#[derive(Debug)]
enum ParsedArguments {
    Run(Arguments),
    Help,
}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let arguments = match parse_arguments(std::env::args_os().skip(1)) {
        Ok(ParsedArguments::Run(arguments)) => arguments,
        Ok(ParsedArguments::Help) => {
            println!("{USAGE}");
            return std::process::ExitCode::SUCCESS;
        }
        Err(error) => {
            eprintln!("gilti: {error}\n{USAGE}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let config = match config::Config::load(&arguments.config) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("gilti: {error}");
            return std::process::ExitCode::FAILURE;
        }
    };

    match arguments.command {
        Command::Daemon => daemon::main(arguments.check, &config).await,
        Command::Shell if arguments.is_enabled => shell::enabled(&config),
        Command::Shell => shell::main(arguments.check, &config),
    }
}

fn parse_arguments(
    arguments: impl IntoIterator<Item = std::ffi::OsString>,
) -> Result<ParsedArguments, String> {
    let mut arguments = arguments.into_iter();
    let mut config = None;
    let mut command = Command::Daemon;
    let mut shell = false;
    let mut check = false;
    let mut is_enabled = false;

    while let Some(argument) = arguments.next() {
        if argument == "--config" {
            if config.is_some() {
                return Err("--config may only be specified once".to_owned());
            }
            let path = arguments
                .next()
                .ok_or_else(|| "--config requires a path".to_owned())?;
            if path.is_empty() {
                return Err("--config requires a non-empty path".to_owned());
            }
            config = Some(path.into());
        } else if argument == "--check" {
            if check {
                return Err("--check may only be specified once".to_owned());
            }
            check = true;
        } else if argument == "shell" {
            if shell {
                return Err("shell may only be specified once".to_owned());
            }
            shell = true;
            command = Command::Shell;
        } else if argument == "--is-enabled" {
            if is_enabled {
                return Err("--is-enabled may only be specified once".to_owned());
            }
            is_enabled = true;
        } else if argument == "--help" || argument == "-h" {
            return Ok(ParsedArguments::Help);
        } else {
            return Err(format!("unknown argument {}", argument.to_string_lossy()));
        }
    }

    let config = config.ok_or_else(|| "--config is required".to_owned())?;
    if is_enabled && (!shell || check) {
        return Err("--is-enabled requires shell and cannot be combined with --check".to_owned());
    }
    Ok(ParsedArguments::Run(Arguments {
        config,
        command,
        check,
        is_enabled,
    }))
}

#[cfg(test)]
mod tests {
    use super::{Arguments, Command, ParsedArguments, parse_arguments};

    fn parse(arguments: &[&str]) -> Result<ParsedArguments, String> {
        parse_arguments(arguments.iter().map(std::ffi::OsString::from))
    }

    #[test]
    fn parses_daemon_and_shell_arguments() {
        let ParsedArguments::Run(daemon) = parse(&["--config", "gilti.toml", "--check"]).unwrap()
        else {
            panic!("expected runnable arguments");
        };
        assert_eq!(
            daemon,
            Arguments {
                config: "gilti.toml".into(),
                command: Command::Daemon,
                check: true,
                is_enabled: false,
            }
        );

        let ParsedArguments::Run(shell) = parse(&["shell", "--config", "gilti.yaml"]).unwrap()
        else {
            panic!("expected runnable arguments");
        };
        assert_eq!(shell.command, Command::Shell);
        assert_eq!(shell.config, std::path::Path::new("gilti.yaml"));
        assert!(!shell.check);
        assert!(!shell.is_enabled);

        let ParsedArguments::Run(enabled) =
            parse(&["--config", "gilti.json", "shell", "--is-enabled"]).unwrap()
        else {
            panic!("expected runnable arguments");
        };
        assert!(enabled.is_enabled);
    }

    #[test]
    fn requires_an_explicit_configuration_path() {
        assert!(parse(&[]).unwrap_err().contains("--config is required"));
        assert!(
            parse(&["--config"])
                .unwrap_err()
                .contains("requires a path")
        );
        assert!(
            parse(&["gilti.toml"])
                .unwrap_err()
                .contains("unknown argument")
        );
    }
}
