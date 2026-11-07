using System.Net;
using System.Text.Json;
using System.Text.RegularExpressions;

namespace RobloxManager;

/// <summary>
/// A game's fan wiki read straight from this PC, as a browser would read it: any wiki on Fandom or wiki.gg (both are
/// MediaWiki, so one API reads all of them, and a new game's wiki needs nothing here). Only reading: the page asks for
/// a query or a parsed page (WikiSite.tsx turns the answer into the Wiki part), a list of wikis for a game's name, or
/// what wiki a pasted link is. Nothing else is sent, and only to those hosts. Answers are kept a while, so going back
/// to a page asks nothing again.
/// </summary>
static partial class WikiNet
{
    static readonly HttpClient Http = MakeClient();
    static readonly Dictionary<string, (DateTime At, JsonElement Value)> Cache = new(StringComparer.Ordinal);
    static readonly TimeSpan Keep = TimeSpan.FromMinutes(20);
    const int CacheMax = 120;

    static HttpClient MakeClient()
    {
        var http = new HttpClient(new SocketsHttpHandler { UseCookies = false, AllowAutoRedirect = false, AutomaticDecompression = DecompressionMethods.All })
            { Timeout = TimeSpan.FromSeconds(25) };
        http.DefaultRequestHeaders.UserAgent.ParseAdd($"Bloxkeep/{Application.ProductVersion.Split('+')[0]} (+https://bloxkeep.github.io/roblox-account-manager)");
        http.DefaultRequestHeaders.Accept.ParseAdd("application/json");
        return http;
    }

    /// <summary>A Fandom or wiki.gg host: lower-case letters, digits, dots and dashes under one of the two.</summary>
    [GeneratedRegex(@"^[a-z0-9]([a-z0-9-]*[a-z0-9])?(\.[a-z0-9]([a-z0-9-]*[a-z0-9])?)*\.(fandom\.com|wiki\.gg)$")]
    private static partial Regex HostRule();
    /// <summary>A Fandom wiki in another language lives under its code ("/vi", "/pt-br").</summary>
    [GeneratedRegex(@"^/[a-z]{2,3}(-[a-z]{2,4})?$")]
    private static partial Regex LangRule();

    /// <summary>The wiki a site address names ("https://x.fandom.com" or "https://x.fandom.com/vi"), or null.</summary>
    public static string? Site(string? site)
    {
        if (!Uri.TryCreate(site?.Trim().TrimEnd('/'), UriKind.Absolute, out var u) || u.Scheme != Uri.UriSchemeHttps || !u.IsDefaultPort
            || u.UserInfo.Length > 0 || u.Query.Length > 0 || u.Fragment.Length > 0)
            return null;
        string host = u.Host.ToLowerInvariant(), path = u.AbsolutePath.TrimEnd('/').ToLowerInvariant();
        if (!HostRule().IsMatch(host) || host.StartsWith("services.", StringComparison.Ordinal) || host.StartsWith("community.", StringComparison.Ordinal))
            return null;
        if (path.Length > 0 && !LangRule().IsMatch(path))
            return null;
        return $"https://{host}{path}";
    }

    /// <summary>The wiki a pasted link points into ("deepwoken.fandom.com/wiki/Shotel", "https://x.wiki.gg/"), or null.</summary>
    public static string? SiteOfLink(string link)
    {
        string s = link.Trim();
        if (!s.Contains("://", StringComparison.Ordinal))
            s = "https://" + s;
        if (!Uri.TryCreate(s, UriKind.Absolute, out var u) || (u.Scheme != Uri.UriSchemeHttps && u.Scheme != Uri.UriSchemeHttp))
            return null;
        var lang = Regex.Match(u.AbsolutePath.ToLowerInvariant(), @"^(/[a-z]{2,3}(-[a-z]{2,4})?)/(wiki/|$)");
        return Site($"https://{u.Host}{(lang.Success ? lang.Groups[1].Value : "")}");
    }

    // expandtemplates: a wiki's own short codes in words ({{Rarity|L}} → Legendary), for the tables (WikiSite.tsx).
    static readonly HashSet<string> Actions = ["query", "parse", "expandtemplates"];
    /// <summary>Settings a request may carry: what is asked of MediaWiki, never how it answers or who asks.</summary>
    static readonly HashSet<string> Never = ["format", "callback", "origin", "token", "centralauthtoken", "assert", "assertuser", "requestid", "servedby", "curtimestamp", "maxlag", "uselang", "errorformat"];

    /// <summary>One read of a wiki's API (action query, parse or expandtemplates), as JSON. Throws <see cref="UserError"/> when it can't be read.</summary>
    public static async Task<JsonElement> ApiAsync(string site, IReadOnlyDictionary<string, string> q, CancellationToken ct = default)
    {
        string wiki = Site(site) ?? throw new UserError(L.T("That isn't a wiki Bloxkeep can read."));
        if (!q.TryGetValue("action", out var action) || !Actions.Contains(action))
            throw new UserError(L.T("That isn't a wiki Bloxkeep can read."));
        var pairs = q.Where(p => !Never.Contains(p.Key) && p.Key.Length <= 40 && p.Value.Length <= 2000)
            .OrderBy(p => p.Key, StringComparer.Ordinal)
            .Select(p => $"{Uri.EscapeDataString(p.Key)}={Uri.EscapeDataString(p.Value)}");
        string url = $"{wiki}/api.php?{string.Join('&', pairs)}&format=json&formatversion=2";
        return await GetJsonAsync(url, ct);
    }

    /// <summary>Fandom's own list of wikis for a game's name, best first: a wiki named after the game, then the most read.</summary>
    public static async Task<List<WikiFound>> FindAsync(string name, CancellationToken ct = default)
    {
        string words = name.Trim();
        if (words.Length is 0 or > 80)
            return [];
        string url = $"https://services.fandom.com/unified-search/community-search?query={Uri.EscapeDataString(words)}&lang=en&limit=25";
        var d = await GetJsonAsync(url, ct);
        var found = new List<WikiFound>();
        if (!d.TryGetProperty("results", out var results) || results.ValueKind != JsonValueKind.Array)
            return found;
        string want = Fold(words);
        foreach (var r in results.EnumerateArray())
        {
            string? site = Site(Text(r, "url")), title = Text(r, "name");
            if (site is null || string.IsNullOrWhiteSpace(title) || Text(r, "language") is { } lang && lang != "en")
                continue;
            long views = Number(r, "pageViews"), pages = Number(r, "pageCount");
            string bare = Fold(Regex.Replace(title, @"\s*wiki\s*$", "", RegexOptions.IgnoreCase));
            // Named after the game outright, then with its name in it; a fan-made spin-off ("… Fanon", "… Ideas") last.
            int match = bare == want ? 3 : bare.StartsWith(want, StringComparison.Ordinal) || want.StartsWith(bare, StringComparison.Ordinal) ? 2
                : bare.Contains(want, StringComparison.Ordinal) ? 1 : 0;
            if (Regex.IsMatch(title, @"\b(fanon|ideas?|fan ?made|rp|roleplay|but)\b", RegexOptions.IgnoreCase))
                match = Math.Min(match, 0);
            string? img = Text(r, "thumbnail") is { } t && t.StartsWith("https://static.wikia.nocookie.net/", StringComparison.Ordinal) ? t : null;
            found.Add(new WikiFound(site, title.Trim(), pages, views, img, match));
        }
        return found.OrderByDescending(f => f.Match).ThenByDescending(f => f.Views).Take(8).ToList();
    }

    /// <summary>What wiki a site is: its name, articles, main page and logo; null when it isn't one.</summary>
    public static async Task<WikiAbout?> AboutAsync(string site, CancellationToken ct = default)
    {
        if (Site(site) is not { } wiki)
            return null;
        try
        {
            var d = await ApiAsync(wiki, new Dictionary<string, string> { ["action"] = "query", ["meta"] = "siteinfo", ["siprop"] = "general|statistics" }, ct);
            if (!d.TryGetProperty("query", out var q) || !q.TryGetProperty("general", out var g))
                return null;
            // A wiki that moved answers from its new address: that one is kept.
            string server = Site(Text(g, "server") is { } s ? (s.StartsWith("//", StringComparison.Ordinal) ? "https:" + s : s) + LangPath(wiki) : wiki) ?? wiki;
            long articles = q.TryGetProperty("statistics", out var st) ? Number(st, "articles") : 0;
            string? logo = Text(g, "logo") is { } l && Uri.TryCreate(l.StartsWith("//", StringComparison.Ordinal) ? "https:" + l : l, UriKind.Absolute, out var lu)
                && lu.Scheme == Uri.UriSchemeHttps && (lu.Host == "static.wikia.nocookie.net" || HostRule().IsMatch(lu.Host)) ? lu.AbsoluteUri : null;
            return new WikiAbout(server, Text(g, "sitename") ?? new Uri(server).Host, articles, Text(g, "mainpage") ?? "Main Page", logo);
        }
        catch (UserError)
        {
            return null;
        }
    }

    static string LangPath(string wiki) => new Uri(wiki).AbsolutePath.TrimEnd('/');

    /// <summary>A GET of JSON, kept a while; a move to another wiki host (a renamed wiki) is followed, nowhere else.</summary>
    static async Task<JsonElement> GetJsonAsync(string url, CancellationToken ct)
    {
        lock (Cache)
            if (Cache.TryGetValue(url, out var hit) && DateTime.UtcNow - hit.At < Keep)
                return hit.Value;
        string at = url;
        for (int hop = 0; ; hop++)
        {
            HttpResponseMessage res;
            try
            {
                res = await Http.GetAsync(at, HttpCompletionOption.ResponseHeadersRead, ct);
            }
            catch (Exception e) when (e is HttpRequestException or TaskCanceledException)
            {
                if (!ct.IsCancellationRequested)
                    Log.Note($"Wiki: request failed ({AppController.LogKindOf(e)}).");
                throw new UserError(L.T("The wiki can't be reached right now."));
            }
            using (res)
            {
                if ((int)res.StatusCode is >= 300 and < 400 && res.Headers.Location is { } next && hop < 3)
                {
                    var to = next.IsAbsoluteUri ? next : new Uri(new Uri(at), next);
                    bool ours = to.Scheme == Uri.UriSchemeHttps && (HostRule().IsMatch(to.Host) || to.Host == "services.fandom.com");
                    if (!ours)
                    {
                        Log.Note("Wiki: a redirect to a site that isn't a wiki was refused.");
                        throw new UserError(L.T("That isn't a wiki Bloxkeep can read."));
                    }
                    at = to.AbsoluteUri;
                    continue;
                }
                if (!res.IsSuccessStatusCode)
                    Log.Note($"Wiki: HTTP {(int)res.StatusCode}.");
                if (!res.IsSuccessStatusCode)
                    throw new UserError(res.StatusCode is HttpStatusCode.NotFound or HttpStatusCode.Unauthorized or HttpStatusCode.Gone
                        ? L.T("That isn't a wiki Bloxkeep can read.") : L.T("The wiki can't be reached right now."));
                if (res.Content.Headers.ContentLength > 12_000_000)
                    Log.Note($"Wiki: an answer of {res.Content.Headers.ContentLength / 1_000_000} MB was too big to read.");
                if (res.Content.Headers.ContentLength > 12_000_000)
                    throw new UserError(L.T("That wiki page is too big to show here."));
                JsonElement value;
                try
                {
                    await using var body = await res.Content.ReadAsStreamAsync(ct);
                    using var doc = await JsonDocument.ParseAsync(body, cancellationToken: ct);
                    value = doc.RootElement.Clone();
                }
                catch (JsonException)
                {
                    Log.Note("Wiki: the answer wasn't JSON.");
                    throw new UserError(L.T("That isn't a wiki Bloxkeep can read."));
                }
                catch (Exception e) when (e is HttpRequestException or TaskCanceledException or IOException)
                {
                    if (!ct.IsCancellationRequested)
                        Log.Note($"Wiki: reading the answer failed ({AppController.LogKindOf(e)}).");
                    throw new UserError(L.T("The wiki can't be reached right now."));
                }
                lock (Cache)
                {
                    if (Cache.Count >= CacheMax)
                        foreach (var old in Cache.OrderBy(c => c.Value.At).Take(CacheMax / 4).Select(c => c.Key).ToList())
                            Cache.Remove(old);
                    Cache[url] = (DateTime.UtcNow, value);
                }
                return value;
            }
        }
    }

    static string? Text(JsonElement e, string name) => e.ValueKind == JsonValueKind.Object && e.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.String ? v.GetString() : null;
    static long Number(JsonElement e, string name) => e.ValueKind == JsonValueKind.Object && e.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.Number && v.TryGetInt64(out long n) ? n : 0;
    static string Fold(string s) => Regex.Replace(s.ToLowerInvariant(), @"[^a-z0-9]+", " ").Trim();

    /// <summary>A link the Wiki part may open in the browser: https, anywhere (a wiki's own links go out to the game's
    /// Roblox page, its Discord, videos…), as the wiki in a browser would.</summary>
    public static bool Openable(string url) =>
        Uri.TryCreate(url, UriKind.Absolute, out var u) && u.Scheme == Uri.UriSchemeHttps && u.UserInfo.Length == 0 && url.Length <= 2000;
}

public sealed record WikiFound(string Site, string Name, long Pages, long Views, string? Img, int Match);
public sealed record WikiAbout(string Site, string Name, long Articles, string MainPage, string? Logo);
