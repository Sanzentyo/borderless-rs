use anyhow::{Result, anyhow};
use borderless_core::{Hwnd, WindowSnapshot};
use borderless_reacter::{ControllerMsg, spawn_reacter};
use borderless_win::WindowsBackend;
use ractor::ActorRef;
use std::ffi::c_void;
use std::future;
use std::sync::{Mutex, OnceLock};
use std::thread;
use tokio::runtime::Runtime;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, DispatchMessageW,
    GetDlgItem, GetMessageW, HMENU, IDC_ARROW, LB_ADDSTRING, LB_GETCURSEL, LB_RESETCONTENT,
    LBS_HASSTRINGS, LBS_NOINTEGRALHEIGHT, LBS_NOTIFY, LoadCursorW, MSG, PostQuitMessage,
    RegisterClassW, SendMessageW, SetWindowTextW, TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE,
    WM_COMMAND, WM_CREATE, WM_DESTROY, WNDCLASSW, WS_BORDER, WS_CHILD, WS_OVERLAPPEDWINDOW,
    WS_TABSTOP, WS_VISIBLE, WS_VSCROLL,
};
use windows::core::PCWSTR;

static CONTROLLER: OnceLock<ActorRef<ControllerMsg>> = OnceLock::new();
static WINDOWS: OnceLock<Mutex<Vec<WindowSnapshot>>> = OnceLock::new();

const ID_REFRESH: usize = 1001;
const ID_APPLY: usize = 1002;
const ID_RESTORE: usize = 1003;
const ID_WATCH: usize = 1004;
const ID_LIST: i32 = 2001;
const ID_STATUS: i32 = 2002;

pub fn run() -> Result<()> {
    let rt = Runtime::new()?;
    let reacter = rt.block_on(spawn_reacter(WindowsBackend::new()))?;
    CONTROLLER
        .set(reacter.controller)
        .map_err(|_| anyhow!("controller initialized twice"))?;
    WINDOWS.get_or_init(|| Mutex::new(Vec::new()));
    thread::spawn(move || rt.block_on(future::pending::<()>()));

    unsafe {
        let instance = GetModuleHandleW(None)?;
        let class_name = wide("BorderlessOxideWindow");
        let cursor = LoadCursorW(None, IDC_ARROW)?;
        let wc = WNDCLASSW {
            hCursor: cursor,
            hInstance: instance.into(),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(window_proc),
            ..Default::default()
        };
        RegisterClassW(&raw const wc);

        let title = wide("Borderless Oxide");
        let _main_window = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            760,
            500,
            None,
            None,
            Some(instance.into()),
            None,
        )?;

        let mut msg = MSG::default();
        while GetMessageW(&raw mut msg, None, 0, 0).into() {
            let _ = TranslateMessage(&raw const msg);
            DispatchMessageW(&raw const msg);
        }
    }
    Ok(())
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => {
            create_static(hwnd, "Target windows", 20, 18, 160, 20, false);
            create_button(hwnd, "Refresh list", ID_REFRESH, 20, 44, 130);
            create_button(hwnd, "Apply borderless", ID_APPLY, 160, 44, 150);
            create_button(hwnd, "Restore selected", ID_RESTORE, 320, 44, 150);
            create_button(hwnd, "Watch favorites", ID_WATCH, 480, 44, 150);
            create_list_box(hwnd, 20, 92, 700, 300);
            create_static(
                hwnd,
                "Select a window, then apply or restore. Favorites watcher starts automatically.",
                20,
                412,
                700,
                26,
                true,
            );
            refresh_windows(hwnd);
            LRESULT(0)
        }
        WM_COMMAND => {
            match wparam.0 & 0xffff {
                ID_REFRESH => refresh_windows(hwnd),
                ID_APPLY => apply_selected(hwnd),
                ID_RESTORE => restore_selected(hwnd),
                ID_WATCH => set_status(
                    hwnd,
                    "Favorites watcher is running. Add favorites in the TOML config.",
                ),
                _ => {}
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn create_button(hwnd: HWND, text: &str, id: usize, x: i32, y: i32, width: i32) {
    let class = wide("BUTTON");
    let text = wide(text);
    unsafe {
        let _ = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(class.as_ptr()),
            PCWSTR(text.as_ptr()),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP,
            x,
            y,
            width,
            32,
            Some(hwnd),
            Some(HMENU(id as *mut c_void)),
            None,
            None,
        );
    }
}

fn create_list_box(hwnd: HWND, x: i32, y: i32, width: i32, height: i32) {
    let class = wide("LISTBOX");
    let style = WS_CHILD
        | WS_VISIBLE
        | WS_BORDER
        | WS_TABSTOP
        | WS_VSCROLL
        | WINDOW_STYLE((LBS_NOTIFY | LBS_HASSTRINGS | LBS_NOINTEGRALHEIGHT) as u32);
    unsafe {
        let _ = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(class.as_ptr()),
            PCWSTR::null(),
            style,
            x,
            y,
            width,
            height,
            Some(hwnd),
            Some(HMENU(ID_LIST as *mut c_void)),
            None,
            None,
        );
    }
}

fn create_static(hwnd: HWND, text: &str, x: i32, y: i32, width: i32, height: i32, bordered: bool) {
    let class = wide("STATIC");
    let text = wide(text);
    let mut style = WS_CHILD | WS_VISIBLE;
    if bordered {
        style |= WS_BORDER;
    }
    unsafe {
        let _ = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(class.as_ptr()),
            PCWSTR(text.as_ptr()),
            style,
            x,
            y,
            width,
            height,
            Some(hwnd),
            bordered.then_some(HMENU(ID_STATUS as *mut c_void)),
            None,
            None,
        );
    }
}

fn refresh_windows(hwnd: HWND) {
    let Some(controller) = CONTROLLER.get().cloned() else {
        set_status(hwnd, "Controller is not ready.");
        return;
    };
    let result = Runtime::new()
        .map_err(|err| err.to_string())
        .and_then(|rt| {
            rt.block_on(async move { ractor::call!(controller, ControllerMsg::ListWindows) })
                .map_err(|err| err.to_string())
        });

    match result {
        Ok(windows) => {
            if let Some(store) = WINDOWS.get() {
                store
                    .lock()
                    .expect("window snapshot store poisoned")
                    .clone_from(&windows);
            }
            reset_list(hwnd);
            for window in &windows {
                add_list_item(hwnd, &format_window(window));
            }
            set_status(
                hwnd,
                &format!("Loaded {} targetable windows.", windows.len()),
            );
        }
        Err(err) => set_status(hwnd, &format!("Refresh failed: {err}")),
    }
}

fn apply_selected(hwnd: HWND) {
    match selected_hwnd(hwnd) {
        Some(target) => call_hwnd_action(hwnd, "Applied", |reply| {
            ControllerMsg::ApplyByHwnd(target, reply)
        }),
        None => set_status(hwnd, "Select a window before applying borderless."),
    }
}

fn restore_selected(hwnd: HWND) {
    match selected_hwnd(hwnd) {
        Some(target) => call_hwnd_action(hwnd, "Restored", |reply| {
            ControllerMsg::Restore(target, reply)
        }),
        None => set_status(hwnd, "Select a window before restoring."),
    }
}

fn call_hwnd_action<T>(
    hwnd: HWND,
    label: &str,
    message: impl FnOnce(ractor::RpcReplyPort<Result<T, String>>) -> ControllerMsg,
) where
    T: Send + std::fmt::Debug + 'static,
{
    let Some(controller) = CONTROLLER.get().cloned() else {
        set_status(hwnd, "Controller is not ready.");
        return;
    };
    let result = Runtime::new()
        .map_err(|err| err.to_string())
        .and_then(|rt| {
            rt.block_on(async move { ractor::call!(controller, message) })
                .map_err(|err| err.to_string())
        })
        .and_then(|inner| inner);

    match result {
        Ok(value) => {
            set_status(hwnd, &format!("{label}: {value:?}"));
            refresh_windows(hwnd);
        }
        Err(err) => set_status(hwnd, &format!("{label} failed: {err}")),
    }
}

fn selected_hwnd(hwnd: HWND) -> Option<Hwnd> {
    let index = selected_index(hwnd)?;
    WINDOWS
        .get()?
        .lock()
        .ok()?
        .get(index)
        .map(|window| window.hwnd)
}

fn selected_index(hwnd: HWND) -> Option<usize> {
    let list = unsafe { GetDlgItem(Some(hwnd), ID_LIST).ok()? };
    let index = unsafe { SendMessageW(list, LB_GETCURSEL, None, None).0 };
    usize::try_from(index).ok()
}

fn reset_list(hwnd: HWND) {
    if let Ok(list) = unsafe { GetDlgItem(Some(hwnd), ID_LIST) } {
        unsafe {
            let _ = SendMessageW(list, LB_RESETCONTENT, None, None);
        }
    }
}

fn add_list_item(hwnd: HWND, text: &str) {
    let Ok(list) = (unsafe { GetDlgItem(Some(hwnd), ID_LIST) }) else {
        return;
    };
    let text = wide(text);
    unsafe {
        let _ = SendMessageW(
            list,
            LB_ADDSTRING,
            None,
            Some(LPARAM(text.as_ptr() as isize)),
        );
    }
}

fn set_status(hwnd: HWND, text: &str) {
    let Ok(status) = (unsafe { GetDlgItem(Some(hwnd), ID_STATUS) }) else {
        return;
    };
    let text = wide(text);
    unsafe {
        let _ = SetWindowTextW(status, PCWSTR(text.as_ptr()));
    }
}

fn format_window(window: &WindowSnapshot) -> String {
    let title = window.title.as_str();
    let title = if title.is_empty() {
        "(untitled)"
    } else {
        title
    };
    format!(
        "{} | pid={} | {} | {} | {}x{}",
        window.hwnd,
        window.pid,
        window.process_name,
        title,
        window.rect.width().0,
        window.rect.height().0
    )
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
