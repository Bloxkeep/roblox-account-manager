using System.Text.Json;

namespace RobloxManager;

/// <summary>
/// Settings needed before the vault is unlocked (language, safe rendering), kept in prefs.json.
/// Nothing in here is secret.
/// </summary>
public static class Prefs
{
    static string PrefsPath => Path.Combine(Vault.DataDir, "prefs.json");

    public static string Language { get; set; } = "en";

    /// <summary>Draw the interface on the CPU instead of the graphics card.</summary>
    public static bool SafeRendering { get; set; }

    /// <summary>Look for new versions by itself (a manual check is always possible).</summary>
    public static bool CheckUpdates { get; set; } = true;

    /// <summary>The version that last ran here; a newer one means the app was updated (see <see cref="WhatsNew"/>).</summary>
    public static string? LastVersion { get; set; }

    public static void Load()
    {
        try
        {
            using var doc = JsonDocument.Parse(File.ReadAllText(PrefsPath));
            var root = doc.RootElement;
            if (root.TryGetProperty("language", out var lang) && lang.GetString() == "vi")
                Language = "vi";
            if (root.TryGetProperty("safeRendering", out var safe) && safe.ValueKind == JsonValueKind.True)
                SafeRendering = true;
            if (root.TryGetProperty("checkUpdates", out var updates) && updates.ValueKind == JsonValueKind.False)
                CheckUpdates = false;
            if (root.TryGetProperty("lastVersion", out var last) && last.ValueKind == JsonValueKind.String)
                LastVersion = last.GetString();
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or JsonException or InvalidOperationException)
        {
            // Missing or unreadable prefs: defaults.
        }
    }

    public static void Save()
    {
        try
        {
            Directory.CreateDirectory(Vault.DataDir);
            File.WriteAllText(PrefsPath, JsonSerializer.Serialize(new { language = Language, safeRendering = SafeRendering, checkUpdates = CheckUpdates, lastVersion = LastVersion }));
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            // The choice still applies for this session.
        }
    }
}
