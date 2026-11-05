using System.Globalization;
using System.Text;
using System.Text.Json;
using System.Text.RegularExpressions;

namespace RobloxManager;

/// <summary>
/// The Game news tab: news other games' bots post (a stock, a boss spawning, a full moon), brought by Bloxkeep's Discord
/// bot (<see cref="FeedLink"/>, services/DiscordBot/worker/feeds.js). Which games and sources there are is the bot's
/// business: Bloxkeep's developers add and remove them there, and apps already out get them at once. Here: the games the
/// person follows (their unread news is the tab's badge; a game not seen yet in the list marks the tab too), how far each
/// was read, and when to be told (every piece, or one with a word they chose) through the usual notifications (Windows,
/// the phone). Only an installed Bloxkeep connects (as the online count): not a development copy or the checks, unless
/// BLOXKEEP_FEEDS_URL points them at a server of their own.
/// </summary>
public sealed partial class AppController
{
    FeedLink? _feeds;
    List<FeedGame> _feedGames = [];
    /// <summary>Newest first.</summary>
    List<FeedCard> _feedCards = [];
    /// <summary>The price lists of the games that have one, by game.</summary>
    Dictionary<string, FeedPrices> _feedPrices = [];
    /// <summary>The bot has told the games once since the app started.</summary>
    bool _feedsLoaded;
    /// <summary>The newest card heard of (ms): after a drop, what came since is news to be told of; before the first, nothing is.</summary>
    long _feedNewest;

    const int MaxFeedWords = 20, MaxFeedWord = 40, FeedCardsPerSource = 30;

    static string? FeedsTestUrl => Environment.GetEnvironmentVariable("BLOXKEEP_FEEDS_URL") is { Length: > 0 } url ? url : null;
    bool FeedsAllowed => _updater.Supported || FeedsTestUrl is not null;

    void TickFeeds()
    {
        if (_feeds is null && !_disposed && FeedsAllowed)
            _feeds = new FeedLink(OnFeeds, OnFeedPrices, OnFeedCard, OnFeedGone, PushState, FeedsTestUrl);
    }

    void OnFeeds(List<FeedGame> games, List<FeedCard> cards, Dictionary<string, FeedPrices> prices)
    {
        _feedPrices = prices;
        Emit("feedPrices", FeedPricesDto());
        bool first = !_feedsLoaded;
        _feedsLoaded = true;
        _feedGames = games;
        var before = _feedCards.Select(c => (c.Id, c.At)).ToHashSet();
        _feedCards = cards.OrderByDescending(c => c.At).ToList();
        // Back after a drop: what came meanwhile (newer than the newest heard of) is told as it would have been.
        if (!first)
            foreach (var card in _feedCards.Where(c => c.At > _feedNewest && !before.Contains((c.Id, c.At))).Reverse())
                AlertFeed(card);
        _feedNewest = Math.Max(_feedNewest, _feedCards.Count > 0 ? _feedCards[0].At : 0);
        _ = LoadFeedGamesAsync();
        FeedsChanged();
    }

    void OnFeedCard(FeedCard card)
    {
        int at = _feedCards.FindIndex(c => c.Id == card.Id);
        if (at >= 0 && _feedCards[at].At >= card.At)
            return;
        if (at >= 0)
            _feedCards.RemoveAt(at);
        _feedCards.Insert(0, card);
        _feedCards = _feedCards.OrderByDescending(c => c.At).ToList();
        // The bot keeps 30 a source: so does the app.
        var count = new Dictionary<string, int>(StringComparer.Ordinal);
        _feedCards.RemoveAll(c => (count[c.Src] = count.GetValueOrDefault(c.Src) + 1) > FeedCardsPerSource);
        _feedNewest = Math.Max(_feedNewest, card.At);
        AlertFeed(card);
        FeedsChanged();
    }

    void OnFeedPrices(string game, FeedPrices prices)
    {
        if (!_feedGames.Any(g => g.Id == game))
            return;
        _feedPrices[game] = prices;
        Emit("feedPrices", FeedPricesDto());
    }

    /// <summary>The price lists, by game (the page asks with feedPrices and hears of new ones by that name).</summary>
    object FeedPricesDto() => _feedPrices.ToDictionary(p => p.Key, p => new
    {
        source = new { name = p.Value.Source.Name, url = p.Value.Source.Url }, at = p.Value.At,
        items = p.Value.Items.Select(i => new
        {
            id = i.Id, name = i.Name, category = i.Category, rarity = i.Rarity, type = i.Type, img = i.Img, value = i.Value, perm = i.Perm,
            demand = i.Demand, permDemand = i.PermDemand, trend = i.Trend, permTrend = i.PermTrend, beli = i.Beli, robux = i.Robux, bestFor = i.BestFor,
            updated = i.Updated, url = i.Url,
        }),
    });

    void OnFeedGone(string id)
    {
        if (_feedCards.RemoveAll(c => c.Id == id) > 0)
            FeedsChanged();
    }

    /// <summary>The page's list of news changed (it asks for it with feedCards and hears of it here); the badge too.</summary>
    void FeedsChanged()
    {
        Emit("feedCards", FeedCardsDto());
        PushState();
    }

    /// <summary>The games' names and pictures from Roblox, for those the bot gave a place.</summary>
    async Task LoadFeedGamesAsync()
    {
        var places = _feedGames.Where(g => g.Place is > 0).Select(g => (g.Place!.Value, 0L)).ToList();
        if (places.Count == 0)
            return;
        await LoadGamesAsync(places);
        PushState();
    }

    /// <summary>A card the person asked to be told of: their game, followed, and every piece or one with their word.</summary>
    void AlertFeed(FeedCard card)
    {
        var prefs = Prefs.GameFeeds;
        if (!prefs.Follow.Contains(card.Game) || prefs.Alerts.GetValueOrDefault(card.Game) is not { Mode: "all" or "words" } alert)
            return;
        string? word = null;
        if (alert.Mode == "words")
        {
            string all = Fold(string.Join("\n", [card.Title, card.Text, .. card.Fields.Select(f => f.Name + " " + f.Value)]));
            word = alert.Words.FirstOrDefault(w => Fold(w) is { Length: > 0 } f && Regex.IsMatch(all, $@"(?<![\p{{L}}\p{{N}}]){Regex.Escape(f)}(?![\p{{L}}\p{{N}}])"));
            if (word is null)
                return;
        }
        string game = _feedGames.FirstOrDefault(g => g.Id == card.Game)?.Name ?? card.Game;
        string title = card.Title.Length > 0 ? card.Title : card.Text.Split('\n')[0];
        string text = word is null ? $"{game} · {title}" : $"{game} · {title} · {word}";
        Notify(text.Length > 200 ? text[..199] + "…" : text, "info", "feed", repeatKey: $"feed:{card.Id}:{card.At}");
    }

    /// <summary>Lower case, without accents (đ as d): "Kitsune" finds "KITSUNE", "rong" finds "Rồng".</summary>
    static string Fold(string text)
    {
        var plain = new StringBuilder(text.Length);
        foreach (char c in text.ToLowerInvariant().Replace('đ', 'd').Normalize(NormalizationForm.FormD))
            if (CharUnicodeInfo.GetUnicodeCategory(c) != UnicodeCategory.NonSpacingMark)
                plain.Append(c);
        return plain.ToString().Trim();
    }

    int FeedUnread(string game)
    {
        if (!Prefs.GameFeeds.Follow.Contains(game))
            return 0;
        long read = Prefs.GameFeeds.Read.GetValueOrDefault(game);
        return _feedCards.Count(c => c.Game == game && c.At > read);
    }

    object FeedsDto()
    {
        var prefs = Prefs.GameFeeds;
        var games = _feedGames.Select(g =>
        {
            var info = g.Place is long place ? _games.GetValueOrDefault(place) : null;
            var alert = prefs.Alerts.GetValueOrDefault(g.Id);
            return new
            {
                id = g.Id, name = g.Name, place = g.Place, iconUrl = info?.IconUrl,
                sources = g.Sources.Select(s => new { id = s.Id, title = s.Title }),
                followed = prefs.Follow.Contains(g.Id), unread = FeedUnread(g.Id), read = prefs.Read.GetValueOrDefault(g.Id), isNew = !prefs.Seen.Contains(g.Id),
                alert = new { mode = alert?.Mode ?? "off", words = alert?.Words ?? [] },
            };
        }).ToList();
        return new
        {
            on = FeedsAllowed, connected = _feeds?.Connected ?? false, loaded = _feedsLoaded, games,
            unread = games.Sum(g => g.unread), fresh = games.Count(g => g.isNew),
        };
    }

    object FeedCardsDto() => _feedCards.Select(c => new
    {
        id = c.Id, src = c.Src, game = c.Game, at = c.At, title = c.Title, text = c.Text,
        fields = c.Fields.Select(f => new { name = f.Name, value = f.Value }), picture = DiscordPicture(c.Picture), image = DiscordPicture(c.Image),
        author = c.Author is { Name.Length: > 0 } by ? new { name = by.Name, icon = DiscordPicture(by.Icon), url = by.Url } : null, link = c.Link, color = c.Color, until = c.Until,
        blocks = c.Blocks, spoiler = c.Spoiler,
    }).ToList();

    /// <summary>
    /// A link in a piece of news (a button, a masked link, its title): opened in the browser, if it is one of the news's
    /// own and https. The page can open nothing else this way.
    /// </summary>
    object? OpenFeedLink(string url)
    {
        if (!Uri.TryCreate(url, UriKind.Absolute, out var uri) || uri.Scheme != Uri.UriSchemeHttps
            || !(_feedCards.Any(c => FeedHasLink(c, url)) || _feedPrices.Values.Any(p => p.Source.Url == url || p.Items.Any(i => i.Url == url))))
            throw new UserError(L.T("That link isn't in the news any more."));
        return OpenUrl(uri.AbsoluteUri);
    }

    static bool FeedHasLink(FeedCard c, string url)
    {
        if (c.Link == url || c.Author?.Url == url)
            return true;
        // In a block: a button's link, or in its words ([words](link), or the link itself).
        static bool In(JsonElement e, string url) => e.ValueKind switch
        {
            JsonValueKind.String => e.GetString() is { } s && (s == url || Regex.IsMatch(s, $@"(?<![^\s(<\[]){Regex.Escape(url)}(?=$|[\s)>\]])")),
            JsonValueKind.Array => e.EnumerateArray().Any(x => In(x, url)),
            JsonValueKind.Object => e.EnumerateObject().Any(p => In(p.Value, url)),
            _ => false,
        };
        return c.Blocks is { } blocks && In(blocks, url);
    }

    /// <summary>A picture from Discord's own servers only (the page's rules allow no other): nothing loads from wherever a game's bot points.</summary>
    static string? DiscordPicture(string? url) =>
        Uri.TryCreate(url, UriKind.Absolute, out var uri) && uri.Scheme == Uri.UriSchemeHttps && uri.IsDefaultPort && uri.UserInfo.Length == 0
        && (uri.Host is "media.discordapp.net" or "cdn.discordapp.com" || uri.Host.EndsWith(".discordapp.net", StringComparison.Ordinal))
            ? url : null;

    /// <summary>Followed from now: what is there already counts as read (the badge is for news that comes after).</summary>
    object? SetFeedFollow(string game, bool on)
    {
        var prefs = Prefs.GameFeeds;
        if (on && _feedGames.Any(g => g.Id == game) && !prefs.Follow.Contains(game))
        {
            prefs.Follow.Add(game);
            prefs.Read[game] = _feedCards.Where(c => c.Game == game).Select(c => c.At).DefaultIfEmpty(0).Max();
        }
        else if (!on)
            prefs.Follow.Remove(game);
        Prefs.Save();
        PushState();
        return null;
    }

    /// <summary>The game's news shown: read up to its newest.</summary>
    object? MarkFeedRead(string game)
    {
        long newest = _feedCards.Where(c => c.Game == game).Select(c => c.At).DefaultIfEmpty(0).Max();
        if (newest <= Prefs.GameFeeds.Read.GetValueOrDefault(game))
            return null;
        Prefs.GameFeeds.Read[game] = newest;
        Prefs.Save();
        PushState();
        return null;
    }

    /// <summary>The tab opened: the games in it are no longer new. Games the bot no longer lists are forgotten.</summary>
    object? MarkFeedsSeen()
    {
        var prefs = Prefs.GameFeeds;
        if (_feedGames.All(g => prefs.Seen.Contains(g.Id)))
            return null;
        prefs.Seen = _feedGames.Select(g => g.Id).Union(prefs.Seen).Take(200).ToList();
        Prefs.Save();
        PushState();
        return null;
    }

    /// <summary>game, mode ("off", "all", "words") and/or words (a list, or text split at commas and new lines).</summary>
    object? SetFeedAlert(JsonElement a)
    {
        string game = Str(a, "game");
        if (!_feedGames.Any(g => g.Id == game))
            throw new UserError(L.T("That game isn't in the news any more."));
        var prefs = Prefs.GameFeeds;
        var alert = prefs.Alerts.GetValueOrDefault(game) ?? new Prefs.FeedAlert();
        if (OptStr(a, "mode") is "off" or "all" or "words")
            alert.Mode = OptStr(a, "mode")!;
        if (Has(a, "words", JsonValueKind.Array))
            alert.Words = a.GetProperty("words").EnumerateArray().Where(w => w.ValueKind == JsonValueKind.String).Select(w => w.GetString()!).ToList();
        else if (OptStr(a, "words") is { } text)
            alert.Words = text.Split([',', '\n', ';']).ToList();
        alert.Words = alert.Words.Select(w => w.Trim()).Where(w => w.Length > 0).Select(w => w.Length > MaxFeedWord ? w[..MaxFeedWord] : w)
            .Distinct(StringComparer.CurrentCultureIgnoreCase).Take(MaxFeedWords).ToList();
        prefs.Alerts[game] = alert;
        // Asked to be told of a game's news: it is followed (the badge too).
        if (alert.Mode != "off" && !prefs.Follow.Contains(game))
            SetFeedFollow(game, true);
        Prefs.Save();
        PushState();
        return null;
    }
}
