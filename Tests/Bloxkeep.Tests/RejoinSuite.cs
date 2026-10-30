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
            // A new process too: nothing Bloxkeep held about the old one's id.
            Harness.Get<HashSet<int>>(app, "_gameBanNoted").Remove(me);
            Harness.Get<System.Collections.IDictionary>(app, "_joinChecks").Remove(me);
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

            // In the game, back to Roblox's home and the window closed with X before Roblox logged the disconnect (the
            // owner's PC, 2026-10-29): a leave all the same, not a drop.
            NewWindow();
            Write(Line(DateTime.UtcNow, Launch));
            Write(Line(DateTime.UtcNow, Join));
            Write(Line(DateTime.UtcNow, "[FLog::Network] serverId: 128.116.53.33|51451"));
            Poll(s => s.Status == "ingame");
            Write(Line(DateTime.UtcNow, Leave));
            var closedWithX = Harness.Invoke(app, "EndedState", me) as RobloxLogWatcher.ClientState;
            c.Check("in a game, a leave and the window closed before its disconnect line: left (fails on the old code)", closedWithX is { Status: "left" }, closedWithX);
            c.Check("…and isn't rejoined", Harness.Invoke(app, "QueueRejoin", account.Id, launch, closedWithX) is false && queue.Count == 0);

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
            c.Check("13 s without a server (a normal join gets one in about a second): silent", Answer() == "silent", Answer());
            // Roblox's join answer for that server decides, not the silence: a join waiting for room logs nothing (four
            // accounts sent to one server waited 17-38 s, then got in, 2026-10-16). Roblox answers from here.
            var partyAsk = Harness.Get<object>(app, "_askJoin");
            RobloxApi.JoinAnswer partyReply = new(200, 5, "The game has ended.");
            int partyAsks = 0;
            Harness.Set(app, "_askJoin", (Func<Account, GameTarget, CancellationToken, Task<RobloxApi.JoinAnswer>>)((_, _, _) =>
            {
                partyAsks++;
                return Task.FromResult(partyReply);
            }));
            var waitForServer = t.GetMethod("WaitForServerAnswerAsync", Harness.F)!;
            Task<string?> Wait() => (Task<string?>)waitForServer.Invoke(app, [account, new GameTarget(107778070777162, DeadServer), CancellationToken.None])!;
            try
            {
                var waited = Stopwatch.StartNew();
                var told = await Wait();
                c.Check("…Roblox says the server has ended (status 5): closed, heard without waiting",
                    told == "closed" && partyAsks == 1 && waited.ElapsedMilliseconds < 2000, $"{told} {partyAsks} {waited.ElapsedMilliseconds} ms");
                Harness.Get<System.Collections.IDictionary>(app, "_joinChecks").Remove(me);
                partyReply = new(200, 6, "The game you requested is currently full.");
                c.Check("…full (status 6): closed", await Wait() == "closed");
                // The server ended with a system error (2026-10-29): Roblox refuses the join to that server with status 12,
                // "Unable to join Game 311", not a ban. That server is gone: closed, so the party goes to another server
                // (before, the accounts went there all the same and weren't tried again).
                Harness.Get<System.Collections.IDictionary>(app, "_joinChecks").Remove(me);
                partyReply = new(200, 12, "Unable to join Game 311");
                c.Check("…refused for that server without a ban (status 12, 311): closed, the party goes elsewhere", await Wait() == "closed");
                var asked311 = await (Task<RobloxApi.JoinAnswer?>)t.GetMethod("CheckJoinAsync", Harness.F)!.Invoke(app,
                    [account, new GameTarget(107778070777162, DeadServer), "test", CancellationToken.None])!;
                var game311 = await (Task<RobloxApi.JoinAnswer?>)t.GetMethod("CheckJoinAsync", Harness.F)!.Invoke(app,
                    [account, new GameTarget(107778070777162), "test", CancellationToken.None])!;
                c.Check("…read as that server gone (ended), not a refusal; the same answer for the game itself (no server named) is a refusal",
                    asked311 is { ServerGone: true, Ended: true, Refused: false } && game311 is { ServerGone: false, Refused: true });
                partyReply = new(200, 12, "Unable to join Game 211");
                var ban211 = await (Task<RobloxApi.JoinAnswer?>)t.GetMethod("CheckJoinAsync", Harness.F)!.Invoke(app,
                    [account, new GameTarget(107778070777162, DeadServer), "test", CancellationToken.None])!;
                c.Check("…a ban by the game (211) on a named server stays a ban and a refusal", ban211 is { Banned: true, ServerGone: false, Refused: true });
                // Roblox would let it in (status 2): the join is slow, not refused. It got its server 2 s on.
                Harness.Get<System.Collections.IDictionary>(app, "_joinChecks").Remove(me);
                partyReply = new(200, 2, null);
                var slow = Wait();
                await Task.Delay(1500);
                Write(Line(DateTime.UtcNow, Join));
                Poll(s => s.JobId is not null); // as the process check reads the log
                c.Check("…Roblox would let it in (status 2), its server named 2 s on: not closed, it got in", await slow == "in", slow.Result);
                c.Check("…and the window's own join check has that answer, not asked again", Harness.Get<System.Collections.IDictionary>(app, "_joinChecks").Contains(me));
            }
            finally
            {
                Harness.Set(app, "_askJoin", partyAsk);
                Harness.Get<System.Collections.IDictionary>(app, "_joinChecks").Remove(me);
            }

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
            // Discord's alert: the game's words (if any) as its text, without a "Reason" saying "Kicked by the game" again.
            var banAlert = Harness.Invoke(app, "AlertFor", "gameBan", account.Id,
                ("{0} was banned from {1} by the game: {2}", new object?[] { account.Label, "Grand Piece Online", "Exploiting" }), banSaid ?? "", DateTime.UtcNow)!;
            var banFields = ((System.Collections.IEnumerable)banAlert.GetType().GetProperty("Fields")!.GetValue(banAlert)!).Cast<object>()
                .Select(f => f.GetType().GetProperty("Name")!.GetValue(f) as string).ToList();
            string? banText = banAlert.GetType().GetProperty("Text")!.GetValue(banAlert) as string;
            c.Check("…Discord's alert says the ban, without a \"Reason\" (kicked) beside it",
                banAlert.GetType().GetProperty("Title")!.GetValue(banAlert) as string == "Banned by the game" && !banFields.Contains("Reason") && banText?.Contains("Reason:") != true,
                $"{string.Join(",", banFields)} | {banText}");
            // Roblox 0.744 often logs why in words only, without the code: the owner's lines of 2026-10-21, a game's kick
            // that went out as "Lost connection", then on the rejoin a sign-in on another device that was rejoined again.
            var notBanned = new GameTarget(107778070777162);
            object? droppedLaunch = null;
            DateTime Dropped(params string[] why) => DroppedAt(DateTime.UtcNow.AddSeconds(-10), why);
            DateTime DroppedAt(DateTime at, params string[] why)
            {
                NewWindow();
                droppedLaunch = Run(notBanned);
                Write(Line(at, Launch));
                Write(Line(at, Join));
                Write(Line(at, "[FLog::Network] serverId: 128.116.46.33|52353"));
                foreach (string line in why)
                    Write(Line(at.AddSeconds(1), line));
                Write(Line(at.AddSeconds(2), "[FLog::Network] Time to disconnect replication data: 7.910900"));
                return at;
            }
            var wordsAt = Dropped("Error [FLog::CreatorError] Error: Server Kick Message: DataStore lock lost, please rejoin the game.; Metadata: ",
                "[FLog::Network] Disconnect from 1",
                "[FLog::Network] Client has been disconnected with reason: You have been kicked from the game",
                "Warning [FLog::Network] Lost connection with reason : You have been kicked from the game");
            state = Poll(s => s.Status == "disconnected" && s.At > wordsAt);
            c.Check("a game's kick logged in words only, no code: 267, with the game's words",
                state is { Reason: 267, KickMessage: "DataStore lock lost, please rejoin the game.", BannedByGame: false }, state);
            closing.Clear();
            Told();
            Tick();
            string? kickSaid = Told();
            c.Check("…rejoined, said as the game's kick with its words, not as a lost connection",
                closing.Contains(me) && kickSaid == $"{account.Label} was kicked by the game, rejoining: DataStore lock lost, please rejoin the game.", kickSaid);
            var kickAlert = Harness.Invoke(app, "AlertFor", "drop", account.Id,
                ("{0} was kicked by the game, rejoining: {1}", new object?[] { account.Label, "DataStore lock lost, please rejoin the game." }), kickSaid ?? "", DateTime.UtcNow)!;
            var kickFields = ((System.Collections.IEnumerable)kickAlert.GetType().GetProperty("Fields")!.GetValue(kickAlert)!).Cast<object>()
                .Select(f => f.GetType().GetProperty("Name")!.GetValue(f) as string).ToList();
            string? kickTitle = kickAlert.GetType().GetProperty("Title")!.GetValue(kickAlert) as string, kickText = kickAlert.GetType().GetProperty("Text")!.GetValue(kickAlert) as string;
            c.Check("…Discord's alert: \"Kicked by the game, rejoining\", the game's words as its text, no \"Reason\" field",
                kickTitle == "Kicked by the game, rejoining" && kickText?.StartsWith("DataStore lock lost") == true && !kickFields.Contains("Reason"),
                $"{kickTitle} | {kickText} | {string.Join(",", kickFields)}");
            var elsewhereAt = Dropped("[FLog::Network] Client has been disconnected with reason: Disconnected from game, possibly due to game joined from another device",
                "Warning [FLog::Network] Lost connection with reason : Disconnected from game, possibly due to game joined from another device");
            state = Poll(s => s.Status == "disconnected" && s.At > elsewhereAt);
            closing.Clear();
            Told();
            Tick();
            c.Check("signed in on another device, in words only: 273, its window left as it is and no rejoin (it would take the other device's place)",
                state is { Reason: 273 } && !closing.Contains(me) && Told() is null && !(bool)shouldRejoin.Invoke(app, [account.Id, droppedLaunch, state, false, null])!, state);
            // Words logged after the disconnect line complete it; a server that shut down is said as that.
            var lateAt = Dropped();
            state = Poll(s => s.Status == "disconnected" && s.At > lateAt);
            bool noReasonFirst = state is { Reason: null };
            Write(Line(lateAt.AddSeconds(3), "Warning [FLog::Network] Lost connection with reason : The server has shut down"));
            state = Poll(s => s is { Status: "disconnected", Reason: not null });
            closing.Clear();
            Told();
            Tick();
            string? shutSaid = Told();
            c.Check("words logged after the disconnect line complete it: a server that shut down, said as that",
                noReasonFirst && state is { Reason: 288 } && closing.Contains(me) && shutSaid == $"{account.Label}'s server shut down — joining another one…", $"{state} | {shutSaid}");
            // Words Roblox has no code for change nothing.
            c.Check("…and words not in Roblox's list read as no code", typeof(RobloxLogWatcher).GetMethod("ReasonOfWords", Harness.F)!.Invoke(null,
                ["[FLog::Network] Client has been disconnected with reason: Something else entirely"]) is null);
            // A code logged as a number wins over one read from words, even logged after the disconnect line (a log of
            // 2026-10-16 has "Disconnection Notification. Reason: 285" after it).
            var numberAt = Dropped("[FLog::Network] Client has been disconnected with reason: You have been kicked from the game");
            Poll(s => s.Status == "disconnected" && s.At > numberAt);
            Write(Line(numberAt.AddSeconds(3), "[FLog::Network] Disconnection Notification. Reason: 277"));
            state = Poll(s => s is { Status: "disconnected", Reason: 277 });
            c.Check("a code logged as a number after the disconnect line wins over the one read from words", state is { Reason: 277 }, state);
            closing.Clear();
            Told();
            Tick();
            string? lostSaid = Told();
            c.Check("…and a failed connection's code is said as a lost connection", lostSaid == $"{account.Label} lost connection — restarting Roblox…", lostSaid);
            // Roblox's words printed by the game itself (its output, even quoting Roblox's tag) aren't Roblox's reason.
            var printAt = Dropped("[FLog::Output] Client has been disconnected with reason: You have been kicked from the game",
                "[FLog::Output] [FLog::Network] Lost connection with reason : Disconnected from game, possibly due to game joined from another device");
            state = Poll(s => s.Status == "disconnected" && s.At > printAt);
            c.Check("Roblox's words printed by the game aren't Roblox's reason", state is { Reason: null }, state);
            closing.Clear();
            Told();
            Tick();
            string? unknownSaid = Told();
            c.Check("…a disconnect without a reason is said as one whose reason isn't known, not as a lost connection",
                closing.Contains(me) && unknownSaid == $"{account.Label} dropped out of the game, reason unknown — restarting Roblox…", unknownSaid);
            var unknownAlert = Harness.Invoke(app, "AlertFor", "drop", account.Id,
                ("{0} dropped out of the game, reason unknown — restarting Roblox…", new object?[] { account.Label }), unknownSaid ?? "", DateTime.UtcNow)!;
            string? unknownTitle = unknownAlert.GetType().GetProperty("Title")!.GetValue(unknownAlert) as string;
            c.Check("…Discord's alert too", unknownTitle == "Disconnected, reason unknown", unknownTitle);
            var codeAt = Dropped("[FLog::Network] Disconnection Notification. Reason: 299");
            Poll(s => s.Status == "disconnected" && s.At > codeAt);
            closing.Clear();
            Told();
            Tick();
            string? codeSaid = Told();
            c.Check("…one with a code Bloxkeep has no words for is said with that code", codeSaid == $"{account.Label} was disconnected (Roblox code 299) — restarting Roblox…", codeSaid);
            // The client's own disconnect (285), which its notification line can repeat, never over a reason known; the
            // server's code as received is.
            var ownAt = Dropped("[FLog::Network] Client has been disconnected with reason: You have been kicked from the game");
            Poll(s => s.Status == "disconnected" && s.At > ownAt);
            Write(Line(ownAt.AddSeconds(3), "[FLog::Network] Disconnection Notification. Reason: 285"));
            state = Poll(_ => true); // one read takes in the line just written
            c.Check("the client's own disconnect (285) after a kick's words: still the kick", state is { Reason: 267 }, state);
            var receivedAt = Dropped("[FLog::Network] Client has been disconnected with reason: You have been kicked from the game",
                "[FLog::Network] Disconnect reason received: 288");
            state = Poll(s => s.Status == "disconnected" && s.At > receivedAt);
            c.Check("…while the server's code as received wins over the words", state is { Reason: 288 }, state);
            // Roblox 0.744's idle kick, in words with its minutes (the client's DisconnectIdle prompt reads the same): 278, so
            // anti-AFK's misses are told again.
            var idleAt = Dropped("[FLog::Network] Client has been disconnected with reason: You were disconnected for being idle 20 minutes");
            state = Poll(s => s.Status == "disconnected" && s.At > idleAt);
            c.Check("an idle kick in words, with its minutes: 278", state is { Reason: 278 }, state);
            // DisconnectRejoin (276): the client says the same as for 273, the same account launched from another device.
            var rejoinAt = Dropped("[FLog::Network] Disconnection Notification. Reason: 276");
            state = Poll(s => s.Status == "disconnected" && s.At > rejoinAt);
            closing.Clear();
            Told();
            Tick();
            c.Check("276 (the same account launched elsewhere, as 273): left as it is, no rejoin",
                state is { Reason: 276 } && !closing.Contains(me) && Told() is null && !(bool)shouldRejoin.Invoke(app, [account.Id, droppedLaunch, state, false, null])!, state);
            // A code Roblox's own error box has no Reconnect for (its Connection.lua): no rejoin, said once, why. Here no
            // access to the private server any more, in Roblox 0.744's words.
            var privateAt = DroppedAt(DateTime.UtcNow.AddSeconds(-2), "[FLog::Network] Client has been disconnected with reason: You have been disconnected because you no longer have access to this private server");
            state = Poll(s => s.Status == "disconnected" && s.At > privateAt);
            closing.Clear();
            Told();
            Tick();
            string? refusedSaid = Told();
            Tick();
            c.Check("a code Roblox gives no Reconnect for (286, no access to the private server): no rejoin, and why, once",
                state is { Reason: 286 } && !closing.Contains(me) && !(bool)shouldRejoin.Invoke(app, [account.Id, droppedLaunch, state, false, null])!
                && refusedSaid == $"{account.Label} isn't rejoined (no access to the private server any more): Roblox doesn't allow reconnecting after this."
                && Told() is null, $"{state} | {refusedSaid}");
            var refusedAlert = Harness.Invoke(app, "AlertFor", "drop", account.Id,
                ("{0} isn't rejoined ({1}): Roblox doesn't allow reconnecting after this.", new object?[] { account.Label, "no access to the private server any more" }), refusedSaid ?? "", DateTime.UtcNow)!;
            string? refusedTitle = refusedAlert.GetType().GetProperty("Title")!.GetValue(refusedAlert) as string;
            c.Check("…Discord's alert: \"Disconnected, not rejoining\"", refusedTitle == "Disconnected, not rejoining", refusedTitle);
            // On the way in: waiting for a server, teleporting, or left before any server took it.
            string? Phase() => Harness.Invoke(app, "Phase", me) as string;
            NewWindow();
            Run(notBanned);
            var wayAt = DateTime.UtcNow.AddSeconds(-5);
            Write(Line(wayAt, Launch));
            Poll(s => s is { Status: "joining", JobId: null } && s.At >= wayAt.AddSeconds(-1));
            c.Check("started, no server yet: waiting for a server", Phase() == "waiting", Phase());
            Write(Line(wayAt.AddSeconds(1), Join));
            Write(Line(wayAt.AddSeconds(1), "[FLog::Network] serverId: 128.116.46.33|52353"));
            Poll(s => s.Status == "ingame" && s.At > wayAt);
            c.Check("…in its server: none", Phase() is null, Phase());
            Write(Line(wayAt.AddSeconds(2), "[FLog::SingleSurfaceApp] launchUGCGameInternal: deferred download complete"));
            Write(Line(wayAt.AddSeconds(2), "[FLog::UgcExperienceController] UgcExperienceController: doTeleport: url https://gamejoin.roblox.com/v1/join-reserved-game"));
            state = Poll(s => s.TeleportStarted is not null);
            c.Check("Roblox's other launchUGCGameInternal lines aren't a game start, and a teleport is shown as one",
                state is { Status: "ingame", PersonStarted: false, TeleportStarted: not null } && Phase() == "teleporting", state);
            Write(Line(wayAt.AddSeconds(3), "[FLog::Output] ! Joining game '22222222-2222-3333-4444-555555555555' place 107778070777162 at 10.0.0.2"));
            Write(Line(wayAt.AddSeconds(3), "[FLog::Network] serverId: 128.116.46.34|52353"));
            state = Poll(s => s is { Status: "ingame", TeleportStarted: null });
            c.Check("…until the next server is reached", state is { JobId: "22222222-2222-3333-4444-555555555555" } && Phase() is null, state);
            NewWindow();
            Run(notBanned);
            var failAt = DateTime.UtcNow.AddSeconds(-70);
            Write(Line(failAt, Launch));
            Write(Line(failAt.AddSeconds(62), Leave));
            state = Poll(s => s.Status == "left" && s.At > failAt);
            c.Check("left before any server took it (a join error's Leave, or a wait given up): told apart from a game left",
                state is { NeverJoined: true } && Phase() == "joinFailed", state);
            closing.Clear();
            // When a join not in yet is asked about: at once for a game known to have banned the account, else only after
            // the longest silent wait a good join was seen to take (38 s on 2026-10-16).
            var checkAfter = t.GetMethod("JoinCheckAfter", Harness.F)!;
            TimeSpan After(long place) => (TimeSpan)checkAfter.Invoke(app, [account, new GameTarget(place)])!;
            c.Check("a join not in yet is asked about after 5 s for a game that banned the account, after 40 s otherwise",
                After(Gpo) == TimeSpan.FromSeconds(5) && After(920587237) == TimeSpan.FromSeconds(40), $"{After(Gpo)} {After(920587237)}");
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
            // Opened into that game again: the window stops on Roblox's Join Error 600 and its log says nothing more than
            // the game start. The card says the ban once Roblox's join answer did (asked after 5 s), rather than "Joining";
            // not before, as the ban may be over.
            NewWindow();
            Run(new GameTarget(Gpo));
            Write(Line(DateTime.UtcNow, Launch));
            Poll(s => s is { Status: "joining", JobId: null });
            bool CardBanned() => System.Text.Json.JsonSerializer.SerializeToElement(Harness.Invoke(app, "AccountDto", account))
                .GetProperty("session").GetProperty("bannedByGame").GetBoolean();
            c.Check("a window opened into a game that banned the account: not said banned before Roblox answers (the ban may be over)", !CardBanned());
            Harness.Get<HashSet<int>>(app, "_gameBanNoted").Add(me);
            c.Check("…its card says the ban, not Joining, once Roblox's join answer said it", CardBanned());
            Harness.Get<HashSet<int>>(app, "_gameBanNoted").Clear();
            // Minutes on that screen: its log no longer decides (JoiningTimeout) and Roblox's status says not in a game,
            // twice. That isn't a lost connection to restart Roblox for.
            NewWindow();
            var stuck = Activator.CreateInstance(launchType, account.Id, new HashSet<int>(), DateTime.UtcNow.AddMinutes(-4), new GameTarget(Gpo), false, null)!;
            running[account.Id] = Activator.CreateInstance(tupleType, me, stuck);
            Write(Line(DateTime.UtcNow.AddMinutes(-4), Launch));
            Poll(s => s is { Status: "joining", JobId: null } && DateTime.UtcNow - s.At > TimeSpan.FromMinutes(3));
            account.UserId = 21;
            Harness.Get<System.Collections.IDictionary>(app, "_presence")[21L] = new Presence(1, "Website", null, null, null, null, null);
            closing.Clear();
            Told();
            for (int i = 0; i < 3; i++)
                Harness.Invoke(app, "DetectDisconnectedSessions", new HashSet<long> { 21 });
            c.Check("…nor is it closed as a lost connection when Roblox's status says not in a game", !closing.Contains(me) && Told() is null, closing.Count);
            // Its ban told after 5 seconds (Roblox's join answer), the window still on Join Error 600 a minute later: the
            // minute's "never got in" doesn't close it with "trying again".
            NewWindow();
            var refusedLaunch = Activator.CreateInstance(launchType, account.Id, new HashSet<int>(), DateTime.UtcNow.AddMinutes(-2), new GameTarget(Gpo), false, null)!;
            running[account.Id] = Activator.CreateInstance(tupleType, me, refusedLaunch);
            Write(Line(DateTime.UtcNow.AddSeconds(-70), Launch));
            Poll(s => s is { Status: "joining", JobId: null } && DateTime.UtcNow - s.At > TimeSpan.FromSeconds(65));
            Harness.Get<HashSet<int>>(app, "_gameBanNoted").Add(me);
            closing.Clear();
            Told();
            Tick();
            c.Check("…nor, its ban told by Roblox's join answer, closed a minute on with \"trying again\"", !closing.Contains(me) && Told() is null, closing.Count);
            Harness.Get<HashSet<int>>(app, "_gameBanNoted").Clear();
            // In that game again later: the ban is over.
            NewWindow();
            Run(new GameTarget(Gpo));
            Write(Line(DateTime.UtcNow, Launch));
            Write(Line(DateTime.UtcNow, $"[FLog::Output] ! Joining game 'e403336e-5756-44c9-b029-0ae6873a3d03' place {Gpo} at 10.32.36.210"));
            Write(Line(DateTime.UtcNow, "[FLog::Network] serverId: 128.116.46.33|56123"));
            Poll(s => s.Status == "ingame");
            Harness.Invoke(app, "ForgetLiftedGameBans");
            c.Check("…and once the account is in that game again, the ban is forgotten", account.GameBans.Count == 0, account.GameBans.Count);

            // What a review of these found (2026-10-21), one case each.
            var noted = Harness.Get<HashSet<int>>(app, "_gameBanNoted");
            var checks = Harness.Get<System.Collections.IDictionary>(app, "_joinChecks");
            const string BanLine = """Error [FLog::CreatorError] Error: Server Kick Message: ; Metadata: {"moderatorType":"creator","dynamicReasonTextFilterStatus":"2","beginTimestamp":"2026-10-21T00:07:04Z"}""";
            // A ban seen again after Bloxkeep restarted (an update), its window still open: told once.
            NewWindow();
            Run(new GameTarget(Gpo));
            Told();
            var seenAt = DateTime.UtcNow;
            Played(seenAt, BanLine);
            Poll(s => s is { Status: "disconnected", BannedByGame: true } && s.At > seenAt);
            Tick();
            string? firstTold = Told();
            noted.Clear(); // as after a restart: nothing of it in memory
            Tick();
            string? againTold = Told();
            c.Check("a ban seen again after a restart, its window still open: not told a second time",
                firstTold is not null && againTold is null && account.GameBans.Count == 1, againTold);
            // Banned in a game the person picked in the window: that game is the one noted (a rejoin would open it), not the
            // one Bloxkeep opened the window into.
            account.GameBans.Clear();
            noted.Clear();
            NewWindow();
            Run(new GameTarget(920587237));
            var pickedAt = DateTime.UtcNow;
            Write(Line(pickedAt, Launch)); // Bloxkeep's start; Played's is the person's
            Played(pickedAt.AddSeconds(0.5), BanLine);
            Poll(s => s is { Status: "disconnected", BannedByGame: true, ChosenPlace: Gpo } && s.At > pickedAt);
            Tick();
            c.Check("banned in a game the person picked in the window: that game is noted", account.GameBans.SingleOrDefault()?.PlaceId == Gpo,
                string.Join(",", account.GameBans.Select(b => b.PlaceId)));
            // A preset (also run from the tray or its schedule, where nobody answers a question) leaves out an account the
            // game banned, rather than asking and so opening nothing at all; opened by hand, it is asked first. Nothing
            // opens either way: the preset has nobody else to open, and the question comes before any opening.
            var launchAccounts = t.GetMethod("LaunchAccountsAsync", Harness.F)!;
            var openBefore = running[account.Id];
            running.Clear();
            // Its own ban, whatever the check above found: without one, these calls would go on to open Roblox.
            var bansBefore = account.GameBans.ToList();
            account.GameBans = [new GameBan { PlaceId = Gpo, At = DateTime.UtcNow }];
            async Task<object?> Open(Guid? preset)
            {
                var task = (Task)launchAccounts.Invoke(app, [vault, new List<Account> { account }, new GameTarget(Gpo), false, 0, null, preset])!;
                await task;
                return task.GetType().GetProperty("Result")!.GetValue(task);
            }
            string? presetError = null;
            object? presetResult = null;
            try { presetResult = await Open(Guid.NewGuid()); } catch (UserError ex) { presetError = ex.Message; }
            c.Check("a preset leaves out an account the game banned, without a question", presetError?.Contains("the preset didn't open it") == true,
                presetError ?? presetResult?.ToString());
            var byHand = await Open(null);
            var asks = byHand?.GetType().GetProperty("Asks")!.GetValue(byHand) as List<string>;
            c.Check("…opened by hand, it is asked first", asks?.Contains("gameBan") == true, asks is null ? byHand?.ToString() : string.Join(",", asks));
            account.GameBans = bansBefore;
            running[account.Id] = openBefore;
            // A window opened into the game that banned the account, the person in another game from it now: the ban stands,
            // and the window isn't held as banned any more.
            NewWindow();
            Run(new GameTarget(Gpo));
            var otherAt = DateTime.UtcNow;
            Write(Line(otherAt, Launch));
            Write(Line(otherAt, Launch));
            Write(Line(otherAt, "[FLog::Output] ! Joining game '77777777-2222-3333-4444-555555555555' place 444 at 10.0.0.7"));
            Write(Line(otherAt, "[FLog::Network] serverId: 10.0.0.7|5000"));
            Poll(s => s is { Status: "ingame", PlaceId: 444 });
            noted.Add(me);
            Tick();
            Harness.Invoke(app, "ForgetLiftedGameBans");
            c.Check("in another game, from a window opened into the one that banned it: that ban isn't forgotten", account.GameBans.Any(b => b.PlaceId == Gpo));
            c.Check("…and the window, in a server now, isn't held as banned", !noted.Contains(me));
            // Nothing open any more: what was held about the window goes (a later client may get its process id).
            var held = running[account.Id];
            running.Clear();
            noted.Add(me);
            checks[me] = (Task.FromResult<RobloxApi.JoinAnswer?>(null), DateTime.UtcNow);
            Tick();
            c.Check("…and once nothing is open, nothing is held about its process id", noted.Count == 0 && checks.Count == 0, $"{noted.Count} {checks.Count}");
            running[account.Id] = held;
            // Roblox's join question failed (an exception of any kind): the check of every window goes on. Roblox's join API
            // answers from here (questions counts them): nothing goes to Roblox.
            account.Cookie = "test";
            var realAsk = Harness.Get<object>(app, "_askJoin");
            int questions = 0;
            RobloxApi.JoinAnswer reply = new(200, 12, "Unable to join Game 211");
            Harness.Set(app, "_askJoin", (Func<Account, GameTarget, CancellationToken, Task<RobloxApi.JoinAnswer>>)((_, _, _) =>
            {
                questions++;
                return Task.FromResult(reply);
            }));
            try
            {
                NewWindow();
                Run(new GameTarget(920587237));
                Write(Line(DateTime.UtcNow.AddSeconds(-45), Launch));
                Poll(s => s is { Status: "joining", JobId: null } && DateTime.UtcNow - s.At > TimeSpan.FromSeconds(42));
                checks[me] = (Task.FromException<RobloxApi.JoinAnswer?>(new InvalidOperationException("test")), DateTime.UtcNow);
                Exception? thrown = null;
                try { Tick(); } catch (Exception ex) { thrown = ex.InnerException ?? ex; }
                c.Check("a join question that failed doesn't stop the check of the windows", thrown is null, thrown?.GetType().Name);
                // A window opened into a game that banned the account, a minute without a server, Roblox not saying the ban
                // (a full server): left as it is, as no rejoin would follow its close.
                NewWindow();
                Run(new GameTarget(920587237));
                account.GameBans.Add(new GameBan { PlaceId = 920587237, At = DateTime.UtcNow.AddHours(-1) });
                Write(Line(DateTime.UtcNow.AddSeconds(-70), Launch));
                Poll(s => s is { Status: "joining", JobId: null } && DateTime.UtcNow - s.At > TimeSpan.FromSeconds(65));
                checks[me] = (Task.FromResult<RobloxApi.JoinAnswer?>(new RobloxApi.JoinAnswer(200, 6, "The game you requested is currently full.")), DateTime.UtcNow);
                closing.Clear();
                Tick();
                c.Check("a window into a game that banned the account, not in a minute on: not closed when no rejoin follows",
                    !closing.Contains(me) && account.GameBans.Any(b => b.PlaceId == 920587237));
                // Roblox would let it in now (status 2): the ban is over, and it is tried again as any other.
                checks[me] = (Task.FromResult<RobloxApi.JoinAnswer?>(new RobloxApi.JoinAnswer(200, 2, null)), DateTime.UtcNow);
                Tick();
                c.Check("…but once Roblox would let it in, the ban is forgotten and it is tried again",
                    closing.Contains(me) && account.GameBans.All(b => b.PlaceId != 920587237), string.Join(",", account.GameBans.Select(b => b.PlaceId)));
                closing.Clear();
                // A join question that got no answer (no network, often right after the PC wakes): asked again half a minute
                // on, not never; a ban in that answer is then told. Before, a window known banned stayed on "Joining".
                NewWindow();
                Run(new GameTarget(Gpo));
                account.GameBans = [new GameBan { PlaceId = Gpo, At = DateTime.UtcNow.AddHours(-1) }];
                Write(Line(DateTime.UtcNow.AddSeconds(-20), Launch));
                Poll(s => s is { Status: "joining", JobId: null } && DateTime.UtcNow - s.At > TimeSpan.FromSeconds(15));
                reply = new(200, 12, "Unable to join Game 211");
                questions = 0;
                checks[me] = (Task.FromResult<RobloxApi.JoinAnswer?>(null), DateTime.UtcNow.AddSeconds(-10));
                Tick();
                c.Check("a join question without an answer isn't asked again at once", questions == 0 && !noted.Contains(me), questions);
                checks[me] = (Task.FromResult<RobloxApi.JoinAnswer?>(null), DateTime.UtcNow.AddSeconds(-40));
                Tick();
                Tick();
                c.Check("…but half a minute on, and the ban in that answer is then noted", questions == 1 && noted.Contains(me), $"{questions} {noted.Contains(me)}");
                // A refused request (too many requests: an answer without a status) is no answer either.
                NewWindow();
                Run(new GameTarget(Gpo));
                Write(Line(DateTime.UtcNow.AddSeconds(-20), Launch));
                Poll(s => s is { Status: "joining", JobId: null } && DateTime.UtcNow - s.At > TimeSpan.FromSeconds(15));
                reply = new(429, null, "Too many requests");
                questions = 0;
                Tick();
                var refusedAsk = ((Task<RobloxApi.JoinAnswer?> Answer, DateTime At))checks[me]!;
                checks[me] = (refusedAsk.Answer, DateTime.UtcNow.AddSeconds(-40));
                Tick();
                c.Check("a join question Roblox refused (too many requests) is asked again too", questions == 2, questions);
                // Roblox refuses the account the game (a status its own client counts as a join error, not a wait): the window
                // stays, why is said once, and it isn't rejoined once closed. Before, it was closed and opened again every
                // minute, for good.
                c.Check("Roblox's join statuses its client counts as errors (12, 29-33) read as refusals; a full game, a wait or a join don't",
                    new[] { 12, 29, 30, 31, 32, 33 }.All(s => new RobloxApi.JoinAnswer(200, s, null).Refused)
                    && new[] { 0, 1, 2, 6, 10, 28 }.All(s => !new RobloxApi.JoinAnswer(200, s, null).Refused));
                NewWindow();
                var refusedGame = Run(new GameTarget(920587237));
                Write(Line(DateTime.UtcNow.AddSeconds(-70), Launch));
                Poll(s => s is { Status: "joining", JobId: null } && DateTime.UtcNow - s.At > TimeSpan.FromSeconds(65));
                checks[me] = (Task.FromResult<RobloxApi.JoinAnswer?>(new RobloxApi.JoinAnswer(200, 29, "This experience needs a trusted friend in the server.")), DateTime.UtcNow);
                closing.Clear();
                Told();
                Tick();
                string? refusalSaid = Told();
                c.Check("a join Roblox refuses (status 29), a minute without a server: not closed to try again, and why is said",
                    !closing.Contains(me) && refusalSaid?.Contains("can't get into the game, so Bloxkeep doesn't try again: This experience needs a trusted friend", StringComparison.Ordinal) == true,
                    refusalSaid);
                Tick();
                c.Check("…said once", Told() is null && !closing.Contains(me));
                var refusedEnd = Harness.Invoke(app, "EndedState", me) as RobloxLogWatcher.ClientState;
                c.Check("…and closed by the person, not rejoined", Harness.Invoke(app, "QueueRejoin", account.Id, refusedGame, refusedEnd) is false && queue.Count == 0, queue.Count);
                queue.Clear();
                var refusalAlert = Harness.Invoke(app, "AlertFor", "drop", account.Id,
                    ("{0} can't get into the game, so Bloxkeep doesn't try again: {1}", new object?[] { account.Label, "No." }), refusalSaid ?? "", DateTime.UtcNow)!;
                string? refusalTitle = refusalAlert.GetType().GetProperty("Title")!.GetValue(refusalAlert) as string;
                c.Check("…Discord's alert: \"Roblox won't let it in, not trying again\"", refusalTitle == "Roblox won't let it in, not trying again", refusalTitle);
                // In a full game's line (Roblox's answer gives its place): kept, as a new window would start at the line's
                // end; asked again every minute, its place shown meanwhile; once Roblox gives none, tried again as before.
                NewWindow();
                Run(new GameTarget(920587237));
                Write(Line(DateTime.UtcNow.AddSeconds(-70), Launch));
                Poll(s => s is { Status: "joining", JobId: null } && DateTime.UtcNow - s.At > TimeSpan.FromSeconds(65));
                const string FullGame = "The game you requested is currently full. Please try again later.";
                checks[me] = (Task.FromResult<RobloxApi.JoinAnswer?>(new RobloxApi.JoinAnswer(200, 6, FullGame, null, 3)), DateTime.UtcNow);
                closing.Clear();
                Told();
                questions = 0;
                Tick();
                c.Check("in a full game's line (place 3), a minute without a server: not closed", !closing.Contains(me) && questions == 0 && Told() is null, questions);
                checks[me] = (Task.FromResult<RobloxApi.JoinAnswer?>(new RobloxApi.JoinAnswer(200, 6, FullGame, null, 3)), DateTime.UtcNow.AddSeconds(-61));
                reply = new(200, 6, FullGame, null, 2);
                Tick();
                c.Check("…asked again a minute on: its new place read, still not closed",
                    questions == 1 && !closing.Contains(me) && (Harness.Invoke(app, "JoinAnswerOf", me) as RobloxApi.JoinAnswer)?.QueuePosition == 2, questions);
                checks[me] = (new TaskCompletionSource<RobloxApi.JoinAnswer?>().Task, DateTime.UtcNow);
                c.Check("…while it is asked, the place it had is shown", (Harness.Invoke(app, "JoinAnswerOf", me) as RobloxApi.JoinAnswer)?.QueuePosition == 3);
                checks[me] = (Task.FromResult<RobloxApi.JoinAnswer?>(new RobloxApi.JoinAnswer(200, 6, FullGame, null, 2)), DateTime.UtcNow.AddSeconds(-61));
                reply = new(200, 6, FullGame);
                Tick();
                Tick();
                c.Check("…and once Roblox gives it no place, closed to try again as before", closing.Contains(me));
                closing.Clear();
                // A private server's join is asked as its public game's: that answer says nothing of the private server.
                NewWindow();
                Run(new GameTarget(920587237, null, "0123456789"));
                Write(Line(DateTime.UtcNow.AddSeconds(-70), Launch));
                Poll(s => s is { Status: "joining", JobId: null } && DateTime.UtcNow - s.At > TimeSpan.FromSeconds(65));
                checks[me] = (Task.FromResult<RobloxApi.JoinAnswer?>(new RobloxApi.JoinAnswer(200, 12, "Unable to join Game 517")), DateTime.UtcNow);
                Told();
                Tick();
                string? privateSaid = Told();
                c.Check("a private server's window isn't held by its public game's refusal: tried again as before",
                    closing.Contains(me) && privateSaid?.Contains("trying again", StringComparison.Ordinal) == true, privateSaid);
                closing.Clear();
                // Roblox holds the join for its check that the account isn't a bot: the window's own log says so (its line of
                // 2026-10-23), and shows Roblox's Security page, blank in Vietnam, then Join Error 529. The window stays, why is
                // said once, and it isn't rejoined once closed. Before, it was closed a minute on and opened again, for good.
                NewWindow();
                var checkedGame = Run(new GameTarget(920587237));
                questions = 0;
                Write(Line(DateTime.UtcNow.AddSeconds(-70), Launch));
                Write(Line(DateTime.UtcNow.AddSeconds(-69), """Error [DFLog::HttpTraceError] HttpResponse(#21 0x1dbb2c53ff8) time:320.8ms (net:320.8ms timeInRetryQueue:0.0ms)status:403  bodySize:82 url:{ "https://gamejoin.roblox.com/v1/join-game" } ip:128.116.97.3 external:0 numberOfTimesRetried:0 challengedByGcs"""));
                Poll(_ => watcher.JoinChallengedAt(me) is not null);
                c.Check("Roblox's bot check on the join read from the window's log", watcher.JoinChallengedAt(me) is not null);
                closing.Clear();
                Told();
                Tick();
                string? checkSaid = Told();
                c.Check("…a minute without a server: not closed to try again, and why is said",
                    !closing.Contains(me) && checkSaid?.Contains("Roblox wants a check that it isn't a bot first", StringComparison.Ordinal) == true, checkSaid);
                var checkAlert = Harness.Invoke(app, "AlertFor", "drop", account.Id,
                    ("{0} can't get into the game: Roblox wants a check that it isn't a bot first, so Bloxkeep leaves the window open and doesn't try again.",
                        new object?[] { account.Label }), checkSaid ?? "", DateTime.UtcNow)!;
                string? checkTitle = checkAlert.GetType().GetProperty("Title")!.GetValue(checkAlert) as string;
                string? checkWhy = checkAlert.GetType().GetProperty("Text")!.GetValue(checkAlert) as string;
                c.Check("…Discord's alert says it: \"Roblox wants a bot check, not trying again\", with why",
                    checkTitle == "Roblox wants a bot check, not trying again" && checkWhy?.Contains("isn't a bot", StringComparison.Ordinal) == true, $"{checkTitle} / {checkWhy}");
                Tick();
                c.Check("…said once, and Roblox's join API not asked", Told() is null && !closing.Contains(me) && questions == 0, questions);
                c.Check("…its card says so (Roblox wants a bot check), not joining", Harness.Invoke(app, "Phase", me) as string == "challenged", Harness.Invoke(app, "Phase", me));
                var checkedEnd = Harness.Invoke(app, "EndedState", me) as RobloxLogWatcher.ClientState;
                c.Check("…closed by the person, not rejoined", Harness.Invoke(app, "QueueRejoin", account.Id, checkedGame, checkedEnd) is false && queue.Count == 0, queue.Count);
                queue.Clear();
                // A game started again in that window (Retry) is a new join: the check isn't held over.
                Write(Line(DateTime.UtcNow, Launch));
                Poll(_ => watcher.JoinChallengedAt(me) is null);
                c.Check("…a game started again in the window: the bot check isn't held over", watcher.JoinChallengedAt(me) is null);
                // The person starts a game in a window held as banned (another, or the same again): the card no longer
                // says the ban, and the window isn't held as banned.
                NewWindow();
                Run(new GameTarget(Gpo));
                Write(Line(DateTime.UtcNow.AddSeconds(-20), Launch));
                Poll(s => s is { Status: "joining", JobId: null } && DateTime.UtcNow - s.At > TimeSpan.FromSeconds(15));
                noted.Add(me);
                bool bannedBefore = CardBanned();
                Write(Line(DateTime.UtcNow, Launch));
                Poll(s => s is { Status: "joining", PersonStarted: true });
                c.Check("the person starts a game in a window held as banned: the card no longer says the ban", bannedBefore && !CardBanned());
                Tick();
                c.Check("…nor is the window held as banned", !noted.Contains(me));
                // A ban noted before is asked about before opening (a game's ban may last a day): one Roblox would let in now
                // is forgotten, so the account opens as any other; one it still refuses is kept.
                var recheck = t.GetMethod("RecheckGameBansAsync", Harness.F)!;
                async Task Recheck() => await (Task)recheck.Invoke(app, [new List<Account> { account }, new GameTarget(Gpo), CancellationToken.None])!;
                reply = new(200, 2, null);
                account.GameBans = [new GameBan { PlaceId = Gpo, At = DateTime.UtcNow.AddDays(-1) }];
                await Recheck();
                c.Check("a ban noted before that Roblox no longer holds: forgotten before opening", account.GameBans.Count == 0);
                reply = new(200, 12, "Unable to join Game 211");
                account.GameBans = [new GameBan { PlaceId = Gpo, At = DateTime.UtcNow.AddDays(-1) }];
                await Recheck();
                c.Check("…one Roblox still holds: kept", account.GameBans.Count == 1);
                // …and a preset asks before leaving the account out (here Roblox still holds it: nothing opens).
                var openNow = running[account.Id];
                running.Clear();
                questions = 0;
                string? leftOutSaid = null;
                try { await Open(Guid.NewGuid()); } catch (UserError ex) { leftOutSaid = ex.Message; }
                c.Check("…a preset asks Roblox before leaving out an account a game banned", questions == 1 && leftOutSaid is not null, $"{questions} {leftOutSaid}");
                running[account.Id] = openNow;
                // Play asks Roblox whether each account can play the game, as Roblox's client does before joining
                // (multiget-playability-status), and asks the person first, with why, when Roblox says no. Roblox answers
                // from here (playabilityAsks counts them).
                var gameCache = Harness.Get<System.Collections.IDictionary>(app, "_games");
                const long Gated = 555, GatedUniverse = 5550;
                gameCache[Gated] = new GameInfo(Gated, GatedUniverse, "Gated", "someone", 0, null);
                var realPlayability = Harness.Get<object>(app, "_askPlayability");
                int playabilityAsks = 0;
                Func<Account, long, CancellationToken, Task<Playability?>> robloxSays = (_, _, _) => Task.FromResult<Playability?>(new Playability(false, 26, "Roblox's own words"));
                Harness.Set(app, "_askPlayability", (Func<Account, long, CancellationToken, Task<Playability?>>)((who, universe, ct) =>
                {
                    playabilityAsks++;
                    return universe == GatedUniverse ? robloxSays(who, universe, ct) : Task.FromResult<Playability?>(null);
                }));
                var playOpen = running[account.Id];
                running.Clear();
                var bansBeforePlay = account.GameBans;
                // A ban noted too, so that nothing opens whatever the checks find.
                account.GameBans = [new GameBan { PlaceId = Gated, At = DateTime.UtcNow }];
                try
                {
                    var playTask = (Task)launchAccounts.Invoke(app, [vault, new List<Account> { account }, new GameTarget(Gated), false, 0, null, null])!;
                    await playTask;
                    var played = playTask.GetType().GetProperty("Result")!.GetValue(playTask);
                    var playAsks = played?.GetType().GetProperty("Asks")!.GetValue(played) as List<string>;
                    string? playSaid = played?.GetType().GetProperty("Message")!.GetValue(played) as string;
                    c.Check("Play asks first when Roblox says the account can't play the game, with why",
                        playAsks?.Contains("playability") == true && playSaid?.Contains($"{account.Label} can't play this game: it needs an age check.") == true,
                        playSaid ?? played?.ToString());
                    var unplayable = t.GetMethod("UnplayableAsync", Harness.F)!;
                    async Task<int> Unplayable()
                    {
                        var task = (Task)unplayable.Invoke(app, [new List<Account> { account }, Gated, CancellationToken.None])!;
                        await task;
                        return ((System.Collections.ICollection)task.GetType().GetProperty("Result")!.GetValue(task)!).Count;
                    }
                    robloxSays = (_, _, _) => Task.FromResult<Playability?>(new Playability(true, 1, null));
                    c.Check("…not when Roblox says it can", await Unplayable() == 0);
                    robloxSays = (_, _, _) => Task.FromException<Playability?>(new HttpRequestException("offline"));
                    c.Check("…nor when Roblox couldn't be asked", await Unplayable() == 0);
                    robloxSays = async (_, _, ct) =>
                    {
                        await Task.Delay(Timeout.Infinite, ct); // a request given up throws, as HttpClient's does
                        return new Playability(false, 11, null);
                    };
                    var slow = Stopwatch.StartNew();
                    int slowCount = await Unplayable();
                    c.Check("…nor when Roblox takes too long: Play waits 6 s at most", slowCount == 0 && slow.Elapsed < TimeSpan.FromSeconds(9), slow.Elapsed);
                    int asksBefore = playabilityAsks;
                    string signIn = account.Cookie;
                    account.Cookie = "";
                    robloxSays = (_, _, _) => Task.FromResult<Playability?>(new Playability(false, 11, null));
                    c.Check("…and an account without a sign-in isn't asked", await Unplayable() == 0 && playabilityAsks == asksBefore);
                    account.Cookie = signIn;
                    c.Check("a reason Bloxkeep has no words for is said in Roblox's own",
                        t.GetMethod("PlayabilityWhy", Harness.F)!.Invoke(null, [new Playability(false, 25, "This experience is not available right now.")]) as string
                        == "This experience is not available right now");
                    // Roblox's live answer names the status ("GuestProhibited", seen 2026-10-21) where its docs give a number.
                    var standIn = typeof(RobloxApi).GetProperty("StandIn", Harness.F)!;
                    var realStandIn = standIn.GetValue(null);
                    standIn.SetValue(null, (Func<HttpRequestMessage, Task<HttpResponseMessage>>)(_ => Task.FromResult(new HttpResponseMessage(System.Net.HttpStatusCode.OK)
                    {
                        Content = new StringContent("""[{"playabilityStatus":"ContextualPlayabilityAgeCheckRequired","isPlayable":false,"universeId":5550,"unplayableDisplayText":"Age check required"}]"""),
                    })));
                    try
                    {
                        var parsed = await RobloxApi.GetPlayabilityAsync("test", Realm.Global, GatedUniverse);
                        c.Check("Roblox's answer read with its status as a name: 26, with Roblox's words", parsed is { Playable: false, Status: 26, Text: "Age check required" }, parsed);
                    }
                    finally
                    {
                        standIn.SetValue(null, realStandIn);
                    }
                }
                finally
                {
                    account.GameBans = bansBeforePlay;
                    running[account.Id] = playOpen;
                    Harness.Set(app, "_askPlayability", realPlayability);
                    gameCache.Remove(Gated);
                }
            }
            finally
            {
                account.Cookie = "";
                checks.Clear();
                Harness.Set(app, "_askJoin", realAsk);
            }
            // A rejoin waiting its turn for an account a game banned since it was queued: not done (no question to Roblox,
            // no window: it returns at once).
            account.GameBans = [new GameBan { PlaceId = Gpo, At = DateTime.UtcNow }];
            var queuedOpen = running[account.Id];
            running.Clear();
            var requestType = t.GetNestedType("RejoinRequest", Harness.F)!;
            var waitingRejoin = Activator.CreateInstance(requestType, account.Id, new GameTarget(Gpo), DateTime.UtcNow, 0, null, false)!;
            var rejoinTask = (Task)t.GetMethod("RejoinAsync", Harness.F)!.Invoke(app, [waitingRejoin])!;
            bool returnedAtOnce = rejoinTask.IsCompleted;
            await rejoinTask;
            c.Check("a rejoin waiting its turn for an account the game banned since: not done", returnedAtOnce);
            running[account.Id] = queuedOpen;
            account.GameBans.Clear();
            // A ban in the last lines of a window that closed before the logs were looked at: noted (and told) all the same.
            Told();
            var closedAtOnce = Activator.CreateInstance(launchType, account.Id, new HashSet<int>(), DateTime.UtcNow.AddMinutes(-2), new GameTarget(Gpo), false, null)!;
            var lastLines = new RobloxLogWatcher.ClientState("disconnected", Gpo, "e403336e-5756-44c9-b029-0ae6873a3d03", 267, DateTime.UtcNow, null, false, null, true);
            Harness.Invoke(app, "OnSessionEnded", account.Id, closedAtOnce, lastLines);
            c.Check("a ban in the last lines of a window closed at once: noted and told all the same",
                account.GameBans.SingleOrDefault()?.PlaceId == Gpo && Told() is not null && queue.Count == 0, string.Join(",", account.GameBans.Select(b => b.PlaceId)));
            // A game's ban holds in all its places: one noted for one place counts for another of the same game.
            var games = Harness.Get<System.Collections.IDictionary>(app, "_games");
            const long GpoOther = 1730877807;
            games[Gpo] = new GameInfo(Gpo, 648454481, "Grand Piece Online", "", 0, null);
            games[GpoOther] = new GameInfo(GpoOther, 648454481, "Grand Piece Online", "", 0, null);
            c.Check("a game's ban noted for one of its places counts for another", Harness.Invoke(app, "GameBanOf", account, (long?)GpoOther) is not null);
            games.Remove(Gpo);
            games.Remove(GpoOther);
            account.GameBans.Clear();
            // A game started in the window while still on a server, its old server's kick (a ban) logged after the new join:
            // that ban goes with the old server, not with the next server's disconnect.
            NewWindow();
            Run(new GameTarget(Gpo));
            var hopAt = DateTime.UtcNow;
            Write(Line(hopAt, Launch));
            Write(Line(hopAt, $"[FLog::Output] ! Joining game 'e403336e-5756-44c9-b029-0ae6873a3d03' place {Gpo} at 10.32.36.210"));
            Write(Line(hopAt, "[FLog::Network] serverId: 128.116.46.33|56123"));
            Write(Line(hopAt.AddSeconds(1), "[FLog::Output] ! Joining game '88888888-2222-3333-4444-555555555555' place 444 at 10.0.0.8"));
            Write(Line(hopAt.AddSeconds(1.1), BanLine));
            Write(Line(hopAt.AddSeconds(1.2), "[FLog::Network] Time to disconnect replication data: 0.2"));
            Write(Line(hopAt.AddSeconds(2), "[FLog::Network] serverId: 10.0.0.8|5000"));
            Write(Line(hopAt.AddSeconds(3), "[FLog::Network] Disconnection Notification. Reason: 277"));
            Write(Line(hopAt.AddSeconds(3.1), "[FLog::Network] Time to disconnect replication data: 0.2"));
            state = Poll(s => s.Status == "disconnected" && s.At > hopAt.AddSeconds(2.5));
            c.Check("an old server's ban logged after the next join: not taken for the next server's disconnect",
                state is { Status: "disconnected", Reason: 277, BannedByGame: false }, state);
            noted.Clear();
            account.GameBans.Clear();

            // Opening a game for an account whose window is somewhere already: asked first only if it is in a game.
            var leavesAGame = t.GetMethod("LeavesAGame", Harness.F)!;
            bool Asks(object launchOpen, GameTarget wanted) =>
                (bool)leavesAGame.Invoke(app, [Activator.CreateInstance(tupleType, me, launchOpen), wanted])!;
            NewWindow();
            var playing = Run(new GameTarget(Gpo));
            Write(Line(DateTime.UtcNow, Launch));
            Write(Line(DateTime.UtcNow, $"[FLog::Output] ! Joining game 'e403336e-5756-44c9-b029-0ae6873a3d03' place {Gpo} at 10.32.36.210"));
            Write(Line(DateTime.UtcNow, "[FLog::Network] serverId: 128.116.46.33|56123"));
            Poll(s => s is { Status: "ingame", PlaceId: Gpo });
            c.Check("opening a game for an account in another game: asked first", Asks(playing, new GameTarget(920587237)));
            NewWindow();
            var home = Run(null!);
            c.Check("…but not for one only on Roblox's home screen: it gives way without a word", !Asks(home, new GameTarget(920587237)));
            NewWindow();
            var stuckOpen = Run(new GameTarget(Gpo));
            Write(Line(DateTime.UtcNow, Launch));
            Poll(s => s is { Status: "joining", JobId: null });
            c.Check("…nor for one stuck on a join without a server (a game's ban, Join Error 600)", !Asks(stuckOpen, new GameTarget(920587237)));

            // Windows that end together right after another opened (Multi-Roblox on): Roblox closing them for the new one,
            // unless they ran out of memory. 2026-10-22: 6 windows, Windows' commit limit at 99.7 %, two logging Roblox's
            // own "cannot commit OS memory" before crashing, read as "Roblox closed 3 windows when another one opened".
            var armed = typeof(MultiInstance).GetProperty("Armed")!;
            bool wasArmed = MultiInstance.Armed;
            armed.SetValue(null, true);
            var starts = Harness.Get<Dictionary<int, DateTime>>(app, "_clientStarts");
            var suspects = Harness.Get<System.Collections.IList>(app, "_suspectEnds");
            var closedBy = Harness.Get<HashSet<Guid>>(app, "_closedByRoblox");
            var check = t.GetMethod("CheckClosedByRoblox", Harness.F)!;
            var shortAt = t.GetField("_memoryShortAt", Harness.F)!;
            Func<int, List<int>> noChildren = _ => [];
            bool Ended(Guid who, int pid, DateTime at, bool inOrder = true) => (bool)check.Invoke(app, [who, pid, at.AddMinutes(-2), at, inOrder, noChildren])!;
            try
            {
                var now = DateTime.UtcNow;
                Guid first = Guid.NewGuid(), second = Guid.NewGuid();
                void Fresh()
                {
                    starts.Clear();
                    starts[4_000_001] = now.AddSeconds(-5); // a new window, 5 s ago
                    suspects.Clear();
                    closedBy.Clear();
                }
                Fresh();
                shortAt.SetValue(app, null);
                bool firstSaid = Ended(first, 4_000_002, now), secondSaid = Ended(second, 4_000_003, now.AddSeconds(1));
                c.Check("two windows ending together right after another opened: still taken for Roblox closing them", !firstSaid && secondSaid);
                Fresh();
                shortAt.SetValue(app, now.AddSeconds(-8));
                firstSaid = Ended(first, 4_000_002, now);
                secondSaid = Ended(second, 4_000_003, now.AddSeconds(1));
                c.Check("…but not when Windows' virtual memory had just run out (they crashed for want of it)", !firstSaid && !secondSaid && closedBy.Count == 0);
                // Only windows that closed in order (shut down, notification-area copy started) can be Roblox's close: a crash
                // or a killed process writes neither, whatever the timing.
                Fresh();
                shortAt.SetValue(app, null);
                firstSaid = Ended(first, 4_000_002, now, inOrder: false);
                secondSaid = Ended(second, 4_000_003, now.AddSeconds(1), inOrder: false);
                c.Check("two windows that crashed or were killed together right after another opened: never taken for Roblox closing them",
                    !firstSaid && !secondSaid && closedBy.Count == 0);
                // The window's own log saying Windows refused it memory, Roblox's real lines of 2026-10-22.
                Fresh();
                shortAt.SetValue(app, null);
                NewWindow();
                Run(notBanned);
                var oomAt = DateTime.UtcNow.AddSeconds(-3);
                Write(Line(oomAt, Launch));
                Write(Line(oomAt, Join));
                Write(Line(oomAt, "Critical [DFLog::Mimalloc] cannot commit OS memory (error: 1455 (0x5AF), address: 0x019D2C5C0000, size: 0x250000 bytes)"));
                Write(Line(oomAt, "Critical [DFLog::Mimalloc] out-of-memory on OS allocation, try again... (attempt 1, 0x280000 bytes, error code: 0x5AF, address: 0x00000000, alignment: 0x10000, flags: 0x3000)"));
                Write(Line(oomAt.AddSeconds(1), "[LOGCHANNELS + 1] RBXCRASH: OutOfMemoryGraphics"));
                Poll(_ => watcher.OutOfMemoryAt(me) is not null);
                c.Check("Roblox's own out-of-memory lines are read", watcher.OutOfMemoryAt(me) is not null);
                c.Check("…and a crash isn't read as a window closed in order", watcher.ClosedInOrderAt(me) is null);
                Ended(second, 4_000_003, now);
                c.Check("…and a window that logged them isn't taken for one Roblox closed", !Ended(first, me, now.AddSeconds(1)) && !closedBy.Contains(first));
                NewWindow();
                Run(notBanned);
                var closedAt = DateTime.UtcNow.AddSeconds(-3);
                Write(Line(closedAt, Launch));
                Write(Line(closedAt, Join));
                Write(Line(closedAt.AddSeconds(1), "[FLog::Systray] Successfully launched new process in systray mode"));
                Write(Line(closedAt.AddSeconds(1), "[FLog::SingleSurfaceApp] shutDown: (stage:UGCGame)."));
                Poll(_ => watcher.ClosedInOrderAt(me) is not null);
                c.Check("a window closed in order is read so from its log (its shutDown and notification-area lines)", watcher.ClosedInOrderAt(me) is not null);
            }
            finally
            {
                armed.SetValue(null, wasArmed);
                shortAt.SetValue(app, null);
                suspects.Clear();
                closedBy.Clear();
            }
            // A window that ran out of memory: said so, and its rejoin waits while Windows' virtual memory is still all but
            // used up (a new window would crash as it opens, as on 2026-10-22); once there's room it goes ahead.
            {
                account.AutoRejoin = true;
                queue.Clear();
                Told();
                var oomLaunch = Run(new GameTarget(920587237));
                Harness.Get<HashSet<Guid>>(app, "_ranOutOfMemory").Add(account.Id);
                Harness.Invoke(app, "QueueRejoin", account.Id, oomLaunch, null);
                string? oomSaid = Told();
                c.Check("a window that ran out of memory: said so, not \"left the game\"",
                    oomSaid?.Contains("ran out of memory and Roblox crashed", StringComparison.Ordinal) == true && queue.Count == 1, oomSaid);
                var heldRequest = queue[0]!;
                var dueProp = heldRequest.GetType().GetProperty("Due")!;
                dueProp.SetValue(heldRequest, DateTime.UtcNow.AddSeconds(-1));
                var realRead = Harness.Get<object>(app, "_readMemory");
                // 64 GB of commit limit, 2 GB of it left: one more 3 GB client doesn't fit.
                var memoryNow = new MemoryStatus(32768, 16000, 50, 65536, 2048, 0);
                Harness.Set(app, "_readMemory", (Func<MemoryStatus>)(() => memoryNow));
                Harness.Set(app, "_lastSessionStart", DateTime.MinValue);
                var heldBans = account.GameBans;
                try
                {
                    Harness.Invoke(app, "RunDueRejoin", DateTime.UtcNow);
                    string? heldSaid = Told();
                    c.Check("…its rejoin waits while Windows' virtual memory is still all but used up, and says why",
                        queue.Count == 1 && (DateTime)dueProp.GetValue(queue[0])! > DateTime.UtcNow && Harness.Get<bool>(app, "_rejoining") is false
                        && heldSaid?.Contains("waits to rejoin: Windows' virtual memory is almost full", StringComparison.Ordinal) == true, heldSaid);
                    dueProp.SetValue(queue[0], DateTime.UtcNow.AddSeconds(-1));
                    Harness.Invoke(app, "RunDueRejoin", DateTime.UtcNow);
                    c.Check("…said once", Told() is null && queue.Count == 1);
                    // Room again (another window closed, the page file grew): it goes ahead. A ban by the game noted, so
                    // nothing opens: RejoinAsync returns at once.
                    account.GameBans = [new GameBan { PlaceId = 920587237, At = DateTime.UtcNow }];
                    memoryNow = new MemoryStatus(32768, 16000, 50, 65536, 20480, 0);
                    dueProp.SetValue(queue[0], DateTime.UtcNow.AddSeconds(-1));
                    Harness.Invoke(app, "RunDueRejoin", DateTime.UtcNow);
                    c.Check("…once there's room, it goes ahead", queue.Count == 0, queue.Count);
                    var heldAlert = Harness.Invoke(app, "AlertFor", "rejoinFail", account.Id,
                        ("{0} waits to rejoin: Windows' virtual memory is almost full, so a new Roblox window would crash as it opens. It rejoins once there's room.",
                            new object?[] { account.Label }), heldSaid ?? "", DateTime.UtcNow)!;
                    string? heldTitle = heldAlert.GetType().GetProperty("Title")!.GetValue(heldAlert) as string;
                    c.Check("…Discord's alert: \"Waiting for memory to rejoin\"", heldTitle == "Waiting for memory to rejoin", heldTitle);
                }
                finally
                {
                    account.GameBans = heldBans;
                    Harness.Set(app, "_readMemory", realRead);
                    queue.Clear();
                }
            }
            // A launch of several accounts lets rejoins go while it waits between two of them (with two minutes between
            // accounts, one dropped early would otherwise wait for the whole launch), but not of the accounts it is about
            // to open, and none while it opens one. A rejoin keeps the wait between accounts of the preset that opened it.
            {
                account.AutoRejoin = true;
                var heldBans = account.GameBans;
                // A ban by the game noted, so nothing opens: RejoinAsync returns at once.
                account.GameBans = [new GameBan { PlaceId = 920587237, At = DateTime.UtcNow }];
                running.Clear();
                queue.Clear();
                object DueRejoin(Guid? preset = null) =>
                    Activator.CreateInstance(requestType, account.Id, new GameTarget(920587237), DateTime.UtcNow.AddSeconds(-1), 0, preset, false)!;
                var realRead = Harness.Get<object>(app, "_readMemory");
                Harness.Set(app, "_readMemory", (Func<MemoryStatus>)(() => new MemoryStatus(32768, 16000, 50, 65536, 20480, 0)));
                var preset = new LaunchPreset { Name = "Slow", AccountIds = [account.Id], DelaySeconds = 120 };
                int heldDelay = vault.Data.Settings.LaunchDelaySeconds;
                var launchCts = new CancellationTokenSource();
                try
                {
                    Harness.Set(app, "_lastSessionStart", DateTime.MinValue);
                    Harness.Set(app, "_launchCts", launchCts);
                    queue.Add(DueRejoin());
                    Harness.Invoke(app, "RunDueRejoin", DateTime.UtcNow);
                    c.Check("a launch opening an account: no rejoin goes meanwhile", queue.Count == 1);
                    Harness.Set(app, "_launchWaiting", new HashSet<Guid> { account.Id });
                    Harness.Invoke(app, "RunDueRejoin", DateTime.UtcNow);
                    c.Check("…waiting between two accounts: not of an account it is about to open", queue.Count == 1);
                    Harness.Set(app, "_launchWaiting", new HashSet<Guid>());
                    Harness.Invoke(app, "RunDueRejoin", DateTime.UtcNow);
                    c.Check("…but of the others, yes", queue.Count == 0, queue.Count);
                    Harness.Set(app, "_launchWaiting", null);
                    Harness.Set(app, "_launchCts", null);

                    vault.Data.Settings.Presets.Add(preset);
                    vault.Data.Settings.LaunchDelaySeconds = 8;
                    Harness.Set(app, "_lastSessionStart", DateTime.UtcNow.AddSeconds(-30));
                    queue.Add(DueRejoin(preset.Id));
                    Harness.Invoke(app, "RunDueRejoin", DateTime.UtcNow);
                    c.Check("a rejoin of a preset's window keeps the preset's wait between accounts (2 minutes after the last window)", queue.Count == 1);
                    queue.Clear();
                    queue.Add(DueRejoin());
                    Harness.Invoke(app, "RunDueRejoin", DateTime.UtcNow);
                    c.Check("…one opened by Play keeps Settings' (8 s)", queue.Count == 0, queue.Count);
                    Harness.Set(app, "_lastSessionStart", DateTime.UtcNow.AddSeconds(-121));
                    queue.Add(DueRejoin(preset.Id));
                    Harness.Invoke(app, "RunDueRejoin", DateTime.UtcNow);
                    c.Check("…and the preset's goes once its wait is over", queue.Count == 0, queue.Count);
                }
                finally
                {
                    Harness.Set(app, "_launchWaiting", null);
                    Harness.Set(app, "_launchCts", null);
                    launchCts.Dispose();
                    vault.Data.Settings.Presets.Remove(preset);
                    vault.Data.Settings.LaunchDelaySeconds = heldDelay;
                    Harness.Set(app, "_readMemory", realRead);
                    account.GameBans = heldBans;
                    queue.Clear();
                }
                // The wait itself: rejoins of the others may go during it, and one started meanwhile pushes the next account
                // to the same gap after it, so windows still start one every N seconds.
                var waitBetween = t.GetMethod("WaitBeforeNextLaunchAsync", Harness.F)!;
                Harness.Set(app, "_lastSessionStart", DateTime.UtcNow);
                var took = Stopwatch.StartNew();
                var waiting = (Task)waitBetween.Invoke(app, [account, DateTime.UtcNow, 2, new[] { Guid.NewGuid() }, CancellationToken.None])!;
                await Task.Delay(300);
                bool letsRejoinsGo = Harness.Get<HashSet<Guid>?>(app, "_launchWaiting") is { Count: 1 };
                await Task.Delay(700);
                Harness.Set(app, "_lastSessionStart", DateTime.UtcNow); // a rejoin's window started a second in
                await waiting;
                took.Stop();
                c.Check("waiting between two accounts, rejoins of the others may go", letsRejoinsGo);
                c.Check("…and one started meanwhile pushes the next account to the same gap after it (about 3 s here, not 2)",
                    took.Elapsed >= TimeSpan.FromSeconds(2.8) && took.Elapsed < TimeSpan.FromSeconds(5), took.Elapsed);
                c.Check("…and once the wait is over, none go", Harness.Get<HashSet<Guid>?>(app, "_launchWaiting") is null);
            }
        }
        finally
        {
            running.Clear();
            queue.Clear();
            closing.Clear();
        }
    }
}
