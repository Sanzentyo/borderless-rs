use borderless_core::{CoreError, CoreResult};
use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};

const CF_UNICODETEXT: u32 = 13;

#[derive(Clone, Debug, Default)]
pub struct Clipboard;

impl Clipboard {
    pub fn set_text(&self, text: &str) -> CoreResult<()> {
        set_text(text)
    }
}

fn set_text(text: &str) -> CoreResult<()> {
    let wide = text
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let byte_len = wide
        .len()
        .checked_mul(std::mem::size_of::<u16>())
        .ok_or(CoreError::Transition("clipboard text is too large"))?;
    let memory = unsafe { GlobalAlloc(GMEM_MOVEABLE, byte_len) }
        .map_err(|_| CoreError::Transition("GlobalAlloc failed"))?;
    let mut memory = GlobalMemory::new(memory);

    let ptr = unsafe { GlobalLock(memory.handle) };
    if ptr.is_null() {
        return Err(CoreError::Transition("GlobalLock failed"));
    }

    unsafe {
        std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr.cast::<u16>(), wide.len());
        let _ = GlobalUnlock(memory.handle);
    }

    let _clipboard = ClipboardGuard::open()?;
    unsafe {
        EmptyClipboard().map_err(|_| CoreError::Transition("EmptyClipboard failed"))?;
        SetClipboardData(CF_UNICODETEXT, Some(HANDLE(memory.handle.0)))
            .map_err(|_| CoreError::Transition("SetClipboardData failed"))?;
    }
    memory.disown();
    Ok(())
}

struct ClipboardGuard;

impl ClipboardGuard {
    fn open() -> CoreResult<Self> {
        unsafe { OpenClipboard(None).map_err(|_| CoreError::Transition("OpenClipboard failed")) }?;
        Ok(Self)
    }
}

impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseClipboard();
        }
    }
}

struct GlobalMemory {
    handle: HGLOBAL,
    owned: bool,
}

impl GlobalMemory {
    const fn new(handle: HGLOBAL) -> Self {
        Self {
            handle,
            owned: true,
        }
    }

    const fn disown(&mut self) {
        self.owned = false;
    }
}

impl Drop for GlobalMemory {
    fn drop(&mut self) {
        if self.owned {
            unsafe {
                let _ = GlobalFree(Some(self.handle));
            }
        }
    }
}
