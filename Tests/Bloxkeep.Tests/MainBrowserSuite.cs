using Microsoft.Web.WebView2.Core;
using Microsoft.Web.WebView2.WinForms;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// The main window's browser (WebView2Setup.MainWindowOptions): it leaves out the browser's own background work and
/// painting on the graphics card, still starts with those switches and shows a page with its scripts running, and CPU
/// drawing (safe rendering) keeps them. Also when Windows counts as short of memory (the browser then goes;
/// otherwise, in the background, it sleeps and comes back at once).
/// Also the small files written so a power cut can't leave them empty (DurableFile). Needs the WebView2 runtime.
/// </summary>
static class MainBrowserSuite
{
    static readonly Type Setup = typeof(Prefs).Assembly.GetType("RobloxManager.WebView2Setup")!;
    static readonly Type Durable = typeof(Prefs).Assembly.GetType("RobloxManager.DurableFile")!;

    public static Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);

        // Small files: the new content in place, no temp file left beside it.
        string file = Path.Combine(dir, "durable-check.json");
        void WriteText(string text) => Durable.GetMethod("WriteText", Harness.F)!.Invoke(null, [file, text]);
        WriteText("{\"first\":1}");
        WriteText("{\"second\":2}");
        c.Check("a small file written safely holds the new text", File.ReadAllText(file) == "{\"second\":2}", File.ReadAllText(file));
        c.Check("…with no temp file left beside it", !File.Exists(file + ".tmp"));

        var done = new TaskCompletionSource();
        var thread = new Thread(() =>
        {
            // WebView2 needs a window thread with a message loop.
            using var host = new Form { ShowInTaskbar = false, StartPosition = FormStartPosition.Manual, Location = new Point(-2000, -2000), Size = new Size(300, 200) };
            host.Load += async (_, _) =>
            {
                try
                {
                    await Body(c, host, dir);
                }
                catch (Exception ex)
                {
                    c.Check("the main browser checks ran to their end", false, ex);
                }
                finally
                {
                    host.Close();
                }
            };
            WindowsFormsSynchronizationContext.AutoInstall = true;
            Application.Run(host);
            done.SetResult();
        });
        thread.SetApartmentState(ApartmentState.STA);
        thread.Start();
        return done.Task;
    }

    static CoreWebView2EnvironmentOptions Options() => (CoreWebView2EnvironmentOptions)Setup.GetMethod("MainWindowOptions", Harness.F)!.Invoke(null, null)!;

    static async Task Body(Checks c, Form host, string dir)
    {
        string args = Options().AdditionalBrowserArguments;
        c.Check("the main window's browser leaves out its background work", args.Contains("--disable-background-networking") && args.Contains("--disable-component-update"), args);
        c.Check("…and paints the page on the processor, putting it together on the graphics card (about 100 MB less)",
            args.Contains("--disable-gpu-rasterization") && !args.Contains("--disable-gpu "), args);

        // Windows short of memory: the browser in the background goes, looked at every half minute; otherwise it stays, asleep.
        bool Tight(long commitLimit, long commitFree, int load) =>
            AppController.MemoryTight(new MemoryStatus(32_000, 8_000, load, commitLimit, commitFree));
        c.Check("memory counts as short when another Roblox (3 GB) and a margin wouldn't fit in the virtual memory left", Tight(50_000, 2_500, 60));
        c.Check("…or when RAM is nearly all used", Tight(50_000, 20_000, 93));
        c.Check("…but not with room to spare", !Tight(50_000, 10_000, 60));
        c.Check("…nor when Windows didn't say how much virtual memory is left", !Tight(0, 0, 60));
        c.Check("…nor when the page file may still grow enough",
            !AppController.MemoryTight(new MemoryStatus(32_000, 8_000, 60, 51_500, 500, PageFileRoomMb: 15_000)));

        // How much the page files may still grow: each one Windows made (made: drive letter and size now in MB), up to its
        // largest size (the one set, or for a system-managed one 3 × RAM or 4 GB, at most the drive's size ÷ 8), within
        // its drive's free space less 1 GB.
        long Room(string[] files, (char Drive, long NowMb)[]? made, long ramMb = 32_768, long freeC = 64_000, long freeE = 900_000,
            long sizeC = 1_000_000, long sizeE = 1_000_000) =>
            (long)typeof(SystemMemory).GetMethod("PageFileRoomMb", Harness.F)!.Invoke(null, [files,
                made?.ToDictionary(m => $"{char.ToUpperInvariant(m.Drive)}:\\", m => m.NowMb, StringComparer.OrdinalIgnoreCase), ramMb,
                (Func<string, long>)(root => root.StartsWith("c", StringComparison.OrdinalIgnoreCase) ? freeC : freeE),
                (Func<string, long>)(root => root.StartsWith("c", StringComparison.OrdinalIgnoreCase) ? sizeC : sizeE)])!;
        string[] set = [@"c:\pagefile.sys 1024 4096", @"e:\pagefile.sys 16348 32768"];
        c.Check("page files set to at most 4 + 32 GB, 18.9 GB now: 17.9 GB still to grow",
            Room(set, [('c', 1_024), ('e', 17_918)]) == 36_864 - 18_942, Room(set, [('c', 1_024), ('e', 17_918)]));
        c.Check("…only as far as the drives' free space allows (less 1 GB each)",
            Room(set, [('c', 1_024), ('e', 17_918)], freeC: 1_500, freeE: 2_000) == 476 + 976, Room(set, [('c', 1_024), ('e', 17_918)], freeC: 1_500, freeE: 2_000));
        c.Check("…none once they're at their largest", Room(set, [('c', 4_096), ('e', 32_768)]) == 0);
        c.Check("…nor with no page file, or none known", Room([], []) == 0 && Room(set, null) == 0);
        // 2026-10-22: E:'s page file set to 16–32 GB but never made, C:'s at its largest 4 GB, the commit limit full.
        c.Check("a page file set but never made by Windows counts for nothing: E: set to 16–32 GB, only C: (4 GB, at its largest) made",
            Room(set, [('c', 4_096)]) == 0, Room(set, [('c', 4_096)]));
        c.Check("…and C:'s room still counts while it can grow", Room(set, [('c', 1_024)]) == 3_072, Room(set, [('c', 1_024)]));

        // Users' PCs: Windows sizes the page file by itself (its default), up to 3 × RAM or 4 GB, at most the drive's size
        // ÷ 8. Play warns only when the windows it opens, 3 GB each, wouldn't fit (less a 1 GB margin) in what is left of
        // the commit limit and that room to grow.
        bool Warns(long ramMb, long roomMb, long commitLeftMb, int windows) =>
            (bool)typeof(AppController).GetMethod("VirtualMemoryShort", Harness.F)!.Invoke(null,
                [new MemoryStatus(ramMb, 1_000, 80, ramMb + 1_229, commitLeftMb, roomMb), windows, 3_072L])!;
        long eightGb = Room([@"?:\pagefile.sys"], [('c', 1_229)], ramMb: 8_192, freeC: 150_000, sizeC: 262_144);
        c.Check("an 8 GB PC, system-managed C: at 1.2 GB on a 256 GB drive: it may grow to 3 × RAM (24 GB)", eightGb == 24_576 - 1_229, eightGb);
        c.Check("…so opening 3 Roblox windows with its commit nearly all used doesn't warn", !Warns(8_192, eightGb, 300, 3));
        long sixteenGb = Room([@"c:\pagefile.sys 0 0"], [('c', 2_560)], ramMb: 16_384, freeC: 300_000, sizeC: 524_288);
        c.Check("a 16 GB PC, C: system-managed (\"0 0\") at 2.5 GB on a 512 GB drive: room to 48 GB", sixteenGb == 49_152 - 2_560, sixteenGb);
        c.Check("…opening 6 windows doesn't warn", !Warns(16_384, sixteenGb, 300, 6));
        c.Check("…on a 128 GB drive it may grow only to its size ÷ 8 (16 GB)",
            Room([@"c:\pagefile.sys 0 0"], [('c', 2_560)], ramMb: 16_384, freeC: 60_000, sizeC: 131_072) == 16_384 - 2_560);
        long full = Room([@"?:\pagefile.sys"], [('c', 1_229)], ramMb: 8_192, freeC: 1_500, sizeC: 262_144);
        c.Check("still warns when a system-managed page file can't grow: its drive almost full", full == 476 && Warns(8_192, full, 300, 2), full);
        long atMost = Room([@"?:\pagefile.sys"], [('c', 24_576)], ramMb: 8_192, freeC: 150_000, sizeC: 262_144);
        c.Check("…or it is already at its largest", atMost == 0 && Warns(8_192, atMost, 300, 1), atMost);
        var pageFiles = typeof(SystemMemory).GetMethod("PageFilesNow", Harness.F)!.Invoke(null, null) as Dictionary<string, long>;
        c.Check("the page files Windows has, and their size, are read from Windows itself",
            pageFiles is { Count: > 0 } && pageFiles.All(f => f.Key.Length == 3 && f.Key[1] == ':' && f.Key[2] == '\\' && f.Value > 0),
            pageFiles is null ? "null" : string.Join(", ", pageFiles.Select(f => $"{f.Key} {f.Value} MB")));
        var now = SystemMemory.Read();
        c.Check("this PC's virtual memory is read, with the page files' room to grow", now.CommitLimitMb > 0 && now.PageFileRoomMb >= 0 && now.CommitRoomMb >= now.CommitAvailableMb,
            $"limit {now.CommitLimitMb}, left {now.CommitAvailableMb}, room {now.PageFileRoomMb}");
        var wait = typeof(Prefs).Assembly.GetType("RobloxManager.ShellForm")!.GetMethod("ReleaseWait", Harness.F)!;
        int Wait(long hiddenMs, long commitFree) => (int)wait.Invoke(null, [hiddenMs, new MemoryStatus(32_000, 8_000, 60, 50_000, commitFree)])!;
        c.Check("in the background the browser stays, asleep, looked at every half minute", Wait(30_000, 10_000) == 30_000 && Wait(3_600_000, 10_000) == 30_000,
            $"{Wait(30_000, 10_000)} {Wait(3_600_000, 10_000)}");
        c.Check("…and goes once Windows is short of memory", Wait(30_000, 2_500) == 0);

        var env = await CoreWebView2Environment.CreateAsync(null, Path.Combine(dir, "webview"), Options());
        using var web = new WebView2 { Dock = DockStyle.Fill };
        host.Controls.Add(web);
        await web.EnsureCoreWebView2Async(env);
        var loaded = new TaskCompletionSource<bool>();
        web.CoreWebView2.NavigationCompleted += (_, e) => loaded.TrySetResult(e.IsSuccess);
        web.CoreWebView2.NavigateToString("<!doctype html><p id=x>Bloxkeep</p><script>document.title = 'ok:' + (1 + 1)</script>");
        bool ok = await loaded.Task.WaitAsync(TimeSpan.FromSeconds(30));
        string text = await web.CoreWebView2.ExecuteScriptAsync("document.getElementById('x').textContent + '|' + document.title");
        c.Check("…starts with those switches and shows a page, its scripts running", ok && text == "\"Bloxkeep|ok:2\"", text);

        // Into the tray the way the main window goes (ShellForm.SetVisibleCore): the browser hidden while its window still
        // shows hears it, so its page can sleep, and back on screen it is awake at once (made again, as it was after
        // 5 minutes before, it took about 2.5 s). Hidden after its window, it never heard it was: Tests/perf/TrayBench.
        string shownBefore = await web.CoreWebView2.ExecuteScriptAsync("document.visibilityState");
        web.Visible = false;
        host.Hide();
        await Task.Delay(300);
        string hidden = await web.CoreWebView2.ExecuteScriptAsync("document.visibilityState");
        bool asleep = await web.CoreWebView2.TrySuspendAsync();
        c.Check("into the tray the page hears it is hidden, and sleeps", shownBefore == "\"visible\"" && hidden == "\"hidden\"" && asleep && web.CoreWebView2.IsSuspended,
            $"{shownBefore} {hidden} {asleep}");
        var watch = System.Diagnostics.Stopwatch.StartNew();
        host.Show();
        web.Visible = true;
        string back = "";
        while (watch.ElapsedMilliseconds < 5000 && (back = await web.CoreWebView2.ExecuteScriptAsync("document.visibilityState")) != "\"visible\"")
            await Task.Delay(5);
        long backMs = watch.ElapsedMilliseconds;
        c.Check("…and back on screen it is awake within half a second", back == "\"visible\"" && !web.CoreWebView2.IsSuspended && backMs < 500, $"{back} after {backMs} ms");

        bool was = Prefs.SafeRendering;
        Prefs.SafeRendering = true;
        string safe = Options().AdditionalBrowserArguments;
        Prefs.SafeRendering = was;
        c.Check("drawing on the CPU (safe rendering) keeps them, with its own switches", safe.Contains("--disable-gpu") && safe.Contains("--disable-background-networking"), safe);
    }
}
