use borderless_core::CoreResult;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use windows::Win32::UI::WindowsAndMessaging::ShowCursor;

#[derive(Clone, Debug, Default)]
pub struct CursorVisibility {
    hidden: Arc<AtomicBool>,
}

impl CursorVisibility {
    pub fn set_visible(&self, visible: bool) -> CoreResult<()> {
        let was_hidden = self.hidden.swap(!visible, Ordering::SeqCst);
        if visible && was_hidden {
            for _ in 0..64 {
                if unsafe { ShowCursor(true) } >= 0 {
                    break;
                }
            }
        } else if !visible && !was_hidden {
            for _ in 0..64 {
                if unsafe { ShowCursor(false) } < 0 {
                    break;
                }
            }
        }
        Ok(())
    }
}
