using System.Net;
using System.Text.Json;
using System.Text.RegularExpressions;

namespace RobloxManager;

/// <summary>
/// What Bloxkeep's Roblox watch (RobloxWatch/watch.py in the repository, docs/roblox-watch.md) found in each Roblox
/// build: whether the single-window lock Multi-Roblox relies on changed since the build before, and which lock names to
/// hold for it when Roblox renamed that lock. Read from the repository's roblox-status branch. Only the verdicts and
/// names shaped like Roblox's own lock objects are taken from it; anything else in it is ignored.
/// </summary>
public static class RobloxWatch
{
    const string StatusUrl = "https://raw.githubusercontent.com/minhhoang0122/bloxkeep/roblox-status/multi-status.json";
    public const string DocsUrl = "https://github.com/minhhoang0122/bloxkeep/blob/main/docs/roblox-watch.md";
    /// <summary>At most this many names are held for a build (Roblox has used one or two).</summary>
    const int MaxHold = 4;
    static readonly Regex LockName = new(@"^ROBLOX_[A-Za-z0-9_]{1,64}$", RegexOptions.CultureInvariant);
    static readonly HashSet<string> Verdicts = ["first", "same", "changed", "renamed", "gone"];
    static readonly HttpClient Http = new(new SocketsHttpHandler
    {
        UseCookies = false,
        AutomaticDecompression = DecompressionMethods.All,
        PooledConnectionLifetime = TimeSpan.FromMinutes(5),
    })
    {
        Timeout = TimeSpan.FromSeconds(15),
    };

    /// <param name="Hash">The build's folder name ("version-…"), as under Roblox's Versions folder.</param>
    /// <param name="Verdict">first, same, changed, renamed or gone.</param>
    /// <param name="Hold">The lock names to hold for this build; null when the watch didn't say (its own name then).</param>
    public sealed record Build(string Version, string Hash, string Verdict, string[]? Hold);

    /// <param name="Live">The build Roblox serves now, when the watch has checked it.</param>
    /// <param name="Next">The build Roblox offers before release (its beta channel), when newer than the live one.</param>
    public sealed record Status(Build? Live, Build? Next, IReadOnlyDictionary<string, Build> ByHash);

    public static Status? Current { get; private set; }

    /// <summary>When the status was last read; null until it was (offline, or GitHub couldn't be reached).</summary>
    public static DateTime? ReadAt { get; private set; }

    static string? _lastText;

    /// <returns>true when what the watch says changed since the last read.</returns>
    public static async Task<bool> RefreshAsync(CancellationToken ct = default)
    {
        using var req = new HttpRequestMessage(HttpMethod.Get, StatusUrl);
        req.Headers.UserAgent.ParseAdd($"Bloxkeep/{Application.ProductVersion.Split('+')[0]}");
        using var res = await Http.SendAsync(req, ct);
        if (!res.IsSuccessStatusCode)
            return false;
        string text = await res.Content.ReadAsStringAsync(ct);
        if (Parse(text) is not { } status)
            return false;
        ReadAt = DateTime.UtcNow;
        if (text == _lastText)
            return false;
        _lastText = text;
        Current = status;
        return true;
    }

    /// <summary>The status file's findings, or null when the text isn't one. Entries out of shape are left out.</summary>
    public static Status? Parse(string json)
    {
        try
        {
            using var doc = JsonDocument.Parse(json);
            var root = doc.RootElement;
            if (root.ValueKind != JsonValueKind.Object || !root.TryGetProperty("builds", out var builds) || builds.ValueKind != JsonValueKind.Array)
                return null;
            var byHash = new Dictionary<string, Build>(StringComparer.OrdinalIgnoreCase);
            foreach (var b in builds.EnumerateArray())
            {
                if (b.ValueKind != JsonValueKind.Object || Text(b, "version") is not { Length: > 0 and <= 40 } version
                    || Text(b, "hash") is not { Length: > 0 and <= 80 } hash || Text(b, "verdict") is not { } verdict || !Verdicts.Contains(verdict))
                    continue;
                string[]? hold = null;
                if (b.TryGetProperty("hold", out var h) && h.ValueKind == JsonValueKind.Array)
                {
                    hold = h.EnumerateArray().Where(n => n.ValueKind == JsonValueKind.String).Select(n => n.GetString()!)
                        .Where(n => LockName.IsMatch(n)).Distinct(StringComparer.Ordinal).Take(MaxHold).ToArray();
                }
                byHash[hash] = new Build(version, hash, verdict, hold);
            }
            Build? Named(string property) =>
                root.TryGetProperty(property, out var p) && p.ValueKind == JsonValueKind.Object && Text(p, "hash") is { } hash
                    ? byHash.GetValueOrDefault(hash) : null;
            var live = Named("live");
            var next = Named("beta");
            if (next is not null && (live is null || !Newer(next.Version, live.Version)))
                next = null;
            return new Status(live, next, byHash);
        }
        catch (JsonException)
        {
            return null;
        }
    }

    /// <summary>What the watch says about a build (its folder name); null when it hasn't checked it.</summary>
    public static Build? For(string? hash) =>
        hash is { Length: > 0 } && Current?.ByHash.TryGetValue(hash, out var build) == true ? build : null;

    /// <summary>
    /// Lock names to hold besides Multi-Roblox's own: those of the installed build and of the live one (a launcher
    /// updates to it before starting Roblox), when the watch found Roblox renamed its lock.
    /// </summary>
    public static string[] ExtraNames(string? installedHash, string ownName) =>
        new[] { For(installedHash), Current?.Live }
            .SelectMany(b => b?.Hold ?? [])
            .Where(n => n != ownName)
            .Distinct(StringComparer.Ordinal)
            .Take(MaxHold)
            .ToArray();

    static bool Newer(string a, string b) =>
        Version.TryParse(a, out var x) && Version.TryParse(b, out var y) ? x > y : string.CompareOrdinal(a, b) > 0;

    static string? Text(JsonElement e, string name) =>
        e.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.String ? v.GetString() : null;
}
