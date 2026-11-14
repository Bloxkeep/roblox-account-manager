// Ui.cs: what every window of the app shares: its title, its icon (a Santa hat in December), Windows 11's rounded corners
// and themed title bar, message boxes in front of the always-on-top windows, the clipboard for secrets.
use std::{
    sync::{
        LazyLock, Mutex,
        atomic::{AtomicI32, AtomicU64, Ordering},
    },
    time::Duration,
};

use windows_sys::Win32::{
    Foundation::{GlobalFree, HANDLE, HWND},
    Graphics::Dwm::DwmSetWindowAttribute,
    System::{
        DataExchange::{CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard, RegisterClipboardFormatW, SetClipboardData},
        Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock},
        Ole::CF_UNICODETEXT,
        SystemInformation::OSVERSIONINFOW,
    },
    UI::{
        Shell::{SHCNE_ASSOCCHANGED, SHCNE_UPDATEITEM, SHCNF_IDLIST, SHCNF_PATHW, SHChangeNotify},
        WindowsAndMessaging::{CreateIconFromResourceEx, HWND_NOTOPMOST, HWND_TOPMOST, LR_DEFAULTCOLOR, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos},
    },
};

use super::{
    clock, themes,
    win_form::{self, FormRef, colorref},
    win_kit::{Point, Rect, hwnd, wide},
};

pub const TITLE: &str = "Bloxkeep";

/// Environment.GetFolderPath: the folder Windows keeps for `id`; None when it can't say.
fn known_folder(id: &windows_sys::core::GUID) -> Option<std::path::PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::{System::Com::CoTaskMemFree, UI::Shell::SHGetKnownFolderPath};
    let mut path: windows_sys::core::PWSTR = std::ptr::null_mut();
    // SAFETY: a valid known-folder id; `path` receives a CoTaskMemAlloc'd string, freed below even on failure.
    let hr = unsafe { SHGetKnownFolderPath(id, 0, std::ptr::null_mut(), &mut path) };
    let mut out = None;
    if hr >= 0 && !path.is_null() {
        // SAFETY: a NUL-terminated wide string from the call above.
        out = Some(unsafe {
            let mut n = 0usize;
            while *path.add(n) != 0 {
                n += 1;
            }
            std::path::PathBuf::from(std::ffi::OsString::from_wide(std::slice::from_raw_parts(path, n)))
        });
    }
    if !path.is_null() {
        // SAFETY: freed once.
        unsafe { CoTaskMemFree(path as *const std::ffi::c_void) };
    }
    out
}

/// Makes Explorer redraw our shortcuts' icons: an update replaces the program folder, and Explorer can keep a
/// blank icon it cached meanwhile.
pub fn refresh_shell_icons() {
    let folder = |var: &str, sub: &[&str]| -> std::path::PathBuf {
        let mut p = std::path::PathBuf::from(std::env::var_os(var).unwrap_or_default());
        for s in sub {
            p.push(s);
        }
        p
    };
    // Environment.SpecialFolder.DesktopDirectory (where Windows keeps it: OneDrive moves it) and .Programs.
    let desktop = known_folder(&windows_sys::Win32::UI::Shell::FOLDERID_Desktop).unwrap_or_else(|| folder("USERPROFILE", &["Desktop"]));
    let paths = [
        desktop.join(format!("{TITLE}.lnk")),
        folder("APPDATA", &["Microsoft", "Windows", "Start Menu", "Programs"]).join(format!("{TITLE}.lnk")),
        std::env::current_exe().unwrap_or_default(),
    ];
    // Explorer handles these itself; never hold up the app's start for it.
    let _ = std::thread::Builder::new().name("shell-icons".into()).spawn(move || {
        for path in paths.iter().filter(|p| p.is_file()) {
            let w = wide(&path.to_string_lossy());
            // SAFETY: a zero-ended path, read during the call.
            unsafe { SHChangeNotify(SHCNE_UPDATEITEM as i32, SHCNF_PATHW, w.as_ptr() as *const _, std::ptr::null()) };
        }
        // SAFETY: no items for this event.
        unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED as i32, SHCNF_IDLIST, std::ptr::null(), std::ptr::null()) };
    });
}

/// A saved window's place moved just enough for the window to lie inside the screen's work area (give or take
/// `slack`, Windows' invisible resize border, so a window snapped to an edge stays put). Only its
/// size was held to the screen: one saved 1980 wide at x = 1000, back on a 1920 screen, was cut off (review 2026-10-30).
pub fn inside(r: Rect, area: Rect, slack: i32) -> Point {
    let area = area.inflated(slack, slack);
    Point::new(area.left().max(r.x.min(area.right() - r.width)), area.top().max(r.y.min(area.bottom() - r.height)))
}

static APP_ICON: LazyLock<isize> = LazyLock::new(load_app_icon);

/// The app's icon (an HICON, 0 when it couldn't be made).
pub fn app_icon() -> Option<isize> {
    let h = *APP_ICON;
    if h == 0 { None } else { Some(h) }
}

static APP_ICON_SMALL: LazyLock<isize> = LazyLock::new(|| {
    // SAFETY: a plain read of a system size.
    let side = unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetSystemMetrics(windows_sys::Win32::UI::WindowsAndMessaging::SM_CXSMICON) };
    let side = if side > 0 { side } else { 16 };
    // December keeps the Santa hat (its own small pictures); the rest of the year the tray's own drawing.
    if clock::now().local().month == 12 { icon_from_ico(&APP_ICON_BYTES, side) } else { icon_from_ico(TRAY_ICO, side) }
});

/// The Keeper's face alone, upright, drawn on the pixel grid of each small size (16–48 px; scratchpad's trayico page).
const TRAY_ICO: &[u8] = include_bytes!("../../assets/tray.ico");

/// The app's icon at the small size Windows draws in the notification area and a title bar (16 px, 24 at 150 %).
/// Not app.ico's small pictures: those are the big logo shrunk (tilted −8°, the grey block behind, blurred), so the
/// tray looked bent whichever of them was used (the owner's tray, 2026-11-13: "méo vẫn hoàn méo").
pub fn app_icon_small() -> Option<isize> {
    let h = *APP_ICON_SMALL;
    if h == 0 { app_icon() } else { Some(h) }
}

const APP_ICO: &[u8] = include_bytes!("../../../RobloxManager/app.ico");
const APP_CHRISTMAS_ICO: &[u8] = include_bytes!("../../../RobloxManager/app-christmas.ico");

static APP_ICON_BYTES: LazyLock<&'static [u8]> = LazyLock::new(|| if clock::now().local().month == 12 { APP_CHRISTMAS_ICO } else { APP_ICO });

/// In December the logo in a Santa hat (shared.js swaps the page's logo the same way), from the next start.
fn load_app_icon() -> isize {
    icon_from_ico(&APP_ICON_BYTES, 0)
}

/// new Icon(AppIcon, new Size(size, size)).ToBitmap(): the app's icon at `size` pixels as a picture.
pub fn app_icon_picture(size: i32) -> Option<super::win_kit::Picture> {
    use windows_sys::Win32::{
        Graphics::Gdi::{BI_RGB, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, DeleteObject, GetDC, GetDIBits, ReleaseDC},
        UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, ICONINFO},
    };
    let icon = icon_from_ico(&APP_ICON_BYTES, size);
    if icon == 0 {
        return None;
    }
    // SAFETY: the icon's bitmaps are read into a buffer of their size and freed with the icon.
    unsafe {
        let mut info: ICONINFO = std::mem::zeroed();
        if GetIconInfo(icon as _, &mut info) == 0 {
            DestroyIcon(icon as _);
            return None;
        }
        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: size,
            biHeight: -size,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..std::mem::zeroed()
        };
        let mut bgra = vec![0u8; (size * size * 4) as usize];
        let dc = GetDC(std::ptr::null_mut());
        let rows = GetDIBits(dc, info.hbmColor, 0, size as u32, bgra.as_mut_ptr() as *mut _, &mut bmi, DIB_RGB_COLORS);
        ReleaseDC(std::ptr::null_mut(), dc);
        DeleteObject(info.hbmColor);
        DeleteObject(info.hbmMask);
        DestroyIcon(icon as _);
        if rows == 0 {
            return None;
        }
        let rgba: Vec<u8> = bgra.chunks_exact(4).flat_map(|p| [p[2], p[1], p[0], p[3]]).collect();
        Some(super::win_kit::Picture::from_rgba(size, size, &rgba))
    }
}

/// An HICON of an .ico file's image nearest `size` pixels (0: the largest), as new Icon(icon, size) picks it.
pub fn icon_from_ico(bytes: &[u8], size: i32) -> isize {
    // ICONDIR: reserved, type, count; then 16-byte entries: width, height, colours, reserved, planes, bits, bytes, offset.
    if bytes.len() < 6 {
        return 0;
    }
    let count = u16::from_le_bytes([bytes[4], bytes[5]]) as usize;
    let mut best: Option<(i32, usize, usize)> = None;
    for i in 0..count {
        let e = 6 + i * 16;
        if e + 16 > bytes.len() {
            break;
        }
        let w = if bytes[e] == 0 { 256 } else { bytes[e] as i32 };
        let len = u32::from_le_bytes([bytes[e + 8], bytes[e + 9], bytes[e + 10], bytes[e + 11]]) as usize;
        let off = u32::from_le_bytes([bytes[e + 12], bytes[e + 13], bytes[e + 14], bytes[e + 15]]) as usize;
        if off + len > bytes.len() {
            continue;
        }
        let better = match best {
            None => true,
            Some((bw, _, _)) if size <= 0 => w > bw,
            // The smallest at least as big, else the biggest.
            Some((bw, _, _)) => (w >= size && (bw < size || w < bw)) || (bw < size && w > bw),
        };
        if better {
            best = Some((w, off, len));
        }
    }
    let Some((w, off, len)) = best else { return 0 };
    let side = if size > 0 { size } else { w };
    // SAFETY: the image bytes lie inside `bytes`, checked above; Windows copies them into the icon.
    unsafe { CreateIconFromResourceEx(bytes[off..off + len].as_ptr(), len as u32, 1, 0x0003_0000, side, side, LR_DEFAULTCOLOR) as isize }
}

fn dwm_set(h: isize, attribute: u32, value: i32) -> i32 {
    // SAFETY: a 4-byte value on the stack, read during the call.
    unsafe { DwmSetWindowAttribute(hwnd(h) as HWND, attribute, &value as *const i32 as *const _, 4) }
}

/// Windows 11 rounds a borderless window and draws its hairline in the theme's line colour; false on older
/// Windows, where it stays square.
pub fn round_corners(h: isize, theme: &themes::Theme) -> bool {
    const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
    const DWMWA_BORDER_COLOR: u32 = 34;
    const DWMWCP_ROUND: i32 = 2;
    if dwm_set(h, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND) != 0 {
        return false;
    }
    dwm_set(h, DWMWA_BORDER_COLOR, colorref(theme.line) as i32);
    true
}

/// Environment.OSVersion.Version.Build (RtlGetVersion: GetVersionEx lies without a manifest).
pub fn windows_build() -> u32 {
    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn RtlGetVersion(info: *mut OSVERSIONINFOW) -> i32;
    }
    // SAFETY: the structure's size is set as Windows requires.
    unsafe {
        let mut info: OSVERSIONINFOW = std::mem::zeroed();
        info.dwOSVersionInfoSize = std::mem::size_of::<OSVERSIONINFOW>() as u32;
        if RtlGetVersion(&mut info) == 0 { info.dwBuildNumber } else { 0 }
    }
}

/// Title bar in the theme's colours (caption and text colours are ignored before Windows 11).
pub fn use_themed_title_bar(h: isize, theme: &themes::Theme) {
    const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;
    const DWMWA_CAPTION_COLOR: u32 = 35;
    const DWMWA_TEXT_COLOR: u32 = 36;
    // Windows 10 before 20H1 (build 19041) knew the dark-mode attribute under the number 19.
    let dark_mode = if windows_build() >= 19041 { DWMWA_USE_IMMERSIVE_DARK_MODE } else { 19 };
    dwm_set(h, dark_mode, if theme.dark { 1 } else { 0 });
    dwm_set(h, DWMWA_CAPTION_COLOR, colorref(theme.background) as i32);
    dwm_set(h, DWMWA_TEXT_COLOR, colorref(theme.text) as i32);
}

/// An error box (the app's own dialog), in front of the always-on-top windows.
pub fn error(owner: Option<isize>, message: &str) {
    let message = message.to_string();
    modal(move || {
        crate::core::forms::dialog::error(owner, &message);
        0
    });
}

/// The windows the app may keep on top (the mini window): a message box would open behind them.
static ON_TOP: Mutex<Vec<FormRef>> = Mutex::new(Vec::new());

/// Notes a window that can be always on top, so `modal` lowers it while a box is shown.
pub fn note_on_top(form: FormRef) {
    ON_TOP.lock().unwrap_or_else(|e| e.into_inner()).push(form);
}

static MODALS_OPEN: AtomicI32 = AtomicI32::new(0);

/// Message boxes and dialogs open now (an update waits for them, see AppController.QuietForRelaunch).
pub fn modals_open() -> i32 {
    MODALS_OPEN.load(Ordering::SeqCst)
}

/// Shows a message box or dialog with the app's always-on-top windows lowered meanwhile, so it opens in front of them.
pub fn modal<T>(show: impl FnOnce() -> T) -> T {
    let lowered: Vec<FormRef> = {
        let mut list = ON_TOP.lock().unwrap_or_else(|e| e.into_inner());
        list.retain(|f| !f.is_disposed());
        // Not from another thread: the box then opens as before, which is better than a cross-thread error. A page's
        // window (WebToolWindow) is on top by its style, not by TopMost (setting that would take the keyboard).
        list.iter()
            .filter(|f| {
                let h = f.handle();
                h != 0 && win_form::is_visible(h) && win_form::on_own_thread(h) && on_top(h)
            })
            .cloned()
            .collect()
    };
    for form in &lowered {
        set_on_top(form.handle(), false);
    }
    MODALS_OPEN.fetch_add(1, Ordering::SeqCst);
    struct Restore(Vec<FormRef>);
    impl Drop for Restore {
        fn drop(&mut self) {
            MODALS_OPEN.fetch_sub(1, Ordering::SeqCst);
            for form in self.0.iter().filter(|f| !f.is_disposed() && f.handle() != 0) {
                set_on_top(form.handle(), true);
            }
        }
    }
    let _restore = Restore(lowered);
    show()
}

fn on_top(h: isize) -> bool {
    win_form::is_top_most(h)
}

/// On top of every window or not, without taking the keyboard.
fn set_on_top(h: isize, on: bool) {
    // SAFETY: plain call on our own window.
    unsafe {
        SetWindowPos(hwnd(h), if on { HWND_TOPMOST } else { HWND_NOTOPMOST }, 0, 0, 0, 0, SWP_NOSIZE | SWP_NOMOVE | SWP_NOACTIVATE);
    }
}

/// The secret last copied, and a count that tells the 30-second wipe it was replaced since (the WinForms timer).
static CLIPBOARD_SECRET: Mutex<Option<String>> = Mutex::new(None);
static CLIPBOARD_TIMER: AtomicU64 = AtomicU64::new(0);

/// Opens the clipboard, as Clipboard's retries do (retryTimes 5, retryDelay 100).
fn open_clipboard() -> bool {
    for i in 0..5 {
        // SAFETY: plain call; closed by the caller.
        if unsafe { OpenClipboard(std::ptr::null_mut()) } != 0 {
            return true;
        }
        if i < 4 {
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    false
}

fn clipboard_text() -> Option<String> {
    // SAFETY: the clipboard is open; the handle is locked and unlocked around the read.
    unsafe {
        if IsClipboardFormatAvailable(CF_UNICODETEXT as u32) == 0 {
            return None;
        }
        let data = GetClipboardData(CF_UNICODETEXT as u32);
        if data.is_null() {
            return None;
        }
        let p = GlobalLock(data) as *const u16;
        if p.is_null() {
            return None;
        }
        // Within the block: another program's text may lack its ending zero.
        let most = GlobalSize(data) / 2;
        let mut len = 0;
        while len < most && *p.add(len) != 0 {
            len += 1;
        }
        let text = String::from_utf16_lossy(std::slice::from_raw_parts(p, len));
        GlobalUnlock(data);
        Some(text)
    }
}

/// Removes a copied password or cookie now (on lock and exit), if it is still what the clipboard holds.
pub fn clear_secret_clipboard() {
    let mine = CLIPBOARD_TIMER.fetch_add(1, Ordering::SeqCst) + 1;
    wipe_secret(mine, 0);
}

fn wipe_secret(mine: u64, tries: u32) {
    if CLIPBOARD_SECRET.lock().unwrap_or_else(|e| e.into_inner()).is_none() {
        return;
    }
    // The clipboard held by another program: the secret is kept and tried again a little later (a newer copy or
    // clear calls that off), or it stayed on the clipboard for good.
    if !open_clipboard() {
        if tries < 10 {
            let _ = std::thread::Builder::new().name("clipboard-wipe".into()).spawn(move || {
                std::thread::sleep(Duration::from_secs(3));
                if CLIPBOARD_TIMER.load(Ordering::SeqCst) == mine {
                    wipe_secret(mine, tries + 1);
                }
            });
        }
        return;
    }
    let secret = CLIPBOARD_SECRET.lock().unwrap_or_else(|e| e.into_inner()).take();
    if let Some(secret) = secret
        && clipboard_text().as_deref() == Some(secret.as_str())
    {
        // SAFETY: the clipboard is open.
        unsafe { EmptyClipboard() };
    }
    // SAFETY: opened above.
    unsafe { CloseClipboard() };
}

/// Moveable global memory holding `bytes`, for SetClipboardData (which then owns it).
fn global_bytes(bytes: &[u8]) -> Option<HANDLE> {
    // SAFETY: allocated, filled under lock, unlocked; freed by the caller when the clipboard doesn't take it.
    unsafe {
        let h = GlobalAlloc(GMEM_MOVEABLE, bytes.len().max(1));
        if h.is_null() {
            return None;
        }
        let p = GlobalLock(h) as *mut u8;
        if p.is_null() {
            GlobalFree(h);
            return None;
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len());
        GlobalUnlock(h);
        Some(h)
    }
}

fn put(format: u32, bytes: &[u8]) -> bool {
    let Some(h) = global_bytes(bytes) else { return false };
    // SAFETY: the clipboard is open and owns `h` once set; freed here when it wasn't taken.
    unsafe {
        if SetClipboardData(format, h).is_null() {
            GlobalFree(h);
            return false;
        }
    }
    true
}

fn format_named(name: &str) -> u32 {
    let w = wide(name);
    // SAFETY: a zero-ended name.
    unsafe { RegisterClipboardFormatW(w.as_ptr()) }
}

/// Secrets are kept out of clipboard history and cloud sync, and wiped after 30 seconds.
pub fn copy_to_clipboard(text: &str, secret: bool) -> bool {
    if text.is_empty() {
        return false;
    }
    if !open_clipboard() {
        return false;
    }
    // SAFETY: the clipboard is open here, closed below.
    unsafe { EmptyClipboard() };
    let mut utf16: Vec<u8> = text.encode_utf16().chain(std::iter::once(0)).flat_map(|c| c.to_le_bytes()).collect();
    let mut ok = put(CF_UNICODETEXT as u32, &utf16);
    utf16.clear();
    if ok && secret {
        ok &= put(format_named("ExcludeClipboardContentFromMonitorProcessing"), &[0u8; 4]);
        ok &= put(format_named("CanIncludeInClipboardHistory"), &0i32.to_le_bytes());
        ok &= put(format_named("CanUploadToCloudClipboard"), &0i32.to_le_bytes());
        if !ok {
            // Without the marks it would go into clipboard history (and maybe the cloud) and stay there: taken off
            // again before the clipboard closes, and the copy is reported failed.
            // SAFETY: the clipboard is still open.
            unsafe { EmptyClipboard() };
        }
    }
    // SAFETY: opened above.
    unsafe { CloseClipboard() };
    if !ok {
        return false;
    }
    if secret {
        *CLIPBOARD_SECRET.lock().unwrap_or_else(|e| e.into_inner()) = Some(text.to_string());
        let mine = CLIPBOARD_TIMER.fetch_add(1, Ordering::SeqCst) + 1;
        // PORT: the WinForms Timer (30 s, on the UI thread) is a thread that sleeps; a newer copy or a clear calls it off.
        let _ = std::thread::Builder::new().name("clipboard-wipe".into()).spawn(move || {
            std::thread::sleep(Duration::from_millis(30_000));
            if CLIPBOARD_TIMER.load(Ordering::SeqCst) == mine {
                clear_secret_clipboard();
            }
        });
    }
    true
}
