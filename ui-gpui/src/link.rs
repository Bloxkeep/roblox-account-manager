// The line to the C# app (AppController): the same JSON messages the web page sends and gets, one per line, over two
// one-way named pipes made by the app: `<name>` brings its messages, `<name>-up` takes ours. (One two-way pipe opened
// for synchronous use would hold a write until the pending read finished.)
//
// Ours are written by one thread kept for the life of the line, never by the one that draws: a write to a pipe waits for the app to read
// it, and the app, laying this window inside its own, waits for this window to answer (2026-11-11: each waited for
// the other, and both stood still).
//
// Ours:   {"id":1,"cmd":"getState","args":{…}}            a command, answered by a reply with the same id
//         {"type":"hello","hwnd":123}                       once, so the app can put this window inside its own
// Theirs: {"type":"reply","id":1,"ok":true,"result":…}     or ok:false with "error"
//         {"type":"event","name":"state","data":…}         the whole state, metrics, toasts, progress…
use crate::threads::Hold;
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Write},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
        mpsc::{Sender, channel},
    },
};

use futures::channel::{mpsc, oneshot};
use serde_json::{Value, json};

/// What a command came back with: its result, or the app's sentence for why it failed.
pub type Reply = Result<Value, String>;

#[derive(Clone)]
pub struct Link {
    /// The app's core is in this process (src/core): no pipes, the window's handle goes straight to its shell.
    local: bool,
    /// Our lines, in order, to the thread that writes them.
    writer: Sender<String>,
    next: Arc<AtomicU32>,
    waiting: Arc<Mutex<HashMap<u32, oneshot::Sender<Reply>>>>,
}

/// An event from the app: its name and data.
pub type Event = (String, Value);

fn open(name: &str, write: bool) -> std::io::Result<File> {
    OpenOptions::new().read(!write).write(write).open(format!(r"\\.\pipe\{name}"))
}

/// Connects to the app's two pipes; the events come on the receiver. Fails when the app isn't there.
pub fn connect(name: &str) -> anyhow::Result<(Link, mpsc::UnboundedReceiver<Event>)> {
    let down = open(name, false)?;
    let up = open(&format!("{name}-up"), true)?;
    let (lines, written) = channel::<String>();
    let mut up = up;
    crate::threads::own("link-write", move || {
        for line in written {
            if writeln!(up, "{line}").and_then(|_| up.flush()).is_err() {
                break;
            }
        }
    })?;
    let link = Link { local: false, writer: lines, next: Arc::new(AtomicU32::new(1)), waiting: Arc::default() };
    let (tx, rx) = mpsc::unbounded();
    let waiting = link.waiting.clone();
    crate::threads::own("link-read", move || {
        for line in BufReader::new(down).lines() {
            let Ok(line) = line else { break };
            if !take(&line, &waiting, &tx) {
                break;
            }
        }
        // The app went away: whatever waits gets an error, and the window closes (main.rs).
        {
            let mut m = waiting.hold();
            for (_, w) in m.drain() {
                let _ = w.send(Err("Can't reach the app.".into()));
            }
        }
        let _ = tx.unbounded_send(("linkClosed".into(), Value::Null));
    })?;
    Ok((link, rx))
}

/// One of the app's lines: a reply goes to whoever waits for it, an event to the window. False once nobody listens.
fn take(line: &str, waiting: &Mutex<HashMap<u32, oneshot::Sender<Reply>>>, tx: &mpsc::UnboundedSender<Event>) -> bool {
    let Ok(msg) = serde_json::from_str::<Value>(line) else { return true };
    match msg.get("type").and_then(Value::as_str) {
        Some("reply") => {
            let id = msg.get("id").and_then(Value::as_u64).unwrap_or(0) as u32;
            let reply = if msg.get("ok").and_then(Value::as_bool) == Some(true) {
                Ok(msg.get("result").cloned().unwrap_or(Value::Null))
            } else {
                Err(msg.get("error").and_then(Value::as_str).unwrap_or("Unknown error").to_string())
            };
            let w = waiting.hold().remove(&id);
            if let Some(w) = w {
                let _ = w.send(reply);
            }
        }
        Some("event") => {
            let name = msg.get("name").and_then(Value::as_str).unwrap_or_default().to_string();
            let data = msg.get("data").cloned().unwrap_or(Value::Null);
            if tx.unbounded_send((name, data)).is_err() {
                return false;
            }
        }
        _ => {}
    }
    true
}

/// The line to the core in this process (src/core, the C# app moved to Rust): the same messages both ways, handed
/// over in memory (core::gpui_host) instead of through pipes.
#[cfg(feature = "core")]
pub fn local() -> (Link, mpsc::UnboundedReceiver<Event>) {
    let (lines, written) = channel::<String>();
    let _ = crate::threads::own("link-write", move || {
        for line in written {
            crate::core::gpui_host::on_message(line);
        }
    });
    let link = Link { local: true, writer: lines, next: Arc::new(AtomicU32::new(1)), waiting: Arc::default() };
    let (tx, rx) = mpsc::unbounded();
    let waiting = link.waiting.clone();
    crate::core::gpui_host::set_receiver(move |line| {
        take(&line, &waiting, &tx);
    });
    (link, rx)
}

impl Link {
    fn write(&self, msg: &Value) {
        let _ = self.writer.send(msg.to_string());
    }

    /// A command to the app (one of bridge.ts's COMMANDS); the receiver gets its reply.
    pub fn call(&self, cmd: &str, args: Value) -> oneshot::Receiver<Reply> {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.waiting.hold().insert(id, tx);
        self.write(&json!({ "id": id, "cmd": cmd, "args": args }));
        rx
    }

    /// A command whose answer nobody waits for.
    pub fn send(&self, cmd: &str, args: Value) {
        // (Not put among the waiting: a command the app never answered left its entry there for good.)
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        self.write(&json!({ "id": id, "cmd": cmd, "args": args }));
    }

    /// Tells the app this window's handle, so it can lay the window inside its own.
    pub fn hello(&self, hwnd: isize) {
        #[cfg(feature = "core")]
        if self.local {
            return crate::core::gpui_host::window_made(hwnd);
        }
        let _ = self.local;
        self.write(&json!({ "type": "hello", "hwnd": hwnd }));
    }
}
