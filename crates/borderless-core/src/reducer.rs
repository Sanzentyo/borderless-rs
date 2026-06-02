use crate::config::AppConfig;
use crate::favorite::Favorite;
use crate::types::{FavoriteId, Hwnd, Pid, ProcessName, WindowTitle};
use crate::window::WindowSnapshot;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Intent {
    RefreshWindows,
    ApplyByHwnd { hwnd: Hwnd },
    ApplyByPid { pid: Pid },
    ApplyByProcessName { process_name: ProcessName },
    ApplyByTitle { title: WindowTitle },
    Restore { hwnd: Hwnd },
    AddFavorite { favorite: Favorite },
    RemoveFavorite { id: FavoriteId },
    SaveConfig,
    StartWatching,
    StopWatching,
    ToggleTaskbar { visible: bool },
    ToggleCursor { visible: bool },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Effect {
    QueryWindows,
    ApplyByHwnd { hwnd: Hwnd },
    ApplyByPid { pid: Pid },
    ApplyByProcessName { process_name: ProcessName },
    ApplyByTitle { title: WindowTitle },
    Restore { hwnd: Hwnd },
    SaveConfig,
    StartWatcher,
    StopWatcher,
    SetTaskbarVisible { visible: bool },
    SetCursorVisible { visible: bool },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DomainEvent {
    WindowsRefreshed { windows: Vec<WindowSnapshot> },
    WindowApplied { hwnd: Hwnd },
    WindowRestored { hwnd: Hwnd },
    ConfigSaved,
    WatcherStarted,
    WatcherStopped,
    Error { message: String },
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppModel {
    pub config: AppConfig,
    pub windows: Vec<WindowSnapshot>,
    pub watching: bool,
}

impl AppModel {
    pub fn reduce(&mut self, intent: Intent) -> Vec<Effect> {
        match intent {
            Intent::RefreshWindows => vec![Effect::QueryWindows],
            Intent::ApplyByHwnd { hwnd } => vec![Effect::ApplyByHwnd { hwnd }],
            Intent::ApplyByPid { pid } => vec![Effect::ApplyByPid { pid }],
            Intent::ApplyByProcessName { process_name } => {
                vec![Effect::ApplyByProcessName { process_name }]
            }
            Intent::ApplyByTitle { title } => vec![Effect::ApplyByTitle { title }],
            Intent::Restore { hwnd } => vec![Effect::Restore { hwnd }],
            Intent::AddFavorite { favorite } => {
                self.config.favorites.push(favorite);
                vec![Effect::SaveConfig]
            }
            Intent::RemoveFavorite { id } => {
                self.config.favorites.retain(|favorite| favorite.id != id);
                vec![Effect::SaveConfig]
            }
            Intent::SaveConfig => vec![Effect::SaveConfig],
            Intent::StartWatching => {
                self.watching = true;
                vec![Effect::StartWatcher]
            }
            Intent::StopWatching => {
                self.watching = false;
                vec![Effect::StopWatcher]
            }
            Intent::ToggleTaskbar { visible } => vec![Effect::SetTaskbarVisible { visible }],
            Intent::ToggleCursor { visible } => vec![Effect::SetCursorVisible { visible }],
        }
    }

    pub fn apply_event(&mut self, event: DomainEvent) {
        match event {
            DomainEvent::WindowsRefreshed { windows } => self.windows = windows,
            DomainEvent::WatcherStarted => self.watching = true,
            DomainEvent::WatcherStopped => self.watching = false,
            DomainEvent::WindowApplied { .. }
            | DomainEvent::WindowRestored { .. }
            | DomainEvent::ConfigSaved
            | DomainEvent::Error { .. } => {}
        }
    }
}
