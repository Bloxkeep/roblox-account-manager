// The Game monitor's load: how hard the Roblox windows work this PC, at the right of the monitor's head (a state
// word, then CPU, RAM and GPU), and a click on it opens what's behind those figures: a ring for each, the whole PC's
// CPU and RAM, Windows' memory handed out, and about how many windows the PC runs (capacity.rs). The owner,
// 2026-11-17: "làm cái như này trong game monitor này". Figures from the app's metrics, every few seconds.
use gpui_kit::{prelude::*, *};
use serde_json::Value;

use super::{mb_text, pop_alive};
use crate::{kit::*, shell::Root, theme::*};

const KEY: &str = "mon-load";

/// The metrics' figures for the load, read once a frame.
struct Load {
    windows: i64,
    /// The Roblox windows' share of the CPU and GPU, and their RAM against the PC's (percent).
    cpu: Option<f64>,
    gpu: Option<f64>,
    ram: f64,
    ram_mb: f64,
    system_mb: f64,
    /// The whole PC: CPU busy, RAM in use (percent), memory Windows has handed out (MB).
    pc_cpu: Option<f64>,
    pc_ram: f64,
    commit_mb: f64,
}

impl Load {
    fn of(root: &Root) -> Load {
        let m = &root.metrics;
        let f = |k: &str| m.get(k).and_then(Value::as_f64);
        let (ram_mb, system_mb) = (f("totalMb").unwrap_or(0.), f("systemMb").unwrap_or(0.));
        Load {
            windows: m.get("robloxCount").and_then(Value::as_i64).unwrap_or(0),
            cpu: f("cpuPct"),
            gpu: f("gpuPct"),
            ram: if system_mb > 0. { ram_mb * 100. / system_mb } else { 0. },
            ram_mb,
            system_mb,
            pc_cpu: f("pcCpuPct"),
            pc_ram: f("usedPct").unwrap_or(0.),
            commit_mb: f("commitMb").unwrap_or(0.),
        }
    }

    /// The state word, by the fullest of what the PC has (the Roblox windows' GPU share counts: no other figure says
    /// how busy the GPU is): easy, busy, near the limit, at the status bar's figures' steps (kit::load_colour).
    fn state(&self, root: &Root) -> (String, Tone) {
        let worst = [self.pc_cpu.unwrap_or(0.), self.pc_ram, self.gpu.unwrap_or(0.)].into_iter().fold(0., f64::max);
        if worst >= LOAD_FULL {
            (root.tr("mon.load.full"), Tone::Bad)
        } else if worst >= LOAD_BUSY {
            (root.tr("mon.load.busy"), Tone::Warn)
        } else {
            (root.tr("mon.load.easy"), Tone::Ok)
        }
    }
}

fn pct(v: Option<f64>) -> String {
    v.map_or("–".into(), |v| format!("{}%", v.round() as i64))
}

/// At the right of the monitor's head: the state word, CPU, RAM and GPU; pressed, the panel under it.
pub fn strip(root: &Root, cx: &mut Context<Root>) -> Div {
    let load = Load::of(root);
    let open = root.acc_drop == Some(KEY);
    let (word, tone) = load.state(root);
    let figure = |name: &'static str, v: Option<f64>| {
        div()
            .flex()
            .items_center()
            .gap(px(5.))
            .child(div().text_color(c(MUTED)).child(name))
            .child(div().min_w(px(30.)).font_weight(FontWeight::MEDIUM).font_features(tnum()).text_color(load_colour(v.unwrap_or(0.))).child(pct(v)))
    };
    let tip: SharedString = root.tr("mon.load.tip").into();
    let mut wrap = div().relative().flex_none().child(
        tool(KEY, None, open)
            .gap(px(12.))
            .text_size(px(T_S))
            .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
            .child(badge(word, tone))
            .child(figure("CPU", load.cpu))
            .child(figure("RAM", Some(load.ram)))
            .when(load.gpu.is_some(), |d| d.child(figure("GPU", load.gpu)))
            .child(tool_chev())
            .on_click(cx.listener(|r, _, _, cx| {
                // A click outside the open panel closed it just now: this click on its own button only closes it.
                let just = r.acc_drop_closed.is_some_and(|(k, at)| k == KEY && at.elapsed().as_millis() < 300);
                r.acc_drop_closed = None;
                r.acc_drop = if just || r.acc_drop == Some(KEY) { None } else { Some(KEY) };
                cx.notify();
            })),
    );
    if pop_alive(KEY, open) {
        let panel = panel(root, &load, cx);
        wrap = wrap.child(popup(PopAt::BelowRight(TOOL_H + 4.), "mon-load-pop", open, panel));
    }
    wrap
}

/// What's behind the figures.
fn panel(root: &Root, load: &Load, cx: &mut Context<Root>) -> Stateful<Div> {
    let (word, tone) = load.state(root);
    let together = root.i18n.tv("mon.load.together", &[("n", &load.windows.to_string())]);
    let ram_words = root.i18n.tv(
        "mon.load.ram",
        &[("used", &mb_text(load.ram_mb)), ("total", &mb_text(load.system_mb)), ("commit", &mb_text(load.commit_mb))],
    );
    let row = |name: &'static str, v: Option<f64>, words: String| {
        div()
            .flex()
            .items_center()
            .gap(px(14.))
            .child(
                div()
                    .relative()
                    .size(px(48.))
                    .flex_none()
                    .child(ring(v.unwrap_or(0.) as f32, 48., 4., load_colour(v.unwrap_or(0.))))
                    .child(div().absolute().inset_0().flex().items_center().justify_center().text_size(px(T_XS)).font_weight(FontWeight::SEMIBOLD).font_features(tnum()).text_color(c(TEXT)).child(pct(v))),
            )
            .child(
                div()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(div().text_size(px(T_M)).font_weight(FontWeight::MEDIUM).text_color(c(TEXT)).child(name))
                    .child(div().text_size(px(T_S)).text_color(c(MUTED)).child(words)),
            )
    };
    let pc = root.i18n.tv("mon.load.pc", &[("cpu", &pct(load.pc_cpu)), ("ram", &pct(Some(load.pc_ram)))]);
    // About how many windows this PC runs, and what runs out first (capacity.rs): Bloxkeep's own measure.
    let fit = root.metrics.get("fit").filter(|f| f.is_object()).map(|f| {
        let max = f.get("max").and_then(Value::as_i64).unwrap_or(0).to_string();
        let by = f.get("by").and_then(Value::as_str).unwrap_or("ram");
        format!("{} · {}", root.i18n.tv("cap.fits", &[("n", &max)]), root.i18n.tv("mon.load.first", &[("by", &root.tr(&format!("cap.by.{by}")))]))
    });
    div()
        .id("mon-load-panel")
        .occlude()
        .relative()
        .w(px(360.))
        .p(px(16.))
        .rounded(px(R_M))
        .bg(c(RAISED))
        .border_1()
        .border_color(c(LINE))
        .shadow_lg()
        .flex()
        .flex_col()
        .gap(px(14.))
        // (Windows draws the windows' live pictures over this window: none over the panel.)
        .child(root.live.cover())
        .on_mouse_down_out(cx.listener(|r, _, _, cx| {
            if r.acc_drop == Some(KEY) {
                r.acc_drop = None;
                r.acc_drop_closed = Some((KEY, std::time::Instant::now()));
                cx.notify();
            }
        }))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(10.))
                .child(div().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(root.tr("mon.load.head")))
                .child(badge(word, tone)),
        )
        .child(div().text_size(px(T_S)).text_color(c(TEXT_2)).child(pc))
        .child(row("CPU", load.cpu, together.clone()))
        .child(row("RAM", Some(load.ram), ram_words))
        .when(load.gpu.is_some(), |d| d.child(row("GPU", load.gpu, together)))
        .when_some(fit, |d, t| d.child(div().pt(px(10.)).border_t_1().border_color(c(LINE)).text_size(px(T_S)).text_color(c(MUTED)).child(t)))
}
