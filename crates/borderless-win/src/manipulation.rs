use crate::{audio::AudioSessions, cursor::CursorVisibility, ffi, taskbar};
use borderless_core::{
    BorderlessPlan, CoreResult, Hwnd, MenuPolicy, OriginalWindowState, Pid, WindowManipulator,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DrawMenuBar, GetMenu, GetMenuItemCount, RemoveMenu, SetWindowLongW, SetWindowPos, ShowWindow,
    WINDOW_LONG_PTR_INDEX,
};

#[derive(Clone, Debug, Default)]
pub struct WindowsManipulator {
    cursor: CursorVisibility,
    audio: AudioSessions,
}

impl WindowManipulator for WindowsManipulator {
    fn apply_plan(&self, plan: &BorderlessPlan) -> CoreResult<OriginalWindowState> {
        apply_borderless_plan(plan)?;
        if plan.hide_windows_taskbar {
            taskbar::set_visible(false);
        }
        if plan.hide_mouse_cursor {
            self.cursor.set_visible(false)?;
        }
        if plan.mute_in_background {
            // Mute-on-background normally reacts to focus changes. The direct apply path marks
            // the target as unmuted now and lets the watcher flip it when it loses focus.
            self.audio.set_process_muted(plan.original.hwnd, false)?;
        }
        Ok(plan.original.clone())
    }

    fn restore_original(&self, original: &OriginalWindowState) -> CoreResult<()> {
        restore(original)
    }

    fn set_taskbar_visible(&self, visible: bool) -> CoreResult<()> {
        taskbar::set_visible(visible);
        Ok(())
    }

    fn set_cursor_visible(&self, visible: bool) -> CoreResult<()> {
        self.cursor.set_visible(visible)
    }

    fn set_process_muted(&self, pid: Pid, muted: bool) -> CoreResult<()> {
        self.audio.set_pid_muted(pid, muted)
    }
}

fn apply_borderless_plan(plan: &BorderlessPlan) -> CoreResult<()> {
    let hwnd = ffi::hwnd(plan.original.hwnd);

    if matches!(plan.menu_policy, MenuPolicy::Remove) {
        remove_menu(hwnd)?;
    }

    unsafe {
        let _ = SetWindowLongW(
            hwnd,
            WINDOW_LONG_PTR_INDEX(ffi::GWL_STYLE),
            i32::from_ne_bytes(plan.new_style.bits().to_ne_bytes()),
        );
        let _ = SetWindowLongW(
            hwnd,
            WINDOW_LONG_PTR_INDEX(ffi::GWL_EXSTYLE),
            i32::from_ne_bytes(plan.new_ex_style.bits().to_ne_bytes()),
        );
    }

    let rect = plan.placement.rect;
    let insert_after = if plan.placement.topmost {
        ffi::hwnd_topmost()
    } else {
        ffi::null_hwnd()
    };
    let flags = ffi::SWP_FRAMECHANGED
        | ffi::SWP_SHOWWINDOW
        | ffi::SWP_NOOWNERZORDER
        | ffi::SWP_NOSENDCHANGING;

    ffi::ok_bool(unsafe {
        SetWindowPos(
            hwnd,
            Some(insert_after),
            rect.left.0,
            rect.top.0,
            rect.width().0,
            rect.height().0,
            windows::Win32::UI::WindowsAndMessaging::SET_WINDOW_POS_FLAGS(flags),
        )
        .is_ok()
        .into()
    })
    .map_err(|_| borderless_core::CoreError::Transition("SetWindowPos failed"))?;

    if plan.placement.maximize {
        unsafe {
            let _ = ShowWindow(
                hwnd,
                windows::Win32::UI::WindowsAndMessaging::SHOW_WINDOW_CMD(ffi::SW_MAXIMIZE),
            );
        }
    }

    Ok(())
}

fn restore(original: &OriginalWindowState) -> CoreResult<()> {
    let hwnd = ffi::hwnd(original.hwnd);
    unsafe {
        let _ = SetWindowLongW(
            hwnd,
            WINDOW_LONG_PTR_INDEX(ffi::GWL_STYLE),
            i32::from_ne_bytes(original.style.bits().to_ne_bytes()),
        );
        let _ = SetWindowLongW(
            hwnd,
            WINDOW_LONG_PTR_INDEX(ffi::GWL_EXSTYLE),
            i32::from_ne_bytes(original.ex_style.bits().to_ne_bytes()),
        );
    }
    let rect = original.rect;
    ffi::ok_bool(unsafe {
        SetWindowPos(
            hwnd,
            Some(ffi::null_hwnd()),
            rect.left.0,
            rect.top.0,
            rect.width().0,
            rect.height().0,
            windows::Win32::UI::WindowsAndMessaging::SET_WINDOW_POS_FLAGS(
                ffi::SWP_FRAMECHANGED | ffi::SWP_SHOWWINDOW | ffi::SWP_NOZORDER,
            ),
        )
        .is_ok()
        .into()
    })
    .map_err(|_| borderless_core::CoreError::Transition("restore SetWindowPos failed"))?;

    let z = if original.topmost {
        ffi::hwnd_topmost()
    } else {
        ffi::hwnd_notopmost()
    };
    ffi::ok_bool(unsafe {
        SetWindowPos(
            hwnd,
            Some(z),
            0,
            0,
            0,
            0,
            windows::Win32::UI::WindowsAndMessaging::SET_WINDOW_POS_FLAGS(
                ffi::SWP_NOACTIVATE | ffi::SWP_NOMOVE | ffi::SWP_NOSIZE,
            ),
        )
        .is_ok()
        .into()
    })
    .map_err(|_| borderless_core::CoreError::Transition("restore z-order failed"))
}

fn remove_menu(hwnd: windows::Win32::Foundation::HWND) -> CoreResult<()> {
    let menu = unsafe { GetMenu(hwnd) };
    if menu.0.is_null() {
        return Ok(());
    }
    let count = unsafe { GetMenuItemCount(Some(menu)) };
    for _ in 0..count {
        unsafe {
            let _ = RemoveMenu(
                menu,
                0,
                windows::Win32::UI::WindowsAndMessaging::MENU_ITEM_FLAGS(0x0000_0400 | 0x0000_1000),
            );
        }
    }
    ffi::ok_bool(unsafe { DrawMenuBar(hwnd).is_ok().into() })
        .map_err(|_| borderless_core::CoreError::Transition("DrawMenuBar failed"))
}

#[allow(dead_code)]
fn _keep_hwnd(_: Hwnd) {}
