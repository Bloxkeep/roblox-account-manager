using System.Text;
using System.Text.Json;

namespace RobloxManager;

/// <summary>
/// Settings › Bloxkeep updates › Privacy › Add me to the online count (on until turned off): while Bloxkeep is open,
/// locked or not, it tells Bloxkeep's Discord bot so every ten minutes (further apart when the bot says so, once many
/// people have it open) and says goodbye as it closes, so its developers can see how many people have it open right
/// now (DiscordBot/worker/online.js; the bot's status shows the number too). What goes: a
/// random id made for this Bloxkeep (<see cref="Prefs.OnlineId"/>, linked to no account, vault, Roblox or Discord),
/// Bloxkeep's version and the app's language, nothing else. Turned off: a goodbye, the id forgotten, nothing more.
/// Only an installed Bloxkeep counts: a development copy or the checks never do, unless BLOXKEEP_ONLINE_URL points them
/// at a server of their own.
/// </summary>
public sealed partial class AppController
{
    /// <summary>Bloxkeep's Discord bot (DiscordBot/worker), which counts.</summary>
    const string OnlineUrl = "https://bloxkeep-bot.bloxkeep-app.workers.dev/online/ping";
    /// <summary>Until the bot says otherwise (each answer says when to send the next, from a minute to six hours).</summary>
    static readonly TimeSpan OnlineBeat = TimeSpan.FromMinutes(10);
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
    /// <summary>A beat went through since the app started (or the switch was turned on): there is a goodbye to say.</summary>
    bool _onlineSaid;

    static string? OnlineTestUrl => Environment.GetEnvironmentVariable("BLOXKEEP_ONLINE_URL") is { Length: > 0 } url ? url : null;

    /// <summary>An installed Bloxkeep, or the checks with a server of their own.</summary>
    bool OnlineAllowed => _updater.Supported || OnlineTestUrl is not null;

    void TickOnline(DateTime now)
    {
        if (_onlineSending || now < _nextOnline || !Prefs.CountOnline || !OnlineAllowed)
            return;
        _nextOnline = now + _onlineEvery;
        _ = BeatAsync();
    }

    /// <summary>"Open now": the bot's answer says when to say it again.</summary>
    async Task BeatAsync()
    {
        if (Prefs.OnlineId is null)
        {
            Prefs.OnlineId = Guid.NewGuid().ToString("N");
            Prefs.Save();
        }
        string id = Prefs.OnlineId;
        _onlineSending = true;
        try
        {
            using var res = await PostOnlineAsync(new { id, app = _updater.CurrentVersion, language = L.Language });
            if (!res.IsSuccessStatusCode)
            {
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
            _onlineRetry = OnlineRetry;
            using var answer = JsonDocument.Parse(await res.Content.ReadAsStringAsync());
            if (answer.RootElement.ValueKind == JsonValueKind.Object && answer.RootElement.TryGetProperty("next", out var next)
                && next.ValueKind == JsonValueKind.Number && next.TryGetInt32(out int seconds))
            {
                _onlineEvery = TimeSpan.FromSeconds(Math.Clamp(seconds, OnlineBeatMin.TotalSeconds, OnlineBeatMax.TotalSeconds));
                _nextOnline = DateTime.UtcNow + _onlineEvery;
            }
        }
        catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException or JsonException)
        {
            RetryOnline();
        }
        finally
        {
            _onlineSending = false;
        }
    }

    /// <summary>
    /// Sooner than the next beat would be: two minutes, then four, eight… up to the usual gap, so a bot that can't answer
    /// (no internet, or it is over its day's budget) isn't asked more often than usual.
    /// </summary>
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

    object? SetCountOnline(bool on)
    {
        Prefs.CountOnline = on;
        if (!on && Prefs.OnlineId is { } id)
        {
            if (_onlineSaid && OnlineAllowed)
                _ = SayOfflineAsync(id);
            Prefs.OnlineId = null;
            _onlineSaid = false;
        }
        Prefs.Save();
        _nextOnline = DateTime.MinValue; // on again: counted at once, with a new id
        PushState();
        return null;
    }
}
