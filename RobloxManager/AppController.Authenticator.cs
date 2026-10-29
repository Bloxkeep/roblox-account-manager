namespace RobloxManager;

/// <summary>Authenticator (2-step) codes per account (<see cref="Totp"/>). The key sits in the vault, encrypted like the password,
/// and never goes to the page or the log: the page gets only the current code (edit form, "Copy 2FA code", the sign-in's code box).
/// Anyone with the master password can make codes; the edit form says so.</summary>
public sealed partial class AppController
{
    /// <summary>A key typed in the edit form, as kept; a typo is refused with what's wrong rather than kept to make wrong codes.</summary>
    static TotpKey ParseAuthenticator(string input) =>
        Totp.Parse(input, out var problem, out string detail) ?? throw new UserError(AuthenticatorProblem(problem, detail));

    static string AuthenticatorProblem(TotpProblem problem, string detail) => problem switch
    {
        TotpProblem.BadCharacter => L.F("The key can't contain \"{0}\": it only has the letters A–Z and the digits 2–7. Check it for a typo.", detail),
        TotpProblem.TooShort => L.T("The key is too short. Check that you copied all of it."),
        TotpProblem.CutOff => L.T("The key looks cut off or has an extra character. Check that you copied all of it."),
        TotpProblem.NotTimeBased => L.T("This link makes counter-based codes. Bloxkeep only makes time-based codes, the kind Roblox uses."),
        TotpProblem.NoSecret => L.T("The link has no key in it."),
        TotpProblem.Unsupported => L.F("Bloxkeep can't make codes with this setting from the link: {0}.", detail),
        _ => L.T("That isn't an authenticator key. Paste the key (letters A–Z and digits 2–7) or the otpauth:// link."),
    };

    /// <summary>The code of the moment and the seconds it has left: all the page ever gets from a key.</summary>
    static object CodeDto(TotpKey key)
    {
        var now = DateTimeOffset.UtcNow;
        return new { ok = true, code = Totp.Code(key, now), seconds = Totp.SecondsLeft(key, now), period = key.Period };
    }

    /// <summary>The edit form's live code for the key saved with an account.</summary>
    object AuthenticatorCode(Guid id) =>
        CodeDto(RequireAccount(id).Authenticator ?? throw new UserError(L.T("This account has no authenticator key.")));

    /// <summary>A key being typed: whether it reads and its current code, to compare with the app before saving. Nothing is kept.</summary>
    static object CheckAuthenticator(string input) =>
        Totp.Parse(input, out var problem, out string detail) is { } key
            ? CodeDto(key)
            : new { ok = false, error = AuthenticatorProblem(problem, detail) };

    /// <summary>The account a sign-in's 2-step code is for, when its key is saved here: same user id (named in the challenge), else
    /// same username, on the same Roblox.</summary>
    Account? AuthenticatorAccount(RobloxApi.TwoStepPrompt prompt)
    {
        if (_vault is not { } vault || !prompt.Methods.Contains("authenticator"))
            return null;
        string realm = Realm.Normalize(prompt.Realm);
        var withKey = vault.Data.Accounts.Where(a => a.Authenticator is not null && Realm.Normalize(a.Realm) == realm).ToList();
        return withKey.FirstOrDefault(a => a.UserId > 0 && a.UserId == prompt.UserId)
            ?? withKey.FirstOrDefault(a => prompt.Username.Length > 0 && string.Equals(a.Username, prompt.Username, StringComparison.OrdinalIgnoreCase));
    }

    /// <summary>The code for the sign-in's 2-step box, filled in for the person, who still presses Verify: Bloxkeep never sends a
    /// code by itself, nor tries another after Roblox refused one.</summary>
    object LoginTwoStepCode()
    {
        RequireVault();
        if (_twoStep is not { } prompt || AuthenticatorAccount(prompt)?.Authenticator is not { } key)
            throw new UserError(L.T("Start the sign-in again."));
        return CodeDto(key);
    }
}
