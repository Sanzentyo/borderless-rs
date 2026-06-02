$ErrorActionPreference = "Stop"

if (-not (Get-Command rustup -ErrorAction SilentlyContinue)) {
    Write-Host "Installing rustup for x86_64-pc-windows-msvc..."
    Invoke-WebRequest -Uri "https://win.rustup.rs/x86_64" -OutFile "$env:TEMP\rustup-init.exe"
    & "$env:TEMP\rustup-init.exe" -y --profile minimal --default-host x86_64-pc-windows-msvc --component clippy,rustfmt
}

rustup target add x86_64-pc-windows-msvc
cargo check-win
