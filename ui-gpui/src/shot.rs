// --shot <png>: draws the window without ever showing it, saves the picture and quits, for checking screens on a PC
// where nothing may pop up (built with --features shot, which brings GPUI's render_to_image).
#[cfg(feature = "shot")]
use crate::threads::Hold;
use std::{path::PathBuf, time::Duration};

use gpui_kit::*;

/// After `wait` (the app's state and the pictures arriving), pictures the window at `size` and quits.
pub fn schedule<V: 'static>(view: WeakEntity<V>, out: PathBuf, w: f32, h: f32, wait: Duration, cx: &mut Context<V>) -> Task<()> {
    cx.spawn(async move |_, cx| {
        // A window never shown never gets a size, so its renderer stays 1×1: set one (it stays hidden).
        let handle = view.update_in(cx, |_, window, _| {
            window.resize(size(px(w), px(h + 1.)));
            Window::window_handle(window)
        });
        cx.background_executor().timer(Duration::from_millis(150)).await;
        let _ = view.update_in(cx, |_, window, _| window.resize(size(px(w), px(h))));
        // A hidden window is never asked to draw, and what a page reads (and each picture) is asked for as it is
        // drawn: drawn through the wait, so the wait is time for those to come, one read after another.
        #[cfg(feature = "shot")]
        {
            let until = std::time::Instant::now() + wait;
            while std::time::Instant::now() < until {
                if let Ok(h) = handle {
                    let _ = cx.update_window(h, |_, window, cx| {
                        away(window, cx);
                        window.draw(cx).clear(cx)
                    });
                }
                cx.background_executor().timer(Duration::from_millis(500)).await;
            }
        }
        #[cfg(not(feature = "shot"))]
        cx.background_executor().timer(wait).await;
        #[cfg(feature = "shot")]
        {
            // A hidden window is never asked to draw: draw here.
            // A picture is decoded on the draw after it arrives, and some arrive only after others (a game's cover
            // after its look): a few draws, a moment apart.
            for _ in 0..5 {
                if let Ok(h) = handle {
                    let _ = cx.update_window(h, |_, window, cx| {
                        away(window, cx);
                        window.draw(cx).clear(cx)
                    });
                }
                cx.background_executor().timer(Duration::from_millis(700)).await;
            }
            // BLOXKEEP_SHOT_KEYS="tab tab ctrl-k": keys handed to this hidden window itself (never to the desktop), to
            // see focus rings and shortcuts in the picture.
            if let (Ok(&h), Ok(keys)) = (handle.as_ref(), std::env::var("BLOXKEEP_SHOT_KEYS")) {
                for k in keys.split_whitespace() {
                    let k: &'static str = Box::leak(k.to_string().into_boxed_str());
                    if let Ok(stroke) = Keystroke::parse(k) {
                        let _ = cx.update_window(h, |_, window, cx| {
                            // As a real key press arrives (down, then up), so the focus ring shows as it would.
                            let down = KeyDownEvent { keystroke: stroke.clone(), is_held: false, prefer_character_input: false };
                            window.dispatch_event(PlatformInput::KeyDown(down), cx);
                            window.dispatch_event(PlatformInput::KeyUp(KeyUpEvent { keystroke: stroke }), cx);
                            window.draw(cx).clear(cx);
                        });
                    }
                    cx.background_executor().timer(Duration::from_millis(150)).await;
                }
            }
            // BLOXKEEP_SHOT_TALLY=<s>: the window left to draw as it would on screen (a frame whenever something asks,
            // at most every 4 ms, a 240 Hz screen), for 3 counts of s seconds (frame_tally.rs prints them).
            if let (Ok(&h), Some(s)) = (handle.as_ref(), std::env::var("BLOXKEEP_SHOT_TALLY").ok().and_then(|v| v.parse::<u64>().ok())) {
                let _ = gpui_kit::take_frame_tally();
                let until = std::time::Instant::now() + Duration::from_secs(s * 3 + 1);
                while std::time::Instant::now() < until {
                    let _ = cx.update_window(h, |_, window, cx| window.vsync_for_shot(cx));
                    cx.background_executor().timer(Duration::from_millis(4)).await;
                }
                let _ = cx.update(|cx| cx.quit());
                return;
            }
            // BLOXKEEP_SHOT_BENCH=<n>: what each frame costs, with the script's mouse (see `frames`): no pictures.
            if let (Ok(&h), Some(n)) = (handle.as_ref(), std::env::var("BLOXKEEP_SHOT_BENCH").ok().and_then(|v| v.parse::<usize>().ok())) {
                frames(h, &out, n, true, cx).await;
                let _ = cx.update(|cx| cx.quit());
                return;
            }
            // BLOXKEEP_SHOT_FRAMES=<n>: a clip instead of one picture (see `frames`).
            if let (Ok(&h), Some(n)) = (handle.as_ref(), std::env::var("BLOXKEEP_SHOT_FRAMES").ok().and_then(|v| v.parse::<usize>().ok())) {
                frames(h, &out, n, false, cx).await;
                let _ = cx.update(|cx| cx.quit());
                return;
            }
            let pic = handle.and_then(|h| {
                cx.update_window(h, |_, window, cx| {
                    window.draw(cx).clear(cx);
                    window.render_to_image()
                })
            });
            match pic {
                Ok(Ok(pic)) => match pic.save(&out) {
                    Ok(()) => eprintln!("shot: {}", out.display()),
                    Err(e) => eprintln!("shot failed: {e}"),
                },
                Ok(Err(e)) | Err(e) => eprintln!("shot failed: {e}"),
            }
        }
        #[cfg(not(feature = "shot"))]
        {
            let _ = handle;
            eprintln!("--shot needs a build with --features shot ({})", out.display());
        }
        let _ = cx.update(|cx| cx.quit());
    })
}

/// One step of a clip's script (BLOXKEEP_SHOT_ACTS), at `at` ms from the clip's start.
#[cfg(feature = "shot")]
enum Act {
    Move(f32, f32),
    /// To x,y over that many ms, eased.
    Glide(f32, f32, f32),
    Click(f32, f32),
    /// Pressed at x,y, slid to x2,y2 over that many ms, let go there.
    Drag(f32, f32, f32, f32, f32),
    Key(String),
    Type(String),
    Scroll(f32),
    /// A mouse wheel turned that many notches (3 lines each, Windows' default; down is negative), as the wheel sends it.
    Wheel(f32),
}

/// "500:glide:640,300,600; 1200:click:640,300; 1500:type:blox; 2200:key:enter; 2600:scroll:-400; 2700:wheel:-1;
/// 3000:drag:640,300,700,340,500" (window points).
#[cfg(feature = "shot")]
fn acts() -> Vec<(f32, Act)> {
    let script = std::env::var("BLOXKEEP_SHOT_ACTS").unwrap_or_default();
    let nums = |s: &str| s.split(',').filter_map(|n| n.trim().parse::<f32>().ok()).collect::<Vec<_>>();
    let mut out: Vec<(f32, Act)> = script
        .split(';')
        .filter_map(|a| {
            let mut p = a.trim().splitn(3, ':');
            let at = p.next()?.trim().parse::<f32>().ok()?;
            let kind = p.next()?.trim();
            let arg = p.next().unwrap_or("");
            let n = nums(arg);
            Some((
                at,
                match kind {
                    "move" if n.len() >= 2 => Act::Move(n[0], n[1]),
                    "glide" if n.len() >= 3 => Act::Glide(n[0], n[1], n[2]),
                    "click" if n.len() >= 2 => Act::Click(n[0], n[1]),
                    "drag" if n.len() >= 5 => Act::Drag(n[0], n[1], n[2], n[3], n[4]),
                    "key" => Act::Key(arg.trim().to_string()),
                    "type" => Act::Type(arg.to_string()),
                    "scroll" if !n.is_empty() => Act::Scroll(n[0]),
                    "wheel" if !n.is_empty() => Act::Wheel(n[0]),
                    _ => return None,
                },
            ))
        })
        .collect();
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out
}

/// A clip of the hidden window: `n` pictures (<out>-0000.png…), one every BLOXKEEP_SHOT_EVERY ms (40 by default), with
/// the script's mouse and keys handed to this window only (never to the desktop), as the window's own input. Beside them,
/// <out>.txt: per picture its time in ms and where the mouse was (x y, and 1 while pressed), for drawing a pointer later.
#[cfg(feature = "shot")]
/// `bench`: no pictures; each frame's draw (the page made and laid out) and present (handed to the graphics card) are
/// timed, and a present with nothing redrawn after it (what GPUI does every vsync for a second after fast mouse input).
async fn frames(h: AnyWindowHandle, out: &std::path::Path, n: usize, bench: bool, cx: &mut AsyncApp) {
    let every = std::env::var("BLOXKEEP_SHOT_EVERY").ok().and_then(|v| v.parse::<u64>().ok()).unwrap_or(40);
    let stem = out.with_extension("");
    let script = acts();
    let mut next = 0;
    let (mut mx, mut my, mut down) = (-4000f32, -4000f32, false);
    let mut glide: Option<(f32, f32, f32, f32, f32, f32)> = None; // from x, y, to x, y, start, length
    let mut held = false; // the glide under way is a drag: the left button is down until it ends
    let mut log = String::new();
    let mut times: Vec<(Duration, Duration, Duration)> = Vec::new();
    let mut asked: Vec<bool> = Vec::new();
    let mut phases: Vec<[Duration; 4]> = Vec::new(); // each frame's render, layout and paint
    // BLOXKEEP_SHOT_PROF=<file>: the UI thread's stacks sampled over the bench (prof.rs).
    let prof = std::env::var_os("BLOXKEEP_SHOT_PROF").filter(|_| bench).map(|p| (std::path::PathBuf::from(p), crate::prof::start()));
    // The pictures are written by four kept threads, so taking the next one never waits for a PNG.
    let (tx, rx) = std::sync::mpsc::channel::<(PathBuf, image::RgbaImage)>();
    let rx = std::sync::Arc::new(std::sync::Mutex::new(rx));
    let writers: Vec<_> = (0..4)
        .filter_map(|_| {
            let rx = rx.clone();
            crate::threads::own("shot-writer", move || loop {
                let job = rx.hold().recv().ok();
                let Some((p, pic)) = job else { break };
                let _ = pic.save(&p);
            })
            .ok()
        })
        .collect();
    let start = std::time::Instant::now();
    let send = |input: PlatformInput, cx: &mut AsyncApp| {
        let _ = cx.update_window(h, |_, window, cx| window.dispatch_event(input, cx));
    };
    for i in 0..n {
        // (A bench run says, every 250 frames, how many threads the process has started so far: none more once the
        // page is up is what threads.rs is for; see `started`.)
        if bench && i % 250 == 0 {
            eprintln!("bench: frame {i}, {:.0} s, threads started {}", start.elapsed().as_secs_f32(), started::count());
        }
        let target = Duration::from_millis(every * i as u64);
        let now = start.elapsed();
        if target > now {
            cx.background_executor().timer(target - now).await;
        }
        let t = start.elapsed().as_secs_f32() * 1000.;
        while next < script.len() && script[next].0 <= t {
            let (at, act) = &script[next];
            let pos = |x: f32, y: f32| point(px(x), px(y));
            match act {
                Act::Move(x, y) => {
                    (mx, my, glide) = (*x, *y, None);
                    send(PlatformInput::MouseMove(MouseMoveEvent { position: pos(mx, my), pressed_button: None, modifiers: Modifiers::default() }), cx);
                }
                Act::Glide(x, y, d) => glide = Some((mx, my, *x, *y, *at, d.max(1.))),
                Act::Click(x, y) => {
                    (mx, my, glide) = (*x, *y, None);
                    send(PlatformInput::MouseMove(MouseMoveEvent { position: pos(mx, my), pressed_button: None, modifiers: Modifiers::default() }), cx);
                    send(PlatformInput::MouseDown(MouseDownEvent { button: MouseButton::Left, position: pos(mx, my), modifiers: Modifiers::default(), click_count: 1, first_mouse: false }), cx);
                    send(PlatformInput::MouseUp(MouseUpEvent { button: MouseButton::Left, position: pos(mx, my), modifiers: Modifiers::default(), click_count: 1 }), cx);
                    down = true;
                }
                Act::Drag(x, y, x2, y2, d) => {
                    (mx, my) = (*x, *y);
                    send(PlatformInput::MouseMove(MouseMoveEvent { position: pos(mx, my), pressed_button: None, modifiers: Modifiers::default() }), cx);
                    send(PlatformInput::MouseDown(MouseDownEvent { button: MouseButton::Left, position: pos(mx, my), modifiers: Modifiers::default(), click_count: 1, first_mouse: false }), cx);
                    (glide, held) = (Some((mx, my, *x2, *y2, *at, d.max(1.))), true);
                }
                Act::Key(k) => {
                    if let Ok(stroke) = Keystroke::parse(Box::leak(k.clone().into_boxed_str())) {
                        send(PlatformInput::KeyDown(KeyDownEvent { keystroke: stroke.clone(), is_held: false, prefer_character_input: false }), cx);
                        send(PlatformInput::KeyUp(KeyUpEvent { keystroke: stroke }), cx);
                    }
                }
                Act::Type(text) => {
                    for ch in text.chars() {
                        let key = if ch == ' ' { "space".to_string() } else { ch.to_lowercase().to_string() };
                        if let Ok(mut stroke) = Keystroke::parse(Box::leak(key.into_boxed_str())) {
                            stroke.key_char = Some(ch.to_string());
                            let _ = cx.update_window(h, |_, window, cx| window.dispatch_keystroke(stroke, cx));
                        }
                    }
                }
                Act::Scroll(dy) => {
                    send(PlatformInput::Scroll(ScrollEvent { position: pos(mx, my), delta: ScrollDelta::Pixels(point(px(0.), px(*dy))), ..Default::default() }), cx);
                }
                Act::Wheel(notches) => {
                    send(PlatformInput::Scroll(ScrollEvent { position: pos(mx, my), delta: ScrollDelta::Lines(point(0., notches * 3.)), ..Default::default() }), cx);
                }
            }
            next += 1;
        }
        if let Some((fx, fy, tx2, ty, at, len)) = glide {
            let k = ((t - at) / len).clamp(0., 1.);
            let e = if k < 0.5 { 2. * k * k } else { 1. - (-2. * k + 2.).powi(2) / 2. };
            (mx, my) = (fx + (tx2 - fx) * e, fy + (ty - fy) * e);
            send(PlatformInput::MouseMove(MouseMoveEvent { position: point(px(mx), px(my)), pressed_button: held.then_some(MouseButton::Left), modifiers: Modifiers::default() }), cx);
            if k >= 1. {
                glide = None;
                if std::mem::take(&mut held) {
                    send(PlatformInput::MouseUp(MouseUpEvent { button: MouseButton::Left, position: point(px(mx), px(my)), modifiers: Modifiers::default(), click_count: 1 }), cx);
                }
            }
            down |= held;
        }
        if bench {
            let _ = cx.update_window(h, |_, window, cx| {
                // (Whether anything since the last frame asked for this one: the app's window draws only then.)
                // (What waits for the next frame, a wheel's glide, steps here as at each of the app's frames; counted in
                // the frame's draw time, as it is part of it.)
                let t0 = std::time::Instant::now();
                window.simulate_next_frame(cx);
                asked.push(window.dirty_for_bench());
                let clear = window.draw(cx);
                let t1 = std::time::Instant::now();
                phases.push(Window::phases_for_bench());
                window.present_for_bench();
                let t2 = std::time::Instant::now();
                clear.clear(cx);
                window.present_for_bench();
                let t3 = std::time::Instant::now();
                times.push((t1 - t0, t2 - t1, t3 - t2));
            });
        } else {
            let pic = cx.update_window(h, |_, window, cx| {
                // Each picture is a frame of the window's own: the hidden window gets no frames from Windows, so
                // motion stepped frame by frame (request_animation_frame: springs, the capsule's slide and moment)
                // would stand still in the clip where it runs on in the app.
                // (BLOXKEEP_SHOT_CACHED=1: drawn as the app's frames are, the views kept where nothing they read
                // changed; else every view is made anew.)
                // What waits for the next frame (a wheel's glide) gets it here, as each frame of the app's does.
                window.simulate_next_frame(cx);
                if std::env::var("BLOXKEEP_SHOT_CACHED").is_ok() {
                    window.redraw();
                } else {
                    window.refresh();
                }
                window.draw(cx).clear(cx);
                window.render_to_image()
            });
            if let Ok(Ok(pic)) = pic {
                let _ = tx.send((PathBuf::from(format!("{}-{i:04}.png", stem.display())), pic));
            }
        }
        log.push_str(&format!("{t:.0} {mx:.1} {my:.1} {}{}\n", down as u8, if bench { format!(" {} {:.2} {}", asked.last().copied().unwrap_or(false) as u8, times.last().map_or(0., |t| t.0.as_secs_f64() * 1000.), phases.last().map_or(String::new(), |p| format!("{:.2} {:.2} {:.2} {:.2}", p[0].as_secs_f64() * 1000., p[1].as_secs_f64() * 1000., p[2].as_secs_f64() * 1000., p[3].as_secs_f64() * 1000.))) } else { String::new() }));
        down = false;
    }
    drop(tx);
    for w in writers {
        let _ = w.join();
    }
    if bench {
        let ms = |d: Duration| d.as_secs_f64() * 1000.;
        let stat = |mut v: Vec<f64>| {
            v.sort_by(f64::total_cmp);
            let at = |q: f64| v.get(((v.len() as f64 - 1.) * q).round() as usize).copied().unwrap_or(0.);
            format!("median {:.2} ms, p90 {:.2} ms, max {:.2} ms", at(0.5), at(0.9), at(1.))
        };
        let draw = stat(times.iter().map(|t| ms(t.0)).collect());
        let present = stat(times.iter().map(|t| ms(t.1)).collect());
        let again = stat(times.iter().map(|t| ms(t.2)).collect());
        let report = format!("frames: {}
draw: {draw}
present: {present}
present, nothing redrawn: {again}
asked for (the app's window would draw): {} of {}
", times.len(), asked.iter().filter(|a| **a).count(), asked.len());
        eprint!("{report}");
        let _ = std::fs::write(stem.with_extension("bench.txt"), report);
        let _ = std::fs::write(stem.with_extension("bench-frames.txt"), log);
        if let Some((path, p)) = prof {
            p.finish(&path);
        }
        return;
    }
    let _ = std::fs::write(stem.with_extension("txt"), log);
    eprintln!("shot: {n} frames, {} ms", start.elapsed().as_millis());
}

/// The hidden window takes the desktop's real mouse as its own (where the cursor happens to be on the screen), so a
/// tile under it showed its hover in the picture: the mouse is told it is far outside the window, so nothing is hovered.
#[cfg(feature = "shot")]
fn away(window: &mut Window, cx: &mut App) {
    let far = MouseMoveEvent { position: point(px(-4000.), px(-4000.)), pressed_button: None, modifiers: Modifiers::default() };
    window.dispatch_event(PlatformInput::MouseMove(far), cx);
}

/// Every thread the process starts, its own and Windows' and the libraries' alike, counted as Windows starts it (a TLS
/// callback runs for each, as a DLL's DllMain does with DLL_THREAD_ATTACH): what NVIDIA's driver keeps ~184 bytes for.
#[cfg(feature = "shot")]
mod started {
    use std::sync::atomic::{AtomicU64, Ordering};

    static N: AtomicU64 = AtomicU64::new(0);

    unsafe extern "system" fn on_thread(_: *mut std::ffi::c_void, reason: u32, _: *mut std::ffi::c_void) {
        // DLL_THREAD_ATTACH
        if reason == 2 {
            N.fetch_add(1, Ordering::Relaxed);
        }
    }

    #[used]
    #[unsafe(link_section = ".CRT$XLT")]
    static CALLBACK: unsafe extern "system" fn(*mut std::ffi::c_void, u32, *mut std::ffi::c_void) = on_thread;

    pub fn count() -> u64 {
        N.load(Ordering::Relaxed)
    }
}
