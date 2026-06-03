use crate::ffi;
use borderless_core::{CoreError, CoreResult, Hwnd, Rect};
use directories::ProjectDirs;
use std::fs;
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS,
    DeleteDC, DeleteObject, HGDIOBJ, SelectObject,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CopyIcon, DI_NORMAL, DestroyIcon, DrawIconEx, GCLP_HICON, GCLP_HICONSM, GetClassLongPtrW,
    HICON, ICON_BIG, ICON_SMALL, ICON_SMALL2, SEND_MESSAGE_TIMEOUT_FLAGS, SMTO_ABORTIFHUNG,
    SendMessageTimeoutW, WM_GETICON,
};

const ICON_SIZE: i32 = 32;
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
        let Some(icon) = OwnedIcon::from_hwnd(ffi::hwnd(hwnd)) else {
            return Ok(None);
        };
        let path = cache_path("icons", hwnd, "png");
        render_icon_to_png(icon.handle, &path)?;
        Ok(Some(file_uri(&path)))
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
        [ICON_BIG, ICON_SMALL2, ICON_SMALL]
            .into_iter()
            .filter_map(|kind| copy_icon_from_message(hwnd, kind))
            .chain([GCLP_HICON, GCLP_HICONSM].into_iter().filter_map(|kind| {
                let raw = unsafe { GetClassLongPtrW(hwnd, kind) };
                copy_icon(HICON(raw as _))
            }))
            .next()
            .map(|handle| Self { handle })
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
    unsafe {
        let _ = SendMessageTimeoutW(
            hwnd,
            WM_GETICON,
            WPARAM(usize::try_from(kind).ok()?),
            LPARAM(0),
            SEND_MESSAGE_TIMEOUT_FLAGS(SMTO_ABORTIFHUNG.0),
            50,
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

fn render_icon_to_png(icon: HICON, path: &Path) -> CoreResult<()> {
    let mut pixels = render_icon_pixels(icon)?;
    normalize_alpha(&mut pixels);
    write_png(path, ICON_SIZE, ICON_SIZE, &bgra_to_rgba(&pixels))
}

fn render_icon_pixels(icon: HICON) -> CoreResult<Vec<u8>> {
    let memory_dc = unsafe { CreateCompatibleDC(None) };
    if memory_dc.is_invalid() {
        return Err(CoreError::Transition("CreateCompatibleDC failed"));
    }

    let mut bits = std::ptr::null_mut();
    let info = bitmap_info(ICON_SIZE, ICON_SIZE);
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
    let byte_count = image_byte_count(ICON_SIZE, ICON_SIZE)?;
    let pixels = unsafe {
        let surface = std::slice::from_raw_parts_mut(bits.cast::<u8>(), byte_count);
        surface.fill(0);
        DrawIconEx(
            memory_dc, 0, 0, icon, ICON_SIZE, ICON_SIZE, 0, None, DI_NORMAL,
        )
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

fn write_png(path: &Path, width: i32, height: i32, rgba: &[u8]) -> CoreResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|_| CoreError::Transition("create visual cache directory failed"))?;
    }

    let file = File::create(path).map_err(|_| CoreError::Transition("create icon png failed"))?;
    let writer = BufWriter::new(file);
    let width = u32::try_from(width).map_err(|_| CoreError::Transition("invalid png width"))?;
    let height = u32::try_from(height).map_err(|_| CoreError::Transition("invalid png height"))?;
    let mut encoder = png::Encoder::new(writer, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut png_writer = encoder
        .write_header()
        .map_err(|_| CoreError::Transition("write png header failed"))?;
    png_writer
        .write_image_data(rgba)
        .map_err(|_| CoreError::Transition("write png data failed"))
}

fn image_byte_count(width: i32, height: i32) -> CoreResult<usize> {
    let pixels = i64::from(width)
        .checked_mul(i64::from(height))
        .and_then(|value| value.checked_mul(i64::from(ICON_BITS_PER_PIXEL / 8)))
        .ok_or(CoreError::Transition("bitmap dimensions are too large"))?;
    usize::try_from(pixels).map_err(|_| CoreError::Transition("bitmap dimensions are too large"))
}

fn cache_path(kind: &str, hwnd: Hwnd, extension: &str) -> PathBuf {
    let base = ProjectDirs::from("dev", "BorderlessOxide", "Borderless Oxide")
        .map_or_else(std::env::temp_dir, |dirs| dirs.cache_dir().to_path_buf());
    let filename = format!(
        "{:016X}.{extension}",
        usize::from_ne_bytes(hwnd.0.to_ne_bytes())
    );
    base.join("visuals").join(kind).join(filename)
}

fn file_uri(path: &Path) -> String {
    let path = path.to_string_lossy().replace('\\', "/");
    format!("file:///{}", encode_uri_path(&path))
}

fn encode_uri_path(path: &str) -> String {
    path.chars().fold(String::new(), |mut out, ch| {
        match ch {
            ' ' => out.push_str("%20"),
            '#' => out.push_str("%23"),
            '%' => out.push_str("%25"),
            '?' => out.push_str("%3F"),
            _ => out.push(ch),
        }
        out
    })
}
