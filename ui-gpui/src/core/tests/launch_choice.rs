// Tests/Bloxkeep.Tests/LaunchChoiceSuite.cs: Roblox VNG is always the last choice. With another Roblox on the PC,
// "Run RobloxPlayerBeta directly" and Default's fallback never pick VNG's client; with only VNG's, they use it; its own
// option ("vng") opens it. Clients here are empty files in a folder of the test's own (roblox_installs::set_local).
use std::time::{Duration, SystemTime};

use super::{Checks, app};
use crate::core::{
    roblox_installs::{self as installs, ClientVersion, REGISTERED_BUILD_OVERRIDE},
    roblox_launcher,
};

fn client(local: &str, install: &str, version: &str) -> String {
    let folder = format!(r"{local}\{install}\Versions\{version}");
    let _ = std::fs::create_dir_all(&folder);
    let exe = format!(r"{folder}\RobloxPlayerBeta.exe");
    let _ = std::fs::write(&exe, []);
    exe
}

fn set_written(path: &str, days_ago: u64) {
    if let Ok(f) = std::fs::File::options().write(true).open(path) {
        let _ = f.set_modified(SystemTime::now() - Duration::from_secs(days_ago * 24 * 3600));
    }
}

fn registered(build: Option<&'static str>) {
    *REGISTERED_BUILD_OVERRIDE.write().unwrap_or_else(|e| e.into_inner()) =
        build.map(|b| Box::new(move || if b.is_empty() { None } else { Some(b.to_string()) }) as Box<dyn Fn() -> Option<String> + Send + Sync>);
}

fn notepad() -> String {
    let windows = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
    format!(r"{windows}\System32\notepad.exe")
}

pub async fn run(c: &mut Checks, dir: &str) {
    let local = format!(r"{}\local", dir.trim_end_matches(['\\', '/']).replace('/', "\\"));
    installs::set_local(Some(local.clone()));
    body(c, &local).await;
    registered(None);
    installs::set_local(None);
}

async fn body(c: &mut Checks, local: &str) {
    // Roblox VNG's client, newer than the global one: still never ahead of it.
    let global = client(local, "Roblox", "version-global");
    set_written(&global, 2);
    let vng = client(local, "RobloxPCVNG", "version-vng");
    let fallback_exe = || roblox_launcher::fallback_launcher().map(|f| f.0);

    c.check("its own option opens Roblox VNG's client", installs::client_for("vng", "").as_deref() == Some(vng.as_str()), installs::client_for("vng", ""));
    c.check(
        "with the global Roblox too, \"directly\" never picks VNG's (even newer)",
        !installs::is_vng_client(&installs::client_for("direct", "").unwrap_or_default()),
        installs::client_for("direct", ""),
    );
    c.check("…nor Default's fallback", !installs::is_vng_client(&fallback_exe().unwrap_or_default()), fallback_exe());

    // The sidebar's "Roblox doesn't seem to be installed": a client on disk is enough, whatever the roblox-player: links say.
    {
        let (app, _said) = app();
        let state = app.build_state();
        c.check("a Roblox client on the PC: not said to be missing", state["robloxInstalled"] == true, ());
    }

    // Roblox keeps a build per channel side by side: the newest build being there is what the version card asks.
    let live = client(local, "Roblox", "version-live");
    c.check("the newest build next to the links' one is found", installs::has_build(Some(&global), Some("version-live")), ());
    c.check(
        "…and one that isn't there isn't",
        !installs::has_build(Some(&global), Some("version-gone")) && !installs::has_build(Some(&global), Some("")) && !installs::has_build(None, Some("version-live")),
        (),
    );
    let _ = std::fs::remove_file(&live);

    // Of two Roblox builds side by side: the one Roblox's installer registered, then the higher version, wins over the newer file.
    let (channel, main) = (client(local, "Roblox", "version-channel"), client(local, "Roblox", "version-main"));
    set_written(&main, 1);
    registered(Some("version-main"));
    c.check(
        "the build Roblox registered is used, though another's file is newer",
        installs::find_official_client().as_deref() == Some(main.as_str()),
        installs::find_official_client(),
    );
    registered(Some(""));
    // A real program's file version (Windows' Notepad, 10.x) against the empty files (0.0.0.0).
    let _ = std::fs::copy(notepad(), &main);
    set_written(&main, 1);
    c.check(
        "…without one, the higher version, though another's file is newer",
        installs::find_official_client().as_deref() == Some(main.as_str()),
        installs::find_official_client(),
    );
    registered(None);
    let _ = std::fs::remove_file(&channel);
    let _ = std::fs::remove_file(&main);

    // Between installs: Roblox's registered build counts only among Roblox's own builds.
    let pick = |clients: &[&str]| installs::newest(clients.iter().map(|s| s.to_string()).collect());
    let (stale, strap) = (client(local, "Roblox", "version-registered"), client(local, "Bloxstrap", "version-newer"));
    let _ = std::fs::copy(notepad(), &strap);
    set_written(&strap, 1);
    registered(Some("version-registered"));
    c.check(
        "Roblox's stale registered build against Bloxstrap's newer one: Bloxstrap's",
        pick(&[&stale, &strap]).as_deref() == Some(strap.as_str()),
        pick(&[&stale, &strap]),
    );
    let named = client(local, "Bloxstrap", "version-registered");
    c.check(
        "…a launcher's build named like the registered one: by its version, as the others",
        pick(&[&strap, &named]).as_deref() == Some(strap.as_str()),
        pick(&[&strap, &named]),
    );
    let own = client(local, "Roblox", "version-other");
    set_written(&stale, 2);
    c.check(
        "…among Roblox's own builds the registered one still comes first",
        pick(&[&stale, &own]).as_deref() == Some(stale.as_str()),
        pick(&[&stale, &own]),
    );
    registered(None);
    let _ = std::fs::remove_dir_all(format!(r"{local}\Bloxstrap"));
    let _ = std::fs::remove_file(&stale);
    let _ = std::fs::remove_file(format!(r"{local}\Roblox\Versions\version-other\RobloxPlayerBeta.exe"));

    // A build whose DLL is gone can't start (the exe names it); one from before that DLL, or with it beside, can.
    let broken = client(local, "Roblox", "version-broken");
    let mut bytes = vec![0u8; (1 << 20) - 5];
    bytes.extend_from_slice(b"RobloxPlayerBeta.dll");
    bytes.push(0);
    let _ = std::fs::write(&broken, bytes);
    c.check("a client missing the DLL it names, the name across two reads: can't start", installs::is_broken(&broken), ());
    c.check(
        "…and isn't picked while another build is there",
        installs::find_official_client().as_deref() == Some(global.as_str()),
        installs::find_official_client(),
    );
    c.check("…and isn't the build said to be installed", !installs::has_build(Some(&global), Some("version-broken")), ());
    c.check("…Roblox's install is said to have one", installs::own_build_broken(), ());
    let broken_dir = installs::dir_name(&broken).unwrap_or_default();
    let _ = std::fs::write(format!(r"{broken_dir}\RobloxPlayerBeta.dll"), []);
    c.check("with the DLL beside it: fine", !installs::is_broken(&broken), ());
    c.check("a client that doesn't name the DLL: fine without it", !installs::is_broken(&global), ());
    let _ = std::fs::remove_dir_all(&broken_dir);

    // A client behind Roblox's newest build isn't played on: Roblox's own updates itself, a launcher's goes through it.
    c.check("\"directly\", up to date: stays directly", installs::updating_option("direct", Some(true)) == "direct", ());
    c.check("\"directly\", newest build not known: stays directly", installs::updating_option("direct", None) == "direct", ());
    c.check("Roblox's own client updates itself when opened", installs::updates_on_open(Some(&global)), ());
    c.check("another option, out of date: left as chosen", installs::updating_option("roblox", Some(false)) == "roblox", ());
    let own_installer = format!(r"{}\{}", installs::dir_name(&installs::client_for("roblox", "").unwrap_or_default()).unwrap_or_default(), installs::OWN_INSTALLER);
    c.check("Roblox's own copy without its installer: nothing updates it", installs::updater_for("roblox", "").is_none(), installs::updater_for("roblox", ""));
    let _ = std::fs::write(&own_installer, []);
    c.check(
        "…with it: its installer does",
        installs::updater_for("roblox", "").as_deref() == Some(own_installer.as_str()) && installs::is_own_installer(Some(&own_installer)),
        installs::updater_for("roblox", ""),
    );
    c.check(
        "…\"directly\" on it, out of date: stays directly (it updates as it opens)",
        installs::updating_option("direct", Some(false)) == "direct",
        installs::updating_option("direct", Some(false)),
    );
    c.check("…and Roblox VNG's client never gets it", installs::updater_for("vng", "").is_none(), ());
    let _ = std::fs::remove_file(&own_installer);
    let strap_exe = format!(r"{local}\Bloxstrap\Bloxstrap.exe");
    let strap_client = client(local, "Bloxstrap", "version-copy");
    let _ = std::fs::write(&strap_exe, []);
    let _ = std::fs::copy(notepad(), &strap_client);
    c.check(
        "\"directly\" on a launcher's old copy: through that launcher",
        installs::updating_option("direct", Some(false)) == "bloxstrap",
        installs::updating_option("direct", Some(false)),
    );
    let _ = std::fs::remove_dir_all(format!(r"{local}\Bloxstrap"));

    // A window's build against Roblox's newest.
    let build = |version: &str, folder: &str| ClientVersion { version: version.into(), folder: folder.into(), ..Default::default() };
    let newest_build = build("0.745.0.100", "version-new");
    c.check("a window on the newest build isn't behind", !installs::is_behind(Some(&build("0.745.0.100", "version-new")), Some(&newest_build)), ());
    c.check("a window on an older build is", installs::is_behind(Some(&build("0.744.0.100", "version-old")), Some(&newest_build)), ());
    c.check("a new build under the same number counts too", installs::is_behind(Some(&build("0.745.0.100", "version-old")), Some(&newest_build)), ());
    c.check("a newer number is another channel's, not behind", !installs::is_behind(Some(&build("0.746.0.100", "version-test")), Some(&newest_build)), ());
    c.check("the Store's copy has no build folder: not compared", !installs::is_behind(Some(&build("2.744.1.0", "")), Some(&newest_build)), ());
    c.check(
        "newest or window not known: not behind",
        !installs::is_behind(None, Some(&newest_build)) && !installs::is_behind(Some(&newest_build), None),
        (),
    );

    // Only Roblox VNG left (a launcher of this PC's own may still stand ahead of it: then that one is used).
    let _ = std::fs::remove_dir_all(format!(r"{local}\Roblox"));
    if installs::find_any_client().is_none() {
        c.check(
            "only Roblox VNG on the PC: \"directly\" uses it, as the last choice",
            installs::client_for("direct", "").as_deref() == Some(vng.as_str()),
            installs::client_for("direct", ""),
        );
        c.check("…and so does Default's fallback", fallback_exe().as_deref() == Some(vng.as_str()), fallback_exe());
    }
}
