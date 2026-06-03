use std::ops::Deref;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Locale {
    En,
    Ja,
}

impl Locale {
    #[must_use]
    pub fn detect() -> Self {
        std::env::var("BORDERLESS_LOCALE")
            .or_else(|_| std::env::var("LANG"))
            .map_or(Self::En, |value| {
                if value.to_ascii_lowercase().starts_with("ja") {
                    Self::Ja
                } else {
                    Self::En
                }
            })
    }

    #[must_use]
    pub const fn tr(self) -> Text {
        match self {
            Self::En => Text(&TextTable::EN),
            Self::Ja => Text(&TextTable::JA),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Text(&'static TextTable);

impl Deref for Text {
    type Target = TextTable;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

#[derive(Debug)]
pub struct TextTable {
    pub app_title: &'static str,
    pub nav_windows: &'static str,
    pub nav_favorites: &'static str,
    pub nav_settings: &'static str,
    pub nav_logs: &'static str,
    pub search_windows: &'static str,
    pub refresh: &'static str,
    pub apply_selected: &'static str,
    pub apply_aspect_fit: &'static str,
    pub aspect_preset: &'static str,
    pub aspect_width: &'static str,
    pub aspect_height: &'static str,
    pub target_display: &'static str,
    pub display_current: &'static str,
    pub display_primary: &'static str,
    pub display_monitor: &'static str,
    pub restore_selected: &'static str,
    pub add_favorite: &'static str,
    pub targetable: &'static str,
    pub selected: &'static str,
    pub apply: &'static str,
    pub restore: &'static str,
    pub favorite: &'static str,
    pub details: &'static str,
    pub process: &'static str,
    pub pid: &'static str,
    pub hwnd: &'static str,
    pub class: &'static str,
    pub rect: &'static str,
    pub size: &'static str,
    pub preview: &'static str,
    pub preview_unavailable: &'static str,
    pub empty_windows_title: &'static str,
    pub empty_windows_message: &'static str,
    pub empty_selection_title: &'static str,
    pub empty_selection_message: &'static str,
    pub favorites_message: &'static str,
    pub add_selected_favorite: &'static str,
    pub no_selected_window: &'static str,
    pub no_selected_window_message: &'static str,
    pub settings_intro: &'static str,
    pub environment_controls: &'static str,
    pub show_taskbar: &'static str,
    pub hide_taskbar: &'static str,
    pub show_cursor: &'static str,
    pub hide_cursor: &'static str,
    pub watcher_actor: &'static str,
    pub reset_environment_on_exit: &'static str,
    pub running: &'static str,
    pub paused: &'static str,
    pub logs_intro: &'static str,
    pub copy_logs: &'static str,
    pub logs_copied: &'static str,
    pub logs_copied_message: &'static str,
}

impl TextTable {
    pub const EN: Self = Self {
        app_title: "Borderless Oxide",
        nav_windows: "Windows",
        nav_favorites: "Favorites",
        nav_settings: "Settings",
        nav_logs: "Logs",
        search_windows: "Search windows",
        refresh: "Refresh",
        apply_selected: "Apply",
        apply_aspect_fit: "Aspect fit",
        aspect_preset: "Aspect",
        aspect_width: "W",
        aspect_height: "H",
        target_display: "Display",
        display_current: "Current",
        display_primary: "Primary",
        display_monitor: "Monitor",
        restore_selected: "Restore",
        add_favorite: "Favorite",
        targetable: "Targetable",
        selected: "Selected",
        apply: "Apply",
        restore: "Restore",
        favorite: "Favorite",
        details: "Details",
        process: "Process",
        pid: "PID",
        hwnd: "HWND",
        class: "Class",
        rect: "Rect",
        size: "Size",
        preview: "Preview",
        preview_unavailable: "Preview unavailable",
        empty_windows_title: "No targetable windows",
        empty_windows_message: "Launch a game in windowed mode, then refresh the list.",
        empty_selection_title: "Nothing selected",
        empty_selection_message: "Select a target window to inspect and manage it.",
        favorites_message: "Create a process-name favorite from the selected window. Favorites are stored in TOML and used by the watcher.",
        add_selected_favorite: "Add selected window",
        no_selected_window: "No window selected",
        no_selected_window_message: "Select a window on the Windows page first.",
        settings_intro: "Shared runtime controls used by both CLI and GUI.",
        environment_controls: "Environment",
        show_taskbar: "Show taskbar",
        hide_taskbar: "Hide taskbar",
        show_cursor: "Show cursor",
        hide_cursor: "Hide cursor",
        watcher_actor: "Watcher",
        reset_environment_on_exit: "Reset taskbar/cursor on exit",
        running: "Running",
        paused: "Paused",
        logs_intro: "Recent UI and runtime actions.",
        copy_logs: "Copy",
        logs_copied: "Logs copied",
        logs_copied_message: "Recent logs are on the clipboard.",
    };

    pub const JA: Self = Self {
        app_title: "Borderless Oxide",
        nav_windows: "ウィンドウ",
        nav_favorites: "お気に入り",
        nav_settings: "設定",
        nav_logs: "ログ",
        search_windows: "ウィンドウを検索",
        refresh: "更新",
        apply_selected: "適用",
        apply_aspect_fit: "比率固定",
        aspect_preset: "比率",
        aspect_width: "横",
        aspect_height: "縦",
        target_display: "表示先",
        display_current: "現在",
        display_primary: "プライマリ",
        display_monitor: "モニター",
        restore_selected: "復元",
        add_favorite: "登録",
        targetable: "対象",
        selected: "選択中",
        apply: "適用",
        restore: "復元",
        favorite: "登録",
        details: "詳細",
        process: "プロセス",
        pid: "PID",
        hwnd: "HWND",
        class: "クラス",
        rect: "矩形",
        size: "サイズ",
        preview: "プレビュー",
        preview_unavailable: "プレビューなし",
        empty_windows_title: "対象ウィンドウがありません",
        empty_windows_message: "ゲームをウィンドウモードで起動してから更新してください。",
        empty_selection_title: "未選択",
        empty_selection_message: "対象ウィンドウを選択すると詳細を確認できます。",
        favorites_message: "選択中のウィンドウからプロセス名のお気に入りを作成します。設定はTOMLに保存され、監視処理で使われます。",
        add_selected_favorite: "選択中を登録",
        no_selected_window: "ウィンドウ未選択",
        no_selected_window_message: "まずウィンドウ画面で対象を選択してください。",
        settings_intro: "CLIとGUIで共有する実行環境の操作です。",
        environment_controls: "環境",
        show_taskbar: "タスクバー表示",
        hide_taskbar: "タスクバー非表示",
        show_cursor: "カーソル表示",
        hide_cursor: "カーソル非表示",
        watcher_actor: "監視",
        reset_environment_on_exit: "終了時にタスクバー/カーソルを戻す",
        running: "実行中",
        paused: "停止中",
        logs_intro: "最近のUI操作と実行結果です。",
        copy_logs: "コピー",
        logs_copied: "ログをコピーしました",
        logs_copied_message: "最近のログをクリップボードに入れました。",
    };
}
