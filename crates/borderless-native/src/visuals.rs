use crate::ffi;
use base64::Engine;
use borderless_core::{CoreError, CoreResult, Hwnd, Rect};
use windows::Win32::Foundation::{CloseHandle, HANDLE, LPARAM, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS,
    DeleteDC, DeleteObject, HGDIOBJ, SelectObject,
};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
use windows::Win32::UI::Shell::ExtractIconExW;
use windows::Win32::UI::WindowsAndMessaging::{
    CopyIcon, DI_NORMAL, DestroyIcon, DrawIconEx, GCLP_HICON, GCLP_HICONSM, GetClassLongPtrW,
    GetWindowThreadProcessId, HICON, ICON_BIG, ICON_SMALL, ICON_SMALL2, SEND_MESSAGE_TIMEOUT_FLAGS,
    SM_CXICON, SMTO_ABORTIFHUNG, SMTO_BLOCK, SendMessageTimeoutW, WM_GETICON,
};

const MIN_ICON_SIZE: i32 = 64;
const ICON_BITS_PER_PIXEL: u16 = 32;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WindowVisualAssets {
    pub icon_uri: Option<String>,
    pub preview_uri: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct WindowVisuals;

impl WindowVisuals {
    pub fn assets(&self, hwnd: Hwnd, _rect: Rect) -> CoreResult<WindowVisualAssets> {
        Ok(WindowVisualAssets {
            icon_uri: self.icon_uri(hwnd)?,
            preview_uri: None,
        })
    }

    pub fn icon_uri(&self, hwnd: Hwnd) -> CoreResult<Option<String>> {
        let hwnd = ffi::hwnd(hwnd);
        let size = icon_size(hwnd);
        let Some(icon) = OwnedIcon::from_hwnd(hwnd) else {
            return Ok(None);
        };
        render_icon_to_data_uri(icon.handle, size).map(Some)
    }

    pub const fn preview_uri(&self, _hwnd: Hwnd, _rect: Rect) -> CoreResult<Option<String>> {
        Ok(None)
    }
}

struct OwnedIcon {
    handle: HICON,
}

impl OwnedIcon {
    fn from_hwnd(hwnd: windows::Win32::Foundation::HWND) -> Option<Self> {
        let handle = [ICON_BIG, ICON_SMALL2, ICON_SMALL]
            .into_iter()
            .filter_map(|kind| copy_icon_from_message(hwnd, kind))
            .chain([GCLP_HICON, GCLP_HICONSM].into_iter().filter_map(|kind| {
                let raw = unsafe { GetClassLongPtrW(hwnd, kind) };
                copy_icon(HICON(raw as _))
            }))
            .next()
            .or_else(|| extract_process_icon(hwnd))?;
        Some(Self { handle })
    }
}

impl Drop for OwnedIcon {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyIcon(self.handle);
        }
    }
}

fn copy_icon_from_message(hwnd: windows::Win32::Foundation::HWND, kind: u32) -> Option<HICON> {
    let mut result = 0usize;
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    unsafe {
        let _ = SendMessageTimeoutW(
            hwnd,
            WM_GETICON,
            WPARAM(usize::try_from(kind).ok()?),
            LPARAM(isize::try_from(dpi).ok()?),
            SEND_MESSAGE_TIMEOUT_FLAGS(SMTO_ABORTIFHUNG.0 | SMTO_BLOCK.0),
            100,
            Some(&raw mut result),
        );
    }
    copy_icon(HICON(result as _))
}

fn copy_icon(icon: HICON) -> Option<HICON> {
    if icon.0.is_null() {
        return None;
    }
    unsafe { CopyIcon(icon) }.ok()
}

fn extract_process_icon(hwnd: windows::Win32::Foundation::HWND) -> Option<HICON> {
    let path = process_image_path(hwnd)?;
    let mut large = HICON::default();
    let mut small = HICON::default();
    let wide = ffi::wide_z(&path);
    let count = unsafe {
        ExtractIconExW(
            ffi::pcwstr_from_wide(&wide),
            0,
            Some(&raw mut large),
            Some(&raw mut small),
            1,
        )
    };
    if count == 0 {
        return None;
    }

    if !large.0.is_null() {
        destroy_icon_if_present(small);
        Some(large)
    } else if !small.0.is_null() {
        Some(small)
    } else {
        None
    }
}

fn destroy_icon_if_present(icon: HICON) {
    if !icon.0.is_null() {
        unsafe {
            let _ = DestroyIcon(icon);
        }
    }
}

fn process_image_path(hwnd: windows::Win32::Foundation::HWND) -> Option<String> {
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&raw mut pid));
    }
    if pid == 0 {
        return None;
    }

    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    let _guard = ProcessHandle(handle);
    let mut buf = vec![0u16; 32_768];
    let mut len = u32::try_from(buf.len()).ok()?;
    unsafe {
        QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            ffi::pwstr_from_slice(&mut buf),
            &raw mut len,
        )
    }
    .ok()?;
    let len = usize::try_from(len).ok()?;
    Some(String::from_utf16_lossy(&buf[..len]))
}

struct ProcessHandle(HANDLE);

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

fn icon_size(hwnd: windows::Win32::Foundation::HWND) -> i32 {
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    let system_size = unsafe { GetSystemMetricsForDpi(SM_CXICON, dpi) };
    system_size.max(MIN_ICON_SIZE)
}

fn render_icon_to_data_uri(icon: HICON, size: i32) -> CoreResult<String> {
    let mut pixels = render_icon_pixels(icon, size)?;
    normalize_alpha(&mut pixels);
    let png = png_bytes(size, size, &bgra_to_rgba(&pixels))?;
    Ok(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png)
    ))
}

fn render_icon_pixels(icon: HICON, size: i32) -> CoreResult<Vec<u8>> {
    let memory_dc = unsafe { CreateCompatibleDC(None) };
    if memory_dc.is_invalid() {
        return Err(CoreError::Transition("CreateCompatibleDC failed"));
    }

    let mut bits = std::ptr::null_mut();
    let info = bitmap_info(size, size);
    let bitmap = unsafe {
        CreateDIBSection(
            None,
            &raw const info,
            DIB_RGB_COLORS,
            &raw mut bits,
            None,
            0,
        )
    }
    .map_err(|_| CoreError::Transition("CreateDIBSection failed"))?;

    if bits.is_null() {
        unsafe {
            let _ = DeleteDC(memory_dc);
        }
        return Err(CoreError::Transition("CreateDIBSection bits missing"));
    }

    let previous = unsafe { SelectObject(memory_dc, HGDIOBJ(bitmap.0)) };
    let byte_count = image_byte_count(size, size)?;
    let pixels = unsafe {
        let surface = std::slice::from_raw_parts_mut(bits.cast::<u8>(), byte_count);
        surface.fill(0);
        DrawIconEx(memory_dc, 0, 0, icon, size, size, 0, None, DI_NORMAL)
            .map_err(|_| CoreError::Transition("DrawIconEx failed"))?;
        surface.to_vec()
    };

    unsafe {
        let _ = SelectObject(memory_dc, previous);
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(memory_dc);
    }

    Ok(pixels)
}

fn bitmap_info(width: i32, height: i32) -> BITMAPINFO {
    BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: u32::try_from(std::mem::size_of::<BITMAPINFOHEADER>())
                .expect("BITMAPINFOHEADER size fits in u32"),
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: ICON_BITS_PER_PIXEL,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    }
}

fn normalize_alpha(pixels: &mut [u8]) {
    if pixels.chunks_exact(4).any(|pixel| pixel[3] != 0) {
        return;
    }

    pixels.chunks_exact_mut(4).for_each(|pixel| {
        let has_color = pixel[..3].iter().any(|channel| *channel != 0);
        pixel[3] = if has_color { u8::MAX } else { 0 };
    });
}

fn bgra_to_rgba(pixels: &[u8]) -> Vec<u8> {
    pixels
        .chunks_exact(4)
        .flat_map(|pixel| [pixel[2], pixel[1], pixel[0], pixel[3]])
        .collect()
}

fn png_bytes(width: i32, height: i32, rgba: &[u8]) -> CoreResult<Vec<u8>> {
    let width = u32::try_from(width).map_err(|_| CoreError::Transition("invalid png width"))?;
    let height = u32::try_from(height).map_err(|_| CoreError::Transition("invalid png height"))?;
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|_| CoreError::Transition("write png header failed"))?;
        writer
            .write_image_data(rgba)
            .map_err(|_| CoreError::Transition("write png data failed"))?;
    }
    Ok(out)
}

fn image_byte_count(width: i32, height: i32) -> CoreResult<usize> {
    let pixels = i64::from(width)
        .checked_mul(i64::from(height))
        .and_then(|value| value.checked_mul(i64::from(ICON_BITS_PER_PIXEL / 8)))
        .ok_or(CoreError::Transition("bitmap dimensions are too large"))?;
    usize::try_from(pixels).map_err(|_| CoreError::Transition("bitmap dimensions are too large"))
}
