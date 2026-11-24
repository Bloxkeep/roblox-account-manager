// Tests/Bloxkeep.Tests/WebsiteSuite.cs: roblox.com in Bloxkeep's own window, one tab per account (app/website.rs,
// forms/website_tab.rs). Every check the C# made on the browser window and its tabs opens a browser (WebView2) and is
// skipped here, one skip per check. What runs: a Roblox VNG account is refused before any tab is made, which
// addresses a tab may show, and where a VNG account's sign-in goes in the Roblox tab.
use serde_json::json;

use super::{call, user_error, window::quiet_app as app, Checks, PASSWORD};
use crate::core::{
    app::website::game_servers_url,
    base::new_guid,
    forms::website_tab::may_open_here,
    models::Account,
    roblox_api::realm,
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

    // A VNG tab gets its own account's sign-in, marked as VNG's, so the Roblox tab sends its calls to VNG's hosts.
    let lent = app.roblox_sign_in(&json!({ "id": vng_id.to_string() }));
    let vng_cookie = lent["cookie"].as_str().unwrap_or_default().to_string();
    c.check("native VNG tab receives its own sign-in", vng_cookie.starts_with("_TEST_V_") && lent["realm"] == json!("vng"), &lent["realm"]);
    c.check("…marked as a VNG sign-in", crate::roblox::vng::owns(&vng_cookie), ());
    let global = app.roblox_sign_in(&json!({}));
    c.check("native global tabs still receive their own sign-in", global["userId"] == json!(2001) && global["realm"] == json!(""), &global["userId"]);
    c.check("…never a VNG account's, nor marked as one", !crate::roblox::vng::owns(global["cookie"].as_str().unwrap_or_default()), ());
    // Where a VNG sign-in's calls go: the same path on VNG's host; anything not on roblox.com stays where it was.
    for (url, want) in [
        ("https://avatar.roblox.com/v1/avatar", "https://avatar.robloxapp.vnggames.com/v1/avatar"),
        ("https://www.roblox.com/my/settings/json?x=1", "https://www.robloxapp.vnggames.com/my/settings/json?x=1"),
        ("https://apis.roblox.com", "https://apis.robloxapp.vnggames.com"),
        ("https://friends.roblox.com?x", "https://friends.robloxapp.vnggames.com?x"),
        ("https://tr.rbxcdn.com/a.png", "https://tr.rbxcdn.com/a.png"),
        ("https://roblox.com.example.invalid/x", "https://roblox.com.example.invalid/x"),
        ("https://a.b.roblox.com/x", "https://a.b.roblox.com/x"),
    ] {
        let got = crate::roblox::vng::route(url);
        c.check(&format!("VNG route of {url}"), got == want, &got);
    }

    // Content maturity, reminders and age groups, read from answers shaped as Roblox's own website code reads them.
    let m = crate::roblox::maturity_of(
        &json!({ "contentAgeRestriction": { "currentValue": "NinePlus", "options": [
            { "option": { "optionValue": "AllAges" }, "requirement": "SelfUpdateSetting" },
            { "option": { "optionValue": "ThirteenPlus" }, "requirement": null },
            { "option": { "optionValue": "SeventeenPlus" }, "requirement": "ContentAgeRestrictionVerification" },
            { "option": { "optionValue": "EighteenPlus" }, "requirement": "None" } ] } }),
        false,
    );
    let want = Some(crate::roblox::Maturity {
        now: "NinePlus".into(),
        options: vec![("AllAges".into(), None), ("ThirteenPlus".into(), None), ("SeventeenPlus".into(), Some("ContentAgeRestrictionVerification".into()))],
        local: false,
    });
    c.check("content maturity: levels offered, an age check kept as what Restricted needs", m == want, format!("{m:?}"));
    c.check("…none when Roblox has no such setting", crate::roblox::maturity_of(&json!({ "whoCanJoinMeInExperiences": {} }), false).is_none(), ());
    let r = crate::roblox::reminder_of(&json!({ "shouldSurfaceReminder": true, "interventionId": "i1", "policyViolation": "Experiment.Reminders.ViolationScams", "reminderNumber": 1 }));
    c.check("a reminder of the rules is read", r.as_ref().is_some_and(|r| r.id == "i1" && r.rule.ends_with("Scams")), format!("{r:?}"));
    c.check("…none when Roblox says not to show one", crate::roblox::reminder_of(&json!({ "shouldSurfaceReminder": false })).is_none(), ());
    let l = crate::roblox::reminder_of(&json!({ "shouldSurfaceReminder": true, "nudgeType": "TEXT_FILTER_EDUCATION", "experimentVariant": "change_or_block", "contentVariant": "no_spam", "interventionId": "i2" }));
    c.check("the chat-filter lesson is read with its kind and tip", l.as_ref().and_then(|r| r.lesson.clone()) == Some(crate::roblox::Lesson { changed: true, tip: "spam" }), format!("{l:?}"));
    let l = crate::roblox::reminder_of(&json!({ "shouldSurfaceReminder": true, "nudgeType": "TEXT_FILTER_EDUCATION", "experimentVariant": "warning", "interventionId": "i3" }));
    c.check("…the warning one, its tip respect when Roblox names none", l.as_ref().and_then(|r| r.lesson.clone()) == Some(crate::roblox::Lesson { changed: false, tip: "respect" }), format!("{l:?}"));
    c.check("…a nudge of a kind not known is left out", crate::roblox::reminder_of(&json!({ "shouldSurfaceReminder": true, "nudgeType": "NEW_KIND", "policyViolation": "x" })).is_none(), ());
    for (key, want) in [("Label.AgeGroup13To15", "13–15"), ("Label.AgeGroup18Plus", "18+"), ("Label.AgeGroupUnder9", "<9"), ("Label.AgeGroupUnknown", "")] {
        let got = crate::roblox::age_words(key);
        c.check(&format!("age group {key}"), got == want, &got);
    }

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
