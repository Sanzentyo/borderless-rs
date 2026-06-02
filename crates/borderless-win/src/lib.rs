#![cfg_attr(not(windows), forbid(unsafe_code))]

#[cfg(not(windows))]
compile_error!("borderless-win is Windows-only; build for x86_64-pc-windows-msvc");

#[cfg(windows)]
mod audio;
#[cfg(windows)]
mod catalog;
#[cfg(windows)]
mod cursor;
#[cfg(windows)]
mod ffi;
#[cfg(windows)]
mod manipulation;
#[cfg(windows)]
mod monitor;
#[cfg(windows)]
mod process;
#[cfg(windows)]
mod store;
#[cfg(windows)]
mod taskbar;

#[cfg(windows)]
pub use audio::AudioSessions;
#[cfg(windows)]
pub use catalog::WindowsCatalog;
#[cfg(windows)]
pub use cursor::CursorVisibility;
#[cfg(windows)]
pub use manipulation::WindowsManipulator;
#[cfg(windows)]
pub use store::TomlSettingsStore;

#[cfg(windows)]
use borderless_core::{CoreResult, EventSink};

#[cfg(windows)]
#[derive(Clone, Debug, Default)]
pub struct WindowsBackend {
    catalog: WindowsCatalog,
    manipulator: WindowsManipulator,
    store: TomlSettingsStore,
}

#[cfg(windows)]
impl WindowsBackend {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(windows)]
impl borderless_core::WindowCatalog for WindowsBackend {
    fn windows(&self) -> CoreResult<Vec<borderless_core::WindowSnapshot>> {
        borderless_core::WindowCatalog::windows(&self.catalog)
    }

    fn monitors(&self) -> CoreResult<Vec<borderless_core::MonitorSnapshot>> {
        borderless_core::WindowCatalog::monitors(&self.catalog)
    }
}

#[cfg(windows)]
impl borderless_core::WindowManipulator for WindowsBackend {
    fn apply_plan(
        &self,
        plan: &borderless_core::BorderlessPlan,
    ) -> CoreResult<borderless_core::OriginalWindowState> {
        borderless_core::WindowManipulator::apply_plan(&self.manipulator, plan)
    }

    fn restore_original(&self, original: &borderless_core::OriginalWindowState) -> CoreResult<()> {
        borderless_core::WindowManipulator::restore_original(&self.manipulator, original)
    }

    fn set_taskbar_visible(&self, visible: bool) -> CoreResult<()> {
        borderless_core::WindowManipulator::set_taskbar_visible(&self.manipulator, visible)
    }

    fn set_cursor_visible(&self, visible: bool) -> CoreResult<()> {
        borderless_core::WindowManipulator::set_cursor_visible(&self.manipulator, visible)
    }

    fn set_process_muted(&self, pid: borderless_core::Pid, muted: bool) -> CoreResult<()> {
        borderless_core::WindowManipulator::set_process_muted(&self.manipulator, pid, muted)
    }
}

#[cfg(windows)]
impl borderless_core::SettingsStore for WindowsBackend {
    fn load_config(&self) -> CoreResult<borderless_core::AppConfig> {
        borderless_core::SettingsStore::load_config(&self.store)
    }

    fn save_config(&self, config: &borderless_core::AppConfig) -> CoreResult<()> {
        borderless_core::SettingsStore::save_config(&self.store, config)
    }
}

#[cfg(windows)]
impl EventSink for WindowsBackend {
    fn emit(&self, event: borderless_core::reducer::DomainEvent) {
        tracing::debug!(?event, "domain event");
    }
}
