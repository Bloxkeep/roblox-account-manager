using System.Diagnostics;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// Quitting (AppController.Dispose), on a stand-in Roblox folder (never the real one): one step that fails doesn't stop
/// the others, so the last change still reaches the vault on disk and the vault is closed; a Light AFK file that can't be
/// put back at quit is kept for the next start; quitting is quick and a second time does nothing.
/// </summary>
static class ShutdownSuite
{
    public static async Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        using (var seed = Vault.Create(Harness.Password))
            seed.Save();
        var app = new AppController(new Form(), _ => { });
        SynchronizationContext.SetSynchronizationContext(null);
        await Harness.Unlock(app);
        var vault = Harness.Get<Vault>(app, "_vault");

        // A change made just before quitting, not saved yet.
        vault.Data.Accounts.Add(new Account { Id = Guid.NewGuid(), Username = "quit_user", Alias = "changed before quitting" });
        Harness.Invoke(app, "PersistSoon");

        // Light AFK's flags written for a window, the file then held read-only: the restore at quit can't write it.
        string build = Path.Combine(dir, "Roblox", "Versions", "version-aaaa");
        Directory.CreateDirectory(build);
        File.WriteAllText(Path.Combine(build, "RobloxPlayerBeta.exe"), "");
        string flags = Path.Combine(build, "ClientSettings", "ClientAppSettings.json");
        Directory.CreateDirectory(Path.GetDirectoryName(flags)!);
        const string own = """{"FFlagMyOwn": true}""";
        File.WriteAllText(flags, own);
        RobloxFlags.Apply(Path.Combine(build, "RobloxPlayerBeta.exe"));
        Harness.Set(app, "_robloxFlagsPending", true);
        File.SetAttributes(flags, FileAttributes.ReadOnly);

        // A step that fails on the way: Discord's presence gone (any helper that throws).
        Harness.Set(app, "_discordPresence", null);

        // A macro holding W when quitting (on the made-up desktop: nothing is sent).
        var (player, desk, accounts) = MacroSuite.Setup();
        desk.SuspendWhen = () => desk.Log.Any(l => l.Contains("key W down"));
        player.Queue(MacroSuite.M("""[{"kind":"down","key":"W"},{"kind":"wait","ms":20000},{"kind":"up","key":"W"}]"""), accounts);
        Harness.Set(app, "_macroPlayer", player);

        var clock = Stopwatch.StartNew();
        Exception? thrown = null;
        try
        {
            app.Dispose();
        }
        catch (Exception ex)
        {
            thrown = ex;
        }
        clock.Stop();
        c.Check("a failing step doesn't stop quitting", thrown is null, thrown?.GetType().Name);
        c.Check("…quitting takes a few seconds at most", clock.Elapsed < TimeSpan.FromSeconds(4), clock.Elapsed);

        string Alias()
        {
            using var opened = Vault.Open(Harness.Password);
            return opened.Data.Accounts.FirstOrDefault(a => a.Username == "quit_user")?.Alias ?? "(not on disk)";
        }
        c.Check("…the last change is in the vault on disk", Alias() == "changed before quitting", Alias());
        c.Check("…the vault is closed", Harness.Get<Vault?>(app, "_vault") is null);
        c.Check("a Light AFK file that couldn't be put back is kept for the next start", RobloxFlags.Pending);
        c.Check("a macro holding a key when quitting lets it go, its run stopped", desk.Log.Any(l => l.Contains("key W up"))
            && player.Runs[0] is { Status: "stopped", Reason: "closing" }, string.Join(" | ", desk.Log));
        InputTurn.Reset();

        var again = Stopwatch.StartNew();
        thrown = null;
        try
        {
            app.Dispose();
        }
        catch (Exception ex)
        {
            thrown = ex;
        }
        c.Check("quitting a second time does nothing", thrown is null && again.Elapsed < TimeSpan.FromSeconds(1), thrown?.GetType().Name);

        File.SetAttributes(flags, FileAttributes.Normal);
        RobloxFlags.Restore();
        c.Check("…and the next start puts the person's file back", File.ReadAllText(flags) == own && !RobloxFlags.Pending);
    }
}
