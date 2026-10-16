namespace RobloxManager;

/// <summary>
/// The lock screen after a while without use (Settings › Security › Lock automatically), like Windows' own: when nobody
/// has used the PC's keyboard or mouse for the chosen time (anti-AFK's own nudges don't count), the page shows the lock
/// screen and asks for the master password again. The vault stays open behind it, so Roblox windows, auto-rejoin,
/// scheduled presets and Discord carry on; nothing of it reaches the page meanwhile (no accounts, settings, toasts or
/// figures), the commands that would show or change it are refused, and what shows an account on screen goes (the
/// sign-in and Quick Login windows, websites signed in as one, a secret on the clipboard, the monitor tab's live
/// pictures). The Lock button still closes
/// Bloxkeep for real.
/// </summary>
public sealed partial class AppController
{
    /// <summary>The choices Settings offers, in minutes; 0 never.</summary>
    static readonly int[] AutoLockChoices = [0, 5, 10, 15, 30, 60, 120];

    /// <summary>The lock screen is up with the vault open behind it.</summary>
    bool _screenLocked;

    /// <summary>
    /// When Bloxkeep last knew someone was there without Windows' input clock saying so (an unlock, the setting
    /// chosen): the wait counts from the later of the two.
    /// </summary>
    DateTime _activeAt = DateTime.UtcNow;

    /// <summary>The vault, for what the page may see: null while locked, the screen too.</summary>
    Vault? Open => _screenLocked ? null : _vault;

    /// <summary>Whether the lock screen is due: a time chosen, and nobody at the keyboard or mouse that long.</summary>
    public static bool AutoLockDue(int minutes, TimeSpan idle) => minutes > 0 && idle >= TimeSpan.FromMinutes(minutes);

    /// <summary>
    /// The periodic check: idle is how long since a person last used the keyboard or mouse (AntiAfk.IdleTime), now
    /// the time of the check.
    /// </summary>
    void CheckAutoLock(TimeSpan idle, DateTime now)
    {
        if (_vault is not { } vault || _screenLocked)
            return;
        var sinceActive = now - _activeAt;
        if (AutoLockDue(vault.Data.Settings.AutoLockMinutes, sinceActive < idle ? sinceActive : idle))
            ScreenLock();
    }

    /// <summary>Puts the lock screen up, the vault staying open behind it (see the class summary).</summary>
    void ScreenLock()
    {
        _screenLocked = true;
        QuickLoginCancel();
        LoginCancel();
        DropVaultImport();
        CloseWebsites(); // signed in as saved accounts
        Ui.ClearSecretClipboard();
        SetMonitorArea?.Invoke(null); // the live pictures of the Roblox windows go at once, whatever the page does next
        _pageView = null; // what the page showed doesn't come back without the password, nor after a restart
        Log.Note($"Locked the screen after {_vault?.Data.Settings.AutoLockMinutes} min without use; Roblox and the background carry on.");
        PushState();
    }

    /// <summary>The lock screen's password, checked against the open vault's key; the page gets everything again.</summary>
    async Task<object?> UnlockScreenAsync(Vault open, string password)
    {
        if (!_screenLocked)
            return BuildState();
        if (password.Length == 0)
            throw new UserError(L.T("Enter your master password."));
        bool right = await Task.Run(() => open.VerifyPassword(password));
        // Locked for real (or unlocked by another try) meanwhile: that stands.
        if (!ReferenceEquals(_vault, open) || !_screenLocked)
            return BuildState();
        if (!right)
            throw new UserError(L.T("Wrong master password.")); // as the full unlock says it
        _screenLocked = false;
        _activeAt = DateTime.UtcNow;
        _weakPassword = Backup.WeakPassword(password);
        _lastMetrics = null; // none went out while locked: the next check sends them again
        Log.Note("Unlocked the screen.");
        return BuildState();
    }
}
