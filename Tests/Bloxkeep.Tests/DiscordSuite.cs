using System.Collections;
using System.Drawing.Drawing2D;
using System.Drawing.Imaging;
using System.Drawing.Text;
using System.Runtime.CompilerServices;
using System.Text.Encodings.Web;
using System.Text.Json;
using System.Text.Json.Nodes;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// Every Discord message Bloxkeep can send, built with made-up accounts, games and play (nothing is sent: the builders
/// and DiscordWebhook.Build are called directly), in English and Vietnamese, and checked against docs/discord-style.md:
/// sender, footer and logo, colours, titles, three short fields to a row, names hidden, plain text, pings. In a
/// checkout the payloads also go to dev/discord-samples.json (pictures in dev/discord-samples/), for
/// dev/discord-preview.html?src=discord-samples.json; there the made-up Roblox picture links point at stand-in pictures.
/// </summary>
static class DiscordSuite
{
    const string Hook = "https://discord.com/api/webhooks/100000000000000001/abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_abcdef";
    static readonly int[] Palette = [DiscordWebhook.Blue, DiscordWebhook.Green, DiscordWebhook.Yellow, DiscordWebhook.Red, DiscordWebhook.Grey];

    sealed record Sample(int Order, string Label, string Language, JsonObject Payload, Dictionary<string, string>? Files = null);

    public static async Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        string? devDir = FindDev();
        string pictures = devDir is null ? Path.Combine(dir, "pictures") : Path.Combine(devDir, "discord-samples");
        Directory.CreateDirectory(pictures);

        // ── Made-up accounts, games, presets and two weeks of play ──
        var accounts = new[]
        {
            new Account { Username = "NoobFarm2", UserId = 9100000001 },
            new Account { Username = "BloxGrinder_07", UserId = 9100000002 },
            new Account { Username = "pixelknight", Alias = "Pixel*Knight", UserId = 9100000003 },
            new Account { Username = "AfkBot_4", UserId = 9100000004 },
            new Account { Username = "AfkBot_5", UserId = 9100000005 },
            new Account { Username = "SunnyMain", UserId = 9100000006 },
        };
        var games = new (long Place, long Universe, string Name, Color Color)[]
        {
            (2753915549, 994732206, "Blox Fruits", Color.FromArgb(0x2B, 0x7B, 0xD6)),
            (8737899170, 3317771874, "Pet Simulator 99", Color.FromArgb(0xE0, 0x8A, 0x1E)),
            (920587237, 383310974, "Adopt Me!", Color.FromArgb(0xD9, 0x4F, 0x8A)),
            (6516141723, 2440500124, "DOORS", Color.FromArgb(0x5B, 0x4A, 0x3A)),
            (4924922222, 1686885941, "Brookhaven RP", Color.FromArgb(0x3F, 0xA3, 0x5B)),
        };
        string AvatarUrl(int i) => $"https://tr.rbxcdn.com/30DAY-AvatarHeadshot-SAMPLE{i + 1}/150/150/AvatarHeadshot/Png/noFilter";
        string IconUrl(int i) => $"https://tr.rbxcdn.com/180DAY-GameIcon-SAMPLE{i + 1}/150/150/Image/Png/noFilter";
        var localPictures = new Dictionary<string, string>();
        var avatarBytes = new Dictionary<string, byte[]>();
        Color[] avatarColors = [Color.FromArgb(0x4E, 0x8C, 0xC9), Color.FromArgb(0xC9, 0x6B, 0x4E), Color.FromArgb(0x7A, 0x5C, 0xC4), Color.FromArgb(0x4E, 0xA8, 0x7A), Color.FromArgb(0xB8, 0x9A, 0x3C), Color.FromArgb(0xC4, 0x5C, 0x8E)];
        for (int i = 0; i < accounts.Length; i++)
        {
            byte[] png = StandIn(accounts[i].Label, avatarColors[i], round: true);
            avatarBytes[AvatarUrl(i)] = png;
            File.WriteAllBytes(Path.Combine(pictures, $"avatar-{i + 1}.png"), png);
            localPictures[AvatarUrl(i)] = $"discord-samples/avatar-{i + 1}.png";
        }
        for (int i = 0; i < games.Length; i++)
        {
            File.WriteAllBytes(Path.Combine(pictures, $"game-{i + 1}.png"), StandIn(games[i].Name, games[i].Color, round: false));
            localPictures[IconUrl(i)] = $"discord-samples/game-{i + 1}.png";
        }

        var vault = Vault.Create(Harness.Password);
        vault.Data.Accounts.AddRange(accounts);
        var farm = new LaunchPreset
        {
            Name = "Farm", AccountIds = [accounts[0].Id, accounts[1].Id, accounts[2].Id], PlaceInput = "2753915549", GameName = "Blox Fruits",
            Schedule = new PresetSchedule { Enabled = true, Time = "20:00", Days = [0, 1, 2, 3, 4, 5, 6] },
        };
        var afk = new LaunchPreset { Name = "AFK night", AccountIds = [accounts[3].Id, accounts[4].Id], PlaceInput = "8737899170", GameName = "Pet Simulator 99" };
        vault.Data.Settings.Presets.AddRange([farm, afk]);
        var now = DateTime.UtcNow;
        var random = new Random(7);
        int[][] plays = [[0, 0, 0, 4], [0, 0, 3], [0, 0, 0], [1, 1], [1, 1], [2, 3, 4, 2]];
        Guid?[] presetOf = [farm.Id, farm.Id, farm.Id, afk.Id, afk.Id, null];
        int[] codes = [277, 277, 279, 267, 278];
        for (int day = 13; day >= 0; day--)
        {
            for (int a = 0; a < accounts.Length; a++)
            {
                var at = now.Date.AddDays(-day).AddHours(-7 + random.Next(0, 6));
                foreach (int g in plays[a])
                {
                    var length = TimeSpan.FromMinutes(random.Next(25, 170));
                    if (at + length > now - TimeSpan.FromMinutes(5))
                        break;
                    bool dropped = random.Next(0, 5) == 0;
                    vault.Data.PlaySessions.Add(new PlaySession
                    {
                        AccountId = accounts[a].Id, AccountLabel = accounts[a].Label, PlaceId = games[g].Place, UniverseId = games[g].Universe,
                        GameName = games[g].Name, StartedAt = at, EndedAt = at + length, LastSeenAt = at + length,
                        EndReason = dropped ? "disconnected" : "left", ReasonCode = dropped ? codes[random.Next(codes.Length)] : null,
                        PresetId = g == 2 || g == 4 ? null : presetOf[a],
                    });
                    at += length + TimeSpan.FromMinutes(random.Next(2, 40));
                }
            }
        }
        // Today's run of the scheduled preset, for the summary after it.
        foreach (int a in new[] { 0, 1, 2 })
        {
            var start = new[] { now.AddHours(-3), DateTime.Today.ToUniversalTime().AddMinutes(1) }.Max();
            vault.Data.PlaySessions.Add(new PlaySession
            {
                AccountId = accounts[a].Id, AccountLabel = accounts[a].Label, PlaceId = games[0].Place, UniverseId = games[0].Universe, GameName = games[0].Name,
                StartedAt = start.AddMinutes(a * 4), EndedAt = now.AddMinutes(-2 - a), EndReason = a == 1 ? "disconnected" : "left", ReasonCode = a == 1 ? 277 : null, PresetId = farm.Id,
            });
        }
        vault.Data.PlaySessions.Sort((x, y) => x.StartedAt.CompareTo(y.StartedAt));
        var history = vault.Data.PlaySessions.ToList(); // each summary sees this, not the stays the alerts below add
        var d = vault.Data.Discord;
        d.WebhookUrl = Hook; // never sent: nothing here posts, and the process timer never ticks without a message loop
        d.Enabled = true;
        d.Events = ["drop", "schedule", "signin", "ram", "launch"];

        using var form = new Form();
        using var app = new AppController(form, _ => { });
        // The pictures and games Roblox would have given, before anything could ask Roblox; the vault set without an
        // unlock, so nothing starts that asks Roblox's servers.
        var avatarUrls = Harness.Get<Dictionary<long, string>>(app, "_avatarUrls");
        for (int i = 0; i < accounts.Length - 1; i++) // the last one has no picture: the summary shows its initials
            avatarUrls[accounts[i].UserId] = AvatarUrl(i);
        var known = Harness.Get<Dictionary<long, GameInfo?>>(app, "_games");
        for (int i = 0; i < games.Length; i++)
            known[games[i].Place] = new GameInfo(games[i].Place, games[i].Universe, games[i].Name, "Someone", 1000, IconUrl(i));
        var cache = (Dictionary<string, byte[]>)typeof(SummaryCard).GetField("Avatars", Harness.F)!.GetValue(null)!;
        lock (cache)
        {
            foreach (var (url, png) in avatarBytes)
                cache[url] = png;
        }
        Harness.Set(app, "_vault", vault);

        var samples = new List<Sample>();
        int order = 0;
        foreach (string language in new[] { "en", "vi" })
        {
            d.Language = language;
            d.HideNames = false;
            d.Pings.Clear();
            order = 0;
            void Add(string label, object message, DiscordPing? ping = null, bool embed = true, string? username = null, string? avatar = null, Dictionary<string, string>? files = null)
            {
                var payload = DiscordWebhook.Build((DiscordMessage)message, ping ?? DiscordPing.None, embed, username ?? d.Username, avatar ?? d.AvatarUrl, files?.Keys.FirstOrDefault());
                samples.Add(new Sample(order++, label, language, payload, files));
            }

            // ── Test messages ──
            Add("Test message (Settings › Discord › Send test)", Harness.Invoke(app, "TestMessage", null, false)!);
            Add("Test message of a preset", Harness.Invoke(app, "TestMessage", farm, true)!);

            // ── Alerts, through Notify as the app raises them ──
            var events = (IList)typeof(AppController).GetField("_discordEvents", Harness.F)!.GetValue(app)!;
            var notified = Harness.Get<Dictionary<string, DateTime>>(app, "_discordNotified");
            var playing = Harness.Get<Dictionary<Guid, PlaySession>>(app, "_playing");
            object Name(int a) => Harness.Invoke(app, "Name", accounts[a])!;
            void Stay(int a, int game, TimeSpan played, int? code, bool open = false)
            {
                playing.Clear();
                var stay = new PlaySession
                {
                    AccountId = accounts[a].Id, AccountLabel = accounts[a].Label, PlaceId = games[game].Place, UniverseId = games[game].Universe, GameName = games[game].Name,
                    StartedAt = DateTime.UtcNow - played, EndedAt = open ? null : DateTime.UtcNow.AddSeconds(-20), EndReason = open ? null : code is null ? "left" : "disconnected", ReasonCode = code,
                };
                if (open)
                    playing[accounts[a].Id] = stay;
                else
                    vault.Data.PlaySessions.Add(stay);
            }
            // Raises events as Notify does (right after the L.F that made the text), then makes the message they go out as.
            (DiscordMessage Message, DiscordPing Ping) Raise(params Action[] raise)
            {
                events.Clear();
                foreach (var r in raise)
                {
                    notified.Clear();
                    r();
                }
                var alertType = typeof(AppController).GetNestedType("DiscordAlert", Harness.F)!;
                var alerts = (IList)Activator.CreateInstance(typeof(List<>).MakeGenericType(alertType))!;
                var ping = DiscordPing.None;
                foreach (ITuple e in events)
                {
                    alerts.Add(e[1]);
                    ping = ping.Merge((DiscordPing)e[2]!);
                }
                events.Clear();
                return ((DiscordMessage)Harness.Invoke(app, "EventsMessage", alerts)!, ping);
            }
            void Notify(string text, string type, Guid subject = default) => Harness.Invoke(app, "Notify", text, "warn", type, subject, false);

            Stay(0, 0, TimeSpan.FromMinutes(72), 277);
            var drop = Raise(() => Notify(L.F("{0} left the game — rejoining in {1}s…", Name(0), 5), "drop", accounts[0].Id));
            Add("Alert · disconnected, rejoining", drop.Message);
            Stay(1, 0, TimeSpan.FromMinutes(134), null, open: true);
            Add("Alert · lost connection, restarting Roblox", Raise(() => Notify(L.F("{0} seems disconnected — restarting Roblox…", Name(1)), "drop", accounts[1].Id)).Message);
            Stay(2, 0, TimeSpan.FromMinutes(3), 279);
            Add("Alert · keeps getting disconnected", Raise(() => Notify(L.F("{0} keeps getting disconnected — trying again every few minutes.", Name(2)), "rejoinSlow", accounts[2].Id)).Message);
            Add("Alert · couldn't rejoin (an error)", Raise(() => Notify(L.F("Couldn't rejoin {0}: {1}", Name(2), "Roblox took too long to answer."), "rejoinFail", accounts[2].Id)).Message);
            Add("Alert · couldn't rejoin (Multi-Roblox not ready)", Raise(() => Notify(L.F("Didn't rejoin {0}: Multi-Roblox isn't ready, so a new Roblox window would close the one that's open.", Name(2)), "rejoinFail", accounts[2].Id)).Message);
            playing.Clear();
            Add("Alert · couldn't rejoin (no window)", Raise(() => Notify(L.F("{0} didn't rejoin: no Roblox window appeared.", Name(2)), "rejoinFail", accounts[2].Id)).Message);
            Stay(3, 1, TimeSpan.FromMinutes(21), 278);
            Add("Alert · kicked for being AFK", Raise(() => Notify(L.F("{0} was kicked for being AFK: the PC was in use without a pause of two seconds, so anti-AFK couldn't step in.", Name(3)), "afk", accounts[3].Id)).Message);
            Add("Alert · banned", Raise(() => Notify(L.F("{0} is banned on Roblox.", Name(4)), "banned", accounts[4].Id)).Message);
            var names = typeof(AppController).GetMethod("Names", Harness.F)!;
            Add("Alert · several banned", Raise(() => Notify(L.P(2, "{0} account is banned on Roblox: {1}", "{0} accounts are banned on Roblox: {1}", 2,
                names.Invoke(app, [new[] { accounts[3], accounts[4] }])), "banned")).Message);
            Add("Alert · sign-in needed", Raise(() => Notify((string)Harness.Invoke(app, "SignedOutText", new List<Account> { accounts[1], accounts[2], accounts[5] })!, "signin")).Message);
            Add("Alert · low RAM", Raise(() => Notify(L.F("Your PC is using {0}% of its RAM. Close some Roblox windows if things get slow.", 92), "ram")).Message);
            Add("Alert · launch: accounts didn't open", Raise(() => Notify(L.P(2, "{0} account didn't open.", "{0} accounts didn't open.", 2), "launch")).Message);
            Add("Alert · Roblox closed windows", Raise(() => Notify(L.F("Roblox closed {0} window(s) when another one opened — Roblox may have blocked running several windows. Multi-Roblox may need an update.", 2), "launch")).Message);
            Add("Alert · scheduled preset started", Raise(() => Notify(L.P(3, "Scheduled preset {0} started: {1} account opened.", "Scheduled preset {0} started: {1} accounts opened.", farm.Name, 3), "schedule", farm.Id)).Message);
            Add("Alert · scheduled preset, already open", Raise(() => Notify(L.F("Scheduled preset {0}: its accounts were already open.", farm.Name), "schedule", farm.Id)).Message);
            Add("Alert · scheduled preset didn't run", Raise(() => Notify(L.F("Scheduled preset {0} didn't run: {1}", farm.Name, L.T("Wait until Bloxkeep finishes opening Roblox.", d.Language)), "scheduleFail", farm.Id)).Message);
            Add("Alert · scheduled preset, some didn't open", Raise(() => Notify(L.P(1, "Scheduled preset {0}: {1} account didn't open. {2}", "Scheduled preset {0}: {1} accounts didn't open. {2}", farm.Name, 1, "pixelknight: Roblox didn't open in time."), "scheduleFail", farm.Id)).Message);
            Add("Alert · scheduled preset missed", Raise(() => Notify(L.F("{0} didn't run at {1}: the PC was asleep or busy.", farm.Name, "20:00"), "scheduleMissed", farm.Id)).Message);
            Add("Alert · scheduled preset finished", Raise(() => Notify(L.P(3, "Scheduled preset {0} finished: closing {1} account.", "Scheduled preset {0} finished: closing {1} accounts.", farm.Name, 3), "scheduleStop", farm.Id)).Message);
            // Several at once: three drops (one message, a field each), and two about one account.
            Stay(0, 0, TimeSpan.FromMinutes(95), 277);
            Stay(1, 0, TimeSpan.FromMinutes(95), 277);
            var three = Raise(
                () => Notify(L.F("{0} left the game — rejoining in {1}s…", Name(0), 5), "drop", accounts[0].Id),
                () => Notify(L.F("{0} left the game — rejoining in {1}s…", Name(1), 5), "drop", accounts[1].Id),
                () => Notify(L.F("{0} is banned on Roblox.", Name(4)), "banned", accounts[4].Id));
            Add("Alerts · three at once", three.Message);
            Add("Alerts · two about one account", Raise(
                () => Notify(L.F("{0} left the game — rejoining in {1}s…", Name(2), 5), "drop", accounts[2].Id),
                () => Notify(L.F("{0} didn't rejoin: no Roblox window appeared.", Name(2)), "rejoinFail", accounts[2].Id)).Message);
            // A ping chosen for drops: named in the message, and the only one allowed to ping.
            d.Pings["drop"] = "user:123456789012345678";
            Stay(0, 0, TimeSpan.FromMinutes(40), 277);
            var pinged = Raise(() => Notify(L.F("{0} left the game — rejoining in {1}s…", Name(0), 5), "drop", accounts[0].Id));
            d.Pings.Clear();
            Add("Alert with a ping", pinged.Message, pinged.Ping);
            Add("Alert after messages dropped while offline", Harness.Invoke(app, "WithDropped", drop.Message, 3)!);

            // ── The status board ──
            var rowType = typeof(AppController).GetNestedType("BoardRow", Harness.F)!;
            IList Rows(params (int Account, int? Game, string Status, int Minutes, int Pid, long Mb)[] rows)
            {
                var list = (IList)Activator.CreateInstance(typeof(List<>).MakeGenericType(rowType))!;
                foreach (var r in rows)
                    list.Add(Activator.CreateInstance(rowType, accounts[r.Account].Label, r.Game is int g ? games[g].Place : (long?)null, r.Game is int n ? games[n].Name : null,
                        r.Status, (long)r.Minutes * 60, r.Pid, r.Mb * 1024 * 1024));
                return list;
            }
            var boardMethod = typeof(AppController).GetMethods(Harness.F).Single(m => m.Name == "BoardMessage" && m.GetParameters()[0].ParameterType == typeof(int));
            object Board(IList rows) => boardMethod.Invoke(app, [5, rows, DateTime.UtcNow])!;
            Add("Board · six open, one cut off", Board(Rows(
                (0, 0, "ingame", 134, 14212, 812), (1, 0, "ingame", 131, 15880, 790), (2, 0, "disconnected", 128, 9932, 640),
                (3, 1, "ingame", 422, 20144, 455), (4, 1, "joining", 1, 21008, 210), (5, null, "menu", 6, 22540, 380))));
            Add("Board · all in game", Board(Rows((0, 0, "ingame", 72, 14212, 812), (1, 0, "ingame", 70, 15880, 790), (2, 0, "ingame", 69, 9932, 760))));
            Add("Board · nobody playing", Board(Rows()));
            Add("Board · Bloxkeep locked", Harness.Invoke(app, "FinalBoardMessage", false)!);
            Add("Board · Bloxkeep closed", Harness.Invoke(app, "FinalBoardMessage", true)!);

            // ── Summaries, with their picture ──
            async Task Summary(string label, int days, DateTime? dueAt, bool today, LaunchPreset? preset, string kind, bool embed = true)
            {
                vault.Data.PlaySessions = history.ToList();
                var message = (DiscordMessage)Harness.Invoke(app, "SummaryMessage", d, days, dueAt, DateTime.Now, today, preset, kind)!;
                Dictionary<string, string>? files = null;
                if (message.Card is { } card)
                {
                    string file = $"summary-{language}-{order + 1}.png";
                    File.WriteAllBytes(Path.Combine(pictures, file), await SummaryCard.RenderAsync(card));
                    files = new() { ["bloxkeep-summary.png"] = "discord-samples/" + file };
                }
                Add(label, message, embed: embed, files: files);
            }
            await Summary("Summary · daily (24 hours)", 1, DateTime.Now.AddMinutes(-1), false, null, "daily");
            await Summary("Summary · weekly (7 days)", 7, DateTime.Now.AddMinutes(-1), false, null, "weekly");
            await Summary("Summary · daily and weekly at once", 7, DateTime.Now.AddMinutes(-1), false, null, "both");
            await Summary("Summary · after the scheduled preset", 1, null, true, farm, "preset");
            await Summary("Summary · Send summary now (30 days)", 30, null, false, null, "now");
            await Summary("Summary · daily, sent late", 1, DateTime.Now.AddHours(-3), false, null, "daily");
            d.SummaryAccounts = [accounts[5].Id];
            d.SummaryParts = ["total", "games", "kicks", "accounts"];
            await Summary("Summary · chosen accounts, fewer parts", 1, DateTime.Now.AddMinutes(-1), false, null, "daily");
            d.SummaryAccounts = [];
            d.SummaryParts = ["total", "accounts", "games", "sessions", "kicks"];
            await Summary("Summary · nothing played", 1, null, true, afk, "preset");

            // ── Names hidden, plain text, the person's own sender ──
            d.HideNames = true;
            Stay(0, 0, TimeSpan.FromMinutes(72), 277);
            var hidden = Raise(() => Notify(L.F("{0} left the game — rejoining in {1}s…", Name(0), 5), "drop", accounts[0].Id));
            Add("Names hidden · alert", hidden.Message);
            await Summary("Names hidden · summary", 1, DateTime.Now.AddMinutes(-1), false, null, "daily");
            d.HideNames = false;
            Add("Plain text · alert", drop.Message, embed: false);
            Add("Plain text · several alerts", three.Message, embed: false);
            Add("Plain text · board", Board(Rows((0, 0, "ingame", 72, 14212, 812), (2, 0, "disconnected", 69, 9932, 760))), embed: false);
            await Summary("Plain text · summary", 1, DateTime.Now.AddMinutes(-1), false, null, "daily", embed: false);
            Add("Own sender name and picture (Settings › Discord)", drop.Message, username: "Farm watch", avatar: AvatarUrl(5));
        }
        d.Enabled = false;

        Verify(c, samples, accounts);

        if (devDir is not null)
        {
            var list = new JsonArray(samples.OrderBy(s => s.Order).ThenBy(s => s.Language == "en" ? 0 : 1).Select(s =>
            {
                var item = new JsonObject { ["label"] = $"{s.Label} · {s.Language.ToUpperInvariant()}", ["payload"] = s.Payload.DeepClone() };
                if (s.Files is { } files)
                    item["files"] = new JsonObject(files.Select(f => KeyValuePair.Create(f.Key, (JsonNode?)JsonValue.Create(f.Value))));
                return (JsonNode)item;
            }).ToArray());
            string json = list.ToJsonString(new JsonSerializerOptions { WriteIndented = true, Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping });
            // The made-up Roblox pictures exist only here: the preview shows stand-ins.
            foreach (var (url, local) in localPictures)
                json = json.Replace(url, local);
            File.WriteAllText(Path.Combine(devDir, "discord-samples.json"), json);
            Console.WriteLine($"samples: {Path.Combine(devDir, "discord-samples.json")} ({samples.Count})");
        }
    }

    static void Verify(Checks c, List<Sample> samples, Account[] accounts)
    {
        var problems = new List<string>();
        void Problem(Sample s, string what) => problems.Add($"{s.Label} ({s.Language}): {what}");
        foreach (var s in samples)
        {
            var p = s.Payload;
            string sender = p["username"]?.GetValue<string>() ?? "";
            if (sender.Length == 0 || p["avatar_url"]?.GetValue<string>() is not { Length: > 0 })
                Problem(s, "no sender name or picture");
            if (s.Label.StartsWith("Own sender") ? sender != "Farm watch" : sender != "Bloxkeep" || p["avatar_url"]!.GetValue<string>() != DiscordWebhook.Logo)
                Problem(s, "sender isn't Bloxkeep with the logo (or the person's own)");
            if (p["embeds"] is JsonArray embeds)
            {
                for (int i = 0; i < embeds.Count; i++)
                {
                    var e = embeds[i]!.AsObject();
                    string title = e["title"]?.GetValue<string>() ?? "";
                    if (title.Length is 0 or > 70 || title.EndsWith('.') || title.Any(ch => char.IsSurrogate(ch) || ch is >= '☀' and <= '➿'))
                        Problem(s, $"title \"{title}\"");
                    if (!Palette.Contains(e["color"]!.GetValue<int>()))
                        Problem(s, "a colour that isn't Discord's");
                    if (i == 0 && (e["footer"]?["text"]?.GetValue<string>() is not { } footer || !footer.StartsWith("Bloxkeep · ")
                        || e["footer"]!["icon_url"]?.GetValue<string>() != DiscordWebhook.Logo || e["timestamp"] is null))
                        Problem(s, "footer, logo or time missing");
                    // The logo or the subject's picture beside it; not beside the board's accounts (their columns get the width).
                    if (i == 0 && e["thumbnail"] is null && !(s.Label.StartsWith("Board") && e["fields"] is JsonArray))
                        Problem(s, "no thumbnail");
                    foreach (string url in new[] { e["thumbnail"]?["url"], e["author"]?["icon_url"] }.Select(n => n?.GetValue<string>()).OfType<string>())
                        if (url != DiscordWebhook.Logo && !DiscordWebhook.IsRobloxPicture(url))
                            Problem(s, $"a picture from elsewhere: {url}");
                    var fields = (e["fields"] as JsonArray ?? []).Select(f => f!.AsObject()).ToList();
                    if (fields.Count > 25)
                        Problem(s, "more than 25 fields");
                    // Three short fields to a row: one alone on the last row (not the board's accounts) goes to the small line.
                    int inline = fields.Count(f => f["inline"]!.GetValue<bool>());
                    if (!s.Label.StartsWith("Board") && inline % 3 == 1)
                        Problem(s, $"{inline} short fields: one alone on the last row");
                    string text = e.ToJsonString(new JsonSerializerOptions { Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping });
                    if (s.Language == "vi" && text.Contains('—'))
                        Problem(s, "a dash in the Vietnamese");
                    if (text.Contains("⏱") || text.Contains("🎮") || text.Contains("🏆"))
                        Problem(s, "emoji");
                }
            }
            else
            {
                string content = p["content"]?.GetValue<string>() ?? "";
                if (!content.StartsWith("**") || content.Contains("](https://") || !content.Contains("\n-# Bloxkeep · "))
                    Problem(s, "plain text: no bold title, a link, or no footer line");
            }
            string all = p.ToJsonString(new JsonSerializerOptions { Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping });
            if (s.Label.StartsWith("Names hidden"))
            {
                var leaked = accounts.Select(a => a.Label).Where(n => all.Contains(n)).ToList();
                if (leaked.Count > 0 || all.Contains("AvatarHeadshot"))
                    Problem(s, "a name or avatar while names are hidden: " + string.Join(", ", leaked));
            }
            if (s.Label == "Alert with a ping" && (p["content"]?.GetValue<string>() != "<@123456789012345678>"
                || p["allowed_mentions"]?["users"]?[0]?.GetValue<string>() != "123456789012345678" || p["allowed_mentions"]?["parse"]?.AsArray().Count != 0))
                Problem(s, "the ping or allowed_mentions");
            if (s.Label != "Alert with a ping" && p["allowed_mentions"]?["parse"]?.AsArray().Count != 0)
                Problem(s, "mentions not off");
        }
        c.Check($"{samples.Count} messages follow docs/discord-style.md", problems.Count == 0, string.Join(" | ", problems.Take(12)));

        // Each message reads differently in Vietnamese (its words are translated), and a name typed with "*" stays text.
        var untranslated = samples.Where(s => s.Language == "en").Select(en => (en, vi: samples.Single(v => v.Language == "vi" && v.Order == en.Order)))
            .Where(x => !x.en.Label.StartsWith("Own sender") && Words(x.en.Payload) == Words(x.vi.Payload)).Select(x => x.en.Label).ToList();
        c.Check("every message has its Vietnamese", untranslated.Count == 0, string.Join(", ", untranslated));
        var board = samples.First(s => s.Label.StartsWith("Board · six") && s.Language == "en").Payload.ToJsonString();
        c.Check("a name with * is escaped", board.Contains(@"Pixel\\*Knight"), board.Length);
        var group = samples.First(s => s.Label == "Alerts · three at once" && s.Language == "en").Payload;
        c.Check("events at once: one embed, a field each", group["embeds"]!.AsArray().Count == 1 && group["embeds"]![0]!["fields"]!.AsArray().Count == 3);
        c.Check("events at once: each account's facts in its field", group["embeds"]![0]!["fields"]!.AsArray().Take(2).All(f => f!["value"]!.GetValue<string>().Contains("`277`")),
            group.ToJsonString());
        var drop = samples.First(s => s.Label == "Alert · disconnected, rejoining" && s.Language == "en").Payload["embeds"]![0]!;
        c.Check("an account's alert: its name and avatar above, the game's icon beside, game, time played and reason",
            drop["author"]?["name"]?.GetValue<string>() == "NoobFarm2" && drop["author"]?["icon_url"] is not null && drop["thumbnail"]!["url"]!.GetValue<string>().Contains("GameIcon")
            && drop["fields"]!.AsArray().Count == 3 && drop["fields"]![2]!["value"]!.GetValue<string>().Contains("277"), drop.ToJsonString());
        var summary = samples.First(s => s.Label.StartsWith("Summary · daily (") && s.Language == "en");
        c.Check("a summary shows its picture", summary.Payload["embeds"]![0]!["image"]?["url"]?.GetValue<string>() == "attachment://bloxkeep-summary.png"
            && summary.Payload["attachments"] is JsonArray && summary.Files is not null);
    }

    /// <summary>The words of a message, without the names and numbers that are the same in both languages.</summary>
    static string Words(JsonObject payload)
    {
        var e = payload["embeds"]?[0];
        return e is null ? payload["content"]?.GetValue<string>() ?? "" : $"{e["title"]}|{e["description"]}|{e["footer"]?["text"]}";
    }

    /// <summary>dev/ of the checkout these checks were built from, when there is one.</summary>
    static string? FindDev()
    {
        for (var at = new DirectoryInfo(AppContext.BaseDirectory); at is not null; at = at.Parent)
        {
            if (File.Exists(Path.Combine(at.FullName, "dev", "discord-preview.html")))
                return Path.Combine(at.FullName, "dev");
        }
        return null;
    }

    /// <summary>A stand-in for a picture Roblox would give: the name's first letters on a colour.</summary>
    static byte[] StandIn(string name, Color color, bool round)
    {
        using var bmp = new Bitmap(150, 150, PixelFormat.Format32bppArgb);
        using (var g = Graphics.FromImage(bmp))
        {
            g.SmoothingMode = SmoothingMode.AntiAlias;
            g.TextRenderingHint = TextRenderingHint.AntiAliasGridFit;
            using var fill = new SolidBrush(color);
            if (round)
                g.FillEllipse(fill, 0, 0, 150, 150);
            else
                g.FillRectangle(fill, 0, 0, 150, 150);
            string letters = new string(name.Where(char.IsLetterOrDigit).Take(2).ToArray()).ToUpperInvariant();
            using var font = new Font("Segoe UI Semibold", 54, GraphicsUnit.Pixel);
            var size = g.MeasureString(letters, font);
            g.DrawString(letters, font, Brushes.White, (150 - size.Width) / 2, (150 - size.Height) / 2);
        }
        using var png = new MemoryStream();
        bmp.Save(png, ImageFormat.Png);
        return png.ToArray();
    }
}
