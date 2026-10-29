using System.Text.Json.Serialization;
using System.Text.RegularExpressions;

namespace RobloxManager;

/// <summary>A Roblox player an account follows: its rejoins go to the server that player is in by then.</summary>
public sealed record FollowedPlayer(long UserId, string Name);

/// <summary>Where to send an account: a place, optionally a specific server (job) or a private server.</summary>
public sealed record GameTarget(long PlaceId, string? JobId = null, string? LinkCode = null, string? ShareCode = null)
{
    /// <summary>The player this account follows, kept through rejoins, restarts and saved sessions: each rejoin looks the
    /// player up again and goes to their current server instead of this place.</summary>
    public FollowedPlayer? Follow { get; init; }

    /// <summary>A private server's access code (Browse servers): only accounts that own it or were invited get in, unlike a
    /// link code, which lets in anyone with the link.</summary>
    public string? AccessCode { get; init; }

    /// <summary>A private server, by its link or by its access code.</summary>
    [JsonIgnore]
    public bool IsPrivate => LinkCode is not null || AccessCode is not null;

    static readonly Regex GamePath = new(@"/games/(\d+)", RegexOptions.IgnoreCase | RegexOptions.CultureInvariant);
    // A pair also right after "roblox://" or at the start ("placeId=…&gameInstanceId=…"), not only after ? or &.
    static readonly Regex QueryPair = new(@"(?:[?&]|//|^)([^=&#\s/?]+)=([^&#\s]*)", RegexOptions.CultureInvariant);

    /// <summary>Accepts a place id, a roblox.com/games/… link (privateServerLinkCode, gameInstanceId), a
    /// roblox.com/share?code=…&amp;type=Server link (also roblox://navigation/share_links or wrapped in ro.blox.com), or
    /// a roblox:// deep link with placeId, also with accessCode (what Browse servers puts in the game box).</summary>
    public static bool TryParse(string placeInput, string jobInput, out GameTarget? target, out string error) =>
        TryParse(placeInput, jobInput, out target, out error, depth: 0);

    static bool TryParse(string placeInput, string jobInput, out GameTarget? target, out string error, int depth)
    {
        target = null;
        error = "";
        string input = placeInput.Trim();
        string job = jobInput.Trim();
        if (input.Length == 0)
        {
            error = L.T("Enter a Place ID or paste a Roblox game link.");
            return false;
        }

        long placeId = 0;
        string? linkCode = null, shareCode = null, jobId = null, accessCode = null;
        if (!long.TryParse(input, out placeId))
        {
            // Words, not a link (a game typed by name and not picked from the search): say what to do instead.
            if (!input.Contains('/') && !input.Contains('?') && !input.Contains("roblox", StringComparison.OrdinalIgnoreCase))
            {
                error = L.T("Pick the game from the search list, or paste a game link or Place ID.");
                return false;
            }
            var query = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
            foreach (Match m in QueryPair.Matches(input))
                query.TryAdd(Uri.UnescapeDataString(m.Groups[1].Value), Uri.UnescapeDataString(m.Groups[2].Value));

            // A ro.blox.com link from the Roblox app wraps the real link in af_web_dp (or af_dp); read that one.
            if (depth < 2 && (query.TryGetValue("af_web_dp", out var wrapped) || query.TryGetValue("af_dp", out wrapped))
                && wrapped.Contains("roblox", StringComparison.OrdinalIgnoreCase))
                return TryParse(wrapped, jobInput, out target, out error, depth + 1);

            if (GamePath.Match(input) is { Success: true } path)
                long.TryParse(path.Groups[1].Value, out placeId);
            else if (query.TryGetValue("placeId", out var p))
                long.TryParse(p, out placeId);

            bool share = input.Contains("roblox.com/share", StringComparison.OrdinalIgnoreCase)
                         || input.Contains("share_links", StringComparison.OrdinalIgnoreCase);
            if (share && query.TryGetValue("code", out var code))
            {
                if (query.TryGetValue("type", out var type) && !type.Equals("Server", StringComparison.OrdinalIgnoreCase))
                {
                    error = L.T("Only private-server share links (type=Server) are supported. For a regular game, paste a roblox.com/games/… link.");
                    return false;
                }
                shareCode = code;
            }

            if (query.TryGetValue("privateServerLinkCode", out var lc) || query.TryGetValue("linkCode", out lc))
                linkCode = lc;
            if (query.TryGetValue("gameInstanceId", out var gi) || query.TryGetValue("jobId", out gi))
                jobId = gi;
            if (query.TryGetValue("accessCode", out var ac) && ac.Trim().Length > 0)
            {
                if (!Guid.TryParse(ac.Trim(), out var access))
                {
                    error = L.T("Invalid private server access code.");
                    return false;
                }
                accessCode = access.ToString("D");
            }

            if (placeId <= 0 && shareCode is null)
            {
                error = L.T("Couldn't find a Place ID in this link.");
                return false;
            }
        }
        else if (placeId <= 0)
        {
            error = L.T("Invalid Place ID.");
            return false;
        }

        if (job.Length > 0)
            jobId = job;
        if (!string.IsNullOrEmpty(jobId))
        {
            if (!Guid.TryParse(jobId, out var guid))
            {
                error = L.T("Invalid Job ID (expected xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx).");
                return false;
            }
            jobId = guid.ToString("D"); // Guid also accepts {…} and undashed forms, which Roblox and the log comparison don't
        }

        target = new GameTarget(placeId, NullIfEmpty(jobId), NullIfEmpty(linkCode), shareCode) { AccessCode = accessCode };
        return true;
    }

    static string? NullIfEmpty(string? s) => string.IsNullOrWhiteSpace(s) ? null : s.Trim();
}
