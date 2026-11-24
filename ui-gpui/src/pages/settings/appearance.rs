// Settings › Appearance (the old page's AppearancePage.tsx): the theme (its groups, a tile per theme in the theme's own
// colours and picture), Custom's colours, picture, fade and blur, saving it by name, the themes kept by name, then the
// language and hidden names. A pick shows at once (theme::preview) and is sent as the web page sent it (setTheme,
// setCustomTheme once the dragging rests, pickThemePicture, saveTheme…); the app's next state confirms it.
// A video picked for Custom plays behind the page (scene.rs, video.rs).
use std::time::{Duration, Instant};

use gpui_kit::{
    component::input::{InputEvent, InputState},
    prelude::*,
    *,
};
use serde_json::{Value, json};

use super::{chips, flag, group, row, switch};
use crate::ask::answer;
use crate::{
    kit::*,
    shell::Root,
    theme::{self, *},
    theme_table::GROUPS,
};

/// A tile's picture, logical px.
const TILE_W: f32 = 150.;
const TILE_H: f32 = 76.;

/// The custom theme's ready colours (ColorField.tsx's READY).
const READY: [u32; 16] = [0x141a26, 0x0b0d12, 0x1b2a3a, 0x2a1b3d, 0x3a1d2b, 0x123524, 0xf1e9da, 0xffffff, 0x22c55e, 0x3b82f6, 0xa855f7, 0xec4899, 0xef4444, 0xf59e0b, 0x14b8a6, 0xeab308];

pub struct Boxes {
    name: Entity<InputState>,
    hex: Entity<InputState>,
    /// A theme code pasted in (Custom › Theme code).
    code: Entity<InputState>,
}

#[derive(Default)]
pub struct Look {
    /// The group shown (None: the one of the theme in use).
    group: Option<&'static str>,
    boxes: Option<Boxes>,
    /// The colour whose picker is open ("background" or "accent").
    colour: Option<&'static str>,
    /// The custom look being changed (shown at once, sent once the changes rest), and which change is the last.
    preview: Option<Value>,
    round: u64,
    /// A theme picked, until the app's state says so.
    picked: Option<(String, Instant)>,
    /// The kept theme whose Delete is being asked about.
    deleting: Option<String>,
    /// The name box is to be emptied (it was saved).
    clear_name: bool,
    scale: f32,
}

/// Each frame on Settings: the boxes made the first time, the screen's scale kept for the tiles' pictures.
pub fn settle(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    r.settings.look.scale = window.scale_factor();
    if r.settings.look.boxes.is_none() {
        let name = cx.new(|cx| InputState::new(window, cx).placeholder(r.tr("ct.namePh")));
        let hex = cx.new(|cx| InputState::new(window, cx).placeholder("#1b2a3a"));
        cx.subscribe_in(&hex, window, |r: &mut Root, e, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Change) {
                let typed = e.read(cx).value().trim().to_lowercase();
                let typed = if typed.starts_with('#') { typed } else { format!("#{typed}") };
                if let (Some(which), true) = (r.settings.look.colour, typed.len() == 7 && typed[1..].chars().all(|c| c.is_ascii_hexdigit())) {
                    if custom_now(r).get(which).and_then(Value::as_str) != Some(typed.as_str()) {
                        change(r, json!({ which: typed }), cx);
                    }
                }
            }
        })
        .detach();
        cx.subscribe_in(&name, window, |r: &mut Root, _, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::PressEnter { .. }) {
                save(r, cx);
            }
            cx.notify();
        })
        .detach();
        let code = cx.new(|cx| InputState::new(window, cx).placeholder("bloxkeep:141a26-f59e0b-45-0"));
        cx.subscribe_in(&code, window, |r: &mut Root, _, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::PressEnter { .. }) {
                apply_code(r, cx);
            }
            cx.notify();
        })
        .detach();
        r.settings.look.boxes = Some(Boxes { name, hex, code });
    }
    // BLOXKEEP_SHOT_AP=customize | surprise | code:<theme code> | video:<user/file>: a hidden shot
    // presses that control once, through the very code a click runs, so its result can be looked at.
    #[cfg(feature = "shot")]
    {
        static PRESSED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if r.state.get("theme").is_some()
            && let Ok(what) = std::env::var("BLOXKEEP_SHOT_AP")
            && !PRESSED.swap(true, std::sync::atomic::Ordering::SeqCst)
        {
            if what == "customize" {
                let from = current(r);
                customize_from(r, &from, cx);
            } else if what == "surprise" {
                surprise(r, cx);
            } else if let Some(code) = what.strip_prefix("code:") {
                if let Some(bx) = &r.settings.look.boxes {
                    bx.code.update(cx, |s, cx| s.set_value(code.to_string(), window, cx));
                }
                apply_code(r, cx);
            } else if let Some(file) = what.strip_prefix("video:") {
                r.send("debugCustomTheme", json!({ "picture": file, "video": true }));
            }
        }
    }
    if r.settings.look.clear_name {
        r.settings.look.clear_name = false;
        if let Some(bx) = &r.settings.look.boxes {
            bx.name.update(cx, |s, cx| s.set_value("", window, cx));
        }
    }
}

/// The custom look as it shows now: the one being changed, else the app's.
fn custom_now(r: &Root) -> Value {
    r.settings.look.preview.clone().or_else(|| r.state.get("customTheme").filter(|c| c.is_object()).cloned()).unwrap_or_else(|| json!({ "background": "#1b2a3a", "accent": "#f59e0b", "fade": 45, "blur": 0 }))
}

/// The theme chosen (Follow Windows is "system"), a pick not yet confirmed included.
pub(crate) fn current(r: &Root) -> String {
    let said = r.state.get("theme").and_then(Value::as_str).unwrap_or("dark");
    match &r.settings.look.picked {
        Some((id, at)) if id != said && at.elapsed() < Duration::from_secs(2) => id.clone(),
        _ => said.to_string(),
    }
}

/// A custom theme started from the colours of the theme in use (its page's ground and its accent), as Windows
/// Terminal's schemes start from a copy of one.
fn customize_from(r: &mut Root, id: &str, cx: &mut Context<Root>) {
    let p = palette_of(id);
    let solid = |h: Hsla| Hsla { a: 1., ..h };
    let background = hex(solid(theme::over(p.get(PANEL), solid(p.get(GROUND)))));
    let accent = hex(solid(p.get(FACE)));
    r.settings.look.group = Some("custom");
    change(r, json!({ "background": background, "accent": accent }), cx);
    r.send("setTheme", json!({ "theme": "custom" }));
}

/// Two colours by chance: a ground of some hue, dark four times in five, and an accent from across the wheel.
fn surprise(r: &mut Root, cx: &mut Context<Root>) {
    let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0) as u64 / 1000;
    let hue = (n % 360) as f32 / 360.;
    let light = (n / 360) % 5 == 0;
    let ground = gpui_kit::hsla(hue, 0.28, if light { 0.94 } else { 0.11 }, 1.);
    let accent = gpui_kit::hsla((hue + 0.42 + ((n / 1800) % 20) as f32 / 100.) % 1., 0.85, if light { 0.40 } else { 0.62 }, 1.);
    change(r, json!({ "background": hex(ground), "accent": hex(accent) }), cx);
}

/// The custom look as one line to send to someone ("bloxkeep:141a26-f59e0b-45-0": ground, accent, fade, blur; a picture
/// stays on this PC), as Slack shares a theme by its colours.
pub(crate) fn code_of(ct: &Value) -> String {
    let colour = |k: &str, or: &str| ct.get(k).and_then(Value::as_str).unwrap_or(or).trim_start_matches('#').to_lowercase();
    let num = |k: &str, or: i64| ct.get(k).and_then(Value::as_f64).map(|v| v.round() as i64).unwrap_or(or);
    format!("bloxkeep:{}-{}-{}-{}", colour("background", "141a26"), colour("accent", "f59e0b"), num("fade", 45), num("blur", 0))
}

/// The look a code says; None when it isn't one.
pub(crate) fn look_of(code: &str) -> Option<Value> {
    let parts: Vec<&str> = code.trim().strip_prefix("bloxkeep:")?.split('-').collect();
    let colour = |s: &str| (s.len() == 6 && s.chars().all(|c| c.is_ascii_hexdigit())).then(|| format!("#{}", s.to_lowercase()));
    let (background, accent) = (colour(parts.first()?)?, colour(parts.get(1)?)?);
    let fade = parts.get(2).and_then(|s| s.parse::<i64>().ok()).unwrap_or(45).clamp(0, 90);
    let blur = parts.get(3).and_then(|s| s.parse::<i64>().ok()).unwrap_or(0).clamp(0, 30);
    Some(json!({ "background": background, "accent": accent, "fade": fade, "blur": blur }))
}

fn apply_code(r: &mut Root, cx: &mut Context<Root>) {
    let typed = r.settings.look.boxes.as_ref().map(|b| b.code.read(cx).value().to_string()).unwrap_or_default();
    match look_of(&typed) {
        Some(look) => change(r, look, cx),
        None => {
            r.toast = Some((r.tr("ct.codeBad"), "warn".into()));
            cx.notify();
        }
    }
}

/// A theme picked: shown at once, then sent.
fn pick(r: &mut Root, id: &str, cx: &mut Context<Root>) {
    r.settings.look.picked = Some((id.to_string(), Instant::now()));
    match id {
        "custom" => {
            let ct = custom_now(r);
            theme::preview("custom", Some(&ct), cx);
        }
        // (Follow Windows: the app works out which, its state shows it.)
        "system" => theme::release(),
        _ => theme::preview(id, None, cx),
    }
    r.send("setTheme", json!({ "theme": id }));
    cx.notify();
}

/// A change to the custom look: shown at once, sent once the changes rest (app.js previewCustom).
fn change(r: &mut Root, patch: Value, cx: &mut Context<Root>) {
    let mut ct = custom_now(r);
    if let (Some(o), Some(p)) = (ct.as_object_mut(), patch.as_object()) {
        for (k, v) in p {
            o.insert(k.clone(), v.clone());
        }
    }
    theme::preview("custom", Some(&ct), cx);
    r.settings.look.preview = Some(ct);
    r.settings.look.picked = Some(("custom".into(), Instant::now()));
    r.settings.look.round += 1;
    let round = r.settings.look.round;
    cx.spawn(async move |this, cx| {
        cx.background_executor().timer(Duration::from_millis(300)).await;
        let _ = this.update(cx, |r, cx| {
            if r.settings.look.round != round {
                return;
            }
            let Some(c) = r.settings.look.preview.clone() else { return };
            let field = |k: &str| c.get(k).cloned().unwrap_or(Value::Null);
            let Some(link) = r.link.clone() else { return };
            let rx = link.call("setCustomTheme", json!({ "background": field("background"), "accent": field("accent"), "fade": field("fade"), "blur": field("blur") }));
            answer(rx, cx, move |r, answer, cx| {
                // Kept or refused, the look shown is the app's again (a refused one doesn't stay as if saved).
                if let Ok(Err(e)) = &answer {
                    r.toast = Some((e.clone(), "bad".into()));
                }
                if r.settings.look.round == round {
                    r.settings.look.preview = None;
                }
                cx.notify();
            });
        });
    })
    .detach();
    cx.notify();
}

fn save(r: &mut Root, cx: &mut Context<Root>) {
    let typed = r.settings.look.boxes.as_ref().map(|b| b.name.read(cx).value().trim().to_string()).unwrap_or_default();
    let name = if typed.is_empty() { in_use(r).map(|s| s.get("name").and_then(Value::as_str).unwrap_or_default().to_string()).unwrap_or_default() } else { typed };
    if name.is_empty() {
        return;
    }
    r.ask("saveTheme", json!({ "name": name }), cx, move |r, _| {
        r.settings.look.clear_name = true;
        let text = r.i18n.tv("ct.savedToast", &[("name", &name)]);
        r.ok(text);
    });
}

/// The kept theme with the same look as the custom one in use (its name is what Save keeps it under again).
fn in_use(r: &Root) -> Option<Value> {
    if current(r) != "custom" {
        return None;
    }
    let ct = custom_now(r);
    let same = |a: &Value| {
        let s = |v: &Value, k: &str| v.get(k).cloned().unwrap_or(Value::Null);
        ["background", "accent", "picture"].iter().all(|k| s(a, k) == s(&ct, k))
            && s(a, "fade").as_f64() == s(&ct, "fade").as_f64()
            && s(a, "blur").as_f64().unwrap_or(0.) == s(&ct, "blur").as_f64().unwrap_or(0.)
    };
    r.state.get("savedThemes").and_then(Value::as_array)?.iter().find(|s| s.get("look").is_some_and(same)).cloned()
}

fn hex(h: Hsla) -> String {
    let c = h.to_rgb();
    let b = |v: f32| (v.clamp(0., 1.) * 255.).round() as u32;
    format!("#{:02x}{:02x}{:02x}", b(c.r), b(c.g), b(c.b))
}

fn colour_of(text: &str) -> Hsla {
    text.strip_prefix('#').and_then(|s| u32::from_str_radix(s, 16).ok()).map(c).unwrap_or(c(0x000000))
}

pub fn appearance(root: &Root, cx: &mut Context<Root>) -> Vec<Div> {
    let cur = current(root);
    // Popular first (AppearancePage.tsx): the themes most PCs with Bloxkeep open are in now, as the online count's bot
    // sums them (2 PCs or more each, most first); only themes this app has. None said: no such group.
    let popular: Vec<(&'static str, i64)> = root
        .state
        .pointer("/community/data/themes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|t| {
            let id = t.get("id").and_then(Value::as_str)?;
            let known = GROUPS.iter().flat_map(|(_, l)| l.iter()).find(|k| **k == id)?;
            Some((*known, t.get("pcs").and_then(Value::as_i64).unwrap_or(0)))
        })
        .collect();
    let mut groups: Vec<&'static str> = Vec::new();
    if !popular.is_empty() {
        groups.push("popular");
    }
    groups.extend(GROUPS.iter().map(|(g, _)| *g));
    // BLOXKEEP_SHOT_THEMES=popular: a hidden shot opens on that group.
    #[cfg(feature = "shot")]
    let asked: Option<&'static str> = root.settings.look.group.or_else(|| std::env::var("BLOXKEEP_SHOT_THEMES").ok().and_then(|g| groups.iter().copied().find(|k| *k == g)));
    #[cfg(not(feature = "shot"))]
    let asked = root.settings.look.group;
    let shown_group = asked
        .filter(|g| groups.contains(g))
        .unwrap_or_else(|| if cur == crate::theme::KEEP.id { "scenery" } else { GROUPS.iter().find(|(_, l)| l.contains(&cur.as_str())).map(|(g, _)| *g).unwrap_or("basic") });
    let mut tabs = div().flex().flex_wrap().gap(px(4.));
    for (i, g) in groups.iter().copied().enumerate() {
        tabs = tabs.child(chip(("ap-group", i), g == shown_group).child(root.tr(&format!("theme.group.{g}"))).on_click(cx.listener(move |r, _, _, cx| {
            r.settings.look.group = Some(g);
            cx.notify();
        })));
    }
    let ids: Vec<(&'static str, Option<i64>)> = if shown_group == "popular" {
        popular.iter().map(|(id, n)| (*id, Some(*n))).collect()
    } else {
        // (Bloxkeep's own picture theme leads the sceneries: it isn't one of the web page's.)
        let own = (shown_group == "scenery").then_some((crate::theme::KEEP.id, None));
        own.into_iter().chain(GROUPS.iter().find(|(g, _)| *g == shown_group).into_iter().flat_map(|(_, l)| l.iter().map(|id| (*id, None)))).collect()
    };
    let mut grid = div().flex().flex_wrap().gap(px(14.));
    for (i, (id, uses)) in ids.into_iter().enumerate() {
        grid = grid.child(tile(root, id, i, id == cur, uses, cx));
    }
    // The theme: its groups as chips, its tiles under them, in a card of their own under "Theme · <the one in use>".
    let themes = div().px(px(20.)).py(px(18.)).flex().flex_col().gap(px(18.)).child(tabs).child(grid).into_any_element();
    let mut rows = vec![themes];
    if cur == "system" {
        // Which theme for Windows' light apps and which for its dark ones (VS Code keeps a preferred light and a
        // preferred dark theme the same way).
        let all: Vec<(String, String, bool)> = GROUPS
            .iter()
            .flat_map(|(_, l)| l.iter())
            .filter(|id| !matches!(**id, "system" | "custom"))
            .map(|id| (id.to_string(), root.tr(&format!("theme.{id}")), false))
            .collect();
        let of = |k: &str, or: &str| root.state.get(k).and_then(Value::as_str).unwrap_or(or).to_string();
        let (light, dark) = (of("themeLight", "light"), of("themeDark", "dark"));
        let (dark_2, light_2) = (dark.clone(), light.clone());
        rows.push(row(root.tr("th.sysLight"), None, chips(root, cx, "ap-sys-light", all.clone(), &light, move |r, v: String, cx| r.ask("setSystemThemes", json!({ "light": v, "dark": dark_2 }), cx, |_, _| {}))));
        rows.push(row(root.tr("th.sysDark"), None, chips(root, cx, "ap-sys-dark", all, &dark, move |r, v: String, cx| r.ask("setSystemThemes", json!({ "light": light_2, "dark": v }), cx, |_, _| {}))));
    } else if cur != "custom" {
        let from = cur.clone();
        rows.push(row(
            root.tr("ct.fromThis"),
            Some(root.tr("ct.fromThisDesc")),
            Button::new("ct-from", root.tr("ct.fromThisBtn")).small().icon("palette").on_click(cx.listener(move |r, _, _, cx| customize_from(r, &from, cx))),
        ));
    }
    if shown_group == "custom" {
        // Follow Wallpaper Engine (wallpaper_engine.rs): only on a PC that has it. On, the theme becomes the custom one
        // with the wallpaper's picture (or video) and colour, again each time Wallpaper Engine's wallpaper changes.
        if crate::wallpaper_engine::config().is_some() {
            let on = crate::prefs::get("followWallpaperEngine").and_then(|v| v.as_bool()).unwrap_or(false);
            rows.push(row(
                root.tr("we.follow"),
                Some(root.tr("we.followDesc")),
                switch("we-follow", on, cx, |r, v, cx| {
                    crate::prefs::set("followWallpaperEngine", json!(v));
                    // (Read again at once: on, the wallpaper shown now is taken without waiting for a change, and from
                    // another theme too.)
                    r.we_last = None;
                    r.we_wake = v;
                    cx.notify();
                }),
            ));
        }
        if cur == "custom" {
            rows.extend(custom_rows(root, cx));
        }
        rows.extend(saved_rows(root, cx));
    }
    let theme_head = section_head(root.tr("set.theme"), None).child(div().text_size(px(T_SEC)).text_color(c(MUTED)).child(root.tr(&format!("theme.{cur}"))));
    let theme_head = if super::finding() { div() } else { theme_head };
    let langs = super::chips(root, cx, "s-lang", crate::lang::picker_items(), &root.i18n.lang, |r, v, _| r.send("setLanguage", json!({ "language": v })));
    let hide = flag(&root.state, "hideNames");
    let mut more = vec![row(root.tr("set.language"), None, langs)];
    more.push(row(
        root.tr("set.hideNames"),
        Some(root.tr("set.hideNamesDesc")),
        switch("s-hidenames", hide, cx, |r, on, cx| r.ask("setHideNames", json!({ "enabled": on }), cx, |_, _| {})),
    ));
    // Only where there is a picture to see through: glass on everything (the panels), or on the blocks alone.
    if crate::theme::frosted() {
        // (As the theme reads it: on with Sunset until the person sets it.)
        let on = crate::theme::glass_blocks_asked();
        // (On, and the picture too light or too dark for the words outside the blocks: said, as nothing changes.)
        let held = on && !crate::theme::glass_blocks();
        more.push(row(
            root.tr("set.glassBlocks"),
            Some(root.tr(if held { "set.glassBlocksHeld" } else { "set.glassBlocksDesc" })),
            switch("s-glass-blocks", on, cx, |_, on, cx| {
                crate::prefs::set("glassBlocks", json!(on));
                cx.notify();
            }),
        ));
    }
    let still = crate::kit::still_behind();
    more.push(row(
        root.tr("set.stillBehind"),
        Some(root.tr("set.stillBehindDesc")),
        switch("s-still", still, cx, |_, on, cx| {
            crate::prefs::set("stillBehind", json!(on));
            cx.notify();
        }),
    ));
    vec![div().flex().flex_col().gap(px(8.)).child(theme_head).child(super::rows_card(rows)), group(None, more)]
}

/// A theme's tile: a small window in its colours (its picture behind), a line of words and its accent, its name under.
pub(crate) fn tile(root: &Root, id: &'static str, i: usize, on: bool, uses: Option<i64>, cx: &mut Context<Root>) -> impl IntoElement {
    let scale = if root.settings.look.scale > 0. { root.settings.look.scale } else { 1. };
    let (w, h) = ((TILE_W * scale).round() as u32, (TILE_H * scale).round() as u32);
    // A little window of the theme; `half` is one of Sync with Windows' two: its own whole window in half the tile (cut
    // out of a whole one, the split went through the title line and left the dark half a lone bar, review 7.2).
    let mini = |p: &Palette, half: bool, cx: &mut Context<Root>| -> Div {
        let mut ground = div().absolute().inset_0().bg(p.get(GROUND));
        if let Some(b) = &p.backdrop {
            let pic = crate::scene::thumb(&b.file, b.under, b.veil, w, h, cx);
            ground = div()
                .absolute()
                .inset_0()
                .bg(b.under)
                .when_some(pic, |d, pic| d.child(img(ImageSource::Render(pic)).absolute().inset_0().size_full().object_fit(ObjectFit::Cover)))
                .child(div().absolute().inset_0().bg(b.wash));
        }
        div()
            .absolute()
            .inset_0()
            .child(ground)
            .child(
                div()
                    .absolute()
                    .left(px(if half { 8. } else { 12. }))
                    .right(px(if half { 8. } else { 12. }))
                    .top(px(12.))
                    .bottom(px(12.))
                    .rounded(px(7.))
                    .bg(p.get(PANEL))
                    .child(div().absolute().left(px(10.)).top(px(11.)).w(px(if half { 34. } else { 58. })).h(px(4.)).rounded_full().bg(p.get(TEXT)))
                    .child(div().absolute().left(px(10.)).top(px(25.)).w(px(if half { 24. } else { 34. })).h(px(12.)).rounded_full().bg(p.get(FACE))),
            )
    };
    let picture = match id {
        "system" => div()
            .absolute()
            .inset_0()
            .child(div().absolute().left_0().top_0().bottom_0().w(relative(0.5)).overflow_hidden().child(mini(&palette_of("light"), true, cx)))
            .child(div().absolute().right_0().top_0().bottom_0().w(relative(0.5)).overflow_hidden().child(mini(&palette_of("dark"), true, cx))),
        "custom" => {
            let ct = custom_now(root);
            mini(&custom_palette(&ct), false, cx)
        }
        _ => mini(&palette_of(id), false, cx),
    };
    div()
        .id(("ap-tile", i))
        .w(px(TILE_W))
        .flex()
        .flex_col()
        .gap(px(7.))
        .cursor_pointer()
        .child(
            div()
                .relative()
                .w(px(TILE_W))
                .h(px(TILE_H))
                .rounded(px(R_M))
                .overflow_hidden()
                .border_1()
                .border_color(if on { c(FACE) } else { ca(WHITE, 0.14) })
                .child(picture)
                .when(on, |d| d.child(tick_mark(20.).absolute().top(px(6.)).right(px(6.)))),
        )
        .child(
            div()
                .px(px(2.))
                .truncate()
                .text_size(px(T_S))
                .when(on, |d| d.font_weight(FontWeight::SEMIBOLD))
                .text_color(if on { c(TEXT) } else { c(TEXT_2) })
                .child(root.tr(&format!("theme.{id}"))),
        )
        // (Under Popular: how many PCs are in it now.)
        .when_some(uses, |d, n| d.child(div().mt(px(-5.)).px(px(2.)).text_size(px(T_S)).text_color(c(MUTED)).font_features(tnum()).child(root.i18n.tn("set.themeUses", n, &[]))))
        .when(on, |d| d.child(div().absolute().top(px(-3.)).left(px(-3.)).w(px(TILE_W + 6.)).h(px(TILE_H + 6.)).rounded(px(R_M + 3.)).border_2().border_color(c(FACE))))
        .relative()
        .hover(|d| d.opacity(0.92))
        // (Its picture drawn ahead while the mouse rests here: the pick then shows it at once.)
        .on_hover(cx.listener(move |r, over: &bool, window, cx| {
            if *over && current(r) != id {
                crate::scene::warm(id, window, cx);
            }
        }))
        .on_click(cx.listener(move |r, _, _, cx| {
            if current(r) != id {
                pick(r, id, cx);
            }
        }))
}

/// Custom chosen: its two colours, its picture (fade and blur with one), saving it by name.
fn custom_rows(root: &Root, cx: &mut Context<Root>) -> Vec<AnyElement> {
    let ct = custom_now(root);
    let text = |k: &str| ct.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
    let num = |k: &str, or: i64| ct.get(k).and_then(Value::as_f64).map(|v| v.round() as i64).unwrap_or(or);
    let has_pic = ct.get("picture").and_then(Value::as_str).is_some_and(|p| !p.is_empty());
    let mut rows = vec![
        row(root.tr("ct.background"), Some(root.tr("ct.backgroundDesc")), colour_field(root, "background", &text("background"), cx)),
        row(root.tr("ct.accent"), Some(root.tr("ct.accentDesc")), colour_field(root, "accent", &text("accent"), cx)),
    ];
    let this = cx.entity().downgrade();
    let mut pic_btns = div().flex().gap(px(8.));
    if has_pic {
        let t = this.clone();
        pic_btns = pic_btns.child(Button::new("ct-clear", root.tr("ct.remove")).small().quiet().on_click(move |_, _, cx| {
            let _ = t.update(cx, |r, _| r.send("clearThemePicture", json!({})));
        }));
    }
    let t = this.clone();
    pic_btns = pic_btns.child(Button::new("ct-pick", root.tr(if has_pic { "ct.change" } else { "ct.pick" })).small().icon("image").on_click(move |_, _, cx| {
        let _ = t.update(cx, |r, cx| r.ask("pickThemePicture", json!({}), cx, |_, _| {}));
    }));
    rows.push(row(root.tr("ct.picture"), Some(root.tr("ct.pictureDesc")), pic_btns));
    if has_pic {
        let fade = num("fade", 45);
        rows.push(row(root.tr("ct.fade"), Some(root.tr("ct.fadeDesc")), slider("ct-fade", fade, 0, 90, 5, format!("{fade} %"), cx, |r, v, cx| change(r, json!({ "fade": v }), cx))));
        let blur = num("blur", 0);
        rows.push(row(root.tr("ct.blur"), Some(root.tr("ct.blurDesc")), slider("ct-blur", blur, 0, 30, 1, format!("{blur} px"), cx, |r, v, cx| change(r, json!({ "blur": v }), cx))));
    }
    rows.insert(
        2,
        row(root.tr("ct.surprise"), Some(root.tr("ct.surpriseDesc")), Button::new("ct-surprise", root.tr("ct.surpriseBtn")).small().icon("shuffle").on_click(cx.listener(|r, _, _, cx| surprise(r, cx)))),
    );
    if let Some(bx) = &root.settings.look.boxes {
        let code = code_of(&ct);
        let t = this.clone();
        rows.push(row(
            root.tr("ct.code"),
            Some(root.tr("ct.codeDesc")),
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(Button::new("ct-code-copy", root.tr("ct.codeCopy")).small().icon("copy").on_click(cx.listener(move |r, _, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(code.clone()));
                    let text = r.tr("ct.codeCopied");
                    r.ok(text);
                    cx.notify();
                })))
                .child(field(&bx.code, None).w(px(230.)).h(px(34.)))
                .child(Button::new("ct-code-apply", root.tr("ct.codeApply")).small().on_click(move |_, _, cx| {
                    let _ = t.update(cx, |r, cx| apply_code(r, cx));
                })),
        ));
    }
    if let Some(bx) = &root.settings.look.boxes {
        let typed = !bx.name.read(cx).value().trim().is_empty();
        let can = typed || in_use(root).is_some();
        let t = this.clone();
        rows.push(row(
            root.tr("ct.save"),
            Some(root.tr("ct.saveDesc")),
            div().flex().items_center().gap(px(8.)).child(field(&bx.name, None).w(px(210.)).h(px(34.))).child(Button::new("ct-save", root.tr("ct.saveBtn")).small().disabled(!can).on_click(move |_, _, cx| {
                let _ = t.update(cx, |r, cx| save(r, cx));
            })),
        ));
    }
    rows
}

/// Your themes: Add from file, then each kept theme (use it, save it as a file, delete it).
fn saved_rows(root: &Root, cx: &mut Context<Root>) -> Vec<AnyElement> {
    let saved: Vec<Value> = root.state.get("savedThemes").and_then(Value::as_array).cloned().unwrap_or_default();
    let this = cx.entity().downgrade();
    let t = this.clone();
    let mut rows = vec![row(
        root.tr("ct.yours"),
        Some(root.tr(if saved.is_empty() { "ct.yoursNone" } else { "ct.yoursDesc" })),
        Button::new("ct-import", root.tr("ct.import")).small().icon("folder").on_click(move |_, _, cx| {
            let _ = t.update(cx, |r, cx| r.ask("importTheme", json!({}), cx, |_, _| {}));
        }),
    )];
    if saved.is_empty() {
        return rows;
    }
    let using = in_use(root).and_then(|s| s.get("id").and_then(Value::as_str).map(str::to_string));
    let mut list = div().px(px(12.)).py(px(8.)).flex().flex_col().gap(px(2.));
    for (i, s) in saved.iter().enumerate() {
        let id = s.get("id").and_then(Value::as_str).unwrap_or_default().to_string();
        let name = s.get("name").and_then(Value::as_str).unwrap_or_default().to_string();
        let look = s.get("look").cloned().unwrap_or(Value::Null);
        let video = look.get("video").and_then(Value::as_bool).unwrap_or(false);
        let p = custom_palette(&look);
        let on = using.as_deref() == Some(id.as_str());
        let mut dot = div().relative().w(px(34.)).h(px(24.)).flex_none().rounded(px(6.)).overflow_hidden().border_1().border_color(ca(WHITE, 0.14)).bg(p.get(GROUND));
        if let Some(bk) = &p.backdrop {
            let scale = if root.settings.look.scale > 0. { root.settings.look.scale } else { 1. };
            if let Some(pic) = crate::scene::thumb(&bk.file, bk.under, bk.veil, (34. * scale) as u32, (24. * scale) as u32, cx) {
                dot = dot.child(img(ImageSource::Render(pic)).absolute().inset_0().size_full().object_fit(ObjectFit::Cover));
            }
        }
        dot = dot.child(div().absolute().right(px(4.)).bottom(px(4.)).size(px(9.)).rounded_full().bg(p.get(FACE)));
        let (use_id, ex_id, del_id) = (id.clone(), id.clone(), id.clone());
        let deleting = root.settings.look.deleting.as_deref() == Some(id.as_str());
        let t = this.clone();
        let t2 = this.clone();
        list = list.child(
            hover_only(("ct-saved", i))
                .h(px(42.))
                .px(px(8.))
                .rounded(px(R_M))
                .flex()
                .items_center()
                .gap(px(10.))
                .when(on, |d| d.bg(ca(WHITE, 0.06)))
                .child(
                    div()
                        .id(("ct-use", i))
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .cursor_pointer()
                        .child(dot)
                        .child(div().min_w_0().truncate().text_size(px(T_M)).font_weight(if on { FontWeight::SEMIBOLD } else { FontWeight::NORMAL }).child(name.clone()))
                        .when(video, |d| d.child(badge(root.tr("ct.video"), Tone::Muted)))
                        .on_click(cx.listener(move |r, _, _, cx| {
                            r.settings.look.picked = Some(("custom".into(), Instant::now()));
                            // (A change still waiting to be sent would undo the pick: it goes.)
                            r.settings.look.preview = None;
                            r.settings.look.round += 1;
                            r.send("useSavedTheme", json!({ "id": use_id }));
                            cx.notify();
                        })),
                )
                .child(IconButton::new(("ct-export", i), "download").tip(root.tr("ct.export")).on_click(move |_, _, cx| {
                    let id = ex_id.clone();
                    let _ = t.update(cx, |r, cx| {
                        r.ask("exportTheme", json!({ "id": id }), cx, |r, v| {
                            if let Some(file) = v.get("saved").and_then(Value::as_str) {
                                let text = r.i18n.tv("ct.exported", &[("file", file)]);
                                r.ok(text);
                            }
                        })
                    });
                }))
                .child(if deleting {
                    Button::new(("ct-del-sure", i), root.tr("ct.deleteSure")).small().danger().on_click(move |_, _, cx| {
                        let id = del_id.clone();
                        let _ = t2.update(cx, |r, cx| {
                            r.settings.look.deleting = None;
                            r.send("deleteSavedTheme", json!({ "id": id }));
                            cx.notify();
                        });
                    }).into_any_element()
                } else {
                    IconButton::new(("ct-del", i), "trash").tip(root.tr("ct.delete")).on_click(move |_, _, cx| {
                        let id = del_id.clone();
                        let _ = t2.update(cx, |r, cx| {
                            r.settings.look.deleting = Some(id);
                            cx.notify();
                        });
                    }).into_any_element()
                }),
        );
    }
    rows.push(list.into_any_element());
    rows
}

/// A colour's box: its colour, and under it Bloxkeep's own picker (ready colours, the hues, shades of the one chosen,
/// the #rrggbb), never Windows'.
fn colour_field(root: &Root, which: &'static str, value: &str, cx: &mut Context<Root>) -> Div {
    let open = root.settings.look.colour == Some(which);
    let shown = colour_of(value);
    let v = value.to_string();
    let mut wrap = div().relative().flex_none().child(
        hot(SharedString::from(format!("ct-colour-{which}")))
            .w(px(46.))
            .h(px(32.))
            .p(px(3.))
            .rounded(px(R_M))
            .border_1()
            .tones(c(INSET), c(INSET))
            .edge(ca(TEXT, if open { 0.5 } else { 0.16 }), ca(TEXT, 0.4))
            .cursor_pointer()
            .child(div().size_full().rounded(px(6.)).bg(shown).border_1().border_color(ca(WHITE, 0.12)))
            .on_click(cx.listener(move |r, _, window, cx| {
                r.settings.look.colour = if r.settings.look.colour == Some(which) { None } else { Some(which) };
                if let Some(bx) = &r.settings.look.boxes {
                    bx.hex.update(cx, |s, cx| s.set_value(v.clone(), window, cx));
                }
                cx.notify();
            })),
    );
    if !open {
        return wrap;
    }
    let square = |id: (&'static str, usize), col: Hsla, chosen: bool| {
        div().id(id).size(px(22.)).flex_none().rounded(px(6.)).bg(col).border_1().border_color(if chosen { c(TEXT) } else { ca(WHITE, 0.14) }).cursor_pointer().hover(|d| d.border_color(ca(TEXT, 0.7)))
    };
    let mut ready = div().flex().flex_wrap().gap(px(6.));
    for (i, h) in READY.iter().enumerate() {
        let col = c(*h);
        let hv = hex(col);
        ready = ready.child(square(("ct-ready", i), col, hv == value).on_click(pick_colour(which, hv, cx)));
    }
    // The hues, then shades of the one shown (at its own strength; a grey's shades stay grey).
    let mut hues = div().flex().h(px(16.)).rounded(px(4.)).overflow_hidden();
    for i in 0..24usize {
        let col = hsla(i as f32 / 24., 0.78, 0.52, 1.);
        hues = hues.child(div().id(("ct-hue", i)).flex_1().h_full().bg(col).cursor_pointer().on_click(pick_colour(which, hex(col), cx)));
    }
    let base = shown;
    let mut shades = div().flex().gap(px(4.));
    for i in 0..10 {
        let col = hsla(base.h, if base.s < 0.04 { 0. } else { base.s }, 0.08 + i as f32 * 0.093, 1.);
        let hv = hex(col);
        shades = shades.child(square(("ct-shade", i), col, hv == value).flex_1().w_auto().h(px(20.)).on_click(pick_colour(which, hv, cx)));
    }
    let hex_box = root.settings.look.boxes.as_ref().map(|bx| field(&bx.hex, None).h(px(32.)));
    let panel = div()
        .id(SharedString::from(format!("ct-{which}-pop")))
        .occlude()
        .w(px(250.))
        .p(px(12.))
        .rounded(px(R_L))
        .bg(c(RAISED))
        .border_1()
        .border_color(c(LINE))
        .shadow_lg()
        .flex()
        .flex_col()
        .gap(px(10.))
        .child(ready)
        .child(hues)
        .child(shades)
        .children(hex_box)
        .on_mouse_down_out(cx.listener(move |r, _, _, cx| {
            if r.settings.look.colour == Some(which) {
                r.settings.look.colour = None;
                cx.notify();
            }
        }));
    wrap = wrap.child(float(PopAt::BelowRight(38.), 1, panel));
    wrap
}

/// A colour picked in the picker: in its #rrggbb box and on the theme at once.
fn pick_colour(which: &'static str, hexv: String, cx: &mut Context<Root>) -> impl Fn(&ClickEvent, &mut Window, &mut App) + use<> {
    let this = cx.entity().downgrade();
    move |_, window, cx| {
        let _ = this.update(cx, |r, cx| {
            if let Some(bx) = &r.settings.look.boxes {
                bx.hex.update(cx, |s, cx| s.set_value(hexv.clone(), window, cx));
            }
            change(r, json!({ which: hexv.clone() }), cx);
        });
    }
}

/// A value on a track, dragged or clicked (the old page's range input): `step`s from `min` to `max`, its value beside it.
#[allow(clippy::too_many_arguments)]
fn slider(key: &'static str, value: i64, min: i64, max: i64, step: i64, label: String, cx: &mut Context<Root>, f: impl Fn(&mut Root, i64, &mut Context<Root>) + 'static) -> Div {
    let f = std::rc::Rc::new(f);
    let part = ((value - min) as f32 / (max - min).max(1) as f32).clamp(0., 1.);
    let n = ((max - min) / step.max(1)) as usize + 1;
    let mut zones = div().absolute().inset_0().flex();
    for i in 0..n {
        let v = min + i as i64 * step;
        let (f1, f2) = (f.clone(), f.clone());
        zones = zones.child(
            div()
                .id((key, i))
                .flex_1()
                .h_full()
                .cursor_pointer()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |r, _, _, cx| {
                        if v != value {
                            f1(r, v, cx);
                        }
                    }),
                )
                .on_mouse_move(cx.listener(move |r, e: &MouseMoveEvent, _, cx| {
                    if e.pressed_button == Some(MouseButton::Left) && v != value {
                        f2(r, v, cx);
                    }
                })),
        );
    }
    div()
        .flex()
        .items_center()
        .gap(px(14.))
        .child(
            div()
                .relative()
                .w(px(180.))
                .h(px(22.))
                .child(div().absolute().left_0().right_0().top(px(9.)).h(px(4.)).rounded_full().bg(ca(WHITE, 0.14)))
                .child(div().absolute().left_0().top(px(9.)).h(px(4.)).w(px(180. * part)).rounded_full().bg(c(FACE)))
                .child(div().absolute().top(px(2.)).left(px(180. * part - 9.)).size(px(18.)).rounded_full().bg(c(FACE)).border_2().border_color(c(INK)))
                .child(zones),
        )
        .child(div().w(px(46.)).text_size(px(T_M)).font_features(tnum()).text_color(c(TEXT_2)).flex().justify_end().child(label))
}
