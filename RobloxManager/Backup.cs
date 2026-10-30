using System.Globalization;
using System.Text.RegularExpressions;

namespace RobloxManager;

/// <summary>
/// Copies of the encrypted vault in a folder of the person's choice, typically one Google Drive or OneDrive syncs, so
/// accounts survive a dead PC. Bloxkeep never talks to those services: a backup is a byte copy of vault.rmv, and the
/// live vault never moves into the synced folder, where the sync client could lock it mid-save.
/// </summary>
static class Backup
{
    const string Prefix = "Bloxkeep-backup-";
    const int KeepLatest = 10;
    const int KeepDays = 30;
    // Bloxkeep-backup-<vault id>-<time>.rmv; the first releases wrote no id.
    static readonly Regex Name = new(@"^Bloxkeep-backup-(?:([0-9a-f]{8})-)?(\d{8}-\d{6})\.rmv$", RegexOptions.IgnoreCase | RegexOptions.CultureInvariant);
    static readonly TimeSpan SuggestionCacheLifetime = TimeSpan.FromMinutes(1);
    static List<(string Path, string Name)>? _suggestions;
    static DateTime _suggestedAt;

    public sealed record Folder(string Path, string Name);

    public static string FileName(string? vaultId, DateTime at) => $"{Prefix}{(vaultId is null ? "" : vaultId + "-")}{at:yyyyMMdd-HHmmss}.rmv";

    /// <summary>Writes the vault's bytes under a temp name, then renames it so the sync client never sees a half file. Safe
    /// on a worker thread (the caller read the bytes, so no save runs meanwhile).</summary>
    /// <param name="previousIds">Ids the vault had before its password changed (<see cref="AppSettings.PreviousVaultIds"/>).</param>
    public static string Write(string folder, byte[] vault, IReadOnlyCollection<string>? previousIds = null)
    {
        string? id = Vault.IdOf(vault);
        Directory.CreateDirectory(folder);
        string final = Path.Combine(folder, FileName(id, DateTime.Now));
        DurableFile.Write(final, vault); // never an empty file under a backup's name, counted among the kept ones
        var ids = new HashSet<string>(previousIds ?? [], StringComparer.OrdinalIgnoreCase);
        if (id is not null)
            ids.Add(id);
        Prune(folder, ids, justWritten: final);
        return final;
    }

    /// <summary>A copy named later than this past the clock was made before the clock went back (see <see cref="Prune"/>).</summary>
    static readonly TimeSpan FutureSlack = TimeSpan.FromMinutes(5);

    /// <summary>Vault ids read from old-format files' headers, by path, good while the file's size and time are the same.</summary>
    static readonly Dictionary<string, (long Size, DateTime WrittenAt, string? Id)> _headerIds = new(StringComparer.OrdinalIgnoreCase);
    /// <summary>A copy cut short (the app ended mid-write) leaves its temp file behind; one this old isn't being written.</summary>
    static readonly TimeSpan StaleTemp = TimeSpan.FromMinutes(5);

    /// <summary>
    /// Keeps the newest 10, plus the newest of each day in the last 30 days. Only this vault's backups count (current and
    /// earlier-password ids; an old-format file by its header): PCs with different vaults may share one synced folder.
    /// Names carry the PC's clock, which can go back: a copy named later than now ranks after newer ones, and
    /// <paramref name="justWritten"/> is never removed.
    /// </summary>
    public static void Prune(string folder, IReadOnlySet<string> vaultIds, string? justWritten = null)
    {
        DeleteStaleTemps(folder);
        var all = List(folder);
        var backups = all.Where(b => b.VaultId is { } named ? vaultIds.Contains(named) : vaultIds.Count > 0 && HeaderId(b.Path) is { } read && vaultIds.Contains(read)).ToList();
        lock (_headerIds)
        {
            // Headers of files no longer there are forgotten.
            foreach (string path in _headerIds.Keys.Where(p => !all.Any(b => string.Equals(b.Path, p, StringComparison.OrdinalIgnoreCase))).ToList())
                _headerIds.Remove(path);
        }
        var future = DateTime.Now + FutureSlack;
        var keep = new HashSet<string>(backups.OrderBy(b => b.At > future).Take(KeepLatest).Select(b => b.Path), StringComparer.OrdinalIgnoreCase);
        if (justWritten is not null)
            keep.Add(justWritten);
        var days = new HashSet<DateTime>();
        var cutoff = DateTime.Now.Date.AddDays(-KeepDays);
        foreach (var (path, at, _) in backups)
        {
            if (at.Date >= cutoff && days.Add(at.Date))
                keep.Add(path);
        }
        foreach (var (path, _, _) in backups.Where(b => !keep.Contains(b.Path)))
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

    /// <summary>An old-format file's header id, read once per size and time: reading a cloud-only copy downloads it.</summary>
    static string? HeaderId(string path)
    {
        long size;
        DateTime writtenAt;
        try
        {
            var info = new FileInfo(path);
            (size, writtenAt) = (info.Length, info.LastWriteTimeUtc);
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            return null;
        }
        lock (_headerIds)
        {
            if (_headerIds.TryGetValue(path, out var known) && known.Size == size && known.WrittenAt == writtenAt)
                return known.Id;
        }
        string? id = Vault.IdOf(path);
        lock (_headerIds)
            _headerIds[path] = (size, writtenAt, id);
        return id;
    }

    static void DeleteStaleTemps(string folder)
    {
        foreach (string tmp in Directory.EnumerateFiles(folder, Prefix + "*.rmv.tmp"))
        {
            try
            {
                if (DateTime.UtcNow - File.GetLastWriteTimeUtc(tmp) > StaleTemp)
                    File.Delete(tmp);
            }
            catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
            {
                // Held by the sync client; next time.
            }
        }
    }

    /// <summary>Backups in the folder, of every vault, newest first; <c>VaultId</c> is null for files named the old way.</summary>
    public static List<(string Path, DateTime At, string? VaultId)> List(string folder)
    {
        var list = new List<(string, DateTime, string?)>();
        if (!Directory.Exists(folder))
            return list;
        foreach (string path in Directory.EnumerateFiles(folder, Prefix + "*.rmv"))
        {
            var m = Name.Match(Path.GetFileName(path));
            if (m.Success && DateTime.TryParseExact(m.Groups[2].Value, "yyyyMMdd-HHmmss", CultureInfo.InvariantCulture, DateTimeStyles.None, out var at))
                list.Add((path, at, m.Groups[1].Success ? m.Groups[1].Value.ToLowerInvariant() : null));
        }
        return list.OrderByDescending(b => b.Item2).ToList();
    }

    /// <summary>
    /// Puts a backup in place as the vault. With a vault already there, the current one is first copied beside it as
    /// old-vaultsault-before-restore-….rmv, so a restore of the wrong file loses nothing.
    /// </summary>
    /// <returns>The copy kept of the vault it replaced, or null when there was none.</returns>
    public static string? Restore(string file, bool replaceExisting)
    {
        if (!Vault.LooksLikeVault(file))
            throw new UserError(L.T("This file is not a Bloxkeep backup."));
        bool exists = File.Exists(Vault.VaultPath);
        if (exists && !replaceExisting)
            throw new UserError(L.T("Bloxkeep is already set up."));
        byte[] bytes = File.ReadAllBytes(file);
        string? before = null;
        if (exists)
        {
            Directory.CreateDirectory(Vault.OldVaultsDir);
            before = Path.Combine(Vault.OldVaultsDir, $"vault-before-restore-{DateTime.Now:yyyyMMdd-HHmmss}.rmv");
            File.Copy(Vault.VaultPath, before, overwrite: false);
        }
        Vault.ReplaceFile(bytes);
        return before;
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
