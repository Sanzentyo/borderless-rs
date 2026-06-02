use crate::ffi;
use windows::Win32::UI::WindowsAndMessaging::{FindWindowExW, FindWindowW, ShowWindow};

pub(crate) fn set_visible(visible: bool) {
    let cmd = if visible { ffi::SW_SHOW } else { ffi::SW_HIDE };
    show_class("Shell_TrayWnd", cmd);
    show_all_secondary_taskbars(cmd);
}

fn show_class(class_name: &str, cmd: i32) {
    let class = ffi::wide_z(class_name);
    if let Ok(hwnd) = unsafe { FindWindowW(ffi::pcwstr_from_wide(&class), None) } {
        unsafe {
            let _ = ShowWindow(
                hwnd,
                windows::Win32::UI::WindowsAndMessaging::SHOW_WINDOW_CMD(cmd),
            );
        }
    }
}

fn show_all_secondary_taskbars(cmd: i32) {
    let class = ffi::wide_z("Shell_SecondaryTrayWnd");
    let mut hwnd = ffi::null_hwnd();
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
        unsafe {
            let _ = ShowWindow(
                hwnd,
                windows::Win32::UI::WindowsAndMessaging::SHOW_WINDOW_CMD(cmd),
            );
        }
    }
}
