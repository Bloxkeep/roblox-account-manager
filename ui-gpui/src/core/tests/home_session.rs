// Tests/Bloxkeep.Tests/HomeSessionSuite.cs: real Windows file sharing/junctions and process metadata, using only
// disposable files and stand-ins in the test's own folder. The C#'s stand-in process (ping started through a junction)
// is skipped: nothing is started here.
use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    fs::OpenOptions,
    os::windows::fs::OpenOptionsExt,
    path::Path,
    rc::Rc,
};

use super::{Checks, rejoin::quiet_app};
use crate::core::{
    app::tray::home_aliases,
    base::Stamp,
    models::Account,
    prefs,
    roblox_api::eq_ci,
    roblox_home_session::RobloxHomeSession,
    roblox_processes::{self, PlayerProcess},
};

const FILE_SHARE_READ: u32 = 1;
const FILE_SHARE_WRITE: u32 = 2;

fn locked(path: &str) -> bool {
    match OpenOptions::new().read(true).share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE).open(path) {
        Ok(_) => false,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        Err(_) => true,
    }
}

fn read(path: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

fn join(a: &str, b: &str) -> String {
    Path::new(a).join(b).to_string_lossy().into_owned()
}

fn player(pid: i32, created: i64) -> PlayerProcess {
    PlayerProcess { pid, created, ..Default::default() }
}

fn new_alias_name() -> String {
    format!("client-{}", uuid::Uuid::new_v4().simple())
}

pub async fn run(c: &mut Checks, dir: &str) {
    let dir = dir.trim_end_matches(['\\', '/']).replace('/', "\\");
    let install = join(&dir, "install");
    let _ = std::fs::create_dir_all(&install);
    let (exe, keep) = (join(&install, "RobloxPlayerBeta.exe"), join(&install, "do-not-delete.dat"));
    let _ = std::fs::write(&exe, "stub");
    let _ = std::fs::write(&keep, "the real Roblox install");
    let (storage, aliases) = (join(&dir, "LocalStorage"), join(&dir, "aliases"));
    let _ = std::fs::create_dir_all(&storage);
    let (cookies, app_storage) = (join(&storage, "RobloxCookies.dat"), join(&storage, "appStorage.json"));
    let _ = std::fs::write(&cookies, "existing cookie stand-in");
    let _ = std::fs::write(&app_storage, "existing settings stand-in");
    match RobloxHomeSession::new(&storage, &format!("{aliases}\\")) {
        Ok(mut session) => {
            c.check("both existing shared files are locked", locked(&cookies) && locked(&app_storage), ());
            let info = session.create_start_info(&exe);
            let info2 = session.create_start_info(&exe);
            match (&info, &info2) {
                (Ok((cmd, alias)), Ok((cmd2, _))) => {
                    let file = cmd.get_program().to_string_lossy().into_owned();
                    let file2 = cmd2.get_program().to_string_lossy().into_owned();
                    let working = cmd.get_current_dir().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default();
                    c.check(
                        "each start gets a separate junction with the unmodified client reachable",
                        working != install && file != file2 && read(&file) == "stub" && Path::new(&file2).exists(),
                        (),
                    );
                    let parent = Path::new(&file).parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
                    c.check(
                        "launch carries no ticket, game link or shell launch",
                        cmd.get_args().count() == 0 && cmd.get_envs().count() == 0 && working == parent && working == *alias,
                        (),
                    );
                    check_clients(c, &mut session, &file, &aliases, &exe);
                    check_native_paths(c);
                }
                _ => {
                    c.check("each start gets a separate junction with the unmodified client reachable", false, format!("{:?}", info.as_ref().err()));
                    c.check("launch carries no ticket, game link or shell launch", false, ());
                }
            }
            session.dispose();
        }
        Err(e) => c.check("both existing shared files are locked", false, e.to_string()),
    }
    c.check(
        "existing files are unlocked, retained, and unchanged",
        !locked(&cookies) && !locked(&app_storage) && read(&cookies) == "existing cookie stand-in" && read(&app_storage) == "existing settings stand-in",
        (),
    );
    RobloxHomeSession::cleanup_aliases(&aliases);
    let left = std::fs::read_dir(&aliases).map(|d| d.flatten().filter(|e| e.path().is_dir()).count()).unwrap_or(0);
    c.check(
        "cleanup removes junctions without touching the install",
        left == 0 && Path::new(&exe).exists() && read(&keep) == "the real Roblox install",
        left,
    );
    let decoy = join(&aliases, &new_alias_name());
    let _ = std::fs::create_dir_all(&decoy);
    let _ = std::fs::write(join(&decoy, "real.txt"), "not a junction");
    RobloxHomeSession::cleanup_aliases(&aliases);
    c.check("cleanup leaves real directories alone", Path::new(&join(&decoy, "real.txt")).exists(), ());
    check_missing_and_busy_files(c, &dir, &aliases);
    check_controller(c, &dir, &storage, &aliases).await;
}

fn check_clients(c: &mut Checks, session: &mut RobloxHomeSession, alias_client: &str, aliases: &str, normal_client: &str) {
    let alias_dir = Path::new(alias_client).parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
    c.check(
        "only exact alias paths belong: normal, sibling, nested and malformed names do not",
        session.owns_client_path(alias_client)
            && !session.owns_client_path(normal_client)
            && !session.owns_client_path(&join(&join(&format!("{aliases}-sibling"), &new_alias_name()), "RobloxPlayerBeta.exe"))
            && !session.owns_client_path(&join(&join(&alias_dir, "nested"), "RobloxPlayerBeta.exe"))
            && !session.owns_client_path(&join(&join(aliases, "client-invalid"), "RobloxPlayerBeta.exe")),
        (),
    );
    let home = player(101, 1001);
    let normal = player(202, 2002);
    let queries = Cell::new(0);
    let (alias_client, normal_client) = (alias_client.to_string(), normal_client.to_string());
    let found = session.find_clients(
        &[home, normal],
        Some(&|pid, _| {
            queries.set(queries.get() + 1);
            Some(if pid == home.pid { alias_client.clone() } else { normal_client.clone() })
        }),
    );
    c.check("mixed clients: only the alias client keeps the session alive", found.0 == [101] && !found.1, format!("{found:?}"));
    // A query asked again would panic in the C#'s stand-in (throw); here it is counted.
    let again = Cell::new(false);
    let found = session.find_clients(
        &[home, normal],
        Some(&|_, _| {
            again.set(true);
            None
        }),
    );
    c.check("process identity caches paths without repeated native queries", found.0 == [101] && queries.get() == 2 && !again.get(), format!("{found:?}"));
    let found = session.find_clients(
        &[normal],
        Some(&|_, _| {
            again.set(true);
            None
        }),
    );
    c.check("last alias closes: a regular/ticket client alone no longer holds locks", found.0.is_empty() && !found.1 && !again.get(), format!("{found:?}"));
    let found = session.find_clients(&[player(101, 3003), normal], Some(&|_, _| Some(normal_client.clone())));
    c.check("a reused PID does not inherit alias ownership", found.0.is_empty() && !found.1, format!("{found:?}"));
    let unknown = player(303, 4004);
    let found = session.find_clients(&[unknown, normal], Some(&|_, _| None));
    c.check("an unqueryable new client conservatively holds the locks", found.1, format!("{found:?}"));
    let found = session.find_clients(&[unknown, normal], Some(&|_, _| Some(normal_client.clone())));
    c.check("failed queries are retried and release once confirmed regular", !found.1 && found.0.is_empty(), format!("{found:?}"));
    let no_time = player(101, 0);
    session.find_clients(&[no_time], Some(&|_, _| Some(alias_client.clone())));
    let found = session.find_clients(&[no_time], Some(&|_, _| Some(normal_client.clone())));
    c.check("unknown creation times are never cached across PID reuse", found.0.is_empty() && !found.1, format!("{found:?}"));
}

/// This process's creation time as a FILETIME (Process.StartTime.ToFileTimeUtc()).
fn own_created() -> i64 {
    use windows_sys::Win32::{
        Foundation::FILETIME,
        System::Threading::{GetCurrentProcess, GetProcessTimes},
    };
    let zero = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
    let (mut created, mut exit, mut kernel, mut user) = (zero, zero, zero, zero);
    // SAFETY: the pseudo handle of this process and four out values on the stack.
    if unsafe { GetProcessTimes(GetCurrentProcess(), &mut created, &mut exit, &mut kernel, &mut user) } != 0 {
        ((created.dwHighDateTime as i64) << 32) | created.dwLowDateTime as i64
    } else {
        0
    }
}

fn check_native_paths(c: &mut Checks) {
    let me = std::process::id() as i32;
    let created = own_created();
    let exe = std::env::current_exe().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
    let path = roblox_processes::launch_path(me, created);
    c.check(
        "native launch-path query reads the expected process identity",
        path.as_deref().is_some_and(|p| eq_ci(p, &exe)),
        format!("{path:?} {exe}"),
    );
    c.check(
        "native launch-path query rejects reused and missing identities",
        roblox_processes::launch_path(me, created + 1).is_none() && roblox_processes::launch_path(i32::MAX - 7, 1).is_none(),
        (),
    );
    c.skip("Windows launch-path query preserves the junction launch path", "starts a program (ping through the junction)");
}

fn check_missing_and_busy_files(c: &mut Checks, dir: &str, aliases: &str) {
    let storage = join(dir, "missing-storage");
    let (cookie, settings) = (join(&storage, "RobloxCookies.dat"), join(&storage, "appStorage.json"));
    {
        let session = RobloxHomeSession::new(&storage, aliases);
        c.check("a first run locks its own empty placeholders", session.is_ok() && locked(&cookie) && locked(&settings), ());
    }
    c.check("first-run placeholders are deleted when handles close", !Path::new(&cookie).exists() && !Path::new(&settings).exists(), ());
    let _ = std::fs::write(&cookie, "");
    drop(RobloxHomeSession::new(&storage, aliases));
    c.check(
        "a pre-existing empty file is retained; only the new one disappears",
        Path::new(&cookie).exists() && std::fs::metadata(&cookie).map(|m| m.len()).unwrap_or(1) == 0 && !Path::new(&settings).exists(),
        (),
    );
    let _ = std::fs::write(&settings, "busy original");
    {
        let busy = OpenOptions::new().read(true).share_mode(0).open(&settings);
        let failed_io = |r: &crate::core::base::Res<RobloxHomeSession>| r.as_ref().is_err_and(|e| crate::core::roblox_api::is_fail_kind(e, "IOException"));
        let attempt = RobloxHomeSession::new(&storage, aliases);
        c.check("a busy second file rolls back the first lock", busy.is_ok() && failed_io(&attempt) && !locked(&cookie), format!("{:?}", attempt.as_ref().err()));
        drop(attempt);
        let _ = std::fs::remove_file(&cookie);
        let attempt = RobloxHomeSession::new(&storage, aliases);
        c.check("partial acquisition removes its own placeholder", failed_io(&attempt) && !Path::new(&cookie).exists(), format!("{:?}", attempt.as_ref().err()));
        drop(attempt);
        drop(busy);
    }
    c.check("failed acquisition leaves the existing busy file unchanged", read(&settings) == "busy original", ());
}

async fn check_controller(c: &mut Checks, dir: &str, storage: &str, aliases: &str) {
    let (app, _said) = quiet_app();
    let root = home_aliases();
    c.check(
        "controller stores aliases in its data directory's cache, outside the old TEMP root",
        root == Path::new(dir).join("cache").join("roblox-client-paths"),
        root.to_string_lossy().into_owned(),
    );
    let messages: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let ready = || {
        let m = messages.clone();
        app.home_launch_ready(&move |t| m.borrow_mut().push(t.to_string()))
    };
    let was = prefs::lock().multi_roblox_at_start;
    prefs::lock().multi_roblox_at_start = false;
    let r = ready();
    c.check(
        "Multi-Roblox turned off during retry reports why opening stopped",
        !r && messages.borrow().len() == 1 && messages.borrow()[0].contains("Turn on Multi-Roblox"),
        format!("{:?}", messages.borrow()),
    );
    messages.borrow_mut().clear();
    prefs::lock().multi_roblox_at_start = true;
    let r = ready();
    c.check(
        "Multi-Roblox no longer armed during retry reports why",
        !r && messages.borrow().len() == 1 && !messages.borrow()[0].is_empty(),
        format!("{:?}", messages.borrow()),
    );
    prefs::lock().multi_roblox_at_start = false;
    match RobloxHomeSession::new(storage, aliases) {
        Ok(s) => *app.tray.home_session.borrow_mut() = Some(s),
        Err(e) => println!("      (home session not made: {e})"),
    }
    app.tray.home_started_at.set(Stamp::utc_now().plus_secs(-60.0));
    let normal = vec![player(std::process::id() as i32, own_created())];
    let release = |scan: bool| app.release_home_session_if_idle(scan, &normal, &HashSet::new(), Stamp::utc_now());
    let storage_cookie = join(storage, "RobloxCookies.dat");
    release(false);
    c.check("a skipped process scan never unlocks session files", locked(&storage_cookie), ());
    app.tray.opening_home.set(true);
    let blocked = app.check_can_launch(&Vec::<Account>::new()).err().is_some_and(|e| e.to_string().contains("Already launching"));
    c.check("an account launch cannot race a separate-window launch", blocked, ());
    release(true);
    c.check("a launch in progress keeps its locks", locked(&storage_cookie), ());
    app.tray.opening_home.set(false);
    release(true);
    c.check("controller releases when only a regular client remains", !locked(&storage_cookie) && !app.home_sessions_active(), ());
    prefs::lock().multi_roblox_at_start = was;
}
