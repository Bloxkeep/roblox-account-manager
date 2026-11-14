// A game tile plays the game's gameplay clip while the mouse rests on its picture, as a video site's thumbnails do
// (the owner, 2026-11-13). Only some games have one (Roblox's "GamePreviewVideo"); the others keep their picture.
//
// The mouse must stay a moment before anything is asked, so passing over a row of tiles fetches nothing. The clip is
// fetched once (~10 MB) and kept on this PC (cache\videos); it plays muted (video.rs), and stops when the mouse leaves.
use std::{
    collections::{HashMap, VecDeque},
    time::{Duration, Instant},
};

use gpui_kit::{prelude::*, *};

use crate::{images, roblox, shell::Root, video::{Failed, Video}};

/// How long the mouse rests on a picture before its clip is asked for.
const REST: Duration = Duration::from_millis(450);
/// The clips kept ready to play (each a decoder and its frames): the ones played last; an older one is made again
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
            cx.notify();
        }
        return;
    }
    r.games.clips.over = Some(place);
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

/// Asks once for the clip of the game at `place` with no mouse on it: the Games tab's spotlight plays its game's.
pub fn want(root: &Root, place: i64, cx: &mut Context<Root>) {
    if place <= 0 || root.games.clips.has(place) {
        return;
    }
    cx.spawn(async move |this, cx| {
        let ask = this
            .update(cx, |r, _| {
                let ask = !r.games.clips.has(place);
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

/// The clip of the game at `place`, to lay over its picture, once it is here (whatever the mouse does).
pub fn ready(root: &Root, place: i64) -> Option<AnyElement> {
    match root.games.clips.known.get(&place) {
        Some(Clip::Ready(v)) => Some(div().absolute().inset_0().child(v.clone()).into_any_element()),
        _ => None,
    }
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
    let Some(Clip::Ready(v)) = clips.known.get(&place) else { return None };
    let (hit, pending, this) = (clips.hit.clone(), clips.pending.clone(), cx.entity().downgrade());
    let watch = canvas(
        move |bounds, window, cx| {
            // (A hidden shot rests on a tile with no mouse at all.)
            if cfg!(feature = "shot") {
                return;
            }
            if bounds.contains(&window.mouse_position()) {
                hit.set(true);
            }
            // Once a frame, after every picture of it was drawn.
            if !pending.replace(true) {
                cx.defer(move |cx| {
                    pending.set(false);
                    if !hit.replace(false) {
                        let _ = this.update(cx, |r, cx| {
                            if r.games.clips.over == Some(place) {
                                r.games.clips.over = None;
                                cx.notify();
                            }
                        });
                    }
                });
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .inset_0();
    Some(div().absolute().inset_0().child(v.clone()).child(watch).into_any_element())
}

/// What a tile needs for its clip: the clip over its picture (while it plays) and the picture's mouse watch.
pub struct TileClip {
    pub layer: Option<AnyElement>,
    pub hover: Box<dyn Fn(&bool, &mut Window, &mut App) + 'static>,
}

pub fn tile_clip(root: &Root, place: i64, cx: &mut Context<Root>) -> TileClip {
    TileClip { layer: playing(root, place, cx), hover: Box::new(cx.listener(move |r, over: &bool, _, cx| hover(r, place, *over, cx))) }
}
