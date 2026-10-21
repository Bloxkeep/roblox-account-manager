using System.Diagnostics;
using System.Text;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// Bloxkeep starting while Roblox windows already run (an update, a restart), with made-up Roblox logs that this process
/// holds open (the log watcher ties a log to whoever holds it): a long log, an old idle kick, and a client without a
/// log. The logs are written to a folder of the test's own (BLOXKEEP_ROBLOX_LOGS); no Roblox process is touched.
/// </summary>
static class RestartSuite
{
    public static async Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        string logs = Path.Combine(dir, "roblox-logs");
        Directory.CreateDirectory(logs);
        Environment.SetEnvironmentVariable("BLOXKEEP_ROBLOX_LOGS", logs);
        int me = Environment.ProcessId;
        var made = new List<FileStream>();
        string Line(DateTime at, string text) => $"{at:yyyy-MM-ddTHH:mm:ss.fff}Z,1.0,abcd,6 {text}\n";
        FileStream FakeLog(string tag, string text)
        {
            var fs = new FileStream(Path.Combine(logs, $"0.743.0.7431129_{DateTime.UtcNow:yyyyMMddTHHmmss}Z_Player_{tag}_last.log"),
                FileMode.Create, FileAccess.ReadWrite, FileShare.ReadWrite | FileShare.Delete);
            fs.Write(Encoding.UTF8.GetBytes(text));
            fs.Flush();
            made.Add(fs);
            return fs;
        }
        void Append(FileStream fs, string text) { fs.Seek(0, SeekOrigin.End); fs.Write(Encoding.UTF8.GetBytes(text)); fs.Flush(); }
        void CloseLogs() { foreach (var fs in made) fs.Dispose(); made.Clear(); }
        T? Until<T>(Func<T?> get, int ms = 15000) where T : class
        {
            var sw = Stopwatch.StartNew();
            while (sw.ElapsedMilliseconds < ms) { if (get() is { } v) return v; Thread.Sleep(200); }
            return null;
        }
        const string Join = "[FLog::Output] ! Joining game '11111111-2222-3333-4444-555555555555' place 2753915549 at 10.0.0.1";
        const string Connected = "[FLog::Network] serverId: 10.0.0.1|1234";
        const string Kick = "Info [DFLog::NetworkClient] Client:Disconnect setting replicator disconnect reason to 278";
        const string Ended = "[FLog::Network] Time to disconnect replication data: 0.4";
        try
        {
            // A client running for 12 hours: its join is more than the 4 MB a read takes at once back.
            var t0 = DateTime.UtcNow.AddHours(-12);
            var sb = new StringBuilder(Line(t0, Join) + Line(t0.AddSeconds(3), Connected));
            for (int i = 0; i < 60000; i++)
                sb.Append(Line(t0.AddSeconds(10 + i), "[FLog::Output] Wrap-deformer fetching meshes resulted in error.  Skipping follow-on stages."));
            FakeLog("LONGTEST", sb.ToString());
            var watcher = new RobloxLogWatcher();
            var state = Until(() => { watcher.Poll([me]); return watcher.StateOf(me); });
            c.Check("12-hour log: still known to be in its game", state is { Status: "ingame", PlaceId: 2753915549 }, state);
            CloseLogs();
            foreach (var f in Directory.GetFiles(logs))
                File.Delete(f);

            var seed = Vault.Create(Harness.Password);
            seed.Data.Settings.AntiAfk = true;
            var kicked = new Account { Username = "Kicked", UserId = 11 };
            var nolog = new Account { Username = "NoLog", UserId = 12 };
            var fresh = new Account { Username = "Fresh", UserId = 13 };
            seed.Data.Accounts.AddRange([kicked, nolog, fresh]);
            seed.Save();
            using var form = new Form();
            using var app = new AppController(form, _ => { });
            await Harness.Unlock(app);
            var t = typeof(AppController);
            var vault = Harness.Get<Vault>(app, "_vault");
            var started = Harness.Get<DateTime>(app, "_started");
            var running = Harness.Get<object>(app, "_running");
            var launchType = t.GetNestedType("PendingLaunch", Harness.F)!;
            var tupleType = running.GetType().GetGenericArguments()[1];
            void Run(Guid account, int pid, DateTime startedAt) =>
                running.GetType().GetMethod("set_Item")!.Invoke(running, [account, Activator.CreateInstance(tupleType, pid,
                    Activator.CreateInstance(launchType, account, new HashSet<int>(), startedAt, new GameTarget(2753915549), false, null))]);
            var watcherB = Harness.Get<RobloxLogWatcher>(app, "_logs");

            // Kicked for idling an hour ago, the window still on Roblox's message.
            var hourAgo = DateTime.UtcNow.AddHours(-1);
            var kickLog = FakeLog("KICKTEST", Line(hourAgo.AddMinutes(-30), Join) + Line(hourAgo.AddMinutes(-30).AddSeconds(3), Connected)
                + Line(hourAgo, Kick) + Line(hourAgo, Ended));
            Run(kicked.Id, me, DateTime.UtcNow.AddHours(-2));
            var kickState = Until(() => { watcherB.Poll([me]); return watcherB.StateOf(me); });
            c.Check("old kick read from the log", kickState is { Status: "disconnected", Reason: 278 }, kickState);
            // Each idle kick told is also written to error.log.
            int Told() => File.Exists(Path.Combine(dir, "error.log")) ? File.ReadAllText(Path.Combine(dir, "error.log")).Split("Idle kick with anti-AFK on").Length - 1 : 0;
            int before = Told();
            Harness.Invoke(app, "ReportIdleKicks", new HashSet<int> { me });
            c.Check("a kick from before Bloxkeep started isn't told again", Told() == before, Told() - before);
            Harness.Get<HashSet<int>>(app, "_idleKickReported").Clear();
            var nowJoin = DateTime.UtcNow;
            Append(kickLog, Line(nowJoin, Join) + Line(nowJoin.AddSeconds(1), Connected) + Line(nowJoin.AddSeconds(2), Kick) + Line(nowJoin.AddSeconds(2), Ended));
            var newKick = Until(() => { watcherB.Poll([me]); return watcherB.StateOf(me) is { At: var at } s && at > started ? s : null; });
            Harness.Invoke(app, "ReportIdleKicks", new HashSet<int> { me });
            c.Check("a kick after the start is told", newKick is not null && Told() == before + 1, $"{newKick} {Told() - before}");

            // A client open before the start whose log isn't found: no stay from its launch hours ago.
            const int NoLogPid = 999_991;
            Run(nolog.Id, NoLogPid, DateTime.UtcNow.AddHours(-3));
            int stays = vault.Data.PlaySessions.Count(s => s.AccountId == nolog.Id);
            Harness.Invoke(app, "TrackPlay", started.AddSeconds(50));
            c.Check("no-log client open before the start: no stay counted at once", vault.Data.PlaySessions.Count(s => s.AccountId == nolog.Id) == stays);
            Harness.Invoke(app, "TrackPlay", started.AddMinutes(3));
            var stay = vault.Data.PlaySessions.LastOrDefault(s => s.AccountId == nolog.Id);
            c.Check("after the log search: a stay from Bloxkeep's start, not from the launch", stay is not null && Math.Abs((stay.StartedAt - started).TotalSeconds) < 1, stay?.StartedAt);

            // A client opened after the start still gets its no-log stay after 45 s, from its launch.
            const int FreshPid = 999_992;
            var freshStart = started.AddMinutes(4);
            Run(fresh.Id, FreshPid, freshStart);
            Harness.Invoke(app, "TrackPlay", freshStart.AddSeconds(50));
            var freshStay = vault.Data.PlaySessions.LastOrDefault(s => s.AccountId == fresh.Id);
            c.Check("client opened after the start: stay from its launch after 45 s", freshStay is not null && Math.Abs((freshStay.StartedAt - freshStart).TotalSeconds) < 1, freshStay?.StartedAt);

            // Clients closed here just before a restart (an update's): Roblox still says "in game" for minutes, which
            // after the restart must still read as that late report, not as playing on another device.
            const string Server = "11111111-2222-3333-4444-555555555555";
            var ended = Harness.Get<System.Collections.IDictionary>(app, "_endedHere");
            var endedType = ended.GetType().GetGenericArguments()[1];
            object End(DateTime at) => Activator.CreateInstance(endedType, Server, (long?)2753915549, at)!;
            Harness.Get<System.Collections.IDictionary>(app, "_running").Clear();
            ended[kicked.Id] = End(DateTime.UtcNow.AddSeconds(-25));
            ended[nolog.Id] = End(DateTime.UtcNow.AddMinutes(-20)); // too long ago: Roblox has caught up by then
            Harness.Invoke(app, "NoteOpenSessions");
            c.Check("a client that ended here lately is kept in the vault, an old one isn't",
                vault.Data.EndedSessions.Select(e => e.AccountId).SequenceEqual([kicked.Id]) && vault.Data.EndedSessions[0].JobId == Server, vault.Data.EndedSessions.Count);
            await Harness.Call(app, "lock", new { });
            c.Check("…locked: forgotten in memory", ended.Count == 0, ended.Count);
            await Harness.Unlock(app);
            ended = Harness.Get<System.Collections.IDictionary>(app, "_endedHere");
            var p = new Presence(2, "Steal An Egg", 2753915549, 2753915549, Server, null, 1);
            bool Stale(Account a) => (bool)Harness.Invoke(app, "IsStaleInGame", a, p)!;
            c.Check("…and after unlocking (as after an update) its server's \"in game\" is still the late report",
                ended.Contains(kicked.Id) && Stale(kicked) && !Stale(nolog), $"{ended.Count} {Stale(kicked)} {Stale(nolog)}");

            // A rejoin waiting its turn (auto-rejoin's wait after failed tries): it holds no update back, and an update's
            // restart (or a lock) doesn't lose it. On 2026-10-21 one kept "Restart to update" refused while nothing opened.
            var queue = Harness.Get<System.Collections.IList>(app, "_rejoinQueue");
            var requestType = t.GetNestedType("RejoinRequest", Harness.F)!;
            object Waiting(TimeSpan inA) => Activator.CreateInstance(requestType, fresh.Id, new GameTarget(2753915549), DateTime.UtcNow + inA, 0, null, false)!;
            bool Opening() => (bool)t.GetProperty("OpeningRoblox", Harness.F)!.GetValue(app)!;
            queue.Clear();
            queue.Add(Waiting(TimeSpan.FromMinutes(5)));
            c.Check("a rejoin waiting 5 minutes isn't \"opening Roblox\": updating goes ahead", !Opening());
            queue[0] = Waiting(TimeSpan.FromSeconds(10));
            c.Check("…one due in 10 seconds is", Opening());
            queue[0] = Waiting(TimeSpan.FromMinutes(5));
            // With another window open, as at an update: picking the open windows up again saves the vault, which must
            // not wipe the lists before they are read (it did on the first try of this).
            Run(nolog.Id, 999_993, DateTime.UtcNow);
            Harness.Invoke(app, "NoteOpenSessions");
            await Harness.Call(app, "lock", new { });
            await Harness.Unlock(app);
            queue = Harness.Get<System.Collections.IList>(app, "_rejoinQueue");
            var back = queue.Cast<object>().SingleOrDefault();
            var due = back?.GetType().GetProperty("Due")!.GetValue(back) as DateTime?;
            c.Check("…and kept through a lock and unlock (as through an update's restart, with a window open), at its time",
                back is not null && back.GetType().GetProperty("AccountId")!.GetValue(back) is Guid id && id == fresh.Id
                && due is { } d && d > DateTime.UtcNow.AddMinutes(4), $"{queue.Count} {due}");
            ended = Harness.Get<System.Collections.IDictionary>(app, "_endedHere");
            c.Check("…the client that ended lately too", ended.Contains(kicked.Id), ended.Count);
            c.Check("…and the vault is saved again soon (picking the open windows up saved it without the rejoin)",
                Harness.Get<DateTime?>(app, "_saveDue") is not null);
            Harness.Get<System.Collections.IDictionary>(app, "_running").Clear();
            queue.Clear();
            // As the vault holds them when it is opened: one due long ago (Bloxkeep closed with a rejoin waiting, opened
            // the next day) isn't done by itself; one due a minute ago is, shortly.
            vault = Harness.Get<Vault>(app, "_vault");
            vault.Data.WaitingRejoins = [new WaitingRejoin { AccountId = fresh.Id, Target = new GameTarget(2753915549), Due = DateTime.UtcNow.AddHours(-9) }];
            Harness.Invoke(app, "RestoreWaitingRejoins", vault, vault.Data.WaitingRejoins);
            c.Check("…but one from hours ago isn't done after an unlock (no Roblox opening by itself the next day)", queue.Count == 0, queue.Count);
            vault.Data.WaitingRejoins = [new WaitingRejoin { AccountId = fresh.Id, Target = new GameTarget(2753915549), Due = DateTime.UtcNow.AddMinutes(-1) }];
            Harness.Invoke(app, "RestoreWaitingRejoins", vault, vault.Data.WaitingRejoins);
            c.Check("…one due a minute ago is, shortly", queue.Count == 1, queue.Count);
            queue.Clear();
            vault.Data.WaitingRejoins = [];
        }
        finally
        {
            CloseLogs();
        }
    }
}
