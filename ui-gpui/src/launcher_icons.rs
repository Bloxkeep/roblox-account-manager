//! Each launcher's own icon for Settings › what opens Roblox (the real icons, not letters and stand-in symbols).
//! Bloxstrap, Fishstrap and Voidstrap's are their programs' icons from their own repositories (MIT), Roblox's and
//! Roblox VNG's are their clients' icons; a launcher Bloxkeep doesn't know (another Bloxstrap-style one, one picked by
//! hand) shows the icon of its own .exe, read from it once.
use std::{cell::RefCell, collections::HashMap, sync::Arc};

use gpui_kit::RenderImage;

/// Drawn at 16 px, so 32 dots across stays sharp at 200 %.
const SIDE: u32 = 32;

thread_local! {
    static MADE: RefCell<HashMap<String, Option<Arc<RenderImage>>>> = RefCell::new(HashMap::new());
}

fn bundled(name: &str) -> Option<&'static [u8]> {
    Some(match name {
        "bloxstrap" => include_bytes!("../assets/launchers/bloxstrap.png"),
        "fishstrap" => include_bytes!("../assets/launchers/fishstrap.png"),
        "voidstrap" => include_bytes!("../assets/launchers/voidstrap.png"),
        "roblox" => include_bytes!("../assets/launchers/roblox.png"),
        "vng" => include_bytes!("../assets/launchers/vng.png"),
        _ => return None,
    })
}

/// The icon for a launch option: `value` as Settings keeps it ("auto", "bloxstrap", "other:Name", "custom"…), `handler`
/// the program Default opens Roblox with now (its name), `custom` the launcher picked by hand (its path).
pub fn of(value: &str, handler: &str, custom: &str) -> Option<Arc<RenderImage>> {
    let key = format!("{value}|{handler}|{custom}");
    if let Some(made) = MADE.with(|m| m.borrow().get(&key).cloned()) {
        return made;
    }
    let made = make(value, handler, custom).map(Arc::new);
    MADE.with(|m| m.borrow_mut().insert(key, made.clone()));
    made
}

fn make(value: &str, handler: &str, custom: &str) -> Option<RenderImage> {
    let known = |name: &str| bundled(name).and_then(|b| crate::images::shrink(b, SIDE, SIDE, 0.));
    match value {
        "bloxstrap" | "fishstrap" | "voidstrap" | "vng" | "roblox" | "direct" | "store" => known(if matches!(value, "direct" | "store") { "roblox" } else { value }),
        // Default: the icon of what it opens Roblox with now.
        "auto" => {
            let h = handler.to_ascii_lowercase();
            ["bloxstrap", "fishstrap", "voidstrap"].iter().find(|n| h.contains(*n)).and_then(|n| known(n)).or_else(|| known("roblox"))
        }
        "custom" => from_exe(custom),
        v => {
            let name = v.strip_prefix("other:")?;
            #[cfg(feature = "core")]
            {
                crate::core::roblox_installs::find_bootstrapper(name).and_then(|exe| from_exe(&exe))
            }
            #[cfg(not(feature = "core"))]
            {
                let _ = name;
                None
            }
        }
    }
}

/// The icon of a program on this PC, as Explorer shows it.
#[cfg(windows)]
fn from_exe(path: &str) -> Option<RenderImage> {
    use windows_sys::Win32::{
        Graphics::Gdi::{BI_RGB, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, DeleteObject, GetDC, GetDIBits, ReleaseDC},
        UI::{
            Shell::SHDefExtractIconW,
            WindowsAndMessaging::{DestroyIcon, GetIconInfo, HICON, ICONINFO},
        },
    };
    if path.is_empty() || !std::path::Path::new(path).is_file() {
        return None;
    }
    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    let side = SIDE as i32;
    // SAFETY: the icon is read into a buffer of its size, then it and its bitmaps are freed.
    unsafe {
        let mut icon: HICON = std::ptr::null_mut();
        if SHDefExtractIconW(wide.as_ptr(), 0, 0, &mut icon, std::ptr::null_mut(), SIDE) != 0 || icon.is_null() {
            crate::net::note_failed("Launcher icon", path, "couldn't be read from the program");
            return None;
        }
        let mut info: ICONINFO = std::mem::zeroed();
        if GetIconInfo(icon, &mut info) == 0 {
            DestroyIcon(icon);
            return None;
        }
        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: side,
            biHeight: -side,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..std::mem::zeroed()
        };
        let mut bgra = vec![0u8; (SIDE * SIDE * 4) as usize];
        let dc = GetDC(std::ptr::null_mut());
        let rows = GetDIBits(dc, info.hbmColor, 0, SIDE, bgra.as_mut_ptr() as *mut _, &mut bmi, DIB_RGB_COLORS);
        ReleaseDC(std::ptr::null_mut(), dc);
        DeleteObject(info.hbmColor);
        DeleteObject(info.hbmMask);
        DestroyIcon(icon);
        if rows == 0 {
            return None;
        }
        // (An old icon without an alpha channel: its dots all solid.)
        if bgra.chunks_exact(4).all(|p| p[3] == 0) {
            bgra.chunks_exact_mut(4).for_each(|p| p[3] = 255);
        }
        let buffer = image::RgbaImage::from_raw(SIDE, SIDE, bgra)?;
        Some(RenderImage::new(vec![image::Frame::new(buffer)]))
    }
}

#[cfg(not(windows))]
fn from_exe(_: &str) -> Option<RenderImage> {
    None
}
