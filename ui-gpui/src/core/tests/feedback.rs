// Tests/Bloxkeep.Tests/FeedbackSuite.cs: Send feedback (app/feedback.rs): what leaves the PC (the preview and the request
// match, the error log without account names, ids, passwords, cookies, Discord links, e-mail addresses or the Windows
// user), and the refusals (no text, the log while locked, twice in a minute, a busy or unreachable service). Sent to a
// little server of the test's own on 127.0.0.1 (`StandInServer`, which online.rs and remote.rs use too), never the real
// one; and the real address must be set before a build goes out.
use std::{
    cell::RefCell,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    rc::Rc,
    sync::{
        atomic::{AtomicBool, AtomicU16, AtomicUsize, Ordering},
        mpsc, Arc, Mutex,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use serde_json::{json, Value};

use super::{call, unlock, user_error, window::quiet_app as app, Checks, PASSWORD};
use crate::core::{
    app::feedback::FEEDBACK_URL,
    base::Res,
    models::{stamp_min, Account},
    rt,
    vault::Vault,
};

/// One request the stand-in server got.
#[derive(Clone, Debug)]
pub(crate) struct Got {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Got {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
    /// The body read as Latin-1 (each byte one character), as the C#'s StreamReader(Encoding.Latin1) did.
    pub fn latin1(&self) -> String {
        self.body.iter().map(|&b| b as char).collect()
    }
}

/// What the server answers a request: its status and body (JSON). Runs on the connection's own thread, so it may wait.
pub(crate) type Answer = Box<dyn Fn(&Got) -> (u16, String) + Send + Sync>;

/// A little HTTP server on a free port of 127.0.0.1 (the C# suites' HttpListener): keeps every request, answers each
/// with `Answer`. Stopped, its port refuses connections.
pub(crate) struct StandInServer {
    /// `http://127.0.0.1:PORT`
    pub base: String,
    pub got: Arc<Mutex<Vec<Got>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl StandInServer {
    pub fn start(answer: Answer) -> StandInServer {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a free port on 127.0.0.1");
        listener.set_nonblocking(true).expect("non-blocking listener");
        let port = listener.local_addr().expect("its port").port();
        let got: Arc<Mutex<Vec<Got>>> = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let answer: Arc<Answer> = Arc::new(answer);
        let (keep, stopped) = (got.clone(), stop.clone());
        let thread = std::thread::spawn(move || {
            while !stopped.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let (keep, answer) = (keep.clone(), answer.clone());
                        std::thread::spawn(move || serve(stream, &keep, &answer));
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(10)),
                }
            }
            // The listener is dropped here: the port refuses connections from now on.
        });
        StandInServer { base: format!("http://127.0.0.1:{port}"), got, stop, thread: Some(thread) }
    }

    pub fn count(&self) -> usize {
        self.got.lock().unwrap().len()
    }

    pub fn all(&self) -> Vec<Got> {
        self.got.lock().unwrap().clone()
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for StandInServer {
    fn drop(&mut self) {
        self.stop();
    }
}

/// The head and body of one HTTP request (Content-Length bodies, as ureq sends them).
pub(crate) fn read_request(stream: &mut TcpStream) -> Option<Got> {
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 8192];
    let head_end = loop {
        if let Some(at) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break at;
        }
        let n = stream.read(&mut chunk).ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
    let mut lines = head.split("\r\n");
    let mut first = lines.next()?.split(' ');
    let method = first.next()?.to_string();
    let path = first.next()?.to_string();
    let headers: Vec<(String, String)> =
        lines.filter_map(|l| l.split_once(':').map(|(n, v)| (n.trim().to_string(), v.trim().to_string()))).collect();
    let length: usize = headers.iter().find(|(n, _)| n.eq_ignore_ascii_case("content-length")).and_then(|(_, v)| v.parse().ok()).unwrap_or(0);
    let mut body = buf[head_end + 4..].to_vec();
    while body.len() < length {
        let n = stream.read(&mut chunk).ok()?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    body.truncate(length);
    Some(Got { method, path, headers, body })
}

pub(crate) fn write_response(stream: &mut TcpStream, status: u16, body: &str) {
    let response = format!(
        "HTTP/1.1 {status} Stand-in\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
    let _ = stream.shutdown(std::net::Shutdown::Both);
}

fn serve(mut stream: TcpStream, keep: &Mutex<Vec<Got>>, answer: &Answer) {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let Some(got) = read_request(&mut stream) else { return };
    keep.lock().unwrap().push(got.clone());
    let (status, body) = answer(&got);
    write_response(&mut stream, status, &body);
}

fn args(text: &str, log: bool, kind: &str) -> Value {
    json!({ "kind": kind, "text": text, "contact": "me#1", "log": log })
}

fn text_of(v: &Res<Value>, name: &str) -> String {
    v.as_ref().ok().and_then(|v| v.get(name)).and_then(Value::as_str).unwrap_or("").to_string()
}

pub async fn run(c: &mut Checks, dir: &str) {
    let account = Account {
        username: "NoobMaster99".into(),
        display_name: "Noobie".into(),
        alias: "Farm one".into(),
        user_id: 4815162342,
        password: "hunter2pass".into(),
        cookie: "_|WARNING:-DO-NOT-SHARE-THIS.--Sharing-this-will-allow-someone-to-log-in-as-you.|_ABC123".into(),
        cookie_valid: Some(true),
        ..Default::default()
    };
    let seed = Vault::create(PASSWORD, None).expect("vault");
    seed.data().accounts.push(account);
    seed.save().expect("saved");
    drop(seed);
    let user = std::env::var("USERNAME").unwrap_or_default();
    let q80 = "Q".repeat(80);
    std::fs::write(
        std::path::Path::new(dir).join("error.log"),
        [
            "[2026-10-16 02:10:44] System.Net.Http.HttpRequestException: 401 for NoobMaster99 (noobmaster99) / Noobie, user 4815162342".to_string(),
            format!("   at RobloxManager.RobloxApi.GetPresenceAsync() in C:\\Users\\{user}\\Bloxkeep\\RobloxApi.cs:line 512"),
            "   cookie _|WARNING:-DO-NOT-SHARE-THIS.--other|_XYZ, password hunter2pass, Farm one, Farm ones".to_string(),
            "   webhook https://discord.com/api/webhooks/123/abcDEF and mail someone@example.com".to_string(),
            format!("   token {q80}"),
        ]
        .join("\r\n"),
    )
    .expect("error.log");

    let (app, _said) = app();

    // Locked: the log can't go (the names to take out aren't known), the rest can.
    let locked = call(&app, "feedbackPreview", args("The mini window forgets its size.", true, "bug")).await;
    c.check("locked: the log is refused", user_error(&locked).is_some(), &locked);
    unlock(&app).await;

    let preview = text_of(&call(&app, "feedbackPreview", args("The mini window forgets its size.", true, "bug")).await, "preview");
    let secrets: Vec<String> = ["NoobMaster99", "noobmaster99", "Noobie", "4815162342", "hunter2pass", "ABC123", "_|WARNING", "abcDEF", "someone@example.com"]
        .iter()
        .map(|s| s.to_string())
        .chain([q80.clone()])
        .collect();
    let mut leaked: Vec<String> = secrets.iter().filter(|s| preview.contains(s.as_str())).cloned().collect();
    if user.encode_utf16().count() >= 3 && preview.to_lowercase().contains(&format!("\\{}\\", user.to_lowercase())) {
        leaked.push("the Windows user".into());
    }
    c.check("preview: nothing that tells who it is", leaked.is_empty(), leaked.join(", "));
    c.check(
        "preview: the marks in their place",
        ["[account]", "[account id]", "[password]", "[cookie]", "[webhook]", "[email]", "[token]"].iter().all(|m| preview.contains(m)),
        &preview,
    );
    c.check("preview: a name inside a longer word is left alone", preview.contains("Farm ones"), &preview);
    c.check(
        "preview: the message, contact and version",
        preview.contains("The mini window forgets its size.") && preview.contains("me#1") && preview.contains("Bloxkeep "),
        (),
    );
    let no_log = text_of(&call(&app, "feedbackPreview", args("An idea for presets.", false, "idea")).await, "preview");
    c.check("no log asked: none in the preview", !no_log.contains("error-log") && !no_log.contains("NoobMaster99"), &no_log);
    // An emoji across the length limits (2026-10-30 review): left out whole, never cut in half (that can't go as JSON).
    let cut = app
        .read_feedback(&json!({ "kind": "idea", "text": format!("{}\u{1F600}", "a".repeat(1999)), "contact": format!("{}\u{1F600}", "b".repeat(99)), "log": false }))
        .expect("read");
    let (cut_text, cut_contact) = (cut.text.encode_utf16().count(), cut.contact.encode_utf16().count());
    c.check("an emoji at the text's and the contact's limit is left out whole", cut_text == 1999 && cut_contact == 99, format!("{cut_text} {cut_contact}"));

    // The service: a little server here that answers what each step needs.
    let answer = Arc::new(AtomicU16::new(200));
    // Set: the next request is answered only when it is let go (the ones after it at once).
    let gate: Arc<Mutex<Option<mpsc::Receiver<()>>>> = Arc::new(Mutex::new(None));
    // Set (not 0): a body over it is refused as too large, as the service before the longer log.
    let old_service_max = Arc::new(AtomicUsize::new(0));
    let (status, held, old_max) = (answer.clone(), gate.clone(), old_service_max.clone());
    let mut server = StandInServer::start(Box::new(move |got| {
        let slow = held.lock().unwrap().take();
        let most = old_max.load(Ordering::SeqCst);
        let code = if most > 0 && got.body.len() > most { 413 } else { status.load(Ordering::SeqCst) };
        if let Some(slow) = slow {
            let _ = slow.recv_timeout(Duration::from_secs(30));
        }
        (code, String::new())
    }));
    // SAFETY: this suite's own process; read by send_feedback each time.
    unsafe { std::env::set_var("BLOXKEEP_FEEDBACK_URL", format!("{}/feedback/", server.base)) };
    let received = |s: &StandInServer| -> Vec<Value> { s.all().iter().map(|g| serde_json::from_slice(&g.body).unwrap_or(Value::Null)).collect() };

    let too_short = call(&app, "sendFeedback", args("ok", true, "bug")).await;
    c.check("too short: refused before sending", user_error(&too_short).is_some() && server.count() == 0, &too_short);
    answer.store(429, Ordering::SeqCst);
    let busy = call(&app, "sendFeedback", args("The mini window forgets its size.", true, "bug")).await;
    c.check("a busy service: said so", user_error(&busy).is_some_and(|m| m.contains("minute")), &busy);
    answer.store(200, Ordering::SeqCst);
    let sent = call(&app, "sendFeedback", args("The mini window forgets its size.", true, "bug")).await;
    c.check("sent", user_error(&sent).is_none() && server.count() == 2, &sent);
    let got = received(&server);
    if got.len() == 2 {
        let body = &got[1];
        let log = body["log"].as_str().unwrap_or("").to_string();
        c.check(
            "sent: kind, text, contact, version, Windows, language",
            body["kind"] == "bug"
                && body["text"] == "The mini window forgets its size."
                && body["contact"] == "me#1"
                && body["app"].as_str().is_some_and(|a| !a.is_empty())
                && body["windows"].as_str().is_some_and(|w| w.starts_with("Windows"))
                && matches!(body["language"].as_str(), Some("en" | "vi")),
            body,
        );
        let first_line = log.trim().split('\n').next().unwrap_or("").trim().to_string();
        c.check(
            "sent: the same log as the preview",
            !log.is_empty() && preview.contains(&first_line) && !secrets.iter().any(|s| log.contains(s.as_str())),
            (),
        );
    }
    let again = call(&app, "sendFeedback", args("And another thing.", true, "bug")).await;
    c.check("twice in a minute: refused", user_error(&again).is_some() && server.count() == 2, &again);

    // Two sends pressed at once (a double click, the page and a retry): the first is still on its way when the second
    // asks, and the minute's gap counted only a send that had finished. One request goes, not two.
    app.feedback.feedback_sent_at.set(stamp_min());
    let (release, wait_for) = mpsc::channel::<()>();
    *gate.lock().unwrap() = Some(wait_for);
    let sent_before = server.count();
    let first: Rc<RefCell<Option<Res<Value>>>> = Rc::new(RefCell::new(None));
    {
        let (app, first) = (app.clone(), first.clone());
        rt::spawn(async move {
            let r = call(&app, "sendFeedback", args("Two at once: the first.", true, "bug")).await;
            *first.borrow_mut() = Some(r);
        });
    }
    let begun = Instant::now();
    while server.count() == sent_before && begun.elapsed() < Duration::from_secs(5) {
        rt::sleep(Duration::from_millis(20)).await;
    }
    let second = call(&app, "sendFeedback", args("Two at once: the second.", true, "bug")).await;
    let _ = release.send(());
    let begun = Instant::now();
    while first.borrow().is_none() && begun.elapsed() < Duration::from_secs(30) {
        rt::sleep(Duration::from_millis(20)).await;
    }
    let first_done = first.borrow_mut().take();
    c.check("two sends at once: the second is refused while the first is on its way", user_error(&second).is_some(), &second);
    c.check(
        "…and the first one goes through",
        first_done.as_ref().is_some_and(|r| user_error(r).is_none() && r.is_ok()),
        &first_done,
    );
    c.check("…only one request reached the service", server.count() == sent_before + 1, server.count() - sent_before);
    // A send that failed doesn't hold the next one back.
    app.feedback.feedback_sent_at.set(stamp_min());
    answer.store(500, Ordering::SeqCst);
    let failed = call(&app, "sendFeedback", args("The service is down.", true, "bug")).await;
    answer.store(200, Ordering::SeqCst);
    let retried = call(&app, "sendFeedback", args("The service is back.", true, "bug")).await;
    c.check("a failed send can be tried again at once", user_error(&failed).is_some() && user_error(&retried).is_none(), &retried);

    // A long log goes about whole (a report of 2026-11-11 had only 12 KB of it, starting at a rejoin's "try 4"): its
    // first lines too. A service from before (refusing over 40 000) still gets the report, with the log's last 12 000.
    {
        use std::io::Write;
        let mut log = std::fs::OpenOptions::new().append(true).open(crate::core::vault::data_dir().join("error.log")).expect("error.log");
        let lines: String = (1..=1500).map(|i| format!("[2026-11-11 10:00:00] Long log line {i:04} of a rejoin's tries.\n")).collect();
        log.write_all(lines.as_bytes()).expect("written");
    }
    app.feedback.feedback_sent_at.set(stamp_min());
    let before_long = server.count();
    let long_sent = call(&app, "sendFeedback", args("A long log.", true, "bug")).await;
    let got = received(&server);
    let long_log = if got.len() > before_long { got.last().and_then(|g| g["log"].as_str()).unwrap_or("").to_string() } else { String::new() };
    c.check(
        "a long log goes about whole: its early lines too, far past 12 000 characters",
        user_error(&long_sent).is_none()
            && long_log.encode_utf16().count() > 60_000
            && long_log.contains("Long log line 0001 ")
            && long_log.trim_end().ends_with("Long log line 1500 of a rejoin's tries."),
        format!("{long_sent:?} {}", long_log.len()),
    );
    app.feedback.feedback_sent_at.set(stamp_min());
    old_service_max.store(40_000, Ordering::SeqCst);
    let before_old = server.count();
    let old_sent = call(&app, "sendFeedback", args("To the old service.", true, "bug")).await;
    old_service_max.store(0, Ordering::SeqCst);
    let got = received(&server);
    let cut_log = if got.len() == before_old + 2 { got.last().and_then(|g| g["log"].as_str()).unwrap_or("").to_string() } else { String::new() };
    let cut_units = cut_log.encode_utf16().count();
    c.check(
        "the service from before refuses it as too large: sent again with the log's last 12 000 characters, whole lines",
        user_error(&old_sent).is_none()
            && cut_units > 0
            && cut_units <= 12_000
            && cut_log.starts_with("[2026-11-11")
            && cut_log.trim_end().ends_with("Long log line 1500 of a rejoin's tries."),
        format!("{old_sent:?} {} {cut_units}", got.len() - before_old),
    );

    server.stop();
    app.feedback.feedback_sent_at.set(stamp_min());
    let down = call(&app, "sendFeedback", args("The mini window forgets its size.", true, "bug")).await;
    c.check("service unreachable: said so", user_error(&down).is_some(), &down);
    // SAFETY: as above.
    unsafe { std::env::remove_var("BLOXKEEP_FEEDBACK_URL") };

    // The real address: a build mustn't go out before it's set (services/Feedback/worker's workers.dev address).
    let real = url::Url::parse(FEEDBACK_URL);
    c.check(
        "the feedback service's address is set",
        real.as_ref().is_ok_and(|u| {
            let host = u.host_str().unwrap_or("");
            u.scheme() == "https" && host.ends_with(".workers.dev") && !host.to_lowercase().contains("subdomain") && u.path() == "/feedback"
        }),
        FEEDBACK_URL,
    );
}
