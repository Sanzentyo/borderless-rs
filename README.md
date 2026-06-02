# borderless-rs

Windows-only Rust mock project for a Borderless-Gaming-class app.

The project is intentionally split into small crates:

- `borderless-core`: Sans I/O domain model, TypeState transition model, ADTs, newtypes, favorites, reducer, traits.
- `borderless-win`: Windows adapter using `windows-rs` for window enumeration, style changes, placement, taskbar/cursor/audio hooks, and config persistence.
- `borderless-reacter`: actor/reacter layer using `ractor`; it supervises watcher/controller actors and gives CLI/GUI the same control plane.
- `borderless-cli`: command-line front-end.
- `borderless-gui`: raw Win32 GUI front-end via `windows-rs`.

`reacter` note: I could not find a public crate named exactly `reacter`, so this mock uses the documented `ractor` crate and names the project-level actor subsystem `borderless-reacter` / `reacter` in code and docs.

## Feature surface

Borderless-Gaming-equivalent feature surface represented here:

- enumerate visible, targetable top-level windows;
- target by PID, HWND, process name, exact title, or title regex;
- favorites with process/title/regex matching;
- automatic watcher that periodically applies favorites;
- delay-before-borderless policy for engine-specific or per-favorite needs;
- remove standard and extended window borders;
- move to current monitor, specific monitor, or exact rectangle;
- fullscreen, specific size, and no-size-change policies;
- offsets for left/top/right/bottom edges;
- optional maximize after resize;
- optional topmost;
- optional menu removal;
- optional Windows taskbar hide/show/restore;
- optional cursor hide/show/restore;
- original style/location capture and restore;
- GUI and CLI share the same runtime actors;
- config stored as TOML under the user config directory;
- audio mute-in-background modeled as an adapter hook.


## Dependency and validation notes

Pinned dependency versions were refreshed against docs.rs while preparing this artifact. `windows-rs` is pinned as `windows = 0.62.2`; the actor/reacter layer is backed by `ractor = 0.15.13` while retaining the project-level crate name `borderless-reacter` for the requested reacter boundary.

This artifact was generated in a Linux container that does not have `rustc`/`cargo`; outbound DNS was unavailable as well, so `cargo fmt`, `cargo check`, and `cargo clippy` could not be run here. The repo includes `rust-toolchain.toml`, `.cargo/config.toml`, bootstrap scripts, and commands in this README so those checks can be run on Windows/MSVC immediately after extraction.

## Install Rust on Windows

PowerShell:

```powershell
irm https://win.rustup.rs/x86_64 -OutFile rustup-init.exe
.\rustup-init.exe -y --profile minimal --default-host x86_64-pc-windows-msvc --component clippy,rustfmt
rustup target add x86_64-pc-windows-msvc
```

You also need MSVC build tools. Install “Desktop development with C++” from Visual Studio Build Tools.

## Build

```powershell
cargo check-win
cargo clippy-win
cargo fmt-all
cargo build --workspace --release --target x86_64-pc-windows-msvc
```

## CLI examples

```powershell
# Show windows that can be managed
cargo run -p borderless-cli -- list

# Make a running process borderless by executable name
cargo run -p borderless-cli -- apply --process-name Game.exe

# Apply with topmost and taskbar/cursor hiding
cargo run -p borderless-cli -- apply --process-name Game.exe --topmost --hide-taskbar --hide-cursor

# Restore a known window
cargo run -p borderless-cli -- restore --hwnd 0x0000000000120340

# Add a favorite
cargo run -p borderless-cli -- favorite add-process Game.exe --topmost --hide-taskbar --delay-ms 4000

# Watch favorites forever
cargo run -p borderless-cli -- watch

# Launch GUI
cargo run -p borderless-gui
```

## GUI mock

The GUI is deliberately raw Win32: a main window, list view placeholder, command buttons, and a bridge to the same `borderless-reacter` actors used by the CLI. It is small but structured so that replacing controls with a richer WinUI/WebView layer later does not touch the domain model.

## Safety stance

Win32 manipulation requires `unsafe`. All unsafe calls are isolated inside `borderless-win` and `borderless-gui`; the core and reacter crates are safe Rust. See `SAFETY.md`.

## Verification status

This archive was generated in an environment without Rust installed, so I could not run `cargo fmt`, `cargo check`, or `cargo clippy` here. The project includes the exact commands and toolchain aliases expected to be run on Windows.
