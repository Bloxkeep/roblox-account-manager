// A game tile plays the game's gameplay clip while the mouse rests on its picture, as a video site's thumbnails do
// (the owner, 2026-11-13). Only some games have one (Roblox's "GamePreviewVideo"); the others keep their picture.
//
// The mouse must stay a moment before anything is asked, so passing over a row of tiles fetches nothing. The clip is
// fetched once (~10 MB) and kept on this PC (cache\videos); it plays with its sound (video.rs; a button on the clip
// turns sound off for all), and stops when the mouse leaves.
#[cfg(feature = "core")]
use crate::threads::Hold;
use std::{
    collections::{HashMap, VecDeque},
    time::{Duration, Instant},
};

use gpui_kit::{prelude::*, *};

use crate::{images, roblox, shell::Root, video::{Failed, Video}};

/// How long the mouse rests on a picture before its clip is asked for.
const REST: Duration = Duration::from_millis(450);
/// How long the mouse must be seen on a picture before its clip (one kept here) covers it. A "mouse came" with the
/// mouse elsewhere flashed the clip for a frame over the hero and the first tile while the mouse went about the back
/// button (the owner's video of 2026-11-19 05:47, ten times in 20 s): now the picture's own frame must see the mouse in
/// it, and that long, before the clip shows.
const SETTLE: Duration = Duration::from_millis(120);
/// The clips kept ready to play (each its last frame; its decoder only while it is drawn): the ones played last; an older one is made again
/// from its file on this PC when wanted.
const READY_MAX: usize = 4;
/// How long "no clip came" stands before the game may be asked again: Roblox's answer can't tell a game without a
/// clip from a failed asking, and a failure kept for the session hid a clip for good.
const MISSING_KEPT: Duration = Duration::from_secs(10 * 60);

enum Clip {
    /// Its clip is being looked for or fetched.
    Asking,
    Ready(Entity<Video>),
    /// The game has none (or it couldn't be had: then when, to ask again after MISSING_KEPT; None: not playable on
    /// this PC, for good): its picture stays.
    Missing(Option<Instant>),
}

/// The clips by place, and the tile the mouse rests on.
#[derive(Default)]
pub struct Clips {
    over: Option<i64>,
    known: HashMap<i64, Clip>,
    /// The places whose clip is Ready, the one played last at the back.
    ready: VecDeque<i64>,
    /// Whether a playing clip's picture had the mouse in it as last drawn, and whether that check is waiting to run:
    /// a tile that went (another page, a scroll) gets no "mouse left", so `over` would stay and the clip would play
    /// with no mouse on it when the tile came back.
    hit: std::rc::Rc<std::cell::Cell<bool>>,
    pending: std::rc::Rc<std::cell::Cell<bool>>,
    /// When `over` was set, and whether a drawn frame has seen the mouse in that game's picture since.
    since: Option<Instant>,
    sure: std::rc::Rc<std::cell::Cell<bool>>,
    /// The mouse and the picture's box as last checked (for the log line of a "mouse came" with the mouse elsewhere).
    seen: std::rc::Rc<std::cell::Cell<Option<(Point<Pixels>, Bounds<Pixels>)>>>,
}

impl Clips {
    /// Known, and not a "none came" that has stood long enough to be asked again.
    fn has(&self, place: i64) -> bool {
        match self.known.get(&place) {
            Some(Clip::Missing(Some(at))) => at.elapsed() < MISSING_KEPT,
            Some(_) => true,
            None => false,
        }
    }

    /// The clip of `place` played now: last to go.
    fn touch(&mut self, place: i64) {
        if let Some(i) = self.ready.iter().position(|p| *p == place) {
            self.ready.remove(i);
            self.ready.push_back(place);
        }
    }

    /// A clip ready: kept, and the one played longest ago let go past READY_MAX (never the one under the mouse).
    fn keep_ready(&mut self, place: i64, video: Entity<Video>) {
        self.known.insert(place, Clip::Ready(video));
        self.ready.retain(|p| *p != place);
        self.ready.push_back(place);
        while self.ready.len() > READY_MAX {
            let Some(i) = self.ready.iter().position(|p| Some(*p) != self.over) else { break };
            if let Some(old) = self.ready.remove(i) {
                self.known.remove(&old);
            }
        }
    }
}

/// The mouse came onto (or left) the picture of the game at `place`.
pub fn hover(r: &mut Root, place: i64, over: bool, cx: &mut Context<Root>) {
    if place <= 0 {
        return;
    }
    if !over {
        if r.games.clips.over == Some(place) {
            r.games.clips.over = None;
            r.games.clips.since = None;
            r.games.clips.sure.set(false);
            cx.notify();
        }
        return;
    }
    if r.games.clips.over != Some(place) {
        r.games.clips.over = Some(place);
        r.games.clips.since = Some(Instant::now());
        r.games.clips.sure.set(cfg!(feature = "shot"));
        // (Drawn again once settled: the clip shows then if the mouse is still seen on the picture.)
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SETTLE).await;
            let _ = this.update(cx, |r, cx| {
                if r.games.clips.over == Some(place) {
                    cx.notify();
                }
            });
        })
        .detach();
    }
    if r.games.clips.has(place) {
        r.games.clips.touch(place);
        cx.notify();
        return;
    }
    cx.spawn(async move |this, cx| {
        cx.background_executor().timer(REST).await;
        // Still resting on it, and nobody asked meanwhile.
        let ask = this
            .update(cx, |r, _| {
                let ask = r.games.clips.over == Some(place) && !r.games.clips.has(place);
                if ask {
                    r.games.clips.known.insert(place, Clip::Asking);
                }
                ask
            })
            .unwrap_or(false);
        if ask {
            fetch(this, place, cx).await;
        }
    })
    .detach();
}

/// Fetches the clip of the game at `place` (marked Asking already) and keeps what came.
async fn fetch(this: WeakEntity<Root>, place: i64, cx: &mut AsyncApp) {
    let dir = images::kept_dir("videos");
    let got = roblox::run(move || roblox::place_clip(place, &dir)).await.ok().flatten();
    let _ = this.update(cx, |r, cx| {
        match got {
            Some(path) => {
                let video = cx.new(|cx| Video::new(path, cx));
                // Not playable on this PC (no decoder): the picture stays from then on.
                cx.subscribe(&video, move |r, _, _: &Failed, cx| {
                    r.games.clips.ready.retain(|p| *p != place);
                    r.games.clips.known.insert(place, Clip::Missing(None));
                    cx.notify();
                })
                .detach();
                r.games.clips.keep_ready(place, video);
            }
            None => {
                r.games.clips.known.insert(place, Clip::Missing(Some(Instant::now())));
            }
        }
        cx.notify();
    });
}

/// The game whose tile the mouse rests on.
#[cfg(feature = "shot")]
pub fn resting(root: &Root) -> Option<i64> {
    root.games.clips.over
}

/// The clip to lay over the picture of the game at `place`, while the mouse rests on it and the clip is here. As it
/// is drawn it checks the mouse is still on some picture of that game; none has it: `over` is let go.
pub fn playing(root: &Root, place: i64, cx: &mut Context<Root>) -> Option<AnyElement> {
    let clips = &root.games.clips;
    if clips.over != Some(place) {
        return None;
    }
    // The watch runs while the mouse is said to be on the game, clip here or not: a "mouse came" with the mouse
    // elsewhere is let go at the next frame, before anything shows.
    let video = match clips.known.get(&place) {
        Some(Clip::Ready(v)) if clips.sure.get() && clips.since.is_some_and(|t| t.elapsed() >= SETTLE) => Some(v.clone()),
        _ => None,
    };
    let (hit, pending, sure, seen, this) = (clips.hit.clone(), clips.pending.clone(), clips.sure.clone(), clips.seen.clone(), cx.entity().downgrade());
    let watch = canvas(
        move |bounds, window, cx| {
            // (A hidden shot rests on a tile with no mouse at all.)
            if cfg!(feature = "shot") {
                return;
            }
            let mouse = window.mouse_position();
            if bounds.contains(&mouse) {
                hit.set(true);
            } else if !hit.get() {
                seen.set(Some((mouse, bounds)));
            }
            // Once a frame, after every picture of it was drawn.
            if !pending.replace(true) {
                cx.defer(move |cx| {
                    pending.set(false);
                    let inside = hit.replace(false);
                    let first = !sure.replace(inside);
                    let _ = this.update(cx, |r, cx| {
                        if r.games.clips.over != Some(place) {
                            return;
                        }
                        if !inside {
                            #[cfg(feature = "core")]
                            if first {
                                note_stray(place, seen.get());
                            }
                            r.games.clips.over = None;
                            r.games.clips.since = None;
                            cx.notify();
                        } else if first {
                            cx.notify();
                        }
                    });
                });
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .inset_0();
    // Sound off / on, in the clip's top right corner as a video site's preview has it, for every clip from then on.
    let speaker = video.as_ref().filter(|v| v.read(cx).has_sound()).map(|_| {
        let on = crate::video::sound_on();
        let tip: SharedString = root.tr(if on { "clip.mute" } else { "clip.unmute" }).into();
        div()
            .id(("clip-sound", place as u64))
            .absolute()
            .top(px(8.))
            .right(px(8.))
            .size(px(30.))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .bg(gpui_kit::hsla(0., 0., 0., 0.55))
            .cursor_pointer()
            .hover(|d| d.bg(gpui_kit::hsla(0., 0., 0., 0.75)))
            .child(crate::kit::icon(if on { "volume" } else { "volumeOff" }, 16., gpui_kit::hsla(0., 0., 1., 0.95)))
            .tooltip(move |_, cx| cx.new(|_| crate::kit::Tip(tip.clone())).into())
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(|_, _, _, cx| {
                crate::video::set_sound(!crate::video::sound_on());
                cx.stop_propagation();
                cx.notify();
            }))
    });
    Some(div().absolute().inset_0().children(video).child(watch).children(speaker).into_any_element())
}

/// What a tile needs for its clip: the clip over its picture (while it plays) and the picture's mouse watch.
pub struct TileClip {
    pub layer: Option<AnyElement>,
    pub hover: Box<dyn Fn(&bool, &mut Window, &mut App) + 'static>,
}

pub fn tile_clip(root: &Root, place: i64, cx: &mut Context<Root>) -> TileClip {
    TileClip { layer: playing(root, place, cx), hover: Box::new(cx.listener(move |r, over: &bool, _, cx| hover(r, place, *over, cx))) }
}

/// A "mouse came onto the picture" with the mouse not on it as drawn: written to error.log (once a minute at most), so
/// where those come from can be found in a real log.
#[cfg(feature = "core")]
fn note_stray(place: i64, seen: Option<(Point<Pixels>, Bounds<Pixels>)>) {
    use std::sync::Mutex;
    static LAST: Mutex<Option<Instant>> = Mutex::new(None);
    let mut last = LAST.hold();
    if last.is_some_and(|t| t.elapsed() < Duration::from_secs(60)) {
        return;
    }
    *last = Some(Instant::now());
    let at = seen.map(|(m, b)| format!("mouse {:.0},{:.0}, picture {:.0},{:.0} {:.0}x{:.0}", f32::from(m.x), f32::from(m.y), f32::from(b.origin.x), f32::from(b.origin.y), f32::from(b.size.width), f32::from(b.size.height))).unwrap_or_else(|| "mouse not seen".into());
    crate::core::log::note_file(&format!("Game clip: a mouse-over for place {place} with the mouse off its picture ({at}); not shown"));
}
