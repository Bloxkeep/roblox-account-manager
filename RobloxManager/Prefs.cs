using System.Text.Json;

namespace RobloxManager;

/// <summary>
/// Settings needed before the vault is unlocked (language, theme, safe rendering), kept in prefs.json.
/// Nothing in here is secret.
/// </summary>
public static class Prefs
{
    static string PrefsPath => Path.Combine(Vault.DataDir, "prefs.json");

    public static string Language { get; set; } = "en";

    /// <summary>A theme id from <see cref="Themes"/>.</summary>
    public static string Theme { get; set; } = Themes.Default;

    /// <summary>The close button hides the window to the notification area; the app quits from the tray icon.</summary>
    public static bool CloseToTray { get; set; } = true;

    /// <summary>The "still running in the tray" notice was shown once.</summary>
    public static bool TrayHintShown { get; set; }

    /// <summary>Draw the interface on the CPU instead of the graphics card.</summary>
    public static bool SafeRendering { get; set; }

    /// <summary>Look for new versions by itself (a manual check is always possible).</summary>
    public static bool CheckUpdates { get; set; } = true;

    /// <summary>The version that last ran here; a newer one means the app was updated (see <see cref="WhatsNew"/>).</summary>
    public static string? LastVersion { get; set; }

    /// <summary>Important events reach the person as Windows notifications while the window is hidden or in the background.</summary>
    public static bool Notifications { get; set; } = true;

    /// <summary>Where copies of the vault go (see <see cref="Backup"/>); null until chosen. Kept here so restoring works before unlocking.</summary>
    public static string? BackupFolder { get; set; }

    public static bool BackupEnabled { get; set; } = true;

    public static DateTime? LastBackup { get; set; }

    public static void Load()
    {
        try
        {
            using var doc = JsonDocument.Parse(File.ReadAllText(PrefsPath));
            var root = doc.RootElement;
            if (root.TryGetProperty("language", out var lang) && lang.GetString() == "vi")
                Language = "vi";
            if (root.TryGetProperty("theme", out var theme) && theme.ValueKind == JsonValueKind.String)
                Theme = Themes.Normalize(theme.GetString());
            if (root.TryGetProperty("closeToTray", out var tray) && tray.ValueKind == JsonValueKind.False)
                CloseToTray = false;
            if (root.TryGetProperty("trayHintShown", out var hint) && hint.ValueKind == JsonValueKind.True)
                TrayHintShown = true;
            if (root.TryGetProperty("safeRendering", out var safe) && safe.ValueKind == JsonValueKind.True)
                SafeRendering = true;
            if (root.TryGetProperty("checkUpdates", out var updates) && updates.ValueKind == JsonValueKind.False)
                CheckUpdates = false;
            if (root.TryGetProperty("lastVersion", out var last) && last.ValueKind == JsonValueKind.String)
                LastVersion = last.GetString();
            if (root.TryGetProperty("notifications", out var notify) && notify.ValueKind == JsonValueKind.False)
                Notifications = false;
            if (root.TryGetProperty("backupFolder", out var folder) && folder.ValueKind == JsonValueKind.String)
                BackupFolder = folder.GetString() is { Length: > 0 } f ? f : null;
            if (root.TryGetProperty("backupEnabled", out var backup) && backup.ValueKind == JsonValueKind.False)
                BackupEnabled = false;
            if (root.TryGetProperty("lastBackup", out var lastBackup) && lastBackup.ValueKind == JsonValueKind.String && lastBackup.TryGetDateTime(out var at))
                LastBackup = at.ToUniversalTime();
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
            File.WriteAllText(PrefsPath, JsonSerializer.Serialize(new
            {
                language = Language, theme = Theme, closeToTray = CloseToTray, trayHintShown = TrayHintShown,
                safeRendering = SafeRendering, checkUpdates = CheckUpdates, lastVersion = LastVersion,
                notifications = Notifications, backupFolder = BackupFolder, backupEnabled = BackupEnabled, lastBackup = LastBackup,
            }));
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            // The choice still applies for this session.
        }
    }
}
