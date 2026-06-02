# Safety Notes

The core design avoids unsafe Rust except in the Win32 boundary crates.

Allowed unsafe zones:

- `crates/borderless-win/src/*`: calling Win32 APIs through `windows-rs`.
- upstream `windows-reactor` internals: WinUI and Windows App SDK interop.

`crates/borderless-gui` uses `#![forbid(unsafe_code)]`; it should remain view/model/runtime glue and should not call Win32 APIs directly.

Rules:

1. Keep raw HWND/HMONITOR/HANDLE values inside newtypes immediately.
2. Convert UTF-16 strings at the edge.
3. Never store borrowed Win32 pointers beyond the callback where they are received.
4. Check Win32 boolean/error results in helper functions.
5. Preserve original styles and rectangles before mutation.
6. Prefer reversible operations; menu removal remains explicit because Win32 menus are hard to restore.
