using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;

namespace RobloxManager;

/// <summary>
/// A light-AFK preset's windows get Roblox's leanest drawing on top of its lowest graphics: Fast Flags written to the
/// client's ClientSettings\ClientAppSettings.json just before it opens, and the file put back as it was once it has
/// started (Roblox reads it in its first second: "LoadClientSettingsFromLocal" in its log). Only flags on Roblox's
/// allowlist (devforum 3966569, 2025-09-29), each still in the client of 2026-10-29; Roblox ignores any other, and
/// takes no action against setting them. On the owner's PC they turned the sky grey and the light flat, and saved a
/// little RAM (about 70 MB of 3.3 GB, Steal An Egg): every bit helps a PC full of idling accounts. A file of the
/// person's own keeps its other flags. A Bloxstrap-style launcher writes its own file as it opens Roblox: theirs wins.
/// </summary>
public static class RobloxFlags
{
    /// <summary>The flags, as Roblox takes them in the file.</summary>
    static readonly (string Name, JsonNode Value)[] LightAfk =
    [
        ("DFIntDebugFRMQualityLevelOverride", 1),       // the lowest graphics level, whatever the game or Roblox's auto
        ("DFFlagDebugPauseVoxelizer", true),            // no voxel lighting kept up to date
        ("DFIntCSGLevelOfDetailSwitchingDistance", 0),  // unions at their least detail…
        ("DFIntCSGLevelOfDetailSwitchingDistanceL12", 0),
        ("DFIntCSGLevelOfDetailSwitchingDistanceL23", 0),
        ("DFIntCSGLevelOfDetailSwitchingDistanceL34", 0),
        ("FIntDebugForceMSAASamples", 0),               // no anti-aliasing
        ("FIntFRMMaxGrassDistance", 0),                 // no grass
        ("FIntFRMMinGrassDistance", 0),
        ("FFlagDebugSkyGray", true),                    // a grey sky, its pictures not loaded
    ];

    /// <summary>The files written, each with what it held before (null: there was none), kept on disk so a Bloxkeep that
    /// closed before putting them back does it at its next start.</summary>
    static string StatePath => Path.Combine(Vault.DataDir, "roblox-flags.json");

    sealed class State
    {
        public Dictionary<string, string?> Before { get; set; } = new(StringComparer.OrdinalIgnoreCase);
        public Dictionary<string, string> Written { get; set; } = new(StringComparer.OrdinalIgnoreCase);
    }

    public static bool Pending => File.Exists(StatePath);

    /// <summary>
    /// Before opening a light-AFK window: the flags in the file of every build next to <paramref name="client"/> (Roblox
    /// starts the account's channel's build, which may be another folder than the newest). False when nothing was written.
    /// </summary>
    public static bool Apply(string? client)
    {
        if (client is null || Path.GetDirectoryName(Path.GetDirectoryName(client)) is not { } versions)
            return false;
        var state = Load() ?? new State();
        bool any = false;
        foreach (string build in Builds(versions))
        {
            string file = Path.Combine(build, "ClientSettings", "ClientAppSettings.json");
            string? had = Read(file);
            // Still ours from an earlier window: what it held before that is what goes back.
            bool ours = state.Written.TryGetValue(file, out var written) && written == had;
            JsonObject flags;
            try
            {
                flags = (ours ? state.Before[file] : had) is { Length: > 0 } before ? JsonNode.Parse(before) as JsonObject ?? new JsonObject() : new JsonObject();
            }
            catch (JsonException)
            {
                continue; // a file of the person's that isn't JSON: left alone (Roblox wouldn't read it anyway)
            }
            foreach (var (name, value) in LightAfk)
                flags[name] = value.DeepClone();
            string text = flags.ToJsonString(new JsonSerializerOptions { WriteIndented = true });
            if (!ours)
                state.Before[file] = had;
            if (Write(file, text))
            {
                state.Written[file] = text;
                any = true;
            }
            else if (!ours)
                state.Before.Remove(file);
        }
        if (any)
            Save(state);
        return any;
    }

    /// <summary>
    /// Every file written put back as it was: a window not of a light-AFK preset is about to open, or those that were
    /// have started. One changed since (Roblox updated, the person edited it) is left as it is now.
    /// </summary>
    public static void Restore()
    {
        if (Load() is not { } state)
            return;
        foreach (var (file, before) in state.Before)
        {
            if (!state.Written.TryGetValue(file, out var written) || Read(file) != written)
                continue;
            if (before is null)
            {
                Delete(file);
                // The folder too, when Bloxkeep made it and it's empty again.
                try
                {
                    if (Path.GetDirectoryName(file) is { } dir && Directory.Exists(dir) && !Directory.EnumerateFileSystemEntries(dir).Any())
                        Directory.Delete(dir);
                }
                catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
                {
                }
            }
            else
                Write(file, before);
        }
        Delete(StatePath);
    }

    static IEnumerable<string> Builds(string versions)
    {
        try
        {
            return Directory.EnumerateDirectories(versions, "version-*").Where(d => File.Exists(Path.Combine(d, "RobloxPlayerBeta.exe"))).ToList();
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            return [];
        }
    }

    static State? Load()
    {
        try
        {
            return File.Exists(StatePath) ? JsonSerializer.Deserialize<State>(File.ReadAllText(StatePath)) : null;
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or JsonException)
        {
            return null;
        }
    }

    static void Save(State state)
    {
        try
        {
            Directory.CreateDirectory(Vault.DataDir);
            // In one piece or not at all: a half-written copy of the person's own file would never be put back.
            DurableFile.WriteText(StatePath, JsonSerializer.Serialize(state));
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
        }
    }

    static string? Read(string file)
    {
        try
        {
            return File.Exists(file) ? File.ReadAllText(file, new UTF8Encoding(false)) : null;
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            return null;
        }
    }

    /// <summary>Written next to the file and swapped in, so Roblox never reads half a file. False when it couldn't be
    /// (a build in Program Files, say, without the rights).</summary>
    static bool Write(string file, string text)
    {
        string temp = file + ".bloxkeep";
        try
        {
            Directory.CreateDirectory(Path.GetDirectoryName(file)!);
            File.WriteAllText(temp, text, new UTF8Encoding(false));
            File.Move(temp, file, overwrite: true);
            return true;
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            Delete(temp);
            return false;
        }
    }

    static void Delete(string file)
    {
        try
        {
            File.Delete(file);
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
        }
    }
}
