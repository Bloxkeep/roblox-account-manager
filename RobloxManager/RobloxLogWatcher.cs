using System.Globalization;
using System.Runtime.InteropServices;
using System.Runtime.InteropServices.ComTypes;
using System.Text;
using System.Text.RegularExpressions;

namespace RobloxManager;

/// <summary>
/// Follows Roblox's client logs (every client writes one, whichever launcher started it) to know within a second or two
/// when a window joins, changes server, leaves or is kicked. Each log is tied to its window by Windows' Restart Manager
/// (who has the file open). The format is not an official API, so Roblox's presence stays as the fallback.
/// </summary>
public sealed class RobloxLogWatcher
{
    /// <param name="Status">joining, ingame, left (back in the app, by choice), disconnected, or signedout (signed out inside the window).</param>
    /// <param name="Reason">278 = idle for 20 minutes.</param>
    /// <param name="ChosenPlace">
    /// The game the person last started in this window from Roblox's own screens; null while still in the game it was
    /// opened into. Teleports inside that game keep it.
    /// </param>
    /// <param name="BannedByGame">Roblox's ban API, rather than a kick: Roblox gives both code 267.</param>
    /// <param name="TeleportStarted">Roblox's doTeleport, until the next server is reached.</param>
    /// <param name="NeverJoined">
    /// Left before any server took it (2026-10-21: ten such launches, none logging why; Roblox's join error lines are off).
    /// </param>
    public sealed record ClientState(string Status, long? PlaceId, string? JobId, int? Reason, DateTime At, long? ChosenPlace = null,
        bool PersonStarted = false, string? KickMessage = null, bool BannedByGame = false, DateTime? TeleportStarted = null, bool NeverJoined = false);

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
        public int PendingReasonRank;
        public int ShownReasonRank;
        /// <summary>The game's kick line, logged before the disconnect it explains.</summary>
        public string? PendingKickMessage;
        public bool PendingBan;
        /// <summary>A game start that isn't the window's first: the game its "Joining game" names is one the person chose.</summary>
        public bool OwnStart;
        /// <summary>Servers connected to ("serverId:") and not yet left (a disconnect line ends one).</summary>
        public int Connections;
        public DateTime? ClosedByNewWindow;
        public DateTime? OutOfMemory;
        public DateTime? ClosedInOrder;
        public DateTime? JoinChallenged;
        /// <summary>Roblox's updater was told to install in the background: this client goes on (see <see cref="UpdateThreadMark"/>).</summary>
        public bool BackgroundUpdate;
        public DateTime? UpdateHandOff;
    }

    /// <summary>
    /// The usual client's, the Microsoft Store edition's and Roblox VNG's. BLOXKEEP_ROBLOX_LOGS (tests only) points at
    /// made-up logs instead.
    /// </summary>
    static readonly string[] LogDirs = Environment.GetEnvironmentVariable("BLOXKEEP_ROBLOX_LOGS") is { Length: > 0 } testLogs
        ? [testLogs]
        :
        [
            Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Roblox", "logs"),
            Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "RobloxPCGDK", "logs"),
            // Roblox VNG's client keeps its own folder (seen 2026-10-19), and opens global accounts too.
            Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "RobloxPCVNG", "logs"),
        ];
    internal static IReadOnlyList<string> Folders => LogDirs;
    // A game start, logged before "Joining game" (seen 2026-10-13 in 0.742, and in the Store edition): a launch that
    // stalls while loading never gets further, so it counts as joining from here.
    const string LaunchMark = "[FLog::SingleSurfaceApp] launchUGCGameInternal";
    // Lines seen in Roblox 0.742 logs.
    static readonly Regex JoiningLine = new(@"\[FLog::Output\] ! Joining game '([0-9a-fA-F-]+)' place (\d+)", RegexOptions.Compiled | RegexOptions.CultureInvariant);
    static readonly Regex ReasonLine = new(@"setting replicator disconnect reason to (\d+)", RegexOptions.Compiled | RegexOptions.CultureInvariant);
    // Used only when the replicator line gave none (285: the player left). Not yet seen on a real kick, so it may
    // come before or after the disconnect line.
    static readonly Regex NotificationReasonLine = new(@"Disconnection Notification\. Reason: (\d+)", RegexOptions.Compiled | RegexOptions.CultureInvariant);
    // The server's own code, from the 0.744 client's strings; not seen in a log yet.
    static readonly Regex ReceivedReasonLine = new(@"\[FLog::Network\] Disconnect reason received: (\d+)", RegexOptions.Compiled | RegexOptions.CultureInvariant);
    /// <summary>
    /// DisconnectClientRequest: the notification line can repeat it after the server's reason, so it never overrides one.
    /// </summary>
    const int ClientRequest = 285;
    // The reason in words. Roblox 0.744 often logs only these, no code (2026-10-21: a game's kick and a sign-in on
    // another device), so a kick read as a lost connection and a sign-in elsewhere was rejoined over.
    const string ReasonWordsMark = "[FLog::Network] Client has been disconnected with reason: ",
        LostWithReasonMark = "[FLog::Network] Lost connection with reason : ";
    /// <summary>
    /// The words as RobloxPlayerBeta.exe 0.744 holds them, each matched to the Enum.ConnectionError code its name
    /// describes. Words not listed leave the reason unknown.
    /// </summary>
    internal static readonly IReadOnlyDictionary<string, int> ReasonWords = new Dictionary<string, int>(StringComparer.Ordinal)
    {
        ["Protocol mismatch, please reconnect"] = 259, // DisconnectProtocolMismatch
        ["Error while receiving data, please reconnect"] = 260, // DisconnectReceivePacketError
        ["Error while streaming data, please reconnect"] = 261, // DisconnectReceivePacketStreamError
        ["Error while sending data, please reconnect"] = 262, // DisconnectSendPacketError
        ["Invalid teleport destination"] = 263, // DisconnectIllegalTeleport
        ["You are already playing a game. Please shut down the other game and try again"] = 264, // DisconnectDuplicatePlayer
        ["Lost connection to server due to timeout"] = 266, // DisconnectTimeout
        ["Disconnected due to timeout, please reconnect"] = 266,
        ["You have been kicked from the game"] = 267, // DisconnectLuaKick: a game's Player:Kick
        ["Disconnected from game, possibly due to game joined from another device"] = 273, // DisconnectEvicted
        ["This game has been disconnected because you have joined a game from another device"] = 273,
        ["Developer has shut down this game server for maintenance"] = 274, // DisconnectDevMaintenance
        ["Roblox has shut down this game server for maintenance"] = 275, // DisconnectRobloxMaintenance
        ["Lost connection to the game server, please reconnect"] = 277, // DisconnectConnectionLost
        ["Disconnected due to unsupported version. Please upgrade to the latest version of Roblox"] = 280, // DisconnectWrongVersion
        ["Client initiated disconnect"] = 285, // DisconnectClientRequest
        ["You have been disconnected because you no longer have access to this private server"] = 286, // DisconnectPrivateServerKickout
        ["The server has shut down"] = 288, // ServerShutdown
        ["Developer has shut down all game servers or game server has shut down for other reasons, please reconnect"] = 288,
        ["Replication has timed out"] = 290, // ReplicatorTimeout
        ["Player has been removed from the DataModel"] = 291, // PlayerRemoved
        ["Disconnected due to internal network failure"] = 296, // NetworkInternal
        ["Disconnected due to send failure"] = 297, // NetworkSend
        ["Disconnected due to network timeout"] = 298, // NetworkTimeout
        ["Disconnected due to network misbehavior"] = 299, // NetworkMisbehavior
        ["Disconnected due to security failure"] = 300, // NetworkSecurity
    };
    const string ConnectedMark = "[FLog::Network] serverId:";
    // The game's words when it kicks a player, logged right before the disconnect (seen 2026-10-21 in 0.743):
    // "[FLog::CreatorError] Error: Server Kick Message: A fatal error occured. This may be from an internet issue.; Metadata: "
    // A ban by the game (Roblox's ban API) gives the same code 267, but its metadata says who banned and since when:
    // "Server Kick Message: ; Metadata: {"moderatorType":"creator","dynamicReasonTextFilterStatus":"2","beginTimestamp":"2026-10-21T00:07:04Z"}".
    // A ban met when joining again (Join Error 600) logs nothing (seen the same day): see AppController.CheckJoin.
    const string KickMark = "Server Kick Message:";
    static readonly Regex KickLine = new(@"Server Kick Message: (.*?); Metadata: ?(.*)$", RegexOptions.Compiled | RegexOptions.CultureInvariant);
    const string LeavingMark = "[FLog::SingleSurfaceApp] leaveUGCGameInternal";
    // Signing out inside the Roblox window (seen 2026-10-13): the window stays open on the sign-in screen.
    const string SignedOutMark = "logout sync cookie to native";
    const string DisconnectedMark = "[FLog::Network] Time to disconnect replication data";
    // A teleport (seen 2026-10-13): doTeleport, the same disconnect line as a kick, then "Teleported." and a new
    // "Joining game". Or (2026-10-19, 0.743, same place) "Teleported." and "Joining game" first, the disconnect after.
    const string TeleportMark = "UgcExperienceController: doTeleport";
    // A teleport that never got that far (it failed) must not turn a kick minutes later into "joining".
    static readonly TimeSpan TeleportWindow = TimeSpan.FromSeconds(30);
    // Every client polls for a newer one (0.742): "waiting for mutex result is 0X000102" means nothing yet; 0X000001
    // or "EventWaitResultUpdate" means one started, and the client closes to make room: what Multi-Roblox prevents.
    const string WndCheckMark = "[FLog::WndProcessCheck]";
    const string NewProcessEventMark = "WindowsAppReloadTelemetry EventWaitResultUpdate";
    const string NewProcessMutexMark = "waiting for mutex result is 0X000001";
    // Windows refusing a client memory, its commit limit (RAM and page files) used up: "Critical [DFLog::Mimalloc] cannot
    // commit OS memory (error: 1455 (0x5AF), …)", "… out-of-memory on OS allocation, try again…", and the crash it ends in,
    // "RBXCRASH: OutOfMemoryGraphics" (2026-10-22, two windows of six, Windows' low-virtual-memory event at 99.7 %).
    const string MimallocMark = "[DFLog::Mimalloc]";
    const string CommitRefusedMark = "cannot commit OS memory", AllocationRefusedMark = "out-of-memory on OS allocation", OutOfMemoryCrashMark = "RBXCRASH: OutOfMemory";
    // A window closed in order (X, a close message, making room) logs its tray copy starting, then shutDown. A crash or
    // a kill writes neither (2026-10-22: 7 of 40 logs on the owner's PC had both, the 2 crashes neither).
    const string ShutDownMark = "[FLog::SingleSurfaceApp] shutDown:", TrayRelaunchMark = "Successfully launched new process in systray mode";
    // The join held for Roblox's bot check: "[DFLog::HttpTraceError] HttpResponse(…)status:403 … url:{
    // "https://gamejoin.roblox.com/v1/join-game" } … challengedByGcs". The window shows Roblox's "Security" page, blank
    // where the check can't load (Vietnam, 2026-10-23), and Join Error 529 once closed.
    const string ChallengedMark = "challengedByGcs", GameJoinMark = "gamejoin.", JoinRequestMark = "/v1/join-";
    // An outdated client starts Roblox's updater with its own launch link: "[FLog::UpdateController] Update thread: start
    // ""…\RobloxPlayerInstaller.exe" roblox-player:…". After "Update mode is chosen as BACKGROUND" it goes on into its game
    // and the updater opens nothing; without that line it quits a second or two later and the updater opens the updated
    // client with the same link about 8-10 s on (the owner's logs of 2026-10-30, 0.744 -> 0.745: four of each).
    const string UpdateThreadMark = "[FLog::UpdateController] Update thread: start", BackgroundUpdateMark = "[FLog::UpdateController] Update mode is chosen as BACKGROUND";
    const long MaxRead = 4 * 1024 * 1024;

    readonly Dictionary<int, Tail> _tails = new();
    readonly HashSet<string> _foreign = new(StringComparer.OrdinalIgnoreCase);
    /// <summary>
    /// Clients without a log yet. Each miss waits longer (3 s up to 30 s); after two minutes one is left alone: the
    /// "who has this file open" questions aren't free.
    /// </summary>
    readonly Dictionary<int, (DateTime FirstSeen, DateTime NextTry, TimeSpan Wait)> _unmatched = new();
    static readonly TimeSpan FirstWait = TimeSpan.FromSeconds(3), MaxWait = TimeSpan.FromSeconds(30), GiveUp = TimeSpan.FromMinutes(2);

    public static TimeSpan SearchLimit => GiveUp;

    public ClientState? StateOf(int pid) => _tails.GetValueOrDefault(pid)?.State;

    /// <summary>
    /// A newer Roblox client told it to close. These lookups are null when not logged (or the log isn't followed);
    /// read them after <see cref="LastStateOf"/>, which takes in the client's last lines.
    /// </summary>
    public DateTime? ClosedByNewWindowAt(int pid) => _tails.GetValueOrDefault(pid)?.ClosedByNewWindow;

    /// <summary>Windows first refused it memory (its commit limit used up).</summary>
    public DateTime? OutOfMemoryAt(int pid) => _tails.GetValueOrDefault(pid)?.OutOfMemory;

    /// <summary>It closed its window in order; null for a crash or a killed process.</summary>
    public DateTime? ClosedInOrderAt(int pid) => _tails.GetValueOrDefault(pid)?.ClosedInOrder;

    /// <summary>
    /// Roblox held its join for a bot check (<see cref="ChallengedMark"/>); cleared once a game starts or a server is reached.
    /// </summary>
    public DateTime? JoinChallengedAt(int pid) => _tails.GetValueOrDefault(pid)?.JoinChallenged;

    /// <summary>
    /// The client handed its launch to Roblox's updater and quits: the updated client comes from the updater
    /// (<see cref="UpdateThreadMark"/>). Null too for one updating in the background, which goes on itself.
    /// </summary>
    public DateTime? UpdateHandOffAt(int pid) => _tails.GetValueOrDefault(pid)?.UpdateHandOff;

    /// <summary>Its log is found and followed, so what it said (or didn't) can be read.</summary>
    public bool Follows(int pid) => _tails.ContainsKey(pid);

    /// <summary>
    /// For a client that has just quit, with its final lines (a leave, a disconnect): the next <see cref="Poll"/> drops
    /// its log unread.
    /// </summary>
    public ClientState? LastStateOf(int pid)
    {
        if (_tails.GetValueOrDefault(pid) is not { } tail)
            return null;
        Read(tail);
        return Ended(tail);
    }

    /// <summary>
    /// A client's state once it has ended: a leave logged and no game joined since is a leave, even without the
    /// disconnect line that normally completes it. The owner's PC, 2026-10-29: back to Roblox's home from a game
    /// (leaveUGCGameInternal) and the window closed with X 3 s later, before Roblox logged that line; the state stayed
    /// "ingame" and auto-rejoin opened the account again as if it had dropped.
    /// </summary>
    static ClientState? Ended(Tail tail) =>
        tail.Leaving && tail.State is { Status: "ingame" or "joining" } state
            ? state with { Status = "left", Reason = null, TeleportStarted = null }
            : tail.State;

    /// <summary>True when a state changed.</summary>
    public bool Poll(IReadOnlyCollection<int> pids)
    {
        foreach (int gone in _tails.Keys.Where(pid => !pids.Contains(pid)).ToList())
            _tails.Remove(gone);
        TakeSearch(pids);
        foreach (int gone in _unmatched.Keys.Where(pid => !pids.Contains(pid) || _tails.ContainsKey(pid)).ToList())
            _unmatched.Remove(gone);
        if (pids.Count == 0)
            return false;
        var now = DateTime.UtcNow;
        foreach (int pid in pids.Where(pid => !_tails.ContainsKey(pid)))
            _unmatched.TryAdd(pid, (now, now, FirstWait));
        if (_search is null && _unmatched.Any(u => now >= u.Value.NextTry && now - u.Value.FirstSeen < GiveUp))
        {
            // Only a due client starts the search, but every client without a log takes part.
            StartSearch(pids);
            foreach (var (pid, u) in _unmatched.ToList())
                _unmatched[pid] = (u.FirstSeen, now + u.Wait, TimeSpan.FromTicks(Math.Min(u.Wait.Ticks * 2, MaxWait.Ticks)));
        }
        // Read on a worker: reading a log Roblox is writing can take a second or more (2026-10-29, the owner's PC:
        // 0.2-1.8 s on the window's thread, 1,315 checks over 100 ms in an hour). Lines are applied here, so states
        // only change on the window's thread, a poll later.
        bool changed = TakeReads();
        if (_reading is null && _tails.Count > 0)
        {
            var wanted = _tails.Values.Select(t => (t, t.Path, t.Offset)).ToList();
            _reading = Task.Run(() => wanted.Select(w => (w.t, w.Offset, ReadChunk(w.Path, w.Offset))).ToList());
        }
        return changed;
    }

    /// <summary>Each log with the offset read from, the new one and the text (null: nothing new).</summary>
    Task<List<(Tail Tail, long From, (long Offset, string? Text) Chunk)>>? _reading;

    /// <summary>
    /// Skips a log dropped or followed anew meanwhile, or read since on the window's thread (<see cref="LastStateOf"/>).
    /// </summary>
    bool TakeReads()
    {
        if (_reading is not { IsCompleted: true } reading)
            return false;
        _reading = null;
        if (!reading.IsCompletedSuccessfully)
        {
            if (reading.Exception?.InnerException is { } ex)
                Log.Write(ex);
            return false;
        }
        bool changed = false;
        foreach (var (tail, from, (offset, text)) in reading.Result)
        {
            if (!ReferenceEquals(_tails.GetValueOrDefault(tail.Pid), tail) || tail.Offset != from)
                continue;
            tail.Offset = offset;
            if (text is not null)
                changed |= ApplyText(tail, text);
        }
        return changed;
    }

    /// <param name="Found">Already read up to their end then.</param>
    /// <param name="Foreign">Old logs no client looked for holds: not asked about again.</param>
    sealed record LogSearch(List<Tail> Found, List<string> Foreign, List<string> Listed);

    /// <summary>
    /// On a worker: asking Windows who holds a file open takes 15-50 ms a file (up to 40 logs right after a start).
    /// </summary>
    Task<LogSearch>? _search;

    void StartSearch(IReadOnlyCollection<int> pids)
    {
        var wanted = pids.Where(pid => !_tails.ContainsKey(pid)).ToHashSet();
        var skip = _tails.Values.Select(t => t.Path).Concat(_foreign).ToHashSet(StringComparer.OrdinalIgnoreCase);
        _search = Task.Run(() => FindLogs(wanted, skip));
    }

    void TakeSearch(IReadOnlyCollection<int> pids)
    {
        if (_search is not { IsCompleted: true } search)
            return;
        _search = null;
        if (!search.IsCompletedSuccessfully)
        {
            if (search.Exception?.InnerException is { } ex)
                Log.Write(ex);
            return;
        }
        var result = search.Result;
        // So the list doesn't grow with every client ever started.
        if (result.Listed.Count > 0)
            _foreign.IntersectWith(result.Listed);
        _foreign.UnionWith(result.Foreign);
        foreach (var tail in result.Found)
        {
            if (pids.Contains(tail.Pid) && !_tails.ContainsKey(tail.Pid) && !_tails.Values.Any(t => string.Equals(t.Path, tail.Path, StringComparison.OrdinalIgnoreCase)))
                _tails[tail.Pid] = tail;
        }
    }

    static LogSearch FindLogs(HashSet<int> wanted, HashSet<string> skip)
    {
        var result = new LogSearch([], [], []);
        List<FileInfo> logs;
        try
        {
            logs = LogDirs.Select(dir => new DirectoryInfo(dir)).Where(dir => dir.Exists)
                .SelectMany(dir => dir.EnumerateFiles("*_Player_*.log"))
                .Where(f => !f.Name.Contains("CrashHandler", StringComparison.OrdinalIgnoreCase) && f.LastWriteTimeUtc > DateTime.UtcNow.AddDays(-2))
                .OrderByDescending(f => f.LastWriteTimeUtc)
                .Take(40)
                .ToList();
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            return result;
        }
        result.Listed.AddRange(logs.Select(log => log.FullName));
        var found = new HashSet<int>();
        foreach (var log in logs)
        {
            if (skip.Contains(log.FullName))
                continue;
            int? owner = OpenedBy(log.FullName).Cast<int?>().FirstOrDefault(pid => wanted.Contains(pid!.Value) && !found.Contains(pid.Value));
            if (owner is int pid)
            {
                found.Add(pid);
                // From the start: the last join, leave or disconnect may be hours back (about 0.4 MB of log an hour).
                var tail = new Tail(log.FullName, pid);
                while (tail.Offset < log.Length && ReadMore(tail))
                {
                }
                result.Found.Add(tail);
            }
            else if (log.LastWriteTimeUtc < DateTime.UtcNow.AddMinutes(-2))
                result.Foreign.Add(log.FullName); // an old log, or another program's client: don't ask again
            if (wanted.All(found.Contains))
                break;
        }
        return result;
    }

    /// <summary>True while something was read.</summary>
    static bool ReadMore(Tail tail)
    {
        long before = tail.Offset;
        Read(tail);
        return tail.Offset > before;
    }

    static bool Read(Tail tail)
    {
        (tail.Offset, string? text) = ReadChunk(tail.Path, tail.Offset);
        return text is not null && ApplyText(tail, text);
    }

    static (long Offset, string? Text) ReadChunk(string path, long offset)
    {
        try
        {
            using var stream = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete);
            if (stream.Length < offset)
                offset = 0;
            if (stream.Length == offset)
                return (offset, null);
            stream.Seek(offset, SeekOrigin.Begin);
            var buffer = new byte[Math.Min(MaxRead, stream.Length - offset)];
            int read = stream.ReadAtLeast(buffer, buffer.Length, throwOnEndOfStream: false);
            return (offset + read, Encoding.UTF8.GetString(buffer, 0, read));
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            return (offset, null);
        }
    }

    static bool ApplyText(Tail tail, string read)
    {
        string text = tail.Partial + read;
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
        // A number too big for its type reads as no place, never as an error that would stop the process check.
        if (line.Contains("! Joining game", StringComparison.Ordinal) && JoiningLine.Match(line) is { Success: true } join)
        {
            // A teleport stays open: its old server's disconnect line may still come (see TeleportMark).
            tail.Leaving = false;
            tail.PendingReason = null;
            (tail.PendingKickMessage, tail.PendingBan) = (null, false);
            long? place = long.TryParse(join.Groups[2].Value, NumberStyles.None, CultureInfo.InvariantCulture, out long id) ? id : null;
            tail.State = new ClientState("joining", place, join.Groups[1].Value, null, TimeOf(line), tail.OwnStart ? place : tail.State?.ChosenPlace,
                tail.State?.PersonStarted == true, TeleportStarted: tail.State?.TeleportStarted);
            tail.OwnStart = false;
            tail.JoinChallenged = null;
        }
        // The line itself: Roblox's client also has "launchUGCGameInternal: deferred download complete" and the like, no game start.
        else if (line.Contains(LaunchMark, StringComparison.Ordinal) && line.AsSpan().TrimEnd().EndsWith(LaunchMark, StringComparison.Ordinal))
        {
            tail.Leaving = false;
            tail.TeleportAt = null;
            tail.PendingReason = null;
            (tail.PendingKickMessage, tail.PendingBan) = (null, false);
            // Bloxkeep's start is a window's first; a later one is the person picking a game on Roblox's screens after
            // leaving or a kick (seen 2026-10-19).
            tail.OwnStart = tail.State is not null;
            tail.State = new ClientState("joining", null, null, null, TimeOf(line), tail.State?.ChosenPlace, tail.OwnStart);
            tail.JoinChallenged = null;
        }
        else if (line.Contains(ConnectedMark, StringComparison.Ordinal))
        {
            tail.Connections++;
            if (tail.State is { Status: "joining" } joining)
                tail.State = joining with { Status = "ingame", At = TimeOf(line), PersonStarted = false, TeleportStarted = null };
        }
        else if (line.Contains(LeavingMark, StringComparison.Ordinal))
        {
            tail.Leaving = true;
            tail.TeleportAt = null; // leaving, not moving: the disconnect line that follows is the leave's
            // Left while the game was still loading (or before anything was logged): no disconnect line follows.
            if (tail.State is null or { Status: "joining" })
                tail.State = (tail.State ?? new ClientState("left", null, null, null, TimeOf(line))) with
                {
                    Status = "left", Reason = null, At = TimeOf(line), TeleportStarted = null, NeverJoined = tail.State is { Status: "joining", JobId: null },
                };
        }
        else if (line.Contains(SignedOutMark, StringComparison.Ordinal))
            tail.State = (tail.State ?? new ClientState("signedout", null, null, null, TimeOf(line))) with
            {
                Status = "signedout", Reason = null, At = TimeOf(line), TeleportStarted = null,
            };
        else if (line.Contains(TeleportMark, StringComparison.Ordinal))
        {
            tail.TeleportAt = TimeOf(line);
            // Shown as teleporting until the next server is reached (its three doTeleport lines keep the first time).
            if (tail.State is { Status: "ingame" or "joining", TeleportStarted: null } moving)
                tail.State = moving with { TeleportStarted = tail.TeleportAt };
        }
        else if (line.Contains(KickMark, StringComparison.Ordinal) && KickLine.Match(line) is { Success: true } kick)
        {
            string? message = kick.Groups[1].Value.Trim() is { Length: > 0 } words ? words : null;
            bool ban = IsBan(kick.Groups[2].Value);
            // Normally before the disconnect line; one logged after it completes the state shown.
            if (tail.State is { Status: "disconnected", KickMessage: null, BannedByGame: false } shown)
                tail.State = shown with { KickMessage = message, BannedByGame = ban };
            else
                (tail.PendingKickMessage, tail.PendingBan) = (message, ban);
        }
        else if ((line.Contains(ReasonWordsMark, StringComparison.Ordinal) || line.Contains(LostWithReasonMark, StringComparison.Ordinal))
                 && ReasonOfWords(line) is int worded)
            RememberReason(tail, worded, 1);
        else if (line.Contains("disconnect reason to", StringComparison.Ordinal) && ReasonLine.Match(line) is { Success: true } reason
                 && int.TryParse(reason.Groups[1].Value, NumberStyles.None, CultureInfo.InvariantCulture, out int why))
            RememberReason(tail, why, 3);
        else if (line.Contains("Disconnect reason received", StringComparison.Ordinal) && ReceivedReasonLine.Match(line) is { Success: true } received
                 && int.TryParse(received.Groups[1].Value, NumberStyles.None, CultureInfo.InvariantCulture, out int sent))
            RememberReason(tail, sent, 3);
        else if (line.Contains("Disconnection Notification", StringComparison.Ordinal) && NotificationReasonLine.Match(line) is { Success: true } notified
                 && int.TryParse(notified.Groups[1].Value, NumberStyles.None, CultureInfo.InvariantCulture, out int code))
            RememberReason(tail, code, code == ClientRequest ? 0 : 2);
        else if (line.Contains(DisconnectedMark, StringComparison.Ordinal) && tail.State is { } state)
        {
            var at = TimeOf(line);
            // A join under way while a server is still connected: this line ends the old server, in whatever order
            // Roblox logged them, and the join goes on.
            bool oldServer = state is { Status: "joining" } && tail.Connections > 0;
            tail.Connections = Math.Max(0, tail.Connections - 1);
            bool teleporting = tail.TeleportAt is { } teleport && at - teleport < TeleportWindow;
            // The new server already named since the teleport: this line is the old one's, and the new state stands.
            bool joinedSince = tail.TeleportAt is { } since && state.At >= since;
            tail.TeleportAt = null; // once: a later disconnect without a new teleport is a real one
            if (teleporting || oldServer)
            {
                if (state.Status != "joining" && !joinedSince)
                    tail.State = state with { Status = "joining", At = at };
                // The old server's kick words go with it, not with the next server's disconnect.
                tail.PendingReason = null;
                (tail.PendingKickMessage, tail.PendingBan) = (null, false);
                return;
            }
            tail.ShownReasonRank = tail.Leaving ? 0 : tail.PendingReasonRank;
            tail.State = tail.Leaving
                ? state with { Status = "left", Reason = null, At = at, TeleportStarted = null }
                : state with
                {
                    Status = "disconnected", Reason = tail.PendingReason, KickMessage = tail.PendingKickMessage, BannedByGame = tail.PendingBan, At = at,
                    TeleportStarted = null,
                };
            tail.Leaving = false;
            tail.PendingReason = null;
            (tail.PendingKickMessage, tail.PendingBan) = (null, false);
        }
        else if (line.Contains(WndCheckMark, StringComparison.Ordinal)
                 && (line.Contains(NewProcessEventMark, StringComparison.Ordinal) || line.Contains(NewProcessMutexMark, StringComparison.Ordinal)))
            tail.ClosedByNewWindow ??= TimeOf(line);
        else if (line.Contains(MimallocMark, StringComparison.Ordinal)
                     && (line.Contains(CommitRefusedMark, StringComparison.Ordinal) || line.Contains(AllocationRefusedMark, StringComparison.Ordinal))
                 || line.Contains(OutOfMemoryCrashMark, StringComparison.Ordinal))
            tail.OutOfMemory ??= TimeOf(line);
        else if (line.Contains(ShutDownMark, StringComparison.Ordinal) || line.Contains(TrayRelaunchMark, StringComparison.Ordinal))
            tail.ClosedInOrder ??= TimeOf(line);
        else if (line.Contains(ChallengedMark, StringComparison.Ordinal) && line.Contains(GameJoinMark, StringComparison.Ordinal)
                 && line.Contains(JoinRequestMark, StringComparison.Ordinal) && tail.State is { Status: "joining", JobId: null })
            tail.JoinChallenged ??= TimeOf(line);
        else if (line.Contains(BackgroundUpdateMark, StringComparison.Ordinal))
            tail.BackgroundUpdate = true;
        else if (line.Contains(UpdateThreadMark, StringComparison.Ordinal) && !tail.BackgroundUpdate)
            tail.UpdateHandOff ??= TimeOf(line);
    }

    // A numeric code can arrive after the words. Replicator (3) > numeric notification (2) > mapped words (1); equal
    // ranks keep the first.
    static void RememberReason(Tail tail, int code, int rank)
    {
        if (tail.State is { Status: "disconnected" } shown)
        {
            if (shown.Reason is null || rank > tail.ShownReasonRank)
            {
                tail.State = shown with { Reason = code };
                tail.ShownReasonRank = rank;
            }
        }
        else if (tail.PendingReason is null || rank > tail.PendingReasonRank)
        {
            tail.PendingReason = code;
            tail.PendingReasonRank = rank;
        }
    }

    /// <summary>Roblox's ban API fills moderatorType and beginTimestamp; a plain kick leaves the metadata empty.</summary>
    internal static bool IsBan(string metadata)
    {
        if (!metadata.TrimStart().StartsWith('{'))
            return false;
        try
        {
            using var doc = System.Text.Json.JsonDocument.Parse(metadata);
            return doc.RootElement.ValueKind == System.Text.Json.JsonValueKind.Object
                && (doc.RootElement.TryGetProperty("moderatorType", out _) || doc.RootElement.TryGetProperty("beginTimestamp", out _));
        }
        catch (System.Text.Json.JsonException)
        {
            return metadata.Contains("\"moderatorType\"", StringComparison.Ordinal);
        }
    }

    internal static int? ReasonOfWords(string line)
    {
        int tag = line.IndexOf('[');
        if (tag < 0) return null;
        var entry = line.AsSpan(tag);
        int prefix = entry.StartsWith(ReasonWordsMark, StringComparison.Ordinal) ? ReasonWordsMark.Length
            : entry.StartsWith(LostWithReasonMark, StringComparison.Ordinal) ? LostWithReasonMark.Length : -1;
        if (prefix < 0)
            return null;
        string words = entry[prefix..].Trim().ToString();
        return ReasonWords.TryGetValue(words, out int code) ? code : words.StartsWith(IdleWords, StringComparison.Ordinal) ? 278 : null;
    }

    /// <summary>
    /// The client's prompt for DisconnectIdle (278), before its minutes, so idle kicks are told on Roblox 0.744 too.
    /// </summary>
    const string IdleWords = "You were disconnected for being idle ";

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
