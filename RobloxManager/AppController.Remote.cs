using System.Security.Cryptography;
using System.Text;
using System.Text.Json.Nodes;

namespace RobloxManager;

/// <summary>
/// Settings › Discord › Control from Discord (off by default): once the person typed the code shown here into /pc link,
/// Bloxkeep's bot hands this PC their /pc commands (RemoteLink, services/DiscordBot/worker/remote.js). A command can only
/// report the accounts (the status board's card), run a preset, close windows or move an account to another server.
/// Nothing is asked on the PC: a preset that would ask first (Multi-Roblox off…) isn't run. Locked, Bloxkeep answers
/// only that it is locked and names nothing. Turned off: the bot forgets the link and the connection closes.
/// </summary>
public sealed partial class AppController
{
    RemoteLink? _remote;

    /// <summary>The bot's address; the checks point it at a stand-in.</summary>
    static string? RemoteTestUrl => Environment.GetEnvironmentVariable("BLOXKEEP_REMOTE_URL") is { Length: > 0 } url ? url : null;

    static readonly byte[] RemoteEntropy = Encoding.UTF8.GetBytes("bloxkeep-remote-key");

    const string AllAccounts = "All accounts";

    /// <summary>A preset that's still opening after this long is answered "opening…", and its outcome follows up later.</summary>
    static readonly TimeSpan RemoteQuickAnswer = TimeSpan.FromSeconds(1.5);

    /// <summary>Connected while it's on, closed when it's off; the names Discord suggests kept up to date.</summary>
    void TickRemote()
    {
        if (Prefs.RemoteControl && _remote is null && RemoteSecret() is { } secret)
            _remote = new RemoteLink(secret, RemoteCommandAsync, OnRemoteChanged, RemoteTestUrl);
        else if (!Prefs.RemoteControl && _remote is not null)
            StopRemote();
        if (_remote is null)
            return;
        bool open = _vault is not null && !_screenLocked;
        _remote.Hello(Environment.MachineName,
            open ? _vault!.Data.Accounts.Select(DiscordName).Distinct(StringComparer.CurrentCultureIgnoreCase) : [],
            open ? _vault!.Data.Settings.Presets.Select(p => p.Name).Where(n => n.Length > 0).Distinct(StringComparer.CurrentCultureIgnoreCase) : [],
            L.Language);
    }

    void StopRemote()
    {
        _remote?.Dispose();
        _remote = null;
    }

    /// <summary>This PC's secret for the bot, made the first time (random, kept encrypted for this Windows user).</summary>
    static string? RemoteSecret()
    {
        try
        {
            if (Prefs.RemoteKey is { Length: > 0 } saved)
                return Encoding.UTF8.GetString(ProtectedData.Unprotect(Convert.FromBase64String(saved), RemoteEntropy, DataProtectionScope.CurrentUser));
        }
        catch (Exception ex) when (ex is FormatException or CryptographicException)
        {
            // Unreadable (copied from another Windows user): a new one, and the PC is linked again with a new code.
        }
        string secret = Convert.ToBase64String(RandomNumberGenerator.GetBytes(32)).TrimEnd('=').Replace('+', '-').Replace('/', '_');
        Prefs.RemoteKey = Convert.ToBase64String(ProtectedData.Protect(Encoding.UTF8.GetBytes(secret), RemoteEntropy, DataProtectionScope.CurrentUser));
        Prefs.Save();
        return secret;
    }

    object? SetRemote(bool on)
    {
        if (on == Prefs.RemoteControl)
            return null;
        Prefs.RemoteControl = on;
        Prefs.Save();
        if (on)
        {
            TickRemote();
            _remote?.AskCode();
        }
        else if (_remote is { } link)
        {
            // The bot forgets the link before the connection closes (offline now: the next time it's on, a new code).
            _ = link.UnlinkAsync().ContinueWith(_ => link.Dispose(), TaskScheduler.Default);
            _remote = null;
        }
        PushState();
        return null;
    }

    /// <summary>The Discord account last said to be asking: a new one is told with a Windows notification too.</summary>
    string? _remoteAskTold;

    void OnRemoteChanged()
    {
        if (_remote?.Asking is { } ask && _remoteAskTold != ask.User)
        {
            _remoteAskTold = ask.User;
            Notify(L.F("{0} wants to control this PC from Discord. Allow it in Bloxkeep only if that's you.", ask.User), "warn", "remote", toast: false);
        }
        else if (_remote?.Asking is null)
            _remoteAskTold = null;
        PushState();
    }

    object? RemoteAllow(bool allow)
    {
        _remote?.Answer(allow);
        PushState();
        return null;
    }

    /// <summary>A new code for /pc link (the old one stops working).</summary>
    object? RemoteNewCode()
    {
        _remote?.AskCode();
        PushState();
        return null;
    }

    object RemoteDto() => new
    {
        on = Prefs.RemoteControl,
        connected = _remote?.Connected == true,
        linked = _remote?.Linked,
        user = _remote?.User,
        code = _remote?.Code,
        codeUntil = _remote?.CodeUntil is { } until ? new DateTimeOffset(until).ToUnixTimeMilliseconds() : (long?)null,
        // Asked only while unlocked: the lock screen says nothing of it, and an answer waits for the unlock.
        ask = _remote?.Asking is { } asking && _vault is not null && !_screenLocked ? new { user = asking.User, replacing = asking.Replacing } : null,
        busy = _remote?.Busy == true,
        pc = Environment.MachineName,
    };

    static JsonObject RemoteSay(string text) => new() { ["content"] = text };

    /// <summary>A command from Discord, on the window's thread: its answer (an error is said, never thrown).</summary>
    async Task<JsonObject> RemoteCommandAsync(RemoteCommand c)
    {
        try
        {
            if (_vault is null || _screenLocked)
                return RemoteSay(L.T("Bloxkeep is locked on this PC. Unlock it there first."));
            return c.Name switch
            {
                "status" => RemoteStatus(),
                "preset" => await RemotePresetAsync(c),
                "close" => RemoteClose(c.Value ?? ""),
                "switch" => await RemoteSwitchAsync(c.Value ?? ""),
                _ => RemoteSay("?"),
            };
        }
        catch (Exception ex) when (ex is UserError || RobloxApi.IsExpectedError(ex))
        {
            return RemoteSay(ErrorText(ex));
        }
    }

    JsonObject RemoteStatus()
    {
        var card = BoardMessage(_vault!.Data.Discord, RobloxProcesses.GetPlayers()) with { Footer = "Bloxkeep · " + Environment.MachineName };
        var payload = DiscordWebhook.Build(card, DiscordPing.None, embed: true, username: null, avatarUrl: null);
        return new JsonObject { ["embeds"] = payload["embeds"]?.DeepClone() };
    }

    async Task<JsonObject> RemotePresetAsync(RemoteCommand c)
    {
        var preset = _vault!.Data.Settings.Presets.FirstOrDefault(p => p.Name.Equals(c.Value?.Trim(), StringComparison.CurrentCultureIgnoreCase))
            ?? throw new UserError(L.F("No preset is called {0}.", c.Value ?? ""));
        var run = StartPresetAsync(preset.Id, force: false);
        if (await Task.WhenAny(run, Task.Delay(RemoteQuickAnswer)) == run)
            return PresetOutcome(preset, await run);
        _ = FollowUpPresetAsync(c.Id, preset, run);
        return RemoteSay(L.F("Opening {0}…", preset.Name));
    }

    async Task FollowUpPresetAsync(string id, LaunchPreset preset, Task<LaunchResult> run)
    {
        JsonObject outcome;
        try
        {
            outcome = PresetOutcome(preset, await run);
        }
        catch (Exception ex) when (ex is UserError || RobloxApi.IsExpectedError(ex))
        {
            outcome = RemoteSay(ErrorText(ex));
        }
        _remote?.Later(id, outcome);
    }

    JsonObject PresetOutcome(LaunchPreset preset, LaunchResult r)
    {
        if (r.NeedsConfirm)
            return RemoteSay($"{r.Message}\n\n{L.T("Nothing was opened: answer this in Bloxkeep on the PC.")}");
        int total = preset.AccountIds.Distinct().Count(id => FindAccount(id) is not null);
        var lines = new List<string> { r.Cancelled ? L.F("{0}: stopped after {1} of {2} accounts.", preset.Name, r.Launched, total)
            : L.F("{0}: opened {1} of {2} accounts.", preset.Name, r.Launched, total) };
        lines.AddRange((r.Errors ?? []).Take(10).Select(e => "- " + e));
        return RemoteSay(string.Join("\n", lines));
    }

    /// <summary>An account by the name Discord suggested (its Discord name), else its label or username.</summary>
    Account FindRemoteAccount(string name)
    {
        name = name.Trim();
        var accounts = _vault!.Data.Accounts;
        return accounts.FirstOrDefault(a => DiscordName(a).Equals(name, StringComparison.CurrentCultureIgnoreCase))
            ?? accounts.FirstOrDefault(a => a.Label.Equals(name, StringComparison.CurrentCultureIgnoreCase)
                                            || a.Username.Equals(name, StringComparison.CurrentCultureIgnoreCase))
            ?? throw new UserError(L.F("No account is called {0}.", name));
    }

    JsonObject RemoteClose(string name)
    {
        if (name.Trim().Equals(AllAccounts, StringComparison.OrdinalIgnoreCase) || name.Trim().Equals(L.T(AllAccounts), StringComparison.CurrentCultureIgnoreCase))
        {
            int open = _running.Count + _pending.Count;
            if (open == 0)
                return RemoteSay(L.T("No Roblox window is open on this PC."));
            CloseAll(force: false);
            return RemoteSay(L.F("Closing {0} Roblox window(s)…", open));
        }
        var account = FindRemoteAccount(name);
        if (!_running.ContainsKey(account.Id) && !_pending.Any(p => p.AccountId == account.Id))
            return RemoteSay(L.F("{0} has no Roblox window open.", DiscordName(account)));
        CloseAccounts([account.Id], force: false);
        return RemoteSay(L.F("Closing {0}…", DiscordName(account)));
    }

    async Task<JsonObject> RemoteSwitchAsync(string name)
    {
        var account = FindRemoteAccount(name);
        int moved = await SwitchServerAsync([account.Id]) is int n ? n : 0;
        return RemoteSay(moved > 0 ? L.F("{0} is moving to another server…", DiscordName(account))
            : L.T("Didn't switch: no other server of that game has room right now."));
    }
}
