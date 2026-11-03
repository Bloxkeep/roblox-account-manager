using System.ComponentModel;
using System.Diagnostics;

namespace RobloxManager;

/// <summary>The PC's memory watched: Roblox windows' RAM priority, resting windows' RAM emptied, and the RAM limit per window.</summary>
public sealed partial class AppController
{
    readonly HashSet<int> _lowMemoryPriority = new();
    /// <summary>Every Roblox window open at the first look has had its RAM priority set (see GuardResources).</summary>
    bool _memoryPriorityAdopted;

    /// <summary>
    /// Opening <paramref name="toOpen"/> more windows would leave less than the margin of the commit room: Play's
    /// "virtual memory is almost full" warning.
    /// </summary>
    internal static bool VirtualMemoryShort(MemoryStatus memory, int toOpen, long perClientCommitMb) =>
        toOpen > 0 && memory.CommitLimitMb > 0 && memory.CommitRoomMb - toOpen * perClientCommitMb < CommitMarginMb;

    /// <summary>The average of those running, else <see cref="DefaultClientCommitMb"/>.</summary>
    static long ClientCommitMb(IReadOnlyCollection<RobloxProcesses.PlayerProcess> players) =>
        players.Count(p => p.Private > 0) is > 0 and var known
            ? Math.Max(DefaultClientCommitMb / 2, players.Where(p => p.Private > 0).Sum(p => p.Private) / known / MB) : DefaultClientCommitMb;

    /// <summary>
    /// Another Roblox wouldn't fit in the commit room, or RAM is nearly all used. The main window then closes its
    /// browser sooner in the background.
    /// </summary>
    public static bool MemoryTight(MemoryStatus memory) =>
        (memory.CommitLimitMb > 0 && memory.CommitRoomMb < DefaultClientCommitMb + CommitMarginMb)
        || memory.LoadPercent >= MemoryWarningLoad;

    /// <summary>
    /// Warns when the PC runs low on RAM. With "save RAM on background windows", windows you aren't playing get a low
    /// memory priority: Windows takes RAM from them first, only when it runs short.
    /// </summary>
    void GuardResources(List<RobloxProcesses.PlayerProcess> players, MemoryStatus memory, DateTime now)
    {
        var s = _vault!.Data.Settings;
        if (s.MemoryWarnings && players.Count > 0 && memory.LoadPercent >= MemoryWarningLoad
            && now - _lastMemoryWarning > MemoryWarningInterval)
        {
            _lastMemoryWarning = now;
            Notify(L.F("Your PC is using {0}% of its RAM. Close some Roblox windows if things get slow.", memory.LoadPercent), "warn", "ram");
        }
        // RAM may be fine while the commit limit is nearly used up: a Roblox window that needs more then crashes.
        else if (s.MemoryWarnings && players.Count > 0 && memory.CommitLimitMb > 0 && memory.CommitRoomMb < CommitMarginMb
            && now - _lastMemoryWarning > MemoryWarningInterval)
        {
            _lastMemoryWarning = now;
            Notify(L.T("RAM is free, but Windows' virtual memory is almost full, so a Roblox window may crash. Close other apps or increase virtual memory."), "warn", "ram");
        }

        foreach (int gone in _lowMemoryPriority.Where(pid => players.All(p => p.Pid != pid)).ToList())
            _lowMemoryPriority.Remove(gone);
        // The first look at the windows: a Bloxkeep that crashed (or was ended) left its background windows at low priority
        // with no list of them, so one in front, or every one with the option off, stayed low (2026-10-30 review). Each gets
        // what it should have, once.
        if (!_memoryPriorityAdopted && players.Count > 0)
        {
            _memoryPriorityAdopted = true;
            int front = RobloxProcesses.ForegroundPid();
            foreach (var p in players)
            {
                bool low = s.TrimBackground && p.Pid != front;
                if (RobloxProcesses.SetMemoryPriority(p.Pid, low) && low)
                    _lowMemoryPriority.Add(p.Pid);
            }
        }
        if (!s.TrimBackground)
        {
            GiveMemoryPriorityBack();
            return;
        }
        if (players.Count == 0)
            return;

        // Only touch a window when its role changes (it went to the background, or came to the front).
        int foreground = RobloxProcesses.ForegroundPid();
        bool refused = false;
        foreach (var p in players)
        {
            bool background = p.Pid != foreground;
            if (background == _lowMemoryPriority.Contains(p.Pid))
                continue;
            if (!RobloxProcesses.SetMemoryPriority(p.Pid, background))
            {
                refused |= !RobloxProcesses.Ended(p.Pid); // one that closed since the scan had nothing to change
                continue;
            }
            if (background)
                _lowMemoryPriority.Add(p.Pid);
            else
                _lowMemoryPriority.Remove(p.Pid);
        }

        // No emptying the RAM of windows in view: a background game needed it all back at once, and with 5 windows some
        // stuttered, a window grabbed to drag most of all (the owner's video, 2026-10-21). Resting ones: EmptyRestingRamOnce.
        if (refused && !_trimRefusedShown)
        {
            _trimRefusedShown = true;
            // Nothing the person can do about it: noted, not shown.
            Log.Note("Save RAM: Windows refused to lower a Roblox window's memory priority; it runs as usual.");
        }
    }

    /// <summary>Since when each window has rested, and the ones already emptied this rest (see EmptyRestingRamOnce).</summary>
    readonly Dictionary<int, DateTime> _restingSince = new();
    readonly HashSet<int> _ramEmptied = [];
    static readonly TimeSpan EmptyRamRested = TimeSpan.FromSeconds(60);

    /// <summary>
    /// Save RAM: a window that has rested a minute (minimized, or hidden by Bloxkeep) has its RAM emptied, once each rest:
    /// what the game doesn't touch stays out of RAM and the rest comes back. Not again and again, and not a window in view
    /// (both stuttered, 2026-10-21); the rest of the time only the low priority of GuardResources applies. Noted in the
    /// log (pid and MB only), to tell from a real game what it gave. Not yet tried on real Roblox (2026-10-31).
    /// </summary>
    void EmptyRestingRamOnce(List<RobloxProcesses.PlayerProcess> players, Dictionary<int, List<IntPtr>> windows, DateTime now)
    {
        foreach (int gone in _restingSince.Keys.Where(pid => players.All(p => p.Pid != pid)).ToList())
            _restingSince.Remove(gone);
        _ramEmptied.RemoveWhere(pid => players.All(p => p.Pid != pid));
        // A window shown unseen for a moment (anti-AFK's nudge, /pc screenshot) goes on resting.
        if (WindowsInTurn)
            return;
        if (!_vault!.Data.Settings.TrimBackground)
        {
            _restingSince.Clear();
            _ramEmptied.Clear();
            return;
        }
        int front = RobloxProcesses.ForegroundPid();
        foreach (var p in players)
        {
            var window = windows.TryGetValue(p.Pid, out var list) ? RobloxProcesses.MainWindowOf(list) : IntPtr.Zero;
            if (p.Pid == front || window == IntPtr.Zero || !(RobloxProcesses.IsMinimised(window) || RobloxProcesses.IsHidden(window)))
            {
                _restingSince.Remove(p.Pid);
                _ramEmptied.Remove(p.Pid);
                continue;
            }
            if (!_restingSince.TryAdd(p.Pid, now) && now - _restingSince[p.Pid] >= EmptyRamRested && _ramEmptied.Add(p.Pid))
                _ = EmptyRamInTurn(p.Pid, p.WorkingSet / MB);
        }
    }

    /// <summary>The choices of Settings › Staying in game › RAM limit for resting windows, in MB (0 = off).</summary>
    internal static readonly int[] RamLimitChoices = [0, RamLimitAuto, 512, 768, 1024, 1536, 2048];

    /// <summary>
    /// "Auto": each resting window's limit found for its own game (2026-10-30, two designs measured on a stand-in game:
    /// a limit above what a game keeps touching costs nothing, one below it makes the game fetch its own pages back for
    /// ever, ~430k faults/s and 5 fps). Starts at 3/4 of what the window holds, comes down a quarter at a time while it
    /// barely faults, goes up half again when it faults a lot, and a window that needed it twice is left alone a while.
    /// Only once the PC has used <see cref="AutoRamLoad"/> % of its RAM: below that a limit frees RAM nobody needs.
    /// </summary>
    internal const int RamLimitAuto = -1;
    internal const int AutoRamLoad = 60, AutoRamFloorMb = 768;
    static readonly TimeSpan AutoRamRested = TimeSpan.FromSeconds(60), AutoRamStep = TimeSpan.FromSeconds(60), AutoRamRaisedHold = TimeSpan.FromMinutes(10),
        AutoRamLeftAlone = TimeSpan.FromMinutes(30);
    /// <summary>Faults a second (median of the last few looks): under this the limit may come down, over the next it goes up.
    /// A game at its own pace faulted under 700/s on the stand-in; one short of what it touches, hundreds of thousands.</summary>
    const double AutoRamCalm = 5_000, AutoRamStarved = 25_000, AutoRamHardStarved = 50;

    /// <summary>Auto's state for a window: its limit, since when, raises, fault counts last seen, recent rates.</summary>
    sealed class AutoRam
    {
        public long CapMb;
        public DateTime ChangedAt, RestingSince;
        public int Raises;
        public DateTime? LeftAloneUntil;
        public long Faults = -1, HardFaults = -1;
        public DateTime LastLook;
        public readonly List<(double Soft, double Hard)> Rates = [];
    }
    readonly Dictionary<int, AutoRam> _autoRam = new();
    /// <summary>Windows Windows refused a limit for, said once each in the log.</summary>
    readonly HashSet<int> _ramLimitRefused = [];

    /// <summary>Clients given a RAM limit, and the limit (-1: one found already limited, by a Bloxkeep that closed before lifting it).</summary>
    readonly Dictionary<int, long> _ramLimited = new();
    bool _ramLimitsAdopted;

    /// <summary>
    /// RAM limit for resting windows: a Roblox window that isn't drawing (minimized, or hidden by Bloxkeep) keeps at most
    /// the chosen RAM (or Auto's, <see cref="RamLimitAuto"/>), the rest waiting in compressed memory or virtual memory, so
    /// more accounts fit on one PC (a resting game uses little of what it holds: about 1 of its 3 GB, measured 2026-10-29).
    /// Lifted once the window shows, comes to the front, or the option is off. Not for one in view: it still draws, and
    /// limited it took twice the CPU. Every limit set, changed and lifted is noted in the log (pid and MB only), so a
    /// window seen above its limit can be told apart from one that wasn't limited then (the owner saw 2 GB, 2026-10-30).
    /// </summary>
    void LimitRestingRam(List<RobloxProcesses.PlayerProcess> players, Dictionary<int, List<IntPtr>> windows, DateTime now, MemoryStatus memory)
    {
        foreach (int gone in _ramLimited.Keys.Where(pid => players.All(p => p.Pid != pid)).ToList())
            _ramLimited.Remove(gone);
        foreach (int gone in _autoRam.Keys.Where(pid => players.All(p => p.Pid != pid)).ToList())
            _autoRam.Remove(gone);
        _ramLimitRefused.RemoveWhere(pid => players.All(p => p.Pid != pid));
        if (!_ramLimitsAdopted)
        {
            _ramLimitsAdopted = true;
            foreach (var p in players.Where(p => RobloxProcesses.HasRamLimit(p.Pid)))
                _ramLimited[p.Pid] = -1;
        }
        // A window shown unseen for a moment (anti-AFK's nudge, /pc screenshot) is left as it is meanwhile.
        if (WindowsInTurn)
            return;
        long setting = _vault!.Data.Settings.RamLimitMb;
        if (setting != RamLimitAuto)
            _autoRam.Clear(); // back on Auto later, it waits and measures again
        int front = RobloxProcesses.ForegroundPid();
        foreach (var p in players)
        {
            var window = windows.TryGetValue(p.Pid, out var list) ? RobloxProcesses.MainWindowOf(list) : IntPtr.Zero;
            bool resting = setting != 0 && p.Pid != front && window != IntPtr.Zero
                           && (RobloxProcesses.IsMinimised(window) || RobloxProcesses.IsHidden(window));
            long? want = !resting ? null : setting == RamLimitAuto ? AutoLimit(p, now, memory) : setting;
            bool limited = _ramLimited.TryGetValue(p.Pid, out long had);
            if (!resting)
                _autoRam.Remove(p.Pid);
            if (want is { } limit && (!limited || had != limit))
            {
                _ramLimited[p.Pid] = limit;
                _ = SetRamLimit(p.Pid, limit, setting == RamLimitAuto ? "auto" : "chosen", p.Ram / MB);
            }
            else if (want is null && limited)
            {
                _ = LimitRamInTurn(p.Pid, null);
                _ramLimited.Remove(p.Pid);
                Log.Note($"RAM limit: lifted for Roblox window {p.Pid} ({(resting ? "auto left it alone" : "shown, in front, or the option off")}).");
            }
        }
    }

    /// <summary>
    /// Off the interface's thread and one at a time, in the order asked: Windows takes the RAM back before it answers, and
    /// five windows minimized together held the app still for 1.5 s (the owner's log, 2026-11-01).
    /// </summary>
    Task<bool> InRamTurn(Func<bool> ask)
    {
        var work = _ramLimitTurn.ContinueWith(_ => ask(), TaskScheduler.Default);
        _ramLimitTurn = work;
        return work;
    }
    Task _ramLimitTurn = Task.CompletedTask;

    Task<bool> LimitRamInTurn(int pid, long? mb) => InRamTurn(() => RobloxProcesses.LimitRam(pid, mb));

    async Task EmptyRamInTurn(int pid, long heldMb)
    {
        if (await InRamTurn(() => RobloxProcesses.EmptyRam(pid)))
            Log.Note($"Save RAM: Roblox window {pid} emptied once, it held {heldMb} MB.");
    }

    /// <summary>A refused limit is forgotten, so the next look asks again.</summary>
    async Task SetRamLimit(int pid, long limit, string why, long holdingMb)
    {
        if (await LimitRamInTurn(pid, limit))
            Log.Note($"RAM limit: Roblox window {pid} limited to {limit} MB ({why}), holding {holdingMb} MB.");
        else
        {
            if (_ramLimited.TryGetValue(pid, out long now) && now == limit)
                _ramLimited.Remove(pid);
            if (_ramLimitRefused.Add(pid))
                Log.Note($"RAM limit: Windows refused a limit for Roblox window {pid}.");
        }
    }

    /// <summary>
    /// Auto's limit for a resting window now (MB), or null for none: not before it rested <see cref="AutoRamRested"/>, nor
    /// below <see cref="AutoRamLoad"/> % of RAM used (a limit already set stays), nor while it is left alone after needing
    /// more twice. Its fault rates (median of the last five looks, so one garbage collection's burst is no reason) move it.
    /// </summary>
    long? AutoLimit(RobloxProcesses.PlayerProcess p, DateTime now, MemoryStatus memory)
    {
        if (!_autoRam.TryGetValue(p.Pid, out var a))
            _autoRam[p.Pid] = a = new AutoRam { RestingSince = now, LastLook = now };
        if (p.PageFaults > 0 && a.Faults >= 0 && now > a.LastLook)
        {
            double seconds = (now - a.LastLook).TotalSeconds;
            (double soft, double hard) = (Math.Max(0, p.PageFaults - a.Faults) / seconds, Math.Max(0, p.HardFaults - a.HardFaults) / seconds);
            // Faulting while it holds well under its limit: Windows took its RAM (another program needed more than the PC
            // has), which says nothing of the limit, and a higher one gives nothing back. Not counted, so not raised for it.
            bool starved = soft > AutoRamStarved || hard > AutoRamHardStarved;
            if (!starved || a.CapMb == 0 || p.WorkingSet >= a.CapMb * MB * 9 / 10)
                a.Rates.Add((soft, hard));
            if (a.Rates.Count > 5)
                a.Rates.RemoveAt(0);
        }
        (a.Faults, a.HardFaults, a.LastLook) = (p.PageFaults, p.HardFaults, now);
        if (a.LeftAloneUntil is { } until)
        {
            if (now < until)
                return null;
            a.LeftAloneUntil = null;
            a.Raises = 0;
            a.CapMb = 0;
        }
        if (a.CapMb == 0)
        {
            if (now - a.RestingSince < AutoRamRested || memory.LoadPercent < AutoRamLoad)
                return null;
            a.CapMb = Math.Max(AutoRamFloorMb, p.WorkingSet / MB * 3 / 4 / 64 * 64); // what's shared counts towards a limit too
            a.ChangedAt = now;
            a.Rates.Clear();
            return a.CapMb;
        }
        if (a.Rates.Count >= 3)
        {
            double soft = Median(a.Rates.Select(r => r.Soft)), hard = Median(a.Rates.Select(r => r.Hard));
            if (soft > AutoRamStarved || hard > AutoRamHardStarved)
            {
                if (++a.Raises >= 3)
                {
                    a.LeftAloneUntil = now + AutoRamLeftAlone;
                    Log.Note($"RAM limit: Roblox window {p.Pid} needs its RAM ({soft:0} faults/s, {hard:0} from disk); left alone for {AutoRamLeftAlone.TotalMinutes:0} min.");
                    return null;
                }
                a.CapMb = a.CapMb * 3 / 2 / 64 * 64;
                a.ChangedAt = now + AutoRamRaisedHold - AutoRamStep; // held a while before it may come down again
                a.Rates.Clear();
            }
            else if (soft < AutoRamCalm && hard < 1 && now - a.ChangedAt >= AutoRamStep && a.CapMb > AutoRamFloorMb
                     && memory.LoadPercent >= AutoRamLoad)
            {
                a.CapMb = Math.Max(AutoRamFloorMb, a.CapMb * 3 / 4 / 64 * 64);
                a.ChangedAt = now;
                a.Raises = 0; // calm again: an asset load an hour ago is no reason to give up on it
                a.Rates.Clear();
            }
        }
        return a.CapMb;
    }

    static double Median(IEnumerable<double> values)
    {
        var sorted = values.Order().ToList();
        return sorted.Count == 0 ? 0 : sorted[sorted.Count / 2];
    }

    /// <summary>Every window Bloxkeep lowered gets its normal RAM priority back (the option turned off, Bloxkeep closing).</summary>
    void GiveMemoryPriorityBack()
    {
        foreach (int pid in _lowMemoryPriority)
            RobloxProcesses.SetMemoryPriority(pid, background: false);
        _lowMemoryPriority.Clear();
    }

    /// <summary>Every client's RAM limit lifted (Bloxkeep closing: none is left limited without it).</summary>
    void LiftRamLimits()
    {
        foreach (int pid in _ramLimited.Keys)
            _ = LimitRamInTurn(pid, null);
        _ramLimited.Clear();
        _ramLimitTurn.Wait(TimeSpan.FromSeconds(5)); // one asked just before mustn't land after Bloxkeep is gone
    }
}
