use crate::ffi;
use borderless_core::profile::ProfileSpan;
use borderless_core::{CoreResult, MonitorId, MonitorSnapshot};
use std::cell::RefCell;
use windows::Win32::Foundation::{LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
};
use windows::core::BOOL;

pub(crate) fn monitors() -> CoreResult<Vec<MonitorSnapshot>> {
    let _span = ProfileSpan::start("native.monitors");
    thread_local! {
        static MONITORS: RefCell<Vec<MonitorSnapshot>> = const { RefCell::new(Vec::new()) };
    }

    unsafe extern "system" fn enum_monitor_proc(
        hmonitor: HMONITOR,
        _hdc: HDC,
        _rect: *mut RECT,
        _lparam: LPARAM,
    ) -> BOOL {
        let cb_size = u32::try_from(std::mem::size_of::<MONITORINFO>())
            .expect("MONITORINFO size fits in u32");
        let mut info = MONITORINFO {
            cbSize: cb_size,
            ..Default::default()
        };

        if unsafe { GetMonitorInfoW(hmonitor, &raw mut info).as_bool() }
            && let (Ok(rect), Ok(work_area)) = (ffi::rect(info.rcMonitor), ffi::rect(info.rcWork))
        {
            MONITORS.with(|cell| {
                cell.borrow_mut().push(MonitorSnapshot {
                    id: MonitorId(hmonitor.0 as isize),
                    rect,
                    work_area,
                    primary: (info.dwFlags & 1) == 1,
                });
            });
        }
        true.into()
    }

    MONITORS.with(|cell| cell.borrow_mut().clear());
    unsafe {
        ffi::ok_bool(EnumDisplayMonitors(
            None,
            None,
            Some(enum_monitor_proc),
            LPARAM(0),
        ))
        .map_err(|_| borderless_core::CoreError::Transition("EnumDisplayMonitors failed"))?;
    }
    let monitors = MONITORS.with(|cell| cell.borrow().clone());
    ProfileSpan::mark(format!("native.monitors: count={}", monitors.len()));
    Ok(monitors)
}

#[allow(dead_code)]
fn _keep_monitorinfoex_type(_: MONITORINFOEXW) {}
