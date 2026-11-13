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

use crate::{
    i18n::I18n,
    images,
    kit::*,
    link::{Event, Link},
    pages::{self, Page},
    theme::*,
};

actions!(bloxkeep, [FocusSearch, FocusNext, FocusPrev, GoBack, GoForward, FindHere, FocusPlace, PickAll, DeletePicked, EditPicked, PlayPicked, ClearPicked]);

/// Home's games card: this PC's games, or what Bloxkeep's users play.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum HomeTab {
    Games,
    Community,
}

pub struct Root {
    pub link: Option<Link>,
    pub i18n: I18n,
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
    pub toast: Option<(String, String)>,
    /// The notice last shown and the count it came with (drawn while it leaves), and how many were said so far.
    toast_was: ((String, String), u32),
    toast_n: u32,
    shot_asked: bool,
    /// This window's handle, for moving the app's window from the top bar (frame.rs).
    pub hwnd: Option<isize>,
    /// The games this PC played lately (playedGames), asked for once Home is shown.
    pub played: Option<Vec<Value>>,
    asked_played: bool,
    /// Each game's pictures by place (gameLooks: icon, cover), and the places already asked for.
    pub looks: std::collections::HashMap<i64, Value>,
    asked_looks: std::collections::HashSet<i64>,
    /// The game on Home's stage, picked from its games.
    /// Which way the place shown came in: 1. from below, -1. from above (kit::step_in).
    pub step_from: f32,
    pub picked: Option<i64>,
    /// Accounts: the cards picked (by id), the filter over them, the game box of the bar that plays them and the game
    /// it found (resolveGame's answer).
    pub selected: std::collections::HashSet<String>,
    pub acc_filter: pages::accounts::Filter,
    pub place: Entity<InputState>,
    pub game: Option<Value>,
    /// Accounts' live pictures: the Roblox windows as the app sent them last ("monitor": every 1.5 s while the page
    /// says where their pictures go), and that report, gathered while the window is laid out each frame.
    pub windows: Vec<Value>,
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
    /// Settings › Launching: a Roblox window's own width and height, typed.
    pub win_w: Entity<InputState>,
    pub win_h: Entity<InputState>,
    pub pw_cur: Entity<InputState>,
    pub pw_new: Entity<InputState>,
    pub pw_new2: Entity<InputState>,
    /// The vault has been open in this run: a lock after that says what kept running.
    pub seen_open: bool,
    /// Accounts: the picked accounts' "more" menu is open.
    pub acc_menu: bool,
    /// Accounts' short lists ("Group: All", "Sort: Name"): the one open, the one last open (it closes as it
    /// opened), and the one a click outside closed just now (that click on its own button must not reopen it).
    pub acc_drop: Option<&'static str>,
    pub acc_drop_was: Option<&'static str>,
    pub acc_drop_closed: Option<(&'static str, std::time::Instant)>,
    /// Accounts: only this group's (none: every group).
    pub acc_group: Option<String>,
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
    pub af_note: Entity<InputState>,
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
    /// Boxes to open once there is a window to open them in (a launch's answer, a remote-control request).
    pending: Vec<Pending>,
    /// The remote-control request already asked about (its id, user and who it replaces).
    remote_asked: String,
    /// The open vault and its accounts' ids, as last seen: a change asks the lent sign-ins again.
    sign_key: String,
    pub servers: pages::servers::Data,
    /// Quick search (Ctrl+K), Accounts' Windows menu and the selection bar's Presets menu.
    pub palette: pages::palette::Data,
    pub snap: pages::snap::Data,
    pub preset_menu: pages::preset_menu::Data,
    /// The inbox's chat box: what to say in the open chat (Enter).
    pub chat_box: Entity<InputState>,
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
    pub home_tab: HomeTab,
    /// Where the left button went down on the top bar; moving a few pixels from there moves the window.
    drag_from: Option<Point<Pixels>>,
    lost: bool,
    shown: bool,
    /// The window's own focus, so the keys (Ctrl+K, Tab) work before anything is clicked.
    focus: FocusHandle,
    tasks: Vec<Task<()>>,
    /// The shown toast's timer: a new toast drops (cancels) the one before.
    toast_task: Option<Task<()>>,
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
        // Enter asks Roblox; an empty box goes back to the charts.
        cx.subscribe(&game_search, |r: &mut Root, _, e: &InputEvent, cx| match e {
            InputEvent::PressEnter { .. } => pages::games::search(r, cx),
            InputEvent::Change if r.game_search.read(cx).value().trim().is_empty() => {
                r.games.found = None;
                cx.notify();
            }
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
        // The game list's own search: Enter asks Roblox; an empty box goes back to the lists.
        let picker_q = mk("ui.findGame", window, cx);
        cx.subscribe(&picker_q, |r: &mut Root, _, e: &InputEvent, cx| match e {
            InputEvent::PressEnter { .. } => pages::picker::search(r, cx),
            InputEvent::Change if r.picker_q.read(cx).value().trim().is_empty() => {
                pages::picker::cleared(r);
                cx.notify();
            }
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
        let af_note = af("", false, 0, window, cx);
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
        let shop_search = mk("shop.find", window, cx);
        cx.subscribe(&shop_search, |r: &mut Root, _, e: &InputEvent, cx| match e {
            InputEvent::PressEnter { .. } => pages::profile::shop_find(r, cx),
            InputEvent::Change if r.shop_search.read(cx).value().trim().is_empty() => pages::profile::shop_find(r, cx),
            _ => {}
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
        // Enter looks the game up; any change forgets the game found before (Play then says "Open Roblox" or "Play").
        cx.subscribe(&place, |r: &mut Root, _, e: &InputEvent, cx| match e {
            InputEvent::PressEnter { .. } => r.resolve_place(cx),
            InputEvent::Change => {
                r.game = None;
                cx.notify();
            }
            _ => {}
        })
        .detach();
        let mut root = Self {
            link: None,
            i18n,
            state: Value::Null,
            metrics: Value::Null,
            page: Page::Home,
            acc_search,
            progress: None,
            toast: None,
            toast_was: Default::default(),
            toast_n: 0,
            shot_asked: false,
            step_from: 1.,
            hwnd: None,
            played: None,
            asked_played: false,
            looks: Default::default(),
            asked_looks: Default::default(),
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
            rset_about,
            passes: Default::default(),
            lock: Default::default(),
            lock_pw,
            lock_pw2,
            vault_name,
            s_vault_name,
            board_name,
            win_w,
            win_h,
            pw_cur,
            pw_new,
            pw_new2,
            seen_open: false,
            acc_menu: false,
            acc_drop: None,
            acc_drop_was: None,
            acc_drop_closed: None,
            acc_group: None,
            order_edit: Vec::new(),
            help_open: false,
            jp_name,
            af_cookie,
            af_alias,
            af_group,
            af_password,
            af_key,
            af_note,
            acc_form: Default::default(),
            fb_text,
            fb_contact,
            extras: Default::default(),
            picker: Default::default(),
            picker_q,
            sv_name,
            sv_code,
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
            snap: Default::default(),
            preset_menu: Default::default(),
            chat_box,
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
            selected: Default::default(),
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
            tasks: Vec::new(),
            toast_task: None,
        };
        // The playing times count up each second while someone plays.
        root.tasks.push(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let alive = this.update(cx, |r, cx| {
                    if r.page == Page::Home && r.accounts().iter().any(|a| a.get("state").and_then(Value::as_str) == Some("running")) {
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
                let p = self.i18n.t("prices.search");
                self.price_search.update(cx, |s, cx| s.set_placeholder(p, window, cx));
                let p = self.i18n.t("jp.placeholder");
                self.jp_name.update(cx, |s, cx| s.set_placeholder(p, window, cx));
                for (key, e) in [("ui.findGame", self.picker_q.clone()), ("acc.search", self.iv_q.clone()), ("cm.namePh", self.board_name.clone()), ("wiki.searchWiki", self.wiki_q.clone()), ("prices.search", self.wiki_tq.clone()), ("set.search", self.set_q.clone())] {
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
        let on_board = self.state.get("boardName").and_then(Value::as_str).unwrap_or_default().to_string();
        if self.board_name.read(cx).value().as_ref() != on_board && !self.board_name.read(cx).focus_handle(cx).is_focused(window) {
            self.board_name.update(cx, |s, cx| s.set_value(on_board, window, cx));
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
        if sign_key != self.sign_key {
            let first = self.sign_key.is_empty();
            self.sign_key = sign_key;
            if !first {
                self.games.forget_sign_in();
                self.friends.forget_sign_in();
            }
        }
        if was != self.locked() {
            if self.locked() {
                // Nothing stays over the lock screen: the boxes open close, and what was typed into a secret box
                // (a cookie, a password, an authenticator key) goes with them.
                window.close_all_dialogs(cx);
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
        }
        // BLOXKEEP_SHOT_DROP=sort|group: Accounts' short list open; BLOXKEEP_SHOT_HOMETAB=community: Home's games card on
        // what Bloxkeep's users play; BLOXKEEP_SHOT_LOST=1: the window as when the line to the app is gone.
        #[cfg(feature = "shot")]
        {
            match std::env::var("BLOXKEEP_SHOT_DROP").as_deref() {
                Ok("sort") => self.acc_drop = Some("acc-sort"),
                Ok("group") => self.acc_drop = Some("acc-group"),
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
            // BLOXKEEP_SHOT_SHOP=free: on its New free list.
            self.profile.free = kind == "free";
        }
        // BLOXKEEP_SHOT_FIND="anime": a shot starts with that search sent to Roblox.
        #[cfg(feature = "shot")]
        if let (Ok(q), true) = (std::env::var("BLOXKEEP_SHOT_FIND"), self.games.found.is_none()) {
            self.game_search.update(cx, |s, cx| s.set_value(q, window, cx));
            pages::games::search(self, cx);
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
            // BLOXKEEP_SHOT_PLAY=1: with Play's row open, the first two free accounts ticked.
            if std::env::var("BLOXKEEP_SHOT_PLAY").is_ok() {
                let open_id = self.games.open.as_ref().map(|o| o.id.clone()).unwrap_or_default();
                let ticked = self
                    .accounts()
                    .iter()
                    .filter(|a| pages::accounts::can_play(a))
                    .filter_map(|a| a.get("id").and_then(Value::as_str).map(str::to_string))
                    .take(2)
                    .collect();
                self.games.picking = Some((open_id, ticked));
            }
        }
        cx.notify();
    }

    /// An Accounts key (Ctrl+A, Delete, F2, Enter, Escape): only on that page, with nothing typed into a box and no
    /// dialog open; Enter only when the page itself has the focus (on a button or a card it presses that).
    fn acc_key(r: &mut Self, k: pages::accounts::Key, window: &mut Window, cx: &mut Context<Self>) {
        let typing = window.focused_input(cx).is_some() || window.context_stack().iter().any(|c| c.contains("Input"));
        let on_page = window.focused(cx).is_none_or(|f| f == r.focus);
        let fits = r.page == Page::Accounts && !r.locked() && !typing && !window.has_active_dialog(cx) && (on_page || !matches!(k, pages::accounts::Key::Play));
        if !(fits && pages::accounts::key(r, k, window, cx)) {
            cx.propagate();
        }
    }

    /// Opens a page and remembers the one it leaves for Back.
    pub fn go(&mut self, p: Page) {
        if p != self.page {
            // (A place further down the rail comes in from below, one further up from above.)
            let at = |x: Page| Page::ALL.iter().position(|y| *y == x).unwrap_or(0);
            self.step_from = if at(p) >= at(self.page) { 1. } else { -1. };
            self.back.push(self.page);
            self.back.truncate(50);
            self.fwd.clear();
            self.page = p;
        }
    }

    fn step(&mut self, forward: bool) {
        let (from, to) = if forward { (&mut self.fwd, &mut self.back) } else { (&mut self.back, &mut self.fwd) };
        if let Some(p) = from.pop() {
            to.push(self.page);
            self.page = p;
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
                cx.notify();
            }
            "progress" => {
                self.progress = data.get("text").and_then(Value::as_str).map(str::to_string);
                cx.notify();
            }
            "toast" => {
                let text = data.get("text").and_then(Value::as_str).unwrap_or_default().to_string();
                let kind = data.get("kind").and_then(Value::as_str).unwrap_or("info").to_string();
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
        // Games: whose the first tab is (the app's pick) is asked at once, so its tab can say.
        if self.page == Page::Games && !self.state.is_null() && !self.locked() {
            pages::friends::want_sign(self, &link, cx);
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
                _ => {}
            }
        }
        // (Every page lays a game's picture under its top now, 2026-11-13: the games and their pictures are asked on
        // any page, once each. Statistics opened first showed none.)
        if self.state.is_null() || self.locked() {
            return;
        }
        if self.page == Page::Games {
            pages::games::want(self, cx);
        }
        if !self.asked_played {
            self.asked_played = true;
            // The stays come raw from the app, and their sum is made here (history.rs; the app's playedGames command
            // made it until 2026-11-11).
            let rx = link.call("playSessions", json!({}));
            cx.spawn(async move |this, cx| {
                let got = rx.await.ok().and_then(Result::ok);
                let _ = this.update(cx, |r, cx| {
                    r.played = Some(match got {
                        Some(raw) => {
                            let (sessions, known, now) = crate::history::read(&raw);
                            crate::history::played(&sessions, &known, now).into_iter().map(|g| json!({ "name": g.name, "place": g.place, "hours": g.hours, "last": g.last })).collect()
                        }
                        None => Vec::new(),
                    });
                    cx.notify();
                });
            })
            .detach();
        }
        let mut places: Vec<i64> = crate::pages::home::games(self).iter().map(|g| g.place).collect();
        places.extend(self.accounts().iter().filter_map(|a| a.pointer("/session/game/placeId").and_then(Value::as_i64)));
        places.extend(
            self.state.pointer("/community/data/games").and_then(Value::as_array).into_iter().flatten().filter_map(|g| g.get("place").and_then(Value::as_i64)),
        );
        self.want_looks(places, cx);
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
        let rx = crate::roblox::run(move || crate::roblox::looks(&places));
        cx.spawn(async move |this, cx| {
            let Ok(got) = rx.await else { return };
            let _ = this.update(cx, |r, cx| {
                r.looks.extend(got);
                cx.notify();
            });
        })
        .detach();
    }

    /// Puts dropped accounts back in their game, as app.js's rejoinOne does for each (the server they were on is gone).
    pub fn rejoin(&mut self, accounts: &[Value], cx: &mut Context<Self>) {
        for a in accounts {
            let Some(id) = a.get("id").and_then(Value::as_str) else { continue };
            let place = a
                .pointer("/session/jobPlaceId")
                .and_then(Value::as_i64)
                .or_else(|| a.pointer("/session/game/placeId").and_then(Value::as_i64))
                .unwrap_or(0);
            let input = if place > 0 { format!("https://www.roblox.com/games/{place}") } else { String::new() };
            self.launch("launch", json!({ "ids": [id], "appOnly": false, "placeInput": input, "jobId": "" }), false, cx);
        }
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
                cx.notify();
            });
        })
        .detach();
    }

    /// The app's question in a box over the window, as app.js's confirmBox: Cancel and the answer (red when it destroys
    /// something), and "Don't ask again" for the questions the app lets you skip (its SkippableConfirms), whose yes is
    /// then assumed. The box, its cover and how they come and go are GPUI Component's Dialog; the buttons are the kit's.
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
        args["force"] = force.into();
        let rx = link.call(cmd, args.clone());
        cx.spawn(async move |this, cx| {
            let answer = rx.await;
            let _ = this.update(cx, |r, cx| {
                match answer {
                    Ok(Ok(v)) => r.launch_done(cmd, args, v),
                    Ok(Err(e)) => r.toast = Some((e.to_string(), "error".into())),
                    Err(_) => {}
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn launch_done(&mut self, cmd: &'static str, args: Value, answer: Value) {
        // (Locked meanwhile: nothing to ask or report.)
        if !answer.is_object() || self.locked() {
            return;
        }
        let flag = |k: &str| answer.get(k).and_then(Value::as_bool).unwrap_or(false);
        let count = |k: &str| answer.get(k).and_then(Value::as_array).map_or(0, |l| l.len()) as i64;
        if flag("needsConfirm") {
            self.pending.push(Pending::LaunchAsk { cmd, args, answer });
            return;
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
            Pending::LaunchAsk { cmd, args, answer } => {
                let asks: Vec<String> = answer.get("asks").and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
                let can_skip = !asks.is_empty() && asks.iter().all(|k| ["multiOff", "watch", "memory"].contains(&k.as_str()));
                let (multi_off, not_ready) = (asks.iter().any(|k| k == "multiOff"), asks.iter().any(|k| k == "multiNotReady"));
                let mut choices = vec![Choice::new(self.tr("common.cancel"), 0, |_, _, _, _| {})];
                if not_ready {
                    choices.push(Choice::new(self.tr("t.fixMulti"), 0, |r, _, window, cx| r.multi_help(window, cx)));
                }
                {
                    let (args, asks) = (args.clone(), asks.clone());
                    choices.push(Choice::new(self.tr("t.openAnyway"), if multi_off || not_ready { 0 } else { 1 }, move |r, ticked, _, cx| {
                        if can_skip && ticked {
                            for k in &asks {
                                r.send("skipConfirm", json!({ "key": format!("launch.{k}") }));
                            }
                        }
                        r.launch(cmd, args.clone(), true, cx);
                    }));
                }
                if multi_off {
                    // Turned on (after its own warning), the launch runs again with its other questions still asked.
                    choices.push(Choice::new(self.tr("t.multiAndPlay"), 1, move |r, _, window, cx| {
                        let args = args.clone();
                        r.confirm(r.tr("set.multiConfirmTitle"), r.tr("set.multiConfirm"), r.tr("set.multiOk"), false, Some("multi"), window, cx, move |r, cx| {
                            let Some(link) = r.link.clone() else { return };
                            let rx = link.call("setSettings", json!({ "multiInstance": true }));
                            let args = args.clone();
                            cx.spawn(async move |this, cx| {
                                if let Ok(Ok(_)) = rx.await {
                                    let _ = this.update(cx, |r, cx| r.launch(cmd, args, false, cx));
                                }
                            })
                            .detach();
                        });
                    }));
                }
                let text = answer.get("message").and_then(Value::as_str).unwrap_or_default().to_string();
                let tick = can_skip.then(|| self.tr("common.dontAsk"));
                self.ask_box(self.tr("t.confirmTitle"), text, Vec::new(), tick, true, choices, window, cx);
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
                    div().w_full().px(px(24.)).pt(px(16.)).pb(px(12.)).border_b_1().border_color(c(LINE)).text_size(px(18.)).line_height(px(24.)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(title.clone()),
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
                    .child(div().flex().justify_center().child(div().size(px(48.)).rounded_full().bg(ca(OK, 0.16)).flex().items_center().justify_center().child(icon("check", 24., c(OK)))))
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
    fn close_all_now(&mut self, force: bool, cx: &mut Context<Self>) {
        let Some(link) = self.link.clone() else { return };
        let rx = link.call("closeAll", json!({ "force": force }));
        cx.spawn(async move |this, cx| {
            let answer = rx.await;
            let _ = this.update(cx, |r, cx| {
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
        })
        .detach();
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
            cx.notify();
            return;
        }
        let _ = link;
        let asked = self.resolve_game(input);
        cx.spawn(async move |this, cx| {
            let Some(g) = asked.await else { return };
            let _ = this.update(cx, |r, cx| {
                let g = r.worded(g);
                if g.get("ok").and_then(Value::as_bool) == Some(false) {
                    let why = g.get("error").and_then(Value::as_str).unwrap_or_default().to_string();
                    if !why.is_empty() {
                        r.toast = Some((why, "warn".into()));
                    }
                }
                r.game = Some(g);
                cx.notify();
            });
        })
        .detach();
    }

    fn dropped(&self) -> Vec<Value> {
        self.accounts().iter().filter(|a| pages::home::is_dropped(a)).cloned().collect()
    }

    pub fn accounts(&self) -> &[Value] {
        self.state.get("accounts").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default()
    }

    /// The size typed for a Roblox window goes to the app, within what it takes (160–7680 by 120–4320; a box that
    /// holds no number counts as the usual 640 by 480), when it isn't the size already kept.
    fn save_window_size(&mut self, cx: &mut Context<Self>) {
        let num = |e: &Entity<InputState>, lo: i64, hi: i64, or: i64| e.read(cx).value().trim().parse::<f64>().ok().map(|v| v.round() as i64).filter(|v| *v > 0).unwrap_or(or).clamp(lo, hi);
        let (w, h) = (num(&self.win_w, 160, 7680, 640), num(&self.win_h, 120, 4320, 480));
        let kept = |key: &str, or: i64| self.state.pointer(key).and_then(Value::as_i64).unwrap_or(or);
        if w != kept("/settings/windowWidth", 640) || h != kept("/settings/windowHeight", 480) {
            self.send("setSettings", json!({ "windowWidth": w, "windowHeight": h }));
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
        format!("games-{}-{}-{}-{}", self.games.view as u8, self.tabs.at, open, self.profile.part())
    }

    /// The app's state, come as a command's answer (the lock screen's unlock).
    pub fn take_state(&mut self, state: Value, window: &mut Window, cx: &mut Context<Self>) {
        self.set_state(state, window, cx);
    }

    fn rail(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
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
            .child(div().h(px(TOP)).flex_none().flex().items_center().child(img(crate::crisp::logo(32., window)).size(px(32.))))
            .child(div().mt(px(14.)).flex().flex_col().gap(px(6.)).children(
                {
                    // (As the window first shows, and each time it is unlocked, they come in from the left one after
                    // another: the rail isn't drawn while locked.)
                    let mut items = Vec::new();
                    // Games carries how much news hasn't been read (99+ at most), or a dot when some is fresh.
                    let unread = self.state.pointer("/feeds/unread").and_then(Value::as_i64).unwrap_or(0);
                    let fresh = self.state.pointer("/feeds/fresh").and_then(Value::as_bool).unwrap_or(false);
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
            // Switch vault (its face), once there are several: the open vault locks only when another is picked.
            .when(self.state.pointer("/vaults/list").and_then(Value::as_array).is_some_and(|l| l.len() > 1), |d| {
                let v = self.state.get("vaults").cloned().unwrap_or(Value::Null);
                let (id, name) = (v.get("current").and_then(Value::as_str).unwrap_or_default().to_string(), v.get("name").and_then(Value::as_str).unwrap_or_default().to_string());
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
                        .on_click(cx.listener(|r, _, _, _| r.send("pickVault", json!({})))),
                )
            })
            .child(IconButton::new("lock", "lock").size(42.).tip(self.tr("ui.lock")).on_click(cx.listener(|r, _, _, _| r.send("lock", Value::Null))))
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
                    // Without the accounts column (a narrow window, or another place) Add account comes up here.
                    // Accounts has its own Add (the + over its list).
                    .when(!with_side && self.page != Page::Accounts && !locked, |d| {
                        d.child(IconButton::new("add-account-top", "userPlus").size(32.).tip(self.tr("acc.add")).on_click(cx.listener(|r, _, _, cx| r.add_account(cx))))
                            .child(div().w(px(8.)))
                    }),
            )
            .child(own(div().flex().items_center().gap(px(2.))).children([
                window_button("win-min", WinGlyph::Minimize, cx.listener(|r, _, _, _| r.send("window", json!({ "action": "minimize" })))),
                window_button("win-zoom", WinGlyph::Maximize, cx.listener(|r, _, _, _| r.send("window", json!({ "action": "zoom" })))),
                window_button("win-close", WinGlyph::Close, cx.listener(|r, _, _, _| r.send("window", json!({ "action": "close" })))),
            ]))
    }

    fn side(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let all = self.accounts();
        let query = self.acc_search.read(cx).value().to_lowercase();
        let accounts: Vec<&Value> = all
            .iter()
            .filter(|a| {
                query.is_empty()
                    || ["alias", "displayName", "username", "group", "note"]
                        .iter()
                        .any(|k| a.get(*k).and_then(Value::as_str).is_some_and(|v| v.to_lowercase().contains(&query)))
            })
            .collect();
        let playing: Vec<&Value> = accounts.iter().copied().filter(|a| a.get("state").and_then(Value::as_str) == Some("running")).collect();
        let dropped: Vec<&Value> = playing
            .iter()
            .copied()
            .filter(|a| a.pointer("/session/connection").and_then(Value::as_str) == Some("disconnected"))
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

    /// The bar along the foot. `compact` (a window under 1400 px): the meters say their figure without their bar, so
    /// CPU, RAM and GPU still fit beside Roblox's version and the mini window (at 1180 the row ran off the right).
    fn status(&self, width: f32, cx: &mut Context<Self>) -> impl IntoElement {
        // compact: the meters lose their bars; tight (the smallest windows): Roblox's version loses its words, the mini
        // window its name and the app its number, so the bar fits (it ran off the edge at 860 and at 1200 with the GPU shown, 2026-11-12).
        let (compact, tight) = (width < 1400., width < 1360.);
        let m = &self.metrics;
        let windows = m.get("robloxCount").and_then(Value::as_i64).unwrap_or(0);
        let ram = m.get("usedPct").and_then(Value::as_f64).unwrap_or(0.).round() as i64;
        let cpu = m.get("cpuPct").and_then(Value::as_f64).unwrap_or(0.).round() as i64;
        let multi = self.state.pointer("/multi/enabled").and_then(Value::as_bool).unwrap_or(false);
        let version = self.state.pointer("/update/current").and_then(Value::as_str).unwrap_or_default().to_string();
        let words = if windows == 0 { self.tr("sys.none") } else { self.i18n.tn("ui.robloxWindows", windows, &[]) };
        // An update is in and waits for a restart (not a quiet fix, which goes in by itself). Its button says the new
        // version, so the app's own number at the end steps aside while it shows: the bar ran off the window's edge.
        let ready = self.state.pointer("/update/status").and_then(Value::as_str) == Some("ready") && !self.state.pointer("/update/quiet").and_then(Value::as_bool).unwrap_or(false);
        div()
            .h(px(44.))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(18.))
            .px(px(6.))
            .text_size(px(T_S + 0.5))
            .text_color(c(TEXT_2))
            .font_features(tnum())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(div().size(px(7.)).rounded(px(2.)).bg(if windows > 0 { c(OK) } else { c(DIM) }))
                    .child(div().text_color(c(TEXT)).font_weight(FontWeight::MEDIUM).child(words))
                    // Close all: every Roblox window, after a question (the old page's status card had it).
                    .when(windows > 0, |d| {
                        d.child(IconButton::new("sys-close", "power").size(26.).tip(self.tr("sys.closeAll")).on_click(cx.listener(|r, _, window, cx| r.close_all(false, window, cx))))
                    }),
            )
            // (On every page: Home's CPU and RAM cards gave way to its account strip, critique 2026-11-13.)
            .map(|d| {
                d.child(div().w(px(1.)).h(px(16.)).bg(c(LINE)))
                    .child(meter("chip", self.tr("ui.cpu"), cpu, !compact))
                    .child(div().w(px(1.)).h(px(16.)).bg(c(LINE)))
                    .child(meter("gauge", self.tr("ui.ram"), ram, !compact))
                    // The GPU too while Roblox runs (the old PC card's "GPU 34 %").
                    .when_some(m.get("gpuPct").and_then(Value::as_f64).filter(|_| windows > 0), |d, g| {
                        d.child(div().w(px(1.)).h(px(16.)).bg(c(LINE))).child(meter("monitor", "GPU".into(), g.round() as i64, !compact))
                    })
            })
            .when_some(self.progress.clone(), |d, p| d.child(div().text_color(c(TEXT_2)).child(p)))
            // Nothing can be opened without it: said where it is seen on every place.
            .when(self.state.get("robloxInstalled").and_then(Value::as_bool) == Some(false), |d| {
                d.child(div().min_w_0().flex().items_center().gap(px(6.)).text_color(c(WARN)).child(icon("alert", 14., c(WARN))).child(div().truncate().child(self.tr("side.noRoblox"))))
            })
            .child(div().flex_1())
            // Roblox's version here and whether it is the newest (the old PC card's last line); out of date, it leads
            // to Settings › Roblox version, which says how to update.
            .children(self.roblox_version(tight, cx))
            // The mini window: the accounts in a small window that stays over Roblox. Named, not a lone icon; open, a
            // green light in the icon's place.
            .child({
                let on = self.state.get("miniOpen").and_then(Value::as_bool).unwrap_or(false);
                let tip: SharedString = self.tr(if on { "sys.miniHide" } else { "sys.mini" }).into();
                tool("sys-mini", (!on).then_some("screens"), false)
                    .h(px(28.))
                    .px(px(8.))
                    .text_size(px(T_S + 0.5))
                    .when(on, |d| d.child(div().size(px(7.)).rounded_full().bg(c(OK))))
                    .when(!tight, |d| d.child(self.tr("sys.mini")))
                    .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
                    .on_click(cx.listener(|r, _, _, _| r.send("toggleMini", json!({}))))
            })
            .when(ready, |d| {
                let v = self.state.pointer("/update/version").and_then(Value::as_str).unwrap_or_default().to_string();
                // (On a narrow window only "Restart to update"; its version is in Settings › Updates.)
                let label = if compact || v.is_empty() { self.tr("upd.restart") } else { format!("{} · {}", self.tr("upd.restart"), self.i18n.tv("upd.versionN", &[("v", &v)])) };
                d.child(
                    Button::new("apply-update", label)
                        .icon("refresh")
                        .primary()
                        .small()
                        .on_click(cx.listener(|r, _, _, _| r.send("applyUpdate", json!({})))),
                )
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    // Its light (off, working, or on but blocked) and its name open its box (MultiHelp).
                    .child({
                        let armed = self.state.pointer("/multi/armed").and_then(Value::as_bool).unwrap_or(false);
                        focusable(div().id("multi-help"))
                            .flex()
                            .items_center()
                            .gap(px(7.))
                            .cursor_pointer()
                            .hover(|s| s.text_color(c(TEXT)))
                            .child(div().size(px(7.)).rounded_full().bg(c(if !multi { DIM } else if armed { OK } else { WARN })))
                            .child(self.tr("multi.title"))
                            .on_click(cx.listener(|r, _, window, cx| r.multi_help(window, cx)))
                    })
                    .child(Switch::new("multi", multi).on_toggle(
                    cx.listener(move |r, on: &bool, window, cx| {
                        // Like app.js's setMulti: turning it on asks first (Roblox's rules; "Don't ask again" is the
                        // app's to keep); the state that comes back moves the switch.
                        if *on {
                            r.confirm(r.tr("set.multiConfirmTitle"), r.tr("set.multiConfirm"), r.tr("set.multiOk"), false, Some("multi"), window, cx, |r, _| {
                                r.send("setSettings", json!({ "multiInstance": true }))
                            });
                        } else {
                            r.send("setSettings", json!({ "multiInstance": false }));
                        }
                        cx.notify();
                    }),
                )),
            )
            .when(!version.is_empty() && !ready && !tight, |d| d.child(div().flex_none().pl(px(4.)).text_color(c(DIM)).child(format!("v{version}"))))
    }

    /// "● Roblox 0.742 · up to date" in the status bar (Sidebar.tsx's sys-ver): green when it is the newest, orange
    /// with what is newer (a click opens Settings › Roblox version) when not; nothing while it isn't installed or known.
    fn roblox_version(&self, tight: bool, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.state.get("robloxInstalled").and_then(Value::as_bool) == Some(false) {
            return None;
        }
        let rv = self.state.get("robloxVersion")?;
        let installed = rv.get("installed").and_then(Value::as_str).filter(|v| !v.is_empty())?.to_string();
        let latest = rv.get("latest").and_then(Value::as_str).unwrap_or_default();
        let up = rv.get("upToDate").and_then(Value::as_bool);
        let short = |v: &str| v.split('.').take(2).collect::<Vec<_>>().join(".");
        let (dot, words) = match up {
            Some(false) => {
                let more = if short(latest) == short(&installed) { self.tr("rbx.newBuild") } else { self.i18n.tv("rbx.available", &[("v", &short(latest))]) };
                (c(WARN), format!("Roblox {} · {more}", short(&installed)))
            }
            Some(true) => (c(OK), format!("Roblox {} · {}", short(&installed), self.tr("rbx.latestShort"))),
            None => (c(DIM), format!("Roblox {}", short(&installed))),
        };
        let words = if tight { format!("Roblox {}", short(&installed)) } else { words };
        let tip: SharedString = format!("Roblox {installed}").into();
        Some(
            tool("sys-rbx-ver", None, false)
                .h(px(28.))
                .px(px(8.))
                .text_size(px(T_S + 0.5))
                .font_features(tnum())
                .when(up == Some(false), |d| d.text_color(c(WARN)))
                .child(div().size(px(7.)).flex_none().rounded_full().bg(dot))
                .child(words)
                .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
                .on_click(cx.listener(|r, _, _, cx| {
                    r.settings.part = pages::settings::Part::Roblox;
                    r.go(Page::Settings);
                    cx.notify();
                }))
                .into_any_element(),
        )
    }

    fn toast_layer(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
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
                    .pl(px(8.))
                    .pr(px(16.))
                    .py(px(8.))
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
                    .child(div().size(px(26.)).flex_none().rounded(px(R_S)).bg(ca(dot, 0.16)).flex().items_center().justify_center().child(icon(mark, 14., c(dot))))
                    .child(div().min_w_0().line_height(px(19.)).child(text));
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

/// A figure in the status bar: its icon, name, percent and a short bar that turns orange, then red, as it fills.
fn meter(icon_name: &'static str, name: String, pct: i64, with_bar: bool) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .child(icon(icon_name, 15., c(MUTED)))
        .child(div().text_color(c(MUTED)).child(name))
        .child(div().w(px(32.)).text_color(c(TEXT)).font_weight(FontWeight::MEDIUM).font_features(tnum()).child(format!("{pct}%")))
        .when(with_bar, |d| d.child(bar(SharedString::from(format!("meter-{icon_name}")), pct as f32 / 100., Some(72.))))
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
        Some(Button::new(SharedString::from(format!("rejoin-{id}")), root.tr("card.rejoinNow")).small().tint().on_click(cx.listener(move |r: &mut Root, _, _, cx| r.rejoin(&me, cx))).into_any_element())
    } else if trouble == Some("chip.invalid") {
        let vng = s("realm") == "vng";
        Some(Button::new(SharedString::from(format!("relogin-{id}")), root.tr("card.relogin")).small().tint().on_click(cx.listener(move |r: &mut Root, _, window, cx| crate::pages::quick::open(r, vng, window, cx))).into_any_element())
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
        .children(crate::pages::accounts::number(root, &id).map(|n| crate::pages::accounts::key_tag(n, a, 20.)))
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
    LaunchAsk { cmd: &'static str, args: Value, answer: Value },
    LaunchErrors(Value),
    Remote(Value),
    /// Add account: which way.
    Add,
    WhatsNew(Vec<Value>),
}

impl Render for Root {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.live.start();
        let toast = self.toast_layer(window, cx);
        pages::lock::settle(self, cx);
        pages::games::settle_words(self, window, cx);
        pages::rsettings::settle(self, window, cx);
        pages::wiki::settle(self, window, cx);
        // The search reads every part's rows, Discord's too: its boxes are made here the first time something is
        // searched (they were only made when Discord itself was opened, so "discord" found two unrelated rows).
        if self.page == Page::Settings && self.settings.dc.is_none() && !self.set_q.read(cx).value().trim().is_empty() {
            pages::settings::ensure_discord(self, window, cx);
        }
        if self.page == Page::Settings {
            pages::settings::appearance::settle(self, window, cx);
        }
        self.community_tick();
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
        let body = if locked {
            div().size_full().child(pages::lock::lock(self, window, cx))
        } else if self.lost {
            panel().size_full().flex().flex_col().items_center().justify_center().gap(px(10.)).child(
                div().size(px(56.)).flex_none().rounded(px(R_L)).bg(ca(WARN, 0.14)).flex().items_center().justify_center().child(icon("power", 24., c(WARN))).mb(px(6.)),
            )
            .child(div().text_size(px(T_GROUP)).font_weight(FontWeight::SEMIBOLD).child(self.tr("ui.lost")))
            .child(div().text_size(px(T_M + 0.5)).text_color(c(MUTED)).child(self.tr("ui.lostSub")))
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
        let frost_main = if locked { None } else { crate::scene::frost(R_PANEL, window, cx) };
        let frost_side = if with_side { crate::scene::frost(R_PANEL, window, cx) } else { None };
        let frost_rail = if locked || lost { None } else { crate::scene::frost(0., window, cx) };
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
                    Page::Auto => r.preset_search.clone(),
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
            // Ctrl+L, as a browser's address bar: the game box under the picked accounts.
            .on_action(cx.listener(|r, _: &FocusPlace, window, cx| {
                if r.locked() || window.has_active_dialog(cx) || r.selected.is_empty() {
                    return cx.propagate();
                }
                r.go(Page::Accounts);
                cx.notify();
                r.place.update(cx, |s, cx| {
                    s.focus(window, cx);
                    s.select_all(window, cx);
                });
            }))
            .on_action(cx.listener(|r, _: &PickAll, window, cx| Self::acc_key(r, pages::accounts::Key::All, window, cx)))
            .on_action(cx.listener(|r, _: &DeletePicked, window, cx| Self::acc_key(r, pages::accounts::Key::Delete, window, cx)))
            .on_action(cx.listener(|r, _: &EditPicked, window, cx| Self::acc_key(r, pages::accounts::Key::Edit, window, cx)))
            .on_action(cx.listener(|r, _: &PlayPicked, window, cx| Self::acc_key(r, pages::accounts::Key::Play, window, cx)))
            .on_action(cx.listener(|r, _: &ClearPicked, window, cx| Self::acc_key(r, pages::accounts::Key::Clear, window, cx)))
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
                    .child(self.top(with_side, locked || lost, cx))
                    .children(strip)
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .flex()
                            .gap(px(GAP))
                            .child(div().flex_1().min_w_0().h_full().relative().children(frost_main).child(step_in(self.shown_key(), self.step_from, body)))
                            .when(with_side, |d| d.child(div().relative().h_full().flex_none().children(frost_side).child(self.side(cx)))),
                    )
                    .when(!locked && !lost, |d| d.child(self.status(f32::from(window.viewport_size().width), cx))),
            )
            .children(toast)
            .child(self.live.reporter(self.link.clone()))
    }
}

/// Where the live pictures of the Roblox windows go, for the app to lay them there (MonitorTab: Windows draws them over
/// this window, so the page says the boxes, CSS-like pixels from its top-left, and the scale). Gathered anew each frame:
/// the area the pictures may show in (the accounts' scrolling part), each picture's box, and what must not be covered
/// (the toast); a box under a cover is left out. Sent only when it changed, and { hidden } once there is no area.
#[derive(Default, Clone)]
pub struct Live {
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

    /// Fills its parent: the pictures may show there.
    pub fn area(&self) -> impl IntoElement {
        let area = self.area.clone();
        canvas(move |b, _, _| *area.borrow_mut() = Some(b), |_, _, _, _| {}).absolute().inset_0()
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
            move |_, _, window, _| {
                let n = |p: Pixels| (f64::from(f32::from(p)) * 100.).round() / 100.;
                let msg = match *me.area.borrow() {
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
