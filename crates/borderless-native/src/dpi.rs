use borderless_core::{Hwnd, ScaleFactor};
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForWindow, SetProcessDpiAwarenessContext,
};

use crate::ffi;

pub fn enable_per_monitor_awareness() {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
}

#[must_use]
pub fn window_scale_factor(hwnd: Hwnd) -> ScaleFactor {
    let dpi = unsafe { GetDpiForWindow(ffi::hwnd(hwnd)) };
    ScaleFactor::new(f64::from(dpi) / 96.0).unwrap_or_default()
}
