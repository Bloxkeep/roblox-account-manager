using System.Net.NetworkInformation;
using Microsoft.Win32;

namespace RobloxManager;

/// <summary>Bloxkeep's own updates, the launches handed over meanwhile, and What's new.</summary>
public sealed partial class AppController
{
    void OnUpdaterChanged()
    {
        if (_disposed || !_owner.IsHandleCreated)
            return;
        try
        {
            _owner.BeginInvoke(new Action(() =>
            {
                // A download that failed: the lock screen asks again when the next check finds the update.
                if (_updater.Status == "error")
                    _updateAtLock = null;
                NoteUpdateReady(); // Statistics › Activity
                PushState();
            }));
        }
        catch (InvalidOperationException)
        {
            // The window is closing.
        }
    }

    async Task<object?> CheckUpdatesAsync()
    {
        _nextUpdateCheck = DateTime.UtcNow + UpdateCheckInterval;
        await _updater.CheckAsync();
        return _updater.Dto();
    }

    /// <summary>"Restart to update" was clicked and the app is on its way out (see ApplyUpdateAsync).</summary>
    bool _applyingUpdate;

    /// <summary>
    /// Launches whose client quit early and are waiting for the client it may have handed over to, with the pid
    /// that quit (its children are the first choice) and what its log said last.
    /// </summary>
    readonly Dictionary<PendingLaunch, (int Pid, RobloxLogWatcher.ClientState? Log)> _handedOver = new(ReferenceEqualityComparer.Instance);

    /// <summary>
    /// Launches whose window didn't come within <see cref="PendingTimeout"/>: a window that appears for one within
    /// <see cref="LateTimeout"/> more is still claimed for it (see TrackProcesses), so its client isn't left untracked.
    /// </summary>
    readonly HashSet<PendingLaunch> _late = new(ReferenceEqualityComparer.Instance);
    static readonly TimeSpan LateTimeout = TimeSpan.FromMinutes(5);

    /// <summary>
    /// Stops waiting for a launch; one that was already running ends its session (auto-rejoin, toast). A rejoin whose
    /// client never got a window is said so, since its account is otherwise left with no word and no client.
    /// </summary>
    /// <param name="late">Its window may still come (see <see cref="_late"/>): a scheduled stop keeps the account until then.</param>
    void DropPending(PendingLaunch launch, bool late = false)
    {
        _pending.Remove(launch);
        bool closed = _closeWhenClaimed.Remove(launch);
        if (_handedOver.Remove(launch, out var handed) && !closed)
        {
            OnSessionEnded(launch.AccountId, launch, handed.Log);
            return;
        }
        if (launch.Rejoin && !closed && FindAccount(launch.AccountId) is { } account)
            Notify(L.F("{0} didn't rejoin: no Roblox window appeared.", Name(account)), "warn", "rejoinFail", account.Id);
        if (!late && !_running.ContainsKey(launch.AccountId))
            ForgetScheduledOpen(launch.AccountId); // no client came of it: a scheduled stop has nothing of this account's to close
    }

    /// <summary>
    /// Remembers which version last ran. When that is older than this one, the app was updated and the page shows
    /// the release notes in between, once; a fresh install has nothing remembered and shows nothing.
    /// </summary>
    void NoteVersion()
    {
        string current = _updater.CurrentVersion;
        // The updater swapped the program files and rewrote the shortcuts; Explorer may keep showing a blank icon.
        if (Prefs.LastVersion is { } previous && previous != current && _updater.Supported)
            Ui.RefreshShellIcons();
        if (Prefs.LastVersion is { } last && WhatsNew.AfterUpdate(last, current).Length > 0)
        {
            _updatedFrom = last;
            return;
        }
        // Only ever raised: an older build run afterwards (a development copy) must not make "what's new" show again.
        if (Prefs.LastVersion is not { } seen || WhatsNew.Older(seen, current))
        {
            Prefs.LastVersion = current;
            Prefs.Save();
        }
    }

    object? SeenWhatsNew()
    {
        _updatedFrom = null;
        Prefs.LastVersion = _updater.CurrentVersion;
        Prefs.Save();
        return null;
    }

    object? SetAutoUpdate(bool enabled)
    {
        Prefs.CheckUpdates = enabled;
        Prefs.Save();
        if (enabled)
            _nextUpdateCheck = DateTime.UtcNow;
        PushState();
        return null;
    }

    object? SetBetaUpdates(bool enabled)
    {
        Prefs.BetaUpdates = enabled;
        Prefs.Save();
        // Turned on: the beta out now comes with the next check, at once rather than within the hour.
        if (enabled)
            _nextUpdateCheck = DateTime.UtcNow;
        PushState();
        return null;
    }

    /// <summary>The PC woke up less than a minute ago, or has no network: auto-rejoin holds off.</summary>
    bool Settling => DateTime.UtcNow - _resumedAt < ResumeGrace || !NetworkUp;

    /// <summary>
    /// Kept up to date by Windows' notification: asking directly lists every network adapter, too slow for a check
    /// that runs on each process tick.
    /// </summary>
    volatile bool _networkUp = NetworkInterface.GetIsNetworkAvailable();

    void OnNetworkAvailabilityChanged(object? sender, NetworkAvailabilityEventArgs e) => _networkUp = e.IsAvailable;

    DateTime _networkAskedAt;
    static readonly TimeSpan NetworkAskInterval = TimeSpan.FromSeconds(30);

    /// <summary>
    /// Windows' word on the network, asked again every 30 s while it says "down": an "available" notification that
    /// never came (a wake, a Wi-Fi flap, two notifications in the wrong order) would otherwise hold auto-rejoin and
    /// the sign-in check back until the next change.
    /// </summary>
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

    /// <summary>
    /// After sleep every client has lost its connection and the network takes a moment to come back. Disconnect
    /// detection and auto-rejoin wait for it, so the clients aren't all restarted into a dead network at once.
    /// </summary>
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

    void RefreshPresenceSoon(TimeSpan delay)
    {
        var at = DateTime.UtcNow + delay;
        if (at < _nextPresence)
            _nextPresence = at;
    }

    /// <summary>
    /// Asks Roblox which accounts are online and what they are playing. One request covers everyone;
    /// accounts whose game is hidden by privacy settings are asked again with their own cookie.
    /// </summary>
    async Task<object?> RefreshPresenceAsync()
    {
        var vault = _vault;
        if (vault is null)
            return null;
        if (_presenceBusy)
            return new { skipped = true }; // one is under way; its answer is on its way to the page
        // In the background (nobody is looking) only the clients open here whose log isn't followed are asked —
        // auto-rejoin relies on their status — and everyone else waits for the full round every few minutes.
        bool light = _background && _presenceLoaded && DateTime.UtcNow - _lastFullPresence < BackgroundPresenceInterval;
        var accounts = vault.Data.Accounts
            .Where(a => a.UserId > 0 && (!light || _running.TryGetValue(a.Id, out var run) && !LogFollowed(run.Pid)))
            .ToList();
        _nextPresence = DateTime.UtcNow + PresenceInterval;
        if (accounts.Count == 0)
            return null;

        _presenceBusy = true;
        var viewer = accounts.FirstOrDefault(a => a.CookieValid != false && a.Cookie.Length > 0 && a.Realm == Realm.Global);
        try
        {
            // Roblox's accounts only: Roblox and Roblox VNG count user ids apart, so a Roblox VNG account's number is
            // someone else on Roblox (whose game its card showed, and whose server "join" went to, when its own question
            // below wasn't asked or failed). Its status comes from Roblox VNG, with its own sign-in, below.
            var result = await RobloxApi.GetPresenceAsync(viewer?.Cookie, accounts.Where(a => a.Realm == Realm.Global).Select(a => a.UserId).ToList());
            // First answer since unlocking: show it now instead of "status unknown" until every follow-up request
            // (accounts hidden by privacy, game names) is back; those fill in below.
            if (!_presenceLoaded && _vault == vault)
            {
                foreach (var (userId, presence) in result)
                    _presence[userId] = presence;
                _presenceLoaded = true;
                _bootStatusDone = true;
                AdvanceBoot();
                PushStateIfChanged();
            }

            // Privacy settings can hide an account from the others. Accounts with a client open here are asked with
            // their own cookie (auto-rejoin relies on it) unless that client's log is followed, which already says
            // what it is doing; others only when their game is hidden.
            var askOwn = accounts.Where(a => a != viewer && a.CookieValid != false && a.Cookie.Length > 0
                                             && !(_running.TryGetValue(a.Id, out var run) && LogFollowed(run.Pid))
                                             && (a.Realm == Realm.Vng || result.TryGetValue(a.UserId, out var p)
                                                 && (_running.ContainsKey(a.Id) ? p.Type != 2 || p.PlaceId is null : p is { Type: 2, PlaceId: null })))
                .ToList();
            askOwn = askOwn.Where(a => _running.ContainsKey(a.Id)).Concat(askOwn.Where(a => !_running.ContainsKey(a.Id)).Take(10)).ToList();
            var reliable = result.Where(r => r.Value.Type == 2).Select(r => r.Key).ToHashSet();
            if (viewer is not null)
                reliable.Add(viewer.UserId);
            // Accounts whose own request was rejected: at most one is marked per round, after a second call agrees.
            var rejected = new List<Account>();
            using (var gate = new SemaphoreSlim(4))
            {
                await Task.WhenAll(askOwn.Select(async account =>
                {
                    await gate.WaitAsync();
                    try
                    {
                        var own = await RobloxApi.GetPresenceAsync(account.Cookie, [account.UserId], account.Realm);
                        if (own.TryGetValue(account.UserId, out var p))
                        {
                            result[account.UserId] = p;
                            reliable.Add(account.UserId);
                        }
                    }
                    catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
                    {
                        // Keep what the other account saw; auto-rejoin won't act on it.
                        if (ex is RobloxApiException { CookieInvalid: true })
                            rejected.Add(account);
                    }
                    finally
                    {
                        gate.Release();
                    }
                }));
            }
            if (rejected.Count > 0 && _vault == vault)
                await ConfirmSignedOutAsync(rejected[0]); // the rest are asked again next round

            await LoadGamesAsync(result.Values
                .Where(p => (p.RootPlaceId ?? p.PlaceId) > 0)
                .Select(p => ((p.RootPlaceId ?? p.PlaceId)!.Value, p.UniverseId ?? 0))
                .Concat(_running.Values.Select(r => r.Launch).Concat(_pending)
                    .Where(l => l.Target is { PlaceId: > 0 })
                    .Select(l => (l.Target!.PlaceId, 0L)))
                // Many games move players to another place of the same game after joining; the log has that one.
                .Concat(_running.Values.Select(r => _logs.StateOf(r.Pid)?.PlaceId ?? 0).Where(id => id > 0).Select(id => (id, 0L)))
                .ToList());

            if (_vault != vault)
                return null;
            // A light round only updates the accounts it asked about.
            if (!light)
            {
                _presence.Clear();
                _presenceReliable.Clear();
                _lastFullPresence = DateTime.UtcNow;
            }
            foreach (var (userId, presence) in result)
                _presence[userId] = presence;
            _presenceReliable.UnionWith(reliable);
            _presenceLoaded = true;
            DetectDisconnectedSessions(reliable);
            PushStateIfChanged();
        }
        // The account asking was signed out meanwhile: it is marked so (the next check asks with another one) — once
        // a second call agrees, so an outage of Roblox's sign-in service doesn't sign everyone out here one by one.
        catch (RobloxApiException ex) when (ex.CookieInvalid && viewer is not null && _vault == vault)
        {
            await ConfirmSignedOutAsync(viewer);
        }
        catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
        {
            _nextPresence = DateTime.UtcNow + PresenceInterval * 2;
        }
        finally
        {
            _presenceBusy = false;
            // A failed first try leaves the accounts as "status unknown" instead of loading forever.
            if (!_presenceLoaded)
            {
                _bootStatusDone = true;
                AdvanceBoot();
                PushStateIfChanged();
            }
        }
        return null;
    }

    /// <summary>
    /// One rejected request isn't proof that an account was signed out (an outage of Roblox's sign-in service answers
    /// 401 for everyone): it is marked so only when the cheap "who am I" call, the one the sign-in check uses, rejects
    /// the cookie too, and Roblox accepts another account's sign-in at the same time. Without a clear answer it is left
    /// as it was; one accepted after all is signed in (again).
    /// </summary>
    /// <param name="trusted">The daily check already ruled out an outage (or trusts its second run): no other account is asked.</param>
    async Task<bool> ConfirmSignedOutAsync(Account account, CancellationToken ct = default, bool trusted = false)
    {
        string cookie = account.Cookie;
        try
        {
            if (await RobloxApi.GetAuthenticatedUserAsync(cookie, account.Realm, ct) is not null)
            {
                if (account.Cookie == cookie && account.CookieValid == false)
                {
                    SignInAccepted(account);
                    PersistSoon();
                }
                return false;
            }
        }
        catch (Exception ex) when (!ct.IsCancellationRequested && RobloxApi.IsExpectedError(ex))
        {
            return false;
        }
        // Signed in again meanwhile: the old cookie's answer says nothing about the new one.
        if (account.Cookie != cookie)
            return false;
        // Before, this call alone decided: in an outage each request that met it marked one more account (one a status
        // round, every account a preset tried), and a rejoin turned auto-rejoin off.
        if (!trusted && await SignInServiceDownAsync(account, ct))
            return false;
        if (account.Cookie != cookie)
            return false;
        account.CookieValid = false;
        PersistSoon();
        return true;
    }

    /// <summary>
    /// Whether Roblox's sign-in service rejects everyone just now (an outage), by asking up to two other accounts held
    /// signed in, of the same Roblox (Roblox VNG's is another service), the last checked first. False when one is
    /// accepted, or when there is none to ask (one account, the rest signed out: no telling, so a rejection counts as
    /// before); true when those asked were all rejected or gave no answer.
    /// </summary>
    async Task<bool> SignInServiceDownAsync(Account rejected, CancellationToken ct)
    {
        var others = (_vault?.Data.Accounts ?? [])
            .Where(a => a != rejected && a.Realm == rejected.Realm && a.CookieValid != false && a.Cookie.Length > 0 && a.Cookie != rejected.Cookie)
            .OrderByDescending(a => a.CookieValid == true)
            .ThenByDescending(a => a.LastChecked ?? DateTime.MinValue)
            .Take(2)
            .ToList();
        if (others.Count == 0)
            return false;
        foreach (var other in others)
        {
            try
            {
                if (await RobloxApi.GetAuthenticatedUserAsync(other.Cookie, other.Realm, ct) is not null)
                    return false;
            }
            catch (Exception ex) when (!ct.IsCancellationRequested && RobloxApi.IsExpectedError(ex))
            {
            }
        }
        return true;
    }
}
