#![forbid(unsafe_code)]

#[cfg(not(windows))]
compile_error!("borderless-gui is Windows-only; build for x86_64-pc-windows-msvc");

mod app;
mod locale;
mod model;
mod runtime;

pub use app::run;
