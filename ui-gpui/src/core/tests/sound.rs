// "Mute background windows" on a real sound session: a program of the test's own plays silence (nothing is heard, and
// it has no window), so Windows gives it a session as it gives a Roblox window one. It is muted and unmuted through
// roblox_sound, then by the setting's pass as a background window, given its sound back when the setting goes off and
// when Bloxkeep closes, and a mute the person made in the Volume mixer is left alone.
use std::{path::Path, process::Command, rc::Rc, time::Duration};

use serde_json::json;

use super::{Checks, PASSWORD, call, unlock, window::quiet_app as app};
use crate::core::{app::App, base::Stamp, roblox_processes::PlayerProcess, roblox_sound, rt, vault::Vault};

/// One second of silence, 8 kHz 16-bit mono.
fn write_silence(path: &Path) -> std::io::Result<()> {
    let samples = 8000u32;
    let data = samples * 2;
    let mut b = Vec::with_capacity(44 + data as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes()); // PCM
    b.extend_from_slice(&1u16.to_le_bytes()); // mono
    b.extend_from_slice(&8000u32.to_le_bytes());
    b.extend_from_slice(&16000u32.to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data.to_le_bytes());
    b.resize(44 + data as usize, 0);
    std::fs::write(path, b)
}

async fn muted(pid: i32) -> Option<bool> {
    rt::blocking(move || roblox_sound::is_muted(pid)).await
}

/// Waits for the setting's ask to be answered (at most 5 s).
async fn answered(app: &Rc<App>) {
    for _ in 0..50 {
        if app.memory.sound_asked.borrow().is_none() {
            return;
        }
        rt::sleep(Duration::from_millis(100)).await;
    }
}

pub async fn run(c: &mut Checks, dir: &str) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let wav = Path::new(dir).join("silence.wav");
    if let Err(e) = write_silence(&wav) {
        c.check("the silent sound file is written", false, e.to_string());
        return;
    }
    let script = format!("$p = New-Object Media.SoundPlayer '{}'; $p.PlayLooping(); Start-Sleep 90", wav.display());
    let mut child = match Command::new("powershell.exe").args(["-NoProfile", "-NonInteractive", "-Command", &script]).creation_flags(CREATE_NO_WINDOW).spawn() {
        Ok(child) => child,
        Err(e) => {
            c.check("the test's sound program starts", false, e.to_string());
            return;
        }
    };
    let pid = child.id() as i32;

    let mut seen = None;
    for _ in 0..60 {
        seen = muted(pid).await;
        if seen.is_some() {
            break;
        }
        rt::sleep(Duration::from_millis(250)).await;
    }
    if seen.is_none() {
        // No speaker (a build machine): nothing to mute.
        c.skip("muting a real sound session", "this PC has no sound device or session");
        let _ = child.kill();
        return;
    }
    c.check("the test's program has a sound session, not muted", seen == Some(false), seen);

    let found = rt::blocking(move || roblox_sound::set_muted(&[(pid, true)])).await;
    c.check("muted: its session is found", found == vec![pid], &found);
    c.check("…and Windows says it is muted", muted(pid).await == Some(true), muted(pid).await);
    let found = rt::blocking(move || roblox_sound::set_muted(&[(pid, false), (0x7FFF_FFF0, false)])).await;
    c.check("unmuted (a process with no session is simply not found)", found == vec![pid] && muted(pid).await == Some(false), &found);

    // The setting's pass. The window played is another one (this test's own process stands in for it): the test's
    // program is a background window and is muted.
    let seed = Vault::create(PASSWORD, None).expect("vault");
    seed.save().expect("saved");
    drop(seed);
    let (app, _said) = app();
    unlock(&app).await;
    let me = std::process::id() as i32;
    let players = [PlayerProcess { pid, ..Default::default() }, PlayerProcess { pid: me, ..Default::default() }];
    let pass = |app: &Rc<App>| {
        app.memory.sound_tried.set(None);
        app.guard_sound(&players, Stamp::utc_now());
    };
    pass(&app);
    answered(&app).await;
    c.check("setting off: nothing is muted", muted(pid).await == Some(false), muted(pid).await);

    let _ = call(&app, "setSettings", json!({ "muteBackground": true })).await;
    app.memory.sound_played.set(me);
    pass(&app);
    answered(&app).await;
    c.check("setting on: a background window is muted", muted(pid).await == Some(true), muted(pid).await);
    c.check("…and noted as muted by Bloxkeep", app.memory.sound_set.borrow().get(&pid) == Some(&true), app.memory.sound_set.borrow().clone());
    c.check("…the window played is not muted", app.memory.sound_set.borrow().get(&me) != Some(&true), app.memory.sound_set.borrow().clone());

    // It becomes the window played: its sound comes back.
    app.memory.sound_played.set(pid);
    pass(&app);
    answered(&app).await;
    c.check("the window played gets its sound back", muted(pid).await == Some(false), muted(pid).await);

    // Back in the background, then the setting off: its sound comes back.
    app.memory.sound_played.set(me);
    pass(&app);
    answered(&app).await;
    let before = muted(pid).await;
    let _ = call(&app, "setSettings", json!({ "muteBackground": false })).await;
    pass(&app);
    answered(&app).await;
    c.check("setting turned off: the window Bloxkeep muted gets its sound back", before == Some(true) && muted(pid).await == Some(false), (before, muted(pid).await));

    // Bloxkeep closing: none it muted stays muted.
    let _ = call(&app, "setSettings", json!({ "muteBackground": true })).await;
    app.memory.sound_played.set(me);
    pass(&app);
    answered(&app).await;
    let before = muted(pid).await;
    app.give_sound_back();
    c.check("Bloxkeep closing: its sound back", before == Some(true) && muted(pid).await == Some(false), (before, muted(pid).await));

    // A window the person muted in the Volume mixer: the setting going off leaves it muted.
    let _ = call(&app, "setSettings", json!({ "muteBackground": false })).await;
    let _ = rt::blocking(move || roblox_sound::set_muted(&[(pid, true)])).await;
    pass(&app);
    answered(&app).await;
    c.check("a mute the person made is left alone", muted(pid).await == Some(true), muted(pid).await);

    let _ = rt::blocking(move || roblox_sound::set_muted(&[(pid, false)])).await;
    let _ = child.kill();
    let _ = child.wait();
}
