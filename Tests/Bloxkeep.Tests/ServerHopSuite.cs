using System.Net;
using System.Text;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// "Switch to another server" (AppController.ServerHop.cs): windows playing in a public server are closed and opened
/// again into another server of their game with room, the same one for accounts of the same game, never the one they
/// leave; a private server or a window not in a game is left alone; no other server with room: said, nothing closed.
/// Roblox's server list is a stand-in (RobloxApi.StandIn); no window is closed.
/// </summary>
static class ServerHopSuite
{
    const long Place = 5151;

    public static async Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        var seed = Vault.Create(Harness.Password);
        var a1 = new Account { Username = "hop1", Cookie = "x1", UserId = 2001, CookieValid = true };
        var a2 = new Account { Username = "hop2", Cookie = "x2", UserId = 2002, CookieValid = true };
        var a3 = new Account { Username = "private", Cookie = "x3", UserId = 2003, CookieValid = true };
        seed.Data.Accounts.AddRange([a1, a2, a3]);
        seed.Save();
        using var form = new Form();
        using var app = new AppController(form, _ => { });
        await Harness.Unlock(app);

        var t = typeof(AppController);
        var running = Harness.Get<System.Collections.IDictionary>(app, "_running");
        var launchType = t.GetNestedType("PendingLaunch", Harness.F)!;
        var tupleType = running.GetType().GetGenericArguments()[1];
        var watcher = Harness.Get<RobloxLogWatcher>(app, "_logs");
        var tails = Harness.Get<System.Collections.IDictionary>(watcher, "_tails");
        var tailType = typeof(RobloxLogWatcher).GetNestedType("Tail", Harness.F)!;
        void Window(Account account, int pid, GameTarget target, string status, string? job, Guid? preset = null)
        {
            var launch = Activator.CreateInstance(launchType, account.Id, new HashSet<int>(), DateTime.UtcNow.AddMinutes(-5), target, false, preset)!;
            running[account.Id] = Activator.CreateInstance(tupleType, pid, launch);
            var tail = Activator.CreateInstance(tailType, $"stand-in-{pid}.log", pid)!;
            tailType.GetField("State")!.SetValue(tail, new RobloxLogWatcher.ClientState(status, target.PlaceId, job, null, DateTime.UtcNow));
            tails[pid] = tail;
        }
        Window(a1, 801, new GameTarget(Place), "ingame", "old-server");
        Window(a2, 802, new GameTarget(Place), "ingame", "old-server");
        Window(a3, 803, new GameTarget(Place) { AccessCode = "private-code" }, "ingame", "private-server");
        var closed = new List<int>();
        Harness.Set(app, "_closeForReopen", (Action<int, bool>)((pid, _) => closed.Add(pid)));
        var restarts = Harness.Get<System.Collections.IDictionary>(app, "_restarts");
        GameTarget? Going(Account a) => restarts.Contains(a.Id) ? (GameTarget)restarts[a.Id]!.GetType().GetField("Item1")!.GetValue(restarts[a.Id])! : null;

        // Roblox's list: the one they're in, one full, one with room for one, one with room for both.
        string list = """{"data":[{"id":"old-server","playing":5,"maxPlayers":20},{"id":"full","playing":20,"maxPlayers":20},{"id":"one-left","playing":19,"maxPlayers":20},{"id":"roomy","playing":3,"maxPlayers":20}],"nextPageCursor":null}""";
        int serverErrors = 0; // the next server lists Roblox answers with a 500
        Task<HttpResponseMessage> Roblox(HttpRequestMessage req)
        {
            var uri = req.RequestUri!;
            bool servers = uri.AbsolutePath.Contains("/servers/Public");
            if (servers && serverErrors > 0)
            {
                serverErrors--;
                return Task.FromResult(new HttpResponseMessage(HttpStatusCode.InternalServerError) { Content = new StringContent("{}") });
            }
            // A game's details: its name from develop.roblox.com, its player count garbled (a cut-off answer).
            string body = servers ? list
                : uri.AbsolutePath == "/v1/universes/multiget" ? $$"""{"data":[{"id":77,"name":"Hop Game","creatorName":"Maker","rootPlaceId":{{Place}}}]}"""
                : uri.Host == "games.roblox.com" && uri.AbsolutePath == "/v1/games" ? """{"data":[{"id":77,"pla"""
                : "{}";
            return Task.FromResult(new HttpResponseMessage(HttpStatusCode.OK) { Content = new StringContent(body, Encoding.UTF8, "application/json") });
        }
        var standIn = typeof(RobloxApi).GetProperty("StandIn", Harness.F)!;
        standIn.SetValue(null, (Func<HttpRequestMessage, Task<HttpResponseMessage>>)Roblox);
        try
        {
            var onlyPrivate = await Harness.Try(app, "switchServer", new { ids = new[] { a3.Id } });
            c.Check("a private server only: said, nothing closed", onlyPrivate is UserError && closed.Count == 0, onlyPrivate);

            var moved = await Harness.Try(app, "switchServer", new { ids = new[] { a1.Id, a2.Id, a3.Id } });
            c.Check("two accounts of a game: both closed to reopen, the private one left alone", moved is 2 && closed.Order().SequenceEqual([801, 802])
                && !restarts.Contains(a3.Id), string.Join(",", closed));
            c.Check("…into the same server with room for both, not the one they left", Going(a1) is { PlaceId: Place, JobId: "roomy" } && Going(a2) == Going(a1), Going(a1));
            var hopping = Harness.Get<Dictionary<Guid, DateTime>>(app, "_hopping");
            bool Hopping() => (bool)Harness.Invoke(app, "Hopping")!;
            c.Check("…marked as moving, so a preset's Rejoin server doesn't send them back", hopping.Keys.ToHashSet().SetEquals([a1.Id, a2.Id]) && Hopping());
            // 2026-10-30 review: a reopening that never came (the window closed for good, everything closed) left the mark,
            // and /pc switch answered "Accounts are still moving" for ever.
            foreach (var id in hopping.Keys.ToList())
                hopping[id] = DateTime.UtcNow - TimeSpan.FromMinutes(10);
            c.Check("…a mark no reopening took counts no more after a few minutes (fails on the old code)", !Hopping() && hopping.Count == 0);

            // The window closed while Roblox gave the list: nothing to move, and nothing marked.
            closed.Clear();
            restarts.Clear();
            Window(a1, 821, new GameTarget(Place), "ingame", "old-server");
            Func<HttpRequestMessage, Task<HttpResponseMessage>> closing = req =>
            {
                running.Remove(a1.Id);
                return Roblox(req);
            };
            standIn.SetValue(null, closing);
            var gone = await Harness.Try(app, "switchServer", new { ids = new[] { a1.Id } });
            c.Check("a window that closed while Roblox answered: not closed again, not marked as moving (fails on the old code)",
                gone is 0 && closed.Count == 0 && hopping.Count == 0 && !restarts.Contains(a1.Id), $"{gone} {string.Join(",", closed)} {hopping.Count}");
            standIn.SetValue(null, (Func<HttpRequestMessage, Task<HttpResponseMessage>>)Roblox);

            // Locked after the switch: the reopenings went with the vault, and so do the marks.
            Window(a1, 822, new GameTarget(Place), "ingame", "old-server");
            await Harness.Call(app, "switchServer", new { ids = new[] { a1.Id } });
            bool marked = hopping.Count == 1;
            await Harness.Call(app, "lock", new { });
            c.Check("locked after a switch: no account is still marked as moving (fails on the old code)", marked && hopping.Count == 0);
            await Harness.Unlock(app);

            closed.Clear();
            restarts.Clear();
            Window(a1, 811, new GameTarget(Place), "ingame", "roomy");
            list = """{"data":[{"id":"roomy","playing":5,"maxPlayers":20},{"id":"full","playing":20,"maxPlayers":20}],"nextPageCursor":null}""";
            var none = await Harness.Try(app, "switchServer", new { ids = new[] { a1.Id } });
            c.Check("no other server with room: nothing closed", none is 0 && closed.Count == 0, none);

            Window(a2, 812, new GameTarget(Place), "joining", null);
            var notIn = await Harness.Try(app, "switchServer", new { ids = new[] { a2.Id } });
            c.Check("a window still joining: said, nothing closed", notIn is UserError && closed.Count == 0, notIn);

            // ── the server window (review, 2026-10-30) ──
            // A one-off 500 is said, and not kept: before, the window showed "no servers" for the minute the list is kept.
            serverErrors = 1;
            object? failedList;
            try
            {
                failedList = await Harness.Call(app, "servers", new { placeId = Place + 1, fewest = false });
            }
            catch (RobloxApiException ex)
            {
                failedList = ex;
            }
            var afterIt = await Harness.Try(app, "servers", new { placeId = Place + 1, fewest = false });
            int? Count(object? reply) => (reply?.GetType().GetProperty("servers")?.GetValue(reply) as System.Collections.ICollection)?.Count;
            c.Check("the server list answered with a 500: said, not shown as no servers", failedList is Exception, failedList);
            c.Check("…asked again right after: Roblox's list, not an empty one kept from the failure", Count(afterIt) == 2, afterIt);

            // A game's player count that came back garbled leaves the count blank, not the name (before: the name lost too,
            // and remembered as unknown for 2 minutes).
            var games = await RobloxApi.GetGamesAsync([77], "x1");
            c.Check("a game's player count answer garbled: its name still comes back", games.GetValueOrDefault(77)?.Name == "Hop Game",
                string.Join(",", games.Values.Select(g => g.Name)));

            // ── each account in a server of its own (a preset's "Own server each") ──
            // a1 and a2 play in old-server and a3 in the private one: the accounts that open go elsewhere, one each.
            Window(a1, 821, new GameTarget(Place), "ingame", "old-server");
            Window(a2, 822, new GameTarget(Place), "ingame", "old-server");
            list = """{"data":[{"id":"old-server","playing":5,"maxPlayers":20},{"id":"full","playing":20,"maxPlayers":20},{"id":"one-left","playing":19,"maxPlayers":20},{"id":"roomy","playing":3,"maxPlayers":20}],"nextPageCursor":null}""";
            var each = (Func<GameTarget, CancellationToken, Task<GameTarget>>)Harness.Invoke(app, "ServersApart", "fewest")!;
            var first = await each(new GameTarget(Place), default);
            var second = await each(new GameTarget(Place), default);
            var third = await each(new GameTarget(Place), default);
            c.Check("own server each: the first with room, then the next, never where the accounts already are",
                first.JobId == "one-left" && second.JobId == "roomy", $"{first.JobId} {second.JobId}");
            c.Check("…none left: Roblox picks for the rest", third.JobId is null, third.JobId);

            // ── leaving a crowded server ──
            // a1 alone in old-server (5 there, 4 others), a2 in roomy with 2 others: with "more than 3", only a1 moves, to a
            // server with 3 or fewer that none of them is in; with "more than 1", a1 has nowhere to go.
            closed.Clear();
            restarts.Clear();
            var onTheirWay = Harness.Get<System.Collections.IDictionary>(app, "_hopping");
            onTheirWay.Clear();
            void Joined(Account account, int pid, string job, DateTime at, Guid? preset = null)
            {
                Window(account, pid, new GameTarget(Place), "ingame", job, preset);
                tailType.GetField("State")!.SetValue(tails[pid], new RobloxLogWatcher.ClientState("ingame", Place, job, null, at));
            }
            running.Remove(a3.Id);
            Joined(a1, 831, "old-server", DateTime.UtcNow.AddMinutes(-5));
            Joined(a2, 832, "roomy", DateTime.UtcNow.AddMinutes(-5));
            list = """{"data":[{"id":"quiet","playing":2,"maxPlayers":20},{"id":"roomy","playing":3,"maxPlayers":20},{"id":"old-server","playing":5,"maxPlayers":20}],"nextPageCursor":null}""";
            var settings = Harness.Get<Vault>(app, "_vault").Data.Settings;
            async Task CountServers(int above)
            {
                settings.HopAbovePlayers = above;
                Harness.Set(app, "_nextCrowdCheck", DateTime.MinValue);
                Harness.Invoke(app, "HopCrowdedClients", DateTime.UtcNow);
                for (int i = 0; i < 100 && Harness.Get<bool>(app, "_crowdBusy"); i++)
                    await Task.Delay(20);
            }
            await CountServers(0);
            c.Check("leave crowded servers off: nothing closed", closed.Count == 0);
            await CountServers(1);
            c.Check("no server quiet enough: nothing closed", closed.Count == 0, string.Join(",", closed));
            await CountServers(3);
            c.Check("a crowded server: its window moves to a quiet one, the others stay", closed.SequenceEqual([831]) && Going(a1)?.JobId == "quiet",
                $"{string.Join(",", closed)} {Going(a1)?.JobId}");
            closed.Clear();
            await CountServers(3);
            c.Check("…not counted again while one is on its way", closed.Count == 0);
            onTheirWay.Clear();
            restarts.Clear();
            Joined(a1, 833, "old-server", DateTime.UtcNow);
            await CountServers(3);
            c.Check("a window just in is left alone", closed.Count == 0, string.Join(",", closed));
            // A preset that sends its accounts to a server on purpose (its Job ID, or the fullest one) keeps them there.
            foreach (var server in new[] { "job", "most" })
            {
                var preset = new LaunchPreset { Name = server, Server = server, AccountIds = [a1.Id] };
                settings.Presets.Add(preset);
                Joined(a1, 834, "old-server", DateTime.UtcNow.AddMinutes(-5), preset.Id);
                await CountServers(3);
                c.Check($"a preset's window sent to a server on purpose (\"{server}\") stays in it, crowded or not (fails on the old code)",
                    closed.Count == 0, string.Join(",", closed));
                settings.Presets.Remove(preset);
            }
            closed.Clear();
            restarts.Clear();
            onTheirWay.Clear();
            var anyServer = new LaunchPreset { Name = "any", Server = "any", AccountIds = [a1.Id] };
            settings.Presets.Add(anyServer);
            Joined(a1, 835, "old-server", DateTime.UtcNow.AddMinutes(-5), anyServer.Id);
            await CountServers(3);
            c.Check("…one where Roblox picked the server still moves", closed.SequenceEqual([835]), string.Join(",", closed));
            settings.Presets.Remove(anyServer);
        }
        finally
        {
            standIn.SetValue(null, null);
        }
    }
}
