// Text in a window's picture, read by Windows' own text recognition (Windows.Media.Ocr: offline, built into Windows 10
// and 11), in English and, when Windows has its language pack, Vietnamese. The C# called the WinRT objects' COM tables
// by hand (to spare the 57 MB .NET projection); here the `windows` crate's projection makes the same calls (the same
// interfaces, the same order). Runs on a worker thread.
use std::cell::Cell;
use std::sync::Mutex;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::{Duration, Instant};

use windows::Globalization::Language;
use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
use windows::Media::Ocr::{OcrEngine, OcrResult};
use windows::Security::Cryptography::CryptographicBuffer;
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize};
use windows::core::HSTRING;

use crate::core::base::{Fail, Res};
use crate::core::win_kit::{Picture as Bitmap, Rect as Rectangle};
use crate::core::log;
use crate::core::macro_model::net;
use crate::core::macro_vision::{gdip, intersect};

/// System.Drawing.RectangleF: a box in fractional pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RectangleF {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// A word read, and where (pixels of the picture given).
#[derive(Clone, Debug)]
pub struct Word {
    pub text: String,
    pub r#box: RectangleF,
}

/// A line read: its words in order.
#[derive(Clone, Debug)]
pub struct Line {
    pub words: Vec<Word>,
}

impl Line {
    pub fn text(&self) -> String {
        self.words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" ")
    }
}

const ASYNC_STARTED: i32 = 0;
const ASYNC_COMPLETED: i32 = 1;
/// OcrEngine.MaxImageDimension on Windows 10 and 11.
const MAX_SIDE: i32 = 2600;

thread_local! {
    static INITIALISED: Cell<bool> = const { Cell::new(false) };
}

/// (tried, the engines made).
static ENGINES: Mutex<(bool, Option<Vec<OcrEngine>>)> = Mutex::new((false, None));
/// error.log: the last unfinished recognition status written.
static LOGGED_STATUS: AtomicI32 = AtomicI32::new(ASYNC_COMPLETED);

fn com(e: windows::core::Error) -> Fail {
    Fail::Other(format!("{} (0x{:08X})", e.message(), e.code().0))
}

/// RectangleF.Union.
fn union(a: RectangleF, b: RectangleF) -> RectangleF {
    let x1 = a.x.min(b.x);
    let x2 = (a.x + a.width).max(b.x + b.width);
    let y1 = a.y.min(b.y);
    let y2 = (a.y + a.height).max(b.y + b.height);
    RectangleF { x: x1, y: y1, width: x2 - x1, height: y2 - y1 }
}

/// RectangleF.Empty.
fn empty() -> RectangleF {
    RectangleF { x: 0.0, y: 0.0, width: 0.0, height: 0.0 }
}

/// RectangleF.IsEmpty: no width or no height.
fn is_empty(r: &RectangleF) -> bool {
    r.width <= 0.0 || r.height <= 0.0
}

/// Windows can read text here (some editions lack every recognition language).
pub fn available() -> bool {
    match engines() {
        Ok(e) => !e.is_empty(),
        Err(_) => {
            log::note("Macro text reading: Windows' text recognition didn't start (COMException).");
            false
        }
    }
}

/// The lines read in `area` of `picture` by each language's engine, boxes in the
/// picture's pixels; None when Windows has no text recognition. Small text is enlarged first (it reads better).
pub fn read(picture: &Bitmap, area: Rectangle) -> Res<Option<Vec<Vec<Line>>>> {
    let area = intersect(area, Rectangle { x: 0, y: 0, width: picture.width, height: picture.height });
    if area.width < 8 || area.height < 8 {
        return Ok(Some(Vec::new()));
    }
    let engines = engines()?;
    if engines.is_empty() {
        return Ok(None);
    }
    let longest = area.width.max(area.height) as f64;
    let mut scale = 1f64.max(1200.0 / longest).min(3.0);
    scale = scale.min(MAX_SIDE as f64 / longest);
    let w = 8.max((area.width as f64 * scale) as i32);
    let h = 8.max((area.height as f64 * scale) as i32);
    let cut = gdip::draw_scaled(picture, area, w, h).map_err(|status| Fail::Other(format!("GDI+ couldn't draw the picture (status {status}).")))?;
    let bytes = pixels(&cut);
    let mut result = Vec::new();
    let s = scale as f32;
    for engine in &engines {
        let lines = recognize(engine, &bytes, w, h)?;
        // Back to the picture's pixels.
        result.push(
            lines
                .into_iter()
                .map(|l| Line {
                    words: l
                        .words
                        .into_iter()
                        .map(|word| Word {
                            r#box: RectangleF {
                                x: area.x as f32 + word.r#box.x / s,
                                y: area.y as f32 + word.r#box.y / s,
                                width: word.r#box.width / s,
                                height: word.r#box.height / s,
                            },
                            text: word.text,
                        })
                        .collect(),
                })
                .collect(),
        );
    }
    Ok(Some(result))
}

/// Where `text` is (case and spacing aside) in what was read: the box around its words, or an empty
/// box when it runs over several lines. None when it isn't there.
pub fn locate(lines: &[Line], text: &str) -> Option<RectangleF> {
    let want = net::fold(&squeeze(text));
    if want.is_empty() {
        return None;
    }
    for line in lines {
        // Each word's place in the line's text, to box only the words the text covers.
        let mut spans: Vec<(usize, usize, RectangleF)> = Vec::new();
        let mut joined: Vec<char> = Vec::new();
        for word in &line.words {
            if !joined.is_empty() {
                joined.push(' ');
            }
            let start = joined.len();
            joined.extend(net::fold(&squeeze(&word.text)));
            spans.push((start, joined.len(), word.r#box));
        }
        let Some(at) = net::index_of_ci(&joined, &want) else {
            continue;
        };
        let boxes: Vec<RectangleF> = spans.iter().filter(|s| s.1 > at && s.0 < at + want.len()).map(|s| s.2).collect();
        return Some(boxes.into_iter().reduce(union).unwrap_or_else(empty));
    }
    let all = lines.iter().map(Line::text).collect::<Vec<_>>().join(" ");
    if net::contains_ci(&squeeze(&all), &squeeze(text)) { Some(empty()) } else { None }
}

/// Which of `words` is in what was read, and where: line by line, case and spacing aside, a letter
/// read wrong forgiven in longer words (two from 10 letters; game fonts). `all` is the whole list the
/// words were ticked from: a longer name read ("Blessed Song") is not a shorter one in it ("Blessed").
pub fn locate_any(lines: &[Line], words: &[String], all: &[String]) -> Option<(RectangleF, String)> {
    let mut names: Vec<String> = Vec::new();
    for n in all.iter().chain(words).map(|w| squeeze(w)).filter(|w| !w.is_empty()) {
        if !names.iter().any(|o| net::eq_ci(o, &n)) {
            names.push(n);
        }
    }
    for line in lines {
        let text = squeeze(&line.text());
        let hits: Vec<&String> = names.iter().filter(|n| near(&text, n)).collect();
        let kept: Vec<&String> = hits.iter().copied().filter(|h| !hits.iter().any(|o| net::len16(o) > net::len16(h) && net::contains_ci(o, h))).collect();
        for hit in kept {
            if let Some(word) = words.iter().find(|w| net::eq_ci(&squeeze(w), hit)) {
                let found = locate(std::slice::from_ref(line), word).filter(|b| !is_empty(b));
                let r#box = found.unwrap_or_else(|| line.words.iter().map(|w| w.r#box).reduce(union).unwrap_or_else(empty));
                return Some((r#box, word.clone()));
            }
        }
    }
    None
}

/// The word somewhere in the text, with at most one letter wrong from 5 letters, two from 10.
pub fn near(text: &str, word: &str) -> bool {
    if net::contains_ci(text, word) {
        return true;
    }
    let length = net::len16(word);
    let allowed = if length >= 10 {
        2
    } else if length >= 5 {
        1
    } else {
        0
    };
    if allowed == 0 {
        return false;
    }
    // The fewest changes that make the word match some part of the text (it may start anywhere).
    let t: Vec<char> = text.to_lowercase().chars().collect();
    let w: Vec<char> = word.to_lowercase().chars().collect();
    let mut prev: Vec<usize> = (0..=w.len()).collect();
    let mut cur = vec![0usize; w.len() + 1];
    for &c in &t {
        cur[0] = 0;
        for j in 1..=w.len() {
            cur[j] = (prev[j - 1] + if c == w[j - 1] { 0 } else { 1 }).min(prev[j] + 1).min(cur[j - 1] + 1);
        }
        if cur[w.len()] <= allowed {
            return true;
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    false
}

/// Runs of spaces as one, ends trimmed.
pub fn squeeze(text: &str) -> String {
    text.split(char::is_whitespace).filter(|p| !p.is_empty()).collect::<Vec<_>>().join(" ")
}

/// The picture's bytes, B G R A a pixel, rows top to bottom.
fn pixels(bitmap: &Bitmap) -> Vec<u8> {
    let (w, h) = (bitmap.width, bitmap.height);
    let mut bytes = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let argb = bitmap.get(x, y);
            bytes.extend_from_slice(&[(argb & 0xFF) as u8, (argb >> 8 & 0xFF) as u8, (argb >> 16 & 0xFF) as u8, 255]); // a window's picture has no transparency; its alpha byte may be anything
        }
    }
    bytes
}

fn initialise() {
    if INITIALISED.with(Cell::get) {
        return;
    }
    // The worker joins the multithreaded apartment (S_FALSE: it already had; a changed mode is fine too).
    // SAFETY: RoInitialize on this thread; its result is ignored as the C# ignored it.
    let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
    INITIALISED.with(|i| i.set(true));
}

/// English first, then Vietnamese; the person's own languages when neither is installed. Made once.
fn engines() -> Res<Vec<OcrEngine>> {
    let mut state = ENGINES.lock().unwrap_or_else(|e| e.into_inner());
    if state.0 {
        return Ok(state.1.clone().unwrap_or_default());
    }
    state.0 = true;
    initialise();
    // The OcrEngine statics (RoGetActivationFactory): a failure here is the C#'s Factory throwing.
    OcrEngine::MaxImageDimension().map_err(com)?;
    let mut engines = Vec::new();
    for tag in ["en-US", "vi"] {
        let Some(language) = language(tag) else {
            continue;
        };
        if let Ok(engine) = OcrEngine::TryCreateFromLanguage(&language) {
            engines.push(engine);
        }
    }
    if engines.is_empty() {
        if let Ok(engine) = OcrEngine::TryCreateFromUserProfileLanguages() {
            engines.push(engine);
        }
    }
    state.1 = Some(engines.clone());
    log::note(&format!("Macro text reading: {} recognition language(s) ready.", engines.len()));
    Ok(engines)
}

fn language(tag: &str) -> Option<Language> {
    // PORT: the projection makes the factory and the language in one call, so a factory that can't be made is
    // skipped like a language that can't (the C# threw there).
    Language::CreateLanguage(&HSTRING::from(tag)).ok()
}

fn recognize(engine: &OcrEngine, bgra: &[u8], w: i32, h: i32) -> Res<Vec<Line>> {
    initialise();
    let buffer = CryptographicBuffer::CreateFromByteArray(bgra).map_err(com)?;
    let bitmap = SoftwareBitmap::CreateCopyFromBuffer(&buffer, BitmapPixelFormat::Bgra8, w, h).map_err(com)?;
    let operation = engine.RecognizeAsync(&bitmap).map_err(com)?;
    let waited = Instant::now();
    let result = (|| -> Res<Vec<Line>> {
        let mut status;
        loop {
            status = operation.Status().map_err(com)?.0;
            if status != ASYNC_STARTED || waited.elapsed() > Duration::from_secs(10) {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        if status != ASYNC_COMPLETED {
            // Once per status: a read every second would repeat it.
            if LOGGED_STATUS.swap(status, Ordering::SeqCst) != status {
                log::note(&format!(
                    "Macro text reading: Windows' recognition ended with status {status} after {} ms; nothing read.",
                    waited.elapsed().as_millis()
                ));
            }
            let _ = operation.Cancel();
            return Ok(Vec::new());
        }
        let result = operation.GetResults().map_err(com)?;
        lines(&result)
    })();
    let _ = operation.Close();
    result
}

fn lines(result: &OcrResult) -> Res<Vec<Line>> {
    let mut lines = Vec::new();
    let view = result.Lines().map_err(com)?;
    for i in 0..view.Size().map_err(com)? {
        let line = view.GetAt(i).map_err(com)?;
        let mut words = Vec::new();
        let items = line.Words().map_err(com)?;
        for j in 0..items.Size().map_err(com)? {
            let word = items.GetAt(j).map_err(com)?;
            let r = word.BoundingRect().map_err(com)?;
            words.push(Word { text: word.Text().map_err(com)?.to_string_lossy(), r#box: RectangleF { x: r.X, y: r.Y, width: r.Width, height: r.Height } });
        }
        lines.push(Line { words });
    }
    Ok(lines)
}
