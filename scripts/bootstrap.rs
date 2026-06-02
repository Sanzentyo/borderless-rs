#!/usr/bin/env -S cargo +nightly -Zscript
---
[package]
edition = "2024"
---
//! Bootstrap/check script for borderless-rs.
//!
//! This is a Rust single-file package by design. File-changing/toolchain
//! actions are dry-run by default; pass `--apply` to actually run them.
//!
//! ```powershell
//! cargo +nightly -Zscript .\scripts\bootstrap.rs -- --apply --check
//! ```

use std::{
    env, fmt, io,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

const DEFAULT_TOOLCHAIN: &str = "stable";
const DEFAULT_TARGET: &str = "x86_64-pc-windows-msvc";

type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Args {
    repo: PathBuf,
    toolchain: String,
    target: String,
    apply: bool,
    check: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Invocation {
    program: &'static str,
    args: Vec<String>,
}

impl Invocation {
    fn new(program: &'static str, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            program,
            args: args.into_iter().map(Into::into).collect(),
        }
    }

    fn printable(&self) -> String {
        std::iter::once(self.program.to_owned())
            .chain(self.args.iter().map(shell_quote))
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn run(&self, cwd: &Path) -> Result<()> {
        println!("run {}", self.printable());
        let status = Command::new(self.program)
            .args(&self.args)
            .current_dir(cwd)
            .status()
            .map_err(|source| Error::Io {
                context: format!("spawn {}", self.printable()),
                source,
            })?;

        if status.success() {
            Ok(())
        } else {
            Err(Error::CommandFailed {
                command: self.printable(),
                code: status.code(),
            })
        }
    }
}

#[derive(Debug)]
enum Error {
    Help(String),
    Usage(String),
    Io { context: String, source: io::Error },
    CommandFailed { command: String, code: Option<i32> },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Help(message) | Self::Usage(message) => f.write_str(message),
            Self::Io { context, source } => write!(f, "{context}: {source}"),
            Self::CommandFailed { command, code } => match code {
                Some(code) => write!(f, "command failed with exit code {code}: {command}"),
                None => write!(f, "command failed: {command}"),
            },
        }
    }
}

impl std::error::Error for Error {}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(Error::Help(message)) => {
            println!("{message}");
            ExitCode::SUCCESS
        }
        Err(Error::Usage(message)) => {
            eprintln!("{message}");
            ExitCode::from(2)
        }
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<()> {
    let args = Args::parse(env::args().skip(1))?;
    ensure_repo(&args.repo)?;
    print_summary(&args);

    let setup = setup_commands(&args);
    if args.apply {
        run_all(&args.repo, &setup)?;
    } else {
        print_dry_run("toolchain setup", &setup);
    }

    probe_msvc();

    if args.check {
        run_all(&args.repo, &check_commands(&args))?;
    }

    if !args.apply {
        println!("dry-run complete; re-run with --apply to perform setup commands.");
    }
    Ok(())
}

impl Args {
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Self> {
        let mut config = Self {
            repo: env::current_dir().map_err(|source| Error::Io {
                context: "read current directory".to_owned(),
                source,
            })?,
            toolchain: DEFAULT_TOOLCHAIN.to_owned(),
            target: DEFAULT_TARGET.to_owned(),
            apply: false,
            check: false,
        };

        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--" => {}
                "--help" | "-h" => return Err(Error::Help(usage())),
                "--repo" => config.repo = next_path(&mut args, "--repo")?,
                "--toolchain" => config.toolchain = next_value(&mut args, "--toolchain")?,
                "--target" => config.target = next_value(&mut args, "--target")?,
                "--apply" | "-a" => config.apply = true,
                "--check" => config.check = true,
                unknown => {
                    return Err(Error::Usage(format!(
                        "unknown argument `{unknown}`\n\n{}",
                        usage()
                    )));
                }
            }
        }

        Ok(config)
    }
}

fn setup_commands(args: &Args) -> Vec<Invocation> {
    vec![
        Invocation::new(
            "rustup",
            [
                "toolchain",
                "install",
                args.toolchain.as_str(),
                "--profile",
                "minimal",
            ],
        ),
        Invocation::new(
            "rustup",
            [
                "component",
                "add",
                "clippy",
                "rustfmt",
                "--toolchain",
                args.toolchain.as_str(),
            ],
        ),
        Invocation::new(
            "rustup",
            [
                "target",
                "add",
                args.target.as_str(),
                "--toolchain",
                args.toolchain.as_str(),
            ],
        ),
    ]
}

fn check_commands(args: &Args) -> Vec<Invocation> {
    let toolchain = format!("+{}", args.toolchain);
    vec![
        Invocation::new(
            "cargo",
            [toolchain.as_str(), "fmt", "--all", "--", "--check"],
        ),
        Invocation::new(
            "cargo",
            [
                toolchain.as_str(),
                "clippy",
                "--workspace",
                "--all-targets",
                "--target",
                args.target.as_str(),
                "--",
                "-D",
                "warnings",
            ],
        ),
    ]
}

fn run_all(cwd: &Path, commands: &[Invocation]) -> Result<()> {
    commands.iter().try_for_each(|command| command.run(cwd))
}

fn print_summary(args: &Args) {
    println!("repo:      {}", args.repo.display());
    println!("toolchain: {}", args.toolchain);
    println!("target:    {}", args.target);
    println!(
        "mode:      {}",
        if args.apply { "apply" } else { "dry-run" }
    );
}

fn print_dry_run(label: &str, commands: &[Invocation]) {
    println!("would run {label}:");
    commands
        .iter()
        .for_each(|command| println!("  {}", command.printable()));
}

fn usage() -> String {
    format!(
        "\
usage: cargo +nightly -Zscript scripts/bootstrap.rs -- [options]

Options:
  --repo <dir>          Repository root. Defaults to current directory.
  --toolchain <name>    Toolchain to manage. Defaults to {DEFAULT_TOOLCHAIN}.
  --target <triple>     Target to install/check. Defaults to {DEFAULT_TARGET}.
  -a, --apply           Actually run rustup setup commands. Without this, dry-run only.
  --check               Run cargo fmt --check and cargo clippy -D warnings.
  -h, --help            Print this help.
"
    )
}

fn next_path(args: &mut impl Iterator<Item = String>, name: &str) -> Result<PathBuf> {
    args.next()
        .map(PathBuf::from)
        .ok_or_else(|| Error::Usage(format!("{name} expects a path")))
}

fn next_value(args: &mut impl Iterator<Item = String>, name: &str) -> Result<String> {
    args.next()
        .ok_or_else(|| Error::Usage(format!("{name} expects a value")))
}

fn ensure_repo(repo: &Path) -> Result<()> {
    if repo.join("Cargo.toml").is_file() && repo.join("crates").is_dir() {
        Ok(())
    } else {
        Err(Error::Usage(format!(
            "target does not look like borderless-rs repo root: {}",
            repo.display()
        )))
    }
}

fn probe_msvc() {
    if !cfg!(windows) {
        println!("warning: this project targets Windows; run final checks on Windows/MSVC.");
        return;
    }

    match Command::new("where").arg("cl.exe").output() {
        Ok(output) if output.status.success() => println!("MSVC compiler found."),
        _ => println!(
            "warning: cl.exe was not found on PATH. Install Visual Studio Build Tools with Desktop development with C++."
        ),
    }
}

fn shell_quote(value: &String) -> String {
    if value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/' | ':' | '+'))
    {
        value.clone()
    } else {
        format!("\"{}\"", value.replace('"', "\\\""))
    }
}
