#![cfg_attr(not(windows), allow(unused_imports))]

#[cfg(not(windows))]
compile_error!("borderless-cli is Windows-only; build for x86_64-pc-windows-msvc");

#[cfg(windows)]
use anyhow::{Result, anyhow};
#[cfg(windows)]
use borderless_core::{
    Favorite, FavoriteId, FavoriteMatcher, FavoriteOptions, Hwnd, Pid, ProcessName, WindowTitle,
};
#[cfg(windows)]
use borderless_reacter::{ControllerMsg, spawn_reacter};
#[cfg(windows)]
use borderless_win::WindowsBackend;
#[cfg(windows)]
use clap::{Args, Parser, Subcommand};

#[cfg(windows)]
#[derive(Parser, Debug)]
#[command(version, about = "Borderless Oxide CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[cfg(windows)]
#[derive(Subcommand, Debug)]
enum Command {
    List,
    Apply(TargetArgs),
    Restore {
        #[arg(long)]
        hwnd: String,
    },
    Watch,
    Favorite {
        #[command(subcommand)]
        command: FavoriteCommand,
    },
    Taskbar {
        visible: bool,
    },
    Cursor {
        visible: bool,
    },
}

#[cfg(windows)]
#[derive(Args, Debug)]
struct TargetArgs {
    #[arg(long, conflicts_with_all = ["pid", "process_name", "title"])]
    hwnd: Option<String>,
    #[arg(long, conflicts_with_all = ["hwnd", "process_name", "title"])]
    pid: Option<u32>,
    #[arg(long, conflicts_with_all = ["hwnd", "pid", "title"])]
    process_name: Option<String>,
    #[arg(long, conflicts_with_all = ["hwnd", "pid", "process_name"])]
    title: Option<String>,
    #[arg(long)]
    topmost: bool,
    #[arg(long)]
    hide_taskbar: bool,
    #[arg(long)]
    hide_cursor: bool,
}

#[cfg(windows)]
#[derive(Subcommand, Debug)]
enum FavoriteCommand {
    AddProcess {
        process_name: String,
        #[arg(long)]
        topmost: bool,
        #[arg(long)]
        hide_taskbar: bool,
        #[arg(long)]
        hide_cursor: bool,
        #[arg(long, default_value_t = 0)]
        delay_ms: u64,
    },
    Remove {
        id: String,
    },
}

#[cfg(windows)]
#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();
    let cli = Cli::parse();
    let reacter = spawn_reacter(WindowsBackend::new()).await?;

    match cli.command {
        Command::List => {
            let windows = ractor::call!(reacter.controller, ControllerMsg::ListWindows)?;
            for window in windows {
                println!(
                    "{} pid={} process={} title={:?} rect={}x{}",
                    window.hwnd,
                    window.pid,
                    window.process_name,
                    window.title.as_str(),
                    window.rect.width().0,
                    window.rect.height().0
                );
            }
        }
        Command::Apply(target) => apply(reacter.controller, target).await?,
        Command::Restore { hwnd } => {
            let hwnd = parse_hwnd(&hwnd)?;
            ractor::call!(reacter.controller, |reply| ControllerMsg::Restore(
                hwnd, reply
            ))?
            .map_err(|err| anyhow!(err))?;
        }
        Command::Watch => loop {
            tokio::time::sleep(std::time::Duration::from_hours(1)).await;
        },
        Command::Favorite { command } => handle_favorite(reacter.controller, command).await?,
        Command::Taskbar { visible } => {
            ractor::call!(
                reacter.controller,
                |reply| ControllerMsg::SetTaskbarVisible(visible, reply)
            )?
            .map_err(|err| anyhow!(err))?;
        }
        Command::Cursor { visible } => {
            ractor::call!(reacter.controller, |reply| ControllerMsg::SetCursorVisible(
                visible, reply
            ))?
            .map_err(|err| anyhow!(err))?;
        }
    }
    Ok(())
}

#[cfg(windows)]
async fn apply(controller: ractor::ActorRef<ControllerMsg>, target: TargetArgs) -> Result<()> {
    let applied = if let Some(hwnd) = target.hwnd {
        let hwnd = parse_hwnd(&hwnd)?;
        ractor::call!(controller, |reply| ControllerMsg::ApplyByHwnd(hwnd, reply))?
    } else if let Some(pid) = target.pid {
        let pid = Pid::new(pid).ok_or_else(|| anyhow!("pid must be non-zero"))?;
        ractor::call!(controller, |reply| ControllerMsg::ApplyByPid(pid, reply))?
    } else if let Some(process_name) = target.process_name {
        let process_name = ProcessName::new(process_name)?;
        ractor::call!(controller, |reply| ControllerMsg::ApplyByProcessName(
            process_name,
            reply
        ))?
    } else if let Some(title) = target.title {
        let title = WindowTitle::new(title);
        ractor::call!(controller, |reply| ControllerMsg::ApplyByTitle(
            title, reply
        ))?
    } else {
        return Err(anyhow!("provide --hwnd, --pid, --process-name, or --title"));
    };
    println!("applied {}", applied.map_err(|err| anyhow!(err))?);
    Ok(())
}

#[cfg(windows)]
async fn handle_favorite(
    controller: ractor::ActorRef<ControllerMsg>,
    command: FavoriteCommand,
) -> Result<()> {
    match command {
        FavoriteCommand::AddProcess {
            process_name,
            topmost,
            hide_taskbar,
            hide_cursor,
            delay_ms,
        } => {
            let process_name = ProcessName::new(process_name)?;
            let favorite = Favorite {
                id: FavoriteId::new(format!("process-{}", process_name.as_str()))?,
                enabled: true,
                matcher: FavoriteMatcher::ProcessName(process_name),
                options: FavoriteOptions {
                    top_most: topmost,
                    hide_windows_taskbar: hide_taskbar,
                    hide_mouse_cursor: hide_cursor,
                    delay: std::time::Duration::from_millis(delay_ms),
                    ..FavoriteOptions::default()
                },
            };
            ractor::call!(controller, |reply| ControllerMsg::AddFavorite(
                favorite, reply
            ))?
            .map_err(|err| anyhow!(err))?;
        }
        FavoriteCommand::Remove { id } => {
            ractor::call!(controller, |reply| ControllerMsg::RemoveFavorite(id, reply))?
                .map_err(|err| anyhow!(err))?;
        }
    }
    Ok(())
}

#[cfg(windows)]
fn parse_hwnd(value: &str) -> Result<Hwnd> {
    let trimmed = value.trim_start_matches("0x").trim_start_matches("0X");
    let parsed = isize::from_str_radix(trimmed, 16).or_else(|_| value.parse())?;
    Ok(Hwnd(parsed))
}
