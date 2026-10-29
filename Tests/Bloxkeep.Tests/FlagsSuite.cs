using System.Text.Json.Nodes;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// Light AFK's Fast Flags (RobloxFlags), on a stand-in Roblox folder (never the real one): written into every build's
/// ClientAppSettings.json, a file of the person's own keeping its other flags; put back as it was, one Bloxkeep made
/// removed with its folder; a second window's writing doesn't lose the person's original; a file changed since, or one
/// that isn't JSON, is left alone; a Bloxkeep that closed before putting them back does it at its next start.
/// </summary>
static class FlagsSuite
{
    public static Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        string versions = Path.Combine(dir, "Roblox", "Versions");
        string a = Path.Combine(versions, "version-aaaa"), b = Path.Combine(versions, "version-bbbb"), old = Path.Combine(versions, "version-gone");
        foreach (var build in new[] { a, b })
        {
            Directory.CreateDirectory(build);
            File.WriteAllText(Path.Combine(build, "RobloxPlayerBeta.exe"), "");
        }
        Directory.CreateDirectory(old); // a build folder with no client in it: not touched
        string mine = Path.Combine(a, "ClientSettings", "ClientAppSettings.json"), made = Path.Combine(b, "ClientSettings", "ClientAppSettings.json");
        Directory.CreateDirectory(Path.GetDirectoryName(mine)!);
        const string own = """{"FFlagMyOwn": true, "FIntDebugForceMSAASamples": 4}""";
        File.WriteAllText(mine, own);
        JsonObject Flags(string file) => (JsonObject)JsonNode.Parse(File.ReadAllText(file))!;

        c.Check("written into every build next to the client", RobloxFlags.Apply(Path.Combine(b, "RobloxPlayerBeta.exe"))
            && File.Exists(made) && Flags(made)["FFlagDebugSkyGray"]?.GetValue<bool>() == true && Flags(made)["DFIntDebugFRMQualityLevelOverride"]?.GetValue<int>() == 1
            && !Directory.Exists(Path.Combine(old, "ClientSettings")));
        c.Check("…the person's own flags kept, the ones Light AFK sets in its values", Flags(mine)["FFlagMyOwn"]?.GetValue<bool>() == true
            && Flags(mine)["FIntDebugForceMSAASamples"]?.GetValue<int>() == 0 && RobloxFlags.Pending);
        bool again = RobloxFlags.Apply(Path.Combine(a, "RobloxPlayerBeta.exe"));
        c.Check("a second light-AFK window: written again", again && Flags(mine)["DFFlagDebugPauseVoxelizer"]?.ToJsonString() == "true", $"{again} {File.ReadAllText(mine)}");

        RobloxFlags.Restore();
        c.Check("put back: the person's file exactly as it was", File.ReadAllText(mine) == own);
        c.Check("…the one Bloxkeep made removed, with its folder", !File.Exists(made) && !Directory.Exists(Path.GetDirectoryName(made)) && !RobloxFlags.Pending);

        RobloxFlags.Apply(Path.Combine(a, "RobloxPlayerBeta.exe"));
        const string edited = """{"FFlagEditedSince": true}""";
        File.WriteAllText(mine, edited);
        RobloxFlags.Restore();
        c.Check("a file changed since it was written is left as it is now", File.ReadAllText(mine) == edited && !File.Exists(made));

        File.WriteAllText(mine, "not json");
        RobloxFlags.Apply(Path.Combine(a, "RobloxPlayerBeta.exe"));
        c.Check("a file of the person's that isn't JSON is left alone", File.ReadAllText(mine) == "not json" && File.Exists(made));
        RobloxFlags.Restore();
        c.Check("…and the other build's put back", !File.Exists(made));

        // Closed before putting them back: the next start does it (AppController sees it pending from the start).
        File.WriteAllText(mine, own);
        RobloxFlags.Apply(Path.Combine(a, "RobloxPlayerBeta.exe"));
        c.Check("still pending after a restart (kept on disk)", RobloxFlags.Pending);
        RobloxFlags.Restore();
        c.Check("…and put back then", File.ReadAllText(mine) == own && !RobloxFlags.Pending);

        c.Check("no client: nothing written", !RobloxFlags.Apply(null));
        return Task.CompletedTask;
    }
}
