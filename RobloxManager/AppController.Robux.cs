namespace RobloxManager;

/// <summary>
/// Each account's Robux on its card: asked with the account's own cookie (Roblox VNG's service for a VNG account), one
/// account at a time, a little after unlocking and then every half hour while unlocked. Kept in memory only, never in
/// the vault or a log. An account that gets no answer (signed out, offline, a refusal) keeps the last number it had.
/// </summary>
public sealed partial class AppController
{
    static readonly TimeSpan RobuxInterval = TimeSpan.FromMinutes(30);
    /// <summary>Right after unlocking, status and avatars are being loaded; Robux waits its turn.</summary>
    static readonly TimeSpan RobuxStartDelay = TimeSpan.FromSeconds(20);
    static readonly TimeSpan RobuxSpacing = TimeSpan.FromSeconds(1);

    readonly Dictionary<Guid, long> _robux = new();
    /// <summary>The vault the numbers belong to: another one opened (a restored backup) starts over.</summary>
    Vault? _robuxVault;
    DateTime _nextRobux;
    bool _robuxBusy;

    /// <summary>From the process timer: starts a round once it is due.</summary>
    void RefreshRobuxIfDue(DateTime now)
    {
        if (_vault is not { } vault)
            return;
        if (_robuxVault != vault)
        {
            _robuxVault = vault;
            _robux.Clear();
            _nextRobux = now + RobuxStartDelay;
            return;
        }
        if (_robuxBusy || _boot is not null || !NetworkUp || now < _nextRobux)
            return;
        _nextRobux = now + RobuxInterval;
        _ = RefreshRobuxAsync(vault);
    }

    async Task RefreshRobuxAsync(Vault vault)
    {
        _robuxBusy = true;
        try
        {
            var accounts = vault.Data.Accounts.Where(a => a.UserId > 0 && a.Cookie.Length > 0 && a.CookieValid != false && a.Banned != true).ToList();
            foreach (var account in accounts)
            {
                if (_vault != vault || _disposed)
                    return;
                try
                {
                    if (await RobloxApi.GetRobuxAsync(account.Cookie, account.Realm, account.UserId) is { } robux
                        && (!_robux.TryGetValue(account.Id, out long had) || had != robux))
                    {
                        _robux[account.Id] = robux;
                        PushStateIfChanged();
                    }
                }
                catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
                {
                    // Offline or refused: the card keeps what it showed; the next round asks again.
                }
                await Task.Delay(RobuxSpacing);
            }
        }
        // Background work must never take the app down; the next round tries again.
        catch (Exception ex)
        {
            Log.Write(ex);
        }
        finally
        {
            _robuxBusy = false;
        }
    }

    long? RobuxOf(Account a) => _robux.TryGetValue(a.Id, out long robux) ? robux : null;
}
