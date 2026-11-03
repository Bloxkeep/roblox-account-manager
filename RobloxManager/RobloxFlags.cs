using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;

namespace RobloxManager;

/// <summary>
/// With Settings › Staying in game › Fast Flags on, windows get Roblox's leanest drawing: Fast Flags written to the
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
    /// Before opening a window: the flags in the file of every build next to <paramref name="client"/> (Roblox
    /// starts the account's channel's build, which may be another folder than the newest). False when nothing was written.
    /// </summary>
    public static bool Apply(string? client)
    {
        if (client is null || Path.GetDirectoryName(Path.GetDirectoryName(client)) is not { } versions)
            return false;
        // The note can't be read just now: nothing written, so the person's own in it is never replaced.
        if (!TryLoad(out var loaded))
            return false;
        var state = loaded ?? new State();
        var planned = new List<(string File, string Text, bool Ours, string? WrittenBefore)>();
        foreach (string build in Builds(versions))
        {
            string file = Path.Combine(build, "ClientSettings", "ClientAppSettings.json");
            string? had = Read(file);
            if (had is null && File.Exists(file))
                continue; // there but can't be read just now: never written over without knowing what it held
            // Still ours from an earlier window: what it held before that is what goes back.
            bool ours = state.Written.TryGetValue(file, out var written) && written == had && state.Before.ContainsKey(file);
            // The flags already in it with no note of what it held before (the note lost or damaged): that isn't the
            // person's own, so it is never noted as theirs (it would be "put back" with the flags for good).
            if (!ours && had is not null && HoldsLightAfk(had))
                continue;
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
            state.Written[file] = text;
            planned.Add((file, text, ours, written));
        }
        // The note first: a file is never changed without a note on disk of what it held (one written without it could
        // never be put back).
        if (planned.Count == 0 || !Save(state))
            return false;
        bool any = false, undone = false;
        foreach (var (file, text, ours, writtenBefore) in planned)
        {
            if (Write(file, text))
            {
                any = true;
                continue;
            }
            undone = true;
            if (ours)
                state.Written[file] = writtenBefore!; // still holds what was written for the earlier window
            else
            {
                state.Before.Remove(file);
                state.Written.Remove(file);
            }
        }
        if (undone)
            Keep(state);
        return any;
    }

    /// <summary>
    /// Every file written put back as it was: a window not of a light-AFK preset is about to open, or those that were
    /// have started. One changed since (Roblox updated, the person edited it) is left as it is now. One that can't be put
    /// back just now (held, read-only, a build without the rights) stays in the note, tried again next time and at the
    /// next start: the person's own is never forgotten.
    /// </summary>
    public static void Restore()
    {
        if (!TryLoad(out var state) || state is null)
            return;
        foreach (var (file, before) in state.Before.ToList())
        {
            string? now = Read(file);
            if (now is null && File.Exists(file))
                continue; // can't be read just now: kept for the next try
            bool back = true;
            if (state.Written.TryGetValue(file, out var written) && now == written)
            {
                if (before is null)
                {
                    Delete(file);
                    back = !File.Exists(file);
                    // The folder too, when Bloxkeep made it and it's empty again.
                    try
                    {
                        if (back && Path.GetDirectoryName(file) is { } dir && Directory.Exists(dir) && !Directory.EnumerateFileSystemEntries(dir).Any())
                            Directory.Delete(dir);
                    }
                    catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
                    {
                    }
                }
                else
                    back = Write(file, before);
            }
            if (back)
            {
                state.Before.Remove(file);
                state.Written.Remove(file);
            }
        }
        Keep(state);
    }

    /// <summary>The note as it now stands: gone once nothing is left to put back.</summary>
    static void Keep(State state)
    {
        if (state.Before.Count == 0)
            Delete(StatePath);
        else
            Save(state);
    }

    /// <summary>Every Light AFK flag in it, at its value.</summary>
    static bool HoldsLightAfk(string text)
    {
        try
        {
            return JsonNode.Parse(text) is JsonObject flags
                && LightAfk.All(f => flags[f.Name] is { } value && value.ToJsonString() == f.Value.ToJsonString());
        }
        catch (JsonException)
        {
            return false;
        }
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

    /// <summary>The note on disk (null: none). False when it is there but can't be read just now (held for a moment): the
    /// caller then changes nothing. A damaged one is set aside as roblox-flags.json.damaged-&lt;time&gt;, never written over,
    /// and taken as none.</summary>
    static bool TryLoad(out State? state)
    {
        state = null;
        string text;
        try
        {
            if (!File.Exists(StatePath))
                return true;
            text = File.ReadAllText(StatePath);
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            return false;
        }
        try
        {
            state = JsonSerializer.Deserialize<State>(text);
        }
        catch (JsonException)
        {
        }
        if (state is { Before: not null, Written: not null })
            return true;
        state = null;
        try
        {
            File.Move(StatePath, $"{StatePath}.damaged-{DateTime.Now:yyyyMMdd-HHmmss}");
            Log.Note("roblox-flags.json couldn't be read; it was set aside beside it, and files holding Light AFK's flags are left as they are.");
            return true;
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            return false; // not set aside: never written over
        }
    }

    /// <returns>false when it couldn't be written.</returns>
    static bool Save(State state)
    {
        try
        {
            Directory.CreateDirectory(Vault.DataDir);
            // In one piece or not at all: a half-written copy of the person's own file would never be put back.
            DurableFile.WriteText(StatePath, JsonSerializer.Serialize(state));
            return true;
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            return false;
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
