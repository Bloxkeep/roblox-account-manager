namespace RobloxManager;

/// <summary>
/// Each account's Robux on its card: asked with the account's own cookie (Roblox VNG's service for a VNG account), a
/// few accounts at a time, right at unlocking (alongside the loading screen's status and avatars) and then every half
/// hour while unlocked; an account not asked yet (just added, or signed in again) at the next look. Kept in memory only,
/// never in the vault or a log. An account that gets no answer (signed out, offline, a refusal) keeps the last number it
/// had. Until its first answer the card keeps the number's place ("…"), so the row doesn't move when it comes.
/// </summary>
public sealed partial class AppController
{
    static readonly TimeSpan RobuxInterval = TimeSpan.FromMinutes(30);
    /// <summary>After a round Roblox cut short (too many requests) or that couldn't reach it: the next one comes sooner.</summary>
    static readonly TimeSpan RobuxRetry = TimeSpan.FromMinutes(2);
    /// <summary>Accounts asked at once (each with its own cookie), each slot pausing a moment between two.</summary>
    const int RobuxAtOnce = 4;
    static readonly TimeSpan RobuxSpacing = TimeSpan.FromMilliseconds(250);

    readonly Dictionary<Guid, long> _robux = new();
    /// <summary>Accounts asked (and answered, or not) since this vault opened: the others are asked at the next look.</summary>
    readonly HashSet<Guid> _robuxAsked = new();
    /// <summary>The vault the numbers belong to: another one opened (a restored backup) starts over.</summary>
    Vault? _robuxVault;
    DateTime _nextRobux;
    bool _robuxBusy;

    static bool HasRobux(Account a) => a.UserId > 0 && a.Cookie.Length > 0 && a.CookieValid != false && a.Banned != true;

    /// <summary>From the process timer, and at unlocking: starts a round once it is due, or for accounts not asked yet.</summary>
    void RefreshRobuxIfDue(DateTime now)
    {
        if (_vault is not { } vault)
            return;
        if (_robuxVault != vault)
        {
            _robuxVault = vault;
            _robux.Clear();
            _robuxAsked.Clear();
            _nextRobux = now;
        }
        if (_robuxBusy || !NetworkUp)
            return;
        var accounts = vault.Data.Accounts.Where(HasRobux).ToList();
        if (now >= _nextRobux)
            _nextRobux = now + RobuxInterval;
        else if ((accounts = accounts.Where(a => !_robuxAsked.Contains(a.Id)).ToList()).Count == 0)
            return;
        _ = RefreshRobuxAsync(vault, accounts);
    }

    async Task RefreshRobuxAsync(Vault vault, List<Account> accounts)
    {
        _robuxBusy = true;
        bool cutShort = false;
        try
        {
            using var gate = new SemaphoreSlim(RobuxAtOnce);
            await Task.WhenAll(accounts.Select(async account =>
            {
                await gate.WaitAsync();
                try
                {
                    if (cutShort || _vault != vault || _disposed)
                        return;
                    try
                    {
                        if (await RobloxApi.GetRobuxAsync(account.Cookie, account.Realm, account.UserId) is { } robux && _vault == vault)
                            _robux[account.Id] = robux;
                    }
                    catch (RobloxApiException ex) when (ex.RateLimited)
                    {
                        cutShort = true; // the rest wait for the next round, a little later
                    }
                    catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
                    {
                        // Offline or refused: the card keeps what it showed; the next round asks again.
                        cutShort |= !NetworkUp;
                    }
                    if (_vault == vault)
                    {
                        _robuxAsked.Add(account.Id);
                        PushStateIfChanged(); // its number, or its place given back
                    }
                    await Task.Delay(RobuxSpacing);
                }
                finally
                {
                    gate.Release();
                }
            }));
            if (cutShort)
            {
                // Asked again soon, those not asked included (they show nothing meanwhile rather than waiting).
                foreach (var account in accounts)
                    _robuxAsked.Add(account.Id);
                var soon = DateTime.UtcNow + RobuxRetry;
                if (soon < _nextRobux)
                    _nextRobux = soon;
                PushStateIfChanged();
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

    /// <summary>The account's first number is on its way: the card keeps its place.</summary>
    bool RobuxWaiting(Account a) =>
        _robuxVault == _vault && !_robux.ContainsKey(a.Id) && !_robuxAsked.Contains(a.Id) && HasRobux(a) && _networkUp;
}
