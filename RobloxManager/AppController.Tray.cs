using System.Text.Json;

namespace RobloxManager;

/// <summary>
/// What the tray icon's menu (<see cref="TrayMenu"/>) shows and does with Bloxkeep in the tray: Multi-Roblox's state
/// (and turning it off), a new Roblox window, the accounts and presets to open, and closing every Roblox, the same
/// actions as the page's. With Multi-Roblox only (no vault, see <see cref="MultiOnly"/>) Multi-Roblox turns on and off
/// from here as well.
/// </summary>
public sealed partial class AppController
{
    /// <param name="Open">A vault is open and the screen isn't locked: accounts, presets and closing Roblox are offered.</param>
    /// <param name="Multi">"off", "ready" (Multi-Roblox holds Roblox's lock) or "notReady".</param>
    /// <param name="Accounts">The accounts to open, most recently used first (at most 12), and whether each is open.</param>
    /// <param name="MultiOnly">No vault on this PC (Multi-Roblox only): Multi-Roblox turns on and off from the menu.</param>
    public sealed record TrayInfo(bool Open, string Multi, int RobloxCount, List<(Guid Id, string Name)> Presets,
        List<(Guid Id, string Name, bool Running)>? Accounts = null, bool MultiOnly = false);

    const int TrayAccounts = 12;

    public TrayInfo TrayState()
    {
        bool open = _vault is not null && !_screenLocked;
        bool on = open ? _vault!.Data.Settings.MultiInstance : Prefs.MultiRobloxAtStart;
        var presets = open ? _vault!.Data.Settings.Presets.Select(p => (p.Id, p.Name)).ToList() : [];
        var accounts = open
            ? _vault!.Data.Accounts.Where(a => a.UserId > 0 && a.Banned != true && a.Realm != Realm.Vng).OrderByDescending(a => a.LastUsed ?? DateTime.MinValue)
                .Take(TrayAccounts).Select(a => (a.Id, NameText(a), _running.ContainsKey(a.Id))).ToList()
            : [];
        return new TrayInfo(open, !on ? "off" : MultiInstance.Armed ? "ready" : "notReady", RobloxCount, presets, accounts, !Vault.Exists);
    }

    /// <summary>
    /// The menu's Multi-Roblox item: off while it is on, as the sidebar's switch does (on asks first: the page). With
    /// Multi-Roblox only, on and off both (its warning was answered when that was chosen).
    /// </summary>
    public void TrayMulti(bool on)
    {
        if (!Vault.Exists)
        {
            MultiOnly(on);
            return;
        }
        if (on || _vault is null || _screenLocked)
            return;
        SetSettings(JsonSerializer.SerializeToElement(new { multiInstance = false }));
        PushState();
    }

    /// <summary>
    /// First start › "Just use Multi-Roblox": Multi-Roblox without a vault or any account, for someone who signs in to
    /// Roblox only in their own browser. It is kept on from start-up (<see cref="Prefs.MultiRobloxAtStart"/>); a vault
    /// made later takes the setting over.
    /// </summary>
    object? MultiOnly(bool on)
    {
        if (Vault.Exists)
            throw new UserError(L.T("Bloxkeep has a vault on this PC: Multi-Roblox is turned on or off inside it."));
        Prefs.MultiRobloxAtStart = on;
        Prefs.Save();
        if (on)
            MultiInstance.Enable();
        else
            MultiInstance.Disable();
        _lastArmed = MultiInstance.Armed;
        PushState();
        return null;
    }

    RobloxHomeSession? _homeSession;
    bool _openingHome;
    DateTime _homeStartedAt;
    static readonly string HomeAliases = Path.Combine(Vault.DataDir, "roblox-client-paths");

    /// <summary>
    /// A new sign-in window without an account or ticket from Bloxkeep. The installed client is opened directly
    /// through a junction, retaining its installed mods; shared sign-in files stay locked until clients using these aliases close.
    /// </summary>
    public async void TrayNewRoblox()
    {
        if (_disposed || _openingHome || _applyingUpdate || _relaunching)
            return;
        void Tell(string text)
        {
            if (_disposed) return;
            Toast(text, "warn");
            ShowNotification?.Invoke(text, "warn");
        }
        if (OpeningRoblox)
        {
            Tell(L.T("Wait until Bloxkeep finishes opening Roblox."));
            return;
        }
        bool acquired = false, started = false;
        _openingHome = true;
        try
        {
            if (!HomeLaunchReady(Tell))
                return;
            var settings = _vault?.Data.Settings;
            string? client = RobloxInstalls.ClientFor(settings?.LaunchVia ?? "auto", settings?.CustomLauncher ?? "");
            if (client is null)
            {
                Tell(L.T("Roblox isn't installed on this PC."));
                return;
            }
            if (RobloxInstalls.IsStoreClient(client))
            {
                Tell(L.T("Separate sign-in windows currently need Roblox from roblox.com, rather than the Microsoft Store edition."));
                return;
            }
            if (_homeSession is null && !Ui.Modal(() => Dialog.Confirm(_owner,
                L.T("Separate sign-in windows are experimental. All Roblox windows temporarily stop saving sign-ins and some settings. Your usual saved sign-in may expire. If Bloxkeep quits or crashes while a separate window stays open, that window may replace the saved sign-in. Close separate windows before quitting Bloxkeep. Long sessions have not been verified."),
                L.T("Open separate window"), L.T("Cancel"), danger: true)))
                return;
            if (!HomeLaunchReady(Tell))
                return;
            if (RobloxProcesses.CloseTrayCopies() > 0)
                await Task.Delay(250);
            if (_homeSession is null)
            {
                if (RobloxProcesses.GetPlayerPids().Length == 0)
                    RobloxHomeSession.CleanupAliases(HomeAliases);
                string storage = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Roblox", "LocalStorage");
                // Roblox can have one of these files open briefly while saving. Each failed attempt releases both.
                for (int attempt = 0; ; attempt++)
                {
                    if (!HomeLaunchReady(Tell))
                        return;
                    try
                    {
                        _homeSession = new RobloxHomeSession(storage, HomeAliases);
                        acquired = true;
                        break;
                    }
                    catch (IOException) when (attempt < 9)
                    {
                        await Task.Delay(100);
                    }
                }
            }
            if (!HomeLaunchReady(Tell))
            {
                if (acquired)
                {
                    _homeSession?.Dispose();
                    _homeSession = null;
                }
                return;
            }
            _homeStartedAt = DateTime.UtcNow;
            _homeSession.Start(client);
            started = true;
            _lastScanFoundNone = false;
            _lastScan = DateTime.MinValue;
            if (acquired)
            {
                string text = L.T("A separate Roblox sign-in window is opening. Keep Bloxkeep running; these windows don't save their sign-ins when closed.");
                Toast(text, "info");
                ShowNotification?.Invoke(text, "info");
            }
        }
        catch (Exception ex) when (ex is System.ComponentModel.Win32Exception or IOException or UnauthorizedAccessException)
        {
            if (acquired && !started)
            {
                _homeSession?.Dispose();
                _homeSession = null;
            }
            Log.Write(ex);
            Tell(L.T("Couldn't open a separate Roblox sign-in window. Try again, or close Roblox and retry."));
        }
        finally
        {
            _openingHome = false;
            if (!_disposed)
                PushState();
        }
    }

    /// <summary>No automatic or manual update may drop the file locks while separate sign-ins are in use.</summary>
    bool HomeSessionsActive => _homeSession is not null;

    bool HomeLaunchReady(Action<string> tell)
    {
        if (_disposed)
            return false;
        var state = TrayState();
        if (state.Multi == "ready")
            return true;
        tell(state.Multi == "off"
            ? L.T("Turn on Multi-Roblox before opening a separate Roblox sign-in window.")
            : MultiNotReadyText());
        return false;
    }

    void ReleaseHomeSessionIfIdle(bool scan, IReadOnlyList<RobloxProcesses.PlayerProcess> players, HashSet<int> trays, DateTime now)
    {
        if (!scan || _homeSession is null || OpeningRoblox || now - _homeStartedAt < TimeSpan.FromSeconds(15))
            return;
        var (pids, uncertain) = _homeSession.FindClients(players);
        // Only our own windowless copies are ended. A normal client or tray copy never extends this session's life.
        foreach (int pid in pids.Where(trays.Contains))
            RobloxProcesses.Kill(pid);
        // Even a tray we just ended stays counted until a later snapshot confirms it really exited.
        if (pids.Length > 0 || uncertain)
            return;
        _homeSession.Dispose();
        _homeSession = null;
        RobloxHomeSession.CleanupAliases(HomeAliases);
    }

    /// <summary>
    /// An account from the menu, as the account card's Play: into the game chosen in the dock, else Roblox's home
    /// screen. A launch with something to ask first isn't opened from here: that is said, to be done from Bloxkeep.
    /// </summary>
    public async void TrayOpenAccount(Guid id)
    {
        void Tell(string text)
        {
            Toast(text, "warn");
            ShowNotification?.Invoke(text, "warn");
        }
        try
        {
            if (_vault is not { } vault || _screenLocked || FindAccount(id) is not { } account)
                return;
            var s = vault.Data.Settings;
            GameTarget? target = null;
            if (s.LastPlaceInput.Length > 0 && !GameTarget.TryParse(s.LastPlaceInput, s.LastJobId, out target, out string error))
                throw new UserError(error);
            CheckCanLaunch([account]);
            if (await LaunchAccountsAsync(vault, [account], target, force: false, s.LaunchDelaySeconds) is { NeedsConfirm: true } result)
                Tell(L.T("Opening this account asks something first. Open it from Bloxkeep.") + "\n\n" + result.Message);
        }
        catch (UserError ex)
        {
            Tell(ex.Message);
        }
        catch (Exception ex)
        {
            Tell(ErrorText(ex));
        }
    }

    /// <summary>
    /// A preset from the menu, as the Presets tab's Run. A launch with something to ask first (Multi-Roblox off, low
    /// RAM…) isn't run from here: that is said, for it to be run from Bloxkeep, where the question shows.
    /// </summary>
    public async void TrayRunPreset(Guid id)
    {
        void Tell(string text)
        {
            Toast(text, "warn");
            ShowNotification?.Invoke(text, "warn");
        }
        try
        {
            if (_vault is null || _screenLocked)
                return;
            if (await StartPresetAsync(id, force: false) is { NeedsConfirm: true } result)
                Tell(L.T("This preset asks something before it opens. Run it from Bloxkeep's Presets tab.") + "\n\n" + result.Message);
        }
        catch (UserError ex)
        {
            Tell(ex.Message);
        }
        catch (Exception ex)
        {
            Tell(ErrorText(ex));
        }
    }

    /// <summary>The vault's Multi-Roblox setting, copied for the next start (<see cref="Prefs.MultiRobloxAtStart"/>).</summary>
    static void KeepMultiAtStart(bool on)
    {
        if (Prefs.MultiRobloxAtStart == on)
            return;
        Prefs.MultiRobloxAtStart = on;
        Prefs.Save();
    }
}
