#!/usr/bin/env sh
set -eu

# Thin Unix/Git-Bash helper inspired by rustup-init.sh. On real Windows,
# prefer scripts/bootstrap.ps1 because the project targets MSVC.
if ! command -v rustup >/dev/null 2>&1; then
  echo "rustup not found. Install it first with the official rustup-init script."
  exit 1
fi

rustup target add x86_64-pc-windows-msvc
cargo check-win
