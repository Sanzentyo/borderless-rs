# Borderless-Gaming parity and Windows 11 modern GUI roadmap

作成日: 2026-06-02
対象: `Sanzentyo/borderless-rs`

この文書は、現行の `borderless-rs` を C# 版 Borderless-Gaming 相当まで引き上げるための差分整理と、Windows 11 らしいモダン GUI に作り替えるための具体的な設計・適用手順をまとめる。

## 1. 結論

現行 `borderless-rs` は、次の基礎がすでに良い形で入っている。

- `borderless-core`: Sans I/O domain、newtype、ADT、TypeState、trait boundary。
- `borderless-native`: `windows-rs` による native Win32 backend。
- `borderless-runtime`: `ractor` による controller / watcher actor。
- `borderless-cli` と `borderless-gui`: 同じ runtime actor を使う front-end。

したがって、今後は **C# 版の WinForms UI をそのまま再現するのではなく、機能互換のまま Windows 11 Fluent 風の native GUI に再構成する** のがよい。

実装方針は次の 2 段階にする。

1. **Parity layer**: C# 版にある実用機能を Rust 側の domain / native backend / actor runtime に足す。
2. **Modern GUI layer**: raw Win32 のまま ListBox/Button ベースを捨て、DWM Mica、Direct2D/DirectWrite owner-draw、NavigationView 風 shell、CommandBar 風 toolbar、card/list UI、tray integration に移行する。

WinUI 3 / Windows App SDK を Rust から直接使う案は将来的には魅力があるが、現時点では build/deployment complexity が大きい。まずは `windows-rs` の Win32 / DWM / Direct2D / DirectWrite / Shell API で **unpackaged native app** として仕上げる。そのうえで、後から `winui` feature flag を生やせる構成にする。

## 2. 現行 `borderless-rs` の基礎評価

現行 workspace は次の crate 分割で、境界はよい。

```text
crates/
  borderless-core/      # OS 非依存 domain
  borderless-native/       # windows-rs adapter
  borderless-runtime/   # actor runtime control plane
  borderless-cli/       # CLI
  borderless-gui/       # GUI
```

現行 README では feature surface として window enumeration、PID/HWND/process/title/regex target、favorites、watcher、delay、style 除去、monitor/rect placement、offset、topmost、menu removal、taskbar/cursor、restore、TOML config、audio hook まで表現されている。

`borderless-core` は `#![forbid(unsafe_code)]` で、module も `action`, `backend`, `config`, `favorite`, `reducer`, `session`, `types`, `window` に分かれている。これは維持する。

`borderless-native` は `catalog`, `manipulation`, `monitor`, `taskbar`, `cursor`, `audio`, `store` などに Win32 実装を閉じ込めている。これも維持し、追加機能もこの crate に閉じ込める。

`borderless-gui` は現在、raw Win32 の `BUTTON`, `LISTBOX`, `STATIC` を使ったシンプルな 1 画面 UI である。機能確認用としては十分だが、最終 UI としては再設計する。

## 3. C# Borderless-Gaming から見た不足機能一覧

凡例:

- `Done`: Rust 側で実装済み、または概ね同等。
- `Partial`: domain/hook はあるが、GUI や adapter 実体が不足。
- `Missing`: 具体実装がまだない。
- `Optional`: C# 版の周辺機能だが、Rust 版の本質には不要または後回しでよい。

| 領域 | C# 版機能 | Rust 現状 | 実装方針 |
| --- | --- | --- | --- |
| Window enumeration | visible/style による targetable window 検出 | Done | `borderless-native::catalog` を維持し、hidden process / full details filter を追加する。 |
| Apply borderless | style/ex-style 除去、position、maximize | Done | `WindowsManipulator::apply_plan` を維持し、DPI/monitor edge cases を追加検証する。 |
| Restore | style/ex-style/location/topmost restore | Done | process exit 時の環境復旧とセットにする。 |
| Favorites | process/title/regex、size、offset、topmost、taskbar/cursor、delay、mute | Partial | domain は大部分あり。GUI editor と update path を追加する。 |
| Auto apply | watcher が favorites を適用 | Partial | apply 済み window の再適用抑制、process prune、pause/resume を追加する。 |
| Delay borderless | per-favorite delay / engine-specific delay | Partial | delay はある。class-name based policy を追加する。 |
| Mute in background | background 時に process audio mute | Missing | CoreAudio 実装と foreground/focus actor を追加する。 |
| Global hotkeys | Win+F6 borderless toggle、mouse lock/hide hotkey | Missing | `hotkey.rs` と `HotkeyActor` を追加する。 |
| Mouse lock | ScrollLock で mouse lock | Missing | `ClipCursor` ベースの `MouseLock` adapter を追加する。 |
| Tray | start minimized, close to tray, balloon tips | Missing | `Shell_NotifyIconW` ベースの `TrayActor` を追加する。 |
| Startup | Task Scheduler entry | Missing | Task Scheduler COM adapter を追加する。fallback は Startup folder shortcut。 |
| Hidden processes | process hide / reset hidden processes | Missing | `AppConfig.hidden_processes: BTreeSet<ProcessName>` を追加する。 |
| View full process details | basic/detail 表示切替 | Missing | GUI filter と `WindowRowMode` を追加する。 |
| Set window title | selected window title rename | Missing | `SetWindowTextW` adapter と GUI action を追加する。 |
| Desktop area selector | specific rect / screen selector | Partial | domain は `TargetFrame::Exact` がある。GUI overlay selector を追加する。 |
| Open data folder | Explorer で config folder を開く | Missing | `shell.rs` に `open_path` を追加する。 |
| Pause automatic processing | watcher pause | Missing | `WatcherMsg::Pause/Resume` と UI toggle を追加する。 |
| Localization | culture setting / resources | Missing | `fluent-bundle` か simple TOML catalog を追加する。 |
| Updates | check for updates | Missing/Optional | GitHub Releases check を任意機能にする。private repo では off by default。 |
| Steam integration / ads | Steam support, ad option | Optional | 実装しない。settings には compatibility placeholder のみ。 |
| Regex help | regex reference link | Missing | Help flyout / external link action を追加する。 |
| CLI silent/minimize | startup 用 args | Partial | `borderless-gui --silent --minimize` を追加する。 |

## 4. 追加後の crate / module 構成

既存 crate 分割は維持しつつ、次のように拡張する。

```text
crates/
  borderless-core/
    src/
      action.rs
      backend.rs
      config.rs          # AppConfig を AppSettings で拡張
      favorite.rs
      hotkey.rs          # new: OS 非依存 hotkey domain
      process.rs         # new: hidden process / process lifecycle domain
      ui.rs              # new: GUI 用 Sans I/O model
      reducer.rs
      session.rs
      types.rs
      window.rs

  borderless-native/
    src/
      audio.rs           # CoreAudio 実装へ拡張
      catalog.rs
      cursor.rs
      dwm.rs             # new: Mica/dark/corners/backdrop
      hotkey.rs          # new: RegisterHotKey RAII
      manipulation.rs
      monitor.rs
      mouse_lock.rs      # new: ClipCursor
      process.rs
      shell.rs           # new: open folder/url, Explorer integration
      startup.rs         # new: Task Scheduler / startup shortcut
      store.rs
      taskbar.rs
      title.rs           # new: SetWindowTextW
      tray.rs            # new: Shell_NotifyIconW

  borderless-runtime/
    src/
      controller.rs
      foreground.rs      # new: foreground/focus watcher
      gui_bridge.rs      # new: UI snapshot fan-out
      hotkey.rs          # new: hotkey actor
      messages.rs
      supervisor.rs
      tray.rs            # new: tray actor
      watcher.rs

  borderless-gui/
    src/
      main.rs
      app.rs
      shell.rs           # NavigationView 風 shell
      window.rs          # HWND lifecycle / message loop
      theme.rs           # design tokens, dark/light/high contrast
      layout.rs          # responsive layout
      controls/
        command_bar.rs
        nav_rail.rs
        window_list.rs
        favorite_editor.rs
        toggle.rs
        dialogs.rs
      painting/
        d2d.rs
        text.rs
        icons.rs
```

`borderless-core::ui` は Sans I/O にして、Win32 の HWND や paint code を持たない。GUI は `UiIntent -> UiModel -> UiEffect` の reducer とし、OS 操作は runtime に流す。

## 5. Domain model 追加案

### 5.1 AppConfig / AppSettings

現行 `AppConfig` は `slow_window_detection`, `poll_interval`, `favorites` のみなので、C# 版 `AppSettings` 相当を足す。

```rust
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct AppConfig {
    pub detection: DetectionSettings,
    pub lifecycle: LifecycleSettings,
    pub hotkeys: HotkeySettings,
    pub tray: TraySettings,
    pub ui: UiSettings,
    pub hidden_processes: BTreeSet<ProcessName>,
    pub favorites: Vec<Favorite>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct LifecycleSettings {
    pub run_on_startup: bool,
    pub start_minimized: bool,
    pub close_to_tray: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct HotkeySettings {
    pub global_borderless_toggle: Option<HotkeySpec>,
    pub mouse_lock_toggle: Option<HotkeySpec>,
    pub mouse_hide_toggle: Option<HotkeySpec>,
}
```

互換性のため、旧 TOML を読む migration を `store.rs` に置く。`AppConfig::default()` は C# と同じ感覚で slow detection off、poll 3 秒、auto maximize on、tray off にする。

### 5.2 Cursor / mouse policy

現在は cursor visibility だけなので、visibility と lock を分ける。

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CursorPolicy {
    Unchanged,
    Hidden,
    LockedToWindow,
    HiddenAndLocked,
}
```

Native backend は `ShowCursor` の counter 問題を `CursorVisibilityGuard` で吸収し、mouse lock は `ClipCursor(Some(rect))` / `ClipCursor(None)` の RAII にする。

### 5.3 Audio policy

CoreAudio は domain から隔離する。

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackgroundAudioPolicy {
    #[default]
    LeaveUnchanged,
    MuteWhenUnfocused,
}
```

`FavoriteOptions::mute_in_background: bool` は将来的に `background_audio: BackgroundAudioPolicy` へ移行する。TOML 互換のため、旧 bool は deserialize alias として受ける。

### 5.4 Window lifecycle

apply 済み状態は controller の `HashMap<Hwnd, OriginalWindowState>` だけでは足りない。プロセス終了、HWND 差し替え、title 変更、favorite unmatch を扱うため、次の registry を core 側に置く。

```rust
pub struct ManagedWindowRegistry {
    applied: BTreeMap<Hwnd, ManagedWindow>,
}

pub struct ManagedWindow {
    pub original: OriginalWindowState,
    pub favorite_id: Option<FavoriteId>,
    pub pid: Pid,
    pub process_name: ProcessName,
    pub environment_locks: EnvironmentLocks,
}

pub struct EnvironmentLocks {
    pub taskbar_hidden: bool,
    pub cursor_hidden: bool,
    pub cursor_locked: bool,
    pub audio_policy: BackgroundAudioPolicy,
}
```

## 6. Runtime / actor topology

現在の topology は `ControllerActor` と `WatcherActor` が中心。機能互換には次の actor を追加する。

```text
Supervisor
  ├─ ControllerActor
  ├─ WatcherActor
  ├─ ForegroundActor
  ├─ HotkeyActor
  ├─ TrayActor
  ├─ GuiBridgeActor
  └─ ConfigActor   # optional: save debounce / migration / import-export
```

### 6.1 ControllerMsg 拡張

```rust
pub enum ControllerMsg {
    ListWindows(RpcReplyPort<Vec<WindowSnapshot>>),
    ApplyByHwnd(Hwnd, RpcReplyPort<Result<Hwnd, String>>),
    Restore(Hwnd, RpcReplyPort<Result<(), String>>),

    AddFavorite(Favorite, RpcReplyPort<Result<(), String>>),
    UpdateFavorite(Favorite, RpcReplyPort<Result<(), String>>),
    RemoveFavorite(FavoriteId, RpcReplyPort<Result<(), String>>),
    SetFavoriteEnabled(FavoriteId, bool, RpcReplyPort<Result<(), String>>),

    SetWindowTitle(Hwnd, WindowTitle, RpcReplyPort<Result<(), String>>),
    HideProcess(ProcessName, RpcReplyPort<Result<(), String>>),
    ResetHiddenProcesses(RpcReplyPort<Result<(), String>>),

    SetWatcherPaused(bool, RpcReplyPort<Result<(), String>>),
    SetStartupEnabled(bool, RpcReplyPort<Result<(), String>>),
    SetTrayPreference(TraySettings, RpcReplyPort<Result<(), String>>),
    SetHotkeys(HotkeySettings, RpcReplyPort<Result<(), String>>),

    Snapshot(RpcReplyPort<AppSnapshot>),
}
```

### 6.2 WatcherActor 改修

`WatcherActor` は `Tick` ごとに次を行う。

1. `backend.windows()` で現在 window set を取る。
2. hidden process を除外する。
3. `ManagedWindowRegistry` と照合し、消えた HWND/PID を prune する。
4. prune された window が taskbar/cursor/audio を変更していたら復旧する。
5. favorites と未管理 window を照合する。
6. delay policy を評価する。
7. already applied window には再適用しない。
8. `GuiBridgeActor` に snapshot を publish する。

C# 版の `ProcessWatcher` は process が消えたときに taskbar/cursor を戻すので、この挙動は Rust 側でも必須にする。

### 6.3 ForegroundActor

Mute in background と global toggle には foreground tracking が必要。

実装は 2 段階にする。

- v1: 500ms polling で `GetForegroundWindow` -> `GetWindowThreadProcessId`。
- v2: `SetWinEventHook(EVENT_SYSTEM_FOREGROUND, ...)` で foreground event-driven にする。

`ForegroundActor` は `ForegroundChanged { hwnd, pid }` を Controller に送り、Controller が `ManagedWindowRegistry` と favorite の audio policy を見て `set_process_muted(pid, muted)` を呼ぶ。

### 6.4 HotkeyActor

`RegisterHotKey` / `UnregisterHotKey` は GUI window の message loop と関係するので、`HotkeyActor` は GUI HWND を受け取って登録する。

TypeState で二重 unregister を防ぐ。

```rust
pub struct HotkeyRegistration<State> {
    hwnd: Hwnd,
    id: HotkeyId,
    spec: HotkeySpec,
    _state: PhantomData<State>,
}

pub enum Registered {}
pub enum Unregistered {}
```

`WM_HOTKEY` は `borderless-gui::window` が受け、`HotkeyActor` または Controller に `HotkeyPressed` を送る。

### 6.5 TrayActor

`Shell_NotifyIconW` は lifecycle が複雑なので actor に隔離する。

- `TrayMsg::Install(HWND)`
- `TrayMsg::SetTooltip(String)`
- `TrayMsg::ShowBalloon(TrayNotification)`
- `TrayMsg::UpdateMenu(TrayMenuModel)`
- `TrayMsg::Remove`

close-to-tray / start-minimized は `LifecycleSettings` と連動する。

## 7. Native backend 実装の具体策

### 7.1 CoreAudio: mute-in-background

`borderless-native::audio` を skeleton から実装へ変える。

必要な概念:

```text
CoInitializeEx
  -> IMMDeviceEnumerator
  -> GetDefaultAudioEndpoint(eRender, eMultimedia)
  -> IMMDevice::Activate(IAudioSessionManager2)
  -> IAudioSessionManager2::GetSessionEnumerator
  -> IAudioSessionEnumerator::GetSession(index)
  -> IAudioSessionControl2::GetProcessId
  -> ISimpleAudioVolume::SetMute
```

実装単位:

```text
borderless-native/src/audio.rs
  ComApartment
  AudioEndpoint
  AudioSession
  AudioSessions::sessions_for_pid(pid)
  AudioSessions::set_pid_muted(pid, muted)
```

注意点:

- COM 初期化は RAII で行う。
- UI thread と watcher thread の apartment を混ぜない。
- 失敗時は fatal にせず `CoreError::AudioUnavailable` として GUI に warning を出す。
- mute 状態は process が exit したら prune で戻す。

### 7.2 Startup: Task Scheduler

C# 版は Startup folder shortcut を掃除し、Task Scheduler に `BorderlessGaming` task を作る。Rust 版も同じ思想にする。

実装案:

```text
borderless-native/src/startup.rs
  StartupRegistration
  StartupTaskName(newtype)
  StartupArgs(newtype)
  StartupRegistrar trait impl
```

API:

```rust
pub trait StartupManager: Send + Sync {
    fn set_startup_enabled(&self, enabled: bool, args: StartupArgs) -> CoreResult<()>;
    fn is_startup_enabled(&self) -> CoreResult<bool>;
}
```

実装は Task Scheduler COM を第一候補にする。COM が難しい場合の一時 fallback は `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` でもよいが、最終的には C# 版に合わせて Task Scheduler とする。

### 7.3 Tray

`Shell_NotifyIconW` を直接使う。

- icon: `assets/app.ico` を resource script に埋める。
- callback message: `WM_APP + 1`。
- right-click menu: `CreatePopupMenu`, `TrackPopupMenuEx`。
- commands: Show, Refresh, Pause watcher, Toggle taskbar, Toggle cursor, Exit。

GUI を閉じたとき:

- `close_to_tray = true`: hide window、runtime は生かす。
- `close_to_tray = false`: restore taskbar/cursor/audio locks -> remove tray icon -> quit。

### 7.4 Hotkeys

C# 版相当の default:

```toml
[hotkeys]
global_borderless_toggle = { modifiers = ["win"], key = "F6" }
mouse_lock_toggle = { modifiers = [], key = "ScrollLock" }
mouse_hide_toggle = { modifiers = ["win"], key = "ScrollLock" }
```

`HotkeySpec` は string で保持せず、ADT にする。

```rust
pub enum HotkeyModifier { Alt, Control, Shift, Win, NoRepeat }
pub struct VirtualKey(u16);
pub struct HotkeySpec { modifiers: BTreeSet<HotkeyModifier>, key: VirtualKey }
```

### 7.5 Mouse lock

`MouseLock` は target window の rect を `ClipCursor` に渡す。

- window が move/resize されたら rect を再計算する。
- Alt+Tab / focus lost では unlock する。
- exit/prune 時も必ず unlock する。

### 7.6 Hidden processes and full process details

`WindowCatalog::windows()` は全 targetable windows を返し、filter は Controller/Watcher 側で適用する。

```rust
pub enum WindowListMode {
    ManageableOnly,
    FullDetails,
}
```

GUI では Settings の `View full process details` で切り替える。

### 7.7 Set window title

`borderless-native::title::set_window_title(hwnd, title)` を追加する。

- `WindowTitle` newtype を使う。
- 空文字は許可するが GUI では warning を出す。
- 成功後 `WatcherActor` が next tick で snapshot 更新する。

### 7.8 Taskbar restore policy

現行 `taskbar::set_visible(false)` は直接 show/hide するだけなので、誰が隠したかを tracking する。

```rust
pub struct EnvironmentState {
    taskbar_hide_refcount: u32,
    cursor_hide_refcount: u32,
    locked_windows: BTreeSet<Hwnd>,
}
```

複数 favorite が同時に taskbar を隠している場合、1 つが終了しても残りが動いている間は taskbar を戻さない。

## 8. Modern GUI design spec

### 8.1 原則

- **WinForms clone にしない**。機能互換は維持するが、UI は Windows 11 の native/Fluent 風にする。
- **左 rail + content**。NavigationView 風の adaptive shell を raw Win32/Direct2D で作る。
- **Mica background**。メイン window は Mica。transient dialog / context menu は acrylic 風に寄せる。
- **CommandBar 風 toolbar**。各 page 上部に primary actions をまとめる。
- **ListView/DataGrid 風**。window list は文字列 listbox ではなく、rich row/card と detail pane にする。
- **State visible**。watcher 状態、apply 済み、favorite match、hidden、unavailable audio などを badge で見せる。
- **Keyboard first**。F5 refresh、Enter apply、Ctrl+F search、Del remove favorite、Esc close dialog を入れる。

### 8.2 Top-level layout

```text
+--------------------------------------------------------------------------------+
| custom title bar: app icon  Borderless Oxide        watcher: Running   [−][□][x]|
+-------------------------+------------------------------------------------------+
| navigation rail          | page header                                          |
|  Windows                 |  Windows                                            |
|  Favorites               |  [Refresh] [Apply] [Restore] [Add favorite] Search  |
|  Profiles                |------------------------------------------------------|
|  Settings                | window list / details split view                    |
|  Logs                    |                                                      |
|                          |                                                      |
|  bottom: Help/About      | status bar                                          |
+-------------------------+------------------------------------------------------+
```

Default window size:

- minimum: `960x640`
- default: `1120x720`
- large: `1280x800`

Adaptive behavior:

- width >= 1008: rail expanded, detail pane visible。
- 720 <= width < 1008: rail compact, detail pane collapsible。
- width < 720: rail hidden behind menu button, list becomes single-column cards。

### 8.3 Pages

#### Windows page

目的: 現在 targetable な windows を管理する。

UI:

- Search box: title/process/pid filter。
- Filter chips: `Targetable`, `Applied`, `Matched favorite`, `Hidden excluded`, `All details`。
- Row fields: icon, title, process name, pid, HWND, class name, size, monitor, state badges。
- Detail pane: style bits, ex-style bits, rect, monitor, actions。

Actions:

- Apply borderless
- Restore
- Add favorite from process
- Add favorite from title
- Add favorite from regex
- Set window title
- Hide this process
- Open process path if available

#### Favorites page

目的: C# 版の favorite context menu を、明示的な editor に置き換える。

左: favorites list。右: editor panel。

Editor fields:

- Enabled
- Matcher kind: HWND / PID / process / exact title / regex
- Search text
- Size: fullscreen / specific rect / no change
- Target frame: current monitor / primary / monitor / exact rect
- Monitor picker
- Exact rect editor
- Offset L/T/R/B
- Should maximize
- Topmost
- Remove menus
- Hide Windows taskbar
- Hide mouse cursor
- Lock mouse cursor
- Delay ms
- Mute when background

Actions:

- Save
- Duplicate
- Remove
- Test match
- Apply now
- Import/export TOML snippet

#### Profiles page

Borderless-Gaming には明確な profile concept はないが、Rust 版では modern UX として追加価値が高い。

- `Gaming default`
- `Streaming`
- `No taskbar / no cursor`
- `No size change`

Profile は favorite options の template として扱う。domain には optional に置く。

#### Settings page

C# 版 AppSettings 相当を整理して置く。

- Window detection: poll interval, slow detection, view full details。
- Lifecycle: run on startup, start minimized, close to tray。
- Hotkeys: borderless toggle, mouse lock, mouse hide。
- Tray: show balloon tips, tray icon behavior。
- Safety: confirm taskbar hide, confirm cursor hide, restore all on exit。
- Language: default culture。
- Updates: check GitHub releases。private build では default off。
- Hidden processes: list + reset。

#### Logs page

- Recent domain events。
- Apply/restore failures。
- Audio unavailable warnings。
- Hotkey/tray/startup registration status。

### 8.4 Visual style tokens

```rust
pub struct ThemeTokens {
    pub window_corner_radius: f32,     // 8 or system default
    pub card_corner_radius: f32,       // 8
    pub control_corner_radius: f32,    // 4
    pub spacing_xs: i32,               // 4
    pub spacing_sm: i32,               // 8
    pub spacing_md: i32,               // 12
    pub spacing_lg: i32,               // 24
    pub nav_expanded_width: i32,       // 280
    pub nav_compact_width: i32,        // 56
    pub command_bar_height: i32,       // 48
    pub row_height: i32,               // 64
}
```

色は hard-code せず、system theme と high contrast を読む。

- dark/light: `ShouldAppsUseDarkMode` 相当 or registry/theme API。
- high contrast: `SystemParametersInfoW(SPI_GETHIGHCONTRAST)`。
- DWM dark title bar: `DwmSetWindowAttribute`。

### 8.5 Mica / DWM integration

`borderless-native::dwm` を追加し、GUI window creation 後に呼ぶ。

```rust
pub enum BackdropKind {
    None,
    Mica,
    MicaAlt,
    Acrylic,
}

pub struct DwmWindowEffects;

impl DwmWindowEffects {
    pub fn apply(hwnd: Hwnd, theme: WindowTheme, backdrop: BackdropKind) -> CoreResult<()>;
}
```

内部では `DwmSetWindowAttribute` を使い、次を設定する。

- immersive dark mode
- rounded corners preference
- system backdrop type
- caption/border/text color if supported

unsupported OS では失敗扱いにせず、通常背景へ fallback する。

### 8.6 Direct2D / DirectWrite rendering

既存 Win32 controls をやめ、window list / nav rail / cards は owner-draw で作る。

```text
WM_PAINT
  -> RenderContext::begin(hwnd)
  -> ShellView::paint(ctx, model)
  -> NavRail::paint(ctx, model.nav)
  -> CommandBar::paint(ctx, model.page_actions)
  -> WindowList::paint(ctx, model.windows)
  -> StatusBar::paint(ctx, model.status)
```

入力は hit testing で `UiIntent` に変換する。

```rust
pub enum UiIntent {
    SelectPage(PageId),
    RefreshWindows,
    SelectWindow(Hwnd),
    ApplySelected,
    RestoreSelected,
    OpenFavoriteEditor(FavoriteEditorMode),
    SaveFavorite(FavoriteDraft),
    ToggleWatcherPaused,
    SearchChanged(String),
}
```

## 9. GUI implementation steps

### Step 1: UI Sans I/O crate/module

`borderless-core::ui` を追加する。

- `UiModel`
- `UiIntent`
- `UiEffect`
- `PageId`
- `WindowRow`
- `FavoriteDraft`
- `SettingsDraft`
- `StatusMessage`

Unit test:

- selecting window enables Apply/Restore actions。
- search filters row list。
- dirty favorite draft blocks page change unless saved/discarded。

### Step 2: GuiBridgeActor

`borderless-runtime::gui_bridge` を追加する。

- Controller/Watcher events を `AppSnapshot` に集約。
- GUI は pull (`Snapshot`) と push (`Subscribe`) の両方を使えるようにする。
- GUI thread へは `PostMessageW(hwnd, WM_APP_SNAPSHOT, ...)` で通知する。

### Step 3: DWM / theme / DPI

`borderless-gui::window` で次を実装する。

- `SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)`。
- `WM_DPICHANGED` で layout scale 更新。
- `WM_SETTINGCHANGE` / theme change で tokens 更新。
- `DwmWindowEffects::apply`。

### Step 4: Navigation shell

`borderless-gui::shell` と `controls::nav_rail` を追加。

- PageId: Windows, Favorites, Profiles, Settings, Logs。
- bottom item: Help/About。
- width breakpoints: 1008 / 720。

### Step 5: Window list and command bar

- `controls::command_bar`。
- `controls::window_list`。
- row virtualization は後回しでよい。最初は Vec row の visible range paint で十分。

### Step 6: Favorite editor

- `FavoriteDraft` を core に置く。
- GUI draft -> validated `Favorite` conversion を TypeState にする。

```rust
FavoriteDraft<Editing>
    .validate()? -> FavoriteDraft<Validated>
    .commit(id) -> Favorite
```

### Step 7: Settings / tray / startup / hotkeys

Settings page から `ControllerMsg::Set...` を呼ぶ。

- save は immediate ではなく debounce 300ms でよい。
- 失敗時は inline error + Logs に出す。

## 10. CLI の追加互換

GUI だけでなく CLI も parity を持つ。

```powershell
borderless-cli list --all-details
borderless-cli apply --hwnd 0x123456 --favorite foo
borderless-cli restore --hwnd 0x123456
borderless-cli favorite add --process Game.exe --fullscreen --topmost
borderless-cli favorite edit foo --delay-ms 10000 --mute-in-background
borderless-cli hidden add GameLauncher.exe
borderless-cli hidden reset
borderless-cli settings set close-to-tray true
borderless-cli startup enable --silent --minimize
borderless-cli hotkey set borderless --mod win --key F6
```

GUI args:

```powershell
borderless-gui --silent --minimize
borderless-gui --reset-window-position
borderless-gui --safe-mode
```

`--safe-mode` は config を読むが taskbar/cursor/audio/hotkey/tray を自動適用しない。トラブル復旧用に必須。

## 11. Prioritized implementation milestones

### Milestone 0: Documentation and safety gates

- この文書を `docs/feature-parity-and-modern-gui-roadmap.md` に置く。
- `SAFETY.md` に taskbar/cursor/audio restore policy を追記する。
- `cargo fmt`, `cargo clippy --workspace --all-targets --target x86_64-pc-windows-msvc` を CI 化する。

### Milestone 1: Config parity

- `AppConfig` を AppSettings 相当に拡張。
- TOML migration を追加。
- CLI から settings を変更できるようにする。
- hidden processes を catalog filter に接続する。

### Milestone 2: Window lifecycle parity

- `ManagedWindowRegistry` を ControllerState に導入。
- Watcher prune detection を追加。
- taskbar/cursor/audio restore refcount を追加。
- Pause/resume watcher を追加。

### Milestone 3: OS integration parity

- `startup.rs`
- `tray.rs`
- `hotkey.rs`
- `mouse_lock.rs`
- `title.rs`
- `shell.rs`

この段階で C# 版の実用機能の大半に並ぶ。

### Milestone 4: CoreAudio

- CoreAudio session enumeration。
- foreground tracking。
- mute when unfocused。
- error surface を GUI/Logs に出す。

### Milestone 5: Modern GUI shell

- DWM Mica / dark titlebar / rounded corners。
- Direct2D/DirectWrite render context。
- nav rail + command bar + window list。
- keyboard shortcuts。

### Milestone 6: Favorite editor and settings UI

- Favorites page。
- Settings page。
- Desktop area selector overlay。
- Regex editor/help。

### Milestone 7: Polish

- Localization。
- Installer/MSIX or zip release。
- icons/resources。
- crash-safe restore on exit。
- telemetry は入れない。logs のみ local。

## 12. Test plan

### Unit tests

- Favorite matcher: process exact, title exact, regex invalid/valid。
- Config migration: old TOML -> new AppConfig。
- `BorderlessSession` TypeState transition。
- UI reducer: action enable/disable, dirty draft validation。
- Managed registry prune/refcount。

### Windows integration tests/manual tests

- Notepad apply/restore。
- Multi-monitor current/primary/exact rect。
- DPI 100/150/200%。
- Hide taskbar with two managed windows -> one exits -> taskbar stays hidden -> second exits -> taskbar restores。
- Cursor hide + lock -> Alt+Tab -> unlock。
- Global hotkey Win+F6 toggles foreground window。
- Tray close/minimize behavior。
- Startup task creates/removes correctly。
- CoreAudio mute toggles only target PID sessions。
- Game engine delay: Unreal/GameMaker class-name policy。

### Failure tests

- `SetWindowLongW` fails: GUI shows error, no registry insert。
- `SetWindowPos` fails after style change: attempt restore。
- CoreAudio unavailable: borderless still applies。
- DWM Mica unsupported: fallback background。
- Hotkey conflict: show conflict in Settings page。

## 13. Reference notes

### Project references

- Current workspace: `Cargo.toml`
- Current feature list: `README.md`, `FEATURE_MATRIX.md`
- Current architecture: `ARCHITECTURE.md`
- Current GUI prototype: `crates/borderless-gui/src/gui.rs`
- Current Win32 manipulation: `crates/borderless-native/src/manipulation.rs`
- Current audio skeleton: `crates/borderless-native/src/audio.rs`

### Borderless-Gaming C# references

- Purpose and project overview: <https://github.com/andrewmd5/Borderless-Gaming/blob/master/README.md>
- Process watcher / automatic favorites: <https://github.com/andrewmd5/Borderless-Gaming/blob/master/BorderlessGaming.Logic/Core/ProcessWatcher.cs>
- Window manipulation: <https://github.com/andrewmd5/Borderless-Gaming/blob/master/BorderlessGaming.Logic/Windows/Manipulation.cs>
- Window enumeration: <https://github.com/andrewmd5/Borderless-Gaming/blob/master/BorderlessGaming.Logic/Windows/Windows.cs>
- Favorite model: <https://github.com/andrewmd5/Borderless-Gaming/blob/master/BorderlessGaming.Logic/Models/Favorite.cs>
- App settings: <https://github.com/andrewmd5/Borderless-Gaming/blob/master/BorderlessGaming.Logic/Models/AppSettings.cs>
- Startup task behavior: <https://github.com/andrewmd5/Borderless-Gaming/blob/master/BorderlessGaming.Logic/System/AutoStart.cs>
- Main WinForms UI: <https://github.com/andrewmd5/Borderless-Gaming/blob/master/BorderlessGaming/Forms/MainWindow.cs>

### Windows / GUI references

- `windows-rs` samples README: <https://github.com/microsoft/windows-rs/blob/master/crates/samples/readme.md>
- Materials in Windows: <https://learn.microsoft.com/en-us/windows/apps/design/signature-experiences/materials>
- System backdrops / Mica / Acrylic: <https://learn.microsoft.com/en-us/windows/apps/develop/ui/system-backdrops>
- NavigationView guidance: <https://learn.microsoft.com/en-us/windows/apps/develop/ui/controls/navigationview>
- ListView / GridView guidance: <https://learn.microsoft.com/en-us/windows/apps/develop/ui/controls/listview-and-gridview>

## 14. Non-goals

- C# 版の広告表示や Steam promotion は移植しない。
- GPL コードの移植・複写はしない。挙動を仕様として参照し、Rust 側では独立実装する。
- UI を WebView / Electron / React Native にしない。Windows-only native app として進める。
- `unsafe` を core / runtime / CLI / GUI model に漏らさない。`unsafe` は `borderless-native` と低レベル GUI window/painter に閉じ込め、関数単位で安全な wrapper を用意する。
