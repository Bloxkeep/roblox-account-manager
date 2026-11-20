// SummaryCard.cs: a play-time summary's picture for Discord. The C# drew it with GDI+ (System.Drawing); here it is drawn
// off-screen with tiny-skia (shapes, clips, the avatars) and fontdue (text, from Windows' own Segoe UI files), at the same
// sizes, colours and layout, and written out as PNG.
use std::cell::OnceCell;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use fontdue::{Font, FontSettings};
use tiny_skia::{
    Color, FillRule, FilterQuality, Mask, Paint, PathBuilder, Pixmap, PixmapPaint, PremultipliedColorU8, Rect, Stroke, Transform,
};

use crate::core::base::{Cancel, Fail, Res};

/// What a play-time summary's picture shows (drawn by `render`); every text in Discord's language.
#[derive(Clone, Debug, Default)]
pub struct SummaryCardData {
    /// "Activity by hour" / "by day", with the period's total on the right.
    pub chart_title: String,
    pub total_text: String,
    /// One column per hour (or day); each part is a game (its place in `games`, -1 for the rest) and its seconds.
    pub buckets: Vec<SummaryCardBucket>,
    pub accounts_title: String,
    pub accounts: Vec<SummaryCardAccount>,
    /// "+3 more accounts", under the rows, when not all fit.
    pub more_accounts: Option<String>,
    /// The games the colours stand for, most played first.
    pub games: Vec<(String, Option<String>)>,
    pub other_games: String,
    /// Said in place of the chart when nothing was played.
    pub empty: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct SummaryCardBucket {
    pub label: String,
    pub parts: Vec<(i32, f64)>,
}

impl SummaryCardBucket {
    pub fn new(label: String, parts: Vec<(i32, f64)>) -> SummaryCardBucket {
        SummaryCardBucket { label, parts }
    }
}

#[derive(Clone, Debug, Default)]
pub struct SummaryCardAccount {
    pub name: String,
    /// A second, quieter line: its stays and drops.
    pub detail: String,
    pub time: String,
    pub seconds: f64,
    pub parts: Vec<(i32, f64)>,
    /// Roblox's picture of the account (rbxcdn.com only); its initials when there is none or it can't be fetched.
    pub avatar_url: Option<String>,
    pub initials: String,
    pub hue: i32,
}

/// An opaque colour (System.Drawing.Color as the card uses it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

const fn rgb(r: u8, g: u8, b: u8) -> Rgb {
    Rgb { r, g, b }
}

impl Rgb {
    fn color(self) -> Color {
        Color::from_rgba8(self.r, self.g, self.b, 255)
    }
}

// Discord shows it about 450 px wide: drawn at twice that, with text sized to read there.
const WIDTH: i32 = 1000;
const PAD: i32 = 40;
const CHART_HEIGHT: i32 = 230;
const ROW_HEIGHT: i32 = 86;
const AVATAR: i32 = 60;
const LEGEND_ROW_HEIGHT: i32 = 56;

pub(crate) static AVATARS: LazyLock<Mutex<HashMap<String, Vec<u8>>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// The games' colours, in order: distinct on Discord's dark and light themes.
pub const GAME_COLORS: [Rgb; 6] =
    [rgb(0x58, 0x65, 0xF2), rgb(0x3B, 0xA5, 0x5D), rgb(0xF0, 0xB2, 0x32), rgb(0xEB, 0x45, 0x9E), rgb(0x00, 0xA8, 0xE8), rgb(0xE6, 0x7E, 0x22)];
const OTHER: Rgb = rgb(0x6D, 0x6F, 0x78);
const BACK: Rgb = rgb(0x1E, 0x1F, 0x22);
const PANEL: Rgb = rgb(0x2B, 0x2D, 0x31);
const INK: Rgb = rgb(0xF2, 0xF3, 0xF5);
const INK2: Rgb = rgb(0xB5, 0xBA, 0xC1);
const INK3: Rgb = rgb(0x80, 0x84, 0x8E);
const GRID: Rgb = rgb(0x3A, 0x3C, 0x42);
const WHITE: Rgb = rgb(0xFF, 0xFF, 0xFF);

pub fn color_of(game: i32) -> Rgb {
    if game >= 0 && (game as usize) < GAME_COLORS.len() { GAME_COLORS[game as usize] } else { OTHER }
}

pub fn render(data: &SummaryCardData, ct: &Cancel) -> Res<Vec<u8>> {
    let mut pictures: HashMap<String, Pixmap> = HashMap::new();
    let mut urls: Vec<&String> = Vec::new();
    let avatars = data.accounts.iter().filter_map(|a| a.avatar_url.as_ref());
    let icons = data.games.iter().take(GAME_COLORS.len()).filter_map(|(_, icon)| icon.as_ref());
    for url in avatars.chain(icons) {
        if !urls.contains(&url) {
            urls.push(url);
        }
    }
    for url in urls {
        if let Some(bytes) = load_avatar(url, ct) {
            // Not a picture the decoder reads: initials instead.
            if let Some(picture) = decode(&bytes) {
                pictures.insert(url.clone(), picture);
            }
        }
    }
    ct.check()?;
    draw(data, &pictures)
}

/// An avatar's picture from Roblox's CDN (rbxcdn.com only), kept for the next time; None when it can't be had.
pub(crate) fn load_avatar(url: &str, ct: &Cancel) -> Option<Vec<u8>> {
    if let Some(cached) = AVATARS.lock().unwrap_or_else(|e| e.into_inner()).get(url) {
        return Some(cached.clone());
    }
    let host_ok = url
        .strip_prefix("https://")
        .map(|rest| rest.split(['/', '?', '#']).next().unwrap_or(""))
        // A user part ("a.rbxcdn.com:x@other.host") is refused before the port is cut, or the real host hides behind it.
        .filter(|authority| !authority.contains('@'))
        .map(|authority| authority.split(':').next().unwrap_or("").to_ascii_lowercase())
        .is_some_and(|host| host.ends_with(".rbxcdn.com"));
    // A backslash reads as a slash to the web parser ("https://other.host\x.rbxcdn.com" goes to other.host).
    if !host_ok || url.contains('\\') || url.chars().any(char::is_whitespace) {
        return None;
    }
    if ct.is_cancelled() {
        return None;
    }
    // No redirects: only the rbxcdn.com host checked above is ever asked.
    let agent = crate::net::builder().redirects(0).timeout(Duration::from_secs(6)).build();
    let response = agent.get(url).call().ok()?;
    let mut bytes = Vec::new();
    use std::io::Read;
    // One byte past the limit is enough to know it is too big.
    response.into_reader().take(2_000_001).read_to_end(&mut bytes).ok()?;
    if bytes.len() > 2_000_000 {
        return None;
    }
    let mut avatars = AVATARS.lock().unwrap_or_else(|e| e.into_inner());
    if avatars.len() > 200 {
        avatars.clear();
    }
    avatars.insert(url.to_string(), bytes.clone());
    Some(bytes)
}

fn decode(bytes: &[u8]) -> Option<Pixmap> {
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    let mut pixmap = Pixmap::new(w, h)?;
    for (dst, src) in pixmap.pixels_mut().iter_mut().zip(img.pixels()) {
        *dst = tiny_skia::ColorU8::from_rgba(src[0], src[1], src[2], src[3]).premultiply();
    }
    Some(pixmap)
}

// ───────────────────────────── text (GDI+'s DrawString / MeasureString) ─────────────────────────────

// The fonts are read for each card and let go after it: fontdue turns every letter of a font into shapes as it reads
// it, and kept for good they held 400 MB after one summary (the owner's error.log, 2026-11-20: Segoe UI 17 MB, its
// Semibold 16, Symbol 43, Emoji 56, Microsoft YaHei 142, Malgun Gothic 119, Arial 12). A fallback font is read only
// when the card has a letter Segoe UI lacks and that font has (asked of the file itself, without reading it whole).
struct Fonts {
    regular: Font,
    semibold: Font,
    /// Where a character Segoe UI lacks is looked for (GDI+ falls back to other fonts by itself), in this order.
    fallback: Vec<Fallback>,
}

struct Fallback {
    path: String,
    bytes: OnceCell<Option<Vec<u8>>>,
    font: OnceCell<Option<Font>>,
}

impl Fallback {
    /// The font, when it has `c`: read the first time it is needed.
    fn with(&self, c: char) -> Option<&Font> {
        let bytes = self.bytes.get_or_init(|| std::fs::read(&self.path).ok()).as_ref()?;
        ttf_parser::Face::parse(bytes, 0).ok()?.glyph_index(c)?;
        self.font.get_or_init(|| Font::from_bytes(&bytes[..], FontSettings::default()).ok()).as_ref()
    }
}

fn load_fonts() -> Result<Fonts, String> {
    let dir = std::env::var("WINDIR").map(|w| format!("{w}\\Fonts")).unwrap_or_else(|_| "C:\\Windows\\Fonts".into());
    let load = |file: &str| -> Result<Font, String> {
        let bytes = std::fs::read(format!("{dir}\\{file}")).map_err(|e| format!("{file}: {e}"))?;
        Font::from_bytes(bytes, FontSettings::default()).map_err(|e| format!("{file}: {e}"))
    };
    let regular = load("segoeui.ttf")?;
    // "Segoe UI Semibold" is its own file; without it the regular face stands in (GDI+ would do the same).
    let semibold = load("seguisb.ttf").or_else(|_| load("segoeui.ttf"))?;
    let fallback = ["seguisym.ttf", "seguiemj.ttf", "msyh.ttc", "malgun.ttf", "arial.ttf"]
        .iter()
        .map(|f| Fallback { path: format!("{dir}\\{f}"), bytes: OnceCell::new(), font: OnceCell::new() })
        .collect();
    Ok(Fonts { regular, semibold, fallback })
}

#[derive(Clone, Copy)]
struct TextFont {
    semibold: bool,
    /// Pixels (GraphicsUnit.Pixel): the em size.
    size: f32,
}

const fn regular(size: f32) -> TextFont {
    TextFont { semibold: false, size }
}

const fn semibold(size: f32) -> TextFont {
    TextFont { semibold: true, size }
}

fn fonts() -> Res<Fonts> {
    load_fonts().map_err(|e| Fail::other(format!("Summary card fonts: {e}")))
}

fn face<'a>(fonts: &'a Fonts, f: TextFont, c: char) -> &'a Font {
    let primary = if f.semibold { &fonts.semibold } else { &fonts.regular };
    if primary.lookup_glyph_index(c) != 0 || c.is_whitespace() {
        return primary;
    }
    fonts.fallback.iter().find_map(|f| f.with(c)).unwrap_or(primary)
}

/// The line's height as GDI+ measures it (Segoe UI's ascent + descent + gap).
fn line_height(fonts: &Fonts, f: TextFont) -> f32 {
    let primary = if f.semibold { &fonts.semibold } else { &fonts.regular };
    primary.horizontal_line_metrics(f.size).map(|m| m.ascent - m.descent + m.line_gap).unwrap_or(f.size * 1.33)
}

fn ascent(fonts: &Fonts, f: TextFont) -> f32 {
    let primary = if f.semibold { &fonts.semibold } else { &fonts.regular };
    primary.horizontal_line_metrics(f.size).map(|m| m.ascent).unwrap_or(f.size * 1.08)
}

/// The pen's advance over `text` (no padding).
fn advance(fonts: &Fonts, f: TextFont, text: &str) -> f32 {
    let mut width = 0.0;
    let mut prev: Option<char> = None;
    for c in text.chars() {
        let font = face(fonts, f, c);
        if let Some(p) = prev {
            width += font.horizontal_kern(p, c, f.size).unwrap_or(0.0);
        }
        width += font.metrics(c, f.size).advance_width;
        prev = Some(c);
    }
    width
}

/// GDI+'s default layout pads each side by 1/12 em (MeasureString's width is the text's plus 1/6 em).
fn side_pad(f: TextFont) -> f32 {
    f.size / 12.0
}

/// MeasureString: (width, height).
fn measure(fonts: &Fonts, f: TextFont, text: &str) -> (f32, f32) {
    // PORT: GDI+ measures with its own layout (hinted with AntiAliasGridFit); fontdue's advances are unhinted, so widths
    // can differ by a pixel or two.
    (advance(fonts, f, text) + 2.0 * side_pad(f), line_height(fonts, f))
}

/// DrawString at (x, y): the layout box's top left, as GDI+ places it.
fn draw_text(pixmap: &mut Pixmap, fonts: &Fonts, f: TextFont, text: &str, color: Rgb, x: f32, y: f32) {
    let baseline = y + ascent(fonts, f);
    let mut pen = x + side_pad(f);
    let mut prev: Option<char> = None;
    for c in text.chars() {
        let font = face(fonts, f, c);
        if let Some(p) = prev {
            pen += font.horizontal_kern(p, c, f.size).unwrap_or(0.0);
        }
        let (metrics, coverage) = font.rasterize(c, f.size);
        let left = (pen + metrics.xmin as f32).round() as i32;
        let top = (baseline - metrics.ymin as f32 - metrics.height as f32).round() as i32;
        for row in 0..metrics.height {
            for col in 0..metrics.width {
                let a = coverage[row * metrics.width + col];
                if a > 0 {
                    blend(pixmap, left + col as i32, top + row as i32, color, a);
                }
            }
        }
        pen += metrics.advance_width;
        prev = Some(c);
    }
}

/// One pixel of `color` at `coverage` over what is there (source-over, premultiplied).
fn blend(pixmap: &mut Pixmap, x: i32, y: i32, color: Rgb, coverage: u8) {
    if x < 0 || y < 0 || x >= pixmap.width() as i32 || y >= pixmap.height() as i32 {
        return;
    }
    let width = pixmap.width() as usize;
    let px = &mut pixmap.pixels_mut()[y as usize * width + x as usize];
    let a = coverage as u32;
    let inv = 255 - a;
    let mix = |src: u8, dst: u8| ((src as u32 * a + dst as u32 * inv + 127) / 255) as u8;
    let out_a = ((a * 255 + px.alpha() as u32 * inv + 127) / 255) as u8;
    let (r, g, b) = (mix(color.r, px.red()), mix(color.g, px.green()), mix(color.b, px.blue()));
    if let Some(c) = PremultipliedColorU8::from_rgba(r.min(out_a), g.min(out_a), b.min(out_a), out_a) {
        *px = c;
    }
}

/// The text cut with "…" to fit the width.
fn fit(fonts: &Fonts, text: &str, f: TextFont, width: f32) -> String {
    if measure(fonts, f, text).0 <= width {
        return text.to_string();
    }
    let units: Vec<u16> = text.encode_utf16().collect();
    for length in (1..units.len()).rev() {
        let cut = format!("{}…", String::from_utf16_lossy(&units[..length]).trim_end());
        if measure(fonts, f, &cut).0 <= width {
            return cut;
        }
    }
    "…".to_string()
}

// ───────────────────────────── shapes ─────────────────────────────

fn paint(color: Rgb) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(color.color());
    p.anti_alias = true;
    p
}

/// A rectangle with round corners as GDI+'s four AddArc calls make it.
fn rounded(x: f32, y: f32, w: f32, h: f32, radius: f32) -> Option<tiny_skia::Path> {
    let d = (radius * 2.0).min(w.min(h)).max(1.0);
    let r = d / 2.0;
    // A quarter circle as one cubic Bézier.
    let k = r * 0.552_284_8;
    let (right, bottom) = (x + w, y + h);
    let mut pb = PathBuilder::new();
    pb.move_to(x, y + r);
    pb.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    pb.line_to(right - r, y);
    pb.cubic_to(right - r + k, y, right, y + r - k, right, y + r);
    pb.line_to(right, bottom - r);
    pb.cubic_to(right, bottom - r + k, right - r + k, bottom, right - r, bottom);
    pb.line_to(x + r, bottom);
    pb.cubic_to(x + r - k, bottom, x, bottom - r + k, x, bottom - r);
    pb.close();
    pb.finish()
}

fn fill_path(pixmap: &mut Pixmap, path: &Option<tiny_skia::Path>, color: Rgb, mask: Option<&Mask>) {
    if let Some(path) = path {
        pixmap.fill_path(path, &paint(color), FillRule::Winding, Transform::identity(), mask);
    }
}

fn fill_rect(pixmap: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, color: Rgb, mask: Option<&Mask>) {
    if let Some(rect) = Rect::from_xywh(x, y, w, h) {
        pixmap.fill_rect(rect, &paint(color), Transform::identity(), mask);
    }
}

fn line(pixmap: &mut Pixmap, x1: f32, y1: f32, x2: f32, y2: f32, color: Rgb, width: f32) {
    let mut pb = PathBuilder::new();
    pb.move_to(x1, y1);
    pb.line_to(x2, y2);
    if let Some(path) = pb.finish() {
        let stroke = Stroke { width, ..Stroke::default() };
        pixmap.stroke_path(&path, &paint(color), &stroke, Transform::identity(), None);
    }
}

/// A clip (Graphics.SetClip) of this path.
fn clip_of(pixmap: &Pixmap, path: &Option<tiny_skia::Path>) -> Option<Mask> {
    let mut mask = Mask::new(pixmap.width(), pixmap.height())?;
    mask.fill_path(path.as_ref()?, FillRule::Winding, true, Transform::identity());
    Some(mask)
}

fn rect_path(x: f32, y: f32, w: f32, h: f32) -> Option<tiny_skia::Path> {
    Rect::from_xywh(x, y, w, h).map(PathBuilder::from_rect)
}

// ───────────────────────────── the picture ─────────────────────────────

fn draw(data: &SummaryCardData, pictures: &HashMap<String, Pixmap>) -> Res<Vec<u8>> {
    let fonts = &fonts()?;
    let rows = data.accounts.len() as i32;
    let game_count = data.games.len().min(GAME_COLORS.len() + 1);
    let legend_rows = game_count.div_ceil(3) as i32;
    let height = PAD
        + 62
        + CHART_HEIGHT
        + 46
        + if rows > 0 { 72 + rows * ROW_HEIGHT + if data.more_accounts.is_none() { 0 } else { 44 } } else { 0 }
        + if legend_rows > 0 { 24 + legend_rows * LEGEND_ROW_HEIGHT } else { 0 }
        + PAD
        - 10;
    let mut pixmap = Pixmap::new(WIDTH as u32, height as u32).ok_or_else(|| Fail::other("Summary card: no bitmap"))?;
    // Clear(Color.Transparent): a new pixmap is transparent already.
    fill_path(&mut pixmap, &rounded(0.0, 0.0, WIDTH as f32, height as f32, 28.0), BACK, None);

    let title = semibold(32.0);
    let label = regular(26.0);
    let name = semibold(30.0);
    let small = regular(25.0);
    let big = semibold(34.0);
    let (w, pad) = (WIDTH as f32, PAD as f32);

    let mut y = pad;
    // Activity by hour (or day), the total on the right.
    draw_text(&mut pixmap, fonts, title, &data.chart_title, INK, pad, y);
    let total_size = measure(fonts, big, &data.total_text);
    draw_text(&mut pixmap, fonts, big, &data.total_text, INK, w - pad - total_size.0, y - 2.0);
    y += 62.0;
    draw_chart(&mut pixmap, fonts, (pad, y, w - 2.0 * pad, CHART_HEIGHT as f32), data, label);
    y += (CHART_HEIGHT + 46) as f32;

    if rows > 0 {
        line(&mut pixmap, pad, y, w - pad, y, GRID, 2.0);
        y += 18.0;
        draw_text(&mut pixmap, fonts, title, &data.accounts_title, INK, pad, y);
        y += 54.0;
        let most = data.accounts.iter().map(|a| a.seconds).fold(f64::MIN, f64::max).max(1.0);
        let avatar = AVATAR as f32;
        let (bar_left, bar_right) = (pad + avatar + 18.0 + 270.0, w - pad - 218.0);
        for account in &data.accounts {
            let mid = y + ROW_HEIGHT as f32 / 2.0;
            draw_avatar(&mut pixmap, fonts, account, pictures, (pad, mid - avatar / 2.0, avatar, avatar));
            let text_left = pad + avatar + 18.0;
            let text_width = bar_left - text_left - 16.0;
            draw_text(&mut pixmap, fonts, name, &fit(fonts, &account.name, name, text_width), INK, text_left, mid - 37.0);
            draw_text(&mut pixmap, fonts, small, &fit(fonts, &account.detail, small, text_width), INK3, text_left, mid + 3.0);
            // The bar: its length is the account's share of the most played one's, split by game.
            fill_path(&mut pixmap, &rounded(bar_left, mid - 11.0, bar_right - bar_left, 22.0, 11.0), PANEL, None);
            let full = ((bar_right - bar_left) as f64 * account.seconds / most) as f32;
            let mut x = bar_left;
            if full >= 2.0 {
                let clip = clip_of(&pixmap, &rounded(bar_left, mid - 11.0, full.max(22.0), 22.0, 11.0));
                let mut parts = account.parts.clone();
                parts.sort_by_key(|p| if p.0 < 0 { i32::MAX } else { p.0 });
                for (game, seconds) in parts {
                    let pw = (full as f64 * seconds / account.seconds.max(1.0)) as f32;
                    fill_rect(&mut pixmap, x, mid - 11.0, pw + 1.0, 22.0, color_of(game), clip.as_ref());
                    x += pw;
                }
            }
            let time_size = measure(fonts, name, &account.time);
            draw_text(&mut pixmap, fonts, name, &account.time, INK, w - pad - time_size.0, mid - time_size.1 / 2.0);
            y += ROW_HEIGHT as f32;
        }
        if let Some(more) = &data.more_accounts {
            draw_text(&mut pixmap, fonts, small, more, INK3, pad + avatar + 18.0, y + 4.0);
            y += 44.0;
        }
    }

    if legend_rows > 0 {
        y += 24.0;
        let mut items: Vec<_> = data.games.iter().take(GAME_COLORS.len()).map(|(game, icon)| (game.as_str(), icon.as_ref())).collect();
        if data.games.len() > GAME_COLORS.len() {
            items.push((&data.other_games, None));
        }
        let column = (w - 2.0 * pad) / 3.0;
        for (i, (game, icon)) in items.into_iter().enumerate() {
            let x = pad + (i % 3) as f32 * column;
            let top = y + (i / 3) as f32 * LEGEND_ROW_HEIGHT as f32;
            let color = color_of(i as i32);
            if let Some(picture) = icon.and_then(|url| pictures.get(url)) {
                let clip = clip_of(&pixmap, &rounded(x, top, 42.0, 42.0, 8.0));
                let transform = Transform::from_row(42.0 / picture.width() as f32, 0.0, 0.0, 42.0 / picture.height() as f32, x, top);
                let paint = PixmapPaint { quality: FilterQuality::Bicubic, ..PixmapPaint::default() };
                pixmap.draw_pixmap(0, 0, picture.as_ref(), &paint, transform, clip.as_ref());
                fill_path(&mut pixmap, &rounded(x + 30.0, top + 30.0, 14.0, 14.0, 4.0), color, None);
            } else {
                fill_path(&mut pixmap, &rounded(x + 10.0, top + 10.0, 22.0, 22.0, 5.0), color, None);
            }
            draw_text(&mut pixmap, fonts, label, &fit(fonts, game, label, column - 64.0), INK2, x + 52.0, top + 4.0);
        }
    }

    pixmap.encode_png().map_err(Fail::other)
}

fn draw_chart(pixmap: &mut Pixmap, fonts: &Fonts, area: (f32, f32, f32, f32), data: &SummaryCardData, label: TextFont) {
    let (left, area_top, width, height) = area;
    let right = left + width;
    let bottom = area_top + height - 38.0; // the labels go under the bars
    let top = area_top + 10.0;
    for i in 0..=2 {
        let line_y = top + (bottom - top) * i as f32 / 2.0;
        line(pixmap, left, line_y, right, line_y, GRID, 1.5);
    }
    if data.empty.is_some() || data.buckets.is_empty() {
        let text = data.empty.clone().unwrap_or_default();
        let size = measure(fonts, label, &text);
        draw_text(pixmap, fonts, label, &text, INK2, left + (width - size.0) / 2.0, top + (bottom - top - size.1) / 2.0);
        return;
    }
    let most = data.buckets.iter().map(|b| b.parts.iter().map(|p| p.1).sum::<f64>()).fold(f64::MIN, f64::max).max(1.0);
    let count = data.buckets.len();
    let slot = width / count as f32;
    let bar = (slot * if count > 12 { 0.68 } else { 0.56 }).max(4.0);
    let every = if count > 12 { 3 } else { 1 };
    for (i, bucket) in data.buckets.iter().enumerate() {
        let x = left + i as f32 * slot + (slot - bar) / 2.0;
        let mut y = bottom;
        let sum: f64 = bucket.parts.iter().map(|p| p.1).sum();
        if sum > 0.0 {
            let h = ((bottom - top) as f64 * sum / most) as f32;
            let mut clip = clip_of(pixmap, &rounded(x, bottom - h, bar, h + 6.0, 7f32.min(bar / 2.0)));
            if let (Some(mask), Some(rect)) = (clip.as_mut(), rect_path(x, top - 2.0, bar, bottom - top + 2.0)) {
                mask.intersect_path(&rect, FillRule::Winding, true, Transform::identity());
            }
            let mut parts = bucket.parts.clone();
            parts.sort_by_key(|p| if p.0 < 0 { i32::MAX } else { p.0 });
            for (game, seconds) in parts {
                let part = (h as f64 * seconds / sum) as f32;
                fill_rect(pixmap, x, y - part, bar, part + 1.0, color_of(game), clip.as_ref());
                y -= part;
            }
        }
        if i % every == 0 {
            let size = measure(fonts, label, &bucket.label);
            draw_text(pixmap, fonts, label, &bucket.label, INK3, x + bar / 2.0 - size.0 / 2.0, bottom + 6.0);
        }
    }
}

fn draw_avatar(pixmap: &mut Pixmap, fonts: &Fonts, account: &SummaryCardAccount, pictures: &HashMap<String, Pixmap>, b: (f32, f32, f32, f32)) {
    let (x, y, w, h) = b;
    let circle = Rect::from_xywh(x, y, w, h).and_then(PathBuilder::from_oval);
    if let Some(picture) = account.avatar_url.as_ref().and_then(|u| pictures.get(u)) {
        let clip = clip_of(pixmap, &circle);
        fill_path(pixmap, &circle, PANEL, clip.as_ref());
        let transform = Transform::from_row(w / picture.width() as f32, 0.0, 0.0, h / picture.height() as f32, x, y);
        let paint = PixmapPaint { quality: FilterQuality::Bicubic, ..PixmapPaint::default() };
        pixmap.draw_pixmap(0, 0, picture.as_ref(), &paint, transform, clip.as_ref());
        return;
    }
    fill_path(pixmap, &circle, from_hue(account.hue), None);
    let font = semibold(20.0);
    let size = measure(fonts, font, &account.initials);
    draw_text(pixmap, fonts, font, &account.initials, WHITE, x + (w - size.0) / 2.0, y + (h - size.1) / 2.0);
}

/// The page's initials colour (hsl(h 38% 40%)).
fn from_hue(hue: i32) -> Rgb {
    // C#'s % keeps the sign of the dividend.
    let h = (hue % 360) as f64 / 60.0;
    let c = (1.0 - (2.0 * 0.40 - 1.0f64).abs()) * 0.38;
    let x_part = c * (1.0 - (h % 2.0 - 1.0).abs());
    let m = 0.40 - c / 2.0;
    let (r, g, b) = match h as i32 {
        0 => (c, x_part, 0.0),
        1 => (x_part, c, 0.0),
        2 => (0.0, c, x_part),
        3 => (0.0, x_part, c),
        4 => (x_part, 0.0, c),
        _ => (c, 0.0, x_part),
    };
    rgb(((r + m) * 255.0) as u8, ((g + m) * 255.0) as u8, ((b + m) * 255.0) as u8)
}
