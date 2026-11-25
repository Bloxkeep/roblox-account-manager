// Bloxkeep's own badges on a profile, in a row under the name, as Discord shows its badges: how long the account has
// been on Roblox, how many hours (its own profile) and how many games it has played, each the highest step reached,
// its words under the mouse. The pictures are Fluent Emoji 3D objects (assets/deco, NOTICE.txt there), drawn as the decorations are (deco::still).
use gpui_kit::{prelude::*, *};

use crate::{kit::Tip, pages::deco, shell::Root};

/// Years on Roblox: a seedling, a tree, a star, a trophy, a crown.
const AGE: [(i64, &str); 5] = [(1, "seedling"), (3, "deciduous_tree"), (5, "glowing_star"), (7, "trophy"), (10, "crown")];
/// Hours played: a game pad, an alarm clock, a flame, a rocket, a gem.
const HOURS: [(i64, &str); 5] = [(10, "video_game"), (50, "alarm_clock"), (100, "fire"), (500, "rocket"), (1000, "gem_stone")];
/// Games played: a ticket, a compass, a map, the globe, the Milky Way.
const GAMES: [(i64, &str); 5] = [(10, "admission_tickets"), (50, "compass"), (100, "world_map"), (250, "globe"), (500, "milky_way")];

const SIDE: f32 = 22.;

/// The highest step `n` reached, if any.
fn step(steps: &[(i64, &'static str)], n: i64) -> Option<&'static str> {
    steps.iter().rev().find(|s| n >= s.0).map(|s| s.1)
}

fn medal(id: &'static str, name: &'static str, words: String, cx: &App) -> Option<AnyElement> {
    let pic = deco::still(name, cx)?;
    Some(
        div()
            .id(id)
            .size(px(SIDE))
            .flex_none()
            .child(img(ImageSource::Render(pic)).size_full())
            .tooltip(move |_, cx| cx.new(|_| Tip(words.clone().into())).into())
            .into_any_element(),
    )
}

/// The row: `joined` as Roblox writes it (2019-08-20T…), `hours` the minutes played kept from Roblox's Screen time and
/// the first day kept (the account's own profile only), `games` the games their badges came from and whether every
/// page of them was read (None while counted, or when Roblox doesn't tell). None when no step is reached.
pub fn row(root: &Root, joined: &str, hours: Option<&(i64, String)>, games: Option<(i64, bool)>, cx: &App) -> Option<AnyElement> {
    let mut all: Vec<AnyElement> = Vec::new();
    if let Some(at) = crate::time::iso_secs(joined) {
        let years = ((crate::time::now_secs() - at) / 86_400).max(0) / 365;
        if let Some(name) = step(&AGE, years) {
            let day = super::profile::day(joined);
            all.extend(medal("pf-medal-age", name, root.i18n.tn("pf.medalAge", years, &[("d", &day)]), cx));
        }
    }
    if let Some((minutes, since)) = hours
        && let Some(name) = step(&HOURS, minutes / 60)
    {
        let day = super::profile::day(since);
        all.extend(medal("pf-medal-hours", name, root.i18n.tn("pf.medalHours", minutes / 60, &[("d", &day)]), cx));
    }
    if let Some((n, whole)) = games
        && let Some(name) = step(&GAMES, n)
    {
        all.extend(medal("pf-medal-games", name, root.i18n.tn(if whole { "pf.medalGames" } else { "pf.medalGamesMore" }, n, &[]), cx));
    }
    (!all.is_empty()).then(|| div().flex().items_center().gap(px(4.)).pt(px(4.)).children(all).into_any_element())
}
