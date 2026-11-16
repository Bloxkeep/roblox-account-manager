use gpui_kit::{prelude::*, *};

use crate::{
    core::{app::App, remote_watch::{self, Status}, rt, ui},
    kit::*,
    shell::Root,
    theme::*,
};

fn start(compat: bool, cx: &mut Context<Root>) {
    rt::post(move || App::with(|app| {
        if app.unlocked() && !app.controller.disposed.get() { remote_watch::start(compat); }
    }));
    cx.spawn(async move |this, cx| {
        loop {
            cx.background_executor().timer(std::time::Duration::from_millis(350)).await;
            if this.update(cx, |_, cx| cx.notify()).is_err() { break; }
            if matches!(remote_watch::status(), Status::Idle | Status::Failed) { break; }
        }
    }).detach();
}

pub(super) fn controls(root: &Root, cx: &mut Context<Root>) -> Div {
    let status = remote_watch::status();
    let message = match status {
        Status::Idle => "mon.remote.hint",
        Status::Preparing => "mon.remote.preparing",
        Status::Connecting => "mon.remote.connecting",
        Status::Ready(_) => "mon.remote.ready",
        Status::Stopping => "mon.remote.stopping",
        Status::Failed => "mon.remote.failed",
    };
    let actions = match status {
        Status::Idle | Status::Failed => div().flex().flex_wrap().gap(px(8.))
            .child(Button::new("mon-remote-start", root.tr("mon.remote.start")).small().icon("globe")
                .on_click(cx.listener(|_, _, _, cx| start(false, cx))))
            .child(Button::new("mon-remote-compat", root.tr("mon.remote.compat")).small()
                .on_click(cx.listener(|_, _, _, cx| start(true, cx)))),
        Status::Ready(_) => div().flex().flex_wrap().gap(px(8.))
            .child(Button::new("mon-remote-open", root.tr("mon.remote.open")).small().icon("external")
                .on_click(|_, _, cx| if let Status::Ready(link) = remote_watch::status() { cx.open_url(&link); }))
            .child(Button::new("mon-remote-copy", root.tr("mon.remote.copy")).small().icon("copy")
                .on_click(cx.listener(|root, _, _, cx| {
                    if let Status::Ready(link) = remote_watch::status() && ui::copy_to_clipboard(&link, true) {
                        root.toast = Some((root.tr("mon.remote.copied"), "ok".into()));
                        cx.notify();
                    }
                })))
            .child(stop(root, cx)),
        Status::Preparing | Status::Connecting => div().child(stop(root, cx)),
        Status::Stopping => div(),
    };
    div().flex().flex_wrap().items_center().justify_between().gap(px(12.))
        .rounded(px(R_M)).bg(c(RAISED)).p(px(14.))
        .child(div().flex().flex_col().gap(px(4.)).flex_1().min_w(px(230.))
            .child(div().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).child(root.tr("mon.remote.title")))
            .child(div().text_size(px(T_S)).text_color(c(MUTED)).child(root.tr(message))))
        .child(actions)
}

fn stop(root: &Root, cx: &mut Context<Root>) -> Button {
    Button::new("mon-remote-stop", root.tr("mon.remote.stop")).small()
        .on_click(cx.listener(|_, _, _, cx| {
            remote_watch::stop();
            ui::clear_secret_clipboard();
            cx.notify();
        }))
}
