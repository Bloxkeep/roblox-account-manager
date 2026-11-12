// Tests/Bloxkeep.Tests/WebsiteSuite.cs: roblox.com in Bloxkeep's own window, one tab per account (app/website.rs,
// forms/website_tab.rs). Every check the C# made on the browser window and its tabs opens a browser (WebView2) and is
// skipped here, one skip per check. What runs: a Roblox VNG account is refused before any tab is made, and which
// addresses a tab may show.
use serde_json::json;

use super::{call, unlock, user_error, window::quiet_app as app, Checks, PASSWORD};
use crate::core::{
    base::new_guid,
    forms::website_tab::may_open_here,
    models::Account,
    roblox_api::realm,
    vault::Vault,
};

const BROWSER: &str = "opens a browser";

pub async fn run(c: &mut Checks, _dir: &str) {
    let mark = new_guid().simple().to_string()[..10].to_string();
    let seed = Vault::create(PASSWORD, None).expect("vault");
    let vng_id = {
        let mut d = seed.data();
        let a = Account { username: "webA".into(), cookie: format!("_TEST_A_{mark}"), user_id: 2001, cookie_valid: Some(true), ..Default::default() };
        let b = Account { username: "webB".into(), cookie: format!("_TEST_B_{mark}"), user_id: 2002, cookie_valid: Some(true), ..Default::default() };
        let vng = Account {
            username: "webV".into(),
            cookie: format!("_TEST_V_{mark}"),
            user_id: 2003,
            cookie_valid: Some(true),
            realm: realm::VNG.to_string(),
            ..Default::default()
        };
        let id = vng.id;
        d.accounts.extend([a, b, vng]);
        id
    };
    seed.save().expect("saved");
    drop(seed);
    let (app, _said) = app();
    unlock(&app).await;

    // Refused before any tab (or browser) is made: open_website checks the realm first.
    let refused = call(&app, "openWebsite", json!({ "id": vng_id.to_string() })).await;
    c.check(
        "a Roblox VNG account is refused",
        user_error(&refused).is_some() && app.website.websites.borrow().is_empty(),
        format!("{refused:?}"),
    );

    for what in [
        "one tab per account, both in one window",
        "a new tab is the one shown",
        "opening an account again goes back to its tab",
        "both browsers start",
        "the window opens on screen",
        "the tabs and the window say which account",
        "each tab is InPrivate",
        "each tab has a profile of its own",
        "the tabs share one browser",
        "tab A holds A's session only",
        "tab B holds B's session only",
        "signing out from the page is stopped and explained",
        "the session survives it",
        "a sign-out link is stopped too",
        "\"log out of other sessions\" isn't stopped",
        "a renewed session is handed to the app once",
        "tab B still holds B's session",
        "no session cookie on disk while open",
        "\"Close tab\" closes that tab only",
        "the app lets go of a closed tab",
        "locking closes the window and its tabs",
        "the browser's folder is deleted after the last tab",
        "the next tab gets a new folder",
        "each Open roblox.com is a new blank tab in the one window",
        "both blank browsers start",
        "a blank tab starts signed out",
        "+ opens a blank tab and shows it",
        "a tab's ✕ closes only that tab",
        "a tab not saved signs out like any browser",
        "signing in on the site names the tab and offers Save",
        "the two blank tabs keep their own sign-in",
        "nothing is saved before Save",
        "Save to Bloxkeep adds the account with that sign-in",
        "the tab is then that account's",
        "once saved, its sign-out is stopped",
        "signing out on a blank tab: nobody again",
        "a sign-in Roblox answers \"User is moderated\" for: asked once, not at every read",
        "…Roblox saying \"too many\": asked once, then after a wait",
        "a page off Roblox's site doesn't load in the tab (a link, a redirect, a script)",
    ] {
        c.skip(what, BROWSER);
    }

    c.check(
        "…Roblox's own pages and Play's link may",
        may_open_here("https://www.roblox.com/games/1")
            && may_open_here("https://auth.roblox.com/v2/login")
            && may_open_here("roblox-player:1+launchmode:play+gameinfo:x")
            && !may_open_here("https://roblox.com.evil.example/")
            && !may_open_here("http://www.roblox.com/")
            && !may_open_here("https://example.com/?u=www.roblox.com"),
        (),
    );

    for what in ["no blank tab's session on disk", "closing the window closes every tab", "their folder is deleted"] {
        c.skip(what, BROWSER);
    }
}
