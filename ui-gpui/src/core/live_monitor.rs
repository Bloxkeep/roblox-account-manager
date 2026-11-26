//! The web encoder uses a loopback connection to the running app. Only this process changes window state.
use std::{
    io::{self, BufRead, BufReader, Read, Write},
    net::{Ipv4Addr, TcpListener, TcpStream},
    sync::{
        Arc, Mutex, mpsc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use super::{
    app::App,
    input_turn, log, monitor_frames, roblox_processes as players, rt,
    win_kit::Picture,
    window_loan::{self, How, Loan},
    window_shot,
};

const PORT: u16 = 8790;
const ROSTER_PORT: u16 = 8793;
const MAX_IDLE: Duration = Duration::from_secs(15);
static ENDING: AtomicBool = AtomicBool::new(false);
/// The page's commands that ended after their answer was given ("working"): (its number, when, what it did), told to
/// the page with the list of windows for a few minutes.
static DONE: Mutex<Vec<(u32, std::time::Instant, serde_json::Value)>> = Mutex::new(Vec::new());
static NEXT_COMMAND: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);
/// How long the page's command is waited for before it is answered "working" (its outcome comes with the list).
const COMMAND_WAIT: Duration = Duration::from_millis(1500);
const DONE_KEPT: Duration = Duration::from_secs(5 * 60);
/// Bloxkeep closing: how long the viewed window put back down is waited for (each of the two asks).
const SHUTDOWN_WAIT: Duration = Duration::from_millis(300);
static ACTIVE: Mutex<Option<Target>> = Mutex::new(None);

struct Target {
    pid: i32,
    /// The window, lent for as long as the view is open (window_loan.rs): the app's window rules leave it alone, and it
    /// goes back as it was (minimised again, its place back) when the view ends. None when it couldn't be lent (gone,
    /// not answering).
    loan: Option<Loan>,
}

/// The window of `pid` is made big for a sharper view just now ("Sharper" on the viewer's page): where it stands isn't
/// its own place (not to be remembered as the account's, rejoin_place.rs).
pub fn sharp(pid: i32) -> bool {
    ACTIVE.lock().unwrap_or_else(|e| e.into_inner()).as_ref().is_some_and(|target| target.pid == pid && target.loan.as_ref().is_some_and(|l| l.moved()))
}

/// A macro plays or is being recorded: it presses by position, so no window is resized under it.
fn macros_busy() -> bool {
    let (tx, rx) = mpsc::sync_channel(1);
    rt::post(move || App::with(|app| {
        let _ = tx.send(app.macros_playing());
    }));
    rx.recv_timeout(Duration::from_millis(900)).unwrap_or(true)
}

/// Where the window goes for a sharper view: its screen's whole working area. None when it is big already (maximised,
/// full screen, or nearly the screen's size).
fn big_place(hwnd: isize) -> Option<super::win_kit::Rect> {
    if !players::is_plain_window(hwnd) {
        return None;
    }
    let before = players::window_rect(hwnd)?;
    let area = super::win_kit::screen_from_handle(hwnd).working_area;
    if before.width * 10 >= area.width * 9 && before.height * 10 >= area.height * 9 {
        return None;
    }
    Some(area)
}

pub fn watching(pid: i32) -> bool {
    ACTIVE.lock().unwrap_or_else(|e| e.into_inner()).as_ref().is_some_and(|target| target.pid == pid)
}

pub(super) fn start() {
    let Ok(key) = std::env::var("BLOXKEEP_WATCH_BRIDGE_KEY") else { return };
    if key.len() != 48 || !key.bytes().all(|b| b.is_ascii_hexdigit()) {
        return;
    }
    if let Err(error) = listeners(PORT, ROSTER_PORT, key, Arc::new(AtomicBool::new(true))) {
        log::note(&format!("Live monitor: could not bind localhost: {error}"));
    }
}

pub(super) fn listeners(video_port: u16, roster_port: u16, key: String, live: Arc<AtomicBool>) -> io::Result<(u16, u16)> {
    let video = TcpListener::bind((Ipv4Addr::LOCALHOST, video_port))?;
    let roster = TcpListener::bind((Ipv4Addr::LOCALHOST, roster_port))?;
    let ports = (video.local_addr()?.port(), roster.local_addr()?.port());
    // Read the roster separately so refreshing it does not wait for the viewer to disconnect.
    listen(roster, "live-monitor-roster", key.clone(), live.clone(), serve_roster)?;
    if let Err(error) = listen(video, "live-monitor", key, live.clone(), serve) {
        live.store(false, Ordering::Release);
        return Err(error);
    }
    Ok(ports)
}

fn listen(listener: TcpListener, name: &str, key: String, live: Arc<AtomicBool>, handler: fn(TcpStream, &str, &AtomicBool) -> io::Result<()>) -> io::Result<()> {
    listener.set_nonblocking(true)?;
    // One round a connection (threads::keep: a request that panics its handler costs that request, not the monitor).
    crate::threads::keep(name, move || {
        if !listening(&live) {
            return false;
        }
        match listener.accept() {
            Ok((stream, _)) => {
                if stream.set_nonblocking(false).is_ok() {
                    let _ = handler(stream, &key, &live);
                }
                true
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(40));
                true
            }
            Err(_) => false,
        }
    })?;
    Ok(())
}

fn listening(live: &AtomicBool) -> bool {
    live.load(Ordering::Acquire) && !ENDING.load(Ordering::Acquire)
}

fn serve_roster(mut stream: TcpStream, key: &str, live: &AtomicBool) -> io::Result<()> {
    let timeout = Duration::from_secs(2);
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    if !authenticated(&receive(&mut reader)?, key) || !listening(live) {
        return Ok(());
    }
    // Then what is asked: "roster" (the windows' names, and what the page's commands need), or "do {json}", a
    // command (remote_page.rs).
    let ask = receive(&mut reader)?;
    if let Some(order) = ask.strip_prefix("do ") {
        return command(&mut stream, order, live);
    }

    let players = players::get_players();
    let (tx, rx) = mpsc::sync_channel(1);
    rt::post(move || App::with(|app| {
        let mut labels = serde_json::Map::new();
        if app.unlocked() && !app.controller.disposed.get() {
            let ids: std::collections::HashMap<_, _> = players.into_iter()
                .filter(|p| p.created > 0)
                .map(|p| (p.pid, format!("{}-{}", p.pid, p.created)))
                .collect();
            for client in app.monitor_clients() {
                if client.account_id.is_some() && let Some(id) = ids.get(&client.pid) {
                    labels.insert(id.clone(), serde_json::Value::String(client.label));
                }
            }
            let mut roster = serde_json::json!({ "state": "ready", "labels": labels, "language": crate::lang::cur().code });
            if let (Some(all), serde_json::Value::Object(extras)) = (roster.as_object_mut(), app.remote_page_extras(&ids)) {
                all.extend(extras);
                all.insert("done".into(), serde_json::Value::Array(done_lately()));
            }
            let _ = tx.send(roster);
        } else {
            let _ = tx.send(serde_json::json!({ "state": "locked", "language": crate::lang::cur().code }));
        }
    }));
    if let Ok(roster) = rx.recv_timeout(Duration::from_millis(900)) && listening(live) {
        send(&mut stream, &serde_json::to_vec(&roster)?)?;
    }
    Ok(())
}

/// The commands that ended lately, each as {id, ...what it did}.
fn done_lately() -> Vec<serde_json::Value> {
    let mut done = DONE.lock().unwrap_or_else(|e| e.into_inner());
    done.retain(|(_, at, _)| at.elapsed() < DONE_KEPT);
    done.iter()
        .map(|(id, _, v)| {
            let mut v = v.clone();
            if let Some(o) = v.as_object_mut() {
                o.insert("id".into(), (*id).into());
            }
            v
        })
        .collect()
}

/// A command of the page: run on the app's thread, answered with what it did, or "working" when it takes longer
/// than COMMAND_WAIT (an opening preset, Clean RAM): what it did then comes with the list of windows.
fn command(stream: &mut TcpStream, order: &str, live: &AtomicBool) -> io::Result<()> {
    let id = NEXT_COMMAND.fetch_add(1, Ordering::Relaxed);
    let answer = |stream: &mut TcpStream, mut v: serde_json::Value| {
        if let Some(o) = v.as_object_mut() {
            o.insert("id".into(), id.into());
        }
        send(stream, &serde_json::to_vec(&v)?)
    };
    let Ok(order) = serde_json::from_str::<serde_json::Value>(order) else {
        return answer(stream, serde_json::json!({ "code": "bad" }));
    };
    if !allowed(live) {
        return answer(stream, serde_json::json!({ "code": "locked" }));
    }
    let players = players::get_players();
    let (tx, rx) = mpsc::sync_channel(1);
    rt::post(move || App::with(|app| {
        let app = app.clone();
        rt::spawn(async move {
            let did = app.remote_page_do(order, players).await;
            // (Kept for the list too, whether or not the answer waited for it: one that ends just as the wait
            // runs out isn't lost. The page shows each command's outcome once.)
            DONE.lock().unwrap_or_else(|e| e.into_inner()).push((id, std::time::Instant::now(), did.clone()));
            let _ = tx.try_send(did);
        });
    }));
    match rx.recv_timeout(COMMAND_WAIT) {
        Ok(did) => answer(stream, did),
        Err(_) => answer(stream, serde_json::json!({ "code": "working" })),
    }
}

fn authenticated(supplied: &str, key: &str) -> bool {
    supplied.len() == key.len()
        && supplied.bytes().zip(key.bytes()).fold(0, |sum, (a, b)| sum | (a ^ b)) == 0
}

fn allowed(live: &AtomicBool) -> bool {
    if !listening(live) {
        return false;
    }
    let (tx, rx) = mpsc::sync_channel(1);
    rt::post(move || App::with(|app| {
        let _ = tx.send(app.unlocked() && !app.controller.disposed.get());
    }));
    rx.recv_timeout(Duration::from_millis(900)).unwrap_or(false) && listening(live)
}

fn receive(reader: &mut BufReader<TcpStream>) -> io::Result<String> {
    let mut line = String::new();
    reader.take(160).read_line(&mut line)?;
    if line.ends_with('\n') {
        line.truncate(line.trim_end().len());
        Ok(line)
    } else {
        Err(io::Error::new(io::ErrorKind::UnexpectedEof, "live monitor link ended"))
    }
}

fn send(stream: &mut TcpStream, body: &[u8]) -> io::Result<()> {
    stream.write_all(&(body.len() as u32).to_le_bytes())?;
    stream.write_all(body)
}

fn serve(mut stream: TcpStream, key: &str, live: &AtomicBool) -> io::Result<()> {
    stream.set_nodelay(true)?;
    stream.set_read_timeout(Some(MAX_IDLE))?;
    stream.set_write_timeout(Some(MAX_IDLE))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    if !authenticated(&receive(&mut reader)?, key) {
        return Ok(());
    }
    let line = receive(&mut reader)?;
    let (line, sharp) = match line.strip_suffix(" sharp") {
        Some(rest) => (rest.to_string(), true),
        None => (line, false),
    };
    let direct = line.strip_suffix(" direct");
    let Some((pid, created)) = direct.unwrap_or(&line).split_once(' ') else { return send(&mut stream, b"window") };
    let direct = direct.is_some();
    if !allowed(live) {
        return send(&mut stream, b"locked");
    }
    if sharp && macros_busy() {
        return send(&mut stream, b"macro");
    }
    let Some(turn) = input_turn::try_enter("live-open") else { return send(&mut stream, b"busy") };
    if !allowed(live) {
        return send(&mut stream, b"locked");
    }
    let (Ok(pid_number), Some(hwnd)) = (pid.parse(), monitor_frames::window(pid, created)) else {
        return send(&mut stream, b"window");
    };
    let target_pid = pid_number;
    {
        let mut current = ACTIVE.lock().unwrap_or_else(|e| e.into_inner());
        if current.is_some() {
            return send(&mut stream, b"busy");
        }
        *current = Some(Target { pid: pid_number, loan: None });
    }
    let stop = Stop;
    // Lent from here until the view ends; a minimised window is opened in view without taking the keyboard.
    let mut loan = window_loan::borrow(hwnd, How::VIEW);
    if loan.as_ref().is_some_and(|l| l.was().minimised) {
        for _ in 0..10 {
            if !players::is_minimised(hwnd) {
                break;
            }
            std::thread::sleep(Duration::from_millis(30));
        }
    }
    if sharp
        && !players::is_minimised(hwnd)
        && let Some(l) = loan.as_mut()
        && let Some(area) = big_place(hwnd)
    {
        l.move_to(area);
        log::note(&format!("Live view: Roblox window {target_pid} made big for a sharper view; put back when the view ends."));
    }
    if let Some(current) = ACTIVE.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        current.loan = loan;
    }
    drop(turn);
    if players::is_minimised(hwnd) || !players::still_of(hwnd, target_pid) {
        return send(&mut stream, b"window");
    }
    if direct {
        send(&mut stream, format!("ok {hwnd}").as_bytes())?;
    } else {
        send(&mut stream, b"ok")?;
    }
    loop {
        let command = receive(&mut reader)?;
        let recognised = if direct { command == "check" } else { matches!(command.as_str(), "frame" | "bmp") };
        if !recognised || !players::still_of(hwnd, target_pid) {
            break;
        }
        // Skip the brief focus/opacity changes of AFK or a macro. Capture itself only reads pixels.
        if input_turn::busy() {
            if !allowed(live) {
                break;
            }
            send(&mut stream, &[])?;
            continue;
        }
        if players::is_minimised(hwnd) || players::is_hidden(hwnd) {
            break;
        }
        if direct {
            if !allowed(live) {
                break;
            }
            send(&mut stream, b"ok")?;
        } else if command == "bmp" {
            let frame = window_shot::monitor_picture(hwnd);
            if !allowed(live) {
                break;
            }
            send_bitmap(&mut stream, frame.filter(|_| !input_turn::busy()))?;
        } else {
            let frame = window_shot::monitor_frame(hwnd);
            if !allowed(live) {
                break;
            }
            let jpeg = frame.filter(|_| !input_turn::busy()).unwrap_or_default();
            send(&mut stream, &jpeg)?;
        }
    }
    drop(stop);
    Ok(())
}

/// A top-down BMP lets the encoder resize the actual client pixels without an intermediate JPEG.
fn send_bitmap(stream: &mut TcpStream, picture: Option<Picture>) -> io::Result<()> {
    const HEADER_SIZE: usize = 54;
    const MAX_FRAME_SIZE: usize = 64 * 1024 * 1024;
    let Some(picture) = picture else { return send(stream, &[]) };
    let pixel_size = std::mem::size_of_val(picture.pixels.as_slice());
    let size = HEADER_SIZE + pixel_size;
    if size > MAX_FRAME_SIZE {
        return send(stream, &[]);
    }

    let mut header = [0u8; HEADER_SIZE];
    header[0..2].copy_from_slice(b"BM");
    header[2..6].copy_from_slice(&(size as u32).to_le_bytes());
    header[10..14].copy_from_slice(&(HEADER_SIZE as u32).to_le_bytes());
    header[14..18].copy_from_slice(&40u32.to_le_bytes());
    header[18..22].copy_from_slice(&picture.width.to_le_bytes());
    header[22..26].copy_from_slice(&(-picture.height).to_le_bytes());
    header[26..28].copy_from_slice(&1u16.to_le_bytes());
    header[28..30].copy_from_slice(&32u16.to_le_bytes());
    header[34..38].copy_from_slice(&(pixel_size as u32).to_le_bytes());

    // SAFETY: the u32 pixel slice is contiguous and initialized, with BGRX layout on little-endian Windows.
    let pixels = unsafe { std::slice::from_raw_parts(picture.pixels.as_ptr().cast::<u8>(), pixel_size) };
    stream.write_all(&(size as u32).to_le_bytes())?;
    stream.write_all(&header)?;
    stream.write_all(pixels)
}

struct Stop;
impl Drop for Stop {
    fn drop(&mut self) {
        input_turn::run_when_free("live-close", || put_back(window_loan::WAIT));
    }
}

/// The viewed window given back (window_loan::back): its place back while it is still the plain window it was made
/// (maximised or made full screen by hand since, that is the person's own doing and stays), minimised again when the
/// view opened it, unless the person has it in front now.
fn put_back(wait: Duration) {
    let target = ACTIVE.lock().unwrap_or_else(|e| e.into_inner()).take();
    if let Some(Target { pid, loan: Some(loan) }) = target {
        let back = loan.give_back_with(None, wait);
        if back.placed_back {
            log::note(&format!("Live view: Roblox window {pid} put back where it stood."));
        }
    }
}

pub(super) fn shutdown() {
    ENDING.store(true, Ordering::Release);
    // Macros have already been stopped by the normal quit path.
    for _ in 0..60 {
        if ACTIVE.lock().unwrap_or_else(|e| e.into_inner()).is_none() {
            break;
        }
        if let Some(_turn) = input_turn::try_enter("live-shutdown") {
            put_back(SHUTDOWN_WAIT);
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}
