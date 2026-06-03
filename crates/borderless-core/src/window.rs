use crate::types::{Hwnd, MonitorId, Pid, ProcessName, Rect, WindowTitle};
use bitflags::bitflags;
use serde::{Deserialize, Serialize};

bitflags! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
    pub struct StyleBits: u32 {
        const BORDER = 0x0080_0000;
        const DLG_FRAME = 0x0040_0000;
        const THICK_FRAME = 0x0004_0000;
        const SYSTEM_MENU = 0x0008_0000;
        const MINIMIZE_BOX = 0x0002_0000;
        const MAXIMIZE_BOX = 0x0001_0000;
        const CAPTION = Self::BORDER.bits() | Self::DLG_FRAME.bits();
        const VISIBLE = 0x1000_0000;
    }
}

bitflags! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
    pub struct ExStyleBits: u32 {
        const DLG_MODAL_FRAME = 0x0000_0001;
        const COMPOSITED = 0x0200_0000;
        const WINDOW_EDGE = 0x0000_0100;
        const CLIENT_EDGE = 0x0000_0200;
        const LAYERED = 0x0008_0000;
        const STATIC_EDGE = 0x0002_0000;
        const TOOL_WINDOW = 0x0000_0080;
        const APP_WINDOW = 0x0004_0000;
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowSnapshot {
    pub hwnd: Hwnd,
    pub pid: Pid,
    pub process_name: ProcessName,
    pub title: WindowTitle,
    pub class_name: String,
    pub rect: Rect,
    pub style: StyleBits,
    pub ex_style: ExStyleBits,
    pub is_visible: bool,
}

impl WindowSnapshot {
    #[must_use]
    pub fn is_targetable(&self) -> bool {
        self.has_targetable_chrome() || self.is_borderless_like()
    }

    #[must_use]
    pub fn is_borderless_like(&self) -> bool {
        self.is_visible
            && !self.hwnd.is_null()
            && self.rect.width().0 > 0
            && self.rect.height().0 > 0
            && !self.style.intersects(Self::target_chrome_bits())
    }

    #[must_use]
    fn has_targetable_chrome(&self) -> bool {
        self.is_visible
            && !self.hwnd.is_null()
            && self.rect.width().0 > 0
            && self.rect.height().0 > 0
            && self.style.intersects(Self::target_chrome_bits())
    }

    fn target_chrome_bits() -> StyleBits {
        StyleBits::CAPTION | StyleBits::BORDER | StyleBits::THICK_FRAME | StyleBits::SYSTEM_MENU
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonitorSnapshot {
    pub id: MonitorId,
    pub rect: Rect,
    pub work_area: Rect,
    pub primary: bool,
}

impl MonitorSnapshot {
    #[must_use]
    pub fn contains_window_origin(self, window: &WindowSnapshot) -> bool {
        self.rect.contains_point(window.rect.left, window.rect.top)
    }

    #[must_use]
    pub fn window_intersection_area(self, window: &WindowSnapshot) -> i64 {
        self.rect.intersection_area(window.rect)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OriginalWindowState {
    pub hwnd: Hwnd,
    pub style: StyleBits,
    pub ex_style: ExStyleBits,
    pub rect: Rect,
    pub topmost: bool,
}

impl From<&WindowSnapshot> for OriginalWindowState {
    fn from(value: &WindowSnapshot) -> Self {
        Self {
            hwnd: value.hwnd,
            style: value.style,
            ex_style: value.ex_style,
            rect: value.rect,
            topmost: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(style: StyleBits) -> WindowSnapshot {
        WindowSnapshot {
            hwnd: Hwnd(42),
            pid: Pid::new(1).unwrap(),
            process_name: ProcessName::new("game.exe").unwrap(),
            title: WindowTitle::new("game"),
            class_name: "GameWindow".to_owned(),
            rect: Rect::new(0, 0, 1280, 720).unwrap(),
            style,
            ex_style: ExStyleBits::default(),
            is_visible: true,
        }
    }

    #[test]
    fn borderless_like_window_stays_targetable_for_restore() {
        let window = snapshot(StyleBits::VISIBLE);

        assert!(window.is_borderless_like());
        assert!(window.is_targetable());
    }

    #[test]
    fn regular_chromed_window_is_targetable_not_borderless() {
        let window = snapshot(StyleBits::VISIBLE | StyleBits::CAPTION | StyleBits::SYSTEM_MENU);

        assert!(!window.is_borderless_like());
        assert!(window.is_targetable());
    }
}
