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
    public static bool Exists => File.Exists(VaultPath);

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

    /// <exception cref="InvalidPasswordException">The password does not decrypt the file.</exception>
    public static Vault Open(string password)
    {
        byte[] file = File.ReadAllBytes(VaultPath);
        if (file.Length < HeaderSize + NonceSize + TagSize || !file.AsSpan(0, 4).SequenceEqual(Magic))
            throw new InvalidDataException(L.T("The data file is not in the right format or is damaged."));

        int iterations = BinaryPrimitives.ReadInt32LittleEndian(file.AsSpan(4, 4));
        if (iterations is < 10_000 or > 50_000_000)
            throw new InvalidDataException(L.T("The data file is not in the right format or is damaged."));

        byte[] salt = file.AsSpan(8, SaltSize).ToArray();
        byte[] key = DeriveKey(password, salt, iterations);
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
            return new Vault(key, salt, iterations, data);
        }
        catch (AuthenticationTagMismatchException)
        {
            CryptographicOperations.ZeroMemory(key);
            throw new InvalidPasswordException();
        }
        finally
        {
            CryptographicOperations.ZeroMemory(plain);
        }
    }

    public void Save()
    {
        lock (_sync)
        {
            ObjectDisposedException.ThrowIf(_key.Length == 0, this);
            byte[] plain = JsonSerializer.SerializeToUtf8Bytes(Data);
            try
            {
                byte[] file = new byte[HeaderSize + NonceSize + TagSize + plain.Length];
                Magic.CopyTo(file, 0);
                BinaryPrimitives.WriteInt32LittleEndian(file.AsSpan(4, 4), _iterations);
                _salt.CopyTo(file, 8);
                var nonce = file.AsSpan(HeaderSize, NonceSize);
                RandomNumberGenerator.Fill(nonce);
                using (var aes = new AesGcm(_key, TagSize))
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
        string tmp = VaultPath + ".tmp";
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

    public void ChangePassword(string newPassword)
    {
        byte[] salt = RandomNumberGenerator.GetBytes(SaltSize);
        byte[] key = DeriveKey(newPassword, salt, DefaultIterations);
        lock (_sync)
        {
            CryptographicOperations.ZeroMemory(_key);
            _key = key;
            _salt = salt;
            _iterations = DefaultIterations;
        }
        Save();
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
