using System.Net;
using System.Net.WebSockets;
using System.Text;
using System.Text.Json.Nodes;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// Control from Discord (AppController.Remote.cs, RemoteLink): off until turned on; on, Bloxkeep connects to the bot (a
/// little WebSocket server of the test's own here, never the bot) with a secret kept encrypted, says only the PC's,
/// accounts' and presets' names, asks for a code and shows it, shows who it's linked to; answers the bot's commands
/// (the status card; a preset, an account that doesn't exist; nothing open to close), and only "locked" while locked;
/// connects again after a drop; turned off, tells the bot to unlink and closes.
/// </summary>
static class RemoteSuite
{
    public static async Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        // Roblox isn't asked anything for real: every question gets "not found".
        typeof(RobloxApi).GetProperty("StandIn", Harness.F)!.SetValue(null,
            (Func<HttpRequestMessage, Task<HttpResponseMessage>>)(_ => Task.FromResult(new HttpResponseMessage(HttpStatusCode.NotFound) { Content = new StringContent("{}") })));
        var seed = Vault.Create(Harness.Password);
        seed.Data.Accounts.Add(new Account { Username = "Alpha", UserId = 3001, Cookie = "_TEST_remote_A", CookieValid = true });
        seed.Data.Accounts.Add(new Account { Username = "Beta", UserId = 3002, Cookie = "_TEST_remote_B", CookieValid = true });
        seed.Data.Settings.Presets.Add(new LaunchPreset { Name = "Farm", AccountIds = [seed.Data.Accounts[0].Id] });
        seed.Save();

        var bot = new StandInBot();
        Environment.SetEnvironmentVariable("BLOXKEEP_REMOTE_URL", bot.Url);
        using var form = new Form();
        using var app = new AppController(form, _ => { });
        await Harness.Unlock(app);
        // The connection's callbacks run where it was made: here, the thread pool (no window's message loop to post to).
        SynchronizationContext.SetSynchronizationContext(null);
        async Task<bool> Until(Func<bool> done, int ms = 8000)
        {
            for (int waited = 0; !done() && waited < ms; waited += 50)
            {
                Harness.Invoke(app, "TickRemote");
                await Task.Delay(50);
            }
            return done();
        }
        object? Field(string name) { var r = Harness.Invoke(app, "RemoteDto")!; return r.GetType().GetProperty(name)!.GetValue(r); }

        c.Check("off until turned on: nothing connects", !Prefs.RemoteControl && !await Until(() => bot.Connections > 0, 600));

        await Harness.Call(app, "setRemote", new { enabled = true });
        c.Check("turned on: it connects to the bot", await Until(() => bot.Connections == 1 && Field("connected") is true), bot.Connections);
        string secret = bot.LastSecret ?? "";
        c.Check("…with a secret of its own (long and random), kept in prefs.json encrypted, not as it is",
            secret.Length >= 40 && Prefs.RemoteKey is { Length: > 0 } key && !key.Contains(secret) && !File.ReadAllText(Path.Combine(dir, "prefs.json")).Contains(secret), secret.Length);
        c.Check("…says the PC's, accounts' and presets' names, nothing else", await Until(() => bot.Last("hello") is not null)
            && bot.Last("hello")!["name"]!.GetValue<string>() == Environment.MachineName
            && Names(bot.Last("hello")!, "accounts") == "Alpha,Beta" && Names(bot.Last("hello")!, "presets") == "Farm"
            && !bot.Heard.Any(m => m.ToJsonString().Contains("_TEST_remote_")), bot.Last("hello")?.ToJsonString());
        c.Check("…and asks for a code", await Until(() => bot.Last("code") is not null));
        await bot.SendAsync(new JsonObject { ["t"] = "link", ["linked"] = false });
        await bot.SendAsync(new JsonObject { ["t"] = "code", ["code"] = "ABC234", ["until"] = DateTimeOffset.UtcNow.AddMinutes(10).ToUnixTimeMilliseconds() });
        c.Check("the code the bot gives is shown, with when it stops working", await Until(() => Field("code") as string == "ABC234")
            && Field("codeUntil") is long until && until > DateTimeOffset.UtcNow.AddMinutes(9).ToUnixTimeMilliseconds() && Field("linked") is false);
        await bot.SendAsync(new JsonObject { ["t"] = "link", ["linked"] = true, ["user"] = "Five" });
        c.Check("linked: who to is shown, the code gone", await Until(() => Field("linked") is true) && Field("user") as string == "Five" && Field("code") is null);

        // Someone typed the code into /pc link: nothing is linked until it's answered here.
        object? Asking() { var r = Harness.Invoke(app, "RemoteDto")!; return r.GetType().GetProperty("ask")!.GetValue(r); }
        string? AskField(string name) => Asking()?.GetType().GetProperty(name)?.GetValue(Asking()) as string;
        await bot.SendAsync(new JsonObject { ["t"] = "ask", ["user"] = "Stranger", ["replacing"] = "Five" });
        c.Check("asked to allow a link: who asks and whom it replaces are shown", await Until(() => Asking() is not null) && AskField("user") == "Stranger" && AskField("replacing") == "Five");
        await Harness.Call(app, "remoteAllow", new { allow = false });
        c.Check("…Don't allow: the bot is told no, the question gone", await Until(() => bot.Last("deny") is not null) && Asking() is null && bot.Last("allow") is null);
        await bot.SendAsync(new JsonObject { ["t"] = "ask", ["user"] = "Five" });
        await Until(() => Asking() is not null);
        await Harness.Call(app, "lock", new { });
        c.Check("…while locked, nothing is asked (no name on the lock screen)", Asking() is null);
        await Harness.Unlock(app);
        c.Check("…unlocked, the question is back", await Until(() => Asking() is not null) && AskField("user") == "Five");
        await Harness.Call(app, "remoteAllow", new { allow = true });
        c.Check("…Allow: the bot is told yes", await Until(() => bot.Last("allow") is not null) && Asking() is null);
        await bot.SendAsync(new JsonObject { ["t"] = "link", ["linked"] = true, ["user"] = "Five" });
        await Until(() => Field("linked") is true);

        // ── commands ──
        async Task<JsonObject?> Ask(string command, string? value = null)
        {
            string id = Guid.NewGuid().ToString("N");
            await bot.SendAsync(new JsonObject { ["t"] = "command", ["id"] = id, ["command"] = command, ["value"] = value });
            await Until(() => bot.Heard.Any(m => m["t"]?.GetValue<string>() == "answer" && m["id"]?.GetValue<string>() == id));
            return bot.Heard.LastOrDefault(m => m["id"]?.GetValue<string>() == id);
        }
        string Text(JsonObject? answer) => answer?["content"]?.GetValue<string>() ?? "";
        var status = await Ask("status");
        c.Check("/pc status: the status card, named after this PC", status?["embeds"]?[0]?["title"]?.GetValue<string>() == "Nobody is playing"
            && status["embeds"]![0]!["footer"]?["text"]?.GetValue<string>() == "Bloxkeep · " + Environment.MachineName, status?.ToJsonString());
        c.Check("/pc preset with a name it doesn't have: said so", Text(await Ask("preset", "Nope")) == "No preset is called Nope.");
        c.Check("/pc close All accounts with nothing open: said so", Text(await Ask("close", "All accounts")) == "No Roblox window is open on this PC.");
        c.Check("/pc close an account with no window: said so", Text(await Ask("close", "beta")) == "Beta has no Roblox window open.");
        c.Check("/pc switch an account that doesn't exist: said so", Text(await Ask("switch", "Ghost")) == "No account is called Ghost.");
        c.Check("…and one that isn't playing: said why it can't", Text(await Ask("switch", "Alpha")) == "Only accounts playing in a public server can switch to another one.");

        // Locked: nothing but that, and no names to suggest.
        await Harness.Call(app, "lock", new { });
        c.Check("locked: a command is answered only that it is locked", Text(await Ask("status")) == "Bloxkeep is locked on this PC. Unlock it there first.");
        c.Check("…and Discord is sent no names to suggest", await Until(() => bot.Last("hello") is { } h && Names(h, "accounts") == "" && Names(h, "presets") == ""));
        await Harness.Unlock(app);

        // A drop: connected again, the names said again.
        int before = bot.Connections, hellos = bot.Count("hello");
        bot.Drop();
        c.Check("dropped: it connects again and says its names again", await Until(() => bot.Connections == before + 1 && Field("connected") is true && bot.Count("hello") > hellos, 10_000),
            $"{bot.Connections} {bot.Count("hello")}");
        c.Check("…with the same secret (still linked)", bot.LastSecret == secret);

        // Turned off: the bot told to unlink, the connection closed, nothing more.
        await Harness.Call(app, "setRemote", new { enabled = false });
        c.Check("turned off: the bot is told to unlink, and the connection closes", await Until(() => bot.Last("unlink") is not null && bot.Open == 0), bot.Open);
        c.Check("…and stays closed", !await Until(() => bot.Open > 0, 2500) && Field("on") is false);
        bot.Dispose();
    }

    static string Names(JsonObject hello, string list) => string.Join(",", hello[list]!.AsArray().Select(n => n!.GetValue<string>()));

    /// <summary>The bot's /remote/connect, here: takes connections, keeps what they say, says what the test wants.</summary>
    sealed class StandInBot : IDisposable
    {
        readonly HttpListener _listener = new();
        readonly List<JsonObject> _heard = new();
        WebSocket? _socket;
        int _connections, _open;
        public string Url { get; }
        public string? LastSecret { get; private set; }
        public int Connections => Volatile.Read(ref _connections);
        public int Open => Volatile.Read(ref _open);
        public List<JsonObject> Heard { get { lock (_heard) return _heard.ToList(); } }
        public JsonObject? Last(string t) => Heard.LastOrDefault(m => m["t"]?.GetValue<string>() == t);
        public int Count(string t) => Heard.Count(m => m["t"]?.GetValue<string>() == t);

        public StandInBot()
        {
            for (int port = 48861; ; port++)
            {
                _listener.Prefixes.Clear();
                _listener.Prefixes.Add($"http://127.0.0.1:{port}/remote/connect/");
                try
                {
                    _listener.Start();
                    Url = $"ws://127.0.0.1:{port}/remote/connect/";
                    break;
                }
                catch (HttpListenerException) when (port < 48900)
                {
                }
            }
            _ = Task.Run(ServeAsync);
        }

        async Task ServeAsync()
        {
            while (_listener.IsListening)
            {
                HttpListenerContext ctx;
                try
                {
                    ctx = await _listener.GetContextAsync();
                }
                catch (Exception)
                {
                    break;
                }
                if (!ctx.Request.IsWebSocketRequest)
                {
                    ctx.Response.StatusCode = 400;
                    ctx.Response.Close();
                    continue;
                }
                LastSecret = ctx.Request.Headers["Authorization"]?.Replace("Bearer ", "");
                var ws = (await ctx.AcceptWebSocketAsync(null)).WebSocket;
                _socket = ws;
                Interlocked.Increment(ref _connections);
                Interlocked.Increment(ref _open);
                _ = ReadAsync(ws);
            }
        }

        async Task ReadAsync(WebSocket ws)
        {
            var buffer = new byte[65536];
            try
            {
                while (ws.State == WebSocketState.Open)
                {
                    var got = await ws.ReceiveAsync(buffer, CancellationToken.None);
                    if (got.MessageType == WebSocketMessageType.Close)
                        break;
                    string text = Encoding.UTF8.GetString(buffer, 0, got.Count);
                    if (text != "ping" && JsonNode.Parse(text) is JsonObject m)
                        lock (_heard)
                            _heard.Add(m);
                }
            }
            catch (Exception ex) when (ex is WebSocketException or HttpListenerException or ObjectDisposedException)
            {
            }
            Interlocked.Decrement(ref _open);
        }

        public async Task SendAsync(JsonObject m)
        {
            if (_socket is { State: WebSocketState.Open } ws)
                await ws.SendAsync(Encoding.UTF8.GetBytes(m.ToJsonString()), WebSocketMessageType.Text, true, CancellationToken.None);
        }

        public void Drop() => _socket?.Abort();

        public void Dispose()
        {
            _socket?.Abort();
            _listener.Close();
        }
    }
}
