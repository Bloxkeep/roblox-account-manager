using System.ComponentModel;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text;
using Microsoft.Win32.SafeHandles;

namespace RobloxManager;

/// <summary>
/// Ticket-free Roblox home screens. A directory junction gives each start its own path-based .shm/.mtx names,
/// without copying or modifying the client. Exclusive handles keep both shared sign-in files unreadable and
/// unwritable: each new client signs in in memory instead of adopting or overwriting another client's session.
/// Keep this alive while clients using these aliases run, including while Bloxkeep is locked or Multi-Roblox is switched off.
/// Verified with Roblox 0.743.0.7431134: signing in and out in the new window leaves the old window signed in.
/// </summary>
sealed class RobloxHomeSession : IDisposable
{
    readonly List<FileStream> _files = [];
    readonly string _aliases;
    bool _disposed;
    readonly Dictionary<(int Pid, long Created), bool> _clientPaths = [];

    public RobloxHomeSession(string storage, string aliases)
    {
        _aliases = Path.TrimEndingDirectorySeparator(Path.GetFullPath(aliases));
        Directory.CreateDirectory(storage);
        try
        {
            foreach (string name in new[] { "RobloxCookies.dat", "appStorage.json" })
                _files.Add(LockFile(Path.Combine(storage, name)));
        }
        catch
        {
            Dispose(); // If the second file is busy, never leave just the first one locked.
            throw;
        }
    }

    static FileStream LockFile(string path)
    {
        for (int attempt = 0; ; attempt++)
        {
            try
            {
                return new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.None);
            }
            catch (FileNotFoundException)
            {
                try
                {
                    // Only our own empty placeholder disappears, atomically when its handle closes (even on a crash).
                    return new FileStream(path, FileMode.CreateNew, FileAccess.ReadWrite, FileShare.None, 4096, FileOptions.DeleteOnClose);
                }
                catch (IOException) when (attempt < 2 && File.Exists(path))
                {
                    // Someone created the file between Open and CreateNew: open it normally, never truncate it.
                }
            }
        }
    }

    /// <summary>Only a client executable directly inside one of our GUID-named junctions belongs to this session.</summary>
    public bool OwnsClientPath(string path)
    {
        try
        {
            if (!Path.IsPathFullyQualified(path) || !Path.GetFileName(path).Equals("RobloxPlayerBeta.exe", StringComparison.OrdinalIgnoreCase))
                return false;
            var parent = new DirectoryInfo(Path.GetDirectoryName(Path.GetFullPath(path))!);
            return parent.Parent?.FullName.Equals(_aliases, StringComparison.OrdinalIgnoreCase) == true
                && parent.Name.Length == 39 && parent.Name.StartsWith("client-", StringComparison.OrdinalIgnoreCase)
                && Guid.TryParseExact(parent.Name[7..], "N", out _);
        }
        catch (Exception ex) when (ex is ArgumentException or NotSupportedException or IOException)
        {
            return false;
        }
    }

    /// <summary>
    /// Query each process identity once; a reused PID is a new identity. Failure to inspect an unknown client keeps
    /// the locks held until its path can be confirmed or it exits, rather than risking an early unlock.
    /// </summary>
    public (int[] Pids, bool Uncertain) FindClients(IReadOnlyList<RobloxProcesses.PlayerProcess> players, Func<int, long, string?>? query = null)
    {
        query ??= RobloxProcesses.LaunchPath;
        var alive = players.Select(p => (p.Pid, p.Created)).ToHashSet();
        foreach (var key in _clientPaths.Keys.Where(k => !alive.Contains(k)).ToArray())
            _clientPaths.Remove(key);
        var ours = new List<int>();
        bool uncertain = false;
        foreach (var player in players)
        {
            var key = (player.Pid, player.Created);
            if (player.Created == 0 || !_clientPaths.TryGetValue(key, out bool own))
            {
                string? path = query(player.Pid, player.Created);
                if (path is null)
                {
                    uncertain = true;
                    continue;
                }
                own = OwnsClientPath(path);
                if (player.Created != 0)
                    _clientPaths[key] = own;
            }
            if (own)
                ours.Add(player.Pid);
        }
        return (ours.ToArray(), uncertain);
    }

    public ProcessStartInfo CreateStartInfo(string client)
    {
        ObjectDisposedException.ThrowIf(_disposed, this);
        client = Path.GetFullPath(client);
        if (!File.Exists(client) || !Path.GetFileName(client).Equals("RobloxPlayerBeta.exe", StringComparison.OrdinalIgnoreCase))
            throw new FileNotFoundException("RobloxPlayerBeta.exe was not found.", client);
        string target = Path.GetDirectoryName(client)!;
        if (target.StartsWith(@"\\", StringComparison.Ordinal))
            throw new IOException("A Roblox home screen needs a client installed on a local drive.");
        Directory.CreateDirectory(_aliases);
        string alias = Path.Combine(_aliases, "client-" + Guid.NewGuid().ToString("N"));
        CreateJunction(alias, target);
        return new ProcessStartInfo(Path.Combine(alias, "RobloxPlayerBeta.exe"))
        {
            UseShellExecute = false,
            WorkingDirectory = alias,
        }; // No ticket, game link, or environment override; Roblox handles the sign-in itself.
    }

    public void Start(string client)
    {
        var info = CreateStartInfo(client);
        try
        {
            using var process = Process.Start(info) ?? throw new IOException("Roblox did not start.");
            try
            {
                _clientPaths[(process.Id, process.StartTime.ToFileTimeUtc())] = true;
            }
            catch (Exception ex) when (ex is Win32Exception or InvalidOperationException)
            {
                // The snapshot and launch-path query will identify it if it is still running.
            }
        }
        catch
        {
            DeleteAlias(new DirectoryInfo(info.WorkingDirectory));
            throw;
        }
    }

    /// <summary>Call only when no clients using these aliases are running. Delete junctions themselves, never their targets.</summary>
    public static void CleanupAliases(string root)
    {
        if (!Directory.Exists(root))
            return;
        foreach (var dir in new DirectoryInfo(root).EnumerateDirectories("client-*"))
        {
            if (dir.Name.Length == 39 && Guid.TryParseExact(dir.Name[7..], "N", out _))
                DeleteAlias(dir);
        }
    }

    static void DeleteAlias(DirectoryInfo dir)
    {
        try
        {
            if ((dir.Attributes & FileAttributes.ReparsePoint) != 0)
                dir.Delete(); // Non-recursive: removing a junction must never walk the Roblox installation.
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            // An alias still in use can wait for the next time no clients are open.
        }
    }

    static void CreateJunction(string alias, string target)
    {
        byte[] substitute = Encoding.Unicode.GetBytes(@"\??\" + target);
        byte[] print = Encoding.Unicode.GetBytes(target);
        int dataLength = checked(8 + substitute.Length + 2 + print.Length + 2);
        if (dataLength > ushort.MaxValue)
            throw new PathTooLongException();
        byte[] data = new byte[8 + dataLength];
        BitConverter.TryWriteBytes(data.AsSpan(0), 0xA0000003u); // IO_REPARSE_TAG_MOUNT_POINT
        BitConverter.TryWriteBytes(data.AsSpan(4), (ushort)dataLength);
        BitConverter.TryWriteBytes(data.AsSpan(10), (ushort)substitute.Length);
        BitConverter.TryWriteBytes(data.AsSpan(12), (ushort)(substitute.Length + 2));
        BitConverter.TryWriteBytes(data.AsSpan(14), (ushort)print.Length);
        substitute.CopyTo(data, 16);
        print.CopyTo(data, 16 + substitute.Length + 2);
        Directory.CreateDirectory(alias);
        try
        {
            using var handle = CreateFile(alias, 0x40000000, 7, IntPtr.Zero, 3, 0x02200000, IntPtr.Zero);
            if (handle.IsInvalid || !DeviceIoControl(handle, 0x000900A4, data, data.Length, IntPtr.Zero, 0, out _, IntPtr.Zero))
                throw new Win32Exception(Marshal.GetLastWin32Error());
        }
        catch
        {
            Directory.Delete(alias); // A fresh, empty directory or the junction itself; never recurse.
            throw;
        }
    }

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    static extern SafeFileHandle CreateFile(string path, uint access, uint share, IntPtr security, uint creation, uint flags, IntPtr template);

    [DllImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    static extern bool DeviceIoControl(SafeFileHandle file, uint code, byte[] input, int inputSize, IntPtr output, int outputSize, out int returned, IntPtr overlapped);

    public void Dispose()
    {
        if (_disposed)
            return;
        _disposed = true;
        foreach (var file in _files)
            file.Dispose();
        _files.Clear();
        _clientPaths.Clear();
        // Paths stay usable if someone explicitly quits Bloxkeep before closing their clients.
    }
}