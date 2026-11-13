// The app's places, one per rail button. Since 2026-11-09 five and Settings: the Roblox windows (the old Game monitor)
// live in Accounts, beside the accounts they belong to; a game's own page is in Games; Presets and Macros share
// Automation (things Bloxkeep does for you). Each is designed new for 1.1 in the "Tiết chế" look; the behaviour (the
// commands and what they do) is the app's as it is.
pub mod account_form;
pub mod accounts;
pub mod auto;
pub mod extras;
pub mod avatar;
pub mod friends;
pub mod preset_form;
pub mod preset_menu;
pub mod profile;
pub mod quick;
pub mod settings;
pub mod lock;
pub mod macro_form;
pub mod palette;
pub mod passes;
pub mod password;
pub mod picker;
pub mod servers;
pub mod snap;
pub mod stats;
pub mod tabs;
pub mod visibility;
pub mod games;
pub mod clips;
pub mod home;
pub mod import_vault;
pub mod inbox;
pub mod rsettings;
pub mod join;
pub mod wiki;
pub mod blocks;

use gpui_kit::{prelude::*, *};

use crate::{shell::Root, theme::*};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Home,
    Accounts,
    Games,
    Auto,
    Stats,
    Settings,
}

impl Page {
    pub const ALL: [Page; 6] = [Page::Home, Page::Accounts, Page::Games, Page::Auto, Page::Stats, Page::Settings];

    /// The place for a saved view's name, the old places' names included (they moved into the new ones).
    pub fn from_key(key: &str) -> Option<Page> {
        match key {
            "monitor" => Some(Page::Accounts),
            "macros" => Some(Page::Auto),
            k => Page::ALL.into_iter().find(|p| p.key() == k),
        }
    }

    /// The name the app keeps in its saved view (keepView) and app.js used.
    pub fn key(self) -> &'static str {
        match self {
            Page::Home => "home",
            Page::Accounts => "accounts",
            Page::Games => "news",
            Page::Auto => "presets",
            Page::Stats => "stats",
            Page::Settings => "settings",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Page::Home => "home",
            Page::Accounts => "users",
            Page::Games => "gamepad",
            Page::Auto => "repeat",
            Page::Stats => "clock",
            Page::Settings => "sliders",
        }
    }

    pub fn label_key(self) -> &'static str {
        match self {
            Page::Home => "nav.home",
            Page::Accounts => "nav.accounts",
            Page::Games => "nav.roblox",
            Page::Auto => "nav.auto",
            Page::Stats => "nav.stats",
            Page::Settings => "nav.settings",
        }
    }
}

/// A place not made yet: its name, so the frame can be checked around it.
pub fn not_yet(root: &Root, page: Page) -> impl IntoElement {
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(6.))
        .child(div().text_size(px(T_TITLE)).font_weight(FontWeight::SEMIBOLD).child(root.i18n.t(page.label_key())))
        .child(div().text_size(px(T_M)).text_color(c(MUTED)).child("1.1"))
}
