#![forbid(unsafe_code)]

pub mod action;
pub mod backend;
pub mod config;
pub mod error;
pub mod favorite;
pub mod profile;
pub mod reducer;
pub mod session;
pub mod types;
pub mod window;

pub use action::{BorderlessPlan, EdgeOffsets, MenuPolicy, Placement, TargetFrame};
pub use backend::{AppliedStateStore, EventSink, SettingsStore, WindowCatalog, WindowManipulator};
pub use config::{AppConfig, PollInterval};
pub use error::{CoreError, CoreResult};
pub use favorite::{Favorite, FavoriteMatcher, FavoriteOptions, FavoriteSize};
pub use reducer::{AppModel, Effect, Intent};
pub use session::{Applied, BorderlessSession, Observed, Prepared, Restored};
pub use types::{FavoriteId, Hwnd, MonitorId, Pid, Pixels, ProcessName, Rect, WindowTitle};
pub use window::{ExStyleBits, MonitorSnapshot, OriginalWindowState, StyleBits, WindowSnapshot};
