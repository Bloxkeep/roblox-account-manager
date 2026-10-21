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
    /// <param name="Status">joining, ingame, left (back in the app, by choice), disconnected, or signedout (signed out inside the window).</param>
    /// <param name="Reason">Roblox's disconnect code when it gave one (278 = idle for 20 minutes).</param>
    /// <param name="ChosenPlace">
    /// The game the person last started in this window themselves (left a game or was kicked, then started one from
    /// Roblox's own screens); null while it is still in the game it was opened into. Teleports inside that game keep it.
    /// </param>
    /// <param name="PersonStarted">This join comes from a game start the person made in the window, not Bloxkeep's.</param>
    /// <param name="KickMessage">The game's own words when it kicked or banned the player (null when it gave none).</param>
    /// <param name="BannedByGame">The game banned the player (Roblox's ban API), rather than kicked: Roblox gives both code 267.</param>
    /// <param name="TeleportStarted">When the teleport under way started (Roblox's doTeleport), until the next server is reached.</param>
    /// <param name="NeverJoined">
    /// Left before any server took it: Roblox's join failed and its error's Leave was clicked, or the wait was given up
    /// (2026-10-21: ten such launches, none logging why; Roblox's join error lines are switched off).
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
        /// <summary>When the client logged that another Roblox client's start told it to close (see <see cref="ClosedByNewWindowAt"/>).</summary>
        public DateTime? ClosedByNewWindow;
    }

    /// <summary>
    /// The usual client's logs, and the Microsoft Store edition's and Roblox VNG's (each writes to a folder of its own).
    /// BLOXKEEP_ROBLOX_LOGS (set by the tests only) points at a folder of made-up logs instead.
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
    /// <summary>Where Roblox writes its clients' logs (the Microsoft Store edition has its own folder).</summary>
    internal static IReadOnlyList<string> Folders => LogDirs;
    // A game start, logged before "Joining game" (seen 2026-10-13 in 0.742, and in the Store edition): a launch that
    // stalls while loading never gets further, so it counts as joining from here.
    const string LaunchMark = "[FLog::SingleSurfaceApp] launchUGCGameInternal";
    // Lines seen in Roblox 0.742 logs.
    static readonly Regex JoiningLine = new(@"\[FLog::Output\] ! Joining game '([0-9a-fA-F-]+)' place (\d+)", RegexOptions.Compiled | RegexOptions.CultureInvariant);
    static readonly Regex ReasonLine = new(@"setting replicator disconnect reason to (\d+)", RegexOptions.Compiled | RegexOptions.CultureInvariant);
    // A second source of the reason, used only when the replicator line gave none (285: the player left). Not yet
    // seen on a real kick, so it may come before or after the disconnect line.
    static readonly Regex NotificationReasonLine = new(@"Disconnection Notification\. Reason: (\d+)", RegexOptions.Compiled | RegexOptions.CultureInvariant);
    // The server's own code, as the client got it ("[FLog::Network] Disconnect reason received: %d" in the 0.744 client;
    // not seen in a log yet).
    static readonly Regex ReceivedReasonLine = new(@"\[FLog::Network\] Disconnect reason received: (\d+)", RegexOptions.Compiled | RegexOptions.CultureInvariant);
    /// <summary>
    /// Roblox's DisconnectClientRequest: the client's own disconnect, which its notification line can repeat after the
    /// server's reason (a teleport's or a close's); never over a reason already known.
    /// </summary>
    const int ClientRequest = 285;
    // A third source, the reason in words: "[FLog::Network] Client has been disconnected with reason: You have been kicked
    // from the game" (also "Lost connection with reason : …"). Roblox 0.744 often logs only these words, no code (2026-10-21:
    // a game's kick and a sign-in on another device), so a kick read as a lost connection and a sign-in elsewhere was
    // rejoined over.
    const string ReasonWordsMark = "[FLog::Network] Client has been disconnected with reason: ",
        LostWithReasonMark = "[FLog::Network] Lost connection with reason : ";
    /// <summary>
    /// Roblox's words after "disconnected with reason:", with their code in Roblox's Enum.ConnectionError: the words as
    /// the client holds them (RobloxPlayerBeta.exe 0.744, next to that log line's own format), each matched to the code
    /// its name describes. Words not listed leave the reason unknown, as before.
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
    // A game moving the player to another of its places (seen 2026-10-13): doTeleport, then the same disconnect line
    // as a kick, then "Teleported." and a new "Joining game" a fraction of a second later. Or the other way round (seen
    // 2026-10-19 on 0.743, a move to another server of the same place): "Teleported." and the new "Joining game"
    // first, and the old server's disconnect line a fraction of a second after them.
    const string TeleportMark = "UgcExperienceController: doTeleport";
    // A teleport that never got that far (it failed) must not turn a kick minutes later into "joining".
    static readonly TimeSpan TeleportWindow = TimeSpan.FromSeconds(30);
    // Every client waits for word that a newer one started (seen 2026-10-13 in 0.742): it logs
    // "waitForNewPlayerProcess new waiting for mutex result is 0X000102" (nothing yet) four times a second. A result
    // of 0X000001, or "WindowsAppReloadTelemetry EventWaitResultUpdate", is that word arriving, and the client then
    // closes: Roblox making room for the new window, which is what Multi-Roblox is meant to prevent.
    const string WndCheckMark = "[FLog::WndProcessCheck]";
    const string NewProcessEventMark = "WindowsAppReloadTelemetry EventWaitResultUpdate";
    const string NewProcessMutexMark = "waiting for mutex result is 0X000001";
    const long MaxRead = 4 * 1024 * 1024;

    readonly Dictionary<int, Tail> _tails = new();
    readonly HashSet<string> _foreign = new(StringComparer.OrdinalIgnoreCase);
    /// <summary>
    /// Clients without a log yet: since when, and when to look again. Each miss waits longer (3 s up to 30 s), and
    /// one still without a log after two minutes (a copy that writes none) is left alone — the folder scan and the
    /// "who has this file open" questions aren't free.
    /// </summary>
    readonly Dictionary<int, (DateTime FirstSeen, DateTime NextTry, TimeSpan Wait)> _unmatched = new();
    static readonly TimeSpan FirstWait = TimeSpan.FromSeconds(3), MaxWait = TimeSpan.FromSeconds(30), GiveUp = TimeSpan.FromMinutes(2);

    /// <summary>How long a client's log is looked for before it is taken as one that writes none.</summary>
    public static TimeSpan SearchLimit => GiveUp;

    public ClientState? StateOf(int pid) => _tails.GetValueOrDefault(pid)?.State;

    /// <summary>
    /// When this client's log said a newer Roblox client told it to close; null when it didn't (or its log isn't
    /// followed). Read after <see cref="LastStateOf"/>, which takes in the client's last lines.
    /// </summary>
    public DateTime? ClosedByNewWindowAt(int pid) => _tails.GetValueOrDefault(pid)?.ClosedByNewWindow;

    /// <summary>
    /// What a client that has just quit logged last, including the lines written in its final moments (a leave by
    /// choice, a disconnect): read now, since the next <see cref="Poll"/> drops its log unread.
    /// </summary>
    public ClientState? LastStateOf(int pid)
    {
        if (_tails.GetValueOrDefault(pid) is not { } tail)
            return null;
        Read(tail);
        return tail.State;
    }

    /// <summary>Reads what the clients with these process ids logged since the last call; true when a state changed.</summary>
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
            // Every client without a log takes part (a log found is a log found); only a due one starts the search.
            StartSearch(pids);
            foreach (var (pid, u) in _unmatched.ToList())
                _unmatched[pid] = (u.FirstSeen, now + u.Wait, TimeSpan.FromTicks(Math.Min(u.Wait.Ticks * 2, MaxWait.Ticks)));
        }
        bool changed = false;
        foreach (var tail in _tails.Values)
            changed |= Read(tail);
        return changed;
    }

    /// <param name="Found">Logs held open by a client looked for, already read up to their end then.</param>
    /// <param name="Foreign">Old logs no client looked for holds: not asked about again.</param>
    /// <param name="Listed">The recent logs the search went through.</param>
    sealed record LogSearch(List<Tail> Found, List<string> Foreign, List<string> Listed);

    /// <summary>
    /// A search for the logs of clients without one, on a worker: asking Windows who holds a file open takes 15-50 ms
    /// a file (more right after Bloxkeep starts, with up to 40 recent logs to ask about), too long for the window's
    /// thread. Its result is taken in at the next poll.
    /// </summary>
    Task<LogSearch>? _search;

    void StartSearch(IReadOnlyCollection<int> pids)
    {
        var wanted = pids.Where(pid => !_tails.ContainsKey(pid)).ToHashSet();
        var skip = _tails.Values.Select(t => t.Path).Concat(_foreign).ToHashSet(StringComparer.OrdinalIgnoreCase);
        _search = Task.Run(() => FindLogs(wanted, skip));
    }

    /// <summary>The finished search's logs are followed (for clients still running and still without one).</summary>
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
        // Only logs still among the recent ones are remembered: the list doesn't grow with every client ever started.
        if (result.Listed.Count > 0)
            _foreign.IntersectWith(result.Listed);
        _foreign.UnionWith(result.Foreign);
        foreach (var tail in result.Found)
        {
            if (pids.Contains(tail.Pid) && !_tails.ContainsKey(tail.Pid) && !_tails.Values.Any(t => string.Equals(t.Path, tail.Path, StringComparison.OrdinalIgnoreCase)))
                _tails[tail.Pid] = tail;
        }
    }

    /// <summary>A new client opens a new log; ask Windows which of the recent logs each client looked for holds open.</summary>
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
                // From the start, a few MB at a time: the client's state is wherever its last join, leave or
                // disconnect is, hours back for one that has run long (at about 0.4 MB of log an hour).
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

    /// <summary>Like <see cref="Read"/>, for a tail read up to an offset: true while something was read.</summary>
    static bool ReadMore(Tail tail)
    {
        long before = tail.Offset;
        Read(tail);
        return tail.Offset > before;
    }

    static bool Read(Tail tail)
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
        // A number too big for its type (a line not in the format seen so far) reads as no place, never as an error
        // that would stop the process check.
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
        }
        // The line itself: Roblox's client also has "launchUGCGameInternal: deferred download complete" and the like, no game start.
        else if (line.Contains(LaunchMark, StringComparison.Ordinal) && line.AsSpan().TrimEnd().EndsWith(LaunchMark, StringComparison.Ordinal))
        {
            tail.Leaving = false;
            tail.TeleportAt = null;
            tail.PendingReason = null;
            (tail.PendingKickMessage, tail.PendingBan) = (null, false);
            // Bloxkeep's own start is a window's first (it opens a new window for every launch); a later one is the
            // person picking a game on Roblox's screens, after leaving one or being kicked (seen 2026-10-19: a kick, Leave,
            // then a game start in the same window).
            tail.OwnStart = tail.State is not null;
            tail.State = new ClientState("joining", null, null, null, TimeOf(line), tail.State?.ChosenPlace, tail.OwnStart);
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
            // A join under way while a server is still connected: this line ends that old server, whatever order
            // Roblox logged them in (a teleport, a game started in the window), and the join goes on.
            bool oldServer = state is { Status: "joining" } && tail.Connections > 0;
            tail.Connections = Math.Max(0, tail.Connections - 1);
            // Leaving the old place on the way to the next one: still in the game, about to join.
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
    }

    // Preserve the source's strength after showing the disconnect too: a numeric code can arrive later than words.
    // Replicator (3) > numeric notification (2) > mapped words (1). Equal-strength repeats retain the first result.
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

    /// <summary>
    /// A kick line's metadata says it is a ban: Roblox's ban API names who banned (moderatorType) and since when; a plain
    /// kick leaves it empty.
    /// </summary>
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

    /// <summary>The code of the reason a disconnect line gives in words (<see cref="ReasonWords"/>); null for words not listed.</summary>
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
    /// Roblox's idle kick in words, before its minutes ("You were disconnected for being idle 20 minutes"): the client's
    /// own prompt for DisconnectIdle (278) reads the same, so anti-AFK's kicks are told on Roblox 0.744 too.
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
