# borderless-rs

Windows-only Rust project for a Borderless-Gaming-class app.

Most design and implementation notes live under `docs/`:

- [Architecture](docs/architecture.md)
- [Feature matrix](docs/feature-matrix.md)
- [Safety notes](docs/safety.md)
- [Feature parity and modern GUI roadmap](docs/feature-parity-and-modern-gui-roadmap.md)
- [Reactor GUI implementation notes](docs/reactor-gui-implementation-notes.md)

## Install Rust on Windows

PowerShell:

```powershell
irm https://win.rustup.rs/x86_64 -OutFile rustup-init.exe
.\rustup-init.exe -y --profile minimal --default-host x86_64-pc-windows-msvc --component clippy,rustfmt
rustup target add x86_64-pc-windows-msvc
```

You also need MSVC build tools. Install “Desktop development with C++” from Visual Studio Build Tools.

The repository bootstrap/check helper is a Rust cargo script. It is dry-run by default:

```powershell
cargo +nightly -Zscript .\scripts\bootstrap.rs -- --repo . --check
cargo +nightly -Zscript .\scripts\bootstrap.rs -- --repo . --apply --check
```

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
cargo run -p borderless-cli --target x86_64-pc-windows-msvc -- list

# Make a running process borderless by executable name
cargo run -p borderless-cli --target x86_64-pc-windows-msvc -- apply --process-name Game.exe

# Apply with topmost and taskbar/cursor hiding
cargo run -p borderless-cli --target x86_64-pc-windows-msvc -- apply --process-name Game.exe --topmost --hide-taskbar --hide-cursor

# Restore a known window
cargo run -p borderless-cli --target x86_64-pc-windows-msvc -- restore --hwnd 0x0000000000120340

# Add a favorite
cargo run -p borderless-cli --target x86_64-pc-windows-msvc -- favorite add-process Game.exe --topmost --hide-taskbar --delay-ms 4000

# Watch favorites forever
cargo run -p borderless-cli --target x86_64-pc-windows-msvc -- watch

# Launch GUI
cargo run -p borderless-gui --target x86_64-pc-windows-msvc

# Launch GUI with startup profiling
$env:BORDERLESS_PROFILE = "1"
cargo run -p borderless-gui --target x86_64-pc-windows-msvc
Get-Content "$env:TEMP\borderless-oxide-profile.log" -Tail 80
```

## GUI

The GUI is a `windows-reactor` / WinUI frontend. It uses the same `borderless-runtime` controller actor as the CLI, so window enumeration, apply, restore, favorite writes, taskbar, and cursor operations share one runtime boundary.

## Safety stance

Win32 manipulation requires `unsafe`. Unsafe calls are isolated in `borderless-native` and upstream Reactor internals; `borderless-core`, `borderless-runtime`, and `borderless-gui` stay safe Rust. See [docs/safety.md](docs/safety.md).

## Verification status

Development and verification target Windows/MSVC. Use `cargo check-win`, `cargo clippy-win`, and `cargo fmt-all` for the regular local pass.

## License

The default, non-Magpie crates are licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

Magpie-derived scaler crates and builds that enable `magpie-port` or `magpie-wgpu-compare` are
licensed under GPL-3.0-or-later ([LICENSE-GPL-3.0](LICENSE-GPL-3.0)).

See [docs/licensing/spdx-and-gpl-boundary.md](docs/licensing/spdx-and-gpl-boundary.md) for the crate
and feature boundary.

Third-party dependency and redistributable notes live in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
