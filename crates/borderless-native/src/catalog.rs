use crate::{ffi, monitor, process};
use borderless_core::profile::ProfileSpan;
use borderless_core::{CoreResult, Pid, ProcessName, WindowCatalog, WindowSnapshot, WindowTitle};
use std::cell::RefCell;
use std::time::Instant;
use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetClientRect, GetWindowLongW, GetWindowRect, GetWindowTextLengthW,
    GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible, WINDOW_LONG_PTR_INDEX,
};
use windows::core::BOOL;

#[derive(Clone, Debug, Default)]
pub struct WindowsCatalog;

impl WindowCatalog for WindowsCatalog {
    fn windows(&self) -> CoreResult<Vec<WindowSnapshot>> {
        Ok(enumerate_windows())
    }

    fn monitors(&self) -> CoreResult<Vec<borderless_core::MonitorSnapshot>> {
        monitor::monitors()
    }

    fn by_hwnd(&self, hwnd: borderless_core::Hwnd) -> CoreResult<Option<WindowSnapshot>> {
        let raw = ffi::hwnd(hwnd);
        if ffi::is_null_hwnd(raw) {
            return Ok(None);
        }
        let process_names = process::process_names();
        Ok(snapshot_from_hwnd(raw, &process_names))
    }
}

fn enumerate_windows() -> Vec<WindowSnapshot> {
    let _span = ProfileSpan::start("native.enumerate_windows");
    thread_local! {
        static WINDOWS: RefCell<Vec<HWND>> = const { RefCell::new(Vec::new()) };
    }

    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let pass = lparam.0;
        let style = u32::from_ne_bytes(
            unsafe { GetWindowLongW(hwnd, WINDOW_LONG_PTR_INDEX(ffi::GWL_STYLE)) }.to_ne_bytes(),
        );
        let visible = unsafe { IsWindowVisible(hwnd).as_bool() };
        let strict = visible
            && (style & 0x00C0_0000) != 0
            && ((style & 0x0080_0000) != 0 || (style & 0x0004_0000) != 0);
        let loose = visible && style != 0;

        if (pass == 0 && strict) || (pass == 1 && loose) {
            WINDOWS.with(|cell| cell.borrow_mut().push(hwnd));
        }
        true.into()
    }

    WINDOWS.with(|cell| cell.borrow_mut().clear());
    unsafe {
        let _ = EnumWindows(Some(enum_proc), LPARAM(0));
        let _ = EnumWindows(Some(enum_proc), LPARAM(1));
    }

    let hwnds = WINDOWS.with(|cell| cell.borrow().clone());
    ProfileSpan::mark(format!(
        "native.enumerate_windows: hwnd candidates={}",
        hwnds.len()
    ));
    let process_names = process::process_names();
    ProfileSpan::mark(format!(
        "native.enumerate_windows: process names={}",
        process_names.len()
    ));
    let snapshots = hwnds
        .into_iter()
        .filter_map(|hwnd| snapshot_from_hwnd(hwnd, &process_names))
        .filter(WindowSnapshot::is_targetable)
        .collect::<Vec<_>>()
        .into_iter()
        .fold(Vec::new(), |mut acc, window| {
            if !acc
                .iter()
                .any(|existing: &WindowSnapshot| existing.hwnd == window.hwnd)
            {
                acc.push(window);
            }
            acc
        });
    ProfileSpan::mark(format!(
        "native.enumerate_windows: targetable snapshots={}",
        snapshots.len()
    ));
    snapshots
}

fn snapshot_from_hwnd(hwnd: HWND, process_names: &[(Pid, ProcessName)]) -> Option<WindowSnapshot> {
    let start = Instant::now();
    let mut raw_rect = RECT::default();
    unsafe { GetWindowRect(hwnd, &raw mut raw_rect) }.ok()?;
    let rect = ffi::rect(raw_rect).ok()?;
    if rect.width().0 <= 0 || rect.height().0 <= 0 {
        return None;
    }

    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&raw mut pid)) };
    let pid = Pid::new(pid)?;
    if pid.get() == std::process::id() {
        return None;
    }
    let process_name = process_names
        .iter()
        .find_map(|(candidate_pid, name)| (pid == *candidate_pid).then(|| name.clone()))
        .unwrap_or_else(|| {
            ProcessName::new(format!("pid-{}.exe", pid.get())).expect("generated process name")
        });

    let style = unsafe { GetWindowLongW(hwnd, WINDOW_LONG_PTR_INDEX(ffi::GWL_STYLE)) };
    let ex_style = unsafe { GetWindowLongW(hwnd, WINDOW_LONG_PTR_INDEX(ffi::GWL_EXSTYLE)) };
    let title = window_text(hwnd);
    let class_name = class_name(hwnd);

    let snapshot = WindowSnapshot {
        hwnd: ffi::from_hwnd(hwnd),
        pid,
        process_name,
        title,
        class_name,
        rect,
        client_rect: client_rect(hwnd),
        style: ffi::style(style),
        ex_style: ffi::ex_style(ex_style),
        is_visible: unsafe { IsWindowVisible(hwnd).as_bool() },
    };
    let elapsed = start.elapsed();
    if elapsed.as_millis() >= 25 {
        ProfileSpan::mark(format!(
            "native.snapshot_from_hwnd: slow hwnd={} pid={} class={} title={} elapsed={elapsed:?}",
            snapshot.hwnd, snapshot.pid, snapshot.class_name, snapshot.title
        ));
    }
    Some(snapshot)
}

fn client_rect(hwnd: HWND) -> Option<borderless_core::Rect> {
    let mut raw_rect = RECT::default();
    unsafe { GetClientRect(hwnd, &raw mut raw_rect) }.ok()?;
    ffi::rect(raw_rect).ok()
}

fn window_text(hwnd: HWND) -> WindowTitle {
    let len = unsafe { GetWindowTextLengthW(hwnd) };
    if len <= 0 {
        return WindowTitle::default();
    }
    let Ok(len) = usize::try_from(len) else {
        return WindowTitle::default();
    };
    let mut buf = vec![0u16; len + 1];
    unsafe {
        let _ = GetWindowTextW(hwnd, &mut buf);
    }
    WindowTitle::new(ffi::pwstr_to_string(&buf))
}

fn class_name(hwnd: HWND) -> String {
    let mut buf = vec![0u16; 256];
    unsafe {
        let _ = GetClassNameW(hwnd, &mut buf);
    }
    ffi::pwstr_to_string(&buf)
}
