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

use super::{app::App, input_turn, log, monitor_frames, roblox_processes as players, rt, win_kit::Picture, window_shot};

const PORT: u16 = 8790;
const ROSTER_PORT: u16 = 8793;
const MAX_IDLE: Duration = Duration::from_secs(15);
static ENDING: AtomicBool = AtomicBool::new(false);
static ACTIVE: Mutex<Option<Target>> = Mutex::new(None);

#[derive(Clone, Copy)]
struct Target {
    pid: i32,
    hwnd: isize,
    opened: bool,
}

pub fn watching(pid: i32) -> bool {
    ACTIVE.lock().unwrap_or_else(|e| e.into_inner()).is_some_and(|target| target.pid == pid)
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
    crate::net::Named::new(name.into()).spawn(move || {
        while listening(&live) {
            match listener.accept() {
                Ok((stream, _)) => {
                    if stream.set_nonblocking(false).is_ok() {
                        let _ = handler(stream, &key, &live);
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(40)),
                Err(_) => break,
            }
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
            let _ = tx.send(serde_json::json!({ "state": "ready", "labels": labels }));
        } else {
            let _ = tx.send(serde_json::json!({ "state": "locked" }));
        }
    }));
    if let Ok(roster) = rx.recv_timeout(Duration::from_millis(900)) && listening(live) {
        send(&mut stream, &serde_json::to_vec(&roster)?)?;
    }
    Ok(())
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
    let direct = line.strip_suffix(" direct");
    let Some((pid, created)) = direct.unwrap_or(&line).split_once(' ') else { return send(&mut stream, b"window") };
    let direct = direct.is_some();
    if !allowed(live) {
        return send(&mut stream, b"locked");
    }
    let Some(turn) = input_turn::try_enter("live-open") else { return send(&mut stream, b"busy") };
    if !allowed(live) {
        return send(&mut stream, b"locked");
    }
    let (Ok(pid_number), Some(hwnd)) = (pid.parse(), monitor_frames::window(pid, created)) else {
        return send(&mut stream, b"window");
    };
    let target = Target { pid: pid_number, hwnd, opened: players::is_minimised(hwnd) };
    {
        let mut current = ACTIVE.lock().unwrap_or_else(|e| e.into_inner());
        if current.is_some() {
            return send(&mut stream, b"busy");
        }
        *current = Some(target);
    }
    let stop = Stop;
    if target.opened {
        players::restore_quietly(hwnd);
        for _ in 0..10 {
            if !players::is_minimised(hwnd) {
                break;
            }
            std::thread::sleep(Duration::from_millis(30));
        }
    }
    drop(turn);
    if players::is_minimised(hwnd) || !players::still_of(hwnd, target.pid) {
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
        if !recognised || !players::still_of(hwnd, target.pid) {
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
        input_turn::run_when_free("live-close", put_back);
    }
}

fn put_back() {
    let target = ACTIVE.lock().unwrap_or_else(|e| e.into_inner()).take();
    if let Some(target) = target.filter(|target| target.opened) {
        if players::still_of(target.hwnd, target.pid) && !players::is_hidden(target.hwnd) {
            players::minimise(target.hwnd);
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
            put_back();
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}
