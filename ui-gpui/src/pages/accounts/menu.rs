// The account menu (app.js's cardMenu): a right click on a card, a tile or a list row, or its ⋮. On a picked account
// it acts on all those picked; on any other, on that one alone. In groups, as the web page's: the window, its server,
// the account, Copy, then what closes or deletes; what is about one account only isn't offered for several, and what
// can't apply isn't either. Its second lists (the FPS limit, Copy, Run macro) open in its place. The commands are the
// app's, as the web page sends them.
use std::rc::Rc;

use gpui_kit::{prelude::*, *};
use serde_json::{Value, json};

use super::{act, ask_delete, check_accounts, conn, copy, held, hidden, id_of, is_local, label, rejoin_on, s};
use crate::{kit::*, shell::Root, theme::*};

/// The Accounts page's own state that Root doesn't hold: the layout picked here, and the account menu.
#[derive(Default)]
pub struct Ui {
    /// The layout picked here (true: the list) until the app's settings say the same; None: as the settings say.
    pub list: Option<bool>,
    pub menu: Option<Menu>,
    /// The menu a click outside closed just now: that click on its own ⋮ must not open it again.
    closed: Option<(String, std::time::Instant)>,
    /// The bar's Job ID box (made the first time its button is pressed: a box needs the window) and whether it shows.
    pub job: Option<Entity<gpui_kit::component::input::InputState>>,
    pub job_open: bool,
    /// The game the Job ID goes with (app.js's S.jobPlace): the last one the game box was known to hold.
    pub job_place: Option<i64>,
    /// The Job ID box to be emptied at the next frame (where no window was at hand: a lookup's answer, Play).
    pub job_stale: bool,
    /// Ctrl+L or "/" with none picked: the bar shows anyway, its game box focused, until Esc or another page (the
    /// Root's page-change count it was asked at).
    pub place_wanted: Option<u64>,
    /// The header's refresh is waiting for the app (its icon turns until then).
    pub refreshing: bool,
    #[cfg(feature = "shot")]
    shot_done: bool,
    #[cfg(feature = "shot")]
    pub shot_place_done: bool,
}
impl Global for Ui {}

/// The menu open (or on its way out): whose, where (None: under its ⋮; else where the right click was) and which list.
#[derive(Clone)]
pub struct Menu {
    pub id: String,
    pub at: Option<Point<Pixels>>,
    pub sub: Sub,
    pub open: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Sub {
    Main,
    Fps,
    Copy,
    Macro,
}

/// The ⋮ pressed: its menu opens under it, or closes when it was open there.
pub fn open_button(id: String, cx: &mut App) {
    let ui = cx.default_global::<Ui>();
    let just = ui.closed.take().is_some_and(|(k, at)| k == id && at.elapsed().as_millis() < 300);
    let there = ui.menu.as_ref().is_some_and(|m| m.open && m.id == id && m.at.is_none());
    if just || there {
        if let Some(m) = ui.menu.as_mut() {
            m.open = false;
        }
    } else {
        ui.menu = Some(Menu { id, at: None, sub: Sub::Main, open: true });
    }
}

/// A right click: the menu opens where the mouse is.
pub fn open_at(id: String, at: Point<Pixels>, cx: &mut App) {
    cx.default_global::<Ui>().menu = Some(Menu { id, at: Some(at), sub: Sub::Main, open: true });
}

pub fn is_open(cx: &App) -> bool {
    cx.try_global::<Ui>().and_then(|u| u.menu.as_ref()).is_some_and(|m| m.open)
}

/// This account's menu is open (under its ⋮ when `button`).
pub fn open_here(id: &str, button: bool, cx: &App) -> bool {
    cx.try_global::<Ui>().and_then(|u| u.menu.as_ref()).is_some_and(|m| m.open && m.id == id && m.at.is_none() == button)
}

pub fn shut(cx: &mut App) {
    if let Some(m) = cx.default_global::<Ui>().menu.as_mut() {
        m.open = false;
    }
}

/// A shot (BLOXKEEP_SHOT_LAYOUT=list) shows the list layout; BLOXKEEP_SHOT_CARDMENU="Farm 1" opens that account's
/// menu under its ⋮, BLOXKEEP_SHOT_CARDSUB=fps|copy|macro on that second list. Once, when the accounts are there.
#[cfg(feature = "shot")]
pub fn shot_hooks(root: &Root, cx: &mut App) {
    if root.accounts().is_empty() || cx.try_global::<Ui>().is_some_and(|u| u.shot_done) {
        return;
    }
    let list = std::env::var("BLOXKEEP_SHOT_LAYOUT").ok().map(|v| v == "list");
    let menu = std::env::var("BLOXKEEP_SHOT_CARDMENU").ok().and_then(|name| root.accounts().iter().find(|a| label(a) == name).map(id_of));
    let sub = match std::env::var("BLOXKEEP_SHOT_CARDSUB").as_deref() {
        Ok("fps") => Sub::Fps,
        Ok("copy") => Sub::Copy,
        Ok("macro") => Sub::Macro,
        _ => Sub::Main,
    };
    // BLOXKEEP_SHOT_SCROLL=<px>: the page scrolled down that far (to the bottom when it is shorter).
    if let Some(y) = std::env::var("BLOXKEEP_SHOT_SCROLL").ok().and_then(|v| v.parse::<f32>().ok()) {
        crate::scrolled::handle("acc-body").set_offset(point(px(0.), px(-y)));
    }
    let ui = cx.default_global::<Ui>();
    ui.shot_done = true;
    if list.is_some() {
        ui.list = list;
    }
    if let Some(id) = menu {
        ui.menu = Some(Menu { id, at: None, sub, open: true });
    }
}

/// What picking a line does: another list in the menu's place, or something done (the menu closes first).
type Run = Rc<dyn Fn(&mut Root, &mut Window, &mut Context<Root>)>;
enum Go {
    Sub(Sub),
    Run(Run),
    Nothing,
}

/// A line of the menu.
struct Line {
    id: ElementId,
    icon: Option<&'static str>,
    text: String,
    /// Its key on the right (Enter, F2, Del) or what it is set to now ("60 FPS").
    hint: Option<String>,
    /// A tick: on, off, or some of them (a bar).
    tick: Option<Tick>,
    danger: bool,
    off: bool,
    tip: Option<String>,
    go: Go,
}

#[derive(Clone, Copy)]
enum Tick {
    On,
    Off,
    Some,
}

fn line(id: impl Into<ElementId>, icon: Option<&'static str>, text: String, go: Go) -> Line {
    Line { id: id.into(), icon, text, hint: None, tick: None, danger: false, off: false, tip: None, go }
}
fn run(f: impl Fn(&mut Root, &mut Window, &mut Context<Root>) + 'static) -> Go {
    Go::Run(Rc::new(f))
}

/// The server an account is in, when it can be followed: its place and job.
pub(crate) fn server_of(a: &Value) -> Option<(i64, String)> {
    let pick = |job: Option<&str>, places: [Option<i64>; 2]| {
        let job = job.filter(|j| !j.is_empty())?;
        let place = places.into_iter().flatten().find(|p| *p > 0)?;
        Some((place, job.to_string()))
    };
    if is_local(a) && conn(a) == "ingame" {
        if let Some(found) = pick(
            a.pointer("/session/jobId").and_then(Value::as_str),
            [a.pointer("/session/jobPlaceId").and_then(Value::as_i64), a.pointer("/session/game/placeId").and_then(Value::as_i64)],
        ) {
            return Some(found);
        }
    }
    if a.pointer("/presence/type").and_then(Value::as_str) == Some("ingame") {
        return pick(
            a.pointer("/presence/jobId").and_then(Value::as_str),
            [a.pointer("/presence/joinPlaceId").and_then(Value::as_i64), a.pointer("/presence/placeId").and_then(Value::as_i64)],
        );
    }
    None
}

const FPS_CAPS: [i64; 8] = [15, 20, 30, 45, 60, 120, 144, 240];

fn main_lines(root: &Root, a: &Value, ids: &[String], picked: &[&Value]) -> Vec<Option<Line>> {
    let id = id_of(a);
    let many = ids.len() > 1;
    let n = ids.len();
    let flag = |x: &Value, k: &str| x.get(k).and_then(Value::as_bool).unwrap_or(false);
    let any_local = picked.iter().any(|x| is_local(x));
    let rejoins = picked.iter().filter(|x| rejoin_on(x)).count();
    let all_main = picked.iter().all(|x| flag(x, "isMain"));
    let mut out: Vec<Option<Line>> = Vec::new();
    let ids_of = || ids.to_vec();

    let go = ids_of();
    let mut play = line("cm-play", Some("play"), if many { root.i18n.tv("m.playN", &[("n", &n.to_string())]) } else { root.tr("m.play") }, run(move |r, _, cx| r.play(&go, cx)));
    play.hint = Some("Enter".into());
    out.push(Some(play));
    let go = ids_of();
    out.push(Some(line(
        "cm-app",
        Some("monitor"),
        root.tr("m.openApp"),
        run(move |r, _, cx| r.launch("launch", json!({ "ids": go, "appOnly": true, "placeInput": "", "jobId": "" }), false, cx)),
    )));
    // The window goes (and its taskbar button), the game runs on; shown again from here.
    if picked.iter().any(|x| is_local(x) && !hidden(x)) {
        let go = ids_of();
        out.push(Some(line("cm-hide", Some("eyeOff"), root.tr("m.hideWindow"), run(move |r, _, _| r.send("hideWindows", json!({ "ids": go }))))));
    }
    if picked.iter().any(|x| hidden(x)) {
        let go = ids_of();
        out.push(Some(line("cm-show", Some("eye"), root.tr("m.showWindow"), run(move |r, _, _| r.send("showWindows", json!({ "ids": go }))))));
    }
    // A macro on these windows only: picked from a second list in the menu's place.
    let mut mac = line("cm-macro", Some("keyboard"), root.tr("macro.runOn"), Go::Sub(Sub::Macro));
    if !any_local {
        mac.off = true;
        mac.tip = Some(root.tr("macro.runOnNoWindow"));
    }
    out.push(Some(mac));
    out.push(None);
    // Its server: put in the bar (game and Job ID), for the accounts picked to Play into, as the web page did.
    if !many && let Some((place, job)) = server_of(a) {
        let name = super::label(a);
        out.push(Some(line(
            "cm-follow",
            Some("server"),
            root.tr("m.follow"),
            run(move |r, window, cx| {
                super::use_server(r, place, job.clone(), window, cx);
                r.toast = Some((r.i18n.tv("srv.followed", &[("name", &name)]), "ok".into()));
            }),
        )));
    }
    // The window closes and opens again into another server of its game.
    if picked.iter().any(|x| is_local(x) && server_of(x).is_some()) {
        let go = ids_of();
        out.push(Some(line("cm-switch", Some("shuffle"), root.tr("m.switchServer"), run(move |r, _, _| r.send("switchServer", json!({ "ids": go }))))));
    }
    // Ticked as the card shows it (a running window's preset may decide, and says so); some on and some off shows a bar,
    // and a click turns it on for all.
    {
        let go = ids_of();
        let enable = rejoins < n;
        let mut l = line(
            "cm-rejoin",
            Some("repeat"),
            root.tr("m.autoRejoin"),
            run(move |r, _, cx| {
                act(r, "setAutoRejoin", json!({ "ids": go, "enabled": enable }), cx, move |r, v| {
                    let k = v.as_i64().unwrap_or(0);
                    r.toast = Some((r.i18n.tn(if enable { "t.rejoinOn" } else { "t.rejoinOff" }, k, &[]), "ok".into()));
                })
            }),
        );
        l.tick = Some(if rejoins == n { Tick::On } else if rejoins > 0 { Tick::Some } else { Tick::Off });
        let preset = picked.iter().any(|x| {
            let own = x.pointer("/session/autoRejoin").and_then(Value::as_bool);
            own.is_some() && own != x.get("autoRejoin").and_then(Value::as_bool)
        });
        if preset {
            l.tip = Some(root.tr("m.rejoinPreset"));
        }
        out.push(Some(l));
    }
    out.push(None);
    if !many {
        let me = id.clone();
        let mut l = line("cm-edit", Some("edit"), root.tr("m.edit"), run(move |r, window, cx| crate::pages::account_form::open(r, Some(me.clone()), window, cx)));
        l.hint = Some("F2".into());
        out.push(Some(l));
    }
    let go = ids_of();
    out.push(Some(line("cm-check", Some("refresh"), root.tr("m.check"), run(move |r, _, cx| check_accounts(r, go.clone(), cx)))));
    // Roblox holds its join for a bot check: solved in Bloxkeep's own window, then the window opens again.
    if !many && held(a) {
        let me = id.clone();
        out.push(Some(line("cm-bot", Some("shield"), root.tr("m.botCheck"), run(move |r, _, _| r.send("solveBotCheck", json!({ "id": me }))))));
    }
    if root.state.pointer("/settings/robloxConfig/mode").and_then(Value::as_str) == Some("marked") {
        let go = ids_of();
        let all = picked.iter().all(|x| flag(x, "useRobloxConfig"));
        let mut l = line(
            "cm-rc",
            Some("sliders"),
            root.tr("m.robloxConfig"),
            run(move |r, _, cx| {
                let enable = !all;
                act(r, "setUseRobloxConfig", json!({ "ids": go, "enabled": enable }), cx, move |r, v| {
                    let k = v.as_i64().unwrap_or(0);
                    r.toast = Some((r.i18n.tn(if enable { "t.rcOn" } else { "t.rcOff" }, k, &[]), "ok".into()));
                })
            }),
        );
        l.tick = Some(if all { Tick::On } else { Tick::Off });
        out.push(Some(l));
    }
    {
        let mut l = line("cm-fps", Some("sliders"), root.tr("m.fps"), Go::Sub(Sub::Fps));
        let cap = a.get("fpsCap").and_then(Value::as_i64).unwrap_or(0);
        if !many && cap > 0 {
            l.hint = Some(format!("{cap} FPS"));
        }
        out.push(Some(l));
    }
    // The one played on: left out of Select all, and its window of Save RAM, the RAM limit and Arrange windows.
    {
        let go = ids_of();
        let mut l = line("cm-main", Some(if all_main { "starFill" } else { "star" }), root.tr("m.main"), run(move |r, _, _| r.send("setMain", json!({ "ids": go, "on": !all_main }))));
        l.tick = Some(if all_main { Tick::On } else { Tick::Off });
        l.tip = Some(root.tr("m.mainTip"));
        out.push(Some(l));
    }
    // Every pair of these becomes Roblox friends; a check Roblox asks is shown to solve.
    if many {
        let go = ids_of();
        let able = picked.iter().filter(|x| s(x, "realm") != "vng" && x.get("cookieValid").and_then(Value::as_bool) != Some(false)).count();
        let mut l = line("cm-friends", Some("userPlus"), root.tr("m.friends"), run(move |r, _, _| r.send("makeFriends", json!({ "ids": go }))));
        l.off = able < 2;
        l.tip = Some(root.tr("m.friendsTip"));
        out.push(Some(l));
    } else {
        if let Some(user) = a.get("userId").and_then(Value::as_i64).filter(|u| *u > 0) {
            out.push(Some(line("cm-profile", Some("external"), root.tr("m.profile"), run(move |r, _, _| r.send("openProfile", json!({ "userId": user }))))));
        }
        // roblox.com signed in as this account, in a window of Bloxkeep's own.
        let me = id.clone();
        let mut l = line("cm-web", Some("globe"), root.tr("m.website"), run(move |r, _, _| r.send("openWebsite", json!({ "id": me }))));
        if s(a, "realm") == "vng" {
            l.off = true;
            l.tip = Some(root.tr("card.vngNoWebsite"));
        }
        out.push(Some(l));
        out.push(None);
        out.push(Some(line("cm-copy", Some("copy"), root.tr("m.copy"), Go::Sub(Sub::Copy))));
    }
    out.push(None);
    if any_local {
        let go = ids_of();
        out.push(Some(line("cm-close", Some("power"), root.tr("m.close"), run(move |r, _, cx| super::close(r, go.clone(), false, cx)))));
        let go = ids_of();
        let mut l = line("cm-kill", Some("x"), root.tr("m.kill"), run(move |r, _, cx| super::close(r, go.clone(), true, cx)));
        l.danger = true;
        out.push(Some(l));
    }
    {
        let go = ids_of();
        let names: Vec<String> = picked.iter().map(|x| label(x)).collect();
        let text = if many { root.i18n.tv("m.deleteN", &[("n", &n.to_string())]) } else { root.tr("m.delete") };
        let mut l = line("cm-delete", Some("trash"), text, run(move |r, window, cx| ask_delete(r, go.clone(), names.clone(), window, cx)));
        l.danger = true;
        l.hint = Some("Del".into());
        out.push(Some(l));
    }
    out
}

/// Each account's own frame rate limit in Roblox (setFpsCap), from its next opening.
fn fps_lines(root: &Root, ids: &[String], picked: &[&Value]) -> Vec<Option<Line>> {
    let mut caps: Vec<i64> = picked.iter().map(|x| x.get("fpsCap").and_then(Value::as_i64).unwrap_or(0)).collect();
    caps.sort_unstable();
    caps.dedup();
    let now = (caps.len() == 1).then(|| caps[0]);
    std::iter::once(0)
        .chain(FPS_CAPS)
        .enumerate()
        .map(|(i, fps)| {
            let go = ids.to_vec();
            let text = if fps == 0 { root.tr("m.fpsNone") } else { format!("{fps} FPS") };
            let mut l = line(("cm-fps-to", i), None, text, run(move |r, _, cx| {
                act(r, "setFpsCap", json!({ "ids": go, "fps": fps }), cx, move |r, v| {
                    let k = v.as_i64().unwrap_or(0);
                    r.toast = Some((r.i18n.tn(if fps > 0 { "t.fpsOn" } else { "t.fpsOff" }, k, &[("fps", &fps.to_string())]), "ok".into()));
                })
            }));
            l.tick = Some(if now == Some(fps) { Tick::On } else { Tick::Off });
            Some(l)
        })
        .collect()
}

/// Copy: the username, the password, a 2FA code; nothing saved to copy, picking it says why.
fn copy_lines(root: &Root, a: &Value) -> Vec<Option<Line>> {
    let flag = |k: &str| a.get(k).and_then(Value::as_bool).unwrap_or(false);
    [("cm-copy-user", "m.copyUser", "username", true, ""), ("cm-copy-pw", "m.copyPw", "password", flag("hasPassword"), "m.noPw"), ("cm-copy-code", "m.copyCode", "totp", flag("hasAuthenticator"), "m.noCode")]
        .into_iter()
        .map(|(key, text, field, has, none)| {
            let me = id_of(a);
            let mut l = line(key, None, root.tr(text), run(move |r, _, cx| {
                if has {
                    copy(r, me.clone(), field, cx);
                } else {
                    r.toast = Some((r.tr(none), "info".into()));
                }
            }));
            if !has {
                l.tip = Some(root.tr(none));
            }
            Some(l)
        })
        .collect()
}

/// Run a macro on these windows only.
fn macro_lines(root: &Root, ids: &[String]) -> Vec<Option<Line>> {
    let list = root.state.pointer("/settings/macros").and_then(Value::as_array).cloned().unwrap_or_default();
    if list.is_empty() {
        let mut l = line("cm-macro-none", Some("keyboard"), root.tr("macro.noneYet"), Go::Nothing);
        l.off = true;
        // (And a way to make one, as app.js's sub-menu had: to Macros, a new one started.)
        let new = line("cm-macro-new", Some("plus"), root.tr("macro.newPal"), run(|r, window, cx| {
            r.auto.tab = crate::pages::auto::Tab::Macros;
            r.auto.confirm = None;
            r.go(crate::pages::Page::Auto);
            crate::pages::macro_form::open(r, None, window, cx);
        }));
        return vec![Some(l), Some(new)];
    }
    list.iter()
        .enumerate()
        .map(|(i, m)| {
            let (mid, go) = (s(m, "id").to_string(), ids.to_vec());
            let mut l = line(("cm-macro-run", i), Some("play"), s(m, "name").to_string(), run(move |r, _, cx| {
                r.run_macro(mid.clone(), Some(go.clone()), cx)
            }));
            l.hint = m.get("hotkey").and_then(Value::as_str).filter(|k| !k.is_empty()).map(|k| format!("Ctrl+Alt+{k}"));
            Some(l)
        })
        .collect()
}

/// This account's menu where it opens: under its ⋮ (`button`) or where the right click was; None while it is shut
/// (and gone) or open elsewhere.
pub fn place(root: &Root, a: &Value, button: bool, cx: &mut Context<Root>) -> Option<AnyElement> {
    // (Whose it is looked at before the menu is copied: this runs for every card each frame.)
    let mine = cx.try_global::<Ui>()?.menu.as_ref().is_some_and(|m| m.id == s(a, "id") && m.at.is_none() == button);
    if !mine {
        return None;
    }
    let m = cx.try_global::<Ui>()?.menu.clone()?;
    let key = SharedString::from(format!("cm-pop-{}-{}", m.id, if button { "b" } else { "p" }));
    let menu = pop(key, m.open, body(root, a, &m, cx));
    Some(match m.at {
        None => div().absolute().top(px(32.)).right_0().child(deferred(anchored().anchor(Anchor::TopRight).snap_to_window_with_margin(px(8.)).child(menu)).with_priority(2)).into_any_element(),
        Some(at) => deferred(anchored().position(at).snap_to_window_with_margin(px(8.)).child(menu)).with_priority(2).into_any_element(),
    })
}

fn body(root: &Root, a: &Value, m: &Menu, cx: &mut Context<Root>) -> Stateful<Div> {
    let id = id_of(a);
    let all = root.accounts();
    // On a picked account: all those picked; on any other, that one alone (the ticks left as they are).
    let ids: Vec<String> = if root.selected.contains(&id) { all.iter().filter(|x| root.selected.contains(s(x, "id"))).map(id_of).collect() } else { vec![id.clone()] };
    let picked: Vec<&Value> = ids.iter().filter_map(|i| all.iter().find(|x| s(x, "id") == i)).collect();
    let lines = match m.sub {
        Sub::Main => main_lines(root, a, &ids, &picked),
        Sub::Fps => fps_lines(root, &ids, &picked),
        Sub::Copy => copy_lines(root, a),
        Sub::Macro => macro_lines(root, &ids),
    };
    let (mid, mat) = (m.id.clone(), m.at);
    let mut menu = div()
        .id(SharedString::from(format!("cm-{}", m.id)))
        .occlude()
        .relative()
        .w(px(if m.sub == Sub::Main { 260. } else { 220. }))
        .p(px(4.))
        .rounded(px(R_M))
        .bg(c(RAISED_2))
        .border_1()
        .border_color(ca(WHITE, 0.1))
        .shadow_lg()
        .flex()
        .flex_col()
        // (No live picture of a Roblox window over it.)
        .child(root.live.cover())
        .on_mouse_down_out(cx.listener(move |_, _, _, cx| {
            let ui = cx.default_global::<Ui>();
            let mine = ui.menu.as_ref().is_some_and(|m| m.open && m.id == mid && m.at == mat);
            if mine {
                if let Some(m) = ui.menu.as_mut() {
                    m.open = false;
                }
                if mat.is_none() {
                    ui.closed = Some((mid.clone(), std::time::Instant::now()));
                }
                cx.notify();
            }
        }));
    // Its head: whose menu it is (the face and name, or how many are picked); in a second list, the way back and
    // what the list is.
    menu = match m.sub {
        Sub::Main => menu.child(
            div()
                .px(px(8.))
                .pt(px(6.))
                .pb(px(8.))
                .flex()
                .items_center()
                .gap(px(10.))
                .child(super::avatar(a, 32., RAISED_2, cx))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(div().truncate().text_size(px(T_L)).font_weight(FontWeight::BOLD).child(if ids.len() > 1 { root.i18n.tv("stats.accountsN", &[("n", &ids.len().to_string())]) } else { label(a) }))
                        .child(div().truncate().text_size(px(T_S)).text_color(c(MUTED)).child(if ids.len() > 1 { label(a) } else { format!("@{}", s(a, "username")) })),
                ),
        ),
        sub => {
            let title = root.tr(match sub {
                Sub::Fps => "m.fps",
                Sub::Copy => "m.copy",
                _ => "macro.runOn",
            });
            menu.child(
                menu_row("cm-back")
                    .h(px(34.))
                    .gap(px(8.))
                    .text_color(c(TEXT))
                    .font_weight(FontWeight::BOLD)
                    .child(icon("chevronLeft", 14., c(TEXT_2)))
                    .child(div().flex_1().min_w_0().truncate().child(title))
                    .on_click(cx.listener(|_, _, _, cx| {
                        if let Some(m) = cx.default_global::<Ui>().menu.as_mut() {
                            m.sub = Sub::Main;
                        }
                        cx.notify();
                    })),
            )
        }
    };
    menu = menu.child(div().h(px(1.)).mb(px(4.)).mx(px(6.)).bg(ca(WHITE, 0.08)));
    for l in lines {
        menu = match l {
            None => menu.child(div().h(px(1.)).my(px(4.)).mx(px(6.)).bg(ca(WHITE, 0.08))),
            Some(l) => menu.child(row(l, cx)),
        };
    }
    menu
}

fn row(l: Line, cx: &mut Context<Root>) -> impl IntoElement {
    let (fg, mark) = if l.off {
        (DIM, DIM)
    } else if l.danger {
        (BAD, BAD)
    } else {
        (TEXT, TEXT_2)
    };
    menu_row(l.id)
        .h(px(32.))
        .gap(px(10.))
        .text_color(c(fg))
        .when(l.off, |d| d.cursor_default())
        .when_some(l.icon, |d, name| d.child(icon(name, 14., c(mark))))
        .child(div().flex_1().min_w_0().truncate().child(l.text))
        .when_some(l.hint, |d, h| {
            d.child(div().h(px(18.)).px(px(6.)).flex().flex_none().items_center().rounded(px(5.)).border_1().border_color(c(LINE)).text_size(px(T_XS)).text_color(c(MUTED)).child(h))
        })
        .when_some(l.tick, |d, t| {
            d.child(div().w(px(16.)).flex_none().flex().items_center().justify_center().map(|d| match t {
                // (What is on: a small solid yellow block.)
                Tick::On => d.child(tick_mark(16.)),
                Tick::Some => d.child(div().w(px(9.)).h(px(2.)).rounded_full().bg(c(TEXT_2))),
                Tick::Off => d,
            }))
        })
        .when_some(l.tip, |d, t| {
            let t: SharedString = t.into();
            d.tooltip(move |_, cx| cx.new(|_| Tip(t.clone())).into())
        })
        .map(|d| match l.go {
            Go::Nothing => d,
            _ if l.off => d,
            Go::Sub(to) => d.on_click(cx.listener(move |_, _, _, cx| {
                if let Some(m) = cx.default_global::<Ui>().menu.as_mut() {
                    m.sub = to;
                }
                cx.notify();
            })),
            Go::Run(f) => d.on_click(cx.listener(move |r, _, window, cx| {
                shut(cx);
                f(r, window, cx);
                cx.notify();
            })),
        })
}
