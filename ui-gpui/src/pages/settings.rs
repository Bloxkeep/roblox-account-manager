// Settings: its parts listed on the left, the one picked on the right (the old Settings tab, ui/src/settings/*.tsx).
// Built so far: Launching (Multi-Roblox, windows, how they open and are laid out, what opens Roblox, the wait), Staying in
// game (auto-rejoin for every account, Anti-AFK, leaving crowded servers, the daily sign-in check, RAM and cores),
// Notifications (Windows', each kind, the phone, questions answered "Don't ask again"), Appearance (the web page's themes,
// language, hidden names: appearance.rs), Security (locking by itself, the vault picker), Backup &
// data, and Bloxkeep (updates, the app, help, privacy); Roblox (its version, updating it, the Multi-Roblox check, and
// the settings Bloxkeep gives it, in Staying in game) and Discord are in roblox.rs and discord.rs. Lists of choices are
// chips, never a select; a question the old page asked in a box (Multi-Roblox's risk, Restore) is asked in
// place under its row. Every value is the app's: a change is sent (setSettings and the others) and shows once the app's
// next state comes.
use std::collections::HashSet;

use gpui_kit::{prelude::*, *};
use serde_json::{Value, json};

use crate::{kit::*, shell::Root, theme::*};

pub mod appearance;
mod discord;
mod roblox;

/// Settings › Discord's text boxes, made the first time it is shown (shots open it straight away).
pub fn ensure_discord(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    if r.settings.dc.is_none() {
        discord::opened(r, window, cx);
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum Part {
    #[default]
    Launch,
    Roblox,
    Perf,
    Notify,
    Discord,
    Appearance,
    Security,
    Backup,
    Updates,
}

impl Part {
    pub const ALL: [Part; 9] = [Part::Launch, Part::Roblox, Part::Perf, Part::Notify, Part::Discord, Part::Appearance, Part::Security, Part::Backup, Part::Updates];
    pub fn key(self) -> &'static str {
        match self {
            Part::Launch => "launch",
            Part::Roblox => "roblox",
            Part::Perf => "perf",
            Part::Notify => "notify",
            Part::Discord => "discord",
            Part::Appearance => "appearance",
            Part::Security => "security",
            Part::Backup => "backup",
            Part::Updates => "updates",
        }
    }
    pub fn icon(self) -> &'static str {
        match self {
            Part::Launch => "play",
            Part::Roblox => "gamepad",
            Part::Perf => "gauge",
            Part::Notify => "bell",
            Part::Discord => "discord",
            Part::Appearance => "palette",
            Part::Security => "key",
            Part::Backup => "cloud",
            Part::Updates => "download",
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            Part::Launch => "set.launch",
            Part::Roblox => "set.p.roblox",
            Part::Perf => "set.perf",
            Part::Notify => "set.notifications",
            // A name, the same in every language (no words for it).
            Part::Discord => "Discord",
            Part::Appearance => "set.appearance",
            Part::Security => "set.p.security",
            Part::Backup => "set.p.backup",
            Part::Updates => "set.p.updates",
        }
    }
    pub fn from_key(k: &str) -> Option<Part> {
        Part::ALL.into_iter().find(|p| p.key() == k)
    }
    /// The one line under a part's name: what it controls.
    fn about(self) -> String {
        format!("set.about.{}", self.key())
    }
    /// The nav's groups: playing (Roblox and how it runs), what Bloxkeep tells you, and Bloxkeep itself; each group
    /// starts at its first part.
    fn group_key(self) -> Option<&'static str> {
        match self {
            Part::Launch => Some("set.nav.play"),
            Part::Notify => Some("set.nav.alerts"),
            Part::Appearance => Some("set.nav.app"),
            _ => None,
        }
    }
}

#[derive(Default)]
pub struct Data {
    pub part: Part,
    /// The master password's form: what it says under the boxes (a problem in red, or the new one's strength), and
    /// whether the app is being asked.
    pub pw_msg: Option<(String, bool)>,
    pub pw_busy: bool,
    /// A question asked in place: "multi" (Multi-Roblox's risk), "restore" (put a backup back), "rbx-update" and
    /// "rbx-reopen" (Roblox windows are open), "dc-remove" (the Discord link).
    pub asking: Option<&'static str>,
    /// Multi-Roblox's question: don't ask again.
    pub no_ask: bool,
    pub scroll: ScrollHandle,
    /// Roblox settings: Custom shown though the settings match a mode; the accounts ticked while choosing them.
    pub rc_custom: bool,
    pub rc_pick: Option<HashSet<String>>,
    /// Discord: its text boxes; the link shown (with the masked hint it was shown for, so a new link hides it); the
    /// paste box open (true: sending turns on once saved) and what it said was wrong; the summary's parts or accounts
    /// being ticked; a ping waiting for its ID (kind, user|role); the parts under More open; the preview's message
    /// (the status board, else an alert); every alert kind being turned on or off.
    pub dc: Option<discord::Boxes>,
    pub dc_shown: Option<(String, String)>,
    pub dc_link: Option<bool>,
    pub dc_link_err: Option<String>,
    pub dc_pick: Option<(&'static str, HashSet<String>)>,
    pub dc_ping: Option<(String, String)>,
    pub dc_kinds: bool,
    pub dc_look: bool,
    pub dc_board_preview: bool,
    pub dc_alerts_want: Option<bool>,
    /// Discord's tab: "summary", "alerts", "messages", "control".
    pub dc_tab: &'static str,
    /// The menu open (its key), and the one a click outside just closed (its own button mustn't open it again).
    pub menu: Option<&'static str>,
    /// The choice whose menu was open last: still built, so it can close as it opened (kit::pop).
    pub menu_was: Option<&'static str>,
    pub menu_closed: Option<(&'static str, std::time::Instant)>,
    /// Appearance: the theme groups, Custom's colours and boxes (appearance.rs).
    pub look: appearance::Look,
    /// Settings was the page shown at the last frame (`settle` notices it being left).
    pub here: bool,
}

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or_default()
}
fn b(v: &Value, k: &str) -> bool {
    v.get(k).and_then(Value::as_bool).unwrap_or(false)
}
fn st(root: &Root) -> &Value {
    root.state.get("settings").unwrap_or(&Value::Null)
}

/// Each frame (Root::render): Settings being left is noticed here, as the page only changes in shell.rs's `go`.
pub fn settle(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    let here = r.page == crate::pages::Page::Settings && !r.locked();
    if r.settings.here && !here {
        on_leave(r, window, cx);
    }
    r.settings.here = here;
}

/// Settings left (app.js's setView): the master password's boxes and what was said under them are emptied
/// (SecurityPage's clearPassword), and a Discord link shown is masked again (hideDiscordLink).
pub fn on_leave(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    for e in [r.pw_cur.clone(), r.pw_new.clone(), r.pw_new2.clone()] {
        if !e.read(cx).value().is_empty() {
            e.update(cx, |s, cx| s.set_value("", window, cx));
        }
    }
    r.settings.pw_msg = None;
    r.settings.dc_shown = None;
}

/// A command whose answer matters: what it returns goes to `done`; a refusal is said in a toast.
pub(crate) fn ask(r: &mut Root, cmd: &str, args: Value, cx: &mut Context<Root>, done: impl FnOnce(&mut Root, Value) + 'static) {
    let Some(link) = r.link.clone() else { return };
    let rx = link.call(cmd, args);
    cx.spawn(async move |this, cx| {
        let answer = rx.await;
        let _ = this.update(cx, |r, cx| {
            match answer {
                Ok(Ok(v)) => done(r, v),
                Ok(Err(e)) => r.toast = Some((e, "bad".into())),
                Err(_) => {}
            }
            cx.notify();
        });
    })
    .detach();
}
fn ok(r: &mut Root, text: String) {
    r.toast = Some((text, "ok".into()));
}

/// A setting sent as it is (setSettings), the app's next state shows it.
fn set(r: &mut Root, v: Value, cx: &mut Context<Root>) {
    ask(r, "setSettings", v, cx, |_, _| {});
}

// ───────────────────────── parts of a page ─────────────────────────

/// Rows in one card, under their section's head (kit::section_head) when it has one; a hairline between rows.
fn group(title: Option<String>, rows: Vec<AnyElement>) -> Div {
    // (While searched: its name is noted, for the search to find a part by the name of one of its groups.)
    if let Some(t) = &title {
        WORDS.with(|w| {
            if let Some(w) = w.borrow_mut().as_mut() {
                w.push(with_en(t.clone(), &[t]));
            }
        });
        TITLES.with(|l| {
            if let Some(l) = l.borrow_mut().as_mut() {
                l.push(t.clone());
            }
        });
    }
    div().flex().flex_col().gap(px(8.)).when_some(title, |d, t| d.child(section_head(t, None))).child(rows_card(rows))
}

/// A group's rows: no card round them any more, a hairline over each, flush with the heads' left edge (critique
/// 2026-11-13, P2: every page was the same stack of rounded cards; Settings reads as a list now, as a settings page is).
fn rows_card(rows: Vec<AnyElement>) -> Div {
    let mut card = div().flex().flex_col();
    for r in rows {
        card = card.child(div().border_t_1().border_color(c(LINE)).child(r));
    }
    card
}

thread_local! {
    /// While Settings is searched: the words (folded), whether the part being built matched by its name, and the rows
    /// that match, kept as they are (their switches still work) to be shown by part.
    static FIND: std::cell::RefCell<Option<(String, bool, Vec<AnyElement>)>> = const { std::cell::RefCell::new(None) };
    /// While searched in Vietnamese: each text's English words, so "proxy" or "backup" finds its row too.
    static EN: std::cell::RefCell<Option<std::rc::Rc<std::collections::HashMap<String, String>>>> = const { std::cell::RefCell::new(None) };
    /// That map, made once (the words don't change while the app runs): the search used to make it again every frame.
    static EN_MADE: std::cell::RefCell<Option<std::rc::Rc<std::collections::HashMap<String, String>>>> = const { std::cell::RefCell::new(None) };
    /// While searched: the names of the groups the part being built has.
    static TITLES: std::cell::RefCell<Option<Vec<String>>> = const { std::cell::RefCell::new(None) };
    /// While the palette reads the parts: every row's words and group's name of the part being built.
    static WORDS: std::cell::RefCell<Option<Vec<String>>> = const { std::cell::RefCell::new(None) };
}

/// Each part's rows and group names, for the palette to find a part by one of its settings (app.js's palette read
/// every Settings page's row and group titles). Built as the search builds them; nothing is shown.
pub fn part_words(root: &Root, cx: &mut Context<Root>) -> Vec<(Part, Vec<String>)> {
    EN.with(|m| *m.borrow_mut() = en_of(root));
    let out = Part::ALL
        .into_iter()
        .map(|p| {
            // (A query no row has: every row is read, none is kept.)
            FIND.with(|f| *f.borrow_mut() = Some(("\u{1}".into(), false, Vec::new())));
            WORDS.with(|w| *w.borrow_mut() = Some(Vec::new()));
            let _ = build_part(root, p, cx);
            FIND.with(|f| f.borrow_mut().take());
            (p, WORDS.with(|w| w.borrow_mut().take()).unwrap_or_default())
        })
        .collect();
    EN.with(|m| m.borrow_mut().take());
    out
}

fn build_part(root: &Root, p: Part, cx: &mut Context<Root>) -> Vec<Div> {
    match p {
        Part::Launch => launch(root, cx),
        Part::Roblox => roblox::roblox(root, cx),
        Part::Perf => perf(root, cx),
        Part::Notify => notify(root, cx),
        Part::Discord => discord::discord(root, cx),
        Part::Appearance => appearance::appearance(root, cx),
        Part::Security => security(root, cx),
        Part::Backup => backup(root, cx),
        Part::Updates => updates(root, cx),
    }
}

/// Whether the parts are being built for the search box (every row read, not shown as built).
pub(super) fn finding() -> bool {
    FIND.with(|f| f.borrow().is_some())
}

/// A setting: its name and what it does on the left, its control on the right.
fn row(title: impl Into<SharedString>, desc: Option<String>, ctl: impl IntoElement) -> AnyElement {
    let title: SharedString = title.into();
    row_el(title.to_string(), title, desc, ctl)
}

/// A row whose name is more than words (a badge beside it, a path under it): `words` is what the search reads.
fn row_el(words: String, title: impl IntoElement, desc: Option<String>, ctl: impl IntoElement) -> AnyElement {
    let words = with_en(format!("{words} {}", desc.as_deref().unwrap_or_default()), &[&words, desc.as_deref().unwrap_or_default()]);
    let el = div()
        .px(px(ROW_X))
        .py(px(12.))
        .min_h(px(58.))
        .flex()
        .items_center()
        .gap(px(32.))
        .child(row_words(title, desc))
        .child(div().flex_none().flex().items_center().justify_end().gap(px(8.)).child(ctl))
        .into_any_element();
    found(&words, el)
}

/// A row's sides: none, the rows stand flush with the group's head (they sat 20 px into a card).
const ROW_X: f32 = 0.;

/// A row's name (14 px, medium) and what it does under it in muted.
fn row_words(title: impl IntoElement, desc: Option<String>) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(3.))
        .child(div().text_size(px(T_L)).line_height(px(20.)).font_weight(FontWeight::MEDIUM).text_color(c(TEXT)).child(title))
        .when_some(desc, |d, t| d.child(div().text_size(px(T_M)).line_height(px(18.)).text_color(c(MUTED)).child(t)))
}

/// A setting whose control is too wide to sit beside its name (the layouts as pictures): the control under the words,
/// from the same left edge.
fn row_wide(title: impl Into<SharedString>, desc: Option<String>, ctl: impl IntoElement) -> AnyElement {
    let title: SharedString = title.into();
    let words = with_en(format!("{title} {}", desc.as_deref().unwrap_or_default()), &[&title, desc.as_deref().unwrap_or_default()]);
    let el = div().px(px(ROW_X)).pt(px(14.)).pb(px(18.)).flex().flex_col().gap(px(14.)).child(row_words(title, desc)).child(ctl).into_any_element();
    found(&words, el)
}

/// Each Vietnamese text's English (I18n::en_by_vi) while the page is in Vietnamese, made the first time it is asked.
fn en_of(root: &Root) -> Option<std::rc::Rc<std::collections::HashMap<String, String>>> {
    if !root.i18n.vietnamese {
        return None;
    }
    Some(EN_MADE.with(|m| m.borrow_mut().get_or_insert_with(|| std::rc::Rc::new(root.i18n.en_by_vi())).clone()))
}

/// A row's words, and while searched in Vietnamese the English of its name and description too.
fn with_en(words: String, parts: &[&str]) -> String {
    EN.with(|m| match m.borrow().as_ref() {
        Some(m) => {
            let en: Vec<&str> = parts.iter().filter_map(|p| m.get(*p)).map(String::as_str).collect();
            if en.is_empty() { words } else { format!("{words} {}", en.join(" ")) }
        }
        None => words,
    })
}

/// A built row as the search sees it: shown as is with no search; with one, kept for the results when its words (or
/// its part's name) match, and an empty spot in the part's own card either way.
pub(super) fn found(words: &str, el: AnyElement) -> AnyElement {
    WORDS.with(|w| {
        if let Some(w) = w.borrow_mut().as_mut() {
            w.push(words.to_string());
        }
    });
    let hit = FIND.with(|f| f.borrow().as_ref().map(|(q, all, _)| *all || folded(words).contains(q.as_str())));
    match hit {
        None => el,
        Some(true) => {
            FIND.with(|f| {
                if let Some((_, _, found)) = f.borrow_mut().as_mut() {
                    found.push(el);
                }
            });
            div().into_any_element()
        }
        Some(false) => div().into_any_element(),
    }
}

/// A setting whose choice is a menu or a short switch of words: since the menus are compact, the same as a row.
fn row_below(title: impl Into<SharedString>, desc: Option<String>, ctl: impl IntoElement) -> AnyElement {
    row(title, desc, ctl)
}

/// A line of its own in a card: a note, a path, a question.
fn note(icon_name: &'static str, text: String, tone: u32) -> AnyElement {
    div()
        .px(px(ROW_X))
        .py(px(11.))
        .flex()
        .items_start()
        .gap(px(10.))
        .text_size(px(T_S))
        .text_color(c(TEXT_2))
        .child(div().pt(px(1.)).child(icon(icon_name, 14., c(tone))))
        .child(div().flex_1().min_w_0().child(text))
        .into_any_element()
}

fn switch(id: &'static str, on: bool, cx: &mut Context<Root>, f: impl Fn(&mut Root, bool, &mut Context<Root>) + 'static) -> Switch {
    let this = cx.entity().downgrade();
    let f = std::rc::Rc::new(f);
    Switch::new(id, on).on_toggle(move |v, _, cx| {
        let v = *v;
        let f = f.clone();
        let _ = this.update(cx, |r, cx| f(r, v, cx));
    })
}

/// A choice: up to three short words side by side (one chosen), more as a menu under a compact button. `off` ones are
/// dimmed and can't be picked.
fn chips<T: Clone + PartialEq + 'static>(
    root: &Root,
    cx: &mut Context<Root>,
    key: &'static str,
    items: Vec<(T, String, bool)>,
    current: &T,
    f: impl Fn(&mut Root, T, &mut Context<Root>) + 'static,
) -> Div {
    chips_with(root, cx, key, items, current, move |r, v, _, cx| f(r, v, cx))
}

/// The same, for a choice that needs the window (to fill or focus a box).
fn chips_with<T: Clone + PartialEq + 'static>(
    root: &Root,
    cx: &mut Context<Root>,
    key: &'static str,
    items: Vec<(T, String, bool)>,
    current: &T,
    f: impl Fn(&mut Root, T, &mut Window, &mut Context<Root>) + 'static,
) -> Div {
    chips_marked(root, cx, key, items, Vec::new(), current, f)
}

/// What stands before a choice in a menu (and on its button when chosen): one of the app's icons, or a letter for a
/// named program that has no icon of its own here (Bloxstrap's B).
#[derive(Clone, Copy)]
enum Mark {
    Icon(&'static str),
    Letter(char),
}

fn mark(m: Mark, on: bool) -> Div {
    let tone = if on { c(TEXT) } else { c(MUTED) };
    // The mark alone, no box round it: a box on every choice read as a row of stickers.
    let tile = div().min_w(px(14.)).flex_none().flex().items_center().justify_center();
    match m {
        Mark::Icon(name) => tile.child(icon(name, 14., tone)),
        Mark::Letter(l) => tile.text_size(px(T_XS)).font_weight(FontWeight::SEMIBOLD).text_color(tone).child(l.to_string()),
    }
}

/// The same, with a mark before each choice (`marks` in the same order; empty for none).
fn chips_marked<T: Clone + PartialEq + 'static>(
    root: &Root,
    cx: &mut Context<Root>,
    key: &'static str,
    items: Vec<(T, String, bool)>,
    marks: Vec<Mark>,
    current: &T,
    f: impl Fn(&mut Root, T, &mut Window, &mut Context<Root>) + 'static,
) -> Div {
    let f = std::rc::Rc::new(f);
    if items.len() <= 3 && marks.is_empty() {
        let mut track = div().flex().flex_none().gap(px(2.)).p(px(3.)).rounded(px(R_M)).bg(c(INSET)).border_1().border_color(c(LINE));
        for (i, (value, label, off)) in items.into_iter().enumerate() {
            let on = value == *current;
            let f = f.clone();
            track = track.child(
                segment((key, i), on)
                    .h(px(26.))
                    .px(px(12.))
                    .text_size(px(T_M))
                    .whitespace_nowrap()
                    .when(off, |d| d.opacity(0.4))
                    .child(label)
                    .when(!off && !on, |d| d.on_click(cx.listener(move |r, _, w, cx| f(r, value.clone(), w, cx)))),
            );
        }
        return track;
    }
    let open = root.settings.menu == Some(key);
    let label = items.iter().find(|(v, _, _)| v == current).map(|(_, l, _)| l.clone()).unwrap_or_else(|| "—".into());
    let chosen_mark = items.iter().position(|(v, _, _)| v == current).and_then(|i| marks.get(i).copied());
    let mut wrap = div().relative().flex_none().child(
        hot(key)
            .h(px(34.))
            .min_w(px(132.))
            .max_w(px(260.))
            .pl(px(12.))
            .pr(px(9.))
            .flex()
            .items_center()
            .gap(px(10.))
            .rounded(px(R_M))
            .border_1()
            .map(|d| if open { d.tones(c(RAISED_2), c(RAISED_2)).edge(ca(WHITE, 0.22), ca(WHITE, 0.22)) } else { d.tones(c(RAISED), c(RAISED_2)).edge(ca(WHITE, 0.08), ca(WHITE, 0.17)) })
            .cursor_pointer()
            .text_size(px(T_M))
            .text_color(c(TEXT))
            .when_some(chosen_mark, |d, m| d.pl(px(8.)).child(mark(m, true)))
            .child(div().flex_1().min_w_0().truncate().child(label))
            .child(icon(if open { "chevronUp" } else { "chevronDown" }, 14., c(MUTED)))
            .on_click(cx.listener(move |r, _, _, cx| {
                // A click outside the open menu closed it just now: this click on its own button only closes it.
                let just = r.settings.menu_closed.is_some_and(|(k, at)| k == key && at.elapsed().as_millis() < 300);
                r.settings.menu_closed = None;
                r.settings.menu = if just || r.settings.menu == Some(key) { None } else { Some(key) };
                r.settings.menu_was = Some(key);
                cx.notify();
            })),
    );
    if open || root.settings.menu_was == Some(key) {
        let mut menu = div()
            .id(SharedString::from(format!("{key}-menu")))
            .occlude()
            .min_w(px(200.))
            .max_h(px(320.))
            .overflow_y_scroll()
            .p(px(4.))
            .rounded(px(R_M))
            .bg(c(RAISED))
            .border_1()
            .border_color(c(LINE))
            .shadow_lg()
            .flex()
            .flex_col()
            .on_mouse_down_out(cx.listener(move |r, _, _, cx| {
                if r.settings.menu == Some(key) {
                    r.settings.menu = None;
                    r.settings.menu_closed = Some((key, std::time::Instant::now()));
                    cx.notify();
                }
            }));
        for (i, (value, label, off)) in items.into_iter().enumerate() {
            let on = value == *current;
            let f = f.clone();
            let m = marks.get(i).copied();
            menu = menu.child(
                menu_row((key, 1000 + i))
                    .text_color(if on { c(TEXT) } else { c(TEXT_2) })
                    .when(off, |d| d.opacity(0.4))
                    .when_some(m, |d, m| d.gap(px(10.)).child(mark(m, on)))
                    .child(div().min_w_0().flex_1().whitespace_nowrap().child(label))
                    .child(div().w(px(14.)).flex_none().when(on, |d| d.child(icon("check", 14., c(TEXT)))))
                    .when(!off, |d| {
                        d.on_click(cx.listener(move |r, _, w, cx| {
                            r.settings.menu = None;
                            if !on {
                                f(r, value.clone(), w, cx);
                            }
                            cx.notify();
                        }))
                    }),
            );
        }
        wrap = wrap.child(div().absolute().top(px(38.)).right_0().child(deferred(anchored().anchor(Anchor::TopRight).snap_to_window_with_margin(px(8.)).child(pop(SharedString::from(format!("{key}-pop")), open, menu))).with_priority(1)));
    }
    wrap
}

/// "9/11" in Vietnamese, "11/9" in English.
fn civil_dm(days: i64, vi: bool) -> String {
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    if vi { format!("{d}/{m}") } else { format!("{m}/{d}") }
}

/// A day since 1970 as (year, month, day).
fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + if m <= 2 { 1 } else { 0 }, m, d)
}

/// Words under the mouse, when there are any.
fn tip(d: Stateful<Div>, t: Option<String>) -> Stateful<Div> {
    match t {
        Some(t) => {
            let t = SharedString::from(t);
            d.tooltip(move |_, cx| cx.new(|_| Tip(t.clone())).into())
        }
        None => d,
    }
}

/// A question asked in place under its row: what it does, Cancel, and the answer that goes ahead.
#[allow(clippy::too_many_arguments)]
fn question(
    root: &Root,
    key: &'static str,
    title: String,
    text: String,
    ok_label: String,
    danger: bool,
    cx: &mut Context<Root>,
    f: impl Fn(&mut Root, &mut Context<Root>) + 'static,
) -> AnyElement {
    let go = Button::new((key, 1usize), ok_label).small();
    let go = if danger { go.danger() } else { go.primary() };
    div()
        .px(px(ROW_X))
        .py(px(12.))
        .flex()
        .flex_col()
        .gap(px(10.))
        .bg(ca(WARN, 0.06))
        .child(div().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).child(title))
        .child(div().text_size(px(T_S)).text_color(c(TEXT_2)).child(crate::i18n::plain(&text)))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(div().flex_1())
                .child(Button::new((key, 0usize), root.tr("common.cancel")).small().on_click(cx.listener(|r, _, _, cx| {
                    r.settings.asking = None;
                    cx.notify();
                })))
                .child(go.on_click(cx.listener(move |r, _, _, cx| {
                    r.settings.asking = None;
                    f(r, cx);
                    cx.notify();
                }))),
        )
        .into_any_element()
}

/// A list to tick in place under its row (accounts, parts of a message): Done saves it, Cancel leaves it.
#[allow(clippy::too_many_arguments)]
fn picker(
    root: &Root,
    key: &'static str,
    text: String,
    items: Vec<(String, String, String)>,
    ticked: &HashSet<String>,
    cx: &mut Context<Root>,
    toggle: impl Fn(&mut Root, String, bool) + 'static,
    extra: Vec<AnyElement>,
    done: impl Fn(&mut Root, &mut Context<Root>) + 'static,
    cancel: impl Fn(&mut Root) + 'static,
) -> AnyElement {
    let toggle = std::rc::Rc::new(toggle);
    let mut list = div().flex().flex_wrap().gap_y(px(4.));
    for (i, (id, label, sub)) in items.into_iter().enumerate() {
        let toggle = toggle.clone();
        let this = cx.entity().downgrade();
        list = list.child(
            div()
                .w(px(220.))
                .flex()
                .items_center()
                .gap(px(6.))
                .child(Check::new((key, i), ticked.contains(&id)).label(label).on_toggle(move |v, _, cx| {
                    let v = *v;
                    let id = id.clone();
                    let toggle = toggle.clone();
                    let _ = this.update(cx, |r, cx| {
                        toggle(r, id, v);
                        cx.notify();
                    });
                }))
                .when(!sub.is_empty(), |d| d.child(div().text_size(px(T_S)).text_color(c(MUTED)).child(sub))),
        );
    }
    div()
        .px(px(ROW_X))
        .py(px(12.))
        .flex()
        .flex_col()
        .gap(px(10.))
        .bg(ca(WHITE, 0.02))
        .child(div().text_size(px(T_S)).text_color(c(TEXT_2)).child(text))
        .child(list)
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .children(extra)
                .child(div().flex_1())
                .child(Button::new((key, 10_000usize), root.tr("common.cancel")).small().on_click(cx.listener(move |r, _, _, cx| {
                    cancel(r);
                    cx.notify();
                })))
                .child(Button::new((key, 10_001usize), root.tr("common.done")).small().primary().on_click(cx.listener(move |r, _, _, cx| done(r, cx)))),
        )
        .into_any_element()
}

fn path_text(p: &str) -> Div {
    div().text_size(px(T_S)).text_color(c(TEXT_2)).child(p.to_string())
}

// ───────────────────────── the page ─────────────────────────

/// A part picked in the nav (or by its name over search results): the search is emptied, what was half done in the
/// part being left is dropped, and the page goes back to its top.
fn open_part(r: &mut Root, p: Part, window: &mut Window, cx: &mut Context<Root>) {
    let searched = !r.set_q.read(cx).value().is_empty();
    if searched {
        r.set_q.update(cx, |s, cx| s.set_value("", window, cx));
    }
    if searched || r.settings.part != p {
        r.settings.part = p;
        r.settings.asking = None;
        r.settings.rc_pick = None;
        // Leaving Discord masks its link again and drops what was half done there.
        r.settings.dc_shown = None;
        r.settings.dc_link = None;
        r.settings.dc_pick = None;
        r.settings.dc_ping = None;
        r.settings.menu = None;
        if p == Part::Discord {
            discord::opened(r, window, cx);
        }
        // Appearance's Popular comes from the community's sums: with the online count off the app doesn't hear them
        // by itself, so they are asked for (the app asks Roblox's bot at most once a minute, and not with the count on).
        if p == Part::Appearance {
            r.send("communityNow", json!({}));
        }
        r.settings.scroll.set_offset(point(px(0.), px(0.)));
        cx.notify();
    }
}

/// The quiet line under the page's title: Multi-Roblox, Discord and the last backup at a glance; what needs a look in
/// its warning colour.
fn summary(root: &Root) -> Vec<(String, Option<Hsla>)> {
    let mut out = Vec::new();
    let multi = root.state.pointer("/multi/enabled").and_then(Value::as_bool).unwrap_or(false);
    let armed = root.state.pointer("/multi/armed").and_then(Value::as_bool).unwrap_or(false);
    out.push(match (multi, armed) {
        (false, _) => (root.tr("set.sum.multiOff"), None),
        (true, true) => (root.tr("set.sum.multiOn"), None),
        (true, false) => (root.tr("set.sum.multiBlocked"), Some(c(WARN))),
    });
    let d = root.state.get("discord").unwrap_or(&Value::Null);
    let stopped = !s(d, "stopped").is_empty() || !s(d, "error").is_empty();
    let state = if !b(d, "hasWebhook") {
        "set.dc.state.none"
    } else if b(d, "enabled") {
        "set.dc.state.on"
    } else {
        "set.dc.state.off"
    };
    out.push((format!("Discord: {}", root.tr(state)), stopped.then(|| c(WARN))));
    let bk = root.state.get("backup").unwrap_or(&Value::Null);
    if !s(bk, "folder").is_empty() {
        let last = bk.get("lastBackup").and_then(Value::as_str).and_then(crate::time::iso_secs);
        let text = match last {
            Some(at) => root.i18n.tv("bk.last", &[("t", &crate::time::ago(crate::time::now_secs() - at, &root.i18n))]),
            None => root.tr("bk.never"),
        };
        out.push((text, (!s(bk, "error").is_empty()).then(|| c(WARN))));
    }
    out
}

pub fn settings(root: &Root, width: f32, cx: &mut Context<Root>) -> AnyElement {
    let part = root.settings.part;
    // A dot on a part when something there needs a look (app.js's syncSettingsNav): Roblox closed windows to update,
    // or its new build may have broken Multi-Roblox; Discord stopped sending.
    let said = |path: &str| root.state.pointer(path).is_some_and(|v| !(v.is_null() || v == &Value::Bool(false) || v.as_i64() == Some(0) || v.as_array().is_some_and(|l| l.is_empty()) || v.as_str() == Some("")));
    let watch_warns = {
        let check = root.state.pointer("/robloxUpdates/watch/check").and_then(Value::as_str).unwrap_or_default();
        ["changed", "renamed", "gone"].contains(&check) && !said("/robloxUpdates/watch/worked")
    };
    let needs_look = |p: Part| match p {
        Part::Roblox => said("/robloxUpdates/closed") || (said("/multi/enabled") && watch_warns),
        Part::Discord => said("/discord/stopped"),
        _ => false,
    };
    let narrow = width < 1100.;
    let nav_w = if narrow { 208. } else { 232. };
    // The parts in three groups under a quiet label each; the open one is the yellow block.
    let mut nav = div().w(px(nav_w)).flex_none().flex().flex_col().gap(px(2.)).pl(px(16.)).pr(px(12.)).pb(px(24.));
    for (i, p) in Part::ALL.into_iter().enumerate() {
        let on = p == part && root.set_q.read(cx).value().trim().is_empty();
        if let Some(g) = p.group_key() {
            nav = nav.child(
                div()
                    .when(i > 0, |d| d.mt(px(18.)))
                    .mb(px(4.))
                    .px(px(12.))
                    .text_size(px(T_S))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(c(MUTED))
                    .child(root.tr(g)),
            );
        }
        let look = needs_look(p);
        nav = nav.child(
            segment(("set-nav", i), on)
                .h(px(36.))
                .px(px(12.))
                .rounded(px(R_M))
                .gap(px(11.))
                .text_size(px(T_L))
                .font_weight(if on { FontWeight::SEMIBOLD } else { FontWeight::MEDIUM })
                .child(icon(p.icon(), 16., if on { c(INK) } else { c(MUTED) }))
                .child(div().flex_1().min_w_0().truncate().child(root.tr(p.title())))
                .when(look, |d| {
                    // (Shell.tsx's dot says what it is under the mouse.)
                    let tip: SharedString = root.tr("set.needsLook").into();
                    d.child(div().id(("set-look", i)).size(px(7.)).flex_none().rounded_full().bg(if on { c(INK) } else { c(WARN) }).tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into()))
                })
                .when(!on, |d| d.on_click(cx.listener(move |r, _, window, cx| open_part(r, p, window, cx)))),
        );
    }
    let build = |p: Part, cx: &mut Context<Root>| build_part(root, p, cx);
    // Words in the search box: every part is built, and only its rows that match are shown, under the part's name (a
    // part whose name matches shows all its rows); Open goes to that part.
    let q = folded(root.set_q.read(cx).value().trim());
    let searching = !q.is_empty();
    // The English words a search set up go once it is cleared: left, every later frame looked each row up in them.
    if !searching {
        EN.with(|m| m.borrow_mut().take());
    }
    let mut hits = 0usize;
    let body: Vec<Div> = if searching {
        let mut out = Vec::new();
        EN.with(|m| *m.borrow_mut() = en_of(root));
        for p in Part::ALL {
            let all = folded(&root.tr(p.title())).contains(q.as_str()) || folded(&root.i18n.en(p.title())).contains(q.as_str());
            FIND.with(|f| *f.borrow_mut() = Some((q.clone(), all, Vec::new())));
            TITLES.with(|l| *l.borrow_mut() = Some(Vec::new()));
            let _ = build(p, cx);
            let mut found = FIND.with(|f| f.borrow_mut().take()).map(|(_, _, v)| v).unwrap_or_default();
            // A group's name that matches (app.js's set-grp-title, in either language) shows its rows: the part is
            // built again with all its rows.
            let titles = TITLES.with(|l| l.borrow_mut().take()).unwrap_or_default();
            if !all && titles.iter().any(|t| folded(&with_en(t.clone(), &[t])).contains(q.as_str())) {
                FIND.with(|f| *f.borrow_mut() = Some((q.clone(), true, Vec::new())));
                let _ = build(p, cx);
                found = FIND.with(|f| f.borrow_mut().take()).map(|(_, _, v)| v).unwrap_or_default();
            }
            if found.is_empty() {
                continue;
            }
            hits += found.len();
            let head = section_head(root.tr(p.title()), Some(found.len())).child(
                Button::new(SharedString::from(format!("set-found-{}", p.key())), root.tr("set.openPart"))
                    .icon_end("chevronRight")
                    .small()
                    .quiet()
                    .on_click(cx.listener(move |r, _, window, cx| open_part(r, p, window, cx))),
            );
            out.push(div().flex().flex_col().gap(px(8.)).child(head).child(rows_card(found)));
        }
        if out.is_empty() {
            out.push(
                surface()
                    .py(px(40.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(12.))
                    .child(div().size(px(40.)).rounded(px(R_M)).bg(c(RAISED_2)).flex().items_center().justify_center().child(icon("search", 18., c(MUTED))))
                    .child(div().text_size(px(T_L)).text_color(c(TEXT_2)).child(root.tr("set.noMatch"))),
            );
        }
        out
    } else {
        build(part, cx)
    };
    // BLOXKEEP_SHOT_SCROLL=<px>: a shot of the page further down, once its content has been laid out.
    #[cfg(feature = "shot")]
    {
        use std::sync::atomic::{AtomicBool, Ordering};
        static DONE: AtomicBool = AtomicBool::new(false);
        if let Some(y) = std::env::var("BLOXKEEP_SHOT_SCROLL").ok().and_then(|v| v.parse::<f32>().ok()) {
            let h = &root.settings.scroll;
            if !DONE.load(Ordering::Relaxed) && h.max_offset().y > px(0.) {
                DONE.store(true, Ordering::Relaxed);
                h.set_offset(point(px(0.), -px(y).min(h.max_offset().y)));
                cx.notify();
            }
        }
    }
    // The part's own head: its name, and one line on what it controls (with a search: how many rows matched).
    let (part_title, part_about) = if searching {
        (root.tr("set.search"), (hits > 0).then(|| root.i18n.tn("set.found", hits as i64, &[])))
    } else {
        (root.tr(part.title()), Some(root.tr(&part.about())))
    };
    let part_head = div()
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(div().text_size(px(T_FIG)).line_height(px(30.)).font_weight(FontWeight::BOLD).child(part_title))
        .when_some(part_about, |d, t| d.child(div().text_size(px(T_L)).line_height(px(20.)).text_color(c(TEXT_2)).child(t)));
    let header = div()
        .flex_none()
        .px(px(24.))
        .pt(px(22.))
        .pb(px(22.))
        .flex()
        .items_center()
        .gap(px(16.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(div().text_size(px(T_TITLE)).line_height(px(38.)).font_weight(FontWeight::BOLD).child(root.tr("set.title")))
                .child(facts(summary(root))),
        )
        .child(div().w(px(if narrow { 240. } else { 300. })).min_w(px(160.)).flex_shrink(1.).child(field(&root.set_q, Some("search"))));
    div()
        .id("settings")
        .size_full()
        .rounded(px(R_PANEL))
        .bg(c(PANEL))
        .overflow_hidden()
        .relative()
        .flex()
        .flex_col()
        .child(header)
        .child(
            div().flex_1().min_h_0().flex().child(div().id("set-nav").h_full().flex_none().overflow_y_scroll().child(nav)).child(
                // (The list stops a little above the panel's foot, so a row cut there reads as going on under the
                // edge, not as touching the status bar: the shots had "Wait between accounts" cut flush at the foot.)
                div()
                    .flex_1()
                    .min_w_0()
                    .relative()
                    .flex()
                    .flex_col()
                    .pb(px(16.))
                    .child(
                        div().id("set-body").flex_1().min_h_0().w_full().overflow_y_scroll().track_scroll(&root.settings.scroll).child(
                            div()
                                // As wide as the room: the cards end where the header's search ends (a cap left them
                                // short of it on a wide window, two right edges).
                                .pl(px(16.))
                                .pr(px(24.))
                                .pb(px(40.))
                                .flex()
                                .flex_col()
                                .gap(px(SECTION_GAP))
                                .child(part_head)
                                .children(body),
                        ),
                    )
                    // A scroll bar while the page is longer than the window, so a row cut at the foot reads as "more below".
                    .child(crate::pages::auto::scroll_bar("set-bar", &root.settings.scroll).bottom(px(16.))),
            ),
        )
        .into_any_element()
}

// ───────────────────────── Launching ─────────────────────────

const SIZES: [(i64, i64); 15] = [
    (1920, 1080),
    (1600, 900),
    (1280, 720),
    (1024, 576),
    (960, 540),
    (854, 480),
    (640, 360),
    (480, 270),
    (320, 180),
    (1024, 768),
    (800, 600),
    (640, 480),
    (480, 360),
    (400, 300),
    (320, 240),
];
const DELAYS: [i64; 12] = [0, 3, 5, 8, 10, 15, 20, 30, 45, 60, 90, 120];

fn launch(root: &Root, cx: &mut Context<Root>) -> Vec<Div> {
    let st = st(root);
    let m = root.state.get("multi").unwrap_or(&Value::Null);
    let ins = root.state.get("installs").unwrap_or(&Value::Null);
    let multi_on = b(m, "enabled");
    let (pill, tone) = if !multi_on {
        ("pill.off", Tone::Muted)
    } else if b(m, "armed") {
        ("pill.ready", Tone::Ok)
    } else {
        ("pill.notReady", Tone::Warn)
    };
    let asking = root.settings.asking == Some("multi");
    let skip = root.state.get("skipConfirm").and_then(Value::as_array).is_some_and(|l| l.iter().any(|k| k.as_str() == Some("multi")));
    let multi = switch("s-multi", multi_on || asking, cx, move |r, on, cx| {
        if on && !skip {
            r.settings.asking = Some("multi");
            r.settings.no_ask = false;
            cx.notify();
            return;
        }
        r.settings.asking = None;
        set(r, json!({ "multiInstance": on }), cx);
    });
    let mut windows = vec![row_el(
        root.tr("set.multi"),
        div().flex().items_center().gap(px(8.)).child(root.tr("set.multi")).child(badge(root.tr(pill), tone)),
        Some(root.tr("set.multiDesc")),
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(Button::new("s-multi-help", root.tr("set.details")).small().quiet().on_click(cx.listener(|r, _, window, cx| r.multi_help(window, cx))))
            .child(multi),
    )];
    if asking {
        let no_ask = root.settings.no_ask;
        windows.push(
            div()
                .px(px(ROW_X))
                .py(px(12.))
                .flex()
                .flex_col()
                .gap(px(10.))
                .bg(ca(WARN, 0.06))
                .child(div().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).child(root.tr("set.multiConfirmTitle")))
                .child(div().text_size(px(T_S)).text_color(c(TEXT_2)).child(crate::i18n::plain(&root.tr("set.multiConfirm"))))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(Check::new("s-multi-skip", no_ask).label(root.tr("common.dontAsk")).on_toggle({
                            let this = cx.entity().downgrade();
                            move |v, _, cx| {
                                let v = *v;
                                let _ = this.update(cx, |r, cx| {
                                    r.settings.no_ask = v;
                                    cx.notify();
                                });
                            }
                        }))
                        .child(div().flex_1())
                        .child(Button::new("s-multi-no", root.tr("common.cancel")).small().on_click(cx.listener(|r, _, _, cx| {
                            r.settings.asking = None;
                            cx.notify();
                        })))
                        .child(Button::new("s-multi-ok", root.tr("set.multiOk")).small().primary().on_click(cx.listener(|r, _, _, cx| {
                            r.settings.asking = None;
                            if r.settings.no_ask {
                                r.send("skipConfirm", json!({ "key": "multi" }));
                            }
                            set(r, json!({ "multiInstance": true }), cx);
                        }))),
                )
                .into_any_element(),
        );
    }
    let rename = b(st, "renameWindows");
    windows.push(row(root.tr("set.rename"), Some(root.tr("set.renameDesc")), switch("s-rename", rename, cx, |r, on, cx| set(r, json!({ "renameWindows": on }), cx))));
    let pictures = st.get("windowPictures").and_then(Value::as_bool).unwrap_or(rename);
    windows.push(row(root.tr("set.pictures"), Some(root.tr("set.picturesDesc")), switch("s-pictures", pictures, cx, |r, on, cx| set(r, json!({ "windowPictures": on }), cx))));
    let taskbar = Some(s(st, "taskbarGroups")).filter(|x| !x.is_empty()).unwrap_or("roblox").to_string();
    windows.push(row_below(
        root.tr("set.taskbar"),
        Some(root.tr("set.taskbarDesc")),
        chips(
            root,
            cx,
            "s-taskbar",
            ["roblox", "account", "preset"].iter().map(|k| (k.to_string(), root.tr(&format!("set.taskbar.{k}")), false)).collect(),
            &taskbar,
            |r, v, cx| set(r, json!({ "taskbarGroups": v }), cx),
        ),
    ));
    let open_as = if b(m, "tile") { "tile" } else if b(st, "maximizeOnOpen") { "maximize" } else { "roblox" }.to_string();
    windows.push(row_below(
        root.tr("set.openAs"),
        Some(root.tr("set.openAsDesc")),
        chips(
            root,
            cx,
            "s-openas",
            ["roblox", "maximize", "tile"].iter().map(|k| (k.to_string(), root.tr(&format!("set.openAs.{k}")), false)).collect(),
            &open_as,
            |r, v, cx| {
                let args = match v.as_str() {
                    "tile" => json!({ "multiTile": true }),
                    "maximize" => json!({ "maximizeOnOpen": true }),
                    _ => json!({ "multiTile": false, "maximizeOnOpen": false }),
                };
                set(r, args, cx)
            },
        ),
    ));
    let layout = Some(s(st, "windowLayout")).filter(|x| !x.is_empty()).unwrap_or("fill").to_string();
    let (w, h) = (st.get("windowWidth").and_then(Value::as_i64).unwrap_or(640), st.get("windowHeight").and_then(Value::as_i64).unwrap_or(480));
    // The layouts as small screen pictures (pages/snap.rs), as the web page's Settings had them.
    let pick: crate::pages::snap::Pick = std::rc::Rc::new(|r: &mut Root, v: &'static str, _: &mut Window, cx: &mut Context<Root>| set(r, json!({ "windowLayout": v }), cx));
    let mut layout_ctl = div().flex().flex_col().items_start().gap(px(12.)).child(crate::pages::snap::picker(root, "s-layout", &layout, w, h, 12, 74., pick, cx));
    if layout == "custom" {
        let mut sizes: Vec<(i64, i64)> = SIZES.to_vec();
        if !sizes.contains(&(w, h)) {
            sizes.insert(0, (w, h));
        }
        layout_ctl = layout_ctl.child(chips(
            root,
            cx,
            "s-size",
            sizes.into_iter().map(|(sw, sh)| ((sw, sh), format!("{sw} × {sh}"), false)).collect(),
            &(w, h),
            |r, (sw, sh), cx| set(r, json!({ "windowWidth": sw, "windowHeight": sh }), cx),
        ));
        // A size of one's own, typed; and every Roblox window laid out now.
        layout_ctl = layout_ctl.child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(8.))
                .text_size(px(T_M))
                .text_color(c(TEXT_2))
                .child(root.tr("snap.width"))
                .child(field(&root.win_w, None).w(px(84.)))
                .child(div().text_color(c(MUTED)).child("×"))
                .child(root.tr("snap.height"))
                .child(field(&root.win_h, None).w(px(84.)))
                .child(div().text_color(c(MUTED)).child("px"))
                .child(div().w(px(6.)))
                .child(Button::new("s-arrange", root.tr("m.arrange")).icon("screens").small().on_click(cx.listener(|r, _, _, cx| crate::pages::accounts::arrange(r, cx)))),
        );
    }
    windows.push(row_wide(root.tr("set.winLayout"), Some(root.tr("set.winLayoutDesc")), layout_ctl));

    // What opens Roblox: other Bloxstrap-style launchers only when found here (or still the one chosen); VNG always last.
    let has = |k: &str| b(ins, k);
    let mut options: Vec<(String, String, bool)> = vec![(
        "auto".into(),
        if s(ins, "handler").is_empty() { root.tr("launch.autoNone") } else { root.i18n.tv("launch.auto", &[("name", s(ins, "handler"))]) },
        true,
    )];
    for (k, n) in [("bloxstrap", "Bloxstrap"), ("fishstrap", "Fishstrap"), ("voidstrap", "Voidstrap")] {
        options.push((k.into(), n.into(), has(k)));
    }
    let via_saved = Some(s(st, "launchVia")).filter(|x| !x.is_empty()).unwrap_or("auto").to_string();
    let others: Vec<String> = ins.get("others").and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
    let mut other_names = others.clone();
    if let Some(n) = via_saved.strip_prefix("other:") {
        if !other_names.iter().any(|x| x == n) {
            other_names.push(n.to_string());
        }
    }
    for n in other_names {
        let ok = others.contains(&n);
        options.push((format!("other:{n}"), n, ok));
    }
    options.push(("roblox".into(), root.tr("launch.roblox"), has("roblox")));
    options.push(("direct".into(), root.tr("launch.direct"), has("direct")));
    if has("store") || via_saved == "store" {
        options.push(("store".into(), root.tr("launch.storeName"), has("store")));
    }
    let custom = s(st, "customLauncher");
    if !custom.is_empty() {
        options.push(("custom".into(), root.i18n.tv("launch.custom", &[("name", custom)]), true));
    }
    options.push(("vng".into(), "Roblox VNG".into(), has("vng")));
    let marks: Vec<Mark> = options
        .iter()
        .map(|(v, words, _)| match v.as_str() {
            "auto" => Mark::Icon("link"),
            "roblox" => Mark::Icon("roblox"),
            "direct" => Mark::Icon("play"),
            "store" => Mark::Icon("download"),
            "custom" => Mark::Icon("folder"),
            "vng" => Mark::Icon("globe"),
            _ => Mark::Letter(words.chars().find(|ch| ch.is_alphanumeric()).unwrap_or('?').to_ascii_uppercase()),
        })
        .collect();
    let options: Vec<(String, String, bool)> =
        options.into_iter().map(|(v, words, ok)| if ok { (v, words, false) } else { (v, root.i18n.tv("launch.missing", &[("name", &words)]), true) }).collect();
    let via = if options.iter().any(|o| o.0 == via_saved) { via_saved.clone() } else { "auto".into() };
    let store_note = if !has("store") {
        None
    } else if via_saved == "store" {
        Some(root.tr("launch.storeTest"))
    } else if !has("direct") {
        Some(root.tr("launch.storeOnly"))
    } else {
        None
    };
    let mut client = vec![row_below(
        root.tr("set.client"),
        Some(root.tr("set.clientDesc")),
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(chips_marked(root, cx, "s-launch", options, marks, &via, |r, v, _, cx| set(r, json!({ "launchVia": v }), cx)))
            // After the choices and in words ("Other launcher…"): a bare folder before them said nothing (review 7.3).
            .child(Button::new("s-pick-launcher", root.tr("launch.pickBtn")).icon("folder").small().quiet().on_click(cx.listener(|r, _, _, cx| {
                ask(r, "pickLauncher", json!({}), cx, |_, _| {})
            }))),
    )];
    if let Some(n) = store_note {
        client.push(note("info", n, TEXT_2));
    }
    let delay = st.get("launchDelaySeconds").and_then(Value::as_i64).unwrap_or(0);
    let mut delays: Vec<i64> = DELAYS.to_vec();
    if !delays.contains(&delay) {
        delays.push(delay);
        delays.sort_unstable();
    }
    client.push(row_below(
        root.tr("set.delay"),
        Some(root.tr("set.delayDesc")),
        chips(
            root,
            cx,
            "s-delay",
            delays.into_iter().map(|n| (n, root.i18n.tv("set.seconds", &[("n", &n.to_string())]), false)).collect(),
            &delay,
            |r, v, cx| set(r, json!({ "launchDelaySeconds": v }), cx),
        ),
    ));
    vec![group(Some(root.tr("set.g.windows")), windows), group(Some(root.tr("set.g.client")), client)]
}

// ───────────────────────── Staying in game ─────────────────────────

fn perf(root: &Root, cx: &mut Context<Root>) -> Vec<Div> {
    let st = st(root);
    let accounts = root.accounts();
    let total = accounts.len();
    let on_n = accounts.iter().filter(|a| b(a, "autoRejoin")).count();
    let all_on = total > 0 && on_n == total;
    let ids: Vec<String> = accounts.iter().map(|a| s(a, "id").to_string()).collect();
    let rejoin = switch("s-rejoin", all_on, cx, move |r, on, cx| {
        ask(r, "setAutoRejoin", json!({ "ids": ids.clone(), "enabled": on }), cx, move |r, v| {
            let n = v.as_i64().unwrap_or(0);
            let text = r.i18n.tn(if on { "t.rejoinOn" } else { "t.rejoinOff" }, n, &[]);
            ok(r, text);
        })
    })
    .disabled(total == 0);
    let rejoin_ctl = div()
        .flex()
        .items_center()
        .gap(px(10.))
        .when(on_n > 0 && !all_on, |d| {
            d.child(div().text_size(px(T_S)).text_color(c(MUTED)).child(root.i18n.tv("set.rejoinCount", &[("n", &on_n.to_string()), ("total", &total.to_string())])))
        })
        .child(rejoin);
    let hop = st.get("hopAbovePlayers").and_then(Value::as_i64).unwrap_or(0);
    let mut hops: Vec<(i64, String, bool)> = vec![(0, root.tr("set.hopOff"), false)];
    for n in [1, 2, 3, 5, 8, 10, 15, 20] {
        hops.push((n, if n == 1 { root.tr("set.hopN1") } else { root.i18n.tv("set.hopN", &[("n", &n.to_string())]) }, false));
    }
    let daily = root.state.get("dailySignInCheck").and_then(Value::as_bool).unwrap_or(true);
    let mut daily_desc = root.tr("set.dailyDesc");
    if let Some(at) = root.state.get("lastSignInCheck").and_then(Value::as_str).and_then(crate::time::iso_secs) {
        let local = at + crate::time::utc_offset_secs();
        let today = (crate::time::now_secs() + crate::time::utc_offset_secs()).div_euclid(86400);
        let hm = format!("{:02}:{:02}", local.rem_euclid(86400) / 3600, local.rem_euclid(3600) / 60);
        let when = if local.div_euclid(86400) == today {
            hm
        } else {
            let t = local.div_euclid(86400) * 86400;
            let date = civil_dm(t / 86400, root.i18n.vietnamese);
            format!("{date} {hm}")
        };
        daily_desc = format!("{daily_desc} {}", root.i18n.tv("set.dailyLast", &[("t", &when)]));
    }
    let keep = vec![
        row(root.tr("set.rejoin"), Some(root.tr("set.rejoinDesc")), rejoin_ctl),
        row(root.tr("set.antiAfk"), Some(root.tr("set.antiAfkDesc")), switch("s-antiafk", b(st, "antiAfk"), cx, |r, on, cx| set(r, json!({ "antiAfk": on }), cx))),
        row_below(root.tr("set.hop"), Some(root.tr("set.hopDesc")), chips(root, cx, "s-hop", hops, &hop, |r, v, cx| set(r, json!({ "hopAbovePlayers": v }), cx))),
        row(
            root.tr("set.daily"),
            Some(daily_desc),
            switch("s-daily", daily, cx, |r, on, cx| ask(r, "setDailySignInCheck", json!({ "enabled": on }), cx, |_, _| {})),
        ),
    ];
    let ram = st.get("ramLimitMb").and_then(Value::as_i64).unwrap_or(0);
    let rams: Vec<(i64, String, bool)> = vec![
        (0, root.tr("set.ramLimitOff"), false),
        (-1, root.tr("set.ramLimitAuto"), false),
        (512, "512 MB".into(), false),
        (768, "768 MB".into(), false),
        (1024, "1 GB".into(), false),
        (1536, "1.5 GB".into(), false),
        (2048, "2 GB".into(), false),
    ];
    let heavy = st.get("restartAboveMb").and_then(Value::as_i64).unwrap_or(0);
    let mut heavies: Vec<(i64, String, bool)> = vec![(0, root.tr("set.heavyOff"), false)];
    for g in [3, 4, 6, 8] {
        heavies.push((g * 1024, root.i18n.tv("set.heavyAbove", &[("n", &g.to_string())]), false));
    }
    let cores = st.get("cores").unwrap_or(&Value::Null);
    let cores_desc = if b(cores, "hybrid") {
        root.i18n.tv("set.coresDesc", &[("e", &cores.get("efficient").and_then(Value::as_i64).unwrap_or(0).to_string())])
    } else {
        root.tr("set.coresDescOne")
    };
    let memory = vec![
        row(root.tr("set.memWarn"), Some(root.tr("set.memWarnDesc")), switch("s-memwarn", b(st, "memoryWarnings"), cx, |r, on, cx| set(r, json!({ "memoryWarnings": on }), cx))),
        row(root.tr("set.trim"), Some(root.tr("set.trimDesc")), switch("s-trim", b(st, "trimBackground"), cx, |r, on, cx| set(r, json!({ "trimBackground": on }), cx))),
        row_below(root.tr("set.ramLimit"), Some(root.tr("set.ramLimitDesc")), chips(root, cx, "s-ram", rams, &ram, |r, v, cx| set(r, json!({ "ramLimitMb": v }), cx))),
        row_below(root.tr("set.heavy"), Some(root.tr("set.heavyDesc")), chips(root, cx, "s-heavy", heavies, &heavy, |r, v, cx| set(r, json!({ "restartAboveMb": v }), cx))),
        row(root.tr("set.cores"), Some(cores_desc), switch("s-cores", b(st, "splitCores"), cx, |r, on, cx| set(r, json!({ "splitCores": on }), cx))),
    ];
    vec![roblox::config(root, cx), group(Some(root.tr("set.g.keep")), keep), group(Some(root.tr("set.g.memory")), memory)]
}

// ───────────────────────── Notifications ─────────────────────────

const NOTIFY_KINDS: [&str; 5] = ["drop", "schedule", "signin", "ram", "launch"];

fn notify(root: &Root, cx: &mut Context<Root>) -> Vec<Div> {
    let on = root.state.get("notifications").and_then(Value::as_bool).unwrap_or(true);
    let off: Vec<String> = root.state.get("notifyOff").and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
    let main = vec![row(
        root.tr("set.notify"),
        Some(root.tr("set.notifyDesc")),
        switch("s-notify", on, cx, |r, on, cx| ask(r, "setNotifications", json!({ "enabled": on }), cx, |_, _| {})),
    )];
    let kinds: Vec<AnyElement> = NOTIFY_KINDS
        .iter()
        .enumerate()
        .map(|(i, k)| {
            let kind = k.to_string();
            let sw = Switch::new(("s-notify-k", i), !off.contains(&kind)).disabled(!on).on_toggle({
                let this = cx.entity().downgrade();
                move |v, _, cx| {
                    let (v, kind) = (*v, kind.clone());
                    let _ = this.update(cx, |r, cx| ask(r, "setNotifications", json!({ "kind": kind, "enabled": v }), cx, |_, _| {}));
                }
            });
            div().when(!on, |d| d.opacity(0.5)).child(row(root.tr(&format!("set.notify.{k}")), None, sw)).into_any_element()
        })
        .collect();
    let phone = root.state.get("phone").unwrap_or(&Value::Null);
    let phone_on = b(phone, "enabled");
    let mut phone_rows = vec![row(
        root.tr("set.phoneOn"),
        Some(root.tr("set.phoneDesc")),
        switch("s-phone", phone_on, cx, |r, on, cx| ask(r, "setPhone", json!({ "enabled": on }), cx, |_, _| {})),
    )];
    let topic = s(phone, "topic").to_string();
    if phone_on && !topic.is_empty() {
        let mut desc = root.tr("set.phoneTopicDesc");
        if !s(phone, "error").is_empty() {
            desc = format!("{desc} {}", s(phone, "error"));
        }
        let copy = topic.clone();
        phone_rows.push(row_el(
            root.tr("set.phoneTopic"),
            root.tr("set.phoneTopic"),
            Some(desc),
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(div().w(px(240.)).child(crate::kit::field(&root.phone_topic, None)))
                .child(Button::new("s-phone-copy", root.tr("set.phoneCopy")).icon("copy").small().quiet().on_click(cx.listener(move |r, _, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()));
                    ok(r, r.tr("set.phoneCopied"));
                    cx.notify();
                })))
                .child(Button::new("s-phone-test", root.tr("set.phoneTest")).small().on_click(cx.listener(|r, _, _, cx| {
                    ask(r, "phoneTest", json!({}), cx, |r, _| ok(r, r.tr("set.phoneSent")))
                }))),
        ));
    }
    let skipped = root.state.get("skipConfirm").and_then(Value::as_array).map(Vec::len).unwrap_or(0) as i64;
    let asks = vec![row(
        root.tr("set.asks"),
        Some(if skipped > 0 { root.i18n.tn("set.asksN", skipped, &[]) } else { root.tr("set.asksNone") }),
        Button::new("s-asks", root.tr("set.asksBtn")).small().disabled(skipped == 0).on_click(cx.listener(|r, _, _, cx| {
            ask(r, "resetSkipConfirm", json!({}), cx, |r, _| ok(r, r.tr("set.asksDone")))
        })),
    )];
    vec![group(None, main), group(Some(root.tr("set.notifyKinds")), kinds), group(Some(root.tr("set.phone")), phone_rows), group(Some(root.tr("set.g.asks")), asks)]
}

// ───────────────────────── Security ─────────────────────────

/// Something was typed in the master password's boxes: an old problem goes, and the new one's strength shows.
pub fn password_typed(r: &mut Root, cx: &mut Context<Root>) {
    let new = r.pw_new.read(cx).value().to_string();
    r.settings.pw_msg = (!new.is_empty()).then(|| {
        let word = r.tr(&format!("lock.strength.{}", crate::pages::lock::strength(&new).1));
        (r.i18n.tv("set.pwStrength", &[("s", &word)]), false)
    });
}

/// Change password: checked in the order the boxes are filled, then the app is asked ("changePassword").
pub fn change_password(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    if r.settings.pw_busy {
        return;
    }
    let (cur, new, new2) = (r.pw_cur.read(cx).value().to_string(), r.pw_new.read(cx).value().to_string(), r.pw_new2.read(cx).value().to_string());
    let fail = |r: &mut Root, key: &str, cx: &mut Context<Root>| {
        r.settings.pw_msg = Some((r.tr(key), true));
        cx.notify();
    };
    if cur.is_empty() {
        return fail(r, "set.pwNeedCur", cx);
    }
    if new.chars().count() < 6 {
        return fail(r, "set.pwShort", cx);
    }
    if new.trim().is_empty() {
        return fail(r, "lock.blank", cx);
    }
    if new == cur {
        return fail(r, "set.pwSame", cx);
    }
    if new != new2 {
        return fail(r, "set.pwMismatch", cx);
    }
    let Some(link) = r.link.clone() else { return };
    r.settings.pw_busy = true;
    r.settings.pw_msg = None;
    cx.notify();
    let rx = link.call("changePassword", json!({ "current": cur, "next": new }));
    cx.spawn_in(window, async move |this, cx| {
        let answer = rx.await;
        let _ = this.update_in(cx, |r, window, cx| {
            r.settings.pw_busy = false;
            match answer {
                Ok(Ok(_)) => {
                    for e in [r.pw_cur.clone(), r.pw_new.clone(), r.pw_new2.clone()] {
                        e.update(cx, |s, cx| s.set_value("", window, cx));
                    }
                    r.settings.pw_msg = None;
                    r.toast = Some((r.tr("set.pwDone"), "ok".into()));
                }
                Ok(Err(e)) => r.settings.pw_msg = Some((e.to_string(), true)),
                Err(_) => {}
            }
            cx.notify();
        });
    })
    .detach();
}

const AUTO_LOCK: [i64; 7] = [0, 5, 10, 15, 30, 60, 120];

fn security(root: &Root, cx: &mut Context<Root>) -> Vec<Div> {
    let mut out = Vec::new();
    let vaults = root.state.get("vaults").unwrap_or(&Value::Null);
    let open = s(vaults, "open");
    if vaults.get("list").and_then(Value::as_array).is_some_and(|l| l.iter().any(|v| s(v, "id") == open && !open.is_empty())) {
        let name = vaults.get("list").and_then(Value::as_array).and_then(|l| l.iter().find(|v| s(v, "id") == open)).map(|v| s(v, "name").to_string()).unwrap_or_default();
        out.push(group(
            Some(root.tr("vault.group")),
            vec![
                row(root.tr("vault.name"), Some(root.tr("vault.nameDesc")), field(&root.s_vault_name, None).w(px(220.))),
                // Its picture on the lock screen, instead of the first letter of its name.
                row(root.tr("vault.picture"), Some(root.tr("vault.pictureDesc")), {
                    let picture = vaults.get("picture").and_then(Value::as_str);
                    let (id, id2) = (open.to_string(), open.to_string());
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(crate::pages::lock::face(open, &name, picture, 32., R_S, 14.))
                        .when(picture.is_some(), |d| {
                            d.child(Button::new("s-vault-pic-x", root.tr("vault.pictureRemove")).small().on_click(cx.listener(move |r, _, _, cx| ask(r, "clearVaultPicture", json!({ "id": id }), cx, |_, _| {}))))
                        })
                        .child(Button::new("s-vault-pic", root.tr("vault.pick")).icon("image").small().on_click(cx.listener(move |r, _, _, cx| ask(r, "pickVaultPicture", json!({ "id": id2 }), cx, |_, _| {}))))
                }),
                row(root.tr("vault.ask"), None, switch("s-vault-ask", b(vaults, "ask"), cx, |r, on, cx| ask(r, "setAskVault", json!({ "enabled": on }), cx, |_, _| {}))),
                row(
                    root.tr("vault.switch"),
                    Some(root.tr("vault.switchDesc")),
                    Button::new("s-vault-pick", root.tr("vault.switch")).icon("repeat").small().on_click(cx.listener(|r, _, _, cx| ask(r, "pickVault", json!({}), cx, |_, _| {}))),
                ),
            ],
        ));
    }
    // The master password: the current one, the new one twice (SecurityPage.tsx's pw-form).
    let boxed = |label: &str, e: &Entity<gpui_kit::component::input::InputState>| {
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(div().text_size(px(T_S)).font_weight(FontWeight::MEDIUM).text_color(c(TEXT_2)).child(root.tr(label)))
            .child(field(e, None).w(px(340.)))
    };
    out.push(group(
        Some(root.tr("set.master")),
        vec![
            div()
                .px(px(ROW_X))
                .py(px(16.))
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(boxed("set.pwLabelCur", &root.pw_cur))
                .child(boxed("set.pwLabelNew", &root.pw_new))
                .child(boxed("set.pwLabelNew2", &root.pw_new2))
                .child(
                    div()
                        .pt(px(2.))
                        .flex()
                        .items_center()
                        .gap(px(12.))
                        .child(Button::new("pw-btn", root.tr("set.pwBtn")).primary().on_click(cx.listener(|r, _, window, cx| change_password(r, window, cx))))
                        .when_some(root.settings.pw_msg.clone(), |d, (text, bad)| d.child(div().min_w_0().text_size(px(T_S)).text_color(c(if bad { BAD } else { MUTED })).child(text))),
                )
                .into_any_element(),
        ],
    ));
    let lock = st(root).get("autoLockMinutes").and_then(Value::as_i64).unwrap_or(0);
    let words = |n: i64| {
        if n == 0 {
            root.tr("set.autoLock.never")
        } else if n < 60 {
            root.i18n.tv("set.autoLock.min", &[("n", &n.to_string())])
        } else if n == 60 {
            root.tr("set.autoLock.hour1")
        } else {
            root.i18n.tv("set.autoLock.hours", &[("n", &(n / 60).to_string())])
        }
    };
    out.push(group(
        Some(root.tr("set.g.autoLock")),
        vec![row_below(
            root.tr("set.autoLock"),
            Some(root.tr("set.autoLockDesc")),
            chips(root, cx, "s-autolock", AUTO_LOCK.iter().map(|n| (*n, words(*n), false)).collect(), &lock, |r, v, cx| set(r, json!({ "autoLockMinutes": v }), cx)),
        )],
    ));
    out
}

// ───────────────────────── Backup & data ─────────────────────────

fn backup(root: &Root, cx: &mut Context<Root>) -> Vec<Div> {
    let bk = root.state.get("backup").unwrap_or(&Value::Null);
    let folder = s(bk, "folder").to_string();
    let has_folder = !folder.is_empty();
    let on = bk.get("enabled").and_then(Value::as_bool).unwrap_or(true) && has_folder;
    let mut rows = vec![row(
        root.tr("bk.on"),
        Some(root.tr("bk.onDesc")),
        switch("s-bk-on", on, cx, move |r, v, cx| {
            if v && !has_folder {
                ask(r, "pickBackupFolder", json!({}), cx, |_, _| {});
            } else {
                ask(r, "setBackup", json!({ "enabled": v }), cx, |_, _| {});
            }
        }),
    )];
    // The folder: where it is (or that there is none), with Drive / OneDrive offered beside "Choose…" when found.
    let no_folder = root.tr("bk.noFolder");
    let suggested: Vec<(String, String)> =
        bk.get("suggested").and_then(Value::as_array).map(|l| l.iter().map(|x| (s(x, "path").to_string(), s(x, "name").to_string())).collect()).unwrap_or_default();
    let mut ctl = div().flex().items_center().gap(px(8.));
    for (i, (p, n)) in suggested.into_iter().enumerate() {
        ctl = ctl.child(Button::new(("s-bk-use", i), root.i18n.tv("bk.use", &[("name", &n)])).small().quiet().icon("cloud").on_click(cx.listener(move |r, _, _, cx| {
            ask(r, "setBackup", json!({ "folder": p.clone() }), cx, |_, _| {})
        })));
    }
    ctl = ctl
        .when(has_folder, |d| {
            d.child(IconButton::new("s-bk-open", "folder").tip(root.tr("set.openFolder")).on_click(cx.listener(|r, _, _, _| r.send("openBackupFolder", json!({})))))
        })
        .child(Button::new("s-bk-pick", root.tr("bk.choose")).small().on_click(cx.listener(|r, _, _, cx| ask(r, "pickBackupFolder", json!({}), cx, |_, _| {}))));
    rows.push(row_el(
        root.tr("bk.folder"),
        div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(root.tr("bk.folder"))
            .child(path_text(if has_folder { &folder } else { &no_folder }))
            // None yet: nothing is backed up until one is chosen (BackupPage.tsx).
            .when(!has_folder, |d| d.child(div().text_size(px(T_S)).text_color(c(WARN)).child(root.tr("bk.needFolderDesc")))),
        None,
        ctl,
    ));
    if b(bk, "cloud") && b(bk, "weakPassword") {
        rows.push(note("alert", root.tr("bk.weak"), WARN));
    }
    let last = bk.get("lastBackup").and_then(Value::as_str).and_then(crate::time::iso_secs);
    let status = match last {
        Some(at) => root.i18n.tv("bk.last", &[("t", &crate::time::ago(crate::time::now_secs() - at, &root.i18n))]),
        None => root.tr("bk.never"),
    };
    rows.push(row(
        status,
        Some(s(bk, "error").to_string()).filter(|e| !e.is_empty()),
        Button::new("s-bk-now", root.tr("bk.now")).small().disabled(!has_folder).tip(if has_folder { String::new() } else { root.tr("bk.needFolder") }).on_click(cx.listener(|r, _, _, cx| {
            ask(r, "backupNow", json!({}), cx, |r, _| ok(r, r.tr("bk.done")))
        })),
    ));

    let asking = root.settings.asking == Some("restore");
    let mut restore = vec![row(
        root.tr("bk.restore"),
        Some(root.tr("bk.restoreDesc")),
        Button::new("s-bk-restore", root.tr("bk.restoreBtn")).small().disabled(asking).on_click(cx.listener(|r, _, _, cx| {
            r.settings.asking = Some("restore");
            cx.notify();
        })),
    )];
    if asking {
        restore.push(
            div()
                .px(px(ROW_X))
                .py(px(12.))
                .flex()
                .flex_col()
                .gap(px(10.))
                .bg(ca(BAD, 0.06))
                .child(div().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).child(root.tr("bk.restoreConfirmTitle")))
                .child(div().text_size(px(T_S)).text_color(c(TEXT_2)).child(crate::i18n::plain(&root.tr("bk.restoreConfirm"))))
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap(px(8.))
                        .child(Button::new("s-bk-no", root.tr("common.cancel")).small().on_click(cx.listener(|r, _, _, cx| {
                            r.settings.asking = None;
                            cx.notify();
                        })))
                        .child(Button::new("s-bk-yes", root.tr("bk.restoreBtn")).small().danger().on_click(cx.listener(|r, _, _, cx| {
                            r.settings.asking = None;
                            ask(r, "restoreBackup", json!({}), cx, |_, _| {});
                        }))),
                )
                .into_any_element(),
        );
    }

    let paths = root.state.get("paths").unwrap_or(&Value::Null);
    let mut data = vec![row_el(
        root.tr("set.dataPath"),
        div().flex().flex_col().gap(px(4.)).child(root.tr("set.dataPath")).child(path_text(s(&root.state, "dataPath"))),
        None,
        Button::new("s-open-data", root.tr("set.openFolder")).icon("folder").small().on_click(cx.listener(|r, _, _, _| r.send("openDataFolder", json!({})))),
    )];
    for (k, title, btn) in [("vault", "set.vaultPath", "set.showFile"), ("log", "set.logPath", "set.showFile"), ("app", "set.appPath", "set.openFolder")] {
        let p = s(paths, k);
        if p.is_empty() {
            continue;
        }
        data.push(row_el(
            root.tr(title),
            div().flex().flex_col().gap(px(4.)).child(root.tr(title)).child(path_text(p)),
            None,
            Button::new(SharedString::from(format!("s-show-{k}")), root.tr(btn)).icon("folder").small().on_click(cx.listener(move |r, _, _, _| {
                r.send("showPath", json!({ "which": k }))
            })),
        ));
    }
    // Accounts from another Bloxkeep file (a backup, or another PC's vault): the ones here stay as they are.
    let import = vec![row(
        root.tr("iv.setRow"),
        Some(root.tr("iv.setRowDesc")),
        Button::new("s-import", root.tr("iv.setBtn")).icon("download").small().on_click(cx.listener(|r, _, window, cx| crate::pages::import_vault::open(r, window, cx))),
    )];
    vec![group(Some(root.tr("bk.title")), rows), group(Some(root.tr("bk.restore")), restore), group(None, import), group(Some(root.tr("set.data")), data)]
}

// ───────────────────────── Bloxkeep ─────────────────────────

const HOTKEYS: [(&str, &str); 5] = [("H", "set.hk.hide"), ("M", "set.hk.min"), ("N", "set.hk.next"), ("A", "set.hk.arrange"), ("B", "set.hk.open")];

fn updates(root: &Root, cx: &mut Context<Root>) -> Vec<Div> {
    let u = root.state.get("update").unwrap_or(&Value::Null);
    let supported = b(u, "supported");
    let status = s(u, "status");
    let v = s(u, "version");
    // The time; on another day than today, the date too (UpdatesPage.tsx).
    let when = u.get("checkedAt").and_then(Value::as_str).and_then(crate::time::iso_secs).map(|at| {
        let l = at + crate::time::utc_offset_secs();
        let hm = format!("{:02}:{:02}", l.rem_euclid(86400) / 3600, l.rem_euclid(3600) / 60);
        let today = (crate::time::now_secs() + crate::time::utc_offset_secs()).div_euclid(86400);
        if l.div_euclid(86400) == today { hm } else { format!("{} {hm}", civil_dm(l.div_euclid(86400), root.i18n.vietnamese)) }
    });
    let text = if !supported {
        root.tr("upd.unsupported")
    } else {
        match status {
            "checking" => root.tr("upd.checking"),
            "downloading" => root.i18n.tv("upd.downloading", &[("v", v), ("p", &u.get("progress").map(|p| p.to_string()).unwrap_or_default())]),
            "ready" if b(u, "quiet") => root.tr("upd.readyQuiet"),
            "ready" => root.i18n.tv(if b(u, "auto") { "upd.readyAuto" } else { "upd.ready" }, &[("v", v)]),
            "upToDate" => root.tr("upd.latest") + &when.map(|w| format!(" · {}", root.i18n.tv("upd.checkedAt", &[("t", &w)]))).unwrap_or_default(),
            "error" if !s(u, "error").is_empty() => format!("{}: {}", root.tr("upd.error"), s(u, "error")),
            "error" => root.tr("upd.error"),
            _ if b(u, "auto") => root.tr("upd.idle"),
            _ => root.tr("upd.off"),
        }
    };
    let ready = status == "ready" && !b(u, "quiet");
    let ctl: AnyElement = if !supported {
        div().into_any_element()
    } else if ready {
        Button::new("s-upd-apply", root.tr("upd.restart")).icon("refresh").small().primary().on_click(cx.listener(|r, _, _, cx| ask(r, "applyUpdate", json!({}), cx, |_, _| {}))).into_any_element()
    } else {
        Button::new("s-upd-check", root.tr("upd.check"))
            .icon("refresh")
            .small()
            .disabled(matches!(status, "checking" | "downloading" | "ready"))
            .on_click(cx.listener(|r, _, _, cx| ask(r, "checkUpdates", json!({}), cx, |_, _| {})))
            .into_any_element()
    };
    let upd = vec![
        row(format!("Bloxkeep {}", s(u, "current")), Some(text), ctl),
        row(root.tr("upd.auto"), None, switch("s-autoupd", b(u, "auto"), cx, |r, on, cx| ask(r, "setAutoUpdate", json!({ "enabled": on }), cx, |_, _| {})).disabled(!supported)),
        row(
            root.tr("upd.beta"),
            Some(root.tr("upd.betaDesc")),
            switch("s-betaupd", b(u, "beta"), cx, |r, on, cx| ask(r, "setBetaUpdates", json!({ "enabled": on }), cx, |_, _| {})).disabled(!supported),
        ),
    ];

    let keys = root.state.get("hotkeys").unwrap_or(&Value::Null);
    let keys_on = keys.get("on").and_then(Value::as_bool).unwrap_or(true);
    let mut chips_line = div().flex().flex_wrap().gap(px(10.)).text_size(px(T_S)).text_color(c(MUTED));
    let kbd = |k: &str| div().px(px(5.)).py(px(1.)).rounded(px(4.)).bg(ca(WHITE, 0.08)).text_color(c(TEXT_2)).child(k.to_string());
    for (i, (k, key)) in HOTKEYS.iter().enumerate() {
        let mut one = div().flex().items_center().gap(px(3.));
        if i == 0 {
            one = one.child(kbd("Ctrl")).child("+").child(kbd("Alt")).child("+");
        }
        chips_line = chips_line.child(one.child(kbd(k)).child(div().ml(px(3.)).child(root.tr(key))));
    }
    let taken: Vec<String> = keys.get("taken").and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
    // One the keyboard types a character with (AltGr is Ctrl+Alt) is said too, when its hook was refused.
    let typing: Vec<String> = keys.get("typing").and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
    let app = vec![
        row(
            root.tr("set.boot"),
            Some(root.tr("set.bootDesc")),
            switch("s-boot", b(&root.state, "startWithWindows"), cx, |r, on, cx| ask(r, "setStartWithWindows", json!({ "enabled": on }), cx, |_, _| {})),
        ),
        row(
            root.tr("set.tray"),
            Some(root.tr("set.trayDesc")),
            switch("s-tray", root.state.get("closeToTray").and_then(Value::as_bool).unwrap_or(true), cx, |r, on, cx| {
                ask(r, "setCloseToTray", json!({ "enabled": on }), cx, |_, _| {})
            }),
        ),
        div()
            .px(px(ROW_X))
            .py(px(14.))
            .flex()
            .items_center()
            .gap(px(24.))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(div().text_size(px(T_M)).font_weight(FontWeight::MEDIUM).child(root.tr("set.hotkeys")))
                    .child(chips_line)
                    .when(keys_on && !taken.is_empty(), |d| {
                        d.child(div().text_size(px(T_S)).text_color(c(WARN)).child(root.i18n.tv("set.hotkeysTaken", &[("keys", &taken.join(", "))])))
                    })
                    .when(keys_on && !typing.is_empty(), |d| {
                        d.child(div().text_size(px(T_S)).text_color(c(WARN)).child(root.i18n.tv("set.hotkeysTyping", &[("keys", &typing.join(", "))])))
                    }),
            )
            .child(switch("s-hotkeys", keys_on, cx, |r, on, cx| ask(r, "setHotkeys", json!({ "enabled": on }), cx, |_, _| {})))
            .into_any_element(),
        // Hardware acceleration of the windows still drawn as web pages (mini window, macro panel, sign-in browser):
        // off is the safe rendering WebView2 starts with (UpdatesPage.tsx's s-gpu).
        row(
            root.tr("set.gpu"),
            Some(root.tr("set.gpuDesc")),
            switch("s-gpu", !b(&root.state, "safeRendering"), cx, |r, on, cx| ask(r, "setRendering", json!({ "gpu": on }), cx, |_, _| {})),
        ),
    ];
    let link = |id: &'static str, icon_name: &'static str, label: String, cmd: &'static str| {
        Button::new(id, label).icon(icon_name).small().on_click(cx.listener(move |r, _, _, _| r.send(cmd, json!({}))))
    };
    let help = vec![
        // Straight to the person who makes Bloxkeep: something that doesn't work, or an idea.
        row(
            root.tr("fb.title"),
            Some(root.tr("fb.rowDesc")),
            Button::new("s-feedback", root.tr("fb.open")).icon("message").small().on_click(cx.listener(|r, _, window, cx| crate::pages::extras::open_feedback(r, window, cx))),
        ),
        row(
            root.tr("wn.heading"),
            None,
            Button::new("s-whatsnew", root.tr("common.show")).icon("news").small().on_click(cx.listener(|r, _, _, cx| crate::pages::extras::ask_whats_new(r, cx))),
        ),
        row(root.tr("help.title"), Some(root.tr("help.desc")), link("s-guide", "book", root.tr("help.open"), "openGuide")),
        row(root.tr("dc.title"), Some(root.tr("dc.desc")), link("s-discord", "discord", root.tr("dc.open"), "openDiscord")),
        row(root.tr("star.title"), Some(root.tr("star.desc")), link("s-github", "star", root.tr("star.open"), "openGitHub")),
    ];
    let counted = root.state.get("countOnline").and_then(Value::as_bool).unwrap_or(true);
    let privacy = vec![row(
        root.tr("onl.title"),
        Some(root.tr("onl.desc")),
        switch("s-online", counted, cx, |r, on, cx| ask(r, "setCountOnline", json!({ "enabled": on }), cx, |_, _| {})),
    )];
    vec![
        group(Some(root.tr("upd.title")), upd),
        group(Some(root.tr("set.g.app")), app),
        group(Some(root.tr("fb.group")), help),
        group(Some(root.tr("onl.group")), privacy),
    ]
}
