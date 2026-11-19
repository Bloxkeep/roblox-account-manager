// WikiLook.cs: how each game looks in the Wiki tab.
//! Each game's look in the Wiki tab (WikiLookInfo), from the Roblox place the bot names for it. The pictures
//! come back as soon as Roblox gives their addresses; the colour, which needs each icon downloaded, is worked out after
//! (the tab asks again a moment later). All of it is kept on disk for a week, so the tab opens with its pictures at once
//! (2026-11-06: the owner found them "super slow", every game's icon downloaded before any picture showed, each app start).
//! Nothing when Roblox doesn't answer: the tab then shows the game in the app's own colours, and it is asked again later.
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::core::base::{Cancel, Res, Stamp};
use crate::core::roblox_api::{
    self as api, ARGUMENT_EXCEPTION, HTTP_REQUEST_EXCEPTION, HttpRequest, INVALID_OPERATION_EXCEPTION, TASK_CANCELED_EXCEPTION, fail_of, is_fail_kind, lock,
};
use crate::core::{log, vault};

/// How a game looks in the Wiki tab: its icon and wide picture from its Roblox page, and a colour taken from the
/// icon (its strongest hue, made bright enough to read on), with black or white words for on top of that colour.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct WikiLookInfo {
    pub icon: Option<String>,
    pub cover: Option<String>,
    pub tint: Option<String>,
    pub ink: Option<String>,
}

static HTTP: LazyLock<ureq::Agent> = LazyLock::new(|| crate::net::builder().timeout(Duration::from_secs(10)).build());
const KEPT_FOR_SECS: f64 = 7.0 * 24.0 * 3600.0;

/// DateTime's default (0001-01-01), for a kept entry without its time.
fn min_stamp() -> Stamp {
    Stamp::from_unix_secs(-62_135_596_800)
}

/// A place's look and when it was asked of Roblox; tinted once its icon's colour is worked out (none is a
/// colour too: an icon with next to none).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
struct Kept {
    icon: Option<String>,
    cover: Option<String>,
    tint: Option<String>,
    ink: Option<String>,
    tinted: bool,
    at: Stamp,
}

impl Default for Kept {
    fn default() -> Self {
        Kept { icon: None, cover: None, tint: None, ink: None, tinted: false, at: min_stamp() }
    }
}

static KNOWN: LazyLock<Mutex<HashMap<i64, Kept>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
static GATE: Mutex<()> = Mutex::new(());
static LOADED: AtomicBool = AtomicBool::new(false);
/// Places whose icon colour is being worked out now: a second look meanwhile doesn't queue them (and download them) again.
static TINTING: LazyLock<Mutex<std::collections::HashSet<i64>>> = LazyLock::new(|| Mutex::new(std::collections::HashSet::new()));

fn file_path() -> std::path::PathBuf {
    vault::cache_dir().join("game-looks.json")
}

pub fn of(places: impl IntoIterator<Item = i64>) -> Res<HashMap<i64, WikiLookInfo>> {
    let mut wanted: Vec<i64> = Vec::new();
    for p in places {
        if p > 0 && !wanted.contains(&p) {
            wanted.push(p);
        }
    }
    // Only the first read of the kept list is one at a time: the Roblox lookups below (up to ~30 s) no longer make
    // another call wait for them.
    {
        let _gate = lock(&GATE);
        load();
    }
    let missing: Vec<i64> = {
        let known = lock(&KNOWN);
        wanted
            .iter()
            .copied()
            .filter(|p| match known.get(p) {
                None => true,
                Some(k) => k.at.age_secs() > if k.icon.is_none() && k.cover.is_none() { 5.0 * 60.0 } else { KEPT_FOR_SECS },
            })
            .collect()
    };
    if !missing.is_empty() {
        let ct = Cancel::new();
        let ct = &ct;
        // Four at a time, as Games looks them up (SemaphoreSlim(4)), not a thread per place (up to ~100).
        let queue = Mutex::new(missing.iter().copied().enumerate());
        let answers: Mutex<Vec<Option<Res<i64>>>> = Mutex::new(vec![None; missing.len()]);
        crate::threads::scope(|s| {
            for _ in 0..4 {
                s.spawn(|| {
                    loop {
                        let Some((i, place)) = lock(&queue).next() else { break };
                        // A lookup that panicked counts as not found, as a failed thread's join did.
                        let got = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| try_or(api::get_universe_id(place, ct), 0i64))).unwrap_or(Ok(0));
                        lock(&answers)[i] = Some(got);
                    }
                });
            }
        });
        let answers = answers.into_inner().unwrap_or_else(|e| e.into_inner());
        let universes: Vec<(i64, i64)> = missing.iter().zip(answers).map(|(&p, got)| got.unwrap_or(Ok(0)).map(|u| (p, u))).collect::<Res<Vec<_>>>()?;
        let ids: Vec<i64> = universes.iter().map(|u| u.1).filter(|u| *u > 0).collect();
        let (icons, covers) = crate::threads::scope(|s| {
            let icons = s.spawn(|| try_or(api::get_game_icons(ids.clone(), ct), HashMap::new()));
            let covers = s.spawn(|| try_or(api::get_game_covers(ids.clone(), ct), HashMap::new()));
            (icons.join().unwrap_or(Ok(HashMap::new())), covers.join().unwrap_or(Ok(HashMap::new())))
        });
        let (icons, covers) = (icons?, covers?);
        {
            let mut known = lock(&KNOWN);
            for (place, universe) in &universes {
                let (icon, cover) = (icons.get(universe).cloned(), covers.get(universe).cloned());
                // The colour stays while the icon does.
                let kept = match known.get(place) {
                    // Nothing came back (offline, Roblox busy): the pictures known before stay, asked again later.
                    Some(had) if icon.is_none() && cover.is_none() && (had.icon.is_some() || had.cover.is_some()) => {
                        Kept { at: Stamp::utc_now(), ..had.clone() }
                    }
                    Some(had) if had.tinted && had.icon == icon && icon.is_some() => Kept { cover, at: Stamp::utc_now(), ..had.clone() },
                    _ => Kept { tinted: icon.is_none(), icon, cover, tint: None, ink: None, at: Stamp::utc_now() },
                };
                known.insert(*place, kept);
            }
        }
        save();
    }
    let untinted: Vec<(i64, String)> = {
        let known = lock(&KNOWN);
        let mut tinting = lock(&TINTING);
        wanted
            .iter()
            .filter_map(|p| known.get(p).filter(|k| !k.tinted).and_then(|k| k.icon.clone()).map(|icon| (*p, icon)))
            .filter(|(p, _)| tinting.insert(*p))
            .collect()
    };
    if !untinted.is_empty() {
        crate::threads::run(move || tint_all(untinted));
    }
    let known = lock(&KNOWN);
    Ok(wanted
        .iter()
        .filter_map(|p| {
            known.get(p).map(|k| (*p, WikiLookInfo { icon: k.icon.clone(), cover: k.cover.clone(), tint: k.tint.clone(), ink: k.ink.clone() }))
        })
        .collect())
}

/// The icons' colours, a few at a time, kept as each is worked out.
fn tint_all(icons: Vec<(i64, String)>) {
    let queue = Mutex::new(icons);
    crate::threads::scope(|s| {
        for _ in 0..6 {
            s.spawn(|| {
                loop {
                    // Take from the front, as the C#'s tasks start in order.
                    let next = {
                        let mut q = lock(&queue);
                        if q.is_empty() { None } else { Some(q.remove(0)) }
                    };
                    let Some((place, icon)) = next else { break };
                    // Off the list however this one ends: the next look may queue it again if it stayed untinted.
                    let _done = crate::core::base::on_drop(|| {
                        lock(&TINTING).remove(&place);
                    });
                    // An error the C# didn't catch went unseen in its Task.Run: the icon stays untinted, asked again next time.
                    let Ok((tint, ink)) = try_or(tint_url(&icon), (None, None)) else { continue };
                    let mut known = lock(&KNOWN);
                    if let Some(k) = known.get_mut(&place)
                        && k.icon.as_deref() == Some(icon.as_str())
                    {
                        k.tint = tint;
                        k.ink = ink;
                        k.tinted = true;
                    }
                }
            });
        }
    });
    save();
}

fn load() {
    if LOADED.swap(true, Ordering::SeqCst) {
        return;
    }
    let path = file_path();
    if !path.is_file() {
        return;
    }
    let result = std::fs::read(&path).map_err(|e| e.to_string()).and_then(|bytes| {
        let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes).to_vec();
        serde_json::from_slice::<Option<HashMap<i64, Kept>>>(&bytes).map_err(|e| e.to_string())
    });
    match result {
        Ok(Some(kept)) => {
            let mut known = lock(&KNOWN);
            for (place, k) in kept {
                known.entry(place).or_insert(k);
            }
        }
        Ok(None) => {}
        Err(message) => log::note(&format!("Game pictures: the kept list couldn't be read ({message}); asked of Roblox again.")),
    }
}

/// Only what Roblox answered is kept (a game it didn't is asked again next time).
/// One at a time (a lookup's and the colours' thread both save), so a newer list is never written over by an older one.
fn save() {
    static SAVING: Mutex<()> = Mutex::new(());
    let _one = lock(&SAVING);
    let text = {
        let known = lock(&KNOWN);
        let kept: HashMap<&i64, &Kept> = known.iter().filter(|(_, k)| k.icon.is_some() || k.cover.is_some()).collect();
        serde_json::to_string(&kept).unwrap_or_else(|_| "{}".into())
    };
    let done = std::fs::create_dir_all(vault::cache_dir())
        .map_err(|e| e.to_string())
        .and_then(|_| crate::core::durable_file::write_text(file_path(), &text).map_err(|e| e.to_string()));
    if let Err(e) = done {
        log::note(&format!("Game pictures: the list couldn't be kept ({e})."));
    }
}

/// The C#'s Try: the failures it names give `otherwise`; any other still fails.
fn try_or<T>(got: Res<T>, otherwise: T) -> Res<T> {
    match got {
        Err(e)
            if is_fail_kind(&e, HTTP_REQUEST_EXCEPTION)
                || is_fail_kind(&e, TASK_CANCELED_EXCEPTION)
                || is_fail_kind(&e, ARGUMENT_EXCEPTION)
                || is_fail_kind(&e, INVALID_OPERATION_EXCEPTION) =>
        {
            Ok(otherwise)
        }
        other => other,
    }
}

fn tint_url(icon_url: &str) -> Res<(Option<String>, Option<String>)> {
    let res = api::send_on(&HTTP, &HttpRequest::new("GET", icon_url))?;
    if !res.is_success() {
        return Err(fail_of(HTTP_REQUEST_EXCEPTION, format!("Response status code does not indicate success: {}.", res.status)));
    }
    // new Bitmap(stream) on a picture it can't read: ArgumentException.
    // PORT: GDI+ read PNG, JPEG, GIF and BMP; the image crate reads the formats its features name.
    let picture = image::load_from_memory(&res.body).map_err(|e| fail_of(ARGUMENT_EXCEPTION, e))?.to_rgba8();
    Ok(tint(&picture))
}

/// Color.GetHue: degrees from the bytes, 0 for a grey.
fn get_hue(r: u8, g: u8, b: u8) -> f32 {
    if r == g && g == b {
        return 0.0;
    }
    let (r, g, b) = (r as f32, g as f32, b as f32);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let mut hue = if r == max {
        (g - b) / delta
    } else if g == max {
        (b - r) / delta + 2.0
    } else {
        (r - g) / delta + 4.0
    };
    hue *= 60.0;
    if hue < 0.0 {
        hue += 360.0;
    }
    hue
}

/// The icon's strongest hue: each pixel's hue weighted by how coloured and how bright it is (greys, black and
/// white count for nothing), as a colour of medium lightness. None for an icon with next to no colour.
pub(crate) fn tint(bitmap: &image::RgbaImage) -> (Option<String>, Option<String>) {
    let (width, height) = (bitmap.width() as i32, bitmap.height() as i32);
    let (mut x, mut y, mut weight, mut sat, mut seen) = (0f64, 0f64, 0f64, 0f64, 0f64);
    let step = 1.max(width.min(height) / 60);
    let mut i = 0;
    while i < width {
        let mut j = 0;
        while j < height {
            let c = bitmap.get_pixel(i as u32, j as u32).0;
            j += step;
            if c[3] < 128 {
                continue;
            }
            seen += 1.0;
            let max = c[0].max(c[1]).max(c[2]) as f64 / 255.0;
            let min = c[0].min(c[1]).min(c[2]) as f64 / 255.0;
            let s = if max == 0.0 { 0.0 } else { (max - min) / max };
            let w = s * s * max;
            if w < 0.02 {
                continue;
            }
            let h = get_hue(c[0], c[1], c[2]) as f64 * std::f64::consts::PI / 180.0;
            x += h.cos() * w;
            y += h.sin() * w;
            sat += s * w;
            weight += w;
        }
        i += step;
    }
    if seen == 0.0 || weight / seen < 0.03 {
        return (None, None);
    }
    let hue = (y.atan2(x) * 180.0 / std::f64::consts::PI + 360.0) % 360.0;
    let saturation = (sat / weight).clamp(0.45, 0.8);
    let (r, g, b) = from_hsl(hue, saturation, 0.55);
    let luminance = 0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b);
    (Some(format!("#{:02x}{:02x}{:02x}", r, g, b)), Some(if luminance > 0.2 { "#111111" } else { "#ffffff" }.into()))
}

fn linear(c: i32) -> f64 {
    let v = c as f64 / 255.0;
    if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
}

fn from_hsl(h: f64, s: f64, l: f64) -> (i32, i32, i32) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = if hp < 1.0 {
        (c, x, 0.0)
    } else if hp < 2.0 {
        (x, c, 0.0)
    } else if hp < 3.0 {
        (0.0, c, x)
    } else if hp < 4.0 {
        (0.0, x, c)
    } else if hp < 5.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    (((r + m) * 255.0).round_ties_even() as i32, ((g + m) * 255.0).round_ties_even() as i32, ((b + m) * 255.0).round_ties_even() as i32)
}
