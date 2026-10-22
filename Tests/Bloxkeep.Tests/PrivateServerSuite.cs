using System.Net;
using System.Text;
using System.Text.Json;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// Your private servers in Browse servers: each selected account asks Roblox (games' /v1/games/{placeId}/private-servers,
/// in the shape Roblox's API reference gives) with its own sign-in, the servers are merged by access code with the
/// accounts that can join each, and one picked goes in the game box as roblox://placeId=…&amp;accessCode=…, which
/// GameTarget reads and the launcher sends as RequestPrivateGame&amp;accessCode=…, like the website's Join button.
/// Roblox here is a stand-in (RobloxApi.StandIn); nothing is launched.
/// </summary>
static class PrivateServerSuite
{
    const long Place = 2753915549;
    const string Shared = "11111111-2222-4333-8444-555555555555", OnlyA = "66666666-7777-4888-8999-000000000000";

    public static async Task Run(Checks c, string dir)
    {
        // ── The game box's deep link ──
        bool ok = GameTarget.TryParse($"roblox://placeId={Place}&accessCode={Shared.ToUpperInvariant()}", "", out var target, out string error);
        c.Check("roblox://placeId=…&accessCode=…: that place, that access code, a private server", ok && target is { PlaceId: Place, AccessCode: Shared, LinkCode: null, IsPrivate: true }, target ?? (object)error);
        ok = GameTarget.TryParse($"roblox://placeId={Place}&accessCode=not-a-code", "", out target, out error);
        c.Check("…a code that isn't one: refused, saying so", !ok && error == "Invalid private server access code.", error);
        ok = GameTarget.TryParse($"https://www.roblox.com/games/{Place}", "", out target, out _);
        c.Check("a plain game link: no access code, not private", ok && target is { AccessCode: null, IsPrivate: false }, target);

        string link = Uri.UnescapeDataString(RobloxLauncher.BuildLaunchUri("TICKET", new GameTarget(Place) { AccessCode = Shared }));
        c.Check("the launch: RequestPrivateGame with the access code, as the website's Join button asks",
            link.Contains($"request=RequestPrivateGame&", StringComparison.Ordinal) && link.Contains($"&placeId={Place}&", StringComparison.Ordinal)
            && link.Contains($"&accessCode={Shared}&", StringComparison.Ordinal) && !link.Contains("gameId=", StringComparison.Ordinal), link);

        string json = JsonSerializer.Serialize(new GameTarget(Place) { AccessCode = Shared });
        var back = JsonSerializer.Deserialize<GameTarget>(json);
        c.Check("saved with the open and waiting sessions and read back: the access code kept (IsPrivate isn't stored)",
            back is { AccessCode: Shared, IsPrivate: true } && !json.Contains("IsPrivate", StringComparison.Ordinal), json);

        // ── Asking Roblox ──
        Harness.UseDataFolder(dir);
        var seed = Vault.Create(Harness.Password);
        seed.Data.Accounts.Add(new Account { Username = "Alpha", UserId = 31, Cookie = "cookie-a" });
        seed.Data.Accounts.Add(new Account { Username = "Beta", UserId = 32, Cookie = "cookie-b" });
        seed.Data.Accounts.Add(new Account { Username = "Gamma", UserId = 33, Cookie = "cookie-c" });
        seed.Save();
        using var form = new Form();
        using var app = new AppController(form, _ => { });
        await Harness.Unlock(app);
        var vault = Harness.Get<Vault>(app, "_vault");
        Guid A = vault.Data.Accounts.Single(a => a.UserId == 31).Id, B = vault.Data.Accounts.Single(a => a.UserId == 32).Id;

        var asked = new List<string>();
        bool failB = false;
        object Server(string code, string name, int playing) => new
        {
            id = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee", maxPlayers = 12, playing, playerTokens = Array.Empty<string>(), players = Array.Empty<object>(),
            fps = 59.9, ping = 80, name, vipServerId = code == Shared ? 901 : 902, accessCode = code,
            owner = new { id = 31, name = "alpha_owner", displayName = "Alpha", hasVerifiedBadge = false },
        };
        async Task<HttpResponseMessage> Roblox(HttpRequestMessage request)
        {
            await Task.Yield();
            string cookie = request.Headers.TryGetValues("Cookie", out var sent) ? sent.First().Replace(".ROBLOSECURITY=", "") : "";
            var uri = request.RequestUri!;
            if (uri.Host != "games.roblox.com" || uri.AbsolutePath != $"/v1/games/{Place}/private-servers")
                return Json(HttpStatusCode.NotFound, new { });
            lock (asked)
                asked.Add(cookie + uri.Query);
            if (cookie == "cookie-b" && failB)
                return Json(HttpStatusCode.TooManyRequests, new { });
            // Alpha's list comes in two pages; Beta sees only the shared server; a server without an access code is left out.
            return cookie switch
            {
                "cookie-a" when !uri.Query.Contains("exclusiveStartKeyCursor") => Json(HttpStatusCode.OK, new { data = new[] { Server(Shared, "Farm", 3) }, nextPageCursor = "page2" }),
                "cookie-a" => Json(HttpStatusCode.OK, new { data = new object[] { Server(OnlyA, "Base", 7), new { name = "No code", playing = 1, maxPlayers = 5 } }, nextPageCursor = (string?)null }),
                "cookie-b" => Json(HttpStatusCode.OK, new { data = new[] { Server(Shared, "Farm", 3) }, nextPageCursor = (string?)null }),
                _ => Json(HttpStatusCode.OK, new { data = Array.Empty<object>() }),
            };
        }
        var standIn = typeof(RobloxApi).GetProperty("StandIn", Harness.F)!;
        standIn.SetValue(null, (Func<HttpRequestMessage, Task<HttpResponseMessage>>)Roblox);
        try
        {
            var list = await RobloxApi.GetPrivateServersAsync("cookie-a", Place);
            c.Check("one account's list: both pages read (the next asked with exclusiveStartKeyCursor), the server without a code left out",
                list.Select(s => s.AccessCode).SequenceEqual([Shared, OnlyA]) && asked.Any(q => q.EndsWith("?exclusiveStartKeyCursor=page2", StringComparison.Ordinal)),
                string.Join(",", list.Select(s => s.Name)));
            c.Check("…each with its name, owner, players and running server",
                list[0] is { Id: 901, Name: "Farm", OwnerName: "alpha_owner", OwnerDisplayName: "Alpha", Playing: 3, MaxPlayers: 12, JobId: "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee" }, list[0]);

            asked.Clear();
            var reply = await (Task<object?>)Harness.Invoke(app, "PrivateServersAsync", Place, new[] { A, B })!;
            var dto = JsonDocument.Parse(JsonSerializer.Serialize(reply)).RootElement;
            var servers = dto.GetProperty("servers").EnumerateArray().ToList();
            string Code(JsonElement s) => s.GetProperty("accessCode").GetString()!;
            string[] Via(JsonElement s) => s.GetProperty("via").EnumerateArray().Select(v => v.GetString()!).ToArray();
            c.Check("two selected accounts: each asked with its own sign-in, the third (not selected) not asked",
                asked.Select(q => q.Split('?')[0]).Distinct().OrderBy(x => x).SequenceEqual(["cookie-a", "cookie-b"]), string.Join(",", asked));
            c.Check("…the server both can join once, first, saying both can join",
                servers.Count == 2 && Code(servers[0]) == Shared && Via(servers[0]).OrderBy(x => x).SequenceEqual(["Alpha", "Beta"]), dto);
            c.Check("…the one only Alpha can join after it, saying only Alpha can", Code(servers[1]) == OnlyA && Via(servers[1]).SequenceEqual(["Alpha"]), dto);
            c.Check("…owner shown by display name", servers[0].GetProperty("owner").GetString() == "Alpha", dto);

            failB = true;
            Harness.Get<System.Collections.IDictionary>(app, "_privateServers").Clear();
            reply = await (Task<object?>)Harness.Invoke(app, "PrivateServersAsync", Place, new[] { A, B })!;
            dto = JsonDocument.Parse(JsonSerializer.Serialize(reply)).RootElement;
            c.Check("Roblox refusing one account (429): the other's servers still listed, not marked failed",
                dto.GetProperty("servers").GetArrayLength() == 2 && !dto.GetProperty("failed").GetBoolean(), dto);
            reply = await (Task<object?>)Harness.Invoke(app, "PrivateServersAsync", Place, new[] { B })!;
            dto = JsonDocument.Parse(JsonSerializer.Serialize(reply)).RootElement;
            c.Check("…only that account selected: failed, so the page says it couldn't load instead of \"none\"",
                dto.GetProperty("failed").GetBoolean() && dto.GetProperty("servers").GetArrayLength() == 0, dto);

            // The dock: the game box with the deep link is a private server.
            var game = await (Task<object?>)Harness.Invoke(app, "ResolveGameAsync", $"roblox://placeId={Place}&accessCode={Shared}")!;
            var g = JsonDocument.Parse(JsonSerializer.Serialize(game)).RootElement;
            c.Check("the game box holding it reads as a private server of that game",
                g.GetProperty("ok").GetBoolean() && g.GetProperty("kind").GetString() == "private" && g.GetProperty("placeId").GetInt64() == Place, g);
        }
        finally
        {
            standIn.SetValue(null, null);
        }
    }

    static HttpResponseMessage Json(HttpStatusCode status, object body) =>
        new(status) { Content = new StringContent(JsonSerializer.Serialize(body), Encoding.UTF8, "application/json") };
}
