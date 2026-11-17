// How hard the window's thread works, in error.log: every 10 minutes that had any frame, one line with the frames
// drawn and the frames only presented again (GPUI presents every display refresh for a second after fast mouse input,
// 240 a second on a 240 Hz screen), the busiest second, and the UI thread's CPU time over those minutes. Measured
// 2026-11-17 on the owner's PC: the UI thread used ~30 % of a core while they used the app, ~1 % idle; this line says
// how many frames that was before anything is capped.
use std::time::Duration;

use gpui_kit::App;

const EVERY: Duration = Duration::from_secs(600);

/// Starts the count's timer on the UI thread (its CPU time is the UI thread's).
pub fn start(cx: &mut App) {
    let _ = gpui_kit::take_frame_tally();
    let mut cpu_before = ui_thread_cpu();
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(EVERY).await;
            let t = gpui_kit::take_frame_tally();
            let cpu = ui_thread_cpu();
            let used = cpu.saturating_sub(cpu_before);
            cpu_before = cpu;
            if t.busy_secs == 0 {
                continue;
            }
            let busy = t.busy_secs as f64;
            let avg_ms = if t.draws > 0 { t.draw_time.as_secs_f64() * 1000. / t.draws as f64 } else { 0. };
            write(&format!(
                "UI frames, last {} min: busy {} s; drawn {} ({:.0}/s while busy, {:.1} ms each); shown again with nothing redrawn {} ({:.0}/s while busy); most in one second {}; UI thread CPU {:.1} s.",
                EVERY.as_secs() / 60,
                t.busy_secs,
                t.draws,
                t.draws as f64 / busy,
                avg_ms,
                t.presents,
                t.presents as f64 / busy,
                t.most_in_a_sec,
                used.as_secs_f64(),
            ));
        }
    })
    .detach();
}

fn write(line: &str) {
    #[cfg(feature = "core")]
    crate::core::log::note_file(line);
    #[cfg(not(feature = "core"))]
    eprintln!("{line}");
}

/// This thread's CPU time so far (kernel + user); zero where Windows won't say.
#[cfg(windows)]
fn ui_thread_cpu() -> Duration {
    use windows_sys::Win32::{
        Foundation::FILETIME,
        System::Threading::{GetCurrentThread, GetThreadTimes},
    };
    let zero = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
    let (mut created, mut ended, mut kernel, mut user) = (zero, zero, zero, zero);
    // SAFETY: GetCurrentThread is a pseudo handle needing no close; the four FILETIMEs are this frame's own.
    if unsafe { GetThreadTimes(GetCurrentThread(), &mut created, &mut ended, &mut kernel, &mut user) } == 0 {
        return Duration::ZERO;
    }
    let ticks = |f: FILETIME| ((f.dwHighDateTime as u64) << 32) | f.dwLowDateTime as u64;
    Duration::from_nanos((ticks(kernel) + ticks(user)).saturating_mul(100))
}

#[cfg(not(windows))]
fn ui_thread_cpu() -> Duration {
    Duration::ZERO
}
