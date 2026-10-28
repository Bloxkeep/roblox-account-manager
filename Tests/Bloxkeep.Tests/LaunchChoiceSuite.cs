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
