using System.Net.NetworkInformation;
using Microsoft.Win32;

namespace RobloxManager;

/// <summary>Sleep and wake, the network coming and going, and the clock changing: what the app holds back or catches up on meanwhile.</summary>
public sealed partial class AppController
{
    /// <summary>The PC woke up less than a minute ago, or has no network: auto-rejoin holds off.</summary>
    bool Settling => DateTime.UtcNow - _resumedAt < ResumeGrace || !NetworkUp;

    /// <summary>Kept by Windows' notification: asking lists every adapter, too slow for each process tick.</summary>
    volatile bool _networkUp = NetworkInterface.GetIsNetworkAvailable();

    void OnNetworkAvailabilityChanged(object? sender, NetworkAvailabilityEventArgs e) => _networkUp = e.IsAvailable;

    DateTime _networkAskedAt;
    static readonly TimeSpan NetworkAskInterval = TimeSpan.FromSeconds(30);

    /// <summary>Asked again every 30 s while "down": an "available" notification that never came (a wake, a Wi-Fi flap, two in
    /// the wrong order) would otherwise hold auto-rejoin and the sign-in check back until the next change.</summary>
    bool NetworkUp
    {
        get
        {
            var now = DateTime.UtcNow;
            if (!_networkUp && (now - _networkAskedAt >= NetworkAskInterval || now < _networkAskedAt))
            {
                _networkAskedAt = now;
                _networkUp = NetworkInterface.GetIsNetworkAvailable();
            }
            return _networkUp;
        }
    }

    /// <summary>The time zone was changed: local times (the schedules' minutes) must come from the new one, not a cached one.</summary>
    static void OnTimeChanged(object? sender, EventArgs e) => TimeZoneInfo.ClearCachedData();

    /// <summary>After sleep every client has lost its connection and the network takes a moment: disconnect detection and
    /// auto-rejoin wait, so the clients aren't all restarted into a dead network at once.</summary>
    void OnPowerModeChanged(object? sender, PowerModeChangedEventArgs e)
    {
        if (_disposed)
            return;
        try
        {
            // Going to sleep: a pending save and backup are written first (the PC may never wake up the same way);
            // waited for, since Windows suspends right after this. Waking: on the window's thread, when it gets there.
            if (e.Mode == PowerModes.Suspend)
                _owner.Invoke(new Action(OnSuspending));
            else if (e.Mode == PowerModes.Resume)
                _owner.BeginInvoke(new Action(OnResumed));
        }
        catch (InvalidOperationException)
        {
            // The window is closing.
        }
    }

    void OnSuspending()
    {
        _asleepTick = Environment.TickCount64;
        StopMacrosForSleep();
        if (_vault is null)
        {
            // Stays left open by the lock end at this sleep, once unlocked (see ResumePlaySessions).
            _sleptWhileLocked ??= DateTime.UtcNow;
            return;
        }
        // Open stays end here: time asleep isn't play time, and the drop every client logs at wake isn't a kick.
        if (SuspendPlay(DateTime.UtcNow) | NoteOpenSessions() | NoteScheduledRuns() | _saveDue is not null)
            Persist();
        BackupIfDirty();
    }

    void OnResumed()
    {
        _asleepTick = 0;
        _resumedAt = DateTime.UtcNow;
        _offlineStrikes.Clear();
        _nextPresence = _resumedAt + ResumeGrace;
        if (_nextUpdateCheck > _resumedAt + TimeSpan.FromMinutes(2))
            _nextUpdateCheck = _resumedAt + TimeSpan.FromMinutes(2);
        Resumed?.Invoke();
    }

    /// <summary>Raised on the UI thread when the PC wakes from sleep.</summary>
    public Action? Resumed { get; set; }
}
