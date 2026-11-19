// A piece of game news drawn the way Discord drew it (ui/src/DiscordBlocks.tsx; the bot's feeds.js cardsOf makes the
// blocks): Discord's markdown, fields, a section with a picture or a button beside it, a gallery, a file, a line between,
// a row of buttons, a footer, a poll, a sticker. A kind Discord adds later shows its words. Nothing here is HTML from
// outside: every piece becomes GPUI's own elements, pictures come only from https (Discord's servers), and a link opens
// through the app (feedOpen), which checks it is the news's own.

use crate::threads::Hold;
use std::collections::HashSet;
use std::ops::Range;
use std::sync::{LazyLock, Mutex};

use gpui_kit::{prelude::*, *};
use regex::Regex;
use serde_json::{Value, json};

use crate::{images, kit::*, shell::Root, theme::*};
use crate::val::s;

/// Spoilers shown (clicked) this run: "card:place".
static SHOWN: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

fn shown(key: &str) -> bool {
    SHOWN.hold().contains(key)
}

fn show(key: String) {
    SHOWN.hold().insert(key);
    SHOWN_N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

/// Counted up by each spoiler shown: the lines worked out before are worked out again.
static SHOWN_N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

thread_local! {
    /// Lines with their marks already worked out (by the line's id and words), while the language, the spoilers shown
    /// and the half minute (times read "in 5 min") stay the same; and texts already split into their parts.
    static RUNS: std::cell::RefCell<(u64, std::collections::HashMap<u64, std::rc::Rc<Run>>)> = Default::default();
    static PARTS: std::cell::RefCell<std::collections::HashMap<u64, std::rc::Rc<Vec<Part>>>> = Default::default();
}

/// At most this many of each kept; past it they all go and are worked out again as drawn.
const KEPT: usize = 3000;

/// A line's words and marks, worked out once (Discord's markdown is a long regex: not on every frame).
fn run_of(root: &Root, id: &str, text: &str) -> std::rc::Rc<Run> {
    let stamp = hash_of((crate::time::now_secs() / 30, root.i18n.vietnamese, SHOWN_N.load(std::sync::atomic::Ordering::Relaxed)));
    let key = hash_of((id, text));
    RUNS.with(|c| {
        let mut c = c.borrow_mut();
        if c.0 != stamp || c.1.len() > KEPT {
            c.0 = stamp;
            c.1.clear();
        }
        c.1.entry(key)
            .or_insert_with(|| {
                let mut run = Run::default();
                let mut n = 0;
                walk(root, text, &Mark::default(), id, &mut n, &mut run);
                std::rc::Rc::new(run)
            })
            .clone()
    })
}

/// A text's parts (paragraphs, lists, quotes…), split once.
fn parts_of(text: &str) -> std::rc::Rc<Vec<Part>> {
    let key = hash_of(text);
    PARTS.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() > KEPT {
            c.clear();
        }
        c.entry(key).or_insert_with(|| std::rc::Rc::new(parts(text))).clone()
    })
}

/// A picture's address as one to load: Discord's own servers over https only (a picture from any host would tell
/// that host the viewer's IP), a mock run's samples from the source tree aside.
fn media(url: &str) -> Option<String> {
    if discord_media(url) {
        return Some(url.to_string());
    }
    let rel = url.strip_prefix("/ui/dev/")?;
    if rel.contains("..") {
        return None;
    }
    let assets = std::env::var_os("BLOXKEEP_UI_ASSETS")?;
    let f = std::path::PathBuf::from(assets).join("..").join("..").join("ui").join("dev").join(rel);
    f.is_file().then(|| f.to_string_lossy().into_owned())
}

/// Whether an address is a picture on Discord's servers (where the news's pictures live).
pub fn discord_media(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else { return false };
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default().to_ascii_lowercase();
    matches!(host.as_str(), "cdn.discordapp.com" | "media.discordapp.net" | "images-ext-1.discordapp.net" | "images-ext-2.discordapp.net")
}

pub fn emoji_url(id: &str) -> String {
    format!("https://cdn.discordapp.com/emojis/{id}.png?size=48")
}

// ───────────────────────── Discord's inline marks ─────────────────────────

static INLINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?s)``(.+?)``|`([^`]+)`",                                  // 1, 2 code
        r"|<t:(-?\d{1,13})(?::([tTdDfFR]))?>",                       // 3, 4 time
        r"|<(a?):(\w{1,40}):(\d{5,25})>",                            // 5, 6, 7 emoji
        r"|\[([^\]\n]{1,300})\]\(<?(https?://[^\s)>]+)>?\)",         // 8, 9 masked link
        r#"|<?(https?://[^\s<>]*[^\s<>.,:;"')\]])>?"#,               // 10 link
        r"|\|\|(.+?)\|\|",                                           // 11 spoiler
        r"|\*\*\*(.+?)\*\*\*",                                       // 12 bold italic
        r"|\*\*(.+?)\*\*",                                           // 13 bold
        r"|__(.+?)__",                                               // 14 underline
        r"|~~(.+?)~~",                                               // 15 strike
        r"|\*(\S(?:.*?\S)?)\*",                                      // 16 italic
        r"|\b_(\S(?:.*?\S)?)_\b",                                    // 17 italic
        r"|\\([\\*_~`|<>\[\]()#-])",                                 // 18 an escaped mark
    ))
    .expect("inline pattern")
});

#[derive(Clone, Default)]
struct Mark {
    bold: bool,
    italic: bool,
    under: bool,
    strike: bool,
    code: bool,
    link: Option<String>,
    spoiler: Option<String>,
}

/// A line's words with its marks as ranges: what StyledText draws, and the links and spoilers to click.
#[derive(Default)]
struct Run {
    text: String,
    marks: Vec<(Range<usize>, Mark)>,
}

impl Run {
    fn push(&mut self, s: &str, m: &Mark) {
        if s.is_empty() {
            return;
        }
        let start = self.text.len();
        self.text.push_str(s);
        self.marks.push((start..self.text.len(), m.clone()));
    }
}

/// How a moment reads: R as "in 5 min" / "2 hours ago", t as the clock, the others as the day and time.
fn moment(root: &Root, secs: i64, style: &str) -> String {
    match style {
        "R" => {
            let d = secs - crate::time::now_secs();
            if d <= 0 {
                crate::time::ago(-d, &root.i18n)
            } else if d < 3600 {
                root.i18n.tv("ui.inMin", &[("n", &(d / 60).max(1).to_string())])
            } else {
                root.i18n.tv("ui.inHour", &[("n", &(d / 3600).to_string())])
            }
        }
        "t" | "T" => crate::time::day_time(secs).split_once(' ').map(|(_, t)| t.to_string()).unwrap_or_default(),
        _ => crate::time::day_time(secs),
    }
}

fn walk(root: &Root, text: &str, m: &Mark, key: &str, n: &mut usize, out: &mut Run) {
    let mut at = 0;
    for cap in INLINE.captures_iter(text) {
        let all = cap.get(0).expect("match");
        out.push(&text[at..all.start()], m);
        at = all.end();
        let g = |i: usize| cap.get(i).map(|x| x.as_str());
        if let Some(code) = g(1).or(g(2)) {
            out.push(code.trim(), &Mark { code: true, ..m.clone() });
        } else if let Some(t) = g(3) {
            let secs = t.parse::<i64>().unwrap_or(0);
            out.push(&moment(root, secs, g(4).unwrap_or("f")), m);
        } else if g(7).is_some() {
            // A server's emoji inside the words: left out (one in front of a line is drawn as a picture).
        } else if let (Some(words), Some(url)) = (g(8), g(9)) {
            walk(root, words, &Mark { link: Some(url.to_string()), ..m.clone() }, key, n, out);
        } else if let Some(url) = g(10) {
            out.push(url, &Mark { link: Some(url.to_string()), ..m.clone() });
        } else if let Some(inner) = g(11) {
            *n += 1;
            let k = format!("{key}:s{n}");
            if shown(&k) {
                walk(root, inner, &Mark { spoiler: Some(k), ..m.clone() }, key, n, out);
            } else {
                // Hidden: its place says so in words (a click shows it), instead of a blank grey bar in the line.
                out.push(&format!(" {} ", root.tr("news.spoiler")), &Mark { spoiler: Some(k), ..m.clone() });
            }
        } else if let Some(inner) = g(12) {
            walk(root, inner, &Mark { bold: true, italic: true, ..m.clone() }, key, n, out);
        } else if let Some(inner) = g(13) {
            walk(root, inner, &Mark { bold: true, ..m.clone() }, key, n, out);
        } else if let Some(inner) = g(14) {
            walk(root, inner, &Mark { under: true, ..m.clone() }, key, n, out);
        } else if let Some(inner) = g(15) {
            walk(root, inner, &Mark { strike: true, ..m.clone() }, key, n, out);
        } else if let Some(inner) = g(16).or(g(17)) {
            walk(root, inner, &Mark { italic: true, ..m.clone() }, key, n, out);
        } else if let Some(ch) = g(18) {
            out.push(ch, m);
        } else {
            out.push(all.as_str(), m);
        }
    }
    out.push(&text[at..], m);
}

/// One line (or a few with their breaks) with Discord's inline marks; a server's emoji in front drawn as a picture.
fn inline(root: &Root, id: String, text: &str, cx: &mut Context<Root>) -> AnyElement {
    static LEAD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*<a?:\w{1,40}:(\d{5,25})>\s*").expect("lead"));
    let (lead, rest) = match LEAD.captures(text) {
        Some(cap) => (images::pic(&emoji_url(&cap[1]), cx), &text[cap.get(0).map_or(0, |m| m.end())..]),
        None => (None, text),
    };
    let run = run_of(root, &id, rest);
    let mut highlights = Vec::new();
    let mut clicks: Vec<(Range<usize>, Click)> = Vec::new();
    for (range, m) in &run.marks {
        let hidden = m.spoiler.as_deref().is_some_and(|k| !shown(k));
        let mut h = HighlightStyle::default();
        if m.bold {
            h.font_weight = Some(FontWeight::SEMIBOLD);
            h.color = Some(c(TEXT));
        }
        if m.italic {
            h.font_style = Some(FontStyle::Italic);
        }
        if m.under || m.link.is_some() {
            h.underline = Some(UnderlineStyle { thickness: px(1.), color: None, wavy: false });
        }
        if m.strike {
            h.strikethrough = Some(StrikethroughStyle { thickness: px(1.), color: None });
        }
        if m.code {
            h.background_color = Some(ca(WHITE, 0.09));
            h.color = Some(c(TEXT));
        }
        if m.link.is_some() {
            h.color = Some(c(TEXT));
        }
        if m.spoiler.is_some() {
            // Hidden: "Spoiler: click to see" in muted words on a quiet raised ground.
            h.background_color = Some(if hidden { c(RAISED_2) } else { ca(WHITE, 0.06) });
            if hidden {
                h.color = Some(c(MUTED));
                h.underline = None;
                h.font_weight = Some(FontWeight::MEDIUM);
            }
        }
        if h != HighlightStyle::default() {
            highlights.push((range.clone(), h));
        }
        match (&m.spoiler, &m.link) {
            (Some(k), _) if hidden => clicks.push((range.clone(), Click::Show(k.clone()))),
            (_, Some(url)) if url.starts_with("https://") => clicks.push((range.clone(), Click::Open(url.clone()))),
            _ => {}
        }
    }
    let styled = StyledText::new(run.text.clone()).with_highlights(highlights);
    let words: AnyElement = if clicks.is_empty() {
        styled.into_any_element()
    } else {
        let this = cx.entity().downgrade();
        let ranges = clicks.iter().map(|(r, _)| r.clone()).collect();
        let what: Vec<Click> = clicks.into_iter().map(|(_, w)| w).collect();
        InteractiveText::new(ElementId::Name(id.into()), styled)
            .on_click(ranges, move |i, _, cx| {
                let Some(w) = what.get(i).cloned() else { return };
                let _ = this.update(cx, |r, cx| {
                    match w {
                        Click::Show(k) => show(k),
                        Click::Open(url) => r.send("feedOpen", json!({ "url": url })),
                    }
                    cx.notify();
                });
            })
            .into_any_element()
    };
    match lead {
        Some(p) => div().flex().items_start().gap(px(6.)).child(img(p).mt(px(1.)).size(px(17.)).flex_none()).child(div().flex_1().min_w_0().child(words)).into_any_element(),
        None => words,
    }
}

#[derive(Clone)]
enum Click {
    Show(String),
    Open(String),
}

// ───────────────────────── Discord's markdown ─────────────────────────

enum Part {
    P(Vec<String>),
    H(usize, String),
    Sub(String),
    Quote(String),
    Code(String),
    List { ordered: bool, start: u32, items: Vec<(usize, String)> },
}

fn parts(text: &str) -> Vec<Part> {
    static FENCE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*```(\w{0,20})\s*(.*)$").expect("fence"));
    static HEAD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(#{1,3}) +(.+)$").expect("head"));
    static SUB: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^-# +(.+)$").expect("sub"));
    static ITEM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\s*)(?:([-*])|(\d{1,9})\.) +(.*)$").expect("item"));
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = text.split('\n').collect();
    let mut out: Vec<Part> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if let Some(f) = FENCE.captures(line) {
            let after = f.get(2).map_or("", |m| m.as_str());
            if let Some(same) = after.strip_suffix("```").or_else(|| after.trim_end().strip_suffix("```")).filter(|_| !after.is_empty()) {
                out.push(Part::Code(same.to_string()));
                i += 1;
                continue;
            }
            let mut body: Vec<String> = if after.is_empty() { Vec::new() } else { vec![after.to_string()] };
            i += 1;
            while i < lines.len() && !lines[i].trim_end().ends_with("```") {
                body.push(lines[i].to_string());
                i += 1;
            }
            if i < lines.len() {
                let end = lines[i].trim_end().trim_end_matches("```");
                if !end.is_empty() {
                    body.push(end.to_string());
                }
            }
            out.push(Part::Code(body.join("\n")));
            i += 1;
            continue;
        }
        if let Some(q) = line.strip_prefix(">>> ") {
            let mut rest = vec![q.to_string()];
            rest.extend(lines[i + 1..].iter().map(|l| l.to_string()));
            out.push(Part::Quote(rest.join("\n")));
            break;
        }
        if let Some(q) = line.strip_prefix('>') {
            let q = q.strip_prefix(' ').unwrap_or(q);
            match out.last_mut() {
                Some(Part::Quote(t)) => {
                    t.push('\n');
                    t.push_str(q);
                }
                _ => out.push(Part::Quote(q.to_string())),
            }
            i += 1;
            continue;
        }
        if let Some(h) = HEAD.captures(line) {
            out.push(Part::H(h[1].len(), h[2].to_string()));
        } else if let Some(s) = SUB.captures(line) {
            out.push(Part::Sub(s[1].to_string()));
        } else if let Some(li) = ITEM.captures(line) {
            let ordered = li.get(3).is_some();
            let depth = (li[1].replace('\t', "  ").len() / 2).min(3);
            let text = li[4].to_string();
            match out.last_mut() {
                Some(Part::List { ordered: o, items, .. }) if *o == ordered || depth > 0 => items.push((depth, text)),
                _ => out.push(Part::List { ordered, start: li.get(3).and_then(|n| n.as_str().parse().ok()).unwrap_or(1), items: vec![(depth, text)] }),
            }
        } else if line.trim().is_empty() {
            if let Some(Part::P(l)) = out.last_mut() {
                l.push(String::new());
            }
        } else {
            match out.last_mut() {
                Some(Part::P(l)) => l.push(line.to_string()),
                _ => out.push(Part::P(vec![line.to_string()])),
            }
        }
        i += 1;
    }
    // A paragraph's empty last lines are only room before what follows.
    for p in &mut out {
        if let Part::P(l) = p {
            while l.last().is_some_and(|x| x.trim().is_empty()) {
                l.pop();
            }
        }
    }
    out.retain(|p| !matches!(p, Part::P(l) if l.is_empty()));
    out
}

/// Text with Discord's markdown: headings, small print, quotes, lists, code, and the inline marks.
pub fn md(root: &Root, id: &str, text: &str, cx: &mut Context<Root>) -> Div {
    let mut d = div().flex().flex_col().gap(px(6.)).text_size(px(T_M)).line_height(px(19.)).text_color(c(TEXT_2));
    for (i, p) in parts_of(text).iter().enumerate() {
        let pid = format!("{id}.{i}");
        d = d.child(match p {
            Part::P(lines) => div()
                .flex()
                .flex_col()
                .children(lines.iter().enumerate().map(|(j, l)| if l.trim().is_empty() { div().h(px(8.)).into_any_element() } else { inline(root, format!("{pid}.{j}"), l, cx) }).collect::<Vec<_>>()),
            Part::H(level, t) => div()
                .pt(px(2.))
                .text_size(px(match *level {
                    1 => T_H + 5.,
                    2 => T_H + 2.,
                    _ => T_L,
                }))
                .line_height(px(match *level {
                    1 => 26.,
                    2 => 23.,
                    _ => 20.,
                }))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(TEXT))
                .child(inline(root, pid, t, cx)),
            Part::Sub(t) => div().text_size(px(T_S)).line_height(px(17.)).text_color(c(MUTED)).child(inline(root, pid, t, cx)),
            Part::Quote(t) => div().pl(px(12.)).border_l(px(3.)).border_color(ca(WHITE, 0.18)).child(md(root, &pid, t, cx)),
            Part::Code(t) => div()
                .px(px(10.))
                .py(px(8.))
                .rounded(px(R_S))
                .bg(c(INSET))
                .border_1()
                .border_color(c(LINE))
                .text_size(px(T_S))
                .line_height(px(17.))
                .text_color(c(TEXT))
                .font_features(tnum())
                .children(t.split('\n').map(|l| div().child(if l.is_empty() { " ".to_string() } else { l.to_string() })).collect::<Vec<_>>()),
            Part::List { ordered, start, items } => div().flex().flex_col().gap(px(2.)).children(
                items
                    .iter()
                    .enumerate()
                    .map(|(j, (depth, t))| {
                        let mark = if *ordered { format!("{}.", *start as usize + j) } else { "•".into() };
                        div()
                            .ml(px(*depth as f32 * 18.))
                            .flex()
                            .items_start()
                            .gap(px(8.))
                            .child(div().flex_none().min_w(px(12.)).text_color(c(MUTED)).font_features(tnum()).child(mark))
                            .child(div().flex_1().min_w_0().child(inline(root, format!("{pid}.{j}"), t, cx)))
                            .into_any_element()
                    })
                    .collect::<Vec<_>>(),
            ),
        });
    }
    d
}

// ───────────────────────── the blocks ─────────────────────────

/// Something Discord hides until clicked: a quiet raised block saying so, a small eye before the words.
fn hidden(root: &Root, id: String, key: String, h: f32, cx: &mut Context<Root>) -> AnyElement {
    hot(SharedString::from(id))
        .h(px(h))
        .w_full()
        .rounded(px(R_M))
        .border_1()
        .tones(c(RAISED), c(RAISED_2))
        .edge(c(LINE), ca(WHITE, 0.14))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(7.))
        .cursor_pointer()
        .text_size(px(T_S))
        .font_weight(FontWeight::MEDIUM)
        .text_color(c(MUTED))
        .hover(|d| d.text_color(c(TEXT_2)))
        .child(icon("eye", 13., c(MUTED)))
        .child(root.tr("news.spoiler"))
        .on_click(cx.listener(move |_, _, _, cx| {
            show(key.clone());
            cx.notify();
        }))
        .into_any_element()
}

fn size_text(b: f64) -> String {
    if b < 1024. {
        format!("{b} B")
    } else if b < 1_048_576. {
        if b < 10_240. { format!("{:.1} KB", b / 1024.) } else { format!("{:.0} KB", b / 1024.) }
    } else {
        format!("{:.1} MB", b / 1_048_576.)
    }
}

fn button(root: &Root, id: String, b: &Value, cx: &mut Context<Root>) -> AnyElement {
    let label = format!("{} {}", s(b, "emoji"), s(b, "label")).trim().to_string();
    if let Some(url) = b.get("url").and_then(Value::as_str).filter(|u| u.starts_with("https://")).map(str::to_string) {
        return Button::new(SharedString::from(id), label).icon_end("external").quiet().small().on_click(cx.listener(move |r, _, _, _| r.send("feedOpen", json!({ "url": url })))).into_any_element();
    }
    // Another bot's own button or menu: it answers only in Discord.
    let tip: SharedString = root.tr("news.onlyDiscord").into();
    div()
        .id(SharedString::from(id))
        .h(px(28.))
        .px(px(10.))
        .flex()
        .items_center()
        .gap(px(6.))
        .rounded(px(R_S))
        .border_1()
        .border_color(c(LINE))
        .text_size(px(T_S))
        .text_color(c(MUTED))
        .child(label)
        .when(b.get("menu").and_then(Value::as_bool).unwrap_or(false), |d| d.child(icon("chevronDown", 12., c(MUTED))))
        .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
        .into_any_element()
}

fn picture(url: &str, cx: &mut Context<Root>) -> Option<std::sync::Arc<Image>> {
    media(url).and_then(|u| images::pic(&u, cx))
}

fn gallery(root: &Root, id: &str, items: &[Value], w: f32, cx: &mut Context<Root>) -> AnyElement {
    let one = items.len() == 1;
    let cols = if one { 1. } else { 2. };
    let tile_w = ((w - 6. * (cols - 1.)) / cols).floor();
    let tiles: Vec<AnyElement> = items
        .iter()
        .take(10)
        .enumerate()
        .map(|(i, m)| {
            let key = format!("{id}:g{i}");
            let h = if one {
                let (mw, mh) = (m.get("w").and_then(Value::as_f64).unwrap_or(0.) as f32, m.get("h").and_then(Value::as_f64).unwrap_or(0.) as f32);
                if mw > 0. && mh > 0. { (tile_w * mh / mw).min(340.) } else { 240. }
            } else {
                150.
            };
            let url = s(m, "url").to_string();
            let video = m.get("video").and_then(Value::as_bool).unwrap_or(false);
            // A picture that can't be had (offline, gone) keeps no empty slab: a quiet line says so.
            let lost = !video && media(&url).is_none_or(|u| images::failed(&u, cx));
            let frame = div().w(px(tile_w)).h(px(h)).rounded(px(R_M)).overflow_hidden().bg(c(RAISED));
            if m.get("spoiler").and_then(Value::as_bool).unwrap_or(false) && !shown(&key) {
                return frame.child(hidden(root, format!("{id}-g{i}"), key, h, cx)).into_any_element();
            }
            if lost {
                return div()
                    .w(px(tile_w))
                    .h(px(36.))
                    .px(px(12.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .rounded(px(R_M))
                    .bg(c(RAISED))
                    .border_1()
                    .border_color(c(LINE))
                    .text_size(px(T_S))
                    .text_color(c(MUTED))
                    .child(icon("image", 13., c(MUTED)))
                    .child(div().min_w_0().truncate().child(root.tr("news.noPicture")))
                    .into_any_element();
            }
            if video {
                // A video plays in Discord or the browser: here a box that opens it.
                let open = url.clone();
                return frame
                    .child(
                        hot(SharedString::from(format!("{id}-v{i}")))
                            .size_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .child(icon("play", 22., c(TEXT_2)))
                            .on_click(cx.listener(move |r, _, _, _| r.send("feedOpen", json!({ "url": open })))),
                    )
                    .into_any_element();
            }
            match media(&url).and_then(|u| images::pic_at(&u, tile_w, h, cx)) {
                Some(p) => frame.child(img(p).size_full().object_fit(ObjectFit::Cover)).into_any_element(),
                // Not here yet: its place on the raised ground, a small mark in the middle.
                None => frame.flex().items_center().justify_center().child(icon("image", 16., c(DIM))).into_any_element(),
            }
        })
        .collect();
    div().flex().flex_wrap().gap(px(6.)).children(tiles).into_any_element()
}

fn fields(root: &Root, id: &str, items: &[Value], w: f32, cx: &mut Context<Root>) -> AnyElement {
    static LEAD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*<a?:\w+:(\d+)>\s*").expect("lead"));
    let cols = if w > 560. { 3. } else { 2. };
    let field_w = ((w - 16. * (cols - 1.)) / cols).floor();
    div()
        .flex()
        .flex_wrap()
        .gap_x(px(16.))
        .gap_y(px(10.))
        .children(
            items
                .iter()
                .enumerate()
                .map(|(i, f)| {
                    let wide = f.get("inline").and_then(Value::as_bool) == Some(false);
                    let name = s(f, "name");
                    let (pic, rest) = match LEAD.captures(name).filter(|_| !wide) {
                        Some(cap) => (images::pic(&emoji_url(&cap[1]), cx), &name[cap.get(0).map_or(0, |m| m.end())..]),
                        None => (None, name),
                    };
                    div()
                        .w(px(if wide { w } else { field_w }))
                        .flex()
                        .items_start()
                        .gap(px(8.))
                        .when_some(pic, |d, p| d.child(img(p).mt(px(2.)).size(px(26.)).flex_none()))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap(px(2.))
                                .child(div().text_size(px(T_S)).font_weight(FontWeight::SEMIBOLD).text_color(c(MUTED)).child(inline(root, format!("{id}.f{i}n"), rest, cx)))
                                .child(md(root, &format!("{id}.f{i}v"), s(f, "value"), cx).text_color(c(TEXT)).font_features(tnum())),
                        )
                        .into_any_element()
                })
                .collect::<Vec<_>>(),
        )
        .into_any_element()
}

fn poll(root: &Root, id: &str, p: &Value, cx: &mut Context<Root>) -> AnyElement {
    let answers: Vec<Value> = p.get("answers").and_then(Value::as_array).cloned().unwrap_or_default();
    let votes = |a: &Value| a.get("votes").and_then(Value::as_f64).unwrap_or(0.);
    let total: f64 = answers.iter().map(votes).sum();
    let top = answers.iter().map(votes).fold(0., f64::max);
    let done = p.get("done").and_then(Value::as_bool).unwrap_or(false);
    let mut foot = root.i18n.tv("news.votes", &[("n", &(total as i64).to_string())]);
    if done {
        foot = format!("{foot} · {}", root.tr("news.pollDone"));
    } else if let Some(ends) = p.get("ends").and_then(Value::as_f64) {
        foot = format!("{foot} · {}", root.i18n.tv("news.pollEnds", &[("t", &moment(root, (ends / 1000.) as i64, "R"))]));
    }
    div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(div().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(inline(root, format!("{id}.q"), s(p, "q"), cx)))
        .children(
            answers
                .iter()
                .enumerate()
                .map(|(i, a)| {
                    let share = if total > 0. { votes(a) / total } else { 0. };
                    let win = done && top > 0. && votes(a) == top;
                    let words = format!("{} {}", s(a, "emoji"), s(a, "text")).trim().to_string();
                    div()
                        .relative()
                        .h(px(34.))
                        .rounded(px(R_S))
                        .overflow_hidden()
                        .bg(ca(WHITE, 0.04))
                        .border_1()
                        .border_color(if win { ca(TEXT, 0.35) } else { c(LINE) })
                        .child(div().absolute().top_0().left_0().bottom_0().w(relative(share as f32)).bg(ca(WHITE, if win { 0.12 } else { 0.07 })))
                        .child(
                            div()
                                .relative()
                                .size_full()
                                .px(px(12.))
                                .flex()
                                .items_center()
                                .gap(px(10.))
                                .text_size(px(T_M))
                                .child(div().flex_1().min_w_0().truncate().text_color(c(TEXT)).child(inline(root, format!("{id}.a{i}"), &words, cx)))
                                .child(div().flex_none().font_features(tnum()).text_color(c(TEXT_2)).child(format!("{}%", (share * 100.).round() as i64))),
                        )
                        .into_any_element()
                })
                .collect::<Vec<_>>(),
        )
        .child(div().text_size(px(T_S)).text_color(c(MUTED)).child(foot))
        .into_any_element()
}

fn block(root: &Root, id: &str, b: &Value, w: f32, cx: &mut Context<Root>) -> Option<AnyElement> {
    let items = || b.get("items").and_then(Value::as_array).cloned().unwrap_or_default();
    Some(match s(b, "t") {
        "md" => md(root, id, s(b, "md"), cx).into_any_element(),
        "section" => {
            let thumb = b.get("thumb").and_then(Value::as_str).and_then(|u| picture(u, cx));
            let side: Option<AnyElement> = match (thumb, b.get("button").filter(|x| x.is_object())) {
                (Some(p), _) => Some(img(p).size(px(72.)).rounded(px(R_M)).object_fit(ObjectFit::Cover).flex_none().into_any_element()),
                (None, Some(btn)) => Some(div().flex_none().child(button(root, format!("{id}-b"), btn, cx)).into_any_element()),
                _ => None,
            };
            div().flex().items_start().gap(px(12.)).child(md(root, id, s(b, "md"), cx).flex_1().min_w_0()).children(side).into_any_element()
        }
        "fields" => fields(root, id, &items(), w, cx),
        "gallery" => gallery(root, id, &items(), w, cx),
        "file" => div()
            .h(px(40.))
            .px(px(12.))
            .flex()
            .items_center()
            .gap(px(10.))
            .rounded(px(R_M))
            .bg(ca(WHITE, 0.04))
            .border_1()
            .border_color(c(LINE))
            .text_size(px(T_M))
            .child(icon("download", 14., c(TEXT_2)))
            .child(div().flex_1().min_w_0().truncate().text_color(c(TEXT)).child(s(b, "name").to_string()))
            .when_some(b.get("size").and_then(Value::as_f64), |d, s| d.child(div().text_size(px(T_S)).text_color(c(MUTED)).font_features(tnum()).child(size_text(s))))
            .into_any_element(),
        "sep" => {
            let big = b.get("big").and_then(Value::as_bool).unwrap_or(false);
            let line = b.get("line").and_then(Value::as_bool) != Some(false);
            div().py(px(if big { 8. } else { 2. })).child(div().h(px(1.)).when(line, |d| d.bg(c(LINE)))).into_any_element()
        }
        "buttons" => div().flex().flex_wrap().gap(px(6.)).children(items().iter().enumerate().map(|(i, x)| button(root, format!("{id}-b{i}"), x, cx)).collect::<Vec<_>>()).into_any_element(),
        "footer" => {
            let pic = b.get("icon").and_then(Value::as_str).and_then(|u| picture(u, cx));
            let at = b.get("at").and_then(Value::as_f64).map(|ms| crate::time::day_time((ms / 1000.) as i64));
            let words = s(b, "md");
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .text_size(px(T_S))
                .text_color(c(MUTED))
                .when_some(pic, |d, p| d.child(img(p).size(px(16.)).rounded_full().flex_none()))
                .when(!words.is_empty(), |d| d.child(div().min_w_0().child(inline(root, format!("{id}.w"), words, cx))))
                .when(!words.is_empty() && at.is_some(), |d| d.child("•"))
                .when_some(at, |d, t| d.child(div().flex_none().font_features(tnum()).child(t)))
                .into_any_element()
        }
        "sticker" => img(picture(s(b, "url"), cx)?).size(px(120.)).object_fit(ObjectFit::Contain).into_any_element(),
        "poll" => poll(root, id, b, cx),
        // A kind Discord adds later: its words, if it has some.
        _ => md(root, id, b.get("md").and_then(Value::as_str)?, cx).into_any_element(),
    })
}

/// A button that is of no use in the app: one that only answers inside Discord (another bot's own button or menu), or
/// a link that leads back into Discord (the bot's "Add me to your server", its support server).
fn idle_button(b: &Value) -> bool {
    match b.get("url").and_then(Value::as_str) {
        None => true,
        Some(u) => {
            let host = u.strip_prefix("https://").unwrap_or(u).split(['/', '?']).next().unwrap_or_default().to_ascii_lowercase();
            ["discord.gg", "discord.com", "discordapp.com", "top.gg", "dsc.gg"].iter().any(|d| host == *d || host.ends_with(&format!(".{d}")))
        }
    }
}

/// A card's blocks without what is only the bot talking about itself (the owner, 2026-11-15: the News tab "trông lộn
/// xộn": every alert ended with the bot's own line and an "Add me to your server" button): buttons of no use here, a
/// footer that only names the sender (the card's head has the sender and the time), and a rule left at the end or
/// doubled by what went.
fn tidied(list: &[Value], author: &str) -> Vec<Value> {
    let name = author.trim().to_lowercase();
    let mut out: Vec<Value> = Vec::new();
    for b in list {
        match s(b, "t") {
            "buttons" => {
                let kept: Vec<Value> = b.get("items").and_then(Value::as_array).into_iter().flatten().filter(|x| !idle_button(x)).cloned().collect();
                if kept.is_empty() {
                    continue;
                }
                let mut b = b.clone();
                b["items"] = Value::Array(kept);
                out.push(b);
            }
            "section" if b.get("button").is_some_and(|x| x.is_object() && idle_button(x)) && b.get("thumb").and_then(Value::as_str).is_none() => {
                let mut b = b.clone();
                if let Some(o) = b.as_object_mut() {
                    o.remove("button");
                }
                out.push(b);
            }
            "footer" if s(b, "md").trim().is_empty() || (!name.is_empty() && s(b, "md").to_lowercase().contains(&name)) => continue,
            "sep" if out.last().is_none_or(|l| s(l, "t") == "sep") => continue,
            _ => out.push(b.clone()),
        }
    }
    while out.last().is_some_and(|l| s(l, "t") == "sep") {
        out.pop();
    }
    out
}

/// A card's blocks in order; a card Discord hides as a spoiler stays hidden until clicked. `author` is who sent it.
pub fn blocks(root: &Root, card_id: &str, list: &[Value], author: &str, spoiler: bool, w: f32, cx: &mut Context<Root>) -> AnyElement {
    let key = format!("{card_id}:all");
    if spoiler && !shown(&key) {
        return hidden(root, format!("nb-{card_id}-hidden"), key, 44., cx);
    }
    let list = &tidied(list, author);
    div()
        .flex()
        .flex_col()
        .gap(px(10.))
        .children(list.iter().enumerate().filter_map(|(i, b)| block(root, &format!("nb-{card_id}-{i}"), b, w, cx)).collect::<Vec<_>>())
        .into_any_element()
}
