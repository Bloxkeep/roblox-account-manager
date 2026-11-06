using System.Text.Json;

namespace RobloxManager;

/// <summary>A game in the Games tab: a price list (kind "prices"), a wiki's items ("wiki") or only the game ("game": the app finds its wiki), how many, when the bot read them (ms),
/// and its place on Roblox (0 when the bot names none), for its picture and colour (<see cref="WikiLook"/>).</summary>
/// Wiki: the wiki the bot names for it (Fandom's search would find another first), null to find it by name.
public sealed record WikiGameInfo(string Id, string Name, string Kind, int Count, long? At, long Place, string? Wiki = null);
/// <summary>Where a game's items come from: the wiki's (or website's) name and address, and the licence of its words.</summary>
public sealed record WikiSource(string Name, string Url);
/// <summary>
/// A wiki page as an item (services/DiscordBot/worker/wiki.js): its name, category, the lines of its box of facts in
/// plain words ([name, value]), its first lines, its picture (on Fandom's picture host) and its page.
/// </summary>
public sealed record WikiItem(string Id, string Name, string Cat, List<string[]> Fields, string Lead, string? Img, string? Url);
/// <summary>One game of the Wiki tab as the bot gave it: a wiki's items, or (Prices) a price list.</summary>
public sealed record WikiGame(string Id, string Name, string Kind, long? At, WikiSource Source, WikiSource? License, List<string> Cats,
    List<WikiItem> Items, List<FeedPriceItem> Prices);

/// <summary>Where a game's prices come from: the website's name and address (shown and linked under the list).</summary>
public sealed record FeedPriceSource(string Name, string Url);
/// <summary>
/// An item in a game's price list (services/DiscordBot/worker/prices.js): its name, kind, picture (through Bloxkeep's
/// bot), value and permanent value, demand of 10 and trend, prices in Beli and Robux, and its page on the website.
/// </summary>
public sealed record FeedPriceItem(string Id, string Name, string? Category, string? Rarity, string? Type, string? Img, double? Value, double? Perm,
    double? Demand, double? PermDemand, string? Trend, string? PermTrend, double? Beli, double? Robux, string? BestFor, long? Updated, string? Url);

/// <summary>
/// The Wiki tab's data from Bloxkeep's Discord bot (/wiki: the games; /wiki/&lt;game&gt;: one game), which reads the games'
/// wikis and price lists once a day: the app never calls a wiki itself, and a game the bot adds shows here without an
/// update. What comes is checked: pictures only from Fandom's picture host (a price list's only through the bot), links
/// only to the wiki or website it names. Kept for half an hour.
/// </summary>
static class WikiLink
{
    public const string Base = "https://bloxkeep-bot.bloxkeep-app.workers.dev";
    const int MaxItems = 2000, MaxFields = 30, MaxText = 1000;
    static readonly TimeSpan Keep = TimeSpan.FromMinutes(30);
    static readonly HttpClient Http = new(new SocketsHttpHandler { UseCookies = false, AllowAutoRedirect = false })
    {
        Timeout = TimeSpan.FromSeconds(30),
    };
    static readonly JsonSerializerOptions Json = new() { PropertyNameCaseInsensitive = true };
    static readonly Dictionary<string, (DateTime At, object Value)> Kept = [];

    /// <summary>The bot's address: the one game news uses in a test (BLOXKEEP_FEEDS_URL, ws:// as http://), else Bloxkeep's.</summary>
    public static Uri BaseOf(string? feedsUrl) => feedsUrl is { Length: > 0 } && Uri.TryCreate(feedsUrl, UriKind.Absolute, out var u)
        ? new Uri($"{(u.Scheme == "ws" ? "http" : "https")}://{u.Authority}") : new Uri(Base);

    public static async Task<List<WikiGameInfo>> GamesAsync(Uri bot)
    {
        if (Fresh("") is List<WikiGameInfo> kept)
            return kept;
        using var doc = await GetAsync(new Uri(bot, "/wiki"));
        var games = new List<WikiGameInfo>();
        if (doc.RootElement.TryGetProperty("games", out var list) && list.ValueKind == JsonValueKind.Array)
            foreach (var g in list.EnumerateArray().Take(100))
                if (Str(g, "id") is { Length: > 0 and <= 40 } id && Str(g, "name") is { Length: > 0 } name && Str(g, "kind") is "wiki" or "prices" or "game")
                    games.Add(new WikiGameInfo(id, Cut(name, 60), Str(g, "kind")!, Int(g, "count"), Long(g, "at"), Math.Max(0, Long(g, "place") ?? 0), WikiNet.Site(Str(g, "wiki"))));
        Kept[""] = (DateTime.UtcNow, games);
        return games;
    }

    public static async Task<WikiGame?> GameAsync(Uri bot, string id)
    {
        if (!System.Text.RegularExpressions.Regex.IsMatch(id, "^[a-z0-9-]{1,40}$"))
            return null;
        if (Fresh(id) is WikiGame kept)
            return kept;
        using var doc = await GetAsync(new Uri(bot, $"/wiki/{id}"));
        var root = doc.RootElement;
        string kind = Str(root, "kind") ?? "";
        if (kind is not ("wiki" or "prices") || !root.TryGetProperty("source", out var src) || Str(src, "url") is not { } siteUrl
            || !Uri.TryCreate(siteUrl, UriKind.Absolute, out var site) || site.Scheme != Uri.UriSchemeHttps)
            return null;
        bool OnSite(string? url) => Uri.TryCreate(url, UriKind.Absolute, out var u) && u.Scheme == Uri.UriSchemeHttps && u.Host == site.Host;
        var source = new WikiSource(Cut(Str(src, "name") ?? site.Host, 80), site.AbsoluteUri.TrimEnd('/'));
        var game = new WikiGame(id, Cut(Str(root, "name") ?? id, 60), kind, Long(root, "at"), source, null, [], [], []);
        if (kind == "prices")
        {
            bool FromBot(string? img) => Uri.TryCreate(img, UriKind.Absolute, out var u) && u.Host == bot.Host && u.Port == bot.Port
                && (u.Scheme == Uri.UriSchemeHttps || (bot.Scheme == "http" && u.Scheme == Uri.UriSchemeHttp)) && u.AbsolutePath == "/prices/img";
            var items = root.TryGetProperty("items", out var arr) && arr.ValueKind == JsonValueKind.Array
                ? arr.Deserialize<List<FeedPriceItem>>(Json) ?? [] : [];
            game = game with
            {
                Prices = items.Where(i => i is { Id.Length: > 0, Name.Length: > 0 }).Take(MaxItems)
                    .Select(i => i with { Img = FromBot(i.Img) ? i.Img : null, Url = OnSite(i.Url) ? i.Url : null }).ToList(),
            };
        }
        else
        {
            var items = new List<WikiItem>();
            if (root.TryGetProperty("items", out var arr) && arr.ValueKind == JsonValueKind.Array)
                foreach (var i in arr.EnumerateArray().Take(MaxItems))
                {
                    if (Str(i, "id") is not { Length: > 0 } itemId || Str(i, "name") is not { Length: > 0 } name)
                        continue;
                    var fields = new List<string[]>();
                    if (i.TryGetProperty("fields", out var fs) && fs.ValueKind == JsonValueKind.Array)
                        foreach (var f in fs.EnumerateArray().Take(MaxFields))
                            if (f.ValueKind == JsonValueKind.Array && f.GetArrayLength() == 2 && f[0].ValueKind == JsonValueKind.String && f[1].ValueKind == JsonValueKind.String)
                                fields.Add([Cut(f[0].GetString()!, 60), Cut(f[1].GetString()!, MaxText)]);
                    string? img = Str(i, "img") is { } p && Uri.TryCreate(p, UriKind.Absolute, out var pu) && pu.Scheme == Uri.UriSchemeHttps
                        && pu.Host == "static.wikia.nocookie.net" ? p : null;
                    items.Add(new WikiItem(Cut(itemId, 60), Cut(name, 100), Cut(Str(i, "cat") ?? "", 40), fields, Cut(Str(i, "lead") ?? "", MaxText), img,
                        OnSite(Str(i, "url")) ? Str(i, "url") : null));
                }
            var cats = root.TryGetProperty("cats", out var cs) && cs.ValueKind == JsonValueKind.Array
                ? cs.EnumerateArray().Where(c => c.ValueKind == JsonValueKind.String).Select(c => Cut(c.GetString()!, 40)).Take(40).ToList() : [];
            WikiSource? license = root.TryGetProperty("license", out var lic) && Str(lic, "name") is { Length: > 0 } ln
                && Str(lic, "url") is { } lu && lu.StartsWith("https://creativecommons.org/", StringComparison.Ordinal) ? new WikiSource(Cut(ln, 40), lu) : null;
            game = game with { Items = items, Cats = cats, License = license };
        }
        Kept[id] = (DateTime.UtcNow, game);
        return game;
    }

    /// <summary>Whether a link is one of a game the tab has shown: its wiki or website, or one of its items' pages.</summary>
    public static bool Knows(string url) => Kept.Values.Select(k => k.Value).OfType<WikiGame>().Any(g => g.Source.Url == url || g.License?.Url == url
        || g.Items.Any(i => i.Url == url) || g.Prices.Any(i => i.Url == url));

    static object? Fresh(string key) => Kept.TryGetValue(key, out var k) && DateTime.UtcNow - k.At < Keep ? k.Value : null;

    static async Task<JsonDocument> GetAsync(Uri url)
    {
        using var res = await Http.GetAsync(url, HttpCompletionOption.ResponseHeadersRead);
        if (!res.IsSuccessStatusCode)
            throw new UserError(L.T("The wiki can't be reached right now. Try again in a moment."));
        await using var body = await res.Content.ReadAsStreamAsync();
        return await JsonDocument.ParseAsync(body);
    }

    static string? Str(JsonElement e, string name) => e.ValueKind == JsonValueKind.Object && e.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.String ? v.GetString() : null;
    static int Int(JsonElement e, string name) => e.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.Number && v.TryGetInt32(out int n) ? n : 0;
    static long? Long(JsonElement e, string name) => e.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.Number && v.TryGetInt64(out long n) ? n : null;
    static string Cut(string s, int n) => s.Length <= n ? s : s[..n];
}
