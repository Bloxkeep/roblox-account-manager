using System.Text.Json;

namespace RobloxManager;

/// <summary>
/// The Wiki tab: each game's items (weapons, gear, bosses… from the game's fan wiki) and price lists, which Bloxkeep's
/// Discord bot reads once a day (<see cref="WikiLink"/>, services/DiscordBot/worker/wiki.js and prices.js). The page asks
/// for the games when the tab opens and for a game's items when it is picked; links open only to what the tab showed.
/// Only an installed Bloxkeep asks (as game news does), or one pointed at a server of its own (BLOXKEEP_FEEDS_URL).
/// </summary>
public sealed partial class AppController
{
    Uri WikiBot => WikiLink.BaseOf(FeedsTestUrl);

    async Task<object?> WikiGamesAsync()
    {
        if (!FeedsAllowed)
            return new { games = Array.Empty<object>(), off = true };
        var games = await WikiLink.GamesAsync(WikiBot);
        return new { games = games.Select(g => new { id = g.Id, name = g.Name, kind = g.Kind, count = g.Count, at = g.At, place = g.Place, wiki = g.Wiki }) };
    }

    /// <summary>
    /// The games this PC's accounts played in the last 90 days (Statistics' stays), most played first, as cards of the Games
    /// tab with their wiki found by name: {name, place (the game's start place when known), hours, last (ms)}. None while locked.
    /// </summary>
    object PlayedGames()
    {
        if (Open is not { } vault)
            return Array.Empty<object>();
        var now = DateTime.UtcNow;
        var since = now - PlayHistory;
        var universeOf = vault.Data.PlaySessions.Where(s => s.UniverseId > 0).GroupBy(s => s.PlaceId).ToDictionary(g => g.Key, g => g.Last().UniverseId);
        long KeyOf(PlaySession s) => s.UniverseId > 0 ? s.UniverseId
            : universeOf.GetValueOrDefault(s.PlaceId) is > 0 and var u ? u : _games.GetValueOrDefault(s.PlaceId)?.UniverseId is > 0 and var c ? c : -s.PlaceId;
        return vault.Data.PlaySessions
            .Where(s => s.PlaceId > 0 && (s.EndedAt ?? s.LastSeenAt ?? now) > since)
            .GroupBy(KeyOf)
            .Select(g =>
            {
                var last = g.MaxBy(s => s.StartedAt)!;
                string name = g.Where(s => s.GameName.Length > 0).MaxBy(s => s.StartedAt)?.GameName ?? _games.GetValueOrDefault(last.PlaceId)?.Name ?? "";
                long place = g.Select(s => _games.GetValueOrDefault(s.PlaceId)?.PlaceId ?? 0).FirstOrDefault(p => p > 0) is > 0 and var root ? root : last.PlaceId;
                double hours = g.Sum(s => Math.Max(0, ((s.EndedAt ?? s.LastSeenAt ?? now) - (s.StartedAt > since ? s.StartedAt : since)).TotalHours));
                return (name, place, hours, last: last.StartedAt);
            })
            .Where(g => g.name.Length > 0)
            .OrderByDescending(g => g.hours)
            .Take(30)
            .Select(g => new { g.name, g.place, hours = Math.Round(g.hours, 1), last = new DateTimeOffset(DateTime.SpecifyKind(g.last, DateTimeKind.Utc)).ToUnixTimeMilliseconds() })
            .ToList();
    }

    /// <summary>How the Games tab draws each game (its picture, icon and colour), by Roblox place.</summary>
    async Task<object?> GameLooksAsync(JsonElement a)
    {
        var places = a.TryGetProperty("places", out var arr) && arr.ValueKind == JsonValueKind.Array
            ? arr.EnumerateArray().Select(x => x.TryGetInt64(out long v) ? v : 0).Where(v => v > 0).Distinct().Take(100).ToList()
            : [];
        var looks = await WikiLook.OfAsync(places);
        return looks.ToDictionary(l => l.Key.ToString(), l => new { icon = l.Value.Icon, cover = l.Value.Cover, tint = l.Value.Tint, ink = l.Value.Ink });
    }

    async Task<object?> WikiGameAsync(string id)
    {
        if (!FeedsAllowed || await WikiLink.GameAsync(WikiBot, id) is not { } g)
        {
            Log.Note($"Wiki: a game's items not shown ({(FeedsAllowed ? "the bot had nothing for it" : "off for this copy")}).");
            return null;
        }
        object source = new { name = g.Source.Name, url = g.Source.Url };
        if (g.Kind == "prices")
            return new
            {
                id = g.Id, name = g.Name, kind = g.Kind, at = g.At, source,
                items = g.Prices.Select(i => new
                {
                    id = i.Id, name = i.Name, category = i.Category, rarity = i.Rarity, type = i.Type, img = i.Img, value = i.Value, perm = i.Perm,
                    demand = i.Demand, permDemand = i.PermDemand, trend = i.Trend, permTrend = i.PermTrend, beli = i.Beli, robux = i.Robux, bestFor = i.BestFor,
                    updated = i.Updated, url = i.Url,
                }),
            };
        return new
        {
            id = g.Id, name = g.Name, kind = g.Kind, at = g.At, source, cats = g.Cats,
            license = g.License is { } l ? new { name = l.Name, url = l.Url } : null,
            items = g.Items.Select(i => new { id = i.Id, name = i.Name, cat = i.Cat, fields = i.Fields, lead = i.Lead, img = i.Img, url = i.Url }),
        };
    }

    /// <summary>A link from the Wiki part, in the browser: one the bot's list showed, or any https link of a wiki page.</summary>
    object? OpenWikiLink(string url)
    {
        if (!Uri.TryCreate(url, UriKind.Absolute, out var uri) || uri.Scheme != Uri.UriSchemeHttps || !(WikiLink.Knows(url) || WikiNet.Openable(url)))
            throw new UserError(L.T("That link isn't in the wiki any more."));
        return OpenUrl(uri.AbsoluteUri);
    }

    // ---- A game's whole wiki, read from this PC (WikiNet.cs, ui/src/WikiSite.tsx) ----

    /// <summary>One read of a wiki's API for the page: {site, q: {action, …}}.</summary>
    static async Task<object?> WikiApiAsync(JsonElement a)
    {
        var q = new Dictionary<string, string>(StringComparer.Ordinal);
        if (Has(a, "q", JsonValueKind.Object))
            foreach (var p in a.GetProperty("q").EnumerateObject().Take(30))
            {
                string? v = p.Value.ValueKind switch
                {
                    JsonValueKind.String => p.Value.GetString(),
                    JsonValueKind.Number => p.Value.GetRawText(),
                    JsonValueKind.True => "1",
                    _ => null,
                };
                if (v is not null)
                    q[p.Name] = v;
            }
        return await WikiNet.ApiAsync(Str(a, "site"), q);
    }

    /// <summary>Wikis for a game's name, best first.</summary>
    static async Task<object?> WikiFindAsync(JsonElement a) =>
        (await WikiNet.FindAsync(Str(a, "name"))).Select(f => new { site = f.Site, name = f.Name, pages = f.Pages, views = f.Views, img = f.Img, match = f.Match });

    /// <summary>The wiki a pasted link (or a site) is, or a message saying it isn't one.</summary>
    static async Task<object?> WikiAboutAsync(JsonElement a)
    {
        string link = Str(a, "url");
        string site = WikiNet.Site(link) ?? WikiNet.SiteOfLink(link) ?? throw new UserError(L.T("Paste a link to a page of the game's wiki (Fandom or wiki.gg)."));
        var about = await WikiNet.AboutAsync(site) ?? throw new UserError(L.T("That isn't a wiki Bloxkeep can read."));
        return new { site = about.Site, name = about.Name, articles = about.Articles, mainPage = about.MainPage, logo = about.Logo };
    }

    /// <summary>A game's wiki picked (or, with no site, forgotten): {id, name, site, place, img, added}.</summary>
    object? SetWikiPick(JsonElement a)
    {
        string id = Str(a, "id").Trim();
        if (id.Length is 0 or > 120)
            return null;
        var games = Prefs.Wikis.Games;
        games.RemoveAll(g => g.Id == id);
        if (WikiNet.Site(Str(a, "site")) is { } site)
        {
            string? img = OptStr(a, "img") is { } i && Uri.TryCreate(i, UriKind.Absolute, out var iu) && iu.Scheme == Uri.UriSchemeHttps
                && (iu.Host is "static.wikia.nocookie.net" || iu.Host.EndsWith(".rbxcdn.com", StringComparison.Ordinal) || iu.Host.EndsWith(".wiki.gg", StringComparison.Ordinal)) ? i : null;
            string name = Str(a, "name").Trim();
            games.Add(new Prefs.WikiPick
            {
                Id = id, Name = name.Length is > 0 and <= 80 ? name : new Uri(site).Host, Site = site,
                Place = Math.Max(0, Long(a, "place")), Img = img, Added = Bool(a, "added"),
            });
            if (games.Count > 200)
                games.RemoveAt(0);
        }
        Prefs.Wikis.Saved.RemoveAll(m => m.Game == id && !games.Any(g => g.Id == id));
        Prefs.Save();
        PushState();
        return null;
    }

    /// <summary>A page saved or unsaved ({list: "saved", on}), or seen ({list: "recent"}): {site, title, game}.</summary>
    object? MarkWikiPage(JsonElement a)
    {
        string? site = WikiNet.Site(Str(a, "site"));
        string title = Str(a, "title").Trim(), game = Str(a, "game").Trim();
        if (site is null || title.Length is 0 or > 250 || game.Length > 120)
            return null;
        bool recent = Str(a, "list") == "recent";
        var list = recent ? Prefs.Wikis.Recent : Prefs.Wikis.Saved;
        list.RemoveAll(m => m.Site == site && m.Title == title);
        if (recent || Bool(a, "on"))
        {
            list.Insert(0, new Prefs.WikiMark { Site = site, Title = title, Game = game, At = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds() });
            int max = recent ? 30 : 300;
            if (list.Count > max)
                list.RemoveRange(max, list.Count - max);
        }
        Prefs.Save();
        PushState();
        return null;
    }

    static object WikisDto() => new
    {
        games = Prefs.Wikis.Games.Select(g => new { id = g.Id, name = g.Name, site = g.Site, place = g.Place, img = g.Img, added = g.Added }),
        saved = Prefs.Wikis.Saved.Select(m => new { site = m.Site, title = m.Title, game = m.Game, at = m.At }),
        recent = Prefs.Wikis.Recent.Select(m => new { site = m.Site, title = m.Title, game = m.Game, at = m.At }),
    };
}
