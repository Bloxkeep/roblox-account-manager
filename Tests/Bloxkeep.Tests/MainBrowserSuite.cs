using Microsoft.Web.WebView2.Core;
using Microsoft.Web.WebView2.WinForms;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// The main window's browser (WebView2Setup.MainWindowOptions): it leaves out the browser's own background work and
/// painting on the graphics card, still starts with those switches and shows a page with its scripts running, and CPU
/// drawing (safe rendering) keeps them.
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

        bool was = Prefs.SafeRendering;
        Prefs.SafeRendering = true;
        string safe = Options().AdditionalBrowserArguments;
        Prefs.SafeRendering = was;
        c.Check("drawing on the CPU (safe rendering) keeps them, with its own switches", safe.Contains("--disable-gpu") && safe.Contains("--disable-background-networking"), safe);
    }
}
