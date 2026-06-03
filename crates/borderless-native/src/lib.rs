mod audio;
mod catalog;
mod cursor;
mod ffi;
mod manipulation;
mod monitor;
mod process;
mod store;
mod taskbar;
mod visuals;

pub use audio::AudioSessions;
pub use catalog::WindowsCatalog;
pub use cursor::CursorVisibility;
pub use manipulation::WindowsManipulator;
pub use store::TomlSettingsStore;
pub use visuals::{WindowVisualAssets, WindowVisuals};

use borderless_core::{CoreResult, EventSink};

#[derive(Clone, Debug, Default)]
pub struct NativeBackend {
    catalog: WindowsCatalog,
    manipulator: WindowsManipulator,
    store: TomlSettingsStore,
}

impl NativeBackend {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl borderless_core::WindowCatalog for NativeBackend {
    fn windows(&self) -> CoreResult<Vec<borderless_core::WindowSnapshot>> {
        borderless_core::WindowCatalog::windows(&self.catalog)
    }

    fn monitors(&self) -> CoreResult<Vec<borderless_core::MonitorSnapshot>> {
        borderless_core::WindowCatalog::monitors(&self.catalog)
    }
}

impl borderless_core::WindowManipulator for NativeBackend {
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

impl borderless_core::SettingsStore for NativeBackend {
    fn load_config(&self) -> CoreResult<borderless_core::AppConfig> {
        borderless_core::SettingsStore::load_config(&self.store)
    }

    fn save_config(&self, config: &borderless_core::AppConfig) -> CoreResult<()> {
        borderless_core::SettingsStore::save_config(&self.store, config)
    }
}

impl EventSink for NativeBackend {
    fn emit(&self, event: borderless_core::reducer::DomainEvent) {
        tracing::debug!(?event, "domain event");
    }
}
