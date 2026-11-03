using System.Net.NetworkInformation;
using Microsoft.Win32;

namespace RobloxManager;

/// <summary>Bloxkeep's own updates and What's new.</summary>
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

    /// <summary>Remembers which version last ran. Older than this one: the page shows the release notes in between, once.
    /// A fresh install has nothing remembered and shows nothing.</summary>
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
        // Turned on: the beta out now comes with the next check, at once rather than within the hour. Turned off with a beta
        // downloaded: that one goes, and the regular versions are looked at again.
        if (enabled || _updater.BetaTurnedOff())
            _nextUpdateCheck = DateTime.UtcNow;
        PushState();
        return null;
    }

    readonly AppUpdater _updater = new();
    /// <summary>Due at the first tick: an update found before the vault is opened installs at the lock screen
    /// (see <see cref="UpdateOffersAtLock"/>), so the sooner it is known the better.</summary>
    DateTime _nextUpdateCheck = DateTime.UtcNow;
    DateTime _nextRobloxVersionCheck;
    RobloxInstalls.ClientVersion? _latestRoblox;
    /// <summary>The version that ran before an update, until the page has shown what's new since then.</summary>
    string? _updatedFrom;
}
