using System.Text.RegularExpressions;

namespace RobloxManager;

/// <summary>
/// Several vaults on one PC, picked like Steam's accounts ("Who's using Bloxkeep?" on the lock screen): each its own
/// file and master password, with its own accounts, presets and settings, and its backups apart (Backup names them by
/// the vault's own id). The list and the one last used are in prefs.json (<see cref="Prefs.VaultList"/>); the file the
/// lock screen opens is <see cref="Vault.ActiveFile"/>. Roblox windows opened from one keep running while another is in use.
/// </summary>
public static class Vaults
{
    /// <summary>The first vault's file, as it was before there could be several.</summary>
    public const string FirstFile = "vault.rmv";

    /// <summary>The vault in use, or the one the lock screen offers; null before one is set up (or while one is being added).</summary>
    public static Prefs.VaultEntry? Current =>
        Prefs.VaultList.FirstOrDefault(v => v.File.Equals(Vault.ActiveFile, StringComparison.OrdinalIgnoreCase));

    public static bool IsId(string id) => Regex.IsMatch(id, "^[0-9a-f]{8}$");
    public static bool IsFile(string file) => file == FirstFile || Regex.IsMatch(file, @"^vault-[0-9a-f]{8}\.rmv$");

    /// <summary>A name as typed, trimmed and at most 40 characters; null when there is nothing left.</summary>
    public static string? CleanName(string? name) => name?.Trim() is { Length: > 0 } n ? n.Length > 40 ? n[..40].TrimEnd() : n : null;

    /// <summary>
    /// After prefs.json is read: a vault from before there could be several (vault.rmv) joins the list with the backup
    /// state kept for it then; vaults whose file is gone leave it; the lock screen offers the last one used.
    /// </summary>
    public static void Load()
    {
        Vault.ActiveFile = FirstFile;
        if (!Prefs.VaultList.Any(v => v.File == FirstFile) && Vault.Exists)
        {
            var (lastBackup, pending) = (Prefs.LastBackup, Prefs.BackupPending); // before it joins: what was kept for it
            Prefs.VaultList.Insert(0, new Prefs.VaultEntry { Id = NewId(), File = FirstFile, Name = NameFor(0), LastBackup = lastBackup, BackupPending = pending });
        }
        Prefs.VaultList.RemoveAll(v => !ExistsOnDisk(v.File));
        Use(Prefs.VaultList.FirstOrDefault(v => v.Id == Prefs.LastVault) ?? Prefs.VaultList.FirstOrDefault());
    }

    /// <summary>The lock screen offers <paramref name="entry"/> (the first vault's file when none is set up yet).</summary>
    public static void Use(Prefs.VaultEntry? entry) => Vault.ActiveFile = entry?.File ?? FirstFile;

    /// <summary>A vault about to be set up (not in the list until it is): a new id, its file, "Vault 2" and so on.</summary>
    public static Prefs.VaultEntry New()
    {
        string id;
        do
            id = NewId();
        while (Prefs.VaultList.Any(v => v.Id == id) || File.Exists(Path.Combine(Vault.DataDir, $"vault-{id}.rmv")));
        // The first vault keeps vault.rmv (what builds before several vaults open), the others their own file.
        bool first = !Prefs.VaultList.Any(v => v.File == FirstFile) && !ExistsOnDisk(FirstFile);
        return new Prefs.VaultEntry { Id = id, File = first ? FirstFile : $"vault-{id}.rmv", Name = NameFor(Prefs.VaultList.Count) };
    }

    static string NewId() => Convert.ToHexString(System.Security.Cryptography.RandomNumberGenerator.GetBytes(4)).ToLowerInvariant();

    /// <summary>"Vault 1", "Vault 2"…: the first such name not in use.</summary>
    static string NameFor(int count)
    {
        for (int n = count + 1; ; n++)
        {
            string name = $"Vault {n}";
            if (!Prefs.VaultList.Any(v => v.Name.Equals(name, StringComparison.OrdinalIgnoreCase)))
                return name;
        }
    }

    /// <summary>The vault file, or a save of it that <see cref="Vault.Exists"/> would put back.</summary>
    static bool ExistsOnDisk(string file)
    {
        string path = Path.Combine(Vault.DataDir, file);
        return File.Exists(path) || File.Exists(path + ".tmp") || File.Exists(path + ".bak");
    }
}
