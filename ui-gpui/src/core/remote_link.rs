//! RemoteLink.cs: the connection to Bloxkeep's Discord bot that Settings › Discord › Control from Discord keeps open
//! (services/DiscordBot/worker/remote.js): a WebSocket the bot hands this PC's commands down, kept open while it's on,
//! connected again after a drop (2 s, then longer, up to a minute). The PC proves itself with its secret, which only the
//! bot's hash of it is kept against. What goes up: the PC's name and its accounts' and presets' names (for Discord's
//! suggestions while typing), answers to commands, a code when asked for one. Everything else stays here. The pictures
//! of /pc screenshot don't go to the bot: they are sent to Discord itself, to the link the bot handed with the command.
//!
//! PORT: the C# runs its callbacks on the thread that made it (the window's, through its SynchronizationContext); here
//! they run on the link's own threads, and the caller hops to its window as it needs to.
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard, mpsc};
use std::time::{Duration, Instant};

use regex::Regex;
use serde_json::{Map, Value, json};
use tungstenite::client::IntoClientRequest;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

use crate::core::base::{Cancel, Fail, Res, Stamp};
use crate::core::discord_webhook::Multipart;
use crate::core::log;
use crate::core::models::to_net_json_string;

/// A command from Discord (/pc): its id (to answer it), its name ("status", "preset", "close", "switch", "screenshot")
/// and value; for a screenshot, the link its pictures are sent to (Discord's own, for that one answer).
#[derive(Clone, PartialEq, Eq)]
pub struct RemoteCommand {
    pub id: String,
    pub name: String,
    pub value: Option<String>,
    pub reply: Option<String>,
}

/// `reply` left out: a Discord link that posts as the bot's answer.
impl std::fmt::Debug for RemoteCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteCommand")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("value", &self.value)
            .field("reply", &self.reply.as_ref().map(|_| crate::core::base::Redacted))
            .finish()
    }
}

pub const URL: &str = "wss://bloxkeep-bot.bloxkeep-app.workers.dev/remote/connect";
const KEEP_ALIVE: Duration = Duration::from_secs(30);
const FIRST_RETRY: Duration = Duration::from_secs(2);
const LAST_RETRY: Duration = Duration::from_secs(60);
/// A connection up this long starts the waits over. One the bot closes at once (a hello it won't take, another
/// Bloxkeep with the same secret pushing this one off) keeps them growing, rather than connecting every 2-3 s
/// forever (2026-10-30 review).
const STAYED_UP: Duration = Duration::from_secs(60);
const MAX_MESSAGE: usize = 64 * 1024;
/// The bot closes a socket whose message is over 16,000 characters (remote.js MAX_MESSAGE), and keeps 200 names of 60
/// characters at most: the names sent fit well under that, the presets first (2026-10-30 review: a PC with many
/// accounts was refused at every connection).
const MAX_NAMES: usize = 200;
const MAX_NAME: usize = 60;
const NAMES_BUDGET: usize = 13_000;
const PRESETS_BUDGET: usize = 4_000;
/// How often the socket's thread looks up from a quiet socket (to send, ping or stop).
const POLL: Duration = Duration::from_millis(200);
// PORT: ClientWebSocket.ConnectAsync has no time limit of its own (the stop token only); a TCP connect here waits at
// most this long.
const CONNECT_LIMIT: Duration = Duration::from_secs(30);

/// Runs a command and gives the answer: content and/or embeds; None when answered already (`shot`).
pub type CommandFn = Box<dyn Fn(RemoteCommand) -> Option<Map<String, Value>> + Send + Sync>;

/// Someone typed this PC's code into /pc link: the Discord account (its unique @username), the one it would replace,
/// and its id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asking {
    pub user: String,
    pub replacing: Option<String>,
    pub id: Option<String>,
}

#[derive(Default)]
struct LinkState {
    connected: bool,
    linked: Option<bool>,
    user: Option<String>,
    code: Option<String>,
    code_until: Option<Stamp>,
    asking: Option<Asking>,
    busy: bool,
    hello: Option<String>,
    want_code: bool,
}

struct Inner {
    url: String,
    secret: String,
    command: CommandFn,
    changed: Box<dyn Fn() + Send + Sync>,
    stop: Cancel,
    state: Mutex<LinkState>,
    /// What goes up, while the socket is open (C#'s `_ws` in Open state); the socket's thread sends it.
    out: Mutex<Option<mpsc::Sender<String>>>,
}

fn guard<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

pub struct RemoteLink {
    inner: Arc<Inner>,
}

impl RemoteLink {
    /// `command`: runs a command and gives the answer (see CommandFn). `changed`: connected, linked or a code came: the
    /// page shows it.
    pub fn new(secret: &str, command: CommandFn, changed: Box<dyn Fn() + Send + Sync>, url: Option<&str>) -> RemoteLink {
        let inner = Arc::new(Inner {
            url: url.unwrap_or(URL).to_string(),
            secret: secret.to_string(),
            command,
            changed,
            stop: Cancel::new(),
            state: Mutex::new(LinkState::default()),
            out: Mutex::new(None),
        });
        let run = inner.clone();
        crate::net::Named::new("remote-link".into()).spawn(move || run_loop(&run)).expect("a thread for the remote link");
        RemoteLink { inner }
    }

    /// Connected to the bot now.
    pub fn connected(&self) -> bool {
        guard(&self.inner.state).connected
    }
    /// Linked to a Discord account (None until the bot said).
    pub fn linked(&self) -> Option<bool> {
        guard(&self.inner.state).linked
    }
    /// The name Discord shows that account by, when the bot knows it.
    pub fn user(&self) -> Option<String> {
        guard(&self.inner.state).user.clone()
    }
    /// The code to type in Discord (/pc link), and until when it works.
    pub fn code(&self) -> Option<String> {
        guard(&self.inner.state).code.clone()
    }
    pub fn code_until(&self) -> Option<Stamp> {
        guard(&self.inner.state).code_until
    }
    /// Someone typed this PC's code into /pc link: the Discord account (its unique @username, which nobody else can
    /// take, unlike the name it shows), the one it would replace, and its id (the answer names it, so an Allow never
    /// links someone who asked after the question was put). Nothing is linked until it is answered (`answer`).
    pub fn asking(&self) -> Option<Asking> {
        guard(&self.inner.state).asking.clone()
    }
    /// The bot had no room for a new code just now.
    pub fn busy(&self) -> bool {
        guard(&self.inner.state).busy
    }

    /// What the bot shows while someone types (the PC's, accounts' and presets' names): sent when it changes.
    pub fn hello(&self, name: &str, accounts: &[String], presets: &[String], language: &str) {
        let (preset_names, used) = fit(presets, PRESETS_BUDGET);
        let (account_names, _) = fit(accounts, NAMES_BUDGET.saturating_sub(used));
        let mut m = Map::new();
        m.insert("t".into(), json!("hello"));
        m.insert("name".into(), json!(cut(name)));
        m.insert("lang".into(), json!(language));
        // It asks before a link is made (the bot links an app that doesn't say so at once, as 1.0.12 did).
        m.insert("ask".into(), json!(true));
        // It answers /pc screenshot (the bot tells whoever asks an older one to update it).
        m.insert("shots".into(), json!(true));
        m.insert("accounts".into(), json!(account_names));
        m.insert("presets".into(), json!(preset_names));
        let hello = to_net_json_string(&Value::Object(m)).unwrap_or_default();
        let connected = {
            let mut s = guard(&self.inner.state);
            if s.hello.as_deref() == Some(hello.as_str()) {
                return;
            }
            s.hello = Some(hello.clone());
            s.connected
        };
        if connected {
            send(&self.inner, hello);
        }
    }

    /// A new code to link this PC (shown once the bot sends it; asked again on the next connection if offline now).
    pub fn ask_code(&self) {
        let connected = {
            let mut s = guard(&self.inner.state);
            s.code = None;
            s.code_until = None;
            s.want_code = true;
            s.connected
        };
        if connected {
            send(&self.inner, r#"{"t":"code"}"#.into());
        }
    }

    /// The answer to `asking`: linked, or nothing changes. `id`: the asker the page showed; someone else asking now
    /// (the question changed under the click) isn't answered by it: their own question shows. The bot is told whose
    /// question it answers, and links nobody else.
    pub fn answer(&self, allow: bool, id: Option<&str>) {
        let asking = {
            let mut s = guard(&self.inner.state);
            let Some(asking) = s.asking.clone() else { return };
            if let (Some(id), Some(asker)) = (id, asking.id.as_deref()) {
                if id != asker {
                    return;
                }
            }
            s.asking = None;
            asking
        };
        let mut answer = Map::new();
        answer.insert("t".into(), json!(if allow { "allow" } else { "deny" }));
        if let Some(user) = asking.id {
            answer.insert("user".into(), json!(user));
        }
        send(&self.inner, to_net_json_string(&Value::Object(answer)).unwrap_or_default());
    }

    /// Not linked any more (the bot forgets which Discord account it was). Offline, it never gets there: the caller
    /// drops the secret too, so the bot's link is to a PC that never connects again.
    pub fn unlink(&self) {
        if self.connected() {
            send(&self.inner, r#"{"t":"unlink"}"#.into());
        }
    }

    /// How a command went once done (a preset's windows open): a follow-up in Discord, within its 15 minutes.
    pub fn later(&self, id: &str, mut answer: Map<String, Value>) {
        answer.insert("t".into(), json!("later"));
        answer.insert("id".into(), json!(id));
        if self.connected() {
            send(&self.inner, to_net_json_string(&Value::Object(answer)).unwrap_or_default());
        }
    }

    /// A link pictures may be sent to: the answer link of a command to Bloxkeep's own bot (discord.com/api/…/webhooks/
    /// <its application>/<that command's token>/messages/@original), never anywhere else the bot (or someone posing as
    /// it) might name, not even another webhook on Discord (2026-10-30 review). The checks' stand-in bot names its own.
    fn may_send_to(&self, reply: &str) -> bool {
        if self.inner.url != URL {
            url::Url::parse(reply).is_ok_and(|u| u.scheme() == "http" || u.scheme() == "https")
        } else {
            is_bot_reply(reply)
        }
    }

    /// The answer to /pc screenshot (`command`): its pictures (JPEG) and a line about them, put in place of Discord's
    /// "thinking…" through the link the bot handed with it (no bot token: that link is good for that answer only), then
    /// the bot is told it's done. They never pass through the bot. False when they didn't get there.
    pub fn shot(&self, command: &RemoteCommand, content: &str, pictures: &[Vec<u8>]) -> bool {
        let Some(reply) = command.reply.as_deref().filter(|r| self.may_send_to(r)) else { return false };
        let form = || {
            let attachments: Vec<Value> = (0..pictures.len()).map(|i| json!({ "id": i, "filename": format!("screenshot-{}.jpg", i + 1) })).collect();
            let mut payload = Map::new();
            payload.insert("content".into(), json!(content));
            payload.insert("allowed_mentions".into(), json!({ "parse": [] }));
            payload.insert("attachments".into(), Value::Array(attachments));
            let mut form = Multipart::new();
            form.part("payload_json", None, "application/json; charset=utf-8", to_net_json_string(&Value::Object(payload)).unwrap_or_default().as_bytes());
            for (i, picture) in pictures.iter().enumerate() {
                let name = format!("screenshot-{}.jpg", i + 1);
                form.part(&format!("files[{i}]"), Some(&name), "image/jpeg", picture);
            }
            form.finish()
        };
        let mut attempt = 0;
        loop {
            if self.inner.stop.is_cancelled() {
                return false;
            }
            let (content_type, body) = form();
            let status = match HTTP.request("PATCH", reply).set("Content-Type", &content_type).send_bytes(&body) {
                Ok(r) => r.status(),
                Err(ureq::Error::Status(code, _)) => code,
                Err(e) => {
                    log::note(&format!("Remote control: the answer to Discord didn't go ({:?}).", e.kind()));
                    return false;
                }
            };
            // Quicker than Discord: its "thinking…" isn't there to edit yet (once); asked too often: waited out (once).
            if attempt == 0 && (status == 404 || status == 429) {
                if !self.inner.stop.sleep(Duration::from_secs(2)) {
                    return false;
                }
                attempt += 1;
                continue;
            }
            if !(200..300).contains(&status) {
                log::note(&format!("Remote control: Discord refused the answer (HTTP {status})."));
                return false;
            }
            break;
        }
        send(&self.inner, to_net_json_string(&json!({ "t": "shown", "id": command.id })).unwrap_or_default());
        true
    }

    pub fn dispose(&self) {
        self.inner.stop.cancel();
        // The socket's thread closes it as it sees the stop (C#'s Abort).
        guard(&self.inner.out).take();
    }
}

impl Drop for RemoteLink {
    fn drop(&mut self) {
        self.dispose();
    }
}

// No redirects: `may_send_to` checked only the first address, and a redirect could take the screenshots elsewhere.
static HTTP: LazyLock<ureq::Agent> = LazyLock::new(|| crate::net::builder().redirects(0).timeout(Duration::from_secs(60)).build());

static BOT_REPLY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"^/api(/v\d{{1,2}})?/webhooks/{}/[A-Za-z0-9._-]{{20,500}}/messages/@original\n?\z",
        regex::escape(crate::core::app::discord_presence::DISCORD_APPLICATION_ID)
    ))
    .unwrap()
});

pub(crate) fn is_bot_reply(reply: &str) -> bool {
    let Ok(uri) = url::Url::parse(reply) else { return false };
    uri.scheme() == "https"
        && uri.host_str() == Some("discord.com")
        && uri.port().is_none()
        && uri.username().is_empty()
        && uri.password().is_none()
        && uri.query().is_none()
        && uri.fragment().is_none()
        && BOT_REPLY.is_match(uri.path())
}

/// A name cut as the bot cuts them (60 UTF-16 units).
fn cut(name: &str) -> String {
    crate::core::base::cut_utf16(name, MAX_NAME).to_string()
}

/// The names, cut as the bot cuts them, as many as fit in `budget` characters as sent; and the characters used.
fn fit(names: &[String], budget: usize) -> (Vec<String>, usize) {
    let mut fit = Vec::new();
    let mut used = 0;
    for name in names.iter().take(MAX_NAMES).map(|n| cut(n)) {
        // JsonSerializer.Serialize(name).Length + 1: escaped as .NET writes it (all ASCII, so bytes = UTF-16 units).
        let size = to_net_json_string(&name).map(|s| s.len()).unwrap_or(0) + 1;
        if used + size > budget {
            break;
        }
        fit.push(name);
        used += size;
    }
    (fit, used)
}

/// SendAsync: only while the socket is open; dropped otherwise.
fn send(inner: &Inner, text: String) {
    if let Some(out) = guard(&inner.out).as_ref() {
        let _ = out.send(text);
    }
}

type Socket = WebSocket<MaybeTlsStream<TcpStream>>;

/// The socket's own TCP stream, for its timeouts. A timeout set on a `try_clone()` of it doesn't reach it on Windows (each
/// duplicated socket keeps its own SO_RCVTIMEO): reads then waited for the bot's next message, so answers went up late.
fn tcp_of(ws: &Socket) -> Option<&TcpStream> {
    match ws.get_ref() {
        MaybeTlsStream::Plain(s) => Some(s),
        MaybeTlsStream::Rustls(s) => Some(&s.sock),
        _ => None,
    }
}

fn set_read_timeout(ws: &Socket, timeout: Duration) -> bool {
    tcp_of(ws).is_some_and(|s| s.set_read_timeout(Some(timeout)).is_ok())
}

/// Opens the socket with the secret as its bearer token.
fn connect(url: &str, secret: &str, limit: Duration) -> Result<Socket, String> {
    let parsed = url::Url::parse(url).map_err(|e| e.to_string())?;
    let host = parsed.host_str().ok_or("no host")?.to_string();
    let port = parsed.port_or_known_default().ok_or("no port")?;
    let until = Instant::now() + limit;
    let mut stream = None;
    for addr in (host.as_str(), port).to_socket_addrs().map_err(|e| e.to_string())? {
        let left = until.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        if let Ok(s) = TcpStream::connect_timeout(&addr, left) {
            stream = Some(s);
            break;
        }
    }
    let stream = stream.ok_or("couldn't connect")?;
    // The handshake may take no longer than what is left.
    let left = until.saturating_duration_since(Instant::now()).max(Duration::from_millis(1));
    stream.set_read_timeout(Some(left)).map_err(|e| e.to_string())?;
    stream.set_write_timeout(Some(left)).map_err(|e| e.to_string())?;
    let mut request = url.into_client_request().map_err(|e| e.to_string())?;
    request.headers_mut().insert("Authorization", format!("Bearer {secret}").parse().map_err(|_| "bad secret")?);
    // A message over 64 KB is refused as it comes in (a read error, which ends the reading as the C# does), never
    // gathered in memory first.
    let mut config = tungstenite::protocol::WebSocketConfig::default();
    config.max_message_size = Some(MAX_MESSAGE);
    config.max_frame_size = Some(MAX_MESSAGE);
    // ClientWebSocket's KeepAliveInterval = 0: no pings of its own (the bot answers our own "ping" without waking).
    let (ws, _) = tungstenite::client_tls_with_config(request, stream, Some(config), None).map_err(|e| e.to_string())?;
    // A send stuck on a hung network ends the session after this (the read, ping and send share one thread, so a send
    // with no limit held dispose and the next reconnect back until TCP gave up).
    tcp_of(&ws).ok_or("no stream")?.set_write_timeout(Some(Duration::from_secs(30))).map_err(|e| e.to_string())?;
    Ok(ws)
}

/// A connection of its own to tell the bot to forget a secret's link, for one no RemoteLink runs for any more.
/// True only once the bot has said that secret is linked to nobody.
pub fn forget(secret: &str, url: Option<&str>) -> bool {
    let limit = Duration::from_secs(15);
    let until = Instant::now() + limit;
    let Ok(mut ws) = connect(url.unwrap_or(URL), secret, limit) else { return false };
    if ws.send(Message::text(r#"{"t":"unlink"}"#)).is_err() {
        return false;
    }
    loop {
        let left = until.saturating_duration_since(Instant::now());
        if left.is_zero() || !set_read_timeout(&ws, left) {
            return false;
        }
        let text = match ws.read() {
            Ok(Message::Close(_)) => return false,
            Ok(Message::Text(t)) => t.to_string(),
            Ok(Message::Binary(b)) => String::from_utf8_lossy(&b).into_owned(),
            Ok(_) => continue,
            Err(_) => return false,
        };
        // JsonNode.Parse failing, or "t"/"linked" of another kind, threw (caught): false.
        let Ok(m) = serde_json::from_str::<Value>(&text) else { return false };
        let Some(o) = m.as_object() else { continue };
        let t = match o.get("t") {
            None | Some(Value::Null) => None,
            Some(Value::String(s)) => Some(s.as_str()),
            Some(_) => return false,
        };
        let linked = match o.get("linked") {
            None | Some(Value::Null) => None,
            Some(Value::Bool(b)) => Some(*b),
            Some(_) => return false,
        };
        if t == Some("link") && linked == Some(false) {
            let _ = ws.close(None);
            let _ = ws.flush();
            return true;
        }
    }
}

fn changed(inner: &Inner, f: impl FnOnce(&mut LinkState)) {
    f(&mut guard(&inner.state));
    (inner.changed)();
}

fn random_ms(below: u32) -> u64 {
    let mut b = [0u8; 4];
    getrandom::fill(&mut b).expect("the system's random source");
    (u32::from_le_bytes(b) % below) as u64
}

fn run_loop(inner: &Arc<Inner>) {
    let mut wait = FIRST_RETRY;
    // Said when it changes, not on every try (one a minute while offline).
    let mut last_fail: Option<String> = None;
    while !inner.stop.is_cancelled() {
        let mut connected_at: Option<Instant> = None;
        let attempt = connect(&inner.url, &inner.secret, CONNECT_LIMIT);
        if let Err(e) = &attempt {
            if last_fail.as_deref() != Some(e.as_str()) {
                log::note(&format!("Remote control: couldn't connect to the bot ({e}); tried again, less and less often."));
                last_fail = Some(e.clone());
            }
        }
        if let Ok(ws) = attempt {
            if last_fail.take().is_some() {
                log::note("Remote control: connected again.");
            }
            let (tx, rx) = mpsc::channel::<String>();
            *guard(&inner.out) = Some(tx);
            connected_at = Some(Instant::now());
            changed(inner, |s| s.connected = true);
            let (hello, want_code, code, code_until, linked) = {
                let s = guard(&inner.state);
                (s.hello.clone(), s.want_code, s.code.clone(), s.code_until, s.linked)
            };
            if let Some(hello) = hello {
                send(inner, hello);
            }
            if want_code {
                send(inner, r#"{"t":"code"}"#.into());
            }
            // A code shown here may be gone at the bot (it keeps codes in memory: a deploy or a restart forgets them,
            // and typing a forgotten one counts as a wrong guess): the bot keeps it if it still knows it, else gives
            // a new one (2026-10-30 review).
            else if let Some(shown) = code.filter(|_| code_until.is_some_and(|u| u > Stamp::utc_now()) && linked != Some(true)) {
                send(inner, to_net_json_string(&json!({ "t": "code", "keep": shown })).unwrap_or_default());
            }
            if let Err(e) = session(inner, ws, &rx) {
                // Anything else: logged, and connected again as after a drop, so /pc doesn't stay dead until a restart.
                log::write(&e);
            }
        }
        guard(&inner.out).take();
        if guard(&inner.state).connected {
            changed(inner, |s| s.connected = false);
        }
        if inner.stop.is_cancelled() {
            break;
        }
        if connected_at.is_some_and(|at| at.elapsed() >= STAYED_UP) {
            wait = FIRST_RETRY;
        }
        if !inner.stop.sleep(wait + Duration::from_millis(random_ms(1000))) {
            break;
        }
        wait = if wait * 2 < LAST_RETRY { wait * 2 } else { LAST_RETRY };
    }
}

/// The open socket: what goes up is sent, "ping" every 30 s, what comes down is read; Ok when it closed or dropped
/// (the network's errors are quiet), Err for a message that broke the reading (logged).
fn session(inner: &Arc<Inner>, mut ws: Socket, out: &mpsc::Receiver<String>) -> Res<()> {
    if !set_read_timeout(&ws, POLL) {
        return Ok(());
    }
    let mut next_ping = Instant::now() + KEEP_ALIVE;
    loop {
        if inner.stop.is_cancelled() {
            // What was sent before the stop goes first: C#'s Unlink awaited its send before Dispose closed the socket.
            while let Ok(text) = out.try_recv() {
                if ws.send(Message::text(text)).is_err() {
                    break;
                }
            }
            let _ = ws.close(None);
            let _ = ws.flush();
            return Ok(());
        }
        while let Ok(text) = out.try_recv() {
            if ws.send(Message::text(text)).is_err() {
                return Ok(());
            }
        }
        if Instant::now() >= next_ping {
            if ws.send(Message::text("ping")).is_err() {
                return Ok(());
            }
            next_ping = Instant::now() + KEEP_ALIVE;
        }
        let text = match ws.read() {
            Ok(Message::Close(_)) => return Ok(()),
            Ok(Message::Text(t)) => t.to_string(),
            Ok(Message::Binary(b)) => String::from_utf8_lossy(&b).into_owned(),
            Ok(_) => continue,
            Err(tungstenite::Error::Io(e)) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => continue,
            Err(e) => {
                log::note(&format!("Remote control: connection lost ({e}); connected again shortly."));
                return Ok(());
            }
        };
        // A message over 64 KB never gets here: the socket's own limit (see `connect`) ended the reading.
        if text != "pong" {
            on_message(inner, &text)?;
        }
    }
}

fn str_of(m: &Map<String, Value>, name: &str) -> Option<String> {
    m.get(name).and_then(Value::as_str).map(String::from)
}

fn on_message(inner: &Arc<Inner>, text: &str) -> Res<()> {
    let Ok(parsed) = serde_json::from_str::<Value>(text) else { return Ok(()) };
    let Some(m) = parsed.as_object() else { return Ok(()) };
    match str_of(m, "t").as_deref() {
        Some("link") => changed(inner, |s| {
            s.linked = Some(matches!(m.get("linked"), Some(Value::Bool(true))));
            s.user = if s.linked == Some(true) { str_of(m, "user") } else { None };
            s.asking = None; // allowed, refused or given up on: either way not asked any more
            if s.linked == Some(true) {
                s.code = None;
                s.code_until = None;
                s.want_code = false;
            }
        }),
        Some("ask") => {
            if let Some(asker) = str_of(m, "user").filter(|u| !u.is_empty()) {
                changed(inner, |s| s.asking = Some(Asking { user: asker, replacing: str_of(m, "replacing"), id: str_of(m, "id") }));
            }
        }
        Some("busy") => changed(inner, |s| {
            s.busy = true;
            s.want_code = false;
        }),
        Some("code") => {
            if let Some(code) = str_of(m, "code").filter(|c| !c.is_empty()) {
                // GetValue<long> on a number that isn't a whole one throws: the reading stops and is logged.
                let until = match m.get("until") {
                    Some(Value::Number(n)) => n.as_i64().ok_or_else(|| Fail::other(format!("\"until\" isn't a whole number: {n}")))?,
                    _ => 0,
                };
                changed(inner, |s| {
                    s.code = Some(code);
                    s.busy = false;
                    s.code_until = Some(if until > 0 { Stamp::from_unix_ms(until) } else { Stamp::utc_now().plus_secs(10.0 * 60.0) });
                    s.want_code = false;
                });
            }
        }
        // Not waited for here: a long one (ten windows pictured, about a minute) would hold back every message behind
        // it, an Allow or a code among them (2026-10-30 review).
        Some("command") => {
            if let (Some(id), Some(name)) = (str_of(m, "id").filter(|i| !i.is_empty()), str_of(m, "command").filter(|c| !c.is_empty())) {
                let command = RemoteCommand { id, name, value: str_of(m, "value"), reply: str_of(m, "reply") };
                let run = inner.clone();
                std::thread::spawn(move || run_command(&run, command));
            }
        }
        _ => {}
    }
    Ok(())
}

fn run_command(inner: &Inner, command: RemoteCommand) {
    let id = command.id.clone();
    // A command's own error (a panic here) is logged and left unanswered, as the C# does with its exception.
    let answer = match crate::core::program::catch_unwind(std::panic::AssertUnwindSafe(|| (inner.command)(command))) {
        Ok(answer) => answer,
        Err(_) => {
            log::note("Remote command failed.");
            return;
        }
    };
    let Some(mut answer) = answer else { return };
    answer.insert("t".into(), json!("answer"));
    answer.insert("id".into(), json!(id));
    send(inner, to_net_json_string(&Value::Object(answer)).unwrap_or_default());
}

