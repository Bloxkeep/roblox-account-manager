using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// Roblox VNG is always the last choice (the owner's rule, 2026-10-28): with another Roblox on the PC, "Run
/// RobloxPlayerBeta directly" and Default's fallback never pick VNG's client; with only VNG's, they use it; its own
/// option ("vng") opens it. Clients here are empty files in a folder of the test's own.
/// </summary>
static class LaunchChoiceSuite
{
    public static Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        string local = Path.Combine(dir, "local");
        typeof(RobloxInstalls).GetField("Local", Harness.F)!.SetValue(null, local);
        string Client(string install, string version)
        {
            string folder = Path.Combine(local, install, "Versions", version);
            Directory.CreateDirectory(folder);
            string exe = Path.Combine(folder, "RobloxPlayerBeta.exe");
            File.WriteAllBytes(exe, []);
            return exe;
        }
        // Roblox VNG's client, newer than the global one: still never ahead of it.
        string global = Client("Roblox", "version-global");
        File.SetLastWriteTimeUtc(global, DateTime.UtcNow.AddDays(-2));
        string vng = Client("RobloxPCVNG", "version-vng");
        var fallback = typeof(RobloxLauncher).GetMethod("FallbackLauncher", Harness.F)!;
        string? FallbackExe() => fallback.Invoke(null, []) is ValueTuple<string, string[]> f ? f.Item1 : null;

        c.Check("its own option opens Roblox VNG's client", RobloxInstalls.ClientFor("vng") == vng, RobloxInstalls.ClientFor("vng"));
        c.Check("with the global Roblox too, \"directly\" never picks VNG's (even newer)", !RobloxInstalls.IsVngClient(RobloxInstalls.ClientFor("direct") ?? ""),
            RobloxInstalls.ClientFor("direct"));
        c.Check("…nor Default's fallback", !RobloxInstalls.IsVngClient(FallbackExe() ?? ""), FallbackExe());

        // The sidebar's "Roblox doesn't seem to be installed": a client on disk is enough, whatever the roblox-player:
        // links say (Roblox VNG's uninstaller took them on 2026-10-28 with the global Roblox still here).
        using (var form = new Form())
        using (var app = new AppController(form, _ => { }))
        {
            var state = Harness.Invoke(app, "BuildState")!;
            c.Check("a Roblox client on the PC: not said to be missing", state.GetType().GetProperty("robloxInstalled")!.GetValue(state) is true);
        }

        // Roblox keeps a build per channel side by side (2026-10-29: its links on a test channel's 0.744.478, the main
        // channel's 0.745 next to it): the newest build being there is what the version card asks.
        string live = Client("Roblox", "version-live");
        c.Check("the newest build next to the links' one is found", RobloxInstalls.HasBuild(global, "version-live"));
        c.Check("…and one that isn't there isn't", !RobloxInstalls.HasBuild(global, "version-gone") && !RobloxInstalls.HasBuild(global, "")
            && !RobloxInstalls.HasBuild(null, "version-live"));
        File.Delete(live);

        // Of two Roblox builds side by side (a test channel's and the main one, 2026-10-30): the one Roblox's installer
        // registered, then the higher version, wins over the newer file.
        string channel = Client("Roblox", "version-channel"), main = Client("Roblox", "version-main");
        File.SetLastWriteTimeUtc(main, DateTime.UtcNow.AddDays(-1));
        var registered = typeof(RobloxInstalls).GetProperty("RegisteredBuildOverride", Harness.F)!;
        void Registered(Func<string?>? build) => registered.SetValue(null, build);
        Registered(() => "version-main");
        c.Check("the build Roblox registered is used, though another's file is newer", RobloxInstalls.FindOfficialClient() == main, RobloxInstalls.FindOfficialClient());
        Registered(() => null);
        // A real program's file version (Windows' Notepad, 10.x) against the empty files (0.0.0.0).
        File.Copy(Path.Combine(Environment.SystemDirectory, "notepad.exe"), main, overwrite: true);
        File.SetLastWriteTimeUtc(main, DateTime.UtcNow.AddDays(-1));
        c.Check("…without one, the higher version, though another's file is newer", RobloxInstalls.FindOfficialClient() == main, RobloxInstalls.FindOfficialClient());
        Registered(null);
        File.Delete(channel);
        File.Delete(main);

        // Between installs ("directly" looks at Roblox's own and every launcher's): Roblox's registered build counts only
        // among Roblox's own builds, so a stale one never wins over Bloxstrap's newer copy, and a launcher's build named
        // like it gets nothing from the name.
        var newest = typeof(RobloxInstalls).GetMethod("Newest", Harness.F)!;
        string? Pick(params string[] clients) => (string?)newest.Invoke(null, [clients]);
        string stale = Client("Roblox", "version-registered"), strap = Client("Bloxstrap", "version-newer");
        File.Copy(Path.Combine(Environment.SystemDirectory, "notepad.exe"), strap, overwrite: true);
        File.SetLastWriteTimeUtc(strap, DateTime.UtcNow.AddDays(-1));
        Registered(() => "version-registered");
        try
        {
            c.Check("Roblox's stale registered build against Bloxstrap's newer one: Bloxstrap's", Pick(stale, strap) == strap, Pick(stale, strap));
            string named = Client("Bloxstrap", "version-registered");
            c.Check("…a launcher's build named like the registered one: by its version, as the others", Pick(strap, named) == strap, Pick(strap, named));
            string own = Client("Roblox", "version-other");
            File.SetLastWriteTimeUtc(stale, DateTime.UtcNow.AddDays(-2));
            c.Check("…among Roblox's own builds the registered one still comes first", Pick(stale, own) == stale, Pick(stale, own));
        }
        finally
        {
            Registered(null);
            Directory.Delete(Path.Combine(local, "Bloxstrap"), recursive: true);
            File.Delete(stale);
            File.Delete(Path.Combine(local, "Roblox", "Versions", "version-other", "RobloxPlayerBeta.exe"));
        }

        // A test channel's build registered and newer, Roblox's live build beside it (a user's PC, 2026-11-13: each window
        // opened on the test build updated Roblox and closed every other): the live one is started, as RoALT does.
        string test = Client("Roblox", "version-test"), liveBuild = Client("Roblox", "version-live2");
        File.Copy(Path.Combine(Environment.SystemDirectory, "notepad.exe"), test, overwrite: true);
        Registered(() => "version-test");
        try
        {
            c.Check("without the live build known: the registered test build, as before", RobloxInstalls.FindOfficialClient() == test, RobloxInstalls.FindOfficialClient());
            RobloxInstalls.LiveBuild = "version-live2";
            c.Check("the live build known and installed: it, over the registered and higher test build", RobloxInstalls.FindOfficialClient() == liveBuild, RobloxInstalls.FindOfficialClient());
            c.Check("…the links' test build gives way to it", RobloxInstalls.LiveBeside(test) == liveBuild, RobloxInstalls.LiveBeside(test));
            c.Check("…the live build itself stays", RobloxInstalls.LiveBeside(liveBuild) == liveBuild);
            File.Delete(liveBuild);
            c.Check("the live build not installed: the test build, nothing beside it", RobloxInstalls.FindOfficialClient() == test && RobloxInstalls.LiveBeside(test) == test,
                RobloxInstalls.FindOfficialClient());
        }
        finally
        {
            RobloxInstalls.LiveBuild = null;
            Registered(null);
            Directory.Delete(Path.GetDirectoryName(test)!, recursive: true);
            if (Directory.Exists(Path.GetDirectoryName(liveBuild)))
                Directory.Delete(Path.GetDirectoryName(liveBuild)!, recursive: true);
        }

        // A build whose DLL is gone can't start (the exe names it); one from before that DLL, or with it beside, can.
        string broken = Client("Roblox", "version-broken");
        File.WriteAllBytes(broken, [.. new byte[(1 << 20) - 5], .. System.Text.Encoding.ASCII.GetBytes("RobloxPlayerBeta.dll"), 0]);
        c.Check("a client missing the DLL it names, the name across two reads: can't start", RobloxInstalls.IsBroken(broken));
        c.Check("…and isn't picked while another build is there", RobloxInstalls.FindOfficialClient() == global, RobloxInstalls.FindOfficialClient());
        c.Check("…and isn't the build said to be installed", !RobloxInstalls.HasBuild(global, "version-broken"));
        c.Check("…Roblox's install is said to have one", RobloxInstalls.OwnBuildBroken());
        File.WriteAllBytes(Path.Combine(Path.GetDirectoryName(broken)!, "RobloxPlayerBeta.dll"), []);
        c.Check("with the DLL beside it: fine", !RobloxInstalls.IsBroken(broken));
        c.Check("a client that doesn't name the DLL: fine without it", !RobloxInstalls.IsBroken(global));
        Directory.Delete(Path.GetDirectoryName(broken)!, recursive: true);

        // A client behind Roblox's newest build isn't played on: Roblox's own updates itself, a launcher's goes through it.
        c.Check("\"directly\", up to date: stays directly", RobloxInstalls.UpdatingOption("direct", true) == "direct");
        c.Check("\"directly\", newest build not known: stays directly", RobloxInstalls.UpdatingOption("direct", null) == "direct");
        c.Check("Roblox's own client updates itself when opened", RobloxInstalls.UpdatesOnOpen(global));
        c.Check("another option, out of date: left as chosen", RobloxInstalls.UpdatingOption("roblox", false) == "roblox");
        // Roblox's own copy is updated by its installer beside the client (it opens the Roblox app after; Bloxkeep closes it).
        string ownInstaller = Path.Combine(Path.GetDirectoryName(RobloxInstalls.ClientFor("roblox")!)!, RobloxInstalls.OwnInstaller);
        c.Check("Roblox's own copy without its installer: nothing updates it", RobloxInstalls.UpdaterFor("roblox") is null, RobloxInstalls.UpdaterFor("roblox"));
        File.WriteAllBytes(ownInstaller, []);
        c.Check("…with it: its installer does", RobloxInstalls.UpdaterFor("roblox") == ownInstaller && RobloxInstalls.IsOwnInstaller(ownInstaller), RobloxInstalls.UpdaterFor("roblox"));
        c.Check("…\"directly\" on it, out of date: stays directly (it updates as it opens)", RobloxInstalls.UpdatingOption("direct", false) == "direct",
            RobloxInstalls.UpdatingOption("direct", false));
        c.Check("…and Roblox VNG's client never gets it", RobloxInstalls.UpdaterFor("vng") is null);
        File.Delete(ownInstaller);
        string strapExe = Path.Combine(local, "Bloxstrap", "Bloxstrap.exe");
        string strapClient = Client("Bloxstrap", "version-copy");
        File.WriteAllBytes(strapExe, []);
        File.Copy(Path.Combine(Environment.SystemDirectory, "notepad.exe"), strapClient, overwrite: true);
        c.Check("\"directly\" on a launcher's old copy: through that launcher", RobloxInstalls.UpdatingOption("direct", false) == "bloxstrap", RobloxInstalls.UpdatingOption("direct", false));
        Directory.Delete(Path.Combine(local, "Bloxstrap"), recursive: true);

        // A window's build against Roblox's newest.
        RobloxInstalls.ClientVersion Build(string version, string folder) => new(version, folder, "", "");
        var newestBuild = Build("0.745.0.100", "version-new");
        c.Check("a window on the newest build isn't behind", !RobloxInstalls.IsBehind(Build("0.745.0.100", "version-new"), newestBuild));
        c.Check("a window on an older build is", RobloxInstalls.IsBehind(Build("0.744.0.100", "version-old"), newestBuild));
        c.Check("a new build under the same number counts too", RobloxInstalls.IsBehind(Build("0.745.0.100", "version-old"), newestBuild));
        c.Check("a newer number is another channel's, not behind", !RobloxInstalls.IsBehind(Build("0.746.0.100", "version-test"), newestBuild));
        c.Check("the Store's copy has no build folder: not compared", !RobloxInstalls.IsBehind(Build("2.744.1.0", ""), newestBuild));
        c.Check("newest or window not known: not behind", !RobloxInstalls.IsBehind(null, newestBuild) && !RobloxInstalls.IsBehind(newestBuild, null));

        // Roblox releasing a build while windows are open (the owner's question, 2026-11-14): a launch with Roblox open asks
        // for the live build again unless it was just asked; with none open the 30-minute check is enough.
        var at = new DateTime(2026, 11, 14, 12, 0, 0, DateTimeKind.Utc);
        c.Check("live build never asked: a launch asks", AppController.AskLiveAgain(false, DateTime.MinValue, at, 0));
        c.Check("Roblox open, asked a minute ago: asked again", AppController.AskLiveAgain(true, at, at.AddMinutes(1), 3));
        c.Check("Roblox open, just asked (a batch): not asked again", !AppController.AskLiveAgain(true, at, at.AddSeconds(10), 3));
        c.Check("no Roblox open: the update closes nothing, not asked", !AppController.AskLiveAgain(true, at, at.AddMinutes(20), 0));
        // A staged rollout: Roblox lists a build this PC doesn't get yet. A client started for it, in a game on its old build, shows it.
        c.Check("asked for the new build, in a game on the old one: not rolled out here", AppController.NotRolledOut("version-new", Build("0.744.0.100", "version-old"), true));
        c.Check("…still loading: not known yet", !AppController.NotRolledOut("version-new", Build("0.744.0.100", "version-old"), false));
        c.Check("…in a game on the new build: it was rolled out", !AppController.NotRolledOut("version-new", Build("0.745.0.100", "VERSION-NEW"), true));
        c.Check("…its build not read: not known", !AppController.NotRolledOut("version-new", null, true));

        // When: "keep": a window opened while Roblox has an update plays on the build running, from a copy without Roblox's
        // updater (Roblox looks for it beside itself only, and without it skips the update).
        RobloxInstalls.KeptRoot = Path.Combine(dir, "kept");
        string keepExe = Client("Roblox", "version-keep");
        string keepDir = Path.GetDirectoryName(keepExe)!;
        File.WriteAllText(Path.Combine(keepDir, "RobloxPlayerInstaller.exe"), "updater");
        Directory.CreateDirectory(Path.Combine(keepDir, "content", "fonts"));
        File.WriteAllText(Path.Combine(keepDir, "content", "fonts", "a.ttf"), "font");
        Directory.CreateDirectory(Path.Combine(keepDir, "ClientSettings"));
        File.WriteAllText(Path.Combine(keepDir, "ClientSettings", "ClientAppSettings.json"), "{\"a\":1}");
        string kept = RobloxInstalls.KeptCopy(keepExe);
        string keptDir = Path.GetDirectoryName(kept)!;
        c.Check("kept: Roblox's client copied outside Roblox's install", File.Exists(kept) && !RobloxInstalls.IsRobloxOwn(kept), kept);
        c.Check("…without Roblox's updater beside it", !File.Exists(Path.Combine(keptDir, "RobloxPlayerInstaller.exe")));
        c.Check("…with its game files", File.ReadAllText(Path.Combine(keptDir, "content", "fonts", "a.ttf")) == "font");
        c.Check("…and its Fast Flags", File.ReadAllText(Path.Combine(keptDir, "ClientSettings", "ClientAppSettings.json")) == "{\"a\":1}");
        File.WriteAllText(Path.Combine(keepDir, "ClientSettings", "ClientAppSettings.json"), "{\"a\":2}");
        c.Check("the next window: the same copy, its Fast Flags as written now", RobloxInstalls.KeptCopy(keepExe) == kept
            && File.ReadAllText(Path.Combine(keptDir, "ClientSettings", "ClientAppSettings.json")) == "{\"a\":2}");
        c.Check("…and the Fast Flags aren't Roblox's own file (a write there stays there)",
            File.ReadAllText(Path.Combine(keepDir, "ClientSettings", "ClientAppSettings.json")) == "{\"a\":2}");
        c.Check("a launcher's copy has no updater beside it: started as it is", RobloxInstalls.KeptCopy(Path.Combine(dir, "Bloxstrap", "RobloxPlayerBeta.exe"))
            == Path.Combine(dir, "Bloxstrap", "RobloxPlayerBeta.exe"));
        RobloxInstalls.ForgetKeptCopies();
        c.Check("no Roblox open: the kept build goes, Roblox's own stays", !Directory.Exists(RobloxInstalls.KeptRoot) && File.Exists(keepExe));
        Directory.Delete(keepDir, recursive: true);

        // Only Roblox VNG left (a launcher of this PC's own may still stand ahead of it: then that one is used).
        Directory.Delete(Path.Combine(local, "Roblox"), recursive: true);
        if (RobloxInstalls.FindAnyClient() is null)
        {
            c.Check("only Roblox VNG on the PC: \"directly\" uses it, as the last choice", RobloxInstalls.ClientFor("direct") == vng, RobloxInstalls.ClientFor("direct"));
            c.Check("…and so does Default's fallback", FallbackExe() == vng, FallbackExe());
        }
        return Task.CompletedTask;
    }
}
