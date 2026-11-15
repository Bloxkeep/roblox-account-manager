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
    /// When it was last played (ms since 1970; 0 not known).
    pub last: f64,
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

/// Dropped from its game, as the Accounts page counts it (Accounts.tsx's isKicked): disconnected, or banned by the game.
pub fn is_dropped(a: &Value) -> bool {
    crate::pages::accounts::is_kicked(a)
}

/// Running games first (most accounts first), then those played lately that aren't running.
pub fn games(root: &Root) -> Vec<Game> {
    let mut now: Vec<Game> = Vec::new();
    for a in root.accounts() {
        let st = a.get("state").and_then(Value::as_str).unwrap_or_default();
        let Some(place) = a.pointer("/session/game/placeId").and_then(Value::as_i64) else { continue };
        // (A window back on Roblox's menu still names its last game: it isn't playing it.)
        if (st != "running" && st != "launching") || crate::pages::accounts::in_menu(a) {
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
                last: 0.,
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
            Some((last, Game { place, name, icon: None, accounts: Vec::new(), hours: p.get("hours").and_then(Value::as_f64).unwrap_or(0.), last }))
        })
        .collect();
    recent.sort_by(|a, b| b.0.total_cmp(&a.0));
    // No play history (yet): the games put in the Play box lately instead (Home.tsx's recentGames).
    if recent.is_empty() {
        for g in root.state.pointer("/settings/recentGames").and_then(Value::as_array).into_iter().flatten().take(8) {
            let place = g.get("placeId").and_then(Value::as_i64).unwrap_or(0);
            let name = clean(g.get("name").and_then(Value::as_str).unwrap_or_default());
            if place > 0 && !name.is_empty() && !now.iter().any(|x| x.place == place || bare(&x.name) == bare(&name)) {
                let icon = g.get("iconUrl").and_then(Value::as_str).map(str::to_string);
                recent.push((0., Game { place, name, icon, accounts: Vec::new(), hours: 0., last: 0. }));
            }
        }
    }
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
    // No accounts column beside the page (a narrow window): a button at the top right slides it in over the page
    // (the owner's mock-up, 2026-11-15: "nút Accounts 8 ở góc phải để mở danh sách tài khoản").
    let accounts_button = root.no_side.get().then(|| {
        div().pt(px(14.)).px(px(SIDE)).flex().justify_end().child(
            Button::new("home-accounts", format!("{}  {}", root.tr("nav.accounts"), root.accounts().len())).icon("users").on_click(cx.listener(|r, _, _, cx| {
                // (A click outside the open column shut it just now: this click on its own button only shuts it.)
                let just = r.side_shut.take().is_some_and(|at| at.elapsed().as_millis() < 300);
                r.side_open = !just && !r.side_open;
                cx.notify();
            })),
        )
    });
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
                .gap(px(if small(root) { 14. } else { 24. }))
                .pb(px(SECTION_GAP))
                .children(accounts_button)
                .child(stage(root, &all, focus, width, cx))
                // The rows under the stage, on the page's rhythm: the figures rise over the picture's faded foot,
                // the games and the presets a section apart.
                .child(
                    div()
                        .px(px(SIDE))
                        .flex()
                        .flex_col()
                        .gap(px(SECTION_GAP))
                        // (No strip of the accounts here: the column beside the page lists them already, the owner
                        // 2026-11-13: "thừa, bên cạnh có rồi".)
                        .child(shelf(root, &all, focus, inner, cx))
                        .when(root.accounts().is_empty(), |d| d.child(get_started(root, cx)))
                        .child(presets(root, inner, cx))
                        .when(!root.accounts().is_empty(), |d| d.children(crate::pages::stats::recent(root, cx))),
                ),
        )
}

/// Home drawn small: on a window too narrow for the accounts column, or a short one. Its stage is then a low band
/// (the game's picture and the faces left out, what to do at its right) and its games stand in short tiles, so two
/// rows of them show whole under it (the owner's mock-up of a small window, 2026-11-15: before, the stage took half
/// the window and the second row of games was cut at the window's foot).
fn small(root: &Root) -> bool {
    root.no_side.get() || root.window_h.get() < 760.
}

/// How tall the game's cover stands at the stage's right: the stage is as tall as it and no taller (it kept the
/// height of the edge-to-edge picture that went, and the words sat at its foot under a dark empty band, the owner
/// 2026-11-13).
fn cover_h(width: f32) -> f32 {
    (width * 0.17).clamp(200., if width > 2000. { 360. } else { 300. }).round()
}

/// The stage's room above its contents, from the panel's top edge.
const TOP: f32 = 28.;

/// How many accounts are in a Roblox window now.
pub fn playing_now(root: &Root) -> usize {
    root.accounts().iter().filter(|a| a.get("state").and_then(Value::as_str) == Some("running")).count()
}

/// The game on the stage, on its own picture: what it is doing, and what to do with it.
fn stage(root: &Root, all: &[Game], focus: usize, width: f32, cx: &mut Context<Root>) -> impl IntoElement {
    // A row as tall as its contents: the words beside the cover, centred on it.
    let compact = small(root);
    // (Under the Accounts button, on a window without the column, the stage needs no room of its own above it.)
    let base = div().pt(px(if root.no_side.get() { 0. } else if compact { 16. } else { TOP })).px(px(SIDE)).flex().items_center().gap(px(2. * SIDE)).flex_none();
    let none = root.accounts().is_empty();
    // No accounts: the welcome and Add account, whatever was played before (Home.tsx's focus).
    let Some(g) = all.get(focus).filter(|_| !none) else {
        // Nothing played yet: the welcome stands on the picture of the game Bloxkeep's users play most (the stage was
        // a bare dark block with a line of words in its corner, owner 2026-11-12), words at its foot as a game's are.
        // (No picture behind it any more: the blurred game washes went, the owner, 2026-11-13.)
        return base.child(
            div()
                .flex_1()
                .min_w_0()
                .py(px(24.))
                .flex()
                .flex_col()
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
    // The stage is ONE block on the game's own colour (taken from its picture, solid: the base its tiles stand on
    // elsewhere), the words on its left half and the cover filling its right half edge to edge, sharp. It was words on
    // the bare panel with a small cover floating at the right (the owner, 2026-11-13: "trông đơn điệu quá").
    let block_h = cover_h(width).max(if running { 268. } else { 236. });
    let pic_w = (block_h * 16. / 9.).round();
    // Dropped when the words would have too little room.
    let picture = cover.as_deref().filter(|_| width - 2. * SIDE - pic_w >= 460.).and_then(|u| images::pic_pair(u, pic_w, block_h, cx));
    let icon_url = look(root, g.place, "icon").map(str::to_string).or_else(|| g.icon.clone());
    let tone = icon_url.as_deref().and_then(|u| images::tone(u, cx)).or_else(|| cover.as_deref().and_then(|u| images::tone(u, cx)));
    let ground = tone.map(|t| tinted(RAISED, t, 0.2)).unwrap_or_else(|| c(RAISED));
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
    } else {
        if g.hours > 0. {
            facts = facts.child(sep()).child(root.i18n.tv("ui.playedH", &[("n", &format!("{:.0}", g.hours.max(1.)))]));
        }
        // When it was last played (Home.tsx's stage: "last 3 days ago").
        if g.last > 0. {
            let ago = crate::time::ago(crate::time::now_secs() - (g.last / 1000.) as i64, &root.i18n);
            facts = facts.child(sep()).child(root.i18n.tv("home.last", &[("t", &ago)]));
        }
    }

    let mut acts = div().when(!compact, |d| d.mt(px(6.))).flex().flex_wrap().gap(px(8.));
    if running {
        acts = acts.child(Button::new("stage-windows", root.i18n.tn("home.windowsN", n, &[])).icon("screens").primary().big().on_click(
            cx.listener(|r, _, _, cx| {
                // Their windows: Accounts' windows view.
                crate::pages::accounts::set_win_view(r, true);
                r.go(Page::Accounts);
                cx.notify();
            }),
        ));
    }
    if !dropped.is_empty() {
        let label = root.i18n.tv("ui.rejoinN", &[("n", &dropped.len().to_string())]);
        acts = acts.child(Button::new("stage-rejoin", label).icon("refresh").tint().big().on_click(cx.listener(move |r, _, _, cx| r.rejoin(&dropped, cx))));
    }
    // The game into the Play box on Accounts, to pick who plays it (Home.tsx's put-game): "Add accounts" while it runs.
    let place = g.place;
    acts = acts.child(
        Button::new("stage-put", root.tr(if running { "home.addMore" } else { "home.pickAccounts" }))
            .icon(if running { "userPlus" } else { "play" })
            .map(|b| if running { b.glass() } else { b.primary() })
            .big()
            .on_click(cx.listener(move |r, _, window, cx| crate::pages::accounts::put_game(r, place, window, cx))),
    );
    // Its page in Games: Roblox's figures, your accounts in it, the quiet hours, its news.
    let (place, name) = (g.place, g.name.clone());
    acts = acts.child(Button::new("stage-game", root.tr("home.gamePage")).icon("info").glass().big().on_click(cx.listener(move |r, _, _, cx| {
        crate::pages::games::open_place(r, place, &name);
        cx.notify();
    })));

    // Who is in it, faces overlapping (six at most, then "+N").
    let who = running.then(|| {
        let more = g.accounts.len().saturating_sub(6);
        div()
            .flex()
            .items_center()
            .children(g.accounts.iter().take(6).enumerate().map(|(i, a)| div().when(i > 0, |d| d.ml(px(-8.))).child(crate::pages::accounts::avatar(a, 30., RAISED, cx))))
            .when(more > 0, |d| d.child(div().ml(px(8.)).text_size(px(T_S)).text_color(on_picture(0.86)).font_features(tnum()).child(format!("+{more}"))))
    });
    // Its cover, the block's right half: the gameplay clip plays on it while the mouse rests there; a click opens its page.
    let clip = crate::pages::clips::tile_clip(root, g.place, cx);
    let (place, name) = (g.place, g.name.clone());
    let art = picture.map(|(src, orig)| {
        div()
            .id("stage-art")
            .on_hover(clip.hover)
            .relative()
            .w(px(pic_w))
            .h(px(block_h))
            .flex_none()
            .cursor_pointer()
            .child(fade_in(img(src).size_full().object_fit(ObjectFit::Cover).rounded_tr(px(R_L)).rounded_br(px(R_L)), &orig))
            .children(clip.layer)
            .on_click(cx.listener(move |r, _, _, cx| {
                crate::pages::games::open_place(r, place, &name);
                cx.notify();
            }))
    });
    if compact {
        // One low band: the game's icon, its name and what it is doing at the left, what to do at the right (wrapping
        // under them on a very narrow window).
        return base.child(
            div()
                .flex_1()
                .min_w_0()
                .min_h(px(104.))
                .px(px(24.))
                .py(px(18.))
                .flex()
                .flex_wrap()
                .items_center()
                .gap_x(px(20.))
                .gap_y(px(12.))
                .rounded(px(R_L))
                // (Plain charcoal: the game's colour is on its icon and name, not the band; the owner's rule of
                // 2026-11-15.)
                .bg(crate::kit::block_ground())
                .child(
                    div()
                        .flex_1()
                        .min_w(px(260.))
                        .flex()
                        .items_center()
                        .gap(px(16.))
                        .when_some(badge, |d, b| {
                            d.child(div().size(px(60.)).flex_none().rounded(px(R_L)).overflow_hidden().border_1().border_color(ca(WHITE, 0.14)).child(img(b).size_full().object_fit(ObjectFit::Cover).rounded(px(R_L))))
                        })
                        .child(
                            div()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap(px(4.))
                                .child(div().text_size(px(30.)).line_height(px(36.)).font_weight(FontWeight::BOLD).truncate().child(game_name(g.name.clone(), icon_url.as_deref(), cx)))
                                .child(facts),
                        ),
                )
                .child(div().flex_none().max_w_full().child(acts)),
        );
    }
    base.child(
        div()
            .flex_1()
            .min_w_0()
            .h(px(block_h))
            .flex()
            .rounded(px(R_L))
            .overflow_hidden()
            .bg(ground)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .px(px(28.))
                    .flex()
                    .flex_col()
                    .justify_center()
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
                                        .child(img(b).size_full().object_fit(ObjectFit::Cover).rounded(px(R_L))),
                                )
                            })
                            .child(match crate::pages::games::logo_img(root, g.place, &g.name, 96., 380., cx) {
                                // (The game's own logo in place of its name in words, when its wiki has one.)
                                Some(logo) => div().flex().child(logo),
                                None => div().min_w_0().max_w(px(560.)).text_size(px(40.)).font_weight(FontWeight::BOLD).line_height(px(46.)).truncate().child(game_name(g.name.clone(), icon_url.as_deref(), cx)),
                            }),
                    )
                    .child(facts)
                    .children(who)
                    .child(acts),
            )
            .children(art),
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
            .tones(c(RAISED), c(RAISED_2))
            .edge(c(LINE), ca(WHITE, 0.17))
            .child(
                div()
                    .size(px(28.))
                    .flex_none()
                    .rounded_full()
                    .border_1()
                    .border_color(if now { ca(TEXT, 0.6) } else { c(LINE) })
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(T_M))
                    .font_weight(FontWeight::SEMIBOLD)
                    .font_features(tnum())
                    .text_color(if now { c(TEXT) } else { c(MUTED) })
                    .child(format!("{}", i + 1)),
            )
            .child(div().flex_none().child(icon(mark, 18., c(MUTED))))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
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

/// The stage's game's cover, else the game Bloxkeep's users play most (Profile's banner falls back to it).
pub fn wash(root: &Root, cx: &mut App) -> Option<(String, std::sync::Arc<Image>)> {
    let url = hero_place(root)
        .and_then(|p| look(root, p, "cover"))
        .or_else(|| root.state.pointer("/community/data/games/0/place").and_then(Value::as_i64).and_then(|p| look(root, p, "cover")))?
        .to_string();
    images::pic(&url, cx).map(|p| (url, p))
}

/// The page's side margin, the Accounts page's: the stage's words and every row under it start here.
const SIDE: f32 = 24.;

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
    let short = small(root);
    let rows = if short || (all.is_empty() && no_presets) || playing_now(root) == 0 { 2 } else { 1 };
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
    // Full rows only (a lone game on a second row looked dropped); the tabs count what shows, See all has the rest.
    let full_rows = |n: usize| {
        let k = n.min(cols * rows);
        if k > cols { k - k % cols } else { k }
    };
    let (games_shown, community_shown) = (full_rows(all.len()), full_rows(community.len()));
    let tiles: Vec<AnyElement> = match tab_now {
        HomeTab::Games => all
            .iter()
            .take(games_shown)
            .enumerate()
            .map(|(i, g)| {
                let place = g.place;
                let sub = if g.accounts.is_empty() && g.hours <= 0. {
                    (root.tr("home.recent"), false)
                } else if g.accounts.is_empty() {
                    (root.i18n.tv("ui.playedH", &[("n", &format!("{:.0}", g.hours.max(1.)))]), false)
                } else {
                    (root.tr("home.playing"), true)
                };
                let icon_url = look(root, g.place, "icon").map(str::to_string).or_else(|| g.icon.clone());
                let cover = look(root, g.place, "cover").map(str::to_string);
                cascade(
                    "home-games",
                    i,
                    cover_tile(("game", i), cover, icon_url, g.name.clone(), sub, g.accounts.len(), i == focus, tile_w, short, crate::pages::clips::tile_clip(root, place, cx), cx).on_click(cx.listener(move |r, _, _, cx| {
                        r.picked = Some(place);
                        cx.notify();
                    })),
                )
                .into_any_element()
            })
            .collect(),
        HomeTab::Community => community
            .iter()
            .take(community_shown)
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
                        short,
                        crate::pages::clips::tile_clip(root, place, cx),
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
            .child(icon(icon_name, 14., if on { crate::theme::on_text() } else { c(MUTED) }))
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
                .child(view_switch(vec![pick(HomeTab::Games, "gamepad", root.tr("ui.myGames"), games_shown, cx), pick(HomeTab::Community, "globe", root.tr("ui.community"), community_shown, cx)]))
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
pub fn cover_tile(id: (&'static str, usize), cover: Option<String>, icon_url: Option<String>, name: String, (sub, live): (String, bool), windows: usize, on: bool, w: f32, short: bool, clip: crate::pages::clips::TileClip, cx: &mut App) -> Hot {
    // (`short`: a low picture, cut to the middle of the game's own, for a small Home's two rows.)
    let art_h = (w * if short { 0.43 } else { 9. / 16. }).round();
    let picture = cover.as_deref().and_then(|u| images::pic_pair(u, w, art_h, cx));
    // The game's own colour, from its icon (else its cover): the ground of its picture.
    let tone = icon_url.as_deref().and_then(|u| images::tone(u, cx)).or_else(|| cover.as_deref().and_then(|u| images::tone(u, cx)));
    let badge = icon_url.as_deref().and_then(|u| images::pic_sq(u, 36., cx));
    let first: String = name.chars().find(|c| c.is_alphanumeric()).map(|c| c.to_uppercase().collect()).unwrap_or_default();
    let icon_side = (art_h * 0.5).round();
    let big_icon = if picture.is_none() { icon_url.as_deref().and_then(|u| images::pic_sq(u, icon_side, cx)) } else { None };
    let crate::pages::clips::TileClip { layer, hover } = clip;
    let art = div()
        .id(SharedString::from(format!("{}-art-{}", id.0, id.1)))
        .on_hover(hover)
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
            (Some((p, k)), _) => d.child(fade_in(img(p).size_full().object_fit(ObjectFit::Cover).rounded(px(R_L)), &k)),
            (None, Some(i)) => d.child(img(i).size(px(icon_side)).rounded(px(R_L))),
            (None, None) => d.child(div().text_size(px(T_STAGE + 8.)).font_weight(FontWeight::BOLD).text_color(c(DIM)).child(first.clone())),
        })
        // Its gameplay clip while the mouse rests on it (pages/clips.rs).
        .children(layer)
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
                            Some(i) => d.child(img(i).size_full().object_fit(ObjectFit::Cover).rounded(px(R_M))),
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
                        .child(div().truncate().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).line_height(px(19.)).child(game_name(name, icon_url.as_deref(), cx)))
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
    let alive: std::collections::HashSet<&str> = root.accounts().iter().filter_map(|a| a.get("id").and_then(Value::as_str)).collect();
    let mut cards = grid(GRID_GAP).pb(px(4.));
    for (i, p) in presets.iter().take(cols * 2).enumerate() {
        let s = |k: &str| p.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
        let n = p
            .get("accountIds")
            .and_then(Value::as_array)
            .map(|ids| ids.iter().filter(|id| id.as_str().is_some_and(|x| alive.contains(x))).count())
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
        // (Of its game's colour, as an account's card is of the game it plays.)
        let (ground, ground_hot, edge, edge_hot) = toned(if rich() { p.get("iconUrl").and_then(Value::as_str).and_then(|u| images::tone(u, cx)) } else { None });
        cards = cards.child(crate::kit::block_card(("preset", i))
                .tones(ground, ground_hot)
                .edge(edge, edge_hot)
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
                            Some(i) => d.child(img(i).size_full().object_fit(ObjectFit::Cover).rounded(px(R_M))),
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
