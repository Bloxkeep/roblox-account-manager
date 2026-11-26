// Tests/Bloxkeep.Tests/RejoinSuite.cs: auto-rejoin on a client that started its game and never got a server, the leave
// Roblox logs when Bloxkeep closes a client, the servers a party is sent to, teleports, games the person starts in the
// window, kicks and bans in words and codes, Roblox's join answers, playability, windows Roblox closes, memory and the
// launch's wait between accounts. Made-up Roblox logs held open by this process (the log watcher ties a log to whoever
// holds it), in a folder of the test's own; Bloxkeep's close refuses this process (not a Roblox client), so nothing is
// closed. Roblox's join and playability answers come from here; any other request to Roblox is refused (no network).
//
// Also the helpers the other suites of this group share (an App whose process timer doesn't tick, as the C#'s WinForms
// timer never ticked without a message loop; a vault seeded with accounts; the last toast; no network).
use std::{
    cell::RefCell,
    collections::HashSet,
    io::{Seek, SeekFrom, Write},
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicI32, Ordering},
    },
    time::{Duration, Instant},
};

use futures::FutureExt;
use serde_json::{Value, json};

use super::{Checks, PASSWORD, Said, app, unlock};
use crate::core::{
    app::{
        App,
        controller::{PendingLaunch, RejoinRequest},
        rejoin::JoinCheck,
    },
    base::{Cancel, Guid, Stamp},
    close_plan::{Cause, DropWhy, End, Then},
    game_target::GameTarget,
    models::{Account, GameBan, LaunchPreset, stamp_min},
    multi_instance,
    roblox_api::{self, GameInfo, HttpResponse, JoinAnswer, Playability, Presence, ServerInfo},
    roblox_log_watcher::{self, ClientState},
    rt,
    system_memory::MemoryStatus,
    vault::Vault,
};

// ───────────── helpers shared by this group's suites ─────────────

/// An App with no window whose process timer doesn't tick (the C#'s WinForms timer never ticked without a message loop):
/// nothing reads the real Roblox processes behind the checks' backs.
pub(super) fn quiet_app() -> (Rc<App>, Said) {
    let (app, said) = app();
    app.controller.proc_interval_ms.set(24 * 3600 * 1000);
    (app, said)
}

/// Every request to Roblox refused as if offline: a check never reaches the network by mistake.
pub(super) fn no_network() {
    *roblox_api::STAND_IN.write().unwrap_or_else(|e| e.into_inner()) =
        Some(Box::new(|req| Err(roblox_api::fail_of(roblox_api::HTTP_REQUEST_EXCEPTION, format!("no network in the checks ({})", req.url)))));
}

/// A vault made with the harness password holding these accounts (unlocked afterwards by `unlock`).
pub(super) fn seed(accounts: Vec<Account>) {
    seed_with(accounts, |_| {});
}

/// The same, with more of the vault's data set before it is saved.
pub(super) fn seed_with(accounts: Vec<Account>, more: impl FnOnce(&mut crate::core::models::VaultData)) {
    let vault = Vault::create(PASSWORD, None).expect("a vault");
    {
        let mut d = vault.data();
        d.accounts.extend(accounts);
        more(&mut d);
    }
    vault.save().expect("saved");
}

/// The last toast since the last call (the page's copy of a notification); forgets what was said.
pub(super) fn told(said: &Said) -> Option<String> {
    let mut last = None;
    for line in said.borrow().iter() {
        if let Ok(v) = serde_json::from_str::<Value>(line)
            && v["name"] == "toast"
        {
            last = v["data"]["text"].as_str().map(str::to_string);
        }
    }
    said.borrow_mut().clear();
    last
}

/// The account as the vault holds it now.
pub(super) fn acc(app: &App, id: Guid) -> Account {
    app.find_account(id).expect("the account")
}

/// A refusal's words, whatever the command answers (user_error for any result type).
pub(super) fn refusal<T>(result: &crate::core::base::Res<T>) -> Option<String> {
    match result {
        Err(crate::core::base::Fail::User(m)) => Some(m.clone()),
        _ => None,
    }
}

pub(super) fn answer(http: i32, status: Option<i32>, message: Option<&str>) -> JoinAnswer {
    JoinAnswer { http, status, message: message.map(str::to_string), ..Default::default() }
}

/// A join check whose answer came (Some) or is still asked (None).
pub(super) fn check_with(answer: Option<Option<JoinAnswer>>, at: Stamp) -> JoinCheck {
    JoinCheck { answer: Rc::new(RefCell::new(answer)), at }
}

/// A Roblox log line as client 0.743 writes it.
pub(super) fn log_line(at: Stamp, text: &str) -> String {
    let c = at.utc();
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z,1.0,abcd,6 {text}\n",
        c.year,
        c.month,
        c.day,
        c.hour,
        c.minute,
        c.second,
        at.unix_ms().rem_euclid(1000)
    )
}

/// "yyyyMMddTHHmmss" in UTC, as Roblox names its logs.
pub(super) fn log_stamp(at: Stamp) -> String {
    let c = at.utc();
    format!("{:04}{:02}{:02}T{:02}{:02}{:02}", c.year, c.month, c.day, c.hour, c.minute, c.second)
}

fn now() -> Stamp {
    Stamp::utc_now()
}

const LAUNCH: &str = "[FLog::SingleSurfaceApp] launchUGCGameInternal";
const JOIN: &str = "[FLog::Output] ! Joining game '11111111-2222-3333-4444-555555555555' place 107778070777162 at 10.0.0.1";
const LEAVE: &str = "[FLog::SingleSurfaceApp] leaveUGCGameInternal";
const DEAD_SERVER: &str = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";
const PLACE: i64 = 107778070777162;
const GPO: i64 = 1730877806;
const BAN_LINE: &str = r#"Error [FLog::CreatorError] Error: Server Kick Message: ; Metadata: {"moderatorType":"creator","dynamicReasonTextFilterStatus":"2","beginTimestamp":"2026-10-21T00:07:04Z"}"#;

/// A place known as its game's start place, with no name or universe of its own.
fn start_place_of_its_own(app: &App, place: i64) {
    app.controller.games.borrow_mut().insert(place, Some(GameInfo { place_id: place, ..Default::default() }));
}

fn game(place: i64) -> Option<GameTarget> {
    Some(GameTarget::new(place, None, None, None))
}

struct R {
    app: Rc<App>,
    said: Said,
    me: i32,
    id: Guid,
    log: std::fs::File,
}

impl R {
    fn edit(&self, f: impl FnOnce(&mut Account)) {
        self.app.update_account(self.id, f);
    }

    fn acc(&self) -> Account {
        acc(&self.app, self.id)
    }

    fn label(&self) -> String {
        self.acc().label()
    }

    fn told(&self) -> Option<String> {
        told(&self.said)
    }

    /// The account's window, opened `started` ago into `target`: this process stands in for its client.
    fn run_at(&self, target: Option<GameTarget>, started: Stamp) -> Rc<PendingLaunch> {
        let launch = Rc::new(PendingLaunch::new(self.id, HashSet::new(), started, target, false, None));
        self.app.controller.running.borrow_mut().insert(self.id, (self.me, launch.clone()));
        launch
    }

    fn run(&self, target: Option<GameTarget>) -> Rc<PendingLaunch> {
        self.run_at(target, now().plus_secs(-120.0))
    }

    fn write(&mut self, text: &str) {
        self.log.seek(SeekFrom::End(0)).expect("seek");
        self.log.write_all(text.as_bytes()).expect("write");
        self.log.flush().expect("flush");
    }

    fn line(&mut self, at: Stamp, text: &str) {
        self.write(&log_line(at, text));
    }

    fn state(&self) -> Option<ClientState> {
        self.app.controller.logs.borrow().state_of(self.me)
    }

    /// The log's state once it has read the line just written (the first read waits for the log to be found).
    async fn poll(&self, read: impl Fn(&ClientState) -> bool) -> Option<ClientState> {
        let begun = Instant::now();
        while begun.elapsed() < Duration::from_secs(15) {
            self.app.controller.logs.borrow_mut().poll(&[self.me]);
            if let Some(s) = self.state()
                && read(&s)
            {
                return Some(s);
            }
            rt::sleep(Duration::from_millis(200)).await;
        }
        self.state()
    }

    /// RejoinFromLogs; the join questions it starts answer at once here, as the C#'s completed tasks did.
    async fn tick(&self) {
        self.app.rejoin_from_logs();
        rt::sleep(Duration::from_millis(150)).await;
    }

    fn closing(&self) -> bool {
        self.app.close_desk.closing_of(self.me).is_some()
    }

    fn clear_closing(&self) {
        self.app.close_desk.clear();
    }

    fn queue_len(&self) -> usize {
        self.app.controller.rejoin_queue.borrow().len()
    }

    fn clear_queue(&self) {
        self.app.controller.rejoin_queue.borrow_mut().clear();
    }

    fn queued_target(&self) -> Option<GameTarget> {
        let q = self.app.controller.rejoin_queue.borrow();
        if q.len() == 1 { Some(q[0].target.clone()) } else { None }
    }

    /// A new window of Bloxkeep's (every launch opens one, with a log of its own): this test's one log starts over.
    fn new_window(&self) {
        {
            let mut logs = self.app.controller.logs.borrow_mut();
            let tail = logs.tails.get_mut(&self.me).expect("the log is followed");
            tail.state = None;
            tail.teleport_at = None;
            tail.own_start = false;
            tail.leaving = false;
            tail.dropped_by_reason = false;
            tail.connections = 0;
        }
        // A new process too: nothing Bloxkeep held about the old one's id.
        self.app.rejoin.game_ban_noted.borrow_mut().remove(&self.me);
        self.app.rejoin.join_checks.borrow_mut().remove(&self.me);
    }

    fn alert_title(&self, kind: &str, en: &str, args: Vec<String>, said: &str) -> crate::core::app::discord_alerts::DiscordAlert {
        self.app.alert_for(kind, self.id, Some(&crate::core::l::Made::new(en, args)), said, now())
    }

    fn card_banned(&self) -> Option<bool> {
        self.app.account_dto(&self.acc())["session"]["bannedByGame"].as_bool()
    }

    fn noted(&self) -> bool {
        self.app.rejoin.game_ban_noted.borrow().contains(&self.me)
    }

    fn set_check(&self, answer: Option<Option<JoinAnswer>>, at: Stamp) {
        self.app.rejoin.join_checks.borrow_mut().insert(self.me, check_with(answer, at));
    }

    fn should_rejoin(&self, launch: &Rc<PendingLaunch>, state: Option<&ClientState>) -> bool {
        self.app.should_rejoin(self.id, launch, state, &End::Unasked).is_some()
    }

    /// The account's own windows set aside (no window counts as open) while `f` runs, then back.
    fn take_running(&self) -> Option<(i32, Rc<PendingLaunch>)> {
        self.app.controller.running.borrow_mut().shift_remove(&self.id)
    }

    fn put_running(&self, held: Option<(i32, Rc<PendingLaunch>)>) {
        if let Some(h) = held {
            self.app.controller.running.borrow_mut().insert(self.id, h);
        }
    }
}

/// The launch of `account` into `target` (Play when `preset` is None).
async fn open(app: &Rc<App>, account: Account, target: i64, preset: Option<Guid>) -> crate::core::base::Res<crate::core::app::presets::LaunchResult> {
    let vault = app.controller.vault.borrow().clone().expect("unlocked");
    app.launch_accounts(vault, vec![account], game(target), false, 0, None, preset, None, None).await
}

pub async fn run(c: &mut Checks, dir: &str) {
    no_network();
    let logs = std::path::Path::new(dir).join("roblox-logs");
    std::fs::create_dir_all(&logs).expect("logs folder");
    // SAFETY: this suite's own process, before any other thread reads the environment.
    unsafe { std::env::set_var("BLOXKEEP_ROBLOX_LOGS", &logs) };
    let me = std::process::id() as i32;

    let account = Account { username: "Stuck".into(), user_id: 21, ..Default::default() };
    let id = account.id;
    seed(vec![account]);
    let (app, said) = quiet_app();
    unlock(&app).await;
    // The places asked about are their games' start places: a bare 211 there is a ban (check_join).
    for place in [PLACE, GPO] {
        start_place_of_its_own(&app, place);
    }
    // A launch never touches the real Roblox: no Multi-Roblox mutex taken, no notification-area copy of Roblox closed.
    let _ = app.vault_mut(|d| d.settings.multi_instance = false);
    let log = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .read(true)
        .write(true)
        .open(logs.join(format!("0.743.0.7431134_{}Z_Player_REJOIN_last.log", log_stamp(now()))))
        .expect("a log");
    let mut r = R { app: app.clone(), said, me, id, log };

    body(c, &mut r).await;

    app.controller.running.borrow_mut().clear();
    r.clear_queue();
    r.clear_closing();
}

async fn body(c: &mut Checks, r: &mut R) {
    let app = r.app.clone();
    let me = r.me;
    let id = r.id;

    // Sent by a preset's server choice to a server that, it turns out, has ended.
    let launch = r.run(Some(GameTarget::new(PLACE, Some(DEAD_SERVER.into()), None, None)));
    r.edit(|a| a.auto_rejoin = true);
    r.line(now().plus_secs(-30.0), LAUNCH);
    let state = r.poll(|s| s.status == "joining").await;
    c.check("a game start reads as joining, without a server yet", state.as_ref().is_some_and(|s| s.status == "joining" && s.job_id.is_none()), &state);
    r.tick().await;
    c.check("half a minute without a server: still waited for", !r.closing(), ());

    // A server found, the game still loading after a minute: not this.
    r.line(now().plus_secs(-70.0), JOIN);
    r.poll(|s| s.job_id.is_some()).await;
    r.tick().await;
    c.check("a server found, still loading after a minute: left alone", !r.closing(), ());

    // The same launch again in a new window, a minute and more without a server.
    r.new_window();
    r.line(now().plus_secs(-70.0), LAUNCH);
    r.poll(|s| s.job_id.is_none()).await;
    r.edit(|a| a.auto_rejoin = false);
    r.tick().await;
    c.check("auto-rejoin off: the window stays as it is", !r.closing(), ());
    r.edit(|a| a.auto_rejoin = true);
    r.tick().await;
    c.check("a minute without a server: closed to rejoin", r.closing(), ());
    let said = r.told();
    c.check(
        "…saying the server couldn't take it, and that another one is joined",
        said.as_deref().is_some_and(|s| s.contains("couldn't get into that server (full or closed) — joining another one")),
        &said,
    );
    let alert = r.alert_title("drop", "{0} couldn't get into that server (full or closed) — joining another one…", vec![r.label()], said.as_deref().unwrap_or(""));
    c.check("Discord's alert says the same", alert.title == "Couldn't get into the server, joining another", &alert.title);

    // Closing it makes Roblox leave the game first (seen on 0.743), and the client ends.
    r.line(now(), LEAVE);
    let ended = app.ended_state(me);
    c.check("the leave Bloxkeep's close made isn't the person's", ended.as_ref().is_some_and(|s| s.status == "disconnected"), &ended);
    app.on_session_ended(id, &launch, ended, End::Rejoin);
    let target = r.queued_target();
    c.check("…so it rejoins: the same game, any server", target.as_ref().is_some_and(|t| t.place_id == PLACE && t.job_id.is_none()), &target);
    r.clear_queue();

    r.clear_closing();
    let own = app.ended_state(me);
    c.check("a leave nobody else made is still the person's", own.as_ref().is_some_and(|s| s.status == "left"), &own);
    c.check("…and isn't rejoined", !app.queue_rejoin(id, &launch, own.as_ref(), End::Unasked) && r.queue_len() == 0, ());

    // In the game, back to Roblox's home and the window closed with X before Roblox logged the disconnect: a leave all
    // the same, not a drop.
    r.new_window();
    r.line(now(), LAUNCH);
    r.line(now(), JOIN);
    r.line(now(), "[FLog::Network] serverId: 128.116.53.33|51451");
    r.poll(|s| s.status == "ingame").await;
    r.line(now(), LEAVE);
    let closed_with_x = app.ended_state(me);
    c.check(
        "in a game, a leave and the window closed before its disconnect line: left (fails on the old code)",
        closed_with_x.as_ref().is_some_and(|s| s.status == "left"),
        &closed_with_x,
    );
    c.check("…and isn't rejoined", !app.queue_rejoin(id, &launch, closed_with_x.as_ref(), End::Unasked) && r.queue_len() == 0, ());

    // Sent to the game (Roblox picks the server), and none came.
    r.run(game(PLACE));
    r.new_window();
    r.line(now().plus_secs(-65.0), LAUNCH);
    r.poll(|s| s.status == "joining").await;
    r.told();
    r.tick().await;
    let said = r.told();
    c.check(
        "a game start without a server, no server asked for: tried again",
        r.closing() && said.as_deref().is_some_and(|s| s.contains("couldn't get into the game — trying again")),
        &said,
    );
    r.clear_closing();

    // The server list: an empty server (its last player gone, so it is closing) is never offered.
    let page = json!({"data":[
      {"id":"one","maxPlayers":7,"playing":1,"fps":60,"ping":54},
      {"id":"emptied","maxPlayers":7,"fps":60,"ping":686},
      {"id":"zero","maxPlayers":7,"playing":0,"fps":60},
      {"id":"","maxPlayers":7,"playing":2},
      {"maxPlayers":7,"playing":3},
      {"id":"two","maxPlayers":7,"playing":2,"fps":59.9}
    ]});
    let read = roblox_api::read_servers(&page).unwrap_or_default();
    let ids: Vec<String> = read.iter().map(|s| s.job_id.clone()).collect();
    c.check("empty servers (no player count, or 0) left out of the list", ids == ["one", "two"], ids.join(","));
    let server = |id: &str, playing: i32| ServerInfo { job_id: id.into(), playing, max_players: 7, ping: 50, fps: 60.0 };
    let list = vec![server("a", 1), server("b", 2), server("c", 3), server("d", 6)];
    let roomy = |party: i32, closed: &[&str]| {
        let closed: Vec<String> = closed.iter().map(|s| s.to_string()).collect();
        crate::core::app::launching::roomy_server(&list, party, &closed).map(|s| s.job_id.clone())
    };
    c.check("the next server for a party of 5: room for all, the busiest of those", roomy(5, &[]).as_deref() == Some("b"), roomy(5, &[]));
    c.check("…never one found closed", roomy(5, &["b"]).as_deref() == Some("a"), roomy(5, &["b"]));
    c.check("…and none when no server has room", roomy(7, &[]).is_none(), roomy(7, &[]));

    // A party of 5 sent to one server: what the first account's client says of it.
    r.run(Some(GameTarget::new(PLACE, Some(DEAD_SERVER.into()), None, None)));
    r.new_window();
    r.line(now().plus_secs(-3.0), LAUNCH);
    r.poll(|s| s.status == "joining" && now().secs_since(s.at) < 10.0).await;
    c.check("3 s after the game start, no server yet: not known", app.server_answer(id).is_none(), app.server_answer(id));
    r.line(now(), JOIN);
    r.poll(|s| s.job_id.is_some()).await;
    c.check("a server named in its log: the server took it", app.server_answer(id) == Some("in"), app.server_answer(id));
    r.new_window(); // the next account's window
    r.line(now().plus_secs(-13.0), LAUNCH);
    r.poll(|s| s.job_id.is_none()).await;
    c.check("13 s without a server (a normal join gets one in about a second): silent", app.server_answer(id) == Some("silent"), app.server_answer(id));
    // Roblox's join answer for that server decides, not the silence: a join waiting for room logs nothing. Roblox answers
    // from here.
    let party_ask = app.rejoin.ask_join.borrow().clone();
    let party_reply = Arc::new(Mutex::new(answer(200, Some(5), Some("The game has ended."))));
    let party_asks = Arc::new(AtomicI32::new(0));
    {
        let (reply, asks) = (party_reply.clone(), party_asks.clone());
        *app.rejoin.ask_join.borrow_mut() = Some(Arc::new(move |_, _, _| {
            asks.fetch_add(1, Ordering::SeqCst);
            Ok(reply.lock().unwrap().clone())
        }));
    }
    let dead = GameTarget::new(PLACE, Some(DEAD_SERVER.into()), None, None);
    let account = r.acc();
    let waited = Instant::now();
    let told_now = app.wait_for_server_answer(&account, &dead, &Cancel::new()).await;
    c.check(
        "…Roblox says the server has ended (status 5): closed, heard without waiting",
        matches!(told_now, Ok(Some("closed"))) && party_asks.load(Ordering::SeqCst) == 1 && waited.elapsed() < Duration::from_millis(2000),
        format!("{told_now:?} {} {} ms", party_asks.load(Ordering::SeqCst), waited.elapsed().as_millis()),
    );
    app.rejoin.join_checks.borrow_mut().remove(&me);
    *party_reply.lock().unwrap() = answer(200, Some(6), Some("The game you requested is currently full."));
    let full = app.wait_for_server_answer(&account, &dead, &Cancel::new()).await;
    c.check("…full (status 6): closed", matches!(full, Ok(Some("closed"))), &full);
    // The server ended with a system error (2026-10-29): status 12, "Unable to join Game 311", not a ban: closed.
    app.rejoin.join_checks.borrow_mut().remove(&me);
    *party_reply.lock().unwrap() = answer(200, Some(12), Some("Unable to join Game 311"));
    let gone = app.wait_for_server_answer(&account, &dead, &Cancel::new()).await;
    c.check("…refused for that server without a ban (status 12, 311): closed, the party goes elsewhere", matches!(gone, Ok(Some("closed"))), &gone);
    let asked311 = app.check_join(&account, &dead, "test", &Cancel::new()).await;
    let game311 = app.check_join(&account, &GameTarget::new(PLACE, None, None, None), "test", &Cancel::new()).await;
    c.check(
        "…read as that server gone (ended), not a refusal; the same answer for the game itself (no server named) is a refusal",
        asked311.as_ref().is_some_and(|a| a.server_gone && a.ended() && !a.refused()) && game311.as_ref().is_some_and(|a| !a.server_gone && a.refused()),
        (&asked311, &game311),
    );
    *party_reply.lock().unwrap() = answer(200, Some(12), Some("Unable to join Game 211"));
    let ban211 = app.check_join(&account, &dead, "test", &Cancel::new()).await;
    c.check(
        "…a ban by the game (211) on a named server stays a ban and a refusal",
        ban211.as_ref().is_some_and(|a| a.banned() && !a.server_gone && a.refused()),
        &ban211,
    );
    // A bare 211 at another place of the game (Blox Fruits' Third Sea, a full server) is asked again at the game's start
    // place: a ban holds for the whole game.
    {
        const SEA: i64 = 100117331123089;
        const START: i64 = 2753915549;
        app.controller.games.borrow_mut().insert(SEA, Some(GameInfo { place_id: START, universe_id: 994732206, ..Default::default() }));
        let start_says = Arc::new(Mutex::new(answer(200, Some(2), None)));
        let asked_at = Arc::new(Mutex::new(Vec::<i64>::new()));
        let party_hook = app.rejoin.ask_join.borrow().clone();
        {
            let (says, at) = (start_says.clone(), asked_at.clone());
            *app.rejoin.ask_join.borrow_mut() = Some(Arc::new(move |_, t, _| {
                at.lock().unwrap().push(t.place_id);
                Ok(if t.place_id == START { says.lock().unwrap().clone() } else { answer(200, Some(12), Some("Unable to join Game 211")) })
            }));
        }
        let sea = GameTarget::new(SEA, None, None, None);
        let let_in = app.check_join(&account, &sea, "test", &Cancel::new()).await;
        c.check(
            "a bare 211 at another place of the game, the start place letting the account in: not a ban, not a refusal",
            let_in.as_ref().is_some_and(|a| !a.banned() && !a.refused() && a.start_status == Some(2)) && *asked_at.lock().unwrap() == [SEA, START],
            (&let_in, asked_at.lock().unwrap().clone()),
        );
        *start_says.lock().unwrap() = answer(200, Some(6), Some("The game you requested is currently full."));
        let full_there = app.check_join(&account, &sea, "test", &Cancel::new()).await;
        c.check("…the start place full: not a ban either", full_there.as_ref().is_some_and(|a| !a.banned() && !a.refused()), &full_there);
        let sea_server = GameTarget::new(SEA, Some(DEAD_SERVER.into()), None, None);
        let server = app.check_join(&account, &sea_server, "test", &Cancel::new()).await;
        c.check("…for a named server: that server is gone, another is tried", server.as_ref().is_some_and(|a| !a.banned() && a.server_gone && a.ended()), &server);
        *start_says.lock().unwrap() = answer(200, Some(12), Some("Unable to join Game 211"));
        let both = app.check_join(&account, &sea, "test", &Cancel::new()).await;
        c.check("…the start place says 211 too: a ban", both.as_ref().is_some_and(|a| a.banned() && a.refused()), &both);
        *start_says.lock().unwrap() = answer(429, None, Some("Too many requests"));
        let unknown = app.check_join(&account, &sea, "test", &Cancel::new()).await;
        c.check("…the start place gives no answer: no answer, asked again later", unknown.is_none(), &unknown);
        let with_words = JoinAnswer { data: Some(r#"{"moderationMessage":"Exploiting"}"#.into()), ..answer(200, Some(12), Some("Unable to join Game 211")) };
        c.check("…a 211 with the game's words is a ban without asking", !with_words.bare_211() && with_words.banned(), ());
        // The game's start place not known (Roblox can't be asked): no answer, so nothing is said yet.
        let unknown_place = app.check_join(&account, &GameTarget::new(987654321, None, None, None), "test", &Cancel::new()).await;
        c.check("…a bare 211 for a place whose game isn't known: no answer, asked again later", unknown_place.is_none(), &unknown_place);
        // A ban noted wrongly before, at that place: forgotten once the start place lets the account in.
        *start_says.lock().unwrap() = answer(200, Some(2), None);
        let cookie_was = r.acc().cookie.clone();
        r.edit(|a| {
            a.cookie = "test".into(); // asked with its own sign-in
            a.game_bans = vec![GameBan { place_id: SEA, game_name: None, at: now().plus_secs(-3600.0), message: None, until: None }];
        });
        let _ = app.recheck_game_bans(&[r.acc()], &sea, &Cancel::new()).await;
        c.check("…and a ban noted before from such a 211 is forgotten before opening", r.acc().game_bans.is_empty(), r.acc().game_bans.len());
        r.edit(|a| {
            a.game_bans.clear();
            a.cookie = cookie_was;
        });
        app.controller.games.borrow_mut().remove(&SEA);
        *app.rejoin.ask_join.borrow_mut() = party_hook;
    }
    // One account opened into a chosen server that had ended (Roblox's "This experience has ended, or the server became
    // unavailable unexpectedly due to a system error", answered 311 at 46 s and closed only at 60 s). Asked 12 s on, and
    // another server at once on that answer.
    app.rejoin.join_checks.borrow_mut().remove(&me);
    *party_reply.lock().unwrap() = answer(200, Some(12), Some("Unable to join Game 311"));
    party_asks.store(0, Ordering::SeqCst);
    let cookie_before = r.acc().cookie.clone();
    r.edit(|a| a.cookie = "test".into()); // asked with its own sign-in
    r.new_window();
    r.run(Some(dead.clone()));
    r.line(now().plus_secs(-8.0), LAUNCH);
    r.poll(|s| s.status == "joining" && s.job_id.is_none() && now().secs_since(s.at) < 9.0).await;
    r.clear_closing();
    r.tick().await;
    let asked_early = party_asks.load(Ordering::SeqCst) > 0;
    r.new_window();
    r.line(now().plus_secs(-14.0), LAUNCH);
    r.poll(|s| s.status == "joining" && s.job_id.is_none() && now().secs_since(s.at) > 13.0).await;
    r.told();
    r.tick().await; // asks
    r.tick().await; // reads the answer
    let gone_said = r.told();
    c.check(
        "a chosen server that ended: not asked about before 12 s, then closed for another server on Roblox's answer, not a minute on",
        !asked_early
            && party_asks.load(Ordering::SeqCst) == 1
            && r.closing()
            && gone_said.as_deref() == Some(format!("{} couldn't get into that server (full or closed) — joining another one…", r.label()).as_str()),
        format!("{asked_early} {} {} {gone_said:?}", party_asks.load(Ordering::SeqCst), r.closing()),
    );
    r.clear_closing();
    r.edit(|a| a.cookie = cookie_before.clone());
    // Roblox would let it in (status 2): the join is slow, not refused. It got its server 2 s on.
    app.rejoin.join_checks.borrow_mut().remove(&me);
    *party_reply.lock().unwrap() = answer(200, Some(2), None);
    let slow: Rc<RefCell<Option<crate::core::base::Res<Option<&'static str>>>>> = Rc::new(RefCell::new(None));
    {
        let (app, account, dead, slow) = (app.clone(), account.clone(), dead.clone(), slow.clone());
        rt::spawn(async move {
            let got = app.wait_for_server_answer(&account, &dead, &Cancel::new()).await;
            *slow.borrow_mut() = Some(got);
        });
    }
    rt::sleep(Duration::from_millis(1500)).await;
    r.line(now(), JOIN);
    r.poll(|s| s.job_id.is_some()).await; // as the process check reads the log
    let begun = Instant::now();
    while slow.borrow().is_none() && begun.elapsed() < Duration::from_secs(30) {
        rt::sleep(Duration::from_millis(50)).await;
    }
    let slow_said = slow.borrow().clone();
    c.check(
        "…Roblox would let it in (status 2), its server named 2 s on: not closed, it got in",
        matches!(slow_said, Some(Ok(Some("in")))),
        &slow_said,
    );
    c.check("…and the window's own join check has that answer, not asked again", app.rejoin.join_checks.borrow().contains_key(&me), ());
    *app.rejoin.ask_join.borrow_mut() = party_ask;
    app.rejoin.join_checks.borrow_mut().remove(&me);

    // A move to another server of the same place, as client 0.743 logged it on 2026-10-19: "Teleported." and the new
    // join first, the old server's disconnect line after them.
    r.run(game(PLACE));
    let mut at = now().plus_secs(-10.0);
    const MOVED: &str = "99999999-2222-3333-4444-555555555555";
    r.line(at, JOIN);
    r.line(at.plus_secs(0.1), "[FLog::Network] serverId: 10.0.0.1|5000");
    r.line(at.plus_secs(2.0), "[FLog::UgcExperienceController] UgcExperienceController: doTeleport: url ");
    r.line(at.plus_secs(2.2), "[FLog::SessionTransitionFSM] Teleported.");
    r.line(at.plus_secs(2.2), &format!("[FLog::Output] ! Joining game '{MOVED}' place 107778070777162 at 10.0.0.2"));
    r.line(at.plus_secs(2.4), "[FLog::Network] Time to disconnect replication data: 0.125100");
    let state = r.poll(|s| s.job_id.as_deref() == Some(MOVED)).await; // the lines written together are read together
    c.check(
        "a teleport whose old server logs its disconnect after the new join: still joining, not a drop",
        state.as_ref().is_some_and(|s| s.status == "joining" && s.job_id.as_deref() == Some(MOVED)),
        &state,
    );
    r.tick().await;
    c.check("…and the window is left alone", !r.closing(), ());
    r.line(at.plus_secs(3.5), "[FLog::Network] serverId: 10.0.0.2|5000");
    let state = r.poll(|s| s.status == "ingame").await;
    c.check("…in the game once the new server answers", state.as_ref().is_some_and(|s| s.status == "ingame" && s.job_id.as_deref() == Some(MOVED)), &state);
    // The usual order (the disconnect before the new join) is still a teleport too.
    at = at.plus_secs(4.0);
    r.line(at, "[FLog::UgcExperienceController] UgcExperienceController: doTeleport: url ");
    r.line(at.plus_secs(0.2), "[FLog::Network] Time to disconnect replication data: 0.1");
    let state = r.poll(|s| s.at >= at.plus_secs(0.1)).await;
    c.check("a teleport that disconnects first: joining", state.as_ref().is_some_and(|s| s.status == "joining"), &state);
    r.line(at.plus_secs(0.3), JOIN);
    r.line(at.plus_secs(0.5), "[FLog::Network] serverId: 10.0.0.1|5000");
    r.poll(|s| s.status == "ingame").await;

    // The person leaves the game Bloxkeep opened and starts another one in the same window: a drop there rejoins that
    // game, not the one Bloxkeep opened.
    const OTHER: i64 = 126256942507957;
    let opened = r.run(game(PLACE));
    at = at.plus_secs(1.0);
    r.line(at, LEAVE);
    r.line(at.plus_secs(0.1), "[FLog::Network] Time to disconnect replication data: 0.2");
    r.line(at.plus_secs(1.0), LAUNCH);
    r.line(at.plus_secs(1.5), &format!("[FLog::Output] ! Joining game '77777777-2222-3333-4444-555555555555' place {OTHER} at 10.0.0.3"));
    r.line(at.plus_secs(1.6), "[FLog::Network] serverId: 10.0.0.3|5000");
    r.line(at.plus_secs(2.0), "[FLog::UgcExperienceController] UgcExperienceController: doTeleport: url ");
    r.line(at.plus_secs(2.1), "[FLog::Output] ! Joining game '66666666-2222-3333-4444-555555555555' place 555 at 10.0.0.4");
    r.line(at.plus_secs(2.2), "[FLog::Network] Time to disconnect replication data: 0.2");
    r.line(at.plus_secs(2.3), "[FLog::Network] serverId: 10.0.0.4|5000");
    r.line(at.plus_secs(3.0), "[FLog::Network] Time to disconnect replication data: 0.2");
    let state = r.poll(|s| s.status == "disconnected").await;
    c.check(
        "a game the person starts in the window is noted, and a teleport inside it keeps it",
        state.as_ref().is_some_and(|s| s.status == "disconnected" && s.place_id == Some(555) && s.chosen_place == Some(OTHER)),
        &state,
    );
    app.on_session_ended(id, &opened, state, End::Unasked);
    let target = r.queued_target();
    c.check(
        "…and a drop there rejoins that game (any server), not the one Bloxkeep opened",
        target.as_ref().is_some_and(|t| t.place_id == OTHER && t.job_id.is_none()),
        &target,
    );
    r.clear_queue();
    // Kicked (a game's kick, Leave, then a game start in the same window): a game started after that is the person's choice too.
    r.line(at.plus_secs(4.0), LAUNCH);
    r.line(at.plus_secs(4.5), "[FLog::Output] ! Joining game '55555555-2222-3333-4444-555555555555' place 444 at 10.0.0.5");
    r.line(at.plus_secs(4.6), "[FLog::Network] serverId: 10.0.0.5|5000");
    let state = r.poll(|s| s.status == "ingame" && s.place_id == Some(444)).await;
    c.check(
        "a game started in the window after a kick is the person's choice",
        state.as_ref().is_some_and(|s| s.status == "ingame" && s.chosen_place == Some(444)),
        &state,
    );
    // A preset's "restart every N hours" of that window goes back into it too (and its server), not the first game.
    let running_now = app.controller.running.borrow().get(&id).cloned().expect("open");
    let restart_to = app.restart_target(running_now.0, &running_now.1);
    c.check(
        "…and a preset's restart goes back into it, same server",
        restart_to.place_id == 444 && restart_to.job_id.as_deref() == Some("55555555-2222-3333-4444-555555555555"),
        &restart_to,
    );

    // A game the person starts in the window that gets no server in a minute is theirs to watch.
    r.line(now().plus_secs(-65.0), LAUNCH);
    let state = r.poll(|s| s.status == "joining" && s.job_id.is_none()).await;
    r.tick().await;
    c.check(
        "a game the person started that gets no server in a minute is left alone",
        state.as_ref().is_some_and(|s| s.person_started) && !r.closing(),
        &state,
    );
    // …also after two minutes, when a join of Bloxkeep's would go to Roblox's status check (which closes it).
    r.line(now().plus_secs(-180.0), LAUNCH);
    r.poll(|s| s.status == "joining" && s.job_id.is_none() && now().secs_since(s.at) > 150.0).await;
    c.check("…and after two minutes its log still decides, not Roblox's status", app.log_followed(me), ());

    // A game started in the window while still on a server (no leave logged first): the old server's disconnect line
    // after the new join isn't a drop.
    at = now().plus_secs(-10.0);
    r.line(at, "[FLog::Output] ! Joining game '44444444-2222-3333-4444-555555555555' place 333 at 10.0.0.6");
    r.line(at.plus_secs(0.2), "[FLog::Network] Time to disconnect replication data: 0.2");
    let state = r.poll(|s| s.place_id == Some(333)).await;
    r.tick().await;
    c.check(
        "a new game while still on a server: the old server's disconnect line isn't a drop",
        state.as_ref().is_some_and(|s| s.status == "joining") && !r.closing(),
        &state,
    );
    r.line(at.plus_secs(1.0), "[FLog::Network] serverId: 10.0.0.6|5000");
    let state = r.poll(|s| s.status == "ingame").await;
    c.check("…in the game once its server answers", state.as_ref().is_some_and(|s| s.status == "ingame" && s.place_id == Some(333)), &state);

    // A teleport that fails, then Leave: the disconnect line that follows is the leave's.
    at = at.plus_secs(2.0);
    r.line(at, "[FLog::UgcExperienceController] UgcExperienceController: doTeleport: url ");
    r.line(at.plus_secs(5.0), LEAVE);
    r.line(at.plus_secs(5.1), "[FLog::Network] Time to disconnect replication data: 0.2");
    let state = r.poll(|s| s.status != "ingame").await;
    c.check("a teleport that failed, then Leave: left, not joining", state.as_ref().is_some_and(|s| s.status == "left"), &state);

    // Earlier drops stop counting once a window has stayed in its game ten minutes since it opened.
    r.run_at(game(PLACE), now().plus_secs(-11.0 * 60.0));
    r.line(now(), LAUNCH);
    r.line(now(), JOIN);
    r.line(now(), "[FLog::Network] serverId: 10.0.0.1|5000");
    let state = r.poll(|s| s.status == "ingame" && now().secs_since(s.at) < 30.0).await;
    app.controller.rejoins_in_a_row.borrow_mut().insert(id, 2);
    app.forget_settled_rejoins(now());
    c.check(
        "in its game 11 min after it opened, its last server joined just now: settled",
        !app.controller.rejoins_in_a_row.borrow().contains_key(&id),
        &state,
    );
    // Not a window that is still getting in, however long ago it opened.
    r.line(now(), LAUNCH);
    r.poll(|s| s.status == "joining" && s.job_id.is_none()).await;
    app.controller.rejoins_in_a_row.borrow_mut().insert(id, 2);
    app.forget_settled_rejoins(now());
    c.check("…but not one still getting into its game 11 min after it opened", app.controller.rejoins_in_a_row.borrow().contains_key(&id), ());
    app.controller.rejoins_in_a_row.borrow_mut().remove(&id);

    // The game's own kick, with its words, then a ban by the game (Roblox's ban API): both with code 267. Grand Piece
    // Online's place.
    fn played(r: &mut R, at: Stamp, kick_line: &str) {
        r.line(at, LAUNCH);
        r.line(at, &format!("[FLog::Output] ! Joining game 'e403336e-5756-44c9-b029-0ae6873a3d03' place {GPO} at 10.32.36.210"));
        r.line(at, "[FLog::Network] serverId: 128.116.46.33|56123");
        r.line(at.plus_secs(1.0), kick_line);
        r.line(at.plus_secs(2.0), "[FLog::Network] Disconnect reason received: 267");
        r.line(at.plus_secs(2.0), "[FLog::Network] Disconnection Notification. Reason: 267");
        r.line(at.plus_secs(2.1), "[FLog::Network] Time to disconnect replication data: 0.2");
    }
    r.new_window();
    r.run(game(GPO));
    r.clear_closing();
    r.told();
    let kicked_at = now();
    played(r, kicked_at, "Error [FLog::CreatorError] Error: Server Kick Message: A fatal error occured. This may be from an internet issue.; Metadata: ");
    let state = r.poll(|s| s.status == "disconnected").await;
    c.check(
        "a kick by the game: code 267, with the game's own words, not a ban",
        state.as_ref().is_some_and(|s| {
            s.reason == Some(267) && s.kick_message.as_deref() == Some("A fatal error occured. This may be from an internet issue.") && !s.banned_by_game
        }),
        &state,
    );
    r.new_window();
    let banned_launch = r.run(game(GPO));
    let banned_at = now();
    played(r, banned_at, BAN_LINE);
    let state = r.poll(|s| s.status == "disconnected" && s.at > banned_at).await;
    c.check(
        "a ban by the game: the same code 267, told apart by the metadata its kick line carries",
        state.as_ref().is_some_and(|s| s.reason == Some(267) && s.banned_by_game && s.kick_message.is_none()),
        &state,
    );
    r.tick().await;
    let ban_said = r.told();
    c.check(
        "…no rejoin (its window stays, with Roblox's message), the ban noted with the account, and said",
        !r.closing() && r.acc().game_bans.len() == 1 && r.acc().game_bans[0].place_id == GPO && ban_said.as_deref().is_some_and(|s| s.contains("banned from")),
        &ban_said,
    );
    c.check("…nor a rejoin once its window is closed", !r.should_rejoin(&banned_launch, state.as_ref()), ());
    // Discord's alert: the game's words (if any) as its text, without a "Reason" saying "Kicked by the game" again.
    let ban_alert = r.alert_title(
        "gameBan",
        "{0} was banned from {1} by the game: {2}",
        vec![r.label(), "Grand Piece Online".into(), "Exploiting".into()],
        ban_said.as_deref().unwrap_or(""),
    );
    let ban_fields: Vec<String> = ban_alert.fields.iter().map(|f| f.name.clone()).collect();
    c.check(
        "…Discord's alert says the ban, without a \"Reason\" (kicked) beside it",
        ban_alert.title == "Banned by the game" && !ban_fields.iter().any(|f| f == "Reason") && !ban_alert.text.as_deref().is_some_and(|t| t.contains("Reason:")),
        format!("{} | {:?}", ban_fields.join(","), ban_alert.text),
    );
    // Roblox 0.744 often logs why in words only, without the code.
    let not_banned = GameTarget::new(PLACE, None, None, None);
    let dropped_launch: Rc<RefCell<Option<Rc<PendingLaunch>>>> = Rc::new(RefCell::new(None));
    fn dropped_at(r: &mut R, not_banned: &GameTarget, slot: &Rc<RefCell<Option<Rc<PendingLaunch>>>>, at: Stamp, why: &[&str]) -> Stamp {
        r.new_window();
        *slot.borrow_mut() = Some(r.run(Some(not_banned.clone())));
        r.line(at, LAUNCH);
        r.line(at, JOIN);
        r.line(at, "[FLog::Network] serverId: 128.116.46.33|52353");
        for line in why {
            r.line(at.plus_secs(1.0), line);
        }
        r.line(at.plus_secs(2.0), "[FLog::Network] Time to disconnect replication data: 7.910900");
        at
    }
    let dropped = |r: &mut R, why: &[&str]| dropped_at(r, &not_banned, &dropped_launch, now().plus_secs(-10.0), why);
    let dl = |slot: &Rc<RefCell<Option<Rc<PendingLaunch>>>>| slot.borrow().clone().expect("a launch");
    let words_at = dropped(
        r,
        &[
            "Error [FLog::CreatorError] Error: Server Kick Message: DataStore lock lost, please rejoin the game.; Metadata: ",
            "[FLog::Network] Disconnect from 1",
            "[FLog::Network] Client has been disconnected with reason: You have been kicked from the game",
            "Warning [FLog::Network] Lost connection with reason : You have been kicked from the game",
        ],
    );
    let state = r.poll(|s| s.status == "disconnected" && s.at > words_at).await;
    c.check(
        "a game's kick logged in words only, no code: 267, with the game's words",
        state.as_ref().is_some_and(|s| s.reason == Some(267) && s.kick_message.as_deref() == Some("DataStore lock lost, please rejoin the game.") && !s.banned_by_game),
        &state,
    );
    r.clear_closing();
    r.told();
    r.tick().await;
    let kick_said = r.told();
    c.check(
        "…rejoined, said as the game's kick with its words, not as a lost connection",
        r.closing() && kick_said.as_deref() == Some(format!("{} was kicked by the game, rejoining: DataStore lock lost, please rejoin the game.", r.label()).as_str()),
        &kick_said,
    );
    let kick_alert = r.alert_title(
        "drop",
        "{0} was kicked by the game, rejoining: {1}",
        vec![r.label(), "DataStore lock lost, please rejoin the game.".into()],
        kick_said.as_deref().unwrap_or(""),
    );
    let kick_fields: Vec<String> = kick_alert.fields.iter().map(|f| f.name.clone()).collect();
    c.check(
        "…Discord's alert: \"Kicked by the game, rejoining\", the game's words as its text, no \"Reason\" field",
        kick_alert.title == "Kicked by the game, rejoining"
            && kick_alert.text.as_deref().is_some_and(|t| t.starts_with("DataStore lock lost"))
            && !kick_fields.iter().any(|f| f == "Reason"),
        format!("{} | {:?} | {}", kick_alert.title, kick_alert.text, kick_fields.join(",")),
    );
    // Roblox's Error Code 256 (its plain "disconnected", the number its box shows when it names no other; people of
    // another manager report it isn't rejoined there, 2026-11-15): logged with its number in any of the three shapes
    // Roblox writes a reason in, it is a drop like any other and the window is closed to rejoin. Logged with nothing at
    // all, the check further down catches it (Roblox's own status says out of a game, three checks running).
    for line in [
        "[FLog::Network] Disconnection Notification. Reason: 256",
        "[FLog::Network] Disconnect reason received: 256",
        "[DFLog::NetworkClient] setting replicator disconnect reason to 256",
    ] {
        let at_256 = dropped(r, &[line]);
        let state = r.poll(|s| s.status == "disconnected" && s.at > at_256).await;
        r.clear_closing();
        r.told();
        r.tick().await;
        c.check(
            &format!("Error 256 in the log ({}): a drop with its code, closed to rejoin", line.split("] ").nth(1).unwrap_or(line)),
            state.as_ref().is_some_and(|s| s.reason == Some(256)) && r.closing(),
            &state,
        );
    }
    r.clear_closing();
    r.told();
    // A wait set before rejoining (Settings › Staying in game): a drop younger than it is left open, in case the game
    // gets into its next server by itself; with no wait it is closed to rejoin.
    let _ = r.app.vault_mut(|d| d.settings.rejoin_wait_seconds = 60);
    let wait_at = dropped(r, &["[FLog::Network] Client has been disconnected with reason: You have been kicked from the game"]);
    let _ = r.poll(|s| s.status == "disconnected" && s.at > wait_at).await;
    r.clear_closing();
    r.tick().await;
    c.check("with a 60 s wait set, a window disconnected 10 s ago is left open", !r.closing(), r.state());
    let _ = r.app.vault_mut(|d| d.settings.rejoin_wait_seconds = 0);
    r.tick().await;
    c.check("…and closed to rejoin once no wait is set", r.closing(), r.state());
    r.clear_closing();
    r.told();
    let elsewhere_at = dropped(
        r,
        &[
            "[FLog::Network] Client has been disconnected with reason: Disconnected from game, possibly due to game joined from another device",
            "Warning [FLog::Network] Lost connection with reason : Disconnected from game, possibly due to game joined from another device",
        ],
    );
    let state = r.poll(|s| s.status == "disconnected" && s.at > elsewhere_at).await;
    r.clear_closing();
    r.told();
    r.tick().await;
    let elsewhere_told = r.told();
    c.check(
        "signed in on another device, in words only: 273, its window left as it is and no rejoin (it would take the other device's place)",
        state.as_ref().is_some_and(|s| s.reason == Some(273)) && !r.closing() && elsewhere_told.is_none() && !r.should_rejoin(&dl(&dropped_launch), state.as_ref()),
        &state,
    );
    // Words logged after the disconnect line complete it; a server that shut down is said as that.
    let late_at = dropped(r, &[]);
    let state = r.poll(|s| s.status == "disconnected" && s.at > late_at).await;
    let no_reason_first = state.as_ref().is_some_and(|s| s.reason.is_none());
    r.line(late_at.plus_secs(3.0), "Warning [FLog::Network] Lost connection with reason : The server has shut down");
    let state = r.poll(|s| s.status == "disconnected" && s.reason.is_some()).await;
    r.clear_closing();
    r.told();
    r.tick().await;
    let shut_said = r.told();
    c.check(
        "words logged after the disconnect line complete it: a server that shut down, said as that",
        no_reason_first
            && state.as_ref().is_some_and(|s| s.reason == Some(288))
            && r.closing()
            && shut_said.as_deref() == Some(format!("{}'s server shut down — joining another one…", r.label()).as_str()),
        format!("{state:?} | {shut_said:?}"),
    );
    // Words Roblox has no code for change nothing.
    c.check(
        "…and words not in Roblox's list read as no code",
        roblox_log_watcher::reason_of_words("[FLog::Network] Client has been disconnected with reason: Something else entirely").is_none(),
        (),
    );
    // A code logged as a number wins over one read from words, even logged after the disconnect line.
    let number_at = dropped(r, &["[FLog::Network] Client has been disconnected with reason: You have been kicked from the game"]);
    r.poll(|s| s.status == "disconnected" && s.at > number_at).await;
    r.line(number_at.plus_secs(3.0), "[FLog::Network] Disconnection Notification. Reason: 277");
    let state = r.poll(|s| s.status == "disconnected" && s.reason == Some(277)).await;
    c.check("a code logged as a number after the disconnect line wins over the one read from words", state.as_ref().is_some_and(|s| s.reason == Some(277)), &state);
    r.clear_closing();
    r.told();
    r.tick().await;
    let lost_said = r.told();
    c.check(
        "…and a failed connection's code is said as a lost connection",
        lost_said.as_deref() == Some(format!("{} lost connection — restarting Roblox…", r.label()).as_str()),
        &lost_said,
    );
    // Roblox's words printed by the game itself (its output, even quoting Roblox's tag) aren't Roblox's reason.
    let print_at = dropped(
        r,
        &[
            "[FLog::Output] Client has been disconnected with reason: You have been kicked from the game",
            "[FLog::Output] [FLog::Network] Lost connection with reason : Disconnected from game, possibly due to game joined from another device",
        ],
    );
    let state = r.poll(|s| s.status == "disconnected" && s.at > print_at).await;
    c.check("Roblox's words printed by the game aren't Roblox's reason", state.as_ref().is_some_and(|s| s.reason.is_none()), &state);
    r.clear_closing();
    r.told();
    r.tick().await;
    let unknown_said = r.told();
    c.check(
        "…a disconnect without a reason is said as one whose reason isn't known, not as a lost connection",
        r.closing() && unknown_said.as_deref() == Some(format!("{} dropped out of the game, reason unknown — restarting Roblox…", r.label()).as_str()),
        &unknown_said,
    );
    let unknown_alert = r.alert_title("drop", "{0} dropped out of the game, reason unknown — restarting Roblox…", vec![r.label()], unknown_said.as_deref().unwrap_or(""));
    c.check("…Discord's alert too", unknown_alert.title == "Disconnected, reason unknown", &unknown_alert.title);
    let code_at = dropped(r, &["[FLog::Network] Disconnection Notification. Reason: 299"]);
    r.poll(|s| s.status == "disconnected" && s.at > code_at).await;
    r.clear_closing();
    r.told();
    r.tick().await;
    let code_said = r.told();
    c.check(
        "…one with a code Bloxkeep has no words for is said with that code",
        code_said.as_deref() == Some(format!("{} was disconnected (Roblox code 299) — restarting Roblox…", r.label()).as_str()),
        &code_said,
    );
    // The client's own disconnect (285), which its notification line can repeat, never over a reason known; the
    // server's code as received is.
    let own_at = dropped(r, &["[FLog::Network] Client has been disconnected with reason: You have been kicked from the game"]);
    r.poll(|s| s.status == "disconnected" && s.at > own_at).await;
    r.line(own_at.plus_secs(3.0), "[FLog::Network] Disconnection Notification. Reason: 285");
    // One read takes in the line just written (a read applies on the poll after it; two polls a moment apart).
    let state = r.poll(|_| true).await;
    let state = if state.as_ref().is_some_and(|s| s.reason == Some(267)) {
        rt::sleep(Duration::from_millis(300)).await;
        r.poll(|_| true).await
    } else {
        state
    };
    c.check("the client's own disconnect (285) after a kick's words: still the kick", state.as_ref().is_some_and(|s| s.reason == Some(267)), &state);
    let received_at = dropped(
        r,
        &["[FLog::Network] Client has been disconnected with reason: You have been kicked from the game", "[FLog::Network] Disconnect reason received: 288"],
    );
    let state = r.poll(|s| s.status == "disconnected" && s.at > received_at).await;
    c.check("…while the server's code as received wins over the words", state.as_ref().is_some_and(|s| s.reason == Some(288)), &state);
    // Roblox 0.744's idle kick, in words with its minutes: 278, so anti-AFK's misses are told again.
    let idle_at = dropped(r, &["[FLog::Network] Client has been disconnected with reason: You were disconnected for being idle 20 minutes"]);
    let state = r.poll(|s| s.status == "disconnected" && s.at > idle_at).await;
    c.check("an idle kick in words, with its minutes: 278", state.as_ref().is_some_and(|s| s.reason == Some(278)), &state);
    // DisconnectRejoin (276): the client says the same as for 273, the same account launched from another device.
    let rejoin_at = dropped(r, &["[FLog::Network] Disconnection Notification. Reason: 276"]);
    let state = r.poll(|s| s.status == "disconnected" && s.at > rejoin_at).await;
    r.clear_closing();
    r.told();
    r.tick().await;
    let rejoin_told = r.told();
    c.check(
        "276 (the same account launched elsewhere, as 273): left as it is, no rejoin",
        state.as_ref().is_some_and(|s| s.reason == Some(276)) && !r.closing() && rejoin_told.is_none() && !r.should_rejoin(&dl(&dropped_launch), state.as_ref()),
        &state,
    );
    // A code Roblox's own error box has no Reconnect for: no rejoin, said once, why.
    let private_at = dropped_at(
        r,
        &not_banned,
        &dropped_launch,
        now().plus_secs(-2.0),
        &["[FLog::Network] Client has been disconnected with reason: You have been disconnected because you no longer have access to this private server"],
    );
    let state = r.poll(|s| s.status == "disconnected" && s.at > private_at).await;
    r.clear_closing();
    r.told();
    r.tick().await;
    let refused_said = r.told();
    r.tick().await;
    let again = r.told();
    c.check(
        "a code Roblox gives no Reconnect for (286, no access to the private server): no rejoin, and why, once",
        state.as_ref().is_some_and(|s| s.reason == Some(286))
            && !r.closing()
            && !r.should_rejoin(&dl(&dropped_launch), state.as_ref())
            && refused_said.as_deref()
                == Some(format!("{} isn't rejoined (no access to the private server any more): Roblox doesn't allow reconnecting after this.", r.label()).as_str())
            && again.is_none(),
        format!("{state:?} | {refused_said:?}"),
    );
    let refused_alert = r.alert_title(
        "drop",
        "{0} isn't rejoined ({1}): Roblox doesn't allow reconnecting after this.",
        vec![r.label(), "no access to the private server any more".into()],
        refused_said.as_deref().unwrap_or(""),
    );
    c.check("…Discord's alert: \"Disconnected, not rejoining\"", refused_alert.title == "Disconnected, not rejoining", &refused_alert.title);
    // On the way in: waiting for a server, teleporting, or left before any server took it.
    r.new_window();
    r.run(Some(not_banned.clone()));
    let way_at = now().plus_secs(-5.0);
    r.line(way_at, LAUNCH);
    r.poll(|s| s.status == "joining" && s.job_id.is_none() && s.at >= way_at.plus_secs(-1.0)).await;
    c.check("started, no server yet: waiting for a server", app.phase(me).as_deref() == Some("waiting"), app.phase(me));
    r.line(way_at.plus_secs(1.0), JOIN);
    r.line(way_at.plus_secs(1.0), "[FLog::Network] serverId: 128.116.46.33|52353");
    r.poll(|s| s.status == "ingame" && s.at > way_at).await;
    c.check("…in its server: none", app.phase(me).is_none(), app.phase(me));
    r.line(way_at.plus_secs(2.0), "[FLog::SingleSurfaceApp] launchUGCGameInternal: deferred download complete");
    r.line(way_at.plus_secs(2.0), "[FLog::UgcExperienceController] UgcExperienceController: doTeleport: url https://gamejoin.roblox.com/v1/join-reserved-game");
    let state = r.poll(|s| s.teleport_started.is_some()).await;
    c.check(
        "Roblox's other launchUGCGameInternal lines aren't a game start, and a teleport is shown as one",
        state.as_ref().is_some_and(|s| s.status == "ingame" && !s.person_started && s.teleport_started.is_some()) && app.phase(me).as_deref() == Some("teleporting"),
        &state,
    );
    r.line(way_at.plus_secs(3.0), "[FLog::Output] ! Joining game '22222222-2222-3333-4444-555555555555' place 107778070777162 at 10.0.0.2");
    r.line(way_at.plus_secs(3.0), "[FLog::Network] serverId: 128.116.46.34|52353");
    let state = r.poll(|s| s.status == "ingame" && s.teleport_started.is_none()).await;
    c.check(
        "…until the next server is reached",
        state.as_ref().is_some_and(|s| s.job_id.as_deref() == Some("22222222-2222-3333-4444-555555555555")) && app.phase(me).is_none(),
        &state,
    );
    r.new_window();
    r.run(Some(not_banned.clone()));
    let fail_at = now().plus_secs(-70.0);
    r.line(fail_at, LAUNCH);
    r.line(fail_at.plus_secs(62.0), LEAVE);
    let state = r.poll(|s| s.status == "left" && s.at > fail_at).await;
    c.check(
        "left before any server took it (a join error's Leave, or a wait given up): told apart from a game left",
        state.as_ref().is_some_and(|s| s.never_joined) && app.phase(me).as_deref() == Some("joinFailed"),
        &state,
    );
    r.clear_closing();
    // When a join not in yet is asked about: at once for a game known to have banned the account, else only after the
    // longest silent wait a good join was seen to take (38 s on 2026-10-16).
    let after = |place: i64| app.join_check_after(&r.acc(), &GameTarget::new(place, None, None, None));
    c.check(
        "a join not in yet is asked about after 5 s for a game that banned the account, after 40 s otherwise",
        after(GPO) == Duration::from_secs(5) && after(920587237) == Duration::from_secs(40),
        (after(GPO), after(920587237)),
    );
    // Roblox's answer to a join it refuses (Join Error 600 in the window, nothing in the log).
    let refused = answer(200, Some(12), Some("You were banned by this experience or its moderators. Moderation message: Exploiting"));
    c.check("Roblox's join answer for a game's ban: read as a ban, with the game's words", refused.banned() && refused.reason().as_deref() == Some("Exploiting"), refused.reason());
    c.check("…and one for a full server isn't", !answer(200, Some(6), Some("The game you requested is currently full. Please try again later.")).banned(), ());
    let real = answer(200, Some(12), Some("Unable to join Game 211"));
    c.check(
        "Roblox's real answer for a game's ban (status 12, \"Unable to join Game 211\"): a ban, without the game's words",
        real.banned() && real.code() == Some(211) && real.reason().is_none(),
        (real.banned(), real.code(), real.reason()),
    );
    c.check("…but not another refusal with status 12", !answer(200, Some(12), Some("Unable to join Game 524")).banned(), ());
    let with_data = JoinAnswer { data: Some(r#"{"moderationMessage":"Exploiting"}"#.into()), ..answer(200, Some(12), Some("Unable to join Game 211")) };
    c.check("…and the game's words when its statusData gives them", with_data.reason().as_deref() == Some("Exploiting"), with_data.reason());
    // Opened into that game again: the card says the ban once Roblox's join answer did, not before.
    r.new_window();
    r.run(game(GPO));
    r.line(now(), LAUNCH);
    r.poll(|s| s.status == "joining" && s.job_id.is_none()).await;
    c.check(
        "a window opened into a game that banned the account: not said banned before Roblox answers (the ban may be over)",
        r.card_banned() == Some(false),
        r.card_banned(),
    );
    app.rejoin.game_ban_noted.borrow_mut().insert(me);
    c.check("…its card says the ban, not Joining, once Roblox's join answer said it", r.card_banned() == Some(true), r.card_banned());
    app.rejoin.game_ban_noted.borrow_mut().clear();
    // Minutes on that screen: its log no longer decides (JoiningTimeout) and Roblox's status says not in a game, twice.
    // That isn't a lost connection to restart Roblox for.
    r.new_window();
    r.run_at(game(GPO), now().plus_secs(-240.0));
    r.line(now().plus_secs(-240.0), LAUNCH);
    r.poll(|s| s.status == "joining" && s.job_id.is_none() && now().secs_since(s.at) > 180.0).await;
    r.edit(|a| a.user_id = 21);
    app.controller.presence.borrow_mut().insert(21, Presence { r#type: 1, location: Some("Website".into()), ..Default::default() });
    r.clear_closing();
    r.told();
    for _ in 0..3 {
        app.detect_disconnected_sessions(&HashSet::from([21i64]));
    }
    let lost_told = r.told();
    c.check(
        "…nor is it closed as a lost connection when Roblox's status says not in a game",
        !r.closing() && lost_told.is_none(),
        app.close_desk.closing.borrow().len(),
    );
    // Its ban told after 5 seconds, the window still on Join Error 600 a minute later: not closed with "trying again".
    r.new_window();
    r.run_at(game(GPO), now().plus_secs(-120.0));
    r.line(now().plus_secs(-70.0), LAUNCH);
    r.poll(|s| s.status == "joining" && s.job_id.is_none() && now().secs_since(s.at) > 65.0).await;
    app.rejoin.game_ban_noted.borrow_mut().insert(me);
    r.clear_closing();
    r.told();
    r.tick().await;
    let trying_told = r.told();
    c.check(
        "…nor, its ban told by Roblox's join answer, closed a minute on with \"trying again\"",
        !r.closing() && trying_told.is_none(),
        app.close_desk.closing.borrow().len(),
    );
    app.rejoin.game_ban_noted.borrow_mut().clear();
    // In that game again later: the ban is over.
    r.new_window();
    r.run(game(GPO));
    r.line(now(), LAUNCH);
    r.line(now(), &format!("[FLog::Output] ! Joining game 'e403336e-5756-44c9-b029-0ae6873a3d03' place {GPO} at 10.32.36.210"));
    r.line(now(), "[FLog::Network] serverId: 128.116.46.33|56123");
    r.poll(|s| s.status == "ingame").await;
    app.forget_lifted_game_bans();
    c.check("…and once the account is in that game again, the ban is forgotten", r.acc().game_bans.is_empty(), r.acc().game_bans.len());

    // What a review of these found (2026-10-21), one case each.
    // A ban seen again after Bloxkeep restarted (an update), its window still open: told once.
    r.new_window();
    r.run(game(GPO));
    r.told();
    let seen_at = now();
    played(r, seen_at, BAN_LINE);
    r.poll(|s| s.status == "disconnected" && s.banned_by_game && s.at > seen_at).await;
    r.tick().await;
    let first_told = r.told();
    app.rejoin.game_ban_noted.borrow_mut().clear(); // as after a restart: nothing of it in memory
    r.tick().await;
    let again_told = r.told();
    c.check(
        "a ban seen again after a restart, its window still open: not told a second time",
        first_told.is_some() && again_told.is_none() && r.acc().game_bans.len() == 1,
        &again_told,
    );
    // Banned in a game the person picked in the window: that game is the one noted, not the one Bloxkeep opened.
    r.edit(|a| a.game_bans.clear());
    app.rejoin.game_ban_noted.borrow_mut().clear();
    r.new_window();
    r.run(game(920587237));
    let picked_at = now();
    r.line(picked_at, LAUNCH); // Bloxkeep's start; played's is the person's
    played(r, picked_at.plus_secs(0.5), BAN_LINE);
    r.poll(|s| s.status == "disconnected" && s.banned_by_game && s.chosen_place == Some(GPO) && s.at > picked_at).await;
    r.tick().await;
    let bans: Vec<i64> = r.acc().game_bans.iter().map(|b| b.place_id).collect();
    c.check("banned in a game the person picked in the window: that game is noted", bans == [GPO], &bans);
    // A preset leaves out an account the game banned, rather than asking; opened by hand, it is asked first. Nothing
    // opens either way.
    let open_before = r.take_running();
    app.controller.running.borrow_mut().clear();
    let bans_before = r.acc().game_bans.clone();
    r.edit(|a| a.game_bans = vec![GameBan { place_id: GPO, game_name: None, at: now(), message: None, until: None }]);
    let preset_result = open(&app, r.acc(), GPO, Some(crate::core::base::new_guid())).await;
    let preset_error = refusal(&preset_result);
    c.check(
        "a preset leaves out an account the game banned, without a question",
        preset_error.as_deref().is_some_and(|e| e.contains("the preset didn't open it")),
        format!("{preset_error:?} {preset_result:?}"),
    );
    let by_hand = open(&app, r.acc(), GPO, None).await;
    let asks = by_hand.as_ref().ok().and_then(|x| x.asks.clone());
    c.check("…opened by hand, it is asked first", asks.as_ref().is_some_and(|a| a.iter().any(|k| k == "gameBan")), format!("{by_hand:?}"));
    r.edit(|a| a.game_bans = bans_before);
    r.put_running(open_before);
    // A window opened into the game that banned the account, the person in another game from it now: the ban stands,
    // and the window isn't held as banned any more.
    r.new_window();
    r.run(game(GPO));
    let other_at = now();
    r.line(other_at, LAUNCH);
    r.line(other_at, LAUNCH);
    r.line(other_at, "[FLog::Output] ! Joining game '77777777-2222-3333-4444-555555555555' place 444 at 10.0.0.7");
    r.line(other_at, "[FLog::Network] serverId: 10.0.0.7|5000");
    r.poll(|s| s.status == "ingame" && s.place_id == Some(444)).await;
    app.rejoin.game_ban_noted.borrow_mut().insert(me);
    r.tick().await;
    app.forget_lifted_game_bans();
    c.check("in another game, from a window opened into the one that banned it: that ban isn't forgotten", r.acc().game_bans.iter().any(|b| b.place_id == GPO), ());
    c.check("…and the window, in a server now, isn't held as banned", !r.noted(), ());
    // Nothing open any more: what was held about the window goes (a later client may get its process id).
    let held = r.take_running();
    app.controller.running.borrow_mut().clear();
    app.rejoin.game_ban_noted.borrow_mut().insert(me);
    r.set_check(Some(None), now());
    r.tick().await;
    c.check(
        "…and once nothing is open, nothing is held about its process id",
        app.rejoin.game_ban_noted.borrow().is_empty() && app.rejoin.join_checks.borrow().is_empty(),
        (app.rejoin.game_ban_noted.borrow().len(), app.rejoin.join_checks.borrow().len()),
    );
    r.put_running(held);
    // Roblox's join question failed: the check of every window goes on. Roblox's join API answers from here (questions
    // counts them): nothing goes to Roblox.
    r.edit(|a| a.cookie = "test".into());
    let real_ask = app.rejoin.ask_join.borrow().clone();
    let questions = Arc::new(AtomicI32::new(0));
    let reply = Arc::new(Mutex::new(answer(200, Some(12), Some("Unable to join Game 211"))));
    {
        let (q, rep) = (questions.clone(), reply.clone());
        *app.rejoin.ask_join.borrow_mut() = Some(Arc::new(move |_, _, _| {
            q.fetch_add(1, Ordering::SeqCst);
            Ok(rep.lock().unwrap().clone())
        }));
    }
    joins(c, r, &questions, &reply).await;
    r.edit(|a| a.cookie = String::new());
    app.rejoin.join_checks.borrow_mut().clear();
    *app.rejoin.ask_join.borrow_mut() = real_ask;

    // A rejoin waiting its turn for an account a game banned since it was queued: not done (no question to Roblox, no
    // window: it returns at once).
    r.edit(|a| a.game_bans = vec![GameBan { place_id: GPO, game_name: None, at: now(), message: None, until: None }]);
    let queued_open = r.take_running();
    app.controller.running.borrow_mut().clear();
    let waiting_rejoin = RejoinRequest::new(id, GameTarget::new(GPO, None, None, None), now());
    let returned_at_once = app.clone().rejoin(waiting_rejoin).now_or_never().is_some();
    c.check("a rejoin waiting its turn for an account the game banned since: not done", returned_at_once, ());
    r.put_running(queued_open);
    r.edit(|a| a.game_bans.clear());
    // A ban in the last lines of a window that closed before the logs were looked at: noted (and told) all the same.
    r.told();
    let closed_at_once = Rc::new(PendingLaunch::new(id, HashSet::new(), now().plus_secs(-120.0), game(GPO), false, None));
    let mut last_lines = ClientState::new("disconnected", Some(GPO), Some("e403336e-5756-44c9-b029-0ae6873a3d03".into()), Some(267), now());
    last_lines.banned_by_game = true;
    app.on_session_ended(id, &closed_at_once, Some(last_lines), End::Unasked);
    let bans: Vec<i64> = r.acc().game_bans.iter().map(|b| b.place_id).collect();
    let ban_told = r.told();
    c.check(
        "a ban in the last lines of a window closed at once: noted and told all the same",
        bans == [GPO] && ban_told.is_some() && r.queue_len() == 0,
        &bans,
    );
    // A game's ban holds in all its places: one noted for one place counts for another of the same game.
    const GPO_OTHER: i64 = 1730877807;
    let gpo_info = |place: i64| GameInfo { place_id: place, universe_id: 648454481, name: "Grand Piece Online".into(), creator: String::new(), playing: 0, icon_url: None };
    app.controller.games.borrow_mut().insert(GPO, Some(gpo_info(GPO)));
    app.controller.games.borrow_mut().insert(GPO_OTHER, Some(gpo_info(GPO_OTHER)));
    c.check("a game's ban noted for one of its places counts for another", app.game_ban_of(&r.acc(), Some(GPO_OTHER)).is_some(), ());
    start_place_of_its_own(&app, GPO);
    app.controller.games.borrow_mut().remove(&GPO_OTHER);
    r.edit(|a| a.game_bans.clear());
    // A game started in the window while still on a server, its old server's kick (a ban) logged after the new join:
    // that ban goes with the old server, not with the next server's disconnect.
    r.new_window();
    r.run(game(GPO));
    let hop_at = now();
    r.line(hop_at, LAUNCH);
    r.line(hop_at, &format!("[FLog::Output] ! Joining game 'e403336e-5756-44c9-b029-0ae6873a3d03' place {GPO} at 10.32.36.210"));
    r.line(hop_at, "[FLog::Network] serverId: 128.116.46.33|56123");
    r.line(hop_at.plus_secs(1.0), "[FLog::Output] ! Joining game '88888888-2222-3333-4444-555555555555' place 444 at 10.0.0.8");
    r.line(hop_at.plus_secs(1.1), BAN_LINE);
    r.line(hop_at.plus_secs(1.2), "[FLog::Network] Time to disconnect replication data: 0.2");
    r.line(hop_at.plus_secs(2.0), "[FLog::Network] serverId: 10.0.0.8|5000");
    r.line(hop_at.plus_secs(3.0), "[FLog::Network] Disconnection Notification. Reason: 277");
    r.line(hop_at.plus_secs(3.1), "[FLog::Network] Time to disconnect replication data: 0.2");
    let state = r.poll(|s| s.status == "disconnected" && s.at > hop_at.plus_secs(2.5)).await;
    c.check(
        "an old server's ban logged after the next join: not taken for the next server's disconnect",
        state.as_ref().is_some_and(|s| s.status == "disconnected" && s.reason == Some(277) && !s.banned_by_game),
        &state,
    );
    app.rejoin.game_ban_noted.borrow_mut().clear();
    r.edit(|a| a.game_bans.clear());

    // Opening a game for an account whose window is somewhere already: asked first only if it is in a game.
    let asks_for = |launch: &Rc<PendingLaunch>, wanted: i64| app.leaves_a_game(&(me, launch.clone()), &GameTarget::new(wanted, None, None, None));
    r.new_window();
    let playing = r.run(game(GPO));
    r.line(now(), LAUNCH);
    r.line(now(), &format!("[FLog::Output] ! Joining game 'e403336e-5756-44c9-b029-0ae6873a3d03' place {GPO} at 10.32.36.210"));
    r.line(now(), "[FLog::Network] serverId: 128.116.46.33|56123");
    r.poll(|s| s.status == "ingame" && s.place_id == Some(GPO)).await;
    c.check("opening a game for an account in another game: asked first", asks_for(&playing, 920587237), ());
    r.new_window();
    let home = r.run(None);
    c.check("…but not for one only on Roblox's home screen: it gives way without a word", !asks_for(&home, 920587237), ());
    r.new_window();
    let stuck_open = r.run(game(GPO));
    r.line(now(), LAUNCH);
    r.poll(|s| s.status == "joining" && s.job_id.is_none()).await;
    c.check("…nor for one stuck on a join without a server (a game's ban, Join Error 600)", !asks_for(&stuck_open, 920587237), ());

    newer_drops(c, r).await;
    closed_by_roblox(c, r, &not_banned).await;
    memory_and_waits(c, r).await;
}

/// Main's 1.0.23 rejoin checks (RejoinSuite.cs): a reason line is a drop by itself (Error Code 256), Roblox's "Connection
/// Failed" box (279), a teleport's old server's reason, a log in a game while Roblox's status says out of one, and a timed
/// game ban's end.
async fn newer_drops(c: &mut Checks, r: &mut R) {
    let app = r.app.clone();
    let me = r.me;
    let not_banned = game(PLACE);
    // Error Code 256, "Lost connection due to an unknown error. Please reconnect." (a user's screenshot, 2026-11-11, "auto
    // rejoin will not work"): by its code and by its words only. Without the disconnect line after too: a game that failed
    // while loading showed Roblox's "Disconnected" box and stayed "In game" (a RoALT user's report, 2026-11-14).
    let cases: [(&str, &[&str], bool); 4] = [
        ("its code", &["[FLog::Network] Disconnect reason received: 256", "[FLog::Network] Disconnection Notification. Reason: 256"], true),
        (
            "its words only",
            &[
                "[FLog::Network] Client has been disconnected with reason: Lost connection due to an unknown error. Please reconnect.",
                "Warning [FLog::Network] Lost connection with reason : Lost connection due to an unknown error. Please reconnect.",
            ],
            true,
        ),
        ("its code, no disconnect line after", &["[FLog::Network] Disconnection Notification. Reason: 256"], false),
        ("its words, no disconnect line after", &["Warning [FLog::Network] Lost connection with reason : Lost connection to the game server, please reconnect"], false),
    ];
    for (what, lines, disconnect_line) in cases {
        let drop_at = now().plus_secs(-10.0);
        r.new_window();
        r.run(not_banned.clone());
        r.line(drop_at, LAUNCH);
        r.line(drop_at, JOIN);
        r.line(drop_at, "[FLog::Network] serverId: 128.116.46.33|52353");
        for line in lines {
            r.line(drop_at.plus_secs(1.0), line);
        }
        if disconnect_line {
            r.line(drop_at.plus_secs(2.0), "[FLog::Network] Time to disconnect replication data: 0.2");
        }
        let state = r.poll(|s| s.status == "disconnected" && s.at > drop_at.plus_secs(-1.0)).await;
        r.clear_closing();
        r.told();
        r.tick().await;
        let said = r.told();
        c.check(&format!("Error Code 256 by {what}: read as a drop and rejoined"), r.closing(), format!("{state:?} | {said:?}"));
    }

    // Roblox's "Connection Failed" box (279, no response from server) after "Joining game": a test, a rejoin into a server
    // that had just run out of memory. No serverId, no disconnect line. The real lines, in order.
    {
        let at = now().plus_secs(-40.0);
        r.new_window();
        r.run(not_banned.clone());
        r.line(at, LAUNCH);
        r.line(at, JOIN);
        r.line(
            at.plus_secs(10.0),
            "Error [DFLog::RbxTransportDummyClient] Failed to establish connection to server at 128.116.46.33:49386, reason NoResponse: 1 (No Response)",
        );
        let still = r.poll(|s| s.status == "joining").await;
        r.clear_closing();
        r.tick().await;
        c.check(
            "Connection Failed: RbxTransport's own failure isn't the end (RakNet is tried after it)",
            still.as_ref().is_some_and(|s| s.status == "joining") && !r.closing(),
            &still,
        );
        r.line(at.plus_secs(30.0), "Error [DFLog::NetworkClient] Failed to connect to server at 128.116.46.33|50694, no response.");
        r.line(
            at.plus_secs(30.0),
            "Info [DFLog::NetworkClient] Failed to connect to server after falling back from RbxTransport to RakNet. Disconnect code: 279. Original connection failure reason: NoResponse: 1 (No Response)",
        );
        let failed = r.poll(|s| s.status == "disconnected").await;
        r.clear_closing();
        r.told();
        r.tick().await;
        let said = r.told();
        c.check(
            "Connection Failed (279) after Joining game: read as a drop and rejoined",
            failed.as_ref().is_some_and(|s| s.status == "disconnected" && s.reason == Some(279)) && r.closing(),
            format!("{failed:?} | {said:?}"),
        );
        c.check("…said as a lost connection", said.as_deref() == Some(format!("{} lost connection — restarting Roblox…", r.label()).as_str()), &said);
    }

    // A teleport's old server may log a reason: still a teleport, not a drop (RoALT's "instanced server" report, 2026-11-13).
    {
        let at = now().plus_secs(-10.0);
        r.new_window();
        r.run(not_banned.clone());
        r.line(at, LAUNCH);
        r.line(at, JOIN);
        r.line(at, "[FLog::Network] serverId: 128.116.46.33|52353");
        r.line(at.plus_secs(5.0), "[FLog::UgcExperienceController] UgcExperienceController: doTeleport: url ");
        r.line(at.plus_secs(5.1), "[FLog::Network] Disconnection Notification. Reason: 277");
        r.line(at.plus_secs(5.2), "[FLog::Network] Time to disconnect replication data: 0.2");
        let state = r.poll(|s| s.at >= at.plus_secs(5.0)).await;
        r.clear_closing();
        r.tick().await;
        c.check(
            "a teleport whose old server logs a reason: joining, left alone",
            state.as_ref().is_some_and(|s| s.status == "joining") && !r.closing(),
            &state,
        );
        // The move failing after the old server is gone: Roblox's "Disconnected" box, Error Code 256 mid-teleport, its
        // words logged (as a user's three windows logged them). A drop at once, closed to rejoin.
        for failed in [
            "[FLog::Network] Client has been disconnected with reason: Disconnected from game, please reconnect",
            "Warning [FLog::Network] Lost connection with reason : Lost connection due to an unknown error. Please reconnect.",
            "[FLog::Network] Disconnection Notification. Reason: 256",
        ] {
            r.new_window();
            r.run(not_banned.clone());
            r.line(at, LAUNCH);
            r.line(at, JOIN);
            r.line(at, "[FLog::Network] serverId: 128.116.46.33|52353");
            r.line(at.plus_secs(5.0), "[FLog::UgcExperienceController] UgcExperienceController: doTeleport: url ");
            r.line(at.plus_secs(5.2), "[FLog::Network] Time to disconnect replication data: 0.2");
            r.line(at.plus_secs(5.3), "[FLog::SessionTransitionFSM] Teleported.");
            r.line(at.plus_secs(6.0), failed);
            let state = r.poll(|s| s.status == "disconnected" && s.at >= at.plus_secs(5.0)).await;
            r.clear_closing();
            r.told();
            r.tick().await;
            c.check(
                &format!("a teleport failing after its old server is gone ({}): Error 256, closed to rejoin at once", failed.split("] ").nth(1).unwrap_or(failed)),
                state.as_ref().is_some_and(|s| s.reason == Some(256)) && r.closing(),
                &state,
            );
        }
        r.clear_closing();
        r.told();
        // Leaving: the client's own request is no drop.
        r.new_window();
        r.run(not_banned.clone());
        r.line(at, LAUNCH);
        r.line(at, JOIN);
        r.line(at, "[FLog::Network] serverId: 128.116.46.33|52353");
        r.line(at.plus_secs(5.0), "[FLog::Network] Disconnection Notification. Reason: 285");
        let own = r.poll(|s| s.at >= at && s.status == "ingame").await;
        rt::sleep(Duration::from_millis(300)).await;
        let own = r.poll(|_| true).await.or(own);
        c.check("…the client's own request (285) alone is no drop", own.as_ref().is_some_and(|s| s.status == "ingame"), &own);
    }

    // Its log in a game, no disconnect line, while Roblox's own status says it's out of one (Roblox's "Disconnected" box
    // over a game that failed while loading, a RoALT user's report 2026-11-14): closed and rejoined after three checks in a
    // row; never on Roblox's word while it says in a game, nor right after the join.
    {
        const IN_PLACE: i64 = 920587237;
        let join_line = format!("[FLog::Output] ! Joining game 'e403336e-5756-44c9-b029-0ae6873a3d03' place {IN_PLACE} at 10.32.36.210");
        r.edit(|a| a.user_id = 21);
        r.new_window();
        r.run_at(game(IN_PLACE), now().plus_secs(-300.0));
        let in_at = now().plus_secs(-240.0);
        r.line(in_at, LAUNCH);
        r.line(in_at, &join_line);
        r.line(in_at, "[FLog::Network] serverId: 128.116.46.33|56123");
        r.poll(|s| s.status == "ingame" && s.at <= in_at.plus_secs(1.0)).await;
        app.controller.offline_strikes.borrow_mut().clear();
        r.clear_closing();
        r.told();
        let presence = |p: Presence| app.controller.presence.borrow_mut().insert(21, p);
        presence(Presence { r#type: 2, location: Some("Game".into()), place_id: Some(IN_PLACE), ..Default::default() });
        for _ in 0..4 {
            app.detect_disconnected_sessions(&HashSet::from([21i64]));
        }
        c.check("a log in a game and Roblox saying so too: left alone", !r.closing(), ());
        presence(Presence { r#type: 1, location: Some("Website".into()), ..Default::default() });
        app.detect_disconnected_sessions(&HashSet::from([21i64]));
        app.detect_disconnected_sessions(&HashSet::from([21i64]));
        let after_two = r.closing();
        app.detect_disconnected_sessions(&HashSet::new());
        let unreliable = r.closing();
        app.detect_disconnected_sessions(&HashSet::from([21i64]));
        c.check(
            "a log in a game while Roblox's own status says out of one: closed and rejoined on the third check, not before, nor on a status not its own",
            !after_two && !unreliable && r.closing(),
            format!("{after_two} {unreliable} {}", r.closing()),
        );
        // Just in: Roblox's status may not have caught up.
        r.new_window();
        r.run_at(game(IN_PLACE), now().plus_secs(-300.0));
        let just_at = now().plus_secs(-30.0);
        r.line(just_at, LAUNCH);
        r.line(just_at, &join_line);
        r.line(just_at, "[FLog::Network] serverId: 128.116.46.33|56123");
        r.poll(|s| s.status == "ingame" && s.at >= just_at.plus_secs(-1.0)).await;
        app.controller.offline_strikes.borrow_mut().clear();
        r.clear_closing();
        for _ in 0..4 {
            app.detect_disconnected_sessions(&HashSet::from([21i64]));
        }
        c.check("…but not in its first two minutes in the game", !r.closing(), ());
        r.clear_closing();
        r.told();
        app.controller.presence.borrow_mut().remove(&21);
    }

    // A teleport that never arrives: Roblox's "Disconnected" box, Error Code 256, in the middle of a game's move to
    // another server (many users of several apps, 2026-11-15: "it doesn't reconnect when this disconnect prompt pops up,
    // still flags it as in game"; games that teleport, Pet Universe). The log says a teleport started and nothing
    // after it. While it may still be under way it is left alone; 45 s after, Roblox's own status decides, on two
    // checks in a row; and never while Roblox says the account is in a game.
    {
        const IN_PLACE: i64 = 920587237;
        let join_line = format!("[FLog::Output] ! Joining game 'e403336e-5756-44c9-b029-0ae6873a3d03' place {IN_PLACE} at 10.32.36.210");
        let presence = |p: Presence| app.controller.presence.borrow_mut().insert(21, p);
        let out = Presence { r#type: 1, location: Some("Website".into()), ..Default::default() };
        // In the game ten minutes, then its teleport `started_ago` seconds ago, and nothing after.
        fn moved(r: &mut R, join_line: &str, started_ago: f64) {
            r.edit(|a| a.user_id = 21);
            r.new_window();
            r.run_at(game(IN_PLACE), now().plus_secs(-900.0));
            let in_at = now().plus_secs(-600.0);
            r.line(in_at, LAUNCH);
            r.line(in_at, join_line);
            r.line(in_at, "[FLog::Network] serverId: 128.116.46.33|56123");
            r.line(now().plus_secs(-started_ago), "[FLog::UgcExperienceController] UgcExperienceController: doTeleport: url ");
        }
        moved(r, &join_line, 20.0);
        r.poll(|s| s.teleport_started.is_some()).await;
        app.controller.offline_strikes.borrow_mut().clear();
        r.clear_closing();
        r.told();
        presence(out.clone());
        for _ in 0..4 {
            app.detect_disconnected_sessions(&HashSet::from([21i64]));
        }
        c.check("a teleport started twenty seconds ago, Roblox's status out of a game (it lags a move): left alone", !r.closing(), ());
        moved(r, &join_line, 120.0);
        r.poll(|s| s.teleport_started.is_some_and(|t| now().secs_since(t) >= 100.0)).await;
        app.controller.offline_strikes.borrow_mut().clear();
        r.clear_closing();
        presence(Presence { r#type: 2, location: Some("Game".into()), place_id: Some(IN_PLACE), ..Default::default() });
        for _ in 0..4 {
            app.detect_disconnected_sessions(&HashSet::from([21i64]));
        }
        c.check("a teleport two minutes old while Roblox says the account is in a game: left alone (a slow move)", !r.closing(), ());
        presence(out.clone());
        app.detect_disconnected_sessions(&HashSet::from([21i64]));
        let after_one = r.closing();
        app.detect_disconnected_sessions(&HashSet::from([21i64]));
        c.check(
            "a teleport that never arrived (Error 256 mid-teleport) while Roblox says the account is out of a game: closed and rejoined on the second check",
            !after_one && r.closing(),
            format!("{after_one} {}", r.closing()),
        );
        r.clear_closing();
        r.told();
        app.controller.presence.borrow_mut().remove(&21);
    }

    // A timed ban (BanAsync with a duration): its end is in the kick line's metadata. a test, the exact metadata:
    // still "banned" ten minutes after the 2-minute ban ended.
    {
        let at = |t: &str| Stamp::parse(t).expect("a time");
        let real_end = roblox_log_watcher::ban_end_of(
            r#"{"moderatorType":"creator","durationSeconds":"120","endTimestamp":"2026-11-13T23:09:32Z","dynamicReasonTextFilterStatus":"2","beginTimestamp":"2026-11-13T23:07:32Z"}"#,
        );
        c.check("a timed ban's end is read from its metadata (endTimestamp, UTC)", real_end == Some(at("2026-11-13T23:09:32Z")), &real_end);
        let from_duration = roblox_log_watcher::ban_end_of(r#"{"moderatorType":"creator","durationSeconds":"120","beginTimestamp":"2026-11-13T23:07:32Z"}"#);
        c.check("…or from its start and duration", from_duration == Some(at("2026-11-13T23:09:32Z")), &from_duration);
        let permanent = roblox_log_watcher::ban_end_of(
            r#"{"moderatorType":"creator","durationSeconds":"-1","dynamicReasonTextFilterStatus":"2","beginTimestamp":"2026-10-21T00:07:04Z"}"#,
        );
        c.check("…a permanent one has none", permanent.is_none(), &permanent);

        let iso = |s: Stamp| format!("{}Z", &s.to_iso()[..19]);
        for (what, ends_in) in [("still running", 120.0), ("already over", -30.0)] {
            r.edit(|a| a.game_bans.clear());
            let ends = now().plus_secs(ends_in);
            r.new_window();
            r.run(game(GPO));
            let timed_at = now();
            let kick = format!(
                "Error [FLog::CreatorError] Error: Server Kick Message: test 12: ban 2 phut; Metadata: {{\"moderatorType\":\"creator\",\"durationSeconds\":\"120\",\"endTimestamp\":\"{}\",\"dynamicReasonTextFilterStatus\":\"2\",\"beginTimestamp\":\"{}\"}}",
                iso(ends),
                iso(ends.plus_secs(-120.0))
            );
            r.line(timed_at, LAUNCH);
            r.line(timed_at, &format!("[FLog::Output] ! Joining game 'e403336e-5756-44c9-b029-0ae6873a3d03' place {GPO} at 10.32.36.210"));
            r.line(timed_at, "[FLog::Network] serverId: 128.116.46.33|56123");
            r.line(timed_at.plus_secs(1.0), &kick);
            r.line(timed_at.plus_secs(2.0), "[FLog::Network] Disconnect reason received: 267");
            r.line(timed_at.plus_secs(2.0), "[FLog::Network] Disconnection Notification. Reason: 267");
            r.line(timed_at.plus_secs(2.1), "[FLog::Network] Time to disconnect replication data: 0.2");
            let timed = r.poll(|s| s.status == "disconnected" && s.at >= timed_at.plus_secs(-1.0)).await;
            r.clear_closing();
            r.told();
            r.tick().await;
            r.told();
            let holds = app.game_ban_of(&r.acc(), Some(GPO)).is_some();
            app.forget_lifted_game_bans();
            let bans = r.acc().game_bans;
            if ends_in > 0.0 {
                c.check(
                    &format!("a timed ban {what}: noted with its end, and it holds"),
                    timed.as_ref().is_some_and(|s| s.banned_by_game && s.ban_ends.is_some())
                        && holds
                        && bans.len() == 1
                        && bans[0].until.is_some_and(|u| u.secs_since(ends).abs() < 1.5),
                    format!("{timed:?} | {:?}", bans.first().and_then(|b| b.until)),
                );
            } else {
                c.check(&format!("a timed ban {what}: holds no longer, and is forgotten (no warning before opening)"), !holds && bans.is_empty(), bans.len());
            }
        }
        r.edit(|a| a.game_bans.clear());
    }

    // A window opened on Roblox's home screen, a game picked there, then Error Code 1025 ("Disconnected from the
    // experience", a real PC, client 0.747's lines): it stayed on Roblox's error box, left as "opened without a
    // game". It goes back to the game the person was in.
    {
        const PICKED: i64 = 98894876188248;
        let picked_join = format!("[FLog::Output] ! Joining game '14a2746a-981f-4fed-92c9-d3a998eb8f4c' place {PICKED} at 128.116.54.33");
        let home_run = |r: &R| -> Rc<PendingLaunch> {
            let mut opened = PendingLaunch::new(r.id, HashSet::new(), now().plus_secs(-120.0), None, false, None);
            opened.home_screen = true;
            let opened = Rc::new(opened);
            r.app.controller.running.borrow_mut().insert(r.id, (r.me, opened.clone()));
            opened
        };
        let home_at = now().plus_secs(-10.0);
        r.new_window();
        let home_launch = home_run(r);
        r.line(home_at, LAUNCH);
        r.line(home_at, &picked_join);
        r.line(home_at, "[FLog::Network] serverId: 128.116.54.33|60219");
        r.line(home_at.plus_secs(1.0), "[FLog::Network] Disconnect from 1");
        r.line(home_at.plus_secs(1.0), "[FLog::Network] Client has been disconnected with reason: Disconnected from game, please reconnect");
        r.line(home_at.plus_secs(1.0), "Warning [FLog::Network] Lost connection with reason : Disconnected from game, please reconnect");
        r.line(home_at.plus_secs(1.0), "[FLog::Network] Time to disconnect replication data: 0.188800");
        let home_state = r.poll(|s| s.status == "disconnected" && s.at > home_at).await;
        r.clear_closing();
        r.told();
        r.tick().await;
        c.check("a home-screen window dropped in a game picked there (1025): closed to rejoin", r.closing(), &home_state);
        r.clear_queue();
        app.on_session_ended(r.id, &home_launch, home_state.clone(), End::Rejoin);
        let home_target = r.queued_target();
        c.check(
            "…into that game, any server",
            home_target.as_ref().is_some_and(|t| t.place_id == PICKED && t.job_id.is_none()),
            format!("{home_target:?}"),
        );
        r.clear_queue();
        r.clear_closing();
        // The same drop in a window adopted from outside Bloxkeep (no game, not opened on the home screen by Bloxkeep): its
        // server may be a private one, so it isn't rejoined into a public one.
        let adopted = r.run(None);
        r.tick().await;
        c.check("…but not one adopted from outside Bloxkeep: left open", !r.closing(), ());
        c.check("…nor rejoined once closed", !app.queue_rejoin(r.id, &adopted, home_state.as_ref(), End::Unasked) && r.queue_len() == 0, r.queue_len());
        // Left by choice there: not rejoined; and one that never was in a game has nowhere to go back to.
        r.new_window();
        let home_launch = home_run(r);
        r.line(home_at.plus_secs(2.0), LAUNCH);
        r.line(home_at.plus_secs(2.0), &picked_join);
        r.line(home_at.plus_secs(2.0), "[FLog::Network] serverId: 128.116.54.33|60219");
        r.line(home_at.plus_secs(3.0), LEAVE);
        r.line(home_at.plus_secs(3.0), "[FLog::Network] Time to disconnect replication data: 0.2");
        let left = r.poll(|s| s.status == "left").await;
        r.tick().await;
        c.check("…one the person left there: not closed", !r.closing(), &left);
        c.check("…nor rejoined once closed (left there)", !app.queue_rejoin(r.id, &home_launch, left.as_ref(), End::Unasked) && r.queue_len() == 0, r.queue_len());
        c.check("…and a home-screen window never in a game isn't rejoined", !app.queue_rejoin(r.id, &home_launch, None, End::Unasked) && r.queue_len() == 0, r.queue_len());
        r.clear_queue();
        r.clear_closing();
    }
    let _ = me;
}

/// Roblox's join answers for windows that don't get in, playability, and rechecking a game's ban (lines 750-1018 of the C#).
async fn joins(c: &mut Checks, r: &mut R, questions: &Arc<AtomicI32>, reply: &Arc<Mutex<JoinAnswer>>) {
    let app = r.app.clone();
    let me = r.me;
    let id = r.id;
    let set_reply = |a: JoinAnswer| *reply.lock().unwrap() = a;
    let q = || questions.load(Ordering::SeqCst);

    r.new_window();
    r.run(game(920587237));
    r.line(now().plus_secs(-45.0), LAUNCH);
    r.poll(|s| s.status == "joining" && s.job_id.is_none() && now().secs_since(s.at) > 42.0).await;
    // PORT: a JoinSlot can't hold an exception: a question that failed reads as one without an answer (check_join's own
    // catch makes it so).
    r.set_check(Some(None), now());
    let ticked = std::panic::AssertUnwindSafe(async { r.tick().await }).catch_unwind().await;
    c.check("a join question that failed doesn't stop the check of the windows", ticked.is_ok(), ());
    // A window opened into a game that banned the account, a minute without a server, Roblox not saying the ban (a full
    // server): left as it is, as no rejoin would follow its close.
    r.new_window();
    r.run(game(920587237));
    r.edit(|a| a.game_bans.push(GameBan { place_id: 920587237, game_name: None, at: now().plus_secs(-3600.0), message: None, until: None }));
    r.line(now().plus_secs(-70.0), LAUNCH);
    r.poll(|s| s.status == "joining" && s.job_id.is_none() && now().secs_since(s.at) > 65.0).await;
    r.set_check(Some(Some(answer(200, Some(6), Some("The game you requested is currently full.")))), now());
    r.clear_closing();
    r.tick().await;
    c.check(
        "a window into a game that banned the account, not in a minute on: not closed when no rejoin follows",
        !r.closing() && r.acc().game_bans.iter().any(|b| b.place_id == 920587237),
        (),
    );
    // Roblox would let it in now (status 2): the ban is over, and it is tried again as any other.
    r.set_check(Some(Some(answer(200, Some(2), None))), now());
    r.tick().await;
    let bans: Vec<i64> = r.acc().game_bans.iter().map(|b| b.place_id).collect();
    c.check(
        "…but once Roblox would let it in, the ban is forgotten and it is tried again",
        r.closing() && bans.iter().all(|p| *p != 920587237),
        &bans,
    );
    r.clear_closing();
    // A join question that got no answer: asked again half a minute on, not never; a ban in that answer is then told.
    r.new_window();
    r.run(game(GPO));
    r.edit(|a| a.game_bans = vec![GameBan { place_id: GPO, game_name: None, at: now().plus_secs(-3600.0), message: None, until: None }]);
    r.line(now().plus_secs(-20.0), LAUNCH);
    r.poll(|s| s.status == "joining" && s.job_id.is_none() && now().secs_since(s.at) > 15.0).await;
    set_reply(answer(200, Some(12), Some("Unable to join Game 211")));
    questions.store(0, Ordering::SeqCst);
    r.set_check(Some(None), now().plus_secs(-10.0));
    r.tick().await;
    c.check("a join question without an answer isn't asked again at once", q() == 0 && !r.noted(), q());
    r.set_check(Some(None), now().plus_secs(-40.0));
    r.tick().await;
    r.tick().await;
    c.check("…but half a minute on, and the ban in that answer is then noted", q() == 1 && r.noted(), (q(), r.noted()));
    // A refused request (too many requests: an answer without a status) is no answer either.
    r.new_window();
    r.run(game(GPO));
    r.line(now().plus_secs(-20.0), LAUNCH);
    r.poll(|s| s.status == "joining" && s.job_id.is_none() && now().secs_since(s.at) > 15.0).await;
    set_reply(answer(429, None, Some("Too many requests")));
    questions.store(0, Ordering::SeqCst);
    r.tick().await;
    let refused_ask = app.rejoin.join_checks.borrow().get(&me).cloned();
    if let Some(asked) = refused_ask {
        app.rejoin.join_checks.borrow_mut().insert(me, JoinCheck { answer: asked.answer, at: now().plus_secs(-40.0) });
    }
    r.tick().await;
    c.check("a join question Roblox refused (too many requests) is asked again too", q() == 2, q());
    // Roblox refuses the account the game: the window stays, why is said once, and it isn't rejoined once closed.
    c.check(
        "Roblox's join statuses its client counts as errors (12, 29-33) read as refusals; a full game, a wait or a join don't",
        [12, 29, 30, 31, 32, 33].iter().all(|s| answer(200, Some(*s), None).refused()) && [0, 1, 2, 6, 10, 28].iter().all(|s| !answer(200, Some(*s), None).refused()),
        (),
    );
    r.new_window();
    let refused_game = r.run(game(920587237));
    r.line(now().plus_secs(-70.0), LAUNCH);
    r.poll(|s| s.status == "joining" && s.job_id.is_none() && now().secs_since(s.at) > 65.0).await;
    r.set_check(Some(Some(answer(200, Some(29), Some("This experience needs a trusted friend in the server.")))), now());
    r.clear_closing();
    r.told();
    r.tick().await;
    let refusal_said = r.told();
    c.check(
        "a join Roblox refuses (status 29), a minute without a server: not closed to try again, and why is said",
        !r.closing() && refusal_said.as_deref().is_some_and(|s| s.contains("can't get into the game, so Bloxkeep doesn't try again: This experience needs a trusted friend")),
        &refusal_said,
    );
    r.tick().await;
    let once = r.told();
    c.check("…said once", once.is_none() && !r.closing(), &once);
    let refused_end = app.ended_state(me);
    c.check(
        "…and closed by the person, not rejoined",
        !app.queue_rejoin(id, &refused_game, refused_end.as_ref(), End::Unasked) && r.queue_len() == 0,
        r.queue_len(),
    );
    r.clear_queue();
    let refusal_alert = r.alert_title("drop", "{0} can't get into the game, so Bloxkeep doesn't try again: {1}", vec![r.label(), "No.".into()], refusal_said.as_deref().unwrap_or(""));
    c.check("…Discord's alert: \"Roblox won't let it in, not trying again\"", refusal_alert.title == "Roblox won't let it in, not trying again", &refusal_alert.title);
    // In a full game's line (Roblox's answer gives its place): kept; asked again every minute, its place shown meanwhile;
    // once Roblox gives none, tried again as before.
    r.new_window();
    r.run(game(920587237));
    r.line(now().plus_secs(-70.0), LAUNCH);
    r.poll(|s| s.status == "joining" && s.job_id.is_none() && now().secs_since(s.at) > 65.0).await;
    const FULL_GAME: &str = "The game you requested is currently full. Please try again later.";
    let in_line = |place: i32| JoinAnswer { queue_position: Some(place), ..answer(200, Some(6), Some(FULL_GAME)) };
    r.set_check(Some(Some(in_line(3))), now());
    r.clear_closing();
    r.told();
    questions.store(0, Ordering::SeqCst);
    r.tick().await;
    let line_told = r.told();
    c.check("in a full game's line (place 3), a minute without a server: not closed", !r.closing() && q() == 0 && line_told.is_none(), q());
    r.set_check(Some(Some(in_line(3))), now().plus_secs(-61.0));
    set_reply(in_line(2));
    r.tick().await;
    c.check(
        "…asked again a minute on: its new place read, still not closed",
        q() == 1 && !r.closing() && app.join_answer_of(me).and_then(|a| a.queue_position) == Some(2),
        q(),
    );
    r.set_check(None, now());
    c.check("…while it is asked, the place it had is shown", app.join_answer_of(me).and_then(|a| a.queue_position) == Some(3), app.join_answer_of(me));
    r.set_check(Some(Some(in_line(2))), now().plus_secs(-61.0));
    set_reply(answer(200, Some(6), Some(FULL_GAME)));
    r.tick().await;
    r.tick().await;
    c.check("…and once Roblox gives it no place, closed to try again as before", r.closing(), ());
    r.clear_closing();
    // A private server's join is asked as its public game's: that answer says nothing of the private server.
    r.new_window();
    r.run(Some(GameTarget::new(920587237, None, Some("0123456789".into()), None)));
    r.line(now().plus_secs(-70.0), LAUNCH);
    r.poll(|s| s.status == "joining" && s.job_id.is_none() && now().secs_since(s.at) > 65.0).await;
    r.set_check(Some(Some(answer(200, Some(12), Some("Unable to join Game 517")))), now());
    r.told();
    r.tick().await;
    let private_said = r.told();
    c.check(
        "a private server's window isn't held by its public game's refusal: tried again as before",
        r.closing() && private_said.as_deref().is_some_and(|s| s.contains("trying again")),
        &private_said,
    );
    r.clear_closing();
    // Roblox holds the join for its check that the account isn't a bot: the window stays, why is said once, and it isn't
    // rejoined once closed.
    r.new_window();
    let checked_game = r.run(game(920587237));
    questions.store(0, Ordering::SeqCst);
    r.line(now().plus_secs(-70.0), LAUNCH);
    r.line(
        now().plus_secs(-69.0),
        r#"Error [DFLog::HttpTraceError] HttpResponse(#21 0x1dbb2c53ff8) time:320.8ms (net:320.8ms timeInRetryQueue:0.0ms)status:403  bodySize:82 url:{ "https://gamejoin.roblox.com/v1/join-game" } ip:128.116.97.3 external:0 numberOfTimesRetried:0 challengedByGcs"#,
    );
    let challenged = |r: &R| r.app.controller.logs.borrow().join_challenged_at(r.me).is_some();
    let begun = Instant::now();
    while !challenged(r) && begun.elapsed() < Duration::from_secs(15) {
        app.controller.logs.borrow_mut().poll(&[me]);
        rt::sleep(Duration::from_millis(200)).await;
    }
    c.check("Roblox's bot check on the join read from the window's log", challenged(r), ());
    r.clear_closing();
    r.told();
    r.tick().await;
    let check_said = r.told();
    c.check(
        "…a minute without a server: not closed to try again, and why is said",
        !r.closing() && check_said.as_deref().is_some_and(|s| s.contains("Roblox wants a check that it isn't a bot first")),
        &check_said,
    );
    let check_alert = r.alert_title(
        "drop",
        "{0} can't get into the game: Roblox wants a check that it isn't a bot first, so Bloxkeep leaves the window open and doesn't try again.",
        vec![r.label()],
        check_said.as_deref().unwrap_or(""),
    );
    c.check(
        "…Discord's alert says it: \"Roblox wants a bot check, not trying again\", with why",
        check_alert.title == "Roblox wants a bot check, not trying again" && check_alert.text.as_deref().is_some_and(|t| t.contains("isn't a bot")),
        format!("{} / {:?}", check_alert.title, check_alert.text),
    );
    r.tick().await;
    let once = r.told();
    c.check("…said once, and Roblox's join API not asked", once.is_none() && !r.closing() && q() == 0, q());
    c.check("…its card says so (Roblox wants a bot check), not joining", app.phase(me).as_deref() == Some("challenged"), app.phase(me));
    let checked_end = app.ended_state(me);
    c.check(
        "…closed by the person, not rejoined",
        !app.queue_rejoin(id, &checked_game, checked_end.as_ref(), End::Unasked) && r.queue_len() == 0,
        r.queue_len(),
    );
    r.clear_queue();
    // A game started again in that window (Retry) is a new join: the check isn't held over.
    r.line(now(), LAUNCH);
    let begun = Instant::now();
    while challenged(r) && begun.elapsed() < Duration::from_secs(15) {
        app.controller.logs.borrow_mut().poll(&[me]);
        rt::sleep(Duration::from_millis(200)).await;
    }
    c.check("…a game started again in the window: the bot check isn't held over", !challenged(r), ());
    // The person starts a game in a window held as banned: the card no longer says the ban, and the window isn't held as banned.
    r.new_window();
    r.run(game(GPO));
    r.line(now().plus_secs(-20.0), LAUNCH);
    r.poll(|s| s.status == "joining" && s.job_id.is_none() && now().secs_since(s.at) > 15.0).await;
    app.rejoin.game_ban_noted.borrow_mut().insert(me);
    let banned_before = r.card_banned();
    r.line(now(), LAUNCH);
    r.poll(|s| s.status == "joining" && s.person_started).await;
    c.check(
        "the person starts a game in a window held as banned: the card no longer says the ban",
        banned_before == Some(true) && r.card_banned() == Some(false),
        (banned_before, r.card_banned()),
    );
    r.tick().await;
    c.check("…nor is the window held as banned", !r.noted(), ());
    // A ban noted before is asked about before opening: one Roblox would let in now is forgotten; one it still refuses is kept.
    let gpo = GameTarget::new(GPO, None, None, None);
    set_reply(answer(200, Some(2), None));
    r.edit(|a| a.game_bans = vec![GameBan { place_id: GPO, game_name: None, at: now().plus_secs(-86400.0), message: None, until: None }]);
    let _ = app.recheck_game_bans(&[r.acc()], &gpo, &Cancel::new()).await;
    c.check("a ban noted before that Roblox no longer holds: forgotten before opening", r.acc().game_bans.is_empty(), r.acc().game_bans.len());
    set_reply(answer(200, Some(12), Some("Unable to join Game 211")));
    r.edit(|a| a.game_bans = vec![GameBan { place_id: GPO, game_name: None, at: now().plus_secs(-86400.0), message: None, until: None }]);
    let _ = app.recheck_game_bans(&[r.acc()], &gpo, &Cancel::new()).await;
    c.check("…one Roblox still holds: kept", r.acc().game_bans.len() == 1, r.acc().game_bans.len());
    // …and a preset asks before leaving the account out (here Roblox still holds it: nothing opens).
    let open_now = r.take_running();
    app.controller.running.borrow_mut().clear();
    questions.store(0, Ordering::SeqCst);
    let left_out = open(&app, r.acc(), GPO, Some(crate::core::base::new_guid())).await;
    let left_out_said = refusal(&left_out);
    c.check("…a preset asks Roblox before leaving out an account a game banned", q() == 1 && left_out_said.is_some(), (q(), &left_out_said));
    r.put_running(open_now);
    // Play asks Roblox whether each account can play the game, and asks the person first, with why, when Roblox says no.
    const GATED: i64 = 555;
    const GATED_UNIVERSE: i64 = 5550;
    app.controller.games.borrow_mut().insert(GATED, Some(GameInfo { place_id: GATED, universe_id: GATED_UNIVERSE, name: "Gated".into(), creator: "someone".into(), playing: 0, icon_url: None }));
    let real_playability = app.rejoin.ask_playability.borrow().clone();
    let playability_asks = Arc::new(AtomicI32::new(0));
    // 0: needs an age check (26), 1: playable, 2: Roblox couldn't be asked, 3: no answer until called off, 4: restricted (11).
    let roblox_says = Arc::new(AtomicI32::new(0));
    {
        let (asks, says) = (playability_asks.clone(), roblox_says.clone());
        *app.rejoin.ask_playability.borrow_mut() = Some(Arc::new(move |_, universe, ct| {
            asks.fetch_add(1, Ordering::SeqCst);
            if universe != GATED_UNIVERSE {
                return Ok(None);
            }
            match says.load(Ordering::SeqCst) {
                0 => Ok(Some(Playability { playable: false, status: Some(26), text: Some("Roblox's own words".into()) })),
                1 => Ok(Some(Playability { playable: true, status: Some(1), text: None })),
                2 => Err(roblox_api::fail_of(roblox_api::HTTP_REQUEST_EXCEPTION, "offline")),
                3 => {
                    // A request given up throws, as HttpClient's does.
                    while !ct.is_cancelled() {
                        std::thread::sleep(Duration::from_millis(25));
                    }
                    Err(roblox_api::fail_of(roblox_api::TASK_CANCELED_EXCEPTION, "The request was canceled."))
                }
                _ => Ok(Some(Playability { playable: false, status: Some(11), text: None })),
            }
        }));
    }
    let play_open = r.take_running();
    app.controller.running.borrow_mut().clear();
    let bans_before_play = r.acc().game_bans.clone();
    // A ban noted too, so that nothing opens whatever the checks find.
    r.edit(|a| a.game_bans = vec![GameBan { place_id: GATED, game_name: None, at: now(), message: None, until: None }]);
    let played = open(&app, r.acc(), GATED, None).await;
    let play_asks = played.as_ref().ok().and_then(|p| p.asks.clone());
    let play_said = played.as_ref().ok().and_then(|p| p.message.clone());
    c.check(
        "Play asks first when Roblox says the account can't play the game, with why",
        play_asks.as_ref().is_some_and(|a| a.iter().any(|k| k == "playability"))
            && play_said.as_deref().is_some_and(|s| s.contains(&format!("{} can't play this game: it needs an age check.", r.label()))),
        format!("{played:?}"),
    );
    let unplayable = |app: Rc<App>, account: Account| async move { app.unplayable(&[account], GATED, &Cancel::new()).await.map(|v| v.len()).unwrap_or(usize::MAX) };
    roblox_says.store(1, Ordering::SeqCst);
    c.check("…not when Roblox says it can", unplayable(app.clone(), r.acc()).await == 0, ());
    roblox_says.store(2, Ordering::SeqCst);
    c.check("…nor when Roblox couldn't be asked", unplayable(app.clone(), r.acc()).await == 0, ());
    roblox_says.store(3, Ordering::SeqCst);
    let slow = Instant::now();
    let slow_count = unplayable(app.clone(), r.acc()).await;
    c.check(
        "…nor when Roblox takes too long: Play waits 6 s at most",
        slow_count == 0 && slow.elapsed() < Duration::from_secs(9),
        (slow_count, slow.elapsed()),
    );
    let asks_before = playability_asks.load(Ordering::SeqCst);
    let sign_in = r.acc().cookie;
    r.edit(|a| a.cookie = String::new());
    roblox_says.store(4, Ordering::SeqCst);
    c.check(
        "…and an account without a sign-in isn't asked",
        unplayable(app.clone(), r.acc()).await == 0 && playability_asks.load(Ordering::SeqCst) == asks_before,
        (),
    );
    r.edit(|a| a.cookie = sign_in);
    c.check(
        "a reason Bloxkeep has no words for is said in Roblox's own",
        crate::core::app::rejoin::playability_why(&Playability { playable: false, status: Some(25), text: Some("This experience is not available right now.".into()) })
            == "This experience is not available right now",
        (),
    );
    // Roblox's live answer names the status ("GuestProhibited", seen 2026-10-21) where its docs give a number.
    *roblox_api::STAND_IN.write().unwrap_or_else(|e| e.into_inner()) = Some(Box::new(|_| {
        Ok(HttpResponse {
            status: 200,
            headers: vec![("Content-Type".into(), "application/json".into())],
            body: br#"[{"playabilityStatus":"ContextualPlayabilityAgeCheckRequired","isPlayable":false,"universeId":5550,"unplayableDisplayText":"Age check required"}]"#.to_vec(),
        })
    }));
    let parsed = rt::blocking(|| roblox_api::get_playability("test", roblox_api::realm::GLOBAL, GATED_UNIVERSE, &Cancel::new())).await;
    no_network();
    c.check(
        "Roblox's answer read with its status as a name: 26, with Roblox's words",
        matches!(&parsed, Ok(Some(p)) if !p.playable && p.status == Some(26) && p.text.as_deref() == Some("Age check required")),
        &parsed,
    );
    r.edit(|a| a.game_bans = bans_before_play);
    r.put_running(play_open);
    *app.rejoin.ask_playability.borrow_mut() = real_playability;
    app.controller.games.borrow_mut().remove(&GATED);
}

/// Windows that end together right after another opened (Multi-Roblox on): Roblox closing them for the new one, unless
/// they ran out of memory or didn't close in order.
async fn closed_by_roblox(c: &mut Checks, r: &mut R, not_banned: &GameTarget) {
    let app = r.app.clone();
    let me = r.me;
    let was_armed = multi_instance::armed();
    multi_instance::set_armed(true);
    let no_children = |_: i32| -> Vec<i32> { Vec::new() };
    let ended = |who: Guid, pid: i32, at: Stamp, in_order: bool| app.check_closed_by_roblox(who, pid, at.plus_secs(-120.0), at, in_order, &no_children);
    let now0 = now();
    let (first, second) = (crate::core::base::new_guid(), crate::core::base::new_guid());
    let fresh = || {
        let mut starts = app.roblox_updates.client_starts.borrow_mut();
        starts.clear();
        starts.insert(4_000_001, now0.plus_secs(-5.0)); // a new window, 5 s ago
        app.roblox_updates.suspect_ends.borrow_mut().clear();
        app.roblox_updates.closed_by_roblox.borrow_mut().clear();
    };
    let closed_count = || app.roblox_updates.closed_by_roblox.borrow().len();
    fresh();
    app.roblox_updates.memory_short_at.set(None);
    let first_said = ended(first, 4_000_002, now0, true);
    let second_said = ended(second, 4_000_003, now0.plus_secs(1.0), true);
    c.check("two windows ending together right after another opened: still taken for Roblox closing them", !first_said && second_said, (first_said, second_said));
    fresh();
    app.roblox_updates.memory_short_at.set(Some(now0.plus_secs(-8.0)));
    let first_said = ended(first, 4_000_002, now0, true);
    let second_said = ended(second, 4_000_003, now0.plus_secs(1.0), true);
    c.check(
        "…but not when Windows' virtual memory had just run out (they crashed for want of it)",
        !first_said && !second_said && closed_count() == 0,
        (first_said, second_said, closed_count()),
    );
    // Only windows that closed in order can be Roblox's close: a crash or a killed process writes neither.
    fresh();
    app.roblox_updates.memory_short_at.set(None);
    let first_said = ended(first, 4_000_002, now0, false);
    let second_said = ended(second, 4_000_003, now0.plus_secs(1.0), false);
    c.check(
        "two windows that crashed or were killed together right after another opened: never taken for Roblox closing them",
        !first_said && !second_said && closed_count() == 0,
        (first_said, second_said, closed_count()),
    );
    // The window's own log saying Windows refused it memory, Roblox's real lines of 2026-10-22.
    fresh();
    app.roblox_updates.memory_short_at.set(None);
    r.new_window();
    r.run(Some(not_banned.clone()));
    let oom_at = now().plus_secs(-3.0);
    r.line(oom_at, LAUNCH);
    r.line(oom_at, JOIN);
    r.line(oom_at, "Critical [DFLog::Mimalloc] cannot commit OS memory (error: 1455 (0x5AF), address: 0x019D2C5C0000, size: 0x250000 bytes)");
    r.line(
        oom_at,
        "Critical [DFLog::Mimalloc] out-of-memory on OS allocation, try again... (attempt 1, 0x280000 bytes, error code: 0x5AF, address: 0x00000000, alignment: 0x10000, flags: 0x3000)",
    );
    r.line(oom_at.plus_secs(1.0), "[LOGCHANNELS + 1] RBXCRASH: OutOfMemoryGraphics");
    let oom = |r: &R| r.app.controller.logs.borrow().out_of_memory_at(r.me);
    let in_order = |r: &R| r.app.controller.logs.borrow().closed_in_order_at(r.me);
    let begun = Instant::now();
    while oom(r).is_none() && begun.elapsed() < Duration::from_secs(15) {
        app.controller.logs.borrow_mut().poll(&[me]);
        rt::sleep(Duration::from_millis(200)).await;
    }
    c.check("Roblox's own out-of-memory lines are read", oom(r).is_some(), ());
    c.check("…and a crash isn't read as a window closed in order", in_order(r).is_none(), ());
    ended(second, 4_000_003, now0, true);
    let first_said = ended(first, me, now0.plus_secs(1.0), true);
    c.check(
        "…and a window that logged them isn't taken for one Roblox closed",
        !first_said && !app.roblox_updates.closed_by_roblox.borrow().contains(&first),
        first_said,
    );
    r.new_window();
    r.run(Some(not_banned.clone()));
    let closed_at = now().plus_secs(-3.0);
    r.line(closed_at, LAUNCH);
    r.line(closed_at, JOIN);
    r.line(closed_at.plus_secs(1.0), "[FLog::Systray] Successfully launched new process in systray mode");
    r.line(closed_at.plus_secs(1.0), "[FLog::SingleSurfaceApp] shutDown: (stage:UGCGame).");
    let begun = Instant::now();
    while in_order(r).is_none() && begun.elapsed() < Duration::from_secs(15) {
        app.controller.logs.borrow_mut().poll(&[me]);
        rt::sleep(Duration::from_millis(200)).await;
    }
    c.check("a window closed in order is read so from its log (its shutDown and notification-area lines)", in_order(r).is_some(), ());
    multi_instance::set_armed(was_armed);
    app.roblox_updates.memory_short_at.set(None);
    app.roblox_updates.suspect_ends.borrow_mut().clear();
    app.roblox_updates.closed_by_roblox.borrow_mut().clear();
}

/// A window that ran out of memory waits for room to rejoin; a launch's wait between accounts lets other rejoins go.
async fn memory_and_waits(c: &mut Checks, r: &mut R) {
    let app = r.app.clone();
    let id = r.id;
    let memory = |commit_available_mb: i64| MemoryStatus {
        total_mb: 32768,
        available_mb: 16000,
        load_percent: 50,
        commit_limit_mb: 65536,
        commit_available_mb,
        page_file_room_mb: 0,
    };
    {
        r.edit(|a| a.auto_rejoin = true);
        r.clear_queue();
        r.told();
        let oom_launch = r.run(game(920587237));
        app.rejoin.ran_out_of_memory.borrow_mut().insert(id);
        app.queue_rejoin(id, &oom_launch, None, End::Unasked);
        let oom_said = r.told();
        c.check(
            "a window that ran out of memory: said so, not \"left the game\"",
            oom_said.as_deref().is_some_and(|s| s.contains("ran out of memory and Roblox crashed")) && r.queue_len() == 1,
            &oom_said,
        );
        let set_due = |app: &App| {
            if let Some(q) = app.controller.rejoin_queue.borrow_mut().first_mut() {
                q.due = now().plus_secs(-1.0);
            }
        };
        set_due(&app);
        let real_read = app.rejoin.read_memory.borrow().clone();
        // 64 GB of commit limit, 2 GB of it left: one more 3 GB client doesn't fit.
        let memory_now = Rc::new(RefCell::new(memory(2048)));
        {
            let m = memory_now.clone();
            *app.rejoin.read_memory.borrow_mut() = Some(Rc::new(move || m.borrow().clone()));
        }
        app.controller.last_session_start.set(stamp_min());
        let held_bans = r.acc().game_bans.clone();
        app.run_due_rejoin(now());
        let held_said = r.told();
        let due_later = app.controller.rejoin_queue.borrow().first().is_some_and(|q| q.due > now());
        c.check(
            "…its rejoin waits while Windows' virtual memory is still all but used up, and says why",
            r.queue_len() == 1
                && due_later
                && !app.controller.rejoining.get()
                && held_said.as_deref().is_some_and(|s| s.contains("waits to rejoin: Windows' virtual memory is almost full")),
            &held_said,
        );
        set_due(&app);
        app.run_due_rejoin(now());
        let once = r.told();
        c.check("…said once", once.is_none() && r.queue_len() == 1, &once);
        // Room again: it goes ahead. A ban by the game noted, so nothing opens: the rejoin returns at once.
        r.edit(|a| a.game_bans = vec![GameBan { place_id: 920587237, game_name: None, at: now(), message: None, until: None }]);
        *memory_now.borrow_mut() = memory(20480);
        set_due(&app);
        app.run_due_rejoin(now());
        c.check("…once there's room, it goes ahead", r.queue_len() == 0, r.queue_len());
        rt::sleep(Duration::from_millis(100)).await; // the rejoin it started stops at its ban
        let held_alert = r.alert_title(
            "rejoinFail",
            "{0} waits to rejoin: Windows' virtual memory is almost full, so a new Roblox window would crash as it opens. It rejoins once there's room.",
            vec![r.label()],
            held_said.as_deref().unwrap_or(""),
        );
        c.check("…Discord's alert: \"Waiting for memory to rejoin\"", held_alert.title == "Waiting for memory to rejoin", &held_alert.title);
        r.edit(|a| a.game_bans = held_bans);
        *app.rejoin.read_memory.borrow_mut() = real_read;
        r.clear_queue();
    }
    // A launch of several accounts lets rejoins go while it waits between two of them, but not of the accounts it is
    // about to open, and none while it opens one. A rejoin keeps the wait between accounts of the preset that opened it.
    {
        r.edit(|a| a.auto_rejoin = true);
        let held_bans = r.acc().game_bans.clone();
        // A ban by the game noted, so nothing opens: the rejoin returns at once.
        r.edit(|a| a.game_bans = vec![GameBan { place_id: 920587237, game_name: None, at: now(), message: None, until: None }]);
        app.controller.running.borrow_mut().clear();
        r.clear_queue();
        let due_rejoin = |preset: Option<Guid>| RejoinRequest {
            preset_id: preset,
            ..RejoinRequest::new(id, GameTarget::new(920587237, None, None, None), now().plus_secs(-1.0))
        };
        let real_read = app.rejoin.read_memory.borrow().clone();
        *app.rejoin.read_memory.borrow_mut() = Some(Rc::new(move || memory(20480)));
        let preset = LaunchPreset { name: "Slow".into(), account_ids: vec![id], delay_seconds: Some(120), ..Default::default() };
        let preset_id = preset.id;
        let held_delay = app.vault(|d| d.settings.launch_delay_seconds).unwrap_or(0);
        let push = |req: RejoinRequest| app.controller.rejoin_queue.borrow_mut().push(req);
        // Each rejoin started stops at its ban: given a moment so `rejoining` is down before the next.
        let settle = || rt::sleep(Duration::from_millis(100));

        app.controller.last_session_start.set(stamp_min());
        *app.controller.launch_cts.borrow_mut() = Some(Cancel::new());
        push(due_rejoin(None));
        app.run_due_rejoin(now());
        c.check("a launch opening an account: no rejoin goes meanwhile", r.queue_len() == 1, r.queue_len());
        *app.controller.launch_waiting.borrow_mut() = Some(HashSet::from([id]));
        app.run_due_rejoin(now());
        c.check("…waiting between two accounts: not of an account it is about to open", r.queue_len() == 1, r.queue_len());
        *app.controller.launch_waiting.borrow_mut() = Some(HashSet::new());
        app.run_due_rejoin(now());
        c.check("…but of the others, yes", r.queue_len() == 0, r.queue_len());
        settle().await;
        *app.controller.launch_waiting.borrow_mut() = None;
        *app.controller.launch_cts.borrow_mut() = None;

        let _ = app.vault_mut(|d| {
            d.settings.presets.push(preset.clone());
            d.settings.launch_delay_seconds = 8;
        });
        app.controller.last_session_start.set(now().plus_secs(-30.0));
        push(due_rejoin(Some(preset_id)));
        app.run_due_rejoin(now());
        c.check(
            "a rejoin of a preset's window keeps the preset's wait between accounts (2 minutes after the last window)",
            r.queue_len() == 1,
            r.queue_len(),
        );
        r.clear_queue();
        push(due_rejoin(None));
        app.run_due_rejoin(now());
        c.check("…one opened by Play keeps Settings' (8 s)", r.queue_len() == 0, r.queue_len());
        settle().await;
        app.controller.last_session_start.set(now().plus_secs(-121.0));
        push(due_rejoin(Some(preset_id)));
        app.run_due_rejoin(now());
        c.check("…and the preset's goes once its wait is over", r.queue_len() == 0, r.queue_len());
        settle().await;

        *app.controller.launch_waiting.borrow_mut() = None;
        *app.controller.launch_cts.borrow_mut() = None;
        let _ = app.vault_mut(|d| {
            d.settings.presets.retain(|p| p.id != preset_id);
            d.settings.launch_delay_seconds = held_delay;
        });
        *app.rejoin.read_memory.borrow_mut() = real_read;
        r.edit(|a| a.game_bans = held_bans);
        r.clear_queue();

        // The wait itself: rejoins of the others may go during it, and one started meanwhile pushes the next account to
        // the same gap after it, so windows still start one every N seconds.
        app.controller.last_session_start.set(now());
        let took = Instant::now();
        let done = Rc::new(RefCell::new(false));
        {
            let (app, account, done) = (app.clone(), r.acc(), done.clone());
            rt::spawn(async move {
                let _ = app.wait_before_next_launch(&account, now(), 2, vec![crate::core::base::new_guid()], &Cancel::new()).await;
                *done.borrow_mut() = true;
            });
        }
        rt::sleep(Duration::from_millis(300)).await;
        let lets_rejoins_go = app.controller.launch_waiting.borrow().as_ref().is_some_and(|w| w.len() == 1);
        rt::sleep(Duration::from_millis(700)).await;
        app.controller.last_session_start.set(now()); // a rejoin's window started a second in
        while !*done.borrow() && took.elapsed() < Duration::from_secs(30) {
            rt::sleep(Duration::from_millis(20)).await;
        }
        let elapsed = took.elapsed();
        c.check("waiting between two accounts, rejoins of the others may go", lets_rejoins_go, ());
        c.check(
            "…and one started meanwhile pushes the next account to the same gap after it (about 3 s here, not 2)",
            elapsed >= Duration::from_secs_f64(2.8) && elapsed < Duration::from_secs(5),
            elapsed,
        );
        c.check("…and once the wait is over, none go", app.controller.launch_waiting.borrow().is_none(), ());
    }

    close_after_leave(c, r).await;
}

/// "Close Roblox when you leave a game": a window the person left its game in closes, as if there were no Roblox app
/// behind the game; Bloxkeep's own closes, a setting left off and a launch still under way keep it.
async fn close_after_leave(c: &mut Checks, r: &mut R) {
    let app = r.app.clone();
    let (me, id) = (r.me, r.id);
    r.clear_queue();
    r.clear_closing();
    // This process stands in for the client: its "close" is only noted, never done.
    let closed: Arc<Mutex<Vec<i32>>> = Arc::new(Mutex::new(Vec::new()));
    let real_close = app.close_desk.close_hook.borrow().clone();
    {
        let closed = closed.clone();
        *app.close_desk.close_hook.borrow_mut() = Some(Rc::new(move |pid, _| closed.lock().unwrap().push(pid)));
    }
    let pids = || closed.lock().unwrap().clone();
    let leaves = || app.close_desk.closing_account(id).is_some_and(|c| c.cause == Cause::Leave && c.then == Then::StayClosed);
    let shut = || pids().contains(&me) || leaves();

    let _launch = r.run(game(PLACE));
    r.new_window();
    r.line(now(), LAUNCH);
    r.line(now(), JOIN);
    r.line(now(), "[FLog::Network] serverId: 128.116.53.33|51451");
    r.poll(|s| s.status == "ingame").await;
    app.close_after_leave(now());
    c.check("close after leaving: in the game, nothing closes", !shut(), ());

    // Left from the game's menu: Roblox logs the leave, then the server's disconnect, and stays on its home screen.
    r.line(now(), LEAVE);
    r.line(now(), "[FLog::Network] Time to disconnect replication data: 0.1");
    let left = r.poll(|s| s.status == "left").await;
    c.check("…the person leaves the game: the log says left", left.as_ref().is_some_and(|s| s.status == "left" && !s.never_joined), &left);
    let _ = app.vault_mut(|d| d.settings.close_after_leave = false);
    app.close_after_leave(now());
    c.check("…with the setting off, the window stays on Roblox's home screen", !shut(), ());

    let _ = app.vault_mut(|d| d.settings.close_after_leave = true);
    // Closing already for a rejoin (recorded only: this process isn't closed).
    app.expect_close(Some(id), me, Cause::Drop(DropWhy::Dropped), Then::Rejoin);
    app.close_after_leave(now());
    c.check("…a leave Bloxkeep's own close made (for a rejoin) isn't closed again", !shut(), ());
    r.clear_closing();

    app.close_after_leave(now().plus_secs(600.0));
    c.check("…a leave from long before (one already there when Bloxkeep starts) is left alone", !shut(), ());

    app.close_after_leave(now());
    let now_closed = pids();
    c.check("…setting on, the person's own leave: the window closes", now_closed.contains(&me), &now_closed);
    c.check("…closed to stay closed (no rejoin of it)", leaves() && r.queue_len() == 0, ());
    closed.lock().unwrap().clear();
    app.close_after_leave(now());
    let again = pids();
    c.check("…and only once", again.is_empty(), &again);

    let _ = app.vault_mut(|d| d.settings.close_after_leave = false);
    *app.close_desk.close_hook.borrow_mut() = real_close;
    app.close_desk.clear();
}
