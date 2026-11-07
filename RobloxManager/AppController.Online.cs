using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Text.RegularExpressions;
using Microsoft.Win32;

namespace RobloxManager;

/// <summary>
/// Settings › Bloxkeep › Privacy › Add me to the online count (on until turned off): while Bloxkeep is open, locked or
/// not, it tells Bloxkeep's Discord bot so every minute (further apart when the bot says so) and says goodbye as it
/// closes (services/DiscordBot/worker/online.js). What goes: an id made from this PC (<see cref="OnlineIdForThisPc"/>),
/// Bloxkeep's version, the app's language, how long this PC has had Bloxkeep in three rough groups
/// (<see cref="InstallAge"/>), once, that this is a new install's first open, and what this PC plays now: the games its
/// accounts are in, how many run, the drops since the last beat and its name on the alts board if it joined; never an
/// account. The bot keeps numbers per day of the first ones, and the rest only in memory while this PC counts as open. Turned off: a goodbye, the id forgotten, nothing more. Only an installed Bloxkeep counts:
/// a development copy or the checks never do, unless BLOXKEEP_ONLINE_URL points them at a server of their own.
/// </summary>
public sealed partial class AppController
{
    /// <summary>Bloxkeep's Discord bot (services/DiscordBot/worker), which counts.</summary>
    const string OnlineUrl = "https://bloxkeep-bot.bloxkeep-app.workers.dev/online/ping";
    /// <summary>Until the bot says otherwise (each answer says when to send the next, from a minute to six hours).</summary>
    static readonly TimeSpan OnlineBeat = TimeSpan.FromMinutes(1);
    static readonly TimeSpan OnlineBeatMin = TimeSpan.FromMinutes(1), OnlineBeatMax = TimeSpan.FromHours(6);
    /// <summary>After a beat that didn't go through (no internet yet, just after Windows started); doubled each time it fails again.</summary>
    static readonly TimeSpan OnlineRetry = TimeSpan.FromMinutes(2);
    static readonly HttpClient OnlineHttp = new(new SocketsHttpHandler { UseCookies = false, AllowAutoRedirect = false })
    {
        Timeout = TimeSpan.FromSeconds(20),
    };

    DateTime _nextOnline;
    /// <summary>How far apart the bot last said beats should be; a failed beat is tried again sooner, never later than this.</summary>
    TimeSpan _onlineEvery = OnlineBeat;
    TimeSpan _onlineRetry = OnlineRetry;
    bool _onlineSending;
    /// <summary>The last beat failed: written once, then once more when one goes through.</summary>
    bool _onlineFailing;
    /// <summary>A beat went through since the app started (or the switch was turned on): there is a goodbye to say.</summary>
    bool _onlineSaid;

    static string? OnlineTestUrl => Environment.GetEnvironmentVariable("BLOXKEEP_ONLINE_URL") is { Length: > 0 } url ? url : null;

    bool OnlineAllowed => _updater.Supported || OnlineTestUrl is not null;

    void TickOnline(DateTime now)
    {
        if (_onlineSending || now < _nextOnline || !Prefs.CountOnline || !OnlineAllowed)
            return;
        _nextOnline = now + _onlineEvery;
        _ = BeatAsync();
    }

    async Task BeatAsync()
    {
        // This PC's id, also in place of one an earlier Bloxkeep made at random.
        string? pc = OnlineIdForThisPc();
        if (Prefs.OnlineId is null || (pc is not null && Prefs.OnlineId != pc))
        {
            Prefs.OnlineId = pc ?? Guid.NewGuid().ToString("N");
            Prefs.Save();
        }
        string id = Prefs.OnlineId;
        // Roughly how long this PC has had Bloxkeep, and whether this is a new install's first open (not an old copy just updated).
        string age = InstallAge();
        bool first = !Prefs.FirstOpenTold && age == "new";
        _onlineSending = true;
        if (!Prefs.BoardNameChosen && Prefs.BoardName is null)
        {
            // On the alts board from the start under a made-up name, changed or emptied (off the board) in Statistics.
            Prefs.BoardName = RandomBoardName();
            Prefs.BoardNameChosen = true;
            Prefs.Save();
            PushState();
        }
        try
        {
            // What this PC plays now, for the sums the bot tells back (CommunityDto): the games its accounts are in, how many
            // run, the drops since the last beat; its name on the alts board if it joined it; the theme picked, for Popular in
            // the theme picker (one of Bloxkeep's own: not a custom one, not "follow Windows"). No account.
            var drops = _onlineDrops.ToList();
            _onlineDrops.Clear();
            using var res = await PostOnlineAsync(new
            {
                id, app = _updater.CurrentVersion, language = L.Language, age, first,
                games = PlayingUniverses(), alts = _running.Count, drops = drops.Select(d => new { game = d.Universe, why = d.Why }), board = Prefs.BoardName,
                theme = Prefs.Theme is Themes.FollowWindows or Themes.Custom ? null : Prefs.Theme,
            });
            if (!res.IsSuccessStatusCode)
            {
                _onlineDrops.InsertRange(0, drops.Take(OnlineDropsMax - _onlineDrops.Count));
                if (!_onlineFailing)
                    Log.Note($"Online count: the bot answered HTTP {(int)res.StatusCode}; tried again sooner, then less and less often.");
                _onlineFailing = true;
                RetryOnline();
                return;
            }
            // Turned off while this was on its way: the goodbye went before it, so it goes again.
            if (Prefs.OnlineId != id)
            {
                _ = SayOfflineAsync(id);
                return;
            }
            _onlineSaid = true;
            if (_onlineFailing)
                Log.Note("Online count: recovered.");
            _onlineFailing = false;
            _onlineRetry = OnlineRetry;
            if (!Prefs.FirstOpenTold)
            {
                Prefs.FirstOpenTold = true;
                Prefs.Save();
            }
            using var answer = JsonDocument.Parse(await res.Content.ReadAsStringAsync());
            if (answer.RootElement.ValueKind == JsonValueKind.Object && answer.RootElement.TryGetProperty("next", out var next)
                && next.ValueKind == JsonValueKind.Number && next.TryGetInt32(out int seconds))
            {
                _onlineEvery = TimeSpan.FromSeconds(Math.Clamp(seconds, OnlineBeatMin.TotalSeconds, OnlineBeatMax.TotalSeconds));
                _nextOnline = DateTime.UtcNow + _onlineEvery;
            }
            if (answer.RootElement.ValueKind == JsonValueKind.Object && answer.RootElement.TryGetProperty("community", out var community)
                && community.ValueKind == JsonValueKind.Object)
            {
                _community = community.Clone();
                _communityAt = DateTime.UtcNow;
                PushState();
            }
        }
        catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException or JsonException)
        {
            if (!_onlineFailing)
                Log.Note($"Online count: beat failed ({LogKindOf(ex)}); tried again sooner, then less and less often.");
            _onlineFailing = true;
            RetryOnline();
        }
        finally
        {
            _onlineSending = false;
        }
    }

    /// <summary>Sooner than the next beat: two minutes, then four, eight… up to the usual gap, so a bot that can't answer
    /// (no internet, or over its day's budget) isn't asked more often than usual.</summary>
    void RetryOnline()
    {
        var wait = _onlineRetry < _onlineEvery ? _onlineRetry : _onlineEvery;
        _nextOnline = DateTime.UtcNow + wait;
        _onlineRetry = wait + wait;
    }

    /// <summary>"Closing": no longer counted at once, rather than once its last beat is too old. Off the window's thread, so it can be waited for.</summary>
    static Task SayOfflineAsync(string id) => Task.Run(async () =>
    {
        try
        {
            using var res = await PostOnlineAsync(new { id, gone = true });
        }
        catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException)
        {
            // Counted until its last beat is too old (a quarter of an hour at most).
        }
    });

    /// <summary>How long this PC has had Bloxkeep: "new" (under a day), "week" (under a week) or "old", from when its data
    /// folder was made, which stays through updates and reinstalls that keep the accounts.</summary>
    static string InstallAge()
    {
        try
        {
            double days = (DateTime.UtcNow - Directory.GetCreationTimeUtc(Vault.DataDir)).TotalDays;
            return days < 1 ? "new" : days < 7 ? "week" : "old";
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or ArgumentException)
        {
            return "old";
        }
    }

    /// <summary>The id this PC is counted under: Windows' MachineGuid hashed with Bloxkeep's own label, so it leads back to
    /// nothing (no account, vault, Roblox or Discord) yet stays the same after a reinstall, a data wipe or for a second copy:
    /// one PC, one in the count. Null where Windows' id can't be read (a random one is used then).</summary>
    static string? OnlineIdForThisPc()
    {
        try
        {
            using var key = RegistryKey.OpenBaseKey(RegistryHive.LocalMachine, RegistryView.Registry64)
                .OpenSubKey(@"SOFTWARE\Microsoft\Cryptography");
            if (key?.GetValue("MachineGuid") is not string guid || guid.Trim().Length == 0)
                return null;
            byte[] hash = SHA256.HashData(Encoding.UTF8.GetBytes("bloxkeep-online:" + guid.Trim().ToLowerInvariant()));
            return Convert.ToHexString(hash, 0, 16).ToLowerInvariant();
        }
        catch (Exception ex) when (ex is System.Security.SecurityException or UnauthorizedAccessException or IOException)
        {
            return null;
        }
    }

    static async Task<HttpResponseMessage> PostOnlineAsync(object body)
    {
        using var content = new StringContent(JsonSerializer.Serialize(body), Encoding.UTF8, "application/json");
        return await OnlineHttp.PostAsync(OnlineTestUrl ?? OnlineUrl, content);
    }

    /// <summary>Bloxkeep closing: the goodbye, if the bot answers within a moment; not for an update in the background, back within seconds.</summary>
    void SayOfflineOnExit()
    {
        if (_onlineSaid && _relaunchArgs is null && OnlineAllowed && Prefs.OnlineId is { } id)
            SayOfflineAsync(id).Wait(TimeSpan.FromSeconds(2));
    }

    // ───────────────────────────── what Bloxkeep's users play ─────────────────────────────

    /// <summary>An account dropped out since the last beat, for the bot's radar: its game (universe) and why.</summary>
    readonly record struct OnlineDrop(long Universe, string Why);
    readonly List<OnlineDrop> _onlineDrops = new();
    const int OnlineDropsMax = 20;
    /// <summary>What the bot last said Bloxkeep's users play (services/DiscordBot/worker/online.js communityOf).</summary>
    JsonElement? _community;
    DateTime _communityAt;

    /// <summary>The games (Roblox universes) this PC's accounts are in now, the ones Bloxkeep knows the universe of.</summary>
    long[] PlayingUniverses() => _running.Values
        .Select(r => _logs.StateOf(r.Pid)?.PlaceId ?? r.Launch.Target?.PlaceId ?? 0)
        .Select(place => place > 0 ? _games.GetValueOrDefault(place)?.UniverseId ?? 0 : 0)
        .Where(u => u > 0).Distinct().Take(20).ToArray();

    /// <summary>A disconnect the radar counts (Activity's "disconnected"): kicked by the game, the connection lost, the server
    /// shut down, or couldn't join. Not the person's own (idle, another device, a private server, an update).</summary>
    void NoteDrop(long placeId, int code, DateTime at)
    {
        string? why = code switch
        {
            279 => "join",
            267 => "kick",
            ServerShutdown => "shutdown",
            _ when LostConnection(code) => "lost",
            _ => null,
        };
        long universe = _games.GetValueOrDefault(placeId)?.UniverseId ?? 0;
        if (why is null || universe <= 0 || DateTime.UtcNow - at > TimeSpan.FromMinutes(5) || !Prefs.CountOnline || _onlineDrops.Count >= OnlineDropsMax)
            return;
        _onlineDrops.Add(new OnlineDrop(universe, why));
    }

    /// <summary>What the Games tab and Statistics show of it: null while nothing came yet. With the count on, from each beat's
    /// answer; off, from <see cref="CommunityNow"/>.</summary>
    object? CommunityDto() => _community is { } c ? new { data = c, at = new DateTimeOffset(_communityAt).ToUnixTimeMilliseconds() } : null;

    /// <summary>The bot's sums for an app not in the count (GET /online/community): what is played, the radar, the board,
    /// nothing of this PC and nothing sent about it.</summary>
    static string CommunityUrl => (OnlineTestUrl ?? OnlineUrl).Replace("/online/ping", "/online/community", StringComparison.Ordinal);
    bool _communityAsking;
    /// <summary>The last ask for the sums failed: written once, then once more when one goes through.</summary>
    bool _communityFailing;

    /// <summary>The Games tab or Statistics is open while the count is off: the sums again, at most once a minute (the bot keeps
    /// them 30 s). With the count on, the beats bring them.</summary>
    object? CommunityNow()
    {
        if (Prefs.CountOnline || !OnlineAllowed || _communityAsking || DateTime.UtcNow - _communityAt < TimeSpan.FromSeconds(55))
            return null;
        _communityAsking = true;
        _ = AskCommunityAsync();
        return null;
    }

    async Task AskCommunityAsync()
    {
        try
        {
            using var res = await OnlineHttp.GetAsync(CommunityUrl);
            if (!res.IsSuccessStatusCode)
            {
                if (!_communityFailing)
                    Log.Note($"Community sums: the bot answered HTTP {(int)res.StatusCode}; asked again in a minute.");
                _communityFailing = true;
                return;
            }
            if (_communityFailing)
                Log.Note("Community sums: recovered.");
            _communityFailing = false;
            using var answer = JsonDocument.Parse(await res.Content.ReadAsStringAsync());
            if (!Prefs.CountOnline && answer.RootElement.ValueKind == JsonValueKind.Object && answer.RootElement.TryGetProperty("community", out var community)
                && community.ValueKind == JsonValueKind.Object)
            {
                _community = community.Clone();
                _communityAt = DateTime.UtcNow;
                PushState();
            }
        }
        catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException or JsonException)
        {
            if (!_communityFailing)
                Log.Note($"Community sums: not loaded ({LogKindOf(ex)}); asked again in a minute.");
            _communityFailing = true;
        }
        finally
        {
            _communityAsking = false;
        }
    }

    static readonly string[] BoardWords1 = ["Swift", "Brave", "Lucky", "Quiet", "Bold", "Calm", "Sly", "Wild", "Mighty", "Clever", "Rapid", "Silent", "Happy", "Fuzzy", "Shiny", "Cosmic"];
    static readonly string[] BoardWords2 = ["Fox", "Otter", "Panda", "Tiger", "Wolf", "Hawk", "Koala", "Lynx", "Falcon", "Badger", "Raven", "Shark", "Gecko", "Moose", "Bison", "Owl"];

    /// <summary>A made-up name for the alts board, like "Swift Otter 42": nothing of the PC or its accounts in it.</summary>
    public static string RandomBoardName() =>
        $"{BoardWords1[RandomNumberGenerator.GetInt32(BoardWords1.Length)]} {BoardWords2[RandomNumberGenerator.GetInt32(BoardWords2.Length)]} {RandomNumberGenerator.GetInt32(10, 100)}";

    /// <summary>A name for the alts board, tidied (spaces made one), or null when it isn't one (2 to 20 letters, digits,
    /// spaces, dots, dashes, underscores, from a letter or digit: the bot takes nothing else).</summary>
    public static string? BoardNameOf(string? name)
    {
        string tidy = Regex.Replace((name ?? "").Normalize(NormalizationForm.FormC), @"\s+", " ").Trim();
        return Regex.IsMatch(tidy, @"^[\p{L}\p{N}][\p{L}\p{N} ._-]{1,19}$") ? tidy : null;
    }

    /// <summary>{name}: on the alts board under this name; empty: off it. A name that isn't one is refused.</summary>
    object? SetBoardName(string name)
    {
        string? tidy = name.Trim().Length == 0 ? null : BoardNameOf(name);
        if (name.Trim().Length > 0 && tidy is null)
            throw new UserError(L.T("Use 2 to 20 letters or digits (spaces, dots, dashes and underscores too)."));
        Prefs.BoardName = tidy;
        Prefs.BoardNameChosen = true;
        Prefs.Save();
        _nextOnline = DateTime.MinValue; // on the board (or off it) at the next tick, not the next beat
        PushState();
        return new { name = tidy };
    }

    object? SetCountOnline(bool on)
    {
        Prefs.CountOnline = on;
        if (!on)
        {
            _community = null; // the sums again without this PC's place (CommunityNow)
            _communityAt = default;
            _onlineDrops.Clear();
        }
        if (!on && Prefs.OnlineId is { } id)
        {
            if (_onlineSaid && OnlineAllowed)
                _ = SayOfflineAsync(id);
            Prefs.OnlineId = null;
            _onlineSaid = false;
        }
        Prefs.Save();
        _nextOnline = DateTime.MinValue; // on again: counted at once (this PC's id again)
        PushState();
        return null;
    }
}
