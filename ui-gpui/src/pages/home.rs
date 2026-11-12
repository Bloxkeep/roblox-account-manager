// Home: the game the accounts play now on a stage of its own picture, the PC's figures in four cards, the games (this
// PC's, or what Bloxkeep's users play) in a tabbed card, and the presets to run in one click. Every figure and button
// is the app's own: the games come from the accounts' sessions and the play history (playedGames), the pictures from
// Roblox (gameLooks), the figures from the metrics, the presets from the settings (runPreset).
//
// One thing leads: the stage, and on it what needs you (a dropped account's Rejoin, in orange). The yellow is kept for
// the stage's one main button; everything else is grey until it matters.
use gpui_kit::{prelude::*, *};
use serde_json::{Value, json};

use crate::{
    images,
    kit::*,
    pages::Page,
    shell::{HomeTab, Root},
    theme::*,
};

/// A game on Home: running here (with its accounts), or played lately.
pub struct Game {
    pub place: i64,
    pub name: String,
    pub icon: Option<String>,
    pub accounts: Vec<Value>,
    pub hours: f64,
}

/// A game's name without the event tag games put in front of it ("[🎃] Adopt Me!" → "Adopt Me!").
pub fn clean(name: &str) -> String {
    let t = name.trim();
    if t.starts_with('[') {
        if let Some(i) = t.find(']') {
            let rest = t[i + 1..].trim();
            if !rest.is_empty() {
                return rest.to_string();
            }
        }
    }
    t.to_string()
}

pub fn is_dropped(a: &Value) -> bool {
    a.get("state").and_then(Value::as_str) == Some("running") && a.pointer("/session/connection").and_then(Value::as_str) == Some("disconnected")
}

/// Running games first (most accounts first), then those played lately that aren't running.
pub fn games(root: &Root) -> Vec<Game> {
    let mut now: Vec<Game> = Vec::new();
    for a in root.accounts() {
        let st = a.get("state").and_then(Value::as_str).unwrap_or_default();
        let Some(place) = a.pointer("/session/game/placeId").and_then(Value::as_i64) else { continue };
        if st != "running" && st != "launching" {
            continue;
        }
        match now.iter_mut().find(|g| g.place == place) {
            Some(g) => g.accounts.push(a.clone()),
            None => now.push(Game {
                place,
                name: clean(a.pointer("/session/game/name").and_then(Value::as_str).unwrap_or_default()),
                icon: a.pointer("/session/game/iconUrl").and_then(Value::as_str).map(str::to_string),
                accounts: vec![a.clone()],
                hours: 0.,
            }),
        }
    }
    now.sort_by(|x, y| y.accounts.len().cmp(&x.accounts.len()).then(x.name.cmp(&y.name)));
    let bare = |s: &str| clean(s).to_lowercase();
    let mut recent: Vec<(f64, Game)> = root
        .played
        .iter()
        .flatten()
        .filter_map(|p| {
            let place = p.get("place").and_then(Value::as_i64)?;
            let name = clean(p.get("name").and_then(Value::as_str)?);
            if place <= 0 || name.is_empty() || now.iter().any(|g| g.place == place || bare(&g.name) == bare(&name)) {
                return None;
            }
            let last = p.get("last").and_then(Value::as_f64).unwrap_or(0.);
            Some((last, Game { place, name, icon: None, accounts: Vec::new(), hours: p.get("hours").and_then(Value::as_f64).unwrap_or(0.) }))
        })
        .collect();
    recent.sort_by(|a, b| b.0.total_cmp(&a.0));
    now.extend(recent.into_iter().map(|(_, g)| g).take(12));
    now
}

fn look<'a>(root: &'a Root, place: i64, part: &str) -> Option<&'a str> {
    root.looks.get(&place).and_then(|l| l.get(part)).and_then(Value::as_str).filter(|s| !s.is_empty())
}

/// The game on the stage: the one picked on the shelf, else the first (running, most accounts).
pub fn hero_place(root: &Root) -> Option<i64> {
    let all = games(root);
    root.picked.filter(|p| all.iter().any(|g| g.place == *p)).or_else(|| all.first().map(|g| g.place))
}

/// One surface, like the accounts column beside it: the stage's picture bleeds into it at the top and everything
/// else sits on it in tone (no boxes in boxes).
pub fn home(root: &Root, width: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let all = games(root);
    let focus = root.picked.and_then(|p| all.iter().position(|g| g.place == p)).unwrap_or(0);
    let inner = width - 2. * SIDE;
    div()
        .id("home")
        .size_full()
        .rounded(px(R_PANEL))
        .bg(c(PANEL))
        .overflow_y_scroll()
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(24.))
                .pb(px(SECTION_GAP))
                .child(stage(root, &all, focus, width, cx))
                // The rows under the stage, on the page's rhythm: the figures rise over the picture's faded foot,
                // the games and the presets a section apart.
                .child(
                    div()
                        .px(px(SIDE))
                        .flex()
                        .flex_col()
                        .gap(px(SECTION_GAP))
                        // (With no Roblox open the four cards said 0 / 8, 0 %, 0.0 GB: the status bar has the PC's CPU
                        // and RAM then.)
                        .when(playing_now(root) > 0, |d| d.child(figures(root, inner)))
                        .child(shelf(root, &all, focus, inner, cx))
                        .when(root.accounts().is_empty(), |d| d.child(get_started(root, cx)))
                        .child(presets(root, inner, cx)),
                ),
        )
}

/// How tall the stage's picture stands: near a third of the surface's width, within what a small window can give and
/// a big one should take.
fn art_h(width: f32) -> f32 {
    // (It was 36 % of the width, 320 to 500: the stage took near half the first screen and the presets fell off its
    // foot on the owner's 1707×1020.)
    // (440 at most left a 2560×1440 window's lower half empty, 2026-11-12: past 2000 px it may grow to 560.)
    (width * 0.30).clamp(300., if width > 2000. { 560. } else { 440. }).round()
}

/// How far the rows under the stage rise over the picture's faded foot.
const RISE: f32 = 96.;

/// How many accounts are in a Roblox window now.
pub fn playing_now(root: &Root) -> usize {
    root.accounts().iter().filter(|a| a.get("state").and_then(Value::as_str) == Some("running")).count()
}

/// The game on the stage, on its own picture: what it is doing, and what to do with it.
fn stage(root: &Root, all: &[Game], focus: usize, width: f32, cx: &mut Context<Root>) -> impl IntoElement {
    // The picture is taller than the stage's own block, so the rows below rise over its faded foot (layout C of the
    // mockup the owner picked, 2026-11-09: the game's picture fills the top of the surface edge to edge and fades
    // into it).
    let art = art_h(width);
    let h = art - RISE;
    let base = div().relative().h(px(h)).flex_none();
    let Some(g) = all.get(focus) else {
        let none = root.accounts().is_empty();
        // Nothing played yet: the welcome stands on the picture of the game Bloxkeep's users play most (the stage was
        // a bare dark block with a line of words in its corner, owner 2026-11-12), words at its foot as a game's are.
        let top = root.state.pointer("/community/data/games/0/place").and_then(Value::as_i64);
        let url = top.and_then(|p| look(root, p, "cover")).map(str::to_string);
        let pic = url.as_deref().and_then(|u| images::pic(u, cx)).zip(url.clone());
        return base.when_some(pic, |d, (p, u)| d.child(backdrop(&u, &p, width, art, cx))).child(
            div()
                .absolute()
                .inset_0()
                .px(px(SIDE))
                .pb(px(4.))
                .flex()
                .flex_col()
                .justify_end()
                .gap(px(10.))
                .child(div().text_size(px(40.)).line_height(px(46.)).font_weight(FontWeight::BOLD).child(root.tr(if none { "home.welcome" } else { "home.noGame" })))
                .child(div().max_w(px(520.)).text_size(px(T_L)).text_color(on_picture(0.86)).child(root.tr(if none { "home.welcomeSub" } else { "home.noGameSub" })))
                .child(
                    div().mt(px(6.)).flex().gap(px(8.)).map(|d| {
                        if none {
                            d.child(Button::new("stage-add", root.tr("acc.add")).icon("plus").primary().big().on_click(cx.listener(|r, _, _, cx| r.add_account(cx))))
                        } else {
                            d.child(Button::new("stage-pick", root.tr("home.pickGame")).icon("gamepad").primary().big().on_click(cx.listener(|r, _, _, cx| {
                                r.go(Page::Games);
                                cx.notify();
                            })))
                        }
                    }),
                ),
        );
    };
    let running = !g.accounts.is_empty();
    let n = g.accounts.len() as i64;
    let cover = look(root, g.place, "cover").map(str::to_string);
    let pic = cover.as_deref().and_then(|u| images::pic(u, cx)).zip(cover.clone());
    // The cover itself, sharp at its own size at the stage's right, the words at its left (`backdrop` is it blurred):
    // dropped when the words would have too little room.
    let card_h = (h - 36.).min(300.).max(0.);
    let card_w = (card_h * 16. / 9.).round();
    let card = cover.as_deref().filter(|_| width - card_w - 3. * SIDE >= 440.).and_then(|u| showcase(u, card_w, card_h.round(), cx));
    let words_room = if card.is_some() { card_w + 2. * SIDE } else { SIDE };
    let badge = look(root, g.place, "icon").map(str::to_string).or_else(|| g.icon.clone()).and_then(|u| images::pic(&u, cx));
    let dropped: Vec<Value> = g.accounts.iter().filter(|a| is_dropped(a)).cloned().collect();
    let since = g.accounts.iter().filter_map(|a| a.pointer("/session/since").and_then(Value::as_str).and_then(crate::time::iso_secs)).min();

    // What it is doing, on one line: its state first (a dot and a word), then the facts.
    let mut facts = div().flex().items_center().gap(px(8.)).text_size(px(T_M)).text_color(on_picture(0.86)).child(
        div()
            .flex()
            .items_center()
            .gap(px(6.))
            .text_color(c(TEXT))
            .font_weight(FontWeight::MEDIUM)
            .child(div().size(px(7.)).rounded_full().bg(if running { c(OK) } else { c(MUTED) }))
            .child(root.tr(if running { "home.playing" } else { "home.recent" })),
    );
    if running {
        facts = facts.child(sep()).child(root.i18n.tn("home.accN", n, &[]));
        if !dropped.is_empty() {
            facts = facts.child(sep()).child(div().text_color(c(WARN)).font_weight(FontWeight::MEDIUM).child(root.i18n.tv(
                "ui.droppedN",
                &[("n", &dropped.len().to_string())],
            )));
        }
        if let Some(t) = since {
            facts = facts.child(sep()).child(div().font_features(tnum()).child(crate::time::clock(crate::time::now_secs() - t)));
        }
    } else if g.hours > 0. {
        facts = facts.child(sep()).child(root.i18n.tv("ui.playedH", &[("n", &format!("{:.0}", g.hours.max(1.)))]));
    }

    let mut acts = div().mt(px(6.)).flex().gap(px(8.));
    if running {
        acts = acts.child(Button::new("stage-windows", root.i18n.tn("home.windowsN", n, &[])).icon("screens").primary().big().on_click(
            cx.listener(|r, _, _, cx| {
                r.go(Page::Accounts);
                cx.notify();
            }),
        ));
    }
    if !dropped.is_empty() {
        let label = root.i18n.tv("ui.rejoinN", &[("n", &dropped.len().to_string())]);
        acts = acts.child(Button::new("stage-rejoin", label).icon("refresh").tint().big().on_click(cx.listener(move |r, _, _, cx| r.rejoin(&dropped, cx))));
    }
    // Its page in Games: Roblox's figures, your accounts in it, the quiet hours, its news.
    let (place, name) = (g.place, g.name.clone());
    acts = acts.child(Button::new("stage-game", root.tr("home.gamePage")).icon("info").glass().big().on_click(cx.listener(move |r, _, _, cx| {
        crate::pages::games::open_place(r, place, &name);
        cx.notify();
    })));

    // The picture, edge to edge. It was a 244 px strip of a 16:9 picture on a wide window (the owner's first real
    // run), and then a small picture in the corner (my wrong turn of 2026-11-11, their second screenshot): its height
    // follows the surface's width now, so it stays a picture. It darkens towards the words on its left and runs into
    // the surface below. (In a box of its own: an img placed absolute itself gets no size and draws nothing.)
    base.when_some(pic, |d, (p, u)| d.child(backdrop(&u, &p, width, art, cx)))
        .when_some(card, |d, k| d.child(div().absolute().right(px(SIDE)).bottom(px(4.)).child(k)))
        .child(
            div()
                .absolute()
                .inset_0()
                .pl(px(SIDE))
                .pr(px(words_room))
                .pb(px(4.))
                .flex()
                .flex_col()
                .justify_end()
                .gap(px(10.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(14.))
                        .when_some(badge, |d, b| {
                            d.child(
                                div()
                                    .size(px(52.))
                                    .flex_none()
                                    .rounded(px(R_L))
                                    .overflow_hidden()
                                    .border_1()
                                    .border_color(ca(WHITE, 0.14))
                                    .child(img(b).size_full().object_fit(ObjectFit::Cover)),
                            )
                        })
                        .child(div().min_w_0().max_w(px(560.)).text_size(px(40.)).font_weight(FontWeight::BOLD).line_height(px(46.)).truncate().child(g.name.clone())),
                )
                .child(facts)
                .child(acts),
        )
}


/// No accounts yet: the three steps as cards under the games, each one a click to where it is done (a new user's Home
/// ended in an empty half, owner 2026-11-12).
fn get_started(root: &Root, cx: &mut Context<Root>) -> impl IntoElement {
    let step = |i: usize, key: &'static str, mark: &'static str, cx: &mut Context<Root>| {
        let now = i == 0;
        hot(("home-step", i))
            .flex_1()
            .min_w_0()
            .h(px(76.))
            .px(px(18.))
            .flex()
            .items_center()
            .gap(px(14.))
            .rounded(px(R_L))
            .border_1()
            .cursor_pointer()
            .map(|d| if now { d.tones(tinted(RAISED, FACE, 0.07), tinted(RAISED_2, FACE, 0.10)).edge(ca(FACE, 0.75), c(FACE)) } else { d.tones(c(RAISED), c(RAISED_2)).edge(c(LINE), ca(WHITE, 0.17)) })
            .child(
                div()
                    .size(px(40.))
                    .flex_none()
                    .rounded(px(R_M))
                    .flex()
                    .items_center()
                    .justify_center()
                    .map(|d| if now { d.bg(c(FACE)).child(icon(mark, 20., c(INK))) } else { d.bg(c(INSET)).child(icon(mark, 20., c(TEXT_2))) }),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(div().text_size(px(T_S)).text_color(c(MUTED)).font_features(tnum()).child(format!("{}", i + 1)))
                    .child(div().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).truncate().child(root.tr(key))),
            )
            .on_click(cx.listener(move |r, _, _, cx| {
                match i {
                    0 => r.add_account(cx),
                    1 => r.go(Page::Games),
                    _ => r.go(Page::Accounts),
                }
                cx.notify();
            }))
    };
    div()
        .flex()
        .flex_col()
        .gap(px(12.))
        .child(section_head(root.tr("guide.title"), None))
        .child(
            div()
                .flex()
                .gap(px(GRID_GAP))
                .child(step(0, "guide.add", "userPlus", cx))
                .child(step(1, "guide.game", "gamepad", cx))
                .child(step(2, "guide.play", "play", cx)),
        )
}

/// The stage's picture, edge to edge. A game's cover is 768 px wide at most: stretched across a wide stage it came out
/// smeared, its own lettering muddled under the title (the owner: "rẻ tiền", beside Lunar's, 2026-11-12). So it is drawn
/// blurred on purpose, a wash of the game's colours, and quieter; the cover stands sharp beside the words
/// (`showcase`). It darkens towards the words on its left and runs into the surface below.
fn backdrop(url: &str, p: &std::sync::Arc<Image>, width: f32, art: f32, cx: &mut App) -> impl IntoElement {
    let soft = images::soft(url, cx);
    div()
        .absolute()
        .top_0()
        .left_0()
        .w(px(width))
        .h(px(art))
        // (Cover draws the whole scaled picture; it is the box that cuts it.)
        .rounded_t(px(R_PANEL))
        .overflow_hidden()
        .children(soft.map(|s| fade_in(img(s).size_full().rounded_t(px(R_PANEL)).object_fit(ObjectFit::Cover), p)))
        // (The wash is quieter than the picture was: the sharp cover and the words lead.)
        .child(div().absolute().inset_0().bg(ca(PANEL, 0.30)))
        // (GPUI's see-through gradients come out about half as strong as their stops say, so each fade is laid more
        // than once; measured on the shots, 2026-11-09.)
        .children((0..2).map(|_| {
            div().absolute().inset_0().rounded_t(px(R_PANEL)).bg(linear_gradient(90., linear_color_stop(ca(PANEL, 0.75), 0.), linear_color_stop(ca(PANEL, 0.), 0.6)))
        }))
        .children((0..2).map(|_| {
            div().absolute().inset_0().rounded_t(px(R_PANEL)).bg(linear_gradient(180., linear_color_stop(ca(PANEL, 0.), 0.45), linear_color_stop(c(PANEL), 1.)))
        }))
}

/// The game's cover at its own sharpness, `w`×`h`, on the stage's wash: a hairline round it, no shadow.
fn showcase(url: &str, w: f32, h: f32, cx: &mut App) -> Option<Div> {
    if w < 160. {
        return None;
    }
    let (src, orig) = images::pic_pair(url, w, h, cx)?;
    Some(
        div()
            .w(px(w))
            .h(px(h))
            .flex_none()
            .rounded(px(R_L))
            .overflow_hidden()
            .bg(c(RAISED))
            .border_1()
            .border_color(ca(WHITE, 0.14))
            .child(fade_in(img(src).size_full().object_fit(ObjectFit::Cover), &orig)),
    )
}

/// The page's side margin, the Accounts page's: the stage's words and every row under it start here.
const SIDE: f32 = 24.;

/// The PC in four figures: accounts playing, CPU, RAM, and the RAM Roblox's windows take. (The status bar shows CPU and
/// RAM on the other places, not here.)
fn figures(root: &Root, width: f32) -> impl IntoElement {
    let m = &root.metrics;
    let f = |k: &str| m.get(k).and_then(Value::as_f64).unwrap_or(0.);
    let all = root.accounts();
    let playing = all.iter().filter(|a| a.get("state").and_then(Value::as_str) == Some("running")).count();
    let system = f("systemMb");
    let used = (system - f("availableMb")).max(0.);
    let gb = |mb: f64| format!("{:.1}", mb / 1024.);
    // Four in a row (the figure's detail left out when a card is narrow), two by two when the room is short: the
    // library under them stays in sight on a small window.
    let (cols, w) = grid_fit(width, 150., GRID_GAP);
    let w = if cols >= 4 { ((width - 3. * GRID_GAP) / 4.).floor() } else { ((width - GRID_GAP) / 2.).floor().max(w) };
    grid(GRID_GAP).children([
        // (Named for the accounts, "in a game" at its right: "Playing now" is the stage's line right above it.)
        figure("users", root.tr("nav.accounts"), format!("{playing} / {}", all.len()), Some(root.tr("sys.mood.play")), if all.is_empty() { 0. } else { playing as f64 / all.len() as f64 }, false, w),
        figure("chip", root.tr("ui.cpu"), format!("{:.0}%", f("cpuPct")), None, f("cpuPct") / 100., true, w),
        figure("gauge", root.tr("ui.ram"), format!("{:.0}%", f("usedPct")), Some(format!("{} / {} GB", gb(used), gb(system))), f("usedPct") / 100., true, w),
        figure("screens", root.tr("ui.robloxRam"), format!("{} GB", gb(f("totalMb"))), (system > 0.).then(|| root.i18n.tv("home.ofRam", &[("pct", &format!("{:.0}", f("totalMb") / system * 100.))])), if system > 0. { f("totalMb") / system } else { 0. }, true, w),
    ])
}

/// A figure: its mark on a small block and its name, the value large in tabular figures (with its detail at the
/// right), and a bar that only turns colour when it's high. A count of accounts isn't a load, so its bar stays grey.
#[allow(clippy::too_many_arguments)]
fn figure(icon_name: &'static str, label: String, value: String, extra: Option<String>, part: f64, load: bool, w: f32) -> impl IntoElement {
    let part = part.clamp(0., 1.) as f32;
    let extra = extra.filter(|_| w >= 240.);
    surface()
        .w(px(w))
        .flex_none()
        .px(px(16.))
        .pt(px(14.))
        .pb(px(16.))
        .flex()
        .flex_col()
        .gap(px(10.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .child(div().size(px(28.)).flex_none().rounded(px(R_S)).bg(ca(WHITE, 0.06)).flex().items_center().justify_center().child(icon(icon_name, 15., c(TEXT_2))))
                .child(div().flex_1().min_w_0().truncate().text_size(px(T_M)).font_weight(FontWeight::MEDIUM).text_color(c(TEXT_2)).child(label))
                // Its detail on the name's line, at the right (it sat on the big figure's baseline, low and apart).
                .when_some(extra, |d, e| d.child(div().flex_none().text_size(px(T_S)).text_color(c(MUTED)).font_features(tnum()).child(e))),
        )
        .child(div().text_size(px(T_STAGE)).font_weight(FontWeight::SEMIBOLD).line_height(px(32.)).font_features(tnum()).child(value))
        .child(bar(SharedString::from(format!("fig-{icon_name}")), if load { part } else { part.min(0.79) }, None))
}

/// The games, under a head with two choices: this PC's (running, then played lately) and what Bloxkeep's users play
/// now; their wide pictures one row of the page's grid, each on its game's colour.
fn shelf(root: &Root, all: &[Game], focus: usize, width: f32, cx: &mut Context<Root>) -> impl IntoElement {
    // (220 px at least: three to a row on a 1180 window, where 236 left two big ones; still four at 1707.)
    let (cols, tile_w) = grid_fit(width, 220., GRID_GAP);
    let community: Vec<Value> = root
        .state
        .pointer("/community/data/games")
        .and_then(Value::as_array)
        .map(|l| l.iter().filter(|g| g.get("name").and_then(Value::as_str).is_some_and(|n| !n.is_empty())).cloned().collect())
        .unwrap_or_default();
    // Nothing played yet: the community's games show instead of a line saying there are none (a new user's Home was
    // one empty row, owner 2026-11-12).
    let tab_now = if root.home_tab == HomeTab::Games && all.is_empty() && !community.is_empty() { HomeTab::Community } else { root.home_tab };
    // (Two rows of them then, as nothing else is under the welcome: a big window had its lower half empty.)
    let no_presets = root.state.pointer("/settings/presets").and_then(Value::as_array).is_none_or(|l| l.is_empty());
    // The library too, two rows while nothing plays (the figures then take a row): the owner's maximised window had its
    // lower third empty under three presets (2026-11-12).
    let rows = if (all.is_empty() && no_presets) || playing_now(root) == 0 { 2 } else { 1 };
    // Fewer games than the row holds: they share the whole row (three to a row at least, so one game isn't a
    // banner), rather than leaving its right part empty.
    let n_items = match tab_now {
        HomeTab::Games => all.len(),
        HomeTab::Community => community.len(),
    };
    let tile_w = if n_items > 0 && n_items < cols {
        let k = n_items.max(3).min(cols);
        ((width - (k - 1) as f32 * GRID_GAP) / k as f32).floor()
    } else {
        tile_w
    };
    let tiles: Vec<AnyElement> = match tab_now {
        HomeTab::Games => all
            .iter()
            // (Full rows only, as the community's.)
            .take({
                let k = all.len().min(cols * rows);
                if k > cols { k - k % cols } else { k }
            })
            .enumerate()
            .map(|(i, g)| {
                let place = g.place;
                let sub = if g.accounts.is_empty() {
                    (root.i18n.tv("ui.playedH", &[("n", &format!("{:.0}", g.hours.max(1.)))]), false)
                } else {
                    (root.tr("home.playing"), true)
                };
                let icon_url = look(root, g.place, "icon").map(str::to_string).or_else(|| g.icon.clone());
                let cover = look(root, g.place, "cover").map(str::to_string);
                cascade(
                    "home-games",
                    i,
                    cover_tile(("game", i), cover, icon_url, g.name.clone(), sub, g.accounts.len(), i == focus, tile_w, cx).on_click(cx.listener(move |r, _, _, cx| {
                        r.picked = Some(place);
                        cx.notify();
                    })),
                )
                .into_any_element()
            })
            .collect(),
        HomeTab::Community => community
            .iter()
            // (Full rows only: a lone game on a second row looked dropped.)
            .take({
                let k = community.len().min(cols * rows);
                if k > cols { k - k % cols } else { k }
            })
            .enumerate()
            .map(|(i, g)| {
                let place = g.get("place").and_then(Value::as_i64).unwrap_or(0);
                let pcs = g.get("pcs").and_then(Value::as_i64).unwrap_or(0);
                let name = clean(g.get("name").and_then(Value::as_str).unwrap_or_default());
                let open = name.clone();
                cascade(
                    "home-community",
                    i,
                    cover_tile(
                        ("cm", i),
                        look(root, place, "cover").map(str::to_string),
                        look(root, place, "icon").map(str::to_string),
                        name,
                        (root.i18n.tn("home.pcsN", pcs, &[]), false),
                        0,
                        false,
                        tile_w,
                        cx,
                    )
                    .on_click(cx.listener(move |r, _, _, cx| {
                        crate::pages::games::open_place(r, place, &open);
                        cx.notify();
                    })),
                )
                .into_any_element()
            })
            .collect(),
    };
    // The section's head as every other on the page (its title at 16 px, its controls at the right): which games is
    // a two-way switch beside See all, not two chips standing in for the title.
    let pick = |t: HomeTab, icon_name: &'static str, label: String, n: usize, cx: &mut Context<Root>| {
        let on = tab_now == t;
        segment(SharedString::from(format!("home-tab-{icon_name}")), on)
            .h(px(TOOL_H - 6.))
            .px(px(10.))
            .gap(px(6.))
            .rounded(px(R_S - 2.))
            .child(icon(icon_name, 14., if on { c(INK) } else { c(MUTED) }))
            .child(label)
            .when(n > 0, |d| d.child(div().text_size(px(T_S)).font_features(tnum()).text_color(if on { ca(INK, 0.6) } else { c(MUTED) }).child(n.to_string())))
            .on_click(cx.listener(move |r, _, _, cx| {
                r.home_tab = t;
                cx.notify();
            }))
    };
    div()
        .flex()
        .flex_col()
        .gap(px(12.))
        .child(
            section_head(root.tr("home.games"), None)
                .child(view_switch(vec![pick(HomeTab::Games, "gamepad", root.tr("ui.myGames"), all.len(), cx), pick(HomeTab::Community, "globe", root.tr("ui.community"), community.len(), cx)]))
                .child(see_all("home-games-all", root.tr("home.all"), cx.listener(|r, _, _, cx| {
                    r.go(Page::Games);
                    cx.notify();
                }))),
        )
        .child(if tiles.is_empty() {
            div().h(px(64.)).flex().items_center().text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("home.noGameSub"))
        } else {
            grid(GRID_GAP).children(tiles)
        })
}

/// "See all ›": a quiet tool at the right of a row's head.
pub fn see_all(id: &'static str, words: String, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> impl IntoElement {
    tool(id, None, false).child(words).child(icon("chevronRight", 13., c(MUTED))).on_click(f)
}

/// A game as a wide picture on its own colour (its cover, else its icon on that colour, else its letter), with its
/// icon, name and one fact under it. `windows`: its Roblox windows open now, said on the picture. The one `on` (the
/// stage's) is ringed in yellow. Flat: no raised base under it.
#[allow(clippy::too_many_arguments)]
pub fn cover_tile(id: (&'static str, usize), cover: Option<String>, icon_url: Option<String>, name: String, (sub, live): (String, bool), windows: usize, on: bool, w: f32, cx: &mut App) -> Hot {
    let art_h = (w * 9. / 16.).round();
    let picture = cover.as_deref().and_then(|u| images::pic_pair(u, w, art_h, cx));
    // The game's own colour, from its icon (else its cover): the ground of its picture.
    let tone = icon_url.as_deref().and_then(|u| images::tone(u, cx)).or_else(|| cover.as_deref().and_then(|u| images::tone(u, cx)));
    let badge = icon_url.as_deref().and_then(|u| images::pic_sq(u, 36., cx));
    let first: String = name.chars().find(|c| c.is_alphanumeric()).map(|c| c.to_uppercase().collect()).unwrap_or_default();
    let icon_side = (art_h * 0.5).round();
    let big_icon = if picture.is_none() { icon_url.as_deref().and_then(|u| images::pic_sq(u, icon_side, cx)) } else { None };
    let art = div()
        .relative()
        .w(px(w))
        .h(px(art_h))
        .rounded(px(R_L))
        .overflow_hidden()
        .bg(match tone {
            Some(t) => mix(c(t), c(PANEL), 0.55),
            None => c(RAISED),
        })
        .flex()
        .items_center()
        .justify_center()
        .map(|d| match (picture, big_icon) {
            (Some((p, k)), _) => d.child(fade_in(img(p).size_full().object_fit(ObjectFit::Cover), &k)),
            (None, Some(i)) => d.child(img(i).size(px(icon_side)).rounded(px(R_L))),
            (None, None) => d.child(div().text_size(px(T_STAGE + 8.)).font_weight(FontWeight::BOLD).text_color(c(DIM)).child(first.clone())),
        })
        // The chosen one's yellow ring, drawn over the picture (a see-through border would show the game's colour as
        // an edge round the others).
        .when(on, |d| d.child(div().absolute().inset_0().rounded(px(R_L)).border_2().border_color(c(FACE))))
        .when(windows > 0, |d| {
            d.child(
                div()
                    .absolute()
                    .left(px(8.))
                    .bottom(px(8.))
                    .h(px(24.))
                    .px(px(8.))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .rounded(px(7.))
                    .bg(ca(GROUND, 0.84))
                    .text_size(px(T_S))
                    .font_weight(FontWeight::SEMIBOLD)
                    .font_features(tnum())
                    .child(div().size(px(6.)).rounded_full().bg(c(OK)))
                    .child(icon("screens", 12., c(TEXT)))
                    .child(windows.to_string()),
            )
        });
    hot(SharedString::from(format!("{}-{}", id.0, id.1)))
        .w(px(w))
        .flex_none()
        .flex()
        .flex_col()
        .gap(px(12.))
        .cursor_pointer()
        .lift(3.)
        .child(art)
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .child(
                    div()
                        .size(px(36.))
                        .flex_none()
                        .rounded(px(R_M))
                        .overflow_hidden()
                        .bg(c(RAISED_2))
                        .flex()
                        .items_center()
                        .justify_center()
                        .map(|d| match badge {
                            Some(i) => d.child(img(i).size_full().object_fit(ObjectFit::Cover)),
                            None => d.text_size(px(T_H)).font_weight(FontWeight::BOLD).text_color(c(MUTED)).child(first),
                        }),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(div().truncate().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).line_height(px(19.)).child(name))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .text_size(px(T_S))
                                .text_color(if live { c(TEXT_2) } else { c(MUTED) })
                                .font_features(tnum())
                                .when(live, |d| d.child(div().size(px(6.)).flex_none().rounded_full().bg(c(OK))))
                                .child(div().truncate().child(sub)),
                        ),
                ),
        )
}

/// The presets, each with its game's icon, which game and how many accounts, and Run: as many as two rows of the
/// page's grid hold.
fn presets(root: &Root, width: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let presets: Vec<Value> = root.state.pointer("/settings/presets").and_then(Value::as_array).cloned().unwrap_or_default();
    let (cols, card_w) = grid_fit(width, 300., GRID_GAP);
    // Fewer presets than the row holds: they share it (two to a row at least), with room for their schedule line.
    let card_w = if !presets.is_empty() && presets.len() < cols {
        let k = presets.len().max(2).min(cols);
        ((width - (k - 1) as f32 * GRID_GAP) / k as f32).floor()
    } else {
        card_w
    };
    let alive: Vec<String> = root.accounts().iter().filter_map(|a| a.get("id").and_then(Value::as_str).map(str::to_string)).collect();
    let mut cards = grid(GRID_GAP).pb(px(4.));
    for (i, p) in presets.iter().take(cols * 2).enumerate() {
        let s = |k: &str| p.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
        let n = p
            .get("accountIds")
            .and_then(Value::as_array)
            .map(|ids| ids.iter().filter(|id| id.as_str().is_some_and(|x| alive.iter().any(|a| a == x))).count())
            .unwrap_or(0) as i64;
        let game = if s("gameName").is_empty() { root.tr("ps.noGame") } else { clean(&s("gameName")) };
        let pic = p.get("iconUrl").and_then(Value::as_str).and_then(|u| images::pic_sq(u, 44., cx));
        let id = s("id");
        // When it is set to run, and next (the Presets tab's line): the one thing worth knowing about a preset here.
        let when = p.get("schedule").filter(|sc| sc.get("enabled").and_then(Value::as_bool).unwrap_or(false) && !sc.get("time").and_then(Value::as_str).unwrap_or_default().is_empty()).map(|sc| {
            let at = crate::pages::auto::schedule_text(root, sc);
            let next = sc.get("date").and_then(Value::as_str).unwrap_or_default().is_empty().then(|| crate::pages::auto::next_text(root, p)).flatten();
            match next {
                Some(n) => format!("{at} · {n}"),
                None => at,
            }
        });
        cards = cards.child(crate::kit::block_card(("preset", i))
                .w(px(card_w))
                .flex_none()
                .min_w_0()
                .p(px(12.))
                .flex()
                .items_center()
                .gap(px(12.))
                .child(
                    div()
                        .size(px(44.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(R_M))
                        .overflow_hidden()
                        .bg(ca(WHITE, 0.06))
                        .map(|d| match pic {
                            Some(i) => d.child(img(i).size_full().object_fit(ObjectFit::Cover)),
                            None => d.child(icon("layers", 18., c(MUTED))),
                        }),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(3.))
                        .child(div().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).truncate().child(s("name")))
                        .child(facts(vec![(game, None), (root.i18n.tn("ps.accN", n, &[]), None)]).text_size(px(T_S)))
                        // (Every card has this line, "No schedule" when none: the cards are one height, review 1.1.)
                        .child({
                            let set = when.is_some();
                            let w = when.unwrap_or_else(|| root.tr("au.noSchedule"));
                            div()
                                .min_w_0()
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .text_size(px(T_S))
                                .text_color(c(if set { TEXT_2 } else { MUTED }))
                                .font_features(tnum())
                                .child(icon("clock", 12., c(MUTED)))
                                .child(div().min_w_0().truncate().child(w))
                        }),
                )
                .child(
                    IconButton::new(("run", i), "play").size(34.).round().filled().disabled(n == 0).tip(root.tr("ui.run")).on_click(cx.listener(
                        move |r, _, _, cx| r.launch("runPreset", json!({ "id": id }), false, cx),
                    )),
                ));
    }
    div().flex().flex_col().gap(px(12.)).when(!presets.is_empty(), |d| {
        d.child(section_head(root.tr("nav.presets"), Some(presets.len())).child(see_all("presets-all", root.tr("home.all"), cx.listener(|r, _, _, cx| {
            r.go(Page::Auto);
            cx.notify();
        }))))
        .child(cards)
    })
}
