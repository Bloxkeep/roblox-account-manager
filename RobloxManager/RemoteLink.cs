using System.Net.WebSockets;
using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;

namespace RobloxManager;

/// <summary>
/// A command from Discord (/pc): its id (to answer it), its name ("status", "preset", "close", "switch", "screenshot")
/// and value; for a screenshot, the link its pictures are sent to (Discord's own, for that one answer).
/// </summary>
public sealed record RemoteCommand(string Id, string Name, string? Value, string? Reply = null);

/// <summary>
/// The connection to Bloxkeep's Discord bot that Settings › Discord › Control from Discord keeps open
/// (services/DiscordBot/worker/remote.js): a WebSocket the bot hands this PC's commands down, kept open while it's on,
/// connected again after a drop (2 s, then longer, up to a minute). The PC proves itself with its secret, which only the
/// bot's hash of it is kept against. What goes up: the PC's name and its accounts' and presets' names (for Discord's
/// suggestions while typing), answers to commands, a code when asked for one. Everything else stays here. The pictures
/// of /pc screenshot don't go to the bot: they are sent to Discord itself, to the link the bot handed with the command.
/// Its callbacks run on the thread that made it (the window's).
/// </summary>
sealed class RemoteLink : IDisposable
{
    public const string Url = "wss://bloxkeep-bot.bloxkeep-app.workers.dev/remote/connect";
    static readonly TimeSpan KeepAlive = TimeSpan.FromSeconds(30);
    static readonly TimeSpan FirstRetry = TimeSpan.FromSeconds(2), LastRetry = TimeSpan.FromMinutes(1);
    const int MaxMessage = 64 * 1024;

    readonly string _url, _secret;
    readonly SynchronizationContext _ui;
    readonly Func<RemoteCommand, Task<JsonObject?>> _command;
    readonly Action _changed;
    readonly CancellationTokenSource _stop = new();
    readonly SemaphoreSlim _sending = new(1, 1);
    ClientWebSocket? _ws;
    string? _hello;
    bool _wantCode;

    /// <summary>Connected to the bot now.</summary>
    public bool Connected { get; private set; }
    /// <summary>Linked to a Discord account (null until the bot said).</summary>
    public bool? Linked { get; private set; }
    /// <summary>The name Discord shows that account by, when the bot knows it.</summary>
    public string? User { get; private set; }
    /// <summary>The code to type in Discord (/pc link), and until when it works.</summary>
    public string? Code { get; private set; }
    public DateTime? CodeUntil { get; private set; }
    /// <summary>
    /// Someone typed this PC's code into /pc link: the Discord account (the name Discord shows), and the one it would
    /// replace. Nothing is linked until it is answered (<see cref="Answer"/>).
    /// </summary>
    public (string User, string? Replacing)? Asking { get; private set; }
    /// <summary>The bot had no room for a new code just now.</summary>
    public bool Busy { get; private set; }

    /// <param name="command">Runs a command (on the window's thread) and gives the answer: content and/or embeds; null when answered already (<see cref="ShotAsync"/>).</param>
    /// <param name="changed">Connected, linked or a code came: the page shows it.</param>
    public RemoteLink(string secret, Func<RemoteCommand, Task<JsonObject?>> command, Action changed, string? url = null)
    {
        _secret = secret;
        _command = command;
        _changed = changed;
        _url = url ?? Url;
        _ui = SynchronizationContext.Current ?? new SynchronizationContext();
        _ = RunAsync(_stop.Token);
    }

    /// <summary>What the bot shows while someone types (the PC's, accounts' and presets' names): sent when it changes.</summary>
    public void Hello(string name, IEnumerable<string> accounts, IEnumerable<string> presets, string language)
    {
        string hello = new JsonObject
        {
            ["t"] = "hello", ["name"] = name, ["lang"] = language,
            // It asks before a link is made (the bot links an app that doesn't say so at once, as 1.0.12 did).
            ["ask"] = true,
            // It answers /pc screenshot (the bot tells whoever asks an older one to update it).
            ["shots"] = true,
            ["accounts"] = new JsonArray(accounts.Select(a => (JsonNode)a).ToArray()),
            ["presets"] = new JsonArray(presets.Select(p => (JsonNode)p).ToArray()),
        }.ToJsonString();
        if (hello == _hello)
            return;
        _hello = hello;
        if (Connected)
            _ = SendAsync(hello);
    }

    /// <summary>A new code to link this PC (shown once the bot sends it; asked again on the next connection if offline now).</summary>
    public void AskCode()
    {
        Code = null;
        CodeUntil = null;
        _wantCode = true;
        if (Connected)
            _ = SendAsync("""{"t":"code"}""");
    }

    /// <summary>The answer to <see cref="Asking"/>: linked, or nothing changes.</summary>
    public void Answer(bool allow)
    {
        if (Asking is null)
            return;
        Asking = null;
        _ = SendAsync(allow ? """{"t":"allow"}""" : """{"t":"deny"}""");
    }

    /// <summary>Not linked any more (the bot forgets which Discord account it was).</summary>
    public Task UnlinkAsync() => Connected ? SendAsync("""{"t":"unlink"}""") : Task.CompletedTask;

    /// <summary>How a command went once done (a preset's windows open): a follow-up in Discord, within its 15 minutes.</summary>
    public void Later(string id, JsonObject answer)
    {
        answer["t"] = "later";
        answer["id"] = id;
        if (Connected)
            _ = SendAsync(answer.ToJsonString());
    }

    static readonly HttpClient Http = new() { Timeout = TimeSpan.FromSeconds(60) };

    /// <summary>
    /// A link pictures may be sent to: Discord's own answer link (discord.com/api/…/webhooks/…), never anywhere else the
    /// bot (or someone posing as it) might name. The checks' stand-in bot names its own.
    /// </summary>
    bool MaySendTo(string reply) =>
        Uri.TryCreate(reply, UriKind.Absolute, out var uri)
        && (_url != Url ? uri.Scheme is "http" or "https"
            : uri.Scheme == "https" && uri.Host == "discord.com" && uri.AbsolutePath.StartsWith("/api/", StringComparison.Ordinal) && uri.AbsolutePath.Contains("/webhooks/"));

    /// <summary>
    /// The answer to /pc screenshot (<paramref name="command"/>): its pictures (JPEG) and a line about them, put in place
    /// of Discord's "thinking…" through the link the bot handed with it (no bot token: that link is good for that answer
    /// only), then the bot is told it's done. They never pass through the bot. False when they didn't get there.
    /// </summary>
    public async Task<bool> ShotAsync(RemoteCommand command, string content, IReadOnlyList<byte[]> pictures)
    {
        if (command.Reply is not { } reply || !MaySendTo(reply))
            return false;
        MultipartFormDataContent Form()
        {
            var payload = new JsonObject
            {
                ["content"] = content,
                ["allowed_mentions"] = new JsonObject { ["parse"] = new JsonArray() },
                ["attachments"] = new JsonArray(pictures.Select((_, i) => (JsonNode)new JsonObject { ["id"] = i, ["filename"] = $"screenshot-{i + 1}.jpg" }).ToArray()),
            };
            var form = new MultipartFormDataContent { { new StringContent(payload.ToJsonString(), Encoding.UTF8, "application/json"), "payload_json" } };
            for (int i = 0; i < pictures.Count; i++)
            {
                var part = new ByteArrayContent(pictures[i]);
                part.Headers.ContentType = new System.Net.Http.Headers.MediaTypeHeaderValue("image/jpeg");
                form.Add(part, $"files[{i}]", $"screenshot-{i + 1}.jpg");
            }
            return form;
        }
        try
        {
            for (int attempt = 0; ; attempt++)
            {
                using var form = Form();
                using var response = await Http.PatchAsync(reply, form, _stop.Token);
                // Quicker than Discord: its "thinking…" isn't there to edit yet (once); asked too often: waited out (once).
                if (attempt == 0 && response.StatusCode is System.Net.HttpStatusCode.NotFound or System.Net.HttpStatusCode.TooManyRequests)
                {
                    await Task.Delay(TimeSpan.FromSeconds(2), _stop.Token);
                    continue;
                }
                if (!response.IsSuccessStatusCode)
                    return false;
                break;
            }
        }
        catch (Exception ex) when (ex is HttpRequestException or OperationCanceledException)
        {
            return false;
        }
        await SendAsync(new JsonObject { ["t"] = "shown", ["id"] = command.Id }.ToJsonString());
        return true;
    }

    async Task RunAsync(CancellationToken stop)
    {
        var wait = FirstRetry;
        while (!stop.IsCancellationRequested)
        {
            using var ws = new ClientWebSocket();
            ws.Options.SetRequestHeader("Authorization", "Bearer " + _secret);
            ws.Options.KeepAliveInterval = TimeSpan.Zero; // the bot answers our own "ping" without waking
            try
            {
                await ws.ConnectAsync(new Uri(_url), stop);
                _ws = ws;
                wait = FirstRetry;
                await OnUi(() =>
                {
                    Connected = true;
                    _changed();
                });
                if (_hello is { } hello)
                    await SendAsync(hello);
                if (_wantCode)
                    await SendAsync("""{"t":"code"}""");
                using var alive = new CancellationTokenSource();
                _ = PingAsync(ws, CancellationTokenSource.CreateLinkedTokenSource(stop, alive.Token).Token);
                await ReadAsync(ws, stop);
                alive.Cancel();
            }
            catch (Exception ex) when (ex is WebSocketException or OperationCanceledException or IOException or HttpRequestException or JsonException)
            {
            }
            // Anything else: logged, and connected again as after a drop, so /pc doesn't stay dead until a restart.
            catch (Exception ex)
            {
                Log.Write(ex);
            }
            finally
            {
                _ws = null;
                if (Connected)
                    await OnUi(() =>
                    {
                        Connected = false;
                        _changed();
                    });
            }
            if (stop.IsCancellationRequested)
                break;
            try
            {
                await Task.Delay(wait + TimeSpan.FromMilliseconds(Random.Shared.Next(1000)), stop);
            }
            catch (OperationCanceledException)
            {
                break;
            }
            wait = wait * 2 < LastRetry ? wait * 2 : LastRetry;
        }
    }

    async Task PingAsync(ClientWebSocket ws, CancellationToken ct)
    {
        try
        {
            while (!ct.IsCancellationRequested && ws.State == WebSocketState.Open)
            {
                await Task.Delay(KeepAlive, ct);
                await SendAsync("ping");
            }
        }
        catch (Exception ex) when (ex is OperationCanceledException or WebSocketException or ObjectDisposedException)
        {
        }
    }

    async Task ReadAsync(ClientWebSocket ws, CancellationToken ct)
    {
        var buffer = new byte[8192];
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
                await OnMessageAsync(text);
        }
    }

    async Task OnMessageAsync(string text)
    {
        JsonObject? m;
        try
        {
            m = JsonNode.Parse(text) as JsonObject;
        }
        catch (JsonException)
        {
            return;
        }
        switch (Str(m, "t"))
        {
            case "link":
                await OnUi(() =>
                {
                    Linked = m!["linked"]?.GetValueKind() == JsonValueKind.True;
                    User = Linked == true ? Str(m, "user") : null;
                    Asking = null; // allowed, refused or given up on: either way not asked any more
                    if (Linked == true)
                    {
                        Code = null;
                        CodeUntil = null;
                        _wantCode = false;
                    }
                    _changed();
                });
                break;
            case "ask" when Str(m, "user") is { Length: > 0 } asker:
                await OnUi(() =>
                {
                    Asking = (asker, Str(m, "replacing"));
                    _changed();
                });
                break;
            case "busy":
                await OnUi(() =>
                {
                    Busy = true;
                    _wantCode = false;
                    _changed();
                });
                break;
            case "code" when Str(m, "code") is { Length: > 0 } code:
                long until = m!["until"]?.GetValueKind() == JsonValueKind.Number ? m["until"]!.GetValue<long>() : 0;
                await OnUi(() =>
                {
                    Code = code;
                    Busy = false;
                    CodeUntil = until > 0 ? DateTimeOffset.FromUnixTimeMilliseconds(until).UtcDateTime : DateTime.UtcNow.AddMinutes(10);
                    _wantCode = false;
                    _changed();
                });
                break;
            case "command" when Str(m, "id") is { Length: > 0 } id && Str(m, "command") is { Length: > 0 } name:
                var command = new RemoteCommand(id, name, Str(m, "value"), Str(m, "reply"));
                var answer = await OnUi(() => _command(command));
                if (answer is null)
                    break;
                answer["t"] = "answer";
                answer["id"] = id;
                await SendAsync(answer.ToJsonString());
                break;
        }
    }

    static string? Str(JsonObject? m, string name) =>
        m?[name] is JsonValue v && v.GetValueKind() == JsonValueKind.String ? v.GetValue<string>() : null;

    async Task SendAsync(string text)
    {
        if (_ws is not { State: WebSocketState.Open } ws)
            return;
        await _sending.WaitAsync();
        try
        {
            await ws.SendAsync(Encoding.UTF8.GetBytes(text), WebSocketMessageType.Text, true, _stop.Token);
        }
        catch (Exception ex) when (ex is WebSocketException or OperationCanceledException or ObjectDisposedException)
        {
        }
        finally
        {
            _sending.Release();
        }
    }

    Task OnUi(Action action) => OnUi(() =>
    {
        action();
        return Task.FromResult(0);
    });

    /// <summary>Runs on the window's thread and waits for it there; a command's own error is its answer.</summary>
    Task<T> OnUi<T>(Func<Task<T>> work)
    {
        var done = new TaskCompletionSource<T>(TaskCreationOptions.RunContinuationsAsynchronously);
        _ui.Post(async _ =>
        {
            try
            {
                done.SetResult(await work());
            }
            catch (Exception ex)
            {
                done.SetException(ex);
            }
        }, null);
        return done.Task;
    }

    public void Dispose()
    {
        _stop.Cancel();
        try
        {
            _ws?.Abort();
        }
        catch (ObjectDisposedException)
        {
        }
    }
}
