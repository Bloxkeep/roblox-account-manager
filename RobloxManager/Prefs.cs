using System.Text.Json;

namespace RobloxManager;

/// <summary>Settings needed before the vault is unlocked (language, theme, safe rendering), in prefs.json. Nothing here is secret.</summary>
public static class Prefs
{
    static string PrefsPath => Path.Combine(Vault.DataDir, "prefs.json");

    public static string Language { get; set; } = "en";

    /// <summary>A theme id from <see cref="Themes"/>.</summary>
    public static string Theme { get; set; } = Themes.Default;

    /// <summary>Settings › Appearance › Custom: page colour and accent (#rrggbb), how much colour lies over the picture (0 to
    /// 90 %), the picture's or video's file in the data folder (theme-background-*.png…), and its blur (0 to 30 px).</summary>
    public sealed record CustomTheme(string Background, string Accent, int Fade, string? Picture, int Blur = 0);
    public static CustomTheme Custom { get; set; } = new("#141a26", "#22c55e", 45, null);

    /// <summary>A custom theme saved under a name (Settings › Appearance › Your themes): picked again, shared as a file.</summary>
    public sealed record SavedTheme(string Id, string Name, CustomTheme Look);
    public static List<SavedTheme> SavedThemes { get; set; } = [];

    /// <summary>The close button hides the window to the notification area; the app quits from the tray icon.</summary>
    public static bool CloseToTray { get; set; } = true;
    /// <summary>Ctrl+Alt+H (hide or show every Roblox window) and Ctrl+Alt+A (arrange them), from any app.</summary>
    public static bool GlobalHotkeys { get; set; } = true;

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

    /// <summary>Settings › Bloxkeep › Add me to the online count: on until the person turns it off (AppController.Online.cs).</summary>
    public static bool CountOnline { get; set; } = true;

    /// <summary>The online count's id for this PC, made from the PC at its first beat (the same after a reinstall) and linked
    /// to nothing else (AppController.Online.cs). Forgotten when the count is turned off.</summary>
    public static string? OnlineId { get; set; }

    /// <summary>The bot was told this copy's first "open" (a new install, AppController.Online.cs): told once.</summary>
    public static bool FirstOpenTold { get; set; }

    /// <summary>Settings › Discord › Control from Discord: off until the person turns it on (AppController.Remote.cs). Each
    /// vault has its own (<see cref="VaultEntry.RemoteControl"/>): vaults are for different people, and one person's /pc
    /// must never run another's presets or picture their accounts (2026-10-30 review).</summary>
    public static bool RemoteControl
    {
        get => Vaults.Current is { } v ? v.RemoteControl : _remoteControl;
        set
        {
            if (Vaults.Current is { } v)
                v.RemoteControl = value;
            else
                _remoteControl = value;
        }
    }
    static bool _remoteControl;

    /// <summary>The secret the vault in use proves itself with to Bloxkeep's Discord bot (a PC of its own there), made when
    /// Control from Discord is turned on and dropped when it's turned off; encrypted for this Windows user
    /// (AppController.Remote.cs). A new one is a new PC for the bot, linked to nobody.</summary>
    public static string? RemoteKey
    {
        get => Vaults.Current is { } v ? v.RemoteKey : _remoteKey;
        set
        {
            if (Vaults.Current is { } v)
                v.RemoteKey = value;
            else
                _remoteKey = value;
        }
    }
    static string? _remoteKey;

    /// <summary>Control from Discord as builds before 2026-10-30 kept it, one for the whole PC: it goes to the vault the lock
    /// screen offers (the one last used), unless that vault has its own already.</summary>
    internal static void MoveRemoteTo(VaultEntry? entry)
    {
        if (entry is null || (!_remoteControl && _remoteKey is null))
            return;
        if (!entry.RemoteControl && entry.RemoteKey is null)
            (entry.RemoteControl, entry.RemoteKey) = (_remoteControl, _remoteKey);
        (_remoteControl, _remoteKey) = (false, null);
    }

    /// <summary>The version that last ran here; a newer one means the app was updated (see <see cref="WhatsNew"/>).</summary>
    public static string? LastVersion { get; set; }

    /// <summary>Important events reach the person as Windows notifications while the window is hidden or in the background.</summary>
    public static bool Notifications { get; set; } = true;

    /// <summary>Event kinds turned off one by one under that switch (AppController.Notify.cs). Names this build doesn't know
    /// (a newer one wrote them) are kept.</summary>
    public static HashSet<string> NotifyOff { get; } = new(StringComparer.Ordinal);

    /// <summary>Questions answered with "Don't ask again" (see AppController.SkippableConfirms): their yes is assumed.</summary>
    public static HashSet<string> SkipConfirm { get; } = new(StringComparer.Ordinal);

    /// <summary>Where copies of the vault go (see <see cref="Backup"/>); null until chosen. Kept here so restoring works before unlocking.</summary>
    public static string? BackupFolder { get; set; }

    public static bool BackupEnabled { get; set; } = true;

    /// <summary>When the vault in use (<see cref="Vaults"/>) was last copied to the backup folder; each vault has its own.</summary>
    public static DateTime? LastBackup
    {
        get => Vaults.Current is { } v ? v.LastBackup : _lastBackup;
        set
        {
            if (Vaults.Current is { } v)
                v.LastBackup = value;
            else
                _lastBackup = value;
        }
    }
    static DateTime? _lastBackup;

    /// <summary>A change worth a backup was saved and not copied yet; still set if the app closed first (killed, or the copy
    /// on exit took too long), so the next unlock makes it. Each vault has its own.</summary>
    public static bool BackupPending
    {
        get => Vaults.Current is { } v ? v.BackupPending : _backupPending;
        set
        {
            if (Vaults.Current is { } v)
                v.BackupPending = value;
            else
                _backupPending = value;
        }
    }
    static bool _backupPending;

    /// <summary>A vault on this PC (the lock screen's picker, <see cref="Vaults"/>): its own file and master password. Names
    /// and pictures show before anything is unlocked, so nothing here is secret.</summary>
    public sealed class VaultEntry
    {
        public string Id { get; set; } = "";
        /// <summary>vault.rmv for the first, vault-&lt;id&gt;.rmv for the others.</summary>
        public string File { get; set; } = "";
        public string Name { get; set; } = "";
        /// <summary>A picture copied into the data folder's pictures (vault-picture-&lt;id&gt;-&lt;time&gt;.png…), or none: the name's first letter shows.</summary>
        public string? Picture { get; set; }
        public DateTime? LastBackup { get; set; }
        public bool BackupPending { get; set; }
        /// <summary>This vault's Control from Discord (<see cref="Prefs.RemoteControl"/>) and its secret for the bot
        /// (<see cref="Prefs.RemoteKey"/>, encrypted for this Windows user, so not readable from the file).</summary>
        public bool RemoteControl { get; set; }
        public string? RemoteKey { get; set; }
    }

    public static List<VaultEntry> VaultList { get; } = new();

    /// <summary>The vault last opened or picked: the one the lock screen offers at the next start.</summary>
    public static string? LastVault { get; set; }

    /// <summary>The lock screen starts on the picker, Steam-like ("Ask which vault to use each time Bloxkeep starts").</summary>
    public static bool AskVault { get; set; }

    /// <summary>Every saved sign-in is verified once a day (see AppController.Health.cs).</summary>
    public static bool DailySignInCheck { get; set; } = true;

    public static DateTime? LastSignInCheck { get; set; }

    /// <summary>The mini window (<see cref="MiniForm"/>) was open when the app last ran, so it comes back at start.</summary>
    public static bool MiniOpen { get; set; }

    /// <summary>The mini window's last place and size, "x,y,w,h" in screen pixels; null until it was moved.</summary>
    public static string? MiniWindow { get; set; }

    /// <summary>The main window's place and size when not maximised, "x,y,w,h" in screen pixels; null until it was first moved, resized or closed.</summary>
    public static string? MainWindow { get; set; }

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

    /// <summary>Roblox builds (folder names) on which two windows played together here with Multi-Roblox on, oldest first,
    /// so a Roblox watch warning about that build no longer applies.</summary>
    public static List<string> MultiWorked { get; } = new();

    /// <summary>Ids of vaults unlocked here (<see cref="Vault.Id"/>, a hash of the salt), old ids included: a restored backup
    /// with an id not in here was made somewhere else.</summary>
    public static HashSet<string> KnownVaultIds { get; } = new(StringComparer.Ordinal);

    /// <summary>A vault restored from another PC's backup, until first unlocked: its Roblox launcher and Discord links are
    /// dropped then (AppController.RestoreBackup).</summary>
    public static string? ForeignRestore { get; set; }

    public static bool IsVaultId(string? id) => id is { Length: 8 } && id.All(char.IsAsciiHexDigitLower);

    /// <summary>A copy of the last vault's Multi-Roblox switch, taken at start before the master password so Roblox windows
    /// opened any way run side by side even at the lock screen. Kept in step by AppController.</summary>
    public static bool MultiRobloxAtStart { get; set; }

    static readonly string[] Known =
    [
        "language", "theme", "closeToTray", "hideNames", "trayHintShown", "safeRendering", "checkUpdates", "betaUpdates", "lastVersion",
        "notifications", "notifyOff", "backupFolder", "backupEnabled", "lastBackup", "backupPending", "dailySignInCheck", "lastSignInCheck", "miniOpen", "miniWindow",
        "mainWindow", "mainMaximized", "robloxHistory", "robloxClosedAt", "robloxClosedCount", "robloxClosedNotified", "skipConfirm", "multiWorked",
        "multiRobloxAtStart", "discordPresence", "customTheme", "vaults", "lastVault", "askVault", "countOnline", "onlineId",
        "knownVaultIds", "foreignRestore", "savedThemes",
    ];
    /// <summary>Keys this build doesn't know (a newer one wrote them): saved back untouched rather than dropped.</summary>
    static readonly Dictionary<string, JsonElement> _extra = new();

    /// <summary>A custom theme as saved (its colours checked, the numbers kept in range); what is missing or wrong from <paramref name="fallback"/>.</summary>
    static CustomTheme ReadCustom(JsonElement e, CustomTheme fallback)
    {
        static int? Number(JsonElement e, string name) => e.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.Number && v.TryGetInt32(out int n) ? n : null;
        return new(Themes.Hex(Text(e, "background")) ?? fallback.Background, Themes.Hex(Text(e, "accent")) ?? fallback.Accent,
            Number(e, "fade") is { } fade ? Math.Clamp(fade, 0, 90) : fallback.Fade,
            Text(e, "picture") is { } picture && Themes.IsThemeMediaName(picture) ? picture : null,
            Number(e, "blur") is { } blur ? Math.Clamp(blur, 0, Themes.MaxBlur) : 0);
    }

    static string? Text(JsonElement e, string name) =>
        e.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.String ? v.GetString() : null;

    public static void Load()
    {
        try
        {
            using var doc = JsonDocument.Parse(File.ReadAllText(PrefsPath));
            var root = doc.RootElement;
            if (root.ValueKind != JsonValueKind.Object)
            {
                KeepDamaged();
                return;
            }
            foreach (var p in root.EnumerateObject())
            {
                if (!Known.Contains(p.Name))
                    _extra[p.Name] = p.Value.Clone();
            }
            if (root.TryGetProperty("language", out var lang) && lang.ValueKind == JsonValueKind.String && lang.GetString() == "vi")
                Language = "vi";
            if (root.TryGetProperty("theme", out var theme) && theme.ValueKind == JsonValueKind.String)
                Theme = Themes.Normalize(theme.GetString());
            if (root.TryGetProperty("vaults", out var vaults) && vaults.ValueKind == JsonValueKind.Array)
            {
                foreach (var v in vaults.EnumerateArray())
                {
                    if (v.ValueKind != JsonValueKind.Object || Text(v, "id") is not { } vid || !Vaults.IsId(vid) || Text(v, "file") is not { } vfile
                        || !Vaults.IsFile(vfile) || VaultList.Any(e => e.Id == vid || e.File == vfile))
                        continue;
                    VaultList.Add(new VaultEntry
                    {
                        Id = vid, File = vfile, Name = Vaults.CleanName(Text(v, "name")) ?? "Vault",
                        Picture = Text(v, "picture") is { } vpic && Themes.IsPictureName(vpic) ? vpic : null,
                        LastBackup = v.TryGetProperty("lastBackup", out var vb) && vb.ValueKind == JsonValueKind.String && vb.TryGetDateTime(out var vbAt)
                            ? vbAt.ToUniversalTime() is var vbUtc && vbUtc > DateTime.UtcNow ? DateTime.UtcNow : vbUtc : null,
                        BackupPending = v.TryGetProperty("backupPending", out var vp) && vp.ValueKind == JsonValueKind.True,
                        RemoteControl = v.TryGetProperty("remoteControl", out var vr) && vr.ValueKind == JsonValueKind.True,
                        RemoteKey = Text(v, "remoteKey"),
                    });
                }
            }
            LastVault = Text(root, "lastVault");
            AskVault = root.TryGetProperty("askVault", out var ask) && ask.ValueKind == JsonValueKind.True;
            if (root.TryGetProperty("customTheme", out var custom) && custom.ValueKind == JsonValueKind.Object)
                Custom = ReadCustom(custom, Custom);
            if (root.TryGetProperty("savedThemes", out var saved) && saved.ValueKind == JsonValueKind.Array)
                SavedThemes = saved.EnumerateArray()
                    .Where(t => t.ValueKind == JsonValueKind.Object && Text(t, "id") is { Length: > 0 and <= 40 } && Themes.ThemeName(Text(t, "name")) is not null)
                    .Select(t => new SavedTheme(Text(t, "id")!, Themes.ThemeName(Text(t, "name"))!, ReadCustom(t, Custom)))
                    .DistinctBy(t => t.Id).Take(Themes.MaxSaved).ToList();
            if (root.TryGetProperty("closeToTray", out var tray) && tray.ValueKind == JsonValueKind.False)
                CloseToTray = false;
            if (root.TryGetProperty("globalHotkeys", out var keys) && keys.ValueKind == JsonValueKind.False)
                GlobalHotkeys = false;
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
            if (root.TryGetProperty("countOnline", out var countOnline) && countOnline.ValueKind == JsonValueKind.False)
                CountOnline = false;
            if (Text(root, "onlineId") is { } onlineId && Guid.TryParseExact(onlineId, "N", out _))
                OnlineId = onlineId;
            // From before each vault had its own: moved to a vault by Vaults.Load (MoveRemoteTo).
            _remoteControl = root.TryGetProperty("remoteControl", out var remote) && remote.ValueKind == JsonValueKind.True;
            FirstOpenTold = root.TryGetProperty("firstOpenTold", out var told) && told.ValueKind == JsonValueKind.True;
            _remoteKey = Text(root, "remoteKey");
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
                _lastBackup = at.ToUniversalTime() is var utc && utc > DateTime.UtcNow ? DateTime.UtcNow : utc;
            if (root.TryGetProperty("backupPending", out var pending) && pending.ValueKind == JsonValueKind.True)
                _backupPending = true;
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
            if (root.TryGetProperty("knownVaultIds", out var known) && known.ValueKind == JsonValueKind.Array)
            {
                foreach (var id in known.EnumerateArray())
                {
                    if (id.ValueKind == JsonValueKind.String && id.GetString() is { } vaultId && IsVaultId(vaultId))
                        KnownVaultIds.Add(vaultId);
                }
            }
            if (Text(root, "foreignRestore") is { } restored && IsVaultId(restored))
                ForeignRestore = restored;
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or JsonException or InvalidOperationException)
        {
            // Missing or unreadable prefs: defaults. One that isn't JSON any more is kept aside first.
            if (ex is JsonException or InvalidOperationException)
                KeepDamaged();
        }
    }

    /// <summary>A prefs.json that can't be read is copied to prefs.json.damaged-&lt;time&gt; before the next save writes the
    /// defaults over it: the vault list, backup folder and the rest can be got back from it (review, 2026-10-30).</summary>
    static void KeepDamaged()
    {
        try
        {
            File.Copy(PrefsPath, $"{PrefsPath}.damaged-{DateTime.Now:yyyyMMdd-HHmmss}", overwrite: false);
            Log.Note("prefs.json couldn't be read; a copy was kept beside it and the settings start from their defaults.");
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            // Already kept this second, or the folder won't take it: the defaults apply all the same.
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
                ["language"] = Language, ["theme"] = Theme, ["closeToTray"] = CloseToTray, ["globalHotkeys"] = GlobalHotkeys, ["hideNames"] = HideNames, ["trayHintShown"] = TrayHintShown,
                ["safeRendering"] = SafeRendering, ["checkUpdates"] = CheckUpdates, ["betaUpdates"] = BetaUpdates, ["lastVersion"] = LastVersion,
                ["discordPresence"] = DiscordPresence, ["countOnline"] = CountOnline, ["onlineId"] = OnlineId,
                // Written (empty once moved to a vault) so the old keys never come back from what this build doesn't know.
                ["remoteControl"] = _remoteControl, ["remoteKey"] = _remoteKey, ["firstOpenTold"] = FirstOpenTold,
                ["customTheme"] = new { background = Custom.Background, accent = Custom.Accent, fade = Custom.Fade, picture = Custom.Picture, blur = Custom.Blur },
                ["savedThemes"] = SavedThemes.Select(t => new
                {
                    id = t.Id, name = t.Name, background = t.Look.Background, accent = t.Look.Accent, fade = t.Look.Fade, picture = t.Look.Picture, blur = t.Look.Blur,
                }).ToArray(),
                ["notifications"] = Notifications, ["notifyOff"] = NotifyOff.Order(StringComparer.Ordinal).ToArray(),
                // The first vault's backup state also where builds before several vaults read it.
                ["backupFolder"] = BackupFolder, ["backupEnabled"] = BackupEnabled,
                ["lastBackup"] = VaultList.FirstOrDefault(v => v.File == Vaults.FirstFile)?.LastBackup ?? _lastBackup,
                ["backupPending"] = VaultList.FirstOrDefault(v => v.File == Vaults.FirstFile)?.BackupPending ?? _backupPending,
                ["vaults"] = VaultList.Select(v => new { id = v.Id, file = v.File, name = v.Name, picture = v.Picture, lastBackup = v.LastBackup, backupPending = v.BackupPending,
                    remoteControl = v.RemoteControl, remoteKey = v.RemoteKey }).ToArray(),
                ["lastVault"] = LastVault, ["askVault"] = AskVault,
                ["dailySignInCheck"] = DailySignInCheck, ["lastSignInCheck"] = LastSignInCheck, ["miniOpen"] = MiniOpen, ["miniWindow"] = MiniWindow,
                ["mainWindow"] = MainWindow, ["mainMaximized"] = MainMaximized,
                ["robloxHistory"] = RobloxHistory.Select(h => new { at = h.At, version = h.Version, folder = h.Folder, source = h.Source, first = h.First }).ToArray(),
                ["robloxClosedAt"] = RobloxClosedAt, ["robloxClosedCount"] = RobloxClosedCount, ["robloxClosedNotified"] = RobloxClosedNotified,
                ["skipConfirm"] = SkipConfirm.Order(StringComparer.Ordinal).ToArray(),
                ["multiWorked"] = MultiWorked.ToArray(),
                ["multiRobloxAtStart"] = MultiRobloxAtStart,
                ["knownVaultIds"] = KnownVaultIds.Order(StringComparer.Ordinal).ToArray(), ["foreignRestore"] = ForeignRestore,
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
