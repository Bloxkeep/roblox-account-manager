// The window's frame, "Tiết chế": the icon rail on the left, the search and the main action on top, the place in a
// panel, the accounts in a column beside it, and the line of Roblox's figures at the bottom. It holds what the app
// sent (state, metrics) and the line to it; the places draw from that.
use std::time::Duration;

use futures::{StreamExt, channel::mpsc};
use gpui_kit::component::dialog::DialogEntrance;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::WindowExt as _;
use gpui_kit::{prelude::*, *};
use serde_json::{Value, json};
use crate::ask::answer;

mod capsule;

use crate::{
    i18n::I18n,
    images,
    kit::*,
    link::{Event, Link},
    pages::{self, Page},
    theme::*,
};

actions!(bloxkeep, [FocusSearch, FocusNext, FocusPrev, GoBack, GoForward, FindHere, FocusPlace, PickAll, DeletePicked, EditPicked, PlayPicked, ClearPicked]);
// The arrows, Home, End and Space among the Accounts page's cards (app.js's moveCardFocus), never while typing.
actions!(bloxkeep, [CardLeft, CardRight, CardUp, CardDown, CardFirst, CardLast, CardPick]);

/// Home's games card: this PC's games, or what Bloxkeep's users play.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum HomeTab {
    Games,
    Community,
}

pub struct Root {
    pub link: Option<Link>,
    pub i18n: I18n,
    /// The faces of the core's small windows (mini window, dialogs, macro panels, overlays): toolpage.rs.
    pub tool_pages: crate::toolpage::Pages,
    /// The app's whole state (AppController.BuildState), as it sent it last.
    pub state: Value,
    /// Roblox's and the PC's figures, every 1.5 s.
    pub metrics: Value,
    pub page: Page,
    /// The pages left by Back and by Forward, for the top bar's arrows.
    back: Vec<Page>,
    fwd: Vec<Page>,
    /// The accounts column's own search (names, groups, notes).
    pub acc_search: Entity<InputState>,
    pub progress: Option<String>,
    /// What Spotify plays (media.rs), for the status bar; None when it is closed or plays nothing.
    pub music: Option<crate::media::Playing>,
    /// The internet or Roblox can't be reached (failed twice running): the notice's words, in the title bar until it
    /// can again.
    pub offline: Option<&'static str>,
    /// Wallpaper Engine's config.json time and wallpaper last read (wallpaper_engine.rs); None reads it again.
    pub we_last: Option<(std::time::SystemTime, String)>,
    /// A still of the wallpaper just handed to the theme: the wallpaper's file and the theme's picture before it. Once
    /// the theme's picture is another (the still, taken in), the two are kept (`wallpaperEngineStill`).
    pub we_still: Option<(String, String)>,
    /// Follow Wallpaper Engine was just turned on (its switch): the wallpaper is taken at once, whatever the theme.
    pub we_wake: bool,
    pub toast: Option<(String, String)>,
    /// The status bar's pill opened into its card (capsule.rs), if one is.
    pub pill: Option<capsule::Pill>,
    /// The tray icon's panel and menu, in a window of their own once first opened (tray.rs).
    pub tray: Option<WindowHandle<crate::tray::Tray>>,
    /// The notice last shown and the count it came with (drawn while it leaves), and how many were said so far.
    toast_was: ((String, String), u32),
    toast_n: u32,
    shot_asked: bool,
    /// This window's handle, for moving the app's window from the top bar (frame.rs).
    pub hwnd: Option<isize>,
    /// Recent local games for Home and Roblox, derived from their shared session snapshot.
    pub played: Option<Vec<Value>>,
    played_at: Option<(std::time::Instant, u64)>,
    played_pending: bool,
    played_gen: u64,
    /// Each game's pictures by place (gameLooks: icon, cover), and the places already asked for.
    pub looks: std::collections::HashMap<i64, Value>,
    asked_looks: std::collections::HashSet<i64>,
    /// How many times each game's pictures came back missing: a game Roblox never answers for (deleted, no icon)
    /// stops being asked after a few, instead of every half minute for the whole session.
    look_misses: std::collections::HashMap<i64, u8>,
    /// The game on Home's stage, picked from its games.
    /// Which way the place shown came in: 1. from below, -1. from above (kit::step_in).
    pub step_from: f32,
    /// Counts the times the app was opened from the lock screen (a password made or typed): the app's parts come in
    /// anew each time (kit::app_in), as the web page's intro did. With none, the app was there at once, in one frame
    /// (the owner, 2026-11-19: "lúc đăng ký xong ko có motion hiển thị trang chủ, kiểu nó đập thẳng vào mặt").
    pub intro: u32,
    /// Counts the page changes (go, Back, Forward): what was asked "for this page" (Ctrl+L's bar) is stale after one.
    pub nav: u64,
    /// Counts the app's states taken in (what is worked out from the accounts once per state is kept by it).
    pub state_gen: u64,
    pub picked: Option<i64>,
    /// Accounts: the cards picked (by id), the filter over them, the game box of the bar that plays them and the game
    /// it found (resolveGame's answer).
    pub selected: std::collections::HashSet<String>,
    /// The last account clicked on its own: Shift+click picks every one shown from it to the one clicked.
    pub acc_anchor: Option<String>,
    /// The accounts open here at the last state, with their game; and when each one's window closed, and its game:
    /// Roblox's web status lags behind for a while, so for a minute the card says "Closing…" (Accounts.tsx's closedAt).
    was_local: std::collections::HashMap<String, String>,
    pub closed_at: std::collections::HashMap<String, (std::time::Instant, String)>,
    /// The account just brought into view on Accounts (Home's keys, the palette), ringed for a moment (app.js's
    /// revealAccount flash).
    pub revealed: Option<(String, std::time::Instant)>,
    /// The card the arrow keys are on (a thin ring), until a click.
    pub acc_cursor: Option<String>,
    /// Accounts shows the Roblox windows (the Game monitor) instead of the cards or the list; kept between runs.
    pub acc_win: bool,
    /// On the windows view: the games folded to their head, and the one game shown alone.
    pub mon_fold: std::collections::HashSet<String>,
    pub mon_solo: Option<String>,
    /// What the app was last told: a form or dialog is open (pageBusy, so an update waits instead of restarting over it).
    busy_told: bool,
    /// The game box's words are being looked up (the bar says "Looking up the game…").
    pub place_asking: bool,
    /// Settings was open (unlocked) at the last frame: opening it asks for the backup folders to suggest.
    on_settings: bool,
    /// A launch or a preset run waiting for its answer: another click meanwhile does nothing (app.js's inFlight).
    pub in_flight: std::collections::HashSet<&'static str>,
    pub acc_filter: pages::accounts::Filter,
    pub place: Entity<InputState>,
    pub game: Option<Value>,
    /// Accounts' live pictures: the Roblox windows as the app sent them last ("monitor": every 1.5 s while the page
    /// says where their pictures go), and that report, gathered while the window is laid out each frame.
    pub windows: Vec<Value>,
    /// Counts the "monitor" reports, so lists made from the windows know when to be made again.
    pub windows_gen: u64,
    pub live: Live,
    /// Games: what it asked the app for and the game open on it, and its two search boxes (games, a price list's items).
    pub games: pages::games::Data,
    /// Auto: the tab shown (Presets or Macros) and the Delete waiting for its second click.
    pub auto: pages::auto::Data,
    /// Statistics: the tab, period and game picked, the figures and events the app sent last.
    pub stats: pages::stats::Data,
    /// Friends (inside Games): the account looked through, its friends as Roblox gave them last, the filter.
    pub friends: pages::friends::Data,
    /// Avatar (inside Games): the avatar of the account Friends looks through, as Roblox gave it last.
    pub avatar: pages::avatar::Data,
    /// Profile and Marketplace (inside Games): that account's profile, and the shelf or search the Marketplace shows.
    pub profile: pages::profile::Data,
    pub shop_search: Entity<InputState>,
    /// Inbox (inside Games): that account's messages, notifications and Robux lines.
    pub inbox: pages::inbox::Data,
    /// The Roblox tab's Settings, and its About box.
    pub rset: pages::rsettings::Data,
    /// The private servers the accounts own (pages/owned.rs).
    pub owned: pages::owned::Data,
    /// Roblox › Mods, and its search box.
    pub mods: pages::mods::Data,
    pub mods_q: Entity<InputState>,
    pub rset_about: Entity<gpui_kit::component::input::TextareaState>,
    pub passes: pages::passes::Data,
    /// The lock screen (pages/lock.rs), and its two password boxes (the second only when a vault is set up).
    pub lock: pages::lock::Data,
    pub lock_pw: Entity<InputState>,
    pub lock_pw2: Entity<InputState>,
    /// A vault being added: its name.
    pub vault_name: Entity<InputState>,
    /// Settings › Security: the open vault's name, and the master password's three boxes.
    pub s_vault_name: Entity<InputState>,
    /// Statistics' alts board: this PC's name on it.
    pub board_name: Entity<InputState>,
    /// Settings › Notifications: the phone's ntfy topic, typed.
    pub phone_topic: Entity<InputState>,
    /// Settings › Launching: a Roblox window's own width and height, typed.
    pub win_w: Entity<InputState>,
    pub win_h: Entity<InputState>,
    /// The frame rate limit typed (Settings › Staying in game), and the limit and language its hint was last set for.
    pub fps_box: Entity<InputState>,
    fps_hint: Option<(i64, bool)>,
    pub pw_cur: Entity<InputState>,
    pub pw_new: Entity<InputState>,
    pub pw_new2: Entity<InputState>,
    /// The vault has been open in this run: a lock after that says what kept running.
    pub seen_open: bool,
    /// Accounts: the picked accounts' "more" menu is open.
    pub acc_menu: bool,
    /// Accounts' short lists ("Group: All", "Sort: Name"): the one open, and the one a click outside closed just now
    /// (that click on its own button must not reopen it). One closing fades out (accounts::pop_alive).
    pub acc_drop: Option<&'static str>,
    pub acc_drop_closed: Option<(&'static str, std::time::Instant)>,
    /// Accounts: only this group's (none: every group).
    pub acc_group: Option<String>,
    /// Accounts' list: the groups whose part is folded ("": the accounts in none). Kept (prefs' accFold).
    pub acc_fold: std::collections::HashSet<String>,
    /// Launch order's box: the accounts' ids in the order being put together.
    pub order_edit: Vec<String>,
    /// Multi-Roblox's box has been opened: it shows the app's state, so the window is drawn again as that changes.
    pub help_open: bool,
    /// Join a player: the name typed, and the box's state.
    pub jp_name: Entity<InputState>,
    /// An account's form: its boxes (the cookie, the nickname, the group, the password, the authenticator key, the
    /// note) and its state.
    pub af_cookie: Entity<InputState>,
    pub af_alias: Entity<InputState>,
    pub af_group: Entity<InputState>,
    pub af_password: Entity<InputState>,
    pub af_key: Entity<InputState>,
    pub acc_form: pages::account_form::Data,
    /// Send feedback's message and contact boxes; its state, and What's new's.
    pub fb_text: Entity<gpui_kit::component::input::TextareaState>,
    pub fb_contact: Entity<InputState>,
    pub extras: pages::extras::Data,
    /// The game box's list (recent, pinned, saved, favourites, popular).
    pub picker: pages::picker::Data,
    /// The game list's search box; Save server's name and code.
    pub picker_q: Entity<InputState>,
    pub sv_name: Entity<InputState>,
    pub sv_code: Entity<InputState>,
    /// The profile's thought bubble, as it is written (pages/profile.rs).
    pub pf_bubble: Entity<InputState>,
    /// Sign in with your phone: the code on show and how it goes.
    pub quick: pages::quick::Data,
    /// Add account's sign-in screen: its state, and its boxes (username, password, 2-step code).
    pub pwl: pages::password::Data,
    pub pl_user: Entity<InputState>,
    pub pl_pass: Entity<InputState>,
    pub pl_code: Entity<InputState>,
    /// Import from a Bloxkeep file: its state, the file's password and the list's search.
    pub import: pages::import_vault::Data,
    pub iv_pw: Entity<InputState>,
    pub iv_q: Entity<InputState>,
    /// What's new was shown by itself in this run (once, after an update).
    wn_shown: bool,
    pub join: pages::join::Data,
    /// Visibility on Roblox: its box's state.
    pub vis: pages::visibility::Data,
    /// Privacy for one game (beside Play): its menu and the put-back.
    pub gpv: pages::game_privacy::Data,
    /// Easy setup's box (pages/easy_setup.rs).
    pub easy: pages::easy_setup::Data,
    /// Boxes to open once there is a window to open them in (a launch's answer, a remote-control request).
    pending: Vec<Pending>,
    /// The remote-control request already asked about (its id, user and who it replaces).
    remote_asked: String,
    /// The open vault and its accounts' ids, as last seen: a change asks the lent sign-ins again.
    sign_key: String,
    pub servers: pages::servers::Data,
    /// Quick search (Ctrl+K), Accounts' Windows menu and the selection bar's Presets menu.
    pub palette: pages::palette::Data,
    /// The page's room is under 1000 px (set each draw): the Roblox pages' header drops its wordmark and keeps its
    /// view capsule in line, so the capsule and the search box don't lie over each other.
    pub narrow: std::cell::Cell<bool>,
    /// The window's height (set each draw), for what must fit it whole (the big 3D avatar).
    pub window_h: std::cell::Cell<f32>,
    /// Home has no accounts column beside it (a narrow window; set each draw): Home is drawn small then (a low stage,
    /// its games in short tiles) with an Accounts button that slides the column in over the page (`side_open`; when
    /// a click outside shut it, so a click on its own button only shuts it).
    pub no_side: std::cell::Cell<bool>,
    pub side_open: bool,
    pub side_shut: Option<std::time::Instant>,
    pub snap: pages::snap::Data,
    pub preset_menu: pages::preset_menu::Data,
    /// The inbox's chat box: what to say in the open chat (Enter).
    pub chat_box: Entity<InputState>,
    /// Inbox › Chat: a group's name (made or renamed) and the friends picked for it, found by name.
    pub group_name: Entity<InputState>,
    pub group_q: Entity<InputState>,
    /// The search of Roblox's choice of account (pages/tabs.rs `picker`).
    pub pick_q: Entity<InputState>,
    /// Games' account tabs: each an account with what was read for it; the one shown lives in the fields above.
    pub tabs: pages::tabs::Data,
    /// Settings: the part picked and a question asked in place.
    pub settings: pages::settings::Data,
    pub game_search: Entity<InputState>,
    pub price_search: Entity<InputState>,
    /// A game's wiki (its tab): what was read, and its two search boxes (the wiki's, a table's).
    pub wiki: pages::wiki::Data,
    pub wiki_q: Entity<InputState>,
    pub wiki_tq: Entity<InputState>,
    /// Settings' search over every part's rows.
    pub set_q: Entity<InputState>,
    /// Presets: the search over their names and games.
    pub preset_search: Entity<InputState>,
    /// A game's news: the words to be told about.
    pub news_words: Entity<InputState>,
    /// When the community's figures were last asked for (only while the online count is off).
    community_at: Option<std::time::Instant>,
    /// Friends' Add friend box: a name, sent to Roblox's people search on Enter.
    pub friend_search: Entity<InputState>,
    /// Friends' labels: a new label's name, added on Enter.
    pub label_box: Entity<InputState>,
    /// Accounts' Group box: a new group's name, given to the selected accounts on Enter.
    pub group_box: Entity<InputState>,
    pub home_tab: HomeTab,
    /// Where the left button went down on the top bar; moving a few pixels from there moves the window.
    drag_from: Option<Point<Pixels>>,
    lost: bool,
    shown: bool,
    /// The window's own focus, so the keys (Ctrl+K, Tab) work before anything is clicked.
    focus: FocusHandle,
    /// The avatar's 3D box, which takes W A S D while clicked (pages/avatar.rs).
    pub avatar_focus: FocusHandle,
    tasks: Vec<Task<()>>,
    /// The shown toast's timer: a new toast drops (cancels) the one before.
    toast_task: Option<Task<()>>,
    /// The notice its timer was started for (its words and kind, and which showing of them).
    toast_timed: Option<((String, String), u32)>,
    /// The last notice the app itself sent (written to error.log by the app): not written again by the page.
    toast_core: Option<String>,
}

impl Root {
    pub fn new(i18n: I18n, link: Option<(Link, mpsc::UnboundedReceiver<Event>)>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let acc_placeholder = i18n.t("ui.accSearch");
        let acc_search = cx.new(|cx| InputState::new(window, cx).placeholder(acc_placeholder));
        cx.subscribe(&acc_search, |_, _, e: &InputEvent, cx| {
            if matches!(e, InputEvent::Change) {
                cx.notify();
            }
        })
        .detach();
        let mk = |key: &str, window: &mut Window, cx: &mut Context<Self>| {
            let hint = i18n.t(key);
            let e = cx.new(|cx| InputState::new(window, cx).placeholder(hint));
            cx.subscribe(&e, |_, _, ev: &InputEvent, cx| {
                if matches!(ev, InputEvent::Change) {
                    cx.notify();
                }
            })
            .detach();
            e
        };
        let game_search = mk("ui.findGame", window, cx);
        // Typing asks Roblox a moment after the last key (Enter at once); an empty box goes back to the charts.
        cx.subscribe(&game_search, |r: &mut Root, _, e: &InputEvent, cx| match e {
            InputEvent::PressEnter { .. } => {
                r.games.typed += 1;
                pages::games::search(r, cx)
            }
            InputEvent::Change => pages::games::search_soon(r, cx),
            _ => {}
        })
        .detach();
        let mods_q = mk("mods.search", window, cx);
        cx.subscribe(&mods_q, |r: &mut Root, _, e: &InputEvent, cx| match e {
            InputEvent::PressEnter { .. } => pages::mods::search(r, cx),
            InputEvent::Change if r.mods_q.read(cx).value().trim().is_empty() && !r.mods.words_empty() => pages::mods::search(r, cx),
            _ => {}
        })
        .detach();
        let price_search = mk("prices.search", window, cx);
        let preset_search = mk("pv.search", window, cx);
        // The wiki's search: suggestions as it is typed, Enter searches every page. Its table's own search.
        let wiki_q = mk("wiki.searchWiki", window, cx);
        cx.subscribe_in(&wiki_q, window, |r: &mut Root, _, e: &InputEvent, window, cx| match e {
            InputEvent::PressEnter { .. } => pages::wiki::search(r, window, cx),
            InputEvent::Change => pages::wiki::typed(r, cx),
            _ => {}
        })
        .detach();
        let wiki_tq = mk("prices.search", window, cx);
        let set_q = mk("set.search", window, cx);
        // A pause while typing searches Roblox; Enter searches immediately.
        let picker_q = mk("ui.findGame", window, cx);
        cx.subscribe(&picker_q, |r: &mut Root, _, e: &InputEvent, cx| match e {
            InputEvent::PressEnter { .. } => pages::picker::search_menu(r, true, cx),
            InputEvent::Change => pages::picker::search_menu(r, false, cx),
            _ => {}
        })
        .detach();
        // Import's boxes: Enter in the password opens the file; the search narrows the list as it is typed.
        let iv_pw = cx.new(|cx| InputState::new(window, cx).masked(true));
        cx.subscribe_in(&iv_pw, window, |r: &mut Root, _, ev: &InputEvent, window, cx| {
            if matches!(ev, InputEvent::PressEnter { .. }) {
                pages::import_vault::next(r, window, cx);
            }
        })
        .detach();
        let iv_hint = i18n.t("acc.search");
        let iv_q = cx.new(|cx| InputState::new(window, cx).placeholder(iv_hint));
        cx.subscribe_in(&iv_q, window, |_: &mut Root, _, ev: &InputEvent, window, _| {
            if matches!(ev, InputEvent::Change) {
                window.refresh();
            }
        })
        .detach();
        let (pl_user, pl_pass, pl_code) = pages::password::boxes(window, cx);
        // Save server's boxes: Enter saves.
        let sv_name = mk("sv.name", window, cx);
        let sv_code = mk("sv.codeHint", window, cx);
        let pf_bubble = mk("pf.bubble.ph", window, cx);
        cx.subscribe_in(&pf_bubble, window, |r: &mut Root, _, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::PressEnter { .. }) {
                pages::profile::save_bubble(r, cx);
            }
        })
        .detach();
        for e in [&sv_name, &sv_code] {
            cx.subscribe_in(e, window, |r: &mut Root, _, ev: &InputEvent, window, cx| {
                if matches!(ev, InputEvent::PressEnter { .. }) {
                    pages::picker::save_server(r, window, cx);
                }
            })
            .detach();
        }
        // Send feedback's boxes: what would be sent changes with what is typed, so an open preview shuts.
        let fb_text = cx.new(|cx| gpui_kit::component::input::TextareaState::new(window, cx).rows(6));
        let rset_about = cx.new(|cx| gpui_kit::component::input::TextareaState::new(window, cx).rows(5));
        let fb_contact = cx.new(|cx| InputState::new(window, cx));
        cx.subscribe_in(&fb_text, window, |r: &mut Root, _, ev: &InputEvent, window, _| {
            if matches!(ev, InputEvent::Change) {
                pages::extras::edited(r);
                window.refresh();
            }
        })
        .detach();
        cx.subscribe_in(&fb_contact, window, |r: &mut Root, _, ev: &InputEvent, window, _| {
            if matches!(ev, InputEvent::Change) {
                pages::extras::edited(r);
                window.refresh();
            }
        })
        .detach();
        // The account form's boxes: typing takes away the old problem (and checks a key typed: box 2); Enter saves.
        let af = |hint: &str, masked: bool, which: u8, window: &mut Window, cx: &mut Context<Self>| {
            let hint = hint.to_string();
            let e = cx.new(|cx| InputState::new(window, cx).masked(masked).placeholder(hint));
            cx.subscribe_in(&e, window, move |r: &mut Root, _, ev: &InputEvent, window, cx| match ev {
                InputEvent::Change => pages::account_form::typed(r, which, window, cx),
                InputEvent::PressEnter { .. } => pages::account_form::save(r, window, cx),
                _ => {}
            })
            .detach();
            e
        };
        let af_cookie = af("_|WARNING:-DO-NOT-SHARE-THIS…", true, 0, window, cx);
        let af_alias = af("", false, 0, window, cx);
        let af_group = af("", false, 0, window, cx);
        let af_password = af("", true, 1, window, cx);
        let af_key = af("", true, 2, window, cx);
        // Join a player's name: Enter looks it up; typing takes away what was said about the last one.
        let jp_hint = i18n.t("jp.placeholder");
        let jp_name = cx.new(|cx| InputState::new(window, cx).placeholder(jp_hint));
        cx.subscribe_in(&jp_name, window, |r: &mut Root, _, ev: &InputEvent, window, cx| match ev {
            InputEvent::PressEnter { .. } => pages::join::submit(r, window, cx),
            InputEvent::Change => {
                pages::join::typed(r);
                window.refresh();
            }
            _ => {}
        })
        .detach();
        // Kept on Enter or when the box is left, when they changed (NewsView.tsx's Alerts).
        let news_words = mk("news.wordsHint", window, cx);
        cx.subscribe(&news_words, |r: &mut Root, e, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                let words = e.read(cx).value().trim().to_string();
                if let Some(game) = r.games.words_for.clone().filter(|_| words != r.games.words_saved) {
                    r.games.words_saved = words.clone();
                    r.send("feedAlert", json!({ "game": game, "words": words }));
                }
            }
        })
        .detach();
        let chat_box = mk("in.say", window, cx);
        cx.subscribe_in(&chat_box, window, |r: &mut Root, _, e: &InputEvent, window, cx| {
            if matches!(e, InputEvent::PressEnter { .. }) {
                pages::inbox::say(r, window, cx);
            }
        })
        .detach();
        let group_name = mk("in.groupName", window, cx);
        let group_q = mk("in.findFriend", window, cx);
        let pick_q = mk("ui.accSearch", window, cx);
        let shop_search = mk("shop.find", window, cx);
        cx.subscribe(&shop_search, |r: &mut Root, _, e: &InputEvent, cx| match e {
            InputEvent::PressEnter { .. } => pages::profile::shop_find(r, cx),
            InputEvent::Change if r.shop_search.read(cx).value().trim().is_empty() => pages::profile::shop_find(r, cx),
            _ => {}
        })
        .detach();
        let label_box = mk("tag.name", window, cx);
        cx.subscribe(&label_box, |r: &mut Root, _, e: &InputEvent, cx| {
            if matches!(e, InputEvent::PressEnter { .. }) {
                pages::friends::add_label(r, cx);
            }
        })
        .detach();
        let group_box = mk("grp.name", window, cx);
        cx.subscribe(&group_box, |r: &mut Root, _, e: &InputEvent, cx| {
            if matches!(e, InputEvent::PressEnter { .. }) {
                pages::accounts::groups::add(r, cx);
            }
        })
        .detach();
        let friend_search = mk("fr.find", window, cx);
        cx.subscribe(&friend_search, |r: &mut Root, _, e: &InputEvent, cx| {
            if matches!(e, InputEvent::PressEnter { .. }) {
                pages::friends::find(r, cx);
            }
        })
        .detach();
        // The lock screen's boxes: what is typed shows as dots, and Enter sends it.
        let lock_box = |key: &str, window: &mut Window, cx: &mut Context<Self>| {
            let hint = i18n.t(key);
            let e = cx.new(|cx| InputState::new(window, cx).masked(true).placeholder(hint));
            cx.subscribe_in(&e, window, |r: &mut Root, _, ev: &InputEvent, window, cx| {
                if matches!(ev, InputEvent::PressEnter { .. }) {
                    pages::lock::submit(r, window, cx);
                } else if matches!(ev, InputEvent::Change) {
                    // (An old problem goes once the password is being typed again; the strength follows it.)
                    pages::lock::typed(r);
                    cx.notify();
                }
            })
            .detach();
            e
        };
        let lock_pw = lock_box("lock.pw", window, cx);
        let lock_pw2 = lock_box("lock.pw2", window, cx);
        let vault_name = cx.new(|cx| InputState::new(window, cx));
        // The open vault's name: kept on Enter or when the box is left (the old page's renameVault).
        let s_vault_name = cx.new(|cx| InputState::new(window, cx));
        cx.subscribe_in(&s_vault_name, window, |r: &mut Root, e, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                let name = e.read(cx).value().trim().to_string();
                let open = r.state.pointer("/vaults/open").and_then(Value::as_str).unwrap_or_default().to_string();
                let was = r.open_vault_name();
                if !name.is_empty() && !open.is_empty() && name != was {
                    r.send("renameVault", json!({ "id": open, "name": name }));
                }
            }
        })
        .detach();
        // A window size typed: kept on Enter or when the box is left.
        let size_box = |window: &mut Window, cx: &mut Context<Root>| {
            let e = cx.new(|cx| InputState::new(window, cx));
            cx.subscribe_in(&e, window, |r: &mut Root, _, ev: &InputEvent, _, cx| {
                if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                    r.save_window_size(cx);
                }
            })
            .detach();
            e
        };
        let win_w = size_box(window, cx);
        let win_h = size_box(window, cx);
        // The frame rate limit typed: kept on Enter or when the box is left.
        let fps_box = cx.new(|cx| InputState::new(window, cx));
        cx.subscribe_in(&fps_box, window, |r: &mut Root, _, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                r.save_fps(cx);
            }
        })
        .detach();
        let board_hint = i18n.t("cm.namePh");
        let board_name = cx.new(|cx| InputState::new(window, cx).placeholder(board_hint));
        cx.subscribe_in(&board_name, window, |r: &mut Root, e, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                // (At most 20 characters, as the board shows them.)
                let name: String = e.read(cx).value().trim().chars().take(20).collect();
                if name != r.state.get("boardName").and_then(Value::as_str).unwrap_or_default() {
                    r.send("setBoardName", json!({ "name": name }));
                }
            }
        })
        .detach();
        let phone_topic = cx.new(|cx| InputState::new(window, cx));
        cx.subscribe_in(&phone_topic, window, |r: &mut Root, e, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                let topic: String = e.read(cx).value().trim().chars().take(200).collect();
                if topic != r.state.pointer("/phone/topic").and_then(Value::as_str).unwrap_or_default() {
                    r.ask("setPhone", json!({ "topic": topic }), cx, |_, _| {});
                }
            }
        })
        .detach();
        let pw_box = |hint: &str, window: &mut Window, cx: &mut Context<Root>| {
            let hint = i18n.t(hint);
            let e = cx.new(|cx| InputState::new(window, cx).masked(true).placeholder(hint));
            cx.subscribe_in(&e, window, |r: &mut Root, _, ev: &InputEvent, window, cx| match ev {
                InputEvent::PressEnter { .. } => pages::settings::change_password(r, window, cx),
                InputEvent::Change => {
                    pages::settings::password_typed(r, cx);
                    cx.notify();
                }
                _ => {}
            })
            .detach();
            e
        };
        let pw_cur = pw_box("set.pwCur", window, cx);
        let pw_new = pw_box("set.pwNew", window, cx);
        let pw_new2 = pw_box("set.pwNew2", window, cx);
        let place_hint = i18n.t("dock.place");
        let place = cx.new(|cx| InputState::new(window, cx).placeholder(place_hint));
        // Names open the picker. A launch target keeps the normal Enter and Play behavior.
        cx.subscribe(&place, |r: &mut Root, _, e: &InputEvent, cx| match e {
            InputEvent::PressEnter { .. } => {
                if pages::picker::search_dock(r, true, cx) {
                    return;
                }
                let v = r.place.read(cx).value().trim().to_string();
                r.resolve_place(cx);
                if v.is_empty() || crate::target::parse(&v, "").is_ok() {
                    pages::accounts::play_selected(r, cx);
                }
            }
            InputEvent::Change => {
                r.game = None;
                // (A lookup still running is for the old words: its answer is dropped, so nothing is asked now.)
                r.place_asking = false;
                if r.page == Page::Accounts && !r.locked() {
                    pages::picker::search_dock(r, false, cx);
                }
                cx.notify();
            }
            _ => {}
        })
        .detach();
        let mut root = Self {
            link: None,
            i18n,
            tool_pages: Default::default(),
            state: Value::Null,
            metrics: Value::Null,
            page: Page::Home,
            acc_search,
            progress: None,
            music: crate::media::shot_stand_in(),
            offline: None,
            we_last: None,
            we_still: None,
            we_wake: false,
            gpv: Default::default(),
            easy: Default::default(),
            toast: None,
            pill: None,
            tray: None,
            toast_was: Default::default(),
            toast_n: 0,
            shot_asked: false,
            step_from: 1.,
            intro: 0,
            nav: 0,
            state_gen: 0,
            windows_gen: 0,
            hwnd: None,
            played: None,
            played_at: None,
            played_pending: false,
            played_gen: 0,
            looks: Default::default(),
            asked_looks: Default::default(),
            look_misses: Default::default(),
            picked: None,
            windows: Vec::new(),
            live: Live::default(),
            games: Default::default(),
            auto: Default::default(),
            friends: Default::default(),
            avatar: Default::default(),
            profile: Default::default(),
            shop_search,
            inbox: Default::default(),
            rset: Default::default(),
            owned: Default::default(),
            rset_about,
            mods: Default::default(),
            mods_q,
            passes: Default::default(),
            lock: Default::default(),
            lock_pw,
            lock_pw2,
            vault_name,
            s_vault_name,
            board_name,
            phone_topic,
            win_w,
            win_h,
            fps_box,
            fps_hint: None,
            pw_cur,
            pw_new,
            pw_new2,
            seen_open: false,
            acc_menu: false,
            acc_drop: None,
            acc_drop_closed: None,
            acc_group: None,
            acc_fold: crate::prefs::get("accFold").and_then(|v| v.as_array().map(|l| l.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())).unwrap_or_default(),
            order_edit: Vec::new(),
            help_open: false,
            jp_name,
            af_cookie,
            af_alias,
            af_group,
            af_password,
            af_key,
            acc_form: Default::default(),
            fb_text,
            fb_contact,
            extras: Default::default(),
            picker: Default::default(),
            picker_q,
            sv_name,
            sv_code,
            pf_bubble,
            quick: Default::default(),
            pwl: Default::default(),
            pl_user,
            pl_pass,
            pl_code,
            import: Default::default(),
            iv_pw,
            iv_q,
            wn_shown: false,
            join: Default::default(),
            vis: Default::default(),
            pending: Vec::new(),
            remote_asked: String::new(),
            sign_key: String::new(),
            servers: Default::default(),
            palette: Default::default(),
            narrow: Default::default(),
            window_h: Default::default(),
            no_side: Default::default(),
            side_open: false,
            side_shut: None,
            snap: Default::default(),
            preset_menu: Default::default(),
            chat_box,
            group_name,
            group_q,
            pick_q,
            tabs: Default::default(),
            stats: Default::default(),
            settings: Default::default(),
            game_search,
            price_search,
            preset_search,
            wiki_q,
            wiki_tq,
            set_q,
            wiki: Default::default(),
            news_words,
            community_at: None,
            friend_search,
            label_box,
            group_box,
            selected: Default::default(),
            acc_anchor: None,
            was_local: Default::default(),
            closed_at: Default::default(),
            revealed: None,
            acc_cursor: None,
            acc_win: crate::prefs::get("accWindows").and_then(|v| v.as_bool()).unwrap_or(false),
            mon_fold: Default::default(),
            mon_solo: None,
            busy_told: false,
            place_asking: false,
            on_settings: false,
            in_flight: Default::default(),
            acc_filter: pages::accounts::Filter::All,
            place,
            game: None,
            home_tab: HomeTab::Games,
            drag_from: None,
            back: Vec::new(),
            fwd: Vec::new(),
            lost: false,
            shown: false,
            focus: cx.focus_handle(),
            avatar_focus: cx.focus_handle(),
            tasks: Vec::new(),
            toast_task: None,
            toast_timed: None,
            toast_core: None,
        };
        // The window's heartbeat for core/log.rs's watch: this runs on the window's own thread, so a beat that stops
        // coming means the window stopped answering (written to error.log and the Console with how long).
        #[cfg(feature = "core")]
        root.tasks.push(cx.spawn(async move |_, cx| {
            loop {
                crate::core::log::window_beat();
                cx.background_executor().timer(Duration::from_millis(500)).await;
            }
        }));
        // Marketplace › Watching (itemwatch.rs): every few minutes the watched things are read again, and what changed
        // is said as Bloxkeep's other news is (a toast, Windows, Discord, phone).
        #[cfg(not(feature = "shot"))]
        root.tasks.push(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(40)).await;
            loop {
                let got = crate::roblox::run(crate::itemwatch::check).await.unwrap_or_default();
                let alive = this.update(cx, |r, cx| {
                    for (w, changes) in &got {
                        for ch in changes {
                            let text = format!("{}: {}", w.name, pages::profile::change_words(r, ch));
                            r.send("notifyItem", serde_json::json!({ "text": text, "key": format!("item:{}:{}:{}", w.id, ch.kind, ch.at) }));
                        }
                    }
                    // The first change also opens the capsule, with the thing's picture.
                    if let Some((w, ch)) = got.iter().find_map(|(w, c)| c.first().map(|ch| (w, ch))) {
                        let l = pages::games::Live {
                            key: format!("item:{}:{}", w.id, ch.at),
                            pic: w.pic.clone(),
                            round: false,
                            icon: "tag",
                            tone: OK,
                            title: w.name.clone(),
                            sub: pages::profile::change_words(r, ch),
                            act: Some(pages::games::LiveAct::Watching),
                            busy: false,
                            at: std::time::Instant::now(),
                        };
                        pages::games::live_show(r, l, cx);
                    }
                    let fresh = crate::itemwatch::list();
                    if !got.is_empty() || fresh.len() != r.profile.watch.len() {
                        r.profile.watch = fresh;
                        if r.profile.watch_view {
                            crate::itemwatch::seen();
                        }
                        cx.notify();
                    }
                });
                if alive.is_err() {
                    break;
                }
                cx.background_executor().timer(crate::itemwatch::EVERY).await;
            }
        }));
        // Spotify's song for the status bar and the profile's song card, read every second off the window (Windows'
        // media controls answer in a few ms; every second, as the card shows where the song is).
        #[cfg(not(feature = "shot"))]
        root.tasks.push(cx.spawn(async move |this, cx| {
            loop {
                let now = cx.background_executor().spawn(async { crate::media::now() }).await;
                let alive = this.update(cx, |r, cx| {
                    if r.music != now {
                        r.music = now;
                        cx.notify();
                    }
                });
                if alive.is_err() {
                    break;
                }
                cx.background_executor().timer(Duration::from_secs(1)).await;
            }
        }));
        // The connection, as Discord's and Facebook's notice says it: the internet (every 4 s, net::online_now) and
        // Roblox itself (every 15 s, and at once when a call to it got no answer): with a VPN turned off, the internet
        // works and Roblox is blocked, which said nothing before (the owner, 2026-11-19). Lost after two failed looks
        // in a row, so one slow answer (waking from sleep, a VPN switching) says nothing; back at the first good one,
        // and what failed to load meanwhile is read again. (A shot shows the notice with BLOXKEEP_SHOT_OFFLINE=1 or
        // =roblox, without looking.)
        root.tasks.push(cx.spawn(async move |this, cx| {
            #[cfg(feature = "shot")]
            if let Ok(v) = std::env::var("BLOXKEEP_SHOT_OFFLINE") {
                let _ = this.update(cx, |r, cx| {
                    r.offline = Some(if v == "roblox" { "net.roblox" } else { "net.lost" });
                    cx.notify();
                });
                return;
            }
            let (mut failed, mut since_roblox) = (0u32, u32::MAX);
            loop {
                let urgent = crate::net::take_roblox_failed();
                let ask_roblox = urgent || failed > 0 || since_roblox >= 15;
                let now = cx.background_executor().spawn(async move {
                    if !crate::net::online_now() {
                        Some("net.lost")
                    } else if ask_roblox && !crate::net::roblox_reachable() {
                        Some("net.roblox")
                    } else {
                        None
                    }
                }).await;
                since_roblox = if ask_roblox { 0 } else { since_roblox.saturating_add(2) };
                failed = if now.is_some() { failed + 1 } else { 0 };
                let alive = this.update(cx, |r, cx| {
                    let shown = if failed >= 2 { now } else if failed == 0 { None } else { r.offline };
                    if shown != r.offline {
                        let was = r.offline;
                        r.offline = shown;
                        #[cfg(feature = "core")]
                        crate::core::log::note_file(match shown {
                            Some("net.lost") => "Connection: the internet can't be reached (no answer twice running)",
                            Some(_) => "Connection: the internet works but Roblox can't be reached (twice running; blocked on this network?)",
                            None => "Connection: back",
                        });
                        if shown.is_none() && was.is_some() {
                            let text = r.tr("net.back");
                            r.toast = Some((text, "ok".into()));
                            r.toast_n += 1;
                            pages::inbox::forget_failed(r);
                        }
                        cx.notify();
                    }
                });
                if alive.is_err() {
                    break;
                }
                cx.background_executor().timer(Duration::from_secs(2)).await;
            }
        }));
        // Follow Wallpaper Engine (Settings › Appearance): its config.json looked at every 3 s while on; a new wallpaper
        // becomes the custom theme's picture or video and accent (the core's useWallpaperEngine).
        #[cfg(not(feature = "shot"))]
        root.tasks.push(cx.spawn(async move |this, cx| {
            loop {
                // (Not before the app's state is here: which picture the theme has is read from it below, and a look
                // without it handed the wallpaper's preview over again.)
                let Ok(ready) = this.update(cx, |r, _| r.state.get("theme").is_some()) else { break };
                if !ready {
                    cx.background_executor().timer(Duration::from_millis(200)).await;
                    continue;
                }
                // (The custom theme's picture now, while Custom is the theme.)
                let picture = |r: &Root| r.state.get("theme").and_then(Value::as_str).filter(|t| *t == "custom").and_then(|_| r.state.pointer("/customTheme/picture")).and_then(Value::as_str).unwrap_or_default().to_string();
                let Ok((on, last, kept)) = this.update(cx, |r, _| {
                    let now = picture(r);
                    // A still handed over has become the theme's picture: kept with its wallpaper for the next start.
                    if r.we_still.as_ref().is_some_and(|(_, before)| !now.is_empty() && now != *before) {
                        let (file, _) = r.we_still.take().unwrap_or_default();
                        crate::prefs::set("wallpaperEngineStill", json!({ "file": file, "picture": now }));
                    }
                    // The wallpaper whose still is the theme's picture already (see wallpaper_engine::check); not when
                    // the switch was just turned on: the wallpaper is handed over then, which makes Custom the theme.
                    let kept = crate::prefs::get("wallpaperEngineStill").filter(|k| !r.we_wake && !now.is_empty() && k.get("picture").and_then(Value::as_str) == Some(now.as_str())).and_then(|k| k.get("file").and_then(Value::as_str).map(str::to_string));
                    // The following is the custom theme's: with another theme in use it rests (no look at Wallpaper
                    // Engine, no second copy of the wallpaper playing), until Custom is the theme again or its switch
                    // is turned on anew. A new wallpaper pulled the app back to Custom otherwise.
                    let custom = r.state.get("theme").and_then(Value::as_str) == Some("custom");
                    let on = crate::prefs::get("followWallpaperEngine").and_then(|v| v.as_bool()).unwrap_or(false) && (custom || r.we_wake);
                    (on, r.we_last.clone(), kept)
                }) else { break };
                if on {
                    let got = cx.background_executor().spawn(async move { crate::wallpaper_engine::check(last.as_ref(), kept.as_deref()) }).await;
                    if let Some((key, wall)) = got {
                        let alive = this.update(cx, |r, _| {
                            if let Some(w) = wall {
                                if crate::wallpaper_engine::is_still(&w.media) {
                                    r.we_still = Some((key.1.clone(), picture(r)));
                                }
                                r.send("useWallpaperEngine", json!({ "path": w.media.to_string_lossy(), "accent": w.accent.unwrap_or_default() }));
                            }
                            r.we_wake = false;
                            r.we_last = Some(key);
                        });
                        if alive.is_err() {
                            break;
                        }
                    }
                    // The wallpaper itself, live, while Wallpaper Engine runs (wallpaper_engine.rs › keep_live).
                    let Ok(file) = this.update(cx, |r, _| r.we_last.as_ref().map(|k| k.1.clone()).unwrap_or_default()) else { break };
                    let was = crate::wallpaper_engine::live();
                    cx.background_executor().spawn(async move { crate::wallpaper_engine::keep_live(&file) }).await;
                    if crate::wallpaper_engine::live() != was && this.update(cx, |_, cx| cx.notify()).is_err() {
                        break;
                    }
                } else if crate::wallpaper_engine::live().is_some() {
                    cx.background_executor().spawn(async { crate::wallpaper_engine::stop_live() }).await;
                    if this.update(cx, |_, cx| cx.notify()).is_err() {
                        break;
                    }
                }
                cx.background_executor().timer(Duration::from_secs(3)).await;
            }
        }));
        // Privacy for one game (pages/game_privacy.rs): accounts that have left the game they were set for get their own
        // settings back, looked at every 10 s.
        #[cfg(not(feature = "shot"))]
        root.tasks.push(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(10)).await;
                if this.update(cx, |r, cx| crate::pages::game_privacy::tick(r, cx)).is_err() {
                    break;
                }
            }
        }));
        // The playing times count up each second while someone plays.
        root.tasks.push(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let alive = this.update(cx, |r, cx| {
                    // (…and a card's countdown, "rejoining in 8 s", wherever cards are.)
                    if (r.page == Page::Home && r.accounts().iter().any(|a| a.get("state").and_then(Value::as_str) == Some("running")))
                        || (matches!(r.page, Page::Home | Page::Accounts) && r.accounts().iter().any(pages::accounts::counting))
                    {
                        cx.notify();
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        }));
        if let Some((link, mut events)) = link {
            let first = link.call("getState", Value::Null);
            root.link = Some(link);
            root.tasks.push(cx.spawn_in(window, async move |this, cx| {
                if let Ok(Ok(v)) = first.await {
                    let _ = this.update_in(cx, |r, window, cx| {
                        r.set_state(v.get("state").cloned().unwrap_or(Value::Null), window, cx);
                        if let Some(view) = v.get("view").and_then(Value::as_str).and_then(|s| serde_json::from_str::<Value>(s).ok()) {
                            r.resume(&view);
                        }
                    });
                }
                while let Some((name, data)) = events.next().await {
                    if this.update_in(cx, |r, window, cx| r.on_event(&name, data, window, cx)).is_err() {
                        break;
                    }
                }
            }));
        }
        window.focus(&root.focus, cx);
        root
    }

    fn set_state(&mut self, state: Value, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(lang) = state.get("language").and_then(Value::as_str) {
            if (lang == "vi") != self.i18n.vietnamese {
                self.i18n.set_language(lang);
                let p = self.i18n.t("ui.accSearch");
                self.acc_search.update(cx, |s, cx| s.set_placeholder(p, window, cx));
                let p = self.i18n.t("dock.place");
                self.place.update(cx, |s, cx| s.set_placeholder(p, window, cx));
                let p = self.i18n.t("ui.findGame");
                self.game_search.update(cx, |s, cx| s.set_placeholder(p, window, cx));
                let p = self.i18n.t("mods.search");
                self.mods_q.update(cx, |s, cx| s.set_placeholder(p, window, cx));
                let p = self.i18n.t("prices.search");
                self.price_search.update(cx, |s, cx| s.set_placeholder(p, window, cx));
                let p = self.i18n.t("jp.placeholder");
                self.jp_name.update(cx, |s, cx| s.set_placeholder(p, window, cx));
                for (key, e) in [("ui.findGame", self.picker_q.clone()), ("acc.search", self.iv_q.clone()), ("cm.namePh", self.board_name.clone()), ("wiki.searchWiki", self.wiki_q.clone()), ("prices.search", self.wiki_tq.clone()), ("set.search", self.set_q.clone()), ("in.groupName", self.group_name.clone()), ("in.findFriend", self.group_q.clone()), ("ui.accSearch", self.pick_q.clone())] {
                    let p = self.i18n.t(key);
                    e.update(cx, |s, cx| s.set_placeholder(p, window, cx));
                }
                let p = self.i18n.t("pv.search");
                self.preset_search.update(cx, |s, cx| s.set_placeholder(p, window, cx));
                let p = self.i18n.t("news.wordsHint");
                self.news_words.update(cx, |s, cx| s.set_placeholder(p, window, cx));
                let p = self.i18n.t("in.say");
                self.chat_box.update(cx, |s, cx| s.set_placeholder(p, window, cx));
                let p = self.i18n.t("shop.find");
                self.shop_search.update(cx, |s, cx| s.set_placeholder(p, window, cx));
                let p = self.i18n.t("fr.find");
                self.friend_search.update(cx, |s, cx| s.set_placeholder(p, window, cx));
                let p = self.i18n.t("tag.name");
                self.label_box.update(cx, |s, cx| s.set_placeholder(p, window, cx));
                let p = self.i18n.t("grp.name");
                self.group_box.update(cx, |s, cx| s.set_placeholder(p, window, cx));
                let p = self.i18n.t("lock.pw");
                self.lock_pw.update(cx, |s, cx| s.set_placeholder(p, window, cx));
                let p = self.i18n.t("lock.pw2");
                self.lock_pw2.update(cx, |s, cx| s.set_placeholder(p, window, cx));
                for (key, e) in [("set.pwCur", self.pw_cur.clone()), ("set.pwNew", self.pw_new.clone()), ("set.pwNew2", self.pw_new2.clone())] {
                    let p = self.i18n.t(key);
                    e.update(cx, |s, cx| s.set_placeholder(p, window, cx));
                }
            }
        }
        // The vault is open (or was closed again): the password typed goes from its boxes, and the next lock screen
        // gets the keyboard afresh.
        let was = self.locked();
        let was_adding = pages::lock::adding(self);
        self.state = state;
        self.state_gen += 1;
        // (Home's newest events follow the app's states, not only its half-minute clock.)
        pages::stats::state_changed(self, cx);
        // Try it's log follows every state the app sends, not only the ones a frame draws.
        pages::macro_form::on_state(self);
        // (The first question's auto-rejoin, put on the first accounts once they are here.)
        pages::home::setup_follow(self, cx);
        // The theme it says is in use (Settings › Appearance): its colours, and the picture behind the window.
        crate::theme::follow(&self.state, cx);
        // Names hidden (Settings › Appearance): every account goes by its first letter everywhere, as the web page
        // showed them (app.js's maskNames); what it is really called is kept under "real", never drawn.
        if self.state.get("hideNames").and_then(Value::as_bool) == Some(true) {
            if let Some(list) = self.state.get_mut("accounts").and_then(Value::as_array_mut) {
                for a in list.iter_mut() {
                    let text = |k: &str| a.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
                    let (label, user, shown, alias) = (text("label"), text("username"), text("displayName"), text("alias"));
                    let first = [label.as_str(), user.as_str()].into_iter().map(str::trim).find(|n| !n.is_empty()).and_then(|n| n.chars().next()).map(String::from).unwrap_or_default();
                    a["real"] = json!({ "label": label, "username": user, "displayName": shown, "alias": alias });
                    a["label"] = format!("{first}…").into();
                    a["username"] = "".into();
                    a["displayName"] = "".into();
                    a["alias"] = "".into();
                }
            }
        }
        self.prune_selected();
        if self.help_open {
            window.refresh();
        }
        // After an update the app's state carries the notes once: shown by themselves, and the app told they were.
        if !self.wn_shown && !self.locked() {
            if let Some(list) = self.state.pointer("/whatsNew/releases").and_then(Value::as_array).filter(|l| !l.is_empty()).cloned() {
                self.show_whats_new(list);
                self.send("seenWhatsNew", json!({}));
            }
        }
        // A window here that just closed: noted, and drawn again once the minute is over.
        let mut closed_now = false;
        let mut open_now = std::collections::HashMap::new();
        let seen: Vec<(String, Option<String>, bool)> = self
            .accounts()
            .iter()
            .map(|a| {
                let id = a.get("id").and_then(Value::as_str).unwrap_or_default().to_string();
                let game = pages::accounts::is_local(a).then(|| {
                    if a.pointer("/presence/type").and_then(Value::as_str) == Some("ingame") { a.pointer("/presence/game").and_then(Value::as_str) } else { None }
                        .or_else(|| a.pointer("/session/game/name").and_then(Value::as_str))
                        .unwrap_or_default()
                        .to_string()
                });
                (id, game, a.get("state").and_then(Value::as_str) == Some("launching"))
            })
            .collect();
        for (id, game, launching) in seen {
            if let Some(game) = game {
                open_now.insert(id, game);
            } else if let Some(game) = self.was_local.get(&id).filter(|_| !launching).cloned() {
                self.closed_at.insert(id, (std::time::Instant::now(), game));
                closed_now = true;
            }
        }
        self.was_local = open_now;
        self.closed_at.retain(|_, (at, _)| at.elapsed() < pages::accounts::CLOSING);
        if closed_now {
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(pages::accounts::CLOSING + std::time::Duration::from_millis(500)).await;
                let _ = this.update(cx, |_, cx| cx.notify());
            })
            .detach();
        }
        if !self.locked() {
            self.seen_open = true;
        } else if !was {
            // (A notice may name an account: none stays over the lock screen.)
            self.toast = None;
        }
        // The window size's boxes follow the app, unless one is being typed in.
        for (e, key, or) in [(self.win_w.clone(), "/settings/windowWidth", 640), (self.win_h.clone(), "/settings/windowHeight", 480)] {
            let v = self.state.pointer(key).and_then(Value::as_i64).unwrap_or(or).to_string();
            if e.read(cx).value().as_ref() != v && !e.read(cx).focus_handle(cx).is_focused(window) {
                e.update(cx, |s, cx| s.set_value(v, window, cx));
            }
        }
        // The frame rate box too: its number, or empty over the words for "as Roblox has it" and for no limit at all.
        {
            let fps = self.state.pointer("/settings/robloxConfig/fps").and_then(Value::as_i64).unwrap_or(0);
            let v = if fps > 0 && fps != pages::settings::FPS_MAX { fps.to_string() } else { String::new() };
            let e = self.fps_box.clone();
            if e.read(cx).value().as_ref() != v && !e.read(cx).focus_handle(cx).is_focused(window) {
                e.update(cx, |s, cx| s.set_value(v, window, cx));
            }
            let said = (fps, self.i18n.vietnamese);
            if self.fps_hint != Some(said) {
                self.fps_hint = Some(said);
                let hint = self.tr(if fps == pages::settings::FPS_MAX { "rc.fpsMax" } else { "rc.keep" });
                e.update(cx, |s, cx| s.set_placeholder(hint, window, cx));
            }
        }
        let on_board = self.state.get("boardName").and_then(Value::as_str).unwrap_or_default().to_string();
        if self.board_name.read(cx).value().as_ref() != on_board && !self.board_name.read(cx).focus_handle(cx).is_focused(window) {
            self.board_name.update(cx, |s, cx| s.set_value(on_board, window, cx));
        }
        let topic = self.state.pointer("/phone/topic").and_then(Value::as_str).unwrap_or_default().to_string();
        if self.phone_topic.read(cx).value().as_ref() != topic && !self.phone_topic.read(cx).focus_handle(cx).is_focused(window) {
            self.phone_topic.update(cx, |s, cx| s.set_value(topic, window, cx));
        }
        // Settings' box for the open vault's name follows the app, unless it is being typed in.
        let name = self.open_vault_name();
        if self.s_vault_name.read(cx).value().as_ref() != name && !self.s_vault_name.read(cx).focus_handle(cx).is_focused(window) {
            self.s_vault_name.update(cx, |s, cx| s.set_value(name, window, cx));
        }
        // A vault being added: its name box starts with the name the app proposes.
        if pages::lock::adding(self) && !was_adding {
            let name = self.state.pointer("/vaults/name").and_then(Value::as_str).unwrap_or_default().to_string();
            self.vault_name.update(cx, |s, cx| s.set_value(name, window, cx));
        }
        // Someone asks to control this PC from Discord (/pc): the box, once per request (app.js's askRemote).
        let ask = self.state.pointer("/remote/ask").filter(|a| a.is_object()).cloned();
        let key = ask.as_ref().map(|a| format!("{}|{}|{}", a.get("id").map(Value::to_string).unwrap_or_default(), a.get("user").and_then(Value::as_str).unwrap_or_default(), a.get("replacing").and_then(Value::as_str).unwrap_or_default())).unwrap_or_default();
        if key != self.remote_asked {
            self.remote_asked = key;
            if let Some(a) = ask {
                self.pending.push(Pending::Remote(a));
            }
        }
        // Another vault, or an account added or removed: a sign-in lent before may be someone else's.
        let sign_key = {
            let mut k = self.state.pointer("/vaults/open").and_then(Value::as_str).unwrap_or_default().to_string();
            for a in self.state.get("accounts").and_then(Value::as_array).into_iter().flatten() {
                k.push('|');
                k.push_str(a.get("id").and_then(Value::as_str).unwrap_or_default());
            }
            k
        };
        // (Locked, the app shows no vault and no accounts: that is not "every account gone", so the key waits for the
        // unlock, and the Games account tabs survive a Lock and Unlock of the same vault. The sign-ins still go at the
        // lock, below.)
        if sign_key != self.sign_key && !self.locked() {
            let first = self.sign_key.is_empty();
            self.sign_key = sign_key;
            if !first {
                self.games.forget_sign_in();
                self.friends.forget_sign_in();
                // Another vault's games played.
                self.clear_play_history();
                // Tabs of accounts no longer here go (Games' account tabs).
                pages::tabs::forget_gone(self);
            }
        }
        if was != self.locked() {
            if self.locked() {
                // Nothing stays over the lock screen: the boxes open close, and what was typed into a secret box
                // (a cookie, a password, an authenticator key) goes with them.
                window.close_all_dialogs(cx);
                // A sign-in lent before the lock is not kept behind it (the vault may not be the same one after).
                self.games.forget_sign_in();
                self.friends.forget_sign_in();
                self.tabs.vng.clear();
                self.clear_play_history();
                for e in [&self.af_cookie, &self.af_password, &self.af_key, &self.pl_pass, &self.pl_code, &self.pw_cur, &self.pw_new, &self.pw_new2, &self.iv_pw] {
                    e.update(cx, |s, cx| s.set_value("", window, cx));
                }
            }
            self.lock_pw.update(cx, |s, cx| {
                s.set_value("", window, cx);
                s.set_masked(true, window, cx);
            });
            self.lock_pw2.update(cx, |s, cx| {
                s.set_value("", window, cx);
                s.set_masked(true, window, cx);
            });
            self.lock = Default::default();
            if !self.locked() {
                window.focus(&self.focus, cx);
                self.intro += 1;
            }
        }
        // BLOXKEEP_SHOT_PICK="Farm 1,Night Owl": a hidden shot starts with those accounts picked (by name).
        #[cfg(feature = "shot")]
        if let Ok(names) = std::env::var("BLOXKEEP_SHOT_PICK") {
            let ids: Vec<String> = self
                .accounts()
                .iter()
                .filter(|a| names.split(',').any(|n| n.trim() == pages::accounts::label(a)))
                .filter_map(|a| a.get("id").and_then(Value::as_str).map(str::to_string))
                .collect();
            self.selected.extend(ids);
            // BLOXKEEP_SHOT_MORE=1: with their "more" menu open. BLOXKEEP_SHOT_PICKER=1: with the game list open.
            self.acc_menu = std::env::var("BLOXKEEP_SHOT_MORE").is_ok();
            if std::env::var("BLOXKEEP_SHOT_PICKER").is_ok() && !self.picker.open {
                pages::picker::open_now(self, cx);
            }
            // Hidden window input goes through the editor so the regular Change subscriptions run.
            if self.page == Page::Accounts && !self.locked()
                && let Ok(input) = std::env::var("BLOXKEEP_SHOT_GAME_INPUT")
            {
                let box_ = if std::env::var("BLOXKEEP_SHOT_PICKER").is_ok() { &self.picker_q } else { &self.place };
                if box_.read(cx).value().is_empty() {
                    box_.update(cx, |state, cx| state.replace_all(input, window, cx));
                }
            }
        }
        // BLOXKEEP_SHOT_DROP=sort|group: Accounts' short list open; BLOXKEEP_SHOT_HOMETAB=community: Home's games card on
        // what Bloxkeep's users play; BLOXKEEP_SHOT_LOST=1: the window as when the line to the app is gone.
        #[cfg(feature = "shot")]
        {
            match std::env::var("BLOXKEEP_SHOT_DROP").as_deref() {
                Ok("launch") => {
                    if self.settings.menu_was.is_none() {
                        self.settings.menu = Some("s-launch");
                        self.settings.menu_was = Some("s-launch");
                    }
                }
                Ok("sort") => self.acc_drop = Some("acc-sort"),
                Ok("group") => self.acc_drop = Some("acc-group"),
                Ok("bargroup") => self.acc_drop = Some("bar-group"),
                Ok("load") => self.acc_drop = Some("mon-load"),
                _ => {}
            }
            if std::env::var("BLOXKEEP_SHOT_HOMETAB").as_deref() == Ok("community") {
                self.home_tab = HomeTab::Community;
            }
            if std::env::var("BLOXKEEP_SHOT_LOST").is_ok() {
                self.lost = true;
            }
        }
        // BLOXKEEP_SHOT_FRIENDS=file.json: a shot's Games opens on Friends, filled from that file (roblox.rs).
        #[cfg(feature = "shot")]
        if std::env::var("BLOXKEEP_SHOT_FRIENDS").is_ok() {
            // (BLOXKEEP_SHOT_HOME=1: on the Game view instead, its row of faces filled from the same file.)
            self.games.view = if std::env::var("BLOXKEEP_SHOT_HOME").is_ok() { pages::games::View::Games } else { pages::games::View::Friends };
            // BLOXKEEP_SHOT_FILTER=offline: only the friends who are offline.
            if std::env::var("BLOXKEEP_SHOT_FILTER").as_deref() == Ok("offline") {
                self.friends.filter = pages::friends::Filter::Offline;
            }
            // BLOXKEEP_SHOT_ADD="builder": Add friend open on that search.
            if let (Ok(q), false) = (std::env::var("BLOXKEEP_SHOT_ADD"), self.friends.adding) {
                self.friends.adding = true;
                self.friend_search.update(cx, |s, cx| s.set_value(q, window, cx));
                pages::friends::find(self, cx);
            }
        }
        // BLOXKEEP_SHOT_PASSES=2753915549: a shot's Games opens on that game's page, on its passes.
        #[cfg(feature = "shot")]
        if let (Some(place), None) = (std::env::var("BLOXKEEP_SHOT_PASSES").ok().and_then(|v| v.parse().ok()), &self.games.open) {
            pages::games::open_place(self, place, "");
            self.games.tab = pages::games::Tab::Passes;
        }
        // BLOXKEEP_SHOT_GAMEPAGE=2753915549: on that game's page, its Overview (with BLOXKEEP_SHOT_EVENTS: its events).
        #[cfg(feature = "shot")]
        if let (Some(place), None) = (std::env::var("BLOXKEEP_SHOT_GAMEPAGE").ok().and_then(|v| v.parse().ok()), &self.games.open) {
            pages::games::open_place(self, place, "");
        }
        // BLOXKEEP_SHOT_CODES=2753915549: on that game's codes.
        #[cfg(feature = "shot")]
        if let (Some(place), None) = (std::env::var("BLOXKEEP_SHOT_CODES").ok().and_then(|v| v.parse().ok()), &self.games.open) {
            pages::games::open_place(self, place, "");
            self.games.tab = pages::games::Tab::Codes;
        }
        // BLOXKEEP_SHOT_SERVERS=2753915549: on that game's private servers.
        #[cfg(feature = "shot")]
        if let (Some(place), None) = (std::env::var("BLOXKEEP_SHOT_SERVERS").ok().and_then(|v| v.parse().ok()), &self.games.open) {
            pages::games::open_place(self, place, "");
            self.games.tab = pages::games::Tab::Servers;
        }
        // BLOXKEEP_SHOT_TABS=2: that many more accounts' tabs are open.
        #[cfg(feature = "shot")]
        if let Some(n) = std::env::var("BLOXKEEP_SHOT_TABS").ok().and_then(|v| v.parse().ok()) {
            pages::tabs::shot_open(self, n);
        }
        // BLOXKEEP_SHOT_AVATAR=156: a shot's Games opens on Avatar, showing that Roblox user's (an avatar is public).
        #[cfg(feature = "shot")]
        if std::env::var("BLOXKEEP_SHOT_AVATAR").is_ok() {
            self.games.view = pages::games::View::Avatar;
        }
        // BLOXKEEP_SHOT_PROFILE=156: a shot's Games opens on that Roblox user's profile; BLOXKEEP_SHOT_SHOP=1: on the
        // Marketplace.
        #[cfg(feature = "shot")]
        if std::env::var("BLOXKEEP_SHOT_PROFILE").is_ok() {
            self.games.view = pages::games::View::Profile;
            pages::profile::shot_club(self, cx);
        }
        // BLOXKEEP_SHOT_LEAVE=ms: a hidden run leaves its page for Settings after that long (what a page left behind
        // still holds, measured from outside).
        #[cfg(feature = "shot")]
        if let Some(ms) = std::env::var("BLOXKEEP_SHOT_LEAVE").ok().and_then(|v| v.parse::<u64>().ok()) {
            static LEFT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
            if !LEFT.swap(true, std::sync::atomic::Ordering::Relaxed) {
                cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(std::time::Duration::from_millis(ms)).await;
                    let _ = this.update(cx, |r, cx| {
                        r.games.open = None;
                        r.go(Page::Settings);
                        cx.notify();
                    });
                })
                .detach();
            }
        }
        // BLOXKEEP_SHOT_OWNED=file.json: on the accounts' own private servers, answered from that file (pages/owned.rs).
        #[cfg(feature = "shot")]
        if std::env::var("BLOXKEEP_SHOT_OWNED").is_ok() {
            self.games.view = pages::games::View::Servers;
        }
        // BLOXKEEP_SHOT_INBOX=file.json: on the inbox, answered from that file (roblox.rs); BLOXKEEP_SHOT_TAB=1|2 its tab.
        #[cfg(feature = "shot")]
        if std::env::var("BLOXKEEP_SHOT_INBOX").is_ok() {
            // (BLOXKEEP_SHOT_RSET=1: on the account's Settings instead, answered from the same file.)
            self.games.view = if std::env::var("BLOXKEEP_SHOT_RSET").is_ok() { pages::games::View::Settings } else { pages::games::View::Inbox };
            self.inbox.tab = std::env::var("BLOXKEEP_SHOT_TAB").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
        }
        #[cfg(feature = "shot")]
        if let Ok(kind) = std::env::var("BLOXKEEP_SHOT_SHOP") {
            self.games.view = pages::games::View::Shop;
            // BLOXKEEP_SHOT_SHOP=free: on its New free list; =watch: on Watching.
            self.profile.free = kind == "free";
            if kind == "watch" && !self.profile.watch_view {
                self.profile.watch_view = true;
                self.profile.watch = crate::itemwatch::list();
            }
        }
        // BLOXKEEP_SHOT_LIVE=friend|item|mod|done: the capsule open for a made-up moment; with BLOXKEEP_SHOT_LIVE_SHUT=a-b
        // (ms) it opens a ms after the first drawing and ends at b (a clip of it opening and closing).
        #[cfg(feature = "shot")]
        if let Ok(kind) = std::env::var("BLOXKEEP_SHOT_LIVE") {
            static ONCE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
            let when = std::env::var("BLOXKEEP_SHOT_LIVE_SHUT").ok().and_then(|v| {
                let (a, b) = v.split_once('-')?;
                Some((a.parse::<u64>().ok()?, b.parse::<u64>().ok()?))
            });
            match when {
                Some((open, shut)) if !ONCE.swap(true, std::sync::atomic::Ordering::Relaxed) => {
                    cx.spawn(async move |this, cx| {
                        cx.background_executor().timer(std::time::Duration::from_millis(open)).await;
                        let _ = this.update(cx, |r, cx| {
                            r.games.live = Some(shot_live(r, &kind));
                            cx.notify();
                        });
                        cx.background_executor().timer(std::time::Duration::from_millis(shut.saturating_sub(open))).await;
                        let _ = this.update(cx, |r, cx| {
                            r.games.live = None;
                            cx.notify();
                        });
                    })
                    .detach();
                }
                None if self.games.live.is_none() => self.games.live = Some(shot_live(self, &kind)),
                _ => {}
            }
        }
        // BLOXKEEP_SHOT_MODS=1|mine: on Roblox › Mods (its library, or the ones put in).
        #[cfg(feature = "shot")]
        if let Ok(kind) = std::env::var("BLOXKEEP_SHOT_MODS") {
            if self.games.view != pages::games::View::Mods {
                self.games.view = pages::games::View::Mods;
                self.mods.mine = kind == "mine";
            }
        }
        // BLOXKEEP_SHOT_FIND="anime": a shot starts with that search sent to Roblox.
        #[cfg(feature = "shot")]
        if let (Ok(q), true) = (std::env::var("BLOXKEEP_SHOT_FIND"), self.games.found.is_none()) {
            self.game_search.update(cx, |s, cx| s.set_value(q, window, cx));
            pages::games::search(self, cx);
        }
        // BLOXKEEP_SHOT_EASY=1: Easy setup's box open.
        #[cfg(feature = "shot")]
        if std::env::var("BLOXKEEP_SHOT_EASY").is_ok() {
            static OPENED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
            if !OPENED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                pages::easy_setup::open(self, window, cx);
            }
        }
        // BLOXKEEP_SHOT_SETQ="discord": Settings searched for those words.
        #[cfg(feature = "shot")]
        if let (Ok(q), true) = (std::env::var("BLOXKEEP_SHOT_SETQ"), self.set_q.read(cx).value().is_empty()) {
            self.set_q.update(cx, |s, cx| s.set_value(q, window, cx));
        }
        // BLOXKEEP_SHOT_SET=perf (a part's key): Settings open on it; "multi" asks Multi-Roblox's question, "restore" Restore's;
        // "rbx-update" asks Roblox's update question, "rc-own" shows Custom Roblox settings; "dc-link" opens Discord's
        // paste box, "dc-alerts"/"dc-messages"/"dc-control" its tabs, "dc-acc" its summary accounts to tick.
        #[cfg(feature = "shot")]
        if let Ok(v) = std::env::var("BLOXKEEP_SHOT_SET") {
            use pages::settings::Part;
            match v.as_str() {
                "multi" => self.settings.asking = Some("multi"),
                "restore" => {
                    self.settings.part = Part::Backup;
                    self.settings.asking = Some("restore");
                }
                "rbx-update" => {
                    self.settings.part = Part::Roblox;
                    self.settings.asking = Some("rbx-update");
                }
                "rc-own" => {
                    self.settings.part = Part::Perf;
                    self.settings.rc_custom = true;
                }
                // (The main account's tick list open, the first account ticked.)
                "main-pick" => {
                    self.settings.part = Part::Perf;
                    self.settings.main_pick = Some(self.accounts().iter().take(1).filter_map(|a| a.get("id").and_then(Value::as_str).map(str::to_string)).collect());
                }
                "dc-link" => {
                    self.settings.part = Part::Discord;
                    self.settings.dc_link.get_or_insert(false);
                }
                "dc-alerts" | "dc-messages" | "dc-control" => {
                    self.settings.part = Part::Discord;
                    self.settings.dc_tab = match v.as_str() {
                        "dc-alerts" => "alerts",
                        "dc-messages" => "messages",
                        _ => "control",
                    };
                }
                "dc-acc" => {
                    self.settings.part = Part::Discord;
                    if self.settings.dc_pick.is_none() {
                        self.settings.dc_pick = Some(("accounts", Default::default()));
                    }
                }
                k => self.settings.part = Part::from_key(k).unwrap_or_default(),
            }
            if self.settings.part == Part::Discord {
                pages::settings::ensure_discord(self, window, cx);
            }
            // BLOXKEEP_SHOT_SETSCROLL=<px>: the part scrolled down that far (to the bottom when it is shorter).
            if let Some(y) = std::env::var("BLOXKEEP_SHOT_SETSCROLL").ok().and_then(|v| v.parse::<f32>().ok()) {
                self.settings.scroll.set_offset(point(px(0.), px(-y)));
            }
        }
        // BLOXKEEP_SHOT_STATS=1|7|30 (the period) or "activity"; BLOXKEEP_SHOT_OPEN=1: the first account's timeline open.
        #[cfg(feature = "shot")]
        if let Ok(v) = std::env::var("BLOXKEEP_SHOT_STATS") {
            match v.parse::<i64>() {
                Ok(d) => self.stats.days = d,
                Err(_) => {
                    self.stats.tab = pages::stats::Tab::Activity;
                    self.stats.menu = v == "menu";
                }
            }
            if std::env::var("BLOXKEEP_SHOT_OPEN").is_ok() {
                self.stats.open = self.accounts().first().and_then(|a| a.get("id")).and_then(Value::as_str).map(str::to_string);
            }
        }
        #[cfg(feature = "shot")]
        if std::env::var("BLOXKEEP_SHOT_AUTO").as_deref() == Ok("macros") {
            self.auto.tab = pages::auto::Tab::Macros;
        }
        // BLOXKEEP_SHOT_MACRO=m3 (a macro's id) or "new": the macro form open; BLOXKEEP_SHOT_STEP=4: with that step
        // (counted from 1) open.
        #[cfg(feature = "shot")]
        if let (Ok(id), true) = (std::env::var("BLOXKEEP_SHOT_MACRO"), self.auto.macro_form.is_none() && !self.accounts().is_empty()) {
            let m = self.state.pointer("/settings/macros").and_then(Value::as_array).and_then(|l| l.iter().find(|m| m.get("id").and_then(Value::as_str) == Some(id.as_str())).cloned());
            pages::macro_form::open(self, m, window, cx);
            if let Ok(n) = std::env::var("BLOXKEEP_SHOT_STEP").map(|v| v.parse::<usize>().unwrap_or(1)) {
                pages::macro_form::shot_step(self, n.saturating_sub(1), window, cx);
            }
            if let (Some(f), Ok(y)) = (self.auto.macro_form.as_mut(), std::env::var("BLOXKEEP_SHOT_SCROLL").map(|v| v.parse::<f32>().unwrap_or(0.))) {
                f.scroll.set_offset(point(px(0.), px(-y)));
            }
            pages::macro_form::shot_more(self, window, cx);
        }
        // BLOXKEEP_SHOT_PRESET=p1 (a preset's id) or "new": the preset form open.
        #[cfg(feature = "shot")]
        if let (Ok(id), true) = (std::env::var("BLOXKEEP_SHOT_PRESET"), self.auto.form.is_none() && !self.accounts().is_empty()) {
            let p = self.state.pointer("/settings/presets").and_then(Value::as_array).and_then(|l| l.iter().find(|p| p.get("id").and_then(Value::as_str) == Some(id.as_str())).cloned());
            pages::preset_form::open(self, p, window, cx);
            // BLOXKEEP_SHOT_PRESET_FIND="blox": the form's game box searched for those words.
            if let Ok(q) = std::env::var("BLOXKEEP_SHOT_PRESET_FIND") {
                pages::preset_form::shot_find(self, q, cx);
            }
            // BLOXKEEP_SHOT_PRESET_GAME="https://www.roblox.com/games/2753915549": the form's game box looked up.
            if let Ok(v) = std::env::var("BLOXKEEP_SHOT_PRESET_GAME") {
                pages::preset_form::shot_resolve(self, v, cx);
            }
            // BLOXKEEP_SHOT_SCROLL=900: the form scrolled down that far, with its folds open.
            if let (Some(f), Ok(y)) = (self.auto.form.as_mut(), std::env::var("BLOXKEEP_SHOT_SCROLL").map(|v| v.parse::<f32>().unwrap_or(0.))) {
                f.open_folds();
                f.scroll.set_offset(point(px(0.), px(-y)));
            }
        }
        // BLOXKEEP_SHOT_CLIP=place: the mouse rests on that game's tile (its gameplay clip plays).
        #[cfg(feature = "shot")]
        if let Ok(place) = std::env::var("BLOXKEEP_SHOT_CLIP").map(|p| p.parse::<i64>().unwrap_or(0)) {
            if pages::clips::resting(self) != Some(place) {
                pages::clips::hover(self, place, true, cx);
            }
        }
        // BLOXKEEP_SHOT_TOAST="words": a notice set the way a page sets one (straight into `toast`, no timer of its
        // own), once: a shot before and one after 5 s show that it goes away by itself (toast_layer's timer).
        #[cfg(feature = "shot")]
        {
            static SAID: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
            if let Ok(words) = std::env::var("BLOXKEEP_SHOT_TOAST")
                && !SAID.swap(true, std::sync::atomic::Ordering::Relaxed)
            {
                self.toast = Some((words, "ok".into()));
            }
        }
        // BLOXKEEP_SHOT_KEEP_ALL=1: a shot of the Games tab with Keep playing showing every game.
        #[cfg(feature = "shot")]
        if std::env::var("BLOXKEEP_SHOT_KEEP_ALL").is_ok() {
            self.games.keep_all = true;
        }
        // BLOXKEEP_SHOT_ACC_WIN=1: Accounts in its Game monitor view; BLOXKEEP_SHOT_ACC_PLAYING=1: the Playing filter on.
        #[cfg(feature = "shot")]
        {
            if std::env::var("BLOXKEEP_SHOT_ACC_WIN").is_ok() {
                self.acc_win = true;
            }
            if std::env::var("BLOXKEEP_SHOT_ACC_PLAYING").is_ok() {
                self.acc_filter = pages::accounts::Filter::Playing;
            }
        }
        // BLOXKEEP_SHOT_GAME="blox-fruits" (or a place id), BLOXKEEP_SHOT_TAB="news|prices": a shot opens that game's page.
        #[cfg(feature = "shot")]
        if let (Ok(id), true) = (std::env::var("BLOXKEEP_SHOT_GAME"), self.games.open.is_none()) {
            let place = id.parse::<i64>().unwrap_or(0);
            self.games.open = Some(pages::games::Open { id: if place > 0 { format!("place:{place}") } else { id.clone() }, place, name: id });
            self.games.tab = match std::env::var("BLOXKEEP_SHOT_TAB").as_deref() {
                Ok("news") => pages::games::Tab::News,
                Ok("prices") => pages::games::Tab::Prices,
                Ok("wiki") => pages::games::Tab::Wiki,
                _ => pages::games::Tab::Overview,
            };
            // BLOXKEEP_SHOT_PLAY=1: with Play's row open, the first two eligible accounts ticked.
            if std::env::var("BLOXKEEP_SHOT_PLAY").is_ok() {
                let open_id = self.games.open.as_ref().map(|o| o.id.clone()).unwrap_or_default();
                let ticked = self
                    .accounts()
                    .iter()
                    .filter(|a| pages::accounts::can_join_game(a))
                    .filter_map(|a| a.get("id").and_then(Value::as_str).map(str::to_string))
                    .take(2)
                    .collect();
                self.games.picking = Some((open_id, ticked));
            }
            // BLOXKEEP_SHOT_REGION=JP: that country picked and the country menu beside Play open.
            if let Ok(cc) = std::env::var("BLOXKEEP_SHOT_REGION") {
                crate::prefs::set("serverRegion", json!(cc));
                self.games.region_menu = true;
                pages::games::load_regions(self, place, cx);
            }
            if std::env::var("BLOXKEEP_SHOT_PRIVACY").is_ok() {
                pages::game_privacy::shot_open(self, place, cx);
            }
        }
        cx.notify();
    }

    /// An Accounts key (Ctrl+A, Delete, F2, Enter, Escape): only on that page, with nothing typed into a box and no
    /// dialog open; Enter only when the page itself has the focus (on a button or a card it presses that).
    /// Not while a button or another control has the focus (app.js skipped them when e.target.closest('button')):
    /// Space presses that, the arrows are its own.
    fn card_key(r: &mut Self, k: pages::accounts::CardKey, window: &mut Window, cx: &mut Context<Self>) {
        let on_page = window.focused(cx).is_none_or(|f| f == r.focus);
        let fits = r.page == Page::Accounts && !r.locked() && !window.has_active_dialog(cx) && window.focused_input(cx).is_none() && on_page;
        if !(fits && pages::accounts::card_key(r, k, cx)) {
            cx.propagate();
        }
    }

    fn acc_key(r: &mut Self, k: pages::accounts::Key, window: &mut Window, cx: &mut Context<Self>) {
        let typing = window.focused_input(cx).is_some() || window.context_stack().iter().any(|c| c.contains("Input"));
        let on_page = window.focused(cx).is_none_or(|f| f == r.focus);
        let fits = r.page == Page::Accounts && !r.locked() && !typing && !window.has_active_dialog(cx) && (on_page || !matches!(k, pages::accounts::Key::Play));
        if !(fits && pages::accounts::key(r, k, window, cx)) {
            cx.propagate();
        }
    }

    /// Opens a page and remembers the one it leaves for Back.
    /// The picked ones that are gone (an account deleted, another program's window closed) are dropped: else Esc was
    /// taken by a selection nothing on the page shows.
    fn prune_selected(&mut self) {
        if self.selected.is_empty() {
            return;
        }
        let ids: std::collections::HashSet<String> = self
            .accounts()
            .iter()
            .filter_map(|a| a.get("id").and_then(Value::as_str).map(str::to_string))
            // (Another program's window goes by "win-" and its id, as accounts::others names it.)
            .chain(self.windows.iter().map(|w| format!("win-{}", w.get("id").and_then(Value::as_str).unwrap_or_default())))
            .collect();
        self.selected.retain(|id| ids.contains(id));
    }

    pub fn go(&mut self, p: Page) {
        if p != self.page {
            if self.page == Page::Accounts {
                pages::picker::close(self);
            }
            // (A place further down the rail comes in from below, one further up from above.)
            let at = |x: Page| Page::ALL.iter().position(|y| *y == x).unwrap_or(0);
            self.step_from = if at(p) >= at(self.page) { 1. } else { -1. };
            self.back.push(self.page);
            // (The newest 50 kept: truncate would drop the one just pushed.)
            if self.back.len() > 50 {
                self.back.drain(..self.back.len() - 50);
            }
            self.fwd.clear();
            self.page = p;
            self.nav += 1;
        }
    }

    fn step(&mut self, forward: bool) {
        let (from, to) = if forward { (&mut self.fwd, &mut self.back) } else { (&mut self.back, &mut self.fwd) };
        if let Some(p) = from.pop() {
            to.push(self.page);
            self.page = p;
            self.nav += 1;
        }
    }

    /// The place the page was on before Bloxkeep restarted (keepView).
    fn resume(&mut self, view: &Value) {
        if let Some(key) = view.get("tab").and_then(Value::as_str) {
            if let Some(p) = Page::from_key(key) {
                self.page = p;
            }
        }
    }

    fn on_event(&mut self, name: &str, data: Value, window: &mut Window, cx: &mut Context<Self>) {
        match name {
            "state" => self.set_state(data, window, cx),
            "tray" => crate::tray::show(self, &data, cx),
            "toolPage" => crate::toolpage::on_event(&mut self.tool_pages, &self.i18n, &data, cx),
            "macroRecorded" | "macroSeen" | "onGameEdit" => pages::macro_form::on_event(self, name, &data, window, cx),
            "metrics" => {
                self.metrics = data;
                cx.notify();
            }
            "feedCards" => {
                self.games.cards = data.as_array().cloned();
                cx.notify();
            }
            "monitor" => {
                self.windows = data.get("tiles").and_then(Value::as_array).cloned().unwrap_or_default();
                self.windows_gen += 1;
                self.prune_selected();
                cx.notify();
            }
            "progress" => {
                self.progress = data.get("text").and_then(Value::as_str).map(str::to_string);
                cx.notify();
            }
            "toast" => {
                let text = data.get("text").and_then(Value::as_str).unwrap_or_default().to_string();
                let kind = data.get("kind").and_then(Value::as_str).unwrap_or("info").to_string();
                self.toast_core = Some(text.clone());
                self.toast = Some((text.clone(), kind));
                self.toast_n += 1;
                cx.notify();
                self.toast_task = Some(cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(Duration::from_secs(5)).await;
                    let _ = this.update(cx, |r, cx| {
                        if r.toast.as_ref().is_some_and(|(t, _)| *t == text) {
                            r.toast = None;
                            cx.notify();
                        }
                    });
                }));
            }
            // Multi-Roblox turned on from the tray: Settings › Launching with its question, as its switch there asks
            // (or on at once when the person ticked not to ask again).
            "askMulti" => {
                let skip = self.state.get("skipConfirm").and_then(Value::as_array).is_some_and(|l| l.iter().any(|k| k.as_str() == Some("multi")));
                if skip {
                    self.send("setSettings", json!({ "multiInstance": true }));
                } else {
                    self.go(Page::Settings);
                    self.settings.part = pages::settings::Part::Launch;
                    self.settings.asking = Some("multi");
                    self.settings.no_ask = false;
                }
                cx.notify();
            }
            // Bloxkeep is about to install an update and start again: the new window comes back on this place.
            "saveView" => {
                let view = json!({ "tab": self.page.key() }).to_string();
                self.send("keepView", json!({ "view": view, "restart": true }));
            }
            // The app closed the line (it quit, or crashed): nothing here can work without it.
            "linkClosed" => {
                self.lost = true;
                cx.notify();
                cx.spawn(async move |_, cx| {
                    cx.background_executor().timer(Duration::from_millis(400)).await;
                    let _ = cx.update(|cx| cx.quit());
                })
                .detach();
            }
            _ => {}
        }
    }

    /// A command to the app whose answer the frame doesn't wait for.
    pub fn send(&self, cmd: &str, args: Value) {
        if let Some(l) = &self.link {
            l.send(cmd, args);
        }
    }

    pub fn tr(&self, key: &str) -> String {
        self.i18n.t(key)
    }

    /// Home's data the state doesn't carry: the games played lately, then the pictures of every game it shows.
    fn want_home_data(&mut self, cx: &mut Context<Self>) {
        let Some(link) = self.link.clone() else { return };
        if self.page == Page::Auto && !self.state.is_null() && !self.locked() {
            pages::auto::want(self);
        }
        if self.page == Page::Stats && !self.state.is_null() && !self.locked() {
            pages::stats::want(self, cx);
        }
        if self.page == Page::Home && !self.state.is_null() && !self.locked() {
            pages::stats::want_home(self, cx);
        }
        // Games: whose the first tab is (the app's pick) is asked at once, so its tab can say.
        if self.page == Page::Games && !self.state.is_null() && !self.locked() {
            pages::friends::want_sign(self, &link, cx);
            pages::tabs::pulse(self, cx);
            pages::vng::want(self, &link, cx);
            pages::games::want_corner(self, cx);
        }
        // (The Game view shows the friends' faces too, as Roblox's home does.)
        if self.page == Page::Games && matches!(self.games.view, pages::games::View::Friends | pages::games::View::Games) && !self.state.is_null() && !self.locked() {
            pages::friends::want(self, &link, cx);
        }
        if self.page == Page::Games && self.games.view == pages::games::View::Avatar && !self.state.is_null() && !self.locked() {
            pages::avatar::want(self, &link, cx);
        }
        // A game's passes, when that tab of its page is shown.
        // (An open game's page shows whatever part of Games was last picked, so that isn't asked about.)
        if self.page == Page::Games && self.games.tab == pages::games::Tab::Passes && !self.state.is_null() && !self.locked() {
            if let Some(place) = pages::games::shown_place(self) {
                pages::passes::want(self, place, cx);
            }
        }
        // Its private servers, likewise.
        if self.page == Page::Games && self.games.tab == pages::games::Tab::Servers && !self.state.is_null() && !self.locked() {
            if let Some(place) = pages::games::shown_place(self) {
                pages::servers::want(self, place, cx);
            }
        }
        if self.page == Page::Games && !self.state.is_null() && !self.locked() {
            match self.games.view {
                pages::games::View::Profile => pages::profile::want(self, &link, cx),
                pages::games::View::Shop => pages::profile::want_shop(self, cx),
                pages::games::View::Inbox => pages::inbox::want(self, &link, cx),
                pages::games::View::Settings => pages::rsettings::want(self, &link, cx),
                pages::games::View::Mods => pages::mods::want(self, cx),
                pages::games::View::Servers => pages::owned::want(self, &link, cx),
                _ => {}
            }
        }
        // (Every page lays a game's picture under its top now, 2026-11-13: the games and their pictures are asked on
        // any page, once each. Statistics opened first showed none.)
        if self.state.is_null() || self.locked() {
            return;
        }
        pages::games::want_wikis(self, cx);
        if self.page == Page::Games {
            pages::games::want(self, cx);
        } else {
            // (News' New marks count from the read before this visit: a visit ends when the page is left.)
            self.games.mark = None;
        }
        if matches!(self.page, Page::Home | Page::Games) {
            self.want_play_history(&link, cx);
        }
        // The stage's game's words and figures from Roblox, for Home's hero (its description, players, votes).
        if self.page == Page::Home
            && let Some(place) = crate::pages::home::hero_place(self)
        {
            pages::games::want_details(self, &link, place, cx);
        }
        let home_games = if self.page == Page::Games && self.games.home_library == Some(HomeTab::Games) {
            crate::pages::home::game_library(self, true)
        } else {
            crate::pages::home::games(self)
        };
        let mut places: Vec<i64> = home_games.iter().map(|g| g.place).collect();
        places.extend(self.accounts().iter().filter_map(|a| a.pointer("/session/game/placeId").and_then(Value::as_i64)));
        places.extend(
            self.state.pointer("/community/data/games").and_then(Value::as_array).into_iter().flatten().filter_map(|g| g.get("place").and_then(Value::as_i64)),
        );
        self.want_looks(places, cx);
    }

    fn clear_play_history(&mut self) {
        self.played = None;
        self.played_at = None;
        self.played_pending = false;
        self.played_gen = self.played_gen.wrapping_add(1);
    }

    fn want_play_history(&mut self, link: &crate::link::Link, cx: &mut Context<Self>) {
        let revision = self.metrics["playRevision"].as_u64().unwrap_or_default();
        if self.played_pending || self.played_at.is_some_and(|(at, last_revision)| {
            last_revision == revision && at.elapsed() < Duration::from_secs(30)
        }) {
            return;
        }
        self.played_pending = true;
        self.played_at = Some((std::time::Instant::now(), revision));
        let generation = self.played_gen;
        let rx = link.call("playSessions", json!({}));
        cx.spawn(async move |this, cx| {
            let got = rx.await.ok().and_then(Result::ok).filter(Value::is_object);
            let _ = this.update(cx, |r, cx| {
                if r.played_gen != generation || r.locked() {
                    return;
                }
                r.played_pending = false;
                if let Some(raw) = got {
                    let (sessions, known, now) = crate::history::read(&raw);
                    r.played = Some(crate::history::recently_played(&sessions, &known, now).into_iter()
                        .map(|game| json!({ "name": game.name, "place": game.place, "hours": game.hours, "last": game.last }))
                        .collect());
                    pages::games::set_play_history(r, raw);
                }
                cx.notify();
            });
        }).detach();
    }

    /// The pictures of these games, each asked once: read from Roblox here (roblox::looks; it was the C# app's
    /// gameLooks command until 2026-11-11).
    pub fn want_looks(&mut self, mut places: Vec<i64>, cx: &mut Context<Self>) {
        places.retain(|p| *p > 0 && !self.asked_looks.contains(p));
        places.sort_unstable();
        places.dedup();
        if places.is_empty() {
            return;
        }
        self.asked_looks.extend(places.iter().copied());
        let asked = places.clone();
        let rx = crate::roblox::run(move || crate::roblox::looks(&places));
        cx.spawn(async move |this, cx| {
            let got = rx.await.unwrap_or_default();
            // (One not answered may be asked again after a while: a passing failure isn't kept for the session.)
            let missed: Vec<i64> = asked.into_iter().filter(|p| !got.contains_key(p)).collect();
            let _ = this.update(cx, |r, cx| {
                r.looks.extend(got);
                cx.notify();
            });
            if !missed.is_empty() {
                cx.background_executor().timer(pages::games::RETRY).await;
                let _ = this.update(cx, |r, cx| {
                    for p in &missed {
                        let tries = r.look_misses.entry(*p).or_insert(0);
                        *tries += 1;
                        if *tries < 3 {
                            r.asked_looks.remove(p);
                        }
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// Puts dropped accounts back in their game, as app.js's rejoinOne does for each (the server they were on is gone):
    /// the game it was in (its server's place, its session's game, else the game Roblox says it is in), else the game in
    /// the Accounts bar's box, else Roblox's home. One at a time: the core takes one launch at once and refuses the
    /// others ("Already launching").
    pub fn rejoin(&mut self, accounts: &[Value], cx: &mut Context<Self>) {
        let dock = self.game.as_ref().filter(|g| g.get("ok").and_then(Value::as_bool) == Some(true)).and_then(|g| g.get("placeId")).and_then(Value::as_i64).filter(|p| *p > 0);
        let jobs: Vec<Value> = accounts
            .iter()
            .filter_map(|a| {
                let id = a.get("id").and_then(Value::as_str)?;
                let presence = (a.pointer("/presence/type").and_then(Value::as_str) == Some("ingame")).then(|| a.pointer("/presence/placeId").and_then(Value::as_i64)).flatten();
                let place = a
                    .pointer("/session/jobPlaceId")
                    .and_then(Value::as_i64)
                    .filter(|p| *p > 0)
                    .or_else(|| a.pointer("/session/game/placeId").and_then(Value::as_i64).filter(|p| *p > 0))
                    .or(presence.filter(|p| *p > 0))
                    .or(dock);
                Some(match place {
                    Some(p) => json!({ "ids": [id], "appOnly": false, "placeInput": format!("https://www.roblox.com/games/{p}"), "jobId": "" }),
                    None => json!({ "ids": [id], "appOnly": true, "placeInput": "", "jobId": "" }),
                })
            })
            .collect();
        self.rejoin_next(jobs, cx);
    }

    fn rejoin_next(&mut self, mut jobs: Vec<Value>, cx: &mut Context<Self>) {
        if jobs.is_empty() {
            return;
        }
        let args = jobs.remove(0);
        self.rejoin_run(args, false, jobs, cx);
    }

    /// One rejoin sent (forced: after "Open anyway"), then the rest. A question first ("Before you open"): the rest
    /// wait for its answer, and go on after it whatever it is (the box keeps them: Pending::LaunchAsk's rest).
    fn rejoin_run(&mut self, mut args: Value, force: bool, rest: Vec<Value>, cx: &mut Context<Self>) {
        let Some(link) = self.link.clone() else { return };
        args["force"] = force.into();
        let rx = link.call("launch", args.clone());
        answer(rx, cx, move |r, answer, cx| {
            let rest = match answer {
                Ok(Ok(v)) => r.launch_done("launch", args, v, rest),
                // (Refused: the rest are left, as before.)
                Ok(Err(e)) => {
                    r.toast = Some((e.to_string(), "error".into()));
                    Vec::new()
                }
                Err(_) => Vec::new(),
            };
            r.rejoin_next(rest, cx);
            cx.notify();
        });
    }

    /// What's new, for these releases (newest first): opened once there is a window to open it in.
    pub fn show_whats_new(&mut self, releases: Vec<Value>) {
        self.wn_shown = true;
        self.pending.push(Pending::WhatsNew(releases));
    }

    /// Add account: the sign-in screen (opened by render: Pending::Add).
    pub fn add_account(&mut self, cx: &mut Context<Self>) {
        self.pending.push(Pending::Add);
        cx.notify();
    }

    /// Roblox's sign-in window (app.js's addBrowser); it can add several in a row.
    pub fn add_browser(&mut self, cx: &mut Context<Self>) {
        self.add_browser_as(false, cx)
    }

    /// The same window on Roblox's sign-up form (create): each account made there is saved here.
    pub fn add_browser_as(&mut self, create: bool, cx: &mut Context<Self>) {
        let Some(link) = self.link.clone() else { return };
        let rx = link.call("addViaBrowser", if create { json!({ "create": true }) } else { json!({}) });
        let vault = self.state.pointer("/vaults/open").cloned();
        cx.spawn(async move |this, cx| {
            let Some(r) = rx.await.ok().and_then(Result::ok) else { return };
            let added = r.get("added").and_then(Value::as_array).cloned().unwrap_or_default();
            if added.is_empty() {
                return;
            }
            let _ = this.update(cx, |r, cx| {
                let text = if added.len() == 1 {
                    let name = added[0].get("label").and_then(Value::as_str).unwrap_or_default();
                    let key = if added[0].get("merged").and_then(Value::as_bool).unwrap_or(false) { "t.merged" } else { "t.added" };
                    r.i18n.tv(key, &[("name", name)])
                } else {
                    r.i18n.tv("t.addedMany", &[("n", &added.len().to_string())])
                };
                r.toast = Some((text, "ok".into()));
                // (Locked, or another vault opened while the browser was up: those ids aren't on the page.)
                if r.locked() || r.state.pointer("/vaults/open").cloned() != vault {
                    cx.notify();
                    return;
                }
                // The ones added are picked, ready to play (app.js's addBrowser).
                let ids: Vec<String> = added.iter().filter_map(|a| a.get("id").and_then(Value::as_str)).map(str::to_string).collect();
                r.acc_anchor = ids.last().cloned();
                r.selected = ids.into_iter().collect();
                cx.notify();
            });
        })
        .detach();
    }

    /// The app's question in a box over the window, as app.js's confirmBox: Cancel and the answer (red when it destroys
    /// something), and "Don't ask again" for the questions the app lets you skip (its SkippableConfirms), whose yes is
    /// then assumed. The box, its cover and how they come and go are GPUI Component's Dialog; the buttons are the kit's.
    /// Runs a macro on its accounts (or on these), as app.js's runMacro: on how many windows, roughly how long one turn
    /// over them takes (the app pushes it with the run, maybe just after the answer), and, when its steps need the
    /// window the whole time, that it takes one account at a time (so a queue isn't taken for a fault).
    pub fn run_macro(&mut self, id: String, ids: Option<Vec<String>>, cx: &mut Context<Self>) {
        let Some(link) = self.link.clone() else { return };
        let args = match &ids {
            Some(ids) => json!({ "id": id, "accountIds": ids }),
            None => json!({ "id": id }),
        };
        let rx = link.call("runMacro", args);
        cx.spawn(async move |this, cx| {
            let answer = rx.await;
            cx.background_executor().timer(std::time::Duration::from_millis(60)).await;
            let _ = this.update(cx, |r, cx| {
                match answer {
                    Ok(Ok(v)) => {
                        let n = v.get("queued").and_then(Value::as_i64).unwrap_or(0);
                        if n == 0 {
                            r.toast = Some((r.tr("macro.queuedNone"), "info".into()));
                        } else {
                            let mut text = r.i18n.tn("macro.queued", n, &[]);
                            if let Some(sweep) = r.state.pointer("/macroSweep/seconds").and_then(Value::as_f64).filter(|s| *s > 0.) {
                                let d = if sweep < 60. {
                                    r.i18n.tv("macro.dur.s", &[("n", &(sweep.round() as i64).max(1).to_string())])
                                } else {
                                    r.i18n.tv("macro.dur.min", &[("n", &((sweep / 60.).round() as i64).to_string())])
                                };
                                text = format!("{text} {}", r.i18n.tv("macro.sweep", &[("d", &d)]));
                            }
                            let constant = r
                                .state
                                .pointer("/settings/macros")
                                .and_then(Value::as_array)
                                .and_then(|l| l.iter().find(|m| m.get("id").and_then(Value::as_str) == Some(id.as_str())))
                                .and_then(|m| m.get("steps"))
                                .and_then(Value::as_array)
                                .is_some_and(|steps| {
                                    steps.iter().any(|st| {
                                        let kind = st.get("kind").and_then(Value::as_str).unwrap_or_default();
                                        kind == "follow" || (kind == "clickColour" && st.get("repeatUntilGoneMs").and_then(Value::as_i64).unwrap_or(0) > 0)
                                    })
                                });
                            if constant && n > 1 {
                                text = format!("{text} {}", r.tr("macro.oneAtATime"));
                            }
                            r.toast = Some((text, "ok".into()));
                        }
                    }
                    Ok(Err(e)) => r.toast = Some((e, "bad".into())),
                    Err(_) => {}
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Lock: Roblox windows stay open, but auto-rejoin, scheduled presets and Discord pause until unlocked
    /// (AppController.Lock); when any of that is in use it says so first, with "Don't ask again" (app.js's lockApp).
    pub fn lock_app(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut paused: Vec<String> = Vec::new();
        let rejoining = self
            .accounts()
            .iter()
            .filter(|a| {
                let local = a.get("state").and_then(Value::as_str) == Some("running");
                let on = a.pointer("/session/autoRejoin").and_then(Value::as_bool).or_else(|| a.get("autoRejoin").and_then(Value::as_bool)).unwrap_or(false);
                (local && on) || a.get("rejoin").is_some_and(|v| !v.is_null())
            })
            .count();
        if rejoining > 0 {
            paused.push(self.i18n.tn("lockq.rejoin", rejoining as i64, &[("n", &rejoining.to_string())]));
        }
        let scheduled = self
            .state
            .pointer("/settings/presets")
            .and_then(Value::as_array)
            .map_or(0, |l| l.iter().filter(|p| p.pointer("/schedule/enabled").and_then(Value::as_bool) == Some(true)).count());
        if scheduled > 0 {
            paused.push(self.i18n.tn("lockq.schedule", scheduled as i64, &[("n", &scheduled.to_string())]));
        }
        if self.state.pointer("/discord/enabled").and_then(Value::as_bool) == Some(true) && self.state.pointer("/discord/hasWebhook").and_then(Value::as_bool) == Some(true) {
            paused.push(self.tr("lockq.discord"));
        }
        if paused.is_empty() {
            self.send("lock", Value::Null);
            return;
        }
        let text = format!("{}\n{}", self.tr("lockq.text"), paused.iter().map(|p| format!("• {p}")).collect::<Vec<_>>().join("\n"));
        self.confirm(self.tr("lockq.title"), text, self.tr("side.lock"), false, Some("lock"), window, cx, |r, _| r.send("lock", Value::Null));
    }

    #[allow(clippy::too_many_arguments)]
    pub fn confirm(
        &mut self,
        title: String,
        text: String,
        ok: String,
        danger: bool,
        remember: Option<&'static str>,
        window: &mut Window,
        cx: &mut Context<Self>,
        yes: impl Fn(&mut Root, &mut Context<Root>) + 'static,
    ) {
        let skipped = |k: &str| self.state.get("skipConfirm").and_then(Value::as_array).is_some_and(|l| l.iter().any(|v| v.as_str() == Some(k)));
        if remember.is_some_and(skipped) {
            yes(self, cx);
            return;
        }
        let this = cx.weak_entity();
        let yes = std::rc::Rc::new(yes);
        let (cancel, again) = (self.tr("common.cancel"), self.tr("common.dontAsk"));
        let tick = std::rc::Rc::new(std::cell::Cell::new(false));
        window.open_dialog(cx, move |dialog, _, _| {
            let (this, yes, tick, ticked) = (this.clone(), yes.clone(), tick.clone(), tick.clone());
            let body = div()
                .flex()
                .flex_col()
                .gap(px(14.))
                .child(div().text_size(px(T_M)).line_height(px(20.)).text_color(c(TEXT_2)).child(text.clone()))
                .when(remember.is_some(), |d| {
                    d.child(div().w_full().flex().child(Check::new("ask-again", ticked.get()).label(again.clone()).on_toggle(move |on, w, _| {
                        ticked.set(*on);
                        w.refresh();
                    })))
                });
            let answer = Button::new("ask-yes", ok.clone());
            let answer = if danger { answer.danger() } else { answer.primary() };
            // (The web page's dialog frame: the title and its × over a hairline, the answers under another.)
            crate::kit::framed(dialog, title.clone(), cancel.clone(), |_, w, cx| w.close_dialog(cx))
                .w(px(440.))
                .centered()
                .entrance(DialogEntrance::FadeSlide)
                .child(crate::kit::dialog_body(body))
                .footer(
                    crate::kit::dialog_foot()
                        .child(Button::new("ask-no", cancel.clone()).on_click(|_, w, cx| w.close_dialog(cx)))
                        .child(answer.on_click(move |_, w, cx| {
                            w.close_dialog(cx);
                            let _ = this.update(cx, |r, cx| {
                                if let (Some(key), true) = (remember, tick.get()) {
                                    r.send("skipConfirm", json!({ "key": key }));
                                }
                                yes(r, cx);
                                cx.notify();
                            });
                        })),
                )
        });
    }

    /// "Discard changes?" over a form closed with something typed in it (app.js's modal askClose): Cancel keeps the
    /// form; Discard runs `then`, which closes it.
    pub fn ask_discard(&mut self, window: &mut Window, cx: &mut Context<Self>, then: impl Fn(&mut Root, &mut Window, &mut Context<Root>) + 'static) {
        let this = cx.weak_entity();
        let then = std::rc::Rc::new(then);
        let (title, text, cancel, ok) = (self.tr("common.discardTitle"), self.tr("common.discardQ"), self.tr("common.cancel"), self.tr("common.discard"));
        window.open_dialog(cx, move |dialog, _, _| {
            let (this, then) = (this.clone(), then.clone());
            crate::kit::framed(dialog, title.clone(), cancel.clone(), |_, w, cx| w.close_dialog(cx))
                .w(px(420.))
                .centered()
                .entrance(DialogEntrance::FadeSlide)
                .child(crate::kit::dialog_body(div().text_size(px(T_M)).line_height(px(20.)).text_color(c(TEXT_2)).child(text.clone())))
                .footer(
                    crate::kit::dialog_foot()
                        .child(Button::new("discard-no", cancel.clone()).on_click(|_, w, cx| w.close_dialog(cx)))
                        .child(Button::new("discard-yes", ok.clone()).danger().on_click(move |_, w, cx| {
                            w.close_dialog(cx);
                            let _ = this.update(cx, |r, cx| {
                                then(r, w, cx);
                                cx.notify();
                            });
                        })),
                )
        });
    }

    /// Play, as app.js's launch: the game in the Accounts bar's box, or Roblox's home screen when it's empty. What the
    /// app asks back (Multi-Roblox off, a game ban…) comes with round 2's dialogs; until then its words are a toast.
    pub fn play(&mut self, ids: &[String], cx: &mut Context<Self>) {
        if ids.is_empty() {
            self.toast = Some((self.tr("t.selectFirst"), "warn".into()));
            self.toast_n += 1;
            cx.notify();
            return;
        }
        let place = self.place.read(cx).value().trim().to_string();
        if self.page == Page::Accounts && pages::picker::search_dock(self, true, cx) {
            return;
        }
        // The server in the Job ID box, as app.js's launch read #job for every Play (card, Enter, menus, palette).
        // (Not one that went with another game: a game typed and played before its lookup said so.)
        let job = crate::pages::accounts::job_for(&place, cx);
        if !job.is_empty() && !place.is_empty() {
            self.launch("launch", json!({ "ids": ids, "appOnly": false, "placeInput": place, "jobId": job }), false, cx);
            return;
        }
        self.play_at(ids, place, cx);
    }

    /// Play these accounts into a game (a link or a Place ID; empty: Roblox's home screen).
    pub fn play_at(&mut self, ids: &[String], place: String, cx: &mut Context<Self>) {
        self.launch("launch", json!({ "ids": ids, "appOnly": place.is_empty(), "placeInput": place, "jobId": "" }), false, cx);
    }

    /// Sends a launch (Play, a preset, a join) and deals with what the app says back, as app.js's launchDone: a
    /// warning that needs an answer (the box runs it again, forced or with Multi-Roblox on), the accounts that
    /// didn't open, and how many did.
    pub fn launch(&mut self, cmd: &'static str, mut args: Value, force: bool, cx: &mut Context<Self>) {
        let Some(link) = self.link.clone() else { return };
        // One at a time, as the web page had it: a double click asked "Open anyway?" twice and ran it twice.
        let busy: &'static str = if cmd == "runPreset" { "preset" } else { "launch" };
        if !self.in_flight.insert(busy) {
            return;
        }
        // A game with its own privacy (beside Play): set on the accounts first (not again for the forced second try).
        let privacy = if cmd == "launch" && !force { crate::pages::game_privacy::before_launch(&args) } else { None };
        args["force"] = force.into();
        let sent = args.clone();
        cx.spawn(async move |this, cx| {
            if let Some((ids, place, pick)) = privacy {
                let got = crate::pages::game_privacy::apply(link.clone(), ids, place, pick).await;
                let _ = this.update(cx, |r, _| crate::pages::game_privacy::applied(r, got));
            }
            let answer = link.call(cmd, sent).await;
            let _ = this.update(cx, |r, cx| {
                r.in_flight.remove(busy);
                match answer {
                    Ok(Ok(v)) => {
                        r.launch_done(cmd, args, v, Vec::new());
                    }
                    Ok(Err(e)) => r.toast = Some((e.to_string(), "error".into())),
                    Err(_) => {}
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// `rest`: the rejoins still to send after this one (Root::rejoin_run); given back to be sent now, or kept by the
    /// question this answer asks (then sent after its answer), or dropped when the app was locked meanwhile.
    fn launch_done(&mut self, cmd: &'static str, args: Value, answer: Value, rest: Vec<Value>) -> Vec<Value> {
        // (Locked meanwhile: nothing to ask or report.)
        if self.locked() {
            return Vec::new();
        }
        if !answer.is_object() {
            return rest;
        }
        let flag = |k: &str| answer.get(k).and_then(Value::as_bool).unwrap_or(false);
        let count = |k: &str| answer.get(k).and_then(Value::as_array).map_or(0, |l| l.len()) as i64;
        if flag("needsConfirm") {
            self.pending.push(Pending::LaunchAsk { cmd, args, answer, rest });
            return Vec::new();
        }
        let launched = answer.get("launched").and_then(Value::as_i64).unwrap_or(0);
        if count("errors") > 0 {
            self.pending.push(Pending::LaunchErrors(answer.clone()));
        }
        if launched > 0 {
            let key = if flag("cancelled") { "t.launchedStopped" } else { "t.launched" };
            self.toast = Some((self.i18n.tn(key, launched, &[("n", &launched.to_string())]), "ok".into()));
        } else if count("skipped") > 0 && !flag("cancelled") {
            // Accounts already open where asked were left alone: worth a note, not an error.
            let n = count("skipped");
            self.toast = Some((self.i18n.tn("t.skipped", n, &[("n", &n.to_string())]), "info".into()));
        }
        self.toast_n += 1;
        rest
    }

    /// A box that was waiting for a window (render opens them).
    fn open_pending(&mut self, p: Pending, window: &mut Window, cx: &mut Context<Self>) {
        match p {
            Pending::WhatsNew(list) => pages::extras::whats_new(self, list, window, cx),
            // Add account: the sign-in screen, with the other ways on it (the phone's code, Roblox's browser sign-in,
            // a cookie, a Bloxkeep file).
            Pending::Add => pages::password::open(self, window, cx),
            // "Before you open": Cancel, Fix Multi-Roblox (it isn't working), Open anyway, Turn on Multi-Roblox and
            // play (it is off). The questions the app lets you skip offer "Don't ask again" (app.js's launchAsk).
            // Rejoins waiting behind it (`rest`) go on after Cancel or once the forced or re-asked launch is answered;
            // the box then has no x, so it can't be put aside with them forgotten. Fix Multi-Roblox leaves them.
            Pending::LaunchAsk { cmd, args, answer, rest } => {
                let asks: Vec<String> = answer.get("asks").and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
                let can_skip = !asks.is_empty() && asks.iter().all(|k| ["multiOff", "watch", "memory"].contains(&k.as_str()));
                let (multi_off, not_ready) = (asks.iter().any(|k| k == "multiOff"), asks.iter().any(|k| k == "multiNotReady"));
                let queued = !rest.is_empty();
                let mut choices = vec![{
                    let rest = rest.clone();
                    Choice::new(self.tr("common.cancel"), 0, move |r, _, _, cx| r.rejoin_next(rest.clone(), cx))
                }];
                if not_ready {
                    choices.push(Choice::new(self.tr("t.fixMulti"), 0, |r, _, window, cx| r.multi_help(window, cx)));
                }
                {
                    let (args, asks, rest) = (args.clone(), asks.clone(), rest.clone());
                    choices.push(Choice::new(self.tr("t.openAnyway"), if multi_off || not_ready { 0 } else { 1 }, move |r, ticked, _, cx| {
                        if can_skip && ticked {
                            for k in &asks {
                                r.send("skipConfirm", json!({ "key": format!("launch.{k}") }));
                            }
                        }
                        if queued {
                            r.rejoin_run(args.clone(), true, rest.clone(), cx);
                        } else {
                            r.launch(cmd, args.clone(), true, cx);
                        }
                    }));
                }
                if multi_off {
                    // Turned on (after its own warning), the launch runs again with its other questions still asked.
                    choices.push(Choice::new(self.tr("t.multiAndPlay"), 1, move |r, _, window, cx| {
                        let (args, rest) = (args.clone(), rest.clone());
                        r.confirm(r.tr("set.multiConfirmTitle"), r.tr("set.multiConfirm"), r.tr("set.multiOk"), false, Some("multi"), window, cx, move |r, cx| {
                            let Some(link) = r.link.clone() else { return };
                            let rx = link.call("setSettings", json!({ "multiInstance": true }));
                            let (args, rest) = (args.clone(), rest.clone());
                            cx.spawn(async move |this, cx| {
                                if let Ok(Ok(_)) = rx.await {
                                    let _ = this.update(cx, |r, cx| if rest.is_empty() { r.launch(cmd, args, false, cx) } else { r.rejoin_run(args, false, rest, cx) });
                                }
                            })
                            .detach();
                        });
                    }));
                }
                let text = answer.get("message").and_then(Value::as_str).unwrap_or_default().to_string();
                let tick = can_skip.then(|| self.tr("common.dontAsk"));
                self.ask_box(self.tr("t.confirmTitle"), text, Vec::new(), tick, !queued, choices, window, cx);
            }
            // The accounts that didn't open, each with why; the ones Roblox wouldn't sign in can be opened on roblox.com.
            Pending::LaunchErrors(answer) => {
                let list: Vec<String> = answer.get("errors").and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
                let refused: Vec<String> = answer.get("refused").and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
                let some = answer.get("launched").and_then(Value::as_i64).unwrap_or(0) > 0;
                let mut choices = Vec::new();
                if !refused.is_empty() {
                    let n = refused.len() as i64;
                    choices.push(Choice::new(self.i18n.tn("t.openRefused", n, &[("n", &n.to_string())]), 0, move |r, _, _, _| {
                        for id in &refused {
                            r.send("openWebsite", json!({ "id": id }));
                        }
                    }));
                }
                choices.push(Choice::new(self.tr("common.close"), 1, |_, _, _, _| {}));
                self.ask_box(self.tr(if some { "t.errsSome" } else { "t.errsAll" }), String::new(), list, None, true, choices, window, cx);
            }
            // Allow or deny; it can't be put aside (closing it would be a no the asker never hears).
            Pending::Remote(ask) => {
                let user = ask.get("user").and_then(Value::as_str).unwrap_or_default().to_string();
                let pc = self.state.pointer("/remote/pc").and_then(Value::as_str).unwrap_or_default().to_string();
                let mut text = self.i18n.tv("dc.remoteAsk", &[("user", &user), ("pc", &pc)]);
                if let Some(old) = ask.get("replacing").and_then(Value::as_str).filter(|o| !o.is_empty()) {
                    text = format!("{text}\n\n{}", self.i18n.tv("dc.remoteAskReplaces", &[("user", old)]));
                }
                let id = ask.get("id").cloned().unwrap_or(Value::Null);
                let answer = |allow: bool| {
                    let id = id.clone();
                    move |r: &mut Root, _: bool, _: &mut Window, _: &mut Context<Root>| r.send("remoteAllow", json!({ "allow": allow, "id": id }))
                };
                let choices = vec![Choice::new(self.tr("dc.remoteDeny"), 0, answer(false)), Choice::new(self.tr("dc.remoteAllow"), 1, answer(true))];
                self.ask_box(self.tr("dc.remoteAskTitle"), text, Vec::new(), None, false, choices, window, cx);
            }
        }
    }

    /// A box with words, maybe a list, maybe "Don't ask again", and its answers in a row (app.js's modal with
    /// actions). Each answer closes the box and runs; one that may not be put aside has no other way out.
    #[allow(clippy::too_many_arguments)]
    pub fn ask_box(&mut self, title: String, text: String, list: Vec<String>, tick: Option<String>, closable: bool, choices: Vec<Choice>, window: &mut Window, cx: &mut Context<Self>) {
        let this = cx.weak_entity();
        let ticked = std::rc::Rc::new(std::cell::Cell::new(false));
        window.open_dialog(cx, move |dialog, _, cx| {
            let close_tip: SharedString = this.upgrade().map(|r| r.read(cx).tr("common.close")).unwrap_or_default().into();
            let mut body = div().flex().flex_col().gap(px(10.));
            for para in text.split('\n').filter(|p| !p.trim().is_empty()) {
                body = body.child(div().text_size(px(T_M)).line_height(px(20.)).text_color(c(TEXT_2)).child(para.to_string()));
            }
            if !list.is_empty() {
                let mut rows = div().max_h(px(300.)).id("ask-list").overflow_y_scroll().flex().flex_col().gap(px(6.));
                for line in &list {
                    rows = rows.child(
                        div()
                            .flex()
                            .gap(px(8.))
                            .text_size(px(T_M))
                            .line_height(px(19.))
                            .text_color(c(TEXT_2))
                            .child(div().mt(px(7.)).size(px(5.)).flex_none().rounded_full().bg(c(DIM)))
                            .child(div().min_w_0().flex_1().child(line.clone())),
                    );
                }
                body = body.child(rows);
            }
            if let Some(label) = tick.clone() {
                let set = ticked.clone();
                body = body.child(div().pt(px(4.)).flex().child(Check::new("ask-again", ticked.get()).label(label).on_toggle(move |on, w, _| {
                    set.set(*on);
                    w.refresh();
                })));
            }
            let mut foot = crate::kit::dialog_foot().flex_wrap();
            for (i, choice) in choices.iter().enumerate() {
                let (this, run, ticked) = (this.clone(), choice.run.clone(), ticked.clone());
                let b = Button::new(("ask-choice", i), choice.label.clone());
                let b = match choice.kind {
                    1 => b.primary(),
                    2 => b.danger(),
                    _ => b,
                };
                foot = foot.child(b.on_click(move |_, w, cx| {
                    w.close_dialog(cx);
                    let _ = this.update(cx, |r, cx| {
                        run(r, ticked.get(), w, cx);
                        cx.notify();
                    });
                }));
            }
            // In the web page's frame (head over a hairline, body, foot over a hairline); the × only when the box
            // may be closed without an answer.
            let dialog = if closable {
                crate::kit::framed(dialog, title.clone(), close_tip, |_, w, cx| w.close_dialog(cx))
            } else {
                dialog.close_button(false).p_0().gap_0().title(
                    div().w_full().px(px(24.)).pt(px(16.)).pb(px(12.)).border_b_1().border_color(c(LINE)).text_size(px(T_GROUP)).line_height(px(24.)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(title.clone()),
                )
            };
            dialog
                .w(px(crate::kit::DIALOG_W - 40.))
                .centered()
                .overlay_closable(closable)
                .keyboard(closable)
                .entrance(DialogEntrance::FadeSlide)
                .child(crate::kit::dialog_body(body))
                .footer(foot)
        });
    }

    /// With the online count off the app doesn't hear the community's figures by itself: they are asked for now and
    /// once a minute while a place that shows them is up (Community.tsx).
    fn community_tick(&mut self) {
        if self.locked() || self.state.get("countOnline").and_then(Value::as_bool) != Some(false) || !matches!(self.page, Page::Home | Page::Games | Page::Stats) {
            return;
        }
        if self.community_at.is_some_and(|t| t.elapsed() < Duration::from_secs(60)) {
            return;
        }
        self.community_at = Some(std::time::Instant::now());
        self.send("communityNow", json!({}));
    }

    /// Multi-Roblox's box (MultiHelp.tsx; app.js's multiHelp): off, what it does and Turn on; on and working, a tick;
    /// blocked (Roblox was open before it), the steps that clear it; and, when on, how the windows open.
    pub fn multi_help(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.help_open = true;
        let this = cx.weak_entity();
        let tray_done = std::rc::Rc::new(std::cell::Cell::new(false));
        window.open_dialog(cx, move |dialog, _, cx| {
            let Some(root) = this.upgrade() else { return dialog };
            let r = root.read(cx);
            let m = r.state.get("multi").cloned().unwrap_or(Value::Null);
            let on = |k: &str| m.get(k).and_then(Value::as_bool).unwrap_or(false);
            let (enabled, armed) = (on("enabled"), on("armed"));
            let text = |key: &str| div().text_size(px(T_M)).line_height(px(20.)).text_color(c(TEXT_2)).child(pages::lock::plain(r.tr(key)));
            let mut body = div().flex().flex_col().gap(px(14.));
            if !enabled {
                let this = this.clone();
                body = body.child(text("mh.off")).child(div().flex().child(Button::new("mh-enable", r.tr("mh.enable")).icon("layers").primary().on_click(move |_, w, cx| {
                    w.close_dialog(cx);
                    let _ = this.update(cx, |r, cx| {
                        r.confirm(r.tr("set.multiConfirmTitle"), r.tr("set.multiConfirm"), r.tr("set.multiOk"), false, Some("multi"), w, cx, |r, _| {
                            r.send("setSettings", json!({ "multiInstance": true }))
                        })
                    });
                })));
            } else if armed {
                body = body
                    .child(div().flex().justify_center().child(icon("check", 28., c(OK))))
                    .child(text("mh.ok1").text_center().text_color(c(TEXT)))
                    .child(text("mh.ok2").text_center().text_color(c(MUTED)));
            } else {
                let step = |n: usize, key: &str, button: Option<AnyElement>| {
                    div()
                        .flex()
                        .gap(px(10.))
                        .child(div().size(px(20.)).flex_none().rounded_full().bg(c(RAISED_2)).flex().items_center().justify_center().text_size(px(T_S)).font_weight(FontWeight::SEMIBOLD).child(n.to_string()))
                        .child(div().min_w_0().flex_1().flex().flex_col().items_start().gap(px(8.)).child(text(key)).children(button))
                };
                let close = {
                    let this = this.clone();
                    Button::new("mh-s1", r.tr("mh.s1btn")).icon("power").small().on_click(move |_, _, cx| {
                        let _ = this.update(cx, |r, cx| r.close_all_now(false, cx));
                    })
                };
                let mut steps = div().flex().flex_col().gap(px(12.)).child(step(1, "mh.s1", Some(close.into_any_element())));
                let mut n = 2;
                if on("trayAutostart") {
                    let (this, done) = (this.clone(), tray_done.clone());
                    let tray = Button::new("mh-s2", r.tr(if tray_done.get() { "mh.s2done" } else { "mh.s2btn" })).small().on_click(move |_, _, cx| {
                        if done.get() {
                            return;
                        }
                        let done = done.clone();
                        let _ = this.update(cx, |r, cx| {
                            let Some(link) = r.link.clone() else { return };
                            let rx = link.call("disableTrayAutostart", json!({}));
                            cx.spawn(async move |_, cx| {
                                if let Ok(Ok(_)) = rx.await {
                                    done.set(true);
                                    let _ = cx.update(|cx| cx.refresh_windows());
                                }
                            })
                            .detach();
                        });
                    });
                    steps = steps.child(step(n, "mh.s2", Some(tray.into_any_element())));
                    n += 1;
                }
                body = body.child(text("mh.bad")).child(steps.child(step(n, "mh.s3", None)));
            }
            if enabled {
                let flag = |id: &'static str, key: &str, setting: &'static str| {
                    let this = this.clone();
                    div()
                        .pt(px(12.))
                        .border_t_1()
                        .border_color(c(LINE))
                        .flex()
                        .items_center()
                        .gap(px(16.))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap(px(3.))
                                .child(div().text_size(px(T_M)).font_weight(FontWeight::MEDIUM).child(r.tr(key)))
                                .child(div().text_size(px(T_S)).line_height(px(16.)).text_color(c(MUTED)).child(r.tr(&format!("{key}Desc")))),
                        )
                        .child(Switch::new(id, on(setting.trim_start_matches("multi").to_lowercase().as_str())).on_toggle(move |v, _, cx| {
                            let v = *v;
                            let _ = this.update(cx, |r, _| r.send("setSettings", json!({ (setting): v })));
                        }))
                };
                body = body.child(flag("mh-windowed", "mh.windowed", "multiWindowed")).child(flag("mh-tile", "mh.tile", "multiTile"));
            }
            let mut foot = crate::kit::dialog_foot().justify_start();
            if enabled {
                let this = this.clone();
                foot = foot.child(Button::new("mh-disable", r.tr("mh.disable")).danger().on_click(move |_, w, cx| {
                    w.close_dialog(cx);
                    let _ = this.update(cx, |r, _| r.send("setSettings", json!({ "multiInstance": false })));
                }));
            }
            foot = foot.child(div().flex_1()).child(Button::new("mh-done", r.tr("common.done")).primary().on_click(|_, w, cx| w.close_dialog(cx)));
            // In the dialogs' frame: its head and × over a hairline, the body, the foot over a hairline.
            crate::kit::framed(dialog, r.tr("multi.title"), r.tr("common.close"), |_, w, cx| w.close_dialog(cx))
                .w(px(480.))
                .centered()
                .entrance(DialogEntrance::FadeSlide)
                .child(crate::kit::dialog_body(body.pt(px(8.))))
                .footer(foot)
        });
    }

    /// Closes every Roblox window now and says how many (the question, where one is asked, came before).
    /// Multi-Roblox is on but Roblox got there first: why, and the button that fixes it, where it is turned on (Settings,
    /// the lock screen's "Just use Multi-Roblox") rather than only behind Details, which few opened (the owner,
    /// 2026-11-19: many didn't know every Roblox has to be closed first).
    pub fn multi_blocked_note(&self, cx: &mut Context<Self>) -> Div {
        let text = |key: &str| div().text_size(px(T_M)).line_height(px(19.)).text_color(c(TEXT_2)).child(pages::lock::plain(self.tr(key)));
        div()
            .flex()
            .items_center()
            .gap(px(12.))
            .child(icon("alert", 16., c(WARN)))
            .child(div().min_w_0().flex_1().flex().flex_col().gap(px(2.)).child(text("mh.bad").text_color(c(TEXT))).child(text("mh.s1")))
            .child(
                Button::new("multi-close-all", self.tr("mh.s1btn"))
                    .icon("power")
                    .small()
                    .on_click(cx.listener(|r, _, _, cx| r.close_all_now(false, cx))),
            )
    }

    fn close_all_now(&mut self, force: bool, cx: &mut Context<Self>) {
        let Some(link) = self.link.clone() else { return };
        let rx = link.call("closeAll", json!({ "force": force }));
        answer(rx, cx, move |r, answer, cx| {
            r.toast = Some(match answer {
                Ok(Ok(n)) => match n.as_i64().unwrap_or(0) {
                    0 => (r.tr("t.noRoblox"), "ok".into()),
                    n => (r.i18n.tn(if force { "t.killing" } else { "t.closingAll" }, n, &[("n", &n.to_string())]), "ok".into()),
                },
                Ok(Err(e)) => (e.to_string(), "error".into()),
                Err(_) => return,
            });
            r.toast_n += 1;
            cx.notify();
        });
    }

    /// Close every Roblox window (or end them, forced), after the question (app.js's closeAll).
    pub fn close_all(&mut self, force: bool, window: &mut Window, cx: &mut Context<Self>) {
        let open = self.metrics.get("robloxCount").and_then(Value::as_i64).unwrap_or(0);
        let (title, text) = if force {
            (self.tr("common.confirm"), self.tr("t.killAllQ"))
        } else {
            (self.tr("t.closeAllTitle"), if open > 0 { self.i18n.tn("t.closeAllN", open, &[("n", &open.to_string())]) } else { self.tr("t.closeAllQ") })
        };
        let ok = self.tr(if force { "t.killAllOk" } else { "t.closeAllOk" });
        self.confirm(title, text, ok, true, Some(if force { "killAll" } else { "closeAll" }), window, cx, move |r, cx| r.close_all_now(force, cx));
    }

    /// What a game box holds, looked up from here (roblox::resolve; it was the C# app's resolveGame command until
    /// 2026-11-11). A sign-in is what lets Roblox count the players: the one already held, else the one the app lends.
    pub fn resolve_game(&self, input: String) -> impl std::future::Future<Output = Option<Value>> + 'static {
        let held = pages::friends::signed(self).map(|s| s.2).filter(|c| !c.is_empty());
        let link = self.link.clone();
        async move {
            let cookie = match (held, link) {
                (Some(c), _) => Some(c),
                (None, Some(link)) => link.call("robloxSignIn", json!({})).await.ok().and_then(Result::ok).and_then(|v| v.get("cookie").and_then(Value::as_str).map(str::to_string)),
                (None, None) => None,
            };
            crate::roblox::run(move || crate::roblox::resolve(&input, cookie.as_deref())).await.ok()
        }
    }

    /// A lookup's answer with its refusal in the interface's language (roblox::resolve names it by its key).
    pub fn worded(&self, mut g: Value) -> Value {
        if let Some(key) = g.get("errorKey").and_then(Value::as_str).map(str::to_string) {
            g["error"] = self.tr(&key).into();
        }
        g
    }

    /// Looks up what the Accounts bar's box holds (a link, a Place ID) and keeps the game found, for its picture and name.
    pub fn resolve_place(&mut self, cx: &mut Context<Self>) {
        let input = self.place.read(cx).value().trim().to_string();
        let Some(link) = self.link.clone() else { return };
        if input.is_empty() {
            self.game = None;
            self.place_asking = false;
            cx.notify();
            return;
        }
        let _ = link;
        let asked = self.resolve_game(input.clone());
        self.place_asking = true;
        cx.notify();
        answer(asked, cx, move |r, got, cx| {
            // (An answer for words no longer in the box is dropped.)
            if r.place.read(cx).value().trim() != input {
                return;
            }
            r.place_asking = false;
            let Some(g) = got else {
                cx.notify();
                return;
            };
            let g = r.worded(g);
            // A Job ID that went with another game is emptied (app.js's resolveGame and S.jobPlace).
            if let Some(p) = g.get("placeId").and_then(Value::as_i64).filter(|p| *p > 0) {
                pages::accounts::game_known(p, cx);
            }
            if g.get("ok").and_then(Value::as_bool) == Some(false) {
                let why = g.get("error").and_then(Value::as_str).unwrap_or_default().to_string();
                if !why.is_empty() {
                    r.toast = Some((why, "warn".into()));
                }
            }
            r.game = Some(g);
            cx.notify();
        });
    }

    fn dropped(&self) -> Vec<Value> {
        self.accounts().iter().filter(|a| pages::home::is_dropped(a)).cloned().collect()
    }

    /// Settings › Appearance › Hide account names is on: what names an account (on Roblox's own pages too) is shown
    /// by its first letter, or not at all.
    pub fn hides_names(&self) -> bool {
        self.state.get("hideNames").and_then(Value::as_bool).unwrap_or(false)
    }

    pub fn accounts(&self) -> &[Value] {
        self.state.get("accounts").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default()
    }

    /// The size typed for a Roblox window goes to the app, within what it takes (160–7680 by 120–4320; a box that
    /// holds no number counts as the usual 640 by 480), when it isn't the size already kept.
    fn save_window_size(&mut self, cx: &mut Context<Self>) {
        let num = |e: &Entity<InputState>, lo: i64, hi: i64, or: i64| e.read(cx).value().trim().parse::<f64>().ok().map(|v| v.round() as i64).filter(|v| *v != 0).unwrap_or(or).clamp(lo, hi);
        let (w, h) = (num(&self.win_w, 160, 7680, 640), num(&self.win_h, 120, 4320, 480));
        let kept = |key: &str, or: i64| self.state.pointer(key).and_then(Value::as_i64).unwrap_or(or);
        if w != kept("/settings/windowWidth", 640) || h != kept("/settings/windowHeight", 480) {
            self.send("setSettings", json!({ "windowWidth": w, "windowHeight": h }));
        }
    }

    /// The frame rate typed goes to the app, within what Roblox takes (5 to 360), when it isn't the limit already
    /// kept. A box left empty, or holding no number, changes nothing: the list in the box sets "as Roblox has it".
    fn save_fps(&mut self, cx: &mut Context<Self>) {
        let Some(fps) = self.fps_box.read(cx).value().trim().parse::<f64>().ok().map(|v| (v.round() as i64).clamp(5, 360)) else { return };
        if self.state.pointer("/settings/robloxConfig/fps").and_then(Value::as_i64) != Some(fps) {
            self.send("setSettings", json!({ "robloxConfig": { "fps": fps } }));
        }
    }

    /// The open vault's name, as the app lists it.
    pub fn open_vault_name(&self) -> String {
        let open = self.state.pointer("/vaults/open").and_then(Value::as_str).unwrap_or_default();
        self.state
            .pointer("/vaults/list")
            .and_then(Value::as_array)
            .and_then(|l| l.iter().find(|v| v.get("id").and_then(Value::as_str) == Some(open) && !open.is_empty()))
            .and_then(|v| v.get("name").and_then(Value::as_str))
            .unwrap_or_default()
            .to_string()
    }

    pub fn locked(&self) -> bool {
        self.state.get("locked").and_then(Value::as_bool).unwrap_or(false)
    }

    /// What the window shows now, as a name: the place, and inside Games its part, its account tab and what is open
    /// in it. The place steps in (kit::step_in) when this changes.
    fn shown_key(&self) -> String {
        if self.lost {
            return "lost".into();
        }
        if self.locked() {
            return "lock".into();
        }
        if self.page != Page::Games {
            return format!("{:?}", self.page);
        }
        let open = self.games.open.as_ref().map(|o| o.id.as_str()).unwrap_or_default();
        let library = self.games.home_library.map_or("", |tab| match tab {
            HomeTab::Games => "library",
            HomeTab::Community => "community-games",
        });
        // (Not the view: switching it in the header's capsule changes the page under a capsule that stays and slides
        // its yellow along; stepping the whole page in faded the capsule out and in with it.)
        // (The tab by its account, not its place: a tab dragged to another place is the same page, and stepping the
        // page in again at each move faded it in and out, the owner's OBS video, 2026-11-19.)
        let tab = self.tabs.list.get(self.tabs.at).and_then(|t| t.account.clone()).unwrap_or_else(|| format!("#{}", self.tabs.at));
        format!("games-{tab}-{open}-{}-{library}", self.profile.part())
    }

    /// The app's state, come as a command's answer (the lock screen's unlock).
    pub fn take_state(&mut self, state: Value, window: &mut Window, cx: &mut Context<Self>) {
        self.set_state(state, window, cx);
    }

    fn rail(&self, _window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let page = self.page;
        div()
            .w(px(RAIL))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .items_center()
            .pb(px(14.))
            .bg(c(RAIL_BG))
            .border_r_1()
            .border_color(c(LINE))
            // The logo has its own colours, so it is a picture, not an icon (an icon takes one colour).
            // (It pops out as the window opens and each time it is unlocked, as the C# app's page had it, logo-pop: the
            // port stood it there at once; the owner asked what differed, 2026-11-18.)
            .child(div().h(px(TOP)).flex_none().flex().items_center().child(pop_out("rail-logo", 34., |side| crate::crisp::blocks(side).into_any_element())))
            .child(div().mt(px(14.)).flex().flex_col().gap(px(6.)).children(
                {
                    // (As the window first shows, and each time it is unlocked, they come in from the left one after
                    // another: the rail isn't drawn while locked.)
                    let mut items = Vec::new();
                    // Games carries how much news hasn't been read (99+ at most), or a dot when some is fresh.
                    let unread = self.state.pointer("/feeds/unread").and_then(Value::as_i64).unwrap_or(0);
                    let fresh = self.state.pointer("/feeds/fresh").and_then(Value::as_i64).unwrap_or(0) > 0; // (a count, not a flag)
                    for (i, p) in Page::ALL.into_iter().filter(|p| *p != Page::Settings).enumerate() {
                        let mark = (p == Page::Games && (unread > 0 || fresh)).then(|| {
                            let base = div().absolute().top(px(3.)).right(px(3.)).rounded_full().bg(c(FACE));
                            if unread > 0 {
                                base.min_w(px(16.)).h(px(16.)).px(px(4.)).flex().items_center().justify_center().text_size(px(9.5)).font_weight(FontWeight::BOLD).text_color(c(INK)).child(if unread > 99 { "99+".to_string() } else { unread.to_string() })
                            } else {
                                base.size(px(8.))
                            }
                        });
                        items.push(tab_in(i, div().relative().child(nav_button(p, page, &self.i18n, cx)).children(mark)).into_any_element());
                    }
                    items
                },
            ))
            // Settings apart from the places you work in, below a line.
            .child(div().w(px(28.)).h(px(1.)).my(px(14.)).bg(c(LINE)))
            .child(tab_in(6, nav_button(Page::Settings, page, &self.i18n, cx)))
            .child(div().flex_1())
            // Switch vault (its face) whenever one is open, as Sidebar.tsx: with one vault it is the way to the lock screen's
            // picker and its Create a new vault. The open vault locks only when another is picked.
            // (The rail's foot rises into place a moment after the buttons above it came in: the page's intro-up.)
            .child(line_in("rail-foot", 3, div().flex().flex_col().items_center()
                .when(self.state.pointer("/vaults/open").and_then(Value::as_str).is_some_and(|o| !o.is_empty()), |d| {
                let v = self.state.get("vaults").cloned().unwrap_or(Value::Null);
                let (id, name) = (v.get("current").and_then(Value::as_str).unwrap_or_default().to_string(), v.get("name").and_then(Value::as_str).unwrap_or_default().to_string());
                let tip: SharedString = self.i18n.tv("vault.switchTip", &[("name", &name)]).into();
                d.child(
                    hot("vault-switch")
                        .size(px(42.))
                        .mb(px(6.))
                        .rounded(px(R_M))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .child(pages::lock::face(&id, &name, v.get("picture").and_then(Value::as_str), 24., 6., 12.))
                        .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
                        .on_click(cx.listener(|r, _, _, _| r.send("pickVault", json!({})))),
                )
            })
                .child(IconButton::new("lock", "lock").size(42.).tip(self.tr("side.lockTip")).on_click(cx.listener(|r, _, window, cx| r.lock_app(window, cx))))))
    }

    /// The top bar, which is the window's title bar too: pressing and moving on its empty part moves the window, a double
    /// click maximises it, and the window's buttons sit at its end.
    fn top(&self, with_side: bool, locked: bool, cx: &mut Context<Self>) -> impl IntoElement {
        // The search and the buttons keep their presses to themselves.
        let own = |d: Div| d.on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
        div()
            .id("top")
            .h(px(TOP))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(GAP))
            // (A theme whose window ground can't carry these words, Retro's teal: the bar on the rail's ground.)
            .when_some(crate::theme::bars_ground(), |d, g| d.bg(g).rounded_b(px(R_M)).px(px(8.)))
            // (A picture too light along its top for them: the bar is glass there, laid under it where it is placed.)
            .when(!locked && !crate::scene::top_reads(cx), |d| d.px(px(8.)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|r, e: &MouseDownEvent, _, _| {
                    if e.click_count == 2 {
                        r.drag_from = None;
                        r.send("window", json!({ "action": "zoom" }));
                    } else {
                        r.drag_from = Some(e.position);
                    }
                }),
            )
            .on_mouse_move(cx.listener(|r, e: &MouseMoveEvent, _, _| {
                let Some(from) = r.drag_from else { return };
                if e.pressed_button != Some(MouseButton::Left) {
                    r.drag_from = None;
                } else if (e.position.x - from.x).abs() > px(3.) || (e.position.y - from.y).abs() > px(3.) {
                    r.drag_from = None;
                    if let Some(h) = r.hwnd {
                        crate::frame::start_move(h);
                    }
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|r, _, _, _| r.drag_from = None))
            .child(
                own(div().flex().items_center().gap(px(2.)))
                    .when(locked, |d| d.invisible())
                    .child(IconButton::new("back", "chevronLeft").size(32.).disabled(self.back.is_empty()).tip(self.tr("ui.back")).on_click(
                        cx.listener(|r, _, _, cx| {
                            r.step(false);
                            cx.notify();
                        }),
                    ))
                    .child(IconButton::new("forward", "chevronRight").size(32.).disabled(self.fwd.is_empty()).tip(self.tr("ui.forward")).on_click(
                        cx.listener(|r, _, _, cx| {
                            r.step(true);
                            cx.notify();
                        }),
                    )),
            )
            .child(
                own(div().ml_auto().flex().items_center())
                    // Quick search (Ctrl+K): every place, action, preset, game and account, also reached from here
                    // (the old sidebar's Game monitor and Mini window are in it too).
                    .when(!locked, |d| {
                        d.child(
                            tool("top-search", Some("search"), false)
                                .child(self.tr("pal.title"))
                                .child(div().ml(px(2.)).px(px(6.)).h(px(20.)).flex().items_center().rounded(px(5.)).bg(ca(WHITE, 0.06)).text_size(px(T_XS)).text_color(c(MUTED)).child("Ctrl K"))
                                .on_click(cx.listener(|r, _, window, cx| pages::palette::open(r, window, cx))),
                        )
                        .child(div().w(px(8.)))
                    })
                    // Report a bug or send an idea, on every place (it was only in Settings: "ai mà biết", the owner,
                    // 2026-11-15).
                    .when(!locked, |d| {
                        d.child(IconButton::new("feedback-top", "message").size(32.).tip(self.tr("fb.reportTip")).on_click(cx.listener(|r, _, window, cx| pages::extras::open_feedback(r, window, cx))))
                            .child(div().w(px(8.)))
                    })
                    // Without the accounts column (a narrow window, or another place) Add account comes up here.
                    // Accounts has its own Add (the + over its list).
                    .when(!with_side && self.page != Page::Accounts && !locked, |d| {
                        d.child(IconButton::new("add-account-top", "userPlus").size(32.).tip(self.tr("acc.add")).on_click(cx.listener(|r, _, _, cx| r.add_account(cx))))
                            .child(div().w(px(8.)))
                    }),
            )
            // (The lock screen over a picture too light along its top: the window's buttons on the ground the language
            // switch under them has. Unlocked, the whole bar is glass there: scene::top_glass.)
            .child(own(div().flex().items_center().gap(px(2.))).when(locked && !crate::scene::top_reads(cx), |d| d.p(px(3.)).rounded(px(R_M)).bg(c(RAISED))).children([
                window_button("win-min", WinGlyph::Minimize, cx.listener(|r, _, _, _| r.send("window", json!({ "action": "minimize" })))),
                window_button("win-zoom", WinGlyph::Maximize, cx.listener(|r, _, _, _| r.send("window", json!({ "action": "zoom" })))),
                window_button("win-close", WinGlyph::Close, cx.listener(|r, _, _, _| r.send("window", json!({ "action": "close" })))),
            ]))
    }

    fn side(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let all = self.accounts();
        // The Accounts page's search (no accents, "@name", user ID, real names while hidden).
        let query = self.acc_search.read(cx).value().to_string();
        let accounts: Vec<&Value> = all.iter().filter(|a| pages::accounts::found(a, &query)).collect();
        let playing: Vec<&Value> = accounts.iter().copied().filter(|a| a.get("state").and_then(Value::as_str) == Some("running")).collect();
        let dropped: Vec<&Value> = playing
            .iter()
            .copied()
            .filter(|a| pages::home::is_dropped(a))
            .collect();
        // Open but back on Roblox's menu (or signed out in it): a group of its own, not "Playing" (owner 2026-11-13).
        let menu: Vec<&Value> = playing.iter().copied().filter(|a| pages::accounts::in_menu(a)).collect();
        let fine: Vec<&Value> = playing.iter().copied().filter(|a| !dropped.iter().any(|d| std::ptr::eq(*d, *a)) && !pages::accounts::in_menu(a)).collect();
        // Not open and in trouble (a sign-in to do again, a ban): with the dropped ones, on top, said in red.
        let troubled = |a: &Value| side_trouble(a).is_some();
        let idle_all: Vec<&Value> = accounts.iter().copied().filter(|a| a.get("state").and_then(Value::as_str) != Some("running")).collect();
        let check: Vec<&Value> = dropped.iter().copied().chain(idle_all.iter().copied().filter(|a| troubled(a))).collect();
        let idle: Vec<&Value> = idle_all.iter().copied().filter(|a| !troubled(a)).collect();
        let mut list = div().flex().flex_col().px(px(8.)).pb(px(12.));
        for (key, rows) in [("ui.grp.check", &check), ("ui.grp.playing", &fine), ("act.menuHere", &menu), ("ui.grp.idle", &idle)] {
            if rows.is_empty() {
                continue;
            }
            let look = key == "ui.grp.check";
            // A group's head as the page's heads are, smaller: its name, its count after it in muted figures.
            list = list.child(
                div()
                    .mt(px(16.))
                    .mb(px(4.))
                    .mx(px(10.))
                    .h(px(20.))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .text_size(px(T_S))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(c(TEXT_2))
                    .when(look, |d| d.child(icon("alert", 13., c(if rows.iter().any(|a| troubled(a)) { BAD } else { WARN }))))
                    .child(self.tr(key))
                    .child(div().text_color(c(MUTED)).font_weight(FontWeight::MEDIUM).font_features(tnum()).child(rows.len().to_string())),
            );
            for a in rows.iter() {
                let dropped_here = look && dropped.iter().any(|d| std::ptr::eq(*d, *a));
                list = list.child(account_row(a, dropped_here, self, cx));
            }
        }
        let panel = panel()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            // The column says what it holds, how many, and how to add one; the counts of who plays are on the cards.
            .child(
                div().pl(px(18.)).pr(px(10.)).pt(px(12.)).pb(px(10.)).child(
                    section_head(self.tr("nav.accounts"), Some(all.len()))
                        .child(IconButton::new("add-account", "plus").size(TOOL_H).tip(self.tr("acc.add")).on_click(cx.listener(|r, _, _, cx| r.add_account(cx)))),
                ),
            )
            .when(!all.is_empty(), |d| d.child(div().px(px(12.)).child(field(&self.acc_search, Some("search")))))
            .child(
                div().id("side-list").flex_1().overflow_y_scroll().child(if accounts.is_empty() {
                    div()
                        .p(px(18.))
                        .text_size(px(T_M))
                        .text_color(c(MUTED))
                        .child(self.tr(if all.is_empty() { "ui.noAccounts" } else { "ui.noMatch" }))
                } else {
                    list
                }),
            );
        div().w(px(SIDE)).h_full().flex_none().flex().flex_col().child(panel)
    }

    /// Without the accounts column, who dropped still shows: a strip on top of the place, with Rejoin for them all.
    fn drop_strip(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let dropped = self.dropped();
        if dropped.is_empty() {
            return None;
        }
        // The Accounts cards say it themselves, each with its Rejoin.
        if self.page == Page::Accounts {
            return None;
        }
        // On Home the stage says it itself (with its Rejoin) when they all dropped from the game on it.
        if self.page == Page::Home {
            let hero = pages::home::hero_place(self);
            if hero.is_some() && dropped.iter().all(|a| a.pointer("/session/game/placeId").and_then(Value::as_i64) == hero) {
                return None;
            }
        }
        let names: Vec<String> = dropped
            .iter()
            .map(|a| {
                let s = |k: &str| a.get(k).and_then(Value::as_str).unwrap_or_default();
                [s("label"), s("alias"), s("displayName"), s("username")].into_iter().find(|n| !n.is_empty()).unwrap_or_default().to_string()
            })
            .collect();
        let n = dropped.len() as i64;
        Some(
            div()
                .mb(px(GAP))
                .h(px(44.))
                .flex_none()
                .pl(px(14.))
                .pr(px(8.))
                .flex()
                .items_center()
                .gap(px(10.))
                .rounded(px(R_L))
                .bg(ca(WARN, 0.1))
                .border_1()
                .border_color(ca(WARN, 0.25))
                .text_size(px(T_M))
                .child(icon("alert", 16., c(WARN)))
                .child(div().flex_none().font_weight(FontWeight::SEMIBOLD).text_color(c(WARN)).child(self.i18n.tn("ui.droppedAcc", n, &[])))
                .child(div().flex_1().min_w_0().text_color(c(TEXT_2)).truncate().child(names.join(", ")))
                .child(Button::new("rejoin-all", self.tr(if n == 1 { "card.rejoinNow" } else { "ui.rejoinAll" })).icon("refresh").small().tint().on_click(
                    cx.listener(move |r, _, _, cx| r.rejoin(&dropped, cx)),
                ))
                .into_any_element(),
        )
    }

    /// The bar along the foot: three pills on the page (the owner's "Thon" mock-up, 2026-11-19, after the Stats menu
    /// bar app and Apple's Dynamic Island): Roblox's windows and the PC's figures on the left, Spotify's song in the
    /// middle while it plays, Roblox's version, the mini window and Multi-Roblox on the right. Each thing is a small
    /// faint name in capitals and its value, on one line; no lights. The bar is measured every frame: when it is too
    /// narrow, things leave in one fixed order (BAR_LEVELS) until all three fit, so two pills never overlap.
    /// `with_side`: the accounts column stands over the bar's right end (Home), and the right pill is made as wide as
    /// it (owner 2026-11-20: a pill 45 px short of the column over it looked like a slip).
    fn status(&self, with_side: bool, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let m = &self.metrics;
        let windows = m.get("robloxCount").and_then(Value::as_i64).unwrap_or(0);
        let ram = m.get("usedPct").and_then(Value::as_f64).unwrap_or(0.).round() as i64;
        // The whole PC's CPU, as RAM beside it is the whole PC's (the owner, 2026-11-19: "không có CPU à" with no Roblox
        // open: Roblox's own share was shown, so it went with the windows). GPU stays Roblox's own (Windows gives no
        // whole-PC figure here), shown while a window is open.
        let cpu = m.get("pcCpuPct").and_then(Value::as_f64).unwrap_or(0.).round() as i64;
        let gpu = m.get("gpuPct").and_then(Value::as_f64).filter(|_| windows > 0).map(|g| g.round() as i64);
        let multi = self.state.pointer("/multi/enabled").and_then(Value::as_bool).unwrap_or(false);
        let version = self.state.pointer("/update/current").and_then(Value::as_str).unwrap_or_default().to_string();
        let words = if windows == 0 { self.tr("bar.none") } else { self.i18n.tn("bar.windows", windows, &[]) };
        // An update is in and waits for a restart (not a quiet fix, which goes in by itself).
        let ready = self.state.pointer("/update/status").and_then(Value::as_str) == Some("ready") && !self.state.pointer("/update/quiet").and_then(Value::as_bool).unwrap_or(false);
        let no_roblox = self.state.get("robloxInstalled").and_then(Value::as_bool) == Some(false);
        let rbx = (!no_roblox).then(|| self.roblox_version_words()).flatten();
        let mini_open = self.state.get("miniOpen").and_then(Value::as_bool).unwrap_or(false);
        // Multi-Roblox's state, as Sidebar.tsx's MultiCard: Roblox closed windows, or the watch found the lock changed
        // in the next build: orange, and a click leads to Settings › Roblox updates instead of its box.
        let on = |p: &str| self.state.pointer(p).and_then(Value::as_bool).unwrap_or(false);
        let armed = on("/multi/armed");
        let closed = multi && on("/robloxUpdates/closed");
        let check = self.state.pointer("/robloxUpdates/watch/check").and_then(Value::as_str).unwrap_or_default();
        let watched = multi && armed && !closed && ["changed", "renamed", "gone"].contains(&check) && !on("/robloxUpdates/watch/worked");
        let to_updates = closed || watched;
        // (On but not ready: its name in orange says so, where a light was.)
        let multi_warn = to_updates || (multi && !armed);
        let privacy = crate::pages::game_privacy::chip(self);
        let song = bar_song(self.music.as_ref());
        let upd_words = self.tr("bar.update");
        capsule::note_ram(&self.metrics);
        // (The song's pill casts a faint shadow of its cover's colour, as Dynamic Island's does.)
        let music_shade = song.as_ref().and_then(|(s, _)| s.cover.as_deref()).and_then(|p| images::tone(p, cx)).map(|t| ca(t, 0.38));

        // ── Measuring ─────────────────────────────────────────────────────────────────────────────────────────────
        // Each pill's width at each level, from its words' real widths (the font the bar draws them in).
        let tw = |s: &str, size: f32, w: FontWeight| bar_text_w(&*window, s, size, w);
        let label_w = |s: &str| tw(&s.to_uppercase(), BAR_LABEL, FontWeight::SEMIBOLD);
        let value_w = |s: &str| tw(s, BAR_VALUE, FontWeight::SEMIBOLD);
        let pct_w = value_w("88%");
        let pair_w = |l: &str, v: f32| label_w(l) + 5. + v;
        let row_gap = |items: &[f32], pad_l: f32, pad_r: f32, gap: f32| {
            let shown: Vec<f32> = items.iter().copied().filter(|w| *w > 0.).collect();
            pad_l + pad_r + shown.iter().sum::<f32>() + gap * shown.len().saturating_sub(1) as f32
        };
        let row = |items: &[f32], pad_l: f32, pad_r: f32| row_gap(items, pad_l, pad_r, BAR_PILL_GAP);
        // (No "ROBLOX" word and no lines between the groups: its mark says it, as the right pill's do.)
        let head_w = 13. + 6. + value_w(&words);
        let cpu_w = pair_w("CPU", pct_w);
        let ram_w = pair_w("RAM", pct_w);
        let gpu_w = gpu.map_or(0., |_| pair_w("GPU", pct_w));
        let progress_w = self.progress.as_deref().map_or(0., |p| tw(p, BAR_VALUE, FontWeight::NORMAL));
        let no_roblox_w = if no_roblox { 14. + 6. + tw(&self.tr("side.noRoblox"), BAR_VALUE, FontWeight::NORMAL) } else { 0. };
        let privacy_w = if privacy.is_some() { 14. + 6. + tw(&self.tr("gpv.chip"), BAR_VALUE, FontWeight::NORMAL) } else { 0. };
        let left_w = |l: u8| {
            let (c, r, g) = (if l < 2 { cpu_w } else { 0. }, if l < 10 { ram_w } else { 0. }, if windows > 0 && l < 2 { gpu_w } else { 0. });
            row_gap(&[head_w, c, r, g, if windows > 0 { BAR_KEY } else { 0. }, progress_w, no_roblox_w, privacy_w], BAR_PAD, if windows > 0 { 6. } else { BAR_PAD }, BAR_GROUP_GAP)
        };
        let (title_w, artist_w, cover) = match &song {
            Some((s, _)) => (
                value_w(&s.title),
                if s.artist.trim().is_empty() { 0. } else { tw(&format!("· {}", s.artist), 12., FontWeight::NORMAL).min(BAR_ARTIST_MAX) },
                s.cover.as_deref().and_then(|p| crate::images::pic_sq(p, 18., cx)),
            ),
            None => (0., 0., None),
        };
        let music_w = |l: u8| {
            if song.is_none() {
                return 0.;
            }
            let title = if l < 8 { title_w.min(if l >= 4 { 110. } else { 190. }) } else { 0. };
            let artist = if l < 5 && l < 8 { artist_w } else { 0. };
            let words = if title > 0. { title + if artist > 0. { 6. + artist } else { 0. } } else { 0. };
            let keys = if l < 3 { 3. } else { 2. };
            row(&[if cover.is_some() { 18. } else { 15. }, words, keys * BAR_KEY + (keys - 1.) * 2.], 12., 4.)
        };
        let chip_w = |l: u8| if ready { 8. + 13. + if l < 9 { 5. + tw(&upd_words, 11.5, FontWeight::SEMIBOLD) } else { 0. } + 8. } else { 0. };
        // (The right pill's groups open with their mark, not a word: Roblox's, Multi's, Bloxkeep's.)
        let rbx_w = rbx.as_ref().map_or(0., |(v, _, _)| BAR_MARK + 5. + value_w(v));
        let multi_label_w = BAR_MARK + 6.;
        let app_w = if version.is_empty() { 0. } else { BAR_MARK + 5. + value_w(&version) };
        let right_w = |l: u8| {
            let r = if l < 7 || rbx.as_ref().is_some_and(|(_, old, _)| *old) { rbx_w } else { 0. };
            let a = if l < 1 { app_w } else { 0. };
            row_gap(
                // (No lines between the groups: each opens with its mark, and the room between groups is wider than
                // inside one (owner 2026-11-19, from the macOS menu bar / iStat Menus: "3, biểu tượng"). The small
                // switch is 36 wide with the room around its track, not the track's 30: measured at 30, the pill came
                // 6 short and the last words sat against its edge.)
                &[chip_w(l), r, BAR_KEY, if l < 6 || multi_warn { multi_label_w } else { 0. } + 36., a],
                if ready { 4. } else { BAR_PAD },
                BAR_PAD,
                BAR_GROUP_GAP,
            )
        };
        let room = BAR_W.with(|w| w.get());
        let room = if room > 0. { room } else { f32::from(window.viewport_size().width) - 120. };
        let live_song = song.as_ref().is_some_and(|(_, live)| *live);
        // (Under the accounts column the right pill is at least the column's width, as Discord's account panel is
        // its sidebar's; wider only while its content needs more, the update waiting.)
        let right_room = |l: u8| if with_side { right_w(l).max(SIDE) } else { right_w(l) };
        let need = |l: u8| left_w(l) + right_room(l) + if live_song { music_w(l) + 2. * BAR_GAP } else { BAR_GAP };
        let level = (0..=BAR_LEVELS).find(|l| need(*l) <= room).unwrap_or(BAR_LEVELS);
        let pill_w = chip_w(level);
        let (lw, mw, rw) = (left_w(level), music_w(level), right_room(level));
        // (Made wider than its content: the groups spread over it, no empty end.)
        let spread = rw > right_w(level) + 0.5;

        // ── Motion ────────────────────────────────────────────────────────────────────────────────────────────────
        // Apple's springs (WWDC23 "Animate with springs"): the side pills give way first, then the song's pill grows
        // out of a small round one with a little bounce; going, it shrinks and fades at once, nothing left behind.
        let lw_now = bar_tween("bar-left", lw, lw, 0.42, 0., 0., window);
        let rw_now = bar_tween("bar-right", rw, rw, 0.42, 0., 0., window);
        let (mw_now, m_fade, m_words) = if live_song {
            (bar_tween("bar-music", mw, BAR_PILL_H, 0.5, 0.15, 0.12, window), bar_tween("bar-music-a", 1., 0., 0.18, 0., 0.08, window), bar_tween("bar-music-in", 1., 0., 0.2, 0., 0.3, window))
        } else {
            (bar_tween("bar-music", 0., 0., 0.26, 0., 0., window), bar_tween("bar-music-a", 0., 0., 0.16, 0., 0.06, window), bar_tween("bar-music-in", 0., 0., 0.09, 0., 0., window))
        };
        // (Gone a moment ago: drawn once more after its last frame, so it is not left at its last width.)
        if song.as_ref().is_some_and(|(_, live)| !live) && !BAR_LEAVE_ASKED.with(|a| a.replace(true)) {
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(BAR_LEAVE).await;
                this.update(cx, |_, cx| cx.notify()).ok();
            })
            .detach();
        }

        // ── Drawing ───────────────────────────────────────────────────────────────────────────────────────────────
        // Three levels only (Apple HIG's label, secondary and tertiary): the figures white, the marks at 60 %, the
        // small words (CPU, RAM, GPU) at 30 %.
        let label = |s: &str| div().flex_none().text_size(px(BAR_LABEL)).font_weight(FontWeight::SEMIBOLD).text_color(ca(TEXT, 0.32)).mt(px(1.)).child(s.to_uppercase());
        let value = |s: String| div().flex_none().text_size(px(BAR_VALUE)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(s);
        // (A figure that changes rolls its digits, as Apple's numericText does.)
        let figure = |id: &'static str, name: &str, pct: i64, tip: String, window: &mut Window| {
            let tip: SharedString = tip.into();
            let tone = if pct as f64 >= LOAD_BUSY { load_colour(pct as f64) } else { c(TEXT) };
            div()
                .id(id)
                .flex()
                .flex_none()
                .items_center()
                .gap(px(5.))
                .child(label(name))
                .child(div().min_w(px(pct_w)).text_size(px(BAR_VALUE)).font_weight(FontWeight::SEMIBOLD).text_color(tone).child(capsule::roll(id, format!("{pct}%"), 16., window)))
                .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
        };

        let left = {
            // Rest the mouse on the count: about how many windows this PC runs, and what runs out first.
            let fit = m.get("fit").filter(|f| f.is_object());
            let tip: Option<SharedString> = fit.map(|f| {
                let by = self.tr(&format!("cap.by.{}", f.get("by").and_then(Value::as_str).unwrap_or("ram")));
                let each = crate::pages::accounts::mb_text(f.get("eachMb").and_then(Value::as_f64).unwrap_or(0.), self.i18n.vietnamese);
                self.i18n.tv("cap.tip", &[("n", &f.get("max").and_then(Value::as_i64).unwrap_or(0).to_string()), ("each", &each), ("by", &by)]).into()
            });
            let free = m.get("availableMb").and_then(Value::as_f64).map(|mb| if mb >= 1024. { format!("{:.1} GB", mb / 1024.) } else { format!("{mb:.0} MB") }).unwrap_or_default();
            let (show_cpu, show_ram) = (level < 2, level < 10);
            let cpu_el = show_cpu.then(|| figure("bar-cpu", "CPU", cpu, self.i18n.tv("bar.cpuTip", &[("pct", &cpu.to_string())]), window));
            let ram_el = show_ram.then(|| figure("bar-ram", "RAM", ram, self.i18n.tv("sys.used", &[("pct", &ram.to_string()), ("free", &free)]), window));
            let gpu_el = gpu.filter(|_| show_cpu && windows > 0).map(|g| figure("bar-gpu", "GPU", g, self.tr("sys.usageTip"), window));
            div()
                .flex()
                .items_center()
                .gap(px(BAR_GROUP_GAP))
                .child(
                    div()
                        .id("sys-windows")
                        .flex()
                        .flex_none()
                        .items_center()
                        .gap(px(6.))
                        .child(icon("roblox", 13., ca(TEXT, 0.6)))
                        .child(value(words).when(windows == 0, |d| d.text_color(ca(TEXT, 0.6))))
                        .when_some(tip, |d, tip| d.tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())),
                )
                // CPU and RAM are the whole PC's; GPU is Roblox's own share (sys.usageTip), only while a window is open.
                .children(cpu_el)
                .children(ram_el)
                .children(gpu_el)
                // Close all: every Roblox window, after a question. (Its own click: not the pill's, which opens it.)
                .when(windows > 0, |d| d.child(no_pick(IconButton::new("sys-close", "power").size(BAR_KEY).tip(self.tr("sys.closeAll")).on_click(cx.listener(|r, _, window, cx| r.close_all(false, window, cx))))))
                .when_some(self.progress.clone(), |d, p| d.child(div().flex_none().text_color(c(TEXT_2)).child(p)))
                // Nothing can be opened without it: said where it is seen on every page.
                .when(no_roblox, |d| d.child(div().flex().flex_none().items_center().gap(px(6.)).text_color(c(WARN)).child(icon("alert", 14., c(WARN))).child(self.tr("side.noRoblox"))))
                .children(privacy.map(no_pick))
        };

        let music = song.map(|(s, _)| {
            let tip: SharedString = format!("{}: {}{}", s.player.name(), s.title, if s.artist.trim().is_empty() { String::new() } else { format!(" · {}", s.artist) }).into();
            let (title_room, artist_on, back_on) = (if level >= 4 { 110f32 } else { 190. }.min(title_w), level < 5 && artist_w > 0., level < 3);
            let press = |id: &'static str, name: &'static str, tip: String, act: crate::media::Act, cx: &mut Context<Self>| music_press(id, name, tip, act, cx).size(BAR_KEY);
            div()
                .flex()
                .items_center()
                .gap(px(BAR_PILL_GAP))
                // The album's picture with the music app's mark in its corner (Spotify, Apple Music, YouTube Music);
                // no picture, the mark alone.
                .child(match cover {
                    Some(src) => div()
                        .relative()
                        .size(px(18.))
                        .flex_none()
                        .child(img(src).size(px(18.)).rounded(px(4.)).object_fit(ObjectFit::Cover))
                        .child(div().absolute().right(px(-4.)).bottom(px(-4.)).size(px(12.)).rounded_full().bg(c(PANEL)).flex().items_center().justify_center().child(icon(s.player.icon(), 8., c(s.player.colour()))))
                        .into_any_element(),
                    // (The mark in its own colour, Spotify's green: the owner, 2026-11-19, "logo của spotify màu xanh".)
                    None => icon(s.player.icon(), 15., c(s.player.colour())).into_any_element(),
                })
                .when(level < 8, |d| {
                    d.child(
                        div()
                            .id("music-now")
                            .flex()
                            .flex_none()
                            .items_center()
                            .gap(px(6.))
                            .opacity(m_words * if s.playing { 1. } else { 0.55 })
                            // (Longer than its room: the title runs, as a player's does, kit::marquee.)
                            .child(div().w(px(title_room)).flex_none().text_size(px(BAR_VALUE)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(marquee("music-status", s.title.clone())))
                            .when(artist_on, |d| d.child(div().max_w(px(BAR_ARTIST_MAX)).flex_none().truncate().text_size(px(12.)).text_color(c(MUTED)).child(format!("· {}", s.artist))))
                            .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into()),
                    )
                })
                .child(no_pick(
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .gap(px(2.))
                        .opacity(m_words)
                        .when(back_on, |d| d.child(press("music-back", "skipBack", self.tr("music.back"), crate::media::Act::Back, cx)))
                        .child(press("music-play", if s.playing { "pause" } else { "play" }, self.tr(if s.playing { "music.pause" } else { "music.play" }), crate::media::Act::Toggle, cx))
                        .child(press("music-next", "skipNext", self.tr("music.next"), crate::media::Act::Next, cx)),
                ))
        });

        let right = {
            let rbx_on = level < 7 || rbx.as_ref().is_some_and(|(_, old, _)| *old);
            let multi_tip: SharedString = {
                let page = self.tr("set.p.roblox");
                format!(
                    "{}: {}",
                    self.tr("multi.title"),
                    if closed {
                        self.i18n.tv("multi.closedTip", &[("page", &page)])
                    } else if watched {
                        self.i18n.tv("multi.watchTip", &[("page", &page)])
                    } else {
                        self.tr(if !multi { "multi.offTip" } else if armed { "multi.readyTip" } else { "multi.notReadyTip" })
                    }
                )
                .into()
            };
            let mini_tip = self.tr(if mini_open { "sys.miniHide" } else { "sys.mini" });
            div()
                .flex()
                .items_center()
                .gap(px(BAR_GROUP_GAP))
                .when(spread, |d| d.w_full().justify_between())
                // The one coloured thing in the bar: the app's update, waiting for a restart. Asked, not just sent:
                // the app can refuse (Roblox being opened, a sign-in window up) and says why.
                .when(ready, |d| {
                    let v = self.state.pointer("/update/version").and_then(Value::as_str).unwrap_or_default().to_string();
                    let tip: SharedString = self.i18n.tv(if self.state.pointer("/update/auto").and_then(Value::as_bool).unwrap_or(false) { "upd.restartTipAuto" } else { "upd.restartTip" }, &[("v", &v)]).into();
                    d.child(pill_in("apply-update-in", pill_w, BAR_GROUP_GAP, no_pick(
                        hot("apply-update")
                            // (Its ground through `tones`: a hot block paints its own, over any `bg`.)
                            .tones(c(FACE), c(FACE_HI))
                            .h(px(20.))
                            .px(px(8.))
                            .flex()
                            .flex_none()
                            .items_center()
                            .gap(px(5.))
                            .rounded(px(10.))
                            .cursor_pointer()
                            .text_size(px(11.5))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(c(INK))
                            .child(icon("refresh", 13., c(INK)))
                            .when(level < 9, |d| d.child(upd_words.clone()))
                            .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
                            .on_click(cx.listener(|r, _, _, cx| r.ask("applyUpdate", json!({}), cx, |_, _| {}))),
                    )))
                })
                // Roblox's version; out of date it is orange with what is newer, and leads to Settings › Roblox
                // version, which says how to update.
                .when_some(rbx.filter(|_| rbx_on), |d, (v, old, tip)| {
                    let tip: SharedString = tip.into();
                    d.child(
                        // (Its ground under the mouse is a rounded one with room around the words, as the bar's
                        // other keys have; the margin gives that room back, so the pill's measure stays the same.)
                        // (Plain: a click opens the pill's card, whose Roblox tile leads on to Settings › Roblox.)
                        div()
                            .id("sys-rbx-ver")
                            .flex()
                            .flex_none()
                            .items_center()
                            .gap(px(5.))
                            .child(icon("roblox", BAR_MARK, ca(TEXT, 0.6)))
                            .child(value(v).when(old, |d| d.text_color(c(WARN))))
                            .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into()),
                    )
                })
                // The mini window: the accounts in a small window that stays over Roblox; open, its button is lit.
                .child(no_pick(IconButton::new("sys-mini", "screens").size(BAR_KEY).on(mini_open).tip(mini_tip).on_click(cx.listener(|r, _, _, _| r.send("toggleMini", json!({}))))))
                .child(
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .gap(px(6.))
                        .when(level < 6 || multi_warn, |d| {
                            d.child(
                                focusable(div().id("multi-help"))
                                    .cursor_pointer()
                                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                    .child(icon("layers", BAR_MARK, if multi_warn { c(WARN) } else { ca(TEXT, 0.6) }))
                                    .tooltip(move |_, cx| cx.new(|_| Tip(multi_tip.clone())).into())
                                    .on_click(cx.listener(move |r, _, window, cx| {
                                        if to_updates {
                                            r.settings.part = pages::settings::Part::Roblox;
                                            r.go(Page::Settings);
                                            cx.notify();
                                        } else {
                                            r.multi_help(window, cx)
                                        }
                                    })),
                            )
                        })
                        .child(no_pick(Switch::new("multi", multi).small().on_toggle(cx.listener(move |r, on: &bool, window, cx| {
                            // Like app.js's setMulti: turning it on asks first (Roblox's rules); the state that comes
                            // back moves the switch.
                            if *on {
                                r.confirm(r.tr("set.multiConfirmTitle"), r.tr("set.multiConfirm"), r.tr("set.multiOk"), false, Some("multi"), window, cx, |r, _| {
                                    r.send("setSettings", json!({ "multiInstance": true }))
                                });
                            } else {
                                r.send("setSettings", json!({ "multiInstance": false }));
                            }
                            cx.notify();
                        })))),
                )
                .when(level < 1 && !version.is_empty(), |d| d.child(div().flex().flex_none().items_center().gap(px(5.)).child(div().flex_none().size(px(BAR_MARK)).child(crate::crisp::blocks(BAR_MARK))).child(value(version.clone()))))
        };

        div()
            .relative()
            .flex_none()
            .pt(px(8.))
            .pb(px(10.))
            .flex()
            .items_center()
            .gap(px(BAR_GAP))
            .font_features(tnum())
            .text_size(px(BAR_VALUE))
            .text_color(c(TEXT_2))
            // (The pills are glass over the theme's picture as the blocks are: they count as on the page.)
            .child(crate::scene::on_page(true))
            // (Its width, kept for the next frame's measuring.)
            .child(
                canvas(
                    |b, window, _| {
                        let w = f32::from(b.size.width);
                        if BAR_W.with(|k| (k.replace(w) - w).abs() > 0.5) {
                            window.refresh();
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            )
            // (A click on a pill's empty part opens its card, capsule.rs; its buttons keep their own clicks.)
            .child(bar_pill("bar-left", lw_now, lw, BAR_PAD, if windows > 0 { 6. } else { BAR_PAD }, 1., None, left.into_any_element()).on_click(cx.listener(|r, _, _, cx| r.pill_toggle(capsule::Pill::Left, cx))))
            .child(div().flex_1().min_w_0().flex().justify_center().children(music.map(|d| {
                bar_pill("bar-music", mw_now, mw, 12., 4., m_fade, music_shade, d.into_any_element()).on_click(cx.listener(|r, _, _, cx| r.pill_toggle(capsule::Pill::Music, cx)))
            })))
            .child(bar_pill("bar-right", rw_now, rw, if ready { 4. } else { BAR_PAD }, BAR_PAD, 1., None, right.into_any_element()).on_click(cx.listener(|r, _, _, cx| r.pill_toggle(capsule::Pill::Right, cx))))
            .child(crate::scene::on_page(false))
    }

    /// Roblox's version for the status bar: its words ("0.742", or "0.742 → 0.743" when a newer one is out), whether
    /// it is out of date, and its tip (the full build). None while it isn't known.
    fn roblox_version_words(&self) -> Option<(String, bool, String)> {
        let rv = self.state.get("robloxVersion")?;
        let installed = rv.get("installed").and_then(Value::as_str).filter(|v| !v.is_empty())?.to_string();
        let latest = rv.get("latest").and_then(Value::as_str).unwrap_or_default();
        let short = |v: &str| v.split('.').take(2).collect::<Vec<_>>().join(".");
        let old = rv.get("upToDate").and_then(Value::as_bool) == Some(false);
        let words = if old && !latest.is_empty() && short(latest) != short(&installed) { format!("{} → {}", short(&installed), short(latest)) } else { short(&installed) };
        let tip = if old {
            let more = if short(latest) == short(&installed) { self.tr("rbx.newBuild") } else { self.i18n.tv("rbx.available", &[("v", &short(latest))]) };
            format!("Roblox {installed} · {more}")
        } else if rv.get("upToDate").and_then(Value::as_bool) == Some(true) {
            format!("Roblox {installed} · {}", self.tr("rbx.latestShort"))
        } else {
            format!("Roblox {installed}")
        };
        Some((words, old, tip))
    }

    /// Spotify's back, play/pause and next for a place that shows its song (the status bar, the profile's song).
    pub fn music_keys(&self, ids: [&'static str; 3], cx: &mut Context<Self>) -> Option<[IconButton; 3]> {
        let playing = self.music.as_ref()?.playing;
        use crate::media::Act;
        Some([
            music_press(ids[0], "skipBack", self.tr("music.back"), Act::Back, cx),
            music_press(ids[1], if playing { "pause" } else { "play" }, self.tr(if playing { "music.pause" } else { "music.play" }), Act::Toggle, cx),
            music_press(ids[2], "skipNext", self.tr("music.next"), Act::Next, cx),
        ])
    }

    fn toast_layer(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        // No connection: its notice holds the place until the connection is back (no other notice shows meanwhile).
        if let Some(words) = self.offline {
            let notice = div()
                .max_w(px(480.))
                .pl(px(14.))
                .pr(px(16.))
                .py(px(11.))
                .flex()
                .items_center()
                .gap(px(11.))
                .rounded(px(R_L))
                .bg(c(RAISED_2))
                .border_1()
                .border_color(ca(WHITE, 0.1))
                .shadow_lg()
                .text_size(px(T_M))
                .child(div().flex_none().child(crate::kit::spinner(16., c(WARN))))
                .child(div().min_w_0().line_height(px(19.)).child(self.tr(words)));
            return Some(div().absolute().top(px(((TOP - 44.) / 2.).max(0.))).left_0().right_0().flex().justify_center().child(slide_in("net-lost".to_string(), notice)).into_any_element());
        }
        // Every notice goes away by itself after 5 s, whoever said it. Only the app's own "toast" event started a
        // timer, so one a page set itself ("Opened 1 account.", "Select accounts first") stayed for good (owner, on
        // the real app, 2026-11-13). The timer starts here, where every notice passes; a newer notice drops it.
        match self.toast.clone() {
            Some(now) => {
                let key = (now, self.toast_n);
                if self.toast_timed.as_ref() != Some(&key) {
                    let text = key.0.0.clone();
                    // A warning or an error the page said by itself goes to error.log here, the one place every
                    // notice passes (the app's own are written where the app says them: `toast_core`). 43 places in
                    // the pages set one; whether each also wrote a line was never checked one by one (the owner,
                    // 2026-11-15: "lúc trước hỏi error log đã cover hết chưa thì cứ gật đầu").
                    #[cfg(feature = "core")]
                    if matches!(key.0.1.as_str(), "bad" | "warn" | "error") && self.toast_core.as_deref() != Some(text.as_str()) {
                        crate::core::log::note_file(&format!("Shown by the page ({}): {}", key.0.1, crate::core::app::feedback::scrub_text(&text, Vec::new())));
                    }
                    self.toast_timed = Some(key);
                    // (A shot says so: a hidden window can't show the notice leaving.)
                    #[cfg(feature = "shot")]
                    eprintln!("shot: toast timed: {text}");
                    self.toast_task = Some(cx.spawn(async move |this, cx| {
                        cx.background_executor().timer(Duration::from_secs(5)).await;
                        let _ = this.update(cx, |r, cx| {
                            if r.toast.as_ref().is_some_and(|(t, _)| *t == text) {
                                r.toast = None;
                                #[cfg(feature = "shot")]
                                eprintln!("shot: toast gone: {text}");
                                cx.notify();
                            }
                        });
                    }));
                }
            }
            None => self.toast_timed = None,
        }
        if let Some(now) = &self.toast {
            if *now != self.toast_was.0 {
                self.toast_was = (now.clone(), self.toast_n);
            }
        }
        let leaving = self.toast.is_none();
        let at = present("toast", !leaving, 180, window, cx)?;
        let ((text, kind), first) = self.toast_was.clone();
        let again = self.toast_n != first;
        let (dot, mark) = match kind.as_str() {
            "ok" => (OK, "check"),
            "warn" => (WARN, "alert"),
            "error" | "bad" => (BAD, "alert"),
            _ => (TEXT_2, "info"),
        };
        let key = format!("toast-{text}");
        let notice = div()
                    .max_w(px(480.))
                    .pl(px(14.))
                    .pr(px(16.))
                    .py(px(11.))
                    .flex()
                    .items_center()
                    .gap(px(11.))
                    .rounded(px(R_L))
                    .bg(c(RAISED_2))
                    .border_1()
                    .border_color(ca(WHITE, 0.1))
                    .shadow_lg()
                    .text_size(px(T_M))
                    .relative()
                    .child(self.live.cover())
                    .child(div().flex_none().child(icon(mark, 16., c(dot))))
                    .child(div().min_w_0().line_height(px(19.)).child(text))
                    // Something failed: reporting it is one click away, on the notice itself.
                    .when(matches!(kind.as_str(), "error" | "bad") && !self.locked(), |d| {
                        d.child(Button::new("toast-report", self.tr("fb.report")).small().quiet().on_click(cx.listener(|r, _, window, cx| pages::extras::open_feedback(r, window, cx))))
                    });
        // It slides up into place, a little past it and back, as the web page's did (kit::slide_in); a new notice
        // (other words) slides in again, the same one said again only shakes, and it sinks and fades as it leaves.
        let notice = if leaving {
            div().opacity(at).mt(px((at - 1.) * 12.)).child(notice).into_any_element()
        } else if again {
            nudge(self.toast_n, slide_in(key, notice)).into_any_element()
        } else {
            slide_in(key, notice).into_any_element()
        };
        // In the middle of the title bar (its empty part between the arrows and the quick search): at the foot it lay
        // over the cards' Close buttons and the play bar.
        Some(div().absolute().top(px(((TOP - 44.) / 2.).max(0.))).left_0().right_0().flex().justify_center().child(notice).into_any_element())
    }
}

/// A place's button in the rail, its name under the mouse; the place you're on is a faint yellow square.
fn nav_button(p: Page, page: Page, i18n: &I18n, cx: &mut Context<Root>) -> AnyElement {
    IconButton::new(SharedString::from(format!("nav-{}", p.key())), p.icon())
        .size(42.)
        .selected(p == page)
        .tip(i18n.t(p.label_key()))
        .on_click(cx.listener(move |r, _, _, cx| {
            r.go(p);
            cx.notify();
        }))
        .into_any_element()
}

// ── The status bar's pills ─────────────────────────────────────────────────────────────────────────────────────
const BAR_PILL_H: f32 = 28.;
const BAR_PAD: f32 = 13.;
const BAR_PILL_GAP: f32 = 11.;
/// The room between the right pill's groups (each opens with its mark, no lines between them), and the marks' size.
const BAR_GROUP_GAP: f32 = 15.;
const BAR_MARK: f32 = 14.;
const BAR_GAP: f32 = 12.;
const BAR_LABEL: f32 = 9.5;
const BAR_VALUE: f32 = 12.5;
const BAR_KEY: f32 = 24.;
const BAR_ARTIST_MAX: f32 = 140.;
/// How long a song's pill is drawn after the song stopped, while it shrinks away.
const BAR_LEAVE: Duration = Duration::from_millis(420);
/// What leaves when the bar is too narrow, in this order: 1 the app's number, 2 CPU and GPU, 3 the song's back
/// button, 4 a shorter title, 5 the artist, 6 Multi-Roblox's name (its switch stays), 7 Roblox's version (unless it
/// is out of date), 8 the song's words (its icon and buttons stay), 9 the update's word (its icon stays), 10 RAM.
const BAR_LEVELS: u8 = 10;

thread_local! {
    static BAR_W: std::cell::Cell<f32> = const { std::cell::Cell::new(0.) };
    static BAR_LEAVE_ASKED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static BAR_TWEENS: std::cell::RefCell<std::collections::HashMap<&'static str, BarTween>> = std::cell::RefCell::new(std::collections::HashMap::new());
    static BAR_SONG: std::cell::RefCell<Option<(crate::media::Playing, Option<std::time::Instant>)>> = const { std::cell::RefCell::new(None) };
}

/// A width or a fade on its way somewhere.
#[derive(Clone, Copy)]
struct BarTween {
    from: f32,
    to: f32,
    at: std::time::Instant,
    dur: f32,
    bounce: f32,
    delay: f32,
}

impl BarTween {
    fn value(&self, now: std::time::Instant) -> (f32, bool) {
        let s = now.saturating_duration_since(self.at).as_secs_f32() - self.delay;
        if s >= self.dur * 1.6 {
            return (self.to, true);
        }
        (self.from + (self.to - self.from) * apple_spring(s.max(0.), self.dur, self.bounce), false)
    }
}

/// Apple's spring (WWDC23 "Animate with springs"): stiffness (2π/duration)², damping 4π(1 − bounce)/duration, so
/// a damping ratio of 1 − bounce; from 0 at the start towards 1.
fn apple_spring(t: f32, dur: f32, bounce: f32) -> f32 {
    if t <= 0. {
        return 0.;
    }
    let w = std::f32::consts::TAU / dur;
    let z = (1. - bounce).clamp(0.05, 1.);
    if z >= 0.999 {
        1. - (-w * t).exp() * (1. + w * t)
    } else {
        let wd = w * (1. - z * z).sqrt();
        1. - (-z * w * t).exp() * ((wd * t).cos() + z * w / wd * (wd * t).sin())
    }
}

/// `key`'s value on its way to `to`: a new `to` starts a spring from where it is now (from `first` when it was at
/// nothing: a pill coming back grows from a small round one). Frames are asked for while it moves.
fn bar_tween(key: &'static str, to: f32, first: f32, dur: f32, bounce: f32, delay: f32, window: &mut Window) -> f32 {
    let now = std::time::Instant::now();
    let (v, done) = BAR_TWEENS.with(|m| {
        let mut m = m.borrow_mut();
        let t = m.entry(key).or_insert(BarTween { from: first, to: first, at: now, dur, bounce, delay: 0. });
        if (t.to - to).abs() > 0.01 {
            let from = if t.to.abs() < 0.01 && to > 0. { first } else { t.value(now).0 };
            *t = BarTween { from, to, at: now, dur, bounce, delay };
        }
        t.value(now)
    });
    if !done {
        window.request_animation_frame();
    }
    v
}

/// The song for the bar's middle pill: the one playing (live), or the last one for a moment after it stopped, while
/// its pill goes.
fn bar_song(now: Option<&crate::media::Playing>) -> Option<(crate::media::Playing, bool)> {
    BAR_SONG.with(|s| {
        let mut s = s.borrow_mut();
        match now {
            Some(p) => {
                BAR_LEAVE_ASKED.with(|a| a.set(false));
                *s = Some((p.clone(), None));
                Some((p.clone(), true))
            }
            None => {
                let (p, gone) = s.as_mut()?;
                let at = *gone.get_or_insert_with(std::time::Instant::now);
                if at.elapsed() < BAR_LEAVE {
                    Some((p.clone(), false))
                } else {
                    *s = None;
                    None
                }
            }
        }
    })
}

/// How wide `s` is drawn in the bar's font at `size`.
fn bar_text_w(window: &Window, s: &str, size: f32, weight: FontWeight) -> f32 {
    if s.is_empty() {
        return 0.;
    }
    let mut f = gpui_kit::font(FONT);
    f.weight = weight;
    f.features = tnum();
    let run = TextRun { len: s.len(), font: f, color: c(TEXT), background_color: None, underline: None, strikethrough: None };
    f32::from(window.text_system().shape_line(SharedString::from(s.to_string()), px(size), &[run], None).width).ceil()
}

/// One of the bar's pills, `now` wide on its way to `to`: its things laid out at `to` (so they don't move while it
/// grows or shrinks; the pill's edge shows or hides them), on glass over the picture or the theme's block ground.
/// Its edge is lit from above (a top rim brighter than the faint ring round it, brighter still under the mouse) and
/// it sits on a soft shadow, `shade` when it has its own colour (the song's cover): the premium look the owner
/// approved on 2026-11-19 (artifacts/mock/con-nhong.html, "Con nhộng sang hơn").
fn bar_pill(id: &'static str, now: f32, to: f32, pad_l: f32, pad_r: f32, alpha: f32, shade: Option<Hsla>, inner: AnyElement) -> Stateful<Div> {
    let light = crate::theme::light();
    let ring = if light { c(LINE) } else { ca(WHITE, 0.06) };
    let group = SharedString::from(format!("{id}-g"));
    let r = BAR_PILL_H / 2.;
    div()
        .id(id)
        .group(group.clone())
        .relative()
        .flex_none()
        .w(px(now.max(0.)))
        .h(px(BAR_PILL_H))
        .rounded(px(r))
        .overflow_hidden()
        .opacity(if capsule::covers(id) { 0. } else { alpha })
        .cursor_pointer()
        .shadow(vec![
            BoxShadow { color: hsla(0., 0., 0., if light { 0.12 } else { 0.3 }), offset: point(px(0.), px(1.)), blur_radius: px(1.5), spread_radius: px(0.), inset: false },
            BoxShadow { color: shade.unwrap_or(hsla(0., 0., 0., if light { 0.18 } else { 0.45 })), offset: point(px(0.), px(8.)), blur_radius: px(22.), spread_radius: px(-8.), inset: false },
        ])
        .child(capsule::note_at(id))
        .child(div().absolute().inset_0().child(crate::kit::glass_pane(r)))
        .child(div().absolute().inset_0().rounded(px(r)).border_1().border_color(ring).group_hover(group.clone(), |s| s.border_color(if light { ca(TEXT, 0.2) } else { ca(WHITE, 0.11) })))
        .when(!light, |d| d.child(div().absolute().inset_0().rounded(px(r)).border_t_1().border_color(ca(WHITE, 0.14)).group_hover(group, |s| s.border_color(ca(WHITE, 0.26)))))
        .child(div().absolute().top_0().left_0().h_full().w(px(to)).pl(px(pad_l)).pr(px(pad_r)).flex().items_center().child(inner))
}

#[derive(Clone, Copy, PartialEq)]
enum WinGlyph {
    Minimize,
    Maximize,
    Close,
}

/// Minimise, maximise or close: drawn quiet, the close one turning red under the mouse, as Windows' own do.
fn window_button(id: &'static str, glyph: WinGlyph, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Stateful<Div> {
    let mark = match glyph {
        WinGlyph::Minimize => div().w(px(10.)).h(px(1.25)).bg(c(TEXT_2)),
        WinGlyph::Maximize => div().size(px(10.)).rounded(px(2.)).border(px(1.25)).border_color(c(TEXT_2)),
        WinGlyph::Close => div().child(icon("x", 15., c(TEXT_2))),
    };
    div()
        .id(id)
        .w(px(40.))
        .h(px(32.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(R_S))
        .tab_index(0)
        .border_1()
        .border_color(ca(WHITE, 0.))
        .focus_visible(|s| s.border_color(ca(TEXT, 0.75)))
        .cursor_pointer()
        .hover(move |d| d.bg(if glyph == WinGlyph::Close { c(BAD) } else { c(RAISED) }))
        .child(mark)
        .on_click(f)
}

/// Why an account that isn't open needs a look, in two or three words: a sign-in to do again, a ban (as the Accounts
/// page's panel of what needs a look). None when it is fine.
fn side_trouble(a: &Value) -> Option<&'static str> {
    if a.get("state").and_then(Value::as_str) == Some("running") {
        None
    } else if a.get("banned").and_then(Value::as_bool).unwrap_or(false) {
        Some("chip.banned")
    } else if a.get("cookieValid").and_then(Value::as_bool) == Some(false) {
        Some("chip.invalid")
    } else {
        None
    }
}

/// An account in the column: its face (its picture at its size, else its first letter) with a dot for how it is, its
/// name, its game or what is wrong, and the time it has played; its ground eases in under the mouse. A dropped one has
/// Rejoin, one that must sign in again Sign in.
fn account_row(a: &Value, dropped: bool, root: &Root, cx: &mut Context<Root>) -> impl IntoElement {
    const FACE: f32 = 32.;
    let s = |k: &str| a.get(k).and_then(Value::as_str).unwrap_or_default();
    // ("label" first: with names hidden it is the only one left, the others blanked; the row showed no name, 2026-11-12.)
    let name = [s("label"), s("alias"), s("displayName"), s("username")].into_iter().find(|n| !n.is_empty()).unwrap_or_default().to_string();
    let running = s("state") == "running";
    // (On Roblox's menu: "Not in a game", not the last game it left.)
    let menu = pages::accounts::in_menu(a);
    let game = if menu { Some(root.tr("act.menuWhere")) } else { a.pointer("/session/game/name").and_then(Value::as_str).filter(|g| !g.is_empty()).map(str::to_string) };
    let face = images::pic_sq(s("avatarUrl"), FACE, cx);
    let id = s("id").to_string();
    let can_open = pages::tabs::can_open_account(a);
    let open_tip: SharedString = root.tr("tabs.openAccount").into();
    let trouble = side_trouble(a);
    // (Banned: grey, not red, critique 2026-11-13: nothing to do now, Play skips it; red is for a sign-in to do.)
    let dot = match trouble { Some("chip.banned") => MUTED, Some(_) => BAD, None if dropped => WARN, None if menu => ONLINE, None if running => OK, None => DIM };
    let game = game.map(|g| crate::pages::home::clean(&g));
    // Playing: for how long (it counts up); not open: when it last was.
    let now = crate::time::now_secs();
    let when = if running {
        a.pointer("/session/since").and_then(Value::as_str).and_then(crate::time::iso_secs).map(|t| crate::time::clock(now - t))
    } else {
        a.get("lastUsed").and_then(Value::as_str).and_then(crate::time::iso_secs).map(|t| crate::time::ago(now - t, &root.i18n))
    };
    // Every row has its second line (a row without one stood shorter, review 1.2): the game here, else the game Roblox
    // says it plays elsewhere, else online, offline or never opened.
    let game = game.or_else(|| {
        let p = a.get("presence")?;
        match p.get("type").and_then(Value::as_str) {
            Some("ingame") => Some(p.get("game").and_then(Value::as_str).filter(|g| !g.is_empty()).map(crate::pages::home::clean).unwrap_or_else(|| root.tr("chip.ingame"))),
            Some("online") => Some(root.tr("chip.online")),
            _ => None,
        }
    });
    let game = game.or_else(|| Some(root.tr(if a.get("lastUsed").and_then(Value::as_str).is_some_and(|t| !t.is_empty()) { "chip.offline" } else { "card.never" })));
    let (sub, sub_col) = match trouble {
        Some(k) => (Some(root.tr(k)), c(if k == "chip.banned" { MUTED } else { BAD })),
        None => (game, if dropped { c(WARN) } else { c(MUTED) }),
    };
    let first: String = name.chars().find(|c| c.is_alphanumeric()).map(|c| c.to_uppercase().collect()).unwrap_or_default();
    let fix: Option<AnyElement> = if dropped {
        let me = vec![a.clone()];
        Some(Button::new(SharedString::from(format!("rejoin-{id}")), root.tr("card.rejoinNow")).small().tint().on_click(cx.listener(move |r: &mut Root, _, _, cx| {
            cx.stop_propagation();
            r.rejoin(&me, cx);
        })).into_any_element())
    } else if trouble == Some("chip.invalid") {
        let vng = s("realm") == "vng";
        let who = name.to_string();
        Some(Button::new(SharedString::from(format!("relogin-{id}")), root.tr("card.relogin")).small().tint().on_click(cx.listener(move |r: &mut Root, _, window, cx| crate::pages::quick::open_for(r, vng, who.clone(), window, cx))).into_any_element())
    } else {
        None
    };
    hover_only(SharedString::from(format!("acc-{id}")))
        .min_h(px(48.))
        .px(px(10.))
        .py(px(7.))
        .flex()
        .items_center()
        .gap(px(12.))
        .rounded(px(R_M))
        .tones(ca(WHITE, 0.), ca(WHITE, 0.045))
        .when(can_open, |d| d.cursor_pointer().tab_index(0)
            .tooltip(move |_, cx| cx.new(|_| Tip(open_tip.clone())).into())
            .on_click(cx.listener(move |r, _, _, cx| {
                r.go(Page::Games);
                pages::tabs::open(r, id.clone());
                cx.notify();
            })))
        // No number tag here (owner 2026-11-14: "bỏ số đi"): the face's dot and the line under the name say the state.
        .child(
            div()
                .relative()
                .size(px(FACE))
                .flex_none()
                .rounded(px(R_S + 1.))
                .bg(c(RAISED_2))
                .flex()
                .items_center()
                .justify_center()
                .map(|d| match face {
                    Some(f) => d.child(img(f).size_full().rounded(px(R_S + 1.)).object_fit(ObjectFit::Cover)),
                    None => d.text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).text_color(c(MUTED)).child(first),
                })
                .child(div().absolute().right(px(-2.)).bottom(px(-2.)).size(px(12.)).rounded_full().border_2().border_color(c(PANEL)).bg(c(dot))),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(1.))
                .child(div().text_size(px(T_L)).line_height(px(20.)).font_weight(FontWeight::MEDIUM).text_color(c(TEXT)).truncate().child(name))
                .when_some(sub, |d, g| d.child(div().text_size(px(T_S)).line_height(px(16.)).text_color(sub_col).truncate().child(g))),
        )
        .map(|d| match fix {
            Some(b) => d.child(b),
            None => d.when_some(when, |d, w| d.child(div().flex_none().text_size(px(T_S)).text_color(c(MUTED)).font_features(tnum()).child(w))),
        })
}

/// An answer in a box (Root::ask_box): its words, its look (0 plain, 1 the one the box suggests, 2 one that destroys
/// something) and what it does; it is told whether "Don't ask again" was ticked.
#[derive(Clone)]
pub struct Choice {
    pub label: String,
    pub kind: u8,
    pub run: std::rc::Rc<dyn Fn(&mut Root, bool, &mut Window, &mut Context<Root>)>,
}

impl Choice {
    pub fn new(label: String, kind: u8, run: impl Fn(&mut Root, bool, &mut Window, &mut Context<Root>) + 'static) -> Self {
        Self { label, kind, run: std::rc::Rc::new(run) }
    }
}

/// A box that waits for a window to open in.
enum Pending {
    /// `rest`: rejoins waiting behind this one, sent after the answer, whichever it is.
    LaunchAsk { cmd: &'static str, args: Value, answer: Value, rest: Vec<Value> },
    LaunchErrors(Value),
    Remote(Value),
    /// Add account: which way.
    Add,
    WhatsNew(Vec<Value>),
}

impl Render for Root {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // (Settings > Appearance > "Stand still while behind": off, the motion goes on with another window in front.)
        let away = !window.is_window_active() && crate::kit::still_behind() && !crate::kit::mouse_over_lately();
        // (A hidden clip, BLOXKEEP_SHOT_FRAMES: its window is never the one in front, yet what moves is what the
        // clip is for. Left to chance, a clip showed running titles standing still on some runs, 2026-11-18.)
        #[cfg(feature = "shot")]
        let away = away && std::env::var_os("BLOXKEEP_SHOT_FRAMES").is_none();
        crate::kit::set_away(away);
        // (A theme came in over another: the frame last drawn, the old look, is kept to fade out over this one.)
        if crate::theme::take_fade() {
            crate::scene::fade_from(window, cx);
        }
        crate::theme::set_glass_blocks(crate::scene::carries_words(cx));
        crate::kit::set_screen_scale(window.scale_factor());
        self.live.start();
        let toast = self.toast_layer(window, cx);
        let pill_card = self.pill_layer(window, cx);
        #[cfg(feature = "shot")]
        let tray_shot = crate::tray::shot_layer(self, window, cx);
        #[cfg(not(feature = "shot"))]
        let tray_shot: Option<AnyElement> = None;
        pages::lock::settle(self, cx);
        pages::games::settle_words(self, window, cx);
        pages::rsettings::settle(self, window, cx);
        pages::wiki::settle(self, window, cx);
        pages::accounts::settle_job(window, cx);
        // Settings left (or the app locked): typed passwords emptied, the Discord link hidden again.
        pages::settings::settle(self, window, cx);
        // The search reads every part's rows, Discord's too: its boxes are made here the first time something is
        // searched (they were only made when Discord itself was opened, so "discord" found two unrelated rows).
        if self.page == Page::Settings && self.settings.dc.is_none() && !self.set_q.read(cx).value().trim().is_empty() {
            pages::settings::ensure_discord(self, window, cx);
        }
        if self.page == Page::Settings {
            pages::settings::appearance::settle(self, window, cx);
        }
        self.community_tick();
        // app.js's syncBusy: something typed in a form would be lost to an update's restart, so it waits.
        let busy = window.has_active_dialog(cx) || self.auto.form.is_some() || self.auto.macro_form.is_some();
        if busy != self.busy_told {
            self.busy_told = busy;
            self.send("pageBusy", json!({ "busy": busy }));
        }
        // app.js asked for the backup folders to suggest (OneDrive, Google Drive, …) each time Settings was opened.
        let on_settings = self.page == Page::Settings && !self.locked();
        if on_settings && !self.on_settings {
            self.send("backupSuggestions", json!({}));
        }
        self.on_settings = on_settings;
        if !self.pending.is_empty() {
            let waiting = std::mem::take(&mut self.pending);
            cx.defer_in(window, move |r, window, cx| {
                for p in waiting {
                    r.open_pending(p, window, cx);
                }
            });
        }
        if !self.shot_asked && !self.state.is_null() {
            self.shot_asked = true;
            // BLOXKEEP_SHOT_ASK=1: a hidden shot opens with a question, to see the box (=multi: the Multi-Roblox one).
            if let Ok(which) = std::env::var("BLOXKEEP_SHOT_ASK") {
                cx.defer_in(window, move |r, window, cx| {
                    // (=add the sign-in screen, =phone its code, =cookie and =edit the account form, =import, =feedback,
                    // =join, =order: the other boxes.)
                    let first = r.accounts().first().and_then(|a| a.get("id")).and_then(Value::as_str).map(str::to_string);
                    // (=launch: Play on an idle account, for the app's "Before you open" (with ?multioff=1); =launcherr:
                    // on an idle and a banned one, for the list of those that didn't open; =send:CMD: that command sent
                    // to the app as it is (send:addVault, send:pickVault); =saveserver: Save server once the game box
                    // knows its game.)
                    #[cfg(feature = "shot")]
                    {
                        let id_of = |a: &Value| a.get("id").and_then(Value::as_str).map(str::to_string);
                        let idle: Vec<String> = r.accounts().iter().filter(|a| pages::accounts::can_play(a) && a.get("banned").and_then(Value::as_bool) != Some(true)).filter_map(id_of).take(2).collect();
                        let banned = r.accounts().iter().filter(|a| a.get("banned").and_then(Value::as_bool) == Some(true)).find_map(id_of);
                        match which.as_str() {
                            // (Two, as the app asks about Multi-Roblox only when more than one window opens.)
                            "launch" => return r.launch("launch", json!({ "ids": idle }), false, cx),
                            "launcherr" => return r.launch("launch", json!({ "ids": idle.into_iter().take(1).chain(banned).collect::<Vec<_>>() }), false, cx),
                            "saveserver" => return pages::picker::shot_save(r, window, cx),
                            _ => {}
                        }
                        if let Some(cmd) = which.strip_prefix("send:") {
                            return r.send(cmd, json!({}));
                        }
                        // BLOXKEEP_SHOT_PW=2fa (a username the mock answers by): the sign-in sent with it, for its next step.
                        if let (true, Ok(user)) = (which == "add", std::env::var("BLOXKEEP_SHOT_PW")) {
                            pages::password::open(r, window, cx);
                            r.pl_user.update(cx, |s, cx| s.set_value(user, window, cx));
                            r.pl_pass.update(cx, |s, cx| s.set_value("hunter22", window, cx));
                            return pages::password::submit(r, window, cx);
                        }
                        // BLOXKEEP_SHOT_IMPORT=pick (the file chosen), =demo (opened with that password: the preview),
                        // =wrong… (a wrong one): the import's later steps.
                        if let (true, Ok(pw)) = (which == "import", std::env::var("BLOXKEEP_SHOT_IMPORT")) {
                            pages::import_vault::open(r, window, cx);
                            cx.spawn_in(window, async move |this, cx| {
                                cx.background_executor().timer(Duration::from_millis(600)).await;
                                let _ = this.update_in(cx, |r, window, cx| pages::import_vault::next(r, window, cx));
                                cx.background_executor().timer(Duration::from_millis(1500)).await;
                                if pw != "pick" {
                                    let _ = this.update_in(cx, |r, window, cx| {
                                        r.iv_pw.update(cx, |s, cx| s.set_value(pw.clone(), window, cx));
                                        pages::import_vault::next(r, window, cx);
                                    });
                                }
                            })
                            .detach();
                            return;
                        }
                    }
                    match which.as_str() {
                        "add" => return pages::password::open(r, window, cx),
                        "phone" => return pages::quick::open(r, false, window, cx),
                        "cookie" => return pages::account_form::open(r, None, window, cx),
                        "edit" => return pages::account_form::open(r, first, window, cx),
                        "import" => return pages::import_vault::open(r, window, cx),
                        "feedback" => return pages::extras::open_feedback(r, window, cx),
                        "whatsnew" => return pages::extras::ask_whats_new(r, cx),
                        "join" => return pages::join::open(r, first.into_iter().collect(), window, cx),
                        "vis" => return pages::visibility::open(r, r.accounts().iter().take(3).filter_map(|a| a.get("id").and_then(Value::as_str)).map(str::to_string).collect(), window, cx),
                        "order" => return pages::accounts::open_order(r, window, cx),
                        // (=palette quick search, =snap Accounts' Windows menu, =presets the bar's Presets menu,
                        // =savepreset its Save as preset….)
                        "palette" => return pages::palette::open(r, window, cx),
                        "snap" => return pages::snap::shot(r, cx),
                        "presets" => return pages::preset_menu::shot(r, cx),
                        "savepreset" => return pages::preset_menu::save_as(r, window, cx),
                        _ => {}
                    }
                    if which == "help" {
                        return r.multi_help(window, cx);
                    }
                    if which == "multi" {
                        return r.confirm(r.tr("set.multiConfirmTitle"), r.tr("set.multiConfirm"), r.tr("set.multiOk"), false, Some("multi"), window, cx, |_, _| {});
                    }
                    r.confirm(r.tr("ps.deleteTitle"), r.i18n.tv("ps.deleteQ", &[("name", "Farm Blox Fruits")]), r.tr("ps.delete"), true, Some("deletePreset"), window, cx, |_, _| {});
                });
            }
        }
        // The cover over the window comes off once there's something to see (ShellForm.CoverOffAsync).
        if !self.shown && !self.state.is_null() {
            self.shown = true;
            let link = self.link.clone();
            cx.on_next_frame(window, move |_, _, _| {
                if let Some(l) = &link {
                    l.send("pageShown", Value::Null);
                }
            });
        }
        self.want_home_data(cx);
        // The window takes the keys from the start (Ctrl+K, Tab), before anything in it is clicked.
        if window.focused(cx).is_none() {
            let f = self.focus.clone();
            cx.on_next_frame(window, move |_, window, cx| {
                if window.focused(cx).is_none() {
                    window.focus(&f, cx);
                }
            });
        }
        // The lock screen is up: its password box takes the keyboard, once.
        let locked = pages::lock::up(self) && !self.lost;
        if locked && !self.lock.focused {
            self.lock.focused = true;
            let h = self.lock_pw.read(cx).focus_handle(cx);
            window.focus(&h, cx);
            // BLOXKEEP_SHOT_UNLOCK="secret": a hidden shot types that password (in both boxes) and sends it.
            #[cfg(feature = "shot")]
            if let Ok(pw) = std::env::var("BLOXKEEP_SHOT_UNLOCK") {
                self.lock_pw.update(cx, |s, cx| s.set_value(pw.clone(), window, cx));
                self.lock_pw2.update(cx, |s, cx| s.set_value(pw, window, cx));
                pages::lock::submit(self, window, cx);
            }
        }
        let narrow = window.viewport_size().width < px(NARROW);
        let page = self.page;
        // (Line lost: nothing of the app is left but the words saying so; the accounts' column, the rail and the status
        // bar would show the last figures as if they were live.)
        let lost = self.lost && !locked;
        // (No accounts yet: no column of them either; it stood tall and empty beside the welcome.)
        let with_side = !narrow && page == Page::Home && !locked && !lost && !self.accounts().is_empty();
        let main_w = f32::from(window.viewport_size().width) - RAIL - (GAP + 4.) - GAP - if with_side { SIDE + GAP } else { 0. };
        self.narrow.set(main_w < 1000.);
        self.window_h.set(f32::from(window.viewport_size().height));
        // (The column that isn't beside Home can slide in over it: see `no_side`.)
        let can_slide = narrow && page == Page::Home && !locked && !lost && !self.accounts().is_empty();
        self.no_side.set(can_slide);
        // (A hidden shot of the column slid in: BLOXKEEP_SHOT_SIDE=1.)
        #[cfg(feature = "shot")]
        if can_slide && std::env::var("BLOXKEEP_SHOT_SIDE").as_deref() == Ok("1") {
            self.side_open = true;
        }
        if !can_slide {
            self.side_open = false;
        }
        let slid = (can_slide && self.side_open).then(|| {
            div()
                .id("side-slid")
                .absolute()
                .top_0()
                .right_0()
                .bottom_0()
                .occlude()
                .rounded(px(R_PANEL))
                .shadow_lg()
                .child(self.side(cx))
                .on_mouse_down_out(cx.listener(|r, _, _, cx| {
                    r.side_open = false;
                    r.side_shut = Some(std::time::Instant::now());
                    cx.notify();
                }))
        });
        let body = if locked {
            div().size_full().child(pages::lock::lock(self, window, cx))
        } else if self.lost {
            panel().size_full().flex().flex_col().items_center().justify_center().gap(px(10.)).child(
                icon("power", 30., c(WARN)).flex_none().mb(px(6.)),
            )
            .child(div().text_size(px(T_GROUP)).font_weight(FontWeight::SEMIBOLD).child(self.tr("ui.lost")))
            .child(div().text_size(px(T_L)).text_color(c(MUTED)).child(self.tr("ui.lostSub")))
        } else {
            if page == Page::Home {
                div().size_full().child(pages::home::home(self, main_w, cx))
            } else if page == Page::Accounts {
                div().size_full().child(pages::accounts::accounts(self, main_w, cx))
            } else if page == Page::Games {
                div().size_full().child(pages::games::games(self, main_w, cx))
            } else if page == Page::Auto {
                div().size_full().child(pages::auto::auto(self, main_w, cx))
            } else if page == Page::Stats {
                div().size_full().child(pages::stats::stats(self, main_w, cx))
            } else if page == Page::Settings {
                div().size_full().child(pages::settings::settings(self, main_w, cx))
            } else {
                panel().size_full().overflow_hidden().child(pages::not_yet(self, page))
            }
        };
        let strip = if with_side || self.locked() || self.lost { None } else { self.drop_strip(cx) };
        // A theme whose web page frosted its panels: the picture behind each, frosted, under its see-through fill (scene.rs).
        // (Glass on the blocks only: nothing frosted behind the panels and the rail, each block is its own glass.)
        let blocks = crate::theme::glass_blocks();
        let frost_main = if locked || blocks { None } else { crate::scene::frost(R_PANEL, window, cx) };
        // (The side panel holds the accounts: with glass on the blocks only it is one of the blocks.)
        let frost_side = match (with_side, blocks) {
            (true, false) => crate::scene::frost(R_PANEL, window, cx),
            (true, true) => Some(crate::kit::glass_pane(R_PANEL)),
            _ => None,
        };
        let frost_rail = if locked || lost || blocks { None } else { crate::scene::frost(0., window, cx) };
        // (The top bar's glass, where the picture is too light along its top for the bar's words: scene::top_glass.)
        let top_glass = if locked || lost { None } else { crate::scene::top_glass(window, cx) };
        div()
            .id("root")
            .track_focus(&self.focus)
            .key_context("Bloxkeep")
            // Ctrl+K: quick search over the app's actions, places, presets, games, accounts and Settings (pages/palette.rs).
            .on_action(cx.listener(|r, _: &FocusSearch, window, cx| pages::palette::open(r, window, cx)))
            .on_action(cx.listener(|_, _: &FocusNext, window, cx| window.focus_next(cx)))
            .on_action(cx.listener(|_, _: &FocusPrev, window, cx| window.focus_prev(cx)))
            .on_action(cx.listener(|r, _: &GoBack, _, cx| {
                r.step(false);
                cx.notify();
            }))
            .on_action(cx.listener(|r, _: &GoForward, _, cx| {
                r.step(true);
                cx.notify();
            }))
            // Ctrl+F: this page's own search (Settings', Presets'), else the accounts' search on Accounts.
            .on_action(cx.listener(|r, _: &FindHere, window, cx| {
                if r.locked() || window.has_active_dialog(cx) {
                    return cx.propagate();
                }
                let field = match r.page {
                    Page::Settings => r.set_q.clone(),
                    Page::Auto if pages::auto::preset_search_shown(r) => r.preset_search.clone(),
                    _ => {
                        r.go(Page::Accounts);
                        r.acc_search.clone()
                    }
                };
                cx.notify();
                field.update(cx, |s, cx| {
                    s.focus(window, cx);
                    s.select_all(window, cx);
                });
            }))
            // Ctrl+L, as a browser's address bar, or "/" (no box focused): the game box on Accounts' bar, which shows for
            // it even with none picked (app.js ~3222).
            .on_action(cx.listener(|r, _: &FocusPlace, window, cx| {
                if r.locked() || window.has_active_dialog(cx) {
                    return cx.propagate();
                }
                r.go(Page::Accounts);
                pages::accounts::want_place(r.nav, cx);
                cx.notify();
                r.place.update(cx, |s, cx| {
                    s.focus(window, cx);
                    s.select_all(window, cx);
                });
            }))
            .on_action(cx.listener(|r, _: &CardLeft, window, cx| Self::card_key(r, pages::accounts::CardKey::Left, window, cx)))
            .on_action(cx.listener(|r, _: &CardRight, window, cx| Self::card_key(r, pages::accounts::CardKey::Right, window, cx)))
            .on_action(cx.listener(|r, _: &CardUp, window, cx| Self::card_key(r, pages::accounts::CardKey::Up, window, cx)))
            .on_action(cx.listener(|r, _: &CardDown, window, cx| Self::card_key(r, pages::accounts::CardKey::Down, window, cx)))
            .on_action(cx.listener(|r, _: &CardFirst, window, cx| Self::card_key(r, pages::accounts::CardKey::First, window, cx)))
            .on_action(cx.listener(|r, _: &CardLast, window, cx| Self::card_key(r, pages::accounts::CardKey::Last, window, cx)))
            .on_action(cx.listener(|r, _: &CardPick, window, cx| Self::card_key(r, pages::accounts::CardKey::Pick, window, cx)))
            .on_action(cx.listener(|r, _: &PickAll, window, cx| Self::acc_key(r, pages::accounts::Key::All, window, cx)))
            .on_action(cx.listener(|r, _: &DeletePicked, window, cx| Self::acc_key(r, pages::accounts::Key::Delete, window, cx)))
            .on_action(cx.listener(|r, _: &EditPicked, window, cx| Self::acc_key(r, pages::accounts::Key::Edit, window, cx)))
            .on_action(cx.listener(|r, _: &PlayPicked, window, cx| Self::acc_key(r, pages::accounts::Key::Play, window, cx)))
            .on_action(cx.listener(|r, _: &ClearPicked, window, cx| {
                // Esc with a status-bar pill open: it closes first.
                if r.pill.is_some() {
                    r.pill = None;
                    return cx.notify();
                }
                // Esc in a preset's or a macro's form: leaving it, asking first when something would be lost (app.js's
                // askClose).
                if r.page == Page::Auto && !r.locked() && !window.has_active_dialog(cx) {
                    if r.auto.macro_form.is_some() {
                        return pages::macro_form::leave(r, window, cx);
                    }
                    if r.auto.form.is_some() {
                        return pages::preset_form::leave(r, window, cx);
                    }
                }
                // Esc in the bar's game box: its list closed first, else out of the box (app.js blurred it), and the bar
                // that showed only for it (Ctrl+L or "/" with none picked) goes.
                if r.page == Page::Accounts && !r.locked() && !window.has_active_dialog(cx) && r.place.read(cx).focus_handle(cx).is_focused(window) {
                    if r.picker.open {
                        pages::picker::close(r);
                    } else {
                        pages::accounts::unwant_place(cx);
                        window.focus(&r.focus, cx);
                    }
                    return cx.notify();
                }
                Self::acc_key(r, pages::accounts::Key::Clear, window, cx)
            }))
            .size_full()
            .relative()
            .flex()
            .bg(c(GROUND))
            // A picture theme: its scene under everything (scene.rs).
            .children(crate::scene::layer(window, cx))
            .font_family(FONT)
            .text_color(c(TEXT))
            .text_size(px(T_M))
            // (Locked: only the lock screen and the window's own buttons; nothing of the app to click.)
            .when(!locked && !lost, |d| d.child(div().relative().h_full().flex_none().children(frost_rail).child(self.rail(window, cx))))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    .pl(px(GAP + 4.))
                    .pr(px(GAP))
                    // (No status bar under it when the line is lost: the panel keeps its gap from the window's foot.)
                    .when(lost, |d| d.pb(px(GAP)))
                    .child(div().relative().flex_none().when(top_glass.is_some(), |d| d.mb(px(GAP))).children(top_glass).child(app_in("app-in-top", self.intro, 60., -10., false, self.top(with_side, locked || lost, cx))))
                    .children(strip)
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .flex()
                            .gap(px(GAP))
                            // (Glass blocks know they are on the page, the picture behind them, by these two marks.)
                            .child(crate::scene::on_page(true))
                            // (Cut at its own edge: the Roblox tab's next row of game pictures showed as a flickering
                            // line under the status bar while the page scrolled, the owner's OBS video, 2026-11-15.)
                            .child(div().flex_1().min_w_0().h_full().relative().overflow_hidden().children(frost_main).child(app_in("app-in-page", self.intro, 140., 26., true, step_in(self.shown_key(), self.step_from, body))).children(slid))
                            .when(with_side, |d| d.child(div().relative().h_full().flex_none().children(frost_side).child(self.side(cx))))
                            .child(crate::scene::on_page(false)),
                    )
                    // (Glass: the gutter the panels keep between them, above it too.)
                    .when(!locked && !lost, |d| d.child(app_in("app-in-status", self.intro, 260., 18., false, self.status(with_side, window, cx)))),
            )
            .children(pill_card)
            .children(tray_shot)
            .children(toast)
            .child(crate::kit::mouse_watch())
            .child(self.live.reporter(self.link.clone()))
            // (A theme coming in: the old look over everything, fading out.)
            .children(crate::scene::fade_layer(window, cx))
    }
}

/// Where the live pictures of the Roblox windows go, for the app to lay them there (MonitorTab: Windows draws them over
/// this window, so the page says the boxes, CSS-like pixels from its top-left, and the scale). Gathered anew each frame:
/// the area the pictures may show in (the accounts' scrolling part), each picture's box, and what must not be covered
/// (the toast); a box under a cover is left out. Sent only when it changed, and { hidden } once there is no area.
#[derive(Default, Clone)]
pub struct Live {
    height: std::rc::Rc<std::cell::Cell<Option<f32>>>,
    area: std::rc::Rc<std::cell::RefCell<Option<Bounds<Pixels>>>>,
    boxes: std::rc::Rc<std::cell::RefCell<Vec<(String, Bounds<Pixels>)>>>,
    covers: std::rc::Rc<std::cell::RefCell<Vec<Bounds<Pixels>>>>,
    sent: std::rc::Rc<std::cell::RefCell<String>>,
}

impl Live {
    fn start(&self) {
        *self.area.borrow_mut() = None;
        self.boxes.borrow_mut().clear();
        self.covers.borrow_mut().clear();
    }

    pub fn area_height(&self) -> Option<f32> {
        self.height.get()
    }

    /// Fills its parent: the pictures may show there.
    pub fn area(&self) -> impl IntoElement {
        let area = self.area.clone();
        let height = self.height.clone();
        canvas(
            move |b, window, _| {
                *area.borrow_mut() = Some(b);
                let measured = Some(f32::from(b.size.height).floor());
                if height.replace(measured) != measured {
                    window.refresh();
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0()
    }

    /// Fills its parent: window id's picture goes there.
    pub fn picture(&self, id: String) -> impl IntoElement {
        let boxes = self.boxes.clone();
        canvas(move |b, _, _| boxes.borrow_mut().push((id, b)), |_, _, _, _| {}).absolute().inset_0()
    }

    /// Fills its parent: no picture over it.
    pub fn cover(&self) -> impl IntoElement {
        let covers = self.covers.clone();
        canvas(move |b, _, _| covers.borrow_mut().push(b), |_, _, _, _| {}).absolute().inset_0()
    }

    /// Painted after everything was laid out: tells the app what changed.
    fn reporter(&self, link: Option<Link>) -> impl IntoElement {
        let me = self.clone();
        canvas(
            |_, _, _| {},
            move |_, _, window, cx| {
                let n = |p: Pixels| (f64::from(f32::from(p)) * 100.).round() / 100.;
                // A dialog (quick search, a form, a question) is drawn over the page, but Windows draws the pictures
                // above the whole window: none while one is open (owner 2026-11-15: a window's picture over Ctrl+K).
                let area = if window.has_active_dialog(cx) { None } else { *me.area.borrow() };
                let msg = match area {
                    Some(a) => {
                        let covers = me.covers.borrow();
                        let pictures: Vec<Value> = me
                            .boxes
                            .borrow()
                            .iter()
                            .filter(|(_, b)| !covers.iter().any(|c| c.intersects(b)))
                            .map(|(id, b)| json!({ "id": id, "x": n(b.origin.x), "y": n(b.origin.y), "w": n(b.size.width), "h": n(b.size.height) }))
                            .collect();
                        json!({ "x": n(a.origin.x), "y": n(a.origin.y), "w": n(a.size.width), "h": n(a.size.height), "scale": window.scale_factor(), "pictures": pictures })
                    }
                    None => json!({ "hidden": true }),
                };
                let text = msg.to_string();
                if *me.sent.borrow() != text {
                    *me.sent.borrow_mut() = text;
                    if let Some(l) = &link {
                        l.send("monitorArea", msg);
                    }
                }
            },
        )
        .absolute()
        .size_0()
    }
}


/// A made-up moment for the capsule in a shot (BLOXKEEP_SHOT_LIVE).
#[cfg(feature = "shot")]
fn shot_live(r: &Root, kind: &str) -> pages::games::Live {
    let join = pages::games::LiveAct::Join { acc: String::new(), place: 2753915549, job: "x".into(), user: 1, name: "minh".into() };
    let (icon, tone, title, sub, act, busy) = match kind {
        "friend" => ("users", OK, "Minh".to_string(), r.i18n.tv("live.inGame", &[("game", "Blox Fruits")]), Some(join), false),
        "item" => ("tag", OK, "Valkyrie Helm".to_string(), "1,200 \u{2192} 950 R$".to_string(), Some(pages::games::LiveAct::Watching), false),
        "done" => ("check", OK, "Skybox Collection".to_string(), r.i18n.tv("mods.added", &[("n", "54")]), Some(pages::games::LiveAct::Mods), false),
        _ => ("layers", TEXT_2, "Skybox Collection".to_string(), r.tr("live.installing"), None, true),
    };
    pages::games::Live { key: "shot".into(), pic: std::env::var("BLOXKEEP_SHOT_LIVE_PIC").unwrap_or_default(), round: kind == "friend", icon, tone, title, sub, act, busy, at: std::time::Instant::now() }
}


/// One of Spotify's controls: sent to Spotify, and the play mark follows at once (the next read says what Spotify
/// really did).
fn music_press(id: &'static str, name: &'static str, tip: String, act: crate::media::Act, cx: &mut Context<Root>) -> IconButton {
    IconButton::new(id, name).tip(tip).on_click(cx.listener(move |r, _, _, cx| music_go(r, act, cx)))
}

/// Sends a music key, then reads the song again at once and twice more shortly after: Spotify takes a moment to say
/// its next song, and the card waited for the regular look (up to 2 s; the owner, 2026-11-18: "hơi chậm").
fn music_go(r: &mut Root, act: crate::media::Act, cx: &mut Context<Root>) {
    cx.spawn(async move |this, cx| {
        cx.background_executor().spawn(async move { crate::media::act(act) }).await;
        for wait in [120u64, 350, 700] {
            cx.background_executor().timer(Duration::from_millis(wait)).await;
            let now = cx.background_executor().spawn(async { crate::media::now() }).await;
            let alive = this.update(cx, |r, cx| {
                if r.music != now {
                    r.music = now;
                    cx.notify();
                }
            });
            if alive.is_err() {
                break;
            }
        }
    })
    .detach();
    if let (crate::media::Act::Toggle, Some(m)) = (act, r.music.as_mut()) {
        m.playing = !m.playing;
    }
    cx.notify();
}
