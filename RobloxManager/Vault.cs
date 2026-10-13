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

    /// <summary>
    /// %LOCALAPPDATA%\Bloxkeep by default; the installed program lives apart in %LOCALAPPDATA%\BloxkeepApp, which
    /// uninstalling deletes, so accounts survive it. The portable zip, or a "data" folder next to the exe, keeps
    /// data beside the app instead; BLOXKEEP_DATA overrides all of these.
    /// </summary>
    public static string DataDir { get; } = ResolveDataDir();

    /// <summary>Removes the interface's browser caches (rebuilt when needed). Accounts and settings stay.</summary>
    public static void DeleteCaches()
    {
        foreach (string name in new[] { "webview", "webview-safe" })
        {
            string dir = Path.Combine(DataDir, name);
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
    public static string VaultPath => Path.Combine(DataDir, "vault.rmv");
    static string BackupPath => Path.Combine(DataDir, "vault.rmv.bak");
    static string TempPath => Path.Combine(DataDir, "vault.rmv.tmp");
    static bool _restoreChecked;

    /// <summary>Decides between "set up" and "unlock", so a vault lost mid-save is put back first (once per run).</summary>
    public static bool Exists
    {
        get
        {
            if (!_restoreChecked)
            {
                _restoreChecked = true;
                RestoreIfMissing();
            }
            return File.Exists(VaultPath);
        }
    }

    readonly object _sync = new();
    byte[] _key;
    byte[] _salt;
    int _iterations;

    public VaultData Data { get; }

    Vault(byte[] key, byte[] salt, int iterations, VaultData data)
    {
        _key = key;
        _salt = salt;
        _iterations = iterations;
        Data = data;
    }

    static string ResolveDataDir()
    {
        if ((Environment.GetEnvironmentVariable("BLOXKEEP_DATA") ?? Environment.GetEnvironmentVariable("ROBLOXMANAGER_DATA")) is { Length: > 0 } custom)
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

    public static Vault Create(string password)
    {
        byte[] salt = RandomNumberGenerator.GetBytes(SaltSize);
        var vault = new Vault(DeriveKey(password, salt, DefaultIterations), salt, DefaultIterations, new VaultData());
        vault.Save();
        return vault;
    }

    /// <summary>
    /// Opens vault.rmv, or the previous save (vault.rmv.bak) when the main file turns out damaged: cut short by a
    /// power cut, or mangled by an antivirus. The damaged file is kept beside the vault, never deleted.
    /// </summary>
    /// <exception cref="InvalidPasswordException">The password does not decrypt the file.</exception>
    public static Vault Open(string password)
    {
        byte[] file = File.ReadAllBytes(VaultPath);
        byte[]? backup = File.Exists(BackupPath) ? File.ReadAllBytes(BackupPath) : null;
        if (HeaderOk(file, out int iterations, out byte[] salt))
        {
            byte[] key = DeriveKey(password, salt, iterations);
            if (Decrypt(file, key) is { } data)
                return new Vault(key, salt, iterations, data);
            // Wrong password, or a damaged file? The previous save was made with the same key (the salt only changes
            // with the password), so if it opens with this key the main file is what's broken.
            if (backup is not null && HeaderOk(backup, out int backupIterations, out byte[] backupSalt)
                && backupIterations == iterations && backupSalt.AsSpan().SequenceEqual(salt)
                && Decrypt(backup, key) is { } previous)
                return Recovered(key, salt, iterations, previous);
            CryptographicOperations.ZeroMemory(key);
            throw new InvalidPasswordException();
        }
        // Not a vault any more (cut short, or overwritten): the previous save is all there is.
        if (backup is not null && HeaderOk(backup, out iterations, out salt))
        {
            byte[] key = DeriveKey(password, salt, iterations);
            if (Decrypt(backup, key) is { } previous)
                return Recovered(key, salt, iterations, previous);
            CryptographicOperations.ZeroMemory(key);
            throw new InvalidPasswordException();
        }
        throw new InvalidDataException(L.T("The data file is not in the right format or is damaged."));
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

    /// <returns>null when the key does not open the file (wrong password, or the file was changed).</returns>
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
            return JsonSerializer.Deserialize<VaultData>(plain) ?? new VaultData();
        }
        catch (AuthenticationTagMismatchException)
        {
            return null;
        }
        finally
        {
            CryptographicOperations.ZeroMemory(plain);
        }
    }

    /// <summary>Puts the previous save back in place of the damaged main file, which is kept as vault.rmv.damaged-….</summary>
    static Vault Recovered(byte[] key, byte[] salt, int iterations, VaultData data)
    {
        try
        {
            File.Move(VaultPath, VaultPath + ".damaged-" + Stamp());
            File.Copy(BackupPath, VaultPath);
            Log.Note("vault.rmv was damaged; opened from vault.rmv.bak and kept the damaged file beside it.");
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            Log.Write(ex); // the next save writes vault.rmv afresh
        }
        return new Vault(key, salt, iterations, data);
    }

    /// <summary>
    /// A save writes vault.rmv.tmp, then swaps it in and keeps the file it replaced as vault.rmv.bak. When that swap
    /// is cut short (antivirus holding the file, power cut) vault.rmv can be gone while .bak/.tmp survive, and the app
    /// would offer to set up a new vault over them. The previous save is complete for sure, so it goes back; .tmp may
    /// be cut short and can only be checked by decrypting, so it is kept beside the vault, never trusted or deleted.
    /// </summary>
    static void RestoreIfMissing()
    {
        try
        {
            if (File.Exists(VaultPath))
                return;
            string? restore = LooksLikeVault(BackupPath) ? BackupPath : LooksLikeVault(TempPath) ? TempPath : null;
            if (restore is null)
                return;
            File.Move(restore, VaultPath);
            foreach (string other in new[] { BackupPath, TempPath })
            {
                if (File.Exists(other))
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
    public static bool LooksLikeVault(string path) => File.Exists(path) && HeaderOk(File.ReadAllBytes(path), out _, out _);

    /// <summary>Puts other vault bytes (a backup) in place of vault.rmv, with the same safe swap as a save.</summary>
    public static void ReplaceFile(byte[] bytes) => WriteAtomic(bytes);

    static string Stamp() => DateTime.Now.ToString("yyyyMMdd-HHmmss");

    public void Save()
    {
        lock (_sync)
        {
            ObjectDisposedException.ThrowIf(_key.Length == 0, this);
            Write(_key, _salt, _iterations);
        }
    }

    void Write(byte[] key, byte[] salt, int iterations)
    {
        {
            byte[] plain = JsonSerializer.SerializeToUtf8Bytes(Data);
            try
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
                WriteAtomic(file);
            }
            finally
            {
                CryptographicOperations.ZeroMemory(plain);
            }
        }
    }

    /// <summary>Writes to a temp file first so a crash mid-write never leaves a half-written vault.</summary>
    static void WriteAtomic(byte[] bytes)
    {
        Directory.CreateDirectory(DataDir);
        string tmp = TempPath;
        using (var fs = new FileStream(tmp, FileMode.Create, FileAccess.Write, FileShare.None))
        {
            fs.Write(bytes);
            fs.Flush(flushToDisk: true);
        }
        if (File.Exists(VaultPath))
            File.Replace(tmp, VaultPath, BackupPath, ignoreMetadataErrors: true);
        else
            File.Move(tmp, VaultPath);
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

    /// <summary>
    /// Writes the vault under the new password and only then switches to it: if the write fails (file locked by an
    /// antivirus, disk full) the old password keeps working, both on disk and for later saves.
    /// </summary>
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
        // The backup is still encrypted with the old password; drop it so the old password stops working.
        File.Delete(BackupPath);
    }

    public void Dispose()
    {
        lock (_sync)
        {
            CryptographicOperations.ZeroMemory(_key);
            _key = [];
        }
    }
}
