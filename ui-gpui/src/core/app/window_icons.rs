// AppController.WindowIcons.cs: the account's avatar as its Roblox window's icon (window_icons.rs), fetched once, and its
// name in the title. And Settings › Taskbar buttons (taskbar_groups.rs).
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
};

use super::App;
use crate::core::{
    base::{Cancel, Guid},
    log,
    models::Account,
    roblox_processes, rt, summary_card, taskbar_groups,
    win_kit::Picture,
    window_icons,
};

/// The most pictures kept by address before they are all dropped.
pub const MAX_ICON_PICTURES: usize = 200;

#[derive(Default)]
pub struct WindowIconsState {
    /// Pictures by address; None when it couldn't be read (the initials then).
    pub icon_pictures: RefCell<HashMap<String, Option<Picture>>>,
    pub icon_loading: RefCell<HashSet<String>>,
    /// Settings › Taskbar buttons (taskbar_groups.rs): a button per account, or a group per preset (windows without one
    /// stay in Roblox's); "roblox" puts every window's own id back.
    pub taskbar_leftovers_cleared: Cell<bool>,
}

/// An avatar's bytes as a picture; None when the decoder can't read them.
// PORT: System.Drawing's Image.FromStream (GDI+) read the picture; here the `image` crate does (PNG, JPEG, GIF, BMP,
// WebP as its features allow). A picture it can't read gets the initials, as one GDI+ couldn't read did.
fn decode_picture(bytes: &[u8]) -> Option<Picture> {
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    Some(Picture::from_rgba(w as i32, h as i32, img.as_raw()))
}

impl App {
    pub fn apply_window_icons(self: &Rc<Self>, alive: &HashSet<i32>, windows: &HashMap<i32, Vec<isize>>) {
        window_icons::forget(alive);
        let on = self.vault(|d| d.settings.window_pictures.unwrap_or(d.settings.rename_windows)).unwrap_or(false);
        if !on {
            for pid in window_icons::pids() {
                window_icons::remove(pid, windows.get(&pid).map_or(&[][..], |w| &w[..]));
            }
            return;
        }
        let running: Vec<(Guid, i32)> = self.controller.running.borrow().iter().map(|(id, r)| (*id, r.0)).collect();
        let mut shown: Vec<(i32, Vec<isize>, Account)> = Vec::new();
        for (id, pid) in running {
            if let Some(account) = self.find_account(id)
                && let Some(list) = windows.get(&pid).filter(|l| !l.is_empty())
            {
                shown.push((pid, list.clone(), account));
            }
        }
        // One client's windows are asked whether they still have their icon at a time, each every half a minute.
        let check = window_icons::next_to_check(&shown.iter().map(|s| s.0).collect::<Vec<_>>());
        for (pid, list, account) in &shown {
            let url = self.controller.avatar_urls.borrow().get(&account.user_id).cloned();
            let mut picture: Option<Picture> = None;
            if let Some(url) = &url {
                match self.window_icons.icon_pictures.borrow().get(url) {
                    Some(known) => picture = known.clone(),
                    None => {
                        let app = self.clone();
                        let url = url.clone();
                        // C#: `_ = LoadIconPictureAsync(url)`.
                        rt::spawn(async move { app.load_icon_picture(url).await });
                    }
                }
            }
            match (picture, &url) {
                (Some(picture), Some(url)) => {
                    window_icons::apply(*pid, list, url, &|| picture.clone(), Some(*pid) == check);
                }
                _ => {
                    let name = self.name_text(account); // "F…" while names are hidden
                    window_icons::apply(*pid, list, &format!("initials:{name}"), &|| window_icons::initials(&name), Some(*pid) == check);
                }
            }
        }
    }

    pub async fn load_icon_picture(self: &Rc<Self>, url: String) {
        if !self.window_icons.icon_loading.borrow_mut().insert(url.clone()) {
            return;
        }
        let address = url.clone();
        let picture: Option<Picture> = rt::blocking(move || summary_card::load_avatar(&address, &Cancel::new()).and_then(|bytes| decode_picture(&bytes))).await;
        {
            let mut pictures = self.window_icons.icon_pictures.borrow_mut();
            if pictures.len() >= MAX_ICON_PICTURES {
                pictures.clear();
            }
            pictures.insert(url.clone(), picture);
        }
        self.window_icons.icon_loading.borrow_mut().remove(&url);
    }

    pub fn apply_taskbar_groups(&self, alive: &HashSet<i32>, windows: &HashMap<i32, Vec<isize>>) {
        taskbar_groups::forget(alive);
        // The first look at the windows: the groups a run that ended suddenly left on them go, whatever the setting.
        if !self.window_icons.taskbar_leftovers_cleared.get() && !windows.is_empty() {
            self.window_icons.taskbar_leftovers_cleared.set(true);
            let all: Vec<isize> = windows.values().flatten().copied().collect();
            let cleared = taskbar_groups::clear_leftovers(&all);
            if cleared > 0 {
                log::note(&format!("Taskbar buttons: {cleared} Roblox window(s) freed from a group a crashed run left."));
            }
        }
        let mode = self.vault(|d| d.settings.taskbar_groups.clone()).unwrap_or_else(|_| "roblox".to_string());
        let mut grouped: HashSet<i32> = HashSet::new();
        let running: Vec<(Guid, i32, Option<Guid>)> = self.controller.running.borrow().iter().map(|(id, r)| (*id, r.0, r.1.preset_id)).collect();
        for (id, pid, preset) in running {
            // `{id:N}`: the id's 32 hex digits, no dashes.
            let group = match (mode.as_str(), preset) {
                ("account", _) => Some(format!("Bloxkeep.Roblox.{}", id.simple())),
                ("preset", Some(preset)) => Some(format!("Bloxkeep.Preset.{}", preset.simple())),
                _ => None,
            };
            let Some(group) = group else { continue };
            let Some(list) = windows.get(&pid).filter(|l| !l.is_empty()) else { continue };
            taskbar_groups::apply(pid, list, &group);
            grouped.insert(pid);
        }
        for pid in taskbar_groups::pids().into_iter().filter(|pid| !grouped.contains(pid)) {
            taskbar_groups::remove(pid, windows.get(&pid).map_or(&[][..], |w| &w[..]));
        }
    }

    /// Bloxkeep closing: the icons are Bloxkeep's and go with it, so a window left pointing at one would show none.
    pub fn give_icons_back(&self) {
        for pid in window_icons::pids() {
            let found = roblox_processes::visible_windows(&[pid]);
            window_icons::remove(pid, found.get(&pid).map_or(&[][..], |w| &w[..]));
        }
        // And their own taskbar button: a group named after Bloxkeep's accounts means nothing once it has closed.
        for pid in taskbar_groups::pids() {
            let found = roblox_processes::visible_windows(&[pid]);
            taskbar_groups::remove(pid, found.get(&pid).map_or(&[][..], |w| &w[..]));
        }
    }
}
