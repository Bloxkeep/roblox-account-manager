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
        // The bot keeps codes in memory: connected again (after a drop, or the bot's restart) with a code shown, it's
        // asked whether it still knows it (else a new one), never left showing a code that may be dead.
        int connections = bot.Connections;
        bot.Drop();
        c.Check("connected again with a code shown: the bot is asked to keep it, or give a new one", await Until(() => bot.Connections > connections
            && bot.Last("code")?["keep"]?.GetValue<string>() == "ABC234", 10_000), bot.Last("code")?.ToJsonString());
        await bot.SendAsync(new JsonObject { ["t"] = "link", ["linked"] = true, ["user"] = "Five" });
        c.Check("linked: who to is shown, the code gone", await Until(() => Field("linked") is true) && Field("user") as string == "Five" && Field("code") is null);

        // Someone typed the code into /pc link: nothing is linked until it's answered here.
        object? Asking() { var r = Harness.Invoke(app, "RemoteDto")!; return r.GetType().GetProperty("ask")!.GetValue(r); }
        string? AskField(string name) => Asking()?.GetType().GetProperty(name)?.GetValue(Asking()) as string;
        await bot.SendAsync(new JsonObject { ["t"] = "ask", ["user"] = "Stranger", ["replacing"] = "Five" });
        c.Check("asked to allow a link: who asks and whom it replaces are shown", await Until(() => Asking() is not null) && AskField("user") == "Stranger" && AskField("replacing") == "Five");
        await Harness.Call(app, "remoteAllow", new { allow = false });
        c.Check("…Don't allow: the bot is told no, the question gone", await Until(() => bot.Last("deny") is not null) && Asking() is null && bot.Last("allow") is null);
        // Another /pc link while the box is up: a click on the box as it was answers nobody (2026-10-30 review: it
        // allowed whoever asked last).
        await bot.SendAsync(new JsonObject { ["t"] = "ask", ["user"] = "@stranger", ["id"] = "8" });
        await Until(() => AskField("id") == "8");
        await bot.SendAsync(new JsonObject { ["t"] = "ask", ["user"] = "@other", ["id"] = "9" });
        await Until(() => AskField("id") == "9");
        await Harness.Call(app, "remoteAllow", new { allow = true, id = "8" });
        await Task.Delay(300);
        c.Check("an Allow pressed on a question someone else's replaced: nothing sent, the new question stays", bot.Last("allow") is null && AskField("id") == "9"
            && AskField("user") == "@other");
        await Harness.Call(app, "remoteAllow", new { allow = false, id = "9" });
        c.Check("…an answer says whose question it was", await Until(() => bot.Last("deny")?["user"]?.GetValue<string>() == "9") && Asking() is null);
        await bot.SendAsync(new JsonObject { ["t"] = "ask", ["user"] = "Five", ["id"] = "5" });
        await Until(() => Asking() is not null);
        await Harness.Call(app, "lock", new { });
        c.Check("…while locked, nothing is asked (no name on the lock screen)", Asking() is null);
        await Harness.Unlock(app);
        c.Check("…unlocked, the question is back", await Until(() => Asking() is not null) && AskField("user") == "Five");
        await Harness.Call(app, "remoteAllow", new { allow = true, id = "5" });
        c.Check("…Allow: the bot is told yes, to that question", await Until(() => bot.Last("allow") is not null) && Asking() is null
            && bot.Last("allow")!["user"]?.GetValue<string>() == "5");
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
        c.Check("/pc switch All accounts with nothing open: said so", Text(await Ask("switch", "All accounts")) == "No Roblox window is open on this PC.");
        c.Check("the bot is told this app answers /pc screenshot", bot.Last("hello")?["shots"]?.GetValue<bool>() == true);
        c.Check("/pc screenshot All accounts with nothing open: said so", Text(await Ask("screenshot", "All accounts")) == "No Roblox window is open on this PC.");
        c.Check("/pc screenshot an account with no window: said so", Text(await Ask("screenshot", "alpha")) == "Alpha has no Roblox window open.");
        c.Check("…one that doesn't exist: said so", Text(await Ask("screenshot", "Ghost")) == "No account is called Ghost.");
        // Asked again and again: one at a time, 15 s apart (anti-AFK keeps its turns).
        Harness.Set(app, "_picturing", true);
        c.Check("/pc screenshot while one is being taken: said so", Text(await Ask("screenshot", "All accounts")) == "Already taking a screenshot. Try again in a moment.");
        Harness.Set(app, "_picturing", false);
        Harness.Set(app, "_lastShot", DateTime.UtcNow.AddSeconds(-5));
        c.Check("…within 15 s of the last one: told how long to wait", Text(await Ask("screenshot", "All accounts")) == "Wait 10 s, then try again.");
        Harness.Set(app, "_lastShot", DateTime.MinValue);
        // The pictures go to the answer link the bot handed (Discord's), not to the bot; then the bot is told it's done.
        var link = Harness.Get<object>(app, "_remote");
        string shotId = Guid.NewGuid().ToString("N");
        var sent = await (Task<bool>)link.GetType().GetMethod("ShotAsync")!.Invoke(link,
            [new RemoteCommand(shotId, "screenshot", "Alpha", bot.ReplyUrl), "Alpha", (IReadOnlyList<byte[]>)[[0xFF, 0xD8, 0xFF, 0xD9]]])!;
        var patch = bot.Replies.LastOrDefault();
        c.Check("the pictures are sent to the answer link handed with the command: a PATCH with the line and a JPEG, no mentions",
            sent && patch is { Method: "PATCH" } && patch.Body.Contains("name=payload_json") && patch.Body.Contains("\"content\":\"Alpha\"") && patch.Body.Contains("\"parse\":[]")
            && patch.Body.Contains("filename=screenshot-1.jpg") && patch.Body.Contains("image/jpeg") && patch.Authorization is null, patch?.Body.Length);
        c.Check("…and the bot is told it's done (so it doesn't say \"no answer\" over them)", await Until(() => bot.Heard.Any(m => m["t"]?.GetValue<string>() == "shown" && m["id"]?.GetValue<string>() == shotId)));
        var none = await (Task<bool>)link.GetType().GetMethod("ShotAsync")!.Invoke(link, [new RemoteCommand("x", "screenshot", "Alpha"), "Alpha", (IReadOnlyList<byte[]>)[[0xFF]]])!;
        c.Check("…a command handed no link: nothing sent", !none);
        // The real bot's links: its own application's answer link only, not any webhook on Discord (2026-10-30 review).
        bool BotReply(string url) => (bool)link.GetType().GetMethod("IsBotReply", Harness.F)!.Invoke(null, [url])!;
        const string BotApp = "1561788269933498408"; // the bot's application (AppController.DiscordApplicationId)
        string token = "aW50ZXJhY3Rpb246MTIzNDU2Nzg5MDEyMzQ1Njc4OnRva2Vu_-.x";
        c.Check("pictures go only to Bloxkeep's bot's own answer link on Discord", BotReply($"https://discord.com/api/v10/webhooks/{BotApp}/{token}/messages/@original")
            && !BotReply($"https://discord.com/api/webhooks/123456789012345678/{token}")
            && !BotReply($"https://discord.com/api/v10/webhooks/123456789012345678/{token}/messages/@original")
            && !BotReply($"https://discord.com/api/v10/webhooks/{BotApp}/{token}/messages/@original?wait=true")
            && !BotReply($"https://discord.com.evil.example/api/v10/webhooks/{BotApp}/{token}/messages/@original")
            && !BotReply($"http://discord.com/api/v10/webhooks/{BotApp}/{token}/messages/@original"));

        // Very many names: what's sent stays under what the bot takes (it closed a bigger one at every connection).
        var many = Enumerable.Range(0, 300).Select(n => $"Tài khoản số {n} của tôi, với một cái tên dài thật là dài").ToArray();
        link.GetType().GetMethod("Hello")!.Invoke(link, [Environment.MachineName, many, many, "vi"]);
        string big = Harness.Get<string>(link, "_hello");
        c.Check("very many accounts and presets: what's said of them stays under the bot's 16,000 characters, some of each kept",
            big.Length < 16_000 && JsonNode.Parse(big)!["presets"]!.AsArray().Count > 10 && JsonNode.Parse(big)!["accounts"]!.AsArray().Count > 10, big.Length);

        // A command that takes long (ten windows pictured, about a minute): nothing else waits behind it.
        var commandField = link.GetType().GetField("_command", Harness.F)!;
        var realCommand = commandField.GetValue(link);
        var hold = new TaskCompletionSource<JsonObject?>();
        commandField.SetValue(link, (Func<RemoteCommand, Task<JsonObject?>>)(_ => hold.Task));
        await bot.SendAsync(new JsonObject { ["t"] = "command", ["id"] = "slow", ["command"] = "status" });
        await bot.SendAsync(new JsonObject { ["t"] = "ask", ["user"] = "@later", ["id"] = "10" });
        c.Check("a command still running holds nothing back: a question to allow shows meanwhile", await Until(() => AskField("id") == "10", 3000));
        hold.SetResult(new JsonObject { ["content"] = "done" });
        c.Check("…and the command's answer goes once it's done", await Until(() => bot.Heard.Any(m => m["t"]?.GetValue<string>() == "answer" && m["id"]?.GetValue<string>() == "slow")));
        commandField.SetValue(link, realCommand);
        await Harness.Call(app, "remoteAllow", new { allow = false, id = "10" });

        // Discord's own Hide account names: text the app wrote for itself (a launch error) says no name in Discord either.
        var vaultData = Harness.Get<Vault>(app, "_vault").Data;
        vaultData.Discord.HideNames = true;
        var said = (JsonObject)Harness.Invoke(app, "RemoteSay", "Alpha: Roblox didn't open in time.")!;
        c.Check("Discord's Hide account names on: an answer's text names no account", said["content"]?.GetValue<string>() == "Account 1: Roblox didn't open in time.", said.ToJsonString());
        vaultData.Discord.HideNames = false;

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
        // Its secret goes too: the unlink may not have reached the bot (offline), and with the same secret the PC came
        // back still linked, without asking (2026-10-30 review).
        c.Check("…and its secret is forgotten", Prefs.RemoteKey is null);
        await Harness.Call(app, "setRemote", new { enabled = true });
        c.Check("turned on again: a new secret (a new PC for the bot, linked to nobody)", await Until(() => bot.Open == 1 && Field("connected") is true)
            && bot.LastSecret is { Length: >= 40 } again && again != secret);
        string first = bot.LastSecret!;

        // Each vault its own: vaults are for different people, and one person's /pc must not reach another's accounts.
        // A switch from before (one for the whole PC) goes to the vault last used.
        Prefs.Save();
        Prefs.VaultList.Clear();
        Prefs.Load();
        Vaults.Load();
        var mine = Vaults.Current;
        c.Check("from before each vault had its own: Control from Discord and its secret go to the vault last used",
            mine is { RemoteControl: true, RemoteKey.Length: > 0 } && Prefs.RemoteControl && await Until(() => Field("connected") is true) && bot.LastSecret == first,
            $"{mine?.RemoteControl} {bot.LastSecret == first}");
        await Harness.Call(app, "addVault", new { });
        await Harness.Call(app, "createVault", new { password = "another vault's password", name = "Other" });
        c.Check("another vault open: this one's link closes, and it's off there", await Until(() => bot.Open == 0) && Field("on") is false && Vaults.Current != mine);
        await Harness.Call(app, "setRemote", new { enabled = true });
        c.Check("…turned on there: a PC of its own for the bot, its own names only", await Until(() => bot.Open == 1 && bot.LastSecret != first
            && bot.Last("hello") is { } h && Names(h, "accounts") == "" && Names(h, "presets") == ""), bot.LastSecret == first);
        await Harness.Call(app, "selectVault", new { id = mine!.Id });
        c.Check("the first vault offered again: its own link, answering only that it's locked", await Until(() => bot.Open == 1 && bot.LastSecret == first)
            && Text(await Ask("status")) == "Bloxkeep is locked on this PC. Unlock it there first.");
        await Harness.Unlock(app);
        c.Check("…unlocked: its own accounts", await Until(() => bot.Last("hello") is { } h && Names(h, "accounts") == "Alpha,Beta"));

        // A bot that closes the connection at once (another Bloxkeep with this secret, a hello it won't take): connected
        // again later and later, not every 2-3 s forever (2026-10-30 review).
        bot.CloseAtOnce = true;
        int tries = bot.Connections;
        bot.Drop();
        await Until(() => false, 9500);
        c.Check("closed at once by the bot each time: it waits longer each time", bot.Connections - tries <= 2, bot.Connections - tries);
        bot.Dispose();
    }

    static string Names(JsonObject hello, string list) => string.Join(",", hello[list]!.AsArray().Select(n => n!.GetValue<string>()));

    /// <summary>The bot's /remote/connect, here: takes connections, keeps what they say, says what the test wants.</summary>
    sealed class StandInBot : IDisposable
    {
        HttpListener _listener = new();
        readonly List<JsonObject> _heard = new();
        WebSocket? _socket;
        int _connections, _open;
        public string Url { get; }
        /// <summary>Where a screenshot's pictures go here, standing in for Discord's answer link.</summary>
        public string ReplyUrl { get; }
        public sealed record Reply(string Method, string Body, string? Authorization);
        readonly List<Reply> _replies = new();
        public List<Reply> Replies { get { lock (_replies) return _replies.ToList(); } }
        public string? LastSecret { get; private set; }
        /// <summary>Each connection is closed as soon as it's made (as the bot does to a socket it won't keep).</summary>
        public bool CloseAtOnce { get; set; }
        public int Connections => Volatile.Read(ref _connections);
        public int Open => Volatile.Read(ref _open);
        public List<JsonObject> Heard { get { lock (_heard) return _heard.ToList(); } }
        public JsonObject? Last(string t) => Heard.LastOrDefault(m => m["t"]?.GetValue<string>() == t);
        public int Count(string t) => Heard.Count(m => m["t"]?.GetValue<string>() == t);

        public StandInBot()
        {
            for (int port = 48861; ; port++)
            {
                // A listener that failed to start is closed: a new one for the next port (one left by the last run).
                _listener = new HttpListener();
                _listener.Prefixes.Add($"http://127.0.0.1:{port}/remote/connect/");
                _listener.Prefixes.Add($"http://127.0.0.1:{port}/webhooks/");
                try
                {
                    _listener.Start();
                    Url = $"ws://127.0.0.1:{port}/remote/connect/";
                    ReplyUrl = $"http://127.0.0.1:{port}/webhooks/app1/tok1/messages/@original";
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
                if (ctx.Request.Url!.AbsolutePath.StartsWith("/webhooks/"))
                {
                    using (var reader = new StreamReader(ctx.Request.InputStream, Encoding.Latin1))
                    {
                        var reply = new Reply(ctx.Request.HttpMethod, reader.ReadToEnd(), ctx.Request.Headers["Authorization"]);
                        lock (_replies)
                            _replies.Add(reply);
                    }
                    ctx.Response.StatusCode = 200;
                    ctx.Response.Close();
                    continue;
                }
                if (!ctx.Request.IsWebSocketRequest)
                {
                    ctx.Response.StatusCode = 400;
                    ctx.Response.Close();
                    continue;
                }
                LastSecret = ctx.Request.Headers["Authorization"]?.Replace("Bearer ", "");
                var ws = (await ctx.AcceptWebSocketAsync(null)).WebSocket;
                Interlocked.Increment(ref _connections);
                if (CloseAtOnce)
                {
                    try
                    {
                        await ws.CloseOutputAsync((WebSocketCloseStatus)4000, "connected again", CancellationToken.None);
                    }
                    catch (Exception ex) when (ex is WebSocketException or HttpListenerException or ObjectDisposedException)
                    {
                    }
                    continue;
                }
                _socket = ws;
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
