//! "Join a player…": the selected accounts open into a Roblox user's server. A username is looked up publicly; where they
//! are comes from presence asked with each selected account's own sign-in until one is shown the server (Roblox shows it
//! only to people the player's privacy lets join). Bloxkeep never works around that (no follow, no friend request).
//! (AppController.JoinPlayer.cs)
use std::{
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
    sync::Arc,
    time::Duration,
};

use serde_json::{Value, json};

use super::{
    App,
    controller::{JsonKind, RejoinRequest, bool_of, has, ids_of, long_of, parse_guid, str_of},
    launching::{launch_fail_kind, require_vault, same_vault, secs, short},
};
use crate::core::{
    base::{Cancel, Fail, Guid, Res, Stamp},
    game_target::{FollowedPlayer, GameTarget},
    l, log,
    models::Account,
    roblox_api::{self, Presence, RobloxUser, realm},
    rt,
    vault::Vault,
};

pub const MAX_RECENT_PLAYERS: usize = 8;
/// A rejoin waiting for its followed player (offline, a hidden or full server) looks for them again after this.
pub const FOLLOW_RECHECK: Duration = Duration::from_secs(60);
/// How many of your signed-in accounts a following rejoin asks where the player is (its own first).
pub const FOLLOW_VIEWERS: usize = 3;
/// A followed player last seen outside a server you can join is taken as still there this long, for every account
/// following them.
pub const FOLLOW_SEEN_FOR: Duration = Duration::from_secs(30);

#[derive(Default)]
pub struct JoinPlayerState {
    /// Who each name typed this session is (keys lower-cased: OrdinalIgnoreCase), so looking the same one up again (a
    /// retry) needs no request.
    pub players: RefCell<HashMap<String, RobloxUser>>,
    /// Why each account waits for its followed player, as last said: said again only when the reason changes.
    pub follow_wait_told: RefCell<HashMap<Guid, String>>,
    /// Weak: what was seen must not keep a locked vault's cookies in memory.
    pub follow_seen: RefCell<HashMap<i64, (std::sync::Weak<Vault>, Stamp, Option<Presence>)>>,
}

/// ^[A-Za-z0-9_]{3,20}$
fn username_text(input: &str) -> bool {
    (3..=20).contains(&input.len()) && input.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// roblox\.com/users/(\d{1,19}), case-insensitive: the digits of the first match.
fn profile_link(input: &str) -> Option<&str> {
    let lower = input.to_ascii_lowercase();
    let mut from = 0;
    while let Some(at) = lower[from..].find("roblox.com/users/") {
        let start = from + at + "roblox.com/users/".len();
        let digits = input[start..].bytes().take_while(u8::is_ascii_digit).take(19).count();
        if digits > 0 {
            return Some(&input[start..start + digits]);
        }
        from = start;
    }
    None
}

impl App {
    /// {input, ids}: where the player is, as the selected accounts may see it: "found" (with placeId, jobId, game name),
    /// "hidden", "studio", "website", "offline" or "notFound".
    pub async fn find_player(self: &Rc<Self>, a: &Value) -> Res<Value> {
        let vault = require_vault(self)?;
        let accounts = self.in_launch_order(&ids_of(a));
        if accounts.is_empty() {
            return Err(Fail::user(l::t("Select at least one account.")));
        }
        let input = str_of(a, "input").trim().trim_start_matches('@').trim().to_string();
        let user = self.look_up_player(&input).await?;
        if !same_vault(self, &vault) {
            return Err(Fail::user(l::t("Bloxkeep was locked.")));
        }
        let Some(user) = user else {
            return Ok(json!({ "status": "notFound", "name": input }));
        };
        self.remember_player(&user.name);

        let viewers: Vec<Account> = accounts.into_iter().filter(App::can_ask_presence).collect();
        if viewers.is_empty() {
            return Err(Fail::user(l::t(
                "None of the selected accounts is signed in, so Bloxkeep can't ask Roblox where this player is. Sign them in again first.",
            )));
        }
        let count = viewers.len();
        let (seen, failed) = self.where_player_is(user.id, viewers, vault, &Cancel::new()).await;
        log::note(&format!(
            "Join a player: {}, asked with {count} account(s){}.",
            seen.as_ref().map_or("no answer", App::player_status),
            failed.as_ref().map(|f| format!(", last failure {}", launch_fail_kind(f))).unwrap_or_default()
        ));
        let Some(seen) = seen else {
            return Err(Fail::user(match &failed {
                Some(f) if f.cookie_invalid() => l::t("Roblox didn't accept the selected accounts' sign-in. Check their sign-in, then try again."),
                Some(f) => App::error_text(f),
                None => l::t("Roblox didn't say where this player is. Try again in a moment."),
            }));
        };
        let status = App::player_status(&seen);
        let mut game: Option<String> = None;
        if status == "found" {
            match self.get_game_info(seen.place_id.unwrap_or(0), false).await {
                Ok(info) => game = info.map(|g| g.name),
                // The name is an extra: the server can be joined without it.
                Err(ex) if roblox_api::is_expected_error(&ex) => {}
                Err(ex) => return Err(ex),
            }
        }
        let found = status == "found";
        Ok(json!({
            "status": status,
            "userId": user.id,
            "name": user.name,
            "displayName": user.display_name,
            "placeId": if found { seen.place_id } else { None },
            "jobId": if found { seen.job_id.clone() } else { None },
            "game": game,
        }))
    }

    /// An account that can ask Roblox where someone is: signed in to Roblox (not Roblox VNG, whose ids are another count).
    pub fn can_ask_presence(account: &Account) -> bool {
        !account.cookie.is_empty() && account.cookie_valid != Some(false) && account.realm == realm::GLOBAL
    }

    /// How much Roblox told about where a player is: a server beats "in a game", which beats Studio, the website, offline.
    pub fn presence_rank(p: &Presence) -> i32 {
        match p.r#type {
            2 if p.place_id.is_some_and(|id| id > 0) && p.job_id.as_deref().is_some_and(|j| !j.is_empty()) => 5,
            2 => 4,
            3 => 3,
            1 => 2,
            _ => 1,
        }
    }

    pub fn player_status(p: &Presence) -> &'static str {
        match App::presence_rank(p) {
            5 => "found",
            4 => "hidden",
            3 => "studio",
            2 => "website",
            _ => "offline",
        }
    }

    /// Where a player is, asked with each account's own sign-in in turn until one is shown a server. The most any was told,
    /// or None with the last failure when none could ask.
    // PORT: the C# threw (a UserError "Bloxkeep was locked." when locked meanwhile, the call-off when `ct` was called off,
    // anything unexpected as it came); here those come back as the failure with nothing seen, which every caller then
    // says or throws as the C# did.
    pub async fn where_player_is(&self, user_id: i64, viewers: Vec<Account>, vault: Arc<Vault>, ct: &Cancel) -> (Option<Presence>, Option<Fail>) {
        let mut seen: Option<Presence> = None;
        let mut failed: Option<Fail> = None;
        for viewer in viewers {
            if let Err(cancelled) = ct.check() {
                return (None, Some(cancelled));
            }
            let (cookie, c) = (viewer.cookie.clone(), ct.clone());
            // GetPresenceAsync(cookie, ids, ct: ct): the realm left at its default, Roblox's.
            match rt::blocking(move || roblox_api::get_presence(Some(&cookie), &[user_id], realm::GLOBAL, &c)).await {
                Ok(found) => {
                    if let Some(p) = found.get(&user_id) {
                        if seen.as_ref().is_none_or(|s| App::presence_rank(p) > App::presence_rank(s)) {
                            seen = Some(p.clone());
                        }
                        if App::presence_rank(p) == 5 {
                            break;
                        }
                    }
                }
                Err(ex) if !ct.is_cancelled() && roblox_api::is_expected_error(&ex) => {
                    let rate_limited = ex.rate_limited();
                    failed = Some(ex); // this account couldn't ask; the next one may
                    if rate_limited {
                        break; // the others would only run into the same limit
                    }
                }
                Err(ex) => return (None, Some(ex)),
            }
            if !same_vault(self, &vault) {
                return (None, Some(Fail::user(l::t("Bloxkeep was locked."))));
            }
        }
        (seen, failed)
    }

    /// The user a username, user ID or profile link stands for, or None. Digits alone: tried as an ID, then a username.
    pub async fn look_up_player(&self, input: &str) -> Res<Option<RobloxUser>> {
        if input.is_empty() {
            return Err(Fail::user(l::t("Type the player's Roblox username or user ID.")));
        }
        let key = input.to_lowercase();
        if let Some(known) = self.join_player.players.borrow().get(&key) {
            return Ok(Some(known.clone()));
        }
        let mut id: i64 = 0;
        if let Some(digits) = profile_link(input) {
            id = digits.parse().unwrap_or(0);
        } else if input.len() <= 19 && input.bytes().all(|b| b.is_ascii_digit()) {
            id = input.parse().unwrap_or(0);
        } else if !username_text(input) {
            return Err(Fail::user(l::t(
                "A Roblox username is 3 to 20 letters, digits or _ (the username, not the display name). A user ID or a profile link works too.",
            )));
        }
        let mut user: Option<RobloxUser> = None;
        // A refused or rate-limited lookup says so, instead of reading as "no such user".
        if id > 0 {
            user = rt::blocking(move || roblox_api::get_users([id], &Cancel::new(), true)).await?.remove(&id);
        }
        if user.is_none() && username_text(input) {
            let name = input.to_string();
            user = rt::blocking(move || roblox_api::get_user_by_name(&name, &Cancel::new())).await?;
        }
        if let Some(u) = &user {
            let mut players = self.join_player.players.borrow_mut();
            if players.len() >= 50 {
                players.clear();
            }
            players.insert(key, u.clone());
        }
        Ok(user)
    }

    /// The last few names looked up, newest first; kept in the vault, since they are other people's names.
    fn remember_player(&self, name: &str) {
        let changed = self
            .vault_mut(|d| {
                let list = &mut d.settings.recent_players;
                if list.first().is_some_and(|n| n == name) {
                    return false;
                }
                list.retain(|n| n.to_lowercase() != name.to_lowercase()); // OrdinalIgnoreCase
                list.insert(0, name.to_string());
                list.truncate(MAX_RECENT_PLAYERS);
                true
            })
            .unwrap_or(false);
        if changed {
            self.persist_soon();
            self.push_state();
        }
    }

    pub fn forget_player(&self, name: &str) -> Res<Value> {
        let removed = self.vault_mut(|d| {
            let list = &mut d.settings.recent_players;
            let before = list.len();
            list.retain(|n| n.to_lowercase() != name.to_lowercase());
            before - list.len()
        })?;
        if removed > 0 {
            self.persist_soon();
        }
        self.push_state();
        Ok(Value::Null)
    }

    /// {ids, placeId, jobId, force, follow, followUserId, followName}: the selected accounts open into the server
    /// find_player found, through the usual launch. With follow, their rejoins follow that player (see follow_target);
    /// the box's choice is remembered.
    pub async fn join_server(self: &Rc<Self>, a: &Value) -> Res<Value> {
        let vault = require_vault(self)?;
        let accounts = self.in_launch_order(&ids_of(a));
        self.check_can_launch(&accounts)?;
        let target = App::join_target(a)?;
        if has(a, "follow", JsonKind::True) || has(a, "follow", JsonKind::False) {
            let follow = bool_of(a, "follow");
            self.vault_mut(|d| d.settings.follow_joined_player = follow)?;
            self.persist_soon();
        }
        let delay = self.vault(|d| d.settings.launch_delay_seconds)?;
        let result = self.launch_accounts(vault, accounts, Some(target), bool_of(a, "force"), delay, None, None, None, None).await?;
        Ok(serde_json::to_value(&result)?)
    }

    /// The server join_server opens, following the player it found when its box was ticked.
    pub(crate) fn join_target(a: &Value) -> Res<GameTarget> {
        let place_id = long_of(a, "placeId");
        let job_id = str_of(a, "jobId").trim().to_string();
        if place_id <= 0 || parse_guid(&job_id).is_none() {
            return Err(Fail::user(l::t("That server can't be joined. Look the player up again.")));
        }
        let follow_id = long_of(a, "followUserId");
        let follow_name = str_of(a, "followName").trim().to_string();
        let mut target = GameTarget::new(place_id, Some(job_id), None, None);
        target.follow = if bool_of(a, "follow") && follow_id > 0 {
            Some(FollowedPlayer { user_id: follow_id, name: if follow_name.is_empty() { follow_id.to_string() } else { follow_name } })
        } else {
            None
        };
        Ok(target)
    }

    // ───────── following a player on rejoin ─────────

    /// Where a following rejoin goes: the player's server now, as Roblox shows it to this account (first) or another
    /// signed-in one, checked with Roblox's join answer. When they can't be followed yet (offline, website, Studio, hidden,
    /// full or ended server, a game that bans or refuses this account), the rejoin waits FOLLOW_RECHECK, said once per
    /// reason, and None comes back. No answer at all (no connection, rate limit) is an Err, so the rejoin's usual retry
    /// takes it.
    pub async fn follow_target(self: &Rc<Self>, account: &Account, request: &RejoinRequest, ct: &Cancel) -> Res<Option<GameTarget>> {
        let vault = require_vault(self)?;
        // PORT: Target.Follow! threw a NullReferenceException for a request without a followed player; an Err here.
        let follow = request.target.follow.clone().ok_or_else(|| Fail::other("NullReferenceException: the rejoin follows no player"))?;
        let player = follow.name.clone();
        let n = self.name(account);
        // A few accounts at most, and a player not in a server they can see is remembered a little while: ten accounts
        // following someone offline each asked with every account every minute, about 100 presence requests a minute
        // (review, 2026-10-30).
        let others: Vec<Account> = self.vault(|d| d.accounts.iter().filter(|o| o.id != account.id).cloned().collect()).unwrap_or_default();
        let viewers: Vec<Account> =
            std::iter::once(account.clone()).chain(others).filter(App::can_ask_presence).take(FOLLOW_VIEWERS).collect();

        // Called off meanwhile (Close, a scheduled stop): it doesn't wait to look again.
        let wait = |app: &App, reason: &str, text: String| -> Option<GameTarget> {
            if ct.is_cancelled() {
                return None;
            }
            {
                let mut queue = app.controller.rejoin_queue.borrow_mut();
                queue.retain(|r| r.account_id != account.id);
                queue.push(RejoinRequest { due: Stamp::utc_now().plus_secs(secs(FOLLOW_RECHECK)), ..request.clone() });
            }
            let told = app.join_player.follow_wait_told.borrow().get(&account.id).cloned();
            if told.as_deref() != Some(reason) {
                app.join_player.follow_wait_told.borrow_mut().insert(account.id, reason.to_string());
                log::note(&format!(
                    "Rejoin: account {} waits for its followed player ({reason}); looks again in {} s.",
                    short(account.id),
                    FOLLOW_RECHECK.as_secs()
                ));
                app.notify(&text, "warn", "rejoinFail", account.id, true, None);
            }
            app.push_state();
            None
        };

        if viewers.is_empty() {
            return Ok(wait(
                self,
                "none",
                l::f(
                    "{0} waits to join {1}: none of your accounts is signed in, so Bloxkeep can't ask Roblox where {1} is. Bloxkeep looks again every minute.",
                    &[&n, &player],
                ),
            ));
        }
        let known = self.join_player.follow_seen.borrow().get(&follow.user_id).cloned();
        let seen: Option<Presence> = match known {
            Some((v, at, seen)) if v.upgrade().is_some_and(|v| Arc::ptr_eq(&v, &vault)) && Stamp::utc_now().secs_since(at) < secs(FOLLOW_SEEN_FOR) => seen,
            _ => {
                let (seen, failed) = self.where_player_is(follow.user_id, viewers, vault.clone(), ct).await;
                if seen.is_none()
                    && let Some(failed) = failed
                {
                    return Err(failed);
                }
                // Only "not in a server yet": a server found is asked afresh each time, since the player may have moved on.
                // Nor a part answer (one account saw nothing, a later one was refused or rate-limited): the accounts that
                // couldn't ask may see the player, so every follower isn't kept waiting on it.
                if failed.is_none() && seen.as_ref().is_none_or(|s| App::presence_rank(s) < 5) {
                    self.join_player.follow_seen.borrow_mut().insert(follow.user_id, (Arc::downgrade(&vault), Stamp::utc_now(), seen.clone()));
                } else {
                    self.join_player.follow_seen.borrow_mut().remove(&follow.user_id);
                }
                seen
            }
        };
        let status = seen.as_ref().map_or("offline", App::player_status);
        if status != "found" {
            let text = match status {
                "hidden" => l::f(
                    "{0} waits to join {1}: {1} is in a game, but Roblox doesn't show which server to your accounts (their privacy settings). Bloxkeep looks again every minute.",
                    &[&n, &player],
                ),
                "studio" => l::f("{0} waits to join {1}: {1} is in Roblox Studio, not in a game. Bloxkeep looks again every minute.", &[&n, &player]),
                "website" => l::f("{0} waits to join {1}: {1} is on the Roblox website, not in a game. Bloxkeep looks again every minute.", &[&n, &player]),
                _ => l::f("{0} waits to join {1}: {1} is offline, or hides being online. Bloxkeep looks again every minute.", &[&n, &player]),
            };
            return Ok(wait(self, status, text));
        }
        let seen = seen.expect("found");
        let mut target = GameTarget::new(seen.place_id.unwrap_or(0), seen.job_id.clone(), None, None);
        target.follow = Some(follow.clone());
        // A game that has banned this account: its server would only show the ban (Join Error 600).
        if self.game_ban_of(account, Some(target.place_id)).is_some() {
            return Ok(wait(
                self,
                "banned",
                l::f("{0} waits to join {1}: the game {1} is in has banned this account. Bloxkeep looks again every minute.", &[&n, &player]),
            ));
        }
        // Roblox's own join answer: a full or ended server or a game refusing this account would only open a failing window.
        // No answer (refused request, no network): the window opens and its join tells as usual.
        let answer = self.check_join(account, &target, "Following a player for a rejoin", ct).await;
        ct.check()?;
        if answer.as_ref().is_some_and(|a| a.full()) {
            return Ok(wait(self, "full", l::f("{0} waits to join {1}: {1}'s server is full. Bloxkeep looks again every minute.", &[&n, &player])));
        }
        if answer.as_ref().is_some_and(|a| a.ended()) {
            return Ok(wait(self, "ended", l::f("{0} waits to join {1}: {1}'s server has just ended. Bloxkeep looks again every minute.", &[&n, &player])));
        }
        if answer.as_ref().is_some_and(|a| a.refused()) {
            return Ok(wait(
                self,
                "refused",
                l::f("{0} waits to join {1}: Roblox won't let this account into the game {1} is in. Bloxkeep looks again every minute.", &[&n, &player]),
            ));
        }
        log::note(&format!("Rejoin: account {} follows its player to place {}.", short(account.id), target.place_id));
        if self.join_player.follow_wait_told.borrow_mut().remove(&account.id).is_some() {
            self.toast(&l::f("{0} found {1} again — rejoining their server…", &[&n, &player]), "info");
        }
        Ok(Some(target))
    }
}
