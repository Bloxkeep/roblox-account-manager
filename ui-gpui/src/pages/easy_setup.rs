// Easy setup (after another manager's "Easy setup" box): this PC (CPU, RAM, GPU, what one Roblox window takes here and
// about how many fit, core/capacity.rs, measured never guessed), how many accounts the person wants at once, and the
// settings tuned for that number (plan): the mode, the leanest drawing, the gap between windows opening, reopening a
// window whose memory grows. Only what changes is listed and sent. Opened from Settings › Roblox.
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::dialog::DialogEntrance;
use gpui_kit::{prelude::*, *};
use serde_json::{Value, json};

use crate::{kit::*, shell::Root, theme::*};

#[derive(Default)]
pub struct Data {
    /// The mode picked in the box over the suggested one, and the leanest drawing switched by hand; None: as planned.
    picked: Option<&'static str>,
    flags: Option<bool>,
    /// How many accounts at once the settings are tuned for.
    target: i64,
    open: bool,
}

/// The CPU's name and the GPUs' names, as Windows has them (read once).
fn pc() -> &'static (String, Vec<String>) {
    static PC: std::sync::OnceLock<(String, Vec<String>)> = std::sync::OnceLock::new();
    PC.get_or_init(|| (tidy(&reg(r"HARDWARE\DESCRIPTION\System\CentralProcessor\0", "ProcessorNameString").unwrap_or_default()), gpus().iter().map(|g| tidy(g)).collect()))
}

/// "13th Gen Intel(R) Core(TM) i9-13980HX @ 2.20GHz" as "Intel Core i9-13980HX": the marks and the generation dropped.
fn tidy(name: &str) -> String {
    let name = name.split(" @ ").next().unwrap_or_default().replace("(R)", "").replace("(TM)", "").replace("(tm)", "");
    let words: Vec<&str> = name.split_whitespace().collect();
    let mut out: Vec<&str> = Vec::new();
    let mut i = 0;
    while i < words.len() {
        let w = words[i];
        let ordinal = w.chars().next().is_some_and(|c| c.is_ascii_digit()) && w.trim_end_matches(|c: char| c.is_ascii_alphabetic()).chars().all(|c| c.is_ascii_digit());
        if ordinal && words.get(i + 1) == Some(&"Gen") {
            i += 2;
            continue;
        }
        if w != "CPU" && w != "Processor" {
            out.push(w);
        }
        i += 1;
    }
    out.join(" ")
}

#[cfg(windows)]
fn reg(key: &str, name: &str) -> Option<String> {
    use windows_sys::Win32::System::Registry::{HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW};
    let key: Vec<u16> = key.encode_utf16().chain([0]).collect();
    let name: Vec<u16> = name.encode_utf16().chain([0]).collect();
    let mut buffer = vec![0u16; 512];
    let mut size = (buffer.len() * 2) as u32;
    // SAFETY: both names end in 0; the buffer is as long as `size` says.
    let r = unsafe { RegGetValueW(HKEY_LOCAL_MACHINE, key.as_ptr(), name.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), buffer.as_mut_ptr().cast(), &mut size) };
    if r != 0 {
        return None;
    }
    let n = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    Some(String::from_utf16_lossy(&buffer[..n]))
}

#[cfg(not(windows))]
fn reg(_: &str, _: &str) -> Option<String> {
    None
}

/// Display adapters (Windows' display class), the discrete one first; not the stand-ins Windows or remote tools add.
fn gpus() -> Vec<String> {
    let mut out: Vec<String> = (0..10)
        .filter_map(|i| reg(&format!(r"SYSTEM\CurrentControlSet\Control\Class\{{4d36e968-e325-11ce-bfc1-08002be10318}}\{i:04}"), "DriverDesc"))
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .filter(|n| !["Microsoft", "Virtual", "Parsec", "Remote", "Meta ", "Citrix", "VMware"].iter().any(|w| n.contains(w)))
        .collect();
    out.dedup();
    out.sort_by_key(|n| if n.contains("NVIDIA") || n.contains("Radeon RX") || n.contains("Arc A") { 0 } else { 1 });
    out
}

fn st(r: &Root) -> &Value {
    r.state.get("settings").unwrap_or(&Value::Null)
}

/// What Easy setup will change for `target` accounts at once: the mode, the leanest drawing, the gap between windows
/// opening, the reopening of a window whose memory grows, each only where it differs from the settings now. Decided from
/// what was measured here (capacity.rs's number, what one window takes, the PC's RAM and threads), not from a guess of
/// what a mode saves.
struct Plan {
    /// The mode suggested, and the one used (the person's pick in the box, else the suggested).
    best: &'static str,
    mode: &'static str,
    flags: bool,
    patch: Value,
    rows: Vec<String>,
    /// About how many fit now, when the target is more than that.
    over: Option<i64>,
    multi_off: bool,
}

/// What one Roblox window takes before any was measured on this PC: as measured on a real one (capacity.json:
/// 1166 MB each), rounded down.
const FIRST_EACH_MB: f64 = 1150.;

/// RAM left to Windows and the rest, as capacity.rs keeps it: 1 GB, or 8 % of the PC's.
fn kept_mb(total: f64) -> f64 {
    1024_f64.max(total * 0.08)
}

/// What one window takes: as measured here (capacity.rs), else FIRST_EACH_MB.
pub(crate) fn each_mb(r: &Root, cx: &App) -> f64 {
    r.metrics(cx).pointer("/fit/eachMb").and_then(Value::as_f64).filter(|e| *e >= 50.).unwrap_or(FIRST_EACH_MB)
}

/// About how many windows fit now: capacity.rs's number once a window was measured; before that (a first run) from
/// the RAM free now at FIRST_EACH_MB each. None when the PC's memory isn't read yet.
fn fit_now(r: &Root, cx: &App) -> Option<i64> {
    let m = r.metrics(cx);
    m.pointer("/fit/max").and_then(Value::as_i64).or_else(|| {
        let free = m.get("availableMb").and_then(Value::as_f64).filter(|f| *f > 0.)?;
        let total = m.get("systemMb").and_then(Value::as_f64).unwrap_or(free);
        Some((((free - kept_mb(total)) / FIRST_EACH_MB).floor() as i64).max(1))
    })
}

/// The most the "how many at once" pickers go to: as many windows as this PC's whole RAM could hold with everything
/// else closed (60 at most). They went to 60 on any PC: 57 at once on a 32 GB laptop took no notice. Past what fits
/// now, the plan says so (`over`).
pub fn most(r: &Root, cx: &App) -> i64 {
    match r.metrics(cx).get("systemMb").and_then(Value::as_f64).filter(|t| *t > 0.) {
        Some(total) => (((total - kept_mb(total)) / each_mb(r, cx)).floor() as i64).clamp(1, 60),
        None => 60,
    }
}

fn plan(r: &Root, target: i64, cx: &App) -> Plan {
    let st = st(r);
    let m = r.metrics(cx);
    let fit = fit_now(r, cx);
    let each = m.pointer("/fit/eachMb").and_then(Value::as_f64).unwrap_or(0.);
    let total_gb = m.get("systemMb").and_then(Value::as_f64).unwrap_or(0.) / 1024.;
    let threads = r.state.pointer("/cores/logical").and_then(Value::as_i64).unwrap_or(0);
    // Near the PC's number (four in five of it) or past it; with nothing measured yet, four windows and up count as near.
    let tight = fit.map_or(target >= 4, |f| target * 5 > f * 4);
    let over = fit.filter(|f| target > *f);
    let best = if target <= 1 { "off" } else if tight { "light" } else { "medium" };
    let mode = r.easy.picked.unwrap_or(best);
    let now = crate::pages::settings::level_now(st);
    let mut patch = json!({});
    let mut rows = Vec::new();
    if Some(mode) != now
        && let Some(set) = crate::pages::settings::level_settings(mode)
    {
        patch = set;
        rows.push(r.i18n.tv("es.do.mode", &[("m", &r.tr(&format!("rc.level.{mode}")))]));
    }
    // The leanest drawing when the target is near or past the number, on a mode that changes Roblox at all.
    let flags_now = st.get("fastFlags").and_then(Value::as_bool).unwrap_or(false);
    let flags = mode != "off" && r.easy.flags.unwrap_or(over.is_some() || (tight && total_gb > 0. && total_gb < 16.) || flags_now);
    if flags != flags_now || (mode == "off" && flags_now) {
        patch["fastFlags"] = json!(flags);
        if flags {
            rows.push(r.tr("es.do.flags"));
        }
    }
    // Windows opening further apart on a smaller PC or for many accounts (a load at once is where joins fail); never
    // shortened from what the person set.
    let delay = st.get("launchDelaySeconds").and_then(Value::as_i64).unwrap_or(8);
    let small = (total_gb > 0. && total_gb < 16.) || (threads > 0 && threads < 8);
    if (small || target >= 8) && delay < 12 {
        patch["launchDelaySeconds"] = json!(12);
        rows.push(r.i18n.tv("es.do.delay", &[("n", "12")]));
    }
    // Near the number: a window whose memory keeps growing is opened again into its server before it pushes the rest.
    let restart = st.get("restartAboveMb").and_then(Value::as_i64).unwrap_or(0);
    if tight && restart == 0 && each > 0. {
        let at = (((each * 2.5) / 500.).ceil() * 500.).max(2500.) as i64;
        patch["restartAboveMb"] = json!(at);
        rows.push(r.i18n.tv("es.do.restart", &[("v", &crate::pages::accounts::mb_text(at as f64))]));
    }
    let multi_off = target > 1 && st.get("multiInstance").and_then(Value::as_bool) == Some(false);
    Plan { best, mode, flags, patch, rows, over, multi_off }
}

/// What the first run's PC step shows: the same measures and plan as the box (its suggested mode).
pub struct Facts {
    pub cpu: Option<String>,
    pub threads: i64,
    pub ram: Option<String>,
    pub free: Option<String>,
    pub gpu: Option<String>,
    /// About how many fit, what one window takes and what runs out first (measured from real windows; none yet on a
    /// first run).
    pub fit: Option<(i64, String, String)>,
    pub rows: Vec<String>,
    pub over: Option<i64>,
    pub patch: Value,
}

pub fn facts(r: &Root, target: i64, cx: &App) -> Facts {
    let (cpu, gpus) = pc();
    let m = r.metrics(cx);
    let total = m.get("systemMb").and_then(Value::as_f64).unwrap_or(0.);
    let free = m.get("availableMb").and_then(Value::as_f64).unwrap_or(0.);
    let gb = |mb: f64| crate::lang::cur().mb((mb / 1024.).round() * 1024.);
    let fit = m.get("fit").filter(|f| f.is_object()).map(|f| {
        (
            f.get("max").and_then(Value::as_i64).unwrap_or(0),
            crate::pages::accounts::mb_text(f.get("eachMb").and_then(Value::as_f64).unwrap_or(0.)),
            r.tr(&format!("cap.by.{}", f.get("by").and_then(Value::as_str).unwrap_or("ram"))),
        )
    });
    let p = plan(r, target, cx);
    Facts {
        cpu: Some(cpu.clone()).filter(|c| !c.is_empty()),
        threads: r.state.pointer("/cores/logical").and_then(Value::as_i64).unwrap_or(0),
        ram: (total > 0.).then(|| gb(total)),
        free: (free > 0.).then(|| gb(free)),
        gpu: gpus.first().cloned(),
        fit,
        rows: p.rows,
        over: p.over,
        patch: p.patch,
    }
}

pub fn open(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    // How many to plan for: the accounts there are, at least one.
    let target = (r.accounts().len() as i64).clamp(1, most(r, cx));
    r.easy = Data { open: true, target, ..Default::default() };
    r.help_open = true;
    let this = cx.weak_entity();
    window.open_dialog(cx, move |dialog, _, cx| {
        let Some(root) = this.upgrade() else { return dialog };
        let r = root.read(cx);
        let m = r.metrics(cx);
        let p = plan(r, r.easy.target, cx);

        // This PC: three tiles.
        let (cpu, gpus) = pc();
        let threads = r.state.pointer("/cores/logical").and_then(Value::as_i64).unwrap_or(0);
        let total = m.get("systemMb").and_then(Value::as_f64).unwrap_or(0.);
        let free = m.get("availableMb").and_then(Value::as_f64).unwrap_or(0.);
        let gb = |mb: f64| crate::lang::cur().mb((mb / 1024.).round() * 1024.);
        let tile = |ic: &'static str, head: String, main: String, sub: Option<String>| {
            div()
                .flex_1()
                .min_w_0()
                .p(px(12.))
                .rounded(px(R_M))
                .bg(c(INSET))
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(div().flex().items_center().gap(px(6.)).text_size(px(T_XS)).font_weight(FontWeight::SEMIBOLD).text_color(c(MUTED)).child(icon(ic, 13., c(MUTED))).child(head))
                .child(div().text_size(px(T_M)).line_height(px(18.)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(main))
                .children(sub.map(|t| div().text_size(px(T_S)).text_color(c(MUTED)).truncate().child(t)))
        };
        let unknown = r.tr("es.unknown");
        let tiles = div()
            .flex()
            .gap(px(8.))
            .child(tile("chip", "CPU".into(), if cpu.is_empty() { unknown.clone() } else { cpu.clone() }, (threads > 0).then(|| r.i18n.tv("es.threads", &[("n", &threads.to_string())]))))
            .child(tile("gauge", "RAM".into(), if total > 0. { gb(total) } else { unknown.clone() }, (free > 0.).then(|| r.i18n.tv("es.free", &[("v", &gb(free))]))))
            .child(tile("monitor", "GPU".into(), gpus.first().cloned().unwrap_or(unknown), gpus.get(1).cloned()));

        // What one window takes here, and the number.
        let fit = m.get("fit").filter(|f| f.is_object());
        let result = match fit {
            Some(f) => {
                let n = f.get("max").and_then(Value::as_i64).unwrap_or(0);
                let by = r.tr(&format!("cap.by.{}", f.get("by").and_then(Value::as_str).unwrap_or("ram")));
                let each = crate::pages::accounts::mb_text(f.get("eachMb").and_then(Value::as_f64).unwrap_or(0.));
                div()
                    .p(px(14.))
                    .rounded(px(R_M))
                    .border_1()
                    .border_color(c(LINE))
                    .flex()
                    .items_center()
                    .gap(px(14.))
                    .child(div().flex_none().text_size(px(30.)).font_weight(FontWeight::BOLD).text_color(c(TEXT)).font_features(tnum()).child(n.to_string()))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(div().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(r.tr("es.runs")))
                            .child(div().text_size(px(T_S)).line_height(px(17.)).text_color(c(MUTED)).child(r.i18n.tv("es.why", &[("each", &each), ("by", &by)]))),
                    )
            }
            None => div().p(px(14.)).rounded(px(R_M)).border_1().border_color(c(LINE)).text_size(px(T_S)).line_height(px(17.)).text_color(c(MUTED)).child(r.tr("es.measure")),
        };

        // How many accounts at once: the plan follows it.
        let step = |id: &'static str, ic: &'static str, by: i64, this: WeakEntity<Root>| {
            IconButton::new(id, ic).size(30.).on_click(move |_, w, cx| {
                let _ = this.update(cx, |r, cx| {
                    r.easy.target = (r.easy.target + by).clamp(1, most(r, cx));
                    r.easy.picked = None;
                    r.easy.flags = None;
                });
                w.refresh();
            })
        };
        let want = div()
            .flex()
            .items_center()
            .gap(px(12.))
            .child(div().flex_1().min_w_0().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(r.tr("es.want")))
            .child(step("es-less", "chevronLeft", -1, this.clone()).disabled(r.easy.target <= 1))
            .child(div().w(px(36.)).text_center().text_size(px(T_H)).font_weight(FontWeight::BOLD).font_features(tnum()).text_color(c(TEXT)).child(r.easy.target.to_string()))
            .child(step("es-more", "chevronRight", 1, this.clone()).disabled(r.easy.target >= most(r, cx)));

        // The modes, the suggested one marked.
        let mut modes = div().flex().flex_col().gap(px(6.));
        for (i, id) in ["light", "medium", "off"].into_iter().enumerate() {
            let on = p.mode == id;
            let this = this.clone();
            modes = modes.child(
                div()
                    .id(("es-mode", i))
                    .p(px(12.))
                    .rounded(px(R_M))
                    .border_1()
                    .border_color(if on { ca(WHITE, 0.5) } else { c(LINE) })
                    .bg(if on { ca(WHITE, 0.06) } else { c(INSET) })
                    .cursor_pointer()
                    .when(!on, |d| d.hover(|d| d.border_color(ca(WHITE, 0.18))))
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(3.))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(div().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(r.tr(&format!("es.goal.{id}"))))
                                    .when(p.best == id, |d| d.child(div().px(px(6.)).rounded(px(R_S)).border_1().border_color(c(LINE)).text_size(px(T_XS)).text_color(c(TEXT_2)).child(r.tr("es.best")))),
                            )
                            .child(div().text_size(px(T_S)).text_color(c(MUTED)).child(r.tr(&format!("rc.level.{id}.says")))),
                    )
                    .child(
                        div()
                            .size(px(16.))
                            .flex_none()
                            .rounded_full()
                            .border_1()
                            .border_color(if on { c(FACE) } else { ca(WHITE, 0.25) })
                            .bg(if on { c(FACE) } else { ca(WHITE, 0.) })
                            .flex()
                            .items_center()
                            .justify_center()
                            .when(on, |d| d.child(icon("check", 11., c(INK)))),
                    )
                    .on_click(move |_, w, cx| {
                        let _ = this.update(cx, |r, _| r.easy.picked = Some(id));
                        w.refresh();
                    }),
            );
        }
        let flags_row = {
            let this = this.clone();
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(div().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(r.tr("rc.flags")))
                        .child(div().text_size(px(T_S)).line_height(px(17.)).text_color(c(MUTED)).child(r.tr("rc.flagsDesc"))),
                )
                .child(Switch::new("es-flags", p.flags).disabled(p.mode == "off").on_toggle(move |on, w, cx| {
                    let on = *on;
                    let _ = this.update(cx, |r, _| r.easy.flags = Some(on));
                    w.refresh();
                }))
        };

        // What Apply changes, one line each.
        let mut changes = div().flex().flex_col().gap(px(6.));
        if p.rows.is_empty() {
            changes = changes.child(div().text_size(px(T_S)).text_color(c(MUTED)).child(r.tr("es.do.none")));
        }
        for row in &p.rows {
            changes = changes.child(div().flex().items_start().gap(px(8.)).text_size(px(T_S)).line_height(px(17.)).text_color(c(TEXT_2)).child(div().mt(px(2.)).child(icon("check", 13., c(OK)))).child(div().flex_1().min_w_0().child(row.clone())));
        }
        if p.multi_off {
            changes = changes.child(div().flex().items_start().gap(px(8.)).text_size(px(T_S)).line_height(px(17.)).text_color(c(WARN)).child(div().mt(px(2.)).child(icon("alert", 13., c(WARN)))).child(div().flex_1().min_w_0().child(r.tr("es.multi"))));
        }

        let label = |t: String| div().text_size(px(T_XS)).font_weight(FontWeight::SEMIBOLD).text_color(c(MUTED)).child(t.to_uppercase());
        let body = div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(label(r.tr("es.pc")))
            .child(tiles)
            .child(result)
            .child(div().h(px(6.)))
            .child(want)
            .when_some(p.over, |d, n| d.child(div().text_size(px(T_S)).line_height(px(17.)).text_color(c(WARN)).child(r.i18n.tv("es.over", &[("n", &n.to_string())]))))
            .child(modes)
            .child(flags_row)
            .child(div().h(px(6.)))
            .child(label(r.tr("es.plan")))
            .child(changes)
            .when(!p.rows.is_empty(), |d| d.child(div().text_size(px(T_S)).text_color(c(MUTED)).child(r.tr("rc.reopen"))));

        let changed = !p.rows.is_empty() || p.patch.as_object().is_some_and(|o| !o.is_empty());
        let (go, shut, closed) = (this.clone(), this.clone(), this.clone());
        let foot = dialog_foot()
            .child(Button::new("es-cancel", r.tr("common.cancel")).on_click({
                let this = this.clone();
                move |_, w, cx| {
                    let _ = this.update(cx, |r, _| r.easy.open = false);
                    w.close_dialog(cx);
                }
            }))
            .child(Button::new("es-apply", r.tr("vis.apply")).icon("check").primary().disabled(!changed).on_click(move |_, w, cx| {
                let _ = go.update(cx, |r, cx| {
                    let p = plan(r, r.easy.target, cx);
                    if p.patch.as_object().is_some_and(|o| !o.is_empty()) {
                        r.send("setSettings", p.patch);
                        r.toast = Some((r.tr("es.done"), "ok".into()));
                    }
                    r.easy.open = false;
                    cx.notify();
                });
                w.close_dialog(cx);
            }));
        framed(dialog, r.tr("es.title"), r.tr("common.close"), move |_, w, cx| {
            let _ = shut.update(cx, |r, _| r.easy.open = false);
            w.close_dialog(cx);
        })
        .w(px(DIALOG_W + 40.))
        .centered()
        .entrance(DialogEntrance::FadeSlide)
        .on_close(move |_, _, cx| {
            let _ = closed.update(cx, |r, _| r.easy.open = false);
        })
        .child(dialog_body(body))
        .footer(foot)
    });
}
