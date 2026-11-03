using System.Net;
using System.Text;
using System.Text.Json;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// Following a player on auto-rejoin: "Join a player…" with its box ticked (or a preset that follows someone) keeps the
/// player in the target, through drops, restarts and Bloxkeep closing; each rejoin asks Roblox where the player is now,
/// with the rejoining account first and the other signed-in ones after, and goes to that server, checked with Roblox's
/// join answer. Offline, the website, Studio, a hidden, full or ended server, a game that bans or refuses the account:
/// the rejoin waits a minute and looks again, said once per reason. Roblox here is a stand-in (RobloxApi.StandIn and
/// the join answer's field); nothing is launched.
/// </summary>
static class FollowSuite
{
    const long Friend = 77, Place = 111, OtherPlace = 222;
    const string Server = "11111111-2222-3333-4444-555555555555";

    public static async Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        var seed = Vault.Create(Harness.Password);
        seed.Data.Accounts.Add(new Account { Username = "Rejoiner", UserId = 21, Cookie = "cookie-a", AutoRejoin = true });
        seed.Data.Accounts.Add(new Account { Username = "Other", UserId = 22, Cookie = "cookie-b" });
        seed.Save();
        using var form = new Form();
        var posted = new List<string>();
        using var app = new AppController(form, json => { lock (posted) posted.Add(json); });
        await Harness.Unlock(app);
        var t = typeof(AppController);
        var vault = Harness.Get<Vault>(app, "_vault");
        var account = vault.Data.Accounts.Single(a => a.UserId == 21);
        var other = vault.Data.Accounts.Single(a => a.UserId == 22);
        var queue = Harness.Get<System.Collections.IList>(app, "_rejoinQueue");
        var running = Harness.Get<System.Collections.IDictionary>(app, "_running");
        var launchType = t.GetNestedType("PendingLaunch", Harness.F)!;
        var requestType = t.GetNestedType("RejoinRequest", Harness.F)!;
        var follow = new FollowedPlayer(Friend, "Friend");

        // The last toast since the last call (the page's copy of a notification).
        string? Told()
        {
            string? said = null;
            lock (posted)
            {
                foreach (string json in posted)
                {
                    using var doc = JsonDocument.Parse(json);
                    if (doc.RootElement.TryGetProperty("name", out var name) && name.GetString() == "toast")
                        said = doc.RootElement.GetProperty("data").GetProperty("text").GetString();
                }
                posted.Clear();
            }
            return said;
        }

        // Roblox's presence service as each sign-in is shown it, and the order they asked in.
        var seen = new Dictionary<string, object>();
        var asked = new List<string>();
        bool rateLimited = false;
        // Set: the presence answer waits for it (a slow Roblox), after telling it was asked.
        TaskCompletionSource? presenceGate = null;
        var presenceAsked = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        async Task<HttpResponseMessage> Roblox(HttpRequestMessage request)
        {
            string cookie = request.Headers.TryGetValues("Cookie", out var sent) ? sent.First().Replace(".ROBLOSECURITY=", "") : "";
            string body = request.Content is null ? "" : await request.Content.ReadAsStringAsync();
            // Never a sign-in ticket: nothing here can open Roblox. A 503 is Roblox busy, which a rejoin tries again later.
            if (request.RequestUri!.AbsolutePath.StartsWith("/v1/authentication-ticket", StringComparison.Ordinal))
                return Json(HttpStatusCode.ServiceUnavailable, new { });
            if (request.RequestUri!.AbsolutePath != "/v1/presence/users")
                return Json(HttpStatusCode.NotFound, new { });
            if (!request.Headers.Contains("x-csrf-token"))
            {
                var csrf = Json(HttpStatusCode.Forbidden, new { errors = new[] { new { code = 0, message = "Token Validation Failed" } } });
                csrf.Headers.Add("x-csrf-token", "token");
                return csrf;
            }
            lock (asked)
                asked.Add(cookie);
            if (presenceGate is { } gate)
            {
                presenceAsked.TrySetResult();
                await gate.Task;
            }
            if (rateLimited)
                return Json(HttpStatusCode.TooManyRequests, new { });
            var presences = body.Contains(Friend.ToString()) && seen.TryGetValue(cookie, out var p) ? new[] { p } : [];
            return Json(HttpStatusCode.OK, new { userPresences = presences });
        }
        object InServer(long place, string job) => new { userPresenceType = 2, lastLocation = "A game", placeId = place, rootPlaceId = place, gameId = job, universeId = 9, userId = Friend };
        object InHiddenServer() => new { userPresenceType = 2, lastLocation = "A game", userId = Friend };
        object Offline() => new { userPresenceType = 0, lastLocation = "", userId = Friend };
        object Website() => new { userPresenceType = 1, lastLocation = "Website", userId = Friend };

        var joinAnswer = new RobloxApi.JoinAnswer(200, 2, null);
        var joinAsked = new List<GameTarget>();
        var realAsk = Harness.Get<object>(app, "_askJoin");
        Harness.Set(app, "_askJoin", (Func<Account, GameTarget, CancellationToken, Task<RobloxApi.JoinAnswer>>)((_, target, _) =>
        {
            lock (joinAsked)
                joinAsked.Add(target);
            return Task.FromResult(joinAnswer);
        }));
        var standIn = typeof(RobloxApi).GetProperty("StandIn", Harness.F)!;
        standIn.SetValue(null, (Func<HttpRequestMessage, Task<HttpResponseMessage>>)Roblox);
        try
        {
            // ── Join a player's box ──
            using (var args = JsonDocument.Parse($$"""{"placeId":{{Place}},"jobId":"{{Server}}","follow":true,"followUserId":{{Friend}},"followName":"Friend"}"""))
            {
                var target = (GameTarget)t.GetMethod("JoinTarget", Harness.F)!.Invoke(null, [args.RootElement])!;
                c.Check("Join a player with its box ticked: the server found, following that player", target is { PlaceId: Place, JobId: Server } && target.Follow == follow, target);
            }
            using (var args = JsonDocument.Parse($$"""{"placeId":{{Place}},"jobId":"{{Server}}","follow":false,"followUserId":{{Friend}},"followName":"Friend"}"""))
            {
                var target = (GameTarget)t.GetMethod("JoinTarget", Harness.F)!.Invoke(null, [args.RootElement])!;
                c.Check("…unticked: the server found, following nobody", target is { PlaceId: Place, JobId: Server, Follow: null }, target);
            }

            // ── kept through a drop, a game picked in the window, and Bloxkeep closing ──
            var opened = new GameTarget(Place, Server) { Follow = follow };
            object Launch(GameTarget target) => Activator.CreateInstance(launchType, account.Id, new HashSet<int>(), DateTime.UtcNow.AddMinutes(-20), target, false, null)!;
            Harness.Invoke(app, "QueueRejoin", account.Id, Launch(opened), null);
            var queued = queue.Cast<object>().SingleOrDefault()?.GetType().GetProperty("Target")!.GetValue(queue.Cast<object>().Single()) as GameTarget;
            c.Check("a drop of a window that follows a player: its rejoin still follows them", queued?.Follow == follow, queued);
            queue.Clear();
            var picked = new RobloxLogWatcher.ClientState("disconnected", OtherPlace, null, 277, DateTime.UtcNow, ChosenPlace: OtherPlace);
            Harness.Invoke(app, "QueueRejoin", account.Id, Launch(opened), picked);
            queued = queue.Cast<object>().SingleOrDefault()?.GetType().GetProperty("Target")!.GetValue(queue.Cast<object>().Single()) as GameTarget;
            c.Check("…a game picked in the window since: still following the player (fails on the old code: a new GameTarget dropped it)",
                queued is { PlaceId: OtherPlace } && queued.Follow == follow, queued);
            queue.Clear();
            var saved = JsonSerializer.Deserialize<WaitingRejoin>(JsonSerializer.Serialize(new WaitingRejoin { AccountId = account.Id, Target = opened }));
            c.Check("a waiting rejoin saved with the vault and read back: still following the player", saved?.Target?.Follow == follow, saved?.Target);

            // ── where a rejoin goes ──
            var followSeen = Harness.Get<System.Collections.IDictionary>(app, "_followSeen");
            // Each check below is a minute after the one before, as a waiting rejoin's: nothing remembered from the last.
            Task<GameTarget?> Follow(GameTarget target, bool fresh = true)
            {
                if (fresh)
                    followSeen.Clear();
                var request = Activator.CreateInstance(requestType, account.Id, target, DateTime.UtcNow, 0, null, false)!;
                return (Task<GameTarget?>)t.GetMethod("FollowTargetAsync", Harness.F)!.Invoke(app, [account, request, CancellationToken.None])!;
            }
            GameTarget? Waiting() => queue.Cast<object>().SingleOrDefault()?.GetType().GetProperty("Target")!.GetValue(queue.Cast<object>().Single()) as GameTarget;
            DateTime? WaitingUntil() => queue.Cast<object>().SingleOrDefault()?.GetType().GetProperty("Due")!.GetValue(queue.Cast<object>().Single()) as DateTime?;

            seen["cookie-a"] = InServer(OtherPlace, "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee");
            asked.Clear();
            Told();
            var went = await Follow(new GameTarget(Place) { Follow = follow });
            c.Check("the player found in another game's server: the rejoin goes there, still following them",
                went is { PlaceId: OtherPlace, JobId: "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee" } && went.Follow == follow, went);
            c.Check("…asked with the rejoining account's own sign-in, and no other once it was shown a server", asked.SequenceEqual(["cookie-a"]), string.Join(",", asked));
            c.Check("…and checked with Roblox's join answer for that server", joinAsked.LastOrDefault() is { PlaceId: OtherPlace, JobId: "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee" }, joinAsked.LastOrDefault());
            c.Check("…nothing waits, nothing said", queue.Count == 0 && Told() is null);

            seen["cookie-a"] = InHiddenServer();
            seen["cookie-b"] = InServer(Place, Server);
            asked.Clear();
            went = await Follow(new GameTarget(Place) { Follow = follow });
            c.Check("a server Roblox shows another of your accounts but not this one: that one's answer is used",
                went is { PlaceId: Place, JobId: Server } && asked.SequenceEqual(["cookie-a", "cookie-b"]), $"{went} asked {string.Join(",", asked)}");

            seen.Remove("cookie-b");
            seen["cookie-a"] = Offline();
            var before = DateTime.UtcNow;
            went = await Follow(new GameTarget(Place) { Follow = follow });
            string? said = Told();
            c.Check("the player offline: no server, the rejoin waits a minute and looks again", went is null && Waiting()?.Follow == follow
                && WaitingUntil() is { } due && due >= before.AddSeconds(55) && due <= DateTime.UtcNow.AddSeconds(65), WaitingUntil());
            c.Check("…said once, as offline", said?.Contains("waits to join Friend: Friend is offline", StringComparison.Ordinal) == true, said);
            queue.Clear();
            went = await Follow(new GameTarget(Place) { Follow = follow });
            c.Check("…still offline a minute later: waits again, not said again", went is null && queue.Count == 1 && Told() is null);
            queue.Clear();

            // Many accounts following someone offline (review, 2026-10-30): a few accounts asked, not every one, and the answer
            // shared for half a minute — before, ten accounts made ~100 presence requests a minute between them.
            var extra = Enumerable.Range(0, 3).Select(i => new Account { Username = $"More{i}", UserId = 30 + i, Cookie = $"cookie-x{i}" }).ToList();
            vault.Data.Accounts.AddRange(extra);
            asked.Clear();
            await Follow(new GameTarget(Place) { Follow = follow });
            int firstAsked = asked.Count;
            queue.Clear();
            await Follow(new GameTarget(Place) { Follow = follow }, fresh: false);
            c.Check("a player offline, five accounts signed in: three asked, not all five", firstAsked == 3, string.Join(",", asked));
            c.Check("…another rejoin following them right after asks Roblox nothing", asked.Count == firstAsked && queue.Count == 1, string.Join(",", asked));
            queue.Clear();
            foreach (var more in extra)
                vault.Data.Accounts.Remove(more);
            Told();

            seen["cookie-a"] = Website();
            went = await Follow(new GameTarget(Place) { Follow = follow });
            said = Told();
            c.Check("now on the website: waits, said again for the new reason", went is null && said?.Contains("Friend is on the Roblox website", StringComparison.Ordinal) == true, said);
            queue.Clear();

            seen["cookie-a"] = InServer(Place, Server);
            joinAnswer = new RobloxApi.JoinAnswer(200, 6, "The game is full");
            went = await Follow(new GameTarget(Place) { Follow = follow });
            said = Told();
            c.Check("the player's server full (Roblox's join answer, status 6): waits instead of opening a window that fails",
                went is null && queue.Count == 1 && said?.Contains("Friend's server is full", StringComparison.Ordinal) == true, said);
            queue.Clear();
            joinAnswer = new RobloxApi.JoinAnswer(200, 5, "The game has ended");
            went = await Follow(new GameTarget(Place) { Follow = follow });
            said = Told();
            c.Check("…a server that has just ended (status 5): waits and looks again", went is null && said?.Contains("has just ended", StringComparison.Ordinal) == true, said);
            queue.Clear();
            joinAnswer = new RobloxApi.JoinAnswer(200, 2, null);

            account.GameBans = [new GameBan { PlaceId = Place, At = DateTime.UtcNow }];
            int joinsBefore = joinAsked.Count;
            went = await Follow(new GameTarget(OtherPlace) { Follow = follow });
            said = Told();
            c.Check("the player's game has banned this account: waits, without asking Roblox to join",
                went is null && joinAsked.Count == joinsBefore && said?.Contains("has banned this account", StringComparison.Ordinal) == true, said);
            queue.Clear();

            // The rejoin itself: a ban in the game it was opened into doesn't stop a follow (the player's game counts),
            // and a player who can't be followed opens no window.
            seen["cookie-a"] = Offline();
            Told();
            running.Clear();
            followSeen.Clear();
            var request =Activator.CreateInstance(requestType, account.Id, new GameTarget(Place) { Follow = follow }, DateTime.UtcNow, 0, null, false)!;
            await (Task)t.GetMethod("RejoinAsync", Harness.F)!.Invoke(app, [request])!;
            c.Check("a rejoin that follows a player, banned in the game it was opened into: still looks for the player (fails on the old code: dropped at once)",
                queue.Count == 1 && Told()?.Contains("Friend is offline", StringComparison.Ordinal) == true);
            c.Check("…and opens no window while the player can't be followed", Harness.Get<System.Collections.IList>(app, "_pending").Count == 0
                && !Harness.Get<bool>(app, "_rejoining"));
            queue.Clear();
            account.GameBans.Clear();

            // Roblox couldn't answer at all: thrown, so the rejoin's own retry for a lost connection takes it.
            rateLimited = true;
            Exception? failed = null;
            try
            {
                await Follow(new GameTarget(Place) { Follow = follow });
            }
            catch (Exception ex)
            {
                failed = ex;
            }
            c.Check("Roblox rate-limiting the question: thrown for the rejoin's usual retry, nothing waits here",
                failed is RobloxApiException { RateLimited: true } && queue.Count == 0, failed);
            rateLimited = false;

            // ── Discord ──
            seen["cookie-a"] = Offline();
            Told();
            await Follow(new GameTarget(Place) { Follow = follow });
            said = Told();
            queue.Clear();
            var made = ("{0} waits to join {1}: {1} is offline, or hides being online. Bloxkeep looks again every minute.", new object?[] { account.Label, "Friend" });
            var alert = Harness.Invoke(app, "AlertFor", "rejoinFail", account.Id, made, said, DateTime.UtcNow)!;
            string? title = alert.GetType().GetProperty("Title")!.GetValue(alert) as string;
            c.Check("Discord's alert: waiting to join a player (not the memory wait's title)", title == "Waiting to join a player", title);

            // ── a preset that follows a player ──
            var preset = new LaunchPreset { Name = "With Friend", AccountIds = [account.Id], Server = "follow", FollowPlayer = "Friend", FollowUserId = Friend };
            Task<GameTarget> PresetTarget() => (Task<GameTarget>)t.GetMethod("PresetFollowTargetAsync", Harness.F)!
                .Invoke(app, [vault, preset, new List<Account> { account }, CancellationToken.None])!;
            seen["cookie-a"] = InServer(Place, Server);
            var presetTarget = await PresetTarget();
            c.Check("a preset that follows a player: opens into their server, its windows following them",
                presetTarget is { PlaceId: Place, JobId: Server } && presetTarget.Follow == follow, presetTarget);
            seen["cookie-a"] = Offline();
            Exception? refused = null;
            try
            {
                await PresetTarget();
            }
            catch (Exception ex)
            {
                refused = ex;
            }
            c.Check("…the player offline: the preset doesn't run, and says why", refused is UserError && refused.Message.Contains("Friend is offline", StringComparison.Ordinal), refused?.Message);

            // ── Close wins over a rejoin already under way ──
            // The rejoin has left the queue and waits on Roblox (here its join answer for the player's server) when the person
            // closes the account, the scheduled stop comes, or its rejoin is called off. Before, the rejoin went on: it waited
            // again or went back in the queue (Roblox busy), and with a ticket it would have opened the window.
            vault.Data.Settings.MultiInstance = false;
            var pending = Harness.Get<System.Collections.IList>(app, "_pending");
            var rejoinAsync = t.GetMethod("RejoinAsync", Harness.F)!;
            bool cardRejoining = true;
            async Task<string> StoppedUnderWay(RobloxApi.JoinAnswer answer, Action stop)
            {
                running.Clear();
                queue.Clear();
                followSeen.Clear();
                seen["cookie-a"] = InServer(Place, Server);
                var joinGate = new TaskCompletionSource<RobloxApi.JoinAnswer>(TaskCreationOptions.RunContinuationsAsynchronously);
                var joinAskedNow = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
                Harness.Set(app, "_askJoin", (Func<Account, GameTarget, CancellationToken, Task<RobloxApi.JoinAnswer>>)((_, _, _) =>
                {
                    joinAskedNow.TrySetResult();
                    return joinGate.Task;
                }));
                var request = Activator.CreateInstance(requestType, account.Id, new GameTarget(Place) { Follow = follow }, DateTime.UtcNow, 0, null, false)!;
                var rejoin = (Task)rejoinAsync.Invoke(app, [request])!;
                await joinAskedNow.Task.WaitAsync(TimeSpan.FromSeconds(10));
                var card = Harness.Invoke(app, "AccountDto", account)!;
                cardRejoining &= card.GetType().GetProperty("rejoin")!.GetValue(card) is not null;
                stop();
                joinGate.SetResult(answer);
                await rejoin.WaitAsync(TimeSpan.FromSeconds(30));
                string state = $"queue {queue.Count}, pending {pending.Count}, rejoining {Harness.Get<bool>(app, "_rejoining")}";
                queue.Clear();
                pending.Clear();
                return state;
            }
            const string Nothing = "queue 0, pending 0, rejoining False";
            var full = new RobloxApi.JoinAnswer(200, 6, "The game is full");
            var letIn = new RobloxApi.JoinAnswer(200, 2, null);
            string after = await StoppedUnderWay(full, () => Harness.Invoke(app, "CloseAccounts", new[] { account.Id }, false));
            c.Check("Close while a rejoin looks for its player: the rejoin stops, it doesn't wait to look again", after == Nothing, after);
            after = await StoppedUnderWay(letIn, () => Harness.Invoke(app, "CloseAccounts", new[] { account.Id }, false));
            c.Check("…nor goes on to open the window (Roblox busy: it would go back in the queue)", after == Nothing, after);
            after = await StoppedUnderWay(full, () => Harness.Invoke(app, "CancelRejoin", account.Id));
            c.Check("…its rejoin called off on the card: stops too", after == Nothing, after);
            c.Check("…and while it is under way, the card says it's rejoining (Cancel and Close on it), not idle", cardRejoining);
            var runType = t.GetNestedType("ScheduledRun", Harness.F)!;
            var run = Activator.CreateInstance(runType)!;
            runType.GetField("PresetId")!.SetValue(run, Guid.NewGuid());
            runType.GetField("Vault")!.SetValue(run, vault.FilePath);
            runType.GetField("Name")!.SetValue(run, "Night farm");
            runType.GetField("Opened")!.SetValue(run, new List<Guid> { account.Id });
            after = await StoppedUnderWay(letIn, () => Harness.Invoke(app, "StopScheduledRun", run));
            c.Check("…and a scheduled stop that comes meanwhile stops it", after == Nothing, after);
            Harness.Set(app, "_askJoin", (Func<Account, GameTarget, CancellationToken, Task<RobloxApi.JoinAnswer>>)((_, _, _) => Task.FromResult(joinAnswer)));

            // ── a follow preset holds the launch while it looks for the player ──
            // Before, the launch was taken only once the player was found: a Play (or a scheduled preset) meanwhile started a
            // second launch, and Stop or Close all stopped only one of them.
            vault.Data.Settings.Presets.Add(preset);
            seen["cookie-a"] = InServer(Place, Server);
            followSeen.Clear();
            presenceGate = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
            var presetRun = Harness.Try(app, "runPreset", new { id = preset.Id, force = false });
            await presenceAsked.Task.WaitAsync(TimeSpan.FromSeconds(10));
            // A banned account, so this Play never gets as far as Roblox whatever happens.
            other.Banned = true;
            var second = await Harness.Try(app, "launch", new { ids = new[] { other.Id }, placeInput = Place.ToString(), jobId = "" });
            c.Check("a follow preset looking for its player: a Play meanwhile is told a launch is under way", second is UserError
                && ((UserError)second).Message.StartsWith("Already launching", StringComparison.Ordinal), (second as Exception)?.Message ?? second);
            await Harness.Try(app, "cancelLaunch", new { });
            presenceGate.SetResult();
            presenceGate = null;
            var stopped = await presetRun.WaitAsync(TimeSpan.FromSeconds(30));
            c.Check("…Stop meanwhile stops the preset before it opens anything", stopped?.GetType().GetProperty("Cancelled")?.GetValue(stopped) is true
                && pending.Count == 0, (stopped as Exception)?.Message ?? stopped);
            c.Check("…and the launch is free again", Harness.Get<CancellationTokenSource?>(app, "_launchCts") is null);
            other.Banned = null;
            vault.Data.Settings.Presets.Remove(preset);
        }
        finally
        {
            standIn.SetValue(null, null);
            Harness.Set(app, "_askJoin", realAsk);
        }
    }

    static HttpResponseMessage Json(HttpStatusCode status, object body) =>
        new(status) { Content = new StringContent(JsonSerializer.Serialize(body), Encoding.UTF8, "application/json") };
}
