// Avatar, as the Roblox app has it, inside Games (its header's switch): the avatar of the account Friends looks
// through (the same pick, pages/friends.rs): its full picture, what it wears (take one off), its saved outfits
// (wear one) and its inventory by shelf (put one thing on); and, what Roblox has not, its history: every look this PC
// has seen it in is kept (roblox.rs), and any of them can be put on again. Reading it needs no sign-in (Roblox shows avatars to anyone); changing it is asked from here (roblox.rs)
// with that account's own sign-in, lent by the app. Roblox draws the new picture a moment after a change, so the
// avatar is read again then.
use gpui_kit::{prelude::*, *};
use crate::ask::{found, reply};

use crate::{
    images,
    kit::*,
    pages::friends,
    roblox::{self, Avatar, Owned, Price, Worth},
    shell::Root,
    theme::*,
};

#[derive(Default)]
pub struct Data {
    /// The Roblox user this avatar is of, and the avatar (None while the first one loads).
    of: i64,
    pub got: Option<Result<Avatar, String>>,
    loading: bool,
    /// The item or outfit a change is under way for, and why Roblox turned the last one down.
    busy: Option<i64>,
    pub refused: Option<String>,
    /// The inventory: the shelf shown, what it holds so far (None while its first page loads), the mark for its next
    /// page, and the shelf a page is being read for.
    pub shelf: usize,
    owned: Option<Result<Vec<Owned>, String>>,
    next: String,
    reading: Option<usize>,
    /// What the worn things are worth (None until Roblox has said), the things it was asked for, and whether an
    /// asking is under way.
    worth: Option<Worth>,
    worth_of: Vec<i64>,
    pricing: bool,
    /// The avatar with depth (mesh.rs): Roblox's model of it once asked for, the way it is turned, the picture drawn
    /// of it, whether one is being drawn (and another is wanted after it), and a drag under way (where it began, and
    /// the turn then).
    asked_solid: bool,
    solid_ask: u64,
    solid: Option<std::sync::Arc<crate::mesh::Model>>,
    /// Roblox gave no model (no sign-in, or it failed): the flat picture stays.
    no_solid: bool,
    /// How it is looked at (mesh::View): turned, the camera raised, and how much nearer than the whole avatar
    /// (0; up to 1 - ZOOM_NEAR, below 0 farther). The room is read from the kept choice when the page first shows.
    yaw: f32,
    pitch: f32,
    near: f32,
    room: Option<usize>,
    background: Option<Owned>,
    profile_background: Option<Owned>,
    profile_background_checked: bool,
    profile_background_ask: Option<u64>,
    studio_override: bool,
    /// Turning by itself (kept between runs), the turning under way (its run, so an old one stops) and whether one is.
    /// The same run moves it in its idle (`moving`, kept too; `t` how far into it), when its parts were found.
    spin: bool,
    moving: bool,
    t: f32,
    /// Its walk (mesh::View): the keys held on the 3D box (1 W, 2 S, 4 A, 8 D, 16 Shift to run), where it has walked
    /// to, the way it faces, its stride and how much it is walking; a jump asked for (Space) and the one under way.
    keys: u8,
    jump: bool,
    up: f32,
    rise: f32,
    /// "Save as outfit" on its way; the outfit whose menu is open, and the one whose Delete waits for a second click.
    saving_look: bool,
    outfit_menu: Option<i64>,
    outfit_armed: Option<i64>,
    at: [f32; 2],
    heading: f32,
    stride: f32,
    pace: f32,
    spin_run: u64,
    spinning: bool,
    /// Shown big, filling the page with its controls beside it.
    big: bool,
    /// Turned by hand once: the hint over it goes.
    turned: bool,
    /// A picture of it being saved.
    saving: bool,
    frame: Option<std::sync::Arc<RenderImage>>,
    /// What `frame` shows (the view, its size, the model): a draw asked for the same is skipped.
    frame_of: Option<(crate::mesh::View, usize, usize)>,
    drawing: bool,
    stale: bool,
    /// The model shown is the one before a change: Roblox's new one is on its way.
    swapping: bool,
    /// Roblox's address of the model shown (empty when not known): after a change, the one not to take again.
    solid_file: String,
    /// A drag under way: where it began, and the turn and the camera's height then.
    drag: Option<(f32, f32, f32, f32)>,
    /// One thing changed in many saved outfits at once (open under Saved outfits).
    swap: Option<Swap>,
    /// The ticket of the avatar's read and of the shelf's page under way (`ticket`): an answer with another is old.
    load_ask: u64,
    shelf_ask: u64,
}

/// A number no other ask has had (this page's and Profile's): an answer is taken only while its ask is the latest, so
/// one that was overtaken (a shelf left and picked again, an account switched and back) is dropped.
pub(crate) fn ticket() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

impl Data {
    /// Put away with its tab (pages/tabs.rs): nothing is on its way any more, so what is missing is asked again.
    pub fn rest(&mut self) {
        self.loading = false;
        self.busy = None;
        self.reading = None;
        self.profile_background_ask = None;
        self.solid_ask = 0;
        // (So is a model awaited after a change: the one before it would stay up for good.)
        if (self.solid.is_none() && !self.no_solid) || self.swapping {
            self.asked_solid = false;
        }
        self.pricing = false;
        // (A picture being drawn is dropped too: another is drawn when the tab is back; so is the turning.)
        self.drawing = false;
        self.stale = false;
        self.spin_run += 1;
        self.spinning = false;
        self.drag = None;
    }
}

/// What can be done to the avatar from here.
#[derive(Clone)]
enum Change {
    Off(i64),
    Outfit(i64),
    On(i64, String),
    Rig(bool),
    Colour(String),
    Scale(usize, f64),
    /// Back to a look of the history, by its place in it.
    Look(usize),
    /// An emote off the wheel: the emote, and its place there.
    EmoteOff(i64, i64),
}

const TILE: f32 = 150.;
const TGAP: f32 = 14.;

pub fn want(r: &mut Root, link: &crate::link::Link, cx: &mut Context<Root>) {
    friends::want_sign(r, link, cx);
    let Some((_, user, _)) = friends::signed(r) else { return };
    // Another account: what was shown (and on its way) for the last one goes at once, even mid-load.
    if r.avatar.of != user {
        // (The old picture leaves GPUI's sprite atlas too, as a replaced one does in draw().)
        if let Some(old) = std::mem::replace(&mut r.avatar, Data { of: user, ..Default::default() }).frame {
            cx.drop_image(old, None);
        }
    }
    if r.avatar.got.is_none() && !r.avatar.loading {
        load(r, cx);
    }
    if !r.avatar.profile_background_checked && r.avatar.profile_background_ask.is_none() {
        load_profile_background(r, cx);
    }
    if r.avatar.of == user && r.avatar.owned.is_none() && r.avatar.reading.is_none() {
        shelf_page(r, cx);
    }
    if r.avatar.room.is_none() {
        r.avatar.room = Some(kept_room());
        r.avatar.spin = crate::prefs::get("avatarSpin").and_then(|v| v.as_bool()).unwrap_or(false);
        r.avatar.moving = crate::prefs::get("avatarMove").and_then(|v| v.as_bool()).unwrap_or(true);
        // BLOXKEEP_SHOT_AV3="big room=day pitch=0.4 near=0.3 turned": a hidden shot's 3D view, set at once.
        #[cfg(feature = "shot")]
        if let Ok(v) = std::env::var("BLOXKEEP_SHOT_AV3") {
            for w in v.split_whitespace() {
                let (k, x) = w.split_once('=').unwrap_or((w, ""));
                match k {
                    "big" => r.avatar.big = true,
                    "turned" => r.avatar.turned = true,
                    "room" => r.avatar.room = crate::mesh::ROOMS.iter().position(|m| m.id == x).or(r.avatar.room),
                    "pitch" => r.avatar.pitch = x.parse().unwrap_or(0.),
                    "near" => r.avatar.near = x.parse().unwrap_or(0.),
                    "t" => r.avatar.t = x.parse().unwrap_or(0.),
                    // walk=1.2: caught mid-stride (that far into it), facing heading=…
                    "walk" => {
                        r.avatar.pace = 1.;
                        r.avatar.stride = x.parse().unwrap_or(1.2);
                    }
                    "heading" => r.avatar.heading = x.parse().unwrap_or(0.),
                    "still" => r.avatar.moving = false,
                    _ => {}
                }
            }
        }
    }
    // (The model is asked beside the avatar's read, not after it: the 3D box was the last thing in, seconds late.)
    if r.avatar.of == user && !r.avatar.asked_solid && !matches!(r.avatar.got, Some(Err(_))) {
        solid(r, cx);
    }
    // BLOXKEEP_SHOT_SWAP=1: a hidden shot goes through what a change does (R6 to R15): with a picture on its way, the
    // model is asked for again and replaced, and four seconds on the box must still take a new picture. What it finds
    // is printed ("[swap] ..."): the box froze here from 1.1.2-alpha.9 to alpha.12.
    #[cfg(feature = "shot")]
    if std::env::var("BLOXKEEP_SHOT_SWAP").is_ok() && r.avatar.solid.is_some() && r.avatar.frame.is_some() && !r.avatar.drawing {
        static ONCE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if !ONCE.swap(true, std::sync::atomic::Ordering::Relaxed) {
            r.avatar.yaw += 0.5;
            draw(r, cx);
            MODELS.with(|m| m.borrow_mut().remove(&user));
            r.avatar.asked_solid = true;
            r.avatar.solid_ask = 0;
            r.avatar.swapping = true;
            after_change(r, false, cx);
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(std::time::Duration::from_secs(4)).await;
                let before = this.update(cx, |r, cx| {
                    let before = r.avatar.frame.clone();
                    r.avatar.yaw += 0.7;
                    draw(r, cx);
                    before
                });
                cx.background_executor().timer(std::time::Duration::from_secs(2)).await;
                let _ = this.update(cx, |r, _| {
                    let new_picture = match (before.ok().flatten(), r.avatar.frame.as_ref()) {
                        (Some(a), Some(b)) => !std::sync::Arc::ptr_eq(&a, b),
                        _ => false,
                    };
                    eprintln!("[swap] drawing still set: {}; still swapping: {}; model there: {}; a new picture drawn after the swap: {new_picture}", r.avatar.drawing, r.avatar.swapping, r.avatar.solid.is_some());
                });
            })
            .detach();
        }
    }
    // Back on the tab: the picture it dropped is drawn again, and the turning goes on.
    if r.avatar.solid.is_some() && r.avatar.frame.is_none() && !r.avatar.drawing {
        draw(r, cx);
    }
    if lively(&r.avatar) && !r.avatar.spinning {
        start_spin(r, cx);
    }
    price(r, cx);
}

/// The room picked last time (ROOMS' id), else the studio.
fn kept_room() -> usize {
    let id = crate::prefs::get("avatarRoom").and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
    crate::mesh::ROOMS.iter().position(|r| r.id == id).unwrap_or(1)
}

fn room_of(av: &Data) -> usize {
    av.room.unwrap_or(1).min(crate::mesh::ROOMS.len() - 1)
}

fn background_of(av: &Data) -> Option<&Owned> {
    av.background.as_ref().or(av.profile_background.as_ref().filter(|_| !av.studio_override))
}

fn load_profile_background(r: &mut Root, cx: &mut Context<Root>) {
    let user = r.avatar.of;
    let ask = ticket();
    r.avatar.profile_background_ask = Some(ask);
    let rx = roblox::run(move || roblox::profile_background(user));
    found(rx, cx, move |r, background, cx| {
        let av = &mut r.avatar;
        if av.of != user || av.profile_background_ask != Some(ask) {
            return;
        }
        av.profile_background_ask = None;
        av.profile_background_checked = true;
        let before = background_of(av).map(|asset| asset.id);
        if let Some(background) = background {
            av.profile_background = background;
        }
        if before != background_of(av).map(|asset| asset.id) {
            draw(r, cx);
        }
        cx.notify();
    });
}

fn view_of(av: &Data) -> crate::mesh::View {
    crate::mesh::View {
        yaw: av.yaw,
        pitch: av.pitch,
        zoom: 1. - av.near,
        room: if background_of(av).is_some() { 0 } else { room_of(av) },
        t: if av.moving { av.t } else { 0. },
        at: av.at,
        heading: av.heading,
        stride: av.stride,
        pace: av.pace,
        up: av.up,
        rise: av.rise,
    }
}

/// Whether it is in the air, or about to be.
fn airborne(av: &Data) -> bool {
    av.jump || av.up > 0. || av.rise > 0.
}

/// Whether the avatar's model can move in its idle.
fn moves(av: &Data) -> bool {
    av.solid.as_ref().is_some_and(|m| m.moves())
}

/// Whether its run should go: turning, or moving in its idle.
fn lively(av: &Data) -> bool {
    av.solid.is_some() && (av.spin || (moves(av) && (av.moving || av.keys & 15 != 0 || av.pace > 0. || airborne(av))))
}

/// A walking key on the 3D box: its bit in `keys` (W or ↑ 1, S or ↓ 2, A or ← 4, D or → 8, Shift 16: it runs).
fn walk_key(key: &str) -> Option<u8> {
    match key {
        "w" | "up" => Some(1),
        "s" | "down" => Some(2),
        "a" | "left" => Some(4),
        "d" | "right" => Some(8),
        "shift" => Some(16),
        _ => None,
    }
}

/// The side of the square the avatar is drawn into, in dots (the box is 312 wide, 468 dots at 150 %), and when it is
/// shown big (up to 760 wide, 1140 dots at 150 %).
const STAGE: usize = 560;
const STAGE_BIG: usize = 1120;
const STAGE_MOVING: usize = 448;
const STAGE_BIG_MOVING: usize = 704;
/// The side of a saved picture, in dots.
const SAVED: usize = 1600;
thread_local! {
    /// Each account's avatar model read this run (by its Roblox user), with Roblox's address of it.
    static MODELS: std::cell::RefCell<std::collections::HashMap<i64, Kept>> = Default::default();
    /// The last models by what they are of (the user, and roblox::Avatar::key: body and look): R6 to R15 and back, or
    /// a look worn again, shows at once instead of waiting for Roblox to make the same model anew.
    static LOOK_MODELS: std::cell::RefCell<Vec<(i64, String, Kept)>> = Default::default();
}

/// A model and Roblox's address of it.
type Kept = (std::sync::Arc<crate::mesh::Model>, String);

/// How many models LOOK_MODELS keeps (each holds its pictures whole, megabytes).
const LOOK_MODELS_KEPT: usize = 4;

/// The model shown is the avatar's as read: kept under what it is of.
fn remember(r: &Root) {
    let av = &r.avatar;
    let (Some(model), Some(Ok(a)), false) = (&av.solid, &av.got, av.swapping) else { return };
    let Some(key) = a.key() else { return };
    LOOK_MODELS.with(|l| {
        let mut l = l.borrow_mut();
        l.retain(|(user, k, _)| !(*user == av.of && *k == key));
        l.push((av.of, key, (model.clone(), av.solid_file.clone())));
        if l.len() > LOOK_MODELS_KEPT {
            l.remove(0);
        }
    });
}

/// The avatar was read after a change: its model is one kept (shown at once), or Roblox is asked for it.
fn after_change(r: &mut Root, read: bool, cx: &mut Context<Root>) {
    let user = r.avatar.of;
    let key = r.avatar.got.as_ref().and_then(|g| g.as_ref().ok()).filter(|_| read).and_then(|a| a.key());
    let kept = key.and_then(|key| LOOK_MODELS.with(|l| l.borrow().iter().find(|(u, k, _)| *u == user && *k == key).map(|(_, _, kept)| kept.clone())));
    let Some((model, file)) = kept else {
        solid(r, cx);
        return;
    };
    MODELS.with(|m| m.borrow_mut().insert(user, (model.clone(), file.clone())));
    r.avatar.solid = Some(model);
    r.avatar.solid_file = file;
    r.avatar.solid_ask = 0;
    r.avatar.no_solid = false;
    r.avatar.swapping = false;
    draw(r, cx);
    if lively(&r.avatar) && !r.avatar.spinning {
        start_spin(r, cx);
    }
}

/// How far round the avatar stands when it first shows; it turns to face you from there.
const TURN_IN: f32 = -0.9;

/// Roblox's model of the avatar, asked once it shows and again after a change. With it the picture can be turned
/// (a drag on it); without it (no sign-in, or Roblox gives none) the flat picture stays.
fn solid(r: &mut Root, cx: &mut Context<Root>) {
    let Some((_, user, cookie)) = friends::signed(r) else { return };
    r.avatar.asked_solid = true;
    let ask = ticket();
    r.avatar.solid_ask = ask;
    let shot = cfg!(feature = "shot") && std::env::var("BLOXKEEP_SHOT_MODEL").is_ok();
    if cookie.is_empty() && !shot {
        r.avatar.no_solid = true;
        return;
    }
    // BLOXKEEP_SHOT_MODEL=blocks: a plain model of boxes (mesh::blocks), for a hidden shot without Roblox.
    let blocks = cfg!(feature = "shot") && std::env::var("BLOXKEEP_SHOT_MODEL").is_ok_and(|v| v == "blocks");
    // (Each account's model is kept for the run: coming back to the page, or to the account, shows it at once; Roblox
    // is asked again only after a change of look. "loading khá lâu", the owner, 2026-11-15.)
    let kept = MODELS.with(|m| m.borrow().get(&user).cloned());
    // (After a change: the model before it, not to be taken for the new one.)
    let after = Some(r.avatar.solid_file.clone()).filter(|f| r.avatar.swapping && !f.is_empty());
    let rx = roblox::run(move || -> Option<Kept> {
        if let Some(m) = kept {
            return Some(m);
        }
        #[cfg(feature = "shot")]
        if blocks {
            return crate::mesh::blocks().map(|m| (std::sync::Arc::new(m), String::new()));
        }
        let _ = blocks;
        let s = roblox::solid(&cookie, user, after.as_deref()).ok()?;
        let model = crate::mesh::Model::parse(&s.obj, &s.mtl, &s.pictures, s.eye, s.min, s.max);
        if model.is_none() {
            crate::net::note_failed("Avatar 3D", &s.file, &format!("Roblox's model couldn't be read ({} bytes, {} pictures)", s.obj.len(), s.pictures.len()));
        }
        Some((std::sync::Arc::new(model?), s.file))
    });
    cx.spawn(async move |this, cx| {
        let Some((model, file)) = rx.await.ok().flatten() else {
            let _ = this.update(cx, |r, cx| {
                if r.avatar.of == user && r.avatar.solid_ask == ask {
                    // (After a change, the model before it would show a look no longer worn.)
                    if std::mem::take(&mut r.avatar.swapping) {
                        reload_model(r, cx);
                        r.avatar.asked_solid = true;
                    }
                    r.avatar.no_solid = true;
                    cx.notify();
                }
            });
            return;
        };
        // BLOXKEEP_SHOT_YAW=1.2: a hidden shot's avatar stands turned that far, at once.
        let fixed: Option<f32> = if cfg!(feature = "shot") { std::env::var("BLOXKEEP_SHOT_YAW").ok().and_then(|y| y.parse().ok()) } else { None };
        // A model that replaces one (after a change) stands as the old one did; the first turns in to face you.
        let first = this
            .update(cx, |r, cx| {
                if r.avatar.of != user || r.avatar.solid_ask != ask {
                    return false;
                }
                let first = r.avatar.solid.is_none();
                // (Three at most: a model holds its pictures whole, megabytes each.)
                MODELS.with(|m| {
                    let mut m = m.borrow_mut();
                    if m.len() >= 3 && !m.contains_key(&user) {
                        m.clear();
                    }
                    m.insert(user, (model.clone(), file.clone()));
                });
                r.avatar.solid = Some(model);
                r.avatar.solid_file = file;
                r.avatar.no_solid = false;
                r.avatar.swapping = false;
                remember(r);
                if let Some(y) = fixed {
                    r.avatar.yaw = y;
                } else if first {
                    r.avatar.yaw = TURN_IN;
                }
                draw(r, cx);
                // (A first one turns in to face you first; the turning starts after.)
                if !(first && fixed.is_none()) && lively(&r.avatar) {
                    start_spin(r, cx);
                }
                first && fixed.is_none()
            })
            .unwrap_or(false);
        if !first {
            return;
        }
        for i in 1..=26 {
            cx.background_executor().timer(std::time::Duration::from_millis(22)).await;
            let k = i as f32 / 26.;
            let on = this
                .update(cx, |r, cx| {
                    if r.avatar.of != user || r.avatar.solid_ask != ask || r.avatar.drag.is_some() {
                        return false;
                    }
                    r.avatar.yaw = TURN_IN * (1. - k).powi(3);
                    draw(r, cx);
                    true
                })
                .unwrap_or(false);
            if !on {
                break;
            }
        }
        let _ = this.update(cx, |r, cx| {
            if r.avatar.of == user && r.avatar.solid_ask == ask && lively(&r.avatar) {
                start_spin(r, cx);
            }
        });
    })
    .detach();
}

/// Turns the avatar by itself, a full turn in about ten seconds, while `spin` is on and the tab shows it (a drag holds
/// it), and moves it in its idle while `moving` is. Off the window's thread like any picture of it, one at a time;
/// none is drawn while Bloxkeep's window isn't the one in use (Roblox played beside it keeps the processor).
fn start_spin(r: &mut Root, cx: &mut Context<Root>) {
    if r.avatar.spinning || !lively(&r.avatar) {
        return;
    }
    r.avatar.spinning = true;
    r.avatar.spin_run += 1;
    let (run, user) = (r.avatar.spin_run, r.avatar.of);
    cx.spawn(async move |this, cx| {
        let mut previous = std::time::Instant::now();
        loop {
            cx.background_executor().timer(std::time::Duration::from_millis(16)).await;
            let now = std::time::Instant::now();
            let dt = now.duration_since(previous).as_secs_f32().min(0.05);
            previous = now;
            let window = this.update(cx, |_, cx| cx.windows().first().copied()).ok().flatten();
            let (active, keyed) = window
                .and_then(|h| {
                    let me = this.clone();
                    cx.update_window(h, move |_, w, cx| (w.is_window_active(), me.read_with(cx, |r, _| r.avatar_focus.is_focused(w)).unwrap_or(false))).ok()
                })
                .unwrap_or((true, true));
            let on = this
                .update(cx, |r, cx| {
                    if r.avatar.of != user || r.avatar.spin_run != run || !lively(&r.avatar) {
                        if r.avatar.spin_run == run {
                            r.avatar.spinning = false;
                        }
                        return false;
                    }
                    // (Keys held when the box lost its focus let go: no key-up comes then.)
                    if !keyed && r.avatar.keys != 0 {
                        r.avatar.keys = 0;
                    }
                    // (Still while the window is behind, unless Settings says it goes on.)
                    if !active && crate::kit::still_behind() && !crate::kit::mouse_over_lately() {
                        return true;
                    }
                    let mut changed = false;
                    // (A hidden shot holds its stride where it was put.)
                    if r.avatar.keys & 15 != 0 || airborne(&r.avatar) || (r.avatar.pace > 0. && !cfg!(feature = "shot")) {
                        if let Some(model) = r.avatar.solid.clone() {
                            let k = r.avatar.keys;
                            let ahead = f32::from(k & 1 != 0) - f32::from(k & 2 != 0);
                            let right = f32::from(k & 8 != 0) - f32::from(k & 4 != 0);
                            let mut v = view_of(&r.avatar);
                            model.step(&mut v, ahead, right, k & 16 != 0, std::mem::take(&mut r.avatar.jump), dt);
                            (r.avatar.at, r.avatar.heading, r.avatar.stride, r.avatar.pace) = (v.at, v.heading, v.stride, v.pace);
                            (r.avatar.up, r.avatar.rise) = (v.up, v.rise);
                            changed = true;
                        }
                    }
                    if r.avatar.spin && r.avatar.drag.is_none() {
                        r.avatar.yaw = (r.avatar.yaw + 0.625 * dt).rem_euclid(std::f32::consts::TAU);
                        changed = true;
                    }
                    if r.avatar.moving && moves(&r.avatar) {
                        r.avatar.t += dt;
                        changed = true;
                    }
                    if changed {
                        draw(r, cx);
                    }
                    true
                })
                .unwrap_or(false);
            if !on {
                break;
            }
        }
    })
    .detach();
}

fn set_spin(r: &mut Root, on: bool, cx: &mut Context<Root>) {
    r.avatar.spin = on;
    crate::prefs::set("avatarSpin", serde_json::Value::Bool(on));
    start_spin(r, cx);
    cx.notify();
}

/// Its idle on or off (kept); off, it stands as at rest.
fn set_moving(r: &mut Root, on: bool, cx: &mut Context<Root>) {
    r.avatar.moving = on;
    crate::prefs::set("avatarMove", serde_json::Value::Bool(on));
    start_spin(r, cx);
    draw(r, cx);
    cx.notify();
}

fn set_room(r: &mut Root, i: usize, cx: &mut Context<Root>) {
    r.avatar.background = None;
    r.avatar.studio_override = true;
    r.avatar.room = Some(i);
    crate::prefs::set("avatarRoom", serde_json::Value::String(crate::mesh::ROOMS[i].id.into()));
    draw(r, cx);
    cx.notify();
}

fn set_big(r: &mut Root, on: bool, cx: &mut Context<Root>) {
    r.avatar.big = on;
    draw(r, cx);
    cx.notify();
}

/// Back to how it first stood: facing you, the whole of it, eased there.
fn reset_view(r: &mut Root, cx: &mut Context<Root>) {
    let tau = std::f32::consts::TAU;
    // (The short way round.)
    let y0 = (r.avatar.yaw + std::f32::consts::PI).rem_euclid(tau) - std::f32::consts::PI;
    let (p0, n0, user) = (r.avatar.pitch, r.avatar.near, r.avatar.of);
    let (a0, h0) = (r.avatar.at, (r.avatar.heading + std::f32::consts::PI).rem_euclid(tau) - std::f32::consts::PI);
    r.avatar.keys = 0;
    r.avatar.pace = 0.;
    (r.avatar.jump, r.avatar.up, r.avatar.rise) = (false, 0., 0.);
    r.avatar.spin_run += 1;
    r.avatar.spinning = false;
    cx.spawn(async move |this, cx| {
        for i in 1..=18 {
            cx.background_executor().timer(std::time::Duration::from_millis(16)).await;
            let k = 1. - (1. - i as f32 / 18.).powi(3);
            let on = this
                .update(cx, |r, cx| {
                    if r.avatar.of != user || r.avatar.drag.is_some() {
                        return false;
                    }
                    r.avatar.yaw = y0 * (1. - k);
                    r.avatar.pitch = p0 * (1. - k);
                    r.avatar.near = n0 * (1. - k);
                    r.avatar.at = [a0[0] * (1. - k), a0[1] * (1. - k)];
                    r.avatar.heading = h0 * (1. - k);
                    draw(r, cx);
                    true
                })
                .unwrap_or(false);
            if !on {
                break;
            }
        }
        let _ = this.update(cx, |r, cx| {
            if r.avatar.of == user && lively(&r.avatar) {
                start_spin(r, cx);
            }
        });
    })
    .detach();
}

/// A picture of the avatar as it is looked at now, big, saved in the Pictures folder (Bloxkeep's own folder in it) and
/// shown there.
fn save_picture(r: &mut Root, cx: &mut Context<Root>) {
    let Some(model) = r.avatar.solid.clone() else { return };
    if r.avatar.saving {
        return;
    }
    let background = if let Some(selected) = background_of(&r.avatar) {
        let Some(background) = images::pic(&selected.pic, cx) else {
            r.toast = Some((r.tr("av3.backgroundLoading"), "bad".into()));
            cx.notify();
            return;
        };
        Some(background)
    } else { None };
    r.avatar.saving = true;
    cx.notify();
    let (view, user) = (view_of(&r.avatar), r.avatar.of);
    let rx = roblox::run(move || -> Option<std::path::PathBuf> {
        let mut dots = model.draw(view, SAVED);
        for p in dots.chunks_mut(4) {
            p.swap(0, 2);
        }
        let mut picture = image::RgbaImage::from_raw(SAVED as u32, SAVED as u32, dots)?;
        if let Some(background) = background {
            let mut backdrop = image::load_from_memory(&background.bytes).ok()?
                .resize_to_fill(SAVED as u32, SAVED as u32, image::imageops::FilterType::Lanczos3).to_rgba8();
            image::imageops::overlay(&mut backdrop, &picture, 0, 0);
            picture = backdrop;
        }
        let dir = pictures_folder()?.join("Bloxkeep");
        std::fs::create_dir_all(&dir).ok()?;
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let file = dir.join(format!("avatar-{user}-{secs}.png"));
        picture.save(&file).ok()?;
        Some(file)
    });
    cx.spawn(async move |this, cx| {
        let file = rx.await.ok().flatten();
        let _ = this.update(cx, |r, cx| {
            r.avatar.saving = false;
            match &file {
                Some(f) => {
                    r.toast = Some((r.tr("av3.saved"), "ok".into()));
                    #[cfg(windows)]
                    {
                        use std::os::windows::process::CommandExt;
                        let _ = std::process::Command::new("explorer.exe").raw_arg(format!("/select,\"{}\"", f.display())).spawn();
                    }
                }
                None => r.toast = Some((r.tr("av3.saveFailed"), "bad".into())),
            }
            cx.notify();
        });
    })
    .detach();
}

/// The user's Pictures folder, wherever Windows keeps it (OneDrive's, when it has moved it there).
#[cfg(windows)]
fn pictures_folder() -> Option<std::path::PathBuf> {
    use windows_sys::Win32::System::Com::CoTaskMemFree;
    use windows_sys::Win32::UI::Shell::{FOLDERID_Pictures, SHGetKnownFolderPath};
    // SAFETY: the path Windows gives is read up to its end and handed back to it once.
    unsafe {
        let mut p: windows_sys::core::PWSTR = std::ptr::null_mut();
        if SHGetKnownFolderPath(&FOLDERID_Pictures, 0, std::ptr::null_mut(), &mut p) != 0 || p.is_null() {
            return None;
        }
        let len = (0..).take_while(|&i| *p.add(i) != 0).count();
        let path = String::from_utf16_lossy(std::slice::from_raw_parts(p, len));
        CoTaskMemFree(p as _);
        Some(std::path::PathBuf::from(path))
    }
}

#[cfg(not(windows))]
fn pictures_folder() -> Option<std::path::PathBuf> {
    std::env::var("HOME").ok().map(|h| std::path::PathBuf::from(h).join("Pictures"))
}

/// Runs one drawing on the shared pool (threads.rs): a thread made for each picture (up to sixty a second while the
/// avatar turns or is dragged) leaks a little memory in the NVIDIA driver for every thread made, and that adds up to
/// hundreds of megabytes in an hour. A drawing that panics answers Err, as a failed thread did.
fn draw_worker(job: impl FnOnce() -> Vec<u8> + Send + 'static) -> futures::channel::oneshot::Receiver<Vec<u8>> {
    let (tx, rx) = futures::channel::oneshot::channel();
    crate::threads::run(move || {
        let _ = tx.send(job());
    });
    rx
}

/// Draws the model as it is looked at now, off the window's thread. One picture at a time: a turn asked for while one
/// is being drawn is drawn after it.
fn draw(r: &mut Root, cx: &mut Context<Root>) {
    let Some(model) = r.avatar.solid.clone() else { return };
    if r.avatar.drawing {
        r.avatar.stale = true;
        return;
    }
    let (view, user, ask) = (view_of(&r.avatar), r.avatar.of, r.avatar.solid_ask);
    // Keep full-quality stills and use fewer pixels while producing a stream of preview frames.
    let size = match (r.avatar.big, lively(&r.avatar) || r.avatar.drag.is_some()) {
        (true, true) => STAGE_BIG_MOVING,
        (true, false) => STAGE_BIG,
        (false, true) => STAGE_MOVING,
        (false, false) => STAGE,
    };
    // Only drawn when what it shows changes: the window drawn again (a live wallpaper behind, 30 times a second) shows
    // the picture it has, and a draw asked for the same view of the same model is not made again.
    let of = (view, size, std::sync::Arc::as_ptr(&model) as usize);
    if r.avatar.frame.is_some() && r.avatar.frame_of == Some(of) {
        return;
    }
    r.avatar.drawing = true;
    let rx = draw_worker(move || model.draw(view, size));
    cx.spawn(async move |this, cx| {
        let dots = rx.await.ok();
        let _ = this.update(cx, |r, cx| {
            // (Another account's avatar by now: its Data was put away with its tab, which let go of `drawing`, or is
            // a new one.)
            if r.avatar.of != user {
                return;
            }
            r.avatar.drawing = false;
            // The model was replaced while this picture of the one before it was drawn (after a change: R6 to R15):
            // the picture is dropped and the new model drawn. Returning here with `drawing` still set let no picture
            // be drawn ever again, and the box stood frozen on the old body though the new model had come (the owner,
            // 2026-11-18: "R6 qua R15 vẫn freeze"; since 1.1.2-alpha.9, when a change stopped emptying the box).
            if r.avatar.solid_ask != ask {
                r.avatar.stale = false;
                draw(r, cx);
                return;
            }
            if let Some(buffer) = dots.and_then(|dots| image::RgbaImage::from_raw(size as u32, size as u32, dots)) {
                let new = std::sync::Arc::new(RenderImage::new(vec![image::Frame::new(buffer)]));
                // (The picture it replaces is taken off the graphics card: a drag draws many.)
                if let Some(old) = r.avatar.frame.replace(new) {
                    cx.drop_image(old, None);
                }
                r.avatar.frame_of = Some(of);
            }
            if r.avatar.stale {
                r.avatar.stale = false;
                draw(r, cx);
            }
            cx.notify();
        });
    })
    .detach();
}

/// What the worn things are worth, read once the avatar shows and again when what is worn changes (roblox::worth). An
/// asking that fails isn't made again for the same things until the avatar is refreshed.
fn price(r: &mut Root, cx: &mut Context<Root>) {
    let Some(Ok(a)) = &r.avatar.got else { return };
    let mut ids: Vec<i64> = a.items.iter().map(|i| i.id).collect();
    ids.sort_unstable();
    if r.avatar.pricing || ids.is_empty() || r.avatar.worth_of == ids {
        return;
    }
    let user = r.avatar.of;
    r.avatar.pricing = true;
    let asked = ids.clone();
    let rx = roblox::run(move || roblox::worth(&ids));
    cx.spawn(async move |this, cx| {
        let got = rx.await.ok().and_then(Result::ok);
        let _ = this.update(cx, |r, cx| {
            // Another account's tab is shown by now: asked again when this one comes back.
            if r.avatar.of != user || !r.avatar.pricing {
                return;
            }
            r.avatar.pricing = false;
            r.avatar.worth_of = asked;
            if let Some(w) = got {
                r.avatar.worth.get_or_insert_with(Default::default).extend(w);
            }
            cx.notify();
        });
    })
    .detach();
}

/// The shelf's next page (its first, when it holds nothing yet).
fn shelf_page(r: &mut Root, cx: &mut Context<Root>) {
    let Some((_, user, cookie)) = friends::signed(r) else { return };
    let shelf = r.avatar.shelf;
    let kinds = roblox::SHELVES[shelf].1;
    let cursor = if r.avatar.owned.is_some() { r.avatar.next.clone() } else { String::new() };
    r.avatar.reading = Some(shelf);
    let ask = ticket();
    r.avatar.shelf_ask = ask;
    let rx = roblox::run(move || roblox::inventory(&cookie, user, kinds, &cursor));
    reply(rx, cx, move |r, got, cx| {
        // A page for a shelf or an account no longer shown, or overtaken by a newer ask, is dropped.
        if r.avatar.of != user || r.avatar.reading != Some(shelf) || r.avatar.shelf_ask != ask {
            return;
        }
        r.avatar.reading = None;
        if r.avatar.shelf != shelf {
            return;
        }
        match (got, &mut r.avatar.owned) {
            (Ok((page, next)), Some(Ok(have))) => {
                have.extend(page);
                r.avatar.next = next;
            }
            (Ok((page, next)), _) => {
                #[cfg(feature = "shot")]
                let mut page = page;
                #[cfg(feature = "shot")]
                crate::near::shot_many(&mut page);
                r.avatar.owned = Some(Ok(page));
                r.avatar.next = next;
            }
            (Err(e), Some(Ok(_))) => r.avatar.refused = Some(if e.is_empty() { "Roblox".into() } else { e }),
            (Err(e), _) => r.avatar.owned = Some(Err(e)),
        }
        cx.notify();
    });
}

fn pick_shelf(r: &mut Root, shelf: usize, cx: &mut Context<Root>) {
    if r.avatar.shelf != shelf {
        r.avatar.shelf = shelf;
        r.avatar.owned = None;
        r.avatar.next.clear();
        r.avatar.reading = None;
        shelf_page(r, cx);
        cx.notify();
    }
}

/// The look worn now kept as a new outfit on Roblox, named for the moment ("Look 14/11 11:42"), then the outfits read
/// again.
/// An outfit made what is worn now (`delete` false) or deleted, then the outfits read again.
fn outfit_do(r: &mut Root, id: i64, delete: bool, cx: &mut Context<Root>) {
    let Some((_, user, cookie)) = friends::signed(r).filter(|s| s.1 == r.avatar.of) else { return };
    r.avatar.outfit_menu = None;
    r.avatar.outfit_armed = None;
    r.avatar.busy = Some(id);
    cx.notify();
    let rx = roblox::run(move || if delete { roblox::delete_outfit(&cookie, id) } else { roblox::update_outfit(&cookie, id) });
    reply(rx, cx, move |r, done, cx| {
        if r.avatar.of != user {
            return;
        }
        r.avatar.busy = None;
        match done {
            Ok(()) => {
                r.toast = Some((r.tr(if delete { "av.outfitDeleted" } else { "av.outfitUpdated" }), "ok".into()));
                load(r, cx);
            }
            Err(why) => r.toast = Some((r.i18n.tv("rs.failed", &[("why", if why.is_empty() { "Roblox" } else { &why })]), "bad".into())),
        }
        cx.notify();
    });
}

fn save_look(r: &mut Root, cx: &mut Context<Root>) {
    let Some((_, user, cookie)) = friends::signed(r).filter(|s| s.1 == r.avatar.of) else { return };
    if r.avatar.saving_look {
        return;
    }
    r.avatar.saving_look = true;
    let name = format!("{} {}", r.tr("av.lookName"), crate::time::day_time(crate::time::now_secs()));
    let rx = roblox::run(move || roblox::save_outfit(&cookie, &name));
    reply(rx, cx, move |r, done, cx| {
        if r.avatar.of != user {
            return;
        }
        r.avatar.saving_look = false;
        match done {
            Ok(()) => {
                r.toast = Some((r.tr("av.lookSaved"), "ok".into()));
                load(r, cx);
            }
            Err(why) => r.toast = Some((r.i18n.tv("rs.failed", &[("why", if why.is_empty() { "Roblox" } else { &why })]), "bad".into())),
        }
        cx.notify();
    });
}

fn load(r: &mut Root, cx: &mut Context<Root>) {
    let user = r.avatar.of;
    r.avatar.loading = true;
    let ask = ticket();
    r.avatar.load_ask = ask;
    let rx = roblox::run(move || roblox::avatar(user));
    reply(rx, cx, move |r, got, cx| {
        if r.avatar.of == user && r.avatar.load_ask == ask {
            // A read that fails after a change keeps what was shown.
            let read = got.is_ok();
            if read || !matches!(r.avatar.got, Some(Ok(_))) {
                r.avatar.got = Some(got);
            }
            r.avatar.loading = false;
            r.avatar.busy = None;
            // (After a change the model waits for this read: the look may be one whose model is kept.)
            if r.avatar.swapping {
                after_change(r, read, cx);
            } else {
                remember(r);
            }
            price(r, cx);
            cx.notify();
        }
    });
}

fn reload_model(r: &mut Root, cx: &mut Context<Root>) {
    let avatar = &mut r.avatar;
    MODELS.with(|models| models.borrow_mut().remove(&avatar.of));
    LOOK_MODELS.with(|l| l.borrow_mut().retain(|(user, ..)| *user != avatar.of));
    avatar.solid_file.clear();
    avatar.asked_solid = false;
    avatar.solid_ask = 0;
    avatar.no_solid = false;
    avatar.solid = None;
    avatar.drawing = false;
    avatar.stale = false;
    avatar.spin_run += 1;
    avatar.spinning = false;
    if let Some(old) = avatar.frame.take() {
        cx.drop_image(old, None);
    }
}

fn again(r: &mut Root, cx: &mut Context<Root>) {
    if !r.avatar.loading {
        if r.avatar.got.as_ref().is_some_and(|g| g.is_err()) {
            r.avatar.got = None;
        }
        // A shelf that failed to read is read again too.
        if matches!(r.avatar.owned, Some(Err(_))) && r.avatar.reading.is_none() {
            r.avatar.owned = None;
            r.avatar.next.clear();
        }
        r.avatar.worth_of.clear();
        reload_model(r, cx);
        load(r, cx);
        load_profile_background(r, cx);
        cx.notify();
    }
}

/// A change for the account looked through: one thing off, a saved outfit on, or one owned thing on. Then the avatar
/// is read again, a moment later so Roblox has started on the new picture.
fn change(r: &mut Root, what: Change, cx: &mut Context<Root>) {
    let Some((_, user, cookie)) = friends::signed(r) else { return };
    if r.avatar.busy.is_some() || user != r.avatar.of {
        return;
    }
    // (The body's changes have no thing of their own to fade: a mark below zero says one is under way.)
    r.avatar.busy = Some(match &what {
        Change::Off(id) | Change::Outfit(id) | Change::On(id, _) | Change::EmoteOff(id, _) => *id,
        Change::Rig(_) | Change::Colour(_) | Change::Scale(..) => -1,
        Change::Look(i) => -2 - *i as i64,
    });
    let look = match (&what, &r.avatar.got) {
        (Change::Look(i), Some(Ok(a))) => a.looks.get(*i).cloned(),
        _ => None,
    };
    r.avatar.refused = None;
    // A new skin shows at once on the model already here (mesh::Model::recoloured), the swatch picked with it; Roblox's
    // own model of it replaces this one when it is made (seconds later).
    let recolour = matches!(what, Change::Colour(_));
    let mut repainted = None;
    if let (Change::Colour(hex), Some(Ok(a))) = (&what, r.avatar.got.as_mut()) {
        let rgb = |h: &str| u32::from_str_radix(h, 16).ok().map(|v| [(v >> 16) as u8, (v >> 8) as u8, v as u8]);
        let mut from: Vec<[u8; 3]> = a.body.iter().filter_map(|h| rgb(h)).collect();
        from.extend(rgb(&a.colour));
        if let (Some(to), Some(model)) = (rgb(hex), r.avatar.solid.clone()) {
            let new = std::sync::Arc::new(model.recoloured(&from, to));
            MODELS.with(|m| m.borrow_mut().insert(user, (new.clone(), r.avatar.solid_file.clone())));
            repainted = Some(new);
        }
        a.colour = hex.clone();
        a.body = vec![hex.clone(); 6];
    }
    if let Some(new) = repainted {
        r.avatar.solid = Some(new);
        draw(r, cx);
    }
    cx.notify();
    let rx = roblox::run(move || {
        let done = match &what {
            Change::Off(id) => roblox::take_off(&cookie, user, *id),
            Change::Outfit(id) => roblox::wear_outfit(&cookie, *id),
            Change::On(id, kind) => roblox::put_on(&cookie, user, *id, kind),
            Change::Rig(r15) => roblox::set_rig(&cookie, *r15),
            Change::Colour(hex) => roblox::set_colour(&cookie, hex),
            Change::Scale(which, value) => roblox::set_scale(&cookie, user, *which, *value),
            Change::Look(_) => look.as_ref().ok_or(String::new()).and_then(|l| roblox::wear_look(&cookie, l)),
            Change::EmoteOff(_, slot) => roblox::clear_emote(&cookie, *slot),
        };
        if done.is_ok() {
            std::thread::sleep(std::time::Duration::from_millis(1200));
        }
        done
    });
    reply(rx, cx, move |r, done, cx| {
        if r.avatar.of != user {
            return;
        }
        match done {
            // The model on screen stays (a new skin already repainted on it) while Roblox's new one is asked for, and
            // is replaced in place, turned as it stood: emptying the box first made R6/R15 and every change blink.
            // (The new model is asked for once the avatar is read again, load: after_change.)
            Ok(()) => {
                MODELS.with(|m| m.borrow_mut().remove(&user));
                r.avatar.asked_solid = true;
                r.avatar.solid_ask = 0;
                r.avatar.swapping = true;
                load(r, cx)
            }
            Err(why) => {
                r.avatar.busy = None;
                r.avatar.refused = Some(if why.is_empty() { "Roblox".into() } else { why });
                // (The skin shown at once wasn't taken: Roblox's own look again.)
                if recolour {
                    reload_model(r, cx);
                    load(r, cx);
                }
            }
        }
        cx.notify();
    });
}

pub fn picture(url: &str, cx: &mut App) -> Option<std::sync::Arc<Image>> {
    (!url.is_empty()).then(|| images::pic(url, cx)).flatten()
}

pub fn avatar(root: &Root, width: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let av = &root.avatar;
    let (able, viewer) = friends::looking_ref(root);
    let shell = div().id("avatar").relative().size_full().rounded(px(R_PANEL)).bg(c(PANEL)).overflow_hidden().flex().flex_col();

    let header = div()
        .flex_none()
        .px(px(PAGE_X))
        .pt(px(18.))
        .pb(px(18.))
        .flex()
        .items_center()
        .gap(px(10.))
        .child(crate::pages::games::brand(root))
        .relative().child(crate::pages::games::island(root, cx))
        .child(div().flex_1())
        .when(viewer.is_some(), |d| {
            d.child(IconButton::new("av-refresh", "refresh").size(34.).busy(av.loading).tip(root.tr("fr.refresh")).on_click(cx.listener(|r, _, _, cx| again(r, cx))))
        });

    let body: AnyElement = if able.is_empty() || friends::nobody(root) {
        friends::need_account(root, "av-add-acc", "av.noAccount", "user", cx)
    } else {
        match &av.got {
            None => empty_note(root.tr("av.loading")).into_any_element(),
            Some(Err(why)) => div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(14.))
                .child(div().text_size(px(T_M)).text_color(c(MUTED)).child(root.i18n.tv("av.failed", &[("why", crate::net::why_words(&root.i18n, why).as_str())])))
                .child(Button::new("av-retry", root.tr("fr.retry")).icon("refresh").on_click(cx.listener(|r, _, _, cx| again(r, cx))))
                .into_any_element(),
            Some(Ok(a)) => filled(root, a, width, cx).into_any_element(),
        }
    };
    shell.child(header).child(body)
}

/// The avatar's square: with depth once Roblox's model is in (a drag turns and tilts it, the wheel brings it nearer,
/// A word over the stage's corner: its model on its way (with a turning mark), or none to be had.
fn stage_note(words: String, coming: bool) -> Div {
    div()
        .absolute()
        .top(px(12.))
        .left(px(12.))
        .flex()
        .items_center()
        .gap(px(6.))
        .px(px(8.))
        .py(px(4.))
        .rounded(px(R_S))
        .bg(ca(GROUND, 0.74))
        .text_size(px(T_S))
        .text_color(on_picture(0.92))
        .when(coming, |d| d.child(spinner(12., on_picture(0.92))))
        .child(words)
}

/// a double click puts it back; its controls over it), else Roblox's flat picture.
fn stage(root: &Root, a: &Avatar, side: f32, cx: &mut Context<Root>) -> Stateful<Div> {
    let av = &root.avatar;
    let selected = background_of(av);
    let room = &crate::mesh::ROOMS[if selected.is_some() { 0 } else { room_of(av) }];
    let background = selected.and_then(|asset| images::pic_pair(&asset.pic, side, side, cx));
    let big = av.big;
    let d = div()
        .id(if big { "av-stage-big" } else { "av-stage" })
        .relative()
        .flex_none()
        .w(px(side))
        .h(px(side))
        .rounded(px(R_M))
        .overflow_hidden()
        .bg(c(if room.wall == 0 { INSET } else { room.wall }))
        .flex()
        .items_center()
        .justify_center()
        .when_some(background, |d, (image, key)| d.child(div().absolute().inset_0()
            .child(fade_in(img(image).size_full().object_fit(ObjectFit::Cover), &key))));
    let Some(frame) = av.frame.clone() else {
        // Roblox's flat picture, and while its model is on its way, a word that it is.
        let coming = !av.no_solid && friends::signed(root).is_some_and(|(_, _, c)| !c.is_empty());
        return d.bg(c(INSET)).when_some(picture(&a.pic, cx), |d, p| d.child(fade_in(img(p.clone()).h_full().object_fit(ObjectFit::Contain), &p)))
            .when(coming || av.no_solid, |d| d.child(stage_note(root.tr(if coming { "av3.loading" } else { "av3.fallback" }), coming)));
    };
    let glass = |id: &'static str, icon: &'static str, tip: String| IconButton::new(id, icon).size(30.).glass().tip(tip);
    d.cursor(if av.drag.is_some() { CursorStyle::ClosedHand } else { CursorStyle::OpenHand })
        .child(img(frame).size_full().object_fit(ObjectFit::Contain))
        // (After a change the model before it stays up until Roblox's new one replaces it: said here.)
        .when(av.swapping, |d| d.child(stage_note(root.tr("av3.loading"), true)))
        // Clicked, it takes the keys: W A S D or the arrows walk the avatar (mesh::Model::step).
        .track_focus(&root.avatar_focus)
        .on_key_down(cx.listener(|r, e: &KeyDownEvent, _, cx| {
            if e.keystroke.key == "escape" && r.avatar.big {
                set_big(r, false, cx);
                return;
            }
            let m = &e.keystroke.modifiers;
            if m.control || m.alt || m.platform {
                return;
            }
            // Space jumps (from the floor; held, it jumps again as it lands, as in Roblox).
            if e.keystroke.key == "space" {
                cx.stop_propagation();
                r.avatar.jump = true;
                start_spin(r, cx);
                return;
            }
            let Some(bit) = walk_key(&e.keystroke.key) else { return };
            cx.stop_propagation();
            if r.avatar.keys & bit == 0 {
                r.avatar.keys |= bit;
                if !r.avatar.turned {
                    r.avatar.turned = true;
                    cx.notify();
                }
                start_spin(r, cx);
            }
        }))
        .on_key_up(cx.listener(|r, e: &KeyUpEvent, _, cx| {
            if let Some(bit) = walk_key(&e.keystroke.key) {
                r.avatar.keys &= !bit;
                cx.stop_propagation();
            }
        }))
        // (Shift alone comes as a change of the held modifiers, not as a key.)
        .on_modifiers_changed(cx.listener(|r, e: &ModifiersChangedEvent, _, _| {
            if e.modifiers.shift {
                r.avatar.keys |= 16;
            } else {
                r.avatar.keys &= !16;
            }
        }))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|r, e: &MouseDownEvent, window, cx| {
                window.focus(&r.avatar_focus, cx);
                if e.click_count >= 2 {
                    r.avatar.drag = None;
                    reset_view(r, cx);
                    return;
                }
                r.avatar.drag = Some((f32::from(e.position.x), f32::from(e.position.y), r.avatar.yaw, r.avatar.pitch));
                cx.notify();
            }),
        )
        .on_mouse_move(cx.listener(|r, e: &MouseMoveEvent, _, cx| {
            if e.pressed_button != Some(MouseButton::Left) {
                if r.avatar.drag.take().is_some() {
                    cx.notify();
                }
            } else if let Some((x0, y0, yaw, pitch)) = r.avatar.drag {
                r.avatar.yaw = yaw + (f32::from(e.position.x) - x0) * 0.012;
                r.avatar.pitch = (pitch + (f32::from(e.position.y) - y0) * 0.006).clamp(crate::mesh::PITCH_LOW, crate::mesh::PITCH_HIGH);
                if !r.avatar.turned {
                    r.avatar.turned = true;
                    cx.notify();
                }
                draw(r, cx);
            }
        }))
        .on_mouse_up(MouseButton::Left, cx.listener(|r, _, _, cx| {
            r.avatar.drag = None;
            cx.notify();
        }))
        .on_mouse_up_out(MouseButton::Left, cx.listener(|r, _, _, cx| {
            r.avatar.drag = None;
            cx.notify();
        }))
        // The wheel brings it nearer: always when big; beside the page's lists, with Ctrl held (else the page scrolls).
        .on_scroll(cx.listener(|r, e: &ScrollEvent, _, cx| {
            if !r.avatar.big && !e.modifiers.control {
                return;
            }
            let dy = f32::from(e.delta.pixel_delta(px(20.)).y);
            r.avatar.near = (r.avatar.near + dy * 0.0025).clamp(1. - crate::mesh::ZOOM_FAR, 1. - crate::mesh::ZOOM_NEAR);
            r.avatar.turned = true;
            draw(r, cx);
            cx.stop_propagation();
        }))
        .child(
            div()
                .absolute()
                .top(px(10.))
                .right(px(10.))
                .flex()
                .gap(px(6.))
                .child(glass("av3-spin", "repeat", root.tr("av3.spin")).on(av.spin).on_click(cx.listener(|r, _, _, cx| {
                    let on = !r.avatar.spin;
                    set_spin(r, on, cx)
                })))
                .when(moves(av), |d| {
                    d.child(glass("av3-move", if av.moving { "pause" } else { "play" }, root.tr(if av.moving { "av3.moveStop" } else { "av3.moveGo" })).on_click(cx.listener(|r, _, _, cx| {
                        let on = !r.avatar.moving;
                        set_moving(r, on, cx)
                    })))
                })
                .child(glass("av3-reset", "refresh", root.tr("av3.reset")).on_click(cx.listener(|r, _, _, cx| reset_view(r, cx))))
                .child(glass("av3-save", "download", root.tr("av3.save")).busy(av.saving).on_click(cx.listener(|r, _, _, cx| save_picture(r, cx))))
                .when(!big, |d| d.child(glass("av3-big", "focus", root.tr("av3.big")).on_click(cx.listener(|r, _, _, cx| set_big(r, true, cx))))),
        )
        .when(!av.turned, |d| {
            d.child(
                div()
                    .absolute()
                    .bottom(px(12.))
                    .left(px(12.))
                    .right(px(12.))
                    .flex()
                    .justify_center()
                    .child(div().min_w_0().max_w_full().px(px(10.)).py(px(6.)).rounded(px(R_S))
                        .bg(ca(GROUND, 0.78)).text_center().text_size(px(T_S)).text_color(on_picture(0.92))
                        .child(root.tr(if big { "av3.hintBig" } else { "av3.hint" }))),
            )
        })
}

/// The rooms to stand the avatar in, each a little picture of its wall over its floor; the picked one ringed yellow.
fn rooms_row(root: &Root, cx: &mut Context<Root>) -> Div {
    let av = &root.avatar;
    let picked = room_of(av);
    let roblox_background = av.background.is_none() && !av.studio_override;
    div().flex().flex_wrap().items_center().gap(px(6.))
        .when(av.profile_background.is_some(), |d| d.child(
            chip("av3-profile-background", roblox_background).h(px(30.))
                .child(root.tr("av3.profileBackground"))
                .on_click(cx.listener(|r, _, _, cx| {
                    r.avatar.background = None;
                    r.avatar.studio_override = false;
                    draw(r, cx);
                    cx.notify();
                }))
        ))
        .children(crate::mesh::ROOMS.iter().enumerate().map(|(i, room)| {
        let tip: SharedString = root.tr(&format!("av3.room.{}", room.id)).into();
        let swatch = div().size_full().rounded(px(R_S)).overflow_hidden().border_1().border_color(c(LINE)).flex().flex_col().map(|d| {
            if room.wall == 0 {
                d.bg(c(INSET))
            } else {
                d.child(div().h(px(14.)).bg(c(room.wall))).child(div().flex_1().bg(c(room.floor)).flex().justify_center().child(div().mt(px(3.)).w(px(16.)).h(px(5.)).rounded(px(3.)).bg(c(room.plate))))
            }
        });
        hot(("av3-room", i))
            .size(px(34.))
            .p(px(3.))
            .rounded(px(R_S + 3.))
            .border_2()
            .border_color(if i == picked && background_of(av).is_none() { c(FACE) } else { gpui_kit::transparent_black() })
            .cursor_pointer()
            .child(swatch)
            .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
            .on_click(cx.listener(move |r, _, _, cx| set_room(r, i, cx)))
    }))
}

/// The picture on the left (above, in a narrow window), what it wears and the saved outfits beside it.
fn filled(root: &Root, a: &Avatar, width: f32, cx: &mut Context<Root>) -> AnyElement {
    let av = &root.avatar;
    let inner = width - 2. * (PAGE_X);
    let wide = inner >= 760.;
    let side = if wide { 340. } else { inner };
    let lists_w = if wide { inner - side - 24. } else { inner };
    let cols = ((lists_w + TGAP) / (TILE + TGAP)).floor().max(1.);
    let tile_w = ((lists_w - TGAP * (cols - 1.)) / cols).floor();
    let busy = av.busy;

    // Shown big: the avatar fills the page, its room and controls beside it (under it, in a narrow window).
    if av.big && av.frame.is_some() {
        let side_by = inner >= 760.;
        // As wide as the page lets it, and no taller than the window shows (the feet were under the fold at 1280x800,
        // 2026-11-15): the window's height less its bars, the tabs and this page's header.
        let fits = (root.window_h.get() - 290.).max(280.);
        let stage_side = if side_by { (inner - 300. - 24.).clamp(360., 780.).min(fits.max(360.)) } else { inner.min(560.).min(fits) };
        let room_word = match background_of(av) {
            Some(_) if av.background.is_none() => root.tr("av3.profileBackground"),
            Some(owned) => owned.name.clone(),
            None => root.tr(&format!("av3.room.{}", crate::mesh::ROOMS[room_of(av)].id)),
        };
        let panel = surface()
            .w(px(if side_by { 300. } else { stage_side }))
            .flex_none()
            .p(px(18.))
            .flex()
            .flex_col()
            .gap(px(18.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    // The way back said in words, first (an unnamed window icon at the end was missed, owner 2026-11-15);
                    // Esc on the stage does the same.
                    .child(Button::new("av3-small", root.tr("av.back")).icon("chevronLeft").small().quiet().on_click(cx.listener(|r, _, _, cx| set_big(r, false, cx))))
                    .child(div().flex_1().text_size(px(T_GROUP)).font_weight(FontWeight::SEMIBOLD).child(root.tr("av3.title"))),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(div().flex().items_baseline().gap(px(8.)).child(div().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).child(root.tr("av3.rooms"))).child(div().text_size(px(T_S)).text_color(c(MUTED)).child(room_word)))
                    .child(rooms_row(root, cx)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(div().flex_1().text_size(px(T_M)).child(root.tr("av3.spin")))
                    .child(Switch::new("av3-spin-sw", av.spin).on_toggle(cx.listener(|r, on: &bool, _, cx| set_spin(r, *on, cx)))),
            )
            .when(moves(av), |d| {
                d.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .child(div().flex_1().text_size(px(T_M)).child(root.tr("av3.move")))
                        .child(Switch::new("av3-move-sw", av.moving).on_toggle(cx.listener(|r, on: &bool, _, cx| set_moving(r, *on, cx)))),
                )
            })
            .child(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(Button::new("av3-reset-b", root.tr("av3.reset")).icon("refresh").on_click(cx.listener(|r, _, _, cx| reset_view(r, cx))))
                    .child(Button::new("av3-save-b", root.tr("av3.save")).icon("download").disabled(av.saving).on_click(cx.listener(|r, _, _, cx| save_picture(r, cx)))),
            )
            // What is worn kept as an outfit on Roblox, here too (it was only under Saved outfits, out of sight in this
            // view; owner 2026-11-15). Only signed in as this account.
            .when(friends::signed(root).is_some_and(|(_, u, c)| !c.is_empty() && u == av.of), |d| {
                d.child(
                    Button::new("av3-outfit", root.tr(if av.saving_look { "av.savingLook" } else { "av.saveLook" }))
                        .icon("plus")
                        .primary()
                        .disabled(av.saving_look)
                        .on_click(cx.listener(|r, _, _, cx| save_look(r, cx))),
                )
            })
            .child(div().text_size(px(T_S)).line_height(px(18.)).text_color(c(MUTED)).child(root.tr("av3.howBig")));
        return crate::scrolled::body(
            "av-body-big",
            div()
                .px(px(PAGE_X))
                .pt(px(4.))
                .pb(px(24.))
                .flex()
                .when(side_by, |d| d.items_start().gap(px(24.)))
                .when(!side_by, |d| d.flex_col().items_center().gap(px(16.)))
                .child(stage(root, a, stage_side, cx))
                .child(panel),
        )
        .into_any_element();
    }

    let portrait = surface()
        .w(px(side))
        .flex_none()
        .p(px(14.))
        .flex()
        .flex_col()
        .gap(px(12.))
        .child(div().w_full().flex().justify_center().child(stage(root, a, if wide { side - 28. } else { (side - 28.).min(420.) }, cx)))
        .when(av.frame.is_some(), |d| d.child(div().flex().flex_wrap().items_center().gap(px(8.)).child(div().text_size(px(T_S)).text_color(c(MUTED)).child(root.tr("av3.rooms"))).child(rooms_row(root, cx))))
        .children(worth(root, a))
        .child(body(root, a, cx));

    let section = |title: String, count: Option<usize>, tiles: Vec<AnyElement>, none: &str| {
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(section_head(title, count))
            .map(|d| if tiles.is_empty() { d.child(div().text_size(px(T_M)).text_color(c(MUTED)).child(root.tr(none))) } else { d.child(div().flex().flex_wrap().gap(px(TGAP)).children(tiles)) })
    };

    let worn: Vec<AnyElement> = a
        .items
        .iter()
        .map(|it| {
            let id = it.id;
            tile_of(tile_w, &it.pic, it.name.clone(), busy == Some(id), cx)
                .child(price_row(root, av.worth.as_ref().and_then(|w| w.get(&id)).copied()))
                .child(
                    div().absolute().top(px(6.)).right(px(6.)).child(
                        IconButton::new(("av-off", id as usize), "x")
                            .size(26.)
                            .filled()
                            .disabled(busy.is_some())
                            .tip(root.tr("av.takeOff"))
                            .on_click(cx.listener(move |r, _, _, cx| change(r, Change::Off(id), cx))),
                    ),
                )
                .into_any_element()
        })
        .collect();
    // The wheel: each emote with its place on it, and a way off.
    let wheel: Vec<AnyElement> = a
        .emotes
        .iter()
        .map(|e| {
            let (id, slot) = (e.id, e.slot);
            tile_of(tile_w, &e.pic, e.name.clone(), busy == Some(id), cx)
                .child(div().absolute().top(px(8.)).left(px(8.)).size(px(22.)).flex().items_center().justify_center().rounded_full().bg(c(RAISED_2)).text_size(px(T_S)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(slot.to_string()))
                .child(
                    div().absolute().top(px(6.)).right(px(6.)).child(
                        IconButton::new(("av-emote-off", slot as usize), "x")
                            .size(26.)
                            .filled()
                            .disabled(busy.is_some())
                            .tip(root.tr("av.takeOff"))
                            .on_click(cx.listener(move |r, _, _, cx| change(r, Change::EmoteOff(id, slot), cx))),
                    ),
                )
                .into_any_element()
        })
        .collect();
    let can_save = friends::signed(root).is_some_and(|(_, u, c)| !c.is_empty() && u == av.of);
    let outfits: Vec<AnyElement> = a
        .outfits
        .iter()
        .map(|o| {
            let id = o.id;
            let n = (id as u64 % 1_000_000_007) as usize;
            let open = av.outfit_menu == Some(id);
            let armed = av.outfit_armed == Some(id);
            tile_of(tile_w, &o.pic, o.name.clone(), busy == Some(id), cx)
                .child(div().pb(px(4.)).flex().child(
                    Button::new(("av-wear", n), root.tr("av.wear")).small().disabled(busy.is_some()).on_click(cx.listener(move |r, _, _, cx| change(r, Change::Outfit(id), cx))),
                ))
                // The menu: make it what is worn now, or delete it (a second click), as Roblox's own outfit menu.
                .when(can_save, |d| {
                    let mut more = div().absolute().top(px(6.)).right(px(6.)).child(IconButton::new(("av-of-more", n), "more").size(26.).filled().disabled(busy.is_some()).tip(root.tr("av.outfitMore")).on_click(cx.listener(move |r, _, _, cx| {
                        r.avatar.outfit_menu = if r.avatar.outfit_menu == Some(id) { None } else { Some(id) };
                        r.avatar.outfit_armed = None;
                        cx.notify();
                    })));
                    if crate::pages::accounts::pop_alive(&format!("av-of-pop-{id}"), open) {
                        let menu = div()
                            .id(("av-of-menu", n))
                            .occlude()
                            .w(px(230.))
                            .p(px(4.))
                            .rounded(px(R_M))
                            .bg(c(RAISED))
                            .border_1()
                            .border_color(c(LINE))
                            .shadow_lg()
                            .flex()
                            .flex_col()
                            .on_mouse_down_out(cx.listener(|r, _, _, cx| {
                                r.avatar.outfit_menu = None;
                                r.avatar.outfit_armed = None;
                                cx.notify();
                            }))
                            .child(menu_row(("av-of-update", n)).h(px(34.)).px(px(8.)).child(icon("refresh", 14., c(TEXT_2))).child(root.tr("av.outfitUpdate")).on_click(cx.listener(move |r, _, _, cx| outfit_do(r, id, false, cx))))
                            .child(
                                menu_row(("av-of-delete", n))
                                    .h(px(34.))
                                    .px(px(8.))
                                    .text_color(c(BAD))
                                    .child(icon("trash", 14., c(BAD)))
                                    .child(root.tr(if armed { "av.outfitDeleteSure" } else { "av.outfitDelete" }))
                                    .on_click(cx.listener(move |r, _, _, cx| {
                                        if r.avatar.outfit_armed == Some(id) {
                                            outfit_do(r, id, true, cx);
                                        } else {
                                            r.avatar.outfit_armed = Some(id);
                                            cx.notify();
                                        }
                                    })),
                            );
                        more = more.child(popup(PopAt::BelowRight(30.), format!("av-of-pop-{id}"), open, menu));
                    }
                    d.child(more)
                })
                .into_any_element()
        })
        .collect();

    // Saved outfits, with "Save as outfit" (the look worn now, signed in as it) and "Change one item" (two outfits or
    // more) in its head.
    let can_swap = a.outfits.len() >= 2 && friends::signed(root).is_some_and(|(_, _, c)| !c.is_empty());
    let outfits_section = div()
        .flex()
        .flex_col()
        .gap(px(12.))
        .child(
            section_head(root.tr("av.outfits"), Some(a.outfits.len()))
                .when(can_save && av.swap.is_none(), |d| {
                    d.child(Button::new("av-save-look", root.tr(if av.saving_look { "av.savingLook" } else { "av.saveLook" })).icon("plus").small().disabled(av.saving_look || busy.is_some()).on_click(cx.listener(|r, _, _, cx| save_look(r, cx))))
                })
                .when(can_swap && av.swap.is_none(), |d| {
                    d.child(Button::new("av-swap", root.tr("sw.open")).icon("shuffle").small().on_click(cx.listener(|r, _, _, cx| open_swap(r, cx))))
                }),
        )
        .when(av.swap.is_some(), |d| d.child(swap_panel(root, lists_w, cx)))
        .map(|d| if outfits.is_empty() { d.child(div().text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("av.noOutfits"))) } else { d.child(div().flex().flex_wrap().gap(px(TGAP)).children(outfits)) });

    let lists = div()
        .w(px(lists_w))
        .flex()
        .flex_col()
        .gap(px(SECTION_GAP))
        .when_some(av.refused.clone(), |d, why| d.child(div().text_size(px(T_M)).text_color(c(WARN)).child(root.i18n.tv("av.refused", &[("why", &why)]))))
        .child(section(root.tr("av.wearing"), Some(a.items.len()), worn, "av.nothing"))
        .when(!a.emotes.is_empty(), |d| d.child(section(format!("Emote  {}/{}", a.emotes.len(), roblox::EMOTES), None, wheel, "av.nothing")))
        .when(a.looks.len() > 1, |d| d.child(history(root, a, tile_w, cx)))
        .child(outfits_section)
        .child(shelves(root, a, tile_w, cx));

    crate::scrolled::body("av-body", 
        div().px(px(PAGE_X)).pt(px(4.)).pb(px(24.)).flex().items_start().gap(px(24.)).when(!wide, |d| d.flex_col()).child(portrait).child(lists),
    )
    .into_any_element()
}

/// What the avatar is worth, which Roblox doesn't add up: the Robux of everything worn (roblox::worth), a bundle
/// counted once however many of its parts are worn, and how many things that counts and leaves out (no price to
/// count). Nothing until Roblox has said, or when nothing worn has a price.
fn worth(root: &Root, a: &Avatar) -> Option<Div> {
    let w = root.avatar.worth.as_ref()?;
    let vi = root.i18n.vietnamese;
    let known: Vec<Option<Price>> = a.items.iter().filter_map(|i| w.get(&i.id).copied()).collect();
    let counted = known.iter().flatten().count();
    if counted == 0 {
        return None;
    }
    let left = known.len() - counted;
    let mut packs: Vec<i64> = Vec::new();
    let mut sum = 0;
    for p in known.iter().flatten() {
        if p.pack == 0 || !packs.contains(&p.pack) {
            sum += p.robux;
        }
        if p.pack != 0 {
            packs.push(p.pack);
        }
    }
    let note = if left == 0 { root.i18n.tv("av.worthAll", &[("n", &counted.to_string())]) } else { root.i18n.tv("av.worthSome", &[("n", &counted.to_string()), ("m", &left.to_string())]) };
    Some(
        div()
            .px(px(2.))
            .flex()
            .items_end()
            .justify_between()
            .gap(px(10.))
            .child(
                div().flex().flex_col().gap(px(2.)).child(div().text_size(px(T_S)).text_color(c(MUTED)).child(root.tr("av.worth"))).child(
                    div().flex().items_center().gap(px(7.)).child(icon("robux", 18., c(TEXT))).child(div().text_size(px(T_TITLE)).font_weight(FontWeight::BOLD).font_features(tnum()).child(thousands(sum, vi))),
                ),
            )
            .child(div().pb(px(4.)).text_size(px(T_S)).text_color(c(MUTED)).child(note)),
    )
}

/// Under a worn thing's name: what it is worth, or that it is free or has no price; a Limited says so, and a part of
/// a bundle that the price is the bundle's. Empty (the same height) until Roblox has said.
fn price_row(root: &Root, known: Option<Option<Price>>) -> Div {
    let vi = root.i18n.vietnamese;
    let row = div().px(px(2.)).pb(px(4.)).h(px(22.)).flex().items_center().gap(px(5.)).text_size(px(T_S)).font_features(tnum());
    match known {
        None => row,
        Some(None) => row.text_color(c(MUTED)).child(root.tr("av.offSale")),
        Some(Some(p)) if p.robux == 0 => row.text_color(c(MUTED)).child(root.tr("av.free")),
        Some(Some(p)) => row
            .text_color(c(TEXT))
            .child(icon("robux", 12., c(TEXT)))
            .child(div().font_weight(FontWeight::MEDIUM).child(thousands(p.robux, vi)))
            .when(p.limited, |d| d.child(div().text_color(c(WARN)).child("Limited")))
            .when(p.pack != 0, |d| d.child(div().text_color(c(MUTED)).child(root.tr("av.pack")))),
    }
}

/// The avatar's history, which Roblox doesn't keep: every look this PC has seen the avatar in, newest first, each with
/// when it was first seen and Back to put it on again (the one it is in now says so instead).
fn history(root: &Root, a: &Avatar, tile_w: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let busy = root.avatar.busy;
    let tiles: Vec<AnyElement> = a
        .looks
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let pic = picture(&l.pic, cx);
            tile(tile_w, pic, crate::time::day_time(l.at), busy == Some(-2 - i as i64))
                .child(div().pb(px(4.)).h(px(34.)).flex().items_center().map(|d| {
                    if i == a.now {
                        d.child(div().px(px(2.)).text_size(px(T_S)).text_color(c(MUTED)).child(root.tr("av.now")))
                    } else {
                        d.child(Button::new(("av-look", i), root.tr("av.back")).small().icon("history").disabled(busy.is_some()).on_click(cx.listener(move |r, _, _, cx| change(r, Change::Look(i), cx))))
                    }
                }))
                .into_any_element()
        })
        .collect();
    div()
        .flex()
        .flex_col()
        .gap(px(12.))
        .child(
            section_head(root.tr("av.history"), Some(a.looks.len())).child(div().flex_none().text_size(px(T_S)).text_color(c(MUTED)).child(root.tr("av.historyNote"))),
        )
        .child(div().flex().flex_wrap().gap(px(TGAP)).children(tiles))
}

/// The body, under the picture: R6 or R15, the skin's colour (Roblox's own tones, one all over), and each size as a
/// row of steps from the least to the most Roblox allows.
fn body(root: &Root, a: &Avatar, cx: &mut Context<Root>) -> impl IntoElement {
    let busy = root.avatar.busy.is_some();
    let label = |key: &str| div().text_size(px(T_S)).text_color(c(MUTED)).child(root.tr(key));
    let rig = div().flex().items_center().gap(px(4.)).children([(false, "R6"), (true, "R15")].into_iter().map(|(r15, word)| {
        let on = (a.rig == "R15") == r15;
        chip(SharedString::from(format!("av-rig-{word}")), on).h(px(TOOL_H)).px(px(14.)).cursor_pointer().child(word).on_click(cx.listener(move |r, _, _, cx| {
            if matches!(&r.avatar.got, Some(Ok(a)) if (a.rig == "R15") != r15) {
                change(r, Change::Rig(r15), cx);
            }
        }))
    }));
    let mut col = div().flex().flex_col().gap(px(14.)).when(busy, |d| d.opacity(0.6)).child(div().flex().child(rig));
    if !a.palette.is_empty() {
        // The colour worn leads when it isn't one of Roblox's skin tones (a colour set elsewhere), so what is worn
        // always shows picked.
        let mut tones: Vec<String> = a.palette.clone();
        if !a.colour.is_empty() && !tones.iter().any(|h| h.eq_ignore_ascii_case(&a.colour)) {
            tones.insert(0, a.colour.clone());
        }
        col = col.child(div().flex().flex_col().gap(px(8.)).child(label("av.skin")).child(div().flex().flex_wrap().gap(px(4.)).children(tones.into_iter().enumerate().map(|(i, hex)| {
            let on = hex.eq_ignore_ascii_case(&a.colour);
            let rgb = u32::from_str_radix(&hex, 16).unwrap_or(0x808080);
            let pick = hex.clone();
            // Picked: the yellow ring round it, a gap between (as everything chosen in the app); the others a
            // hairline, so a dark one still shows on the card.
            focusable(div().id(("av-skin", i)))
                .size(px(28.))
                .p(px(2.))
                .rounded_full()
                .border_2()
                .border_color(if on { c(FACE) } else { ca(WHITE, 0.) })
                .cursor_pointer()
                .child(div().size_full().rounded_full().bg(c(rgb)).border_1().border_color(ca(WHITE, 0.12)))
                .on_click(cx.listener(move |r, _, _, cx| {
                    if !on {
                        change(r, Change::Colour(pick.clone()), cx);
                    }
                }))
        }))));
    }
    const STEPS: usize = 8;
    for (which, (lo, hi)) in a.ranges.iter().copied().enumerate() {
        if hi <= lo {
            continue;
        }
        let at = (((a.scales[which] - lo) / (hi - lo)).clamp(0., 1.) * (STEPS - 1) as f64).round() as usize;
        // Its value in words at the end of the row, as Roblox's editor shows it (a size in per cent of the usual one).
        let shown = format!("{}%", (a.scales[which] * 100.).round() as i64);
        col = col.child(
            div().flex().items_center().gap(px(12.)).child(div().w(px(84.)).flex_none().child(label(&format!("av.scale.{which}")))).child(div().flex_1().flex().gap(px(3.)).children((0..STEPS).map(|i| {
                let value = lo + (hi - lo) * i as f64 / (STEPS - 1) as f64;
                div()
                    .id(("av-scale", which * 16 + i))
                    .flex_1()
                    .h(px(10.))
                    .rounded(px(3.))
                    .cursor_pointer()
                    .bg(if i <= at { ca(WHITE, 0.55) } else { ca(WHITE, 0.1) })
                    .hover(|d| d.bg(ca(WHITE, 0.8)))
                    .on_click(cx.listener(move |r, _, _, cx| {
                        if i != at {
                            change(r, Change::Scale(which, value), cx);
                        }
                    }))
            }))).child(div().w(px(40.)).flex_none().text_right().text_size(px(T_S)).font_features(tnum()).text_color(c(TEXT_2)).child(shown)),
        );
    }
    col
}

/// The inventory: a shelf picked on the track, its things as tiles (Wear, or a word when it is on already), and More
/// while Roblox has another page.
fn shelves(root: &Root, a: &Avatar, tile_w: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let av = &root.avatar;
    let busy = av.busy;
    // The shelves as the page's filters: no track, the one shown a yellow block.
    let track = div().flex().flex_wrap().items_center().gap(px(4.)).children(roblox::SHELVES.iter().enumerate().map(|(i, (key, _))| {
        chip(("av-shelf", i), av.shelf == i).h(px(TOOL_H)).px(px(12.)).cursor_pointer().child(root.tr(&format!("av.shelf.{key}"))).on_click(cx.listener(move |r, _, _, cx| pick_shelf(r, i, cx)))
    }));
    let said = |text: String| div().text_size(px(T_M)).text_color(c(MUTED)).child(text);
    let body: AnyElement = match &av.owned {
        None => waiting_tiles(10, tile_w, TGAP).into_any_element(),
        Some(Err(why)) => said(root.i18n.tv("av.shelfFailed", &[("why", crate::net::why_words(&root.i18n, why).as_str())])).into_any_element(),
        Some(Ok(l)) if l.is_empty() => said(root.tr("av.shelfEmpty")).into_any_element(),
        Some(Ok(l)) => {
            let key = crate::near::sig((av.shelf, l.len(), l.first().map(|o| o.id)));
            let tile = |n: usize| {
                    let o = &l[n];
                    let (id, kind) = (o.id, o.kind.clone());
                    let on = a.items.iter().any(|i| i.id == id) || a.emotes.iter().any(|e| e.id == id);
                    let background = kind == "AvatarBackground";
                    let previewed = av.background.as_ref().is_some_and(|asset| asset.id == id);
                    tile_of(tile_w, &o.pic, o.name.clone(), busy == Some(id), cx)
                        .child(div().pb(px(4.)).h(px(34.)).flex().items_center().map(|d| {
                            if background {
                                let asset = o.clone();
                                d.child(Button::new(("av-background", n), root.tr(if previewed { "av3.backgroundRemove" } else { "av3.backgroundPreview" }))
                                    .small().disabled(o.pic.is_empty()).on_click(cx.listener(move |r, _, _, cx| {
                                        let same = r.avatar.background.as_ref().is_some_and(|current| current.id == id);
                                        r.avatar.background = (!same).then(|| asset.clone());
                                        draw(r, cx);
                                        cx.notify();
                                    })))
                            } else if on {
                                d.child(div().px(px(2.)).text_size(px(T_S)).text_color(c(MUTED)).child(root.tr("av.wearing")))
                            } else {
                                d.child(Button::new(("av-on", n), root.tr("av.wear")).small().disabled(busy.is_some()).on_click(cx.listener(move |r, _, _, cx| change(r, Change::On(id, kind.clone()), cx))))
                            }
                        }))
                        .into_any_element()
                };
            // (Only the lines near the view are made once there are many: each More adds a page of Roblox's.)
            div()
                .flex()
                .flex_col()
                .gap(px(14.))
                .child(crate::near::tiles("av-shelf-tiles", l.len(), tile_w, TGAP, key, false, tile_w + 60., tile))
                .when(!av.next.is_empty(), |d| {
                    d.child(div().flex().child(Button::new("av-more", root.tr("av.more")).disabled(av.reading.is_some()).on_click(cx.listener(|r, _, _, cx| {
                        if r.avatar.reading.is_none() {
                            shelf_page(r, cx);
                            cx.notify();
                        }
                    }))))
                })
                .into_any_element()
        }
    };
    div().flex().flex_col().gap(px(12.)).child(section_head(root.tr("av.inventory"), None)).child(track)
        .when(roblox::SHELVES[av.shelf].0 == "background", |d| d.child(said(root.tr("av3.backgroundHint"))))
        .child(body)
}

/// A worn thing or an outfit: its picture on a block, its name under it (two lines
/// at most, so a row's prices and buttons line up).
pub fn tile(w: f32, pic: Option<std::sync::Arc<Image>>, name: String, busy: bool) -> Div {
    frame(w, name, busy, |d| d.when_some(pic, |d, p| d.child(fade_in(img(p.clone()).size_full().object_fit(ObjectFit::Contain), &p))))
}

/// [`tile`] with its picture read at the exact size it is shown (crisp, not a larger picture scaled down).
pub fn tile_of(w: f32, url: &str, name: String, busy: bool, cx: &mut App) -> Div {
    tile_said(w, url, name, busy, None, cx)
}

/// [`tile_of`], and when its picture couldn't be had, `lost` said quietly in its place (the house line: a small
/// picture mark and the words) rather than an empty block.
pub fn tile_said(w: f32, url: &str, name: String, busy: bool, lost: Option<SharedString>, cx: &mut App) -> Div {
    tile_zoomed(w, url, name, busy, lost, 1., cx)
}

/// `tile_said` with the picture drawn `zoom` times the tile, centred and cut at its edges: a whole figure (a bundle)
/// stands small in the middle of Roblox's picture, a 40 px figure in a 210 px tile beside full-size hats (review 3f.1).
pub fn tile_zoomed(w: f32, url: &str, name: String, busy: bool, lost: Option<SharedString>, zoom: f32, cx: &mut App) -> Div {
    let side = ((w - 16.) * zoom).max(8.);
    let pic = (!url.is_empty()).then(|| images::pic_pair(url, side, side, cx)).flatten();
    let failed = !url.is_empty() && images::failed(url, cx);
    frame(w, name, busy, move |d| match (pic, lost) {
        (Some((src, key)), _) => d.child(fade_in(img(src).flex_none().size(px(side)).object_fit(ObjectFit::Contain), &key)),
        (None, Some(text)) if failed => d.flex_col().gap(px(6.)).text_size(px(T_S)).text_color(c(MUTED)).child(icon("image", 13., c(MUTED))).child(div().text_center().child(text)),
        (None, _) => d.child(icon("image", 22., c(DIM))),
    })
}

fn frame(w: f32, name: String, busy: bool, fill: impl FnOnce(Div) -> Div) -> Div {
    div()
        .relative()
        .w(px(w))
        .flex()
        .flex_col()
        .when(busy, |d| d.opacity(0.5))
        .child(fill(div().w(px(w)).h(px(w)).flex_none().rounded(px(R_L)).overflow_hidden().bg(c(RAISED)).p(px(8.)).flex().items_center().justify_center()))
        // (Two lines and no more: the clip is the text's own box, so a third never shows in the padding under it. A
        // one-line name takes one line: what goes under it follows close, and the room left in a row whose tiles
        // stand as tall as its tallest comes before it, so a row's prices and buttons still line up.)
        .child(div().px(px(2.)).pt(px(10.)).pb(px(4.)).child(div().max_h(px(36.)).text_size(px(T_M)).font_weight(FontWeight::MEDIUM).text_color(c(TEXT)).line_height(px(18.)).overflow_hidden().child(name)))
        .child(div().flex_1())
}

// ───────────────────────── one thing changed in many outfits ─────────────────────────

/// Changing one thing in many saved outfits: the outfits' things (read when it opens), the thing to change, what goes
/// in its place (None: not picked; Some(None): just taken out), what is owned of that kind, the outfits left out, and
/// how it went (each outfit's things before it, for Undo).
#[derive(Default)]
pub struct Swap {
    parts: Option<Result<Vec<roblox::OutfitParts>, String>>,
    from: Option<i64>,
    with: Option<Option<i64>>,
    owned: Option<Result<(Vec<Owned>, String), String>>,
    skip: std::collections::HashSet<i64>,
    /// Under way: how many outfits are done of how many.
    running: Option<(usize, usize)>,
    /// Done: how many were changed and how many Roblox refused; the ones changed, as they were before.
    done: Option<(usize, usize)>,
    before: Vec<roblox::OutfitParts>,
    undone: bool,
}

fn open_swap(r: &mut Root, cx: &mut Context<Root>) {
    let Some((_, user, cookie)) = friends::signed(r) else { return };
    let Some(Ok(a)) = &r.avatar.got else { return };
    let outfits = a.outfits.clone();
    r.avatar.swap = Some(Swap::default());
    cx.notify();
    let rx = roblox::run(move || roblox::outfit_parts(&cookie, &outfits));
    reply(rx, cx, move |r, got, cx| {
        if r.avatar.of != user {
            return;
        }
        if let Some(sw) = r.avatar.swap.as_mut() {
            sw.parts = Some(got);
            cx.notify();
        }
    });
}

/// The thing to change is picked: what else of its kind is owned is asked, and every outfit with it is ticked.
fn pick_from(r: &mut Root, id: i64, kind: i64, cx: &mut Context<Root>) {
    let Some((_, user, cookie)) = friends::signed(r) else { return };
    let Some(sw) = r.avatar.swap.as_mut() else { return };
    if sw.running.is_some() || sw.done.is_some() {
        return;
    }
    sw.from = Some(id);
    sw.with = None;
    sw.owned = None;
    sw.skip.clear();
    cx.notify();
    let rx = roblox::run(move || roblox::owned_of_kind(&cookie, user, kind));
    reply(rx, cx, move |r, got, cx| {
        if let Some(sw) = r.avatar.swap.as_mut().filter(|sw| sw.from == Some(id)) {
            sw.owned = Some(got);
            cx.notify();
        }
    });
}

/// The change made in every ticked outfit with the thing, one after another (Roblox's limits), then the avatar read
/// again so the outfits' pictures follow.
fn run_swap(r: &mut Root, cx: &mut Context<Root>) {
    let Some((_, user, cookie)) = friends::signed(r) else { return };
    let Some(sw) = r.avatar.swap.as_mut() else { return };
    let (Some(from), Some(with), Some(Ok(parts))) = (sw.from, sw.with, &sw.parts) else { return };
    if sw.running.is_some() {
        return;
    }
    let to = with.and_then(|id| match &sw.owned {
        Some(Ok((l, kind_name))) => l.iter().find(|o| o.id == id).map(|_| (id, kind_name.clone())),
        _ => None,
    });
    if with.is_some() && to.is_none() {
        return;
    }
    let kind = parts.iter().flat_map(|p| p.items.iter()).find(|i| i.id == from).map(|i| i.kind_id()).unwrap_or(0);
    let todo: Vec<roblox::OutfitParts> = parts.iter().filter(|p| p.items.iter().any(|i| i.id == from) && !sw.skip.contains(&p.id)).cloned().collect();
    if todo.is_empty() {
        return;
    }
    let total = todo.len();
    sw.running = Some((0, total));
    // (What each outfit held is kept as soon as it is changed, so Undo has it even if the run is cut short.)
    sw.before.clear();
    sw.undone = false;
    cx.notify();
    cx.spawn(async move |this, cx| {
        let (mut ok, mut bad) = (0, 0);
        for (i, outfit) in todo.into_iter().enumerate() {
            // (A breath between saves: Roblox turns away many in a row.)
            if i > 0 {
                cx.background_executor().timer(std::time::Duration::from_millis(300)).await;
            }
            let (c, o, t) = (cookie.clone(), outfit.clone(), to.clone().map(|(id, name)| (id, kind, name)));
            let done = roblox::run(move || roblox::swap_in_outfit(&c, &o, from, t)).await.unwrap_or_else(|_| Err(String::new()));
            if done.is_ok() {
                ok += 1;
            } else {
                bad += 1;
            }
            let on = this
                .update(cx, |r, cx| {
                    let Some(sw) = r.avatar.swap.as_mut().filter(|_| r.avatar.of == user) else { return false };
                    if done.is_ok() {
                        sw.before.push(outfit);
                    }
                    sw.running = Some((i + 1, total));
                    cx.notify();
                    true
                })
                .unwrap_or(false);
            if !on {
                return;
            }
        }
        let _ = this.update(cx, |r, cx| {
            if let Some(sw) = r.avatar.swap.as_mut() {
                sw.running = None;
                sw.done = Some((ok, bad));
            }
            if ok > 0 {
                load(r, cx);
            }
            cx.notify();
        });
    })
    .detach();
}

/// Every outfit changed goes back to what it held before.
fn undo_swap(r: &mut Root, cx: &mut Context<Root>) {
    let Some((_, user, cookie)) = friends::signed(r) else { return };
    let Some(sw) = r.avatar.swap.as_mut() else { return };
    if sw.running.is_some() || sw.before.is_empty() || sw.undone {
        return;
    }
    let back = sw.before.clone();
    let total = back.len();
    sw.running = Some((0, total));
    cx.notify();
    cx.spawn(async move |this, cx| {
        let mut bad = 0;
        for (i, outfit) in back.into_iter().enumerate() {
            if i > 0 {
                cx.background_executor().timer(std::time::Duration::from_millis(300)).await;
            }
            let c = cookie.clone();
            if !matches!(roblox::run(move || roblox::set_outfit_items(&c, outfit.id, &outfit.items)).await, Ok(Ok(()))) {
                bad += 1;
            }
            let on = this
                .update(cx, |r, cx| {
                    let Some(sw) = r.avatar.swap.as_mut().filter(|_| r.avatar.of == user) else { return false };
                    sw.running = Some((i + 1, total));
                    cx.notify();
                    true
                })
                .unwrap_or(false);
            if !on {
                return;
            }
        }
        let _ = this.update(cx, |r, cx| {
            if let Some(sw) = r.avatar.swap.as_mut() {
                sw.running = None;
                // (Some not put back: Undo stays, to try them all again; putting one back twice does no harm.)
                sw.undone = bad == 0;
            }
            if bad > 0 {
                r.toast = Some((r.i18n.tv("sw.undoSome", &[("n", &bad.to_string())]), "bad".into()));
            }
            load(r, cx);
            cx.notify();
        });
    })
    .detach();
}

/// A small square to pick (a thing, "take it out"): its picture or mark, its name and a line under it; ringed yellow
/// when picked.
fn pick_tile(id: impl Into<ElementId>, w: f32, url: &str, mark: Option<&'static str>, name: String, under: Option<String>, picked: bool, cx: &mut App) -> Stateful<Div> {
    let side = w - 18.;
    let pic = (!url.is_empty()).then(|| images::pic_pair(url, side, side, cx)).flatten();
    div()
        .id(id)
        .w(px(w))
        .flex()
        .flex_col()
        .gap(px(6.))
        .cursor_pointer()
        .child(
            div()
                .w(px(w))
                .h(px(w))
                .rounded(px(R_L))
                .bg(c(RAISED))
                .border_2()
                .border_color(if picked { c(FACE) } else { gpui_kit::transparent_black() })
                .hover(|d| d.bg(c(RAISED_2)))
                .flex()
                .items_center()
                .justify_center()
                .map(|d| match (pic, mark) {
                    (Some((src, key)), _) => d.child(fade_in(img(src).size(px(side)).object_fit(ObjectFit::Contain), &key)),
                    (None, Some(m)) => d.child(icon(m, 22., c(MUTED))),
                    (None, None) => d.child(icon("image", 20., c(DIM))),
                }),
        )
        .child(div().text_size(px(T_S)).line_height(px(16.)).text_color(c(if picked { TEXT } else { TEXT_2 })).line_clamp(2).child(name))
        .when_some(under, |d, u| d.child(div().text_size(px(T_XS)).text_color(c(MUTED)).child(u)))
}

/// The panel under Saved outfits: 1 the thing to change (the things the outfits hold, the most used first), 2 what goes
/// in its place (what is owned of that kind, or take it out), 3 the outfits it is changed in, then Change.
fn swap_panel(root: &Root, w: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let Some(sw) = &root.avatar.swap else { return div() };
    const T: f32 = 96.;
    let step = |n: &str, title: String| {
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(div().size(px(20.)).flex_none().rounded_full().bg(c(RAISED_2)).flex().items_center().justify_center().text_size(px(T_XS)).font_weight(FontWeight::BOLD).child(n.to_string()))
            .child(div().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).child(title))
    };
    let close = IconButton::new("sw-close", "x").size(30.).disabled(sw.running.is_some()).tip(root.tr("sw.close")).on_click(cx.listener(|r, _, _, cx| {
        r.avatar.swap = None;
        cx.notify();
    }));
    let mut panel = surface()
        .w(px(w))
        .p(px(16.))
        .flex()
        .flex_col()
        .gap(px(16.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .child(icon("shuffle", 16., c(MUTED)))
                .child(div().flex_1().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).child(root.tr("sw.title")))
                .child(close),
        );
    let parts = match &sw.parts {
        None => return panel.child(div().text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("sw.reading"))),
        Some(Err(why)) => return panel.child(div().text_size(px(T_M)).text_color(c(WARN)).child(root.i18n.tv("av.failed", &[("why", crate::net::why_words(&root.i18n, why).as_str())]))),
        Some(Ok(p)) => p,
    };
    // 1: the things the outfits hold, each once, those in the most outfits first.
    let mut things: Vec<(&roblox::Item, usize)> = Vec::new();
    for p in parts {
        for i in &p.items {
            match things.iter_mut().find(|(t, _)| t.id == i.id) {
                Some(t) => t.1 += 1,
                None => things.push((i, 1)),
            }
        }
    }
    things.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.name.cmp(&b.0.name)));
    let locked = sw.running.is_some() || sw.done.is_some();
    let firsts: Vec<AnyElement> = things
        .iter()
        .take(60)
        .map(|(i, n)| {
            let (id, kind) = (i.id, i.kind_id());
            pick_tile(("sw-from", id as u64 as usize), T, &i.pic, None, i.name.clone(), Some(root.i18n.tv("sw.inN", &[("n", &n.to_string())])), sw.from == Some(id), cx)
                .when(!locked, |d| d.on_click(cx.listener(move |r, _, _, cx| pick_from(r, id, kind, cx))))
                .into_any_element()
        })
        .collect();
    panel = panel.child(div().flex().flex_col().gap(px(10.)).child(step("1", root.tr("sw.step1"))).child(div().flex().flex_wrap().gap(px(10.)).children(firsts)));
    let Some(from) = sw.from else { return panel };
    let from_name = things.iter().find(|(t, _)| t.id == from).map(|(t, _)| t.name.clone()).unwrap_or_default();

    // 2: what goes in its place.
    let mut seconds: Vec<AnyElement> = vec![
        pick_tile("sw-none", T, "", Some("x"), root.tr("sw.takeOut"), None, sw.with == Some(None), cx)
            .when(!locked, |d| {
                d.on_click(cx.listener(|r, _, _, cx| {
                    if let Some(sw) = r.avatar.swap.as_mut() {
                        sw.with = Some(None);
                    }
                    cx.notify();
                }))
            })
            .into_any_element(),
    ];
    let mut owned_note: Option<String> = None;
    match &sw.owned {
        None => owned_note = Some(root.tr("sw.reading")),
        Some(Err(_)) => owned_note = Some(root.tr("sw.noOwned")),
        Some(Ok((l, _))) => {
            for o in l.iter().filter(|o| o.id != from).take(48) {
                let id = o.id;
                seconds.push(
                    pick_tile(("sw-to", id as u64 as usize), T, &o.pic, None, o.name.clone(), None, sw.with == Some(Some(id)), cx)
                        .when(!locked, |d| {
                            d.on_click(cx.listener(move |r, _, _, cx| {
                                if let Some(sw) = r.avatar.swap.as_mut() {
                                    sw.with = Some(Some(id));
                                }
                                cx.notify();
                            }))
                        })
                        .into_any_element(),
                );
            }
            if l.iter().all(|o| o.id == from) {
                owned_note = Some(root.tr("sw.noOwned"));
            }
        }
    }
    panel = panel.child(
        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(step("2", root.i18n.tv("sw.step2", &[("name", &from_name)])))
            .child(div().flex().flex_wrap().gap(px(10.)).children(seconds))
            .when_some(owned_note, |d, n| d.child(div().text_size(px(T_S)).text_color(c(MUTED)).child(n))),
    );
    let Some(with) = sw.with else { return panel };

    // 3: the outfits with it, ticked unless left out.
    let with_it: Vec<&roblox::OutfitParts> = parts.iter().filter(|p| p.items.iter().any(|i| i.id == from)).collect();
    let n_on = with_it.iter().filter(|p| !sw.skip.contains(&p.id)).count();
    let rows: Vec<AnyElement> = with_it
        .iter()
        .map(|p| {
            let id = p.id;
            let on = !sw.skip.contains(&id);
            let pic = (!p.pic.is_empty()).then(|| images::pic_sq(&p.pic, 40., cx)).flatten();
            div()
                .id(("sw-of", id as u64 as usize))
                .h(px(52.))
                .pl(px(6.))
                .pr(px(12.))
                .rounded(px(R_M))
                .bg(c(if on { RAISED_2 } else { RAISED }))
                .border_1()
                .border_color(if on { ca(FACE, 0.55) } else { c(LINE) })
                .flex()
                .items_center()
                .gap(px(10.))
                .cursor_pointer()
                .child(div().size(px(40.)).flex_none().rounded(px(R_S)).bg(c(INSET)).overflow_hidden().when_some(pic, |d, p| d.child(img(p).size_full().object_fit(ObjectFit::Contain))))
                .child(div().max_w(px(160.)).truncate().text_size(px(T_M)).text_color(c(if on { TEXT } else { MUTED })).child(p.name.clone()))
                .child(icon(if on { "check" } else { "plus" }, 14., c(if on { FACE } else { MUTED })))
                .when(!locked, |d| {
                    d.on_click(cx.listener(move |r, _, _, cx| {
                        if let Some(sw) = r.avatar.swap.as_mut() {
                            if !sw.skip.remove(&id) {
                                sw.skip.insert(id);
                            }
                        }
                        cx.notify();
                    }))
                })
                .into_any_element()
        })
        .collect();
    panel = panel.child(div().flex().flex_col().gap(px(10.)).child(step("3", root.i18n.tv("sw.step3", &[("n", &n_on.to_string()), ("all", &with_it.len().to_string())]))).child(div().flex().flex_wrap().gap(px(8.)).children(rows)));

    // The foot: what will happen, and Change; under way, how far; done, how it went and Undo.
    let to_name = with.map(|id| match &sw.owned {
        Some(Ok((l, _))) => l.iter().find(|o| o.id == id).map(|o| o.name.clone()).unwrap_or_default(),
        _ => String::new(),
    });
    let said = match (sw.running, sw.done) {
        (Some((d, t)), _) => root.i18n.tv(if sw.done.is_some() { "sw.undoing" } else { "sw.running" }, &[("d", &d.to_string()), ("t", &t.to_string())]),
        (None, Some((ok, _))) if sw.undone => root.i18n.tv("sw.undone", &[("n", &ok.to_string())]),
        (None, Some((ok, 0))) => root.i18n.tv("sw.doneAll", &[("n", &ok.to_string())]),
        (None, Some((ok, bad))) => root.i18n.tv("sw.doneSome", &[("n", &ok.to_string()), ("bad", &bad.to_string())]),
        (None, None) => match &to_name {
            Some(t) => root.i18n.tv("sw.plan", &[("from", &from_name), ("to", t), ("n", &n_on.to_string())]),
            None => root.i18n.tv("sw.planOut", &[("from", &from_name), ("n", &n_on.to_string())]),
        },
    };
    let mut foot = div().pt(px(14.)).border_t_1().border_color(c(LINE)).flex().items_center().gap(px(10.)).child(div().flex_1().min_w_0().text_size(px(T_M)).text_color(c(TEXT_2)).child(said));
    foot = match (sw.running, sw.done) {
        (Some(_), _) => foot.child(Button::new("sw-wait", root.tr("sw.wait")).disabled(true)),
        (None, Some((ok, _))) => foot
            .when(ok > 0 && !sw.undone, |d| d.child(Button::new("sw-undo", root.tr("sw.undo")).icon("history").on_click(cx.listener(|r, _, _, cx| undo_swap(r, cx)))))
            .child(Button::new("sw-done", root.tr("sw.finish")).primary().on_click(cx.listener(|r, _, _, cx| {
                r.avatar.swap = None;
                cx.notify();
            }))),
        (None, None) => foot.child(Button::new("sw-go", root.i18n.tv("sw.go", &[("n", &n_on.to_string())])).primary().disabled(n_on == 0).on_click(cx.listener(|r, _, _, cx| run_swap(r, cx)))),
    };
    panel.child(foot)
}

