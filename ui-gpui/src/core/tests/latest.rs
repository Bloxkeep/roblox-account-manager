// Tests/Bloxkeep.Tests/LatestSuite.cs: a new Bloxkeep found within minutes (app/latest.rs): the bot's /latest (a little
// server of the test's own here) telling of a newer version brings the hourly update check forward to now, once per
// version; an older or the same version, or one it can't read, changes nothing. Betas: a release is newer than its own
// beta, a beta is never taken from /latest.
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::Checks;
use super::feedback::{read_request, write_response};
use super::rejoin::quiet_app;
use crate::core::app::latest::is_newer_version;
use crate::core::base::Stamp;
use crate::core::models::stamp_min;
use crate::core::{prefs, rt};

pub async fn run(c: &mut Checks, _dir: &str) {
    let newer = |latest: &str, current: &str| is_newer_version(latest, Some(current));
    c.check("1.0.13 is newer than 1.0.12", newer("1.0.13", "1.0.12") && newer("v1.1.0", "1.0.99"), ());
    c.check("…1.0.12 than its own beta", newer("1.0.12", "1.0.12-beta.3"), ());
    c.check(
        "…not the same, an older one, a beta, or something unreadable",
        !newer("1.0.12", "1.0.12") && !newer("1.0.11", "1.0.12") && !newer("1.0.13-beta.1", "1.0.12") && !newer("soon", "1.0.12") && !newer("1.0.13", ""),
        (),
    );

    // The bot's /latest, on a free port of this PC.
    let version = Arc::new(Mutex::new("9.9.9".to_string()));
    let asked = Arc::new(AtomicUsize::new(0));
    let listening = Arc::new(AtomicBool::new(true));
    let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
    let port = listener.local_addr().expect("its address").port();
    {
        let (version, asked, listening) = (version.clone(), asked.clone(), listening.clone());
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                if !listening.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(mut stream) = stream else { continue };
                let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
                if read_request(&mut stream).is_none() {
                    continue;
                }
                asked.fetch_add(1, Ordering::SeqCst);
                let body = format!("{{\"version\":\"{}\"}}", version.lock().unwrap());
                write_response(&mut stream, 200, &body);
            }
        });
    }
    // SAFETY: this suite's process is its own; set before the App reads it.
    unsafe { std::env::set_var("BLOXKEEP_LATEST_URL", format!("http://127.0.0.1:{port}/latest/")) };
    let (app, _said) = quiet_app();
    let later = Stamp::utc_now().plus_secs(50.0 * 60.0);
    let look = async || {
        let before = asked.load(Ordering::SeqCst);
        app.latest.next_latest_look.set(stamp_min());
        app.tick_latest(Stamp::utc_now());
        for _ in 0..100 {
            if asked.load(Ordering::SeqCst) != before && !app.latest.latest_looking.get() {
                break;
            }
            rt::sleep(Duration::from_millis(30)).await;
        }
    };
    let next = || app.updates.next_update_check.get();

    app.updates.next_update_check.set(later);
    look().await;
    c.check(
        "the bot telling of a newer version: the update check is now",
        next() <= Stamp::utc_now() && asked.load(Ordering::SeqCst) == 1,
        format!("{:?} asked {}", next(), asked.load(Ordering::SeqCst)),
    );
    c.check(
        "…and asked again only after a few minutes",
        app.latest.next_latest_look.get() > Stamp::utc_now().plus_secs(4.0 * 60.0),
        (),
    );
    app.updates.next_update_check.set(later);
    look().await;
    c.check(
        "…the same version told again: the hourly check stays where it is (once per version)",
        next() == later && asked.load(Ordering::SeqCst) == 2,
        format!("{:?}", next()),
    );
    *version.lock().unwrap() = "0.0.1".into();
    look().await;
    c.check("an older version: nothing changes", next() == later && asked.load(Ordering::SeqCst) == 3, ());
    *version.lock().unwrap() = "9.9.10".into();
    look().await;
    c.check("another newer one later: the check is now again", next() <= Stamp::utc_now(), ());
    prefs::lock().check_updates = false;
    app.updates.next_update_check.set(later);
    let was = asked.load(Ordering::SeqCst);
    app.latest.next_latest_look.set(stamp_min());
    app.tick_latest(Stamp::utc_now());
    rt::sleep(Duration::from_millis(300)).await;
    c.check("automatic updates off: the bot isn't asked", asked.load(Ordering::SeqCst) == was && next() == later, ());
    listening.store(false, Ordering::SeqCst);
    let _ = std::net::TcpStream::connect(("127.0.0.1", port)); // wakes the listener so it ends
}
