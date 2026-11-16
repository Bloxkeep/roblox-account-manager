// Tests/Bloxkeep.Tests/WebsiteSuite.cs: roblox.com in Bloxkeep's own window, one tab per account (app/website.rs,
// forms/website_tab.rs). Every check the C# made on the browser window and its tabs opens a browser (WebView2) and is
// skipped here, one skip per check. What runs: a Roblox VNG account is refused before any tab is made, and which
// addresses a tab may show.
use serde_json::json;

use super::{call, user_error, window::quiet_app as app, Checks, PASSWORD};
use crate::core::{
    app::website::game_servers_url,
    base::new_guid,
    forms::website_tab::may_open_here,
    models::Account,
    roblox_api::{HttpResponse, STAND_IN, realm},
    vault::Vault,
};

const BROWSER: &str = "opens a browser";

pub async fn run(c: &mut Checks, _dir: &str) {
    c.check("VIP checkout opens the official Servers tab for the requested game",
        game_servers_url(2753915549).is_ok_and(|url| url == "https://www.roblox.com/games/2753915549#!/game-instances"), ());
    c.check("VIP checkout rejects an invalid place", game_servers_url(0).is_err(), ());
    // Native Roblox tabs can browse/launch with VNG without lending its cookie to roblox.com.
    let native = json!({ "id": "vng-tab", "realm": "vng", "userId": 2003, "cookieValid": true });
    c.check("VNG is available in the native Roblox tab picker", crate::pages::tabs::can_open_account(&native), ());
    let mut candidate = native.clone();
    candidate["realm"] = json!("");
    c.check("global accounts remain available in the native tab picker", crate::pages::tabs::can_open_account(&candidate), ());
    candidate["cookieValid"] = json!(false);
    c.check("signed-out accounts are excluded from the native tab picker", !crate::pages::tabs::can_open_account(&candidate), ());
    candidate["cookieValid"] = json!(true);
    candidate["userId"] = json!(0);
    c.check("unidentified accounts are excluded from the native tab picker", !crate::pages::tabs::can_open_account(&candidate), ());
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
    let (app, _said) = app();
    // The vault is already unlocked for this suite. Skip the startup account refresh so the HTTP
    // stand-in below receives only calls made by the panels under test.
    *app.controller.vault.borrow_mut() = Some(std::sync::Arc::new(seed));

    c.check("native VNG tab does not receive a roblox.com cookie", app.roblox_sign_in(&json!({ "id": vng_id.to_string() }))["cookie"].is_null(), ());
    c.check("native global tabs still receive their own sign-in", app.roblox_sign_in(&json!({}))["userId"] == json!(2001), ());

    *STAND_IN.write().unwrap() = Some(Box::new(|request| {
        let url = url::Url::parse(&request.url).expect("VNG URL");
        let host = url.host_str().unwrap();
        assert!(host.ends_with(".robloxapp.vnggames.com"));
        assert_eq!(request.method, "GET");
        let public = ["users.", "avatar.", "thumbnails."].iter().any(|prefix| host.starts_with(prefix));
        assert_eq!(request.headers_named("Cookie").next().is_none(), public);
        if let Some(cookie) = request.headers_named("Cookie").next() {
            assert!(cookie.starts_with(".ROBLOSECURITY=_TEST_V_"));
        }
        let body = match host.split('.').next().unwrap() {
            "friends" => json!({ "PageItems": [{ "id": 4, "name": "Friend" }] }),
            "users" => json!({ "name": "webV", "displayName": "VNG Tester" }),
            "avatar" => json!({ "avatarModel": { "assets": [{ "name": "VNG hat" }] } }),
            "thumbnails" => json!({ "data": [{ "imageUrl": "https://tr.rbxcdn.com/avatar.png" }] }),
            "privatemessages" => json!({ "collection": [{ "subject": "Test message", "sender": { "name": "Friend" } }] }),
            "notifications" => json!([{ "content": { "states": { "default": { "visualItems": { "textBody": [{ "label": { "text": "New activity" } }] } } } } }]),
            "accountsettings" => json!({ "emailAddress": "tester@example.invalid" }),
            "accountinformation" => json!({ "description": "Hello", "isVerified": true, "phone": "hidden" }),
            _ => panic!("unexpected Roblox VNG request"),
        };
        Ok(HttpResponse { status: 200, body: serde_json::to_vec(&body).unwrap(), ..Default::default() })
    }));
    for (panel, pointer, expected) in [
        ("friends", "/rows/0/title", "Friend"),
        ("profile", "/title", "VNG Tester"),
        ("avatar", "/rows/0/title", "VNG hat"),
        ("messages", "/rows/0/title", "Test message"),
        ("notifications", "/rows/0/title", "New activity"),
        ("settings", "/email", "tester@example.invalid"),
    ] {
        let result = call(&app, "vngPanel", json!({ "id": vng_id, "panel": panel })).await;
        c.check(&format!("VNG panel {panel} returns display data"), result.is_ok_and(|v| {
            v.pointer(pointer).and_then(serde_json::Value::as_str) == Some(expected)
                && !v.to_string().contains("_TEST_V_") && v.get("phone").is_none()
        }), ());
    }
    let global_id = app.roblox_sign_in(&json!({}))["id"].clone();
    c.check("VNG panels reject a global account", call(&app, "vngPanel", json!({ "id": global_id, "panel": "settings" })).await.is_err(), ());
    c.check("VNG panels reject arbitrary service names", call(&app, "vngPanel", json!({ "id": vng_id, "panel": "https://example.invalid" })).await.is_err(), ());
    *STAND_IN.write().unwrap() = None;

    // Refused before any tab (or browser) is made: open_website checks the realm first.
    let refused = call(&app, "openWebsite", json!({ "id": vng_id.to_string() })).await;
    c.check(
        "a Roblox VNG account is refused",
        user_error(&refused).is_some() && app.website.websites.borrow().is_empty(),
        format!("{refused:?}"),
    );
    let refused = call(&app, "openGameServers", json!({ "id": vng_id, "placeId": 2753915549_i64 })).await;
    c.check("VIP checkout never opens a Global website tab with a VNG account",
        user_error(&refused).is_some() && app.website.websites.borrow().is_empty(), ());
    let refused = call(&app, "gameCommunity", json!({ "id": vng_id, "groupId": 4372130, "action": "join" })).await;
    c.check("A VNG account cannot change Roblox Global community membership", user_error(&refused).is_some(), ());

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
