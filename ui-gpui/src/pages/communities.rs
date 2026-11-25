// Communities, inside Games (its header's switch): the communities of the account whose tab is shown, a list at the
// left with a search box and Create community (on roblox.com), and the one picked at the right as Roblox's community
// page has it (profile.rs's community page: About, Store, Experiences, Affiliates).
use std::collections::HashMap;

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{prelude::*, *};
use serde_json::json;

use crate::{
    ask::reply,
    images,
    kit::*,
    pages::{friends, games, profile, Page},
    roblox::{self, ClubHit, Group},
    shell::Root,
    theme::*,
};

#[derive(Default)]
pub struct Data {
    /// Each account's communities (None while they are asked), by its Roblox user.
    got: HashMap<i64, Option<Result<Vec<Group>, String>>>,
    /// The one picked last (opened again on coming back: a member's profile opened from it closes it), and the
    /// account it was picked for.
    picked: Option<(i64, i64, String)>,
    /// The list's search box (made once Communities is shown: a box needs the window).
    search: Option<Entity<InputState>>,
    /// What Roblox finds for the box's words across all of Roblox (None while asked), under the account's own that
    /// match.
    found: Option<(String, Option<Result<Vec<ClubHit>, String>>)>,
}

/// What Communities asks while shown: the sign-in, the account's communities, the one picked (the first, until one is)
/// and what its page shows.
pub fn want(r: &mut Root, link: &crate::link::Link, cx: &mut Context<Root>) {
    friends::want_sign(r, link, cx);
    let Some((_, user, _)) = friends::signed(r) else { return };
    if !r.communities.got.contains_key(&user) {
        r.communities.got.insert(user, None);
        let rx = roblox::run(move || roblox::clubs_of(user));
        reply(rx, cx, move |r, got, cx| {
            r.communities.got.insert(user, Some(got));
            cx.notify();
        });
        return;
    }
    let Some(Some(Ok(list))) = r.communities.got.get(&user) else { return };
    let pick = match &r.communities.picked {
        Some((u, id, name)) if *u == user => Some((*id, name.clone())),
        _ => list.first().map(|g| (g.id, g.name.clone())),
    };
    if let Some((id, name)) = pick {
        if profile::club_shown(r, true) != Some(id) {
            r.communities.picked = Some((user, id, name.clone()));
            profile::open_club(r, id, name, true, cx);
        }
    }
    profile::club_asks(r, cx);
}

/// The search box, made the first time Communities is shown; typing in it filters the list as it goes, and Roblox's
/// community search is asked a moment after the last key.
pub fn settle(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    if r.communities.search.is_some() || r.page != Page::Games || r.games.view != games::View::Communities {
        return;
    }
    let e = cx.new(|cx| InputState::new(window, cx).placeholder(r.tr("club.search")));
    // (Typing tells only the box: the list is Root's, so Root is told too.)
    cx.subscribe(&e, |r: &mut Root, e, ev: &InputEvent, cx| {
        if matches!(ev, InputEvent::Change) {
            let words = e.read(cx).value().trim().to_string();
            if words.is_empty() {
                r.communities.found = None;
            } else {
                cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(std::time::Duration::from_millis(400)).await;
                    let _ = this.update(cx, |r, cx| search(r, words, cx));
                })
                .detach();
            }
            cx.notify();
        }
    })
    .detach();
    r.communities.search = Some(e);
}

/// Roblox's community search for `words`, when the box still holds them and they weren't asked already.
fn search(r: &mut Root, words: String, cx: &mut Context<Root>) {
    let now = r.communities.search.as_ref().map(|e| e.read(cx).value().trim().to_string()).unwrap_or_default();
    if now != words || r.communities.found.as_ref().is_some_and(|(at, _)| *at == words) {
        return;
    }
    r.communities.found = Some((words.clone(), None));
    cx.notify();
    let ask = words.clone();
    let rx = roblox::run(move || roblox::club_search(&ask));
    reply(rx, cx, move |r, got, cx| {
        if r.communities.found.as_ref().is_some_and(|(at, _)| *at == words) {
            r.communities.found = Some((words, Some(got)));
            cx.notify();
        }
    });
}

/// One row of the list: the community's picture, its name and what is said under it; a press opens it at the right.
fn row(root: &Root, key: &'static str, (id, name, pic, verified): (i64, &str, &str, bool), facts: String, cx: &mut Context<Root>) -> AnyElement {
    let on = profile::club_shown(root, true) == Some(id);
    let pic = (!pic.is_empty()).then(|| images::pic_sq(pic, 32., cx)).flatten();
    let name = name.to_string();
    let picked = name.clone();
    hot((key, id as usize))
        .h(px(48.))
        .px(px(8.))
        .flex()
        .flex_none()
        .items_center()
        .gap(px(10.))
        .rounded(px(R_M))
        .cursor_pointer()
        .map(|d| if on { d.tones(ca(WHITE, 0.08), ca(WHITE, 0.08)) } else { d.tones(ca(WHITE, 0.), ca(WHITE, 0.05)) })
        .on_click(cx.listener(move |r, _, _, cx| {
            if let Some((_, user, _)) = friends::signed(r) {
                r.communities.picked = Some((user, id, picked.clone()));
            }
            if profile::club_shown(r, true) != Some(id) {
                profile::open_club(r, id, picked.clone(), true, cx);
            }
            cx.notify();
        }))
        .child(div().size(px(32.)).flex_none().rounded(px(R_S)).bg(c(RAISED_2)).overflow_hidden().map(|d| match pic {
            Some(f) => d.child(img(f).size_full().object_fit(ObjectFit::Cover)),
            None => d.flex().items_center().justify_center().child(icon("users", 16., c(MUTED))),
        }))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(5.))
                        .min_w_0()
                        .child(div().min_w_0().text_size(px(T_M)).font_weight(FontWeight::MEDIUM).text_color(if on { c(TEXT) } else { c(TEXT_2) }).truncate().child(name))
                        .when(verified, |d| d.child(icon("creatorVerified", 13., rgb(0x335fff).into()))),
                )
                .child(div().text_size(px(T_XS)).text_color(c(MUTED)).truncate().child(facts)),
        )
        .into_any_element()
}

pub fn page(root: &Root, width: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let top = div().flex_none().px(px(PAGE_X)).pt(px(22.)).pb(px(16.)).flex().items_center().gap(px(10.)).child(games::brand(root)).relative().child(games::island(root, cx)).child(div().flex_1());
    let shell = div().id("communities").relative().size_full().rounded(px(R_PANEL)).bg(c(PANEL)).overflow_hidden().flex().flex_col().child(top);
    let Some((_, user, _)) = friends::signed(root) else {
        return if friends::nobody(root) {
            shell.child(friends::need_account(root, "cm-add-acc", "club.noAccount", "users", cx))
        } else {
            shell.child(empty_note(root.tr("av.reading")))
        };
    };
    // The list takes less of a narrow window; the page beside it the rest.
    let list_w = if width < 900. { 236. } else { 288. };
    let words = root.communities.search.as_ref().map(|e| input_text(e, cx).to_lowercase()).unwrap_or_default();
    let open = profile::club_shown(root, true);
    let got = root.communities.got.get(&user).and_then(|g| g.as_ref());
    let rows: AnyElement = match got {
        None => div().h(px(120.)).flex().child(empty_note(root.tr("av.reading"))).into_any_element(),
        Some(Err(why)) => div().px(px(4.)).text_size(px(T_M)).text_color(c(MUTED)).child(root.i18n.tv("club.listFailed", &[("why", crate::net::why_words(&root.i18n, why).as_str())])).into_any_element(),
        Some(Ok(list)) if list.is_empty() => div().px(px(4.)).text_size(px(T_M)).text_color(c(MUTED)).line_height(px(20.)).child(root.tr("club.none")).into_any_element(),
        Some(Ok(list)) => {
            let shown: Vec<&Group> = list.iter().filter(|g| words.is_empty() || g.name.to_lowercase().contains(&words)).collect();
            let members = |n: i64| root.i18n.tv("pf.members", &[("n", &thousands(n))]);
            let mut col = div().flex().flex_col().gap(px(2.));
            for g in &shown {
                let facts = if g.role.is_empty() { members(g.members) } else { format!("{} · {}", g.role, members(g.members)) };
                col = col.child(row(root, "cm-row", (g.id, &g.name, &g.pic, g.verified), facts, cx));
            }
            // Searching: the rest of Roblox under them (the account's own not again).
            let found = root.communities.found.as_ref().filter(|(at, _)| !words.is_empty() && at.to_lowercase() == words);
            let asking = !words.is_empty() && found.is_none_or(|(_, g)| g.is_none());
            let hits: Vec<&ClubHit> = found.and_then(|(_, g)| g.as_ref()).and_then(|g| g.as_ref().ok()).map(|l| l.iter().filter(|h| !list.iter().any(|g| g.id == h.id)).collect()).unwrap_or_default();
            if !hits.is_empty() || asking {
                col = col.child(div().px(px(4.)).pt(px(if shown.is_empty() { 0. } else { 10. })).pb(px(4.)).text_size(px(T_XS)).font_weight(FontWeight::SEMIBOLD).text_color(c(MUTED)).child(root.tr("gp.swRoblox")));
            }
            if hits.is_empty() && asking {
                col = col.child(div().px(px(4.)).text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("av.reading")));
            }
            let none = shown.is_empty() && hits.is_empty() && !asking;
            for h in hits {
                col = col.child(row(root, "cm-hit", (h.id, &h.name, &h.pic, h.verified), members(h.members), cx));
            }
            if none {
                col = col.child(div().px(px(4.)).text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("club.found0")));
            }
            col.into_any_element()
        }
    };
    let count = got.and_then(|g| g.as_ref().ok()).map(Vec::len);
    // The list in a block of its own as Roblox's page has it: its title and count, the search, the communities, and
    // Create community at its foot.
    let side = div().w(px(list_w)).flex_none().min_h_0().flex().flex_col().pl(px(PAGE_X)).pb(px(18.)).child(
        surface()
            .flex_1()
            .min_h_0()
            .p(px(10.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(div().px(px(4.)).flex().items_center().gap(px(8.)).child(div().text_size(px(T_SEC)).font_weight(FontWeight::SEMIBOLD).child(root.tr("nav.communities"))).when_some(count, |d, n| d.child(badge(n.to_string(), Tone::Muted))))
            .when_some(root.communities.search.as_ref(), |d, e| d.child(field(e, Some("search")).h(px(34.))))
            .child(div().flex_1().min_h_0().child(crate::scrolled::body("cm-list", div().pb(px(4.)).child(rows))))
            .child(div().flex_none().flex().child(Button::new("cm-create", root.tr("club.create")).primary().small().wide().on_click(cx.listener(|r, _, _, _| r.send("openRobloxPage", json!({ "kind": "groupCreate" })))))),
    );
    let main: AnyElement = match open {
        Some(_) => profile::listed_club(root, width - list_w, cx),
        None => div().flex_1().flex().child(empty_note(root.tr(if count == Some(0) { "club.none" } else { "av.reading" }))).into_any_element(),
    };
    shell.child(div().flex_1().min_h_0().flex().child(side).child(div().flex_1().min_w_0().min_h_0().flex().flex_col().child(main)))
}
