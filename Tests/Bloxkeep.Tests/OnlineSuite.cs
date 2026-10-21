using System.Net;
using System.Text;
using System.Text.Json;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// Add me to the online count (AppController.Online.cs): a development copy never counts; an installed one says it's open
/// with a random id, its version and language and nothing else, locked or not, as often as the bot says (held between a
/// minute and six hours), tries a failed beat again sooner but never more often than usual, says goodbye as it closes,
/// and turned off says goodbye, forgets the id and sends nothing more. Sent to a little server of the test's own, never
/// the bot.
/// </summary>
static class OnlineSuite
{
    public static async Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        using var form = new Form();
        using var app = new AppController(form, _ => { });
        c.Check("on until turned off", Prefs.CountOnline);

        // This copy isn't installed and has no server of its own: nothing is sent, not even an id made.
        Harness.Invoke(app, "TickOnline", DateTime.UtcNow);
        c.Check("a development copy never counts", !Harness.Get<bool>(app, "_onlineSending") && Prefs.OnlineId is null
            && Harness.Get<DateTime>(app, "_nextOnline") == default);

        // The bot: a little server here that answers what each step needs.
        var (listener, url) = Listen();
        Environment.SetEnvironmentVariable("BLOXKEEP_ONLINE_URL", url);
        var received = new List<JsonElement>();
        int status = 200;
        string answer = """{"next":120}""";
        var serving = Task.Run(async () =>
        {
            while (listener.IsListening)
            {
                HttpListenerContext ctx;
                try { ctx = await listener.GetContextAsync(); }
                catch (Exception) { break; }
                using (var reader = new StreamReader(ctx.Request.InputStream))
                {
                    var body = JsonDocument.Parse(await reader.ReadToEndAsync()).RootElement.Clone();
                    lock (received)
                        received.Add(body);
                }
                ctx.Response.StatusCode = status;
                byte[] bytes = Encoding.UTF8.GetBytes(answer);
                ctx.Response.ContentType = "application/json";
                await ctx.Response.OutputStream.WriteAsync(bytes);
                ctx.Response.Close();
            }
        });
        int Count() { lock (received) return received.Count; }
        JsonElement Last() { lock (received) return received[^1]; }
        async Task<bool> Beat(DateTime at, int expected)
        {
            Harness.Invoke(app, "TickOnline", at);
            for (int i = 0; i < 100 && (Count() < expected || Harness.Get<bool>(app, "_onlineSending")); i++)
                await Task.Delay(50);
            return Count() == expected && !Harness.Get<bool>(app, "_onlineSending");
        }
        DateTime Next() => Harness.Get<DateTime>(app, "_nextOnline");
        bool Near(DateTime at, TimeSpan from) => Math.Abs((at - (DateTime.UtcNow + from)).TotalSeconds) < 10;

        // Locked (the vault isn't open): a beat all the same.
        c.Check("locked or not: a beat", await Beat(DateTime.UtcNow, 1), Count());
        var first = Last();
        string id = Prefs.OnlineId ?? "";
        var names = first.EnumerateObject().Select(p => p.Name).Order().ToArray();
        c.Check("…with a random id, the version and the language, nothing else", string.Join(",", names) == "app,id,language"
            && Guid.TryParseExact(first.GetProperty("id").GetString(), "N", out _) && first.GetProperty("id").GetString() == id
            && first.GetProperty("app").GetString() == Harness.Get<AppUpdater>(app, "_updater").CurrentVersion
            && first.GetProperty("language").GetString() is "en" or "vi", first);
        string prefs = File.ReadAllText(Path.Combine(dir, "prefs.json"));
        c.Check("…the id kept in prefs.json, outside the vault", prefs.Contains($"\"onlineId\":\"{id}\""), prefs);
        c.Check("…and the next beat when the bot says (2 minutes here)", Near(Next(), TimeSpan.FromMinutes(2)), Next());

        c.Check("nothing before then", await Beat(DateTime.UtcNow, 1) && Count() == 1, Count());
        answer = """{"next":86400}""";
        c.Check("then the same id again", await Beat(DateTime.UtcNow.AddMinutes(3), 2) && Last().GetProperty("id").GetString() == id, Last());
        c.Check("…and a gap longer than six hours is held to six", Near(Next(), TimeSpan.FromHours(6)), Next());
        answer = """{"next":5}""";
        await Beat(DateTime.UtcNow.AddHours(7), 3);
        c.Check("…one shorter than a minute to a minute", Near(Next(), TimeSpan.FromMinutes(1)), Next());

        // The bot can't answer: tried again in two minutes, then four… never later than the usual gap.
        answer = """{"next":600}""";
        await Beat(DateTime.UtcNow.AddHours(8), 4);
        status = 503;
        await Beat(DateTime.UtcNow.AddHours(9), 5);
        c.Check("a beat the bot can't take: tried again in two minutes", Near(Next(), TimeSpan.FromMinutes(2)), Next());
        await Beat(DateTime.UtcNow.AddHours(10), 6);
        c.Check("…then four", Near(Next(), TimeSpan.FromMinutes(4)), Next());
        await Beat(DateTime.UtcNow.AddHours(11), 7);
        await Beat(DateTime.UtcNow.AddHours(12), 8);
        c.Check("…and never later than the usual gap (ten minutes here)", Near(Next(), TimeSpan.FromMinutes(10)), Next());
        status = 200;
        await Beat(DateTime.UtcNow.AddHours(13), 9);
        await Beat(DateTime.UtcNow.AddHours(14), 10);
        c.Check("answered again: back to the usual gap, the same id", Near(Next(), TimeSpan.FromMinutes(10)) && Last().GetProperty("id").GetString() == id, Next());

        // Closing: a goodbye with that id; not for an update in the background, back within seconds.
        Harness.Set(app, "_relaunchArgs", new[] { "--background" });
        Harness.Invoke(app, "SayOfflineOnExit");
        c.Check("closing for an update in the background: no goodbye", Count() == 10, Count());
        Harness.Set(app, "_relaunchArgs", null);
        Harness.Invoke(app, "SayOfflineOnExit");
        c.Check("closing: a goodbye, with the id only", Count() == 11 && Last().GetProperty("id").GetString() == id
            && Last().GetProperty("gone").GetBoolean() && Last().EnumerateObject().Count() == 2, Last());

        // Turned off: a goodbye, the id forgotten, nothing more.
        await Harness.Call(app, "setCountOnline", new { enabled = false });
        for (int i = 0; i < 100 && Count() < 12; i++)
            await Task.Delay(50);
        c.Check("turned off: a goodbye", Count() == 12 && Last().GetProperty("gone").GetBoolean() && Last().GetProperty("id").GetString() == id, Last());
        prefs = File.ReadAllText(Path.Combine(dir, "prefs.json"));
        c.Check("…the id forgotten, the choice kept", Prefs.OnlineId is null && !prefs.Contains(id) && prefs.Contains("\"countOnline\":false"), prefs);
        await Beat(DateTime.UtcNow.AddDays(1), 12);
        await Task.Delay(300);
        c.Check("…and nothing more sent", Count() == 12, Count());

        // On again: counted at once, with a new id.
        await Harness.Call(app, "setCountOnline", new { enabled = true });
        c.Check("on again: a beat at once, with a new id", await Beat(DateTime.UtcNow, 13) && Last().GetProperty("id").GetString() is { } fresh
            && fresh != id && fresh == Prefs.OnlineId, Last());

        listener.Stop();
        await serving;
        Environment.SetEnvironmentVariable("BLOXKEEP_ONLINE_URL", null);
        // Without the test's server this copy is a development one again: closing sends nothing to the real bot.
        c.Check("a development copy says no goodbye to the real bot", !(bool)typeof(AppController).GetProperty("OnlineAllowed", Harness.F)!.GetValue(app)!);

        // The real address: Bloxkeep's bot (DiscordBot/worker), which counts.
        string real = (string)typeof(AppController).GetField("OnlineUrl", Harness.F)!.GetValue(null)!;
        c.Check("the bot's address is set", Uri.TryCreate(real, UriKind.Absolute, out var uri) && uri.Scheme == "https"
            && uri.Host == "bloxkeep-bot.bloxkeep-app.workers.dev" && uri.AbsolutePath == "/online/ping", real);
    }

    /// <summary>An HttpListener on a free port of this PC only.</summary>
    static (HttpListener Listener, string Url) Listen()
    {
        for (int port = 48761; ; port++)
        {
            var listener = new HttpListener();
            string url = $"http://127.0.0.1:{port}/online/ping/";
            listener.Prefixes.Add(url);
            try
            {
                listener.Start();
                return (listener, url);
            }
            catch (HttpListenerException)
            {
                listener.Close();
            }
        }
    }
}
