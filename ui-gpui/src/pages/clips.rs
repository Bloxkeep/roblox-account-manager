// A game tile plays the game's gameplay clip while the mouse rests on its picture, as a video site's thumbnails do
// (the owner, 2026-11-13). Only some games have one (Roblox's "GamePreviewVideo"); the others keep their picture.
//
// The mouse must stay a moment before anything is asked, so passing over a row of tiles fetches nothing. The clip is
// fetched once (~10 MB) and kept on this PC (cache\videos); it plays muted (video.rs), and stops when the mouse leaves.
use std::{collections::HashMap, time::Duration};

use gpui_kit::{prelude::*, *};

use crate::{images, roblox, shell::Root, video::{Failed, Video}};

/// How long the mouse rests on a picture before its clip is asked for.
const REST: Duration = Duration::from_millis(450);

enum Clip {
    /// Its clip is being looked for or fetched.
    Asking,
    Ready(Entity<Video>),
    /// The game has none (or it couldn't be had, or not played here): its picture stays.
    Missing,
}

/// The clips by place, and the tile the mouse rests on.
#[derive(Default)]
pub struct Clips {
    over: Option<i64>,
    known: HashMap<i64, Clip>,
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
    if r.games.clips.known.contains_key(&place) {
        cx.notify();
        return;
    }
    cx.spawn(async move |this, cx| {
        cx.background_executor().timer(REST).await;
        // Still resting on it, and nobody asked meanwhile.
        let ask = this
            .update(cx, |r, _| {
                let ask = r.games.clips.over == Some(place) && !r.games.clips.known.contains_key(&place);
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
        let clip = match got {
            Some(path) => {
                let video = cx.new(|cx| Video::new(path, cx));
                // Not playable on this PC (no decoder): the picture stays from then on.
                cx.subscribe(&video, move |r, _, _: &Failed, cx| {
                    r.games.clips.known.insert(place, Clip::Missing);
                    cx.notify();
                })
                .detach();
                Clip::Ready(video)
            }
            None => Clip::Missing,
        };
        r.games.clips.known.insert(place, clip);
        cx.notify();
    });
}

/// Asks once for the clip of the game at `place` with no mouse on it: the Games tab's spotlight plays its game's.
pub fn want(root: &Root, place: i64, cx: &mut Context<Root>) {
    if place <= 0 || root.games.clips.known.contains_key(&place) {
        return;
    }
    cx.spawn(async move |this, cx| {
        let ask = this
            .update(cx, |r, _| {
                let ask = !r.games.clips.known.contains_key(&place);
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

/// The clip to lay over the picture of the game at `place`, while the mouse rests on it and the clip is here.
pub fn playing(root: &Root, place: i64) -> Option<AnyElement> {
    if root.games.clips.over != Some(place) {
        return None;
    }
    match root.games.clips.known.get(&place) {
        Some(Clip::Ready(v)) => Some(div().absolute().inset_0().child(v.clone()).into_any_element()),
        _ => None,
    }
}

/// What a tile needs for its clip: the clip over its picture (while it plays) and the picture's mouse watch.
pub struct TileClip {
    pub layer: Option<AnyElement>,
    pub hover: Box<dyn Fn(&bool, &mut Window, &mut App) + 'static>,
}

pub fn tile_clip(root: &Root, place: i64, cx: &mut Context<Root>) -> TileClip {
    TileClip { layer: playing(root, place), hover: Box::new(cx.listener(move |r, over: &bool, _, cx| hover(r, place, *over, cx))) }
}
