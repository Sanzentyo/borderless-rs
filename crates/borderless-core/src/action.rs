use crate::types::{MonitorId, Pixels, Rect};
use crate::window::{ExStyleBits, OriginalWindowState, StyleBits};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EdgeOffsets {
    pub left: Pixels,
    pub top: Pixels,
    pub right: Pixels,
    pub bottom: Pixels,
}

impl EdgeOffsets {
    #[must_use]
    pub const fn zero() -> Self {
        Self {
            left: Pixels(0),
            top: Pixels(0),
            right: Pixels(0),
            bottom: Pixels(0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TargetFrame {
    CurrentMonitor,
    PrimaryMonitor,
    Monitor(MonitorId),
    Exact(Rect),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MenuPolicy {
    Keep,
    Remove,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Placement {
    pub rect: Rect,
    pub topmost: bool,
    pub maximize: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BorderlessPlan {
    pub original: OriginalWindowState,
    pub new_style: StyleBits,
    pub new_ex_style: ExStyleBits,
    pub placement: Placement,
    pub menu_policy: MenuPolicy,
    pub hide_windows_taskbar: bool,
    pub hide_mouse_cursor: bool,
    pub mute_in_background: bool,
}

impl BorderlessPlan {
    #[must_use]
    pub fn reversible(original: OriginalWindowState, placement: Placement) -> Self {
        let new_style = original.style
            & !(StyleBits::CAPTION
                | StyleBits::BORDER
                | StyleBits::DLG_FRAME
                | StyleBits::THICK_FRAME
                | StyleBits::SYSTEM_MENU
                | StyleBits::MINIMIZE_BOX
                | StyleBits::MAXIMIZE_BOX);
        let new_ex_style = original.ex_style
            & !(ExStyleBits::DLG_MODAL_FRAME
                | ExStyleBits::COMPOSITED
                | ExStyleBits::WINDOW_EDGE
                | ExStyleBits::CLIENT_EDGE
                | ExStyleBits::LAYERED
                | ExStyleBits::STATIC_EDGE
                | ExStyleBits::TOOL_WINDOW
                | ExStyleBits::APP_WINDOW);

        Self {
            original,
            new_style,
            new_ex_style,
            placement,
            menu_policy: MenuPolicy::Keep,
            hide_windows_taskbar: false,
            hide_mouse_cursor: false,
            mute_in_background: false,
        }
    }
}
