// FeedLink.cs: game news from Bloxkeep's Discord bot, over a WebSocket that only listens.
use std::collections::HashSet;
use std::net::TcpStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use tungstenite::protocol::WebSocketConfig;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message as WsMessage, WebSocket};

use crate::core::log;
use crate::core::roblox_launcher::random_u64;

/// A JSON null read as the type's default (System.Text.Json left a non-nullable string null; the checks then refuse it).
pub(crate) fn null_default<'de, D: Deserializer<'de>, T: Default + Deserialize<'de>>(d: D) -> Result<T, D::Error> {
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

// PORT: these are read with the bot's camelCase names (System.Text.Json matched them ignoring case) and written PascalCase.

/// One source of a game's news (a feed channel at Bloxkeep's bot): "Stock", "Fruit spawn"…
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(serialize = "PascalCase", deserialize = "camelCase"), default)]
pub struct FeedSource {
    #[serde(deserialize_with = "null_default")]
    pub id: String,
    #[serde(deserialize_with = "null_default")]
    pub title: String,
}

/// A game with news, as Bloxkeep's bot lists it: its id (what following remembers), name, place on Roblox and sources.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(serialize = "PascalCase", deserialize = "camelCase"), default)]
pub struct FeedGame {
    #[serde(deserialize_with = "null_default")]
    pub id: String,
    #[serde(deserialize_with = "null_default")]
    pub name: String,
    pub place: Option<i64>,
    #[serde(deserialize_with = "null_default")]
    pub sources: Vec<Option<FeedSource>>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(serialize = "PascalCase", deserialize = "camelCase"), default)]
pub struct FeedField {
    #[serde(deserialize_with = "null_default")]
    pub name: String,
    #[serde(deserialize_with = "null_default")]
    pub value: String,
}

/// Who posted it, as the game's bot names itself in its message (its name and picture).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(serialize = "PascalCase", deserialize = "camelCase"), default)]
pub struct FeedAuthor {
    #[serde(deserialize_with = "null_default")]
    pub name: String,
    pub icon: Option<String>,
    pub url: Option<String>,
}

/// One piece of news: a message another game's bot posted, read by Bloxkeep's bot (feeds.js cardsOf). At: when it came
/// or last changed (ms); Until: when what it tells of ends, if it says (ms). Text may hold Discord's times (<t:…>),
/// which the page shows in the person's own clock. Blocks: what it shows, in Discord's order (text with Discord's markdown,
/// fields, sections, galleries, files, lines, buttons, a footer, a poll, a sticker), passed to the page as they came so a
/// way of posting the app has not seen yet needs no update here; Title, Text and Fields are the same words for alerts.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(serialize = "PascalCase", deserialize = "camelCase"), default)]
pub struct FeedCard {
    #[serde(deserialize_with = "null_default")]
    pub id: String,
    #[serde(deserialize_with = "null_default")]
    pub src: String,
    #[serde(deserialize_with = "null_default")]
    pub game: String,
    pub at: i64,
    #[serde(deserialize_with = "null_default")]
    pub title: String,
    #[serde(deserialize_with = "null_default")]
    pub text: String,
    #[serde(deserialize_with = "null_default")]
    pub fields: Vec<Option<FeedField>>,
    pub picture: Option<String>,
    pub link: Option<String>,
    pub color: Option<i32>,
    pub until: Option<i64>,
    pub image: Option<String>,
    pub author: Option<FeedAuthor>,
    pub blocks: Option<Value>,
    pub spoiler: bool,
}

pub const URL: &str = "wss://bloxkeep-bot.bloxkeep-app.workers.dev/feeds/connect";
const KEEP_ALIVE: Duration = Duration::from_secs(30);
const FIRST_RETRY: Duration = Duration::from_secs(2);
const LAST_RETRY: Duration = Duration::from_secs(120);
const STAYED_UP: Duration = Duration::from_secs(60);
/// The games and every card at once (30 a source, at most a few KB each).
const MAX_MESSAGE: usize = 4 * 1024 * 1024;
/// How often `wait` looks for being stopped.
const POLL: Duration = Duration::from_millis(100);
/// How long a read waits for news before looking for being stopped and for the ping that is due.
const READ_WAIT: Duration = Duration::from_secs(1);
/// The connect and the handshake together; a send waits as long at most.
const CONNECT_LIMIT: Duration = Duration::from_secs(15);

type Socket = WebSocket<MaybeTlsStream<TcpStream>>;

pub type AllFn = Box<dyn Fn(Vec<FeedGame>, Vec<FeedCard>) + Send + Sync>;
pub type CardFn = Box<dyn Fn(FeedCard) + Send + Sync>;
pub type GoneFn = Box<dyn Fn(String) + Send + Sync>;
pub type ChangedFn = Box<dyn Fn() + Send + Sync>;

struct Shared {
    url: String,
    all: AllFn,
    card: CardFn,
    gone: GoneFn,
    changed: ChangedFn,
    stop: AtomicBool,
    connected: AtomicBool,
}

/// Game news (Settings-free: the Game news tab) from Bloxkeep's Discord bot (services/DiscordBot/worker/feeds.js): a
/// WebSocket that only listens. On connecting it is told the games and their latest news, then each new piece as it comes,
/// one deleted there, and the games again when Bloxkeep's developers add or take away a game or a source. Nothing goes up
/// but a "ping" every 30 s, which the bot answers without waking. Connected again after a drop (2 s, then longer, up to
/// two minutes).
// PORT: the C# posted its callbacks to the window's SynchronizationContext; here they run on the link's own thread, one
// at a time, in order. The caller hands them on to its window if it needs to.
pub struct FeedLink {
    shared: Arc<Shared>,
}

impl FeedLink {
    /// `all`: the games and their cards, all of them (on connecting, and when the games change).
    /// `card`: a new or changed card. `gone`: a card's id, deleted where it came from. `changed`: connected or not any more.
    pub fn new(all: AllFn, card: CardFn, gone: GoneFn, changed: ChangedFn, url: Option<&str>) -> FeedLink {
        let shared = Arc::new(Shared {
            url: url.unwrap_or(URL).to_string(),
            all,
            card,
            gone,
            changed,
            stop: AtomicBool::new(false),
            connected: AtomicBool::new(false),
        });
        let run = Arc::clone(&shared);
        // The link's loop lives as long as the link, so it has a thread of its own, not a pool one.
        let _ = crate::threads::own("feed-link", move || run_link(&run));
        FeedLink { shared }
    }

    pub fn connected(&self) -> bool {
        self.shared.connected.load(Ordering::SeqCst)
    }

    pub fn dispose(&self) {
        self.shared.stop.store(true, Ordering::SeqCst);
    }
}

impl Drop for FeedLink {
    fn drop(&mut self) {
        self.dispose();
    }
}

/// Task.Delay with the stop token: false when stopped first.
fn wait(shared: &Shared, d: Duration) -> bool {
    let until = Instant::now() + d;
    while Instant::now() < until {
        if shared.stop.load(Ordering::SeqCst) {
            return false;
        }
        std::thread::sleep(POLL.min(until.saturating_duration_since(Instant::now())));
    }
    !shared.stop.load(Ordering::SeqCst)
}

fn run_link(shared: &Shared) {
    let mut retry = FIRST_RETRY;
    while !shared.stop.load(Ordering::SeqCst) {
        let mut connected_at: Option<Instant> = None;
        let mut config = WebSocketConfig::default();
        config.max_message_size = Some(MAX_MESSAGE);
        // PORT: the connect itself can't be stopped part way (tungstenite blocks); a stop is seen once it returns, within
        // CONNECT_LIMIT. Any failure (WebSocketException, IOException, HttpRequestException in the C#) only means
        // trying again later.
        if let Some(mut ws) = connect(&shared.url, config) {
            connected_at = Some(Instant::now());
            on_ui(|| {
                shared.connected.store(true, Ordering::SeqCst);
                (shared.changed)();
            });
            if set_read_timeout(&ws, READ_WAIT) {
                read(shared, &mut ws);
            }
            let _ = ws.close(None);
            let _ = ws.flush();
        }
        if shared.connected.load(Ordering::SeqCst) {
            on_ui(|| {
                shared.connected.store(false, Ordering::SeqCst);
                (shared.changed)();
            });
        }
        if shared.stop.load(Ordering::SeqCst) {
            break;
        }
        if connected_at.is_some_and(|at| at.elapsed() >= STAYED_UP) {
            retry = FIRST_RETRY;
        }
        if !wait(shared, retry + Duration::from_millis(random_u64() % 2000)) {
            break;
        }
        retry = if retry * 2 < LAST_RETRY { retry * 2 } else { LAST_RETRY };
    }
}

/// TCP within CONNECT_LIMIT, then the handshake within what is left of it (tungstenite's own `connect` waits as long as
/// Windows does, a minute or more on a network that drops packets).
fn connect(url: &str, config: WebSocketConfig) -> Option<Socket> {
    use std::net::ToSocketAddrs;
    let parsed = url::Url::parse(url).ok()?;
    let host = parsed.host_str()?.to_string();
    let port = parsed.port_or_known_default()?;
    let until = Instant::now() + CONNECT_LIMIT;
    let stream = (host.as_str(), port).to_socket_addrs().ok()?.find_map(|addr| {
        let left = until.saturating_duration_since(Instant::now());
        if left.is_zero() { None } else { TcpStream::connect_timeout(&addr, left).ok() }
    })?;
    let left = until.saturating_duration_since(Instant::now()).max(Duration::from_millis(1));
    stream.set_read_timeout(Some(left)).ok()?;
    stream.set_write_timeout(Some(CONNECT_LIMIT)).ok()?;
    let (ws, _) = tungstenite::client_tls_with_config(url, stream, Some(config), None).ok()?;
    Some(ws)
}

/// Reads wait a second at most, so reading can stop and the ping go out on time. Set on the socket's own stream: a
/// timeout on a `try_clone()` of it doesn't reach it on Windows (see remote_link).
fn set_read_timeout(ws: &Socket, timeout: Duration) -> bool {
    let tcp = match ws.get_ref() {
        MaybeTlsStream::Plain(s) => s,
        MaybeTlsStream::Rustls(s) => s.get_ref(),
        _ => return false,
    };
    tcp.set_read_timeout(Some(timeout)).is_ok()
}

/// Nothing came within the wait (or a send couldn't go yet): not a drop.
fn is_would_block(e: &tungstenite::Error) -> bool {
    matches!(e, tungstenite::Error::Io(io) if matches!(io.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut))
}

/// Reads until the link closes or drops, or the link is stopped; the "ping" every 30 s goes out on the way.
fn read(shared: &Shared, ws: &mut Socket) {
    let mut next_ping = Instant::now() + KEEP_ALIVE;
    loop {
        if shared.stop.load(Ordering::SeqCst) {
            return;
        }
        if Instant::now() >= next_ping {
            next_ping = Instant::now() + KEEP_ALIVE;
            match ws.send(WsMessage::Text("ping".into())) {
                Ok(()) => {}
                Err(e) if is_would_block(&e) => {} // queued: written by the next flush
                Err(_) => return,
            }
        }
        match ws.read() {
            Ok(WsMessage::Text(text)) => {
                if text != "pong" {
                    on_message(shared, &text);
                }
            }
            Ok(WsMessage::Binary(bytes)) => {
                let text = String::from_utf8_lossy(&bytes).into_owned();
                if text != "pong" {
                    on_message(shared, &text);
                }
            }
            Ok(WsMessage::Close(_)) => return,
            Ok(_) => {} // Ping and Pong frames: tungstenite answers them itself
            Err(e) if is_would_block(&e) => {
                let _ = ws.flush();
            }
            // A message over MAX_MESSAGE, a drop, a close: connected again later.
            Err(_) => return,
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct Message {
    t: Option<String>,
    games: Option<Vec<Option<FeedGame>>>,
    cards: Option<Vec<Option<FeedCard>>>,
    card: Option<FeedCard>,
    id: Option<String>,
}

fn on_message(shared: &Shared, text: &str) {
    // The C#'s deserialization: any part of the wrong type drops the whole message.
    let Ok(raw) = serde_json::from_str::<Value>(text) else { return };
    let Ok(m) = serde_json::from_value::<Option<Message>>(raw.clone()) else { return };
    let Some(m) = m else { return };
    match m.t.as_deref() {
        Some("feeds") if m.games.is_some() => {
            let raw_games = raw.get("games").and_then(Value::as_array).cloned().unwrap_or_default();
            let ok: Vec<FeedGame> = m
                .games
                .unwrap_or_default()
                .into_iter()
                .zip(raw_games.iter())
                .filter_map(|(g, r)| g.filter(|g| valid_game(g, r)))
                .collect();
            let known: HashSet<String> = ok.iter().map(|g| g.id.clone()).collect();
            let raw_cards = raw.get("cards").and_then(Value::as_array).cloned().unwrap_or_default();
            let cards: Vec<FeedCard> = m
                .cards
                .unwrap_or_default()
                .into_iter()
                .zip(raw_cards.iter())
                .filter_map(|(c, r)| c.filter(|c| valid_card(c, r) && known.contains(&c.game)))
                .collect();
            on_ui(|| (shared.all)(ok, cards));
        }
        Some("card") => {
            if let Some(card) = m.card
                && raw.get("card").is_some_and(|r| valid_card(&card, r))
            {
                on_ui(|| (shared.card)(card));
            }
        }
        Some("gone") => {
            if let Some(id) = m.id.filter(|i| !i.is_empty()) {
                on_ui(|| (shared.gone)(id));
            }
        }
        _ => {}
    }
}

/// Matching the property ignoring case, as System.Text.Json read it.
fn raw_prop<'a>(raw: &'a Value, name: &str) -> Option<&'a Value> {
    raw.as_object()?.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v)
}

fn present(raw: &Value, name: &str) -> bool {
    raw_prop(raw, name).is_some_and(|v| !v.is_null())
}

fn valid_game(g: &FeedGame, raw: &Value) -> bool {
    (1..=40).contains(&g.id.encode_utf16().count()) && !g.name.is_empty() && present(raw, "sources")
}

fn valid_card(c: &FeedCard, raw: &Value) -> bool {
    !c.id.is_empty()
        && !c.game.is_empty()
        && present(raw, "title")
        && present(raw, "text")
        && present(raw, "fields")
        && raw_prop(raw, "blocks").is_none_or(|b| b.is_null() || b.is_array())
}

/// A callback, its failure logged rather than ending the link.
fn on_ui(action: impl FnOnce()) {
    if let Err(panic) = crate::core::program::catch_unwind(std::panic::AssertUnwindSafe(action)) {
        let text = panic.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| panic.downcast_ref::<String>().cloned()).unwrap_or_else(|| "a callback failed".into());
        log::write(&text);
    }
}
