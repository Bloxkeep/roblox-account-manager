namespace RobloxManager;

/// <summary>Whether a new Roblox window can be a launch's, by the account its log says is signed in.</summary>
public sealed partial class AppController
{
    /// <summary>
    /// False when the window's log names another signed-in user than <paramref name="account"/> (its join report, see
    /// <see cref="RobloxLogWatcher.UserIdOf"/>): a late launch mustn't take a window started by hand as someone else (audit
    /// R11). True while the log names nobody yet (not in a game, signed out since, no log): the launch's own checks decide.
    /// A Global and a Roblox VNG account may share the number; the log can't tell those apart, so the number is all that counts.
    /// </summary>
    bool ClaimUserMatches(int pid, Account account) => _logs.UserIdOf(pid) is not { } user || user == account.UserId;
}
