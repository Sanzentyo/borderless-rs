use crate::action::{EdgeOffsets, MenuPolicy, TargetFrame};
use crate::error::CoreResult;
use crate::types::{FavoriteId, Hwnd, Pid, ProcessName, Rect, WindowTitle};
use crate::window::WindowSnapshot;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum FavoriteMatcher {
    Hwnd(Hwnd),
    Pid(Pid),
    ProcessName(ProcessName),
    ExactTitle(WindowTitle),
    TitleRegex(String),
}

impl FavoriteMatcher {
    pub fn matches(&self, window: &WindowSnapshot) -> CoreResult<bool> {
        Ok(match self {
            Self::Hwnd(hwnd) => window.hwnd == *hwnd,
            Self::Pid(pid) => window.pid == *pid,
            Self::ProcessName(process_name) => process_name
                .as_str()
                .eq_ignore_ascii_case(window.process_name.as_str()),
            Self::ExactTitle(title) => title.as_str() == window.title.as_str(),
            Self::TitleRegex(pattern) => Regex::new(pattern)?.is_match(window.title.as_str()),
        })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FavoriteSize {
    #[default]
    FullScreen,
    Specific {
        rect: Rect,
    },
    NoChange,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct FavoriteOptions {
    #[serde(default)]
    pub size: FavoriteSize,
    #[serde(default = "default_target_frame")]
    pub target_frame: TargetFrame,
    #[serde(default)]
    pub should_maximize: bool,
    #[serde(default)]
    pub top_most: bool,
    #[serde(default = "default_menu_policy")]
    pub menu_policy: MenuPolicy,
    #[serde(default)]
    pub hide_windows_taskbar: bool,
    #[serde(default)]
    pub hide_mouse_cursor: bool,
    #[serde(with = "duration_ms", default, rename = "delay_ms")]
    pub delay: Duration,
    #[serde(default)]
    pub mute_in_background: bool,
    #[serde(default)]
    pub offsets: EdgeOffsets,
}

impl Default for FavoriteOptions {
    fn default() -> Self {
        Self {
            size: FavoriteSize::FullScreen,
            target_frame: TargetFrame::CurrentMonitor,
            should_maximize: true,
            top_most: false,
            menu_policy: MenuPolicy::Keep,
            hide_windows_taskbar: false,
            hide_mouse_cursor: false,
            delay: Duration::ZERO,
            mute_in_background: false,
            offsets: EdgeOffsets::zero(),
        }
    }
}

const fn default_target_frame() -> TargetFrame {
    TargetFrame::CurrentMonitor
}

const fn default_menu_policy() -> MenuPolicy {
    MenuPolicy::Keep
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Favorite {
    pub id: FavoriteId,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    pub matcher: FavoriteMatcher,
    #[serde(flatten)]
    pub options: FavoriteOptions,
}

impl Favorite {
    pub fn matches(&self, window: &WindowSnapshot) -> CoreResult<bool> {
        self.matcher
            .matches(window)
            .map(|matches| self.enabled && matches)
    }
}

const fn default_enabled() -> bool {
    true
}

mod duration_ms {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(value: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let millis = u64::try_from(value.as_millis()).map_err(serde::ser::Error::custom)?;
        serializer.serialize_u64(millis)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        u64::deserialize(deserializer).map(Duration::from_millis)
    }
}
