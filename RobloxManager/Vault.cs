using System.Buffers.Binary;
using System.Security.Cryptography;
using System.Text.Json;

namespace RobloxManager;

public sealed class InvalidPasswordException() : Exception(L.T("Wrong master password."));

/// <summary>
/// Encrypted account store kept only on this machine.
/// File layout: "RMV1" | PBKDF2 iterations (int32 LE) | salt (16) | nonce (12) | GCM tag (16) | ciphertext.
/// Key = PBKDF2-HMAC-SHA256(master password, salt); cipher = AES-256-GCM with the header bound as associated data,
/// so any tampering with the file (or a wrong password) is detected on open.
/// </summary>
public sealed class Vault : IDisposable
{
    const int SaltSize = 16, NonceSize = 12, TagSize = 16, KeySize = 32;
    const int HeaderSize = 4 + 4 + SaltSize;
    const int DefaultIterations = 600_000;
    static readonly byte[] Magic = "RMV1"u8.ToArray();

    /// <summary>%LOCALAPPDATA%\Bloxkeep by default, apart from the program (%LOCALAPPDATA%\BloxkeepApp, which uninstalling
    /// deletes), so accounts survive an uninstall. The portable zip, or a "data" folder next to the exe, keeps data beside
    /// the app; BLOXKEEP_DATA overrides all of these.</summary>
    public static string DataDir { get; } = ResolveDataDir();

    /// <summary>Old copies of vaults: one taken off the list (vault-removed-…), the one a restore replaced (vault-before-restore-…).</summary>
    public static string OldVaultsDir => Path.Combine(DataDir, "old-vaults");
    /// <summary>The custom theme's picture and the vaults' pictures (<see cref="Themes.IsPictureName"/>).</summary>
    public static string PicturesDir => Path.Combine(DataDir, "pictures");
    /// <summary>What Bloxkeep rebuilds when it's gone: the browsers' profiles, the sign-in windows' client paths.</summary>
    public static string CacheDir => Path.Combine(DataDir, "cache");

    /// <summary>A browser profile's folder in <see cref="CacheDir"/>, with cache made first (best effort: WebView2 reports its own error).</summary>
    public static string CacheFolder(string name)
    {
        try
        {
            Directory.CreateDirectory(CacheDir);
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
        }
        return Path.Combine(CacheDir, name);
    }
    static readonly string[] CacheFolders = ["webview", "webview-safe", "captcha", "roblox-client-paths"];

    /// <summary>Moves what builds before 1.0.11 kept at the top of the data folder into old-vaults, pictures and cache. Run
    /// at start, before anything opens them; what won't move (held by another program) is tried next start. The vaults,
    /// their .bak and .tmp copies, settings and the log stay at the top. Never stops the start: errors are only logged.</summary>
    public static void Tidy()
    {
        try
        {
            if (Directory.Exists(DataDir))
                TidyFolder();
        }
        catch (Exception ex)
        {
            Log.Write(ex);
        }
    }

    static void TidyFolder()
    {
        foreach (string file in Directory.EnumerateFiles(DataDir).ToList())
        {
            string name = Path.GetFileName(file);
            string? to = IsOldVault(name) ? OldVaultsDir : Themes.IsPictureName(name) ? PicturesDir : null;
            if (to is null)
                continue;
            try
            {
                Directory.CreateDirectory(to);
                if (!File.Exists(Path.Combine(to, name)))
                    File.Move(file, Path.Combine(to, name));
            }
            catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
            {
                Log.Write(ex);
            }
        }
        foreach (string name in CacheFolders)
        {
            string from = Path.Combine(DataDir, name), to = Path.Combine(CacheDir, name);
            // Right after an update's restart the last run's browser may hold its profile a moment longer; moved
            // later, the interface would start on an empty one (the list's layout and sort forgotten).
            for (int attempt = 1; attempt <= 6 && Directory.Exists(from); attempt++)
            {
                try
                {
                    Directory.CreateDirectory(CacheDir);
                    if (!Directory.Exists(to))
                    {
                        Directory.Move(from, to);
                    }
                    else if (name == "roblox-client-paths")
                    {
                        // Junctions to Roblox's own folder: each removed as a link, never walked into.
                        RobloxHomeSession.CleanupAliases(from);
                        Directory.Delete(from);
                    }
                    else
                    {
                        // A cache: with a new one already there the old one is only in the way.
                        Directory.Delete(from, recursive: true);
                    }
                }
                catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
                {
                    // Waited for only to move one: an old copy beside a new one, or junctions in use (until Roblox
                    // closes), wait for the next start instead of slowing every start.
                    if (attempt == 6 || Directory.Exists(to))
                    {
                        Log.Write(ex);
                        break;
                    }
                    Thread.Sleep(500);
                }
            }
        }
    }

    static bool IsOldVault(string name) =>
        (name.StartsWith("vault-before-restore-", StringComparison.OrdinalIgnoreCase) || name.StartsWith("vault-removed-", StringComparison.OrdinalIgnoreCase))
        && name.EndsWith(".rmv", StringComparison.OrdinalIgnoreCase);

    /// <summary>Removes the interface's browser caches (rebuilt when needed). Accounts and settings stay.</summary>
    public static void DeleteCaches()
    {
        // The top of the data folder too: where builds before 1.0.11 kept them. Not in a folder BLOXKEEP_DATA points at: it
        // may be the person's own, with a "webview" or "captcha" folder that isn't ours (review, 2026-10-30).
        bool ownFolder = DataDirVariable() is null;
        foreach (string dir in new[] { "webview", "webview-safe", "captcha" }
                     .SelectMany(n => ownFolder ? new[] { Path.Combine(CacheDir, n), Path.Combine(DataDir, n) } : [Path.Combine(CacheDir, n)]))
        {
            // The browser processes may need a moment to let go of their files.
            for (int attempt = 1; attempt <= 4 && Directory.Exists(dir); attempt++)
            {
                try
                {
                    Directory.Delete(dir, recursive: true);
                }
                catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
                {
                    Thread.Sleep(1000);
                }
            }
        }
    }
    /// <summary>An uninstall that takes everything: vaults with their spare copies, settings, logs and caches. Only Bloxkeep's
    /// own files (DataDir may be a folder BLOXKEEP_DATA points at), then the folder once empty. Backups stay.</summary>
    public static void DeleteAllData()
    {
        if (!Directory.Exists(DataDir))
            return;
        DeleteCaches();
        // By exact name or pattern: in a folder of the person's own, "prefs.json5" or "error.logs" aren't ours.
        var own = new HashSet<string>(StringComparer.OrdinalIgnoreCase)
            { "prefs.json", "prefs.json.tmp", "error.log", "roblox-settings.json", "starting.flag", "resume.bin" };
        bool Own(string name) => own.Contains(name)
            // Every vault (vault.rmv, vault-<id>.rmv) with its spare, unfinished and damaged copies.
            || System.Text.RegularExpressions.Regex.IsMatch(name, @"^vault(-[0-9a-f]{8})?\.rmv(\.bak|\.tmp|\.damaged-.*|\.spare-.*)?$", System.Text.RegularExpressions.RegexOptions.IgnoreCase)
            || name.StartsWith("prefs.json.damaged-", StringComparison.OrdinalIgnoreCase)
            || IsOldVault(name)
            || Themes.IsPictureName(name); // the custom theme's picture, the vaults' pictures
        var folders = new[] { DataDir, OldVaultsDir, PicturesDir }.Where(Directory.Exists);
        foreach (string file in folders.SelectMany(Directory.EnumerateFiles).Where(f => Own(Path.GetFileName(f))).ToList())
        {
            try
            {
                File.Delete(file);
            }
            catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
            {
                // Left behind; the folder then stays too.
            }
        }
        try
        {
            // Never a recursive delete here: cache\roblox-client-paths may hold junctions to Roblox's own folder.
            string aliases = Path.Combine(CacheDir, "roblox-client-paths");
            RobloxHomeSession.CleanupAliases(aliases);
            foreach (string dir in new[] { aliases, OldVaultsDir, PicturesDir, CacheDir })
            {
                if (Directory.Exists(dir) && !Directory.EnumerateFileSystemEntries(dir).Any())
                    Directory.Delete(dir);
            }
            if (!Directory.EnumerateFileSystemEntries(DataDir).Any())
                Directory.Delete(DataDir);
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
        }
    }

    /// <summary>The vault file the lock screen opens or sets up (<see cref="Vaults"/>): vault.rmv for the first, vault-&lt;id&gt;.rmv
    /// for each added since. An open vault keeps writing to its own file whatever this becomes.</summary>
    public static string ActiveFile { get; set; } = "vault.rmv";
    public static string VaultPath => Path.Combine(DataDir, ActiveFile);
    static string BackupPath => VaultPath + ".bak";
    static string TempPath => VaultPath + ".tmp";
    static readonly HashSet<string> _restoreChecked = new(StringComparer.OrdinalIgnoreCase);

    /// <summary>Decides between "set up" and "unlock", so a vault lost mid-save is put back first (once per run and file).</summary>
    public static bool Exists
    {
        get
        {
            if (_restoreChecked.Add(ActiveFile))
                RestoreIfMissing();
            return File.Exists(VaultPath);
        }
    }

    /// <summary>The file this vault was opened from or created in, which every save writes.</summary>
    public string FilePath => _path;

    readonly object _sync = new();
    readonly string _path;
    byte[] _key;
    byte[] _salt;
    int _iterations;

    public VaultData Data { get; }

    /// <param name="path">The file it was read from (or made at), taken before the key's slow derivation: the vault chosen
    /// may change meanwhile (the picker during an unlock), and this one must never save over another's file.</param>
    Vault(string path, byte[] key, byte[] salt, int iterations, VaultData data)
    {
        _path = path;
        _key = key;
        _salt = salt;
        _iterations = iterations;
        Data = data;
    }

    /// <summary>The data folder set with BLOXKEEP_DATA (or its old name), or null.</summary>
    static string? DataDirVariable() =>
        (Environment.GetEnvironmentVariable("BLOXKEEP_DATA") ?? Environment.GetEnvironmentVariable("ROBLOXMANAGER_DATA")) is { Length: > 0 } custom ? custom : null;

    static string ResolveDataDir()
    {
        if (DataDirVariable() is { } custom)
            return custom;
        // The portable zip keeps the app in "<folder>\current", which every update replaces; data sits beside it.
        string appDir = Path.TrimEndingDirectorySeparator(AppContext.BaseDirectory);
        if (Path.GetFileName(appDir).Equals("current", StringComparison.OrdinalIgnoreCase)
            && Path.GetDirectoryName(appDir) is { } root && File.Exists(Path.Combine(root, ".portable")))
            return Path.Combine(root, "data");
        string portable = Path.Combine(appDir, "data");
        if (Directory.Exists(portable))
            return portable;
        string local = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
        string dir = Path.Combine(local, "Bloxkeep");
        CopyFromOldName(Path.Combine(local, "RobloxManager"), dir);
        return dir;
    }

    /// <summary>Early builds were called "Roblox Manager"; carry their vault over once (the old copy is left in place).</summary>
    static void CopyFromOldName(string oldDir, string newDir)
    {
        try
        {
            if (File.Exists(Path.Combine(newDir, "vault.rmv")) || !File.Exists(Path.Combine(oldDir, "vault.rmv")))
                return;
            Directory.CreateDirectory(newDir);
            foreach (var name in new[] { "vault.rmv", "vault.rmv.bak", "prefs.json" })
            {
                string source = Path.Combine(oldDir, name);
                if (File.Exists(source))
                    File.Copy(source, Path.Combine(newDir, name), overwrite: false);
            }
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            // Start with an empty vault rather than failing to start.
        }
    }

    static byte[] DeriveKey(string password, byte[] salt, int iterations) =>
        Rfc2898DeriveBytes.Pbkdf2(password, salt, iterations, HashAlgorithmName.SHA256, KeySize);

    /// <param name="path">The file to make; the chosen vault's when not given (take it before going to a worker).</param>
    public static Vault Create(string password, string? path = null)
    {
        path ??= VaultPath;
        byte[] salt = RandomNumberGenerator.GetBytes(SaltSize);
        var vault = new Vault(path, DeriveKey(password, salt, DefaultIterations), salt, DefaultIterations, new VaultData());
        vault.Save();
        return vault;
    }

    /// <summary>Opens vault.rmv, or the previous save (vault.rmv.bak) when the main file is damaged (a power cut, an
    /// antivirus). The damaged file is kept beside the vault, never deleted.</summary>
    /// <exception cref="InvalidPasswordException">The password does not decrypt the file.</exception>
    /// <param name="path">The file to open; the chosen vault's when not given (take it before going to a worker).</param>
    public static Vault Open(string password, string? path = null)
    {
        path ??= VaultPath;
        string backupPath = path + ".bak";
        byte[] file = File.ReadAllBytes(path);
        byte[]? backup = File.Exists(backupPath) ? File.ReadAllBytes(backupPath) : null;
        if (HeaderOk(file, out int iterations, out byte[] salt))
        {
            byte[] key = DeriveKey(password, salt, iterations);
            if (Decrypt(file, key) is { } data)
                return new Vault(path, key, salt, iterations, data);
            // Wrong password, or a damaged file? The previous save was made with the same key (the salt only changes
            // with the password), so if it opens with this key the main file is what's broken.
            if (backup is not null && HeaderOk(backup, out int backupIterations, out byte[] backupSalt))
            {
                bool sameKey = backupIterations == iterations && backupSalt.AsSpan().SequenceEqual(salt);
                if (sameKey && Decrypt(backup, key) is { } previous)
                    return Recovered(path, key, salt, iterations, previous);
                // A main file whose salt or iteration count was damaged still looks like a vault, but no password opens
                // it: the previous save is tried with its own (review, 2026-10-30). Rare, since a password change drops
                // the .bak: a wrong password pays a second derivation only then.
                if (!sameKey)
                {
                    byte[] backupKey = DeriveKey(password, backupSalt, backupIterations);
                    if (Decrypt(backup, backupKey) is { } older)
                    {
                        CryptographicOperations.ZeroMemory(key);
                        return Recovered(path, backupKey, backupSalt, backupIterations, older);
                    }
                    CryptographicOperations.ZeroMemory(backupKey);
                }
            }
            CryptographicOperations.ZeroMemory(key);
            throw new InvalidPasswordException();
        }
        // Not a vault any more (cut short, or overwritten): the previous save is all there is.
        if (backup is not null && HeaderOk(backup, out iterations, out salt))
        {
            byte[] key = DeriveKey(password, salt, iterations);
            if (Decrypt(backup, key) is { } previous)
                return Recovered(path, key, salt, iterations, previous);
            CryptographicOperations.ZeroMemory(key);
            throw new InvalidPasswordException();
        }
        throw new InvalidDataException(L.T("The data file is not in the right format or is damaged."));
    }

    /// <summary>Opens with the key the previous run handed over for an update's restart (<see cref="Relaunch"/>). Only the
    /// main file, and only while it still has that key's salt; otherwise the lock screen asks as usual.</summary>
    /// <exception cref="InvalidPasswordException">The key doesn't open the file.</exception>
    public static Vault OpenWithKey(byte[] key, byte[] salt, int iterations)
    {
        string path = VaultPath;
        byte[] file = File.ReadAllBytes(path);
        if (HeaderOk(file, out int fileIterations, out byte[] fileSalt) && fileIterations == iterations
            && fileSalt.AsSpan().SequenceEqual(salt) && Decrypt(file, key) is { } data)
            return new Vault(path, key, salt, iterations, data);
        CryptographicOperations.ZeroMemory(key);
        throw new InvalidPasswordException();
    }

    /// <summary>A copy of the key, salt and iteration count, for the run that follows an update (see <see cref="Relaunch"/>).</summary>
    public (byte[] Key, byte[] Salt, int Iterations) ExportKey()
    {
        lock (_sync)
        {
            ObjectDisposedException.ThrowIf(_key.Length == 0, this);
            return (_key.ToArray(), _salt.ToArray(), _iterations);
        }
    }

    /// <summary>The file is big enough to hold a vault, starts with the magic and has a sane iteration count.</summary>
    static bool HeaderOk(byte[] file, out int iterations, out byte[] salt)
    {
        iterations = 0;
        salt = [];
        if (file.Length < HeaderSize + NonceSize + TagSize || !file.AsSpan(0, 4).SequenceEqual(Magic))
            return false;
        iterations = BinaryPrimitives.ReadInt32LittleEndian(file.AsSpan(4, 4));
        if (iterations is < 10_000 or > 50_000_000)
            return false;
        salt = file.AsSpan(8, SaltSize).ToArray();
        return true;
    }

    /// <summary>The newest <see cref="VaultData.Version"/> this build understands; a newer one must not be re-saved by it.</summary>
    const int SupportedVersion = 1;

    /// <returns>null when the key does not open the file (wrong password, or the file was changed).</returns>
    /// <exception cref="InvalidDataException">The file opens but was written by a newer Bloxkeep.</exception>
    static VaultData? Decrypt(byte[] file, byte[] key)
    {
        var cipher = file.AsSpan(HeaderSize + NonceSize + TagSize);
        byte[] plain = new byte[cipher.Length];
        try
        {
            using (var aes = new AesGcm(key, TagSize))
            {
                aes.Decrypt(file.AsSpan(HeaderSize, NonceSize), cipher,
                    file.AsSpan(HeaderSize + NonceSize, TagSize), plain, file.AsSpan(0, HeaderSize));
            }
            var data = JsonSerializer.Deserialize<VaultData>(plain) ?? new VaultData();
            if (data.Version > SupportedVersion)
                throw new InvalidDataException(L.T("This data file was made by a newer Bloxkeep. Update Bloxkeep to open it."));
            return data;
        }
        catch (AuthenticationTagMismatchException)
        {
            return null;
        }
        // The password was right (the tag matched): a field changed shape in a newer version.
        catch (JsonException)
        {
            throw new InvalidDataException(L.T("This data file was made by a newer Bloxkeep. Update Bloxkeep to open it."));
        }
        finally
        {
            CryptographicOperations.ZeroMemory(plain);
        }
    }

    /// <summary>Puts the previous save back in place of the damaged main file, which is kept as vault.rmv.damaged-….</summary>
    static Vault Recovered(string path, byte[] key, byte[] salt, int iterations, VaultData data)
    {
        try
        {
            File.Move(path, path + ".damaged-" + Stamp());
            File.Copy(path + ".bak", path);
            Log.Note("vault.rmv was damaged; opened from vault.rmv.bak and kept the damaged file beside it.");
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            Log.Write(ex); // the next save writes vault.rmv afresh
        }
        return new Vault(path, key, salt, iterations, data);
    }

    /// <summary>
    /// A save writes vault.rmv.tmp, then swaps it in and keeps the replaced file as vault.rmv.bak. A swap cut short
    /// (antivirus, power cut) can leave no vault.rmv but .bak/.tmp, and the app would offer a new vault over them.
    /// The .tmp, written in full before the swap, goes back with .bak kept as its fallback (see <see cref="Open"/>);
    /// without a usable .tmp the previous save goes back.
    /// </summary>
    static void RestoreIfMissing()
    {
        try
        {
            if (File.Exists(VaultPath))
                return;
            string? restore = LooksLikeVault(TempPath) ? TempPath : LooksLikeVault(BackupPath) ? BackupPath : null;
            if (restore is null)
                return;
            File.Move(restore, VaultPath);
            // Only what can't serve as the fallback is set aside (a .tmp that isn't a vault, a .bak when .bak went back).
            foreach (string other in new[] { BackupPath, TempPath })
            {
                if (File.Exists(other) && (restore == BackupPath || other != BackupPath))
                    File.Move(other, VaultPath + ".spare-" + Stamp());
            }
            Log.Note($"vault.rmv was missing; put back {Path.GetFileName(restore)}.");
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            Log.Write(ex);
        }
    }

    /// <summary>Starts like a vault file; whether it opens is only known with its password (see <see cref="Backup"/>).</summary>
    public static bool LooksLikeVault(string path) =>
        File.Exists(path) && File.ReadAllBytes(path) is var bytes && bytes.Length >= MinVaultSize && HeaderOk(bytes, out _, out _);

    /// <summary>Below any vault ever written (an empty one is about 1.7 KB, its settings alone hundreds of bytes): a file this
    /// small with the right first bytes was cut short, and restoring it made every unlock fail (review, 2026-10-30).</summary>
    const int MinVaultSize = 256;

    /// <summary>What <see cref="ReadOther"/> found in a file.</summary>
    public enum OtherStatus { Opened, TooLarge, Unreadable, NotAVault, WrongPassword, Newer }

    /// <summary>Far above any real vault (a thousand accounts are well under 5 MB); a bigger file isn't read into memory.</summary>
    public const long MaxOtherSize = 50L * 1024 * 1024;

    /// <summary>
    /// Opens another vault file (from another PC, or a backup) with its own master password, to bring accounts over.
    /// Read-only: neither that file nor this vault's key and files are touched. The derived key and the decrypted JSON
    /// are wiped before returning (see <see cref="Decrypt"/>). Slow (key stretching): call it off the UI thread.
    /// </summary>
    /// <returns>The data when <see cref="OtherStatus.Opened"/>; null otherwise.</returns>
    public static (OtherStatus Status, VaultData? Data) ReadOther(string path, string password)
    {
        byte[] file;
        try
        {
            if (new FileInfo(path).Length > MaxOtherSize)
                return (OtherStatus.TooLarge, null);
            file = File.ReadAllBytes(path);
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or System.Security.SecurityException)
        {
            Log.Write(ex);
            return (OtherStatus.Unreadable, null);
        }
        if (!HeaderOk(file, out int iterations, out byte[] salt))
            return (OtherStatus.NotAVault, null);
        byte[] key = DeriveKey(password, salt, iterations);
        try
        {
            return Decrypt(file, key) is { } data ? (OtherStatus.Opened, data) : (OtherStatus.WrongPassword, null);
        }
        catch (InvalidDataException)
        {
            return (OtherStatus.Newer, null);
        }
        finally
        {
            CryptographicOperations.ZeroMemory(key);
        }
    }

    /// <summary>Puts other vault bytes (a backup) in place of vault.rmv, with the same safe swap as a save.</summary>
    public static void ReplaceFile(byte[] bytes)
    {
        lock (FileGate)
        {
            WriteAtomic(VaultPath, bytes);
            // The replaced vault, left as the spare by the write: with its password (the usual one) Open would take the
            // restored file for damaged and put this one back. A copy of it is in old-vaults.
            try
            {
                File.Delete(VaultPath + ".bak");
            }
            catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
            {
                Log.Note($"Restore: the replaced vault's spare copy couldn't be removed ({ex.GetType().Name}).");
            }
        }
    }

    /// <summary>
    /// A vault taken off this PC's list (the lock screen's picker): its file and spare copies are moved aside as
    /// old-vaultsault-removed-&lt;time&gt;.rmv, never deleted (it is restored like a backup), so nothing brings it back.
    /// </summary>
    /// <returns>The file the vault now is.</returns>
    public static string SetAside(string file)
    {
        lock (FileGate)
        {
            Directory.CreateDirectory(OldVaultsDir);
            string path = Path.Combine(DataDir, file), stamp = Stamp(), kept = Path.Combine(OldVaultsDir, $"vault-removed-{stamp}.rmv");
            File.Move(path, kept);
            foreach (var (from, what) in new[] { (path + ".bak", "spare"), (path + ".tmp", "unsaved") })
            {
                if (File.Exists(from))
                    File.Move(from, Path.Combine(OldVaultsDir, $"vault-removed-{stamp}-{what}.rmv"));
            }
            return kept;
        }
    }

    static string Stamp() => DateTime.Now.ToString("yyyyMMdd-HHmmss");

    /// <summary>Writes the vault now (lock, exit, a changed password: callers that need it on disk before going on).</summary>
    public void Save()
    {
        lock (_sync)
        {
            ObjectDisposedException.ThrowIf(_key.Length == 0, this);
            Write(_key, _salt, _iterations);
        }
    }

    /// <summary>One write to the vault file at a time: a save in the background and one made now never interleave.</summary>
    static readonly object FileGate = new();
    /// <summary>A background save waiting for the worker: the vault as it was then, and the key it goes under.</summary>
    sealed record Pending(long Number, byte[] Plain, byte[] Key, byte[] Salt, int Iterations, string Path);
    readonly object _queueGate = new();
    Pending? _queued;
    Task _background = Task.CompletedTask;
    /// <summary>Saves are numbered as they take the vault (under _sync); the file gate keeps the newest written. A background
    /// save that took the vault before a save made now but reaches the file after it is left out, so the file never goes
    /// back to an older vault, nor to an old password.</summary>
    long _taken, _written;

    /// <summary>Test hook (Tests/Bloxkeep.Tests): a background save calls it between taking the vault and reaching the
    /// file, as a busy PC may pause there. The app never sets it.</summary>
    internal static Action? BeforeBackgroundWrite { get; set; }

    /// <summary>Takes the vault now (on the caller's thread, so nothing changes under it) and encrypts, writes and flushes
    /// it on a worker. One write at a time; a newer save replaces one still waiting. The task ends once this vault (or
    /// newer) is on disk, faulted when that write failed (it is then tried again with the next save).</summary>
    public Task SaveInBackground()
    {
        Pending next;
        lock (_sync)
        {
            ObjectDisposedException.ThrowIf(_key.Length == 0, this);
            next = new Pending(++_taken, JsonSerializer.SerializeToUtf8Bytes(Data), _key.ToArray(), _salt.ToArray(), _iterations, _path);
        }
        lock (_queueGate)
        {
            Forget(_queued);
            _queued = next;
            return _background = _background.ContinueWith(_ => WriteQueued(), CancellationToken.None, TaskContinuationOptions.None, TaskScheduler.Default);
        }
    }

    void WriteQueued()
    {
        Pending? p;
        lock (_queueGate)
        {
            p = _queued;
            _queued = null;
        }
        if (p is null)
            return; // written already, by the save before this one or by one made now
        BeforeBackgroundWrite?.Invoke();
        try
        {
            lock (FileGate)
            {
                if (p.Number < _written)
                    return; // a newer save took the file first: one made now, while this was on its way
                WriteEncrypted(p.Plain, p.Key, p.Salt, p.Iterations, p.Path);
                _written = p.Number;
            }
        }
        catch
        {
            // Kept for the next save to try again, unless a newer one is already waiting.
            lock (_queueGate)
            {
                if (_queued is null)
                {
                    _queued = p;
                    p = null;
                }
            }
            throw;
        }
        finally
        {
            Forget(p);
        }
    }

    /// <summary>A save waiting in the background goes: what is written now is newer (and may be under a new password).</summary>
    void DropQueued()
    {
        lock (_queueGate)
        {
            Forget(_queued);
            _queued = null;
        }
    }

    static void Forget(Pending? p)
    {
        if (p is null)
            return;
        CryptographicOperations.ZeroMemory(p.Plain);
        CryptographicOperations.ZeroMemory(p.Key);
    }

    /// <summary>A background save is waiting or being written.</summary>
    public bool SavingInBackground
    {
        get
        {
            lock (_queueGate)
                return !_background.IsCompleted;
        }
    }

    /// <summary>Waits for a background save still being written (the vault is about to close: lock, exit, an update).</summary>
    public void WaitForSaves(TimeSpan timeout)
    {
        Task pending;
        lock (_queueGate)
            pending = _background;
        try
        {
            pending.Wait(timeout);
        }
        catch (AggregateException)
        {
            // Its failure was reported to whoever asked for that save.
        }
    }

    void Write(byte[] key, byte[] salt, int iterations)
    {
        byte[] plain = JsonSerializer.SerializeToUtf8Bytes(Data);
        long number = ++_taken; // callers hold _sync
        try
        {
            lock (FileGate)
            {
                DropQueued(); // older than this, and possibly under the old password (ChangePassword)
                WriteEncrypted(plain, key, salt, iterations, _path);
                _written = number;
            }
        }
        finally
        {
            CryptographicOperations.ZeroMemory(plain);
        }
    }

    static void WriteEncrypted(byte[] plain, byte[] key, byte[] salt, int iterations, string path)
    {
        byte[] file = new byte[HeaderSize + NonceSize + TagSize + plain.Length];
        Magic.CopyTo(file, 0);
        BinaryPrimitives.WriteInt32LittleEndian(file.AsSpan(4, 4), iterations);
        salt.CopyTo(file, 8);
        var nonce = file.AsSpan(HeaderSize, NonceSize);
        RandomNumberGenerator.Fill(nonce);
        using (var aes = new AesGcm(key, TagSize))
        {
            aes.Encrypt(nonce, plain, file.AsSpan(HeaderSize + NonceSize + TagSize),
                file.AsSpan(HeaderSize + NonceSize, TagSize), file.AsSpan(0, HeaderSize));
        }
        WriteAtomic(path, file);
    }

    /// <summary>Writes to a temp file first so a crash mid-write never leaves a half-written vault.</summary>
    static void WriteAtomic(string path, byte[] bytes)
    {
        Directory.CreateDirectory(DataDir);
        string tmp = path + ".tmp";
        using (var fs = new FileStream(tmp, FileMode.Create, FileAccess.Write, FileShare.None))
        {
            fs.Write(bytes);
            fs.Flush(flushToDisk: true);
        }
        if (File.Exists(path))
            File.Replace(tmp, path, path + ".bak", ignoreMetadataErrors: true);
        else
            File.Move(tmp, path);
    }

    public bool VerifyPassword(string password)
    {
        byte[] salt, key;
        int iterations;
        lock (_sync)
        {
            salt = _salt.ToArray();
            key = _key.ToArray();
            iterations = _iterations;
        }
        byte[] candidate = DeriveKey(password, salt, iterations);
        try
        {
            return CryptographicOperations.FixedTimeEquals(candidate, key);
        }
        finally
        {
            CryptographicOperations.ZeroMemory(candidate);
            CryptographicOperations.ZeroMemory(key);
        }
    }

    /// <summary>The slow part of a password change (key stretching), safe to run off the UI thread.</summary>
    public static (byte[] Key, byte[] Salt) PrepareKey(string newPassword)
    {
        byte[] salt = RandomNumberGenerator.GetBytes(SaltSize);
        return (DeriveKey(newPassword, salt, DefaultIterations), salt);
    }

    /// <summary>Writes the vault under the new password and only then switches to it: if the write fails (antivirus lock,
    /// disk full) the old password keeps working, on disk and for later saves.</summary>
    public void ChangePassword((byte[] Key, byte[] Salt) prepared)
    {
        lock (_sync)
        {
            ObjectDisposedException.ThrowIf(_key.Length == 0, this);
            Write(prepared.Key, prepared.Salt, DefaultIterations);
            CryptographicOperations.ZeroMemory(_key);
            _key = prepared.Key;
            _salt = prepared.Salt;
            _iterations = DefaultIterations;
        }
        // The previous save is still encrypted with the old password; drop it so it doesn't open with that one.
        // (Older copies — backups in the backup folder, .spare/.damaged/before-restore files — still do; they are
        // the person's to delete.) The change itself has succeeded by now, so a file that won't go is only logged.
        try
        {
            File.Delete(_path + ".bak"); // this vault's, whichever is chosen now
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            Log.Write(ex);
        }
    }

    /// <summary>A short id from the header's salt (no password needed); null when not a vault. Backups carry it in their
    /// name, so vaults sharing a folder don't prune each other's.</summary>
    public static string? IdOf(byte[] file) =>
        HeaderOk(file, out _, out byte[] salt) ? IdOfSalt(salt) : null;

    static string IdOfSalt(byte[] salt) => Convert.ToHexString(SHA256.HashData(salt))[..8].ToLowerInvariant();

    /// <summary>This vault's own id, as its backups carry it; it changes with the password (a new salt).</summary>
    public string Id
    {
        get
        {
            lock (_sync)
                return IdOfSalt(_salt);
        }
    }

    /// <summary>The same id from a file on disk, reading its header only; null when unreadable or not a vault.</summary>
    public static string? IdOf(string path)
    {
        try
        {
            using var stream = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete);
            byte[] header = new byte[HeaderSize + NonceSize + TagSize];
            return stream.ReadAtLeast(header, header.Length, throwOnEndOfStream: false) == header.Length ? IdOf(header) : null;
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            return null;
        }
    }

    public void Dispose()
    {
        // A save still on its way to disk is finished first: closing (lock, exit, an update's restart) mustn't lose it.
        WaitForSaves(TimeSpan.FromSeconds(10));
        DropQueued();
        lock (_sync)
        {
            CryptographicOperations.ZeroMemory(_key);
            _key = [];
        }
    }
}
