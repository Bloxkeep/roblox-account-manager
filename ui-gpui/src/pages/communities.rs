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
    roblox::{self, Group},
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

/// The search box, made the first time Communities is shown; typing in it filters the list as it goes.
pub fn settle(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    if r.communities.search.is_some() || r.page != Page::Games || r.games.view != games::View::Communities {
        return;
    }
    let e = cx.new(|cx| InputState::new(window, cx).placeholder(r.tr("club.search")));
    // (Typing tells only the box: the list is Root's, so Root is told too.)
    cx.subscribe(&e, |_: &mut Root, _, ev: &InputEvent, cx| {
        if matches!(ev, InputEvent::Change) {
            cx.notify();
        }
    })
    .detach();
    r.communities.search = Some(e);
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
            if shown.is_empty() {
                div().px(px(4.)).text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("club.found0")).into_any_element()
            } else {
                div().flex().flex_col().gap(px(2.)).children(shown.into_iter().map(|g| {
                    let on = open == Some(g.id);
                    let (id, name) = (g.id, g.name.clone());
                    let pic = (!g.pic.is_empty()).then(|| images::pic_sq(&g.pic, 32., cx)).flatten();
                    let facts = if g.role.is_empty() {
                        root.i18n.tv("pf.members", &[("n", &thousands(g.members))])
                    } else {
                        format!("{} · {}", g.role, root.i18n.tv("pf.members", &[("n", &thousands(g.members))]))
                    };
                    hot(("cm-row", g.id as usize))
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
                                r.communities.picked = Some((user, id, name.clone()));
                            }
                            if profile::club_shown(r, true) != Some(id) {
                                profile::open_club(r, id, name.clone(), true, cx);
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
                                        .child(div().min_w_0().text_size(px(T_M)).font_weight(FontWeight::MEDIUM).text_color(if on { c(TEXT) } else { c(TEXT_2) }).truncate().child(g.name.clone()))
                                        .when(g.verified, |d| d.child(icon("creatorVerified", 13., rgb(0x335fff).into()))),
                                )
                                .child(div().text_size(px(T_XS)).text_color(c(MUTED)).truncate().child(facts)),
                        )
                        .into_any_element()
                }))
                .into_any_element()
            }
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
