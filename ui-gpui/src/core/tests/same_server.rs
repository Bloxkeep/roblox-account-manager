// Settings' "Rejoin the same server": a window dropped from a public server is queued to rejoin that server, and goes to
// the game (any server) when that server closed, the game sent it out in words, it is a private server or a followed
// player, the drop came before it got in, or it was just rejoined into that very server and dropped again. Off, every
// rejoin goes to the game as before. Only the queue is read: nothing opens Roblox. Last, auto clean RAM with no Roblox
// window open: it doesn't clean.
use std::{collections::HashSet, rc::Rc};

use serde_json::json;

use super::{
    Checks, call,
    rejoin::{no_network, quiet_app, seed},
    unlock,
};
use crate::core::{
    app::{App, controller::PendingLaunch, rejoin::same_server},
    base::{Guid, Stamp, new_guid},
    close_plan::End,
    game_target::{FollowedPlayer, GameTarget},
    models::{Account, stamp_min},
    system_memory::MemoryStatus,
    roblox_log_watcher::ClientState,
};

const PLACE: i64 = 4242;
const OTHER_PLACE: i64 = 4343;
const SERVER: &str = "11111111-2222-3333-4444-555555555555";
const NEXT: &str = "66666666-7777-8888-9999-000000000000";

fn launch(id: Guid, target: GameTarget) -> Rc<PendingLaunch> {
    Rc::new(PendingLaunch::new(id, HashSet::new(), Stamp::utc_now().plus_secs(-120.0), Some(target), false, None))
}

/// Its log at the end: `status` in place `place`, server `job`, Roblox's code `code`.
fn log(status: &str, place: i64, job: Option<&str>, code: Option<i32>) -> ClientState {
    ClientState::new(status, Some(place), job.map(str::to_string), code, Stamp::utc_now())
}

/// The server the one rejoin queued goes to ("" for the game); None when nothing was queued.
fn queued(app: &App, id: Guid, launch: &Rc<PendingLaunch>, last: &ClientState) -> Option<String> {
    app.controller.rejoin_queue.borrow_mut().clear();
    app.controller.rejoins_in_a_row.borrow_mut().remove(&id);
    app.queue_rejoin(id, launch, Some(last), End::Unasked);
    let q = app.controller.rejoin_queue.borrow();
    q.first().map(|r| r.target.job_id.clone().unwrap_or_default())
}

pub async fn run(c: &mut Checks, _dir: &str) {
    no_network();
    let id = new_guid();
    seed(vec![Account { id, username: "Dropper".into(), user_id: 51, cookie: "cookie-dropper".into(), cookie_valid: Some(true), auto_rejoin: true, ..Default::default() }]);
    let (app, _said) = quiet_app();
    unlock(&app).await;
    let game = GameTarget::new(PLACE, None, None, None);
    let opened = launch(id, game.clone());

    // ── the rule alone ──
    let dropped = log("disconnected", PLACE, Some(SERVER), Some(277));
    c.check("a drop from a public server: back to that server", same_server(&game, &opened, Some(&dropped), false).as_deref() == Some(SERVER), ());
    let crashed = log("ingame", PLACE, Some(SERVER), None);
    c.check("a crash in its game (no disconnect line): back to that server", same_server(&game, &opened, Some(&crashed), false).as_deref() == Some(SERVER), ());
    for code in [271, 274, 275, 287, 288, 309] {
        let closed = log("disconnected", PLACE, Some(SERVER), Some(code));
        c.check(&format!("the server closed (code {code}): the game"), same_server(&game, &opened, Some(&closed), false).is_none(), ());
    }
    let mut kicked = log("disconnected", PLACE, Some(SERVER), Some(267));
    kicked.kick_message = Some("Server is restarting, please rejoin".into());
    c.check("sent out by the game in words: the game", same_server(&game, &opened, Some(&kicked), false).is_none(), ());
    let idle = log("disconnected", PLACE, Some(SERVER), Some(278));
    c.check("kicked for being idle: back to that server", same_server(&game, &opened, Some(&idle), false).as_deref() == Some(SERVER), ());
    let joining = log("joining", PLACE, None, None);
    c.check("never got in: the game", same_server(&game, &opened, Some(&joining), false).is_none(), ());
    let failed_join = log("joining", PLACE, Some(SERVER), None);
    c.check("still joining a server named: the game", same_server(&game, &opened, Some(&failed_join), false).is_none(), ());
    let elsewhere = log("disconnected", OTHER_PLACE, Some(SERVER), Some(277));
    c.check("a server of another place (a teleport into a sub-place): the game", same_server(&game, &opened, Some(&elsewhere), false).is_none(), ());
    c.check("no log: the game", same_server(&game, &opened, None, false).is_none(), ());
    let private = GameTarget::new(PLACE, None, Some("link-code".into()), None);
    c.check("a private server: what it was opened into", same_server(&private, &launch(id, private.clone()), Some(&dropped), false).is_none(), ());
    let follow = GameTarget { follow: Some(FollowedPlayer { user_id: 9, name: "Friend".into() }), ..game.clone() };
    c.check("following a player: the player is looked up", same_server(&follow, &launch(id, follow.clone()), Some(&dropped), false).is_none(), ());
    let into_server = launch(id, GameTarget::new(PLACE, Some(SERVER.into()), None, None));
    c.check("just rejoined into that server and dropped again: the game", same_server(&game, &into_server, Some(&dropped), true).is_none(), ());
    c.check("rejoined into it, played past settling (the count reset): that server again", same_server(&game, &into_server, Some(&dropped), false).as_deref() == Some(SERVER), ());
    let moved = log("disconnected", PLACE, Some(NEXT), Some(277));
    c.check("rejoined into one, moved to another by the game, dropped there: the new one", same_server(&game, &into_server, Some(&moved), true).as_deref() == Some(NEXT), ());

    // ── the queue, with the setting ──
    let settings = app.vault(|d| d.settings.clone()).ok();
    let shown = settings.map(|s| app.settings_dto(&s)).and_then(|v| v["rejoinSameServer"].as_bool());
    c.check("on by default, and the page is told", shown == Some(true), shown);
    let q = queued(&app, id, &opened, &dropped);
    c.check("a drop is queued into its server", q.as_deref() == Some(SERVER), &q);
    let q = queued(&app, id, &opened, &log("disconnected", PLACE, Some(SERVER), Some(288)));
    c.check("…a shut-down server is queued into the game", q.as_deref() == Some(""), &q);
    app.controller.rejoin_queue.borrow_mut().clear();
    app.controller.rejoins_in_a_row.borrow_mut().insert(id, 1);
    app.queue_rejoin(id, &into_server, Some(&dropped), End::Unasked);
    let q = app.controller.rejoin_queue.borrow().first().map(|r| r.target.job_id.clone().unwrap_or_default());
    c.check("…a second drop in a row from the server it was rejoined into: the game", q.as_deref() == Some(""), &q);
    // Never got into the chosen server (full or ended): the next rejoin goes to the game.
    let q = queued(&app, id, &into_server, &joining);
    c.check("…a rejoin that never got into its server: the game", q.as_deref() == Some(""), &q);

    let _ = call(&app, "setSettings", json!({ "rejoinSameServer": false })).await;
    let on = app.vault(|d| d.settings.rejoin_same_server).ok();
    c.check("turned off in Settings", on == Some(false), on);
    let q = queued(&app, id, &opened, &dropped);
    c.check("off: a drop goes to the game as before", q.as_deref() == Some(""), &q);
    let _ = call(&app, "setSettings", json!({ "rejoinSameServer": true })).await;
    app.controller.rejoin_queue.borrow_mut().clear();

    // Auto clean RAM with no Roblox window: nothing is cleaned, however full the RAM and however long the timer ran.
    // (With one open it would run Windows' clean through its task: not run here.)
    let _ = app.vault_mut(|d| {
        d.settings.auto_clean_at = 50;
        d.settings.auto_clean_every = 5;
    });
    app.controller.auto_clean_since.set(Stamp::utc_now().plus_secs(-3600.0));
    let full = MemoryStatus { load_percent: 99, ..Default::default() };
    app.guard_resources(&[], &full, Stamp::utc_now());
    c.check("auto clean RAM: no Roblox window, no clean", app.controller.last_auto_clean.get() == stamp_min(), ());
    c.check("…and its timer starts again when one opens", app.controller.auto_clean_since.get() == stamp_min(), ());
    let _ = app.vault_mut(|d| {
        d.settings.auto_clean_at = 0;
        d.settings.auto_clean_every = 0;
    });
}
