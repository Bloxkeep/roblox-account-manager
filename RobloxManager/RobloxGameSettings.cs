using System.Globalization;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;
using System.Text.RegularExpressions;

namespace RobloxManager;

/// <summary>
/// Roblox keeps its in-game settings (graphics, FPS limit, volume…) in one file shared by every account:
/// %LOCALAPPDATA%\Roblox\GlobalBasicSettings_*.xml, read as a client starts. Writing values just before opening an
/// account gives it its own setup; only those values change (the rest stays byte for byte), and the person's own are
/// put back once that client has started.
/// </summary>
public static class RobloxGameSettings
{
    static readonly string RobloxDir = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Roblox");
    static string BaselinePath => Path.Combine(Vault.DataDir, "roblox-settings.json");

    /// <summary>The values Bloxkeep manages, as (element type, name) in the file.</summary>
    static readonly (string Type, string Name)[] Keys =
    [
        ("token", "SavedQualityLevel"), ("int", "GraphicsQualityLevel"), ("int", "FramerateCap"), ("float", "MasterVolume"),
        ("bool", "Fullscreen"),
    ];

    static string? _settingsFile;
    static DateTime _nextFileLookup;
    static readonly TimeSpan FileLookupInterval = TimeSpan.FromSeconds(10);

    /// <param name="cached">The file found last time is fine while it still exists; the folder is listed again every 10 s at most.</param>
    static string? SettingsFile(bool cached = false)
    {
        if (cached && _settingsFile is not null && DateTime.UtcNow < _nextFileLookup && File.Exists(_settingsFile))
            return _settingsFile;
        _nextFileLookup = DateTime.UtcNow + FileLookupInterval;
        try
        {
            return _settingsFile = new DirectoryInfo(RobloxDir).Exists
                ? new DirectoryInfo(RobloxDir).EnumerateFiles("GlobalBasicSettings_*.xml")
                    .Where(f => !f.Name.Contains("Studio", StringComparison.OrdinalIgnoreCase))
                    .OrderByDescending(f => f.LastWriteTimeUtc).FirstOrDefault()?.FullName
                : null;
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            return _settingsFile = null;
        }
    }

    static Regex Element(string type, string name) =>
        new($"(<{type} name=\"{Regex.Escape(name)}\">)([^<]*)(</{type}>)", RegexOptions.CultureInvariant);

    /// <summary>The values the given setup asks for (null fields are left alone).</summary>
    static Dictionary<string, string> Wanted(RobloxConfig c)
    {
        var values = new Dictionary<string, string>();
        if (c.Graphics is >= 1 and <= 10)
        {
            values["SavedQualityLevel"] = c.Graphics.ToString(CultureInfo.InvariantCulture);
            values["GraphicsQualityLevel"] = EngineQuality(c.Graphics).ToString(CultureInfo.InvariantCulture);
        }
        if (c.Fps > 0)
            values["FramerateCap"] = c.Fps.ToString(CultureInfo.InvariantCulture);
        if (c.Volume is >= 0 and <= 100)
            values["MasterVolume"] = (c.Volume / 100.0).ToString("0.##", CultureInfo.InvariantCulture);
        return values;
    }

    /// <summary>The graphics slider (1–10) as Roblox's internal quality level (1–21) in the same file.</summary>
    public static int EngineQuality(int slider) => slider >= 10 ? 21 : slider * 2 - 1;

    /// <summary>A preset's light AFK profile: lowest graphics, the lowest FPS limit Roblox's menu offers (30; lower is
    /// untested) and no sound.</summary>
    public static RobloxConfig LightAfk => new() { Mode = "all", Graphics = 1, Fps = 30, Volume = 0 };

    /// <summary>
    /// Before opening an account: the shared setup if it uses it, the person's own values for everything else managed
    /// (so an account never inherits an earlier one's, such as a light-AFK preset's 30 FPS). <paramref name="windowed"/>:
    /// fullscreen off too, since every client reads the one switch and saves it as it joins a game. The person's values
    /// are what the file holds, except a value still equal to what Bloxkeep last wrote (as in <see cref="Restore"/>).
    /// False when there was nothing to write or the write failed.
    /// </summary>
    public static bool PrepareFor(RobloxConfig config, bool useConfig, bool windowed)
    {
        string? file = SettingsFile();
        if (file is null)
            return false;
        var baseline = LoadBaseline();
        if (!useConfig && !windowed && baseline is null)
            return false; // nothing of ours was ever written: leave the file alone
        string text = Read(file);
        if (text.Length == 0)
            return false;
        if (baseline is null)
        {
            baseline = new Baseline { Values = Current(text), Written = new() };
            SaveBaseline(baseline);
        }
        else
        {
            // What the file holds now is the person's own unless it is still what Bloxkeep wrote: changed in a Roblox
            // window since the copy, or missing from the copy (an older version didn't manage it).
            bool refreshed = false;
            foreach (var (name, value) in Current(text))
            {
                if (baseline.Written.TryGetValue(name, out var written) && written == value)
                    continue;
                if (!baseline.Values.TryGetValue(name, out var mine) || mine != value)
                {
                    baseline.Values[name] = value;
                    refreshed = true;
                }
            }
            if (refreshed)
                SaveBaseline(baseline);
        }
        // Bloxkeep's values for this client over the person's own, for everything it manages.
        var ours = useConfig ? Wanted(config) : new Dictionary<string, string>();
        if (windowed)
            ours["Fullscreen"] = "false";
        var target = new Dictionary<string, string>(baseline.Values);
        foreach (var (name, value) in ours)
            target[name] = value;
        if (target.Count == 0)
            return false;
        string updated = text;
        foreach (var (type, name) in Keys)
        {
            if (target.TryGetValue(name, out var value))
                updated = Element(type, name).Replace(updated, m => m.Groups[1].Value + value + m.Groups[3].Value, 1);
        }
        if (updated == text)
            return true;
        // Not written (the file held by another program): nothing of Bloxkeep's is in it, so nothing is noted as written.
        if (!Write(file, updated))
            return false;
        _lastSeen = default;
        if (ours.Count > 0)
        {
            foreach (var (name, value) in ours)
                baseline.Written[name] = value;
            SaveBaseline(baseline);
        }
        return true;
    }

    static (string? File, DateTime Time, long Length) _lastSeen;

    /// <summary>Puts the person's values back once a client has read Bloxkeep's, and again whenever one saves them (Roblox
    /// saves as it joins a game). A value no longer equal to what Bloxkeep wrote is the person's and is kept.
    /// <paramref name="final"/>: Roblox is closed, forget the copy.</summary>
    public static void Restore(bool final)
    {
        string? file = SettingsFile(cached: !final);
        // Nothing changed since the last look: skip reading anything (this runs every couple of seconds).
        if (!final && file is not null && new FileInfo(file) is { Exists: true } info && _lastSeen == (file, info.LastWriteTimeUtc, info.Length))
            return;
        var baseline = LoadBaseline();
        if (baseline is null)
            return;
        if (file is not null && Read(file) is { Length: > 0 } text)
        {
            var now = Current(text);
            string updated = text;
            foreach (var (type, name) in Keys)
            {
                if (baseline.Values.TryGetValue(name, out var mine) && baseline.Written.TryGetValue(name, out var ours)
                    && now.GetValueOrDefault(name) == ours)
                    updated = Element(type, name).Replace(updated, m => m.Groups[1].Value + mine + m.Groups[3].Value, 1);
            }
            if (updated != text)
                _ = Write(file, updated); // not written: tried again when the file changes, and at the final restore
            if (new FileInfo(file) is { Exists: true } written)
                _lastSeen = (file, written.LastWriteTimeUtc, written.Length);
        }
        if (!final)
            return;
        try
        {
            File.Delete(BaselinePath);
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
        }
    }

    public static bool HasBaseline => File.Exists(BaselinePath);

    static Dictionary<string, string> Current(string text) =>
        Keys.Select(k => (k.Name, Match: Element(k.Type, k.Name).Match(text)))
            .Where(x => x.Match.Success)
            .ToDictionary(x => x.Name, x => x.Match.Groups[2].Value);

    sealed class Baseline
    {
        public Dictionary<string, string> Values { get; set; } = new();
        public Dictionary<string, string> Written { get; set; } = new();
    }

    static Baseline? LoadBaseline()
    {
        try
        {
            return File.Exists(BaselinePath) ? JsonSerializer.Deserialize<Baseline>(File.ReadAllText(BaselinePath)) : null;
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or JsonException)
        {
            return null;
        }
    }

    static void SaveBaseline(Baseline baseline)
    {
        try
        {
            Directory.CreateDirectory(Vault.DataDir);
            // In one piece or not at all: a half-written copy of the person's own settings would never be put back.
            DurableFile.WriteText(BaselinePath, JsonSerializer.Serialize(baseline));
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
        }
    }

    static string Read(string file)
    {
        try
        {
            using var stream = new FileStream(file, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete);
            using var reader = new StreamReader(stream, new UTF8Encoding(false));
            return reader.ReadToEnd();
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            return "";
        }
    }

    /// <summary>Written next to the file and swapped in, so Roblox never reads half a file.</summary>
    /// <returns>false when it couldn't be written (logged).</returns>
    static bool Write(string file, string text)
    {
        string temp = file + ".bloxkeep";
        try
        {
            File.WriteAllText(temp, text, new UTF8Encoding(false));
            File.Move(temp, file, overwrite: true);
            return true;
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            Log.Write(ex);
            try { File.Delete(temp); } catch (Exception cleanup) when (cleanup is IOException or UnauthorizedAccessException) { }
            return false;
        }
    }
}

/// <summary>The Roblox in-game settings Bloxkeep gives accounts it opens. 0 / -1 = leave as it is.</summary>
public sealed class RobloxConfig
{
    /// <summary>off, all (every account), or marked (accounts with <see cref="Account.UseRobloxConfig"/>).</summary>
    public string Mode { get; set; } = "off";
    /// <summary>1–10 like Roblox's graphics slider; 0 = leave as it is.</summary>
    public int Graphics { get; set; }
    /// <summary>Frame rate limit (30, 60, 144, 240); 0 = leave as it is.</summary>
    public int Fps { get; set; }
    /// <summary>0–100 %; -1 = leave as it is.</summary>
    public int Volume { get; set; } = -1;
    /// <summary>Fields a newer Bloxkeep added, saved back as they were (see Models.cs).</summary>
    [JsonExtensionData]
    public Dictionary<string, JsonElement>? Extra { get; set; }
}
