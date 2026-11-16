//! Pipes for a local Game Monitor prototype. The caller accepts HTTP and selects the Roblox window.
use std::{
    io::{self, BufRead, Write},
    time::{Duration, Instant},
};

use super::{roblox_processes as players, window_shot};

pub fn run() -> io::Result<()> {
    let mut output = io::stdout().lock();
    let mut minimized = None;
    for line in io::stdin().lock().lines() {
        let line = line?;
        let command: Vec<&str> = line.split_whitespace().collect();
        let body = match command.as_slice() {
            ["list"] => list()?,
            ["frame", pid, started] => window(pid, started).and_then(window_shot::monitor_frame).unwrap_or_default(),
            ["frame-slow", pid, started] => preview(pid, started, &mut minimized),
            _ => Vec::new(),
        };
        // Binary stdout framing keeps images off disk and allows repeated requests to the same process.
        output.write_all(&(body.len() as u32).to_le_bytes())?;
        output.write_all(&body)?;
        output.flush()?;
    }
    Ok(())
}

fn list() -> io::Result<Vec<u8>> {
    let found = players::get_players();
    let pids: Vec<i32> = found.iter().map(|p| p.pid).collect();
    let windows = players::visible_windows(&pids);
    let mut shown: Vec<_> = found.iter().filter(|p| {
        p.created > 0 && windows.get(&p.pid).is_some_and(|list| players::main_window_of(list) != 0)
    }).collect();
    shown.sort_by_key(|p| (p.created, p.pid));
    let values: Vec<_> = shown.iter().map(|p| serde_json::json!({
        "id": format!("{}-{}", p.pid, p.created),
        "label": format!("Roblox · {}", p.pid),
    })).collect();
    Ok(serde_json::to_vec(&values)?)
}

pub(super) fn window(pid: &str, started: &str) -> Option<isize> {
    let pid = pid.parse::<i32>().ok().filter(|p| *p > 0)?;
    let started = started.parse::<i64>().ok().filter(|p| *p > 0)?;
    if !players::get_players().iter().any(|p| p.pid == pid && p.created == started) {
        return None;
    }
    let windows = players::visible_windows(&[pid]);
    let window = players::main_window_of(windows.get(&pid)?);
    if !players::still_of(window, pid) {
        return None;
    }
    Some(window)
}

struct Minimized {
    window: isize,
    pid: String,
    started: String,
    taken: Instant,
    body: Vec<u8>,
}

fn preview(pid: &str, started: &str, minimized: &mut Option<Minimized>) -> Vec<u8> {
    let Some(window) = window(pid, started) else { return Vec::new() };
    if !players::is_minimised(window) {
        *minimized = None;
        let Some(jpeg) = window_shot::monitor_frame(window) else { return Vec::new() };
        let mut body = vec![1];
        body.extend(jpeg);
        return body;
    }
    if let Some(last) = minimized.as_ref().filter(|last| {
        last.window == window && last.pid == pid && last.started == started && last.taken.elapsed() < Duration::from_secs(7)
    }) {
        return last.body.clone();
    }
    let mut body = vec![2];
    let (shot, _) = window_shot::capture(window, 960);
    if let Some(shot) = shot.filter(|shot| !shot.blank && !shot.black) {
        body.extend(shot.jpeg);
    }
    *minimized = Some(Minimized { window, pid: pid.into(), started: started.into(), taken: Instant::now(), body: body.clone() });
    body
}
