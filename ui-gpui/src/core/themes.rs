//! Themes.cs: the colour themes. The page draws itself from the tokens in app.css; the window frame, its title bar and
//! the sign-in window need a few of the same colours, which are repeated here (keep the two in step).
use std::sync::LazyLock;

use regex::Regex;

use crate::core::prefs::{self, CustomTheme};

/// System.Drawing.Color without alpha.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const WHITE: Color = Color { r: 255, g: 255, b: 255 };
    pub const BLACK: Color = Color { r: 0, g: 0, b: 0 };
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Theme {
    pub id: &'static str,
    pub dark: bool,
    pub background: Color,
    pub surface: Color,
    pub text: Color,
    pub text2: Color,
    pub line: Color,
}

const fn rgb(rgb: u32) -> Color {
    Color { r: ((rgb >> 16) & 0xff) as u8, g: ((rgb >> 8) & 0xff) as u8, b: (rgb & 0xff) as u8 }
}

const fn theme(id: &'static str, dark: bool, background: u32, surface: u32, text: u32, text2: u32, line: u32) -> Theme {
    Theme { id, dark, background: rgb(background), surface: rgb(surface), text: rgb(text), text2: rgb(text2), line: rgb(line) }
}

// Background and Surface are app.css's --bg and --surface; Text/Text2 its --text/--text-2. For the glass themes
// (a scene behind a translucent page) Background is the page's top edge as it actually shows — the scene's top
// colour under the page's white or dark wash — so the title bar runs into it; their Surface is a solid stand-in.
#[rustfmt::skip]
static ALL: &[Theme] = &[
    theme("light", false, 0xf3f4f7, 0xffffff, 0x14171f, 0x3d4352, 0xd5d8df),
    theme("paper", false, 0xf3eee3, 0xfbf8f1, 0x2a241a, 0x50473a, 0xdcd3c1),
    theme("dim", true, 0x1d2026, 0x24272e, 0xe6e8ec, 0xbabec8, 0x464b56),
    theme("dark", true, 0x0a0c11, 0x11151e, 0xe9ecf3, 0xb6bccc, 0x3c4456),
    theme("black", true, 0x000000, 0x0c0d10, 0xeceef2, 0xb4b8c2, 0x34363d),
    theme("frost", true, 0x2e3440, 0x3b4252, 0xeceff4, 0xd8dee9, 0x4c566a),
    theme("ocean", true, 0x0a1622, 0x0f2030, 0xe3eef7, 0xa9c1d4, 0x24425a),
    theme("forest", true, 0x0d1410, 0x142019, 0xe4efe6, 0xb3c7b8, 0x2e4435),
    theme("mocha", true, 0x1b1512, 0x241c18, 0xf2e8df, 0xcdbcae, 0x4a3d34),
    theme("grape", true, 0x17141f, 0x201c2b, 0xefeaf9, 0xc4bbdb, 0x403856),
    theme("retro", false, 0x008080, 0xc0c0c0, 0x000000, 0x1a1a1a, 0x808080),
    theme("terminal", true, 0x030603, 0x081008, 0xb9f7c3, 0x86d996, 0x1f4a27),
    // Text2 must differ from Text: it is what a disabled menu item is drawn in.
    theme("contrast", true, 0x000000, 0x000000, 0xffffff, 0xc4c4c4, 0x8a8a8a),
    // Bloxkeep's own picture theme (the keep at dusk; theme.rs's KEEP).
    theme("keep", true, 0x070a14, 0x0c1230, 0xf1f4fa, 0xd0d6ea, 0x2a3358),
    theme("aurora", true, 0x040813, 0x141b2c, 0xf1f4fa, 0xc9d0de, 0x3a4458),
    theme("sunset", true, 0x22133f, 0x3a1a40, 0xfff1ea, 0xf0cfc4, 0x6a3f5c),
    theme("city", true, 0x050718, 0x161830, 0xf1f4fa, 0xc9d0de, 0x363a5c),
    theme("space", true, 0x05040f, 0x141223, 0xf1f4fa, 0xc9d0de, 0x34304a),
    theme("mist", false, 0xcfdad5, 0xeef2ef, 0x15241f, 0x3a4c45, 0xc3cfc9),
    theme("snow", false, 0x8cb5dd, 0xeef4fa, 0x122033, 0x36475f, 0xc2d0e0),
    theme("desert", false, 0xf2c595, 0xfbeedb, 0x2b1a0b, 0x56391d, 0xe2c8a4),
    theme("meadow", false, 0x89c9f0, 0xeaf6fc, 0x13240f, 0x344b2e, 0xbcd8c4),
    theme("lava", true, 0x140806, 0x1f0e0a, 0xfbe9e1, 0xe0b9a8, 0x4d2a1f),
    theme("neon", true, 0x0b0820, 0x151033, 0xf3eaff, 0xc7b8f0, 0x3a2f6e),
    // Text2 stays a step lighter than app.css's --text-2 (now the same as --text): it draws disabled menu items.
    theme("gameboy", false, 0x9bbc0f, 0xa9c934, 0x0f380f, 0x1d4a1d, 0x306230),
    theme("blueprint", true, 0x0f3a68, 0x134577, 0xeef6ff, 0xc0dbf5, 0x3b77ad),
    theme("candy", false, 0xfdf4fb, 0xfffafe, 0x3b2340, 0x6b4a73, 0xecd3e6),
    theme("matcha", false, 0xf1f3e6, 0xfafbf3, 0x263018, 0x4b5636, 0xd6dcbf),
    theme("newspaper", false, 0xf4f1ea, 0xfbf9f4, 0x111111, 0x333333, 0xcfc9bb),
    theme("halloween", true, 0x0d0714, 0x1d1222, 0xf6eee6, 0xd6c7bb, 0x3e3346),
    theme("lunar", true, 0x2c0709, 0x3a0c0f, 0xfbeee0, 0xe6c9b0, 0x6e3a33),
    theme("spring", false, 0xdaeefa, 0xf3f8fb, 0x2a1d22, 0x5a4048, 0xe2d2d8),
    theme("sakura", false, 0xfbf0f2, 0xfffafb, 0x3a1f27, 0x6a4450, 0xe9cdd4),
    theme("summer", false, 0xa0ddf5, 0xeef8fc, 0x0f2a3a, 0x2e4d5e, 0xc6dde6),
    theme("autumn", false, 0xf7dfbe, 0xfbf0e0, 0x2f1a0c, 0x5a3a22, 0xe6cfb4),
    theme("midautumn", true, 0x07112c, 0x13224a, 0xeef2fb, 0xc4cde2, 0x34406a),
    theme("christmas", true, 0x091426, 0x14263d, 0xeef4fb, 0xc6d3e2, 0x33465e),
    theme("newyear", true, 0x070714, 0x14112e, 0xf3f1fb, 0xcbc6e2, 0x36325a),
    theme("valentine", true, 0x260f2d, 0x41153f, 0xfdeef3, 0xecc6d4, 0x764d6e),
    theme("rainy", true, 0x0b141e, 0x14222f, 0xe8eff6, 0xb8c7d6, 0x495765),
    theme("skylanterns", true, 0x080c22, 0x12193f, 0xeef0fb, 0xc4c9e2, 0x4c5275),
    theme("hoian", true, 0x0d0f25, 0x181737, 0xfbf1e4, 0xe2cdb2, 0x595362),
    theme("underwater", true, 0x09556e, 0x073452, 0xe8f6fc, 0xb4d6e6, 0x406982),
    theme("jungle", true, 0x0b261e, 0x103224, 0xe8f6ee, 0xb6d6c2, 0x49695a),
    theme("moon", true, 0x03040b, 0x0e101e, 0xf0f2f8, 0xc8ccd8, 0x494b59),
    theme("winter", false, 0xd6e4f5, 0xf7f8fc, 0x14233a, 0x3a4c66, 0xdbdee3),
    theme("school", false, 0xb4dff7, 0xeef8fd, 0x10283a, 0x2e4a5e, 0xd9e0e5),
    theme("kites", false, 0x99d3f3, 0xe9f6fc, 0x0f2a3a, 0x2e4d5e, 0xd9e0e5),
    theme("dalat", false, 0xe3eff1, 0xf3f8f6, 0x12291f, 0x2f4a3f, 0xd9e0dd),
    theme("savanna", false, 0xf7d6ad, 0xfdf5e8, 0x2a1a08, 0x54391b, 0xe4dccf),
    theme("phongnha", true, 0x101e22, 0x0c1e20, 0xe6f4ec, 0xb4d2c4, 0x44605a),
    theme("nightbeach", true, 0x070f25, 0x0e1832, 0xeaeef8, 0xbcc6dc, 0x454f6a),
    theme("coral", false, 0xfbefe9, 0xfff9f5, 0x2e1a14, 0x5c3c32, 0xecd8cd),
    theme("rose", true, 0x1d1216, 0x27181e, 0xf6e7ec, 0xd6bcc6, 0x4d3540),
    theme("olive", true, 0x17180f, 0x202216, 0xeeeedd, 0xc8c9ad, 0x42452f),
    theme("solarized", true, 0x002b36, 0x04313c, 0xeee8d5, 0xc3c8bd, 0x24535f),
    theme("blockworld", false, 0xa0dcf7, 0xf4faf6, 0x13241a, 0x2f4a36, 0xd9e0dc),
    theme("peachfall", false, 0xfdf1f2, 0xfff8f9, 0x2e141c, 0x5a3040, 0xe6ddde),
    theme("greatwave", false, 0xd7ccaf, 0xf8f4ea, 0x0f2340, 0x2c4568, 0xdcdbd6),
    theme("waterlilies", false, 0x799ea2, 0xf2f7f8, 0x10242e, 0x2c4756, 0xd7dee0),
    theme("blocksart", false, 0xf7f3e7, 0xfbf9f2, 0x141414, 0x333333, 0xdfded7),
    theme("miaminight", true, 0x1f2356, 0x14103a, 0xf2eefb, 0xcbc4e6, 0x423c66),
    theme("lanternriver", true, 0x08091a, 0x0e0f22, 0xf1eff6, 0xcac7d8, 0x3f3e52),
    theme("deepsea", true, 0x0d3560, 0x061c3a, 0xe8f4fc, 0xb9d5e8, 0x36506a),
    theme("snowfall", true, 0x4a525c, 0x121c28, 0xeef4fa, 0xc4d4e4, 0x3e4e60),
    theme("nightharbour", true, 0x3a3238, 0x18121e, 0xf3eef6, 0xcfc6d6, 0x463e50),
    theme("koipond", true, 0x0e4646, 0x0a2a2c, 0xe8f5f1, 0xb8d6cf, 0x3b5757),
    theme("fireflies", true, 0x091b28, 0x0c1a1c, 0xeef5df, 0xc3d4ae, 0x3e4a47),
    theme("rainglass", true, 0x0b101f, 0x101626, 0xe9eef8, 0xbcc6da, 0x404654),
    theme("starrynight", true, 0x122c66, 0x0e1a3e, 0xeef1fa, 0xc2cbe2, 0x3f4967),
    theme("cubism", true, 0x352c23, 0x26201a, 0xf3ead8, 0xd2c6ae, 0x534c44),
    theme("goldleaf", true, 0x191109, 0x20160a, 0xf7ecd0, 0xd8c89e, 0x4f4536),
    theme("flametree", false, 0xb3def6, 0xf0f9fd, 0x2a1612, 0x5a3a30, 0xe5dbd9),
    theme("halong", false, 0xcde8f0, 0xf3f9fa, 0x102a26, 0x2f4c47, 0xd9e0df),
    theme("terraces", false, 0xfae0bb, 0xfdf5e8, 0x262a10, 0x4b5230, 0xe1e0d9),
    theme("fuji", false, 0xf9dcdf, 0xfdf4f4, 0x2a1a22, 0x5a404a, 0xe1dbdd),
    theme("skyislands", false, 0xabd8f6, 0xe9f5fc, 0x0f2a3a, 0x2e4d5e, 0xd9e0e5),
    theme("gray", false, 0xe9eaec, 0xf4f4f5, 0x18191c, 0x3c3e44, 0xd2d2d4),
    theme("midnight", true, 0x0b1020, 0x111833, 0xe6ebfa, 0xb3bedc, 0x38405c),
    theme("contrastlight", false, 0xffffff, 0xffffff, 0x000000, 0x1a1a1a, 0xd6d6d6),
    theme("indigo", true, 0x12122a, 0x1a1a3a, 0xececfb, 0xc0c0e6, 0x424261),
    theme("crimson", true, 0x1c0b10, 0x261018, 0xfbeaf0, 0xe2bcc8, 0x513941),
    theme("teal", true, 0x071a1a, 0x0c2626, 0xe2f7f4, 0xa8d4cf, 0x34504f),
    theme("amber", true, 0x1a1406, 0x241b0a, 0xfbf2e0, 0xe2cfa8, 0x504632),
    theme("sky", false, 0xeef6fc, 0xfafdff, 0x10243a, 0x34506a, 0xd5dee6),
    theme("lavender", false, 0xf5f2fc, 0xfdfcff, 0x241a38, 0x4e4068, 0xded8e6),
    theme("peach", false, 0xfdf3ec, 0xfffaf6, 0x36200f, 0x64422a, 0xe6dad2),
    theme("mint", false, 0xeffaf5, 0xfbfffd, 0x0f2a1e, 0x2e5442, 0xd6e3dd),
    theme("notebook", false, 0xfdfcf6, 0xfffffb, 0x1f2a44, 0x3e4a66, 0xdedcd2),
    theme("chalkboard", true, 0x2a3d34, 0x2f443a, 0xf4f4ee, 0xdcdcd2, 0x6e7e74),
    theme("brutal", false, 0xfff6dc, 0xffffff, 0x000000, 0x1a1a1a, 0x000000),
    theme("arcade", true, 0x0a0a12, 0x14142a, 0xffffff, 0xd8d8f0, 0x3a3a60),
    theme("cyberpunk", true, 0x0b0b10, 0x15151d, 0xf2f2f8, 0xc8c8d6, 0x3a3a4a),
    theme("macclassic", false, 0x808080, 0xffffff, 0x000000, 0x000000, 0x000000),
    theme("macos", false, 0xf5f5f7, 0xffffff, 0x1d1d1f, 0x3c3c43, 0xd8d8dc),
    theme("macosdark", true, 0x1e1e1e, 0x2c2c2e, 0xf5f5f7, 0xd1d1d6, 0x48484a),
    theme("gnome", false, 0xfafafa, 0xffffff, 0x2e2e2e, 0x464646, 0xdedede),
    theme("ubuntu", false, 0xfafafa, 0xffffff, 0x3d3d3d, 0x4f4f4f, 0xd6d6d6),
    theme("kde", true, 0x202326, 0x292c30, 0xfcfcfc, 0xd6d8da, 0x4a4f54),
];

/// Follows the Windows "app mode" (light or dark).
pub const FOLLOW_WINDOWS: &str = "system";
pub const DEFAULT: &str = "dark";
/// The person's own colours and picture (`prefs::State::custom`); the page works out the rest (app.js customVars).
pub const CUSTOM: &str = "custom";

/// A known theme id (or `FOLLOW_WINDOWS`); anything else falls back to the default.
pub fn normalize(id: Option<&str>) -> String {
    match id {
        Some(id) if id == FOLLOW_WINDOWS || id == CUSTOM || ALL.iter().any(|t| t.id == id) => id.to_string(),
        _ => DEFAULT.to_string(),
    }
}

/// The theme in use: the chosen one, or for `FOLLOW_WINDOWS` light or dark like Windows apps.
pub fn current() -> Theme {
    let (chosen, custom) = {
        let s = prefs::lock();
        (s.theme.clone(), s.custom.clone())
    };
    let id = if chosen == FOLLOW_WINDOWS { (if windows_uses_light() { "light" } else { "dark" }).to_string() } else { chosen };
    if id == CUSTOM {
        return from_custom(&custom);
    }
    ALL.iter().find(|t| t.id == id).or_else(|| ALL.iter().find(|t| t.id == DEFAULT)).cloned().expect("the default theme")
}

/// The custom theme for native parts (frame, title bar, mini window): dark or light by its colour as the page
/// decides, surfaces a step from it, text colours from Dark or Light.
fn from_custom(custom: &CustomTheme) -> Theme {
    // Convert.ToInt32(hex, 16); `prefs` only keeps a checked "#rrggbb".
    let bg = rgb(u32::from_str_radix(custom.background.get(1..).unwrap_or(""), 16).unwrap_or(0));
    let dark = is_dark(bg);
    let basis = ALL.iter().find(|t| t.id == if dark { "dark" } else { "light" }).expect("light and dark");
    Theme {
        id: CUSTOM,
        dark,
        background: bg,
        surface: mix(bg, Color::WHITE, if dark { 0.06 } else { 0.7 }),
        text: basis.text,
        text2: basis.text2,
        line: mix(bg, if dark { Color::WHITE } else { Color::BLACK }, 0.2),
    }
}

/// White words read better on it than black ones (relative luminance under 0.179); app.js decides the same way.
pub fn is_dark(c: Color) -> bool {
    fn linear(v: u8) -> f64 {
        let s = v as f64 / 255.0;
        if s <= 0.04045 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
    }
    0.2126 * linear(c.r) + 0.7152 * linear(c.g) + 0.0722 * linear(c.b) < 0.179
}

/// Math.Round: halves to the even neighbour.
fn mix(a: Color, b: Color, t: f64) -> Color {
    let one = |x: u8, y: u8| (x as f64 + (y as f64 - x as f64) * t).round_ties_even() as u8;
    Color { r: one(a.r, b.r), g: one(a.g, b.g), b: one(a.b, b.b) }
}

/// "#rrggbb" in lower case, or None for anything else.
pub fn hex(text: Option<&str>) -> Option<String> {
    let text = text?;
    if text.encode_utf16().count() == 7 && text.starts_with('#') && text[1..].chars().all(|c| c.is_ascii_hexdigit()) { Some(text.to_lowercase()) } else { None }
}

// .NET's `$` also matches before a final "\n": `\n?\z` here.
static VAULT_PICTURE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^vault-picture-[0-9a-f]{8}-\d{1,20}\.(png|jpe?g|webp|gif|bmp)\n?\z").unwrap());
static THEME_MEDIA: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^theme-background-\d{1,20}\.(png|jpe?g|webp|gif|bmp|mp4|webm)\n?\z").unwrap());

/// The only files the page may load from the pictures folder: the custom theme's (theme-background-<time>.png…)
/// and each vault's (vault-picture-<id>-<time>.png…).
pub fn is_picture_name(name: &str) -> bool {
    is_theme_media_name(name) || VAULT_PICTURE.is_match(name)
}

/// The custom theme's picture or video in the data folder, by its own kind of name.
pub fn is_theme_media_name(name: &str) -> bool {
    THEME_MEDIA.is_match(name)
}

/// A video, not a picture, behind the page.
pub fn is_video(name: Option<&str>) -> bool {
    let Some(name) = name else { return false };
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".mp4") || lower.ends_with(".webm")
}

/// How many custom themes are kept by name, and the most the picture is blurred (px).
pub const MAX_SAVED: usize = 30;
pub const MAX_BLUR: i32 = 30;

/// A saved theme's name: trimmed, without control characters, 1 to 40 characters; None when there is none.
pub fn theme_name(text: Option<&str>) -> Option<String> {
    let cleaned: String = text.unwrap_or("").chars().filter(|ch| !ch.is_control()).collect();
    let name = cleaned.trim();
    let units: Vec<u16> = name.encode_utf16().collect();
    if units.is_empty() {
        None
    } else if units.len() > 40 {
        Some(String::from_utf16_lossy(&units[..40]).trim_end().to_string())
    } else {
        Some(name.to_string())
    }
}

fn windows_uses_light() -> bool {
    use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
    let subkey: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize\0".encode_utf16().collect();
    let value: Vec<u16> = "AppsUseLightTheme\0".encode_utf16().collect();
    let mut data: u32 = 0;
    let mut size: u32 = std::mem::size_of::<u32>() as u32;
    // SAFETY: NUL-terminated UTF-16 names that live across the call; `data` and `size` are a 4-byte buffer and its size.
    let status = unsafe {
        RegGetValueW(HKEY_CURRENT_USER, subkey.as_ptr(), value.as_ptr(), RRF_RT_REG_DWORD, std::ptr::null_mut(), (&mut data as *mut u32).cast(), &mut size)
    };
    // Missing, not a number, or not allowed: Windows' dark.
    status == 0 && data != 0
}
