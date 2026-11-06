using System.Net.WebSockets;
using System.Text;
using System.Text.Json;

namespace RobloxManager;

/// <summary>One source of a game's news (a feed channel at Bloxkeep's bot): "Stock", "Fruit spawn"…</summary>
public sealed record FeedSource(string Id, string Title);
/// <summary>A game with news, as Bloxkeep's bot lists it: its id (what following remembers), name, place on Roblox and sources.</summary>
public sealed record FeedGame(string Id, string Name, long? Place, List<FeedSource> Sources);
public sealed record FeedField(string Name, string Value);
/// <summary>Who posted it, as the game's bot names itself in its message (its name and picture).</summary>
public sealed record FeedAuthor(string Name, string? Icon, string? Url = null);
/// <summary>
/// One piece of news: a message another game's bot posted, read by Bloxkeep's bot (feeds.js cardsOf). At: when it came
/// or last changed (ms); Until: when what it tells of ends, if it says (ms). Text may hold Discord's times (&lt;t:…&gt;),
/// which the page shows in the person's own clock. Blocks: what it shows, in Discord's order (text with Discord's markdown,
/// fields, sections, galleries, files, lines, buttons, a footer, a poll, a sticker), passed to the page as they came so a
/// way of posting the app has not seen yet needs no update here; Title, Text and Fields are the same words for alerts.
/// </summary>
public sealed record FeedCard(string Id, string Src, string Game, long At, string Title, string Text, List<FeedField> Fields,
    string? Picture, string? Link, int? Color, long? Until, string? Image = null, FeedAuthor? Author = null, JsonElement? Blocks = null, bool Spoiler = false);

/// <summary>
/// Game news (Settings-free: the Game news tab) from Bloxkeep's Discord bot (services/DiscordBot/worker/feeds.js): a
/// WebSocket that only listens. On connecting it is told the games and their latest news, then each new piece as it comes,
/// one deleted there, and the games again when Bloxkeep's developers add or take away a game or a source. Nothing goes up
/// but a "ping" every 30 s, which the bot answers without waking. Connected again after a drop (2 s, then longer, up to
/// two minutes). Its callbacks run on the thread that made it (the window's).
/// </summary>
sealed class FeedLink : IDisposable
{
    public const string Url = "wss://bloxkeep-bot.bloxkeep-app.workers.dev/feeds/connect";
    static readonly TimeSpan KeepAlive = TimeSpan.FromSeconds(30);
    static readonly TimeSpan FirstRetry = TimeSpan.FromSeconds(2), LastRetry = TimeSpan.FromMinutes(2), StayedUp = TimeSpan.FromSeconds(60);
    /// <summary>The games and every card at once (30 a source, at most a few KB each).</summary>
    const int MaxMessage = 4 * 1024 * 1024;
    static readonly JsonSerializerOptions Json = new() { PropertyNameCaseInsensitive = true };

    readonly string _url;
    readonly SynchronizationContext _ui;
    readonly Action<List<FeedGame>, List<FeedCard>> _all;
    readonly Action<FeedCard> _card;
    readonly Action<string> _gone;
    readonly Action _changed;
    readonly CancellationTokenSource _stop = new();

    public bool Connected { get; private set; }

    /// <param name="all">The games and their cards, all of them (on connecting, and when the games change).</param>
    /// <param name="card">A new or changed card.</param>
    /// <param name="gone">A card's id, deleted where it came from.</param>
    /// <param name="changed">Connected or not any more.</param>
    public FeedLink(Action<List<FeedGame>, List<FeedCard>> all, Action<FeedCard> card, Action<string> gone, Action changed, string? url = null)
    {
        _all = all;
        _card = card;
        _gone = gone;
        _changed = changed;
        _url = url ?? Url;
        _ui = SynchronizationContext.Current ?? new SynchronizationContext();
        _ = RunAsync(_stop.Token);
    }

    async Task RunAsync(CancellationToken stop)
    {
        var wait = FirstRetry;
        while (!stop.IsCancellationRequested)
        {
            using var ws = new ClientWebSocket();
            ws.Options.KeepAliveInterval = TimeSpan.Zero; // the bot answers our own "ping" without waking
            DateTime? connectedAt = null;
            try
            {
                await ws.ConnectAsync(new Uri(_url), stop);
                connectedAt = DateTime.UtcNow;
                OnUi(() =>
                {
                    Connected = true;
                    _changed();
                });
                using var alive = CancellationTokenSource.CreateLinkedTokenSource(stop);
                _ = PingAsync(ws, alive.Token);
                await ReadAsync(ws, stop);
                alive.Cancel();
            }
            catch (Exception ex) when (ex is WebSocketException or OperationCanceledException or IOException or HttpRequestException)
            {
            }
            catch (Exception ex)
            {
                Log.Write(ex);
            }
            finally
            {
                if (Connected)
                    OnUi(() =>
                    {
                        Connected = false;
                        _changed();
                    });
            }
            if (stop.IsCancellationRequested)
                break;
            if (connectedAt is { } at && DateTime.UtcNow - at >= StayedUp)
                wait = FirstRetry;
            try
            {
                await Task.Delay(wait + TimeSpan.FromMilliseconds(Random.Shared.Next(2000)), stop);
            }
            catch (OperationCanceledException)
            {
                break;
            }
            wait = wait * 2 < LastRetry ? wait * 2 : LastRetry;
        }
    }

    static async Task PingAsync(ClientWebSocket ws, CancellationToken ct)
    {
        try
        {
            while (!ct.IsCancellationRequested && ws.State == WebSocketState.Open)
            {
                await Task.Delay(KeepAlive, ct);
                await ws.SendAsync("ping"u8.ToArray(), WebSocketMessageType.Text, true, ct);
            }
        }
        catch (Exception ex) when (ex is OperationCanceledException or WebSocketException or ObjectDisposedException or InvalidOperationException)
        {
        }
    }

    async Task ReadAsync(ClientWebSocket ws, CancellationToken ct)
    {
        var buffer = new byte[16 * 1024];
        var message = new MemoryStream();
        while (ws.State == WebSocketState.Open)
        {
            var got = await ws.ReceiveAsync(buffer, ct);
            if (got.MessageType == WebSocketMessageType.Close)
                return;
            message.Write(buffer, 0, got.Count);
            if (message.Length > MaxMessage)
                return;
            if (!got.EndOfMessage)
                continue;
            string text = Encoding.UTF8.GetString(message.GetBuffer(), 0, (int)message.Length);
            message.SetLength(0);
            if (text != "pong")
                OnMessage(text);
        }
    }

    sealed record Message(string? T, List<FeedGame>? Games, List<FeedCard>? Cards, FeedCard? Card, string? Id);

    void OnMessage(string text)
    {
        Message? m;
        try
        {
            m = JsonSerializer.Deserialize<Message>(text, Json);
        }
        catch (JsonException)
        {
            return;
        }
        switch (m?.T)
        {
            case "feeds" when m.Games is { } games:
                var ok = games.Where(Valid).ToList();
                var known = ok.Select(g => g.Id).ToHashSet(StringComparer.Ordinal);
                var cards = (m.Cards ?? []).Where(c => Valid(c) && known.Contains(c.Game)).ToList();
                OnUi(() => _all(ok, cards));
                break;
            case "card" when m.Card is { } card && Valid(card):
                OnUi(() => _card(card));
                break;
            case "gone" when m.Id is { Length: > 0 } id:
                OnUi(() => _gone(id));
                break;
        }
    }

    static bool Valid(FeedGame g) => g.Id is { Length: > 0 and <= 40 } && g.Name is { Length: > 0 } && g.Sources is not null;
    static bool Valid(FeedCard c) => c.Id is { Length: > 0 } && c.Game is { Length: > 0 } && c.Title is not null && c.Text is not null && c.Fields is not null
        && c.Blocks is not { ValueKind: not JsonValueKind.Array };

    void OnUi(Action action) => _ui.Post(_ =>
    {
        try
        {
            action();
        }
        catch (Exception ex)
        {
            Log.Write(ex);
        }
    }, null);

    public void Dispose() => _stop.Cancel();
}
