using System.Diagnostics;
using System.Text.Json;
using Microsoft.Web.WebView2.Core;
using Microsoft.Web.WebView2.WinForms;

// Coming back from the tray, timed in a WebView2 of its own with Bloxkeep's real page (ui/dev/preview.html: the page and
// the fake app, answering at once). After a while hidden, how long from showing the window until the page is drawn
// again, and how much memory the browser holds while hidden, three ways:
//   low       hidden with MemoryUsageTargetLevel Low while the app keeps sending states (Bloxkeep before this change)
//   suspend   hidden and suspended (TrySuspendAsync), nothing sent meanwhile, one state on showing
//   recreate  the browser closed while hidden and made again on showing (Bloxkeep after 5 minutes in the tray)
//   dotnet run --project Tests/perf/TrayBench -- <repo> <seconds hidden> <rounds> <out.json>
// A small window shows in a corner for each round; it never takes the keyboard (ShowWithoutActivation).
static class Program
{
    [STAThread]
    static void Main(string[] args)
    {
        ApplicationConfiguration.Initialize();
        string repo = args.Length > 0 ? args[0] : Path.GetFullPath(Path.Combine(AppContext.BaseDirectory, "..", "..", "..", "..", "..", ".."));
        int hidden = args.Length > 1 ? int.Parse(args[1]) : 30, rounds = args.Length > 2 ? int.Parse(args[2]) : 3;
        string output = args.Length > 3 ? args[3] : Path.Combine(Path.GetTempPath(), "traybench.json");
        Application.Run(new Bench(repo, hidden, rounds, output));
    }
}

sealed class Bench : Form
{
    const string Url = "https://bench.local/ui/dev/preview.html?lang=en&accounts=50&nodelay=1";
    // The page's own: drawn after showing (two frames after visible), and drawn the first time with the accounts.
    const string Hooks = """
        (() => {
          const wv = window.chrome.webview, post = m => wv.postMessage(m);
          Object.defineProperty(window, 'chrome', { value: {}, configurable: true }); // the page then uses the fake app
          window.__benchPost = post;
          const ready = () => document.querySelector('#grid [role=option]')
            ? requestAnimationFrame(() => requestAnimationFrame(() => post('ready'))) : setTimeout(ready, 10);
          document.addEventListener('DOMContentLoaded', ready);
        })();
        """;
    readonly string _repo, _output, _data = Path.Combine(Path.GetTempPath(), "bloxkeep-traybench-" + Environment.ProcessId);
    readonly int _hidden, _rounds;
    WebView2? _web;
    TaskCompletionSource<string>? _signal;

    public Bench(string repo, int hidden, int rounds, string output)
    {
        (_repo, _hidden, _rounds, _output) = (repo, hidden, rounds, output);
        Text = "Bloxkeep tray bench";
        StartPosition = FormStartPosition.Manual;
        var screen = Screen.PrimaryScreen!.WorkingArea;
        Bounds = new Rectangle(screen.Right - 660, screen.Bottom - 440, 640, 420);
        ShowInTaskbar = false;
        Shown += async (_, _) => await RunAsync();
    }

    protected override bool ShowWithoutActivation => true;

    async Task<double> CreateAsync()
    {
        var clock = Stopwatch.StartNew();
        var env = await CoreWebView2Environment.CreateAsync(null, _data,
            new CoreWebView2EnvironmentOptions("--disable-features=CalculateNativeWinOcclusion"));
        _web = new WebView2 { Dock = DockStyle.Fill };
        Controls.Add(_web);
        await _web.EnsureCoreWebView2Async(env);
        var core = _web.CoreWebView2;
        core.SetVirtualHostNameToFolderMapping("bench.local", _repo, CoreWebView2HostResourceAccessKind.Allow);
        await core.AddScriptToExecuteOnDocumentCreatedAsync(Hooks);
        core.WebMessageReceived += (_, e) => _signal?.TrySetResult(e.TryGetWebMessageAsString());
        var ready = Expect();
        core.Navigate(Url);
        await Wait(ready, "ready");
        return clock.Elapsed.TotalMilliseconds;
    }

    Task<string> Expect() => (_signal = new TaskCompletionSource<string>(TaskCreationOptions.RunContinuationsAsynchronously)).Task;

    static async Task Wait(Task<string> signal, string want)
    {
        var done = await Task.WhenAny(signal, Task.Delay(20000));
        if (done != signal || signal.Result != want)
            throw new TimeoutException($"no '{want}' from the page");
    }

    /// <summary>Working set and private bytes of every process this browser runs, in MB.</summary>
    object Memory()
    {
        double ws = 0, priv = 0;
        foreach (var info in _web!.CoreWebView2.Environment.GetProcessInfos())
        {
            try
            {
                using var p = Process.GetProcessById(info.ProcessId);
                ws += p.WorkingSet64;
                priv += p.PrivateMemorySize64;
            }
            catch (ArgumentException)
            {
            }
        }
        return new { ws = Math.Round(ws / 1048576), privateMb = Math.Round(priv / 1048576) };
    }

    async Task<object> RoundAsync(string mode)
    {
        var core = _web!.CoreWebView2;
        var awake = Memory();
        // Bloxkeep hides its window first and its browser after (OnVisibleChanged); "first" hides the browser first.
        if (mode is "low")
        {
            Hide();
            _web.Visible = false;
        }
        else
        {
            _web.Visible = false;
            Hide();
        }
        if (mode is "low" or "low-first")
            core.MemoryUsageTargetLevel = CoreWebView2MemoryUsageTargetLevel.Low;
        // The control tells the browser it is hidden on the next turn of the message loop; suspending before that is refused.
        await Task.Delay(300);
        bool suspended = mode == "suspend" && await core.TrySuspendAsync();
        object asleep = new { };
        if (mode == "recreate")
        {
            await Task.Delay(1000);
            Controls.Remove(_web);
            _web.Dispose();
            _web = null;
            GC.Collect();
        }
        var until = DateTime.UtcNow.AddSeconds(_hidden);
        for (int i = 0; DateTime.UtcNow < until; i++)
        {
            // Bloxkeep's app sends a state now and then (a window's RAM, a check): only to a page that is awake.
            if (mode is "low" or "low-first")
                await core.ExecuteScriptAsync($"window.__mockBridge.call('setSettings', {{ autoLockMinutes: {(i % 2 == 0 ? 5 : 0)} }})");
            await Task.Delay(1000);
            if (mode != "recreate" && DateTime.UtcNow > until.AddSeconds(-2))
                asleep = Memory();
        }
        // What the page believes while hidden (asking a suspended page would wake it: its suspension is asked instead).
        string seen = mode == "recreate" ? "-" : mode == "suspend" ? (core.IsSuspended ? "suspended" : "awake")
            : await core.ExecuteScriptAsync("document.visibilityState");
        var clock = Stopwatch.StartNew();
        double back;
        if (mode == "recreate")
        {
            Show();
            back = await CreateAsync();
        }
        else
        {
            var painted = Expect();
            if (mode is "low" or "low-first")
                core.MemoryUsageTargetLevel = CoreWebView2MemoryUsageTargetLevel.Normal;
            Show();
            _web.Visible = true;
            if (mode == "suspend")
                await core.ExecuteScriptAsync("window.__mockBridge.call('setSettings', { autoLockMinutes: 0 })"); // the one state on showing
            // Drawn again: two frames from the page once it runs again after showing.
            _ = core.ExecuteScriptAsync("requestAnimationFrame(() => requestAnimationFrame(() => window.__benchPost('painted')))");
            await Wait(painted, "painted");
            back = clock.Elapsed.TotalMilliseconds;
        }
        return new { mode, suspended, pageSaw = seen, backMs = Math.Round(back, 1), awakeMb = awake, hiddenMb = asleep };
    }

    async Task RunAsync()
    {
        var results = new List<object>();
        try
        {
            results.Add(new { mode = "first open", backMs = Math.Round(await CreateAsync(), 1) });
            await Task.Delay(3000);
            foreach (string mode in new[] { "low", "low-first", "suspend", "recreate" })
            {
                for (int r = 0; r < _rounds; r++)
                {
                    results.Add(await RoundAsync(mode));
                    await Task.Delay(2000);
                }
            }
        }
        catch (Exception ex)
        {
            results.Add(new { error = ex.ToString() });
        }
        File.WriteAllText(_output, JsonSerializer.Serialize(results, new JsonSerializerOptions { WriteIndented = true }));
        Close();
    }
}
