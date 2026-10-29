using System.Net;
using System.Text;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// A new Bloxkeep found within minutes (AppController.Latest.cs): the bot's /latest (a little server of the test's own
/// here) telling of a newer version brings the hourly update check forward to now, once per version; an older or the
/// same version, or one it can't read, changes nothing. Betas: a release is newer than its own beta, a beta is never
/// taken from /latest.
/// </summary>
static class LatestSuite
{
    public static async Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        var newer = typeof(AppController).GetMethod("IsNewerVersion", Harness.F)!;
        bool Newer(string latest, string current) => (bool)newer.Invoke(null, [latest, current])!;
        c.Check("1.0.13 is newer than 1.0.12", Newer("1.0.13", "1.0.12") && Newer("v1.1.0", "1.0.99"));
        c.Check("…1.0.12 than its own beta", Newer("1.0.12", "1.0.12-beta.3"));
        c.Check("…not the same, an older one, a beta, or something unreadable",
            !Newer("1.0.12", "1.0.12") && !Newer("1.0.11", "1.0.12") && !Newer("1.0.13-beta.1", "1.0.12") && !Newer("soon", "1.0.12") && !Newer("1.0.13", ""));

        string version = "9.9.9";
        var listener = new HttpListener();
        int port = 48961;
        for (; ; port++)
        {
            listener.Prefixes.Clear();
            listener.Prefixes.Add($"http://127.0.0.1:{port}/latest/");
            try
            {
                listener.Start();
                break;
            }
            catch (HttpListenerException) when (port < 49000)
            {
            }
        }
        int asked = 0;
        _ = Task.Run(async () =>
        {
            while (listener.IsListening)
            {
                HttpListenerContext ctx;
                try { ctx = await listener.GetContextAsync(); }
                catch (Exception) { break; }
                Interlocked.Increment(ref asked);
                byte[] body = Encoding.UTF8.GetBytes($"{{\"version\":\"{version}\"}}");
                ctx.Response.ContentType = "application/json";
                await ctx.Response.OutputStream.WriteAsync(body);
                ctx.Response.Close();
            }
        });
        Environment.SetEnvironmentVariable("BLOXKEEP_LATEST_URL", $"http://127.0.0.1:{port}/latest/");
        using var form = new Form();
        using var app = new AppController(form, _ => { });
        var later = DateTime.UtcNow.AddMinutes(50);
        async Task Look()
        {
            int before = asked;
            Harness.Set(app, "_nextLatestLook", DateTime.MinValue);
            Harness.Invoke(app, "TickLatest", DateTime.UtcNow);
            for (int i = 0; i < 100 && (asked == before || Harness.Get<bool>(app, "_latestLooking")); i++)
                await Task.Delay(30);
        }
        DateTime Next() => Harness.Get<DateTime>(app, "_nextUpdateCheck");

        Harness.Set(app, "_nextUpdateCheck", later);
        await Look();
        c.Check("the bot telling of a newer version: the update check is now", Next() <= DateTime.UtcNow && asked == 1, $"{Next():O} asked {asked}");
        c.Check("…and asked again only after a few minutes", Harness.Get<DateTime>(app, "_nextLatestLook") > DateTime.UtcNow.AddMinutes(4));
        Harness.Set(app, "_nextUpdateCheck", later);
        await Look();
        c.Check("…the same version told again: the hourly check stays where it is (once per version)", Next() == later && asked == 2, Next().ToString("O"));
        version = "0.0.1";
        await Look();
        c.Check("an older version: nothing changes", Next() == later && asked == 3);
        version = "9.9.10";
        await Look();
        c.Check("another newer one later: the check is now again", Next() <= DateTime.UtcNow);
        Prefs.CheckUpdates = false;
        Harness.Set(app, "_nextUpdateCheck", later);
        int was = asked;
        Harness.Set(app, "_nextLatestLook", DateTime.MinValue);
        Harness.Invoke(app, "TickLatest", DateTime.UtcNow);
        await Task.Delay(300);
        c.Check("automatic updates off: the bot isn't asked", asked == was && Next() == later);
        listener.Close();
    }
}
