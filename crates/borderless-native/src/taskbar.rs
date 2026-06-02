use crate::ffi;
use borderless_core::Rect;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowExW, FindWindowW, GetWindowRect, ShowWindow,
};

pub(crate) fn set_visible(visible: bool) {
    let cmd = if visible { ffi::SW_SHOW } else { ffi::SW_HIDE };
    show_class("Shell_TrayWnd", cmd);
    show_all_secondary_taskbars(cmd);
}

pub(crate) fn set_visible_for_rect(visible: bool, target: Rect) {
    let cmd = if visible { ffi::SW_SHOW } else { ffi::SW_HIDE };
    let taskbars = taskbar_windows();
    let mut changed = false;
    for hwnd in taskbars {
        if window_rect(hwnd).is_some_and(|rect| intersects(rect, target)) {
            show_window(hwnd, cmd);
            changed = true;
        }
    }
    if !changed {
        set_visible(visible);
    }
}

fn show_class(class_name: &str, cmd: i32) {
    let class = ffi::wide_z(class_name);
    if let Ok(hwnd) = unsafe { FindWindowW(ffi::pcwstr_from_wide(&class), None) } {
        show_window(hwnd, cmd);
    }
}

fn show_all_secondary_taskbars(cmd: i32) {
    for hwnd in secondary_taskbars() {
        show_window(hwnd, cmd);
    }
}

fn taskbar_windows() -> Vec<HWND> {
    let mut windows = Vec::new();
    let class = ffi::wide_z("Shell_TrayWnd");
    if let Ok(hwnd) = unsafe { FindWindowW(ffi::pcwstr_from_wide(&class), None) }
        && !ffi::is_null_hwnd(hwnd)
    {
        windows.push(hwnd);
    }
    windows.extend(secondary_taskbars());
    windows
}

fn secondary_taskbars() -> Vec<HWND> {
    let class = ffi::wide_z("Shell_SecondaryTrayWnd");
    let mut hwnd = ffi::null_hwnd();
    let mut windows = Vec::new();
    loop {
        let Ok(next) =
            (unsafe { FindWindowExW(None, Some(hwnd), ffi::pcwstr_from_wide(&class), None) })
        else {
            break;
        };
        hwnd = next;
        if ffi::is_null_hwnd(hwnd) {
            break;
        }
        windows.push(hwnd);
    }
    windows
}

fn show_window(hwnd: HWND, cmd: i32) {
    unsafe {
        let _ = ShowWindow(
            hwnd,
            windows::Win32::UI::WindowsAndMessaging::SHOW_WINDOW_CMD(cmd),
        );
    }
}

fn window_rect(hwnd: HWND) -> Option<Rect> {
    let mut raw = windows::Win32::Foundation::RECT::default();
    unsafe { GetWindowRect(hwnd, &raw mut raw) }.ok()?;
    ffi::rect(raw).ok()
}

fn intersects(left: Rect, right: Rect) -> bool {
    left.left.0 < right.right.0
        && left.right.0 > right.left.0
        && left.top.0 < right.bottom.0
        && left.bottom.0 > right.top.0
}
