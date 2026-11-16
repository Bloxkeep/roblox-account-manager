//! A temporary, opt-in remote viewer. Keep the link in memory and close the helper's stdin to revoke it.
use std::{
    fs::{self, File},
    io::{self, BufRead, BufReader, Read, Write},
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}, mpsc},
    time::{Duration, Instant},
};

use sha2::{Digest, Sha256};

const NO_WINDOW: u32 = 0x0800_0000;
const NODE_URL: &str = "https://nodejs.org/dist/v24.22.3/win-x64/node.exe";
const NODE_SHA: &str = "86fc4785327db5f5c7558dc6156c543b433c313e813a222e088086222ab4afa6";
const TUNNEL_URL: &str = "https://github.com/cloudflare/cloudflared/releases/download/2026.11.0/cloudflared-windows-amd64.exe";
const TUNNEL_SHA: &str = "d5c1d82031da6253dcdcff64c944cd0fe063016edeb2e4fd088075fc9bd52d0a";
const VIDEO_BUILD: &str = "ffmpeg-2026-11-11-git-23c4540053-essentials_build";
const VIDEO_URL: &str = "https://github.com/GyanD/codexffmpeg/releases/download/2026-11-11-git-23c4540053/ffmpeg-2026-11-11-git-23c4540053-essentials_build.7z";
const ARCHIVE_SHA: &str = "2711e98a2a053574be81047d37af2cc4dcec069fc3f690cd9386f93212fd6d54";
const VIDEO_SHA: &str = "2ff6881a0012e1f59a5add38a1986c157eed4da7b2e5f6442705a55246007190";

#[derive(Clone, PartialEq, Eq)]
pub enum Status {
    Idle,
    Preparing,
    Connecting,
    Verifying,
    Ready(String),
    Stopping,
    Failed,
}

static WATCH: Mutex<(Status, Option<Arc<AtomicBool>>)> = Mutex::new((Status::Idle, None));

fn watch() -> std::sync::MutexGuard<'static, (Status, Option<Arc<AtomicBool>>)> {
    WATCH.lock().unwrap_or_else(|error| error.into_inner())
}

pub fn status() -> Status { watch().0.clone() }

fn script(exe: &Path) -> Option<PathBuf> {
    exe.parent().map(|parent| parent.join("wwwroot/remote-monitor/start.mjs"))
}

pub fn available() -> bool {
    std::env::current_exe().ok().and_then(|exe| script(&exe)).is_some_and(|script| script.is_file())
}

pub fn start(compat: bool) {
    let mut state = watch();
    if state.1.is_some() || !available() { return; }
    let live = Arc::new(AtomicBool::new(true));
    *state = (Status::Preparing, Some(live.clone()));
    if crate::net::Named::new("remote-watch".into()).spawn(move || {
        let result = run(&live, compat);
        let running = live.swap(false, Ordering::AcqRel);
        let mut state = watch();
        if let Err(error) = result && running {
            super::log::note(&format!("Remote viewing: {error}"));
            *state = (Status::Failed, None);
        } else {
            *state = (Status::Idle, None);
        }
    }).is_err() {
        *state = (Status::Failed, None);
    }
}

pub fn stop() {
    let mut state = watch();
    if let Some(live) = state.1.as_ref() {
        live.store(false, Ordering::Release);
        state.0 = Status::Stopping;
    }
}

fn check(live: &AtomicBool) -> io::Result<()> {
    if live.load(Ordering::Acquire) { Ok(()) } else { Err(io::ErrorKind::Interrupted.into()) }
}

fn publish(live: &AtomicBool, status: Status) -> io::Result<()> {
    let mut state = watch();
    check(live)?;
    state.0 = status;
    Ok(())
}

fn valid_link(value: &str) -> bool {
    let Ok(url) = url::Url::parse(value) else { return false };
    let host = url.host_str().and_then(|host| host.strip_suffix(".trycloudflare.com"));
    url.scheme() == "https" && url.port().is_none() && url.username().is_empty() && url.password().is_none()
        && url.path() == "/" && url.query().is_none()
        && host.is_some_and(|host| !host.is_empty() && host.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'))
        && url.fragment().is_some_and(|key| key.len() == 48 && key.bytes().all(|b| b.is_ascii_hexdigit()))
}

fn run(live: &Arc<AtomicBool>, compat: bool) -> io::Result<()> {
    let exe = std::env::current_exe()?;
    let script = script(&exe).ok_or_else(|| io::Error::other("Missing remote viewing files"))?;
    let runtime = super::vault::cache_dir().join("remote-watch");
    fs::create_dir_all(&runtime)?;
    let node = runtime.join("node-24.22.3.exe");
    let tunnel = runtime.join("cloudflared-2026.11.0.exe");
    let ffmpeg = runtime.join("ffmpeg-23c4540053.exe");
    prepare(&node, NODE_URL, NODE_SHA, live)?;
    prepare(&tunnel, TUNNEL_URL, TUNNEL_SHA, live)?;
    prepare_video(&ffmpeg, live)?;
    publish(live, Status::Connecting)?;

    let mut random = [0u8; 24];
    getrandom::fill(&mut random).map_err(|_| io::Error::other("Could not generate a private session key"))?;
    let key: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
    let (video_port, roster_port) = super::live_monitor::listeners(0, 0, key.clone(), live.clone())?;
    let mut child = Command::new(node).arg(script)
        .env_remove("NODE_OPTIONS").env_remove("NODE_PATH")
        .env("BK_MONITOR_KEY", key).env("BK_MONITOR_APP", &exe)
        .env("BK_MONITOR_FFMPEG", ffmpeg).env("BK_MONITOR_TUNNEL", tunnel)
        .env("BK_MONITOR_VIDEO_PORT", video_port.to_string()).env("BK_MONITOR_ROSTER_PORT", roster_port.to_string())
        .env("BK_MONITOR_COMPAT", if compat { "1" } else { "0" })
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).creation_flags(NO_WINDOW).spawn()?;
    let stdout = child.stdout.take().expect("piped output");
    let (tx, rx) = mpsc::channel();
    let drain = crate::net::Named::new("remote-watch-status".into()).spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if tx.send(line).is_err() { break; }
        }
    });
    let result = (|| {
        drain?;
        let limit = Instant::now() + Duration::from_secs(85);
        let mut ready = false;
        while live.load(Ordering::Acquire) {
            if let Some(exit) = child.try_wait()? {
                return if ready && exit.success() { Ok(()) } else { Err(io::Error::other("The temporary HTTPS tunnel stopped")) };
            }
            if !ready && Instant::now() >= limit { return Err(io::Error::other("The HTTPS tunnel did not start in time")); }
            if let Ok(line) = rx.recv_timeout(Duration::from_millis(80)) {
                if let Some(link) = line.strip_prefix("READY ").filter(|link| valid_link(link)) {
                    publish(live, Status::Ready(link.to_owned()))?;
                    ready = true;
                } else if line == "VERIFYING" && !ready {
                    publish(live, Status::Verifying)?;
                } else if line == "FAILED" { return Err(io::Error::other("The temporary HTTPS tunnel could not be reached")); }
            }
        }
        Ok(())
    })();
    drop(child.stdin.take());
    for _ in 0..70 {
        if child.try_wait()?.is_some() { return result; }
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = child.kill();
    let _ = child.wait();
    result
}

fn has_sha(path: &Path, expected: &str) -> bool {
    (|| -> io::Result<bool> {
        let mut file = File::open(path)?;
        let mut hash = Sha256::new();
        io::copy(&mut file, &mut hash)?;
        Ok(format!("{:x}", hash.finalize()) == expected)
    })().unwrap_or(false)
}

fn prepare(path: &Path, url: &str, sha: &str, live: &AtomicBool) -> io::Result<()> {
    check(live)?;
    if has_sha(path, sha) { return Ok(()); }
    let tmp = path.with_extension("download");
    let result = (|| {
        let response = ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(12))
            .timeout_read(Duration::from_secs(16)).build().get(url).call()
            .map_err(|_| io::Error::other("Could not download a remote viewing component"))?;
        let mut body = response.into_reader().take(125 * 1024 * 1024);
        let mut file = File::create(&tmp)?;
        let mut buffer = [0; 64 * 1024];
        let mut hash = Sha256::new();
        loop {
            check(live)?;
            let count = body.read(&mut buffer)?;
            if count == 0 { break; }
            file.write_all(&buffer[..count])?;
            hash.update(&buffer[..count]);
        }
        if format!("{:x}", hash.finalize()) != sha { return Err(io::Error::other("Remote viewing component failed its checksum")); }
        drop(file);
        fs::rename(&tmp, path)
    })();
    let _ = fs::remove_file(tmp);
    result
}

fn prepare_video(path: &Path, live: &AtomicBool) -> io::Result<()> {
    check(live)?;
    if has_sha(path, VIDEO_SHA) { return Ok(()); }
    let archive = path.with_extension("7z");
    let extracted = path.with_extension("extracting");
    let result = (|| {
        prepare(&archive, VIDEO_URL, ARCHIVE_SHA, live)?;
        let system = std::env::var_os("SystemRoot").map(PathBuf::from).ok_or_else(|| io::Error::other("Missing Windows system folder"))?;
        let result = Command::new(system.join("System32/tar.exe"))
            .arg("-xOf").arg(&archive).arg(format!("{VIDEO_BUILD}/bin/ffmpeg.exe"))
            .stdin(Stdio::null()).stdout(File::create(&extracted)?).stderr(Stdio::null()).creation_flags(NO_WINDOW).status()?;
        if !result.success() || !has_sha(&extracted, VIDEO_SHA) { return Err(io::Error::other("The video encoder failed its checksum")); }
        fs::rename(&extracted, path)
    })();
    let _ = fs::remove_file(archive);
    let _ = fs::remove_file(extracted);
    result
}
