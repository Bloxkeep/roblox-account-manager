using System.Security.Cryptography;
using System.Text;
using System.Text.Json.Nodes;

namespace RobloxManager;

/// <summary>
/// Settings › Discord › Control from Discord (off until turned on): Bloxkeep's Discord bot hands this PC the person's /pc
/// commands (RemoteLink, services/DiscordBot/worker/remote.js), once they typed the code shown here into /pc link. What a
/// command can do is only this: say how the accounts are doing (the status board's card), run a preset, close windows,
/// switch accounts to another server, picture their windows. Nothing is asked of the person on the PC: a preset that would ask first
/// (Multi-Roblox off…) isn't run. Locked, Bloxkeep answers only that it is locked, and names nothing.
/// Turned off: the bot is told to forget the link, the secret is dropped (a new PC for the bot next time), and the
/// connection closes.
/// Each vault has its own switch and secret (Prefs.RemoteControl): vaults are for different people, so the link is
/// the vault's, a PC of its own for the bot, connected only while that vault is the one offered or open. Another vault
/// unlocked, it is closed (Discord says that PC isn't connected), and nothing of the other vault reaches it.
/// </summary>
public sealed partial class AppController
{
    RemoteLink? _remote;
    /// <summary>The vault (<see cref="Prefs.VaultEntry.Id"/>) whose link <see cref="_remote"/> is.</summary>
    string? _remoteVault;

    /// <summary>The bot's address; the checks point it at a stand-in.</summary>
    static string? RemoteTestUrl => Environment.GetEnvironmentVariable("BLOXKEEP_REMOTE_URL") is { Length: > 0 } url ? url : null;

    static readonly byte[] RemoteEntropy = Encoding.UTF8.GetBytes("bloxkeep-remote-key");

    /// <summary>"All accounts", as Discord's suggestion for /pc close says it.</summary>
    const string AllAccounts = "All accounts";

    /// <summary>A preset that's still opening after this long is answered "opening…", and its outcome follows up later. Within
    /// the 1.5 s the bot waits before Discord's "thinking…" (remote.js ANSWER_WAIT).</summary>
    static readonly TimeSpan RemoteQuickAnswer = TimeSpan.FromSeconds(1);

    /// <summary>
    /// Connected while the vault offered or open has it on, closed when it's off or another vault is offered; the names
    /// Discord suggests kept up to date.
    /// </summary>
    void TickRemote()
    {
        ForgetRemoteLinksIfDue();
        string? vault = Vaults.Current?.Id;
        if (_remote is not null && (!Prefs.RemoteControl || _remoteVault != vault))
            StopRemote();
        if (Prefs.RemoteControl && _remote is null && RemoteSecret() is { } secret)
        {
            _remote = new RemoteLink(secret, RemoteCommandAsync, OnRemoteChanged, RemoteTestUrl);
            _remoteVault = vault;
        }
        if (_remote is null)
            return;
        bool open = RemoteOpen;
        _remote.Hello(Environment.MachineName,
            open ? _vault!.Data.Accounts.Select(DiscordName).Distinct(StringComparer.CurrentCultureIgnoreCase) : [],
            open ? _vault!.Data.Settings.Presets.Select(p => p.Name).Where(n => n.Length > 0).Distinct(StringComparer.CurrentCultureIgnoreCase) : [],
            L.Language);
    }

    static readonly TimeSpan RemoteForgetEvery = TimeSpan.FromMinutes(10), RemoteForgetAfterOff = TimeSpan.FromSeconds(30);
    DateTime _nextRemoteForget;
    bool _remoteForgetting;

    /// <summary>Links turned off or of vaults gone, told to the bot until it has said so (<see cref="Prefs.RemoteGone"/>).
    /// A key this Windows user can't read any more can't be told: dropped.</summary>
    void ForgetRemoteLinksIfDue()
    {
        if (_remoteForgetting || Prefs.RemoteGone.Count == 0 || DateTime.UtcNow < _nextRemoteForget)
            return;
        _nextRemoteForget = DateTime.UtcNow + RemoteForgetEvery;
        _remoteForgetting = true;
        _ = ForgetRemoteLinksAsync();
    }

    async Task ForgetRemoteLinksAsync()
    {
        try
        {
            foreach (string key in Prefs.RemoteGone.ToList())
            {
                string? secret = null;
                try
                {
                    secret = Encoding.UTF8.GetString(ProtectedData.Unprotect(Convert.FromBase64String(key), RemoteEntropy, DataProtectionScope.CurrentUser));
                }
                catch (Exception ex) when (ex is FormatException or CryptographicException)
                {
                }
                if (secret is null || await RemoteLink.ForgetAsync(secret, RemoteTestUrl))
                {
                    Prefs.RemoteGone.Remove(key);
                    Prefs.Save();
                }
            }
        }
        finally
        {
            _remoteForgetting = false;
        }
    }

    /// <summary>The link's own vault is open and unlocked: only then does a command see anything.</summary>
    bool RemoteOpen => _vault is not null && !_screenLocked && _remoteVault == Vaults.Current?.Id;

    void StopRemote()
    {
        _remote?.Dispose();
        _remote = null;
        _remoteVault = null;
    }

    /// <summary>This vault's secret for the bot, made when it's turned on (random, kept encrypted for this Windows user).</summary>
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
        // Off, the secret goes: turned on again, this is a new PC for the bot, linked to nobody until a new code is
        // typed and allowed. The unlink below doesn't reach a bot that can't be reached right now, and with the same
        // secret the PC came back still linked to that Discord account, without asking (2026-10-30 review).
        if (!on)
        {
            Prefs.ForgetRemoteKey(Prefs.RemoteKey);
            Prefs.RemoteKey = null;
            _nextRemoteForget = DateTime.UtcNow + RemoteForgetAfterOff; // after the unlink below, if that one got through
        }
        Prefs.Save();
        if (on)
        {
            TickRemote();
            _remote?.AskCode();
        }
        else if (_remote is { } link)
        {
            // The bot forgets the link before the connection closes.
            _ = link.UnlinkAsync().ContinueWith(_ => link.Dispose(), TaskScheduler.Default);
            _remote = null;
            _remoteVault = null;
        }
        PushState();
        return null;
    }

    /// <summary>The Discord account last said to be asking: a new one is told with a Windows notification too.</summary>
    string? _remoteAskTold;

    void OnRemoteChanged()
    {
        if (_remote?.Asking is { } ask && _remoteAskTold != (ask.Id ?? ask.User))
        {
            _remoteAskTold = ask.Id ?? ask.User;
            Notify(L.F("{0} wants to control this PC from Discord. Allow it in Bloxkeep only if that's you.", ask.User), "warn", "remote", toast: false);
        }
        else if (_remote?.Asking is null)
            _remoteAskTold = null;
        PushState();
    }

    /// <summary>The answer to someone linking this PC (the page's Allow / Don't allow).</summary>
    object? RemoteAllow(bool allow, string? id = null)
    {
        _remote?.Answer(allow, id);
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
        ask = _remote?.Asking is { } asking && RemoteOpen ? new { user = asking.User, replacing = asking.Replacing, id = asking.Id } : null,
        busy = _remote?.Busy == true,
        pc = Environment.MachineName,
    };

    /// <summary>
    /// An answer's text, with names hidden as Discord's own switch says (Settings › Discord): what the app wrote for itself
    /// (a launch error, a question a preset would ask) follows the app's switch, and showed real names in Discord while
    /// Discord's was on (2026-10-30 review).
    /// </summary>
    JsonObject RemoteSay(string text) => new() { ["content"] = _vault?.Data.Discord.HideNames == true ? HideNamesIn(text) : text };

    /// <summary>
    /// A command from Discord, on the window's thread: its answer (an error is said, never thrown); null when it was
    /// answered already (the pictures of /pc screenshot go up on their own).
    /// </summary>
    async Task<JsonObject?> RemoteCommandAsync(RemoteCommand c)
    {
        try
        {
            // Another vault open (the link's own is closed a moment later): as locked, nothing of it said.
            if (!RemoteOpen)
                return RemoteSay(L.T("Bloxkeep is locked on this PC. Unlock it there first."));
            return c.Name switch
            {
                "status" => RemoteStatus(),
                "preset" => await RemotePresetAsync(c),
                "close" => RemoteClose(c.Value ?? ""),
                "switch" => await RemoteSwitchAsync(c.Value ?? ""),
                "screenshot" => await RemoteScreenshotAsync(c),
                _ => RemoteSay("?"),
            };
        }
        catch (Exception ex) when (ex is UserError || RobloxApi.IsExpectedError(ex))
        {
            return RemoteSay(ErrorText(ex));
        }
        // Anything else (a picture Windows wouldn't make, memory running out): answered and logged, never let out, where it
        // would end the link to the bot until Bloxkeep restarts (2026-10-30 review).
        catch (Exception ex)
        {
            Log.Write(ex);
            return RemoteSay(L.T("Something went wrong on this PC. Try again."));
        }
    }

    /// <summary>The status board's card, as it is now, named after this PC.</summary>
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
        catch (Exception ex)
        {
            Log.Write(ex);
            outcome = RemoteSay(L.T("Something went wrong on this PC. Try again."));
        }
        _remote?.Later(id, outcome);
    }

    /// <summary>How a preset's run went: opened how many, what went wrong, or the question it would have asked.</summary>
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

    static bool IsAllAccounts(string name) =>
        name.Trim().Equals(AllAccounts, StringComparison.OrdinalIgnoreCase) || name.Trim().Equals(L.T(AllAccounts), StringComparison.CurrentCultureIgnoreCase);

    /// <summary>The accounts with a Roblox window on this PC, in the list's order.</summary>
    List<Account> PlayingAccounts() => _vault!.Data.Accounts.Where(a => _running.ContainsKey(a.Id)).ToList();

    JsonObject RemoteClose(string name)
    {
        if (IsAllAccounts(name))
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

    /// <summary>A /pc switch waiting for Roblox's server list: commands run side by side, and Hopping only knows of it after.</summary>
    bool _remoteSwitching;

    async Task<JsonObject> RemoteSwitchAsync(string name)
    {
        // Asked again before the last switch landed (Discord's /pc sent over and over): not moved again meanwhile.
        if (Hopping() || _remoteSwitching)
            return RemoteSay(L.T("Accounts are still moving to another server. Try again once they're in."));
        _remoteSwitching = true;
        try
        {
            return await RemoteSwitchNowAsync(name);
        }
        finally
        {
            _remoteSwitching = false;
        }
    }

    async Task<JsonObject> RemoteSwitchNowAsync(string name)
    {
        if (IsAllAccounts(name))
        {
            var playing = PlayingAccounts();
            if (playing.Count == 0)
                return RemoteSay(L.T("No Roblox window is open on this PC."));
            // Those in one game go to one server together, as Switch server does for several selected.
            int all = await SwitchServerAsync(playing.Select(a => a.Id).ToArray()) is int m ? m : 0;
            return RemoteSay(all > 0 ? L.F("Moving {0} account(s) to another server…", all)
                : L.T("Didn't switch: no other server of that game has room right now."));
        }
        var account = FindRemoteAccount(name);
        int moved = await SwitchServerAsync([account.Id]) is int n ? n : 0;
        return RemoteSay(moved > 0 ? L.F("{0} is moving to another server…", DiscordName(account))
            : L.T("Didn't switch: no other server of that game has room right now."));
    }

    /// <summary>Discord shows 10 pictures in a message at most.</summary>
    const int MaxShots = 10;

    /// <summary>
    /// Between two screenshots of this PC: picturing shows minimized windows for a moment and holds anti-AFK back, so
    /// asked again and again (by the person, or by whoever got into their Discord) the windows still get their nudges.
    /// </summary>
    static readonly TimeSpan ShotGap = TimeSpan.FromSeconds(15);

    /// <summary>A window being pictured: anti-AFK waits (both make a window unseen for a moment, and would undo each other).</summary>
    bool _picturing;
    /// <summary>When the last screenshot was done.</summary>
    DateTime _lastShot = DateTime.MinValue;

    /// <summary>
    /// /pc screenshot: a picture of one account's Roblox window, or of each (10 at most), sent to Discord by this PC
    /// (RemoteLink.ShotAsync), seen there only by who asked. What can't be pictured is said under them. One at a time,
    /// <see cref="ShotGap"/> apart; the person at the PC is told each time.
    /// </summary>
    async Task<JsonObject?> RemoteScreenshotAsync(RemoteCommand c)
    {
        var now = DateTime.UtcNow;
        if (_picturing)
            return RemoteSay(L.T("Already taking a screenshot. Try again in a moment."));
        if (now - _lastShot < ShotGap)
            return RemoteSay(L.F("Wait {0} s, then try again.", (int)Math.Ceiling((ShotGap - (now - _lastShot)).TotalSeconds)));
        string name = c.Value ?? "";
        List<Account> accounts;
        if (IsAllAccounts(name))
        {
            accounts = PlayingAccounts();
            if (accounts.Count == 0)
                return RemoteSay(L.T("No Roblox window is open on this PC."));
        }
        else
        {
            var account = FindRemoteAccount(name);
            if (!_running.ContainsKey(account.Id))
                return RemoteSay(L.F("{0} has no Roblox window open.", DiscordName(account)));
            accounts = [account];
        }
        if (_remote is not { } link)
            return null;
        var pictures = new List<byte[]>();
        var named = new List<string>();
        var notes = new List<string>();
        _picturing = true;
        try
        {
            // A nudge under way finishes first (a few seconds at most).
            for (int i = 0; _nudging && i < 50; i++)
                await Task.Delay(100);
            Notify(L.F("{0} took a screenshot of this PC's Roblox windows from Discord.", link.User ?? "Discord"), "info", "remote");
            foreach (var account in accounts.Take(MaxShots))
            {
                // Locked, or turned off, since it was asked: no pictures go out.
                if (!RemoteOpen)
                    return null;
                if (!_running.TryGetValue(account.Id, out var run))
                    continue;
                var window = RobloxProcesses.MainWindowOf(RobloxProcesses.VisibleWindows([run.Pid])[run.Pid]);
                var shot = await WindowShot.CaptureAsync(window, accounts.Count == 1 ? 1920 : 1280);
                if (shot is not { } s)
                {
                    notes.Add(L.F("{0}: no picture (its window isn't answering, or Roblox is still opening).", DiscordName(account)));
                    continue;
                }
                pictures.Add(s.Jpeg);
                named.Add(DiscordName(account));
                if (s.Black)
                    notes.Add(L.F("{0}: the picture came out black (Windows may be locked, or the game isn't drawing).", DiscordName(account)));
            }
        }
        finally
        {
            _picturing = false;
            _lastShot = DateTime.UtcNow;
        }
        if (accounts.Count > MaxShots)
            notes.Add(L.F("{0} more not pictured: Discord shows 10 at most.", accounts.Count - MaxShots));
        if (pictures.Count == 0)
            return RemoteSay(string.Join("\n", notes));
        string content = string.Join("\n", new[] { string.Join(" · ", named) }.Concat(notes));
        return await link.ShotAsync(c, content, pictures) ? null : RemoteSay(L.T("Couldn't send the picture to Discord. Try again in a moment."));
    }
}
