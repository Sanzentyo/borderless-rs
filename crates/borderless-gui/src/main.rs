#![windows_subsystem = "windows"]

#[cfg(not(windows))]
compile_error!("borderless-gui is Windows-only; build for x86_64-pc-windows-msvc");

#[cfg(windows)]
fn main() -> anyhow::Result<()> {
    borderless_gui::run()
}
