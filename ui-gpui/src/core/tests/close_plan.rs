// close_plan.rs: who may close a Roblox client and what follows its end, a check per rule (A1-A6, tick, E1-E4), who
// may open one (O1-O4), and per fix it brings, all made up here (no window, no clock). Then the close desk on an App,
// closing nothing: its hook is the test's, and the "windows" are made-up process ids. No Roblox is started: a launch
// here never gets past its gate, and a sign-in ticket would be refused (no network in the checks).
use std::{cell::RefCell, collections::HashSet, rc::Rc, time::Duration};

use super::{
    Checks,
    rejoin::{no_network, quiet_app, seed},
    unlock,
};
use crate::core::{
    app::{
        App,
        close_desk::{Asked, moving_over},
        controller::PendingLaunch,
    },
    base::{Guid, Stamp, new_guid},
    base::Fail,
    close_plan::{
        Answer, Ask, AskFacts, Cause, Closing, DropWhy, End, EndFacts, Hold, No, NoOpen, Open, OpenFacts, OpenHold, Step, Then, Who, Why, ask, ended, may_open, rank, stale,
        tick,
    },
    l, rt,
    game_target::GameTarget,
    models::Account,
    multi_instance,
    roblox_log_watcher::{ClientState, Tail},
};

fn t(s: f64) -> Stamp {
    Stamp::from_unix_secs(1_800_000_000).plus_secs(s)
}

const PID: i32 = 7;

fn target(job: &str) -> GameTarget {
    GameTarget::new(606, Some(job.into()), None, None)
}

/// A client of a found, signed-in account, alone, in no game, nobody at the PC, long after the last start.
fn facts() -> AskFacts {
    AskFacts {
        now: t(1000.),
        update_reopen: false,
        account_found: true,
        signed_out: false,
        banned: false,
        game_banned: false,
        has_target: true,
        multi_armed: false,
        running: 1,
        in_game: false,
        main: false,
        front_in_use: false,
        rejoin_on: true,
        settling: false,
        busy: false,
        last_start: t(0.),
        reopen_under_way: false,
    }
}

fn asking(cause: Cause, then: Then) -> Ask {
    Ask { pid: PID, account: Some(Guid::nil()), cause, then, force: false, send: true }
}

fn record(cause: Cause, then: Then, asked: f64) -> Closing {
    Closing { pid: PID, account: Some(Guid::nil()), cause, then, asked: t(asked), forced: false, sent: true }
}

fn closes(a: &Answer) -> bool {
    matches!(a, Answer::Close { .. })
}

fn forced(a: &Answer) -> bool {
    matches!(a, Answer::Close { force: true, .. })
}

fn reopen() -> Then {
    Then::Reopen(target("old"))
}

pub async fn run(c: &mut Checks, _dir: &str) {
    one_record(c);
    ranks(c);
    update_holds(c);
    way_back(c);
    waits(c);
    force(c);
    ticks(c);
    ends(c);
    opens(c);
    desk(c).await;
}

fn one_record(c: &mut Checks) {
    let a = new_guid();
    let records = [(PID, Some(a)), (8, Some(a)), (9, Some(new_guid())), (10, None)];
    c.check("A1: another process's record of the same account is stale", stale(Some(a), PID, &records) == [8], stale(Some(a), PID, &records));
    c.check("A1: a window of no account leaves every record", stale(None, PID, &records).is_empty(), ());
}

fn ranks(c: &mut Checks) {
    let f = facts();
    c.check(
        "A2: the ranks, highest first",
        rank(&Cause::User) == 5
            && rank(&Cause::Stop) == 5
            && rank(&Cause::Untracked) == 5
            && rank(&Cause::UpdateReopen) == 4
            && rank(&Cause::Switch) == 3
            && [Cause::Restart(Why::Hours), Cause::Hop, Cause::RejoinNow, Cause::BotCheck, Cause::Macro].iter().all(|c| rank(c) == 2)
            && rank(&Cause::Drop(DropWhy::Dropped)) == 1
            && rank(&Cause::Leave) == 0,
        (),
    );
    let hop = record(Cause::Hop, reopen(), 990.);
    let user = ask(Some(&hop), &asking(Cause::User, Then::StayClosed), &f);
    c.check(
        "A2: the person's close over a server switch wins, and it stays closed (the switch is over)",
        matches!(&user, Answer::Close { record, .. } if record.cause == Cause::User && record.then == Then::StayClosed),
        &user,
    );
    let tie = ask(Some(&hop), &asking(Cause::RejoinNow, reopen()), &f);
    c.check("A2: a tie is refused", tie == Answer::Refuse(No::AlreadyClosing(Cause::Hop)), &tie);
    // Fix 12: a drop's close of a window the person is closing (slow to go) was a second close; refused now.
    let person = record(Cause::User, Then::StayClosed, 990.);
    let drop = ask(Some(&person), &asking(Cause::Drop(DropWhy::Dropped), Then::Rejoin), &f);
    c.check("fix 12: a drop's close of a window the person is closing is refused (fails on the old code)", drop == Answer::Refuse(No::AlreadyClosing(Cause::User)), &drop);
    let again = ask(Some(&person), &asking(Cause::User, Then::StayClosed), &f);
    let forced_again = ask(Some(&person), &Ask { force: true, ..asking(Cause::User, Then::StayClosed) }, &f);
    c.check(
        "A2/A6: the person's Close again does nothing more; Close all forced still ends it at once",
        matches!(again, Answer::Refuse(No::AlreadyClosing(Cause::User))) && forced(&forced_again),
        (&again, &forced_again),
    );
}

fn update_holds(c: &mut Checks) {
    let f = AskFacts { update_reopen: true, ..facts() };
    let below: Vec<Answer> = [Cause::Switch, Cause::Hop, Cause::RejoinNow, Cause::BotCheck, Cause::Macro, Cause::Restart(Why::Hours), Cause::Drop(DropWhy::Dropped)]
        .into_iter()
        .map(|cause| ask(None, &asking(cause, reopen()), &f))
        .collect();
    c.check("A3: while Close, update, reopen runs, every ask below its rank is refused", below.iter().all(|a| *a == Answer::Refuse(No::UpdateReopen)), &below);
    let user = ask(None, &asking(Cause::User, Then::StayClosed), &f);
    c.check("A3: …not the person's Close", closes(&user), &user);
    // Fix 4: a server switch's target was overwritten by whoever wrote last; the update's own now wins, and a switch
    // asked after it is refused.
    let hop = record(Cause::Hop, Then::Reopen(target("hop")), 990.);
    let update = ask(Some(&hop), &asking(Cause::UpdateReopen, Then::Reopen(target("update"))), &facts());
    let hop_after = ask(Some(&record(Cause::UpdateReopen, Then::Reopen(target("update")), 990.)), &asking(Cause::Hop, Then::Reopen(target("hop"))), &f);
    c.check(
        "fix 4: Close, update, reopen outranks a server switch: its target stands, a switch after it is refused (fails on the old code)",
        matches!(&update, Answer::Close { record, .. } if record.then == Then::Reopen(target("update"))) && matches!(hop_after, Answer::Refuse(_)),
        (&update, &hop_after),
    );
}

fn way_back(c: &mut Checks) {
    let f = facts();
    let reopens = [Cause::Restart(Why::Hours), Cause::Hop, Cause::RejoinNow, Cause::BotCheck, Cause::Macro, Cause::UpdateReopen];
    let each = |f: &AskFacts| -> Vec<Answer> { reopens.iter().map(|cause| ask(None, &asking(*cause, reopen()), f)).collect() };
    c.check("A4: a reopen with a way back closes", each(&f).iter().all(closes), each(&f));
    // Fix 9: a server switch, Rejoin now, a bot check and a macro closed without asking whether the window could come
    // back; each now gets the restart's checks.
    let cases = [
        ("account gone", AskFacts { account_found: false, ..f }, No::AccountGone),
        ("signed out", AskFacts { signed_out: true, ..f }, No::SignedOut),
        ("banned on Roblox", AskFacts { banned: true, ..f }, No::Banned),
        ("banned by the game", AskFacts { game_banned: true, ..f }, No::GameBanned),
        ("no target", AskFacts { has_target: false, ..f }, No::NoTarget),
        ("Multi-Roblox not ready with another window", AskFacts { running: 2, ..f }, No::MultiNotReady),
    ];
    for (what, f, no) in cases {
        // Close, update, reopen closes every window, so other windows open now never keep one of them closed.
        let got: Vec<Answer> = each(&f).into_iter().zip(reopens).filter(|(_, cause)| !(no == No::MultiNotReady && *cause == Cause::UpdateReopen)).map(|(a, _)| a).collect();
        c.check(&format!("A4, fix 9: {what}: every reopen refused, server switch and the rest included (fails on the old code)"), got.iter().all(|a| *a == Answer::Refuse(no)), got);
    }
    let update = ask(None, &asking(Cause::UpdateReopen, reopen()), &AskFacts { running: 3, multi_armed: false, ..f });
    c.check("A4: Close, update, reopen reopens every window, Multi-Roblox ready or not (it closes them all first)", closes(&update), &update);
    let armed = each(&AskFacts { running: 3, multi_armed: true, ..f });
    c.check("A4: Multi-Roblox ready: other windows open don't matter", armed.iter().all(closes), armed);
    // Fix 2: a restart never asked about a game ban and closed a window auto-rejoin then left closed.
    let banned = ask(None, &asking(Cause::Restart(Why::Hours), reopen()), &AskFacts { game_banned: true, ..f });
    c.check("fix 2: a restart of a window the game banned: left running (fails on the old code)", banned == Answer::Refuse(No::GameBanned), &banned);
    let rejoin = |f: AskFacts| ask(None, &asking(Cause::Drop(DropWhy::Dropped), Then::Rejoin), &f);
    c.check(
        "A4: a drop's rejoin needs the account, no ban and a target; its sign-in and Multi-Roblox are auto-rejoin's to check",
        closes(&rejoin(AskFacts { signed_out: true, running: 3, ..f }))
            && rejoin(AskFacts { banned: true, ..f }) == Answer::Refuse(No::Banned)
            && rejoin(AskFacts { game_banned: true, ..f }) == Answer::Refuse(No::GameBanned)
            && rejoin(AskFacts { has_target: false, ..f }) == Answer::Refuse(No::NoTarget),
        (),
    );
    let stays = ask(None, &asking(Cause::User, Then::StayClosed), &AskFacts { account_found: false, banned: true, has_target: false, ..f });
    c.check("A4: a close that stays closed needs no way back", closes(&stays), &stays);
}

fn waits(c: &mut Checks) {
    let f = facts();
    let restart = |why: Why, f: AskFacts| ask(None, &asking(Cause::Restart(why), reopen()), &f);
    c.check("A5: a restart waits while its window is in front and in use", restart(Why::Hours, AskFacts { front_in_use: true, ..f }) == Answer::Wait(Hold::InUse), ());
    c.check("A5: …a heavy one too", restart(Why::Heavy, AskFacts { front_in_use: true, ..f }) == Answer::Wait(Hold::InUse), ());
    c.check("A5: …not a frozen one", closes(&restart(Why::Hung, AskFacts { front_in_use: true, ..f })), ());
    c.check(
        "A5: a heavy or frozen main account's window is never restarted; an hours one is",
        restart(Why::Heavy, AskFacts { main: true, ..f }) == Answer::Refuse(No::Main)
            && restart(Why::Hung, AskFacts { main: true, ..f }) == Answer::Refuse(No::Main)
            && closes(&restart(Why::Hours, AskFacts { main: true, ..f })),
        (),
    );
    c.check("A5: a frozen window reopens only where auto-rejoin is on", restart(Why::Hung, AskFacts { rejoin_on: false, ..f }) == Answer::Refuse(No::RejoinOff), ());
    c.check(
        "A5: restarts one at a time, spaced after the last start",
        restart(Why::Hours, AskFacts { reopen_under_way: true, ..f }) == Answer::Wait(Hold::OneAtATime)
            && restart(Why::Hours, AskFacts { last_start: t(990.), ..f }) == Answer::Wait(Hold::Spacing),
        (),
    );
    c.check(
        "A5: restarts and drops wait while the PC settles; a leave's close (as before) and a server switch (the person's) don't",
        restart(Why::Hours, AskFacts { settling: true, ..f }) == Answer::Wait(Hold::Settling)
            && ask(None, &asking(Cause::Drop(DropWhy::OutOfGame), Then::Rejoin), &AskFacts { settling: true, ..f }) == Answer::Wait(Hold::Settling)
            && closes(&ask(None, &asking(Cause::Leave, Then::StayClosed), &AskFacts { settling: true, ..f }))
            && closes(&ask(None, &asking(Cause::Hop, reopen()), &AskFacts { settling: true, ..f })),
        (),
    );
    c.check(
        "A5: a restart and a leave wait while a launch or rejoin of the account is under way",
        restart(Why::Hours, AskFacts { busy: true, ..f }) == Answer::Wait(Hold::Busy)
            && ask(None, &asking(Cause::Leave, Then::StayClosed), &AskFacts { busy: true, ..f }) == Answer::Wait(Hold::Busy),
        (),
    );
}

fn force(c: &mut Checks) {
    let f = facts();
    let in_game = AskFacts { in_game: true, ..f };
    c.check("A6: a window in a game is ended at once (it would ask \"leave the game?\")", forced(&ask(None, &asking(Cause::User, Then::StayClosed), &in_game)), ());
    c.check("A6: …not a drop's, on Roblox's error screen", !forced(&ask(None, &asking(Cause::Drop(DropWhy::NeverGotIn), Then::Rejoin), &in_game)), ());
    c.check("A6: a frozen window always", forced(&ask(None, &asking(Cause::Restart(Why::Hung), reopen()), &f)), ());
    c.check("A6: asked so (Close all forced, a party's server move)", forced(&ask(None, &Ask { force: true, ..asking(Cause::Switch, Then::StayClosed) }, &f)), ());
    c.check("A6: otherwise its X", !forced(&ask(None, &asking(Cause::User, Then::StayClosed), &f)), ());
    let unsent = ask(None, &Ask { send: false, ..asking(Cause::Switch, Then::StayClosed) }, &in_game);
    c.check("a close Roblox makes itself is only recorded, never taken as forced", matches!(&unsent, Answer::Close { record, .. } if !record.sent && !record.forced), &unsent);
}

fn ticks(c: &mut Checks) {
    let r = record(Cause::Restart(Why::Hours), reopen(), 0.);
    c.check("tick: a close asked a moment ago waits", tick(&r, t(10.), true) == Step::Wait, ());
    c.check("tick: a reopen is never forced by the tick (the close ends a silent window itself)", tick(&r, t(30.), true) == Step::Wait, ());
    // Fix 1: a restart that gave up dropped its target but kept "closing for a rejoin" on the process, so its later end
    // read as Bloxkeep's own close with no restart: a counted drop. The whole record goes now.
    c.check("fix 1: still there after 60 s: the whole record goes (fails on the old code)", tick(&r, t(60.), true) == Step::GiveUp, ());
    // The person's Close, a schedule's stop, a drop's and a leave's close had no time limit: one given up came back
    // as a drop to rejoin, or was closed again every minute.
    for (cause, then) in [
        (Cause::User, Then::StayClosed),
        (Cause::Stop, Then::StayClosed),
        (Cause::Leave, Then::StayClosed),
        (Cause::Drop(DropWhy::Dropped), Then::Rejoin),
    ] {
        let kept = record(cause, then, 0.);
        c.check(&format!("fix 14: a {} close is kept until its window ends, never given up", cause.word()), tick(&kept, t(600.), true) == Step::Wait && tick(&kept, t(-600.), true) == Step::Wait, ());
    }
    // A close asked while a tick ran is stamped after that tick's `now`: in a real run every such close (one asked as
    // a window came up, a rejoin's) was given up the same second and its end read as nobody's.
    c.check("fix 11: a close asked after the tick's now waits (fails on the old code)", tick(&r, t(-1.), true) == Step::Wait, ());
    c.check("fix 11: …a few seconds after too", tick(&r, t(-15.), true) == Step::Wait, ());
    c.check("tick: the clock set back past what was mended: given up", tick(&r, t(-61.), true) == Step::GiveUp, ());
    c.check("fix 11: a switch stamped after the tick's now is still on its way", !moving_over(t(0.), t(2.)), ());
    c.check("…over after its time", moving_over(t(181.), t(0.)), ());
    c.check("…and when the clock was set back far", moving_over(t(0.), t(181.)), ());
    c.check("tick: the process gone and nobody took the record: it goes quietly", tick(&r, t(5.), false) == Step::Gone, ());
    let unsent = Closing { sent: false, ..r };
    c.check("tick: a close Roblox makes itself is left to whoever waits for it", tick(&unsent, t(500.), true) == Step::Wait, ());
}

fn ends(c: &mut Checks) {
    let none = EndFacts::default();
    let banned = EndFacts { banned_by_game: true };
    let stayed = ended(Some(&record(Cause::User, Then::StayClosed, 0.)), &none);
    c.check(
        "E1: closed to stay closed: no rejoin, no \"ended on its own\", no hand-over",
        matches!(&stayed, End::Closed { cause: Cause::User, why } if *why == crate::core::close_plan::ON_PURPOSE) && stayed.asked() && !stayed.own(),
        &stayed,
    );
    let back = ended(Some(&record(Cause::Restart(Why::Heavy), Then::Reopen(target("here")), 0.)), &none);
    c.check("E2: closed to reopen: back into its target", back == End::Reopen(target("here")) && back.own(), &back);
    let ban = ended(Some(&record(Cause::Hop, Then::Reopen(target("here")), 0.)), &banned);
    c.check("E2: …unless its last log says the game banned it", matches!(&ban, End::Closed { cause: Cause::Hop, why } if *why == crate::core::close_plan::GAME_BANNED), &ban);
    let drop = ended(Some(&record(Cause::Drop(DropWhy::Dropped), Then::Rejoin, 0.)), &none);
    c.check("E3: closed for a rejoin: auto-rejoin's drop path", drop == End::Rejoin && drop.own(), &drop);
    let left = ended(Some(&record(Cause::Leave, Then::StayClosed, 0.)), &none);
    c.check("E1: close after leaving: closed for good, and not Bloxkeep's own (its log's leave stays a leave)", matches!(left, End::Closed { .. }) && !left.own() && !Cause::Leave.own(), &left);
    let nobody = ended(None, &banned);
    c.check("E4: nobody closed it: today's path", nobody == End::Unasked && !nobody.asked() && !nobody.own(), &nobody);
}

fn opens(c: &mut Checks) {
    let none = OpenFacts { who: Who::Person, updating: false, held_since: None, asked_at: t(100.), sweeping: false, settling: false, closing: false };
    let auto = OpenFacts { who: Who::Auto, ..none };
    c.check("O4: nothing in the way: it opens", may_open(&none) == Open::Go && may_open(&auto) == Open::Go, ());
    // Fix 5: a person's launch during Close, update, reopen opened a window the update then waited on, and gave up.
    c.check(
        "O1, fix 5: Roblox updating: the person's open is refused with a word, an automatic one waits (fails on the old code)",
        may_open(&OpenFacts { updating: true, ..none }) == Open::Refuse(NoOpen::Updating) && may_open(&OpenFacts { updating: true, ..auto }) == Open::Wait(OpenHold::Updating),
        (),
    );
    // Fix 10: Close while the launch fetched the account's ticket found nothing to close, and the window opened anyway.
    c.check(
        "O2, fix 10: closed by the person after the open was asked: refused, silently (fails on the old code)",
        may_open(&OpenFacts { held_since: Some(t(101.)), ..none }) == Open::Refuse(NoOpen::ClosedSince)
            && may_open(&OpenFacts { held_since: Some(t(101.)), ..auto }) == Open::Refuse(NoOpen::ClosedSince)
            && App::open_refused_text(NoOpen::ClosedSince).is_none(),
        (),
    );
    c.check(
        "O2: a close before the open was asked holds nothing",
        may_open(&OpenFacts { held_since: Some(t(99.)), ..none }) == Open::Go && may_open(&OpenFacts { held_since: Some(t(100.)), ..none }) == Open::Go,
        (),
    );
    c.check("O1 before O2: updating, the person is told so", may_open(&OpenFacts { updating: true, held_since: Some(t(101.)), ..none }) == Open::Refuse(NoOpen::Updating), ());
    c.check(
        "O3: an automatic open waits while Roblox's copies are swept, the PC settles, or a window of the account still closes",
        may_open(&OpenFacts { sweeping: true, ..auto }) == Open::Wait(OpenHold::Sweeping)
            && may_open(&OpenFacts { settling: true, ..auto }) == Open::Wait(OpenHold::Settling)
            && may_open(&OpenFacts { closing: true, ..auto }) == Open::Wait(OpenHold::Closing),
        (),
    );
    c.check("O3: …the person's own launch makes its own waits", may_open(&OpenFacts { sweeping: true, settling: true, closing: true, ..none }) == Open::Go, ());
}

/// A client of the account as Bloxkeep's whose log says its player left the game by choice at `at`.
fn left_window(app: &App, account: Guid, pid: i32, at: Stamp) {
    let launch = Rc::new(PendingLaunch::new(account, HashSet::new(), at.plus_secs(-600.0), Some(target("old")), false, None));
    app.controller.running.borrow_mut().insert(account, (pid, launch));
    let mut tail = Tail::new(format!("stand-in-{pid}.log"), pid);
    tail.state = Some(ClientState::new("left", Some(606), Some("old".into()), None, at));
    app.controller.logs.borrow_mut().tails.insert(pid, tail);
}

/// A client of the account as Bloxkeep's, in its game.
fn window(app: &App, account: Guid, pid: i32) {
    let launch = Rc::new(PendingLaunch::new(account, HashSet::new(), Stamp::utc_now().plus_secs(-600.0), Some(target("old")), false, None));
    app.controller.running.borrow_mut().insert(account, (pid, launch));
    let mut tail = Tail::new(format!("stand-in-{pid}.log"), pid);
    tail.state = Some(ClientState::new("ingame", Some(606), Some("old".into()), None, Stamp::utc_now().plus_secs(-300.0)));
    app.controller.logs.borrow_mut().tails.insert(pid, tail);
}

async fn desk(c: &mut Checks) {
    no_network();
    let made = |name: &str| Account { username: name.into(), cookie: format!("cookie-{name}"), cookie_valid: Some(true), ..Default::default() };
    let (one, two) = (made("one"), made("two"));
    let (a, b) = (one.id, two.id);
    seed(vec![one, two]);
    let (app, _said) = quiet_app();
    unlock(&app).await;
    app.controller.running.borrow_mut().clear();
    let closed: Rc<RefCell<Vec<(i32, bool)>>> = Rc::new(RefCell::new(Vec::new()));
    let real_close = app.close_desk.close_hook.borrow().clone();
    {
        let closed = closed.clone();
        *app.close_desk.close_hook.borrow_mut() = Some(Rc::new(move |pid, force| closed.borrow_mut().push((pid, force))));
    }
    let was_armed = multi_instance::armed();
    multi_instance::set_armed(true);
    // Odd numbers: never a real process id.
    window(&app, a, 900_001);
    window(&app, b, 900_003);
    let alive = HashSet::from([900_001, 900_003]);

    // Fix 1, on the desk: given up at 60 s, and its end is then nobody's close.
    let asked = app.ask_close(Some(a), 900_001, Cause::Hop, Then::Reopen(target("new")), false);
    c.check("desk: a server switch with a way back closes (in its game: at once)", asked == Asked::Closing && closed.borrow().as_slice() == [(900_001, true)], closed.borrow().clone());
    closed.borrow_mut().clear();
    if let Some(r) = app.close_desk.closing.borrow_mut().get_mut(&900_001) {
        r.forced = false; // as one asked gently
    }
    // Fix 11: the tick read its clock before this close was asked (as a close asked while the tick ran).
    app.close_desk_tick(Stamp::utc_now().plus_secs(-1.0), &alive);
    c.check(
        "fix 11, desk: a tick whose now came before the ask keeps the record and the switch (fails on the old code)",
        app.close_desk.closing_of(900_001).is_some() && app.close_desk.moving_of(a) && closed.borrow().is_empty(),
        closed.borrow().clone(),
    );
    app.close_desk_tick(Stamp::utc_now().plus_secs(30.0), &alive);
    c.check("desk: still there after 30 s: kept, nothing sent again", app.close_desk.closing_of(900_001).is_some() && closed.borrow().is_empty(), closed.borrow().clone());
    app.close_desk_tick(Stamp::utc_now().plus_secs(61.0), &alive);
    let end = app.close_ended(a, 900_001, None);
    c.check(
        "fix 1, desk: given up after 60 s: no record, no switch, and its later end is an ordinary one (fails on the old code)",
        app.close_desk.closing_of(900_001).is_none() && !app.close_desk.moving_of(a) && end == End::Unasked,
        &end,
    );

    // Fix 3: a server switch ended only by its clock; the person's Close and a refused reopen end it now.
    closed.borrow_mut().clear();
    app.ask_close(Some(a), 900_001, Cause::Hop, Then::Reopen(target("new")), false);
    let moving = app.close_desk.moving_of(a);
    let _ = app.close_accounts(&[a], false);
    c.check(
        "fix 3: Close of an account on its way to another server ends its switch, and the window stays closed (fails on the old code)",
        moving && !app.close_desk.moving_of(a) && app.close_desk.closing_of(900_001).is_some_and(|r| r.cause == Cause::User && r.then == Then::StayClosed),
        app.close_desk.closing_of(900_001),
    );
    // Fix 12: a second Close of a window already closing asks nothing new; the person is still told it is closing.
    closed.borrow_mut().clear();
    let again = app.close_accounts(&[a], false);
    c.check(
        "fix 12: a second Close of a window already closing sends no close and still counts it as closing",
        again == serde_json::json!(1) && closed.borrow().is_empty() && app.close_desk.closing_of(900_001).is_some_and(|r| r.cause == Cause::User),
        (&again, closed.borrow().clone()),
    );
    app.close_desk.clear();
    app.ask_close(Some(b), 900_003, Cause::RejoinNow, Then::Reopen(target("old")), false);
    let mut gone = ClientState::new("disconnected", Some(606), None, Some(267), Stamp::utc_now());
    gone.banned_by_game = true;
    let refused = app.close_ended(b, 900_003, Some(&gone));
    c.check(
        "fix 3: a reopen its last log rules out (banned by the game): not reopened, the switch is over (fails on the old code)",
        matches!(refused, End::Closed { .. }) && !app.close_desk.moving_of(b) && !app.hopping(),
        &refused,
    );

    // Fix 12, on the desk: a rejoin's close of a window the person is closing.
    app.close_desk.clear();
    closed.borrow_mut().clear();
    app.ask_close(Some(a), 900_001, Cause::User, Then::StayClosed, false);
    let second = app.ask_close(Some(a), 900_001, Cause::Drop(DropWhy::Dropped), Then::Rejoin, false);
    c.check(
        "fix 12, desk: a rejoin's close of a window the person is closing: refused, closed once (fails on the old code)",
        second == Asked::Refused(No::AlreadyClosing(Cause::User)) && closed.borrow().len() == 1 && app.close_desk.closing_of(900_001).is_some_and(|r| r.cause == Cause::User),
        (&second, closed.borrow().clone()),
    );

    // Fix 4 and A3, on the desk: Close, update, reopen holds the rest and keeps its own target.
    app.close_desk.clear();
    app.ask_close(Some(b), 900_003, Cause::Hop, Then::Reopen(target("hop")), false);
    app.roblox_updates.update_reopen_since.set(Some(Stamp::utc_now()));
    let update = app.ask_close(Some(b), 900_003, Cause::UpdateReopen, Then::Reopen(target("update")), false);
    let hop = app.ask_close(Some(a), 900_001, Cause::Hop, Then::Reopen(target("hop")), false);
    app.roblox_updates.update_reopen_since.set(None);
    c.check(
        "fix 4, desk: the update's target replaces a switch's; a switch while it runs is refused (fails on the old code)",
        update == Asked::Closing && app.close_desk.reopen_of(b) == Some(target("update")) && hop == Asked::Refused(No::UpdateReopen) && app.close_desk.closing_of(900_001).is_none(),
        (&update, &hop),
    );

    // Fix 9, on the desk: Multi-Roblox not ready with another window open: Rejoin now closes nothing.
    app.close_desk.clear();
    closed.borrow_mut().clear();
    multi_instance::set_armed(false);
    let now_rejoin = app.rejoin_now(a);
    multi_instance::set_armed(true);
    c.check(
        "fix 9, desk: Rejoin now without Multi-Roblox ready, another window open: nothing closed, nothing moving (fails on the old code)",
        !now_rejoin && closed.borrow().is_empty() && !app.close_desk.moving_of(a),
        closed.borrow().clone(),
    );

    // A1, on the desk: an older window's record of the account goes when another window of it is asked.
    app.close_desk.clear();
    app.expect_close(Some(a), 900_005, Cause::Switch, Then::StayClosed);
    app.ask_close(Some(a), 900_001, Cause::User, Then::StayClosed, false);
    c.check(
        "A1, desk: one record per account's client",
        app.close_desk.closing_of(900_005).is_none() && app.close_desk.closing_of(900_001).is_some(),
        app.close_desk.closing.borrow().len(),
    );
    let quiet = app.close_desk.clear();
    c.check("…and a client nobody follows any more goes from the desk once its process is gone", quiet == 1, quiet);

    // Fix 3: the account opened again (start_session) drops its closes, but a server switch lasts until the new window
    // is in its game; deleting the account ends both.
    app.close_desk.moving.borrow_mut().insert(a, Stamp::utc_now());
    app.expect_close(Some(a), 900_006, Cause::Switch, Then::StayClosed);
    app.close_desk.forget_closes(a);
    let kept = app.close_desk.moving_of(a) && app.close_desk.closing_of(900_006).is_none();
    // As the old "closed by the person" mark: only that goes; its earlier window still closing for a restart keeps its
    // record, so that window's end still reads as a restart, not as nobody's close (a drop to rejoin).
    app.close_desk.closing.borrow_mut().insert(900_007, Closing { pid: 900_007, account: Some(a), ..record(Cause::Restart(Why::Hours), reopen(), 0.) });
    app.close_desk.forget_closes(a);
    let restart_kept = app.close_desk.closing_of(900_007).is_some();
    app.close_desk.forget_account(a);
    c.check(
        "fix 3: opened again, its server switch still lasts and a restart's close stays (fix 15, fails on the old code); deleted, all ends",
        kept && restart_kept && !app.close_desk.moving_of(a) && app.close_desk.closing_of(900_007).is_none(),
        (kept, restart_kept),
    );

    stage_b(c, &app, a, b, &closed).await;

    app.controller.running.borrow_mut().clear();
    *app.close_desk.close_hook.borrow_mut() = real_close;
    multi_instance::set_armed(was_armed);
    app.close_desk.clear();
}

/// The open gate, close after leaving and the automatic update's bookkeeping on the App; locks it last.
async fn stage_b(c: &mut Checks, app: &Rc<App>, a: Guid, b: Guid, closed: &Rc<RefCell<Vec<(i32, bool)>>>) {
    app.close_desk.clear();
    app.controller.running.borrow_mut().clear();
    closed.borrow_mut().clear();
    let _ = app.vault_mut(|d| d.settings.multi_instance = false); // nothing of Multi-Roblox's is touched on this PC
    let vault = || app.controller.vault.borrow().clone().expect("unlocked");
    let account = |id: Guid| app.find_account(id).expect("seeded");

    // Fix 5, on the App: Play during Close, update, reopen is refused with a word, nothing opens.
    app.roblox_updates.update_reopen_since.set(Some(Stamp::utc_now()));
    let refused = app.launch_accounts(vault(), vec![account(a)], None, true, 0, None, None, None, None).await;
    let auto = app.may_open(Who::Auto, Some(a), Stamp::utc_now());
    app.roblox_updates.update_reopen_since.set(None);
    app.controller.roblox_updating.set(true);
    let updater = app.may_open(Who::Person, None, Stamp::utc_now());
    let updater_auto = app.may_open(Who::Auto, Some(a), Stamp::utc_now());
    app.controller.roblox_updating.set(false);
    c.check(
        "fix 5, App: Play while Close, update, reopen runs: refused, \"Roblox is updating\"; a rejoin waits; Roblox's updater alone: the person's launch goes on (start_session waits for it), an automatic open waits (fails on the old code)",
        matches!(&refused, Err(Fail::User(text)) if *text == l::t("Roblox is updating. Try again in a moment."))
            && auto == Open::Wait(OpenHold::Updating)
            && updater == Open::Go
            && updater_auto == Open::Wait(OpenHold::Updating),
        format!("{:?}", refused.as_ref().map(|r| r.launched)),
    );

    // Fix 10, on the App: Close of an account the launch hasn't reached yet (it waits for Roblox to finish closing):
    // the launch leaves it closed, and never gets to its sign-in ticket.
    app.controller.sweep_background_until.set(Stamp::utc_now().plus_secs(1.5));
    let (launch, _) = futures::join!(app.launch_accounts(vault(), vec![account(a)], None, true, 0, None, None, None, None), async {
        rt::sleep(Duration::from_millis(300)).await;
        let _ = app.close_accounts(&[a], false);
    });
    c.check(
        "fix 10, App: closed while the launch waited: it skips the account, no window, no error (fails on the old code)",
        matches!(&launch, Ok(r) if r.launched == 0 && r.errors.as_ref().is_none_or(|e| e.is_empty())) && app.controller.pending.borrow().is_empty(),
        format!("{:?}", launch.as_ref().map(|r| (r.launched, r.errors.clone()))),
    );
    let held = app.may_open(Who::Person, Some(a), Stamp::utc_now().plus_secs(-60.0));
    app.close_desk.release_closed([a]);
    let freed = app.may_open(Who::Person, Some(a), Stamp::utc_now().plus_secs(-60.0));
    c.check(
        "O2, App: a Close with no window yet holds the account against an earlier launch; the person's next launch lets it go",
        held == Open::Refuse(NoOpen::ClosedSince) && freed == Open::Go,
        (held, freed),
    );

    // O3, on the App: a rejoin of an account whose window still closes waits.
    window(app, b, 900_011);
    app.ask_close(Some(b), 900_011, Cause::User, Then::StayClosed, false);
    let waits = app.may_open(Who::Auto, Some(b), Stamp::utc_now());
    app.close_desk.clear();
    app.controller.running.borrow_mut().clear();
    c.check("O3, App: an automatic open of an account still closing waits", waits == Open::Wait(OpenHold::Closing), waits);

    // Fix 8: close after leaving is the desk's Leave record, which leaves with its process: a new window under the same
    // process id is closed after its own leave.
    let _ = app.vault_mut(|d| d.settings.close_after_leave = true);
    closed.borrow_mut().clear();
    let at = Stamp::utc_now();
    left_window(app, a, 900_013, at);
    app.close_after_leave(at);
    app.close_after_leave(at);
    let first = closed.borrow().clone();
    let record = app.close_desk.closing_of(900_013);
    let end = app.close_ended(a, 900_013, None);
    left_window(app, a, 900_013, at);
    app.close_after_leave(at);
    let again = closed.borrow().clone();
    c.check(
        "fix 8: left its game: closed once while it goes (a Leave record), to stay closed; a new window under the reused pid is closed after its own leave (fails on the old code)",
        first == [(900_013, false)]
            && record.is_some_and(|r| r.cause == Cause::Leave && r.then == Then::StayClosed)
            && matches!(end, End::Closed { cause: Cause::Leave, .. })
            && again == [(900_013, false), (900_013, false)],
        (first, again),
    );
    // As before: closed once per window. The close stays until the window ends (it is ended within 2 s if it doesn't
    // answer its X), so nothing asks it again.
    app.close_desk.clear();
    closed.borrow_mut().clear();
    left_window(app, a, 900_015, at);
    for k in 0..6 {
        app.close_after_leave(at.plus_secs(61.0 * k as f64));
        if let Some(r) = app.close_desk.closing.borrow_mut().get_mut(&900_015) {
            r.asked = r.asked.plus_secs(-61.0);
        }
        app.close_desk_tick(Stamp::utc_now(), &HashSet::from([900_015]));
    }
    let asks = closed.borrow().iter().filter(|(_, force)| !force).count();
    let kept = app.close_desk.closing_of(900_015).is_some_and(|r| r.cause == Cause::Leave);
    c.check("close after leaving: a window that won't go is asked once, and its close is kept, not given up", asks == 1 && kept, closed.borrow().clone());
    let _ = app.vault_mut(|d| d.settings.close_after_leave = false);
    app.close_desk.clear();
    app.controller.running.borrow_mut().clear();

    // Fix 11: the automatic update is done for a build only once it started; a failed start is tried again after the
    // version check's spacing, not every tick, nor never.
    let now = Stamp::utc_now();
    let latest = Some("version-test".to_string());
    app.note_auto_update(latest.clone(), None, now);
    let failed = (app.roblox_updates.auto_updated_for.borrow().clone(), app.auto_update_due(&latest, now.plus_secs(60.0)), app.auto_update_due(&latest, now.plus_secs(31.0 * 60.0)));
    app.note_auto_update(latest.clone(), Some(false), now);
    let turned_down = app.auto_update_due(&latest, now.plus_secs(31.0 * 60.0));
    app.note_auto_update(latest.clone(), Some(true), now);
    let done = (app.roblox_updates.auto_updated_for.borrow().clone(), app.auto_update_due(&latest, now.plus_secs(31.0 * 60.0)));
    c.check(
        "fix 11: a failed automatic update isn't marked done: tried again after 30 min, not within; done only once it started (fails on the old code)",
        failed == (None, false, true) && turned_down && done == (latest.clone(), false),
        (failed, done),
    );
    *app.roblox_updates.auto_updated_for.borrow_mut() = None;
    app.roblox_updates.auto_update_failed_at.set(None);

    // Fix 6: lock during Close, update, reopen ends its wait; the windows it asked to close still close, and with no vault
    // their reopen finds no account.
    window(app, b, 900_017);
    app.ask_close(Some(b), 900_017, Cause::UpdateReopen, Then::Reopen(target("old")), false);
    app.roblox_updates.update_reopen_since.set(Some(Stamp::utc_now()));
    app.roblox_updates.update_reopen_updating.set(true);
    app.close_desk.hold_closed(a);
    let locked = app.lock();
    let kept = app.close_desk.closing_of(900_017).is_some();
    let run = app.controller.running.borrow().get(&b).cloned();
    let end = app.close_ended(b, 900_017, None);
    let reopened = run.is_some_and(|(_, launch)| app.queue_rejoin(b, &launch, None, end.clone()));
    c.check(
        "fix 6: locked during Close, update, reopen: its wait ends, its closes stay, and nothing reopens without the vault (fails on the old code)",
        locked.is_ok()
            && app.roblox_updates.update_reopen_since.get().is_none()
            && !app.roblox_updates.update_reopen_updating.get()
            && kept
            && matches!(end, End::Reopen(_))
            && !reopened
            && app.controller.rejoin_queue.borrow().is_empty()
            && app.close_desk.held_closed.borrow().is_empty(),
        (kept, reopened),
    );
}
