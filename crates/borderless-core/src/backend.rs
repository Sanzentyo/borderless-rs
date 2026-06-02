use crate::action::BorderlessPlan;
use crate::config::AppConfig;
use crate::error::CoreResult;
use crate::types::{Hwnd, Pid, ProcessName, WindowTitle};
use crate::window::{MonitorSnapshot, OriginalWindowState, WindowSnapshot};

pub trait WindowCatalog: Send + Sync {
    fn windows(&self) -> CoreResult<Vec<WindowSnapshot>>;
    fn monitors(&self) -> CoreResult<Vec<MonitorSnapshot>>;

    fn by_hwnd(&self, hwnd: Hwnd) -> CoreResult<Option<WindowSnapshot>> {
        self.windows()
            .map(|windows| windows.into_iter().find(|window| window.hwnd == hwnd))
    }

    fn by_pid(&self, pid: Pid) -> CoreResult<Option<WindowSnapshot>> {
        self.windows()
            .map(|windows| windows.into_iter().find(|window| window.pid == pid))
    }

    fn by_process_name(&self, name: &ProcessName) -> CoreResult<Option<WindowSnapshot>> {
        self.windows().map(|windows| {
            windows.into_iter().find(|window| {
                name.as_str()
                    .eq_ignore_ascii_case(window.process_name.as_str())
            })
        })
    }

    fn by_title(&self, title: &WindowTitle) -> CoreResult<Option<WindowSnapshot>> {
        self.windows().map(|windows| {
            windows
                .into_iter()
                .find(|window| window.title.as_str() == title.as_str())
        })
    }
}

pub trait WindowManipulator: Send + Sync {
    fn apply_plan(&self, plan: &BorderlessPlan) -> CoreResult<OriginalWindowState>;
    fn restore_original(&self, original: &OriginalWindowState) -> CoreResult<()>;
    fn set_taskbar_visible(&self, visible: bool) -> CoreResult<()>;
    fn set_cursor_visible(&self, visible: bool) -> CoreResult<()>;
    fn set_process_muted(&self, pid: Pid, muted: bool) -> CoreResult<()>;
}

pub trait SettingsStore: Send + Sync {
    fn load_config(&self) -> CoreResult<AppConfig>;
    fn save_config(&self, config: &AppConfig) -> CoreResult<()>;
}

pub trait EventSink: Send + Sync {
    fn emit(&self, event: crate::reducer::DomainEvent);
}
