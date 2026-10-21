using System.Diagnostics;
using System.Text;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// Auto-rejoin on a client that started its game and never got a server (a server that had ended, a full one's join
/// queue), as seen on 2026-10-17: closed after a minute, the rejoin going to any server of the game; and the leave Roblox
/// logs when Bloxkeep closes a client not taken for the person's own. Then the servers a party is sent to: empty ones
/// left out of the list, the first account's client telling whether the server took it, and the server tried next. A
/// teleport whose old server disconnects after the new join isn't a drop; a game the person starts in the window
/// themselves is the one rejoined. Made-up Roblox logs held by this process (the log
/// watcher ties a log to whoever holds it), in a folder of the test's own; Bloxkeep's close refuses this process (not a
/// Roblox client), so nothing is closed.
/// </summary>
static class RejoinSuite
{
    public static async Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        string logs = Path.Combine(dir, "roblox-logs");
        Directory.CreateDirectory(logs);
        Environment.SetEnvironmentVariable("BLOXKEEP_ROBLOX_LOGS", logs);
        int me = Environment.ProcessId;
        string Line(DateTime at, string text) => $"{at:yyyy-MM-ddTHH:mm:ss.fff}Z,1.0,abcd,6 {text}\n";
        const string Launch = "[FLog::SingleSurfaceApp] launchUGCGameInternal";
        const string Join = "[FLog::Output] ! Joining game '11111111-2222-3333-4444-555555555555' place 107778070777162 at 10.0.0.1";
        const string Leave = "[FLog::SingleSurfaceApp] leaveUGCGameInternal";
        const string DeadServer = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";

        var seed = Vault.Create(Harness.Password);
        var account = new Account { Username = "Stuck", UserId = 21 };
        seed.Data.Accounts.Add(account);
        seed.Save();
        using var form = new Form();
        var posted = new List<string>();
        using var app = new AppController(form, json => { lock (posted) posted.Add(json); });
        await Harness.Unlock(app);
        var t = typeof(AppController);
        var vault = Harness.Get<Vault>(app, "_vault");
        account = vault.Data.Accounts.Single();
        var running = Harness.Get<System.Collections.IDictionary>(app, "_running");
        var launchType = t.GetNestedType("PendingLaunch", Harness.F)!;
        var tupleType = running.GetType().GetGenericArguments()[1];
        object Run(GameTarget target)
        {
            var launch = Activator.CreateInstance(launchType, account.Id, new HashSet<int>(), DateTime.UtcNow.AddMinutes(-2), target, false, null)!;
            running[account.Id] = Activator.CreateInstance(tupleType, me, launch);
            return launch;
        }
        var watcher = Harness.Get<RobloxLogWatcher>(app, "_logs");
        var closing = Harness.Get<HashSet<int>>(app, "_closingForRejoin");
        var queue = Harness.Get<System.Collections.IList>(app, "_rejoinQueue");
        // The last toast since the last call (the page's copy of a notification).
        string? Told()
        {
            string? said = null;
            lock (posted)
            {
                foreach (string json in posted)
                {
                    using var doc = System.Text.Json.JsonDocument.Parse(json);
                    if (doc.RootElement.TryGetProperty("name", out var name) && name.GetString() == "toast")
                        said = doc.RootElement.GetProperty("data").GetProperty("text").GetString();
                }
                posted.Clear();
            }
            return said;
        }
        // The log's state once it has read the line just written (the first read waits for the log to be found).
        RobloxLogWatcher.ClientState? Poll(Func<RobloxLogWatcher.ClientState, bool> read)
        {
            var sw = Stopwatch.StartNew();
            while (sw.ElapsedMilliseconds < 15000)
            {
                watcher.Poll([me]);
                if (watcher.StateOf(me) is { } s && read(s))
                    return s;
                Thread.Sleep(200);
            }
            return watcher.StateOf(me);
        }
        void Tick() => Harness.Invoke(app, "RejoinFromLogs");
        // A new window of Bloxkeep's (every launch opens one, with a log of its own): this test's one log starts over.
        void NewWindow()
        {
            var tail = Harness.Get<System.Collections.IDictionary>(watcher, "_tails")[me]!;
            foreach (string field in new[] { "State", "TeleportAt" })
                tail.GetType().GetField(field)!.SetValue(tail, null);
            foreach (string field in new[] { "OwnStart", "Leaving" })
                tail.GetType().GetField(field)!.SetValue(tail, false);
            tail.GetType().GetField("Connections")!.SetValue(tail, 0);
        }

        using var log = new FileStream(Path.Combine(logs, $"0.743.0.7431134_{DateTime.UtcNow:yyyyMMddTHHmmss}Z_Player_REJOIN_last.log"),
            FileMode.Create, FileAccess.ReadWrite, FileShare.ReadWrite | FileShare.Delete);
        void Write(string text) { log.Seek(0, SeekOrigin.End); log.Write(Encoding.UTF8.GetBytes(text)); log.Flush(); }
        try
        {
            // Sent by a preset's server choice to a server that, it turns out, has ended.
            var launch = Run(new GameTarget(107778070777162, DeadServer));
            account.AutoRejoin = true;
            Write(Line(DateTime.UtcNow.AddSeconds(-30), Launch));
            var state = Poll(s => s.Status == "joining");
            c.Check("a game start reads as joining, without a server yet", state is { Status: "joining", JobId: null }, state);
            Tick();
            c.Check("half a minute without a server: still waited for", !closing.Contains(me));

            // A server found, the game still loading after a minute: not this.
            Write(Line(DateTime.UtcNow.AddSeconds(-70), Join));
            Poll(s => s.JobId is not null);
            Tick();
            c.Check("a server found, still loading after a minute: left alone", !closing.Contains(me));

            // The same launch again in a new window, a minute and more without a server.
            NewWindow();
            Write(Line(DateTime.UtcNow.AddSeconds(-70), Launch));
            Poll(s => s.JobId is null);
            account.AutoRejoin = false;
            Tick();
            c.Check("auto-rejoin off: the window stays as it is", !closing.Contains(me));
            account.AutoRejoin = true;
            Tick();
            c.Check("a minute without a server: closed to rejoin", closing.Contains(me));
            string? said = Told();
            c.Check("…saying the server couldn't take it, and that another one is joined",
                said?.Contains("couldn't get into that server (full or closed) — joining another one", StringComparison.Ordinal) == true, said);
            var made = ("{0} couldn't get into that server (full or closed) — joining another one…", new object?[] { account.Label });
            var alert = Harness.Invoke(app, "AlertFor", "drop", account.Id, made, said, DateTime.UtcNow)!;
            string? title = alert.GetType().GetProperty("Title")!.GetValue(alert) as string;
            c.Check("Discord's alert says the same", title == "Couldn't get into the server, joining another", title);

            // Closing it makes Roblox leave the game first (seen on 0.743), and the client ends.
            Write(Line(DateTime.UtcNow, Leave));
            var ended = Harness.Invoke(app, "EndedState", me) as RobloxLogWatcher.ClientState;
            c.Check("the leave Bloxkeep's close made isn't the person's", ended is { Status: "disconnected" }, ended);
            Harness.Invoke(app, "OnSessionEnded", account.Id, launch, ended);
            var request = queue.Cast<object>().SingleOrDefault();
            var target = request?.GetType().GetProperty("Target")!.GetValue(request) as GameTarget;
            c.Check("…so it rejoins: the same game, any server", target is { PlaceId: 107778070777162, JobId: null }, target);
            queue.Clear();

            closing.Clear();
            var own = Harness.Invoke(app, "EndedState", me) as RobloxLogWatcher.ClientState;
            c.Check("a leave nobody else made is still the person's", own is { Status: "left" }, own);
            c.Check("…and isn't rejoined", Harness.Invoke(app, "QueueRejoin", account.Id, launch, own) is false && queue.Count == 0);

            // Sent to the game (Roblox picks the server), and none came.
            Run(new GameTarget(107778070777162));
            NewWindow();
            Write(Line(DateTime.UtcNow.AddSeconds(-65), Launch));
            Poll(s => s.Status == "joining");
            Told();
            Tick();
            said = Told();
            c.Check("a game start without a server, no server asked for: tried again",
                closing.Contains(me) && said?.Contains("couldn't get into the game — trying again", StringComparison.Ordinal) == true, said);
            closing.Clear();

            // The server list: an empty server (its last player gone, so it is closing) is never offered.
            var page = System.Text.Json.JsonDocument.Parse("""
                {"data":[
                  {"id":"one","maxPlayers":7,"playing":1,"fps":60,"ping":54},
                  {"id":"emptied","maxPlayers":7,"fps":60,"ping":686},
                  {"id":"zero","maxPlayers":7,"playing":0,"fps":60},
                  {"id":"","maxPlayers":7,"playing":2},
                  {"maxPlayers":7,"playing":3},
                  {"id":"two","maxPlayers":7,"playing":2,"fps":59.9}
                ]}
                """);
            var read = (List<ServerInfo>)typeof(RobloxApi).GetMethod("ReadServers", Harness.F)!.Invoke(null, [page.RootElement])!;
            c.Check("empty servers (no player count, or 0) left out of the list", read.Select(sv => sv.JobId).SequenceEqual(["one", "two"]),
                string.Join(",", read.Select(sv => sv.JobId)));
            var roomy = t.GetMethod("RoomyServer", Harness.F)!;
            ServerInfo Server(string id, int playing) => new(id, playing, 7, 50, 60);
            var list = new List<ServerInfo> { Server("a", 1), Server("b", 2), Server("c", 3), Server("d", 6) };
            string? Roomy(int party, params string[] closed) => (roomy.Invoke(null, [list, party, new HashSet<string>(closed)]) as ServerInfo)?.JobId;
            c.Check("the next server for a party of 5: room for all, the busiest of those", Roomy(5) == "b", Roomy(5));
            c.Check("…never one found closed", Roomy(5, "b") == "a", Roomy(5, "b"));
            c.Check("…and none when no server has room", Roomy(7) is null, Roomy(7));

            // A party of 5 sent to one server: what the first account's client says of it.
            var answer = t.GetMethod("ServerAnswer", Harness.F)!;
            string? Answer() => (string?)answer.Invoke(app, [account.Id]);
            Run(new GameTarget(107778070777162, DeadServer));
            NewWindow();
            Write(Line(DateTime.UtcNow.AddSeconds(-3), Launch));
            Poll(s => s.Status == "joining" && DateTime.UtcNow - s.At < TimeSpan.FromSeconds(10));
            c.Check("3 s after the game start, no server yet: not known", Answer() is null, Answer());
            Write(Line(DateTime.UtcNow, Join));
            Poll(s => s.JobId is not null);
            c.Check("a server named in its log: the server took it", Answer() == "in", Answer());
            NewWindow(); // the next account's window
            Write(Line(DateTime.UtcNow.AddSeconds(-13), Launch));
            Poll(s => s.JobId is null);
            c.Check("13 s without a server (a normal join gets one in about a second): it didn't", Answer() == "closed", Answer());
            var waited = Stopwatch.StartNew();
            var told = await (Task<string?>)t.GetMethod("WaitForServerAnswerAsync", Harness.F)!.Invoke(app, [account, CancellationToken.None])!;
            c.Check("…and the launch hears it without waiting", told == "closed" && waited.ElapsedMilliseconds < 2000, $"{told} {waited.ElapsedMilliseconds} ms");

            // A move to another server of the same place, as client 0.743 logged it on 2026-10-19: "Teleported." and the
            // new join first, the old server's disconnect line after them. Bloxkeep took that for a drop and closed the
            // window 3 s later.
            Run(new GameTarget(107778070777162));
            var at = DateTime.UtcNow.AddSeconds(-10);
            const string Moved = "99999999-2222-3333-4444-555555555555";
            Write(Line(at, Join));
            Write(Line(at.AddSeconds(0.1), "[FLog::Network] serverId: 10.0.0.1|5000"));
            Write(Line(at.AddSeconds(2), "[FLog::UgcExperienceController] UgcExperienceController: doTeleport: url "));
            Write(Line(at.AddSeconds(2.2), "[FLog::SessionTransitionFSM] Teleported."));
            Write(Line(at.AddSeconds(2.2), $"[FLog::Output] ! Joining game '{Moved}' place 107778070777162 at 10.0.0.2"));
            Write(Line(at.AddSeconds(2.4), "[FLog::Network] Time to disconnect replication data: 0.125100"));
            state = Poll(s => s.JobId == Moved); // the lines written together are read together
            c.Check("a teleport whose old server logs its disconnect after the new join: still joining, not a drop", state is { Status: "joining", JobId: Moved }, state);
            Tick();
            c.Check("…and the window is left alone", !closing.Contains(me));
            Write(Line(at.AddSeconds(3.5), "[FLog::Network] serverId: 10.0.0.2|5000"));
            state = Poll(s => s.Status == "ingame");
            c.Check("…in the game once the new server answers", state is { Status: "ingame", JobId: Moved }, state);
            // The usual order (the disconnect before the new join) is still a teleport too.
            at = at.AddSeconds(4);
            Write(Line(at, "[FLog::UgcExperienceController] UgcExperienceController: doTeleport: url "));
            Write(Line(at.AddSeconds(0.2), "[FLog::Network] Time to disconnect replication data: 0.1"));
            state = Poll(s => s.At >= at.AddSeconds(0.1));
            c.Check("a teleport that disconnects first: joining", state is { Status: "joining" }, state);
            Write(Line(at.AddSeconds(0.3), Join));
            Write(Line(at.AddSeconds(0.5), "[FLog::Network] serverId: 10.0.0.1|5000"));
            state = Poll(s => s.Status == "ingame");

            // The person leaves the game Bloxkeep opened and starts another one in the same window: a drop there
            // rejoins that game, not the one Bloxkeep opened (on 2026-10-19 it went back to the first game).
            const long Other = 126256942507957;
            var opened = Run(new GameTarget(107778070777162));
            at = at.AddSeconds(1);
            Write(Line(at, Leave));
            Write(Line(at.AddSeconds(0.1), "[FLog::Network] Time to disconnect replication data: 0.2"));
            Write(Line(at.AddSeconds(1), Launch));
            Write(Line(at.AddSeconds(1.5), $"[FLog::Output] ! Joining game '77777777-2222-3333-4444-555555555555' place {Other} at 10.0.0.3"));
            Write(Line(at.AddSeconds(1.6), "[FLog::Network] serverId: 10.0.0.3|5000"));
            Write(Line(at.AddSeconds(2), "[FLog::UgcExperienceController] UgcExperienceController: doTeleport: url "));
            Write(Line(at.AddSeconds(2.1), "[FLog::Output] ! Joining game '66666666-2222-3333-4444-555555555555' place 555 at 10.0.0.4"));
            Write(Line(at.AddSeconds(2.2), "[FLog::Network] Time to disconnect replication data: 0.2"));
            Write(Line(at.AddSeconds(2.3), "[FLog::Network] serverId: 10.0.0.4|5000"));
            Write(Line(at.AddSeconds(3), "[FLog::Network] Time to disconnect replication data: 0.2"));
            state = Poll(s => s.Status == "disconnected");
            c.Check("a game the person starts in the window is noted, and a teleport inside it keeps it",
                state is { Status: "disconnected", PlaceId: 555, ChosenPlace: Other }, state);
            Harness.Invoke(app, "OnSessionEnded", account.Id, opened, state);
            request = queue.Cast<object>().SingleOrDefault();
            target = request?.GetType().GetProperty("Target")!.GetValue(request) as GameTarget;
            c.Check("…and a drop there rejoins that game (any server), not the one Bloxkeep opened", target is { PlaceId: Other, JobId: null }, target);
            queue.Clear();
            // Kicked (seen 2026-10-19: a game's kick, Leave, then a game start in the same window): a game started
            // after that is the person's choice too.
            Write(Line(at.AddSeconds(4), Launch));
            Write(Line(at.AddSeconds(4.5), "[FLog::Output] ! Joining game '55555555-2222-3333-4444-555555555555' place 444 at 10.0.0.5"));
            Write(Line(at.AddSeconds(4.6), "[FLog::Network] serverId: 10.0.0.5|5000"));
            state = Poll(s => s is { Status: "ingame", PlaceId: 444 });
            c.Check("a game started in the window after a kick is the person's choice", state is { Status: "ingame", ChosenPlace: 444 }, state);
            // A preset's "restart every N hours" of that window goes back into it too (and its server), not the first game.
            var restartTo = Harness.Invoke(app, "RestartTarget", running[account.Id]!) as GameTarget;
            c.Check("…and a preset's restart goes back into it, same server",
                restartTo is { PlaceId: 444, JobId: "55555555-2222-3333-4444-555555555555" }, restartTo);

            // A game the person starts in the window that gets no server in a minute is theirs to watch: closing it
            // would reopen the game Bloxkeep opened instead.
            Write(Line(DateTime.UtcNow.AddSeconds(-65), Launch));
            state = Poll(s => s is { Status: "joining", JobId: null });
            Tick();
            c.Check("a game the person started that gets no server in a minute is left alone", state is { PersonStarted: true } && !closing.Contains(me), state);
            // …also after two minutes, when a join of Bloxkeep's would go to Roblox's status check (which closes it).
            Write(Line(DateTime.UtcNow.AddMinutes(-3), Launch));
            Poll(s => s is { Status: "joining", JobId: null } && DateTime.UtcNow - s.At > TimeSpan.FromMinutes(2.5));
            c.Check("…and after two minutes its log still decides, not Roblox's status", Harness.Invoke(app, "LogFollowed", me) is true);

            // A game started in the window while still on a server (no leave logged first): the old server's disconnect
            // line after the new join isn't a drop.
            at = DateTime.UtcNow.AddSeconds(-10);
            Write(Line(at, "[FLog::Output] ! Joining game '44444444-2222-3333-4444-555555555555' place 333 at 10.0.0.6"));
            Write(Line(at.AddSeconds(0.2), "[FLog::Network] Time to disconnect replication data: 0.2"));
            state = Poll(s => s.PlaceId == 333);
            Tick();
            c.Check("a new game while still on a server: the old server's disconnect line isn't a drop", state is { Status: "joining" } && !closing.Contains(me), state);
            Write(Line(at.AddSeconds(1), "[FLog::Network] serverId: 10.0.0.6|5000"));
            state = Poll(s => s.Status == "ingame");
            c.Check("…in the game once its server answers", state is { Status: "ingame", PlaceId: 333 }, state);

            // A teleport that fails, then Leave: the disconnect line that follows is the leave's.
            at = at.AddSeconds(2);
            Write(Line(at, "[FLog::UgcExperienceController] UgcExperienceController: doTeleport: url "));
            Write(Line(at.AddSeconds(5), Leave));
            Write(Line(at.AddSeconds(5.1), "[FLog::Network] Time to disconnect replication data: 0.2"));
            state = Poll(s => s.Status != "ingame");
            c.Check("a teleport that failed, then Leave: left, not joining", state is { Status: "left" }, state);

            // Earlier drops stop counting once a window has stayed in its game ten minutes since it opened, even when the
            // game moved it between servers meanwhile (one teleporting every few minutes never settled before).
            var rejoins = Harness.Get<System.Collections.IDictionary>(app, "_rejoinsInARow");
            var longAgo = Activator.CreateInstance(launchType, account.Id, new HashSet<int>(), DateTime.UtcNow.AddMinutes(-11), new GameTarget(107778070777162), false, null)!;
            running[account.Id] = Activator.CreateInstance(tupleType, me, longAgo);
            Write(Line(DateTime.UtcNow, Launch));
            Write(Line(DateTime.UtcNow, Join));
            Write(Line(DateTime.UtcNow, "[FLog::Network] serverId: 10.0.0.1|5000"));
            state = Poll(s => s.Status == "ingame" && DateTime.UtcNow - s.At < TimeSpan.FromSeconds(30));
            rejoins[account.Id] = 2;
            Harness.Invoke(app, "ForgetSettledRejoins", DateTime.UtcNow);
            c.Check("in its game 11 min after it opened, its last server joined just now: settled", !rejoins.Contains(account.Id), state);
            // Not a window that is still getting in, however long ago it opened.
            Write(Line(DateTime.UtcNow, Launch));
            Poll(s => s is { Status: "joining", JobId: null });
            rejoins[account.Id] = 2;
            Harness.Invoke(app, "ForgetSettledRejoins", DateTime.UtcNow);
            c.Check("…but not one still getting into its game 11 min after it opened", rejoins.Contains(account.Id));
            rejoins.Remove(account.Id);

            // The game's own kick, with its words, then a ban by the game (Roblox's ban API): the lines of 2026-10-21, both
            // with code 267. Grand Piece Online's place.
            const long Gpo = 1730877806;
            void Played(DateTime at, string kickLine)
            {
                Write(Line(at, Launch));
                Write(Line(at, $"[FLog::Output] ! Joining game 'e403336e-5756-44c9-b029-0ae6873a3d03' place {Gpo} at 10.32.36.210"));
                Write(Line(at, "[FLog::Network] serverId: 128.116.46.33|56123"));
                Write(Line(at.AddSeconds(1), kickLine));
                Write(Line(at.AddSeconds(2), "[FLog::Network] Disconnect reason received: 267"));
                Write(Line(at.AddSeconds(2), "[FLog::Network] Disconnection Notification. Reason: 267"));
                Write(Line(at.AddSeconds(2.1), "[FLog::Network] Time to disconnect replication data: 0.2"));
            }
            NewWindow();
            Run(new GameTarget(Gpo));
            closing.Clear();
            Told();
            var kickedAt = DateTime.UtcNow;
            Played(kickedAt, "Error [FLog::CreatorError] Error: Server Kick Message: A fatal error occured. This may be from an internet issue.; Metadata: ");
            state = Poll(s => s.Status == "disconnected");
            c.Check("a kick by the game: code 267, with the game's own words, not a ban",
                state is { Reason: 267, KickMessage: "A fatal error occured. This may be from an internet issue.", BannedByGame: false }, state);
            NewWindow();
            var bannedLaunch = Run(new GameTarget(Gpo));
            var bannedAt = DateTime.UtcNow;
            Played(bannedAt, """Error [FLog::CreatorError] Error: Server Kick Message: ; Metadata: {"moderatorType":"creator","dynamicReasonTextFilterStatus":"2","beginTimestamp":"2026-10-21T00:07:04Z"}""");
            state = Poll(s => s.Status == "disconnected" && s.At > bannedAt);
            c.Check("a ban by the game: the same code 267, told apart by the metadata its kick line carries",
                state is { Reason: 267, BannedByGame: true, KickMessage: null }, state);
            Tick();
            string? banSaid = Told();
            c.Check("…no rejoin (its window stays, with Roblox's message), the ban noted with the account, and said",
                !closing.Contains(me) && account.GameBans.SingleOrDefault() is { PlaceId: Gpo } && banSaid?.Contains("banned from") == true, banSaid);
            var shouldRejoin = t.GetMethod("ShouldRejoin", Harness.F)!;
            object?[] rejoinArgs = [account.Id, bannedLaunch, state, false, null];
            c.Check("…nor a rejoin once its window is closed", !(bool)shouldRejoin.Invoke(app, rejoinArgs)!);
            // Roblox's answer to a join it refuses (Join Error 600 in the window, nothing in the log).
            var refused = new RobloxApi.JoinAnswer(200, 12, "You were banned by this experience or its moderators. Moderation message: Exploiting");
            c.Check("Roblox's join answer for a game's ban: read as a ban, with the game's words", refused is { Banned: true, Reason: "Exploiting" }, refused.Reason);
            c.Check("…and one for a full server isn't", !new RobloxApi.JoinAnswer(200, 6, "The game you requested is currently full. Please try again later.").Banned);
            // What Roblox really answered on 2026-10-21 for an account a game had banned (its window: Join Error 600).
            var real = new RobloxApi.JoinAnswer(200, 12, "Unable to join Game 211");
            c.Check("Roblox's real answer for a game's ban (status 12, \"Unable to join Game 211\"): a ban, without the game's words",
                real is { Banned: true, Code: 211, Reason: null }, $"{real.Banned} {real.Code} {real.Reason}");
            c.Check("…but not another refusal with status 12",
                !new RobloxApi.JoinAnswer(200, 12, "Unable to join Game 524").Banned);
            c.Check("…and the game's words when its statusData gives them",
                new RobloxApi.JoinAnswer(200, 12, "Unable to join Game 211", """{"moderationMessage":"Exploiting"}""").Reason == "Exploiting");
            // In that game again later: the ban is over.
            NewWindow();
            Run(new GameTarget(Gpo));
            Write(Line(DateTime.UtcNow, Launch));
            Write(Line(DateTime.UtcNow, $"[FLog::Output] ! Joining game 'e403336e-5756-44c9-b029-0ae6873a3d03' place {Gpo} at 10.32.36.210"));
            Write(Line(DateTime.UtcNow, "[FLog::Network] serverId: 128.116.46.33|56123"));
            Poll(s => s.Status == "ingame");
            Harness.Invoke(app, "ForgetLiftedGameBans");
            c.Check("…and once the account is in that game again, the ban is forgotten", account.GameBans.Count == 0, account.GameBans.Count);
        }
        finally
        {
            running.Clear();
            queue.Clear();
            closing.Clear();
        }
    }
}
