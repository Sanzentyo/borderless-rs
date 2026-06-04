# Third Party Notices

The default source distribution is primarily licensed under `MIT OR Apache-2.0`.
Magpie-derived scaler crates and binaries built with `magpie-port` or `magpie-wgpu-compare` are
licensed under `GPL-3.0-or-later`.

The Rust dependencies are listed in `Cargo.lock`; their license metadata is available through:

```powershell
cargo metadata --format-version 1
```

Most Rust dependencies used by this project are permissively licensed under MIT, Apache-2.0, or
MIT/Apache-2.0-compatible terms. Notable dependency families include:

- `windows`, `windows-*`, and `windows-reactor`: MIT OR Apache-2.0, sourced from `microsoft/windows-rs`.
- `wgpu`: MIT OR Apache-2.0; it is used only by GPL `borderless-upscale-wgpu` in this repository.
- Magpie-compatible scaler code: GPL-3.0-or-later, isolated in `borderless-magpie` and
  `borderless-upscale-wgpu`. Source files that directly follow Magpie behavior carry Magpie
  attribution in their SPDX headers.
- `tokio`, `ractor`, `tracing`, `serde`, `clap`, `regex`, and related transitive crates: permissive Rust ecosystem licenses.
- `directories` may pull `option-ext`, which is MPL-2.0.

Bundled shader or effect assets must be reviewed file by file. Magpie itself includes GPL effect
assets as well as third-party shader notices, so asset files should keep their original SPDX/license
expression instead of inheriting the Rust crate metadata automatically.

Self-contained GUI builds may include Windows App SDK / Windows App Runtime redistributable files.
Those files are distributed under Microsoft's applicable redistributable terms, not this project's
source license. Review the Windows App SDK deployment and redistributable terms before publishing a
binary installer or archive.
