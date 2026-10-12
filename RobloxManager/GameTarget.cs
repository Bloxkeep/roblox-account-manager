using System.Text.RegularExpressions;

namespace RobloxManager;

/// <summary>Where to send an account: a place, optionally a specific server (job) or a private server.</summary>
public sealed record GameTarget(long PlaceId, string? JobId = null, string? LinkCode = null, string? ShareCode = null)
{
    static readonly Regex GamePath = new(@"/games/(\d+)", RegexOptions.IgnoreCase | RegexOptions.CultureInvariant);
    static readonly Regex QueryPair = new(@"[?&]([^=&#\s]+)=([^&#\s]*)", RegexOptions.CultureInvariant);

    /// <summary>
    /// Accepts a place id, a roblox.com/games/… link (with privateServerLinkCode or gameInstanceId),
    /// a roblox.com/share?code=…&amp;type=Server link, or a roblox:// deep link with placeId.
    /// </summary>
    public static bool TryParse(string placeInput, string jobInput, out GameTarget? target, out string error)
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
        string? linkCode = null, shareCode = null, jobId = null;
        if (!long.TryParse(input, out placeId))
        {
            var query = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
            foreach (Match m in QueryPair.Matches(input))
                query.TryAdd(Uri.UnescapeDataString(m.Groups[1].Value), Uri.UnescapeDataString(m.Groups[2].Value));

            if (GamePath.Match(input) is { Success: true } path)
                long.TryParse(path.Groups[1].Value, out placeId);
            else if (query.TryGetValue("placeId", out var p))
                long.TryParse(p, out placeId);

            if (input.Contains("roblox.com/share", StringComparison.OrdinalIgnoreCase) && query.TryGetValue("code", out var code))
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
        if (!string.IsNullOrEmpty(jobId) && !Guid.TryParse(jobId, out _))
        {
            error = L.T("Invalid Job ID (expected xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx).");
            return false;
        }

        target = new GameTarget(placeId, NullIfEmpty(jobId), NullIfEmpty(linkCode), shareCode);
        return true;
    }

    static string? NullIfEmpty(string? s) => string.IsNullOrWhiteSpace(s) ? null : s.Trim();
}
