// Settings › Appearance › Follow Wallpaper Engine: the custom theme takes the wallpaper Wallpaper Engine shows. Read
// from its own files, nothing hooked into it: Steam's libraries (libraryfolders.vdf) lead to
// steamapps/common/wallpaper_engine/config.json, whose selectedwallpapers names the wallpaper's file; the folder beside it
// holds project.json (its type, its preview picture and its "scheme colour", the colour Wallpaper Engine itself gives
// Windows) and the picture or video. A scene (scene.pkg) can't be played outside Wallpaper Engine, so its preview
// picture stands in; a video wallpaper comes as the video.
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub struct Wall {
    /// The picture or video for the custom theme.
    pub media: PathBuf,
    /// Its scheme colour as "#rrggbb", when it is one an accent can be (not near black, white or grey).
    pub accent: Option<String>,
}

static SAID: Mutex<String> = Mutex::new(String::new());

/// A failure, in error.log once (not every few seconds while it lasts).
fn say(line: String) {
    let mut said = SAID.lock().unwrap_or_else(|p| p.into_inner());
    if *said != line {
        #[cfg(feature = "core")]
        crate::core::log::write(&line);
        *said = line;
    }
}

#[cfg(windows)]
fn steam_dir() -> Option<PathBuf> {
    use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_SZ, RegGetValueW};
    let key: Vec<u16> = "Software\\Valve\\Steam\0".encode_utf16().collect();
    let name: Vec<u16> = "SteamPath\0".encode_utf16().collect();
    let mut buffer = vec![0u16; 1024];
    let mut size = (buffer.len() * 2) as u32;
    let r = unsafe { RegGetValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), buffer.as_mut_ptr().cast(), &mut size) };
    if r != 0 {
        return None;
    }
    let n = buffer.iter().position(|&u| u == 0).unwrap_or(buffer.len());
    let p = PathBuf::from(String::from_utf16_lossy(&buffer[..n]));
    p.is_dir().then_some(p)
}

#[cfg(not(windows))]
fn steam_dir() -> Option<PathBuf> {
    None
}

/// Wallpaper Engine's config.json, in whichever Steam library holds it; None when it isn't installed.
pub fn config() -> Option<PathBuf> {
    static FOUND: OnceLock<Option<PathBuf>> = OnceLock::new();
    FOUND
        .get_or_init(|| {
            let steam = steam_dir()?;
            let mut libraries = vec![steam.clone()];
            // "path"		"D:\\SteamLibrary"
            if let Ok(vdf) = std::fs::read_to_string(steam.join("steamapps").join("libraryfolders.vdf")) {
                for line in vdf.lines() {
                    let parts: Vec<&str> = line.split('"').collect();
                    if parts.len() >= 4 && parts[1] == "path" {
                        libraries.push(PathBuf::from(parts[3].replace("\\\\", "\\")));
                    }
                }
            }
            libraries.into_iter().map(|l| l.join("steamapps").join("common").join("wallpaper_engine").join("config.json")).find(|c| c.is_file())
        })
        .clone()
}

/// The wallpaper's file on the first screen, as config.json names it.
fn selected(config: &Value) -> Option<String> {
    let users = config.as_object()?;
    for user in users.values() {
        let Some(chosen) = user.pointer("/general/wallpaperconfig/selectedwallpapers").and_then(Value::as_object) else { continue };
        let first = chosen.get("Monitor0").or_else(|| chosen.values().next());
        if let Some(file) = first.and_then(|m| m.get("file")).and_then(Value::as_str).filter(|f| !f.is_empty()) {
            return Some(file.to_string());
        }
    }
    None
}

/// "0.58 0.75 0.88" as "#94bfe0", when it is a colour with some life (an accent of near black or grey reads as nothing).
fn scheme(text: &str) -> Option<String> {
    let v: Vec<f32> = text.split_whitespace().filter_map(|n| n.parse().ok()).collect();
    if v.len() < 3 {
        return None;
    }
    let (r, g, b) = (v[0].clamp(0., 1.), v[1].clamp(0., 1.), v[2].clamp(0., 1.));
    let (hi, lo) = (r.max(g).max(b), r.min(g).min(b));
    if hi < 0.25 || hi - lo < 0.12 {
        return None;
    }
    Some(format!("#{:02x}{:02x}{:02x}", (r * 255.).round() as u8, (g * 255.).round() as u8, (b * 255.).round() as u8))
}

// ── The wallpaper live ──────────────────────────────────────────────────────────────────────────────────────────────
// Wallpaper Engine plays a wallpaper in a window of its own when asked on its command line ("-control openWallpaper
// -file <project.json> -playInWindow <name> -width -height", help.wallpaperengine.io › Command Line Controls; it must be
// running). Bloxkeep asks for one named POP_OUT, puts it out of sight, and shows its picture behind the page
// (window_capture.rs through video.rs), so scenes and web wallpapers move as they do on the desktop.

const POP_OUT: &str = "Bloxkeep wallpaper";

/// The window playing it and the wallpaper's file, while one is open.
static LIVE: std::sync::Mutex<Option<(isize, String)>> = std::sync::Mutex::new(None);

/// When Bloxkeep's window last showed its backdrop, and its size then (pixels): scene.rs › layer, while seen.
static SEEN: Mutex<Option<(std::time::Instant, u32, u32)>> = Mutex::new(None);

/// Not seen for this long (minimized, hidden in the tray): Wallpaper Engine's second copy is closed, as it draws for
/// nobody; it opens again within a few seconds of the window coming back, its picture standing in meanwhile.
const UNSEEN: std::time::Duration = std::time::Duration::from_secs(30);

/// The widest the second copy is played: past this the copying costs more than the sharpness gives (the page often
/// blurs or washes it). video.rs keeps live frames this wide, so none is thinned.
pub const LIVE_W: u32 = 1920;

/// Bloxkeep's window shows its backdrop at `w`×`h` pixels now (scene.rs, while the window is seen).
pub fn seen(w: u32, h: u32) {
    *SEEN.lock().unwrap_or_else(|p| p.into_inner()) = Some((std::time::Instant::now(), w, h));
}

/// The size to play it at: the window's own shape (no part cropped or stretched out of shape), at most LIVE_W wide.
fn play_size() -> Option<(u32, u32)> {
    let (at, w, h) = (*SEEN.lock().unwrap_or_else(|p| p.into_inner()))?;
    if at.elapsed() > UNSEEN || w < 2 || h < 2 {
        return None;
    }
    let (w, h) = if w > LIVE_W { (LIVE_W, ((h as u64 * LIVE_W as u64) / w as u64) as u32) } else { (w, h) };
    Some((w.max(320), h.max(180)))
}

#[cfg(windows)]
fn is_window(h: isize) -> bool {
    // SAFETY: IsWindow takes any handle.
    unsafe { windows_sys::Win32::UI::WindowsAndMessaging::IsWindow(h as _) != 0 }
}

#[cfg(not(windows))]
fn is_window(_: isize) -> bool {
    false
}

/// The window to show behind the page, while the wallpaper plays in it.
pub fn live() -> Option<isize> {
    LIVE.lock().unwrap_or_else(|p| p.into_inner()).as_ref().map(|l| l.0).filter(|h| is_window(*h))
}

/// Wallpaper Engine's running program (wallpaper64.exe or wallpaper32.exe), when it runs.
#[cfg(windows)]
fn running_exe() -> Option<PathBuf> {
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        System::{
            Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS},
            Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW},
        },
    };
    // SAFETY: a process snapshot walked with a sized entry, closed after; the found process opened only to read its path.
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap.is_null() || snap as isize == -1 {
            return None;
        }
        let mut e: PROCESSENTRY32W = std::mem::zeroed();
        e.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut pid = None;
        let mut ok = Process32FirstW(snap, &mut e) != 0;
        while ok {
            let n = e.szExeFile.iter().position(|&c| c == 0).unwrap_or(e.szExeFile.len());
            let name = String::from_utf16_lossy(&e.szExeFile[..n]).to_ascii_lowercase();
            if name == "wallpaper64.exe" || name == "wallpaper32.exe" {
                pid = Some(e.th32ProcessID);
                break;
            }
            ok = Process32NextW(snap, &mut e) != 0;
        }
        CloseHandle(snap);
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid?);
        if process.is_null() {
            return None;
        }
        let mut buffer = [0u16; 1024];
        let mut size = buffer.len() as u32;
        let got = QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut size) != 0;
        CloseHandle(process);
        got.then(|| PathBuf::from(String::from_utf16_lossy(&buffer[..size as usize])))
    }
}

#[cfg(not(windows))]
fn running_exe() -> Option<PathBuf> {
    None
}

/// Keeps `file`'s wallpaper playing live for the page (Follow Wallpaper Engine on): asked of Wallpaper Engine when it
/// runs and no window plays this wallpaper yet. Waits for the window: off the window thread.
pub fn keep_live(file: &str) {
    if file.is_empty() {
        return;
    }
    let Some(exe) = running_exe() else {
        // (Wallpaper Engine closed: its window went with it.)
        *LIVE.lock().unwrap_or_else(|p| p.into_inner()) = None;
        return;
    };
    let Some((pw, ph)) = play_size() else {
        // Bloxkeep's window unseen for a while: nobody watches the second copy.
        stop_live();
        return;
    };
    let playing = LIVE.lock().unwrap_or_else(|p| p.into_inner()).as_ref().filter(|(h, f)| f == file && is_window(*h)).map(|l| l.0);
    if let Some(window) = playing {
        // (Bloxkeep's window resized or maximized: the copy follows, so its picture keeps the window's shape.)
        fit(window, pw, ph);
        return;
    }
    let Some(project) = Path::new(file).parent().map(|f| f.join("project.json")).filter(|p| p.is_file()) else {
        say(format!("Wallpaper Engine: no project.json beside {file}, so it can't be played live"));
        return;
    };
    // (One left behind by a Bloxkeep ended without closing it, from Task Manager say, may still play another wallpaper:
    // closed first, so the window found below is the new one.)
    if let Some(old) = find_pop_out() {
        close(old);
        for _ in 0..20 {
            if !is_window(old) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
    // Asked for past the right edge of every screen from the start (Wallpaper Engine's own -x and -y), so the window
    // is never on a screen: found there and moved away up to a quarter second later, it showed for a moment each time
    // the following was turned on (a "terminal" flashing). Should Wallpaper Engine make no window for a place off the
    // screens, it is asked as before, and the window looked for often, to be moved soon.
    let ask = |off_screen: bool| -> Option<isize> {
        let mut command = std::process::Command::new(&exe);
        command.args(["-control", "openWallpaper", "-file"]).arg(&project).args(["-playInWindow", POP_OUT, "-width"]).arg(pw.to_string()).arg("-height").arg(ph.to_string());
        if off_screen {
            command.arg("-x").arg(off_screens().to_string()).args(["-y", "0"]);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        if let Err(e) = command.status() {
            say(format!("Wallpaper Engine: asking {} to play the wallpaper in a window: {e}", exe.display()));
            return None;
        }
        // Its window, once Wallpaper Engine has made it.
        (0..if off_screen { 100 } else { 160 }).find_map(|_| {
            let h = find_pop_out();
            if h.is_none() {
                std::thread::sleep(std::time::Duration::from_millis(30));
            }
            h
        })
    };
    let Some(h) = ask(true).or_else(|| {
        say("Wallpaper Engine: no window came for a place off the screens; asked again without one".to_string());
        ask(false)
    }) else {
        say(format!("Wallpaper Engine: no window named \"{POP_OUT}\" came after asking it to play {} in one", project.display()));
        return;
    };
    hide(h);
    // (-width/-height size its inside, as measured 2026-11-18; made so again should Wallpaper Engine differ.)
    fit(h, pw, ph);
    *LIVE.lock().unwrap_or_else(|p| p.into_inner()) = Some((h, file.to_string()));
    still_of(h, file, &project);
}

/// A still of the wallpaper as it plays, for the custom theme's picture in place of its preview (a scene's or web
/// wallpaper's preview is a small square, 768×768 on a real one: the panels' frosted copy of it lay at another size
/// and place than the live wallpaper around them, and it showed blown up whenever Wallpaper Engine was closed). Once
/// per wallpaper; a video wallpaper's own file is already the picture.
#[cfg(all(windows, feature = "core"))]
static STILL_TAKEN: Mutex<String> = Mutex::new(String::new());
/// A still taken and not yet handed to the theme (check() hands it): the wallpaper's file and the still's.
static STILL: Mutex<Option<(String, PathBuf)>> = Mutex::new(None);

#[cfg(all(windows, feature = "core"))]
fn still_of(window: isize, file: &str, project: &Path) {
    let kind = std::fs::read(project).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()).and_then(|p| p.get("type").and_then(Value::as_str).map(str::to_lowercase)).unwrap_or_default();
    if kind == "video" {
        return;
    }
    {
        let mut taken = STILL_TAKEN.lock().unwrap_or_else(|p| p.into_inner());
        if *taken == file {
            return;
        }
        *taken = file.to_string();
    }
    let Some((w, h, bgra)) = crate::window_capture::still(window, LIVE_W) else {
        say(format!("Wallpaper Engine: no still of {file} came from its window"));
        return;
    };
    let rgb: Vec<u8> = bgra.chunks_exact(4).flat_map(|p| [p[2], p[1], p[0]]).collect();
    let to = still_file(file);
    let written = std::fs::File::create(&to).map_err(|e| e.to_string()).and_then(|f| {
        image::codecs::jpeg::JpegEncoder::new_with_quality(std::io::BufWriter::new(f), 90).encode(&rgb, w, h, image::ExtendedColorType::Rgb8).map_err(|e| e.to_string())
    });
    match written {
        Ok(()) => *STILL.lock().unwrap_or_else(|p| p.into_inner()) = Some((file.to_string(), to)),
        Err(e) => say(format!("Wallpaper Engine: writing a still of {file} to {}: {e}", to.display())),
    }
}

#[cfg(not(all(windows, feature = "core")))]
fn still_of(_: isize, _: &str, _: &Path) {}

/// Closes the window playing it (Follow Wallpaper Engine turned off).
pub fn stop_live() {
    let Some((h, _)) = LIVE.lock().unwrap_or_else(|p| p.into_inner()).take() else { return };
    close(h);
}

fn close(h: isize) {
    #[cfg(windows)]
    // SAFETY: a close asked of a window by its handle; a gone one ignores it.
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW(h as _, windows_sys::Win32::UI::WindowsAndMessaging::WM_CLOSE, 0, 0);
    }
    let _ = h;
}

#[cfg(windows)]
fn find_pop_out() -> Option<isize> {
    let name: Vec<u16> = POP_OUT.encode_utf16().chain([0]).collect();
    // SAFETY: a window looked up by its title, ending in 0.
    let h = unsafe { windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW(std::ptr::null(), name.as_ptr()) };
    (!h.is_null()).then_some(h as isize)
}

#[cfg(not(windows))]
fn find_pop_out() -> Option<isize> {
    None
}

/// Where "out of sight" is: this far right, past the right edge of every screen.
#[cfg(windows)]
fn off_screens() -> i32 {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_XVIRTUALSCREEN};
    // SAFETY: plain queries.
    unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) + GetSystemMetrics(SM_CXVIRTUALSCREEN) + 400 }
}

#[cfg(not(windows))]
fn off_screens() -> i32 {
    0
}

/// Out of sight: past the right edge of every screen, off the taskbar and Alt+Tab, never taking the focus. Not
/// minimized: a minimized window isn't drawn, so there'd be nothing to show.
#[cfg(windows)]
fn hide(h: isize) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GetWindowLongPtrW, HWND_BOTTOM, SWP_NOACTIVATE, SWP_NOSIZE, SetWindowLongPtrW, SetWindowPos, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
    };
    // SAFETY: Windows calls on a window handle Windows gave; a gone window makes them fail harmlessly.
    unsafe {
        let ex = GetWindowLongPtrW(h as _, GWL_EXSTYLE);
        SetWindowLongPtrW(h as _, GWL_EXSTYLE, (ex | WS_EX_TOOLWINDOW as isize) & !(WS_EX_APPWINDOW as isize));
        SetWindowPos(h as _, HWND_BOTTOM, off_screens(), 0, 0, 0, SWP_NOACTIVATE | SWP_NOSIZE);
    }
}

#[cfg(not(windows))]
fn hide(_: isize) {}

/// Makes the window's inside `w`×`h` pixels, when it is more than a few pixels off (left alone otherwise: a resize
/// makes Wallpaper Engine set up its drawing again).
#[cfg(windows)]
fn fit(window: isize, w: u32, h: u32) {
    use windows_sys::Win32::{
        Foundation::RECT,
        UI::WindowsAndMessaging::{GetClientRect, GetWindowRect, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOZORDER, SetWindowPos},
    };
    let mut outer = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    let mut inner = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    // SAFETY: queries and a resize of a window by its handle; a gone window fails them harmlessly.
    unsafe {
        if GetWindowRect(window as _, &mut outer) == 0 || GetClientRect(window as _, &mut inner) == 0 {
            return;
        }
        let (iw, ih) = (inner.right, inner.bottom);
        if (iw - w as i32).abs() <= 8 && (ih - h as i32).abs() <= 8 {
            return;
        }
        let (edge_w, edge_h) = ((outer.right - outer.left) - iw, (outer.bottom - outer.top) - ih);
        SetWindowPos(window as _, std::ptr::null_mut(), 0, 0, w as i32 + edge_w, h as i32 + edge_h, SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE);
    }
}

#[cfg(not(windows))]
fn fit(_: isize, _: u32, _: u32) {}

const VIDEO_LIMIT: u64 = 50 * 1024 * 1024;
const PICTURE_LIMIT: u64 = 20 * 1024 * 1024;

fn wall_in(folder: &Path, file: &Path) -> Result<Option<Wall>, String> {
    let project: Value = std::fs::read(folder.join("project.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .ok_or_else(|| format!("no readable project.json in {}", folder.display()))?;
    let kind = project.get("type").and_then(Value::as_str).unwrap_or_default().to_lowercase();
    let ext = |p: &Path| p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let size = |p: &Path| std::fs::metadata(p).map(|m| m.len()).unwrap_or(u64::MAX);
    let video = (kind == "video" && ["mp4", "webm"].contains(&ext(file).as_str()) && size(file) <= VIDEO_LIMIT).then(|| file.to_path_buf());
    let picture = || {
        let named = project.get("preview").and_then(Value::as_str).map(|p| folder.join(p));
        named
            .into_iter()
            .chain(["preview.jpg", "preview.png", "preview.gif"].map(|n| folder.join(n)))
            .find(|p| p.is_file() && ["jpg", "jpeg", "png", "gif", "webp", "bmp"].contains(&ext(p).as_str()) && size(p) <= PICTURE_LIMIT)
    };
    let Some(media) = video.or_else(picture) else { return Ok(None) };
    let accent = project.pointer("/general/properties/schemecolor/value").and_then(Value::as_str).and_then(scheme);
    Ok(Some(Wall { media, accent }))
}

/// The start of a still's file name (`still_of`), as it is handed to the theme.
const STILL_NAME: &str = "bloxkeep-wallpaper-";

/// Where the still of the wallpaper `file` is written (a name of its own per wallpaper; useWallpaperEngine copies it
/// into the pictures folder).
fn still_file(file: &str) -> PathBuf {
    let tag = file.bytes().fold(0xcbf2_9ce4_8422_2325u64, |n, b| (n ^ b as u64).wrapping_mul(0x100_0000_01b3));
    std::env::temp_dir().join(format!("{STILL_NAME}{tag:016x}.jpg"))
}

/// `media` is a still taken of the wallpaper as it plays (not its preview or its video).
pub fn is_still(media: &Path) -> bool {
    media.file_name().is_some_and(|n| n.to_string_lossy().starts_with(STILL_NAME))
}

/// What changed since `last` (config.json's time and the wallpaper's file): Some(key, wall) when the wallpaper is a new
/// one; None when nothing changed, Wallpaper Engine isn't there or shows nothing we can use. Reads files: off the window.
///
/// `kept`: the wallpaper whose own still is the theme's picture already, from a run before. At a start (`last` None)
/// with that wallpaper still on show nothing is handed over and no second still is taken: every start put the
/// wallpaper's small preview back over the kept still, and the page showed it blown up and soft until the new still
/// came seconds later.
pub fn check(last: Option<&(SystemTime, String)>, kept: Option<&str>) -> Option<((SystemTime, String), Option<Wall>)> {
    let config = config()?;
    let at = std::fs::metadata(&config).and_then(|m| m.modified()).ok()?;
    // A still of the wallpaper on show just taken (still_of): it becomes the picture, the accent kept.
    let fresh = STILL.lock().unwrap_or_else(|p| p.into_inner()).take();
    if let (Some((file, still)), Some((t, shown))) = (fresh, last)
        && file == *shown
        && let Some(folder) = Path::new(&file).parent()
    {
        let wall = wall_in(folder, Path::new(&file)).ok().flatten().map(|w| Wall { media: still.clone(), ..w }).unwrap_or(Wall { media: still, accent: None });
        return Some(((*t, file), Some(wall)));
    }
    if last.is_some_and(|(t, _)| *t == at) {
        return None;
    }
    let parsed: Value = match std::fs::read(&config).map_err(|e| e.to_string()).and_then(|b| serde_json::from_slice(&b).map_err(|e| e.to_string())) {
        Ok(v) => v,
        Err(e) => {
            // (Wallpaper Engine may be writing it this moment: tried again on the next look.)
            say(format!("Wallpaper Engine: reading {}: {e}", config.display()));
            return None;
        }
    };
    let Some(file) = selected(&parsed) else { return Some(((at, String::new()), None)) };
    if last.is_some_and(|(_, f)| *f == file) {
        return Some(((at, file), None));
    }
    if last.is_none() && kept == Some(file.as_str()) {
        #[cfg(all(windows, feature = "core"))]
        {
            *STILL_TAKEN.lock().unwrap_or_else(|p| p.into_inner()) = file.clone();
        }
        return Some(((at, file), None));
    }
    let path = PathBuf::from(&file);
    let Some(folder) = path.parent() else { return Some(((at, file), None)) };
    let wall = match wall_in(folder, &path) {
        Ok(w) => w,
        Err(e) => {
            say(format!("Wallpaper Engine: {e}"));
            None
        }
    };
    // A still of this wallpaper taken before (this run or an earlier one) is handed over in place of its preview, at
    // once: the preview first and the still seconds later was two changes of look, the first blown up and soft.
    #[cfg(all(windows, feature = "core"))]
    let wall = wall.map(|w| {
        let still = still_file(&file);
        let is_picture = !["mp4", "webm"].contains(&w.media.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default().as_str());
        if is_picture && still.metadata().is_ok_and(|m| m.len() > 0) {
            *STILL_TAKEN.lock().unwrap_or_else(|p| p.into_inner()) = file.clone();
            Wall { media: still, ..w }
        } else {
            w
        }
    });
    if wall.is_none() {
        say(format!("Wallpaper Engine: nothing to show for {file} (no video or preview picture it can use)"));
    }
    Some(((at, file), wall))
}
