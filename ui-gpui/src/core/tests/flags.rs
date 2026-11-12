// Tests/Bloxkeep.Tests/FlagsSuite.cs: Light AFK's Fast Flags (roblox_flags.rs), on a stand-in Roblox folder (never the
// real one): written into every build's ClientAppSettings.json, a file of the person's own keeping its other flags; put
// back as it was, one Bloxkeep made removed with its folder; a second window's writing doesn't lose the person's
// original; a file changed since, or one that isn't JSON, is left alone; a Bloxkeep that closed before putting them back
// does it at its next start.
use std::os::windows::fs::OpenOptionsExt;
use std::path::Path;

use serde_json::{Map, Value};

use super::Checks;
use crate::core::roblox_flags;

fn read_only(path: &Path, on: bool) {
    if let Ok(meta) = std::fs::metadata(path) {
        let mut p = meta.permissions();
        p.set_readonly(on);
        let _ = std::fs::set_permissions(path, p);
    }
}

fn text(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

pub async fn run(c: &mut Checks, dir: &str) {
    let versions = Path::new(dir).join("Roblox").join("Versions");
    let (a, b, old) = (versions.join("version-aaaa"), versions.join("version-bbbb"), versions.join("version-gone"));
    for build in [&a, &b] {
        std::fs::create_dir_all(build).expect("a build");
        std::fs::write(build.join("RobloxPlayerBeta.exe"), "").expect("a client");
    }
    std::fs::create_dir_all(&old).expect("a build folder"); // a build folder with no client in it: not touched
    let mine = a.join("ClientSettings").join("ClientAppSettings.json");
    let made = b.join("ClientSettings").join("ClientAppSettings.json");
    std::fs::create_dir_all(mine.parent().unwrap()).expect("a folder");
    const OWN: &str = r#"{"FFlagMyOwn": true, "FIntDebugForceMSAASamples": 4}"#;
    std::fs::write(&mine, OWN).expect("written");
    let flags = |file: &Path| -> Map<String, Value> {
        match serde_json::from_str::<Value>(&text(file)) {
            Ok(Value::Object(o)) => o,
            _ => Map::new(),
        }
    };
    let exe_of = |build: &Path| build.join("RobloxPlayerBeta.exe").to_string_lossy().into_owned();

    let wrote = roblox_flags::apply(Some(&exe_of(&b)));
    c.check(
        "written into every build next to the client",
        wrote
            && made.exists()
            && flags(&made).get("FFlagDebugSkyGray").and_then(Value::as_bool) == Some(true)
            && flags(&made).get("DFIntDebugFRMQualityLevelOverride").and_then(Value::as_i64) == Some(1)
            && !old.join("ClientSettings").exists(),
        (),
    );
    c.check(
        "…the person's own flags kept, the ones Light AFK sets in its values",
        flags(&mine).get("FFlagMyOwn").and_then(Value::as_bool) == Some(true)
            && flags(&mine).get("FIntDebugForceMSAASamples").and_then(Value::as_i64) == Some(0)
            && roblox_flags::pending(),
        (),
    );
    let again = roblox_flags::apply(Some(&exe_of(&a)));
    c.check(
        "a second light-AFK window: written again",
        again && flags(&mine).get("DFFlagDebugPauseVoxelizer").map(|v| v.to_string()).as_deref() == Some("true"),
        format!("{again} {}", text(&mine)),
    );

    roblox_flags::restore();
    c.check("put back: the person's file exactly as it was", text(&mine) == OWN, ());
    c.check(
        "…the one Bloxkeep made removed, with its folder",
        !made.exists() && !made.parent().unwrap().exists() && !roblox_flags::pending(),
        (),
    );

    roblox_flags::apply(Some(&exe_of(&a)));
    const EDITED: &str = r#"{"FFlagEditedSince": true}"#;
    std::fs::write(&mine, EDITED).expect("written");
    roblox_flags::restore();
    c.check("a file changed since it was written is left as it is now", text(&mine) == EDITED && !made.exists(), ());

    std::fs::write(&mine, "not json").expect("written");
    roblox_flags::apply(Some(&exe_of(&a)));
    c.check("a file of the person's that isn't JSON is left alone", text(&mine) == "not json" && made.exists(), ());
    roblox_flags::restore();
    c.check("…and the other build's put back", !made.exists(), ());

    // Closed before putting them back: the next start does it (the App sees it pending from the start).
    std::fs::write(&mine, OWN).expect("written");
    roblox_flags::apply(Some(&exe_of(&a)));
    c.check("still pending after a restart (kept on disk)", roblox_flags::pending(), ());
    roblox_flags::restore();
    c.check("…and put back then", text(&mine) == OWN && !roblox_flags::pending(), ());

    c.check("no client: nothing written", !roblox_flags::apply(None), ());

    // A restore that can't write (the file held, a build without the rights) keeps the person's own for the next try.
    let exe = exe_of(&a);
    let state = Path::new(dir).join("roblox-flags.json");
    roblox_flags::apply(Some(&exe));
    read_only(&mine, true);
    read_only(&made, true);
    roblox_flags::restore();
    c.check(
        "a file that can't be put back stays pending",
        roblox_flags::pending() && flags(&mine).contains_key("FFlagDebugSkyGray") && made.exists(),
        format!("pending {}, mine {}, made {}", roblox_flags::pending(), text(&mine), made.exists()),
    );
    read_only(&mine, false);
    read_only(&made, false);
    roblox_flags::restore();
    c.check(
        "…and goes back as it was at the next try",
        text(&mine) == OWN && !made.exists() && !roblox_flags::pending(),
        format!("{} {}", roblox_flags::pending(), text(&mine)),
    );

    // The note of what to put back can't be kept: nothing is written, so nothing is left that can't be put back.
    let blocker = Path::new(&format!("{}.tmp", state.display())).to_path_buf();
    std::fs::create_dir_all(&blocker).expect("a folder in the way");
    let unsaved = roblox_flags::apply(Some(&exe));
    c.check("no note of the person's own, no flags written", !unsaved && text(&mine) == OWN && !made.exists(), text(&mine));
    let _ = std::fs::remove_dir(&blocker);

    // The note can't be read for a moment: the files aren't touched, and the person's own isn't replaced in it.
    roblox_flags::apply(Some(&exe));
    {
        // FileShare.Write | FileShare.Delete: nobody else may read it while this is open.
        let held = std::fs::OpenOptions::new().write(true).share_mode(0x2 | 0x4).open(&state);
        c.check("a note that can't be read: nothing written", held.is_ok() && !roblox_flags::apply(Some(&exe)), held.is_ok());
    }
    roblox_flags::restore();
    c.check(
        "…and the person's own still goes back after",
        text(&mine) == OWN && !made.exists() && !roblox_flags::pending(),
        text(&mine),
    );

    // A damaged note: set aside, never overwritten, and a file already holding the flags is never taken for the person's own.
    roblox_flags::apply(Some(&exe));
    std::fs::write(&state, "{ damaged").expect("written");
    roblox_flags::apply(Some(&exe));
    let aside: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
        .map(|d| d.flatten().map(|e| e.path()).filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("roblox-flags.json.damaged-"))).collect())
        .unwrap_or_default();
    c.check(
        "a damaged note is set aside as it was",
        aside.len() == 1 && text(&aside[0]) == "{ damaged",
        aside.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", "),
    );
    c.check(
        "…and a flagged file isn't noted as the person's own",
        !state.exists() || !text(&state).contains("FFlagDebugSkyGray"),
        if state.exists() { text(&state) } else { String::new() },
    );
}
