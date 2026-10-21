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

        // How much the page files may still grow: up to each one's largest size, within its drive's free space less 1 GB,
        // counting only the page files Windows made (made: the drives with one, "ce" for both).
        long Room(string[] files, long nowMb, long freeC = 64_000, long freeE = 900_000, string made = "ce") =>
            (long)typeof(SystemMemory).GetMethod("PageFileRoomMb", Harness.F)!.Invoke(null, [files, nowMb,
                (Func<string, long>)(root => root.StartsWith("c", StringComparison.OrdinalIgnoreCase) ? freeC : freeE),
                (Func<string, bool>)(root => made.Contains(char.ToLowerInvariant(root[0])))])!;
        string[] set = [@"c:\pagefile.sys 1024 4096", @"e:\pagefile.sys 16348 32768"];
        c.Check("page files set to at most 4 + 32 GB, 18.9 GB now: 17.9 GB still to grow", Room(set, 18_942) == 36_864 - 18_942, Room(set, 18_942));
        c.Check("…only as far as the drives' free space allows (less 1 GB each)", Room(set, 18_942, 1_500, 2_000) == 476 + 976, Room(set, 18_942, 1_500, 2_000));
        c.Check("…none once they're at their largest", Room(set, 36_864) == 0);
        c.Check("a page file Windows sizes by itself counts for nothing (its limits aren't known)",
            Room([@"c:\pagefile.sys 0 0"], 4_000) == 0 && Room([@"?:\pagefile.sys"], 4_000) == 0 && Room([@"c:\pagefile.sys 1024 4096", @"e:\pagefile.sys"], 4_000) == 0);
        c.Check("…nor does no page file at all", Room([], 0) == 0);
        // 2026-10-22: E:'s page file set to 16–32 GB but never made, C:'s at its largest 4 GB, the commit limit full.
        c.Check("a page file set but never made by Windows counts for nothing: E: set to 16–32 GB, only C: (4 GB, at its largest) made",
            Room(set, 4_096, made: "c") == 0, Room(set, 4_096, made: "c"));
        c.Check("…and C:'s room still counts while it can grow", Room(set, 1_024, made: "c") == 3_072, Room(set, 1_024, made: "c"));
        var drives = typeof(SystemMemory).GetMethod("PageFileDrives", Harness.F)!.Invoke(null, null) as HashSet<string>;
        c.Check("the drives Windows made a page file on are read from Windows itself", drives is { Count: > 0 } && drives.All(d => d.Length == 3 && d[1] == ':' && d[2] == '\\'),
            drives is null ? "null" : string.Join(",", drives));
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
