using System.Diagnostics;
using System.Text;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// Auto-rejoin on a client that started its game and never got a server (a server that had ended, a full one's join
/// queue), as seen on 2026-10-17: closed after a minute, the rejoin going to any server of the game; and the leave Roblox
/// logs when Bloxkeep closes a client not taken for the person's own. Then the servers a party is sent to: empty ones
/// left out of the list, the first account's client telling whether the server took it, and the server tried next. Made-up Roblox logs held by this process (the log
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

            // Back on the start (the next game start from the Roblox window), a minute and more without a server.
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
            Write(Line(DateTime.UtcNow.AddSeconds(-3), Launch));
            Poll(s => s.Status == "joining" && DateTime.UtcNow - s.At < TimeSpan.FromSeconds(10));
            c.Check("3 s after the game start, no server yet: not known", Answer() is null, Answer());
            Write(Line(DateTime.UtcNow, Join));
            Poll(s => s.JobId is not null);
            c.Check("a server named in its log: the server took it", Answer() == "in", Answer());
            Write(Line(DateTime.UtcNow.AddSeconds(-13), Launch));
            Poll(s => s.JobId is null);
            c.Check("13 s without a server (a normal join gets one in about a second): it didn't", Answer() == "closed", Answer());
            var waited = Stopwatch.StartNew();
            var told = await (Task<string?>)t.GetMethod("WaitForServerAnswerAsync", Harness.F)!.Invoke(app, [account, CancellationToken.None])!;
            c.Check("…and the launch hears it without waiting", told == "closed" && waited.ElapsedMilliseconds < 2000, $"{told} {waited.ElapsedMilliseconds} ms");
        }
        finally
        {
            running.Clear();
            queue.Clear();
            closing.Clear();
        }
    }
}
