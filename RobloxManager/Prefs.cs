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

    /// <summary>
    /// Settings › Appearance › Custom: the page's colour and its accent (#rrggbb), how much of that colour lies over the
    /// picture (0 to 90 %), and the picture's file in the data folder (theme-background-*.png…) when there is one.
    /// </summary>
    public sealed record CustomTheme(string Background, string Accent, int Fade, string? Picture);
    public static CustomTheme Custom { get; set; } = new("#141a26", "#22c55e", 45, null);

    /// <summary>The close button hides the window to the notification area; the app quits from the tray icon.</summary>
    public static bool CloseToTray { get; set; } = true;

    /// <summary>Accounts show as "Account 1", "Account 2"… instead of their names (see AppController.Names.cs).</summary>
    public static bool HideNames { get; set; }

    /// <summary>The "still running in the tray" notice was shown once.</summary>
    public static bool TrayHintShown { get; set; }

    /// <summary>Draw the interface on the CPU instead of the graphics card.</summary>
    public static bool SafeRendering { get; set; }

    /// <summary>Look for new versions by itself (a manual check is always possible).</summary>
    public static bool CheckUpdates { get; set; } = true;

    /// <summary>Updates include beta versions (GitHub pre-releases), automatic or not; off until the person turns it on.</summary>
    public static bool BetaUpdates { get; set; }

    /// <summary>Settings › Discord › Show on my Discord profile: off until the person turns it on (AppController.DiscordPresence.cs).</summary>
    public static bool DiscordPresence { get; set; }

    /// <summary>The version that last ran here; a newer one means the app was updated (see <see cref="WhatsNew"/>).</summary>
    public static string? LastVersion { get; set; }

    /// <summary>Important events reach the person as Windows notifications while the window is hidden or in the background.</summary>
    public static bool Notifications { get; set; } = true;

    /// <summary>
    /// Kinds of event turned off one by one under that switch (see AppController.Notify.cs); every kind is on until
    /// listed here. Names this build doesn't know (a newer one wrote them) are kept as they are.
    /// </summary>
    public static HashSet<string> NotifyOff { get; } = new(StringComparer.Ordinal);

    /// <summary>Questions answered with "Don't ask again" (see AppController.SkippableConfirms): their yes is assumed.</summary>
    public static HashSet<string> SkipConfirm { get; } = new(StringComparer.Ordinal);

    /// <summary>Where copies of the vault go (see <see cref="Backup"/>); null until chosen. Kept here so restoring works before unlocking.</summary>
    public static string? BackupFolder { get; set; }

    public static bool BackupEnabled { get; set; } = true;

    public static DateTime? LastBackup { get; set; }

    /// <summary>
    /// A change worth a backup was saved and not copied yet; cleared by the copy. Set when the app closed first (killed,
    /// or the copy on exit took too long), so the next unlock makes it.
    /// </summary>
    public static bool BackupPending { get; set; }

    /// <summary>Every saved sign-in is verified once a day (see AppController.Health.cs).</summary>
    public static bool DailySignInCheck { get; set; } = true;

    public static DateTime? LastSignInCheck { get; set; }

    /// <summary>The mini window (<see cref="MiniForm"/>) was open when the app last ran, so it comes back at start.</summary>
    public static bool MiniOpen { get; set; }

    /// <summary>The mini window's last place and size, "x,y,w,h" in screen pixels; null until it was moved.</summary>
    public static string? MiniWindow { get; set; }

    /// <summary>The main window's place and size when not maximised, "x,y,w,h" in screen pixels; null until it was first moved, resized or closed.</summary>
    public static string? MainWindow { get; set; }

    /// <summary>The main window was maximised when last left.</summary>
    public static bool MainMaximized { get; set; }

    /// <param name="At">When the build was installed (its folder's time), or first seen for the first entry.</param>
    /// <param name="Source">Whose copy ("Roblox", "Bloxstrap", …).</param>
    /// <param name="First">The first build Bloxkeep saw here, not an update it saw happen.</param>
    public sealed record RobloxBuild(DateTime At, string Version, string Folder, string Source, bool First);

    /// <summary>The Roblox client builds seen on this PC, oldest first (see AppController.RobloxUpdates.cs).</summary>
    public static List<RobloxBuild> RobloxHistory { get; } = new();

    /// <summary>The last time Roblox closed windows when another one opened, how many, and the day it was told.</summary>
    public static DateTime? RobloxClosedAt { get; set; }

    public static int RobloxClosedCount { get; set; }

    public static DateTime? RobloxClosedNotified { get; set; }

    /// <summary>
    /// Roblox builds (folder names) on which two windows played together here with Multi-Roblox on, oldest first: what
    /// "it works" looks like, so a warning from the Roblox watch about that build no longer applies.
    /// </summary>
    public static List<string> MultiWorked { get; } = new();

    /// <summary>
    /// Multi-Roblox was on in the vault last opened: it is taken as Bloxkeep starts, before the master password, so
    /// Roblox windows opened any way (roblox.com in any browser, the Roblox app) run side by side even while Bloxkeep
    /// waits at the lock screen. A copy of the vault's setting, kept in step by AppController.
    /// </summary>
    public static bool MultiRobloxAtStart { get; set; }

    static readonly string[] Known =
    [
        "language", "theme", "closeToTray", "hideNames", "trayHintShown", "safeRendering", "checkUpdates", "betaUpdates", "lastVersion",
        "notifications", "notifyOff", "backupFolder", "backupEnabled", "lastBackup", "backupPending", "dailySignInCheck", "lastSignInCheck", "miniOpen", "miniWindow",
        "mainWindow", "mainMaximized", "robloxHistory", "robloxClosedAt", "robloxClosedCount", "robloxClosedNotified", "skipConfirm", "multiWorked",
        "multiRobloxAtStart", "discordPresence", "customTheme",
    ];
    /// <summary>Keys this build doesn't know (a newer one wrote them): saved back untouched rather than dropped.</summary>
    static readonly Dictionary<string, JsonElement> _extra = new();

    static string? Text(JsonElement e, string name) =>
        e.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.String ? v.GetString() : null;

    public static void Load()
    {
        try
        {
            using var doc = JsonDocument.Parse(File.ReadAllText(PrefsPath));
            var root = doc.RootElement;
            if (root.ValueKind != JsonValueKind.Object)
                return;
            foreach (var p in root.EnumerateObject())
            {
                if (!Known.Contains(p.Name))
                    _extra[p.Name] = p.Value.Clone();
            }
            if (root.TryGetProperty("language", out var lang) && lang.ValueKind == JsonValueKind.String && lang.GetString() == "vi")
                Language = "vi";
            if (root.TryGetProperty("theme", out var theme) && theme.ValueKind == JsonValueKind.String)
                Theme = Themes.Normalize(theme.GetString());
            if (root.TryGetProperty("customTheme", out var custom) && custom.ValueKind == JsonValueKind.Object)
                Custom = new(Themes.Hex(Text(custom, "background")) ?? Custom.Background, Themes.Hex(Text(custom, "accent")) ?? Custom.Accent,
                    custom.TryGetProperty("fade", out var fade) && fade.ValueKind == JsonValueKind.Number && fade.TryGetInt32(out int f) ? Math.Clamp(f, 0, 90) : Custom.Fade,
                    Text(custom, "picture") is { } picture && Themes.IsPictureName(picture) ? picture : null);
            if (root.TryGetProperty("closeToTray", out var tray) && tray.ValueKind == JsonValueKind.False)
                CloseToTray = false;
            if (root.TryGetProperty("hideNames", out var hide) && hide.ValueKind == JsonValueKind.True)
                HideNames = true;
            if (root.TryGetProperty("trayHintShown", out var hint) && hint.ValueKind == JsonValueKind.True)
                TrayHintShown = true;
            if (root.TryGetProperty("safeRendering", out var safe) && safe.ValueKind == JsonValueKind.True)
                SafeRendering = true;
            if (root.TryGetProperty("checkUpdates", out var updates) && updates.ValueKind == JsonValueKind.False)
                CheckUpdates = false;
            if (root.TryGetProperty("betaUpdates", out var beta) && beta.ValueKind == JsonValueKind.True)
                BetaUpdates = true;
            if (root.TryGetProperty("lastVersion", out var last) && last.ValueKind == JsonValueKind.String)
                LastVersion = last.GetString();
            DiscordPresence = root.TryGetProperty("discordPresence", out var presence) && presence.ValueKind == JsonValueKind.True;
            if (root.TryGetProperty("notifications", out var notify) && notify.ValueKind == JsonValueKind.False)
                Notifications = false;
            if (root.TryGetProperty("notifyOff", out var off) && off.ValueKind == JsonValueKind.Array)
            {
                foreach (var kind in off.EnumerateArray())
                {
                    if (kind.ValueKind == JsonValueKind.String && kind.GetString() is { Length: > 0 and <= 40 } name)
                        NotifyOff.Add(name);
                }
            }
            if (root.TryGetProperty("skipConfirm", out var skip) && skip.ValueKind == JsonValueKind.Array)
            {
                foreach (var question in skip.EnumerateArray())
                {
                    if (question.ValueKind == JsonValueKind.String && question.GetString() is { Length: > 0 and <= 40 } name)
                        SkipConfirm.Add(name);
                }
            }
            if (root.TryGetProperty("backupFolder", out var folder) && folder.ValueKind == JsonValueKind.String)
                BackupFolder = folder.GetString() is { Length: > 0 } f ? f : null;
            if (root.TryGetProperty("backupEnabled", out var backup) && backup.ValueKind == JsonValueKind.False)
                BackupEnabled = false;
            if (root.TryGetProperty("lastBackup", out var lastBackup) && lastBackup.ValueKind == JsonValueKind.String && lastBackup.TryGetDateTime(out var at))
                // A time in the future (the clock was set back) would hold the next backup off for days.
                LastBackup = at.ToUniversalTime() is var utc && utc > DateTime.UtcNow ? DateTime.UtcNow : utc;
            if (root.TryGetProperty("backupPending", out var pending) && pending.ValueKind == JsonValueKind.True)
                BackupPending = true;
            if (root.TryGetProperty("dailySignInCheck", out var daily) && daily.ValueKind == JsonValueKind.False)
                DailySignInCheck = false;
            if (root.TryGetProperty("lastSignInCheck", out var lastCheck) && lastCheck.ValueKind == JsonValueKind.String && lastCheck.TryGetDateTime(out var checkedAt))
                // Likewise: a time in the future would hold the next check off.
                LastSignInCheck = checkedAt.ToUniversalTime() is var checkedUtc && checkedUtc > DateTime.UtcNow ? DateTime.UtcNow : checkedUtc;
            if (root.TryGetProperty("miniOpen", out var mini) && mini.ValueKind == JsonValueKind.True)
                MiniOpen = true;
            if (root.TryGetProperty("multiRobloxAtStart", out var multiAtStart) && multiAtStart.ValueKind == JsonValueKind.True)
                MultiRobloxAtStart = true;
            if (root.TryGetProperty("miniWindow", out var miniWindow) && miniWindow.ValueKind == JsonValueKind.String)
                MiniWindow = miniWindow.GetString();
            if (root.TryGetProperty("mainWindow", out var mainWindow) && mainWindow.ValueKind == JsonValueKind.String)
                MainWindow = mainWindow.GetString();
            if (root.TryGetProperty("mainMaximized", out var mainMax) && mainMax.ValueKind == JsonValueKind.True)
                MainMaximized = true;
            if (root.TryGetProperty("robloxHistory", out var history) && history.ValueKind == JsonValueKind.Array)
            {
                foreach (var h in history.EnumerateArray())
                {
                    if (h.ValueKind == JsonValueKind.Object && Text(h, "version") is { Length: > 0 and <= 40 } version
                        && Text(h, "folder") is { Length: > 0 and <= 80 } buildFolder
                        && h.TryGetProperty("at", out var hAt) && hAt.ValueKind == JsonValueKind.String && hAt.TryGetDateTime(out var builtAt))
                        RobloxHistory.Add(new RobloxBuild(builtAt.ToUniversalTime(), version, buildFolder, Text(h, "source") ?? "",
                            h.TryGetProperty("first", out var first) && first.ValueKind == JsonValueKind.True));
                }
            }
            if (root.TryGetProperty("robloxClosedAt", out var closedAt) && closedAt.ValueKind == JsonValueKind.String && closedAt.TryGetDateTime(out var closedUtc))
                RobloxClosedAt = closedUtc.ToUniversalTime();
            if (root.TryGetProperty("robloxClosedCount", out var closedCount) && closedCount.ValueKind == JsonValueKind.Number && closedCount.TryGetInt32(out int count))
                RobloxClosedCount = Math.Max(0, count);
            if (root.TryGetProperty("robloxClosedNotified", out var notified) && notified.ValueKind == JsonValueKind.String && notified.TryGetDateTime(out var notifiedUtc))
                RobloxClosedNotified = notifiedUtc.ToUniversalTime();
            if (root.TryGetProperty("multiWorked", out var worked) && worked.ValueKind == JsonValueKind.Array)
            {
                foreach (var build in worked.EnumerateArray())
                {
                    if (build.ValueKind == JsonValueKind.String && build.GetString() is { Length: > 0 and <= 80 } workedFolder)
                        MultiWorked.Add(workedFolder);
                }
            }
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or JsonException or InvalidOperationException)
        {
            // Missing or unreadable prefs: defaults.
        }
    }

    /// <summary>Written to a temp file first, so a crash mid-write never leaves an empty prefs.json (and defaults).</summary>
    public static void Save()
    {
        try
        {
            Directory.CreateDirectory(Vault.DataDir);
            var values = new Dictionary<string, object?>
            {
                ["language"] = Language, ["theme"] = Theme, ["closeToTray"] = CloseToTray, ["hideNames"] = HideNames, ["trayHintShown"] = TrayHintShown,
                ["safeRendering"] = SafeRendering, ["checkUpdates"] = CheckUpdates, ["betaUpdates"] = BetaUpdates, ["lastVersion"] = LastVersion,
                ["discordPresence"] = DiscordPresence,
                ["customTheme"] = new { background = Custom.Background, accent = Custom.Accent, fade = Custom.Fade, picture = Custom.Picture },
                ["notifications"] = Notifications, ["notifyOff"] = NotifyOff.Order(StringComparer.Ordinal).ToArray(),
                ["backupFolder"] = BackupFolder, ["backupEnabled"] = BackupEnabled, ["lastBackup"] = LastBackup,
                ["backupPending"] = BackupPending,
                ["dailySignInCheck"] = DailySignInCheck, ["lastSignInCheck"] = LastSignInCheck, ["miniOpen"] = MiniOpen, ["miniWindow"] = MiniWindow,
                ["mainWindow"] = MainWindow, ["mainMaximized"] = MainMaximized,
                ["robloxHistory"] = RobloxHistory.Select(h => new { at = h.At, version = h.Version, folder = h.Folder, source = h.Source, first = h.First }).ToArray(),
                ["robloxClosedAt"] = RobloxClosedAt, ["robloxClosedCount"] = RobloxClosedCount, ["robloxClosedNotified"] = RobloxClosedNotified,
                ["skipConfirm"] = SkipConfirm.Order(StringComparer.Ordinal).ToArray(),
                ["multiWorked"] = MultiWorked.ToArray(),
                ["multiRobloxAtStart"] = MultiRobloxAtStart,
            };
            // Never over a known key: a pref missing from Known would otherwise revert to its loaded value every restart.
            foreach (var (key, value) in _extra)
                values.TryAdd(key, value);
            // Flushed to disk before it replaces the old file: a power cut can't leave it empty (language, theme and
            // the backup folder would quietly reset, and backups stop).
            DurableFile.WriteText(PrefsPath, JsonSerializer.Serialize(values));
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            // The choice still applies for this session.
        }
    }
}
