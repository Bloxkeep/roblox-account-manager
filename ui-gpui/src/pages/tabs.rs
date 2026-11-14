// Account tabs, as a browser has them, over Games (the owner's call, 2026-11-10: several accounts open at once, not one
// account switched for another). A tab is an account with everything of Roblox's read for it (friends, avatar,
// profile, inbox) and the part it was left on; picking a tab shows it at once, as it was. "+" opens another account.
//
// The pages go on reading `root.friends`, `root.avatar`… : those hold the tab shown. Leaving a tab puts them away in
// it; coming to one takes them out. An answer still on its way for a tab put away finds another account there and is
// dropped (each page checks whose answer it is), and the tab asks again when it is shown.
use gpui_kit::{prelude::*, *};
use serde_json::Value;

use crate::{
    kit::*,
    pages::{
        accounts::{avatar, label},
        avatar as avatar_page, friends,
        games::View,
        inbox, profile,
    },
    shell::Root,
    theme::*,
};

#[derive(Default)]
pub struct Tab {
    /// The account, by its id in the app (None: the first tab before the app has said which its pick, the main one, is).
    pub account: Option<String>,
    view: View,
    friends: friends::Data,
    avatar: avatar_page::Data,
    profile: profile::Kept,
    inbox: inbox::Data,
}

pub struct Data {
    pub list: Vec<Tab>,
    pub at: usize,
    /// "+"'s list of accounts is open.
    pub menu: bool,
}

impl Default for Data {
    fn default() -> Self {
        Data { list: vec![Tab::default()], at: 0, menu: false }
    }
}

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or_default()
}

/// The account of the tab shown: the one the app answered for, else the one it was opened for.
fn shown_account(r: &Root) -> Option<String> {
    friends::signed(r).map(|s| s.0).or(r.friends.account.clone())
}

/// What is shown goes into its tab.
fn put_away(r: &mut Root) {
    let account = shown_account(r);
    r.friends.rest();
    r.avatar.rest();
    r.inbox.rest();
    r.rset = Default::default();
    let at = r.tabs.at;
    let Some(t) = r.tabs.list.get_mut(at) else { return };
    t.account = account;
    t.view = r.games.view;
    t.friends = std::mem::take(&mut r.friends);
    t.avatar = std::mem::take(&mut r.avatar);
    t.profile = r.profile.take_profile();
    t.inbox = std::mem::take(&mut r.inbox);
}

/// The tab at `tabs.at` comes out to be shown.
fn take_out(r: &mut Root) {
    let at = r.tabs.at;
    let Some(t) = r.tabs.list.get_mut(at) else { return };
    r.games.view = t.view;
    r.friends = std::mem::take(&mut t.friends);
    r.avatar = std::mem::take(&mut t.avatar);
    let kept = std::mem::take(&mut t.profile);
    r.inbox = std::mem::take(&mut t.inbox);
    r.profile.put_profile(kept);
}

pub fn switch(r: &mut Root, to: usize) {
    if to != r.tabs.at && to < r.tabs.list.len() {
        put_away(r);
        r.tabs.at = to;
        take_out(r);
    }
}

/// A tab for that account: the one it has already, else a new one at the end, on the part being looked at now.
pub fn open(r: &mut Root, id: String) {
    r.tabs.menu = false;
    if shown_account(r).as_deref() == Some(id.as_str()) {
        return;
    }
    if let Some(i) = r.tabs.list.iter().position(|t| t.account.as_deref() == Some(id.as_str())) {
        return switch(r, i);
    }
    let view = r.games.view;
    put_away(r);
    r.tabs.list.push(Tab { account: Some(id.clone()), view, friends: friends::Data::of(id), ..Default::default() });
    r.tabs.at = r.tabs.list.len() - 1;
    take_out(r);
}

/// Another vault opened, or accounts deleted: the tabs of accounts no longer there go (with what was read for them);
/// the tab shown stays, on the app's pick again if its account went.
pub fn forget_gone(r: &mut Root) {
    let ids: std::collections::HashSet<String> = r.accounts().iter().map(|a| s(a, "id").to_string()).collect();
    let gone = |t: &Tab| t.account.as_ref().is_some_and(|id| !ids.contains(id));
    let at = r.tabs.at;
    let shown_gone = r.friends.account.as_ref().is_some_and(|id| !ids.contains(id));
    if shown_gone {
        r.friends.account = None;
    }
    if let Some(t) = r.tabs.list.get_mut(at) {
        if shown_gone || gone(t) {
            t.account = None;
        }
    }
    // (The shown one is kept: its place is how many kept tabs come before it.)
    let (mut i, mut before) = (0, 0);
    r.tabs.list.retain(|t| {
        let keep = i == at || !gone(t);
        if keep && i < at {
            before += 1;
        }
        i += 1;
        keep
    });
    r.tabs.at = before.min(r.tabs.list.len().saturating_sub(1));
}

/// A tab closes; the last one stays. Closing the one shown shows its neighbour.
pub fn close(r: &mut Root, i: usize) {
    if r.tabs.list.len() < 2 || i >= r.tabs.list.len() {
        return;
    }
    if i == r.tabs.at {
        let to = if i + 1 < r.tabs.list.len() { i + 1 } else { i - 1 };
        switch(r, to);
    }
    r.tabs.list.remove(i);
    if r.tabs.at > i {
        r.tabs.at -= 1;
    }
}

/// The strip: a tab an account (its face and name; a cross while there is more than one), then "+".
pub fn strip(root: &Root, cx: &mut Context<Root>) -> impl IntoElement {
    let able = friends::able(root);
    let (_, active) = friends::looking(root);
    let tabs = &root.tabs;
    let find = |id: &Option<String>| id.as_ref().and_then(|id| able.iter().find(|a| s(a, "id") == id)).cloned();
    let many = tabs.list.len() > 1;
    let mut open_ids: Vec<String> = Vec::new();
    let mut row = div().flex_none().flex().items_center().gap(px(4.)).min_w_0();
    for (i, t) in tabs.list.iter().enumerate() {
        let on = i == tabs.at;
        let acc = if on { active.clone() } else { find(&t.account) };
        if let Some(a) = &acc {
            open_ids.push(s(a, "id").to_string());
        }
        // No account to see Roblox as (none signed in, or it isn't known yet): Roblox itself, signed out, with a globe
        // in the face's place, rather than a bare "…".
        let name = acc.as_ref().map(label).unwrap_or_else(|| "Roblox".into());
        row = row.child(
            hot(("rbx-tab", i))
                .h(px(34.))
                .pl(px(8.))
                .pr(px(if many { 5. } else { 12. }))
                .flex()
                // (One tab keeps its whole name: shrunk, the owner's showed "N…", 2026-11-12.)
                .map(|d| if many { d.flex_shrink(1.).min_w(px(64.)) } else { d.flex_none() })
                .items_center()
                .gap(px(8.))
                .rounded(px(R_M))
                .text_size(px(T_M))
                .font_weight(FontWeight::MEDIUM)
                .cursor_pointer()
                .map(|d| if on { d.tones(c(PANEL), c(PANEL)).text_color(c(TEXT)) } else { d.tones(ca(WHITE, 0.), ca(WHITE, 0.06)).text_color(c(TEXT_2)).hover(|d| d.text_color(c(TEXT))) })
                .map(|d| match acc.as_ref() {
                    Some(a) => d.child(avatar(a, 22., if on { PANEL } else { GROUND }, cx)),
                    None => d.child(div().size(px(22.)).flex_none().rounded_full().bg(ca(WHITE, 0.06)).flex().items_center().justify_center().child(icon("globe", 13., c(MUTED)))),
                })
                .child(div().max_w(px(150.)).min_w_0().truncate().child(name))
                .when(many, |d| {
                    d.child(IconButton::new(("rbx-tab-x", i), "x").size(24.).tip(root.tr("fr.close")).on_click(cx.listener(move |r, _, _, cx| {
                        // The cross only: the tab under it isn't picked by the same press.
                        cx.stop_propagation();
                        close(r, i);
                        cx.notify();
                    })))
                })
                .on_click(cx.listener(move |r, _, _, cx| {
                    switch(r, i);
                    cx.notify();
                })),
        );
    }

    // "+": the accounts not open yet, under it (a second click closes it).
    let rest: Vec<Value> = able.iter().filter(|a| !open_ids.iter().any(|id| id == s(a, "id"))).cloned().collect();
    let mut plus = div().relative().flex_none().child(IconButton::new("rbx-tab-add", "plus").size(30.).disabled(rest.is_empty()).tip(root.tr("tabs.open")).on_click(cx.listener(|r, _, _, cx| {
        r.tabs.menu = !r.tabs.menu;
        cx.notify();
    })));
    // (Built while open or fading out only, not every frame: an avatar a row.)
    if !rest.is_empty() && crate::pages::accounts::pop_alive("rbx-tab-pop", tabs.menu) {
        let mut menu = div()
            .id("rbx-tab-menu")
            .occlude()
            .w(px(260.))
            .max_h(px(360.))
            .overflow_y_scroll()
            .p(px(4.))
            .rounded(px(R_M))
            .bg(c(RAISED))
            .border_1()
            .border_color(c(LINE))
            .shadow_lg()
            .flex()
            .flex_col()
            .on_mouse_down_out(cx.listener(|r, _, _, cx| {
                r.tabs.menu = false;
                cx.notify();
            }));
        for (i, acc) in rest.iter().enumerate() {
            let id = s(acc, "id").to_string();
            menu = menu.child(
                menu_row(("rbx-tab-acc", i))
                    .h(px(36.))
                    .px(px(8.))
                    .child(avatar(acc, 22., RAISED, cx))
                    .child(div().min_w_0().flex_1().truncate().child(label(acc)))
                    .on_click(cx.listener(move |r, _, _, cx| {
                        open(r, id.clone());
                        cx.notify();
                    })),
            );
        }
        plus = plus.child(div().absolute().top(px(36.)).left_0().child(deferred(anchored().snap_to_window_with_margin(px(8.)).child(pop("rbx-tab-pop", tabs.menu, menu))).with_priority(1)));
    }
    row.child(plus)
}

/// BLOXKEEP_SHOT_TABS=2: a hidden shot opens that many more accounts' tabs, then shows the first again.
#[cfg(feature = "shot")]
pub fn shot_open(r: &mut Root, more: usize) {
    if r.tabs.list.len() > 1 {
        return;
    }
    let ids: Vec<String> = friends::able(r).iter().skip(1).take(more).map(|a| s(a, "id").to_string()).collect();
    for id in ids {
        open(r, id);
    }
    switch(r, 0);
}
