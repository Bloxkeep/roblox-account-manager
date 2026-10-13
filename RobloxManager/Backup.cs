using System.Globalization;
using System.Text.RegularExpressions;

namespace RobloxManager;

/// <summary>
/// Copies of the encrypted vault in a folder of the person's choice, typically one their Google Drive or OneDrive
/// client syncs, so the accounts survive a dead PC. Bloxkeep never talks to those services itself: a backup is a byte
/// copy of vault.rmv (already encrypted with the master password), and the live vault never moves into the synced
/// folder, where the sync client could lock it mid-save.
/// </summary>
static class Backup
{
    const string Prefix = "Bloxkeep-backup-";
    const int KeepLatest = 10;
    const int KeepDays = 30;
    static readonly Regex Name = new(@"^Bloxkeep-backup-(\d{8}-\d{6})\.rmv$", RegexOptions.IgnoreCase | RegexOptions.CultureInvariant);
    static readonly TimeSpan SuggestionCacheLifetime = TimeSpan.FromMinutes(1);
    static List<(string Path, string Name)>? _suggestions;
    static DateTime _suggestedAt;

    public sealed record Folder(string Path, string Name);

    public static string FileName(DateTime at) => $"{Prefix}{at:yyyyMMdd-HHmmss}.rmv";

    /// <summary>Copies vault.rmv into the folder under a temp name, then renames it so the sync client never sees a half file.</summary>
    public static string Write(string folder)
    {
        Directory.CreateDirectory(folder);
        string final = Path.Combine(folder, FileName(DateTime.Now));
        string tmp = final + ".tmp";
        File.Copy(Vault.VaultPath, tmp, overwrite: true);
        File.Move(tmp, final, overwrite: true);
        Prune(folder);
        return final;
    }

    /// <summary>Keeps the newest 10, plus the newest of each day in the last 30 days; only files named like a backup are touched.</summary>
    public static void Prune(string folder)
    {
        var backups = List(folder);
        var keep = new HashSet<string>(backups.Take(KeepLatest).Select(b => b.Path), StringComparer.OrdinalIgnoreCase);
        var days = new HashSet<DateTime>();
        var cutoff = DateTime.Now.Date.AddDays(-KeepDays);
        foreach (var (path, at) in backups)
        {
            if (at.Date >= cutoff && days.Add(at.Date))
                keep.Add(path);
        }
        foreach (var (path, _) in backups.Where(b => !keep.Contains(b.Path)))
        {
            try
            {
                File.Delete(path);
            }
            catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
            {
                // The sync client may be uploading it; next time.
            }
        }
    }

    /// <summary>Backups in the folder, newest first.</summary>
    public static List<(string Path, DateTime At)> List(string folder)
    {
        var list = new List<(string, DateTime)>();
        if (!Directory.Exists(folder))
            return list;
        foreach (string path in Directory.EnumerateFiles(folder, Prefix + "*.rmv"))
        {
            var m = Name.Match(Path.GetFileName(path));
            if (m.Success && DateTime.TryParseExact(m.Groups[1].Value, "yyyyMMdd-HHmmss", CultureInfo.InvariantCulture, DateTimeStyles.None, out var at))
                list.Add((path, at));
        }
        return list.OrderByDescending(b => b.Item2).ToList();
    }

    /// <summary>
    /// Puts a backup in place as the vault. With a vault already there, the current one is first copied beside it as
    /// vault-before-restore-….rmv, so a restore of the wrong file loses nothing.
    /// </summary>
    public static void Restore(string file, bool replaceExisting)
    {
        if (!Vault.LooksLikeVault(file))
            throw new UserError(L.T("This file is not a Bloxkeep backup."));
        bool exists = File.Exists(Vault.VaultPath);
        if (exists && !replaceExisting)
            throw new UserError(L.T("Bloxkeep is already set up."));
        byte[] bytes = File.ReadAllBytes(file);
        if (exists)
            File.Copy(Vault.VaultPath, Path.Combine(Vault.DataDir, $"vault-before-restore-{DateTime.Now:yyyyMMdd-HHmmss}.rmv"), overwrite: false);
        Vault.ReplaceFile(bytes);
    }

    /// <summary>The live vault must stay out of the backup folder, and the folder out of the live data.</summary>
    public static bool IsDataFolder(string folder)
    {
        string a = Path.TrimEndingDirectorySeparator(Path.GetFullPath(folder));
        string b = Path.TrimEndingDirectorySeparator(Path.GetFullPath(Vault.DataDir));
        return a.Equals(b, StringComparison.OrdinalIgnoreCase) || a.StartsWith(b + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase);
    }

    /// <summary>A path inside a folder a cloud client syncs, judged by name only.</summary>
    public static bool LooksLikeCloud(string folder) =>
        folder.Contains("OneDrive", StringComparison.OrdinalIgnoreCase) || folder.Contains("My Drive", StringComparison.OrdinalIgnoreCase)
        || folder.Contains("Google Drive", StringComparison.OrdinalIgnoreCase) || folder.Contains("Dropbox", StringComparison.OrdinalIgnoreCase)
        || folder.Contains("iCloud", StringComparison.OrdinalIgnoreCase);

    /// <summary>Short enough, or made of digits and lowercase letters only, to be guessed from a leaked backup.</summary>
    public static bool WeakPassword(string password) =>
        password.Length < 8 || password.All(c => char.IsAsciiDigit(c) || char.IsAsciiLetterLower(c));

    /// <summary>Synced folders found on this PC (OneDrive, Google Drive's "My Drive"), each with a Bloxkeep subfolder.</summary>
    public static List<Folder> Suggestions()
    {
        if (_suggestions is null || DateTime.UtcNow - _suggestedAt > SuggestionCacheLifetime)
        {
            _suggestions = FindSyncedFolders();
            _suggestedAt = DateTime.UtcNow;
        }
        return _suggestions.Select(s => new Folder(s.Path, s.Name)).ToList();
    }

    static List<(string Path, string Name)> FindSyncedFolders()
    {
        var found = new List<(string, string)>();
        var seen = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        void Add(string? root, string name)
        {
            try
            {
                if (root is { Length: > 0 } && Directory.Exists(root) && seen.Add(Path.GetFullPath(root)))
                    found.Add((Path.Combine(root, "Bloxkeep"), name));
            }
            catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or ArgumentException)
            {
            }
        }
        foreach (string variable in new[] { "OneDrive", "OneDriveConsumer", "OneDriveCommercial" })
            Add(Environment.GetEnvironmentVariable(variable), "OneDrive");
        string profile = Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);
        // Google Drive for desktop: a virtual drive (G:\My Drive) when streaming, a folder in the profile when mirroring.
        Add(Path.Combine(profile, "My Drive"), "Google Drive");
        Add(Path.Combine(profile, "Google Drive"), "Google Drive");
        try
        {
            foreach (var drive in DriveInfo.GetDrives())
            {
                if (drive.DriveType == DriveType.Fixed && drive.IsReady)
                    Add(Path.Combine(drive.RootDirectory.FullName, "My Drive"), "Google Drive");
            }
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
        }
        return found;
    }
}
