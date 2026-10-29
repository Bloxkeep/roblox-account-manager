using System.Text.Json;

namespace RobloxManager;

/// <summary>Adding, editing, checking and removing accounts, and what is done to them from the page.</summary>
public sealed partial class AppController
{
    /// <summary>Opens the sign-in window; every account signed in there is saved right away (several in a row).</summary>
    async Task<object?> AddViaBrowserAsync()
    {
        RequireVault();
        var added = new List<AddResult>();
        using (var form = new BrowserLoginForm(async cookie =>
               {
                   var user = await RobloxApi.GetAuthenticatedUserAsync(cookie)
                       ?? throw new UserError(L.T("Roblox didn't accept that sign-in. Please try again."));
                   var result = AddOrMerge(new Account
                   {
                       Cookie = cookie,
                       UserId = user.Id,
                       Username = user.Name,
                       DisplayName = user.DisplayName,
                       CookieValid = true,
                       LastChecked = DateTime.Now,
                   });
                   added.Add(result);
                   return L.F(result.Merged ? "{0} was already here — its sign-in was updated." : "{0} saved.", result.Label);
               }))
        {
            form.ShowDialog(_owner);
        }
        return new { added };
    }

    /// <summary>The code on screen in the Quick Login window; one at a time.</summary>
    QuickLoginCode? _quickLogin;
    bool _quickLoginFinishing;
    int _quickLoginStarts, _quickLoginFinishTries;
    /// <summary>The session a confirmed code was traded for, while its account lookup is being retried.</summary>
    (QuickLoginCode Login, string Cookie)? _quickLoginCookie;

    async Task<object?> QuickLoginStartAsync(string realm)
    {
        RequireVault();
        QuickLoginCancel();
        int start = ++_quickLoginStarts;
        var login = await RobloxApi.CreateQuickLoginAsync(realm);
        // Another code was asked for meanwhile (tab switched, "new code" pressed twice): that one is on screen.
        if (start != _quickLoginStarts || _vault is null)
        {
            _ = RobloxApi.CancelQuickLoginAsync(login);
            return null;
        }
        _quickLogin = login;
        string? qr = await RobloxApi.GetQuickLoginQrAsync(login);
        int seconds = (int)Math.Max(0, (login.ExpiresUtc - DateTime.UtcNow).TotalSeconds);
        return new { code = login.Code, qr, seconds };
    }

    /// <summary>
    /// Called every couple of seconds while the window is open. Once the user confirms on their device the code is
    /// traded for a session and the account is saved, just like signing in through the browser.
    /// </summary>
    async Task<object?> QuickLoginPollAsync()
    {
        if (_quickLogin is not { } login || _quickLoginFinishing)
            return new { status = _quickLoginFinishing ? "adding" : "none" };
        // A session already in hand only needs its account looked up again: the spent code's status no longer matters.
        var status = _quickLoginCookie is { } pending && ReferenceEquals(pending.Login, login)
            ? new QuickLoginStatus("Validated", null, null)
            : await RobloxApi.GetQuickLoginStatusAsync(login);
        if (!ReferenceEquals(login, _quickLogin))
            return new { status = "none" }; // the window asked for a new code meanwhile
        string? picture = Uri.TryCreate(status.PictureUrl, UriKind.Absolute, out var p) && p.Host.EndsWith(".rbxcdn.com") ? status.PictureUrl : null;
        switch (status.Status)
        {
            case "Validated":
                _quickLoginFinishing = true;
                bool retry = false;
                try
                {
                    // The code is spent once traded: a retry after the account lookup failed reuses the session it gave.
                    if (_quickLoginCookie is not { } held || !ReferenceEquals(held.Login, login))
                        _quickLoginCookie = held = (login, await RobloxApi.FinishQuickLoginAsync(login));
                    string cookie = held.Cookie;
                    var user = await RobloxApi.GetAuthenticatedUserAsync(cookie, login.Realm)
                        ?? throw new UserError(L.T("Roblox didn't accept that sign-in. Please try again."));
                    var added = AddOrMerge(new Account
                    {
                        Cookie = cookie,
                        Realm = login.Realm,
                        UserId = user.Id,
                        Username = user.Name,
                        DisplayName = user.DisplayName,
                        CookieValid = true,
                        LastChecked = DateTime.Now,
                    });
                    return new { status = "added", added };
                }
                // A network hiccup or Roblox down right after the person approved on their phone: the next poll tries
                // again a couple of times rather than making them start over. A refusal (403, 429) is final: the code
                // may be spent. Once the session is in hand, only the account lookup is repeated.
                catch (Exception ex) when ((IsNetworkError(ex) || _quickLoginCookie is { } got && ReferenceEquals(got.Login, login)
                                               && RobloxApi.IsExpectedError(ex)) && ++_quickLoginFinishTries < 3)
                {
                    retry = true;
                    return new { status = "adding" };
                }
                finally
                {
                    _quickLoginFinishing = false;
                    if (!retry)
                    {
                        _quickLoginFinishTries = 0;
                        _quickLoginCookie = null;
                        if (ReferenceEquals(login, _quickLogin))
                            _quickLogin = null;
                    }
                }
            case "UserLinked":
                return new { status = "linked", name = status.AccountName, picture };
            case "Cancelled":
            case "Expired":
                if (ReferenceEquals(login, _quickLogin))
                    _quickLogin = null;
                return new { status = status.Status.ToLowerInvariant() };
            default:
                return new { status = "waiting", seconds = (int)Math.Max(0, (login.ExpiresUtc - DateTime.UtcNow).TotalSeconds) };
        }
    }

    object? QuickLoginCancel()
    {
        if (_quickLogin is { } login && !_quickLoginFinishing)
            _ = RobloxApi.CancelQuickLoginAsync(login);
        if (!_quickLoginFinishing)
        {
            _quickLogin = null;
            _quickLoginFinishTries = 0;
            _quickLoginCookie = null;
        }
        ++_quickLoginStarts; // a code still being asked for is cancelled when it arrives
        return null;
    }

    // A two-step prompt or captcha waiting for the user; held only while the sign-in is in progress, never persisted.
    /// <summary>
    /// The password sign-in window closed: drop the half-done sign-in, which still holds the password. A reply still
    /// on its way belongs to the sign-in before this (see <see cref="_loginGeneration"/>) and is dropped too.
    /// </summary>
    object? LoginCancel()
    {
        _loginGeneration++;
        // A check being answered on its own (proof-of-work) stops where it is: its answer would send the password again.
        _loginCts.Cancel();
        _loginCts = new CancellationTokenSource();
        DropLoginPrompts();
        return null;
    }

    /// <summary>Forgets what the sign-in in progress holds: its prompts (with the password) and a session not saved yet.</summary>
    void DropLoginPrompts()
    {
        _twoStep = null;
        _captcha = null;
        _humanCaptcha = null;
        _captchaUsed = false;
        _freshPuzzles = 0;
        _loginCookie = null;
        _twoStepWaitUntil = default;
    }

    RobloxApi.TwoStepPrompt? _twoStep;
    RobloxApi.CaptchaPrompt? _captcha;
    RobloxApi.HumanCaptchaPrompt? _humanCaptcha;
    /// <summary>The puzzle of this sign-in attempt was already shown and closed: it can't be opened again (it comes up blank).</summary>
    bool _captchaUsed;
    /// <summary>Puzzles asked for again in this attempt after their window was closed; each one sends the password again.</summary>
    int _freshPuzzles;
    const int MaxFreshPuzzles = 3;
    /// <summary>
    /// A session Roblox handed out whose account couldn't be looked up yet (a network hiccup): "Try again" only looks it
    /// up again, instead of signing in again with the password or a code that is spent.
    /// </summary>
    (string Cookie, string Realm)? _loginCookie;
    /// <summary>Roblox said "too many tries" to a 2-step code: Verify and Send wait until then.</summary>
    DateTime _twoStepWaitUntil;
    static readonly TimeSpan TwoStepWait = TimeSpan.FromSeconds(60);
    /// <summary>Bumped by a cancel or a lock: a sign-in step that started before it must not keep its result.</summary>
    int _loginGeneration;
    /// <summary>Cancelled (and replaced) with each bump of <see cref="_loginGeneration"/>, for the steps that go on by themselves.</summary>
    CancellationTokenSource _loginCts = new();

    bool HasLoginPrompt => _twoStep is not null || _captcha is not null || _humanCaptcha is not null;

    /// <summary>
    /// One step of a password sign-in. Roblox ending the attempt (a wrong password, a locked account, too many tries)
    /// isn't an error of this step: the prompt, which holds the password, is dropped, so nothing can send it again,
    /// and the page goes back to the username and password with the reason ({ status: "login", error }).
    /// </summary>
    /// <param name="newAttempt">A new password sign-in: whatever attempt was going on is dropped, and a late reply to it too.</param>
    async Task<object?> LoginStepAsync(Func<Task<object?>> step, bool newAttempt = false)
    {
        if (newAttempt)
            LoginCancel();
        int generation = _loginGeneration;
        bool hadPrompt = HasLoginPrompt;
        try
        {
            return await step();
        }
        // Called off (cancel, lock, the page gone) while a check was being answered: nothing is kept.
        catch (OperationCanceledException) when (generation != _loginGeneration)
        {
            return null;
        }
        catch (RobloxApiException ex) when (ex.AttemptOver)
        {
            if (generation != _loginGeneration)
                return null;
            DropLoginPrompts();
            return new { status = "login", error = ex.Message };
        }
        // Too many codes tried: the code screen stays (the prompt is still good), with Verify held back for a minute.
        catch (RobloxApiException ex) when (ex.RateLimited && _twoStep is not null)
        {
            if (generation != _loginGeneration)
                return null;
            _twoStepWaitUntil = DateTime.UtcNow + TwoStepWait;
            return new { status = "wait", error = ex.Message, seconds = (int)TwoStepWait.TotalSeconds };
        }
        // The step spent its prompt (the puzzle or the code was answered) and then failed: nothing is left to answer on
        // this screen, so the page goes back to the username and password with the reason. A session already handed
        // out is kept, and only looked up again.
        catch (Exception ex) when (ex is UserError || RobloxApi.IsExpectedError(ex))
        {
            if (generation != _loginGeneration)
                return null;
            if (_loginCookie is not null)
                return new { status = "finish", error = ErrorText(ex) };
            if (hadPrompt && !HasLoginPrompt)
                return new { status = "login", error = ErrorText(ex) };
            throw;
        }
    }

    async Task<object?> LoginPasswordAsync(string username, string password, string realm)
    {
        RequireVault();
        if (username.Trim().Length == 0 || password.Length == 0)
            throw new UserError(L.T("Enter the username and password."));
        int generation = _loginGeneration;
        return await HandleLoginAsync(await RobloxApi.LoginWithPasswordAsync(username.Trim(), password, realm), realm, generation);
    }

    /// <returns>null when the sign-in was cancelled (or the vault locked) while Roblox was answering: nothing is kept.</returns>
    async Task<object?> HandleLoginAsync(RobloxApi.LoginResult result, string realm, int generation)
    {
        // Proof-of-work needs no input: solve it and carry on. Once: answering it repeats the password sign-in, which
        // happens at most once per click — and not at all once the sign-in was cancelled or Bloxkeep locked, also
        // while the puzzle is being solved (the token stops it before anything more is sent).
        if (result.ProofOfWork is { } pow)
        {
            if (generation != _loginGeneration)
                return null;
            result = await RobloxApi.FinishProofOfWorkLoginAsync(pow, _loginCts.Token);
        }
        // Which 2-step methods the account has decides the tabs, or that only the phone or browser can finish it.
        if (result.TwoStep is { } asked)
            result = await RobloxApi.WithTwoStepMethodsAsync(asked);
        if (generation != _loginGeneration)
            return null;
        if (result.ProofOfWork is not null)
            throw new UserError(L.T("Roblox didn't accept the sign-in check. Please try again."));
        // A check only Roblox's site or app can show ends this attempt, with the reason, like a refusal does.
        if (result.Blocked is { } blocked)
            throw new RobloxApiException(blocked, attemptOver: true);
        if (result.Captcha is { } captcha)
        {
            _captcha = captcha;
            _captchaUsed = false;
            return new { status = "captcha" };
        }
        if (result.HumanCaptcha is { } human)
        {
            _humanCaptcha = human;
            _captchaUsed = false;
            return new { status = "captcha" };
        }
        if (result.TwoStep is { } prompt)
        {
            _twoStep = prompt;
            // The first method is the one the code was sent by, or Roblox's preferred one.
            // autofill: the account's authenticator key is saved here, so the page fills its code in (Verify stays the person's).
            return new { status = "twostep", media = prompt.Methods[0], methods = prompt.Methods, autofill = AuthenticatorAccount(prompt) is not null };
        }
        return await SaveLoginAsync(result.Cookie!, realm, generation);
    }

    /// <summary>Shows the captcha for the user to solve; on success the sign-in carries on (it may still ask for 2-step).</summary>
    async Task<object?> LoginCaptchaAsync()
    {
        RequireVault();
        int generation = _loginGeneration;
        // A puzzle belongs to one sign-in attempt; once its window was closed, ask Roblox for a fresh one.
        if (_captchaUsed && (_captcha?.Username ?? _humanCaptcha?.Username) is { } username)
        {
            // Each fresh puzzle sends the password again: a few per attempt, then the attempt ends.
            if (_freshPuzzles >= MaxFreshPuzzles)
                throw new RobloxApiException(L.T("Roblox keeps asking for a new puzzle. Sign in with your phone instead, or try again later."), attemptOver: true);
            _freshPuzzles++;
            string password = _captcha?.Password ?? _humanCaptcha!.Password;
            string realm = _captcha?.Realm ?? _humanCaptcha!.Realm;
            _captcha = null;
            _humanCaptcha = null;
            _captchaUsed = false;
            var again = await HandleLoginAsync(await RobloxApi.LoginWithPasswordAsync(username, password, realm), realm, generation);
            // Roblox may let it through without a puzzle this time, or ask for 2-step instead.
            if (generation != _loginGeneration || (_captcha is null && _humanCaptcha is null))
                return again;
        }
        if (_humanCaptcha is { } human)
        {
            string? redemption = ShowCaptcha(CaptchaForm.Human(human.Realm, human.CaptchaChallengeId));
            // Cancelled, locked or the page gone while the puzzle was open: nothing more goes to Roblox.
            if (generation != _loginGeneration || _vault is null || !ReferenceEquals(_humanCaptcha, human))
                return null;
            if (redemption is null)
            {
                _captchaUsed = true;
                return new { status = "captcha", closed = true };
            }
            _humanCaptcha = null;
            return await HandleLoginAsync(await RobloxApi.FinishHumanCaptchaLoginAsync(human, redemption), human.Realm, generation);
        }
        if (_captcha is not { } prompt)
            throw new UserError(L.T("Start the sign-in again."));
        string key = await RobloxApi.GetLoginCaptchaKeyAsync(prompt.Realm);
        if (generation != _loginGeneration)
            return null;
        string? token = ShowCaptcha(CaptchaForm.Arkose(prompt.Realm, key, prompt.Blob));
        if (generation != _loginGeneration || _vault is null || !ReferenceEquals(_captcha, prompt))
            return null;
        if (token is null)
        {
            _captchaUsed = true;
            return new { status = "captcha", closed = true };
        }
        _captcha = null;
        return await HandleLoginAsync(await RobloxApi.FinishCaptchaLoginAsync(prompt, token), prompt.Realm, generation);
    }

    /// <returns>What the solved puzzle handed back, or null when the window was closed first.</returns>
    string? ShowCaptcha(CaptchaForm form)
    {
        using (form)
        {
            form.ShowDialog(_owner);
            return form.Token;
        }
    }

    async Task<object?> LoginTwoStepAsync(string method, string code)
    {
        RequireVault();
        if (_twoStep is not { } prompt)
            throw new UserError(L.T("Start the sign-in again."));
        if (code.Trim().Length == 0 && prompt.VerificationToken is null)
            throw new UserError(L.T("Enter the verification code."));
        WaitForTwoStep();
        int generation = _loginGeneration;
        RobloxApi.LoginResult result;
        try
        {
            // A wrong code throws and keeps the prompt, so it can be typed again; anything else moves the sign-in on.
            // Once the code was accepted, a retry after a network hiccup skips it (it is spent) and only finishes.
            result = await RobloxApi.VerifyTwoStepAsync(prompt, TwoStepMethod(prompt, method), code.Trim());
        }
        catch (Exception ex) when (prompt.VerificationToken is not null && !IsNetworkError(ex))
        {
            // Accepted, then refused on the way: nothing is left to answer on this screen.
            if (generation == _loginGeneration && ReferenceEquals(_twoStep, prompt))
                _twoStep = null;
            throw;
        }
        if (generation != _loginGeneration)
            return null;
        _twoStep = null;
        return await HandleLoginAsync(result, prompt.Realm, generation);
    }

    /// <summary>Has Roblox send the code (again) by email or SMS, when the user asks for it.</summary>
    async Task<object?> LoginTwoStepSendAsync(string method)
    {
        RequireVault();
        if (_twoStep is not { } prompt)
            throw new UserError(L.T("Start the sign-in again."));
        string chosen = TwoStepMethod(prompt, method);
        if (chosen == "authenticator")
            throw new UserError(L.T("Enter the code from the authenticator app."));
        WaitForTwoStep();
        await RobloxApi.SendTwoStepCodeAsync(prompt, chosen);
        return new { status = "sent" };
    }

    /// <summary>The method the page picked, if it is one this account was offered; otherwise the first offered.</summary>
    static string TwoStepMethod(RobloxApi.TwoStepPrompt prompt, string method) =>
        prompt.Methods.Contains(method) ? method : prompt.Methods[0];

    /// <summary>Roblox said "too many tries" a moment ago: nothing goes to it until the minute is over.</summary>
    void WaitForTwoStep()
    {
        var left = _twoStepWaitUntil - DateTime.UtcNow;
        if (left > TimeSpan.Zero)
            throw new UserError(L.F("Too many tries. Wait {0} s, then try again.", (int)Math.Ceiling(left.TotalSeconds)));
    }

    /// <summary>Offline, a timeout or Roblox down: the request may never have been answered, so it's worth repeating.</summary>
    static bool IsNetworkError(Exception ex) =>
        ex is HttpRequestException or TaskCanceledException || ex is RobloxApiException { ServerError: true };

    /// <summary>"Try again" after the account couldn't be looked up: the session Roblox handed out is used, no new sign-in.</summary>
    async Task<object?> LoginFinishAsync()
    {
        RequireVault();
        if (_loginCookie is not { } held)
            throw new UserError(L.T("Start the sign-in again."));
        return await SaveLoginAsync(held.Cookie, held.Realm, _loginGeneration);
    }

    async Task<object?> SaveLoginAsync(string cookie, string realm, int generation)
    {
        // Held until the account is saved: a failed lookup is tried again without signing in again.
        _loginCookie = (cookie, realm);
        var user = await RobloxApi.GetAuthenticatedUserAsync(cookie, realm);
        // Locked or cancelled meanwhile: the account isn't saved (RequireVault would say so anyway when locked).
        if (generation != _loginGeneration)
            return null;
        if (user is null)
        {
            _loginCookie = null; // Roblox's answer, not a hiccup: the session is no good
            throw new UserError(L.T("Roblox didn't accept that sign-in. Please try again."));
        }
        var added = AddOrMerge(new Account
        {
            Cookie = cookie,
            Realm = realm,
            UserId = user.Id,
            Username = user.Name,
            DisplayName = user.DisplayName,
            CookieValid = true,
            LastChecked = DateTime.Now,
        });
        _loginCookie = null;
        return new { status = "added", added };
    }

    async Task<object?> AddViaCookieAsync(JsonElement a)
    {
        RequireVault();
        string cookie = RobloxApi.NormalizeCookie(Str(a, "cookie"));
        if (cookie.Length == 0)
            throw new UserError(L.T("Paste the .ROBLOSECURITY cookie."));
        var user = await RobloxApi.GetAuthenticatedUserAsync(cookie)
            ?? throw new UserError(L.T("The cookie is invalid or has expired."));
        return AddOrMerge(new Account
        {
            Cookie = cookie,
            UserId = user.Id,
            Username = user.Name,
            DisplayName = user.DisplayName,
            Alias = Str(a, "alias").Trim(),
            Group = Str(a, "group").Trim(),
            Password = Str(a, "password"),
            Note = Str(a, "note").Trim(),
            CookieValid = true,
            LastChecked = DateTime.Now,
        });
    }

    sealed record AddResult(Guid Id, string Label, bool Merged);

    /// <summary>Signed in again: auto-rejoin comes back on if a sign-in Roblox ended was what switched it off.</summary>
    static void SignedInAgain(Account account)
    {
        if (!account.RejoinOffBySignOut)
            return;
        account.RejoinOffBySignOut = false;
        account.AutoRejoin = true;
    }

    /// <summary>
    /// Roblox accepted the account's sign-in (a call made with it went through): not signed out, and auto-rejoin back on
    /// if a sign-out switched it off. Before, a sign-out marked in an outage stayed: a later accepted call set the account
    /// signed in with auto-rejoin still off.
    /// </summary>
    static void SignInAccepted(Account account)
    {
        account.CookieValid = true;
        SignedInAgain(account);
    }

    /// <summary>
    /// Adding an account that is already in the list just refreshes its cookie. Roblox and Roblox VNG count user ids
    /// apart, so the same id in the other realm is another account.
    /// </summary>
    AddResult AddOrMerge(Account incoming)
    {
        var vault = RequireVault();
        string realm = Realm.Normalize(incoming.Realm);
        var existing = vault.Data.Accounts.FirstOrDefault(a => a.UserId != 0 && a.UserId == incoming.UserId && Realm.Normalize(a.Realm) == realm);
        if (existing is not null)
        {
            existing.Cookie = incoming.Cookie;
            existing.Realm = incoming.Realm;
            existing.Username = incoming.Username;
            existing.DisplayName = incoming.DisplayName;
            existing.CookieValid = incoming.CookieValid;
            existing.LastChecked = incoming.LastChecked;
            if (incoming.CookieValid == true)
                SignedInAgain(existing);
            if (incoming.Password.Length > 0) existing.Password = incoming.Password;
            if (incoming.Alias.Length > 0) existing.Alias = incoming.Alias;
            if (incoming.Group.Length > 0) existing.Group = incoming.Group;
            if (incoming.Note.Length > 0) existing.Note = incoming.Note;
        }
        else
        {
            vault.Data.Accounts.Add(incoming);
        }
        var account = existing ?? incoming;
        Persist();
        PushState();
        _ = LoadAvatarsAsync([account.UserId]);
        RefreshPresenceSoon(TimeSpan.FromSeconds(2));
        return new AddResult(account.Id, NameText(account), existing is not null);
    }

    /// <summary>Imported cookies may be old, so verify them right away and report the result.</summary>
    async Task CheckImportedAsync(Guid[] ids)
    {
        try
        {
            while (_checking)
                await Task.Delay(1000);
            var vault = _vault;
            if (vault is null)
                return;
            await CheckAccountsAsync(ids);
            var checkedAccounts = vault.Data.Accounts.Where(a => ids.Contains(a.Id)).ToList();
            Toast(L.F("Imported accounts checked: {0} valid, {1} expired.",
                checkedAccounts.Count(a => a.CookieValid == true), checkedAccounts.Count(a => a.CookieValid == false)), "ok");
        }
        catch (UserError)
        {
            // Vault locked or another check started meanwhile; the user can check again from the menu.
        }
    }

    async Task<object?> UpdateAccountAsync(JsonElement a)
    {
        var vault = RequireVault();
        var account = RequireAccount(Id(a));
        // Checked before anything changes: a key with a typo is refused with what's wrong, and nothing is saved.
        // "" removes the key; no field at all keeps it.
        string? typedKey = OptStr(a, "authenticator")?.Trim();
        TotpKey? authenticator = typedKey is { Length: > 0 } ? ParseAuthenticator(typedKey) : null;
        RobloxUser? user = null;
        string? cookie = OptStr(a, "cookie") is { } raw ? RobloxApi.NormalizeCookie(raw) : null;
        if (cookie is { Length: > 0 } && cookie != account.Cookie)
        {
            user = await RobloxApi.GetAuthenticatedUserAsync(cookie, account.Realm)
                ?? throw new UserError(L.T("The new cookie is invalid or has expired."));
            // Locked (and maybe reopened) meanwhile: this account object belongs to the old vault.
            if (_vault != vault)
                throw new UserError(L.T("Bloxkeep was locked."));
        }

        // No alias sent while names are hidden: the form can't show it then, so it stays as it was.
        if (OptStr(a, "alias") is { } alias)
            account.Alias = alias.Trim();
        account.Group = Str(a, "group").Trim();
        account.Note = Str(a, "note").Trim();
        if (OptStr(a, "password") is { } password)
            account.Password = password;
        if (typedKey is not null)
            account.Authenticator = authenticator;
        if (user is not null)
        {
            account.Cookie = cookie!;
            account.UserId = user.Id;
            account.Username = user.Name;
            account.DisplayName = user.DisplayName;
            account.CookieValid = true;
            account.LastChecked = DateTime.Now;
            SignedInAgain(account);
            _ = LoadAvatarsAsync([user.Id]);
        }
        Persist();
        PushState();
        return true;
    }

    object SetUseRobloxConfig(Guid[] ids, bool enabled)
    {
        int changed = 0;
        foreach (var account in RequireVault().Data.Accounts.Where(a => ids.Contains(a.Id)))
        {
            account.UseRobloxConfig = enabled;
            changed++;
        }
        Persist();
        PushState();
        return changed;
    }

    object SetAutoRejoin(Guid[] ids, bool enabled)
    {
        int changed = 0;
        foreach (var account in RequireVault().Data.Accounts.Where(a => ids.Contains(a.Id)))
        {
            account.AutoRejoin = enabled;
            account.RejoinOffBySignOut = false; // the person's own choice from now on
            if (enabled)
                _rejoinsInARow.Remove(account.Id);
            else // a client whose preset turns auto-rejoin on, and a planned restart, still go ahead
                _rejoinQueue.RemoveAll(r => r.AccountId == account.Id && !r.Restart && !RejoinOn(account, r.PresetId));
            changed++;
        }
        Persist();
        PushState();
        return changed;
    }

    object Reveal(Guid id, string field)
    {
        var account = RequireAccount(id);
        return field switch
        {
            "password" => account.Password,
            _ => throw new UserError(L.T("Invalid field.")),
        };
    }

    object DeleteAccounts(Guid[] ids)
    {
        var set = ids.ToHashSet();
        var data = RequireVault().Data;
        int removed = data.Accounts.RemoveAll(a => set.Contains(a.Id));
        CloseWebsites(set);
        // Gone from the presets too; one left with no account has nothing to run, so its schedule goes off rather than
        // reporting "no accounts left" every day.
        foreach (var preset in data.Settings.Presets)
        {
            if (preset.AccountIds.RemoveAll(set.Contains) > 0 && preset.AccountIds.Count == 0 && preset.Schedule is { Enabled: true } schedule)
                schedule.Enabled = false;
        }
        data.Settings.LaunchOrder.RemoveAll(set.Contains);
        data.Discord.SummaryAccounts.RemoveAll(set.Contains);
        foreach (var id in set)
        {
            _running.Remove(id);
            _rejoinsInARow.Remove(id);
            _restarts.Remove(id);
            _offlineStrikes.Remove(id);
            _userClosed.Remove(id);
            EndPlay(id, null, DateTime.UtcNow); // its stay ends now, not at the next lock
            ForgetScheduledOpen(id);
        }
        _pending.RemoveAll(p => set.Contains(p.AccountId));
        _late.RemoveWhere(l => set.Contains(l.AccountId));
        foreach (var launch in _handedOver.Keys.Where(l => set.Contains(l.AccountId)).ToList())
            _handedOver.Remove(launch);
        _closeWhenClaimed.RemoveWhere(l => set.Contains(l.AccountId));
        _rejoinQueue.RemoveAll(r => set.Contains(r.AccountId));
        Persist();
        PushState();
        return removed;
    }

    const int CheckParallelism = 6;

    /// <summary>
    /// Asks Roblox who each saved sign-in belongs to: the same read-only call the website makes on every page.
    /// A rejected sign-in means Roblox ended that session (password change, "log out everywhere", security).
    /// Six accounts are checked at a time; when Roblox asks to slow down, all checks pause and try again.
    /// </summary>
    async Task<object?> CheckAccountsAsync(Guid[] ids)
    {
        var vault = RequireVault();
        if (_checking)
            throw new UserError(L.T("Already checking — please wait."));
        var accounts = ids.Length == 0
            ? vault.Data.Accounts.ToList()
            : vault.Data.Accounts.Where(a => ids.Contains(a.Id)).ToList();

        _checking = true;
        PushState();
        int ok = 0, bad = 0, failed = 0, done = 0;
        var refused = new List<Guid>();
        int moderated = 0;
        var queue = new Queue<Account>(accounts);
        var rejected = new List<Account>();
        var resumeAt = DateTime.UtcNow;
        var newlyBanned = new List<Account>();

        // Every continuation runs on the UI thread, so the queue and counters need no locking.
        async Task WorkAsync()
        {
            while (_vault == vault && queue.TryDequeue(out var account))
            {
                for (int attempt = 1; ; attempt++)
                {
                    if (resumeAt > DateTime.UtcNow)
                        await Task.Delay(resumeAt - DateTime.UtcNow);
                    string cookie = account.Cookie;
                    try
                    {
                        var user = await RobloxApi.GetAuthenticatedUserAsync(cookie, account.Realm);
                        if (account.Cookie != cookie)
                            break; // signed in again meanwhile: the old cookie's answer says nothing about the new one
                        // A rejection is marked below, once Roblox's sign-in service is known to be up.
                        if (user is null)
                            rejected.Add(account);
                        else
                        {
                            SignInAccepted(account);
                            account.UserId = user.Id;
                            account.Username = user.Name;
                            account.DisplayName = user.DisplayName;
                            ok++;
                        }
                        account.LastChecked = DateTime.Now;
                    }
                    catch (RobloxApiException ex) when (ex.RateLimited && attempt < 4)
                    {
                        var wait = DateTime.UtcNow + TimeSpan.FromSeconds(4 * attempt);
                        if (wait > resumeAt)
                            resumeAt = wait;
                        continue;
                    }
                    catch (RobloxApiException ex) when (!ex.ServerError && !ex.RateLimited)
                    {
                        // Roblox answered and said no (403: a warning waiting to be read, a lock…): not the network.
                        refused.Add(account.Id);
                        if (ex.Moderated)
                            moderated++;
                    }
                    catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
                    {
                        failed++;
                    }
                    break;
                }
                // A ban, from the account's public profile (no cookie). No clear answer leaves it as it was. Roblox asking
                // to slow down pauses every check, like the sign-in call above.
                for (int attempt = 1; _vault == vault; attempt++)
                {
                    if (resumeAt > DateTime.UtcNow)
                        await Task.Delay(resumeAt - DateTime.UtcNow);
                    try
                    {
                        if (_vault == vault && await CheckBanAsync(account) is true)
                            newlyBanned.Add(account);
                    }
                    catch (RobloxApiException ex) when (ex.RateLimited)
                    {
                        var wait = DateTime.UtcNow + TimeSpan.FromSeconds(4 * attempt);
                        if (wait > resumeAt)
                            resumeAt = wait;
                        if (attempt < 3)
                            continue;
                    }
                    catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
                    {
                    }
                    break;
                }
                Progress(L.F("Checking sign-ins… {0}/{1}", ++done, accounts.Count));
            }
        }

        try
        {
            Progress(L.F("Checking sign-ins… {0}/{1}", 0, accounts.Count));
            await Task.WhenAll(Enumerable.Range(0, Math.Min(CheckParallelism, accounts.Count)).Select(_ => WorkAsync()));
            // Another account accepted in this check: Roblox's sign-in service is up, so a rejection means a sign-out.
            // With none accepted, an outage answers 401 to everyone: each is marked only if Roblox accepts another account
            // now (ConfirmSignedOutAsync), else counted as not checked.
            foreach (var account in rejected)
            {
                if (_vault != vault)
                    break;
                if (ok > 0)
                    account.CookieValid = false;
                if (ok > 0 || await ConfirmSignedOutAsync(account))
                    bad++;
                else
                    failed++;
            }
        }
        finally
        {
            _checking = false;
            Progress(null);
            Persist();
            PushState();
        }
        _ = LoadAvatarsAsync(accounts.Select(a => a.UserId));
        if (newlyBanned.Count > 0 && _vault == vault)
            NotifyBanned(newlyBanned);
        return new { ok, bad, failed, refused = refused.Count, moderated, refusedIds = refused, banned = accounts.Count(a => a.Banned == true) };
    }

    object Copy(Guid id, string field)
    {
        var account = RequireAccount(id);
        var now = DateTimeOffset.UtcNow;
        (string value, bool secret, string missing) = field switch
        {
            "username" => (account.Username, false, "This account has no saved username."),
            "password" => (account.Password, true, "This account has no saved password."),
            // The authenticator code of the moment (never the key); off the clipboard again like a password.
            "totp" => (account.Authenticator is { } key ? Totp.Code(key, now) : "", true, "This account has no authenticator key."),
            // The client's process id, for other tools that pick a window by it.
            "pid" => (_running.TryGetValue(id, out var run) ? run.Pid.ToString() : "", false, "This account has no Roblox window open."),
            _ => throw new UserError(L.T("Invalid field.")),
        };
        if (value.Length == 0)
            throw new UserError(L.T(missing));
        if (!Ui.CopyToClipboard(value, secret))
            throw new UserError(L.T("Couldn't access the clipboard; try again."));
        // A code says how long it still works.
        return new { secret, seconds = field == "totp" ? Totp.SecondsLeft(account.Authenticator!, now) : (int?)null };
    }

    async Task LoadAvatarsAsync(IEnumerable<long> userIds)
    {
        var missing = userIds.Where(id => id > 0 && !_avatarUrls.ContainsKey(id)).Distinct().ToList();
        try
        {
            if (missing.Count == 0)
                return;
            foreach (var (id, url) in await RobloxApi.GetHeadshotUrlsAsync(missing))
                _avatarUrls[id] = url;
            PushState();
        }
        catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
        {
        }
        finally
        {
            if (!_bootAvatarsDone)
            {
                _bootAvatarsDone = true;
                AdvanceBoot();
            }
        }
    }

    /// <summary>A step of the loading screen and how far along it is (0–1); the page shows it until it is null.</summary>
    sealed record BootProgress(string Stage, double Percent);

    /// <summary>
    /// Moves the loading screen to the first step still running: Roblox status (30 % → 80 %), then avatars
    /// (80 % → 100 %), then done.
    /// </summary>
    void AdvanceBoot(bool push = true)
    {
        if (_boot is null)
            return;
        _boot = !_bootStatusDone ? new BootProgress("status", 0.3)
            : !_bootAvatarsDone ? new BootProgress("avatars", 0.8)
            : null;
        if (push)
            PushState();
        if (_boot is null)
            _ = SettledAsync();
    }

    const int SettledDelayMs = 1500;

    /// <summary>
    /// A moment after the loading screen is done (or a new page came to an open vault), once the accounts and avatars
    /// are drawn: the page hears "settled" and gets ready what its tabs not opened yet show (Statistics' figures, the
    /// presets' icons, and those games' pictures, which may have to be asked of Roblox), instead of when they are first
    /// opened (0.3-1.4 s from here). A sleeping page hears it on waking.
    /// </summary>
    async Task SettledAsync()
    {
        var vault = _vault;
        await Task.Delay(SettledDelayMs);
        if (vault is not null && _vault == vault && _boot is null && PageAttached)
            Emit("settled", null);
    }
}
