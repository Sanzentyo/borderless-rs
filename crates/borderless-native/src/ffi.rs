use borderless_core::{
    ExStyleBits, Hwnd, MonitorId, PhysicalPx, PhysicalRect, Pid, ProcessName, StyleBits,
};
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::core::{BOOL, Error as WinError, PCWSTR, PWSTR};

pub(crate) fn ok_bool(value: BOOL) -> Result<(), WinError> {
    if value.as_bool() {
        Ok(())
    } else {
        Err(WinError::from_thread())
    }
}

pub(crate) fn hwnd(value: Hwnd) -> HWND {
    HWND(value.0 as _)
}

pub(crate) fn null_hwnd() -> HWND {
    HWND(std::ptr::null_mut())
}

pub(crate) fn is_null_hwnd(value: HWND) -> bool {
    value.0.is_null()
}

pub(crate) fn from_hwnd(value: HWND) -> Hwnd {
    Hwnd(value.0 as isize)
}

pub(crate) fn rect(value: RECT) -> borderless_core::CoreResult<PhysicalRect> {
    PhysicalRect::new(value.left, value.top, value.right, value.bottom)
}

pub(crate) fn style(value: i32) -> StyleBits {
    StyleBits::from_bits_truncate(u32::from_ne_bytes(value.to_ne_bytes()))
}

pub(crate) fn ex_style(value: i32) -> ExStyleBits {
    ExStyleBits::from_bits_truncate(u32::from_ne_bytes(value.to_ne_bytes()))
}

pub(crate) fn wide_z(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

pub(crate) fn pwstr_to_string(buf: &[u16]) -> String {
    let end = buf.iter().position(|ch| *ch == 0).unwrap_or(buf.len());
    OsString::from_wide(&buf[..end])
        .to_string_lossy()
        .into_owned()
}

pub(crate) const GWL_STYLE: i32 = -16;
pub(crate) const GWL_EXSTYLE: i32 = -20;

pub(crate) const SW_HIDE: i32 = 0;
pub(crate) const SW_SHOW: i32 = 5;
pub(crate) const SW_MAXIMIZE: i32 = 3;

pub(crate) const SWP_NOSIZE: u32 = 0x0001;
pub(crate) const SWP_NOMOVE: u32 = 0x0002;
pub(crate) const SWP_NOZORDER: u32 = 0x0004;
pub(crate) const SWP_NOACTIVATE: u32 = 0x0010;
pub(crate) const SWP_FRAMECHANGED: u32 = 0x0020;
pub(crate) const SWP_SHOWWINDOW: u32 = 0x0040;
pub(crate) const SWP_NOOWNERZORDER: u32 = 0x0200;
pub(crate) const SWP_NOSENDCHANGING: u32 = 0x0400;

pub(crate) const HWND_TOPMOST: isize = -1;
pub(crate) const HWND_NOTOPMOST: isize = -2;

pub(crate) fn hwnd_topmost() -> HWND {
    HWND(HWND_TOPMOST as _)
}

pub(crate) fn hwnd_notopmost() -> HWND {
    HWND(HWND_NOTOPMOST as _)
}

#[allow(dead_code)]
pub(crate) const fn lparam(value: isize) -> LPARAM {
    LPARAM(value)
}

#[allow(dead_code)]
pub(crate) const fn pixels(value: i32) -> PhysicalPx {
    PhysicalPx(value)
}

#[allow(dead_code)]
pub(crate) const fn monitor_id(value: isize) -> MonitorId {
    MonitorId(value)
}

#[allow(dead_code)]
pub(crate) fn process_name(value: impl Into<String>) -> borderless_core::CoreResult<ProcessName> {
    ProcessName::new(value)
}

#[allow(dead_code)]
pub(crate) fn pid(value: u32) -> Option<Pid> {
    Pid::new(value)
}

#[allow(dead_code)]
pub(crate) fn pcwstr_from_wide(wide: &[u16]) -> PCWSTR {
    PCWSTR(wide.as_ptr())
}

#[allow(dead_code)]
pub(crate) fn pwstr_from_slice(wide: &mut [u16]) -> PWSTR {
    PWSTR(wide.as_mut_ptr())
}
