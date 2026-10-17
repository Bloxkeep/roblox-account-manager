using System.Reflection;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// Separate Roblox sign-in windows (RobloxHomeSession), the dangerous parts tested without starting Roblox: a start
/// gets its own junction path (so the client's path-keyed single-window guard doesn't merge it), the two shared
/// sign-in files are locked while the session lives and free after, and — most important — cleaning up the junctions
/// deletes only the junction, never the real Roblox install it points at.
/// </summary>
static class HomeSessionSuite
{
    static readonly Type T = typeof(Prefs).Assembly.GetType("RobloxManager.RobloxHomeSession")!;

    public static Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        // A stand-in Roblox install with a file that must survive cleanup.
        string install = Path.Combine(dir, "install");
        Directory.CreateDirectory(install);
        string exe = Path.Combine(install, "RobloxPlayerBeta.exe");
        File.WriteAllText(exe, "stub");
        string keep = Path.Combine(install, "do-not-delete.dat");
        File.WriteAllText(keep, "the real Roblox install");
        string storage = Path.Combine(dir, "LocalStorage");
        string aliases = Path.Combine(dir, "aliases");

        var session = Activator.CreateInstance(T, storage, aliases)!;
        try
        {
            c.Check("both shared sign-in files are held while a session lives", Locked(Path.Combine(storage, "RobloxCookies.dat")) && Locked(Path.Combine(storage, "appStorage.json")));

            var info = (System.Diagnostics.ProcessStartInfo)T.GetMethod("CreateStartInfo", Harness.F)!.Invoke(session, [exe])!;
            string started = info.FileName;
            c.Check("a start runs the client through a junction of its own, not the install path",
                Path.GetDirectoryName(started)!.StartsWith(aliases, StringComparison.OrdinalIgnoreCase)
                && !Path.GetDirectoryName(started)!.Equals(install, StringComparison.OrdinalIgnoreCase)
                && File.Exists(started), started);
            c.Check("…and the junction points at the real install (the client is reachable through it)",
                new FileInfo(started).Length == new FileInfo(exe).Length);

            // A second start reuses the install through its own separate junction.
            var info2 = (System.Diagnostics.ProcessStartInfo)T.GetMethod("CreateStartInfo", Harness.F)!.Invoke(session, [exe])!;
            c.Check("each start gets a different junction", Path.GetDirectoryName(info2.FileName) != Path.GetDirectoryName(started));
        }
        finally
        {
            ((IDisposable)session).Dispose();
        }

        c.Check("the files are free again once the session is disposed", !Locked(Path.Combine(storage, "RobloxCookies.dat")) && !Locked(Path.Combine(storage, "appStorage.json")));

        int before = Directory.GetDirectories(aliases).Length;
        c.Check("the junctions exist before cleanup", before >= 2, before);
        T.GetMethod("CleanupAliases", Harness.F)!.Invoke(null, [aliases]);
        c.Check("cleanup removes the junctions", Directory.GetDirectories(aliases).Length == 0, Directory.GetDirectories(aliases).Length);
        c.Check("CRITICAL: cleanup never touches the real install — its files survive", File.Exists(exe) && File.Exists(keep) && File.ReadAllText(keep) == "the real Roblox install");

        // A directory that isn't a junction (name-shaped like one) is left alone: cleanup only removes reparse points.
        string decoy = Path.Combine(aliases, "client-" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(decoy);
        File.WriteAllText(Path.Combine(decoy, "real.txt"), "not a junction");
        T.GetMethod("CleanupAliases", Harness.F)!.Invoke(null, [aliases]);
        c.Check("a real directory (not a junction) is not deleted by cleanup", File.Exists(Path.Combine(decoy, "real.txt")));
        return Task.CompletedTask;
    }

    /// <summary>Whether a file can't be opened at all right now (the session holds it with FileShare.None).</summary>
    static bool Locked(string path)
    {
        try
        {
            using var _ = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite);
            return false;
        }
        catch (IOException)
        {
            return true;
        }
    }
}
