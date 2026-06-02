# Feature Matrix

| Borderless-Gaming behavior | Borderless Oxide location | Notes |
| --- | --- | --- |
| List targetable windows | `borderless-win::catalog` | Uses two-pass visible/style filtering. |
| Apply borderless | `borderless-win::manipulation` | Removes standard/ex styles and sets frame. |
| Restore | `borderless-win::manipulation::restore_original` | Uses captured style/ex-style/location/topmost. |
| Favorites | `borderless-core::favorite`, `config` | Process, exact title, regex. |
| Auto apply favorites | `borderless-reacter::watcher` | Poll interval honors slow detection. |
| Delay borderless | `FavoriteOptions::delay` and watcher | Engine-specific or per-favorite delay. |
| Specific screen/rect | `TargetFrame` and `MonitorSnapshot` | Current monitor, specific monitor, exact rect. |
| Offsets | `EdgeOffsets` | Left/top/right/bottom. |
| No size/fullscreen/specific | `FavoriteSize` | ADT-driven policy. |
| Topmost | `Placement::topmost` | Uses HWND_TOPMOST / NOTOPMOST. |
| Menu removal | `MenuPolicy` | Safe default is keep; removal is explicit. |
| Taskbar hide/show | `taskbar.rs` | Finds `Shell_TrayWnd` and secondary taskbars. |
| Cursor hide/show | `cursor.rs` | Uses ShowCursor balance policy. |
| Mute in background | `audio.rs` hook | Modeled by PID; CoreAudio implementation skeleton. |
| GUI | `borderless-gui` | Raw Win32 window + actor bridge. |
| CLI | `borderless-cli` | Same controller actor as GUI. |
