//! About how many Roblox windows this PC runs at once (the owner, 2026-11-15: "dự đoán máy này chạy tối đa được bao nhiêu
//! acc"). Measured, not guessed: what this person's own windows take each (RAM, memory Windows has promised them, CPU,
//! GPU), against what the PC has left of each; the one that runs out first sets the number. With no window open, what
//! the windows took last time (kept in capacity.json). Nothing is said before a window has been measured.
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use super::{log, roblox_processes::PlayerProcess, system_memory::MemoryStatus, vault};

const MB: i64 = 1024 * 1024;
/// A window younger than this is still loading its game: it would count as a light one.
const SETTLED_SECS: i64 = 90;
/// RAM left alone for Windows and everything else: this much, or 8 % of the PC's.
const RAM_KEPT_MB: f64 = 1024.;
const COMMIT_KEPT_MB: f64 = 1024.;
/// The CPU and GPU aren't run to the very top: games stutter before that.
const CPU_TOP: f64 = 90.;
const GPU_TOP: f64 = 95.;

/// What one window takes, on average.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Each {
    pub ram_mb: f64,
    pub commit_mb: f64,
    /// Percent of the whole CPU; 0 when not known.
    pub cpu: f64,
    /// Percent of the GPU; 0 when not known.
    pub gpu: f64,
}

/// What the PC has now.
#[derive(Clone, Copy, Debug, Default)]
pub struct Now {
    pub windows: i64,
    pub total_mb: f64,
    pub available_mb: f64,
    /// 0 when Windows doesn't say.
    pub commit_limit_mb: f64,
    pub commit_room_mb: f64,
    /// The whole PC's CPU in use, percent; None when not read yet.
    pub cpu_busy: Option<f64>,
    /// Roblox's windows' share of the GPU, percent; None when Windows doesn't say.
    pub gpu_busy: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fit {
    /// Windows open now plus the ones that still fit.
    pub max: i64,
    /// What runs out first: "ram", "commit", "cpu" or "gpu".
    pub by: &'static str,
}

/// The number, from what a window takes and what is left. None when a window's RAM isn't known.
pub fn estimate(each: &Each, now: &Now) -> Option<Fit> {
    if each.ram_mb < 50. {
        return None;
    }
    let mut more: Vec<(f64, &'static str)> = Vec::new();
    let kept = RAM_KEPT_MB.max(now.total_mb * 0.08);
    more.push(((now.available_mb - kept) / each.ram_mb, "ram"));
    if now.commit_limit_mb > 0. && each.commit_mb >= 50. {
        more.push(((now.commit_room_mb - COMMIT_KEPT_MB) / each.commit_mb, "commit"));
    }
    if let (Some(busy), true) = (now.cpu_busy, each.cpu >= 1.) {
        more.push(((CPU_TOP - busy) / each.cpu, "cpu"));
    }
    if let (Some(busy), true) = (now.gpu_busy, each.gpu >= 1.) {
        more.push(((GPU_TOP - busy) / each.gpu, "gpu"));
    }
    let (least, by) = more.into_iter().min_by(|a, b| a.0.total_cmp(&b.0))?;
    Some(Fit { max: (now.windows + (least.floor() as i64).max(0)).clamp(now.windows.max(1), 200), by })
}

#[derive(Default)]
struct Kept {
    each: Option<Each>,
    read: bool,
    saved: Option<Each>,
    /// GetSystemTimes at the last look: (idle, kernel + user).
    cpu_before: Option<(u64, u64)>,
    /// The whole PC's CPU in use at the last tick, percent.
    busy: Option<f64>,
}

static KEPT: Mutex<Kept> = Mutex::new(Kept { each: None, read: false, saved: None, cpu_before: None, busy: None });

/// The whole PC's CPU in use at the last tick, percent (the Game monitor's load).
pub fn pc_busy() -> Option<f64> {
    KEPT.lock().unwrap_or_else(|e| e.into_inner()).busy
}

fn file() -> std::path::PathBuf {
    vault::data_dir().join("capacity.json")
}

/// The whole PC's CPU in use since the last look, percent.
fn cpu_busy(kept: &mut Kept) -> Option<f64> {
    use windows_sys::Win32::{Foundation::FILETIME, System::Threading::GetSystemTimes};
    let zero = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
    let (mut idle, mut kernel, mut user) = (zero, zero, zero);
    // SAFETY: three FILETIMEs on the stack, filled by Windows.
    if unsafe { GetSystemTimes(&mut idle, &mut kernel, &mut user) } == 0 {
        return None;
    }
    let n = |f: &FILETIME| ((f.dwHighDateTime as u64) << 32) | f.dwLowDateTime as u64;
    // (Kernel time counts the idle time too.)
    let sample = (n(&idle), n(&kernel) + n(&user));
    let before = kept.cpu_before.replace(sample)?;
    let (idle, all) = (sample.0.checked_sub(before.0)?, sample.1.checked_sub(before.1)?);
    if all == 0 {
        return None;
    }
    Some((100. - idle as f64 * 100. / all as f64).clamp(0., 100.))
}

/// For the page, with each metrics tick: { max, by, eachMb } or null. `cpu` and `gpu`: Roblox's windows' share now.
pub fn tick(players: &[PlayerProcess], memory: &MemoryStatus, cpu: Option<i32>, gpu: Option<i32>, now_filetime: i64) -> serde_json::Value {
    let mut kept = KEPT.lock().unwrap_or_else(|e| e.into_inner());
    if !kept.read {
        kept.read = true;
        kept.each = crate::net::saved::<Each>("Capacity", &file());
        kept.saved = kept.each;
    }
    let busy = cpu_busy(&mut kept);
    kept.busy = busy;
    // What the settled windows take each, eased into what is kept (a window changing games doesn't swing the number).
    let settled: Vec<&PlayerProcess> = players.iter().filter(|p| p.created == 0 || (now_filetime - p.created) / 10_000_000 >= SETTLED_SECS).collect();
    if !settled.is_empty() {
        let n = settled.len() as f64;
        let share = n / players.len().max(1) as f64;
        let sample = Each {
            ram_mb: settled.iter().map(|p| p.ram()).sum::<i64>() as f64 / MB as f64 / n,
            commit_mb: settled.iter().map(|p| p.private).sum::<i64>() as f64 / MB as f64 / n,
            cpu: cpu.map_or(0., |c| c as f64 * share / n),
            gpu: gpu.map_or(0., |g| g as f64 * share / n),
        };
        let ease = |old: f64, new: f64| if old <= 0. || new <= 0. { old.max(new) } else { old * 0.8 + new * 0.2 };
        kept.each = Some(match kept.each {
            Some(old) => Each { ram_mb: ease(old.ram_mb, sample.ram_mb), commit_mb: ease(old.commit_mb, sample.commit_mb), cpu: ease(old.cpu, sample.cpu), gpu: ease(old.gpu, sample.gpu) },
            None => sample,
        });
        // Kept for the next run when it moved by a tenth (not on every tick).
        let moved = match (kept.saved, kept.each) {
            (Some(s), Some(e)) => (s.ram_mb - e.ram_mb).abs() > s.ram_mb * 0.1 || (s.cpu - e.cpu).abs() > s.cpu.max(1.) * 0.25,
            (None, Some(_)) => true,
            _ => false,
        };
        if moved && let Some(e) = kept.each {
            kept.saved = Some(e);
            if let Ok(text) = serde_json::to_string(&e) {
                let written = std::fs::create_dir_all(vault::data_dir()).and_then(|_| std::fs::write(file(), text));
                crate::net::wrote("Capacity", &file(), written);
            }
        }
    }
    let Some(each) = kept.each else { return serde_json::Value::Null };
    let now = Now {
        windows: players.len() as i64,
        total_mb: memory.total_mb as f64,
        available_mb: memory.available_mb as f64,
        commit_limit_mb: memory.commit_limit_mb as f64,
        commit_room_mb: memory.commit_room_mb() as f64,
        cpu_busy: busy,
        gpu_busy: gpu.map(|g| g as f64),
    };
    match estimate(&each, &now) {
        Some(fit) => {
            // Said once a change, so a report shows what the number came from.
            // (At most every ten minutes, or when another thing runs out first: the number itself wobbles by one.)
            static SAID: Mutex<Option<(&'static str, std::time::Instant)>> = Mutex::new(None);
            let mut said = SAID.lock().unwrap_or_else(|e| e.into_inner());
            if said.is_none_or(|(by, at)| by != fit.by || at.elapsed().as_secs() >= 600) {
                *said = Some((fit.by, std::time::Instant::now()));
                log::note_file(&format!(
                    "Capacity: about {} window(s) fit ({} first): each takes {:.0} MB RAM, {:.0} MB promised, {:.1} % CPU, {:.1} % GPU; the PC has {:.0} of {:.0} MB RAM free, {:.0} MB left to promise, CPU {} % busy.",
                    fit.max,
                    fit.by,
                    each.ram_mb,
                    each.commit_mb,
                    each.cpu,
                    each.gpu,
                    now.available_mb,
                    now.total_mb,
                    now.commit_room_mb,
                    busy.map_or("?".to_string(), |b| format!("{b:.0}"))
                ));
            }
            serde_json::json!({ "max": fit.max, "by": fit.by, "eachMb": (each.ram_mb / 10.).round() * 10. })
        }
        None => serde_json::Value::Null,
    }
}
