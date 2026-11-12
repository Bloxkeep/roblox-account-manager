//! AppUpdater.cs: updates an installed or portable Bloxkeep from GitHub Releases: downloads in the background, offers
//! "Restart to update", and applies on close otherwise. Copies run from a build folder never update.
//!
//! Velopack's Rust crate stands in for its .NET package: UpdateManager, UpdateSource, VelopackAsset. The .NET package's
//! GithubSource (which the Rust crate lacks) is ported below, reading GitHub's releases as the .NET one does.
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};

use serde_json::{Value, json};
use velopack::sources::{AutoSource, UpdateSource};
use velopack::{UpdateCheck, UpdateManager, VelopackAsset};

use crate::core::base::Stamp;
use crate::core::l;
use crate::core::log;
use crate::core::prefs;
use crate::core::whats_new;

/// Where releases come from, set at build time: BLOXKEEP_UPDATE_REPO=https://github.com/owner/name.
// PORT: the C# reads its AssemblyMetadata "UpdateRepo" (-p:UpdateRepo=…); the Rust build takes it from the environment
// at compile time.
fn repo() -> Option<&'static str> {
    option_env!("BLOXKEEP_UPDATE_REPO").filter(|r| !r.is_empty())
}

/// Bloxkeep's installer or uninstaller asked this run to quit (Program's quit event): it runs Velopack's Update.exe
/// itself next, so `apply_on_exit` starts no other one to race it for the folder and its lock.
pub static HELD_FOR_INSTALLER: AtomicBool = AtomicBool::new(false);

pub(crate) fn guard<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

pub(crate) struct State {
    pub(crate) ready: Option<VelopackAsset>,
    /// A check is running (C#'s `_run` not completed).
    running: bool,
    reported_progress: i32,
    /// idle, checking, downloading, ready, upToDate or error.
    pub(crate) status: String,
    new_version: Option<String>,
    progress: i32,
    offline: Option<bool>,
    not_found_logged: bool,
    checked_at: Option<Stamp>,
}

pub struct AppUpdater {
    manager: Option<UpdateManager>,
    current_version: String,
    pub(crate) state: Mutex<State>,
    done: Condvar,
    /// Raised, possibly from a background thread, when the status or download progress changes.
    on_changed: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
}

impl Default for AppUpdater {
    fn default() -> AppUpdater {
        AppUpdater::new()
    }
}

impl AppUpdater {
    pub fn new() -> AppUpdater {
        // For trying an update end to end before publishing: a folder (or URL) holding vpk's release files.
        let source = std::env::var("BLOXKEEP_UPDATE_SOURCE").ok().filter(|s| !s.is_empty());
        // PORT: new UpdateManager(…) in .NET is made even when not installed (IsInstalled false); the Rust one fails
        // instead, which is taken as "not installed" (not logged: a build folder's copy is expected to fail).
        let manager = if let Some(source) = source {
            UpdateManager::new(AutoSource::new(&source), None, None).ok()
        } else if let Some(repo) = repo() {
            UpdateManager::new(ReleaseSource::new(repo), None, None).ok()
        } else {
            None
        };
        let current_version = manager.as_ref().map(|m| m.get_current_version_as_string()).unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());
        let updater = AppUpdater {
            manager,
            current_version,
            state: Mutex::new(State {
                ready: None,
                running: false,
                reported_progress: -1,
                status: "idle".into(),
                new_version: None,
                progress: 0,
                offline: None,
                not_found_logged: false,
                checked_at: None,
            }),
            done: Condvar::new(),
            on_changed: Mutex::new(None),
        };
        // Downloaded earlier but not applied yet (normally applied on exit already).
        if let Some(pending) = updater.manager.as_ref().and_then(|m| m.get_update_pending_restart()) {
            {
                let mut s = guard(&updater.state);
                s.new_version = Some(pending.Version.clone());
                s.ready = Some(pending);
                s.status = "ready".into();
            }
            if updater.drop_unwanted_beta() {
                guard(&updater.state).status = "idle".into();
            }
        }
        updater
    }

    /// C#'s `Changed` event.
    pub fn set_on_changed(&self, handler: Option<Arc<dyn Fn() + Send + Sync>>) {
        *guard(&self.on_changed) = handler;
    }

    fn raise_changed(&self) {
        let handler = guard(&self.on_changed).clone();
        if let Some(h) = handler {
            h();
        }
    }

    pub fn supported(&self) -> bool {
        self.manager.is_some()
    }
    pub fn current_version(&self) -> &str {
        &self.current_version
    }
    /// idle, checking, downloading, ready, upToDate or error.
    pub fn status(&self) -> String {
        guard(&self.state).status.clone()
    }
    pub fn new_version(&self) -> Option<String> {
        guard(&self.state).new_version.clone()
    }
    pub fn progress(&self) -> i32 {
        guard(&self.state).progress
    }
    /// When the last check got its answer (or failed); None before the first.
    pub fn checked_at(&self) -> Option<Stamp> {
        guard(&self.state).checked_at
    }

    /// Why the last check failed, in the app's language at the time it is asked for; the details are in the log.
    pub fn error(&self) -> Option<String> {
        let offline = guard(&self.state).offline;
        offline.map(|o| {
            if o { l::t("Couldn't reach the update server. Check your internet connection.") } else { l::t("The update couldn't be checked or downloaded. Bloxkeep tries again later.") }
        })
    }

    /// Checks for a newer version and downloads it. Calls made while one is running share it (wait for it).
    pub fn check(&self) {
        let Some(manager) = &self.manager else { return };
        {
            let mut s = guard(&self.state);
            if s.ready.is_some() {
                return;
            }
            if s.running {
                while s.running {
                    s = self.done.wait(s).unwrap_or_else(|e| e.into_inner());
                }
                return;
            }
            s.running = true;
        }
        self.run(manager);
        guard(&self.state).running = false;
        self.done.notify_all();
    }

    fn run(&self, manager: &UpdateManager) {
        let failed_before = guard(&self.state).status == "error";
        guard(&self.state).offline = None;
        self.set("checking");
        let result = (|| -> Result<(), velopack::Error> {
            let update = match manager.check_for_updates()? {
                UpdateCheck::UpdateAvailable(update) => update,
                _ => {
                    if failed_before {
                        log::note("Update: check recovered, up to date.");
                    }
                    self.set("upToDate");
                    return Ok(());
                }
            };
            let version = update.TargetFullRelease.Version.clone();
            {
                let mut s = guard(&self.state);
                s.new_version = Some(version.clone());
                s.progress = 0;
                s.reported_progress = -1;
            }
            log::note(&format!("Update: {version} found, downloading."));
            self.set("downloading");
            let (tx, rx) = std::sync::mpsc::channel::<i16>();
            let downloaded = std::thread::scope(|scope| {
                scope.spawn(|| {
                    for percent in rx {
                        self.on_progress(percent as i32);
                    }
                });
                manager.download_updates(&update, Some(tx))
            });
            downloaded?;
            guard(&self.state).ready = Some(update.TargetFullRelease.clone());
            log::note(&format!("Update: {version} downloaded, ready to install."));
            // Get beta versions turned off during the download: not this one.
            let status = if self.drop_unwanted_beta() { "idle" } else { "ready" };
            self.set(status);
            Ok(())
        })();
        // Offline, GitHub busy, or a download that failed its checksum: report it and try again later.
        if let Err(e) = result {
            // PORT: .NET tells these apart by HttpRequestException.StatusCode; velopack's error is read by its text.
            let text = e.to_string();
            let not_found = text.contains("404");
            {
                let mut s = guard(&self.state);
                // A private (or renamed) repository answers 404 every time: that says nothing new after the first one.
                if !not_found || !s.not_found_logged {
                    log::write(&e);
                }
                s.not_found_logged |= not_found;
                let lower = text.to_lowercase();
                s.offline = Some(!lower.contains("status") && (lower.contains("dns") || lower.contains("connect") || lower.contains("timed out") || lower.contains("network") || lower.contains("io error")));
            }
            self.set("error");
        }
    }

    fn on_progress(&self, percent: i32) {
        {
            let mut s = guard(&self.state);
            s.progress = percent;
            if percent / 5 == s.reported_progress / 5 {
                return;
            }
            s.reported_progress = percent;
        }
        self.raise_changed();
    }

    fn set(&self, status: &str) {
        {
            let mut s = guard(&self.state);
            s.status = status.to_string();
            // When the answer came, so "up to date" says as of when (it may be an hour old).
            if matches!(status, "upToDate" | "ready" | "error") {
                s.checked_at = Some(Stamp::utc_now());
            }
        }
        self.raise_changed();
    }

    /// Get beta versions turned off: a beta downloaded but not installed yet is dropped, and true is returned so the
    /// regular versions are checked again.
    pub fn beta_turned_off(&self) -> bool {
        if !self.drop_unwanted_beta() {
            return false;
        }
        self.set("idle");
        true
    }

    /// A beta downloaded while Get beta versions was on, now off: not installed after all. A quiet restart or the quit
    /// still installed it (2026-10-30 review). Its package goes too, else Velopack would install it at the next start.
    fn drop_unwanted_beta(&self) -> bool {
        let beta_updates = prefs::lock().beta_updates;
        let ready = {
            let mut s = guard(&self.state);
            let Some(ready) = s.ready.clone() else { return false };
            if beta_updates || !whats_new::is_beta(Some(&ready.Version)) {
                return false;
            }
            s.ready = None;
            s.new_version = None;
            ready
        };
        log::note(&format!("Update: downloaded beta {} dropped (Get beta versions is off).", ready.Version));
        if self.manager.is_some() {
            // PORT: VelopackLocator.Current.PackagesDir; the Rust crate's locator found from this exe.
            if let Ok(locator) = velopack::locator::auto_locate_app_manifest(velopack::locator::LocationContext::FromCurrentExe) {
                let packages = locator.get_packages_dir();
                if !packages.as_os_str().is_empty() {
                    if let Err(e) = std::fs::remove_file(packages.join(&ready.FileName)) {
                        if e.kind() != std::io::ErrorKind::NotFound {
                            log::write(&e);
                        }
                    }
                }
            }
        }
        true
    }

    /// Installs the download once this process exits; restarts with `restart_args` when given (see Relaunch), else it
    /// waits for the next start.
    pub fn apply_on_exit(&self, restart_args: Option<&[String]>) {
        let Some(manager) = &self.manager else { return };
        let Some(ready) = guard(&self.state).ready.clone() else { return };
        if HELD_FOR_INSTALLER.load(Ordering::SeqCst) {
            log::note("Update: not applied on exit; Bloxkeep's installer runs the update itself.");
            return;
        }
        log::note(&format!("Update: {} installs once Bloxkeep exits{}.", ready.Version, if restart_args.is_none() { "" } else { ", then it restarts" }));
        if let Err(e) = manager.wait_exit_then_apply_updates(ready, true, restart_args.is_some(), restart_args.unwrap_or(&[])) {
            log::write(&e);
        }
    }

    pub fn dto(&self) -> Value {
        let (status, new_version, progress, checked_at) = {
            let s = guard(&self.state);
            (s.status.clone(), s.new_version.clone(), s.progress, s.checked_at)
        };
        let (check_updates, beta_updates) = {
            let p = prefs::lock();
            (p.check_updates, p.beta_updates)
        };
        json!({
            "supported": self.supported(),
            // A quiet fix shows as the version it fixes; the full number stays for anyone looking closer.
            "current": whats_new::shown(&self.current_version),
            "build": self.current_version,
            "status": status,
            "version": new_version,
            // A quiet fix (or a beta copy's next beta) on its way: no "Restart to update" while automatic updates install it by themselves.
            "quiet": whats_new::is_quiet_update(new_version.as_deref(), Some(&self.current_version)) && check_updates,
            "progress": progress,
            "error": self.error(),
            "auto": check_updates,
            "beta": beta_updates,
            "checkedAt": checked_at.map(round_trip),
        })
    }
}

/// DateTime.ToString("o"): always seven fraction digits ("2026-11-11T10:00:00.0000000Z").
fn round_trip(at: Stamp) -> String {
    let iso = at.to_iso();
    // to_iso leaves the fraction out (or short); the zone ("Z", "+07:00") or the end follows the seconds.
    let (date, time) = iso.split_once('T').unwrap_or((&iso, ""));
    let zone_at = time.find(['Z', '+', '-']).unwrap_or(time.len());
    let (clock, zone) = time.split_at(zone_at);
    let (hms, frac) = clock.split_once('.').unwrap_or((clock, ""));
    format!("{date}T{hms}.{frac:0<7}{zone}")
}

/// GitHub releases, pre-releases only while Get beta versions is on at that check. Turned off on a beta, that
/// beta stays until a newer regular version comes out.
#[derive(Clone)]
struct ReleaseSource {
    regular: GithubSource,
    beta: GithubSource,
    /// The source that listed the last feed (true: the beta one); shared by the copies Velopack makes.
    asked: Arc<Mutex<Option<bool>>>,
}

impl ReleaseSource {
    fn new(repo: &str) -> ReleaseSource {
        ReleaseSource { regular: GithubSource::new(repo, None, false), beta: GithubSource::new(repo, None, true), asked: Arc::new(Mutex::new(None)) }
    }
}

impl UpdateSource for ReleaseSource {
    fn get_release_feed(&self, channel: &str, app: &velopack::bundle::Manifest, staged_user_id: &str) -> Result<velopack::VelopackAssetFeed, velopack::Error> {
        let beta = prefs::lock().beta_updates;
        *guard(&self.asked) = Some(beta);
        if beta { self.beta.get_release_feed(channel, app, staged_user_id) } else { self.regular.get_release_feed(channel, app, staged_user_id) }
    }

    // From the source that listed it, should the switch have changed in between.
    fn download_release_entry(&self, asset: &VelopackAsset, local_file: &str, progress_sender: Option<std::sync::mpsc::Sender<i16>>) -> Result<(), velopack::Error> {
        if *guard(&self.asked) == Some(true) {
            self.beta.download_release_entry(asset, local_file, progress_sender)
        } else {
            self.regular.download_release_entry(asset, local_file, progress_sender)
        }
    }

    fn clone_boxed(&self) -> Box<dyn UpdateSource> {
        Box::new(self.clone())
    }
}

/// Velopack's GithubSource (.NET, Velopack.Sources): the update feed from a GitHub repository's releases. No access
/// token (the C# passed none), so assets come from their browser download addresses.
#[derive(Clone)]
struct GithubSource {
    /// The repository's address, as given ("https://github.com/owner/name").
    repo: String,
    /// Pre-releases are listed too.
    prerelease: bool,
    /// Each asset (file name, version) listed by the last feed and its address in the release it came from (.NET keeps
    /// the release on the asset, a GitBaseAsset); shared by the copies Velopack makes.
    listed: Arc<Mutex<HashMap<(String, String), String>>>,
}

/// GitHub's release as its API lists it (the fields Velopack reads).
#[derive(serde::Deserialize, Default)]
#[serde(default)]
struct GithubRelease {
    prerelease: bool,
    published_at: Option<String>,
    assets: Option<Vec<GithubReleaseAsset>>,
}

#[derive(serde::Deserialize, Default)]
#[serde(default)]
struct GithubReleaseAsset {
    name: Option<String>,
    url: Option<String>,
    browser_download_url: Option<String>,
}

impl GithubSource {
    fn new(repo: &str, _access_token: Option<&str>, prerelease: bool) -> GithubSource {
        GithubSource { repo: repo.trim_end_matches('/').to_string(), prerelease, listed: Arc::new(Mutex::new(HashMap::new())) }
    }

    /// GetApiBaseUrl: api.github.com for github.com, else a GitHub Enterprise server's /api/v3/.
    fn api_base(repo: &url::Url) -> String {
        let host = repo.host_str().unwrap_or("");
        if host.eq_ignore_ascii_case("github.com") {
            "https://api.github.com/".to_string()
        } else {
            match repo.port() {
                Some(port) => format!("{}://{}:{}/api/v3/", repo.scheme(), host, port),
                None => format!("{}://{}/api/v3/", repo.scheme(), host),
            }
        }
    }

    /// GetReleases: the first page of 10, newest published first, pre-releases left out unless asked for.
    fn releases(&self, include_prereleases: bool) -> Result<Vec<GithubRelease>, velopack::Error> {
        const PER_PAGE: i32 = 10;
        const PAGE: i32 = 1;
        let repo = url::Url::parse(&self.repo)?;
        let path = format!("repos{}/releases?per_page={PER_PAGE}&page={PAGE}", repo.path().trim_end_matches('/'));
        let address = url::Url::parse(&GithubSource::api_base(&repo))?.join(&path)?;
        let text = ureq::get(address.as_str())
            .set("Accept", "application/vnd.github.v3+json")
            .call()
            .map_err(|e| velopack::Error::Generic(e.to_string()))?
            .into_string()?;
        let mut releases: Vec<GithubRelease> = serde_json::from_str(&text)?;
        // OrderByDescending(PublishedAt): stable, none (a draft) last. GitHub writes every date one way, so the text sorts.
        releases.sort_by(|a, b| b.published_at.cmp(&a.published_at));
        releases.retain(|r| include_prereleases || !r.prerelease);
        Ok(releases)
    }

    /// GetAssetUrlFromName: the release's first asset of that name; its browser address (no access token).
    fn asset_url(release: &GithubRelease, name: &str) -> Result<String, velopack::Error> {
        let Some(assets) = release.assets.as_ref().filter(|a| !a.is_empty()) else {
            return Err(velopack::Error::Generic("No assets found in GitHub Release.".to_string()));
        };
        let Some(asset) = assets.iter().find(|a| a.name.as_deref() == Some(name)) else {
            return Err(velopack::Error::Generic(format!("Could not find asset called '{name}' in GitHub Release.")));
        };
        match (&asset.url, &asset.browser_download_url) {
            (Some(_), Some(browser)) => Ok(browser.clone()),
            _ => Err(velopack::Error::Generic(format!("Could not find a valid asset url for the specified asset '{name}'."))),
        }
    }
}

impl UpdateSource for GithubSource {
    /// GitBase.GetReleaseFeed: each listed release's releases.{channel}.json, their assets together.
    fn get_release_feed(&self, channel: &str, _app: &velopack::bundle::Manifest, _staged_user_id: &str) -> Result<velopack::VelopackAssetFeed, velopack::Error> {
        let releases = self.releases(self.prerelease)?;
        let mut feed = velopack::VelopackAssetFeed { Assets: Vec::new() };
        let mut listed = HashMap::new();
        let releases_file = format!("releases.{channel}.json");
        for release in &releases {
            // A release without the feed file is skipped.
            let Ok(feed_url) = GithubSource::asset_url(release, &releases_file) else { continue };
            let text = velopack::download::download_url_as_string(&feed_url)?;
            let part: velopack::VelopackAssetFeed = serde_json::from_str(text.trim_start_matches('\u{feff}'))?;
            for asset in part.Assets {
                if let Ok(address) = GithubSource::asset_url(release, &asset.FileName) {
                    listed.entry((asset.FileName.clone(), asset.Version.clone())).or_insert(address);
                }
                feed.Assets.push(asset);
            }
        }
        *guard(&self.listed) = listed;
        Ok(feed)
    }

    /// GitBase.DownloadReleaseEntry: from the release that listed it.
    fn download_release_entry(&self, asset: &VelopackAsset, local_file: &str, progress_sender: Option<std::sync::mpsc::Sender<i16>>) -> Result<(), velopack::Error> {
        let address = guard(&self.listed).get(&(asset.FileName.clone(), asset.Version.clone())).cloned();
        let Some(address) = address else {
            return Err(velopack::Error::Generic(format!("Could not find asset called '{}' in GitHub Release.", asset.FileName)));
        };
        velopack::download::download_url_to_file(&address, local_file, move |p| {
            if let Some(sender) = &progress_sender {
                let _ = sender.send(p);
            }
        })
    }

    fn clone_boxed(&self) -> Box<dyn UpdateSource> {
        Box::new(self.clone())
    }
}
