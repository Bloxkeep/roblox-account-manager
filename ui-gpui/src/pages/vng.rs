use gpui_kit::{prelude::*, *};
use serde_json::{Value, json};

use super::games::{self, View};
use crate::{i18n::plain, images, kit::*, link::Link, shell::Root, theme::*};

#[derive(Default)]
pub struct Data {
    request: Option<(String, &'static str)>,
    revision: u64,
    result: Option<Result<Value, String>>,
    notices: bool,
    expanded: Option<usize>,
}

impl Data {
    pub fn clear(&mut self) {
        *self = Self { revision: self.revision.wrapping_add(1), ..Default::default() };
    }
}

pub fn title(view: View) -> Option<&'static str> {
    match view {
        View::Friends => Some("nav.friends"),
        View::Avatar => Some("nav.avatar"),
        View::Profile => Some("nav.profile"),
        View::Inbox => Some("nav.inbox"),
        View::Settings => Some("nav.settings"),
        _ => None,
    }
}

fn panel_key(root: &Root) -> Option<&'static str> {
    if games::shown_place(root).is_some() || !super::tabs::is_vng(root) {
        return None;
    }
    match root.games.view {
        View::Friends => Some("friends"),
        View::Avatar => Some("avatar"),
        View::Profile => Some("profile"),
        View::Inbox if root.tabs.vng.notices => Some("notifications"),
        View::Inbox => Some("messages"),
        View::Settings => Some("settings"),
        _ => None,
    }
}

pub fn want(root: &mut Root, link: &Link, cx: &mut Context<Root>) {
    let Some(key) = panel_key(root) else {
        if root.tabs.vng.request.is_some() {
            root.tabs.vng.clear();
        }
        return;
    };
    let Some(id) = root.friends.account.clone() else { return };
    let key = (id, key);
    let state = &mut root.tabs.vng;
    if state.request.as_ref() == Some(&key) {
        return;
    }
    state.revision = state.revision.wrapping_add(1);
    state.request = Some(key.clone());
    state.result = None;
    state.expanded = None;
    let revision = state.revision;
    let rx = link.call("vngPanel", json!({ "id": key.0, "panel": key.1 }));
    cx.spawn(async move |this, cx| {
        let result = rx.await.unwrap_or_else(|_| Err(String::new()));
        let _ = this.update(cx, |root, cx| {
            if !root.locked() && root.tabs.vng.revision == revision
                && root.friends.account.as_deref() == Some(key.0.as_str())
                && panel_key(root) == Some(key.1)
            {
                root.tabs.vng.result = Some(result);
                cx.notify();
            } else if root.tabs.vng.revision == revision {
                root.tabs.vng.clear();
            }
        });
    }).detach();
}

fn words<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or_default()
}

fn picture(data: &Value, size: f32, cx: &mut Context<Root>) -> Option<impl IntoElement> {
    let picture = images::pic_sq(words(data, "image"), size, cx)?;
    Some(img(picture).size(px(size)).flex_none().rounded(px(R_M)).bg(c(GROUND)))
}

fn line(title: String, detail: String) -> Div {
    div().flex().flex_col().gap(px(5.)).px(px(14.)).py(px(10.)).rounded(px(R_M)).bg(c(RAISED))
        .child(div().font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(title))
        .when(!detail.is_empty(), |d| d.child(div().text_color(c(TEXT_2)).child(detail)))
}

fn setting_row(title: String, value: impl IntoElement) -> Div {
    div().min_w_0().px(px(16.)).py(px(12.)).border_t_1().border_color(c(LINE))
        .flex().flex_wrap().items_center().justify_between().gap(px(8.))
        .child(div().text_color(c(TEXT_2)).child(title))
        .child(div().min_w_0().child(value))
}

fn settings(root: &Root, data: &Value, cx: &mut Context<Root>) -> Div {
    let account = root.friends.account.as_deref().and_then(|id| root.accounts().iter().find(|a| words(a, "id") == id));
    let mut profile = surface().flex().flex_col();
    if let Some(a) = account {
        profile = profile.child(div().p(px(16.)).child(group_head(
            super::accounts::avatar(a, 44., RAISED, cx), super::accounts::label(a),
            div().flex().items_center().gap(px(8.))
                .when(!words(a, "username").is_empty(), |d| d.child(div().truncate().text_color(c(TEXT_2)).child(format!("@{}", words(a, "username")))))
                .child(div().rounded(px(R_S)).bg(ca(ONLINE, 0.13)).px(px(8.)).py(px(3.))
                    .text_size(px(T_S)).text_color(c(ONLINE)).font_weight(FontWeight::SEMIBOLD).child("Roblox VNG")),
        )));
    }
    let value = |key| {
        let text = words(data, key);
        if text.is_empty() { root.tr("rs.unknown") } else { text.to_owned() }
    };
    let verified = |state| badge(root.tr(if state { "rs.verified" } else { "rs.unverified" }), if state { Tone::Ok } else { Tone::Muted });
    let birthday = (|| {
        let day = data.pointer("/birthday/day")?.as_u64()?;
        let month = data.pointer("/birthday/month")?.as_u64()?;
        let year = data.pointer("/birthday/year")?.as_u64()?;
        Some(if root.i18n.vietnamese { format!("{day:02}/{month:02}/{year}") } else { format!("{year}-{month:02}-{day:02}") })
    })();
    let profile = profile
        .child(setting_row(root.tr("rs.email"), div().flex().flex_wrap().gap(px(8.)).items_center()
            .child(div().min_w_0().child(value("email")))
            .children(data["emailVerified"].as_bool().filter(|_| !words(data, "email").is_empty()).map(verified))))
        .child(setting_row(root.tr("rs.phone"), data["phoneVerified"].as_bool().map(verified).map(IntoElement::into_any_element)
            .unwrap_or_else(|| div().child(root.tr("rs.unknown")).into_any_element())))
        .child(setting_row(root.tr("rs.country"), div().child(value("country"))))
        .when_some(birthday, |d, date| d.child(setting_row(root.tr("rs.birthday"), div().child(date))));
    div().flex().flex_col().gap(px(12.))
        .child(profile)
        .when(data["about"].is_string(), |d| d.child(line(root.tr("rs.about"), value("about"))))
        .child(div().text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("tabs.vngSettingsInfo")))
}

fn details(root: &Root, data: &Value, cx: &mut Context<Root>) -> Div {
    let mut body = div().flex().flex_col().gap(px(8.));
    match root.games.view {
        View::Profile => {
            let username = words(data, "username");
            let created: String = words(data, "created").chars().take(10).collect();
            return body.child(div().flex().items_center().gap(px(16.)).children(picture(data, 104., cx)).child(
                div().min_w_0().flex().flex_col().gap(px(6.))
                    .child(div().text_size(px(T_TITLE)).font_weight(FontWeight::SEMIBOLD).child(words(data, "title").to_owned()))
                    .when(!username.is_empty(), |d| d.child(div().text_color(c(MUTED)).child(format!("@{username}")))),
            ))
            .when(!created.is_empty(), |d| d.child(line(root.tr("tabs.vngJoined"), created)))
            .when(!words(data, "detail").is_empty(), |d| d.child(line(root.tr("rs.about"), words(data, "detail").to_owned())));
        }
        View::Settings => return settings(root, data, cx),
        View::Avatar => body = body.children(picture(data, 240., cx)),
        _ => {}
    }
    let rows = data["rows"].as_array().map(Vec::as_slice).unwrap_or_default();
    if rows.is_empty() {
        return body.child(div().py(px(12.)).text_color(c(MUTED)).child(root.tr("tabs.vngEmpty")));
    }
    for (index, row) in rows.iter().enumerate() {
        let title = plain(words(row, "title"));
        let detail = words(row, "detail").to_owned();
        if root.games.view == View::Inbox && !root.tabs.vng.notices {
            let expanded = root.tabs.vng.expanded == Some(index);
            let title = if title.is_empty() { root.tr("in.messages") } else { title };
            body = body.child(
                div().flex().flex_col().rounded(px(R_M)).bg(c(RAISED))
                    .child(menu_row(("vng-message", index)).min_h(px(38.)).px(px(14.))
                        .child(div().flex_1().min_w_0().truncate().child(title))
                        .when(row["unread"] == true, |d| d.child(div().size(px(6.)).rounded_full().bg(c(ONLINE))))
                        .child(div().max_w(px(150.)).truncate().text_color(c(TEXT_2)).child(detail))
                        .on_click(cx.listener(move |root, _, _, cx| {
                            root.tabs.vng.expanded = (!expanded).then_some(index);
                            cx.notify();
                        })))
                    .when(expanded, |d| d.child(div().px(px(14.)).pb(px(14.)).text_color(c(TEXT_2)).child(plain(words(row, "body"))))),
            );
        } else {
            body = body.child(line(title, detail));
        }
    }
    body
}

pub fn page(root: &Root, cx: &mut Context<Root>) -> impl IntoElement {
    let view = root.games.view;
    let state = &root.tabs.vng;
    let key = panel_key(root);
    let reading = state.request.as_ref().is_some_and(|(id, panel)| {
        root.friends.account.as_deref() == Some(id.as_str()) && key == Some(*panel)
    });
    let result = state.result.as_ref().filter(|_| reading);
    let content = match result {
        Some(Ok(data)) => details(root, data, cx),
        Some(Err(error)) => div().flex().flex_col().gap(px(8.))
            .child(root.tr("tabs.vngLoadError"))
            .when(!error.is_empty(), |d| d.child(div().text_color(c(MUTED)).child(error.clone()))),
        None => div().text_color(c(MUTED)).child(root.tr("in.loading")),
    };
    div().size_full().flex().flex_col().rounded(px(R_PANEL)).bg(c(PANEL)).overflow_hidden()
        .child(div().relative().flex().items_center().flex_none().px(px(PAD + 4.)).py(px(16.))
            .child(games::brand(root)).child(games::island(root, cx)))
        .child(div().id("vng-panel-scroll").flex_1().min_h_0().overflow_y_scroll()
            .child(div().w_full().max_w(px(780.)).mx_auto().px(px(16.)).pb(px(24.)).flex().flex_col().gap(px(12.))
                .child(div().flex().items_center().justify_between().gap(px(8.))
                    .child(div().text_size(px(T_H)).font_weight(FontWeight::SEMIBOLD).child(root.tr(title(view).unwrap_or("nav.roblox"))))
                    .child(IconButton::new("vng-refresh", "refresh").size(32.).disabled(result.is_none()).tip(root.tr("fr.refresh"))
                        .on_click(cx.listener(|root, _, _, cx| {
                            root.tabs.vng.request = None;
                            cx.notify();
                        }))))
                .when(view == View::Inbox, |d| {
                    d.child(div().flex().gap(px(4.)).children([(false, "in.messages"), (true, "in.notices")].map(|(notices, label)| {
                        segment(label, state.notices == notices).h(px(30.)).px(px(10.)).cursor_pointer().child(root.tr(label))
                            .on_click(cx.listener(move |root, _, _, cx| {
                                root.tabs.vng.notices = notices;
                                cx.notify();
                            }))
                    })))
                })
                .child(content)))
}
