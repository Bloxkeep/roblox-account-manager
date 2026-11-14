// What the Roblox-window files (roblox_processes, anti_afk, window_*…) share and .NET gave the C# app for free: System.Drawing's
// Rectangle/Size/Point, WinForms' Screen, a Bitmap as plain pixels, the registry (Microsoft.Win32.Registry) and the
// process list (System.Diagnostics.Process by name or id). No C# file of its own: these were .NET's.
#![allow(clippy::missing_safety_doc)]

use std::ffi::c_void;

use windows_sys::Win32::{
    Foundation::{CloseHandle, ERROR_FILE_NOT_FOUND, HWND, INVALID_HANDLE_VALUE, LPARAM, RECT},
    Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITOR_DEFAULTTONEAREST, MONITORINFOEXW, MonitorFromRect, MonitorFromWindow},
    System::{
        Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS},
        Registry::{
            HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WRITE, REG_BINARY, REG_EXPAND_SZ, REG_MULTI_SZ, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey,
            RegCreateKeyExW, RegDeleteValueW, RegEnumValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
        },
    },
};

use super::base::{Fail, Res, Stamp};

/// A window or any other Windows handle as the C# app held it (IntPtr): a number, so it can cross threads.
pub fn hwnd(h: isize) -> HWND {
    h as HWND
}

/// A string for Windows: UTF-16, ended by a zero.
/// C#'s StringComparer.CurrentCultureIgnoreCase: Windows' own order for the person's language ("_" before digits,
/// "Ă" next to "A"), not the code order of lowercased text.
pub fn culture_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    use windows_sys::Win32::Globalization::{CompareStringEx, NORM_IGNORECASE};
    let (wa, wb): (Vec<u16>, Vec<u16>) = (a.encode_utf16().collect(), b.encode_utf16().collect());
    // SAFETY: both buffers are passed with their lengths; a null locale name is the user's default.
    let r = unsafe { CompareStringEx(std::ptr::null(), NORM_IGNORECASE, wa.as_ptr(), wa.len() as i32, wb.as_ptr(), wb.len() as i32, std::ptr::null(), std::ptr::null(), 0) };
    match r {
        1 => std::cmp::Ordering::Less,
        2 => std::cmp::Ordering::Equal,
        3 => std::cmp::Ordering::Greater,
        _ => a.to_lowercase().cmp(&b.to_lowercase()),
    }
}

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Text Windows wrote into a buffer, up to its first zero.
pub fn from_wide(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

/// A FILETIME (100 ns since 1601, UTC) as a moment.
pub fn filetime_to_stamp(ft: i64) -> Stamp {
    Stamp::from_unix_ms((ft - 116_444_736_000_000_000) / 10_000)
}

/// A moment as a FILETIME (DateTime.ToFileTimeUtc).
pub fn stamp_to_filetime(at: Stamp) -> i64 {
    at.unix_ms() * 10_000 + 116_444_736_000_000_000
}

// ───────── System.Drawing ─────────

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub fn new(x: i32, y: i32) -> Point {
        Point { x, y }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Size {
    pub width: i32,
    pub height: i32,
}

impl Size {
    pub fn new(width: i32, height: i32) -> Size {
        Size { width, height }
    }
    /// Size.IsEmpty: both zero.
    pub fn is_empty(self) -> bool {
        self.width == 0 && self.height == 0
    }
}

/// System.Drawing.Rectangle, with its own rules (Contains leaves the right and bottom edge out; IsEmpty only when all
/// four are zero).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Rect {
        Rect { x, y, width, height }
    }
    pub fn at(location: Point, size: Size) -> Rect {
        Rect { x: location.x, y: location.y, width: size.width, height: size.height }
    }
    pub fn from_ltrb(left: i32, top: i32, right: i32, bottom: i32) -> Rect {
        Rect { x: left, y: top, width: right - left, height: bottom - top }
    }
    pub fn from_win(r: &RECT) -> Rect {
        Rect::from_ltrb(r.left, r.top, r.right, r.bottom)
    }
    pub fn to_win(self) -> RECT {
        RECT { left: self.x, top: self.y, right: self.right(), bottom: self.bottom() }
    }
    pub fn left(self) -> i32 {
        self.x
    }
    pub fn top(self) -> i32 {
        self.y
    }
    pub fn right(self) -> i32 {
        self.x + self.width
    }
    pub fn bottom(self) -> i32 {
        self.y + self.height
    }
    pub fn location(self) -> Point {
        Point { x: self.x, y: self.y }
    }
    pub fn size(self) -> Size {
        Size { width: self.width, height: self.height }
    }
    pub fn is_empty(self) -> bool {
        self.x == 0 && self.y == 0 && self.width == 0 && self.height == 0
    }
    pub fn contains_point(self, p: Point) -> bool {
        self.x <= p.x && p.x < self.x + self.width && self.y <= p.y && p.y < self.y + self.height
    }
    pub fn contains_rect(self, r: Rect) -> bool {
        self.x <= r.x && r.x + r.width <= self.x + self.width && self.y <= r.y && r.y + r.height <= self.y + self.height
    }
    /// Rectangle.Inflate(r, w, h): as a new rectangle.
    pub fn inflated(self, w: i32, h: i32) -> Rect {
        Rect { x: self.x - w, y: self.y - h, width: self.width + 2 * w, height: self.height + 2 * h }
    }
    /// Rectangle.Intersect: Empty when they don't meet.
    pub fn intersect(a: Rect, b: Rect) -> Rect {
        let x1 = a.x.max(b.x);
        let x2 = a.right().min(b.right());
        let y1 = a.y.max(b.y);
        let y2 = a.bottom().min(b.bottom());
        if x2 >= x1 && y2 >= y1 { Rect { x: x1, y: y1, width: x2 - x1, height: y2 - y1 } } else { Rect::default() }
    }
}

// ───────── WinForms' Screen ─────────

/// One display (WinForms' Screen): two are the same when their handle is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Screen {
    pub handle: isize,
    pub device_name: String,
    pub bounds: Rect,
    pub working_area: Rect,
    pub primary: bool,
}

fn screen_of(monitor: HMONITOR) -> Option<Screen> {
    // SAFETY: MONITORINFOEXW is plain data; cbSize is set before the call as Windows requires.
    unsafe {
        let mut info: MONITORINFOEXW = std::mem::zeroed();
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if GetMonitorInfoW(monitor, &mut info as *mut MONITORINFOEXW as *mut _) == 0 {
            return None;
        }
        Some(Screen {
            handle: monitor as isize,
            device_name: from_wide(&info.szDevice),
            bounds: Rect::from_win(&info.monitorInfo.rcMonitor),
            working_area: Rect::from_win(&info.monitorInfo.rcWork),
            primary: info.monitorInfo.dwFlags & 1 != 0, // MONITORINFOF_PRIMARY
        })
    }
}

unsafe extern "system" fn add_monitor(monitor: HMONITOR, _dc: HDC, _rect: *mut RECT, data: LPARAM) -> i32 {
    // SAFETY: `data` is the &mut Vec passed to EnumDisplayMonitors in all_screens, alive for the whole call.
    let list = unsafe { &mut *(data as *mut Vec<Screen>) };
    if let Some(s) = screen_of(monitor) {
        list.push(s);
    }
    1
}

/// Screen.AllScreens, in Windows' order.
pub fn all_screens() -> Vec<Screen> {
    let mut list: Vec<Screen> = Vec::new();
    // SAFETY: the callback only touches `list`, which outlives the call.
    unsafe {
        EnumDisplayMonitors(std::ptr::null_mut(), std::ptr::null(), Some(add_monitor), &mut list as *mut Vec<Screen> as LPARAM);
    }
    list
}

/// Screen.FromRectangle: the screen it is most on, else the nearest.
pub fn screen_from_rect(r: Rect) -> Screen {
    let win = r.to_win();
    // SAFETY: plain call with a RECT on the stack.
    let monitor = unsafe { MonitorFromRect(&win, MONITOR_DEFAULTTONEAREST) };
    screen_of(monitor).unwrap_or_else(primary_screen)
}

/// Screen.FromHandle: the screen the window is most on, else the nearest.
pub fn screen_from_handle(h: isize) -> Screen {
    // SAFETY: plain call; an invalid window gives the nearest (primary) monitor.
    let monitor = unsafe { MonitorFromWindow(hwnd(h), MONITOR_DEFAULTTONEAREST) };
    screen_of(monitor).unwrap_or_else(primary_screen)
}

fn primary_screen() -> Screen {
    let all = all_screens();
    all.iter().find(|s| s.primary).cloned().or_else(|| all.first().cloned()).unwrap_or(Screen {
        handle: 0,
        device_name: String::new(),
        bounds: Rect::default(),
        working_area: Rect::default(),
        primary: true,
    })
}

// ───────── a Bitmap as pixels ─────────

/// A picture held as pixels: 0xAARRGGBB, premultiplied by alpha (so in memory B, G, R, A: what a 32-bit top-down DIB
/// section holds). Where System.Drawing's Bitmap was.
#[derive(Clone, Debug, Default)]
pub struct Picture {
    pub width: i32,
    pub height: i32,
    pub pixels: Vec<u32>,
}

impl Picture {
    /// All transparent.
    pub fn new(width: i32, height: i32) -> Picture {
        Picture { width, height, pixels: vec![0; (width.max(0) as usize) * (height.max(0) as usize)] }
    }
    pub fn get(&self, x: i32, y: i32) -> u32 {
        self.pixels[(y * self.width + x) as usize]
    }
    pub fn set(&mut self, x: i32, y: i32, argb: u32) {
        let i = (y * self.width + x) as usize;
        self.pixels[i] = argb;
    }
    /// Graphics.Clear.
    pub fn fill(&mut self, argb: u32) {
        self.pixels.fill(argb);
    }
    /// GetPixel's red, green and blue (as stored; an opaque picture's are its colours).
    pub fn rgb(&self, x: i32, y: i32) -> (u8, u8, u8) {
        let p = self.get(x, y);
        ((p >> 16) as u8, (p >> 8) as u8, p as u8)
    }
    /// Straight (not premultiplied) RGBA bytes, for the `image` crate.
    pub fn to_rgba(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.pixels.len() * 4);
        for &p in &self.pixels {
            let a = (p >> 24) as u8;
            let un = |c: u8| if a == 0 { 0 } else { ((c as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8 };
            out.extend_from_slice(&[un((p >> 16) as u8), un((p >> 8) as u8), un(p as u8), a]);
        }
        out
    }
    /// From straight RGBA bytes.
    pub fn from_rgba(width: i32, height: i32, rgba: &[u8]) -> Picture {
        let mut pic = Picture::new(width, height);
        for (i, px) in rgba.chunks_exact(4).enumerate().take(pic.pixels.len()) {
            let a = px[3] as u32;
            let pre = |c: u8| (c as u32 * a + 127) / 255;
            pic.pixels[i] = (a << 24) | (pre(px[0]) << 16) | (pre(px[1]) << 8) | pre(px[2]);
        }
        pic
    }
}

// ───────── the registry (Microsoft.Win32.Registry) ─────────

#[derive(Clone, Copy, Debug)]
pub enum Hive {
    CurrentUser,
    LocalMachine,
}

fn hive_key(h: Hive) -> HKEY {
    match h {
        Hive::CurrentUser => HKEY_CURRENT_USER,
        Hive::LocalMachine => HKEY_LOCAL_MACHINE,
    }
}

/// An open registry key (RegistryKey), closed when dropped.
pub struct RegKey(HKEY);

impl Drop for RegKey {
    fn drop(&mut self) {
        // SAFETY: the key was opened by this RegKey and is closed once.
        unsafe {
            RegCloseKey(self.0);
        }
    }
}

impl RegKey {
    /// OpenSubKey: None when the key isn't there; Err when Windows refuses it (SecurityException).
    pub fn open(hive: Hive, path: &str, writable: bool) -> Res<Option<RegKey>> {
        let p = wide(path);
        let mut key: HKEY = std::ptr::null_mut();
        // SAFETY: plain call, `p` is zero-ended and `key` receives the handle.
        let err = unsafe { RegOpenKeyExW(hive_key(hive), p.as_ptr(), 0, if writable { KEY_READ | KEY_WRITE } else { KEY_READ }, &mut key) };
        match err {
            0 => Ok(Some(RegKey(key))),
            ERROR_FILE_NOT_FOUND => Ok(None),
            e => Err(Fail::other(format!("registry key {path}: error {e}"))),
        }
    }

    /// CreateSubKey: opened for writing, made when missing.
    pub fn create(hive: Hive, path: &str) -> Res<RegKey> {
        let p = wide(path);
        let mut key: HKEY = std::ptr::null_mut();
        // SAFETY: plain call; no class, no security attributes, `key` receives the handle.
        let err = unsafe {
            RegCreateKeyExW(hive_key(hive), p.as_ptr(), 0, std::ptr::null(), REG_OPTION_NON_VOLATILE, KEY_READ | KEY_WRITE, std::ptr::null(), &mut key, std::ptr::null_mut())
        };
        if err == 0 { Ok(RegKey(key)) } else { Err(Fail::other(format!("registry key {path}: error {err}"))) }
    }

    /// The value's type and bytes; None when it isn't there.
    fn raw(&self, name: &str) -> Option<(u32, Vec<u8>)> {
        let n = wide(name);
        let mut kind = 0u32;
        let mut size = 0u32;
        // SAFETY: first call asks only the size; second fills a buffer of that size.
        unsafe {
            if RegQueryValueExW(self.0, n.as_ptr(), std::ptr::null(), &mut kind, std::ptr::null_mut(), &mut size) != 0 {
                return None;
            }
            let mut buf = vec![0u8; size as usize];
            if RegQueryValueExW(self.0, n.as_ptr(), std::ptr::null(), &mut kind, buf.as_mut_ptr(), &mut size) != 0 {
                return None;
            }
            buf.truncate(size as usize);
            Some((kind, buf))
        }
    }

    fn utf16(bytes: &[u8]) -> Vec<u16> {
        bytes.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect()
    }

    /// GetValue(name) as a string (REG_SZ or REG_EXPAND_SZ); None for anything else or nothing.
    pub fn get_string(&self, name: &str) -> Option<String> {
        let (kind, bytes) = self.raw(name)?;
        if kind != REG_SZ && kind != REG_EXPAND_SZ {
            return None;
        }
        Some(from_wide(&RegKey::utf16(&bytes)))
    }

    /// GetValue(name) as byte[] (REG_BINARY).
    pub fn get_binary(&self, name: &str) -> Option<Vec<u8>> {
        let (kind, bytes) = self.raw(name)?;
        if kind == REG_BINARY { Some(bytes) } else { None }
    }

    /// GetValue(name) as string[] (REG_MULTI_SZ).
    pub fn get_multi_string(&self, name: &str) -> Option<Vec<String>> {
        let (kind, bytes) = self.raw(name)?;
        if kind != REG_MULTI_SZ {
            return None;
        }
        let chars = RegKey::utf16(&bytes);
        let mut out = Vec::new();
        for part in chars.split(|&c| c == 0) {
            if part.is_empty() {
                break;
            }
            out.push(String::from_utf16_lossy(part));
        }
        Some(out)
    }

    pub fn set_string(&self, name: &str, value: &str) -> Res<()> {
        let n = wide(name);
        let v = wide(value);
        // SAFETY: the data is `v`'s bytes, its zero included, as REG_SZ wants.
        let err = unsafe { RegSetValueExW(self.0, n.as_ptr(), 0, REG_SZ, v.as_ptr() as *const u8, (v.len() * 2) as u32) };
        if err == 0 { Ok(()) } else { Err(Fail::other(format!("registry value {name}: error {err}"))) }
    }

    pub fn set_binary(&self, name: &str, value: &[u8]) -> Res<()> {
        let n = wide(name);
        // SAFETY: plain call over `value`'s bytes.
        let err = unsafe { RegSetValueExW(self.0, n.as_ptr(), 0, REG_BINARY, value.as_ptr(), value.len() as u32) };
        if err == 0 { Ok(()) } else { Err(Fail::other(format!("registry value {name}: error {err}"))) }
    }

    /// DeleteValue(name, throwOnMissingValue: false).
    pub fn delete_value(&self, name: &str) -> Res<()> {
        let n = wide(name);
        // SAFETY: plain call.
        let err = unsafe { RegDeleteValueW(self.0, n.as_ptr()) };
        if err == 0 || err == ERROR_FILE_NOT_FOUND { Ok(()) } else { Err(Fail::other(format!("registry value {name}: error {err}"))) }
    }

    /// GetValueNames.
    pub fn value_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        for i in 0u32.. {
            let mut buf = vec![0u16; 16384];
            let mut len = buf.len() as u32;
            // SAFETY: `buf` holds `len` chars; nothing else is asked for.
            let err = unsafe {
                RegEnumValueW(self.0, i, buf.as_mut_ptr(), &mut len, std::ptr::null(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut())
            };
            if err != 0 {
                break;
            }
            names.push(String::from_utf16_lossy(&buf[..len as usize]));
        }
        names
    }
}

// ───────── the process list (System.Diagnostics.Process) ─────────

/// One process of Windows' list.
#[derive(Clone, Debug)]
pub struct ProcEntry {
    pub pid: u32,
    pub parent: u32,
    /// "RobloxPlayerBeta.exe".
    pub exe: String,
}

/// Every process now (Toolhelp's snapshot); None when Windows refused one.
pub fn process_list() -> Option<Vec<ProcEntry>> {
    // SAFETY: the snapshot handle is closed below; the entry's dwSize is set as Windows requires.
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return None;
        }
        let mut list = Vec::new();
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut ok = Process32FirstW(snapshot, &mut entry) != 0;
        while ok {
            list.push(ProcEntry { pid: entry.th32ProcessID, parent: entry.th32ParentProcessID, exe: from_wide(&entry.szExeFile) });
            ok = Process32NextW(snapshot, &mut entry) != 0;
        }
        CloseHandle(snapshot);
        Some(list)
    }
}

/// Path.GetFileNameWithoutExtension of a file name or path.
pub fn file_stem(path: &str) -> String {
    let name = path.rsplit(['\\', '/']).next().unwrap_or(path);
    match name.rfind('.') {
        Some(dot) => name[..dot].to_string(),
        None => name.to_string(),
    }
}

/// Process.GetProcessById(pid).ProcessName: None when no process has that id (ArgumentException).
pub fn process_name(pid: i32) -> Option<String> {
    process_list()?.into_iter().find(|p| p.pid as i32 == pid).map(|p| file_stem(&p.exe))
}

/// Process.GetProcessesByName(name): the ids of every process whose name (without ".exe") is `name`, any case.
pub fn pids_by_name(name: &str) -> Option<Vec<i32>> {
    Some(process_list()?.into_iter().filter(|p| file_stem(&p.exe).eq_ignore_ascii_case(name)).map(|p| p.pid as i32).collect())
}

/// %LOCALAPPDATA% (Environment.SpecialFolder.LocalApplicationData).
pub fn local_app_data() -> std::path::PathBuf {
    std::env::var_os("LOCALAPPDATA").map(std::path::PathBuf::from).unwrap_or_default()
}

/// Opaque pointer helper for calls that take `void*`.
pub fn void<T>(x: &mut T) -> *mut c_void {
    x as *mut T as *mut c_void
}
