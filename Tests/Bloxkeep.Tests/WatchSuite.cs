using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// What Bloxkeep takes from the Roblox watch's status file (RobloxWatch): verdicts and lock names, with anything out of
/// shape left out; the names it holds besides its own; and Multi-Roblox holding several names, made up for the test
/// (its own name is only opened, which an open Roblox or Bloxkeep doesn't notice).
/// </summary>
static class WatchSuite
{
    public static async Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        await LaunchWarning(c);
        // Roblox from the Microsoft Store: its exe has no version, its package folder does (seen 2026-10-16).
        c.Check("the Store client's version, in Roblox's form",
            RobloxInstalls.StoreVersion("ROBLOXCorporation.RobloxGDK_2.743.1134.0_x64__55nm5eh3cm0pr") == "0.743.0.7431134"
            && RobloxInstalls.StoreVersion("ROBLOXCorporation.RobloxGDK_2.744.84.0_x64__55nm5eh3cm0pr") == "0.744.0.7440084"
            && RobloxInstalls.StoreVersion("RobloxPlayerBeta") is null);
        string tooLong = "ROBLOX_" + new string('x', 70);
        string json = $$"""
            {
              "schema": 1,
              "live": { "version": "0.743.0.7431129", "hash": "version-aaaa" },
              "beta": { "version": "0.743.0.7431134", "hash": "version-bbbb" },
              "builds": [
                { "version": "0.742.0.7421053", "hash": "version-old", "verdict": "first", "locks": ["ROBLOX_singletonEvent"] },
                { "version": "0.743.0.7431129", "hash": "version-aaaa", "verdict": "renamed",
                  "hold": ["ROBLOX_singletonEvent2", "not a name", "Global\\Evil", "{{tooLong}}", "ROBLOX_a", "ROBLOX_b", "ROBLOX_c", "ROBLOX_d"] },
                { "version": "0.743.0.7431134", "hash": "version-bbbb", "verdict": "same", "hold": ["ROBLOX_singletonEvent"] },
                { "version": "0.744.0.1", "hash": "version-bad", "verdict": "exploded" },
                "junk",
                { "hash": "version-noversion", "verdict": "same" }
              ]
            }
            """;
        var s = RobloxWatch.Parse(json);
        c.Check("the status file is read", s is not null);
        if (s is null)
            return;
        c.Check("builds out of shape are left out", s.ByHash.Keys.Order().SequenceEqual(["version-aaaa", "version-bbbb", "version-old"]), string.Join(",", s.ByHash.Keys));
        c.Check("only lock-shaped names are taken, at most 4",
            s.ByHash["version-aaaa"].Hold!.SequenceEqual(["ROBLOX_singletonEvent2", "ROBLOX_a", "ROBLOX_b", "ROBLOX_c"]), string.Join(",", s.ByHash["version-aaaa"].Hold!));
        c.Check("no hold list: none taken", s.ByHash["version-old"].Hold is null);
        c.Check("live and next builds found", s.Live?.Hash == "version-aaaa" && s.Next?.Hash == "version-bbbb");
        c.Check("a beta build that isn't newer isn't the next one",
            RobloxWatch.Parse(json.Replace("\"0.743.0.7431134\", \"hash\": \"version-bbbb\", \"verdict\"", "\"0.743.0.7431100\", \"hash\": \"version-bbbb\", \"verdict\""))?.Next is null);
        c.Check("not a status file: nothing", RobloxWatch.Parse("<html>404</html>") is null && RobloxWatch.Parse("{}") is null && RobloxWatch.Parse("[]") is null);

        var current = typeof(RobloxWatch).GetProperty(nameof(RobloxWatch.Current))!;
        current.SetValue(null, s);
        var own = MultiInstance.EventName;
        c.Check("names to hold: the live build's, its own left out",
            RobloxWatch.ExtraNames("version-bbbb", own).SequenceEqual(["ROBLOX_singletonEvent2", "ROBLOX_a", "ROBLOX_b", "ROBLOX_c"]), string.Join(",", RobloxWatch.ExtraNames("version-bbbb", own)));
        c.Check("a build the watch knows: its verdict", RobloxWatch.For("VERSION-AAAA")?.Verdict == "renamed" && RobloxWatch.For("version-zzzz") is null);
        var plain = new RobloxWatch.Build("0.743.0.7431129", "version-aaaa", "same", [own]);
        current.SetValue(null, new RobloxWatch.Status(plain, null, new Dictionary<string, RobloxWatch.Build> { [plain.Hash] = plain }));
        c.Check("no renamed lock anywhere: nothing more to hold", RobloxWatch.ExtraNames("version-old", own).Length == 0,
            string.Join(",", RobloxWatch.ExtraNames("version-old", own)));
        current.SetValue(null, null);

        // Holding several names: made-up ones next to Multi-Roblox's own.
        string a = $"ROBLOX_bloxkeepTestA_{Environment.ProcessId}", b = $"ROBLOX_bloxkeepTestB_{Environment.ProcessId}";
        bool Exists(string name)
        {
            if (!Mutex.TryOpenExisting(name, out var m))
                return false;
            m.Dispose();
            return true;
        }
        MultiInstance.SetExtraNames([a, b, own]);
        MultiInstance.Enable();
        if (!MultiInstance.Armed)
        {
            // A Roblox client is open here: its own name is its event until it closes, so nothing can be held.
            c.Check("several names held (skipped: a Roblox client is open)", true);
            MultiInstance.Disable();
            return;
        }
        c.Check("every name held", Exists(a) && Exists(b) && MultiInstance.ExtraHeld.SequenceEqual([a, b]), string.Join(",", MultiInstance.ExtraHeld));
        c.Check("same names: not stale", !MultiInstance.Stale);
        MultiInstance.SetExtraNames([a]);
        c.Check("other names wanted: stale until taken again", MultiInstance.Stale);
        MultiInstance.Enable();
        c.Check("taken again: only the wanted ones", !MultiInstance.Stale && Exists(a) && !Exists(b));
        MultiInstance.Disable();
        c.Check("off: every name let go", !MultiInstance.Armed && !Exists(a) && MultiInstance.ExtraHeld.Count == 0);
        MultiInstance.SetExtraNames([]);
    }

    /// <summary>
    /// The warning before opening several windows, for the Roblox build installed here (skipped without one): said for
    /// a changed, renamed or gone lock, not for no change, and not once two windows played together on that build.
    /// </summary>
    static async Task LaunchWarning(Checks c)
    {
        var seed = Vault.Create(Harness.Password);
        seed.Save();
        using var form = new Form();
        using var app = new AppController(form, _ => { });
        await Harness.Unlock(app);
        var installs = Harness.Invoke(app, "Installs")!;
        var client = (RobloxInstalls.ClientVersion?)installs.GetType().GetProperty("Client")!.GetValue(installs);
        if (client is not { Folder.Length: > 0 } installed)
        {
            c.Check("launch warning (skipped: no Roblox installed here)", true);
            return;
        }
        var current = typeof(RobloxWatch).GetProperty(nameof(RobloxWatch.Current))!;
        string? Warning(string verdict)
        {
            var build = new RobloxWatch.Build(installed.Version, installed.Folder, verdict, null);
            current.SetValue(null, new RobloxWatch.Status(null, null, new Dictionary<string, RobloxWatch.Build>(StringComparer.OrdinalIgnoreCase) { [build.Hash] = build }));
            return (string?)Harness.Invoke(app, "WatchWarning");
        }
        c.Check("launch warning: changed", Warning("changed")?.Contains(installed.Version) == true, Warning("changed"));
        c.Check("launch warning: renamed and gone too", Warning("renamed") is not null && Warning("gone") is not null);
        c.Check("no warning for no change", Warning("same") is null && Warning("first") is null);
        Prefs.MultiWorked.Add(installed.Folder);
        c.Check("no warning once two windows played together on that build", Warning("changed") is null);
        Prefs.MultiWorked.Clear();
        current.SetValue(null, null);
        c.Check("no warning without the watch's word", (string?)Harness.Invoke(app, "WatchWarning") is null);
    }

    /// <summary>The real status file, from the repository's roblox-status branch.</summary>
    public static async Task RunOnline(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        bool changed = await RobloxWatch.RefreshAsync();
        var s = RobloxWatch.Current;
        c.Check("the status file is read from GitHub", changed && s is not null && RobloxWatch.ReadAt is not null);
        c.Check("it knows the live build", s?.Live is { } live && live.Hash.StartsWith("version-") && s.ByHash.ContainsKey(live.Hash), s?.Live);
        c.Check("read again: nothing changed", !await RobloxWatch.RefreshAsync());
    }
}
