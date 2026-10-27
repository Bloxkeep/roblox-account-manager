using System.Diagnostics;
using System.Reflection;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>Real Windows file sharing/junctions and process metadata, using only disposable files and stand-ins.</summary>
static class HomeSessionSuite
{
    static readonly Type T = typeof(Prefs).Assembly.GetType("RobloxManager.RobloxHomeSession")!;
    static object New(string storage, string aliases) => Activator.CreateInstance(T, storage, aliases)!;
    static object? Invoke(object session, string method, params object?[] args) => T.GetMethod(method, Harness.F)!.Invoke(session, args);
    static void Cleanup(string aliases) => T.GetMethod("CleanupAliases", Harness.F)!.Invoke(null, [aliases]);

    public static Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        string install = Path.Combine(dir, "install");
        Directory.CreateDirectory(install);
        string exe = Path.Combine(install, "RobloxPlayerBeta.exe"), keep = Path.Combine(install, "do-not-delete.dat");
        File.WriteAllText(exe, "stub");
        File.WriteAllText(keep, "the real Roblox install");
        string storage = Path.Combine(dir, "LocalStorage"), aliases = Path.Combine(dir, "aliases");
        Directory.CreateDirectory(storage);
        string cookies = Path.Combine(storage, "RobloxCookies.dat"), appStorage = Path.Combine(storage, "appStorage.json");
        File.WriteAllText(cookies, "existing cookie stand-in");
        File.WriteAllText(appStorage, "existing settings stand-in");
        var session = New(storage, aliases + Path.DirectorySeparatorChar);
        try
        {
            c.Check("both existing shared files are locked", Locked(cookies) && Locked(appStorage));
            var info = (ProcessStartInfo)Invoke(session, "CreateStartInfo", exe)!;
            var info2 = (ProcessStartInfo)Invoke(session, "CreateStartInfo", exe)!;
            c.Check("each start gets a separate junction with the unmodified client reachable", info.WorkingDirectory != install
                && info.FileName != info2.FileName && File.ReadAllText(info.FileName) == "stub" && File.Exists(info2.FileName));
            c.Check("launch carries no ticket, game link or shell launch", info.ArgumentList.Count == 0 && info.Arguments.Length == 0
                && !info.UseShellExecute && info.WorkingDirectory == Path.GetDirectoryName(info.FileName));
            CheckClients(c, session, info.FileName, aliases, exe);
            CheckNativePaths(c, install, info.WorkingDirectory);
        }
        finally { ((IDisposable)session).Dispose(); }
        c.Check("existing files are unlocked, retained, and unchanged", !Locked(cookies) && !Locked(appStorage)
            && File.ReadAllText(cookies) == "existing cookie stand-in" && File.ReadAllText(appStorage) == "existing settings stand-in");
        Cleanup(aliases);
        c.Check("cleanup removes junctions without touching the install", Directory.GetDirectories(aliases).Length == 0
            && File.Exists(exe) && File.ReadAllText(keep) == "the real Roblox install");
        string decoy = Path.Combine(aliases, "client-" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(decoy);
        File.WriteAllText(Path.Combine(decoy, "real.txt"), "not a junction");
        Cleanup(aliases);
        c.Check("cleanup leaves real directories alone", File.Exists(Path.Combine(decoy, "real.txt")));
        CheckMissingAndBusyFiles(c, dir, aliases);
        CheckController(c, dir, storage, aliases);
        return Task.CompletedTask;
    }

    static void CheckClients(Checks c, object session, string aliasClient, string aliases, string normalClient)
    {
        bool Owns(string path) => (bool)Invoke(session, "OwnsClientPath", path)!;
        c.Check("only exact alias paths belong: normal, sibling, nested and malformed names do not", Owns(aliasClient) && !Owns(normalClient)
            && !Owns(Path.Combine(aliases + "-sibling", "client-" + Guid.NewGuid().ToString("N"), "RobloxPlayerBeta.exe"))
            && !Owns(Path.Combine(Path.GetDirectoryName(aliasClient)!, "nested", "RobloxPlayerBeta.exe"))
            && !Owns(Path.Combine(aliases, "client-invalid", "RobloxPlayerBeta.exe")));
        var home = new RobloxProcesses.PlayerProcess(101, 0, Created: 1001);
        var normal = new RobloxProcesses.PlayerProcess(202, 0, Created: 2002);
        int queries = 0;
        (int[] Pids, bool Uncertain) Find(RobloxProcesses.PlayerProcess[] players, Func<int, long, string?> query) =>
            ((int[], bool))Invoke(session, "FindClients", players, query)!;
        var found = Find([home, normal], (pid, _) => { queries++; return pid == home.Pid ? aliasClient : normalClient; });
        c.Check("mixed clients: only the alias client keeps the session alive", found.Pids.SequenceEqual([101]) && !found.Uncertain);
        found = Find([home, normal], (_, _) => throw new Exception("same process must use cached classification"));
        c.Check("process identity caches paths without repeated native queries", found.Pids.SequenceEqual([101]) && queries == 2);
        found = Find([normal], (_, _) => throw new Exception("normal identity must stay cached"));
        c.Check("last alias closes: a regular/ticket client alone no longer holds locks", found.Pids.Length == 0 && !found.Uncertain);
        found = Find([home with { Created = 3003 }, normal], (_, _) => normalClient);
        c.Check("a reused PID does not inherit alias ownership", found.Pids.Length == 0 && !found.Uncertain);
        var unknown = new RobloxProcesses.PlayerProcess(303, 0, Created: 4004);
        found = Find([unknown, normal], (_, _) => null);
        c.Check("an unqueryable new client conservatively holds the locks", found.Uncertain);
        found = Find([unknown, normal], (_, _) => normalClient);
        c.Check("failed queries are retried and release once confirmed regular", !found.Uncertain && found.Pids.Length == 0);
        var noTime = home with { Created = 0 };
        Find([noTime], (_, _) => aliasClient);
        found = Find([noTime], (_, _) => normalClient);
        c.Check("unknown creation times are never cached across PID reuse", found.Pids.Length == 0 && !found.Uncertain);
    }

    static void CheckNativePaths(Checks c, string install, string alias)
    {
        using var me = Process.GetCurrentProcess();
        long created = me.StartTime.ToFileTimeUtc();
        c.Check("native launch-path query reads the expected process identity", string.Equals(RobloxProcesses.LaunchPath(me.Id, created),
            Environment.ProcessPath, StringComparison.OrdinalIgnoreCase));
        c.Check("native launch-path query rejects reused and missing identities", RobloxProcesses.LaunchPath(me.Id, created + 1) is null
            && RobloxProcesses.LaunchPath(int.MaxValue - 7, 1) is null);
        File.Copy(Path.Combine(Environment.SystemDirectory, "ping.exe"), Path.Combine(install, "ping.exe"));
        using var standIn = Process.Start(new ProcessStartInfo(Path.Combine(alias, "ping.exe"))
        {
            UseShellExecute = false, CreateNoWindow = true, ArgumentList = { "-n", "20", "127.0.0.1" },
        })!;
        try
        {
            string? path = RobloxProcesses.LaunchPath(standIn.Id, standIn.StartTime.ToFileTimeUtc());
            c.Check("Windows launch-path query preserves the junction launch path", string.Equals(Path.GetDirectoryName(path), alias, StringComparison.OrdinalIgnoreCase), path);
        }
        finally
        {
            if (!standIn.HasExited) standIn.Kill();
            standIn.WaitForExit();
        }
    }

    static void CheckMissingAndBusyFiles(Checks c, string dir, string aliases)
    {
        string storage = Path.Combine(dir, "missing-storage");
        string cookie = Path.Combine(storage, "RobloxCookies.dat"), settings = Path.Combine(storage, "appStorage.json");
        var session = New(storage, aliases);
        try { c.Check("a first run locks its own empty placeholders", Locked(cookie) && Locked(settings)); }
        finally { ((IDisposable)session).Dispose(); }
        c.Check("first-run placeholders are deleted when handles close", !File.Exists(cookie) && !File.Exists(settings));
        File.WriteAllText(cookie, "");
        session = New(storage, aliases);
        ((IDisposable)session).Dispose();
        c.Check("a pre-existing empty file is retained; only the new one disappears", File.Exists(cookie) && new FileInfo(cookie).Length == 0 && !File.Exists(settings));
        File.WriteAllText(settings, "busy original");
        using (var busy = new FileStream(settings, FileMode.Open, FileAccess.Read, FileShare.None))
        {
            bool failed = false;
            try { ((IDisposable)New(storage, aliases)).Dispose(); }
            catch (TargetInvocationException ex) when (ex.InnerException is IOException) { failed = true; }
            c.Check("a busy second file rolls back the first lock", failed && !Locked(cookie));
            File.Delete(cookie);
            failed = false;
            try { ((IDisposable)New(storage, aliases)).Dispose(); }
            catch (TargetInvocationException ex) when (ex.InnerException is IOException) { failed = true; }
            c.Check("partial acquisition removes its own placeholder", failed && !File.Exists(cookie));
        }
        c.Check("failed acquisition leaves the existing busy file unchanged", File.ReadAllText(settings) == "busy original");
    }

    static void CheckController(Checks c, string dir, string storage, string aliases)
    {
        using var form = new Form();
        using var app = new AppController(form, _ => { });
        string root = (string)typeof(AppController).GetField("HomeAliases", Harness.F)!.GetValue(null)!;
        c.Check("controller stores aliases in its data directory's cache, outside the old TEMP root", root == Path.Combine(dir, "cache", "roblox-client-paths"));
        var messages = new List<string>();
        bool Ready() => (bool)Harness.Invoke(app, "HomeLaunchReady", (Action<string>)messages.Add)!;
        Prefs.MultiRobloxAtStart = false;
        c.Check("Multi-Roblox turned off during retry reports why opening stopped", !Ready() && messages.Count == 1 && messages[0].Contains("Turn on Multi-Roblox"));
        messages.Clear();
        Prefs.MultiRobloxAtStart = true;
        c.Check("Multi-Roblox no longer armed during retry reports why", !Ready() && messages.Count == 1 && messages[0].Length > 0);
        Prefs.MultiRobloxAtStart = false;
        Harness.Set(app, "_homeSession", New(storage, aliases));
        Harness.Set(app, "_homeStartedAt", DateTime.UtcNow.AddMinutes(-1));
        using var me = Process.GetCurrentProcess();
        IReadOnlyList<RobloxProcesses.PlayerProcess> normal = new[] { new RobloxProcesses.PlayerProcess(me.Id, 0, Created: me.StartTime.ToFileTimeUtc()) };
        void Release(bool scan) => Harness.Invoke(app, "ReleaseHomeSessionIfIdle", scan, normal, new HashSet<int>(), DateTime.UtcNow);
        Release(false);
        c.Check("a skipped process scan never unlocks session files", Locked(Path.Combine(storage, "RobloxCookies.dat")));
        Harness.Set(app, "_openingHome", true);
        bool blocked = false;
        try { Harness.Invoke(app, "CheckCanLaunch", new List<Account>()); }
        catch (TargetInvocationException ex) when (ex.InnerException is UserError error && error.Message.Contains("Already launching")) { blocked = true; }
        c.Check("an account launch cannot race a separate-window launch", blocked);
        Release(true);
        c.Check("a launch in progress keeps its locks", Locked(Path.Combine(storage, "RobloxCookies.dat")));
        Harness.Set(app, "_openingHome", false);
        Release(true);
        c.Check("controller releases when only a regular client remains", !Locked(Path.Combine(storage, "RobloxCookies.dat"))
            && !(bool)typeof(AppController).GetProperty("HomeSessionsActive", Harness.F)!.GetValue(app)!);
    }

    static bool Locked(string path)
    {
        try { using var file = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite); return false; }
        catch (FileNotFoundException) { return false; }
        catch (IOException) { return true; }
    }
}