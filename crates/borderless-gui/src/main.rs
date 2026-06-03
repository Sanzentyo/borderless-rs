#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> anyhow::Result<()> {
    borderless_native::enable_per_monitor_awareness();
    borderless_gui::run()
}
