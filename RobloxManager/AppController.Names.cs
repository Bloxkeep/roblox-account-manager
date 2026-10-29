using System.Text.RegularExpressions;

namespace RobloxManager;

/// <summary>Settings › Appearance › Hide account names, for screenshots and streams: wherever Bloxkeep writes an account's name
/// (page, toasts, notifications, progress, mini window, monitor tab, Roblox window titles) it writes "Account N" (its place in
/// the list). The page gets the real names and swaps them itself, so search finds them; copying copies the real one. Discord has
/// its own switch (Settings › Discord), since a channel may be shared while the screen isn't.</summary>
public sealed partial class AppController
{
    /// <summary>Set while a Discord copy of a message is made: its switch and language instead of the app's.</summary>
    (bool Hide, string Language)? _nameStyle;

    object? SetHideNames(bool enabled)
    {
        Prefs.HideNames = enabled;
        Prefs.Save();
        PushState(); // the mini window, the monitor tab and the window titles follow at their next refresh
        return null;
    }

    /// <summary>An account's name for a message: its label, or "Account 3" while hidden. Pass it to L.F as is: it's written out when
    /// the text is made, so the Discord copy (<see cref="DiscordText"/>) follows Discord's switch.</summary>
    Shown Name(Account account) => new(name => name(account), NameText);

    /// <summary>Several accounts' names, comma-separated, written out like <see cref="Name"/>.</summary>
    Shown Names(IEnumerable<Account> accounts)
    {
        var list = accounts.ToList();
        return new(name => string.Join(", ", list.Select(name)), NameText);
    }

    /// <summary>Text written out only when its message is (see <see cref="Name"/>): names as they read now, or another way via
    /// <see cref="Write"/> (the activity log keeps a placeholder per account, so its sentences never hold a name).</summary>
    sealed class Shown(Func<Func<Account, string>, string> write, Func<Account, string> name)
    {
        public string Write(Func<Account, string> other) => write(other);

        public override string ToString() => write(name);
    }

    /// <summary>The name as it reads right now: in the app (window titles, the mini window) or in the message being made.</summary>
    string NameText(Account account)
    {
        var (hide, language) = _nameStyle ?? (Prefs.HideNames, L.Language);
        return hide ? MaskedName(account, language) : account.Label;
    }

    /// <summary>"Account 3": the account's place in the list, as the page numbers them. Never through L.F (see <see cref="L.Made"/>).</summary>
    string MaskedName(Account account, string language)
    {
        int index = _vault?.Data.Accounts.IndexOf(account) ?? -1;
        return index >= 0 ? string.Format(L.T("Account {0}", language), index + 1) : L.T("Deleted account", language);
    }

    /// <summary>An account's name in a Discord status message or summary, shown or hidden by Discord's own switch.</summary>
    string DiscordName(Account account) =>
        _vault?.Data.Discord.HideNames == true ? MaskedName(account, DiscordLanguage) : account.Label;

    /// <summary>An event's text for Discord: remade from the same sentence and values in Discord's language and name switch.
    /// Call it before any other L.F (it needs the one that made the text).</summary>
    string DiscordText(string text)
    {
        bool hide = _vault?.Data.Discord.HideNames == true;
        if (L.Made(text) is { } made)
        {
            _nameStyle = (hide, DiscordLanguage);
            try
            {
                text = string.Format(L.T(made.English, DiscordLanguage), made.Args);
            }
            finally
            {
                _nameStyle = null;
            }
        }
        return hide ? HideNamesIn(text) : text;
    }

    /// <summary>A last guard for Discord's switch: names in already-written text (a launch error kept as text) become "Account N".
    /// Whole words only, names of 3+ characters, one pass.</summary>
    string HideNamesIn(string text)
    {
        if (_vault is not { } vault)
            return text;
        var byName = new Dictionary<string, Account>(StringComparer.Ordinal);
        foreach (var account in vault.Data.Accounts)
        {
            foreach (string name in new[] { account.Label, account.Username })
            {
                if (name.Length >= 3)
                    byName.TryAdd(name, account);
            }
        }
        if (byName.Count == 0)
            return text;
        string names = string.Join("|", byName.Keys.OrderByDescending(n => n.Length).Select(Regex.Escape));
        string language = DiscordLanguage;
        return Regex.Replace(text, $@"(?<![\p{{L}}\p{{N}}_])(?:{names})(?![\p{{L}}\p{{N}}_])", m => MaskedName(byName[m.Value], language));
    }
}
