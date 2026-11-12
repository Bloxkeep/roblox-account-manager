// Tests/Bloxkeep.Tests/MainBrowserSuite.cs: the switches of the app's own pages' browser (web_view2_setup.rs), when
// Windows counts as short of memory (app/memory.rs, system_memory.rs), the small files written so a power cut can't
// leave them empty (durable_file.rs), and the one rule for a page's browser failing (shell_form.rs). Every check that
// started a browser or made a window (a page shown, put to sleep and woken, the page windows started, let go and
// crashed, a macro panel, "Pick on game") is skipped here, one skip per check.
use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_PROCESS_FAILED_KIND, COREWEBVIEW2_PROCESS_FAILED_KIND_BROWSER_PROCESS_EXITED, COREWEBVIEW2_PROCESS_FAILED_KIND_GPU_PROCESS_EXITED,
    COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED, COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_UNRESPONSIVE,
};

use super::{use_data_folder, Checks};
use crate::core::{
    app::App,
    durable_file, prefs,
    shell_form::{failure_of, BrowserFailure},
    system_memory::{self, MemoryStatus},
    web_view2_setup,
};

const BROWSER: &str = "opens a browser";
const WINDOW: &str = "needs a window";

fn status(total: i64, available: i64, load: i32, limit: i64, free: i64, room: i64) -> MemoryStatus {
    MemoryStatus { total_mb: total, available_mb: available, load_percent: load, commit_limit_mb: limit, commit_available_mb: free, page_file_room_mb: room }
}

/// How much the page files may still grow (system_memory::page_file_room_mb), as the C#'s Room: `made` by drive letter
/// and size now in MB, the drives' free space and size by whether the root is C:.
fn room(files: &[&str], made: Option<&[(char, i64)]>, ram_mb: i64, free_c: i64, free_e: i64, size_c: i64, size_e: i64) -> i64 {
    let files: Vec<String> = files.iter().map(|f| f.to_string()).collect();
    let made: Option<Vec<(String, i64)>> = made.map(|m| m.iter().map(|(d, now)| (format!("{}:\\", d.to_ascii_uppercase()), *now)).collect());
    let is_c = |root: &str| root.to_ascii_lowercase().starts_with('c');
    system_memory::page_file_room_mb(
        &files,
        made.as_ref(),
        ram_mb,
        &|root| if is_c(root) { free_c } else { free_e },
        &|root| if is_c(root) { size_c } else { size_e },
    )
}

/// Room with the C#'s defaults: 32 GB of RAM, 64 GB free on C:, 900 GB on E:, both drives 1 TB.
fn room_d(files: &[&str], made: Option<&[(char, i64)]>) -> i64 {
    room(files, made, 32_768, 64_000, 900_000, 1_000_000, 1_000_000)
}

pub async fn run(c: &mut Checks, dir: &str) {
    use_data_folder(dir);

    // Small files: the new content in place, no temp file left beside it.
    let file = std::path::Path::new(dir).join("durable-check.json");
    let _ = durable_file::write_text(&file, "{\"first\":1}");
    let _ = durable_file::write_text(&file, "{\"second\":2}");
    let text = std::fs::read_to_string(&file).unwrap_or_default();
    c.check("a small file written safely holds the new text", text == "{\"second\":2}", &text);
    let mut tmp = file.as_os_str().to_owned();
    tmp.push(".tmp");
    c.check("…with no temp file left beside it", !std::path::Path::new(&tmp).exists(), ());

    // PORT: the C# read the switches off the options object it handed WebView2; here off main_window_args, their text.
    let args = web_view2_setup::main_window_args();
    c.check(
        "the main window's browser leaves out its background work",
        args.contains("--disable-background-networking") && args.contains("--disable-component-update"),
        &args,
    );
    c.check(
        "…and paints the page on the processor, putting it together on the graphics card (about 100 MB less)",
        args.contains("--disable-gpu-rasterization") && !args.contains("--disable-gpu "),
        &args,
    );

    // Windows short of memory: the browser in the background goes, looked at every half minute; otherwise it stays, asleep.
    let tight = |limit: i64, free: i64, load: i32| App::memory_tight(&status(32_000, 8_000, load, limit, free, 0));
    c.check("memory counts as short when another Roblox (3 GB) and a margin wouldn't fit in the virtual memory left", tight(50_000, 2_500, 60), ());
    c.check("…or when RAM is nearly all used", tight(50_000, 20_000, 93), ());
    c.check("…but not with room to spare", !tight(50_000, 10_000, 60), ());
    c.check("…nor when Windows didn't say how much virtual memory is left", !tight(0, 0, 60), ());
    c.check("…nor when the page file may still grow enough", !App::memory_tight(&status(32_000, 8_000, 60, 51_500, 500, 15_000)), ());

    // How much the page files may still grow: each one Windows made, up to its largest size (the one set, or for a
    // system-managed one 3 × RAM or 4 GB, at most the drive's size ÷ 8), within its drive's free space less 1 GB.
    let set = [r"c:\pagefile.sys 1024 4096", r"e:\pagefile.sys 16348 32768"];
    let both: &[(char, i64)] = &[('c', 1_024), ('e', 17_918)];
    let r = room_d(&set, Some(both));
    c.check("page files set to at most 4 + 32 GB, 18.9 GB now: 17.9 GB still to grow", r == 36_864 - 18_942, r);
    let r = room(&set, Some(both), 32_768, 1_500, 2_000, 1_000_000, 1_000_000);
    c.check("…only as far as the drives' free space allows (less 1 GB each)", r == 476 + 976, r);
    c.check("…none once they're at their largest", room_d(&set, Some(&[('c', 4_096), ('e', 32_768)])) == 0, ());
    c.check("…nor with no page file, or none known", room_d(&[], Some(&[])) == 0 && room_d(&set, None) == 0, ());
    // 2026-10-22: E:'s page file set to 16–32 GB but never made, C:'s at its largest 4 GB, the commit limit full.
    let r = room_d(&set, Some(&[('c', 4_096)]));
    c.check("a page file set but never made by Windows counts for nothing: E: set to 16–32 GB, only C: (4 GB, at its largest) made", r == 0, r);
    let r = room_d(&set, Some(&[('c', 1_024)]));
    c.check("…and C:'s room still counts while it can grow", r == 3_072, r);

    // Users' PCs: Windows sizes the page file by itself, up to 3 × RAM or 4 GB, at most the drive's size ÷ 8. Play warns
    // only when the windows it opens, 3 GB each, wouldn't fit (less a 1 GB margin) in what is left of the commit limit
    // and that room to grow.
    let warns = |ram: i64, room_mb: i64, left: i64, windows: i32| App::virtual_memory_short(&status(ram, 1_000, 80, ram + 1_229, left, room_mb), windows, 3_072);
    let eight_gb = room(&[r"?:\pagefile.sys"], Some(&[('c', 1_229)]), 8_192, 150_000, 900_000, 262_144, 1_000_000);
    c.check("an 8 GB PC, system-managed C: at 1.2 GB on a 256 GB drive: it may grow to 3 × RAM (24 GB)", eight_gb == 24_576 - 1_229, eight_gb);
    c.check("…so opening 3 Roblox windows with its commit nearly all used doesn't warn", !warns(8_192, eight_gb, 300, 3), ());
    let sixteen_gb = room(&[r"c:\pagefile.sys 0 0"], Some(&[('c', 2_560)]), 16_384, 300_000, 900_000, 524_288, 1_000_000);
    c.check("a 16 GB PC, C: system-managed (\"0 0\") at 2.5 GB on a 512 GB drive: room to 48 GB", sixteen_gb == 49_152 - 2_560, sixteen_gb);
    c.check("…opening 6 windows doesn't warn", !warns(16_384, sixteen_gb, 300, 6), ());
    let r = room(&[r"c:\pagefile.sys 0 0"], Some(&[('c', 2_560)]), 16_384, 60_000, 900_000, 131_072, 1_000_000);
    c.check("…on a 128 GB drive it may grow only to its size ÷ 8 (16 GB)", r == 16_384 - 2_560, r);
    let full = room(&[r"?:\pagefile.sys"], Some(&[('c', 1_229)]), 8_192, 1_500, 900_000, 262_144, 1_000_000);
    c.check("still warns when a system-managed page file can't grow: its drive almost full", full == 476 && warns(8_192, full, 300, 2), full);
    let at_most = room(&[r"?:\pagefile.sys"], Some(&[('c', 24_576)]), 8_192, 150_000, 900_000, 262_144, 1_000_000);
    c.check("…or it is already at its largest", at_most == 0 && warns(8_192, at_most, 300, 1), at_most);
    let page_files = system_memory::page_files_now();
    c.check(
        "the page files Windows has, and their size, are read from Windows itself",
        page_files.as_ref().is_some_and(|f| {
            !f.is_empty()
                && f.iter().all(|(k, v)| {
                    let b = k.as_bytes();
                    b.len() == 3 && b[1] == b':' && b[2] == b'\\' && *v > 0
                })
        }),
        format!("{page_files:?}"),
    );
    let now = system_memory::read();
    c.check(
        "this PC's virtual memory is read, with the page files' room to grow",
        now.commit_limit_mb > 0 && now.page_file_room_mb >= 0 && now.commit_room_mb() >= now.commit_available_mb,
        format!("limit {}, left {}, room {}", now.commit_limit_mb, now.commit_available_mb, now.page_file_room_mb),
    );
    // PORT: ShellForm.ReleaseWait let the main window's browser go in the background; the Rust main window (GPUI) has no
    // browser, so there is no wait to check.
    c.skip("in the background the browser stays, asleep, looked at every half minute", "not in the Rust port (the main window has no browser)");
    c.skip("…and goes once Windows is short of memory", "not in the Rust port (the main window has no browser)");

    c.skip("…starts with those switches and shows a page, its scripts running", BROWSER);
    c.skip("into the tray the page hears it is hidden, and sleeps", BROWSER);
    c.skip("…and back on screen it is awake within half a second", BROWSER);

    let was = prefs::lock().safe_rendering;
    prefs::lock().safe_rendering = true;
    let safe = web_view2_setup::main_window_args();
    prefs::lock().safe_rendering = was;
    c.check(
        "drawing on the CPU (safe rendering) keeps them, with its own switches",
        safe.contains("--disable-gpu") && safe.contains("--disable-background-networking"),
        &safe,
    );

    // The small windows drawn by a page: their browser started, let go and failing.
    c.skip("a page's window starts its browser", BROWSER);
    for at in ["at once", "its browser starting", "its page loading"] {
        c.skip(&format!("disposed while starting ({at}): the start ends quietly, nothing thrown"), BROWSER);
    }
    for at in ["at once", "its browser starting", "its page loading"] {
        c.skip(&format!("let go while starting ({at}), then started again: the new start's page stays"), BROWSER);
    }
    c.skip("its GPU process restarted: the window stays shown with its page", BROWSER);
    c.skip("…its page's renderer crashed: hidden, the page let go (started again on its next show)", BROWSER);

    // Every kind WebView2 names (0..=9: browser, renderer, renderer hung, frame renderer, utility, sandbox helper, GPU,
    // PPAPI plugin, PPAPI broker, unknown).
    let table: Vec<(i32, BrowserFailure)> = (0..=9).map(|k| (k, failure_of(COREWEBVIEW2_PROCESS_FAILED_KIND(k)))).collect();
    let expected = |k: i32| {
        let kind = COREWEBVIEW2_PROCESS_FAILED_KIND(k);
        if kind == COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED || kind == COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_UNRESPONSIVE {
            BrowserFailure::ReloadPage
        } else if kind == COREWEBVIEW2_PROCESS_FAILED_KIND_GPU_PROCESS_EXITED {
            BrowserFailure::GpuRestarted
        } else if kind == COREWEBVIEW2_PROCESS_FAILED_KIND_BROWSER_PROCESS_EXITED {
            BrowserFailure::BrowserGone
        } else {
            BrowserFailure::None
        }
    };
    c.check(
        "one rule for every page's window: a renderer gone or hung reloads the page, the browser gone starts again, a GPU restart and the rest are left alone",
        table.iter().all(|(k, v)| *v == expected(*k)),
        format!("{table:?}"),
    );

    c.skip("a macro panel is lowered while a message box shows, as the mini window and the overlay are", WINDOW);
    c.skip("a folded panel hidden by its game ending lets its browser go after the keep time", WINDOW);
    c.skip("\"Pick on game\" ended before its overlay had a window still answers (nothing picked)", WINDOW);
}
