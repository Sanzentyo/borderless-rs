#![windows_subsystem = "windows"]

#[cfg(not(windows))]
compile_error!("borderless-gui is Windows-only; build for x86_64-pc-windows-msvc");

#[cfg(windows)]
mod gui;

#[cfg(windows)]
fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();
    gui::run()
}
