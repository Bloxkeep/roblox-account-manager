// The game box's list (ui/src/Accounts.tsx's Picker; app.js's openPicker): the games played from Bloxkeep lately, the
// pinned ones, the saved servers, the accounts' Roblox favourites and what is popular; picking one puts it in the box.
// The app keeps the lists ("gameLists": { recent, pinned, favorites, popular }, each { placeId, universeId, name,
// playing, iconUrl, pinned }); saved servers come with its state (settings.savedServers). A star pins or unpins
// ("pinGame"), a recent game can be taken off ("removeRecent"), a saved server deleted ("deleteServer") or its code
// copied ("copyServerCode": Bloxkeep never types into Roblox).
//
// The play field and the list's own field both search Roblox by name. Save server (the bookmark
// beside the box, once it holds a game; app.js's saveServerModal) keeps a private server's link, or a game with the
// code it asks for, under a name ("saveServer").
use std::time::{Duration, Instant};

use gpui_kit::component::WindowExt as _;
use gpui_kit::component::dialog::DialogEntrance;
use gpui_kit::{prelude::*, *};
use serde_json::{Value, json};

use crate::{images, kit::*, shell::Root, theme::*};
use crate::val::s;
use crate::val::int;
use crate::ask::answer;

const TABS: [&str; 5] = ["recent", "pinned", "saved", "favorites", "popular"];
const TYPE_DELAY: Duration = Duration::from_millis(330);

enum SearchResults {
    Loading,
    Games(Vec<Value>),
    Failed,
}

#[derive(Default)]
pub struct Data {
    pub open: bool,
    inline: bool,
    /// A click outside closed it just now: that click on its own button only closes it.
    closed: Option<Instant>,
    tab: usize,
    lists: Option<Value>,
    loading: bool,
    /// A search by name: what was asked, and its games once Roblox has answered; and which asking is the latest.
    found: Option<(String, SearchResults)>,
    seq: u64,
    /// Save server: what is being saved (a link is kept, the game's place and universe, its name), what is wrong, and
    /// whether the app is being asked.
    saving: Option<(bool, i64, i64, String)>,
    sv_error: Option<String>,
    sv_busy: bool,
    /// Which Save-server box is the latest, and whether it is still open: a late answer closes only its own box.
    sv_seq: u64,
    sv_open: bool,
}

/// Leave all launch targets to the existing parser, including invalid links that need its error.
pub fn is_game_name(input: &str) -> bool {
    let input = input.trim();
    if input.is_empty() || input.chars().all(|ch| ch.is_ascii_digit()) || input.parse::<i64>().is_ok() || crate::target::parse(input, "").is_ok() {
        return false;
    }
    let lower = input.to_ascii_lowercase();
    !["http://", "https://", "roblox:", "roblox-player:", "roblox.com/", "www.roblox.com/", "ro.blox.com/", "placeid="]
        .iter().any(|prefix| lower.starts_with(prefix))
}

pub fn search_menu(r: &mut Root, immediate: bool, cx: &mut Context<Root>) {
    if r.picker.open && !r.picker.inline {
        let query = r.picker_q.read(cx).value().trim().to_string();
        search(r, query, immediate, cx);
    }
}

/// Typing in the main play field uses the same results panel as the explicit game picker.
pub fn search_dock(r: &mut Root, immediate: bool, cx: &mut Context<Root>) -> bool {
    let query = r.place.read(cx).value().trim().to_string();
    let is_name = is_game_name(&query);
    if is_name {
        r.picker.open = true;
        r.picker.inline = true;
        search(r, query, immediate, cx);
    } else if r.picker.inline {
        close(r);
        cx.notify();
    }
    is_name
}

fn search(r: &mut Root, query: String, immediate: bool, cx: &mut Context<Root>) {
    r.picker.seq += 1;
    if query.is_empty() {
        r.picker.found = None;
        cx.notify();
        return;
    }
    let mine = r.picker.seq;
    r.picker.found = Some((query.clone(), SearchResults::Loading));
    cx.notify();
    cx.spawn(async move |this, cx| {
        if !immediate {
            cx.background_executor().timer(TYPE_DELAY).await;
        }
        let Ok(Some(link)) = this.update(cx, |r, _| {
            (r.picker.open && r.picker.seq == mine).then(|| r.link.clone()).flatten()
        }) else { return };
        let result = link.call("searchGames", json!({ "query": query })).await;
        let games = match result {
            Ok(Ok(rows)) => SearchResults::Games(rows.as_array().cloned().unwrap_or_default()),
            _ => SearchResults::Failed,
        };
        let _ = this.update(cx, |r, cx| {
            if r.picker.open && r.picker.seq == mine {
                r.picker.found = Some((query, games));
                cx.notify();
            }
        });
    })
    .detach();
}

/// Save server's button: there once the box holds a game Roblox knows.
pub fn save_button(root: &Root, cx: &mut Context<Root>) -> Option<IconButton> {
    root.game.as_ref().filter(|g| g.get("ok").and_then(Value::as_bool) == Some(true))?;
    Some(IconButton::new("bar-save-server", "bookmark").size(34.).tip(root.tr("sv.saveTip")).on_click(cx.listener(|r, _, window, cx| open_save(r, window, cx))))
}

/// A hidden shot's Save server (BLOXKEEP_SHOT_ASK=saveserver): the box given Blox Fruits, then the box once the game is
/// known (asked for a few seconds).
#[cfg(feature = "shot")]
pub fn shot_save(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    if r.place.read(cx).value().trim().is_empty() {
        r.place.update(cx, |st, cx| st.set_value("2753915549", window, cx));
    }
    r.resolve_place(cx);
    cx.spawn_in(window, async move |this, cx| {
        for _ in 0..30 {
            cx.background_executor().timer(Duration::from_millis(400)).await;
            let done = this.update_in(cx, |r, window, cx| {
                let known = r.game.as_ref().is_some_and(|g| g.get("ok").and_then(Value::as_bool) == Some(true));
                if known {
                    open_save(r, window, cx);
                }
                known
            });
            if !matches!(done, Ok(false)) {
                break;
            }
        }
    })
    .detach();
}

fn open_save(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    let Some(g) = r.game.clone().filter(|g| g.get("ok").and_then(Value::as_bool) == Some(true)) else { return };
    // A private server's link (or a share link) is kept as it is; a plain game is kept with the code it asks for.
    let has_link = matches!(s(&g, "kind"), "private" | "share");
    let place = int(&g, "placeId");
    let game = if !s(&g, "name").is_empty() { s(&g, "name").to_string() } else if place > 0 { r.i18n.tv("dock.placeN", &[("id", &place.to_string())]) } else { String::new() };
    r.picker.saving = Some((has_link, place, int(&g, "universeId"), s(&g, "name").to_string()));
    r.picker.sv_error = None;
    r.picker.sv_busy = false;
    r.picker.open = false;
    r.picker.sv_seq += 1;
    r.picker.sv_open = true;
    let seq = r.picker.sv_seq;
    let gone = move |this: &WeakEntity<Root>, cx: &mut App| {
        let _ = this.update(cx, |r, _| {
            if r.picker.sv_seq == seq {
                r.picker.sv_open = false;
            }
        });
    };
    let (name, hint) = (game.clone(), r.tr("sv.codeHint"));
    r.sv_name.update(cx, |st, cx| st.set_value(name, window, cx));
    r.sv_code.update(cx, |st, cx| {
        st.set_value("", window, cx);
        st.set_placeholder(hint, window, cx);
    });
    let this = cx.weak_entity();
    window.open_dialog(cx, move |dialog, _, cx| {
        let Some(root) = this.upgrade() else { return dialog };
        let r = root.read(cx);
        let label = |key: &str| div().text_size(px(T_S)).font_weight(FontWeight::MEDIUM).text_color(c(TEXT_2)).child(r.tr(key));
        let mut body = div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(div().text_size(px(T_M)).line_height(px(20.)).text_color(c(TEXT_2)).child(r.i18n.tv(if has_link { "sv.withLink" } else { "sv.withCode" }, &[("game", &game)])))
            .child(div().flex().flex_col().gap(px(6.)).child(label("sv.name")).child(field(&r.sv_name, None)))
            .child(div().flex().flex_col().gap(px(6.)).child(label(if has_link { "sv.codeOptional" } else { "sv.code" })).child(field(&r.sv_code, None)));
        if let Some(why) = r.picker.sv_error.clone() {
            body = body.child(form_error(why));
        }
        let go = this.clone();
        let (shut, cancel, closed) = (this.clone(), this.clone(), this.clone());
        framed(dialog, r.tr("sv.title"), r.tr("common.close"), move |_, w, cx| {
            gone(&shut, cx);
            w.close_dialog(cx);
        })
            .w(px(480.))
            .centered()
            .entrance(DialogEntrance::FadeSlide)
            // (Esc or a click beside it.)
            .on_close(move |_, _, cx| gone(&closed, cx))
            .child(dialog_body(body))
            .footer(
                dialog_foot()
                    .child(Button::new("sv-cancel", r.tr("common.cancel")).on_click(move |_, w, cx| {
                        gone(&cancel, cx);
                        w.close_dialog(cx);
                    }))
                    .child(Button::new("sv-save", r.tr("sv.saveBtn")).primary().on_click(move |_, w, cx| {
                        let _ = go.update(cx, |r, cx| save_server(r, w, cx));
                    })),
            )
    });
}

/// Save: the app keeps it, or says why not (not a private server's link, too many saved), under the boxes.
pub fn save_server(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    let Some((has_link, place, universe, game)) = r.picker.saving.clone() else { return };
    if r.picker.sv_busy {
        return;
    }
    let code = r.sv_code.read(cx).value().trim().to_string();
    if !has_link && code.is_empty() {
        r.picker.sv_error = Some(r.tr("sv.needCode"));
        window.refresh();
        return;
    }
    let Some(link) = r.link.clone() else { return };
    let name = r.sv_name.read(cx).value().trim().to_string();
    let at = if has_link { r.place.read(cx).value().trim().to_string() } else { String::new() };
    r.picker.sv_busy = true;
    r.picker.sv_error = None;
    let rx = link.call("saveServer", json!({ "name": name, "code": code, "link": at, "placeId": place, "universeId": universe, "gameName": game }));
    let seq = r.picker.sv_seq;
    cx.spawn_in(window, async move |this, cx| {
        let answer = rx.await;
        let _ = this.update_in(cx, |r, window, cx| {
            // Its box closed meanwhile (or another opened): the newest dialog may be some other one.
            let mine = r.picker.sv_seq == seq && r.picker.sv_open;
            if r.picker.sv_seq == seq {
                r.picker.sv_busy = false;
            }
            match answer {
                Ok(Ok(v)) => {
                    if mine {
                        r.picker.saving = None;
                        r.picker.sv_open = false;
                        window.close_dialog(cx);
                    }
                    r.toast = Some((r.i18n.tv("sv.saved", &[("name", s(&v, "name"))]), "ok".into()));
                    // (The Saved tab is the one shown when the list opens next.)
                    r.picker.tab = 2;
                }
                Ok(Err(e)) if mine => r.picker.sv_error = Some(e),
                Ok(Err(_)) => {}
                Err(_) => {}
            }
            window.refresh();
            cx.notify();
        });
    })
    .detach();
}

/// 412873 as "412,873" ("412.873" in Vietnamese).
fn grouped(n: i64, vi: bool) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(if vi { '.' } else { ',' });
        }
        out.push(ch);
    }
    out
}

/// The lists as they are now (asked again each time the list opens, and after a pin or a removal).
fn load(r: &mut Root, cx: &mut Context<Root>) {
    let Some(link) = r.link.clone() else { return };
    if r.picker.loading {
        return;
    }
    r.picker.loading = true;
    let rx = link.call("gameLists", json!({}));
    answer(rx, cx, move |r, answer, cx| {
        r.picker.loading = false;
        if let Ok(Ok(v)) = answer {
            if v.is_object() {
                r.picker.lists = Some(v);
            }
        }
        cx.notify();
    });
}

/// The list open, its lists asked for (a shot's way in).
pub fn open_now(r: &mut Root, cx: &mut Context<Root>) {
    r.picker.open = true;
    r.picker.inline = false;
    load(r, cx);
    search_menu(r, false, cx);
}

pub fn close(r: &mut Root) {
    if r.picker.open {
        r.picker.open = false;
        r.picker.closed = Some(Instant::now());
    }
    r.picker.inline = false;
    r.picker.seq += 1;
    r.picker.found = None;
}

/// A game, or a saved server, into the box: its picture and name are looked up as for a link typed there.
fn pick(r: &mut Root, link: String, window: &mut Window, cx: &mut Context<Root>) {
    close(r);
    // (Another game, or a saved server: a Job ID left in its box went with the game before, app.js's pickGame/pickSaved.)
    crate::pages::accounts::clear_job(window, cx);
    r.place.update(cx, |st, cx| st.set_value(link, window, cx));
    r.resolve_place(cx);
    cx.notify();
}

fn tell(r: &mut Root, cmd: &'static str, args: Value, cx: &mut Context<Root>, done: impl FnOnce(&mut Root, Value, &mut Context<Root>) + 'static) {
    let Some(link) = r.link.clone() else { return };
    let rx = link.call(cmd, args);
    answer(rx, cx, move |r, answer, cx| {
        match answer {
            Ok(Ok(v)) => done(r, v, cx),
            Ok(Err(e)) => r.toast = Some((e.to_string(), "error".into())),
            Err(_) => {}
        }
        cx.notify();
    });
}

/// The button beside the game box, with its list.
pub fn button(root: &Root, cx: &mut Context<Root>) -> Div {
    let open = root.picker.open;
    div()
        .relative()
        .flex_none()
        .child(IconButton::new("bar-browse", if open { "chevronDown" } else { "chevronUp" }).size(34.).selected(open).tip(root.tr("dock.browseTip")).on_click(cx.listener(|r, _, _, cx| {
            let just = r.picker.closed.take().is_some_and(|at| at.elapsed() < Duration::from_millis(300));
            if r.picker.open {
                close(r);
            } else if !just {
                open_now(r, cx);
            }
            cx.notify();
        })))
        // (The list built while open or fading out only, not every frame after its first opening.)
        .when(crate::pages::accounts::pop_alive("picker-pop", open), |d| {
            d.child(popup(PopAt::AboveLeft(42.), "picker-pop", open, list(root, cx)))
        })
}

fn list(root: &Root, cx: &mut Context<Root>) -> Stateful<Div> {
    let p = &root.picker;
    let vi = root.i18n.vietnamese;
    let rows_of = |key: &str| p.lists.as_ref().and_then(|l| l.get(key)).and_then(Value::as_array).cloned().unwrap_or_default();
    let saved: Vec<Value> = root.state.pointer("/settings/savedServers").and_then(Value::as_array).cloned().unwrap_or_default();
    // (Popular is left out when Roblox gave none.)
    let tabs: Vec<usize> = (0..TABS.len()).filter(|i| TABS[*i] != "popular" || !rows_of("popular").is_empty()).collect();
    let tab = if tabs.contains(&p.tab) { p.tab } else { 0 };
    // (A search's games take the lists' place until its box is emptied.)
    let found = p.found.as_ref();
    let key = if found.is_some() { "search" } else { TABS[tab] };

    // (One line always: "Popular" wrapped alone onto a second one. Longer words, as in Vietnamese, scroll sideways.)
    let mut head = div().id("picker-tabs").flex().flex_nowrap().overflow_x_scroll().gap(px(2.)).p(px(3.)).rounded(px(R_M)).bg(c(INSET));
    for i in tabs {
        head = head.child(
            chip(SharedString::from(format!("picker-tab-{}", TABS[i])), i == tab)
                .flex_none()
                .h(px(26.))
                .px(px(9.))
                .text_size(px(T_M))
                .cursor_pointer()
                .child(root.tr(&format!("picker.{}", TABS[i])))
                .on_click(cx.listener(move |r, _, _, cx| {
                    r.picker.tab = i;
                    cx.notify();
                })),
        );
    }
    let holder = |icon_name: &'static str, pic: Option<std::sync::Arc<Image>>| {
        div().size(px(34.)).flex_none().rounded(px(R_S)).overflow_hidden().bg(ca(WHITE, 0.06)).flex().items_center().justify_center().map(|d| match pic {
            Some(i) => d.child(img(i).size_full().object_fit(ObjectFit::Cover)),
            None => d.child(icon(icon_name, 16., c(MUTED))),
        })
    };
    let words = |name: String, under: String| {
        div()
            .min_w_0()
            .flex_1()
            .flex()
            .flex_col()
            .gap(px(1.))
            .child(div().truncate().text_size(px(T_M)).font_weight(FontWeight::MEDIUM).text_color(c(TEXT)).child(name))
            .child(div().truncate().text_size(px(T_S)).text_color(c(MUTED)).child(under))
    };
    let mut body = div().id("picker-list").max_h(px(360.)).overflow_y_scroll().flex().flex_col();
    if key == "saved" {
        if saved.is_empty() {
            body = body.child(div().p(px(14.)).text_size(px(T_M)).line_height(px(19.)).text_color(c(MUTED)).child(root.tr("picker.empty.saved")));
        }
        for (i, sv) in saved.iter().enumerate() {
            let (id, name, link, code) = (s(sv, "id").to_string(), s(sv, "name").to_string(), s(sv, "link").to_string(), s(sv, "code").to_string());
            let place = int(sv, "placeId");
            let under = [
                if !s(sv, "gameName").is_empty() { s(sv, "gameName").to_string() } else if place > 0 { root.i18n.tv("dock.placeN", &[("id", &place.to_string())]) } else { String::new() },
                if link.is_empty() { String::new() } else { root.tr("sv.kindLink") },
                if code.is_empty() { String::new() } else { root.i18n.tv("sv.kindCode", &[("code", &code)]) },
            ]
            .into_iter()
            .filter(|x| !x.is_empty())
            .collect::<Vec<_>>()
            .join(" · ");
            let pic = Some(s(sv, "iconUrl")).filter(|u| !u.is_empty()).and_then(|u| images::pic(u, cx));
            let (pick_id, pick_code, copy_id, del_id, del_name) = (id.clone(), code.clone(), id.clone(), id.clone(), name.clone());
            body = body.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(2.))
                    // A saved server into the box: its private server's link, or its game. A code the game asks for goes
                    // on the clipboard, to paste in the game.
                    .child(menu_row(("picker-saved", i)).flex_1().min_w_0().h(px(46.)).gap(px(10.)).child(holder("bookmark", pic)).child(words(name, under)).on_click(cx.listener(move |r, _, window, cx| {
                        let to = if link.is_empty() { format!("https://www.roblox.com/games/{place}") } else { link.clone() };
                        pick(r, to, window, cx);
                        if !pick_code.is_empty() {
                            let code = pick_code.clone();
                            tell(r, "copyServerCode", json!({ "id": pick_id }), cx, move |r, v, _| {
                                if v.as_bool() != Some(false) && !v.is_null() {
                                    r.toast = Some((r.i18n.tv("sv.codeCopied", &[("code", &code)]), "ok".into()));
                                }
                            });
                        }
                    })))
                    .when(!code.is_empty(), |d| {
                        d.child(IconButton::new(("picker-copy", i), "copy").size(28.).tip(root.tr("sv.copyCode")).on_click(cx.listener(move |r, _, _, cx| {
                            tell(r, "copyServerCode", json!({ "id": copy_id }), cx, |r, v, _| {
                                if v.as_bool() != Some(false) && !v.is_null() {
                                    r.toast = Some((r.tr("sv.codeCopiedShort"), "ok".into()));
                                }
                            });
                        })))
                    })
                    .child(IconButton::new(("picker-del", i), "x").size(28.).tip(root.tr("sv.delete")).on_click(cx.listener(move |r, _, window, cx| {
                        let id = del_id.clone();
                        r.picker.open = false;
                        r.confirm(r.tr("sv.deleteQ"), del_name.clone(), r.tr("sv.delete"), true, None, window, cx, move |r, _| r.send("deleteServer", json!({ "id": id })));
                        cx.notify();
                    }))),
            );
        }
    } else {
        let rows = match found {
            Some((_, SearchResults::Games(rows))) => rows.clone(),
            Some(_) => Vec::new(),
            None => rows_of(key),
        };
        if rows.is_empty() {
            let text = match found {
                Some((_, SearchResults::Loading)) => root.tr("av.reading"),
                Some((_, SearchResults::Failed)) => root.tr("picker.searchFailed"),
                Some(_) => root.tr("picker.noResults"),
                None if p.lists.is_none() => root.tr("av.reading"),
                None => root.tr(&format!("picker.empty.{key}")),
            };
            body = body.child(div().p(px(14.)).text_size(px(T_M)).line_height(px(19.)).text_color(c(MUTED)).child(text));
        }
        for (i, g) in rows.iter().enumerate() {
            let (place, universe, name) = (int(g, "placeId"), int(g, "universeId"), s(g, "name").to_string());
            let playing = int(g, "playing");
            let pinned = g.get("pinned").and_then(Value::as_bool).unwrap_or(key == "pinned");
            let under = if playing > 0 { root.i18n.tv("dock.playing", &[("n", &grouped(playing, vi))]) } else { root.i18n.tv("dock.placeN", &[("id", &place.to_string())]) };
            let pic = Some(s(g, "iconUrl")).filter(|u| !u.is_empty()).and_then(|u| images::pic(u, cx));
            let pin_name = name.clone();
            body = body.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(2.))
                    .child(menu_row(("picker-game", i)).flex_1().min_w_0().h(px(46.)).gap(px(10.)).child(holder("gamepad", pic)).child(words(crate::pages::home::clean(&name), under)).on_click(cx.listener(move |r, _, window, cx| {
                        pick(r, format!("https://www.roblox.com/games/{place}"), window, cx);
                    })))
                    .child(IconButton::new(("picker-pin", i), if pinned { "starFill" } else { "star" }).size(28.).tip(root.tr(if pinned { "picker.unpin" } else { "picker.pin" })).on_click(cx.listener(move |r, _, _, cx| {
                        tell(r, "pinGame", json!({ "placeId": place, "universeId": universe, "name": pin_name }), cx, |r, _, cx| load(r, cx));
                    })))
                    .when(key == "recent", |d| {
                        d.child(IconButton::new(("picker-forget", i), "x").size(28.).tip(root.tr("dock.recentRemove")).on_click(cx.listener(move |r, _, _, cx| {
                            tell(r, "removeRecent", json!({ "placeId": place }), cx, |r, _, cx| load(r, cx));
                        })))
                    }),
            );
        }
    }
    div()
        .id("picker-menu")
        .occlude()
        .w(px(440.))
        .p(px(6.))
        .rounded(px(R_L))
        .bg(c(RAISED_2))
        .border_1()
        .border_color(ca(WHITE, 0.1))
        .shadow_lg()
        .flex()
        .flex_col()
        .gap(px(6.))
        .on_mouse_down_out(cx.listener(|r, _, _, cx| {
            if r.picker.open {
                close(r);
                cx.notify();
            }
        }))
        .when(!p.inline, |d| d.child(field(&root.picker_q, Some("search")).h(px(32.))))
        .child(match found {
            Some((q, _)) => div().px(px(6.)).text_size(px(T_M)).text_color(c(MUTED)).child(root.i18n.tv("picker.results", &[("q", q)])).into_any_element(),
            None => head.into_any_element(),
        })
        .child(body)
}
