// Tests/Bloxkeep.Tests/FpsCapSuite.cs: an account's own frame rate limit in Roblox (roblox_game_settings for_launch,
// prepare_for, restore), on a stand-in Roblox folder (never the real one): the limit goes over the shared one and works
// without it, light AFK takes only a lower one; the file holds it for that account's opening, the next account gets the
// person's own, and the person's own comes back even after a client saves the limit; a limit is any number from 5 to 360, or 9999 for none.
use std::path::Path;

use serde_json::json;

use super::{call, unlock, window::quiet_app as app, Checks, PASSWORD};
use crate::core::{
    models::Account,
    roblox_game_settings::{self as rgs, RobloxConfig},
    vault::Vault,
};

const XML: &str = r#"<roblox><Item class="UserGameSettings"><Properties>
<int name="FramerateCap">240</int><int name="GraphicsQualityLevel">19</int><token name="SavedQualityLevel">9</token>
<float name="MasterVolume">0.5</float><bool name="Fullscreen">true</bool><string name="Other">kept</string>
</Properties></Item></roblox>"#;

fn cfg(c: &Option<RobloxConfig>) -> Option<(i32, i32, i32)> {
    c.as_ref().map(|c| (c.fps, c.graphics, c.volume))
}

pub async fn run(c: &mut Checks, dir: &str) {
    let roblox = Path::new(dir).join("Roblox");
    std::fs::create_dir_all(&roblox).expect("folder");
    rgs::set_roblox_dir(Some(roblox.to_string_lossy().into_owned()));
    let file = roblox.join("GlobalBasicSettings_13.xml");
    std::fs::write(&file, XML).expect("written");
    let text = || std::fs::read_to_string(&file).unwrap_or_default();
    let fps = || {
        let t = text();
        let re = regex::Regex::new("FramerateCap\">([^<]*)<").unwrap();
        re.captures(&t).map(|m| m[1].to_string()).unwrap_or_default()
    };

    let shared = RobloxConfig { mode: "all".into(), graphics: 3, fps: 60, volume: 20, ..Default::default() };
    let uses_both = rgs::for_launch(&shared, false, true, 15);
    c.check(
        "the account's limit goes over the shared one, the rest of the shared setup stays",
        cfg(&uses_both) == Some((15, 3, 20)) && shared.fps == 60,
        cfg(&uses_both),
    );
    let alone = rgs::for_launch(&shared, false, false, 45);
    c.check("…it works without the shared setup, which is left out", cfg(&alone) == Some((45, 0, -1)), cfg(&alone));
    c.check("no limit and no shared setup: nothing of Bloxkeep's", rgs::for_launch(&shared, false, false, 0).is_none(), ());
    c.check("light AFK takes a lower limit", cfg(&rgs::for_launch(&shared, true, false, 15)) == Some((15, 1, 0)), cfg(&rgs::for_launch(&shared, true, false, 15)));
    c.check(
        "…but stays light under a higher one",
        rgs::for_launch(&shared, true, false, 240).is_some_and(|c| c.fps == 30),
        cfg(&rgs::for_launch(&shared, true, false, 240)),
    );
    let uses_both = uses_both.expect("a setup");
    let alone = alone.expect("a setup");

    c.check("opening the account: the file holds its limit", rgs::prepare_for(&alone, true, false) && fps() == "45", fps());
    c.check("…nothing else changes", text() == XML.replace(">240<", ">45<"), text());
    c.check("the next account without one gets the person's own", rgs::prepare_for(&shared, false, false) && fps() == "240", fps());

    rgs::prepare_for(&uses_both, true, false);
    c.check("a different limit for the next one", fps() == "15", fps());
    rgs::restore(false);
    c.check("once started, the person's own come back", text() == XML, text());

    std::fs::write(&file, XML.replace(">240<", ">15<")).unwrap(); // Roblox saves what it runs at as it joins a game
    rgs::restore(false);
    c.check("…and again after a client saves the limit", fps() == "240", fps());

    rgs::prepare_for(&alone, true, false);
    std::fs::write(&file, XML.replace(">240<", ">144<")).unwrap(); // changed in Roblox's own menu
    rgs::restore(true);
    c.check("a value the person chose in Roblox is kept", fps() == "144" && !rgs::has_baseline(), (fps(), rgs::has_baseline()));

    // A last put-back that can't write (the file held, read-only): the person's own are kept for the next try.
    let copy = Path::new(dir).join("roblox-settings.json");
    let read_only = |on: bool| {
        let mut p = std::fs::metadata(&file).unwrap().permissions();
        p.set_readonly(on);
        std::fs::set_permissions(&file, p).unwrap();
    };
    std::fs::write(&file, XML).unwrap();
    rgs::prepare_for(&alone, true, false);
    read_only(true);
    rgs::restore(true);
    c.check("a last put-back that can't write keeps the person's own", rgs::has_baseline() && fps() == "45", (rgs::has_baseline(), fps()));
    read_only(false);
    rgs::restore(true);
    c.check("…and puts them back at the next try", text() == XML && !rgs::has_baseline(), fps());

    // The copy can't be read for a moment: the file isn't touched, and the copy isn't replaced by Bloxkeep's values.
    std::fs::write(&file, XML).unwrap();
    rgs::prepare_for(&alone, true, false);
    {
        use std::os::windows::fs::OpenOptionsExt;
        let _held = std::fs::OpenOptions::new().write(true).share_mode(0x2 | 0x4).open(&copy).expect("held");
        c.check("a copy that can't be read: nothing written", !rgs::prepare_for(&uses_both, true, false) && fps() == "45", fps());
    }
    c.check("…and the person's own still come back after", rgs::prepare_for(&shared, false, false) && fps() == "240", fps());
    rgs::restore(true);

    // A damaged copy: set aside as it was, and Light AFK's values in the file are never taken for the person's own.
    std::fs::write(&file, XML).unwrap();
    rgs::prepare_for(&rgs::light_afk(), true, false);
    std::fs::write(&copy, "{ damaged").unwrap();
    let rebased = rgs::prepare_for(&alone, true, false);
    let aside: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("roblox-settings.json.damaged-")))
        .collect();
    c.check(
        "a damaged copy is set aside as it was",
        aside.len() == 1 && std::fs::read_to_string(&aside[0]).unwrap_or_default() == "{ damaged",
        &aside,
    );
    c.check(
        "…and Light AFK's values aren't taken for the person's own",
        !rebased && !rgs::has_baseline() && fps() == "30",
        format!("{rebased} {}", fps()),
    );
    std::fs::write(&file, XML).unwrap(); // the person sets their own again in Roblox
    c.check(
        "…once the file holds the person's own, it works as before",
        rgs::prepare_for(&alone, true, false) && fps() == "45" && rgs::has_baseline(),
        fps(),
    );
    rgs::restore(true);
    c.check("…and puts them back", text() == XML && !rgs::has_baseline(), text());

    Vault::create(PASSWORD, None).expect("vault");
    let (app, _said) = app();
    unlock(&app).await;
    let one = Account::default();
    let two = Account::default();
    let (one_id, two_id) = (one.id, two.id);
    let _ = app.vault_mut(|d| d.accounts.extend([one, two]));
    let cap = |id| app.find_account(id).map_or(-1, |a| a.fps_cap);

    let _ = call(&app, "setFpsCap", json!({ "ids": [one_id, two_id], "fps": 20 })).await;
    c.check("a limit is set on several accounts at once", cap(one_id) == 20 && cap(two_id) == 20, (cap(one_id), cap(two_id)));
    // Any whole limit from 5 to 360 (a user asked for 17 and 18, 2026-11-15), and 9999 for none; nothing else.
    let _ = call(&app, "setFpsCap", json!({ "ids": [one_id], "fps": 17 })).await;
    c.check("…a limit of the person's own (17) is taken", cap(one_id) == 17, cap(one_id));
    let _ = call(&app, "setFpsCap", json!({ "ids": [one_id], "fps": 9999 })).await;
    c.check("…and no limit at all (9999)", cap(one_id) == 9999, cap(one_id));
    for odd in [3, 361, 5000] {
        let _ = call(&app, "setFpsCap", json!({ "ids": [one_id], "fps": odd })).await;
    }
    c.check("…a number out of range changes nothing", cap(one_id) == 9999, cap(one_id));
    let _ = call(&app, "setFpsCap", json!({ "ids": [one_id], "fps": 0 })).await;
    c.check("…and taken off one account alone", cap(one_id) == 0 && cap(two_id) == 20, (cap(one_id), cap(two_id)));

    let _ = call(&app, "setSettings", json!({ "robloxConfig": { "fps": 15 } })).await;
    let shared_fps = app.vault(|d| d.settings.roblox_config.fps).unwrap_or(-1);
    c.check("the shared limit offers the low ones too", shared_fps == 15, shared_fps);
}
