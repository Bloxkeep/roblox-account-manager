// The App: what the C# app called AppController, one class written over some sixty-five files. Here each of those
// files is a module of this folder (`AppController.Foo.cs` is `foo.rs`, `AppController.cs` is `controller.rs`) that
// adds its methods to App and keeps the fields it declared in a state of its own (`FooState`, one cell a field), so
// a field is `self.foo.name.borrow()` from anywhere, as it was `_name` from anywhere in C#.
//
// The App lives on the core's thread alone (rt.rs): it is an `Rc`, never shared, never locked; its async methods are
// tasks on that thread and wait where the C# awaited. The interface reaches it through `handle` (controller.rs), the
// same commands the C# app answered; what it tells the interface goes out through `emit`.
use std::{cell::RefCell, rc::Rc};

use crate::core::{
    base::{Fail, Guid, Res},
    l,
    models::{Account, VaultData},
};

pub mod controller;
pub mod accounts;
pub mod activity;
pub mod anti_afk;
pub mod authenticator;
pub mod auto_lock;
pub mod backup;
pub mod bot_check;
pub mod claim_check;
pub mod cores;
pub mod custom_theme;
pub mod discord;
pub mod discord_alerts;
pub mod discord_presence;
pub mod discord_summary;
pub mod drag;
pub mod feedback;
pub mod feeds;
pub mod friends;
pub mod game_page;
pub mod game_picker;
pub mod games;
pub mod gpui;
pub mod health;
pub mod hide_windows;
pub mod hotkeys;
pub mod installs;
pub mod join_player;
pub mod latest;
pub mod launching;
pub mod macros;
pub mod memory;
pub mod mini;
pub mod monitor;
pub mod names;
pub mod notify;
pub mod online;
pub mod panels;
pub mod persist;
pub mod phone;
pub mod power;
pub mod presence;
pub mod presets;
pub mod processes;
pub mod rejoin;
pub mod rejoin_place;
pub mod relaunch;
pub mod remote;
pub mod roblox_updates;
pub mod roblox_words;
pub mod robux;
pub mod saved_servers;
pub mod schedule;
pub mod server_hop;
pub mod settings;
pub mod sign_ins;
pub mod state;
pub mod stats;
pub mod tray;
pub mod unlock;
pub mod updates;
pub mod vault_import;
pub mod vaults;
pub mod website;
pub mod wiki;
pub mod window_icons;

#[derive(Default)]
pub struct App {
    pub controller: controller::ControllerState,
    pub accounts: accounts::AccountsState,
    pub activity: activity::ActivityState,
    pub anti_afk: anti_afk::AntiAfkState,
    pub authenticator: authenticator::AuthenticatorState,
    pub auto_lock: auto_lock::AutoLockState,
    pub backup: backup::BackupState,
    pub bot_check: bot_check::BotCheckState,
    pub claim_check: claim_check::ClaimCheckState,
    pub cores: cores::CoresState,
    pub custom_theme: custom_theme::CustomThemeState,
    pub discord: discord::DiscordState,
    pub discord_alerts: discord_alerts::DiscordAlertsState,
    pub discord_presence: discord_presence::DiscordPresenceState,
    pub discord_summary: discord_summary::DiscordSummaryState,
    pub drag: drag::DragState,
    pub feedback: feedback::FeedbackState,
    pub feeds: feeds::FeedsState,
    pub friends: friends::FriendsState,
    pub game_page: game_page::GamePageState,
    pub game_picker: game_picker::GamePickerState,
    pub games: games::GamesState,
    pub gpui: gpui::GpuiState,
    pub health: health::HealthState,
    pub hide_windows: hide_windows::HideWindowsState,
    pub hotkeys: hotkeys::HotkeysState,
    pub installs: installs::InstallsState,
    pub join_player: join_player::JoinPlayerState,
    pub latest: latest::LatestState,
    pub launching: launching::LaunchingState,
    pub macros: macros::MacrosState,
    pub memory: memory::MemoryState,
    pub mini: mini::MiniState,
    pub monitor: monitor::MonitorState,
    pub names: names::NamesState,
    pub notify: notify::NotifyState,
    pub online: online::OnlineState,
    pub panels: panels::PanelsState,
    pub persist: persist::PersistState,
    pub phone: phone::PhoneState,
    pub power: power::PowerState,
    pub presence: presence::PresenceState,
    pub presets: presets::PresetsState,
    pub processes: processes::ProcessesState,
    pub rejoin: rejoin::RejoinState,
    pub rejoin_place: rejoin_place::RejoinPlaceState,
    pub relaunch: relaunch::RelaunchState,
    pub remote: remote::RemoteState,
    pub roblox_updates: roblox_updates::RobloxUpdatesState,
    pub robux: robux::RobuxState,
    pub saved_servers: saved_servers::SavedServersState,
    pub schedule: schedule::ScheduleState,
    pub server_hop: server_hop::ServerHopState,
    pub settings: settings::SettingsState,
    pub sign_ins: sign_ins::SignInsState,
    pub state: state::StateState,
    pub stats: stats::StatsState,
    pub tray: tray::TrayState,
    pub unlock: unlock::UnlockState,
    pub updates: updates::UpdatesState,
    pub vault_import: vault_import::VaultImportState,
    pub vaults: vaults::VaultsState,
    pub website: website::WebsiteState,
    pub wiki: wiki::WikiState,
    pub window_icons: window_icons::WindowIconsState,
}

thread_local! {
    /// The one App, on the core's thread.
    static APP: RefCell<Option<Rc<App>>> = const { RefCell::new(None) };
}

impl App {
    /// Makes the App; once, on the core's thread.
    pub fn start() -> Rc<App> {
        let app = Rc::new(App::default());
        APP.with(|a| *a.borrow_mut() = Some(app.clone()));
        app
    }

    /// The App, for work that came from another thread through `rt::post` (a watcher's event, a window's click).
    /// Nothing happens before the App is made, or on a thread that isn't the core's.
    pub fn with(f: impl FnOnce(&Rc<App>)) {
        let app = APP.with(|a| a.borrow().clone());
        if let Some(app) = app {
            f(&app);
        }
    }

    /// Reads the open vault's data (C#: `RequireVault().Data`): Err when Bloxkeep is locked.
    pub fn vault<R>(&self, f: impl FnOnce(&VaultData) -> R) -> Res<R> {
        let vault = self.controller.vault.borrow();
        let Some(vault) = vault.as_ref() else { return Err(Fail::user(l::t("Bloxkeep is locked."))) };
        let data = vault.data();
        Ok(f(&data))
    }

    /// Changes the open vault's data; `persist` afterwards where the C# saved.
    pub fn vault_mut<R>(&self, f: impl FnOnce(&mut VaultData) -> R) -> Res<R> {
        let vault = self.controller.vault.borrow();
        let Some(vault) = vault.as_ref() else { return Err(Fail::user(l::t("Bloxkeep is locked."))) };
        let mut data = vault.data();
        Ok(f(&mut data))
    }

    /// An account as it is now, a copy (C#: RequireAccount): Err when locked, or when there is no such account.
    pub fn account(&self, id: Guid) -> Res<Account> {
        self.vault(|d| d.accounts.iter().find(|a| a.id == id).cloned())?.ok_or_else(|| Fail::user(l::t("Account not found.")))
    }

    /// The same, None instead of an error (C#: FindAccount).
    pub fn find_account(&self, id: Guid) -> Option<Account> {
        self.vault(|d| d.accounts.iter().find(|a| a.id == id).cloned()).ok().flatten()
    }

    /// Changes one account in place; nothing happens when locked or when it is gone.
    pub fn update_account(&self, id: Guid, f: impl FnOnce(&mut Account)) {
        let _ = self.vault_mut(|d| {
            if let Some(a) = d.accounts.iter_mut().find(|a| a.id == id) {
                f(a);
            }
        });
    }
}
