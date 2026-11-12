// AppController.Installs.cs: what is installed: the Roblox launchers and clients found, which one a launch starts, and the
// newest Roblox version.
use std::cell::RefCell;
use std::rc::Rc;

use serde::Serialize;
use serde_json::{Value, json};

use super::App;
use super::controller::{INSTALL_CACHE_LIFETIME, REGISTRY_CACHE_LIFETIME};
use crate::core::base::{Cancel, Guid, Res, Stamp};
use crate::core::roblox_installs::{self, ClientVersion};
use crate::core::{l, log, multi_instance, roblox_api, roblox_launcher, rt, win_kit};

/// The instance fields AppController.Installs.cs declares.
#[derive(Default)]
pub struct InstallsState {
    pub installs: RefCell<Option<InstallsInfo>>,
    /// The launcher that can update the client the launch option starts (RobloxInstalls.UpdaterFor).
    pub updater_exe: RefCell<Option<String>>,
}

/// Which Roblox clients and launchers exist, and the version of the client the chosen option starts.
/// `others`: other Bloxstrap-style launchers found here (their option is "other:<name>").
#[derive(Clone, Debug)]
pub struct InstallsInfo {
    pub handler: Option<String>,
    pub bloxstrap: bool,
    pub fishstrap: bool,
    pub others: Vec<String>,
    pub roblox: bool,
    pub direct: bool,
    pub store: bool,
    pub client: Option<ClientVersion>,
    pub vng: bool,
    pub voidstrap: bool,
}

impl InstallsInfo {
    /// As the page gets it (camelCase; the client's Source and Exe are never sent).
    pub fn to_json(&self) -> Value {
        json!({
            "handler": self.handler,
            "bloxstrap": self.bloxstrap,
            "fishstrap": self.fishstrap,
            "others": self.others,
            "roblox": self.roblox,
            "direct": self.direct,
            "store": self.store,
            "client": self.client.as_ref().map(|c| json!({ "version": c.version, "folder": c.folder })),
            "vng": self.vng,
            "voidstrap": self.voidstrap,
        })
    }
}

/// `up_to_date`: None until both versions are known. `store`: Roblox from the Microsoft Store is chosen: the Store
/// installs its updates, not Roblox itself. `updater`: the launcher that can update this copy of Roblox now (its name),
/// when there is one. `old_windows`: windows still on a build older than Roblox's newest. `reopens`: of all the windows
/// open, those "Close, update, reopen" would open again.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RobloxVersionInfo {
    pub installed: Option<String>,
    pub latest: Option<String>,
    pub up_to_date: Option<bool>,
    pub direct: bool,
    pub store: bool,
    pub updater: Option<String>,
    pub updating: bool,
    pub old_windows: i32,
    pub reopens: i32,
}

/// .NET's Version.TryParse: two to four whole numbers; a part not given counts below 0 (1.2 < 1.2.0).
pub fn net_version(text: Option<&str>) -> Option<[i64; 4]> {
    let text = text?.trim();
    let parts: Vec<&str> = text.split('.').collect();
    if !(2..=4).contains(&parts.len()) {
        return None;
    }
    let mut out = [-1i64; 4];
    for (i, p) in parts.iter().enumerate() {
        let p = p.trim();
        if p.is_empty() || !p.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        let n: i64 = p.parse().ok()?;
        if n > i32::MAX as i64 {
            return None;
        }
        out[i] = n;
    }
    Some(out)
}

/// What is installed, from disk and registry: up to a second on a busy PC (2026-10-29, the owner's: 0.9 s in a
/// process check), so the process tick reads it on a worker (`tick_roblox_updates`).
pub fn read_installs(via: &str, custom: &str) -> (InstallsInfo, Option<String>) {
    // What Default opens Roblox with: the links' program, or what stands in for it when they are gone or Roblox
    // VNG's (RobloxLauncher.FallbackLauncher), so Settings names the program really used.
    let used = roblox_installs::usable_handler_path().or_else(|| roblox_launcher::fallback_launcher().map(|f| f.0));
    // PORT: the C# let ClientFor's exception go up from here; roblox_installs::client_for answers None instead.
    let client = roblox_installs::client_for(via, custom);
    let installs = InstallsInfo {
        handler: used.map(|u| win_kit::file_stem(&u)),
        bloxstrap: roblox_installs::find_bootstrapper("Bloxstrap").is_some(),
        fishstrap: roblox_installs::find_bootstrapper("Fishstrap").is_some(),
        others: roblox_installs::find_other_bootstrappers(),
        roblox: roblox_installs::find_official_client().is_some(),
        direct: roblox_installs::find_any_client().or_else(roblox_installs::find_vng_client).is_some(),
        store: roblox_installs::store_edition_installed(),
        client: roblox_installs::version_of(client.as_deref()),
        vng: roblox_installs::find_vng_client().is_some(),
        voidstrap: roblox_installs::find_bootstrapper("Voidstrap").is_some(),
    };
    (installs, roblox_installs::updater_for(via, custom))
}

impl App {
    /// Registry facts shown on the page; BuildState runs on every push, so they're re-read at most every 30 s.
    pub fn registry_info(&self) -> (bool, bool) {
        if let Some(cached) = self.controller.registry.get()
            && Stamp::utc_now().secs_since(self.controller.registry_read_at.get()) < REGISTRY_CACHE_LIFETIME.as_secs_f64()
        {
            return cached;
        }
        self.controller.registry_read_at.set(Stamp::utc_now());
        let tray = multi_instance::find_tray_autostart().ok().flatten();
        let info = (roblox_launcher::is_protocol_registered(), tray.is_some_and(|t| t.enabled));
        self.controller.registry.set(Some(info));
        info
    }

    pub fn installs(&self) -> InstallsInfo {
        if let Some(cached) = self.installs.installs.borrow().clone()
            && Stamp::utc_now().secs_since(self.controller.installs_read_at.get()) < INSTALL_CACHE_LIFETIME.as_secs_f64()
        {
            return cached;
        }
        let (via, custom) = self.launch_option();
        let (installs, updater) = read_installs(&via, &custom);
        self.take_installs(installs, updater, Stamp::utc_now())
    }

    /// The vault's launch option and custom launcher ("auto" and "" without a vault).
    pub fn launch_option(&self) -> (String, String) {
        match self.controller.vault.borrow().clone() {
            Some(v) => {
                let d = v.data();
                (d.settings.launch_via.clone(), d.settings.custom_launcher.clone())
            }
            None => ("auto".into(), String::new()),
        }
    }

    /// A read of what is installed kept, and a new Roblox build since the last one said.
    pub fn take_installs(&self, installs: InstallsInfo, updater: Option<String>, read_at: Stamp) -> InstallsInfo {
        self.controller.installs_read_at.set(read_at);
        let (before, before_version) = {
            let old = self.installs.installs.borrow();
            let client = old.as_ref().and_then(|i| i.client.as_ref());
            (client.map(|c| c.folder.clone()), client.map(|c| c.version.clone()))
        };
        *self.installs.installs.borrow_mut() = Some(installs.clone());
        *self.installs.updater_exe.borrow_mut() = updater;
        // A new build since the last look, or an older one (a launcher pinned to a version), which isn't an update. The Store
        // edition has no build folder, so switching to or from it says nothing. Going older while the newer build stays installed
        // is Roblox switching channel (RobloxInstalls.HasBuild): nothing to say.
        if let Some(before) = before.as_deref().filter(|b| !b.is_empty())
            && let Some(now) = installs.client.as_ref().filter(|c| !c.folder.is_empty())
            && now.folder != before
        {
            let older = match (net_version(Some(&now.version)), net_version(before_version.as_deref())) {
                (Some(have), Some(had)) => have < had,
                _ => false,
            };
            if !(older && roblox_installs::has_build(now.exe.as_deref(), Some(before))) {
                let updated = if older {
                    l::f("Roblox went back to version {0}.", &[&now.version])
                } else {
                    l::f("Roblox updated to {0}.", &[&now.version])
                };
                self.toast(&updated, "ok");
                self.note_activity("robloxUpdated", Guid::nil(), None, None, None, Some(&updated), Some("info"));
            }
        }
        App::note_roblox_build(installs.client.as_ref()); // Settings › Roblox updates (roblox_updates.rs)
        installs
    }

    /// The version the chosen launcher starts, next to the newest Roblox serves. An older client updates itself on
    /// its next start (that window takes longer); Direct only when nothing would update it (RobloxInstalls.UpdatesOnOpen).
    pub fn roblox_version_dto(&self) -> RobloxVersionInfo {
        let installed = self.installs().client;
        let latest = self.updates.latest_roblox.borrow().clone();
        // Roblox can ship a new build under the same version number: the build folder ("version-<hash>") tells them
        // apart. A newer number than Roblox serves (another channel) still counts as up to date.
        let same_build = matches!((&installed, &latest), (Some(i), Some(l)) if i.folder == l.folder);
        let newer = match (&installed, &latest) {
            (Some(i), Some(l)) => matches!((net_version(Some(&i.version)), net_version(Some(&l.version))), (Some(have), Some(latest)) if have > latest),
            _ => false,
        };
        let starts_version = |c: &Option<ClientVersion>| c.as_ref().is_some_and(|c| c.folder.to_lowercase().starts_with("version-"));
        let folders_known = starts_version(&installed) && starts_version(&latest);
        let mut up_to_date = match (&installed, &latest) {
            (Some(i), Some(l)) => Some(same_build || newer || (!folders_known && i.version == l.version)),
            _ => None,
        };
        // The links on another channel's build (Roblox puts some accounts on test builds), with the newest build
        // installed beside it: that one is what an account on the main channel starts.
        let mut shown = installed.as_ref().map(|i| i.version.clone());
        if up_to_date == Some(false)
            && let (Some(i), Some(l)) = (&installed, &latest)
            && roblox_installs::has_build(i.exe.as_deref(), Some(&l.folder))
        {
            shown = Some(l.version.clone());
            up_to_date = Some(true);
        }
        let launch_via = self.controller.vault.borrow().as_ref().map(|v| v.data().settings.launch_via.clone());
        let updater = self.installs.updater_exe.borrow().clone().map(|u| {
            if roblox_installs::is_own_installer(Some(&u)) { "Roblox".to_string() } else { win_kit::file_stem(&u) }
        });
        let running: Vec<(Guid, (i32, Rc<super::controller::PendingLaunch>))> =
            self.controller.running.borrow().iter().map(|(k, v)| (*k, v.clone())).collect();
        let old_windows = running.iter().filter(|(id, run)| self.find_account(*id).is_some_and(|a| self.runs_old_build(&a, run.0))).count() as i32;
        let reopens = running.iter().filter(|(id, run)| self.can_reopen(*id, run)).count() as i32;
        RobloxVersionInfo {
            installed: shown,
            latest: latest.as_ref().map(|l| l.version.clone()),
            up_to_date,
            direct: launch_via.as_deref() == Some("direct") && !roblox_installs::updates_on_open(installed.as_ref().and_then(|i| i.exe.as_deref())),
            store: launch_via.as_deref() == Some("store"),
            updater,
            updating: self.controller.roblox_updating.get() || self.roblox_updates.update_reopen_since.get().is_some(),
            old_windows,
            reopens,
        }
    }

    pub async fn refresh_latest_roblox(self: &Rc<Self>) -> Res<()> {
        match rt::blocking(|| roblox_api::get_latest_client_version(&Cancel::new())).await {
            Ok(Some(latest)) => {
                if self.updates.latest_roblox.borrow().as_ref() != Some(&latest) {
                    *self.updates.latest_roblox.borrow_mut() = Some(latest);
                    self.push_state();
                }
                Ok(())
            }
            Ok(None) => Ok(()),
            // Shown as "couldn't check"; tried again next time.
            Err(e) if roblox_api::is_expected_error(&e) => Ok(()),
            Err(e) => Err(e),
        }
    }
}
