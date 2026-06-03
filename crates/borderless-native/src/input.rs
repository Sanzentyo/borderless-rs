use crate::ffi;
use borderless_core::{Hwnd, Rect};
use std::sync::{Mutex, Once, OnceLock};
use windows::Win32::Foundation::{LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, VK_CONTROL, VK_LBUTTON, VK_MBUTTON, VK_RBUTTON, VK_SHIFT, VK_XBUTTON1, VK_XBUTTON2,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetForegroundWindow, GetMessageW, HHOOK, HOOKPROC, MSG, MSLLHOOKSTRUCT,
    PostMessageW, SetWindowsHookExW, WH_MOUSE_LL, WINDOWS_HOOK_ID, WM_LBUTTONDOWN, WM_LBUTTONUP,
    WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEMOVE, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_XBUTTONDOWN,
    WM_XBUTTONUP,
};

const MK_LBUTTON: u16 = 0x0001;
const MK_RBUTTON: u16 = 0x0002;
const MK_SHIFT: u16 = 0x0004;
const MK_CONTROL: u16 = 0x0008;
const MK_MBUTTON: u16 = 0x0010;
const MK_XBUTTON1: u16 = 0x0020;
const MK_XBUTTON2: u16 = 0x0040;

#[derive(Clone, Debug)]
pub struct InputScaler {
    state: &'static Mutex<HookState>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MouseTransform {
    pub hwnd: Hwnd,
    pub visual_rect: Rect,
    pub source_client_rect: Rect,
}

impl InputScaler {
    pub fn set_transform(&self, transform: MouseTransform) {
        if !transform.needs_scaling() {
            self.clear(transform.hwnd);
            return;
        }
        ensure_hook_thread();
        let mut state = self.state.lock().expect("input hook state poisoned");
        state.upsert(transform);
    }

    pub fn clear(&self, hwnd: Hwnd) {
        let mut state = self.state.lock().expect("input hook state poisoned");
        state.remove(hwnd);
    }
}

impl Default for InputScaler {
    fn default() -> Self {
        Self {
            state: hook_state(),
        }
    }
}

impl MouseTransform {
    fn needs_scaling(self) -> bool {
        self.visual_rect.width() != self.source_client_rect.width()
            || self.visual_rect.height() != self.source_client_rect.height()
    }

    fn contains(self, point: POINT) -> bool {
        self.visual_rect.contains_point(
            borderless_core::Pixels(point.x),
            borderless_core::Pixels(point.y),
        )
    }

    fn map_point(self, point: POINT) -> Option<(i32, i32)> {
        let visual_width = i64::from(self.visual_rect.width().0);
        let visual_height = i64::from(self.visual_rect.height().0);
        let source_width = i64::from(self.source_client_rect.width().0);
        let source_height = i64::from(self.source_client_rect.height().0);
        if visual_width <= 0 || visual_height <= 0 || source_width <= 0 || source_height <= 0 {
            return None;
        }

        let x = (i64::from(point.x - self.visual_rect.left.0) * source_width / visual_width)
            .clamp(0, source_width - 1);
        let y = (i64::from(point.y - self.visual_rect.top.0) * source_height / visual_height)
            .clamp(0, source_height - 1);
        Some((i32::try_from(x).ok()?, i32::try_from(y).ok()?))
    }
}

#[derive(Debug, Default)]
struct HookState {
    transforms: Vec<MouseTransform>,
}

impl HookState {
    fn upsert(&mut self, transform: MouseTransform) {
        if let Some(existing) = self
            .transforms
            .iter_mut()
            .find(|existing| existing.hwnd == transform.hwnd)
        {
            *existing = transform;
        } else {
            self.transforms.push(transform);
        }
    }

    fn remove(&mut self, hwnd: Hwnd) {
        self.transforms.retain(|transform| transform.hwnd != hwnd);
    }

    fn matching_transform(&self, point: POINT) -> Option<MouseTransform> {
        self.transforms
            .iter()
            .copied()
            .find(|transform| transform.contains(point) && is_target_foreground(transform.hwnd))
    }
}

static HOOK_STATE: OnceLock<Mutex<HookState>> = OnceLock::new();
static START_HOOK: Once = Once::new();

fn hook_state() -> &'static Mutex<HookState> {
    HOOK_STATE.get_or_init(|| Mutex::new(HookState::default()))
}

fn ensure_hook_thread() {
    START_HOOK.call_once(|| {
        std::thread::Builder::new()
            .name("borderless-input-scaler".to_owned())
            .spawn(hook_thread)
            .expect("spawn input scaler hook thread");
    });
}

fn hook_thread() {
    let hook = unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook_proc), None, 0) };
    if hook.is_err() {
        return;
    }

    let mut msg = MSG::default();
    while unsafe { GetMessageW(&raw mut msg, None, 0, 0) }.as_bool() {}
}

unsafe extern "system" fn mouse_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code < 0 {
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    }

    let Some(message) = u32::try_from(wparam.0)
        .ok()
        .filter(|message| is_mouse_message(*message))
    else {
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    };

    let hook = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
    let transform = {
        let state = hook_state().lock().expect("input hook state poisoned");
        state.matching_transform(hook.pt)
    };

    let Some(transform) = transform else {
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    };
    let Some((x, y)) = transform.map_point(hook.pt) else {
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    };

    let hwnd = ffi::hwnd(transform.hwnd);
    let _ = unsafe {
        PostMessageW(
            Some(hwnd),
            message,
            WPARAM(usize::from(mouse_key_state())),
            LPARAM(mouse_lparam(x, y)),
        )
    };

    LRESULT(1)
}

fn is_mouse_message(message: u32) -> bool {
    matches!(
        message,
        WM_MOUSEMOVE
            | WM_LBUTTONDOWN
            | WM_LBUTTONUP
            | WM_RBUTTONDOWN
            | WM_RBUTTONUP
            | WM_MBUTTONDOWN
            | WM_MBUTTONUP
            | WM_XBUTTONDOWN
            | WM_XBUTTONUP
    )
}

fn is_target_foreground(hwnd: Hwnd) -> bool {
    let foreground = unsafe { GetForegroundWindow() };
    foreground == ffi::hwnd(hwnd)
}

fn mouse_key_state() -> u16 {
    [
        (VK_LBUTTON.0, MK_LBUTTON),
        (VK_RBUTTON.0, MK_RBUTTON),
        (VK_SHIFT.0, MK_SHIFT),
        (VK_CONTROL.0, MK_CONTROL),
        (VK_MBUTTON.0, MK_MBUTTON),
        (VK_XBUTTON1.0, MK_XBUTTON1),
        (VK_XBUTTON2.0, MK_XBUTTON2),
    ]
    .into_iter()
    .filter_map(|(key, flag)| key_is_down(key).then_some(flag))
    .fold(0, |state, flag| state | flag)
}

fn key_is_down(key: u16) -> bool {
    unsafe { GetKeyState(i32::from(key)) }.is_negative()
}

fn mouse_lparam(x: i32, y: i32) -> isize {
    let x = u16::try_from(x.max(0)).unwrap_or(u16::MAX);
    let y = u16::try_from(y.max(0)).unwrap_or(u16::MAX);
    isize::try_from(u32::from(x) | (u32::from(y) << 16)).expect("mouse lparam fits in isize")
}

#[allow(dead_code)]
fn _keep_hook_types(_: HHOOK, _: HOOKPROC, _: WINDOWS_HOOK_ID) {}
