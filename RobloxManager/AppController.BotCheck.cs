namespace RobloxManager;

/// <summary>
/// Roblox's bot check on a join (the card's "Roblox wants a bot check", the window held on Roblox's Security page, blank
/// in Vietnam): the account menu's "Solve the bot check" asks Roblox's join for the account itself, shows the check it
/// gets in Bloxkeep's own window (the puzzle and press-and-hold kinds; a proof of work is computed, as roblox.com does),
/// hands the answer to Roblox with the account's sign-in, and asks the join again carrying it. Let in: the held window
/// is closed and opened again into its game. Each step is noted in error.log by kind and field names only, so a check
/// of a kind not handled yet shows what it was.
/// </summary>
public sealed partial class AppController
{
    async Task<object?> SolveBotCheckAsync(Guid id)
    {
        var vault = RequireVault();
        var account = RequireAccount(id);
        if (!_running.TryGetValue(id, out var run) || run.Launch.Target is not { } target || _logs.JoinChallengedAt(run.Pid) is null)
            throw new UserError(L.F("{0} isn't held by a bot check right now.", Name(account)));
        long place = _logs.StateOf(run.Pid)?.PlaceId ?? target.PlaceId;

        var first = await RobloxApi.JoinCheckAsync(account.Cookie, place, target.JobId, account.Realm);
        if (first.Challenge is not { } challenge)
        {
            Log.Note($"Bot check asked about: no check now (http {first.Http}, status {first.Status}).");
            Reopen();
            Toast(L.F("Roblox no longer asks {0} for a check. Opening its window again…", Name(account)), "ok");
            return null;
        }

        string? answer = challenge.Type switch
        {
            "proofofwork" when challenge.Meta("sessionId") is { Length: > 0 } session =>
                challenge.MetaWith("redemptionToken", await RobloxApi.ProofOfWorkTokenAsync(account.Realm, session)),
            "captchav2" when challenge.Meta("challengeId") is { Length: > 0 } human =>
                ShowCaptcha(CaptchaForm.Human(account.Realm, human)) is { } redemption ? challenge.MetaWith("redemptionToken", redemption) : null,
            "captcha" when challenge.Meta("dataExchangeBlob") is { Length: > 0 } blob && challenge.Meta("unifiedCaptchaId") is { Length: > 0 } captchaId =>
                await ArkoseAnswerAsync(account, challenge, blob, challenge.Meta("actionType") ?? "Generic"),
            _ => throw new UserError(L.F("Roblox asked {0} for a kind of check Bloxkeep can't show yet ({1}). It was noted, to be added.",
                Name(account), challenge.Type.Length > 0 ? challenge.Type : "?")),
        };
        // The check's window closed before it was solved: nothing goes to Roblox.
        if (answer is null || _vault != vault)
            return null;

        // Roblox's challenge/continue, as a sign-in's check is answered. A join's check (2026-10-27) names the request it
        // holds (requestPath, requestMethod) and continue refused it with 403 "an internal error occurred", with the
        // answer in either shape: then the join itself is asked again carrying the answer, as that request's retry.
        var solved = challenge with { MetadataJson = answer };
        RobloxApi.JoinChallenge passed;
        try
        {
            passed = await RobloxApi.ContinueChallengeAsync(account.Realm, account.Cookie, solved);
        }
        catch (RobloxApiException ex) when (!ex.ServerError && !ex.RateLimited)
        {
            Log.Note("Bot check: continue refused; asking the join again carrying the answer.");
            passed = solved;
        }
        var again = await RobloxApi.JoinCheckAsync(account.Cookie, place, target.JobId, account.Realm, answered: passed);
        Log.Note($"Bot check answered: the join asked again got http {again.Http}, status {again.Status}{(again.Challenge is null ? "" : ", held again")}.");
        if (again.Challenge is not null || again.Http == 403)
            throw new UserError(L.F("The check was solved, but Roblox still won't let {0} join.", Name(account)));
        Reopen();
        Toast(L.F("Check passed. Opening {0}'s window again…", Name(account)), "ok");
        return null;

        // The held window is closed and opened again into its game (as a preset's restart does: whatever auto-rejoin says).
        void Reopen()
        {
            if (!_running.TryGetValue(id, out var now) || now.Pid != run.Pid)
                return;
            _joinRefused.Remove(now.Launch);
            _restarts[id] = (RestartTarget(now), DateTime.UtcNow);
            _closingForRejoin.Add(now.Pid);
            _closeForReopen(now.Pid, InGameNow(now.Pid));
            PushState();
        }
    }

    /// <summary>Closes a window Bloxkeep opens again at once (a field, so the checks don't close a real process).</summary>
    Action<int, bool> _closeForReopen = RobloxProcesses.Close;

    /// <summary>The puzzle kind of check, in Bloxkeep's captcha window; null when it was closed unsolved.</summary>
    async Task<string?> ArkoseAnswerAsync(Account account, RobloxApi.JoinChallenge challenge, string blob, string actionType)
    {
        string key = await RobloxApi.GetCaptchaKeyAsync(account.Realm, actionType)
            ?? throw new UserError(L.F("Roblox asked {0} for a kind of check Bloxkeep can't show yet ({1}). It was noted, to be added.", Name(account), actionType));
        return ShowCaptcha(CaptchaForm.Arkose(account.Realm, key, blob)) is { } token ? CaptchaAnswer(challenge, token) : null;
    }

    /// <summary>
    /// The check's own metadata with the solved puzzle's token in captchaToken. A join's check carries more than a
    /// sign-in's (requestPath, requestMethod, sharedParameters, dataExchangeBlob); sent back with only unifiedCaptchaId,
    /// captchaToken and actionType, Roblox answered 403 "an internal error occurred" (2026-10-27).
    /// </summary>
    static string CaptchaAnswer(RobloxApi.JoinChallenge challenge, string token) => challenge.MetaWith("captchaToken", token);
}
