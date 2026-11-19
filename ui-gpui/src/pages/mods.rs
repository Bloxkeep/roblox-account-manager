// Roblox › Mods: a library of Roblox mods (GameBanana's Roblox section) to put in with one click, and the ones put in,
// each on or off, in the order they win over each other, taken out, or all of Roblox's own files put back. The mods
// are kept by modstore.rs and laid into Roblox's builds as a window opens (core/roblox_mods.rs). For every account on
// this PC: Roblox's files are one for all of them.
use std::collections::{HashMap, HashSet};

use gpui_kit::{prelude::*, *};
use serde_json::{Value, json};
use crate::ask::reply;

use crate::{
    images,
    kit::*,
    modstore::{self, Choice, Installed, Listed},
    roblox,
    shell::Root,
    theme::*,
};

#[derive(Default)]
pub struct Data {
    /// Library (false) or the ones put in (true).
    pub mine: bool,
    /// The library's kind (0: all), sort, words searched, and its pages so far (None while the first is read).
    kind: i64,
    sort: &'static str,
    words: String,
    list: Option<Result<Vec<Listed>, String>>,
    page: u32,
    more: bool,
    reading: bool,
    /// Which reading is the latest: an older one's answer is dropped.
    ask: u64,
    /// Mods being put in, and why the last of each couldn't be.
    adding: HashSet<i64>,
    refused: HashMap<i64, String>,
    /// A mod that offers a few versions to pick one of (six Wii cursors), shown in its card until one is picked.
    picking: Option<(i64, Vec<Choice>)>,
    /// The list put in (read from disk when shown), and the one whose Remove waits for its second click.
    have: Vec<Installed>,
    read_have: bool,
    armed: Option<i64>,
    restoring: bool,
    /// A file from the PC being put in.
    importing: bool,
    /// The mod whose own page is shown (a card clicked), what GameBanana said of it (None while it is read), and which
    /// of its pictures is the big one.
    open: Option<Listed>,
    profile: Option<Result<modstore::Profile, String>>,
    shot: usize,
}

impl Data {
    pub fn words_empty(&self) -> bool {
        self.words.is_empty()
    }
}

pub fn want(r: &mut Root, cx: &mut Context<Root>) {
    if !r.mods.read_have {
        r.mods.read_have = true;
        r.mods.have = modstore::installed();
    }
    if r.mods.list.is_none() && !r.mods.reading {
        read(r, false, cx);
    }
    // BLOXKEEP_SHOT_MODS=open: a hidden shot of the first mod's own page, once the library is read.
    #[cfg(feature = "shot")]
    if std::env::var("BLOXKEEP_SHOT_MODS").is_ok_and(|v| v == "open") && r.mods.open.is_none() {
        if let Some(Ok(list)) = &r.mods.list {
            if let Some(first) = list.first().cloned() {
                open_mod(r, first, cx);
            }
        }
    }
}

/// The library read again from its first page (`next`: the next page added).
fn read(r: &mut Root, next: bool, cx: &mut Context<Root>) {
    let m = &mut r.mods;
    if m.sort.is_empty() {
        m.sort = "popular";
    }
    m.ask += 1;
    let ask = m.ask;
    m.reading = true;
    let page = if next { m.page + 1 } else { 1 };
    if !next {
        m.list = None;
    }
    let (kind, sort, words) = (m.kind, m.sort, m.words.clone());
    let rx = roblox::run(move || modstore::browse(kind, sort, &words, page));
    reply(rx, cx, move |r, got, cx| {
        let m = &mut r.mods;
        if m.ask != ask {
            return;
        }
        m.reading = false;
        match got {
            Ok((list, more)) => {
                m.page = page;
                m.more = more;
                match (next, m.list.as_mut()) {
                    (true, Some(Ok(l))) => {
                        for x in list {
                            if !l.iter().any(|y| y.id == x.id) {
                                l.push(x);
                            }
                        }
                    }
                    _ => m.list = Some(Ok(list)),
                }
            }
            Err(e) if !next => m.list = Some(Err(e)),
            Err(_) => m.more = false,
        }
        cx.notify();
    });
}

/// Enter in the search box: those words searched (an empty box: the list again).
pub fn search(r: &mut Root, cx: &mut Context<Root>) {
    let words = r.mods_q.read(cx).value().trim().to_string();
    if words == r.mods.words && r.mods.list.is_some() {
        return;
    }
    r.mods.words = words;
    r.mods.mine = false;
    read(r, false, cx);
    cx.notify();
}

/// A mod's own page, inside Bloxkeep (the owner, 2026-11-15: a click on a mod should show its page, not only a link out).
fn open_mod(r: &mut Root, m: Listed, cx: &mut Context<Root>) {
    let id = m.id;
    r.mods.open = Some(m);
    r.mods.profile = None;
    r.mods.shot = 0;
    let rx = roblox::run(move || modstore::profile(id));
    reply(rx, cx, move |r, got, cx| {
        // (Another mod opened, or the page left, by now.)
        if r.mods.open.as_ref().is_some_and(|o| o.id == id) {
            r.mods.profile = Some(got);
            cx.notify();
        }
    });
    cx.notify();
}

/// Install: the mod's downloads are read first; one is put in at once, a few are shown in its card to pick from.
fn add(r: &mut Root, m: Listed, cx: &mut Context<Root>) {
    if !r.mods.adding.insert(m.id) {
        return;
    }
    r.mods.refused.remove(&m.id);
    r.mods.picking = None;
    let id = m.id;
    let rx = roblox::run(move || modstore::choices(id));
    reply(rx, cx, move |r, got, cx| {
        match got {
            Ok(list) if list.len() == 1 => {
                r.mods.adding.remove(&id);
                put_in(r, m, list[0].clone(), cx);
            }
            Ok(list) if list.len() > 1 => {
                r.mods.adding.remove(&id);
                r.mods.picking = Some((id, list));
            }
            Ok(_) => {
                r.mods.adding.remove(&id);
                let said = r.tr("mods.noFile");
                r.mods.refused.insert(id, said);
            }
            Err(why) => {
                r.mods.adding.remove(&id);
                let said = r.i18n.tv("mods.failed", &[("why", if why.is_empty() { "GameBanana" } else { &why })]);
                r.mods.refused.insert(id, said);
            }
        }
        cx.notify();
    });
}

fn put_in(r: &mut Root, m: Listed, pick: Choice, cx: &mut Context<Root>) {
    if !r.mods.adding.insert(m.id) {
        return;
    }
    r.mods.picking = None;
    let id = m.id;
    let live_key = format!("mod:{id}");
    {
        let l = crate::pages::games::Live { key: live_key.clone(), pic: m.pic.clone(), round: false, icon: "layers", tone: TEXT_2, title: m.name.clone(), sub: r.tr("live.installing"), act: None, busy: true, at: std::time::Instant::now() };
        crate::pages::games::live_show(r, l, cx);
    }
    let rx = roblox::run(move || crate::net::noted("Mod install", modstore::install(&m, &pick)));
    reply(rx, cx, move |r, done, cx| {
        r.mods.adding.remove(&id);
        match &done {
            Ok(n) => {
                let sub = r.i18n.tv("mods.added", &[("n", &n.to_string())]);
                crate::pages::games::live_done(r, &live_key, "check", OK, sub, Some(crate::pages::games::LiveAct::Mods), cx);
            }
            Err(_) => {
                let sub = r.tr("live.failed");
                crate::pages::games::live_done(r, &live_key, "x", BAD, sub, None, cx);
            }
        }
        match done {
            Ok(n) => {
                r.mods.have = modstore::installed();
                r.toast = Some((r.i18n.tv("mods.added", &[("n", &n.to_string())]), "ok".into()));
            }
            Err(why) => {
                let said = match why.as_str() {
                    "none" => r.tr("mods.none"),
                    "nofile" => r.tr("mods.noFile"),
                    "big" => r.tr("mods.big"),
                    "unpack" => r.tr("mods.unpack"),
                    _ => r.i18n.tv("mods.failed", &[("why", if why.is_empty() { "GameBanana" } else { &why })]),
                };
                r.mods.refused.insert(id, said);
            }
        }
        cx.notify();
    });
}

fn changed(r: &mut Root, done: Result<(), String>) {
    r.mods.have = modstore::installed();
    if let Err(why) = done {
        r.toast = Some((r.i18n.tv("mods.failed", &[("why", &why)]), "bad".into()));
    }
}

/// One place up (`up`) or down in the order: lower wins where two lay the same file.
fn reorder(r: &mut Root, id: i64, up: bool) {
    let mut list = modstore::installed();
    let Some(i) = list.iter().position(|m| m.id == id) else { return };
    let j = if up { i.checked_sub(1) } else { Some(i + 1).filter(|j| *j < list.len()) };
    if let Some(j) = j {
        list.swap(i, j);
        let done = crate::net::noted("Mods reorder", modstore::reorder(list));
        changed(r, done);
    }
}

fn restore(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    let Some(link) = r.link.clone() else { return };
    if r.mods.restoring {
        return;
    }
    r.mods.restoring = true;
    let rx = link.call("restoreMods", json!({}));
    cx.spawn_in(window, async move |this, cx| {
        let answer = rx.await;
        let _ = this.update_in(cx, |r, _, cx| {
            r.mods.restoring = false;
            let (back, left) = match &answer {
                Ok(Ok(v)) => (v.get("back").and_then(Value::as_i64).unwrap_or(0), v.get("left").and_then(Value::as_i64).unwrap_or(0)),
                _ => (0, -1),
            };
            r.toast = Some(if left > 0 {
                (r.i18n.tv("mods.backSome", &[("n", &back.to_string()), ("left", &left.to_string())]), "bad".into())
            } else if left < 0 {
                (r.tr("mods.backFailed"), "bad".into())
            } else {
                (r.i18n.tv("mods.back", &[("n", &back.to_string())]), "ok".into())
            });
            cx.notify();
        });
    })
    .detach();
}

// ───────────────────────── the page ─────────────────────────

pub fn page(root: &Root, width: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let m = &root.mods;
    let shell = div().id("mods").relative().size_full().rounded(px(R_PANEL)).bg(c(PANEL)).overflow_hidden().flex().flex_col();
    let header = div()
        .flex_none()
        .px(px(24.))
        .pt(px(22.))
        .pb(px(16.))
        .flex()
        .items_center()
        .gap(px(10.))
        .child(crate::pages::games::brand(root))
        .relative().child(crate::pages::games::island(root, cx))
        .child(div().flex_1())
        .child(div().w(px(if width < 900. { 200. } else { 260. })).min_w(px(180.)).flex_shrink(1.).child(field(&root.mods_q, Some("search"))));

    // One row: the kinds as chips, "Installed" last among them (it is another place to look, not a kind), and the
    // sort folded into one menu at the right (three rows of choices read as clutter, the owner, 2026-11-14).
    let n = m.have.len();
    fn pick_kind(k: i64, cx: &mut Context<Root>) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
        cx.listener(move |r: &mut Root, _: &ClickEvent, _: &mut Window, cx: &mut Context<Root>| {
            r.mods.mine = false;
            r.mods.armed = None;
            if r.mods.kind != k || r.mods.list.is_none() {
                r.mods.kind = k;
                read(r, false, cx);
            }
            cx.notify();
        })
    }
    let mut row = div().flex().items_center().gap(px(4.));
    if m.words.is_empty() || m.mine {
        row = row.child(chip("mods-k-0", !m.mine && m.kind == 0).child(root.tr("mods.k.all")).on_click(pick_kind(0, cx)));
        for (id, name) in modstore::KINDS {
            let id = id;
            row = row.child(chip(("mods-k", id as usize), !m.mine && m.kind == id).child(root.tr(&format!("mods.k.{name}"))).on_click(pick_kind(id, cx)));
        }
    } else {
        row = row.child(div().text_size(px(T_M)).text_color(c(MUTED)).child(root.i18n.tv("mods.results", &[("q", &m.words)])));
    }
    row = row.child(div().w(px(20.))).child(
        chip("mods-mine", m.mine)
            .child(root.tr("mods.mine"))
            .when(n > 0, |d| d.child(div().text_color(if m.mine { crate::theme::on_text() } else { c(MUTED) }).font_features(tnum()).child(n.to_string())))
            .on_click(cx.listener(|r, _, _, cx| {
                r.mods.mine = true;
                r.mods.armed = None;
                r.mods.have = modstore::installed();
                cx.notify();
            })),
    );
    let mut bar = div().flex().items_center().gap(px(12.)).child(row).child(div().flex_1());
    if !m.mine && m.words.is_empty() {
        let now = if m.sort.is_empty() { "popular" } else { m.sort };
        let items: Vec<crate::pages::accounts::DropItem> = [("popular", "mods.s.popular"), ("new", "mods.s.new"), ("updated", "mods.s.updated")]
            .into_iter()
            .map(|(k, words)| {
                let f: crate::pages::accounts::DropAct = std::rc::Rc::new(move |r: &mut Root, _: &mut Window, cx: &mut Context<Root>| {
                    r.mods.sort = k;
                    read(r, false, cx);
                });
                (root.tr(words), k == now, f)
            })
            .collect();
        let shown = root.tr(&format!("mods.s.{}", match now { "new" => "new", "updated" => "updated", _ => "popular" }));
        bar = bar.child(crate::pages::accounts::drop_list(root, "mods-sort", "sort", None, shown, items, cx));
    }
    // A mod from the PC (a .zip someone sent, one found elsewhere): put in the same way.
    bar = bar.child(Button::new("mods-import", root.tr("mods.import")).icon("folder").disabled(m.importing).on_click(cx.listener(|r, _, window, cx| import_file(r, window, cx))));

    let inner = width - 48.;
    let content = match &m.open {
        Some(x) => div().px(px(24.)).pb(px(28.)).child(detail(root, x, inner, cx)),
        None => {
            let body = if m.mine { mine(root, inner, cx).into_any_element() } else { library(root, inner, cx).into_any_element() };
            div().px(px(24.)).pb(px(28.)).flex().flex_col().gap(px(18.)).child(bar).child(body)
        }
    };
    shell.child(header).child(crate::scrolled::body("mods-body", content))
}

/// Import: the PC's file box (the core's), then the file put in off the page's thread; the toast says how it went and
/// Installed shows it.
fn import_file(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    let Some(link) = r.link.clone() else { return };
    let rx = link.call("pickModFile", json!({}));
    cx.spawn_in(window, async move |this, cx| {
        let Ok(Ok(answer)) = rx.await else { return };
        let Some(path) = answer.get("path").and_then(Value::as_str).map(std::path::PathBuf::from) else { return };
        let _ = this.update(cx, |r, cx| {
            r.mods.importing = true;
            cx.notify();
        });
        let got = cx.background_spawn(async move { crate::net::noted("Mod import", modstore::import(&path)) }).await;
        let _ = this.update(cx, |r, cx| {
            r.mods.importing = false;
            r.toast = Some(match got {
                Ok((name, n)) => {
                    r.mods.mine = true;
                    r.mods.have = modstore::installed();
                    (r.i18n.tv("mods.imported", &[("name", &name), ("n", &n.to_string())]), "ok".into())
                }
                Err(e) => (r.tr(match e.as_str() {
                    "none" => "mods.importNone",
                    "big" => "mods.importBig",
                    "kind" => "mods.importKind",
                    _ => "mods.importFailed",
                }), "bad".into()),
            });
            cx.notify();
        });
    })
    .detach();
}

fn note(text: String) -> Div {
    div().py(px(60.)).flex().items_center().justify_center().text_size(px(T_M)).text_color(c(MUTED)).child(text)
}

fn library(root: &Root, inner: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let m = &root.mods;
    let list = match &m.list {
        None => return div().child(note(root.tr("mods.reading"))),
        Some(Err(why)) => {
            return div().flex().flex_col().items_center().gap(px(12.)).child(note(root.i18n.tv("mods.readFailed", &[("why", if why.is_empty() { "GameBanana" } else { why })]))).child(Button::new("mods-retry", root.tr("fr.retry")).icon("refresh").on_click(cx.listener(|r, _, _, cx| {
                read(r, false, cx);
                cx.notify();
            })));
        }
        Some(Ok(l)) if l.is_empty() => return div().child(note(root.tr("mods.empty"))),
        Some(Ok(l)) => l,
    };
    let cols = ((inner + 16.) / 300.).floor().clamp(1., 6.);
    let w = ((inner - 16. * (cols - 1.)) / cols).floor();
    let pic_h = (w * 9. / 16.).round();
    let have: HashSet<i64> = m.have.iter().map(|x| x.id).collect();
    let vi = root.i18n.vietnamese;
    let cards = list.iter().enumerate().map(|(i, x)| {
        let pic = (!x.pic.is_empty()).then(|| images::pic_pair(&x.pic, w, pic_h, cx)).flatten();
        let (adding, got) = (m.adding.contains(&x.id), have.contains(&x.id));
        let why = m.refused.get(&x.id).cloned();
        let item = x.clone();
        let (open_a, open_b) = (x.clone(), x.clone());
        let id = x.id;
        let act = if got {
            Button::new(("mod-have", i), root.tr("mods.have")).icon("check").small().disabled(true)
        } else {
            Button::new(("mod-add", i), root.tr(if adding { "mods.adding" } else { "mods.add" })).icon("download").small().disabled(adding).on_click(cx.listener(move |r, _, _, cx| {
                add(r, item.clone(), cx);
                cx.notify();
            }))
        };
        surface()
            .w(px(w))
            .overflow_hidden()
            .flex()
            .flex_col()
            .child(div().id(("mod-open-pic", i)).cursor_pointer().on_click(cx.listener(move |r, _, _, cx| open_mod(r, open_a.clone(), cx))).w_full().h(px(pic_h)).bg(c(RAISED)).overflow_hidden().flex().items_center().justify_center().map(|d| match pic {
                Some((src, key)) => d.child(fade_in(img(src).size_full().object_fit(ObjectFit::Cover), &key)),
                None => d.child(icon("image", 22., c(DIM))),
            }))
            .child(
                div()
                    .p(px(14.))
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(
                        div()
                            .id(("mod-open-name", i))
                            .cursor_pointer()
                            .on_click(cx.listener(move |r, _, _, cx| open_mod(r, open_b.clone(), cx)))
                            .flex()
                            .flex_col()
                            .gap(px(3.))
                            .child(div().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).truncate().child(x.name.clone()))
                            .child(div().text_size(px(T_S)).text_color(c(MUTED)).truncate().child(format!("{} · {}", kind_words(root, &x.kind), x.by))),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(div().flex_1().min_w_0().overflow_hidden().flex().items_center().gap(px(5.)).text_size(px(T_S)).text_color(c(MUTED)).font_features(tnum()).child(icon("eye", 12., c(MUTED))).child(crate::kit::thousands(x.views, vi)).when(x.updated > 0, |d| d.child(div().w(px(6.))).child(icon("clock", 12., c(MUTED))).child(div().min_w_0().truncate().child(age(crate::time::now_secs() - x.updated, vi)))))
                            .child(IconButton::new(("mod-web", i), "external").size(30.).tip(root.tr("mods.onBanana")).on_click(cx.listener(move |r, _, _, _| r.send("openModPage", json!({ "id": id })))))
                            .child(act),
                    )
                    .children(why.map(|t| div().text_size(px(T_S)).text_color(c(WARN)).child(t)))
                    .children(m.picking.as_ref().filter(|(of, _)| *of == x.id).map(|(_, list)| {
                        let rows = list.iter().enumerate().map(|(j, ch)| {
                            let (item, ch2) = (x.clone(), ch.clone());
                            let about = if ch.about.is_empty() { ch.file.clone() } else { ch.about.clone() };
                            menu_row(("mod-pick", i * 100 + j))
                                .gap(px(10.))
                                .child(icon("download", 13., c(MUTED)))
                                .child(div().flex_1().min_w_0().flex().flex_col().child(div().text_size(px(T_M)).text_color(c(TEXT)).truncate().child(about)).child(div().text_size(px(T_XS)).text_color(c(MUTED)).truncate().child(format!("{} · {}", ch.file, size_words(ch.size)))))
                                .on_click(cx.listener(move |r, _, _, cx| {
                                    put_in(r, item.clone(), ch2.clone(), cx);
                                    cx.notify();
                                }))
                        });
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .p(px(4.))
                            .rounded(px(R_M))
                            .bg(c(INSET))
                            .border_1()
                            .border_color(c(LINE))
                            .child(div().px(px(8.)).py(px(4.)).flex().items_center().child(div().flex_1().text_size(px(T_S)).text_color(c(MUTED)).child(root.tr("mods.pickOne"))).child(IconButton::new(("mod-pick-x", i), "x").size(24.).tip(root.tr("common.cancel")).on_click(cx.listener(|r, _, _, cx| {
                                r.mods.picking = None;
                                cx.notify();
                            }))))
                            .children(rows)
                    })),
            )
    });
    div()
        .flex()
        .flex_col()
        .gap(px(18.))
        .child(div().flex().flex_wrap().gap(px(16.)).children(cards))
        .when(m.more, |d| {
            d.child(div().flex().justify_center().child(Button::new("mods-more", root.tr(if m.reading { "mods.reading" } else { "mods.more" })).disabled(m.reading).on_click(cx.listener(|r, _, _, cx| {
                read(r, true, cx);
                cx.notify();
            }))))
        })
        .child(div().text_size(px(T_S)).text_color(c(MUTED)).child(root.tr("mods.from")))
}

/// A mod's page: its pictures (the one picked big, the rest under it), what its maker wrote, and beside them its
/// numbers, files and credits, with Install and the way out to GameBanana at its head.
fn detail(root: &Root, x: &Listed, inner: f32, cx: &mut Context<Root>) -> Div {
    let m = &root.mods;
    let vi = root.i18n.vietnamese;
    let id = x.id;
    let (adding, got) = (m.adding.contains(&id), m.have.iter().any(|h| h.id == id));
    let item = x.clone();
    let act = if got {
        Button::new("modp-have", root.tr("mods.have")).icon("check").disabled(true)
    } else {
        Button::new("modp-add", root.tr(if adding { "mods.adding" } else { "mods.add" })).icon("download").primary().disabled(adding).on_click(cx.listener(move |r, _, _, cx| {
            add(r, item.clone(), cx);
            cx.notify();
        }))
    };
    let head = div()
        .flex()
        .items_center()
        .gap(px(12.))
        .child(Button::new("modp-back", root.tr("av.back")).icon("chevronLeft").small().quiet().on_click(cx.listener(|r, _, _, cx| {
            r.mods.open = None;
            r.mods.profile = None;
            cx.notify();
        })))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(div().text_size(px(26.)).line_height(px(32.)).font_weight(FontWeight::BOLD).truncate().child(x.name.clone()))
                .child(div().text_size(px(T_M)).text_color(c(MUTED)).truncate().child(format!("{} · {}", kind_words(root, &x.kind), x.by))),
        )
        .child(Button::new("modp-web", root.tr("mods.onBanana")).icon("external").on_click(cx.listener(move |r, _, _, _| r.send("openModPage", json!({ "id": id })))))
        .child(act);
    let why = m.refused.get(&id).cloned();
    // A few versions to pick one of, as in its card.
    let picks = m.picking.as_ref().filter(|(of, _)| *of == id).map(|(_, list)| {
        let rows = list.iter().enumerate().map(|(j, ch)| {
            let (item, ch2) = (x.clone(), ch.clone());
            let about = if ch.about.is_empty() { ch.file.clone() } else { ch.about.clone() };
            menu_row(("modp-pick", j))
                .gap(px(10.))
                .child(icon("download", 13., c(MUTED)))
                .child(div().flex_1().min_w_0().truncate().text_color(c(TEXT)).child(about))
                .child(div().flex_none().text_size(px(T_S)).text_color(c(MUTED)).child(size_words(ch.size)))
                .on_click(cx.listener(move |r, _, _, cx| {
                    put_in(r, item.clone(), ch2.clone(), cx);
                    cx.notify();
                }))
        });
        div().flex().flex_col().gap(px(2.)).p(px(6.)).rounded(px(R_M)).bg(c(INSET)).border_1().border_color(c(LINE)).child(div().px(px(8.)).py(px(4.)).text_size(px(T_S)).text_color(c(MUTED)).child(root.tr("mods.pickOne"))).children(rows)
    });

    let wide = inner >= 900.;
    let side_w = 340.;
    let main_w = if wide { inner - side_w - 24. } else { inner };
    let profile = m.profile.as_ref();
    let pics: Vec<String> = match profile {
        Some(Ok(p)) if !p.pics.is_empty() => p.pics.clone(),
        _ => (!x.pic.is_empty()).then(|| x.pic.clone()).into_iter().collect(),
    };
    let at = m.shot.min(pics.len().saturating_sub(1));
    // The picture's own shape sets how tall its place is (a square one in a 16:9 box stood between two grey bands), up
    // to 560; and the place is dark as a picture viewer's, so what is left beside a narrow one reads as its frame.
    let shape = pics.get(at).and_then(|u| images::dims(u, cx)).map(|(w, h)| h as f32 / w.max(1) as f32).unwrap_or(9. / 16.);
    let pic_h = (main_w * shape).round().clamp(240., 560.);
    let big = pics.get(at).and_then(|u| images::pic_pair(u, main_w, pic_h, cx));
    let stage = div().w(px(main_w)).h(px(pic_h)).rounded(px(R_L)).overflow_hidden().bg(c(INSET)).border_1().border_color(c(LINE)).flex().items_center().justify_center().map(|d| match big {
        // (Whole, not cut to the frame: a mod's picture often shows several things side by side.)
        Some((src, key)) => d.child(fade_in(img(src).size_full().object_fit(ObjectFit::Contain), &key)),
        None => d.child(icon("image", 28., c(DIM))),
    });
    let thumbs = (pics.len() > 1).then(|| {
        div().flex().flex_wrap().gap(px(8.)).children(pics.iter().enumerate().map(|(i, u)| {
            let small = images::pic_pair(u, 112., 63., cx);
            div()
                .id(("modp-thumb", i))
                .w(px(112.))
                .h(px(63.))
                .rounded(px(R_S))
                .overflow_hidden()
                .bg(c(RAISED))
                .border_2()
                .border_color(if i == at { c(FACE) } else { ca(WHITE, 0.08) })
                .cursor_pointer()
                .children(small.map(|(src, _)| img(src).size_full().object_fit(ObjectFit::Cover)))
                .on_click(cx.listener(move |r, _, _, cx| {
                    r.mods.shot = i;
                    cx.notify();
                }))
        }))
    });
    // A block of the page: its mark, its name, then what it holds.
    let block_of = |mark: &'static str, tone: u32, title: String, body: AnyElement| {
        surface().p(px(18.)).flex().flex_col().gap(px(12.)).child(div().flex().items_center().gap(px(12.)).child(mark_chip(mark, tone, 32.)).child(div().text_size(px(T_SEC)).font_weight(FontWeight::SEMIBOLD).child(title))).child(body)
    };
    let words = match profile {
        None => Some(note(root.tr("mods.reading"))),
        Some(Err(why)) => Some(note(root.i18n.tv("mods.readFailed", &[("why", if why.is_empty() { "GameBanana" } else { why })]))),
        Some(Ok(p)) if p.text.is_empty() => None,
        Some(Ok(p)) => Some(block_of(
            "book",
            ONLINE,
            root.tr("mods.d.about"),
            div().flex().flex_col().gap(px(4.)).children(p.text.lines().map(|l| div().min_h(px(8.)).text_size(px(T_L)).line_height(px(23.)).text_color(c(TEXT_2)).child(l.to_string()))).into_any_element(),
        )),
    };
    let main = div().w(px(main_w)).flex().flex_col().gap(px(14.)).child(stage).children(thumbs).children(words);

    // Its figures in one strip, each with its own mark: what GameBanana's page leads with.
    let now = crate::time::now_secs();
    let figure = |mark: &'static str, tone: u32, name: String, value: String| {
        div()
            .flex_1()
            .min_w(px(150.))
            .px(px(16.))
            .py(px(14.))
            .flex()
            .items_center()
            .gap(px(12.))
            .child(mark_chip(mark, tone, 36.))
            .child(div().flex().flex_col().min_w_0().child(div().text_size(px(T_S)).text_color(c(MUTED)).truncate().child(name)).child(div().text_size(px(20.)).line_height(px(26.)).font_weight(FontWeight::BOLD).font_features(tnum()).truncate().child(value)))
    };
    let strip = match profile {
        Some(Ok(p)) => {
            let mut cells = vec![
                figure("starFill", FACE, root.tr("mods.d.likes"), crate::kit::thousands(p.likes, vi)),
                figure("eye", ONLINE, root.tr("mods.d.views"), crate::kit::thousands(p.views, vi)),
                figure("download", OK, root.tr("mods.d.downloads"), crate::kit::thousands(p.downloads, vi)),
            ];
            if p.updated > 0 {
                cells.push(figure("clock", TEXT_2, root.tr("mods.d.updated"), age(now - p.updated, vi)));
            } else if p.added > 0 {
                cells.push(figure("calendar", TEXT_2, root.tr("mods.d.added"), age(now - p.added, vi)));
            }
            if !p.version.is_empty() {
                cells.push(figure("tag", TEXT_2, root.tr("mods.d.version"), p.version.clone()));
            }
            Some(surface().flex().flex_wrap().children(cells.into_iter().enumerate().map(|(i, cell)| cell.when(i > 0, |d| d.border_l_1().border_color(c(LINE))))))
        }
        _ => None,
    };

    let mut side = div().w(px(if wide { side_w } else { main_w })).flex_none().flex().flex_col().gap(px(14.)).text_size(px(T_M));
    if let Some(Ok(p)) = profile {
        if !p.files.is_empty() {
            let rows = div().flex().flex_col().children(p.files.iter().take(8).enumerate().map(|(i, (name, size))| {
                div()
                    .h(px(40.))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .when(i > 0, |d| d.border_t_1().border_color(ca(WHITE, 0.05)))
                    .child(icon("folder", 14., c(DIM)))
                    .child(div().flex_1().min_w_0().truncate().text_color(c(TEXT)).child(name.clone()))
                    .child(div().flex_none().text_color(c(MUTED)).font_features(tnum()).child(size_words(*size)))
            }));
            side = side.child(block_of("download", OK, root.tr("mods.d.files"), rows.into_any_element()));
        }
        for (group, names) in &p.credits {
            let rows = div().flex().flex_col().gap(px(10.)).children(names.iter().take(12).map(|(name, role)| {
                let first: String = name.chars().find(|c| c.is_alphanumeric()).map(|c| c.to_uppercase().collect()).unwrap_or_else(|| "?".into());
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(div().size(px(30.)).flex_none().rounded_full().bg(c(RAISED_2)).flex().items_center().justify_center().text_size(px(T_S)).font_weight(FontWeight::BOLD).text_color(c(TEXT_2)).child(first))
                    .child(div().flex().flex_col().min_w_0().child(div().text_color(c(TEXT)).font_weight(FontWeight::MEDIUM).truncate().child(name.clone())).when(!role.is_empty(), |d| d.child(div().text_size(px(T_S)).text_color(c(MUTED)).truncate().child(role.clone()))))
            }));
            side = side.child(block_of("users", TEXT_2, if group.is_empty() { root.tr("mods.d.credits") } else { group.clone() }, rows.into_any_element()));
        }
        if !p.category.is_empty() {
            side = side.child(div().flex().items_center().gap(px(8.)).px(px(4.)).text_color(c(MUTED)).child(icon("layers", 14., c(DIM))).child(root.tr("mods.d.kind")).child(div().text_color(c(TEXT_2)).child(p.category.clone())));
        }
    }
    let body = div().flex().items_start().gap(px(24.)).when(!wide, |d| d.flex_col()).child(main).child(side);
    div()
        .flex()
        .flex_col()
        .gap(px(18.))
        .child(head)
        .children(why.map(|t| div().text_size(px(T_M)).text_color(c(WARN)).child(t)))
        .children(picks)
        .children(strip)
        .child(body)
        .child(div().text_size(px(T_S)).text_color(c(MUTED)).child(root.tr("mods.from")))
}

fn mine(root: &Root, _inner: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let m = &root.mods;
    // Where two lay the same file, the lower one in the list is the one Roblox gets.
    let mut owner: HashMap<String, i64> = HashMap::new();
    for x in m.have.iter().filter(|x| x.on) {
        for f in &x.files {
            owner.insert(f.to_ascii_lowercase(), x.id);
        }
    }
    let name_of = |id: i64| m.have.iter().find(|x| x.id == id).map(|x| x.name.clone()).unwrap_or_default();
    let last = m.have.len().saturating_sub(1);
    let rows = m.have.iter().enumerate().map(|(i, x)| {
        let id = x.id;
        let lost: Vec<i64> = if x.on {
            let mut l: Vec<i64> = x.files.iter().filter_map(|f| owner.get(&f.to_ascii_lowercase()).copied()).filter(|o| *o != id).collect();
            l.sort();
            l.dedup();
            l
        } else {
            Vec::new()
        };
        let lost_n = if x.on { x.files.iter().filter(|f| owner.get(&f.to_ascii_lowercase()).is_some_and(|o| *o != id)).count() } else { 0 };
        let sub = format!("{} · {}", kind_words(root, if x.kind.is_empty() { "all" } else { &x.kind }), root.i18n.tn("mods.files", x.files.len() as i64, &[]));
        let clash = (!lost.is_empty()).then(|| root.i18n.tv("mods.clash", &[("n", &lost_n.to_string()), ("name", &lost.iter().map(|o| name_of(*o)).collect::<Vec<_>>().join(", "))]));
        let pic = (!x.pic.is_empty()).then(|| images::pic_pair(&x.pic, 96., 54., cx)).flatten();
        let armed = m.armed == Some(id);
        div()
            .px(px(16.))
            .py(px(12.))
            .flex()
            .items_center()
            .gap(px(14.))
            .when(i > 0, |d| d.border_t_1().border_color(c(LINE)))
            .child(div().w(px(96.)).h(px(54.)).flex_none().rounded(px(R_S)).bg(c(RAISED)).overflow_hidden().flex().items_center().justify_center().map(|d| match pic {
                Some((src, key)) => d.child(fade_in(img(src).size_full().object_fit(ObjectFit::Cover), &key)),
                None => d.border_1().border_color(c(LINE)).child(icon("image", 16., c(DIM))),
            }))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(3.))
                    .child(div().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).truncate().text_color(if x.on { c(TEXT) } else { c(MUTED) }).child(x.name.clone()))
                    .child(div().text_size(px(T_S)).text_color(c(MUTED)).truncate().child(sub))
                    .children(clash.map(|t| div().text_size(px(T_S)).text_color(c(WARN)).truncate().child(t))),
            )
            .child(IconButton::new(("mod-up", i), "chevronUp").size(30.).disabled(i == 0).tip(root.tr("mods.up")).on_click(cx.listener(move |r, _, _, cx| {
                reorder(r, id, true);
                cx.notify();
            })))
            .child(IconButton::new(("mod-down", i), "chevronDown").size(30.).disabled(i == last).tip(root.tr("mods.down")).on_click(cx.listener(move |r, _, _, cx| {
                reorder(r, id, false);
                cx.notify();
            })))
            .child(Switch::new(("mod-on", i), x.on).on_toggle({
                let this = cx.entity().downgrade();
                move |on, _, cx| {
                    let on = *on;
                    let _ = this.update(cx, |r, cx| {
                        let done = crate::net::noted("Mod switch", modstore::set_on(id, on));
                        changed(r, done);
                        cx.notify();
                    });
                }
            }))
            .child(Button::new(("mod-rm", i), root.tr(if armed { "mods.removeSure" } else { "mods.remove" })).icon("trash").small().on_click(cx.listener(move |r, _, _, cx| {
                if r.mods.armed == Some(id) {
                    r.mods.armed = None;
                    let done = crate::net::noted("Mod remove", modstore::remove(id));
                    changed(r, done);
                } else {
                    r.mods.armed = Some(id);
                }
                cx.notify();
            })))
    });
    let list = if m.have.is_empty() {
        div().child(note(root.tr("mods.noneYet")))
    } else {
        surface().overflow_hidden().flex().flex_col().children(rows)
    };
    // How it works, and the way back.
    let foot = surface()
        .px(px(18.))
        .py(px(14.))
        .flex()
        .items_center()
        .gap(px(14.))
        .child(div().flex_none().child(icon("info", 19., c(TEXT_2))))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(3.))
                .child(div().text_size(px(T_M)).child(root.tr("mods.how")))
                .child(div().text_size(px(T_S)).text_color(c(MUTED)).child(root.tr("mods.howSub"))),
        )
        .child(Button::new("mods-restore", root.tr(if m.restoring { "mods.restoring" } else { "mods.restore" })).icon("history").small().disabled(m.restoring).on_click(cx.listener(|r, _, window, cx| {
            restore(r, window, cx);
            cx.notify();
        })));
    div().flex().flex_col().gap(px(16.)).child(list).child(foot)
}

/// "1.2 MB", "340 KB".
fn size_words(n: u64) -> String {
    if n >= 1 << 20 { format!("{:.1} MB", n as f64 / 1048576.) } else { format!("{} KB", (n / 1024).max(1)) }
}

/// A kind's name in words; GameBanana's own name for one Bloxkeep has no words for.
fn kind_words(root: &Root, kind: &str) -> String {
    let key = format!("mods.k.{kind}");
    if root.i18n.raw(&key).is_some() { root.tr(&key) } else { kind.to_string() }
}

/// How old, short: "3d", "5mo", "2y" ("3 ngày", "5 tháng", "2 năm").
fn age(secs: i64, vi: bool) -> String {
    let d = secs.max(0) / 86400;
    let (n, en, v) = if d < 30 { (d.max(1), "d", " ngày") } else if d < 365 { (d / 30, "mo", " tháng") } else { (d / 365, "y", " năm") };
    format!("{n}{}", if vi { v } else { en })
}
