using System.Text.Json;

namespace RobloxManager;

/// <summary>
/// A new Bloxkeep found within minutes: GitHub's API takes an hour's worth of update checks from one address at most
/// (an internet café's PCs share one), so the update check itself stays hourly; but every few minutes Bloxkeep's bot is
/// asked which version is out (services/DiscordBot/worker /latest, read from GitHub every two minutes and kept a minute
/// per Cloudflare location; nothing is sent with the question). One newer than this copy brings the update check
/// forward to now, once per version, so a copy left on the lock screen installs it there before anyone unlocks.
/// Only an installed Bloxkeep with automatic updates on asks; the checks point it at a server of their own.
/// </summary>
public sealed partial class AppController
{
    const string LatestUrl = "https://bloxkeep-bot.bloxkeep-app.workers.dev/latest";
    static readonly TimeSpan LatestLook = TimeSpan.FromMinutes(5);

    DateTime _nextLatestLook;
    bool _latestLooking;
    /// <summary>The newer version the bot last told of: the update check is brought forward once for it.</summary>
    string? _latestSeen;

    static string? LatestTestUrl => Environment.GetEnvironmentVariable("BLOXKEEP_LATEST_URL") is { Length: > 0 } url ? url : null;

    void TickLatest(DateTime now)
    {
        if (_latestLooking || now < _nextLatestLook || !Prefs.CheckUpdates || !(_updater.Supported || LatestTestUrl is not null))
            return;
        _nextLatestLook = now + LatestLook;
        _ = LookAtLatestAsync();
    }

    async Task LookAtLatestAsync()
    {
        _latestLooking = true;
        try
        {
            using var res = await OnlineHttp.GetAsync(LatestTestUrl ?? LatestUrl);
            if (!res.IsSuccessStatusCode)
                return;
            using var doc = JsonDocument.Parse(await res.Content.ReadAsStringAsync());
            if (doc.RootElement.ValueKind == JsonValueKind.Object && doc.RootElement.TryGetProperty("version", out var v)
                && v.ValueKind == JsonValueKind.String && v.GetString() is { } latest && latest != _latestSeen
                && IsNewerVersion(latest, _updater.CurrentVersion))
            {
                _latestSeen = latest;
                if (_nextUpdateCheck > DateTime.UtcNow)
                    _nextUpdateCheck = DateTime.UtcNow;
            }
        }
        catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException or JsonException)
        {
            // Asked again in a few minutes; the hourly check goes on meanwhile.
        }
        finally
        {
            _latestLooking = false;
        }
    }

    /// <summary>"1.0.13" against "1.0.12", or against "1.0.13-beta.1" (a beta comes before its release); anything unreadable isn't newer.</summary>
    static bool IsNewerVersion(string latest, string? current)
    {
        static (Version? Number, bool Pre) Read(string? text)
        {
            string s = (text ?? "").Trim().TrimStart('v', 'V');
            int dash = s.IndexOf('-');
            return (Version.TryParse(dash < 0 ? s : s[..dash], out var n) ? n : null, dash >= 0);
        }
        var (have, havePre) = Read(current);
        var (got, gotPre) = Read(latest);
        if (have is null || got is null || gotPre)
            return false;
        return got > have || (got == have && havePre);
    }
}
