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
    game_library(root, false)
}

pub(crate) fn game_library(root: &Root, full: bool) -> Vec<Game> {
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
        for g in root.state.pointer("/settings/recentGames").and_then(Value::as_array).into_iter().flatten().take(if full { usize::MAX } else { 8 }) {
            let place = g.get("placeId").and_then(Value::as_i64).unwrap_or(0);
            let name = clean(g.get("name").and_then(Value::as_str).unwrap_or_default());
            if place > 0 && !name.is_empty() && !now.iter().any(|x| x.place == place || bare(&x.name) == bare(&name)) {
                let icon = g.get("iconUrl").and_then(Value::as_str).map(str::to_string);
                recent.push((0., Game { place, name, icon, accounts: Vec::new(), hours: 0., last: 0. }));
            }
        }
    }
    now.extend(recent.into_iter().map(|(_, g)| g).take(if full { usize::MAX } else { 12 }));
    now
}

pub(crate) fn look<'a>(root: &'a Root, place: i64, part: &str) -> Option<&'a str> {
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
    let mut all = games(root);
    if cfg!(feature = "shot") && std::env::var("BLOXKEEP_SHOT_NOGAME").is_ok() {
        all.clear();
    }
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
        // No accounts yet: the welcome (its own page, see `welcome`).
        // (Before it, once: what Bloxkeep is for, `setup`.)
        .when(root.accounts().is_empty(), |d| d.child(if setup_waits() && root.state.get("accounts").is_some() { match setup_stage() { "pc" => setup_pc(root, width, cx), "theme" => setup_theme(root, width, cx), _ => setup(root, width, cx) } } else { welcome(root, width, cx).into_any_element() }))
        .when(!root.accounts().is_empty(), |d| d.child(
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
                        .child(shelf(root, &all, focus, inner, None, cx))
                        .child(presets(root, inner, cx))
                        .when(!root.accounts().is_empty(), |d| d.children(crate::pages::stats::recent(root, cx))),
                ),
        ))
}

// ---- The welcome, before any account. Built to a launcher home the owner picked on Dribbble (2026-11-17, "Giữ C"
// of three tried): Paulo Rasera's "Game Launcher" for Turtle (dribbble.com/shots/11189960). The picture of the game
// Bloxkeep's users play most across the page's top, sharp, darkening to the left and down; a line of capitals, the
// game's logo, the welcome and Add account at its left; three blocks over its faded foot, each under its own line of
// capitals with a quiet button at its foot. Without the community's games (offline, before the first beat): no
// picture, the welcome is the title, and the blocks that would be empty are left out.

/// What Bloxkeep's users play now (the bot's community sums): place, name, PCs on it.
fn community_games(root: &Root) -> Vec<(i64, String, i64)> {
    root.state
        .pointer("/community/data/games")
        .and_then(Value::as_array)
        .map(|l| {
            l.iter()
                .filter_map(|g| {
                    let place = g.get("place").and_then(Value::as_i64)?;
                    let name = clean(g.get("name").and_then(Value::as_str)?);
                    (!name.is_empty()).then(|| (place, name, g.get("pcs").and_then(Value::as_i64).unwrap_or(0)))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn community_sum(root: &Root, key: &str) -> Option<i64> {
    root.state.pointer(&format!("/community/data/{key}")).and_then(Value::as_i64)
}

/// A little line of capitals over a block.
fn caps(text: String, color: Hsla) -> Div {
    div().text_size(px(T_XS)).font_weight(FontWeight::SEMIBOLD).text_color(color).child(text.to_uppercase())
}

/// A figure over its label.
fn figure(label: String, value: String) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(3.))
        .child(div().text_size(px(T_GROUP)).font_weight(FontWeight::SEMIBOLD).font_features(tnum()).text_color(c(TEXT)).child(value))
        .child(div().text_size(px(T_S)).text_color(on_picture(0.62)).child(label))
}

thread_local! {
    /// Each game's banner as uploaded, by place: its file on this PC, or None while it is fetched or when none came.
    static BANNERS: std::cell::RefCell<std::collections::HashMap<i64, Option<String>>> = Default::default();
}

/// The banner of the game at `place` as it was uploaded, when it is on this PC; asked for the first time it is wanted.
fn banner(place: i64, cx: &mut Context<Root>) -> Option<String> {
    if let Some(known) = BANNERS.with(|b| b.borrow().get(&place).cloned()) {
        return known;
    }
    BANNERS.with(|b| b.borrow_mut().insert(place, None));
    // (A hidden shot's mock places are made up: nothing to ask Roblox.)
    if place <= 0 {
        return None;
    }
    let dir = images::kept_dir("banners");
    let rx = crate::roblox::run(move || crate::roblox::place_banner(place, &dir));
    cx.spawn(async move |this, cx| {
        if let Some(file) = rx.await.ok().flatten() {
            BANNERS.with(|b| b.borrow_mut().insert(place, Some(file.to_string_lossy().replace('\\', "/"))));
            let _ = this.update(cx, |_, cx| cx.notify());
        }
    })
    .detach();
    None
}

/// The hero's picture, sharp, filling its box; None while it loads or when there is no game to show.
fn hero_art(root: &Root, place: Option<i64>, w: f32, h: f32, (top, foot): (f32, f32), cx: &mut Context<Root>) -> Option<AnyElement> {
    let place = place?;
    // The banner as uploaded once it is here (roblox::place_banner), Roblox's smaller cover until then.
    let url = match banner(place, cx) {
        Some(file) => file,
        None => look(root, place, "cover")?.to_string(),
    };
    let (src, orig) = images::pic_pair(&url, w, h, cx)?;
    Some(div().absolute().inset_0().child(fade_in(img(src).size_full().object_fit(ObjectFit::Cover).rounded_t(px(top)).rounded_b(px(foot)), &orig)).into_any_element())
}

/// The hero's back, a billboard (Netflix's, Steam's library hero, the Xbox app's game page): the picture at the
/// panel's right, top to foot at its own 16:9, so it is neither cut nor blown up; the words stand on the panel's plain
/// ground at its left, the picture fading into that ground leftward and into the panel at its foot. It filled the
/// whole width, near twice its size: a thumbnail's own words stood blurred behind the game's name (the owner's Steal
/// An Egg, 2026-11-18: "cần cải thiện hero cái này nhiều nữa"). `over` lies on the picture (a gameplay clip).
/// The room the hero's words keep at its left when its banner is a picture of its own (`hero_card`): the game's
/// name and the row of buttons under it.
const HERO_WORDS: f32 = 440.;

/// The hero's banner is a picture of its own: the panel is thin glass or clear (a custom theme at a low fade, glass
/// on the blocks only), no ground for a picture to melt into; or the hero is too narrow for a banner at its full
/// height to leave the words their room (it then stands smaller, beside them: the name and the buttons lay on the
/// picture itself in a 1280 px window, the shots of 2026-11-18).
fn hero_card(width: f32, h: f32) -> bool {
    c(PANEL).a < 0.6 || width - (h * 16. / 9.).min(width) < HERO_WORDS + 48.
}

/// The banner as a picture of its own in a hero `width`×`h`: how wide and tall. Its own 16:9, as tall as the hero
/// where there is room; narrower (and less tall) where the words need theirs; nothing where what is left is no
/// picture worth showing. It lay over the name and the buttons in a narrow window (the owner, 2026-11-18).
fn hero_card_size(width: f32, h: f32) -> Option<(f32, f32)> {
    let w = (h * 16. / 9.).min(width - HERO_WORDS).round();
    (w >= 320.).then(|| (w, (w * 9. / 16.).round().min(h)))
}

/// How far from the hero's right edge its words end: before the banner, when it is a picture of its own.
fn hero_words_end(width: f32, h: f32) -> f32 {
    if !hero_card(width, h) {
        // A melting banner: the words end where the banner begins, its melting edge too. The melt is no ground for
        // words: let run on over it (as far as where the banner stands clear, my rule of alpha.26, and to the hero's
        // end before that), a long name lay on the picture ("Throw Food For Animals" across the banner with the
        // panels' glass on, the owner's screenshot, 2026-11-18: "tắt khối kính phát vỡ UI luôn").
        return (h * 16. / 9.).min(width).round();
    }
    match hero_card_size(width, h) {
        Some((w, _)) => w + 24.,
        None => 48.,
    }
}

/// The box of a banner that is a picture of its own, `w`×`tall`: in the hero's top right corner (the panel's own,
/// round as that is), its other corners round. Always from the top: stood in the middle of the hero's height where
/// it was the shorter, its top edge was out of line with the Accounts column beside it (the owner, 2026-11-18: "cái
/// banner ko thẳng hàng với account").
fn hero_card_box(w: f32, tall: f32) -> Div {
    div().absolute().top_0().right_0().w(px(w)).h(px(tall)).rounded(px(R_L)).rounded_tr(px(R_PANEL)).overflow_hidden()
}

/// The height of the hero's banner where it is a picture of its own and shorter than the hero `h`: the hero is then
/// as tall as it (`hero_tall`) and the words stand in the middle of its height (`hero_words`).
fn hero_beside(width: f32, h: f32) -> Option<f32> {
    hero_card(width, h).then(|| hero_card_size(width, h)).flatten().map(|(_, tall)| tall).filter(|tall| *tall < h)
}

/// A hero's box: at least as tall as `h`, or as its shorter banner (`beside`), and as tall as its words where they
/// need more: its banner is laid over it (absolute), its words stand in it (`hero_words`), so the words set its
/// height and none can run out under it into the row below.
fn hero_box(h: f32, beside: Option<f32>) -> Div {
    div().relative().flex_none().w_full().min_h(px(beside.unwrap_or(h)))
}

/// The hero's words, a column at its left, `room` wide and nothing of it drawn past that: in the middle of the
/// hero's height `h`, or of its shorter banner's (`beside`).
fn hero_words(room: f32, h: f32, beside: Option<f32>) -> Div {
    let d = div().relative().ml(px(48.)).w(px(room)).overflow_hidden().flex().flex_col().justify_center();
    match beside {
        Some(tall) => d.min_h(px(tall)).py(px(12.)),
        None => d.min_h(px(h)).pt(px(12.)).pb(px(24.)),
    }
}

/// Bloxkeep's own hero: its banner at the right, a picture of its own (crisp::hero: no game's picture stands in for
/// Bloxkeep); at the left its mark and a line of capitals, a title, a line on it, what to do, and `under`.
fn own_hero(root: &Root, width: f32, kicker: String, title: String, sub: String, act: AnyElement, under: Option<AnyElement>) -> Div {
    let h = hero_h(root, width);
    let size = hero_card_size(width, h);
    let back = div().absolute().inset_0().children(size.map(|(w, tall)| hero_card_box(w, tall).children(crate::crisp::hero(w, tall).map(|p| img(p).size_full()))));
    let beside = size.map(|(_, tall)| tall).filter(|tall| *tall < h);
    let room = (width - 48. - size.map_or(48., |(w, _)| w + 24.)).max(160.);
    // (A narrow room: the title a size smaller, as a game's name is there.)
    let (big, line) = if room < 460. { (36., 42.) } else { (48., 54.) };
    hero_box(h, beside).child(back).child(
        hero_words(room, h, beside)
            .gap(px(14.))
            .child(div().flex().items_center().gap(px(10.)).child(crate::crisp::mark(26.)).child(caps(kicker, on_picture(0.78))))
            .child(div().max_w(px(620.)).text_size(px(big)).line_height(px(line)).font_weight(FontWeight::BOLD).text_color(c(TEXT)).child(title))
            .child(div().max_w(px(520.)).text_size(px(T_L)).line_height(px(21.)).text_color(on_picture(0.82)).child(sub))
            .child(div().mt(px(4.)).flex().gap(px(8.)).child(act))
            .children(under),
    )
}

fn hero_back(root: &Root, place: Option<i64>, width: f32, h: f32, over: Option<AnyElement>, cx: &mut Context<Root>) -> Div {
    // A panel of thin glass, or a clear one, is no ground to melt into: the banner stood there cut square on three
    // corners, round only at the panel's own (the owner, 2026-11-18: "thiếu bo góc à"). It is a picture of its own
    // there, its four corners round, and nothing shaded.
    if hero_card(width, h) {
        let Some((w, tall)) = hero_card_size(width, h) else { return div().absolute().inset_0() };
        let art = hero_art(root, place, w, tall, (0., 0.), cx);
        // (At the hero's full height it fills the panel's corner, round as that is; less tall, it stands in the middle.)
        return div().absolute().inset_0().child(hero_card_box(w, tall).children(art).children(over));
    }
    // (Its own 16:9, whole: nothing of the banner is cut.)
    let art_w = (h * 16. / 9.).min(width).round();
    let art = hero_art(root, place, art_w, h, (0., 0.), cx);
    div()
        .absolute()
        .inset_0()
        .rounded_t(px(R_PANEL))
        .overflow_hidden()
        // (Short shades: the banner's own left edge and foot melt into the panel, the rest of it stays whole.)
        .child(div().absolute().top_0().bottom_0().right_0().w(px(art_w)).children(art).children(over).children(shade(90., PANEL, 0., 0.34, 1., (0., 0.))).children(shade(90., PANEL, 0., 0.12, 1., (0., 0.))))
        .children(shade(0., PANEL, 0., 0.24, 1., (0., 0.)))
}

/// A shade over a picture toward one side (laid twice: GPUI's see-through gradients come out about half as strong).
fn shade(angle: f32, color: u32, from: f32, to: f32, strength: f32, (top, foot): (f32, f32)) -> Vec<Div> {
    (0..2).map(|_| div().absolute().inset_0().rounded_t(px(top)).rounded_b(px(foot)).bg(linear_gradient(angle, linear_color_stop(ca(color, strength), from), linear_color_stop(ca(color, 0.), to)))).collect()
}

/// One of the blocks at the foot of a stage (Turtle's "WEEKLY ACTIVITY" panels): a line of capitals over a card, its
/// insides in the middle, a quiet button across its foot.
/// The row a stage's blocks stand in: abreast, the ones that find no room (`stage_block`'s least width) on a line
/// under.
fn stage_blocks() -> Div {
    div().flex().flex_wrap().gap(px(GRID_GAP + 8.))
}

pub(crate) fn stage_block(head: String, body: AnyElement, foot: Option<AnyElement>, capped: bool) -> Div {
    div()
        .flex_1()
        // (At least this wide: in a row short of room (`stage_blocks`) one goes to the next line. Three abreast in the
        // smallest window were 195 px each: figures ran into each other and the top games' faces lay over the block
        // beside theirs, the shots of 2026-11-18.)
        .min_w(px(250.))
        .when(capped, |d| d.max_w(px(620.)))
        .flex()
        .flex_col()
        .gap(px(10.))
        .child(caps(head, on_picture(0.7)))
        .child(
            div()
                .h(px(170.))
                .flex()
                .flex_col()
                .rounded(px(R_L))
                .bg(crate::kit::block_ground())
                .border_1()
                .border_color(ca(WHITE, 0.06))
                .child(div().flex_1().min_h_0().px(px(18.)).py(px(16.)).flex().items_center().child(body))
                // (No button: the same room kept, so the insides of a row of blocks line up.)
                .child(match foot {
                    Some(f) => div().px(px(10.)).pb(px(10.)).flex().flex_col().child(f),
                    None => div().h(px(40.)),
                }),
        )
}

/// A block's insides: its mark, a title and a line under it.
pub(crate) fn block_text(mark: &'static str, title: Option<String>, sub: String, tag: Option<String>) -> AnyElement {
    div()
        .w_full()
        .flex()
        .items_start()
        .gap(px(14.))
        .child(div().size(px(40.)).flex_none().rounded(px(R_M)).bg(c(RAISED_2)).flex().items_center().justify_center().child(icon(mark, 20., c(TEXT_2))))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(4.))
                .when_some(title, |d, title| {
                    d.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(div().min_w_0().truncate().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(title))
                            .children(tag.map(|t| div().flex_none().text_size(px(T_S)).font_weight(FontWeight::SEMIBOLD).text_color(c(OK)).child(t))),
                    )
                })
                .child(div().text_size(px(T_M)).line_height(px(19.)).text_color(c(TEXT_2)).child(sub)),
        )
        .into_any_element()
}

/// A page with nothing in it yet, in the welcome's look (Turtle's launcher): one of the games Bloxkeep's users play
/// (`pic`: which, so each page shows another) across a block at its top, darkening left and down, a line of
/// capitals, the title, a line on it and its main button at its left; the blocks in a row under it; then `after`.
/// Without the community's games the block is plain.
pub(crate) fn empty_stage(root: &Root, pic: usize, kicker: Option<String>, title: String, sub: String, main: Option<AnyElement>, blocks: Vec<Div>, after: Option<AnyElement>, width: f32, cx: &mut Context<Root>) -> Div {
    let list = community_games(root);
    let place = (!list.is_empty()).then(|| list[pic % list.len()].0);
    let hero_h = ((root.window_h.get() - 300.) * 0.5).clamp(220., 380.).round();
    // The picture at the block's right at its own 16:9, as the game's hero has it, its left melting into the block's
    // ground; the words keep the ground at the left. Across the whole block it was cut to a strip and blown up, and a
    // thumbnail's own big words lay behind the title ("Chưa có tài khoản nào" over "OFFLINE", the shots of the pages
    // with nothing in them, 2026-11-18): the shade that was to carry the title ended before a long one did.
    let ground = crate::kit::block_ground();
    // (A block of thin glass is no ground to melt into: the picture is a card in it, its corners round, a margin
    // round it, and narrower where the block is.)
    let thin = ground.a < 0.6;
    let inset = if thin { 10. } else { 0. };
    let art_h = hero_h - 2. * inset;
    // (At most 46 % of the block: the words keep the rest.)
    let art_w = (art_h * 16. / 9.).min(width * 0.46).round();
    let art = hero_art(root, place, art_w, art_h, (0., 0.), cx);
    let pictured = art.is_some();
    let melt = |to: f32| div().absolute().inset_0().bg(linear_gradient(90., linear_color_stop(ground, 0.), linear_color_stop(ground.opacity(0.), to)));
    // (The words' room: to the card, or to where a melting picture begins: its melt is no ground for words, a
    // title lay on a thumbnail's own letters there.)
    let room = match (pictured, thin) {
        (false, _) => width - 72.,
        (true, true) => width - 36. - inset - art_w - 24.,
        (true, false) => width - 36. - art_w,
    }
    .max(240.);
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(SECTION_GAP))
        .child(
            div()
                .relative()
                .h(px(hero_h))
                .rounded(px(R_L))
                .overflow_hidden()
                .bg(ground)
                // (Laid twice: GPUI's see-through gradients come out about half as strong.)
                .when(pictured, |d| {
                    let pic = div().absolute().top(px(inset)).bottom(px(inset)).right(px(inset)).w(px(art_w)).children(art);
                    d.child(if thin { pic.rounded(px(R_L - 4.)).overflow_hidden() } else { pic.child(melt(0.5)).child(melt(0.5)) })
                })
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left(px(36.))
                        .w(px(room))
                        .flex()
                        .flex_col()
                        .justify_center()
                        .gap(px(12.))
                        .children(kicker.map(|k| caps(k, on_picture(0.7))))
                        .child(div().max_w(px(560.)).text_size(px(36.)).line_height(px(42.)).font_weight(FontWeight::BOLD).text_color(c(TEXT)).child(title))
                        .child(div().max_w(px(500.)).text_size(px(T_M)).line_height(px(19.)).text_color(on_picture(0.8)).child(sub))
                        .children(main.map(|m| div().mt(px(6.)).flex().child(m))),
                ),
        )
        .when(!blocks.is_empty(), |d| d.child(stage_blocks().children(blocks)))
        .children(after)
}

// ───────────────────────── The first question ─────────────────────────

thread_local! {
    /// The settings the first question will turn on, one bit each; an answer ticks its group, each switch at the
    /// right one of them. Several accounts at once is ticked to start with.
    static SETS: std::cell::Cell<u8> = const { std::cell::Cell::new(MULTI) };
}
const S_MULTI: u8 = 1;
const S_TILE: u8 = 2;
const S_REJOIN: u8 = 4;
const S_AFK: u8 = 8;
const S_TRAY: u8 = 16;
/// The answers: the settings each stands for.
const MULTI: u8 = S_MULTI | S_TILE;
const AFK: u8 = S_REJOIN | S_AFK | S_TRAY;

/// The first question still waits: never answered (nor skipped) on this PC. Home shows it before the welcome while
/// there is no account yet.
pub fn setup_waits() -> bool {
    setup_stage() != "done"
}

/// How long after the first question accounts added get auto-rejoin (the sign-in window adds several in a row, each
/// a state of its own; the next day's are the person's own choice).
const REJOIN_FOR: i64 = 2 * 3600;

/// After the first question with "staying in game" ticked: auto-rejoin is each account's own switch, and there was no
/// account to put it on then. Each account added in the next two hours gets it, once: its id is kept as done, so one
/// the person turns off again is left off, also after a restart. Kept done only when the app said yes; a refusal is
/// said (and so logged) and tried with the next state.
pub fn setup_follow(r: &mut Root, cx: &mut Context<Root>) {
    let Some(until) = crate::prefs::get("rejoinNew").and_then(|v| v.as_i64()) else { return };
    if crate::time::now_secs() > until {
        crate::prefs::set("rejoinNew", Value::Null);
        crate::prefs::set("rejoinDone", Value::Null);
        return;
    }
    let done: Vec<String> = crate::prefs::get("rejoinDone").and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
    let fresh: Vec<(String, bool)> = r
        .accounts()
        .iter()
        .filter_map(|a| a.get("id").and_then(Value::as_str).map(|id| (id.to_string(), crate::val::flag(a, "autoRejoin"))))
        .filter(|(id, _)| !done.contains(id))
        .collect();
    if fresh.is_empty() {
        return;
    }
    let all: Vec<String> = fresh.iter().map(|(id, _)| id.clone()).collect();
    let off: Vec<String> = fresh.iter().filter(|(_, on)| !on).map(|(id, _)| id.clone()).collect();
    let keep = move |all: Vec<String>| {
        let mut done: Vec<String> = crate::prefs::get("rejoinDone").and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
        done.extend(all.into_iter().filter(|id| !done.contains(id)).collect::<Vec<_>>());
        crate::prefs::set("rejoinDone", json!(done));
    };
    if off.is_empty() {
        keep(all);
        return;
    }
    #[cfg(feature = "core")]
    crate::core::log::note_file(&format!("First question: auto-rejoin on for {} new account(s)", off.len()));
    r.ask_once("setup-rejoin", "setAutoRejoin", json!({ "ids": off, "enabled": true }), cx, move |_, _| keep(all));
}

/// What Bloxkeep is used for, asked once before the first account (the owner's pick of 2026-11-19, from Asana's "main
/// objective" cards, Attio's preview beside its questions and Airtable's steps bar): each answer ticked turns on the
/// settings it needs, listed at the right as they will be; Skip leaves everything as it is. Then this PC (after
/// "several accounts"), the look, and the welcome (Add account).
fn setup(root: &Root, width: f32, cx: &mut Context<Root>) -> AnyElement {
    let picks = SETS.with(|p| p.get());
    let option = |bit: u8, id: &'static str, mark: &'static str, title: &str, sub: &str| {
        // (Ticked while any of its settings is: one switched off at the right leaves the rest.)
        let on = picks & bit != 0;
        div()
            .id(id)
            .flex()
            .items_start()
            .gap(px(12.))
            .px(px(14.))
            .py(px(12.))
            .rounded(px(R_L))
            .border_1()
            .border_color(if on { c(FACE) } else { c(LINE) })
            .bg(c(RAISED))
            .cursor_pointer()
            .hover(|s| if on { s } else { s.border_color(c(MUTED)) })
            .child(div().size(px(32.)).flex_none().rounded(px(R_M)).bg(c(INSET)).flex().items_center().justify_center().child(icon(mark, 17., if on { c(FACE) } else { c(TEXT_2) })))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(div().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(title.to_string()))
                    .child(div().text_size(px(T_S)).line_height(px(17.)).text_color(c(MUTED)).child(sub.to_string())),
            )
            .child(div().mt(px(8.)).child(if on { tick_mark(16.) } else { div().size(px(16.)).flex_none().rounded(px(5.)).border_1().border_color(c(LINE)) }))
            .on_click(cx.listener(move |_, _, _, cx| {
                SETS.with(|p| p.set(if p.get() & bit != 0 { p.get() & !bit } else { p.get() | bit }));
                cx.notify();
            }))
    };
    let go = Button::new("setup-go", root.tr("setup.go")).icon_end("chevronRight").primary().on_click(cx.listener(|r, _, _, cx| {
        // (No app to tell: nothing is kept as answered, the question stays.)
        if r.link.is_none() {
            r.say(r.tr("common.noBridge"), "bad");
            cx.notify();
            return;
        }
        let sets = SETS.with(|p| p.get());
        let mut used = Vec::new();
        let mut patch = serde_json::Map::new();
        for (bit, key, name) in [(S_MULTI, "multiInstance", "multi"), (S_TILE, "multiTile", "tile"), (S_AFK, "antiAfk", "antiAfk")] {
            if sets & bit != 0 {
                patch.insert(key.into(), json!(true));
                used.push(name);
            }
        }
        if !patch.is_empty() {
            r.ask("setSettings", Value::Object(patch), cx, |_, _| {});
        }
        if sets & S_TRAY != 0 {
            used.push("tray");
            r.ask("setCloseToTray", json!({ "enabled": true }), cx, |_, _| {});
        }
        if sets & S_REJOIN != 0 {
            used.push("rejoin");
            crate::prefs::set("rejoinNew", json!(crate::time::now_secs() + REJOIN_FOR));
            crate::prefs::set("rejoinDone", json!([]));
        }
        #[cfg(feature = "core")]
        crate::core::log::note_file(&format!("First question answered: {}", if used.is_empty() { "nothing".to_string() } else { used.join(", ") }));
        crate::prefs::set("setupUse", json!(used));
        go_stage(if sets & S_MULTI != 0 { "pc" } else { "theme" });
        cx.notify();
    }));
    let left = div()
        .child(step_head(root, 1, if picks & S_MULTI != 0 { 3 } else { 2 }, root.tr("setup.title"), root.tr("setup.sub")))
        .child(
            div()
                .mt(px(20.))
                .flex()
                .flex_col()
                .gap(px(10.))
                .child(option(MULTI, "setup-multi", "users", &root.tr("setup.multi"), &root.tr("setup.multiSub")))
                .child(option(AFK, "setup-afk", "moon", &root.tr("setup.afk"), &root.tr("setup.afkSub"))),
        )
        .child(div().flex_1().min_h(px(24.)))
        .child(step_foot(root, None, go, cx));
    // The right: the settings as they will be, the ones a tick turns on lit.
    // (What is on already, here or by default, shows on and can't be switched: the question only turns things on.)
    let multi_st = root.state.get("multi").unwrap_or(&Value::Null);
    let already = |bit: u8| match bit {
        S_MULTI => crate::val::flag(multi_st, "enabled"),
        S_TILE => crate::val::flag(multi_st, "tile"),
        S_AFK => root.state.get("settings").is_some_and(|st| crate::val::flag(st, "antiAfk")),
        S_TRAY => root.state.get("closeToTray").and_then(Value::as_bool).unwrap_or(true),
        _ => false,
    };
    let weak = cx.entity().downgrade();
    let setting = |key: &'static str, bit: u8| {
        let was = already(bit);
        let on = was || picks & bit != 0;
        let weak = weak.clone();
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(12.))
            .px(px(12.))
            .py(px(9.))
            .rounded(px(R_M))
            .when(on, |d| d.bg(c(RAISED)).border_1().border_color(ca(FACE, 0.35)))
            .when(!on, |d| d.border_1().border_color(ca(FACE, 0.)).when(crate::theme::backdrop().is_some(), |d| d.bg(ca(WHITE, 0.04))))
            .child(div().flex_1().min_w_0().text_size(px(T_M)).line_height(px(18.)).text_color(if on { c(TEXT) } else { c(MUTED) }).child(root.tr(key)))
            .when(was, |d| d.child(div().flex_none().text_size(px(T_S)).text_color(c(DIM)).child(root.tr("setup.already"))))
            .child(Switch::new(SharedString::from(format!("setup-sw-{key}")), on).disabled(was).on_toggle(move |v, _, cx| {
                let v = *v;
                SETS.with(|p| p.set(if v { p.get() | bit } else { p.get() & !bit }));
                let _ = weak.update(cx, |_, cx| cx.notify());
            }))
    };
    let multi = picks & S_MULTI != 0;
    let right = div()
        .child(div().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(root.tr("setup.willTurnOn")))
        .child(div().text_size(px(T_S)).text_color(c(DIM)).child(root.tr("setup.later")))
        .child(div().mt(px(14.)).mb(px(4.)).child(caps(root.tr("setup.secRoblox"), c(DIM))))
        .child(setting("set.multi", S_MULTI))
        .child(setting("set.openAs.tile", S_TILE))
        // (The risk is said where this turns it on, not where it was on already.)
        .when(multi && !already(S_MULTI), |d| d.child(div().mt(px(4.)).px(px(12.)).flex().gap(px(6.)).child(div().pt(px(1.)).child(icon("info", 13., c(WARN)))).child(div().flex_1().min_w_0().text_size(px(T_S)).line_height(px(16.)).text_color(c(MUTED)).child(root.tr("set.multiConfirm")))))
        .child(div().mt(px(14.)).mb(px(4.)).child(caps(root.tr("setup.secAfk"), c(DIM))))
        .child(setting("set.rejoin", S_REJOIN))
        .child(setting("set.antiAfk", S_AFK))
        .child(setting("set.tray", S_TRAY));
    step_card(root, width, left, Some(right))
}

/// Where the first run is: "use" (the question), "pc" (how many at once, after "several accounts"), "theme", "done".
/// (An answer kept from before the steps came, setupUse alone, is done.)
fn setup_stage() -> &'static str {
    // BLOXKEEP_SHOT_SETUP=use|pc|theme: a shot shows that step of the first run (with ?empty).
    #[cfg(feature = "shot")]
    if let Ok(s) = std::env::var("BLOXKEEP_SHOT_SETUP") {
        return match s.as_str() {
            "pc" => "pc",
            "theme" => "theme",
            _ => "use",
        };
    }
    match crate::prefs::get("setupStep").as_ref().and_then(Value::as_str) {
        Some("pc") => "pc",
        Some("theme") => "theme",
        Some("done") => "done",
        Some(_) => "use",
        None if crate::prefs::get("setupUse").is_some() => "done",
        None => "use",
    }
}

fn go_stage(to: &'static str) {
    crate::prefs::set("setupStep", json!(to));
}

/// The PC step is there when "several accounts" was ticked (as answered, or ticked now on the question).
fn has_pc() -> bool {
    match crate::prefs::get("setupUse").and_then(|v| v.as_array().cloned()) {
        Some(used) if setup_stage() != "use" => used.iter().any(|u| u.as_str() == Some("multi")),
        _ => SETS.with(|p| p.get()) & S_MULTI != 0,
    }
}

/// The whole first run left: from any step, nothing more asked.
fn finish(why: &str) {
    #[cfg(feature = "core")]
    crate::core::log::note_file(&format!("First run: {why}"));
    #[cfg(not(feature = "core"))]
    let _ = why;
    if crate::prefs::get("setupUse").is_none() {
        crate::prefs::set("setupUse", json!([]));
    }
    go_stage("done");
}

/// A step's head: the bar of steps, "Step n of m", its question and a line under it.
fn step_head(root: &Root, n: usize, of: usize, title: String, sub: String) -> Div {
    let bar = 120.;
    div()
        .flex()
        .flex_col()
        .child(div().w(px(bar)).h(px(3.)).rounded_full().bg(c(LINE)).child(div().w(px((bar * n as f32 / of as f32).round())).h_full().rounded_full().bg(c(FACE))))
        .child(div().mt(px(20.)).text_size(px(T_S)).text_color(c(DIM)).child(root.i18n.tv("setup.step", &[("n", &n.to_string()), ("of", &of.to_string())])))
        .child(div().mt(px(6.)).text_size(px(22.)).line_height(px(28.)).font_weight(FontWeight::BOLD).text_color(c(TEXT)).child(title))
        .child(div().mt(px(6.)).text_size(px(T_M)).line_height(px(19.)).text_color(c(MUTED)).child(sub))
}

/// A step's foot: Back (from the second on), the main button, Skip (all the rest of the first run).
fn step_foot(root: &Root, back: Option<&'static str>, go: Button, cx: &mut Context<Root>) -> Div {
    let link = |id: &'static str, words: String| div().id(id).text_size(px(T_M)).text_color(c(MUTED)).cursor_pointer().hover(|s| s.text_color(c(TEXT))).child(words);
    div()
        .flex()
        .items_center()
        .gap(px(18.))
        .children(back.map(|to| {
            link("setup-back", root.tr("setup.back")).on_click(cx.listener(move |_, _, _, cx| {
                go_stage(to);
                cx.notify();
            }))
        }))
        .child(go)
        .child(link("setup-skip", root.tr("setup.unsure")).on_click(cx.listener(|_, _, _, cx| {
            finish("skipped");
            cx.notify();
        })))
}

/// The first run's card: the step at the left, what it does at the right (side by side when there is room).
fn step_card(root: &Root, width: f32, left: Div, right: Option<Div>) -> AnyElement {
    let page_h = (root.window_h.get() - 110.).max(520.);
    let side = width >= 760.;
    div()
        .min_h(px(page_h))
        .flex()
        .items_center()
        .justify_center()
        .p(px(24.))
        .child(
            div()
                .w_full()
                .max_w(px(900.))
                .min_h(px(460.))
                .rounded(px(R_L + 4.))
                .border_1()
                .border_color(c(LINE))
                .bg(crate::kit::block_ground())
                .overflow_hidden()
                .flex()
                .when(!side, |d| d.flex_col())
                .child(left.flex_1().min_w_0().p(px(28.)).flex().flex_col())
                .children(right.map(|r| {
                    r.flex_1()
                        .min_w_0()
                        .p(px(24.))
                        // (On a picture theme the card is glass, as Home's blocks are: this half darker over it, so
                        // what is in it reads on any part of the picture.)
                        .bg(if crate::theme::backdrop().is_some() { ca(SHADE, 0.3) } else { c(INSET) })
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                })),
        )
        .into_any_element()
}

thread_local! {
    /// How many accounts at once the PC step plans for (0: not set yet, then the PC's number or 3).
    static TARGET: std::cell::Cell<i64> = const { std::cell::Cell::new(0) };
}

/// Step 2 (after "several accounts"): how many at once, this PC as Easy setup measures it, and what Bloxkeep will
/// change for that number (Easy setup's own plan, its suggested mode).
fn setup_pc(root: &Root, width: f32, cx: &mut Context<Root>) -> AnyElement {
    let fit_now = crate::pages::easy_setup::facts(root, 1).fit.map(|f| f.0);
    let most = crate::pages::easy_setup::most(root);
    // (BLOXKEEP_SHOT_TARGET=n: a shot asks for n at once, as if the arrow was pressed that far.)
    #[cfg(feature = "shot")]
    if let Some(n) = std::env::var("BLOXKEEP_SHOT_TARGET").ok().and_then(|n| n.parse::<i64>().ok()) {
        TARGET.with(|t| if t.get() == 0 { t.set(n) });
    }
    let target = TARGET.with(|t| if t.get() > 0 { t.get() } else { fit_now.map_or(3, |n| n.clamp(2, 6)) }).clamp(1, most);
    let f = crate::pages::easy_setup::facts(root, target);
    let total = if has_pc() { 3 } else { 2 };
    let step = |id: &'static str, ic: &'static str, by: i64, off: bool| {
        IconButton::new(id, ic).size(36.).disabled(off).on_click(cx.listener(move |_, _, _, cx| {
            TARGET.with(|t| t.set((target + by).clamp(1, most)));
            cx.notify();
        }))
    };
    let count = div()
        .mt(px(24.))
        .flex()
        .items_center()
        .gap(px(14.))
        .child(step("setup-less", "chevronLeft", -1, target <= 1))
        .child(div().min_w(px(56.)).text_center().text_size(px(40.)).line_height(px(44.)).font_weight(FontWeight::BOLD).font_features(tnum()).text_color(c(TEXT)).child(target.to_string()))
        .child(step("setup-more", "chevronRight", 1, target >= most))
        .child(div().text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("setup.accounts")));
    let over = f.over.map(|n| div().mt(px(12.)).flex().gap(px(6.)).child(div().pt(px(1.)).child(icon("alert", 13., c(WARN)))).child(div().flex_1().min_w_0().text_size(px(T_S)).line_height(px(17.)).text_color(c(MUTED)).child(root.i18n.tv("es.over", &[("n", &n.to_string())]))));
    let patch = f.patch.clone();
    let go = Button::new("setup-go", root.tr("setup.go")).icon_end("chevronRight").primary().on_click(cx.listener(move |r, _, _, cx| {
        if patch.as_object().is_some_and(|o| !o.is_empty()) {
            r.ask("setSettings", patch.clone(), cx, |_, _| {});
        }
        #[cfg(feature = "core")]
        crate::core::log::note_file(&format!("First run: {target} account(s) at once, {}", if patch.as_object().is_some_and(|o| !o.is_empty()) { patch.to_string() } else { "nothing to change".into() }));
        go_stage("theme");
        cx.notify();
    }));
    let left = div()
        .child(step_head(root, 2, total, root.tr("es.want"), root.tr("setup.pcSub")))
        .child(count)
        .children(over)
        .child(div().flex_1().min_h(px(24.)))
        .child(step_foot(root, Some("use"), go, cx));
    // The right: this PC, then what will change.
    let fact = |ic: &'static str, head: &str, main: String, sub: Option<String>| {
        div()
            .flex()
            .items_center()
            .gap(px(12.))
            .px(px(12.))
            .py(px(9.))
            .rounded(px(R_M))
            .bg(if crate::theme::backdrop().is_some() { ca(WHITE, 0.05) } else { c(RAISED) })
            .child(div().size(px(30.)).flex_none().rounded(px(R_M)).bg(c(INSET)).flex().items_center().justify_center().child(icon(ic, 15., c(TEXT_2))))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(div().text_size(px(T_XS)).font_weight(FontWeight::SEMIBOLD).text_color(c(DIM)).child(head.to_string()))
                    .child(div().truncate().text_size(px(T_M)).text_color(c(TEXT)).child(main)),
            )
            .children(sub.map(|s| div().flex_none().text_size(px(T_S)).text_color(c(MUTED)).child(s)))
    };
    let unknown = root.tr("es.unknown");
    let fit = match &f.fit {
        Some((n, each, by)) => div()
            .mt(px(6.))
            .flex()
            .items_center()
            .gap(px(12.))
            .px(px(12.))
            .py(px(10.))
            .rounded(px(R_M))
            .border_1()
            .border_color(c(LINE))
            .child(div().flex_none().text_size(px(26.)).font_weight(FontWeight::BOLD).font_features(tnum()).text_color(c(TEXT)).child(n.to_string()))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(div().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(root.tr("es.runs")))
                    .child(div().text_size(px(T_S)).line_height(px(17.)).text_color(c(MUTED)).child(root.i18n.tv("es.why", &[("each", each), ("by", by)]))),
            ),
        None => div().mt(px(6.)).px(px(12.)).flex().gap(px(6.)).child(div().pt(px(1.)).child(icon("info", 13., c(MUTED)))).child(div().flex_1().min_w_0().text_size(px(T_S)).line_height(px(17.)).text_color(c(MUTED)).child(root.tr("setup.measureLater"))),
    };
    let mut changes = div().flex().flex_col().gap(px(6.)).px(px(4.));
    if f.rows.is_empty() {
        changes = changes.child(div().text_size(px(T_S)).text_color(c(MUTED)).child(root.tr("es.do.none")));
    }
    for row in &f.rows {
        changes = changes.child(div().flex().items_start().gap(px(8.)).text_size(px(T_S)).line_height(px(17.)).text_color(c(TEXT_2)).child(div().mt(px(2.)).child(icon("check", 13., c(OK)))).child(div().flex_1().min_w_0().child(row.clone())));
    }
    let right = div()
        .child(div().mb(px(4.)).child(caps(root.tr("es.pc"), c(DIM))))
        .child(fact("chip", "CPU", f.cpu.clone().unwrap_or_else(|| unknown.clone()), (f.threads > 0).then(|| root.i18n.tv("es.threads", &[("n", &f.threads.to_string())]))))
        .child(fact("gauge", "RAM", f.ram.clone().unwrap_or_else(|| unknown.clone()), f.free.as_ref().map(|v| root.i18n.tv("es.free", &[("v", v)]))))
        .child(fact("monitor", "GPU", f.gpu.clone().unwrap_or(unknown), None))
        .child(fit)
        .child(div().mt(px(16.)).mb(px(4.)).child(caps(root.tr("es.plan"), c(DIM))))
        .child(changes);
    step_card(root, width, left, Some(right))
}

/// The look: a few themes as Settings › Appearance draws them (a click shows it at once), the ones most Bloxkeep PCs
/// use among them.
fn setup_theme(root: &Root, width: f32, cx: &mut Context<Root>) -> AnyElement {
    let total = if has_pc() { 3 } else { 2 };
    let cur = crate::pages::settings::appearance::current(root);
    let mut ids: Vec<&'static str> = vec!["space", "sunset", "aurora", "dark", "light", "system"];
    // (Then the ones most PCs are in now, as Appearance's Popular has them, up to eight in all.)
    for t in root.state.pointer("/community/data/themes").and_then(Value::as_array).into_iter().flatten() {
        let Some(id) = t.get("id").and_then(Value::as_str) else { continue };
        // (Keep at dusk is left out here, Deep space in its place: the owner 2026-11-19, "bỏ cái thành lúc hoàng hôn ra
        // ngoài chọn giao diện", "cho deep space vào thay thế".)
        if let Some(known) = crate::theme_table::GROUPS.iter().flat_map(|(_, l)| l.iter()).find(|k| **k == id)
            && *known != crate::theme::KEEP.id
            && !ids.contains(known)
            && ids.len() < 8
        {
            ids.push(known);
        }
    }
    let mut grid = div().mt(px(22.)).flex().flex_wrap().gap(px(14.));
    for (i, id) in ids.into_iter().enumerate() {
        grid = grid.child(crate::pages::settings::appearance::tile(root, id, i, id == cur, None, cx));
    }
    let go = Button::new("setup-go", root.tr("setup.finish")).icon_end("chevronRight").primary().on_click(cx.listener(|_, _, _, cx| {
        finish("done");
        cx.notify();
    }));
    let left = div()
        .child(step_head(root, total, total, root.tr("setup.themeTitle"), root.tr("setup.themeSub")))
        .child(grid)
        .child(div().flex_1().min_h(px(24.)))
        .child(step_foot(root, Some(if has_pc() { "pc" } else { "use" }), go, cx));
    step_card(root, width, left, None)
}

/// The welcome: Home before any account.
fn welcome(root: &Root, width: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let list = community_games(root);
    let facts: Vec<(String, String)> = [("pcs", "home.w.pcs"), ("alts", "home.w.alts")]
        .iter()
        .filter_map(|(key, words)| community_sum(root, key).map(|n| (root.tr(words), crate::kit::thousands(n))))
        .collect();
    // The panel's height: the hero at its top, the blocks at its foot.
    let page_h = (root.window_h.get() - 110.).max(520.);
    // (One block alone, without the community's: not stretched across the page.)
    let alone = facts.is_empty() && list.is_empty();
    let block = |head: String, body: AnyElement, foot: AnyElement| stage_block(head, body, Some(foot), alone);
    let wide = |b: Button| div().flex().flex_col().child(b);
    // 1: the steps, numbered.
    let steps = div().w_full().flex().flex_col().gap(px(10.)).children(["guide.add", "guide.game", "guide.play"].iter().enumerate().map(|(i, k)| {
        div()
            .flex()
            .items_center()
            .gap(px(10.))
            .child(
                div()
                    .size(px(22.))
                    .flex_none()
                    .rounded_full()
                    .border_1()
                    .border_color(if i == 0 { c(FACE) } else { c(LINE) })
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(T_S))
                    .font_features(tnum())
                    .text_color(if i == 0 { c(FACE) } else { c(MUTED) })
                    .child((i + 1).to_string()),
            )
            .child(div().min_w_0().truncate().text_size(px(T_M)).text_color(if i == 0 { c(TEXT) } else { c(TEXT_2) }).child(root.tr(k)))
    }));
    // 2: Bloxkeep now, two figures.
    // (Each in its half, its label on two lines where one is too long: side by side at their own widths the two
    // labels ran into each other in a narrow block.)
    let now = div().w_full().flex().gap(px(16.)).children(facts.iter().take(2).map(|(l, v)| figure(l.clone(), v.clone()).flex_1().min_w_0().items_center().text_center()));
    // 3: the top games, faces in a row with their names under (72 px each: three fit the narrowest block).
    let top = div().w_full().flex().justify_center().gap(px(12.)).children(list.iter().take(3).enumerate().map(|(i, x)| {
        let place = x.0;
        let name = x.1.clone();
        let pic = look(root, place, "icon").map(str::to_string).and_then(|u| images::pic_pair(&u, 64., 64., cx));
        hot(("home-w-top", i))
            .lift(2.)
            .w(px(72.))
            .flex()
            .flex_col()
            .items_center()
            .gap(px(8.))
            .cursor_pointer()
            .rounded(px(R_M))
            .child(div().size(px(64.)).rounded(px(R_M)).overflow_hidden().bg(c(RAISED_2)).when_some(pic, |d, (s, o)| d.child(fade_in(img(s).size_full().object_fit(ObjectFit::Cover).rounded(px(R_M)), &o))))
            .child(caps(x.1.clone(), c(TEXT_2)).max_w(px(72.)).truncate())
            .on_click(cx.listener(move |r, _, _, cx| {
                crate::pages::games::open_place(r, place, &name);
                cx.notify();
            }))
    }));
    let all_games = Button::new("home-w-c-games", root.tr("home.pickGame")).small().on_click(cx.listener(|r, _, _, cx| {
        r.go(Page::Games);
        cx.notify();
    }));
    // (Grey here: the yellow is the stage's Add account, the page's one main button.)
    let add_more = Button::new("home-w-c-add", root.tr("acc.add")).icon("plus").small().on_click(cx.listener(|r, _, _, cx| r.add_account(cx)));
    let stats = Button::new("home-w-c-stats", root.tr("home.w.seeStats")).small().on_click(cx.listener(|r, _, _, cx| {
        r.go(Page::Stats);
        cx.notify();
    }));
    // Bloxkeep's own hero at the top: its banner, the welcome, what to do first. A game's picture lay across the
    // page here with the words on it, under two shades of the panel's colour: with glass on the blocks only the
    // panel has none, and the words, the button and the blocks' heads stood on the bare picture (the shots of
    // every kind of theme, 2026-11-18; Sunset with that glass is the look a new Bloxkeep starts in). And it was
    // someone else's game standing in for Bloxkeep (the owner, the same day).
    let add = Button::new("home-w-add", root.tr("acc.add")).icon("plus").primary().big().on_click(cx.listener(|r, _, _, cx| r.add_account(cx)));
    div()
        .relative()
        .min_h(px(page_h))
        .flex()
        .flex_col()
        .child(own_hero(root, width, root.tr("brand.sub"), root.tr("home.welcome"), root.tr("home.welcomeSub"), add.into_any_element(), None))
        .child(div().flex_1().min_h(px(24.)))
        .child(
            stage_blocks()
                .relative()
                .px(px(48.))
                .pb(px(36.))
                .child(block(root.tr("guide.title"), steps.into_any_element(), wide(add_more).into_any_element()))
                .when(!facts.is_empty(), |d| d.child(block(root.tr("home.w.now"), now.into_any_element(), wide(stats).into_any_element())))
                .when(!list.is_empty(), |d| d.child(block(root.tr("home.w.top"), top.into_any_element(), wide(all_games).into_any_element()))),
        )
}

/// Home drawn small: on a window too narrow for the accounts column, or a short one. Its stage is then a low band
/// (the game's picture and the faces left out, what to do at its right) and its games stand in short tiles, so two
/// rows of them show whole under it (the owner's mock-up of a small window, 2026-11-15: before, the stage took half
/// the window and the second row of games was cut at the window's foot).
fn small(root: &Root) -> bool {
    root.no_side.get() || root.window_h.get() < 760.
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
    // Accounts but no game yet (before any account, Home is the welcome instead: `welcome`).
    let Some(g) = all.get(focus) else {
        if !compact {
            return bloxkeep_hero(root, width, cx);
        }
        return base.child(
            div()
                .flex_1()
                .min_w_0()
                .py(px(24.))
                .flex()
                .flex_col()
                .gap(px(10.))
                .child(div().text_size(px(40.)).line_height(px(46.)).font_weight(FontWeight::BOLD).child(root.tr("home.noGame")))
                .child(div().max_w(px(520.)).text_size(px(T_L)).text_color(on_picture(0.86)).child(root.tr("home.noGameSub")))
                .child(div().mt(px(6.)).flex().gap(px(8.)).child(Button::new("stage-pick", root.tr("home.pickGame")).icon("gamepad").primary().big().on_click(cx.listener(|r, _, _, cx| {
                    r.go(Page::Games);
                    cx.notify();
                })))),
        );
    };
    let running = !g.accounts.is_empty();
    let n = g.accounts.len() as i64;
    let cover = look(root, g.place, "cover").map(str::to_string);
    // The stage is ONE block on the game's own colour (taken from its picture, solid: the base its tiles stand on
    // elsewhere), the words on its left half and the cover filling its right half edge to edge, sharp. It was words on
    // the bare panel with a small cover floating at the right (the owner, 2026-11-13: "trông đơn điệu quá").
    let icon_url = look(root, g.place, "icon").map(str::to_string).or_else(|| g.icon.clone());
    let tone = icon_url.as_deref().and_then(|u| images::tone(u, cx)).or_else(|| cover.as_deref().and_then(|u| images::tone(u, cx)));
    // (Plain, not mixed with the game's colour: an orange game made it brown; the owner's rule of 2026-11-15.)
    let _ = tone;
    let badge = look(root, g.place, "icon").map(str::to_string).or_else(|| g.icon.clone()).and_then(|u| images::pic(&u, cx));
    let dropped: Vec<Value> = g.accounts.iter().filter(|a| is_dropped(a)).cloned().collect();
    let since = g.accounts.iter().filter_map(|a| a.pointer("/session/since").and_then(Value::as_str).and_then(crate::time::iso_secs)).min();

    // What it is doing: its state (a dot and a word), then the facts.
    let dot = state_mark("stage-state", running, 7., if compact { c(TEXT_2) } else { on_picture(0.78) });
    let state_word = root.tr(if running { "home.playing" } else { "home.recent" });
    let mut bits: Vec<AnyElement> = Vec::new();
    if running {
        bits.push(root.i18n.tn("home.accN", n, &[]).into_any_element());
        if !dropped.is_empty() {
            bits.push(div().text_color(c(WARN)).font_weight(FontWeight::MEDIUM).child(root.i18n.tv("ui.droppedN", &[("n", &dropped.len().to_string())])).into_any_element());
        }
        if let Some(t) = since {
            bits.push(div().font_features(tnum()).child(crate::time::clock(crate::time::now_secs() - t)).into_any_element());
        }
    } else {
        if g.hours > 0. {
            bits.push(played_time(root, g.hours).into_any_element());
        }
        // When it was last played (Home.tsx's stage: "last 3 days ago").
        if g.last > 0. {
            let ago = crate::time::ago(crate::time::now_secs() - (g.last / 1000.) as i64, &root.i18n);
            bits.push(root.i18n.tv("home.last", &[("t", &ago)]).into_any_element());
        }
    }

    let mut acts = div().when(!compact, |d| d.mt(px(6.))).flex().flex_wrap().gap(px(8.));
    if running {
        acts = acts.child(Button::new("stage-windows", root.i18n.tn("home.windowsN", n, &[])).icon("screens").primary().big().pill().on_click(
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
        acts = acts.child(Button::new("stage-rejoin", label).icon("refresh").tint().big().pill().on_click(cx.listener(move |r, _, _, cx| r.rejoin(&dropped, cx))));
    }
    // The game into the Play box on Accounts, to pick who plays it (Home.tsx's put-game): "Add accounts" while it runs.
    let place = g.place;
    acts = acts.child(
        Button::new("stage-put", root.tr(if running { "home.addMore" } else { "home.pickAccounts" }))
            .icon(if running { "userPlus" } else { "play" })
            .map(|b| if running { b.smoke() } else { b.primary() })
            .big()
            .pill()
            .on_click(cx.listener(move |r, _, window, cx| crate::pages::accounts::put_game(r, place, window, cx))),
    );
    // Its page in Games: Roblox's figures, your accounts in it, the quiet hours, its news.
    let (place, name) = (g.place, g.name.clone());
    acts = acts.child(Button::new("stage-game", root.tr("home.gamePage")).icon("info").smoke().big().pill().on_click(cx.listener(move |r, _, _, cx| {
        crate::pages::games::open_place(r, place, &name);
        cx.notify();
    })));

    // Who is in it, faces overlapping (six at most, then "+N").
    let who = running.then(|| {
        let more = g.accounts.len().saturating_sub(6);
        div()
            .flex()
            .items_center()
            .children(g.accounts.iter().take(6).enumerate().map(|(i, a)| div().when(i > 0, |d| d.ml(px(-8.))).child(crate::pages::accounts::stacked(a, 30., RAISED, cx))))
            .when(more > 0, |d| d.child(div().ml(px(8.)).text_size(px(T_S)).text_color(on_picture(0.86)).font_features(tnum()).child(format!("+{more}"))))
    });
    let clip = crate::pages::clips::tile_clip(root, g.place, cx);
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
                                .child(fact_line(std::iter::once(div().flex().items_center().gap(px(6.)).text_color(c(TEXT)).font_weight(FontWeight::MEDIUM).child(dot).child(state_word).into_any_element()).chain(bits).collect())),
                        ),
                )
                .child(div().flex_none().max_w_full().child(acts)),
        );
    }
    // A hero (the owner, 2026-11-17: "chỗ này làm hero được thì tuyệt"), built as the welcome's stage is, to Turtle's
    // launcher home (dribbble.com/shots/11189960, "Giữ C"): the game's picture across the whole block, sharp, darkening
    // to the left and down; its state in capitals, its logo, what it is doing and what to do at the left. Its gameplay
    // clip plays under the shades while the mouse rests on it.
    // (Edge to edge, the owner 2026-11-17: "ảnh nên tràn viền hơn nữa": the picture fills the panel's top to its
    // edges, its round top corners, and fades into the panel at its foot, as the welcome's does.)
    let hero_h = hero_h(root, width);
    let beside = hero_beside(width, hero_h);
    let back = hero_back(root, Some(g.place), width, hero_h, clip.layer, cx);
    // What it is doing, in words of the page's size: a game being played has the bars music apps mark a song that
    // plays with (Spotify's, Apple Music's), in green; one played before its clock. It was a dot and a line of
    // capitals, then a glass pill with a dot (the owner, 2026-11-19: "trông hơi lạc quẻ và AI"; picked A of three).
    let kicker = if running {
        div().flex().items_center().gap(px(8.)).text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).text_color(c(OK)).child(crate::kit::playing_bars("stage-bars", true, c(OK))).child(state_word)
    } else {
        div().flex().items_center().gap(px(8.)).text_size(px(T_M)).font_weight(FontWeight::MEDIUM).text_color(on_picture(0.78)).child(dot).child(state_word)
    };
    // Roblox's own words on the game (its first paragraph, cleaned: `blurb`) and its figures: how many play it now,
    // how many like it, who made it (Steam's redesigned library hero, dribbble.com/shots/26698011: a short
    // description under the title; the owner, 2026-11-19: "đoạn này hơi trống").
    let details = root.games.details.get(&g.place);
    let about = details.and_then(|d| d.get("description")).and_then(Value::as_str).and_then(blurb);
    let stats = details.map(|d| hero_figures(root, d)).filter(|f| !f.is_empty());
    // The words' room, from the hero's left pad to where its banner starts: the name is given that width in so many
    // px, less its icon's. Left to shrink by itself it didn't: a name wider than the room ran on over the banner
    // ("Throw Food For Animals", the owner's main screen, 2026-11-18; the wider second screen had the room). A long
    // name takes a second line before it is cut.
    let room = (width - 48. - hero_words_end(width, hero_h)).max(160.);
    let lead = if badge.is_some() { 76. } else { 0. };
    let name_el = match crate::pages::games::logo_img(root, g.place, &g.name, 110., room.min(460.), cx) {
        // (The game's own logo in place of its name in words, when its wiki has one.)
        Some(logo) => div().flex().child(logo),
        None => div()
            .flex()
            .items_center()
            .gap(px(16.))
            .when_some(badge, |d, b| {
                d.child(div().size(px(60.)).flex_none().rounded(px(R_L)).overflow_hidden().border_1().border_color(ca(WHITE, 0.14)).child(img(b).size_full().object_fit(ObjectFit::Cover).rounded(px(R_L))))
            })
            // (Smaller in a narrow room: at 48 px two lines of name, the faces and two rows of buttons stood taller
            // than the hero, whose top line was cut off; "Piece Online" was wider than the room besides.)
            .child({
                let (size, line) = if room - lead < 380. { (36., 42.) } else { (48., 54.) };
                div().min_w_0().w_full().max_w(px((room - lead).clamp(120., 640.))).text_size(px(size)).line_height(px(line)).font_weight(FontWeight::BOLD).line_clamp(2).child(game_name(g.name.clone(), icon_url.as_deref(), cx))
            }),
    };
    div().w_full().child(hero_box(hero_h, beside)
        .id("stage-hero")
        .on_hover(clip.hover)
        .child(back)
        .child(
            hero_words(room, hero_h, beside)
                .gap(px(12.))
                .child(kicker)
                .child(name_el)
                .child(div().flex().flex_wrap().items_center().gap(px(12.)).children(who).child(fact_line(bits)))
                .when_some(about, |d, a| d.child(div().max_w(px(room.min(460.))).text_size(px(T_M)).line_height(px(21.)).text_color(on_picture(0.72)).line_clamp(2).child(a)))
                .when_some(stats, |d, s| d.child(div().flex().gap(px(28.)).children(s)))
                .child(acts.mt(px(4.))),
        ))
}

/// A game's figures under its words on the hero: players now, the share that likes it, its maker. Each only when
/// Roblox gave it.
fn hero_figures(root: &Root, d: &Value) -> Vec<Div> {
    let count = |k: &str| d.get(k).and_then(Value::as_i64).filter(|v| *v >= 0);
    let liked = d.get("likes").and_then(Value::as_f64).filter(|v| *v > 0.).or_else(|| {
        let (up, down) = (count("upVotes")? as f64, count("downVotes")? as f64);
        (up + down > 0.).then(|| 100. * up / (up + down))
    });
    let maker = d.get("creator").and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty());
    let mut out = Vec::new();
    if let Some(n) = count("playing").filter(|n| *n > 0) {
        out.push(hero_figure(root.tr("home.fig.playing"), crate::kit::thousands(n)));
    }
    if let Some(p) = liked {
        out.push(hero_figure(root.tr("home.fig.liked"), format!("{}%", p.round() as i64)));
    }
    if let Some(m) = maker {
        out.push(hero_figure(root.tr("gp.creator"), m.to_string()).min_w_0().max_w(px(220.)));
    }
    out
}

/// A figure on the hero: the number at the size of the facts, its word small and faint under it.
fn hero_figure(label: String, value: String) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(1.))
        .child(div().truncate().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).font_features(tnum()).text_color(on_picture(1.)).child(value))
        .child(div().text_size(px(T_XS)).font_weight(FontWeight::MEDIUM).text_color(on_picture(0.55)).child(label))
}

/// The first words of a game's description on Roblox, fit for one line or two on the hero: its first paragraph after
/// the lines that are no words (emoji only, "🔥UPDATE 24🔥", "‼️ NEW CODES ‼️", a link, a rule of ━━━), with
/// links and the signs that are no letters (emoji, stars, rules) taken out. None when nothing like a sentence is left:
/// the line is then not shown (the owner, 2026-11-19: "lỡ dòng mô tả đó nhiều hơn bình thường thì sao").
pub(crate) fn blurb(text: &str) -> Option<String> {
    // A sign kept: letters (every script) and digits, spaces, and the marks of a sentence.
    let kept = |ch: char| ch.is_alphanumeric() || ch.is_whitespace() || ".,;:!?'\"()-–—…’‘“”%&/+#@$".contains(ch);
    let tidy = |line: &str| -> String {
        let words: Vec<&str> = line.split_whitespace().filter(|w| {
            let l = w.to_lowercase();
            !(l.starts_with("http") || l.starts_with("www.") || l.contains("discord.gg") || l.contains(".com/") || l.contains("roblox.com"))
        }).collect();
        let s: String = words.join(" ").chars().filter(|ch| kept(*ch)).collect();
        s.split_whitespace().collect::<Vec<_>>().join(" ").trim_matches(|ch: char| !ch.is_alphanumeric() && !".!?…)\"”’".contains(ch)).to_string()
    };
    // A line of words: a dozen letters at least, and not shouted (an "UPDATE 24 IS OUT" heading).
    let wordy = |s: &str| {
        let letters: Vec<char> = s.chars().filter(|ch| ch.is_alphabetic()).collect();
        let upper = letters.iter().filter(|ch| ch.is_uppercase()).count();
        letters.len() >= 12 && !(upper * 10 > letters.len() * 6 && s.chars().count() < 70)
    };
    let mut out: Vec<String> = Vec::new();
    for line in text.lines() {
        // A line with a link is an invitation (a Discord, a group, a social page), not the game's words.
        let l = line.to_lowercase();
        if ["http", "www.", "discord.gg", ".com/", ".gg/"].iter().any(|k| l.contains(k)) {
            if !out.is_empty() {
                break;
            }
            continue;
        }
        let s = tidy(line);
        if s.is_empty() {
            // A blank line ends the first paragraph (what follows is mostly its updates, codes and links).
            if !out.is_empty() {
                break;
            }
            continue;
        }
        if wordy(&s) {
            out.push(s);
        } else if !out.is_empty() {
            break;
        }
        // (Far more than two lines hold: no need to read on.)
        if out.iter().map(|l| l.chars().count()).sum::<usize>() > 400 {
            break;
        }
    }
    let joined = out.join(" ");
    (joined.chars().count() >= 20).then_some(joined)
}

/// The hero's height: the panel's top, edge to edge.
fn hero_h(root: &Root, width: f32) -> f32 {
    (width * 0.3).clamp(340., 520.).min((root.window_h.get() - 380.).max(340.)).round()
}

/// Accounts but no game played yet (a brand new account): the hero is Bloxkeep's own (the owner 2026-11-17: "thiết
/// kế 1 banner hero về bloxkeep đi"), in the same frame as a game's (Turtle's launcher home): Bloxkeep's own banner
/// at the right (crisp::hero; a game's picture stood there first, and the owner had meant a picture of Bloxkeep's
/// own, 2026-11-18), Bloxkeep's mark and name in capitals, what to do first, and Bloxkeep now in two figures (Nexus'
/// figures under its buttons, dribbble.com/shots/27766654).
fn bloxkeep_hero(root: &Root, width: f32, cx: &mut Context<Root>) -> Div {
    let figures: Vec<Div> = [("pcs", "home.w.pcs"), ("alts", "home.w.alts")]
        .iter()
        .filter_map(|(key, words)| community_sum(root, key).map(|n| figure(root.tr(words), crate::kit::thousands(n))))
        .collect();
    let pick = Button::new("stage-pick", root.tr("home.pickGame")).icon("gamepad").primary().big().on_click(cx.listener(|r, _, _, cx| {
        r.go(Page::Games);
        cx.notify();
    }));
    let tries = first_games(root, cx);
    let under = (!figures.is_empty() || tries.is_some()).then(|| div().flex().flex_col().children(tries).when(!figures.is_empty(), |d| d.child(div().mt(px(10.)).flex().gap(px(36.)).children(figures))).into_any_element());
    div().w_full().child(own_hero(root, width, "Bloxkeep".into(), root.tr("home.noGame"), root.tr("home.noGameSub"), pick.into_any_element(), under))
}

/// The first game, one click away: what Bloxkeep's users play most, each with Play for the first account (the
/// owner's "Cả ba bước", 2026-11-19: after the first account, the top games with a button straight into one).
fn first_games(root: &Root, cx: &mut Context<Root>) -> Option<AnyElement> {
    let id = root.accounts().first().and_then(|a| a.get("id").and_then(Value::as_str)).map(str::to_string)?;
    // (Two, one under the other: the hero's words column is narrow, and a third pushed its figures off a laptop's
    // window.)
    let n = 2;
    let games: Vec<(i64, String)> = community_games(root).into_iter().take(n).map(|(p, name, _)| (p, name)).collect();
    if games.is_empty() {
        return None;
    }
    let card = |i: usize, place: i64, name: String, cx: &mut Context<Root>| {
        let pic = look(root, place, "icon").map(str::to_string).and_then(|u| images::pic_pair(&u, 36., 36., cx));
        let id = id.clone();
        div()
            .w_full()
            .max_w(px(360.))
            .flex()
            .items_center()
            .gap(px(10.))
            .p(px(6.))
            .pr(px(8.))
            .rounded(px(R_L))
            .bg(ca(SHADE, 0.45))
            .border_1()
            .border_color(ca(WHITE, 0.1))
            .child(div().size(px(36.)).flex_none().rounded(px(R_M)).overflow_hidden().bg(c(RAISED_2)).when_some(pic, |d, (s, o)| d.child(fade_in(img(s).size_full().object_fit(ObjectFit::Cover).rounded(px(R_M)), &o))))
            .child(div().flex_1().min_w_0().truncate().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(name))
            .child(Button::new(("first-play", i), root.tr("home.tryPlay")).small().icon("play").on_click(cx.listener(move |r, _, _, cx| {
                #[cfg(feature = "core")]
                crate::core::log::note_file(&format!("First game: Play on place {place} from Home"));
                r.play_at(std::slice::from_ref(&id), place.to_string(), cx);
                cx.notify();
            })))
    };
    let mut row = div().flex().flex_col().gap(px(8.));
    for (i, (place, name)) in games.into_iter().enumerate() {
        row = row.child(card(i, place, name, cx));
    }
    Some(div().mt(px(14.)).flex().flex_col().gap(px(8.)).child(caps(root.tr("home.tryTitle"), on_picture(0.7))).child(row).into_any_element())
}

/// Facts on one line, a dot between each.
fn fact_line(items: Vec<AnyElement>) -> Div {
    crate::kit::dotted(items).text_size(px(T_M)).text_color(on_picture(0.86))
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
const SIDE: f32 = PAGE_X;

/// Show completed minutes from recorded play time, including sessions shorter than an hour.
pub(crate) fn played_time(root: &Root, hours: f64) -> String {
    let minutes = (hours * 3_600_000.).round() as u64 / 60_000;
    let whole_hours = minutes / 60;
    let rest = minutes % 60;
    let duration = match (whole_hours, rest) {
        (0, 0) => root.tr("ui.underMinute"),
        (0, m) => root.i18n.tv("gp.minutes", &[("n", &m.to_string())]),
        (h, 0) => root.i18n.tv("gp.hours", &[("n", &h.to_string())]),
        (h, m) => format!("{} {}", root.i18n.tv("gp.hours", &[("n", &h.to_string())]), root.i18n.tv("gp.minutes", &[("n", &m.to_string())])),
    };
    root.i18n.tv("ui.playedTime", &[("time", &duration)])
}

/// The games, under a head with two choices: this PC's (running, then played lately) and what Bloxkeep's users play
/// now; their wide pictures one row of the page's grid, each on its game's colour.
fn shelf(root: &Root, all: &[Game], focus: usize, width: f32, library: Option<HomeTab>, cx: &mut Context<Root>) -> impl IntoElement {
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
    let tab_now = library.unwrap_or_else(|| {
        if root.home_tab == HomeTab::Games && all.is_empty() && !community.is_empty() { HomeTab::Community } else { root.home_tab }
    });
    // (Two rows of them then, as nothing else is under the welcome: a big window had its lower half empty.)
    let no_presets = root.state.pointer("/settings/presets").and_then(Value::as_array).is_none_or(|l| l.is_empty());
    // The library too, two rows while nothing plays (the figures then take a row): the owner's maximised window had its
    // lower third empty under three presets (2026-11-12).
    let short = library.is_none() && small(root);
    let rows = if short || (all.is_empty() && no_presets) || playing_now(root) == 0 { 2 } else { 1 };
    // Fewer games than the row holds: they share the whole row (three to a row at least, so one game isn't a
    // banner), rather than leaving its right part empty.
    let n_items = match tab_now {
        HomeTab::Games => all.len(),
        HomeTab::Community => community.len(),
    };
    let tile_w = if n_items > 0 && n_items < cols {
        let k = n_items.max(3).min(cols);
        // (No bigger than 420: four games shared a 3840 px row at ~900 px each, a 4K screen at 100 %, the shots of
        // 2026-11-18; up to that they still fill the row on every usual screen.)
        ((width - (k - 1) as f32 * GRID_GAP) / k as f32).floor().min(420.)
    } else {
        tile_w
    };
    // Full rows only (a lone game on a second row looked dropped); the tabs count what shows, See all has the rest.
    let full_rows = |n: usize| {
        if library.is_some() {
            return n;
        }
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
                let name = g.name.clone();
                let sub = if g.accounts.is_empty() && g.hours <= 0. {
                    (root.tr("home.recent"), false)
                } else if g.accounts.is_empty() {
                    (played_time(root, g.hours), false)
                } else {
                    (root.tr("home.playing"), true)
                };
                let icon_url = look(root, g.place, "icon").map(str::to_string).or_else(|| g.icon.clone());
                let cover = look(root, g.place, "cover").map(str::to_string);
                cascade(
                    "home-games",
                    i,
                    cover_tile(("game", i), cover, icon_url, g.name.clone(), sub, g.accounts.len(), library.is_none() && i == focus && root.picked.is_some(), tile_w, short, crate::pages::clips::tile_clip(root, place, cx), cx).on_click(cx.listener(move |r, _, _, cx| {
                        if library.is_some() {
                            super::games::open_place(r, place, &name);
                        } else {
                            r.picked = Some(place);
                        }
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
                if library.is_some() {
                    r.games.home_library = Some(t);
                    crate::scrolled::handle("home-library").set_offset(Point::default());
                } else {
                    r.home_tab = t;
                }
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
                .when(library.is_none(), |d| d.child(see_all("home-games-all", root.tr("home.all"), cx.listener(move |r, _, _, cx| {
                    r.games.open = None;
                    r.games.view = super::games::View::Games;
                    r.games.home_library = Some(tab_now);
                    crate::scrolled::handle("home-library").set_offset(Point::default());
                    r.go(Page::Games);
                    cx.notify();
                })))),
        )
        .child(if tiles.is_empty() {
            div().h(px(64.)).flex().items_center().text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("home.noGameSub"))
        } else {
            grid(GRID_GAP).children(tiles)
        })
}

pub(crate) fn library(root: &Root, tab: HomeTab, width: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let all = game_library(root, true);
    let pad = PAGE_X;
    let header = div().relative().flex().flex_none().items_center().gap(px(10.)).px(px(pad)).py(px(18.))
        .child(super::games::brand(root))
        .child(Button::new("home-library-back", root.tr("wiki.back")).icon("chevronLeft").small().on_click(cx.listener(|r, _, _, cx| {
            r.games.home_library = None;
            r.go(Page::Home);
            cx.notify();
        })))
        .child(super::games::island(root, cx));
    let body = div().px(px(pad)).pb(px(24.)).child(shelf(root, &all, 0, width - 2. * pad, Some(tab), cx));
    div().size_full().flex().flex_col().child(header).child(crate::scrolled::body("home-library", body))
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
                    .rounded_full()
                    .bg(hsla(0., 0., 0., 0.58))
                    .border_1()
                    .border_color(ca(WHITE, 0.12))
                    .text_size(px(T_S))
                    .font_weight(FontWeight::SEMIBOLD)
                    .font_features(tnum())
                    .text_color(c(WHITE))
                    .child(icon("screens", 12., c(WHITE)))
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
        // Its name in the page's own white and one line under it: the picture above says which game it is, so the
        // small icon beside the name went, and the name's two colours with it (Steam's redesigned library,
        // dribbble.com/shots/27091729; the owner approved the mock-up, 2026-11-19). A game being played says so with
        // the bars music apps mark a song that plays with, in green, as the hero above does.
        .child(
            div()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(div().truncate().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).line_height(px(19.)).child(name))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(7.))
                        .text_size(px(T_S))
                        .text_color(if live { c(OK) } else { c(MUTED) })
                        .when(live, |d| d.font_weight(FontWeight::SEMIBOLD).child(crate::kit::playing_bars(if id.0 == "game" { "tile-bars" } else { "tile-bars-cm" }, true, c(OK)).flex_none()))
                        .font_features(tnum())
                        .child(div().truncate().child(sub)),
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
        // The card opens the preset to change it; its Play button runs it (the owner, 2026-11-17).
        let open = p.clone();
        cards = cards.child(crate::kit::block_card(("preset", i))
                .on_click(cx.listener(move |r, _, window, cx| {
                    r.go(Page::Auto);
                    crate::pages::preset_form::open(r, Some(open.clone()), window, cx);
                    cx.notify();
                }))
                .cursor_pointer()
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
                        // (One line whatever the game is called: its name is cut, the count stays. As facts that wrap,
                        // a long name sent the count to a line of its own and that card stood taller than the others in
                        // its row, the name cut with no "…": the shot with long names, 2026-11-18.)
                        .child(
                            div()
                                .min_w_0()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .text_size(px(T_S))
                                .text_color(c(MUTED))
                                .child(div().min_w_0().truncate().child(game))
                                .child(sep())
                                .child(div().flex_none().font_features(tnum()).child(root.i18n.tn("ps.accN", n, &[]))),
                        )
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
                .child(crate::kit::no_pick(
                    IconButton::new(("run", i), "play").size(34.).round().filled().disabled(n == 0).tip(root.tr("ui.run")).on_click(cx.listener(
                        move |r, _, _, cx| r.launch("runPreset", json!({ "id": id }), false, cx),
                    )),
                )));
    }
    div().flex().flex_col().gap(px(12.)).when(!presets.is_empty(), |d| {
        d.child(section_head(root.tr("nav.presets"), Some(presets.len())).child(see_all("presets-all", root.tr("home.all"), cx.listener(|r, _, _, cx| {
            r.go(Page::Auto);
            cx.notify();
        }))))
        .child(cards)
    })
}
