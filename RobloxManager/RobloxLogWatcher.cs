using System.Globalization;
using System.Runtime.InteropServices;
using System.Runtime.InteropServices.ComTypes;
using System.Text;
using System.Text.RegularExpressions;

namespace RobloxManager;

/// <summary>
/// Follows Roblox's own client logs (%LOCALAPPDATA%\Roblox\logs — every client writes one, whichever launcher started
/// it) to know within a second or two when a window joins a server, changes server, leaves, or is kicked, without
/// asking Roblox's servers. Each log is tied to its window through Windows' Restart Manager (which process has the
/// file open). The log format is not an official API, so Roblox's presence stays as the fallback.
/// </summary>
public sealed class RobloxLogWatcher
{
    /// <param name="Status">joining, ingame, left (back in the app, by choice) or disconnected.</param>
    /// <param name="Reason">Roblox's disconnect code when it gave one (278 = idle for 20 minutes).</param>
    public sealed record ClientState(string Status, long? PlaceId, string? JobId, int? Reason, DateTime At);

    sealed class Tail(string path, int pid)
    {
        public string Path { get; } = path;
        public int Pid { get; } = pid;
        public long Offset;
        public string Partial = "";
        public ClientState? State;
        public bool Leaving;
        /// <summary>When the last teleport line was logged; only a disconnect soon after it belongs to that teleport.</summary>
        public DateTime? TeleportAt;
        public int? PendingReason;
    }

    static readonly string LogDir = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Roblox", "logs");
    // Lines seen in Roblox 0.742 logs.
    static readonly Regex JoiningLine = new(@"\[FLog::Output\] ! Joining game '([0-9a-fA-F-]+)' place (\d+)", RegexOptions.Compiled | RegexOptions.CultureInvariant);
    static readonly Regex ReasonLine = new(@"setting replicator disconnect reason to (\d+)", RegexOptions.Compiled | RegexOptions.CultureInvariant);
    const string ConnectedMark = "[FLog::Network] serverId:";
    const string LeavingMark = "[FLog::SingleSurfaceApp] leaveUGCGameInternal";
    const string DisconnectedMark = "[FLog::Network] Time to disconnect replication data";
    // A game moving the player to another of its places (seen 2026-10-13): doTeleport, then the same disconnect line
    // as a kick, then "Teleported." and a new "Joining game" a fraction of a second later.
    const string TeleportMark = "UgcExperienceController: doTeleport";
    // A teleport that never got that far (it failed) must not turn a kick minutes later into "joining".
    static readonly TimeSpan TeleportWindow = TimeSpan.FromSeconds(30);
    const long MaxRead = 4 * 1024 * 1024;

    readonly Dictionary<int, Tail> _tails = new();
    readonly HashSet<string> _foreign = new(StringComparer.OrdinalIgnoreCase);
    DateTime _nextScan;

    public ClientState? StateOf(int pid) => _tails.GetValueOrDefault(pid)?.State;

    /// <summary>Reads what the clients with these process ids logged since the last call; true when a state changed.</summary>
    public bool Poll(IReadOnlyCollection<int> pids)
    {
        foreach (int gone in _tails.Keys.Where(pid => !pids.Contains(pid)).ToList())
            _tails.Remove(gone);
        if (pids.Count == 0)
            return false;
        if (pids.Any(pid => !_tails.ContainsKey(pid)) && DateTime.UtcNow >= _nextScan)
        {
            _nextScan = DateTime.UtcNow + TimeSpan.FromSeconds(3);
            FindLogs(pids);
        }
        bool changed = false;
        foreach (var tail in _tails.Values)
            changed |= Read(tail);
        return changed;
    }

    /// <summary>A new client opens a new log; ask Windows which of the recent logs each client holds open.</summary>
    void FindLogs(IReadOnlyCollection<int> pids)
    {
        IEnumerable<FileInfo> logs;
        try
        {
            logs = new DirectoryInfo(LogDir).Exists
                ? new DirectoryInfo(LogDir).EnumerateFiles("*_Player_*.log")
                    .Where(f => !f.Name.Contains("CrashHandler", StringComparison.OrdinalIgnoreCase) && f.LastWriteTimeUtc > DateTime.UtcNow.AddDays(-2))
                    .OrderByDescending(f => f.LastWriteTimeUtc)
                    .Take(40)
                    .ToList()
                : [];
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            return;
        }
        var followed = _tails.Values.Select(t => t.Path).ToHashSet(StringComparer.OrdinalIgnoreCase);
        foreach (var log in logs)
        {
            if (followed.Contains(log.FullName) || _foreign.Contains(log.FullName))
                continue;
            int? owner = OpenedBy(log.FullName).Cast<int?>().FirstOrDefault(pid => pids.Contains(pid!.Value));
            if (owner is int pid)
                _tails[pid] = new Tail(log.FullName, pid) { Offset = Math.Max(0, log.Length - MaxRead) };
            else if (log.LastWriteTimeUtc < DateTime.UtcNow.AddMinutes(-2))
                _foreign.Add(log.FullName); // an old log, or another program's client: don't ask again
            if (pids.All(_tails.ContainsKey))
                break;
        }
    }

    bool Read(Tail tail)
    {
        string text;
        try
        {
            using var stream = new FileStream(tail.Path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete);
            if (stream.Length < tail.Offset)
                tail.Offset = 0;
            if (stream.Length == tail.Offset)
                return false;
            stream.Seek(tail.Offset, SeekOrigin.Begin);
            var buffer = new byte[Math.Min(MaxRead, stream.Length - tail.Offset)];
            int read = stream.ReadAtLeast(buffer, buffer.Length, throwOnEndOfStream: false);
            tail.Offset += read;
            text = tail.Partial + Encoding.UTF8.GetString(buffer, 0, read);
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            return false;
        }
        int end = text.LastIndexOf('\n');
        tail.Partial = end < 0 ? text : text[(end + 1)..];
        if (end < 0)
            return false;

        var before = tail.State;
        foreach (var line in text.AsSpan(0, end).EnumerateLines())
            Apply(tail, line.ToString());
        return tail.State != before;
    }

    static void Apply(Tail tail, string line)
    {
        if (line.Contains("! Joining game", StringComparison.Ordinal) && JoiningLine.Match(line) is { Success: true } join)
        {
            tail.Leaving = false;
            tail.TeleportAt = null;
            tail.PendingReason = null;
            tail.State = new ClientState("joining", long.Parse(join.Groups[2].Value, CultureInfo.InvariantCulture), join.Groups[1].Value, null, TimeOf(line));
        }
        else if (line.Contains(ConnectedMark, StringComparison.Ordinal) && tail.State is { Status: "joining" } joining)
            tail.State = joining with { Status = "ingame", At = TimeOf(line) };
        else if (line.Contains(LeavingMark, StringComparison.Ordinal))
            tail.Leaving = true;
        else if (line.Contains(TeleportMark, StringComparison.Ordinal))
            tail.TeleportAt = TimeOf(line);
        else if (line.Contains("disconnect reason to", StringComparison.Ordinal) && ReasonLine.Match(line) is { Success: true } reason)
            tail.PendingReason = int.Parse(reason.Groups[1].Value, CultureInfo.InvariantCulture);
        else if (line.Contains(DisconnectedMark, StringComparison.Ordinal) && tail.State is { } state)
        {
            var at = TimeOf(line);
            // Leaving the old place on the way to the next one: still in the game, about to join.
            bool teleporting = tail.TeleportAt is { } teleport && at - teleport < TeleportWindow;
            tail.TeleportAt = null; // once: a later disconnect without a new teleport is a real one
            if (teleporting)
            {
                tail.State = state with { Status = "joining", At = at };
                tail.PendingReason = null;
                return;
            }
            tail.State = tail.Leaving
                ? state with { Status = "left", Reason = null, At = at }
                : state with { Status = "disconnected", Reason = tail.PendingReason, At = at };
            tail.Leaving = false;
            tail.PendingReason = null;
        }
    }

    /// <summary>Lines start with an ISO UTC time ("2026-10-12T05:14:35.082Z,…").</summary>
    static DateTime TimeOf(string line)
    {
        int comma = line.IndexOf(',');
        return comma > 0 && DateTime.TryParse(line.AsSpan(0, comma), CultureInfo.InvariantCulture,
            DateTimeStyles.AdjustToUniversal | DateTimeStyles.AssumeUniversal, out var at) ? at : DateTime.UtcNow;
    }

    // ───────── Restart Manager: which processes hold a file open (what Windows uses for "file in use") ─────────

    [StructLayout(LayoutKind.Sequential)]
    struct RM_UNIQUE_PROCESS
    {
        public int ProcessId;
        public FILETIME ProcessStartTime;
    }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    struct RM_PROCESS_INFO
    {
        public RM_UNIQUE_PROCESS Process;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 256)] public string AppName;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 64)] public string ServiceShortName;
        public int ApplicationType;
        public uint AppStatus;
        public uint TSSessionId;
        [MarshalAs(UnmanagedType.Bool)] public bool Restartable;
    }

    const int ErrorMoreData = 234;

    [DllImport("rstrtmgr.dll", CharSet = CharSet.Unicode)]
    static extern int RmStartSession(out uint session, int flags, StringBuilder key);

    [DllImport("rstrtmgr.dll")]
    static extern int RmEndSession(uint session);

    [DllImport("rstrtmgr.dll", CharSet = CharSet.Unicode)]
    static extern int RmRegisterResources(uint session, uint files, string[] fileNames, uint apps, IntPtr applications, uint services, string[]? serviceNames);

    [DllImport("rstrtmgr.dll")]
    static extern int RmGetList(uint session, out uint needed, ref uint count, [In, Out] RM_PROCESS_INFO[]? info, ref uint rebootReasons);

    static int[] OpenedBy(string path)
    {
        if (RmStartSession(out uint session, 0, new StringBuilder(33)) != 0)
            return [];
        try
        {
            if (RmRegisterResources(session, 1, [path], 0, IntPtr.Zero, 0, null) != 0)
                return [];
            uint count = 0, reasons = 0;
            int result = RmGetList(session, out uint needed, ref count, null, ref reasons);
            if (result != ErrorMoreData || needed == 0)
                return [];
            var info = new RM_PROCESS_INFO[needed];
            count = needed;
            return RmGetList(session, out _, ref count, info, ref reasons) == 0
                ? info.Take((int)count).Select(i => i.Process.ProcessId).ToArray()
                : [];
        }
        finally
        {
            RmEndSession(session);
        }
    }
}
