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
use crate::val::s;
use crate::val::flag;

pub mod appearance;
mod discord;
mod roblox;
pub(crate) use roblox::{level_now, level_settings};
pub(crate) use roblox::FPS_MAX;
/// A custom theme's code, for the checks.
#[cfg(feature = "core")]
pub(crate) mod theme_code {
    pub(crate) use super::appearance::{code_of, look_of};
}

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
    /// A part by its key (a shot opens Settings on it: BLOXKEEP_SHOT_SET).
    #[cfg(feature = "shot")]
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
    /// The main accounts ticked while choosing them (Staying in game's Main account).
    pub main_pick: Option<HashSet<String>>,
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
    pub dc_board_preview: bool,
    /// Anti-AFK's key list open under its row (Other key).
    pub afk_keys: bool,
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

/// A setting sent as it is (setSettings), the app's next state shows it.
fn set(r: &mut Root, v: Value, cx: &mut Context<Root>) {
    r.ask("setSettings", v, cx, |_, _| {});
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
    div().flex().flex_col().gap(px(9.)).when_some(title, |d, t| d.child(section_head(t, None))).child(rows_card(rows))
}

/// A surface per settings group, with thin separators between its rows.
fn rows_card(rows: Vec<AnyElement>) -> Div {
    // (Glass with glass on the blocks only: its faint tint alone left the sharp picture under every row.)
    let mut card = crate::kit::block_of(R_L, ca(WHITE, 0.026)).flex().flex_col();
    for (index, r) in rows.into_iter().enumerate() {
        card = card.child(div().when(index > 0, |d| d.border_t_1().border_color(c(LINE))).child(r));
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
    static EN_MADE: std::cell::RefCell<Option<(&'static str, std::rc::Rc<std::collections::HashMap<String, String>>)>> = const { std::cell::RefCell::new(None) };
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
        // (A control too wide to leave the words 220 px on a narrow window, at 200 % say, goes under them: the words
        // were cut to a letter or two a line, "Taskb / ar".)
        .flex_wrap()
        .items_center()
        .gap_x(px(32.))
        .gap_y(px(10.))
        .child(row_words(title, desc).min_w(px(220.)))
        .child(div().flex_none().flex().items_center().justify_end().gap(px(8.)).child(ctl))
        .into_any_element();
    found(&words, el)
}

const ROW_X: f32 = 16.;

/// A row's name (14 px, medium) and what it does under it in muted.
fn row_words(title: impl IntoElement, desc: Option<String>) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(3.))
        .child(div().text_size(px(T_L)).line_height(px(20.)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(title))
        .when_some(desc, |d, t| d.child(div().text_size(px(T_M)).line_height(px(19.)).text_color(c(TEXT_2)).child(t)))
}

/// A setting whose control is too wide to sit beside its name (the layouts as pictures): the control under the words,
/// from the same left edge.
fn row_wide(title: impl Into<SharedString>, desc: Option<String>, ctl: impl IntoElement) -> AnyElement {
    let title: SharedString = title.into();
    let words = with_en(format!("{title} {}", desc.as_deref().unwrap_or_default()), &[&title, desc.as_deref().unwrap_or_default()]);
    let el = div().px(px(ROW_X)).pt(px(14.)).pb(px(18.)).flex().flex_col().gap(px(14.)).child(row_words(title, desc)).child(ctl).into_any_element();
    found(&words, el)
}

/// Each text's English (I18n::en_by_own) while the page is in another language, made the first time it is asked (and
/// again for another language).
fn en_of(root: &Root) -> Option<std::rc::Rc<std::collections::HashMap<String, String>>> {
    if root.i18n.lang == "en" {
        return None;
    }
    let lang = root.i18n.lang;
    Some(EN_MADE.with(|m| {
        let mut m = m.borrow_mut();
        if m.as_ref().is_none_or(|(l, _)| *l != lang) {
            *m = Some((lang, std::rc::Rc::new(root.i18n.en_by_own())));
        }
        m.as_ref().map(|(_, words)| words.clone()).unwrap_or_default()
    }))
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
pub(crate) fn chips<T: Clone + PartialEq + 'static>(
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
#[derive(Clone)]
enum Mark {
    Icon(&'static str),
    Letter(char),
    /// A program's own icon (launcher_icons.rs), dimmed with its row when it isn't installed.
    Pic(std::sync::Arc<RenderImage>),
    /// Bloxkeep's own mark.
    Logo,
}

fn mark(m: Mark, on: bool) -> Div {
    let tone = if on { c(TEXT) } else { c(MUTED) };
    // The mark alone, no box round it: a box on every choice read as a row of stickers.
    let tile = div().min_w(px(14.)).flex_none().flex().items_center().justify_center();
    match m {
        Mark::Icon(name) => tile.child(icon(name, 14., tone)),
        Mark::Letter(l) => tile.text_size(px(T_XS)).font_weight(FontWeight::SEMIBOLD).text_color(tone).child(l.to_string()),
        Mark::Pic(p) => tile.child(img(p).size(px(16.)).flex_none()),
        Mark::Logo => tile.child(crate::crisp::mark(16.)),
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
    let chosen_mark = items.iter().position(|(v, _, _)| v == current).and_then(|i| marks.get(i).cloned());
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
                toggle_menu(r, key);
                cx.notify();
            })),
    );
    let f: std::rc::Rc<dyn Fn(&mut Root, T, &mut Window, &mut Context<Root>)> = f;
    if let Some(menu) = menu_pop(root, cx, key, items, marks, current, f) {
        wrap = wrap.child(menu);
    }
    wrap
}

/// The choices of a list, under what opened it (settings.menu == key): one row each, the chosen one ticked. None
/// while it is shut and has finished closing. A list's own button shows it, and a box with a chevron of its own.
fn menu_pop<T: Clone + PartialEq + 'static>(
    root: &Root,
    cx: &mut Context<Root>,
    key: &'static str,
    items: Vec<(T, String, bool)>,
    marks: Vec<Mark>,
    current: &T,
    f: std::rc::Rc<dyn Fn(&mut Root, T, &mut Window, &mut Context<Root>)>,
) -> Option<Div> {
    let open = root.settings.menu == Some(key);
    if !(open || root.settings.menu_was == Some(key)) {
        return None;
    }
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
            let m = marks.get(i).cloned();
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
        return Some(popup(PopAt::BelowRight(38.), SharedString::from(format!("{key}-pop")), open, menu));
}

/// A click on what opens a list: it opens, or shuts (a click outside the open one shut it just now: this click on
/// its own button only shuts it).
fn toggle_menu(r: &mut Root, key: &'static str) {
    let just = r.settings.menu_closed.is_some_and(|(k, at)| k == key && at.elapsed().as_millis() < 300);
    r.settings.menu_closed = None;
    r.settings.menu = if just || r.settings.menu == Some(key) { None } else { Some(key) };
    r.settings.menu_was = Some(key);
}

/// "9/11" in Vietnamese, "11/9" in English.
fn civil_dm(days: i64) -> String {
    let (_, m, d) = crate::time::civil(days);
    crate::time::dm(d, m)
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

/// Anti-AFK's key: F15 (the default), Space to jump, or any other from the macro keys' list, opened under it.
fn afk_key(root: &Root, st: &Value, cx: &mut Context<Root>) -> AnyElement {
    let now = st.get("antiAfkKey").and_then(Value::as_str).filter(|k| !k.is_empty()).unwrap_or("F15").to_string();
    let other = now != "F15" && now != "Space";
    let mut items: Vec<(String, String, bool)> = vec![("F15".into(), "F15".into(), false), ("Space".into(), root.tr("set.antiAfkJump"), false)];
    if other {
        items.push((now.clone(), crate::pages::macro_form::key_name(root, &now), false));
    }
    let open = root.settings.afk_keys;
    let row = div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(8.))
        .child(chips(root, cx, "s-afk-key", items, &now, |r, v: String, cx| {
            r.settings.afk_keys = false;
            set(r, json!({ "antiAfkKey": v }), cx)
        }))
        .child(Button::new("s-afk-key-other", root.tr("set.antiAfkKeyOther")).small().quiet().icon(if open { "chevronUp" } else { "chevronDown" }).on_click(cx.listener(|r, _, _, cx| {
            r.settings.afk_keys = !r.settings.afk_keys;
            cx.notify();
        })));
    let grid = open.then(|| {
        let mut grid = div().flex().flex_wrap().gap(px(4.));
        for (n, k) in crate::pages::macro_form::all_keys().into_iter().enumerate() {
            let on = k == now;
            let label = crate::pages::macro_form::key_name(root, &k);
            grid = grid.child(
                hot(("s-afk-k", n))
                    .h(px(30.))
                    .min_w(px(34.))
                    .px(px(8.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(7.))
                    .map(|d| if on { d.tones(c(FACE), c(FACE_HI)).text_color(crate::theme::on_text()) } else { d.tones(ca(WHITE, 0.05), ca(WHITE, 0.12)).text_color(c(TEXT_2)) })
                    .cursor_pointer()
                    .text_size(px(T_S))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(label)
                    .on_click(cx.listener(move |r, _, _, cx| {
                        r.settings.afk_keys = false;
                        set(r, json!({ "antiAfkKey": k.clone() }), cx);
                    })),
            );
        }
        grid
    });
    div().w_full().flex().flex_col().gap(px(10.)).child(row).children(grid.map(|g| g.w_full())).into_any_element()
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
    let mut list = div().flex().flex_wrap().gap_y(px(8.));
    for (i, (id, label, sub)) in items.into_iter().enumerate() {
        let toggle = toggle.clone();
        let this = cx.entity().downgrade();
        list = list.child(
            // (Each in a cell of its own width, its name and @name on a line each, cut with … when long: side by side
            // in 220 dots a long name was squeezed to a letter a line, the owner's shot, 2026-11-15.)
            div().w(px(220.)).flex_none().flex().flex_col().pr(px(12.)).child({
                let check = Check::new((key, i), ticked.contains(&id)).label(label).on_toggle(move |v, _, cx| {
                    let v = *v;
                    let id = id.clone();
                    let toggle = toggle.clone();
                    let _ = this.update(cx, |r, cx| {
                        toggle(r, id, v);
                        cx.notify();
                    });
                });
                if sub.is_empty() { check } else { check.sub(sub) }
            }),
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
        r.settings.main_pick = None;
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
    let state = if !flag(d, "hasWebhook") {
        "set.dc.state.none"
    } else if flag(d, "enabled") {
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
    let compact = width < 760.;
    let nav_w = if compact { 64. } else if narrow { 198. } else { 230. };
    let mut nav = div().w(px(nav_w)).flex_none().flex().flex_col().gap(px(4.)).px(px(10.)).pt(px(16.)).pb(px(20.));
    for (i, p) in Part::ALL.into_iter().enumerate() {
        let on = p == part && root.set_q.read(cx).value().trim().is_empty();
        if let Some(g) = p.group_key().filter(|_| !compact) {
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
        let tip: SharedString = root.tr(p.title()).into();
        nav = nav.child(
            menu_row(("set-nav", i)).relative()
                .h(px(39.))
                .w_full()
                .px(px(if compact { 0. } else { 12. }))
                .when(compact, |d| d.justify_center().tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into()))
                .border_1()
                .edge(if on { ca(FACE, 0.25) } else { ca(WHITE, 0.) }, if on { ca(FACE, 0.40) } else { ca(WHITE, 0.10) })
                .tones(if on { ca(FACE, 0.11) } else { ca(WHITE, 0.) }, if on { ca(FACE, 0.17) } else { ca(WHITE, 0.05) })
                .text_color(c(if on { TEXT } else { TEXT_2 }))
                .text_size(px(T_L))
                .font_weight(if on { FontWeight::SEMIBOLD } else { FontWeight::MEDIUM })
                .child(icon(p.icon(), 17., if on { c(FACE) } else { c(MUTED) }))
                .when(!compact, |d| d.child(div().flex_1().min_w_0().truncate().child(root.tr(p.title()))))
                .when(look, |d| {
                    // (Shell.tsx's dot says what it is under the mouse.)
                    let tip: SharedString = root.tr("set.needsLook").into();
                    d.child(div().id(("set-look", i)).size(px(7.)).flex_none().rounded_full().bg(c(WARN))
                        .when(compact, |d| d.absolute().top(px(4.)).right(px(4.)))
                        .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into()))
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
    let part_head = div().flex().items_center().gap(px(14.)).pt(px(4.)).pb(px(6.))
        .child(div().size(px(46.)).flex_none().rounded(px(R_M)).bg(ca(FACE, 0.12)).flex().items_center().justify_center()
            .child(icon(if searching { "search" } else { part.icon() }, 21., c(FACE))))
        .child(div().min_w_0().flex().flex_col().gap(px(3.))
            .child(div().text_size(px(T_FIG)).line_height(px(28.)).font_weight(FontWeight::BOLD).child(part_title))
            .when_some(part_about, |d, t| d.child(div().text_size(px(T_M)).line_height(px(19.)).text_color(c(TEXT_2)).child(t))));
    let header = div()
        .flex_none()
        .px(px(PAGE_X))
        .pt(px(22.))
        .pb(px(22.))
        // (With glass on the blocks only the head stands on the picture itself: no line across it there, the owner
        // 2026-11-18.)
        .when(!crate::theme::glass_blocks(), |d| d.border_b_1().border_color(c(LINE)))
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
        .child(div().w(px(if narrow { 240. } else { 300. })).min_w(px(if compact { 120. } else { 160. })).flex_shrink(1.).child(field(&root.set_q, Some("search"))));
    // The list of parts: the panel's left column, its own ground to the panel's foot. With glass on the blocks only it
    // is a block of its own, as tall as its list and no more, a gap round it: the whole column as one pane of glass
    // ran on empty to the panel's foot (the owner, 2026-11-18: "layout này là bạn làm có chủ đích hay sao").
    let nav_col = if crate::theme::glass_blocks() {
        div()
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            // (Its block's edge on the page's margin, under the title, as every page's blocks are.)
            .pl(px(PAGE_X))
            .pt(px(12.))
            .pb(px(16.))
            .child(crate::kit::block_of(R_L, ca(GROUND, 0.34)).min_h_0().flex().flex_col().overflow_hidden().child(div().id("set-nav").min_h_0().overflow_y_scroll().child(nav)))
            .into_any_element()
    } else {
        // (Its rows' grounds start on the page's margin, under the title: the list keeps 10 of its own.)
        div().id("set-nav").h_full().flex_none().overflow_y_scroll().border_r_1().border_color(c(LINE)).bg(ca(GROUND, 0.34)).pl(px(PAGE_X - 10.)).child(nav).into_any_element()
    };
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
            div().flex_1().min_h_0().flex().child(nav_col).child(
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
                                // (The page's margin on both sides, as every page has: capped at 1180 and centred, its
                                // right edge sat 75 px in from the search box over it.)
                                .w_full()
                                .px(px(PAGE_X))
                                .pt(px(24.))
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
    let multi_on = flag(m, "enabled");
    let (pill, tone) = if !multi_on {
        ("pill.off", Tone::Muted)
    } else if flag(m, "armed") {
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
    // On, but Roblox was open first: said here at once, with the button, not only behind Details.
    if multi_on && !flag(m, "armed") && !asking {
        windows.push(root.multi_blocked_note(cx).px(px(ROW_X)).py(px(12.)).bg(ca(WARN, 0.06)).into_any_element());
    }
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
    let rename = flag(st, "renameWindows");
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
    let open_as = if flag(m, "tile") { "tile" } else if flag(st, "maximizeOnOpen") { "maximize" } else { "roblox" }.to_string();
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
    let has = |k: &str| flag(ins, k);
    let mut options: Vec<(String, String, bool)> = vec![(
        "auto".into(),
        if s(ins, "handler").is_empty() { root.tr("launch.autoNone") } else { root.i18n.tv("launch.auto", &[("name", s(ins, "handler"))]) },
        true,
    )];
    // Bloxkeep itself, by name and first among the launchers: it starts Roblox's client with no other program between
    // (the same choice that read "Roblox directly (no launcher)"; the owner, 2026-11-18: "trong đây vẫn chưa có
    // bloxkeep à").
    options.push(("direct".into(), root.tr("launch.direct"), has("direct")));
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
    if has("store") || via_saved == "store" {
        options.push(("store".into(), root.tr("launch.storeName"), has("store")));
    }
    let custom = s(st, "customLauncher");
    if !custom.is_empty() {
        options.push(("custom".into(), root.i18n.tv("launch.custom", &[("name", custom)]), true));
    }
    options.push(("vng".into(), "Roblox VNG".into(), has("vng")));
    let handler_name = s(ins, "handler").to_string();
    let marks: Vec<Mark> = options
        .iter()
        .map(|(v, words, _)| match crate::launcher_icons::of(v, &handler_name, custom) {
            _ if v == "direct" => Mark::Logo,
            Some(p) => Mark::Pic(p),
            None => match v.as_str() {
            "auto" => Mark::Icon("link"),
            "roblox" => Mark::Icon("roblox"),
            "store" => Mark::Icon("download"),
            "custom" => Mark::Icon("folder"),
            "vng" => Mark::Icon("globe"),
            _ => Mark::Letter(words.chars().find(|ch| ch.is_alphanumeric()).unwrap_or('?').to_ascii_uppercase()),
            },
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
                r.ask("pickLauncher", json!({}), cx, |_, _| {})
            }))),
    )];
    if let Some(n) = store_note {
        client.push(note("info", n, TEXT_2));
    }
    let close = flag(st, "closeAfterLeave");
    client.push(row(root.tr("set.closeAfterLeave"), Some(root.tr("set.closeAfterLeaveDesc")), switch("s-close-leave", close, cx, |r, on, cx| set(r, json!({ "closeAfterLeave": on }), cx))));
    let own = flag(st, "ownRobloxLinks");
    client.push(row(root.tr("set.ownLinks"), Some(root.tr("set.ownLinksDesc")), switch("s-own-links", own, cx, |r, on, cx| set(r, json!({ "ownRobloxLinks": on }), cx))));
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
    let on_n = accounts.iter().filter(|a| flag(a, "autoRejoin")).count();
    let all_on = total > 0 && on_n == total;
    let ids: Vec<String> = accounts.iter().map(|a| s(a, "id").to_string()).collect();
    let rejoin = switch("s-rejoin", all_on, cx, move |r, on, cx| {
        r.ask("setAutoRejoin", json!({ "ids": ids.clone(), "enabled": on }), cx, move |r, v| {
            let n = v.as_i64().unwrap_or(0);
            let text = r.i18n.tn(if on { "t.rejoinOn" } else { "t.rejoinOff" }, n, &[]);
            r.ok(text);
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
            let date = civil_dm(t / 86400);
            format!("{date} {hm}")
        };
        daily_desc = format!("{daily_desc} {}", root.i18n.tv("set.dailyLast", &[("t", &when)]));
    }
    let keep = vec![
        row(root.tr("set.rejoin"), Some(root.tr("set.rejoinDesc")), rejoin_ctl),
        row_below(root.tr("set.rejoinWait"), Some(root.tr("set.rejoinWaitDesc")), {
            let now = st.get("rejoinWaitSeconds").and_then(Value::as_i64).unwrap_or(0);
            let mut waits: Vec<i64> = vec![0, 10, 30, 60, 120];
            if !waits.contains(&now) {
                waits.push(now);
                waits.sort_unstable();
            }
            let items = waits.into_iter().map(|n| (n, if n == 0 { root.tr("set.rejoinWait0") } else if n % 60 == 0 { root.i18n.tv("set.minutesN", &[("n", &(n / 60).to_string())]) } else { root.i18n.tv("set.seconds", &[("n", &n.to_string())]) }, false)).collect();
            chips(root, cx, "s-rejoin-wait", items, &now, |r, v, cx| set(r, json!({ "rejoinWaitSeconds": v }), cx))
        }),
        row(root.tr("set.antiAfk"), Some(root.tr("set.antiAfkDesc")), switch("s-antiafk", flag(st, "antiAfk"), cx, |r, on, cx| set(r, json!({ "antiAfk": on }), cx))),
        row_below(root.tr("set.antiAfkEvery"), Some(root.tr("set.antiAfkEveryDesc")), {
            let now = st.get("antiAfkMinutes").and_then(Value::as_i64).unwrap_or(12);
            let mut every: Vec<i64> = vec![5, 8, 10, 12, 15, 18];
            if !every.contains(&now) {
                every.push(now);
                every.sort_unstable();
            }
            let items = every.into_iter().map(|n| (n, root.i18n.tv("set.minutesN", &[("n", &n.to_string())]), false)).collect();
            chips(root, cx, "s-afk-every", items, &now, |r, v, cx| set(r, json!({ "antiAfkMinutes": v }), cx))
        }),
        row_wide(root.tr("set.antiAfkKey"), Some(root.tr("set.antiAfkKeyDesc")), afk_key(root, st, cx)),
        row_below(root.tr("set.hop"), Some(root.tr("set.hopDesc")), chips(root, cx, "s-hop", hops, &hop, |r, v, cx| set(r, json!({ "hopAbovePlayers": v }), cx))),
        row(
            root.tr("set.daily"),
            Some(daily_desc),
            switch("s-daily", daily, cx, |r, on, cx| r.ask("setDailySignInCheck", json!({ "enabled": on }), cx, |_, _| {})),
        ),
    ];
    let ram = st.get("ramLimitMb").and_then(Value::as_i64).unwrap_or(0);
    let rams: Vec<(i64, String, bool)> = vec![
        (0, root.tr("set.ramLimitOff"), false),
        (-1, root.tr("set.ramLimitAuto"), false),
        (512, crate::lang::cur().mb(512.), false),
        (768, crate::lang::cur().mb(768.), false),
        (1024, crate::lang::cur().mb(1024.), false),
        (1536, crate::lang::cur().mb(1536.), false),
        (2048, crate::lang::cur().mb(2048.), false),
    ];
    let heavy = st.get("restartAboveMb").and_then(Value::as_i64).unwrap_or(0);
    let mut heavies: Vec<(i64, String, bool)> = vec![(0, root.tr("set.heavyOff"), false)];
    for g in [3, 4, 6, 8] {
        heavies.push((g * 1024, root.i18n.tv("set.heavyAbove", &[("n", &g.to_string())]), false));
    }
    let cores = st.get("cores").unwrap_or(&Value::Null);
    let cores_desc = if flag(cores, "hybrid") {
        root.i18n.tv("set.coresDesc", &[("e", &cores.get("efficient").and_then(Value::as_i64).unwrap_or(0).to_string())])
    } else {
        root.tr("set.coresDescOne")
    };
    let memory = vec![
        row(root.tr("set.memWarn"), Some(root.tr("set.memWarnDesc")), switch("s-memwarn", flag(st, "memoryWarnings"), cx, |r, on, cx| set(r, json!({ "memoryWarnings": on }), cx))),
        row(root.tr("set.trim"), Some(root.tr("set.trimDesc")), switch("s-trim", flag(st, "trimBackground"), cx, |r, on, cx| set(r, json!({ "trimBackground": on }), cx))),
        row(root.tr("ram.clean"), Some(root.tr("ram.cleanDesc")), Button::new("s-clean-ram", root.tr("ram.open")).icon("shield").small().on_click(cx.listener(|r, _, w, cx| crate::clean_ram::open(r, w, cx)))),
        row_below(root.tr("set.ramLimit"), Some(root.tr("set.ramLimitDesc")), chips(root, cx, "s-ram", rams, &ram, |r, v, cx| set(r, json!({ "ramLimitMb": v }), cx))),
        row_below(root.tr("set.heavy"), Some(root.tr("set.heavyDesc")), chips(root, cx, "s-heavy", heavies, &heavy, |r, v, cx| set(r, json!({ "restartAboveMb": v }), cx))),
        row(root.tr("set.cores"), Some(cores_desc), switch("s-cores", flag(st, "splitCores"), cx, |r, on, cx| set(r, json!({ "splitCores": on }), cx))),
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
        switch("s-notify", on, cx, |r, on, cx| r.ask("setNotifications", json!({ "enabled": on }), cx, |_, _| {})),
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
                    let _ = this.update(cx, |r, cx| r.ask("setNotifications", json!({ "kind": kind, "enabled": v }), cx, |_, _| {}));
                }
            });
            div().when(!on, |d| d.opacity(0.5)).child(row(root.tr(&format!("set.notify.{k}")), None, sw)).into_any_element()
        })
        .collect();
    let phone = root.state.get("phone").unwrap_or(&Value::Null);
    let phone_on = flag(phone, "enabled");
    let mut phone_rows = vec![row(
        root.tr("set.phoneOn"),
        Some(root.tr("set.phoneDesc")),
        switch("s-phone", phone_on, cx, |r, on, cx| r.ask("setPhone", json!({ "enabled": on }), cx, |_, _| {})),
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
                    r.ok(r.tr("set.phoneCopied"));
                    cx.notify();
                })))
                .child(Button::new("s-phone-test", root.tr("set.phoneTest")).small().on_click(cx.listener(|r, _, _, cx| {
                    r.ask("phoneTest", json!({}), cx, |r, _| r.ok(r.tr("set.phoneSent")))
                }))),
        ));
    }
    let skipped = root.state.get("skipConfirm").and_then(Value::as_array).map(Vec::len).unwrap_or(0) as i64;
    let asks = vec![row(
        root.tr("set.asks"),
        Some(if skipped > 0 { root.i18n.tn("set.asksN", skipped, &[]) } else { root.tr("set.asksNone") }),
        Button::new("s-asks", root.tr("set.asksBtn")).small().disabled(skipped == 0).on_click(cx.listener(|r, _, _, cx| {
            r.ask("resetSkipConfirm", json!({}), cx, |r, _| r.ok(r.tr("set.asksDone")))
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
                        .child(crate::pages::lock::face(open, &name, picture, 32., R_S, 14., cx))
                        .when(picture.is_some(), |d| {
                            d.child(Button::new("s-vault-pic-x", root.tr("vault.pictureRemove")).small().on_click(cx.listener(move |r, _, _, cx| r.ask("clearVaultPicture", json!({ "id": id }), cx, |_, _| {}))))
                        })
                        .child(Button::new("s-vault-pic", root.tr("vault.pick")).icon("image").small().on_click(cx.listener(move |r, _, _, cx| r.ask("pickVaultPicture", json!({ "id": id2 }), cx, |_, _| {}))))
                }),
                row(root.tr("vault.ask"), None, switch("s-vault-ask", flag(vaults, "ask"), cx, |r, on, cx| r.ask("setAskVault", json!({ "enabled": on }), cx, |_, _| {}))),
                row(
                    root.tr("vault.switch"),
                    Some(root.tr("vault.switchDesc")),
                    Button::new("s-vault-pick", root.tr("vault.switch")).icon("repeat").small().on_click(cx.listener(|r, _, _, cx| r.ask("pickVault", json!({}), cx, |_, _| {}))),
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
                r.ask("pickBackupFolder", json!({}), cx, |_, _| {});
            } else {
                r.ask("setBackup", json!({ "enabled": v }), cx, |_, _| {});
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
            r.ask("setBackup", json!({ "folder": p.clone() }), cx, |_, _| {})
        })));
    }
    ctl = ctl
        .when(has_folder, |d| {
            d.child(IconButton::new("s-bk-open", "folder").tip(root.tr("set.openFolder")).on_click(cx.listener(|r, _, _, _| r.send("openBackupFolder", json!({})))))
        })
        .child(Button::new("s-bk-pick", root.tr("bk.choose")).small().on_click(cx.listener(|r, _, _, cx| r.ask("pickBackupFolder", json!({}), cx, |_, _| {}))));
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
    if flag(bk, "cloud") && flag(bk, "weakPassword") {
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
            r.ask("backupNow", json!({}), cx, |r, _| r.ok(r.tr("bk.done")))
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
                            r.ask("restoreBackup", json!({}), cx, |_, _| {});
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
    let supported = flag(u, "supported");
    let status = s(u, "status");
    let v = s(u, "version");
    // The time; on another day than today, the date too (UpdatesPage.tsx).
    let when = u.get("checkedAt").and_then(Value::as_str).and_then(crate::time::iso_secs).map(|at| {
        let l = at + crate::time::utc_offset_secs();
        let hm = format!("{:02}:{:02}", l.rem_euclid(86400) / 3600, l.rem_euclid(3600) / 60);
        let today = (crate::time::now_secs() + crate::time::utc_offset_secs()).div_euclid(86400);
        if l.div_euclid(86400) == today { hm } else { format!("{} {hm}", civil_dm(l.div_euclid(86400))) }
    });
    let text = if !supported {
        root.tr("upd.unsupported")
    } else {
        match status {
            "checking" => root.tr("upd.checking"),
            "downloading" => root.i18n.tv("upd.downloading", &[("v", v), ("p", &u.get("progress").map(|p| p.to_string()).unwrap_or_default())]),
            "ready" if flag(u, "quiet") => root.tr("upd.readyQuiet"),
            "ready" => root.i18n.tv(if flag(u, "auto") { "upd.readyAuto" } else { "upd.ready" }, &[("v", v)]),
            "upToDate" => root.tr("upd.latest") + &when.map(|w| format!(" · {}", root.i18n.tv("upd.checkedAt", &[("t", &w)]))).unwrap_or_default(),
            "error" if !s(u, "error").is_empty() => format!("{}: {}", root.tr("upd.error"), s(u, "error")),
            "error" => root.tr("upd.error"),
            _ if flag(u, "auto") => root.tr("upd.idle"),
            _ => root.tr("upd.off"),
        }
    };
    let ready = status == "ready" && !flag(u, "quiet");
    let ctl: AnyElement = if !supported {
        div().into_any_element()
    } else if ready {
        Button::new("s-upd-apply", root.tr("upd.restart")).icon("refresh").small().primary().on_click(cx.listener(|r, _, _, cx| r.ask("applyUpdate", json!({}), cx, |_, _| {}))).into_any_element()
    } else {
        Button::new("s-upd-check", root.tr("upd.check"))
            .icon("refresh")
            .small()
            .disabled(matches!(status, "checking" | "downloading" | "ready"))
            .on_click(cx.listener(|r, _, _, cx| r.ask("checkUpdates", json!({}), cx, |_, _| {})))
            .into_any_element()
    };
    let upd = vec![
        row(format!("Bloxkeep {}", s(u, "current")), Some(text), ctl),
        row(root.tr("upd.auto"), None, switch("s-autoupd", flag(u, "auto"), cx, |r, on, cx| r.ask("setAutoUpdate", json!({ "enabled": on }), cx, |_, _| {})).disabled(!supported)),
        row(
            root.tr("upd.beta"),
            Some(root.tr("upd.betaDesc")),
            switch("s-betaupd", flag(u, "beta"), cx, |r, on, cx| r.ask("setBetaUpdates", json!({ "enabled": on }), cx, |_, _| {})).disabled(!supported),
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
            switch("s-boot", flag(&root.state, "startWithWindows"), cx, |r, on, cx| r.ask("setStartWithWindows", json!({ "enabled": on }), cx, |_, _| {})),
        ),
        row(
            root.tr("set.tray"),
            Some(root.tr("set.trayDesc")),
            switch("s-tray", root.state.get("closeToTray").and_then(Value::as_bool).unwrap_or(true), cx, |r, on, cx| {
                r.ask("setCloseToTray", json!({ "enabled": on }), cx, |_, _| {})
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
            .child(switch("s-hotkeys", keys_on, cx, |r, on, cx| r.ask("setHotkeys", json!({ "enabled": on }), cx, |_, _| {})))
            .into_any_element(),
        // Hardware acceleration of the windows still drawn as web pages (mini window, macro panel, sign-in browser):
        // off is the safe rendering WebView2 starts with (UpdatesPage.tsx's s-gpu).
        row(
            root.tr("set.gpu"),
            Some(root.tr("set.gpuDesc")),
            switch("s-gpu", !flag(&root.state, "safeRendering"), cx, |r, on, cx| r.ask("setRendering", json!({ "gpu": on }), cx, |_, _| {})),
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
        switch("s-online", counted, cx, |r, on, cx| r.ask("setCountOnline", json!({ "enabled": on }), cx, |_, _| {})),
    )];
    vec![
        group(Some(root.tr("upd.title")), upd),
        group(Some(root.tr("set.g.app")), app),
        group(Some(root.tr("fb.group")), help),
        group(Some(root.tr("onl.group")), privacy),
    ]
}
