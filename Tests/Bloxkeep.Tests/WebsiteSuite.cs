using System.Text;
using Microsoft.Web.WebView2.Core;
using Microsoft.Web.WebView2.WinForms;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// Account menu › Open website, with made-up session cookies (no real account): two windows at once, each with only
/// its own account's cookie, in memory only; the site's sign-out stopped; a renewed cookie read back; everything gone
/// from %TEMP% once the windows close. Needs the WebView2 runtime; www.roblox.com need not be reachable.
/// </summary>
static class WebsiteSuite
{
    public static Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        var done = new TaskCompletionSource();
        var thread = new Thread(() =>
        {
            // WebView2 needs a window thread with a message loop.
            using var host = new Form { ShowInTaskbar = false, StartPosition = FormStartPosition.Manual, Location = new Point(-2000, -2000), Size = new Size(200, 200) };
            host.Load += async (_, _) =>
            {
                try
                {
                    await Body(c, host);
                }
                catch (Exception ex)
                {
                    c.Check("the website checks ran to their end", false, ex);
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

    static object? Static(string field) => typeof(WebsiteForm).GetField(field, Harness.F)!.GetValue(null);

    static async Task<bool> Until(Func<bool> condition, int seconds = 20)
    {
        for (int i = 0; i < seconds * 10 && !condition(); i++)
            await Task.Delay(100);
        return condition();
    }

    static async Task<string?> SessionIn(CoreWebView2 core) =>
        (await core.CookieManager.GetCookiesAsync("https://www.roblox.com")).FirstOrDefault(k => k.Name == ".ROBLOSECURITY")?.Value;

    static async Task Body(Checks c, Form host)
    {
        string mark = Guid.NewGuid().ToString("N")[..10];
        var seed = Vault.Create(Harness.Password);
        var a = new Account { Username = "webA", Cookie = "_TEST_A_" + mark, UserId = 2001, CookieValid = true };
        var b = new Account { Username = "webB", Cookie = "_TEST_B_" + mark, UserId = 2002, CookieValid = true };
        var vng = new Account { Username = "webV", Cookie = "_TEST_V_" + mark, UserId = 2003, CookieValid = true, Realm = Realm.Vng };
        seed.Data.Accounts.AddRange([a, b, vng]);
        seed.Save();
        using var app = new AppController(host, _ => { });
        await Harness.Unlock(app);
        var windows = Harness.Get<Dictionary<Guid, WebsiteForm>>(app, "_websites");

        c.Check("a Roblox VNG account is refused", await Harness.Try(app, "openWebsite", new { id = vng.Id }) is UserError && windows.Count == 0);
        await Harness.Call(app, "openWebsite", new { id = a.Id });
        await Harness.Call(app, "openWebsite", new { id = b.Id });
        await Harness.Call(app, "openWebsite", new { id = a.Id }); // brings the open one to the front
        c.Check("one window per account, two open", windows.Count == 2, windows.Count);
        var formA = windows[a.Id];
        var formB = windows[b.Id];
        var webA = Harness.Get<WebView2>(formA, "_web");
        var webB = Harness.Get<WebView2>(formB, "_web");
        bool ready = await Until(() => webA.CoreWebView2 is not null && webB.CoreWebView2 is not null);
        c.Check("both browsers start", ready);
        if (!ready)
            return;
        var coreA = webA.CoreWebView2;
        var coreB = webB.CoreWebView2;
        var screen = Screen.FromControl(host).WorkingArea;
        c.Check("the windows open on screen, the second a little lower right",
            screen.Contains(formA.Bounds) && screen.Contains(formB.Bounds) && formB.Left > formA.Left && formB.Top > formA.Top, $"{screen} {formA.Bounds} {formB.Bounds}");
        c.Check("the title says which account", formA.Text == "webA — Roblox" && formB.Text == "webB — Roblox", formA.Text + " | " + formB.Text);
        c.Check("each window is InPrivate", coreA.Profile.IsInPrivateModeEnabled && coreB.Profile.IsInPrivateModeEnabled);
        c.Check("each window has a profile of its own", coreA.Profile.ProfileName != coreB.Profile.ProfileName);
        c.Check("the windows share one browser", coreA.BrowserProcessId == coreB.BrowserProcessId);
        string? inA = await SessionIn(coreA), inB = await SessionIn(coreB);
        c.Check("window A holds A's session only", inA == a.Cookie, inA);
        c.Check("window B holds B's session only", inB == b.Cookie, inB);

        // The site's sign-out, as its menu sends it and as a page link; "log out of other sessions" is let through.
        int asked = 0;
        bool closeOnAsk = false;
        Harness.Set(formA, "_askCloseOnLogout", (Func<WebsiteForm, bool>)(_ => { asked++; return closeOnAsk; }));
        coreA.NavigateToString("<html><body>test</body></html>");
        await Task.Delay(500);
        await coreA.ExecuteScriptAsync("fetch('https://auth.roblox.com/v2/logout',{method:'POST',credentials:'include'}).catch(()=>{})");
        c.Check("signing out from the page is stopped and explained", await Until(() => asked == 1, 10), asked);
        c.Check("the session survives it", await SessionIn(coreA) == a.Cookie);
        coreA.Navigate("https://www.roblox.com/logout");
        c.Check("a sign-out link is stopped too", await Until(() => asked == 2, 10), asked);
        await coreA.ExecuteScriptAsync("fetch('https://auth.roblox.com/v2/logoutfromallsessionsandreauthenticate',{method:'POST',credentials:'include'}).catch(()=>{})");
        await Task.Delay(1500);
        c.Check("\"log out of other sessions\" isn't stopped", asked == 2, asked);

        // Roblox renewing the session: the new cookie goes to the app (here a stand-in that only records it).
        var reported = new List<string>();
        typeof(WebsiteForm).GetField("_onSessionChanged", Harness.F)!.SetValue(formA, (Func<string, Task<bool>>)(v => { reported.Add(v); return Task.FromResult(true); }));
        string renewed = "_TEST_A2_" + mark;
        var cookie = coreA.CookieManager.CreateCookie(".ROBLOSECURITY", renewed, ".roblox.com", "/");
        cookie.IsSecure = true;
        cookie.IsHttpOnly = true;
        cookie.Expires = DateTime.Now.AddDays(30); // as Roblox sets it: a lasting cookie, still not written to disk
        coreA.CookieManager.AddOrUpdateCookie(cookie);
        await (Task)Harness.Invoke(formA, "SyncSessionAsync")!;
        await (Task)Harness.Invoke(formA, "SyncSessionAsync")!;
        c.Check("a renewed session is handed to the app once", reported.SequenceEqual([renewed]), string.Join(",", reported));
        c.Check("window B still holds B's session", await SessionIn(coreB) == b.Cookie);

        // Nothing of either session on disk while the windows are open.
        string envDir = (string)Static("_envDir")!;
        await Task.Delay(2000);
        var found = Scan(envDir, [a.Cookie, b.Cookie, renewed]);
        c.Check("no session cookie on disk while open", found.Count == 0, string.Join(", ", found));

        // Closing: A after another stopped sign-out (the person chose "Close window"), B when the vault locks.
        closeOnAsk = true;
        await coreA.ExecuteScriptAsync("fetch('https://auth.roblox.com/v2/logout',{method:'POST',credentials:'include'}).catch(()=>{})");
        c.Check("\"Close window\" closes it", await Until(() => formA.IsDisposed, 10));
        c.Check("the app lets go of a closed window", !windows.ContainsKey(a.Id));
        await Harness.Call(app, "lock", new { });
        c.Check("locking closes the other window", await Until(() => formB.IsDisposed, 10) && windows.Count == 0, windows.Count);
        c.Check("the browser's folder is deleted after the last window", await Until(() => !Directory.Exists(envDir), 30), envDir);
        c.Check("the next window gets a new folder", Static("_envDir") is null && (int)Static("_windows")! == 0);
    }

    static List<string> Scan(string dir, string[] markers)
    {
        var hits = new List<string>();
        foreach (var file in Directory.EnumerateFiles(dir, "*", SearchOption.AllDirectories))
        {
            byte[] data;
            try
            {
                using var s = new FileStream(file, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete);
                data = new byte[s.Length];
                s.ReadExactly(data);
            }
            catch (IOException)
            {
                continue;
            }
            foreach (var m in markers)
            {
                if (data.AsSpan().IndexOf(Encoding.ASCII.GetBytes(m)) >= 0 || data.AsSpan().IndexOf(Encoding.Unicode.GetBytes(m)) >= 0)
                    hits.Add($"{m} in {Path.GetRelativePath(dir, file)}");
            }
        }
        return hits;
    }
}
