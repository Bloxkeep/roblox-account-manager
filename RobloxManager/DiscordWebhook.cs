using System.Net;
using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;
using System.Text.RegularExpressions;

namespace RobloxManager;

/// <summary>What a message says before it is shaped (embed or plain text), as docs/discord-style.md says: a short title,
/// colour by meaning, facts in short fields, "Bloxkeep · …" in the footer.</summary>
/// <param name="Title">The news in a few words (markdown: names in it are escaped).</param>
/// <param name="Timestamp">When it happened: shown by Discord under an embed, in each reader's own time zone.</param>
public sealed record DiscordMessage(string Title, string Text, IReadOnlyList<DiscordField> Fields, string? Footer = null, int Color = DiscordWebhook.Blue, DateTime? Timestamp = null)
{
    /// <summary>A small line above the title: the account (or preset) the message is about. Plain text, not escaped.</summary>
    public string? Author { get; init; }
    /// <summary>The author's picture: the account's avatar from Roblox's image servers (https only).</summary>
    public string? AuthorIcon { get; init; }
    /// <summary>A picture in the top-right corner: the game's icon from Roblox's servers, or the logo.</summary>
    public string? Thumbnail { get; init; }
    public IReadOnlyList<DiscordMessage> More { get; init; } = [];
    /// <summary>A summary's picture: drawn when it goes out and attached to the message, shown in this embed.</summary>
    public SummaryCardData? Card { get; init; }
}

public sealed record DiscordFile(string Name, byte[] Data);

public sealed record DiscordField(string Name, string Value, bool Inline = false);

/// <summary>Who a message pings: user and role ids (numbers as text), @here, @everyone. Nothing else ever pings.</summary>
public sealed record DiscordPing(IReadOnlyList<string> Users, IReadOnlyList<string> Roles, bool Here, bool Everyone)
{
    public static readonly DiscordPing None = new([], [], false, false);

    public bool Any => Users.Count > 0 || Roles.Count > 0 || Here || Everyone;

    /// <summary>"none", "here", "everyone", "user:&lt;id&gt;" or "role:&lt;id&gt;"; null when it is none of these.</summary>
    public static DiscordPing? Parse(string? value) => value switch
    {
        null or "" or "none" => None,
        "here" => new([], [], true, false),
        "everyone" => new([], [], false, true),
        _ when value.StartsWith("user:", StringComparison.Ordinal) && DiscordWebhook.IsId(value[5..]) => new([value[5..]], [], false, false),
        _ when value.StartsWith("role:", StringComparison.Ordinal) && DiscordWebhook.IsId(value[5..]) => new([], [value[5..]], false, false),
        _ => null,
    };

    /// <summary>Several events in one message ping everyone any of them would have pinged, once each.</summary>
    public DiscordPing Merge(DiscordPing other) => new(
        Users.Concat(other.Users).Distinct().ToList(), Roles.Concat(other.Roles).Distinct().ToList(),
        Here || other.Here, Everyone || other.Everyone);
}

public enum DiscordOutcome { Sent, RateLimited, Gone, MessageGone, Rejected, Offline }

/// <param name="Wait">How long Discord asked to wait (a 429), or until its bucket refills after this one.</param>
/// <param name="Error">Discord's own reason (a 400) or the network error; never contains the link.</param>
public sealed record DiscordResult(DiscordOutcome Outcome, string? MessageId = null, TimeSpan Wait = default, string? Error = null);

/// <summary>Posts to the webhook the person pasted in Settings › Discord: builds the message within Discord's limits, with
/// <c>allowed_mentions</c> naming exactly who may be pinged. Only discord.com is ever contacted, and the link (its token is
/// the only key to the webhook) never reaches a log or an error text.</summary>
public static partial class DiscordWebhook
{
    // Discord's limits (docs.discord.com, Resources › Message / Webhook).
    public const int MaxEmbeds = 10, MaxContent = 2000, MaxTitle = 256, MaxDescription = 4096, MaxFields = 25, MaxFieldName = 256,
        MaxFieldValue = 1024, MaxFooter = 2048, MaxEmbedTotal = 6000, MaxUsername = 80;
    /// <summary>Discord's own colours, by what a message means: news, good or done, needs a look, broken or blocked, nothing to do.</summary>
    public const int Blue = 0x5865F2, Green = 0x57F287, Yellow = 0xFEE75C, Red = 0xED4245, Grey = 0x4E5058;
    /// <summary>The sender's name and picture when the person set none, and the picture beside every footer.</summary>
    public const string DefaultName = "Bloxkeep", Logo = "https://raw.githubusercontent.com/Bloxkeep/roblox-account-manager/main/docs/brand/bloxkeep.png";

    static readonly HttpClient Http = new(new SocketsHttpHandler
    {
        UseCookies = false,
        AllowAutoRedirect = false,
        PooledConnectionLifetime = TimeSpan.FromMinutes(5),
    })
    {
        Timeout = TimeSpan.FromSeconds(15),
    };

    // discord.com, discordapp.com and their canary./ptb. hosts, an optional API version, then the webhook's id and token.
    [GeneratedRegex(@"^https://(?:(?:canary|ptb)\.)?discord(?:app)?\.com/api(?:/v\d{1,2})?/webhooks/(\d{17,20})/([A-Za-z0-9_\-]{50,100})/?$")]
    private static partial Regex WebhookPattern();

    [GeneratedRegex(@"^\d{17,20}$")]
    private static partial Regex IdPattern();

    /// <summary>A user or role id: 17–20 digits (what Discord's "Copy ID" gives).</summary>
    public static bool IsId(string? text) => text is not null && IdPattern().IsMatch(text);

    /// <summary>A pasted webhook link, accepted only in Discord's own form and sent to discord.com whatever host it named.</summary>
    public static bool TryNormalize(string? url, out string normalized)
    {
        normalized = "";
        var m = WebhookPattern().Match((url ?? "").Trim());
        if (!m.Success)
            return false;
        normalized = $"https://discord.com/api/webhooks/{m.Groups[1].Value}/{m.Groups[2].Value}";
        return true;
    }

    /// <summary>What the page shows of a saved link: enough to tell two webhooks apart, never the token.</summary>
    public static string Mask(string url)
    {
        var m = WebhookPattern().Match(url);
        return m.Success ? $"discord.com/api/webhooks/…{m.Groups[1].Value[^4..]}/••••••" : "";
    }

    /// <summary>Names and game titles show as typed: Discord's formatting characters are escaped, so "@everyone" or
    /// "&lt;@123&gt;" shows as text (allowed_mentions already keeps it from pinging).</summary>
    public static string Escape(string? text) =>
        Regex.Replace(text ?? "", @"[\\*_~`|>\[\]<@]", @"\$0");

    /// <summary>A game's name (escaped) linking to its page on Roblox; plain text shows the name only.</summary>
    public static string GameLink(string name, long? placeId) =>
        placeId is > 0 ? $"[{Escape(name)}](https://www.roblox.com/games/{placeId})" : Escape(name);

    /// <summary>A picture from Roblox's image servers (an avatar, a game's icon): the only kind a message shows besides the logo.</summary>
    public static bool IsRobloxPicture(string? url) =>
        Uri.TryCreate(url, UriKind.Absolute, out var uri) && uri.Scheme == Uri.UriSchemeHttps
        && uri.Host.EndsWith(".rbxcdn.com", StringComparison.OrdinalIgnoreCase);

    /// <summary>Cut to at most <paramref name="max"/> UTF-16 units (Discord counts no fewer), ending with "…" when cut.</summary>
    public static string Cut(string? text, int max)
    {
        text ??= "";
        if (text.Length <= max)
            return text;
        int end = Math.Max(0, max - 1);
        if (end > 0 && char.IsHighSurrogate(text[end - 1]))
            end--; // never half an emoji
        return text[..end] + "…";
    }

    /// <summary>The JSON Discord gets: an embed (or plain text) within every limit, pings first in <c>content</c> and
    /// <c>allowed_mentions</c> listing exactly those, so "@everyone" inside the text pings nobody.</summary>
    /// <param name="picture">The name of a picture sent with it (<see cref="DiscordFile"/>): the first embed shows it.</param>
    public static JsonObject Build(DiscordMessage message, DiscordPing ping, bool embed, string? username, string? avatarUrl, string? picture = null)
    {
        var mentions = ping.Users.Select(u => $"<@{u}>").Concat(ping.Roles.Select(r => $"<@&{r}>")).ToList();
        if (ping.Here)
            mentions.Add("@here");
        if (ping.Everyone)
            mentions.Add("@everyone");
        string pings = string.Join(' ', mentions);
        var payload = new JsonObject
        {
            ["username"] = Cut(string.IsNullOrWhiteSpace(username) ? DefaultName : username.Trim(), MaxUsername),
            ["avatar_url"] = string.IsNullOrWhiteSpace(avatarUrl) ? Logo : avatarUrl.Trim(),
        };
        if (embed)
        {
            if (pings.Length > 0)
                payload["content"] = pings;
            // Up to 10 embeds and 6000 characters across them all: the last ones go first.
            var embeds = new List<JsonObject> { Embed(message, picture) };
            embeds.AddRange(message.More.Take(MaxEmbeds - 1).Select(m => Embed(m, null)));
            while (embeds.Count > 1 && embeds.Sum(Length) > MaxEmbedTotal)
                embeds.RemoveAt(embeds.Count - 1);
            payload["embeds"] = new JsonArray(embeds.Select(e => (JsonNode)e).ToArray());
        }
        else
        {
            string body = string.Join("\n\n", new[] { message }.Concat(message.More).Select(PlainText));
            payload["content"] = pings.Length > 0 ? pings + "\n" + Cut(body, MaxContent - pings.Length - 1) : Cut(body, MaxContent);
        }
        var allowed = new JsonObject { ["parse"] = ping.Here || ping.Everyone ? new JsonArray("everyone") : new JsonArray() };
        if (ping.Users.Count > 0)
            allowed["users"] = new JsonArray(ping.Users.Take(100).Select(u => (JsonNode)u!).ToArray());
        if (ping.Roles.Count > 0)
            allowed["roles"] = new JsonArray(ping.Roles.Take(100).Select(r => (JsonNode)r!).ToArray());
        payload["allowed_mentions"] = allowed;
        if (picture is not null)
            payload["attachments"] = new JsonArray(new JsonObject { ["id"] = 0, ["filename"] = picture });
        return payload;
    }

    /// <summary>The characters Discord counts towards an embed's 6000.</summary>
    static int Length(JsonObject e) =>
        Str(e, "title") + Str(e["author"] as JsonObject, "name") + Str(e, "description") + Str(e["footer"] as JsonObject, "text")
        + (e["fields"] as JsonArray ?? []).OfType<JsonObject>().Sum(f => Str(f, "name") + Str(f, "value"));

    static int Str(JsonObject? o, string name) => o?[name]?.GetValue<string>().Length ?? 0;

    static JsonObject Embed(DiscordMessage m, string? picture)
    {
        string title = Cut(m.Title, MaxTitle), description = Cut(m.Text, MaxDescription), footer = Cut(m.Footer, MaxFooter);
        var fields = m.Fields.Select(f => new DiscordField(Cut(Blank(f.Name), MaxFieldName), Cut(Blank(f.Value), MaxFieldValue), f.Inline)).ToList();
        if (fields.Count > MaxFields)
        {
            int more = fields.Count - (MaxFields - 1);
            fields = fields.Take(MaxFields - 1).Append(new DiscordField("…", $"+{more}")).ToList();
        }
        // 6000 characters over the whole embed: the last fields go first, then the description is shortened.
        int Total() => title.Length + description.Length + footer.Length + fields.Sum(f => f.Name.Length + f.Value.Length);
        while (Total() > MaxEmbedTotal && fields.Count > 0)
            fields.RemoveAt(fields.Count - 1);
        if (Total() > MaxEmbedTotal)
            description = Cut(description, Math.Max(0, MaxEmbedTotal - (Total() - description.Length)));
        var embed = new JsonObject { ["title"] = title, ["color"] = m.Color };
        if (!string.IsNullOrEmpty(m.Author))
        {
            var author = new JsonObject { ["name"] = Cut(m.Author, MaxTitle) };
            if (Https(m.AuthorIcon))
                author["icon_url"] = m.AuthorIcon;
            embed["author"] = author;
        }
        if (Https(m.Thumbnail))
            embed["thumbnail"] = new JsonObject { ["url"] = m.Thumbnail };
        if (picture is not null)
            embed["image"] = new JsonObject { ["url"] = "attachment://" + picture };
        if (description.Length > 0)
            embed["description"] = description;
        if (fields.Count > 0)
            embed["fields"] = new JsonArray(fields.Select(f => (JsonNode)new JsonObject { ["name"] = f.Name, ["value"] = f.Value, ["inline"] = f.Inline }).ToArray());
        if (footer.Length > 0)
            embed["footer"] = new JsonObject { ["text"] = footer, ["icon_url"] = Logo };
        if (m.Timestamp is { } at)
            embed["timestamp"] = at.ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ss.fffZ");
        return embed;
    }

    /// <summary>Discord refuses an empty field name or value.</summary>
    static string Blank(string text) => string.IsNullOrWhiteSpace(text) ? "​" : text;

    static bool Https(string? url) =>
        url is { Length: > 0 and <= 2048 } && Uri.TryCreate(url, UriKind.Absolute, out var uri) && uri.Scheme == Uri.UriSchemeHttps;

    // [text](https://…) → text: a link in plain text would make Discord add a preview of the page under the message.
    [GeneratedRegex(@"\[((?:\\.|[^\]\\])*)\]\(https://[^)\s]+\)")]
    private static partial Regex LinkPattern();

    /// <summary>The embed as plain text: account and title in bold, the description, a line per short fact ("**Game:** …",
    /// several lines joined with " · "), a list under its name, the footer in small text.</summary>
    static string PlainText(DiscordMessage m)
    {
        var sb = new StringBuilder("**");
        if (!string.IsNullOrEmpty(m.Author))
            sb.Append(Escape(m.Author)).Append(" · ");
        sb.Append(m.Title).Append("**");
        if (m.Text.Length > 0)
            sb.Append('\n').Append(m.Text);
        foreach (var f in m.Fields)
        {
            if (f.Inline || !f.Value.Contains('\n'))
                sb.Append("\n**").Append(f.Name).Append(":** ").Append(string.Join(" · ", f.Value.Split('\n', StringSplitOptions.RemoveEmptyEntries)));
            else
                sb.Append("\n**").Append(f.Name).Append("**\n").Append(f.Value);
        }
        if (!string.IsNullOrEmpty(m.Footer))
            sb.Append("\n-# ").Append(m.Footer);
        return LinkPattern().Replace(sb.ToString(), "$1");
    }

    /// <summary>Posts a new message (<paramref name="wait"/>: Discord answers with it, for its id) or edits one this webhook
    /// sent. Never throws for Discord's or the network's answers: the outcome says what happened.</summary>
    /// <param name="file">Sent with a new message (a summary's picture), as multipart form data.</param>
    public static async Task<DiscordResult> SendAsync(string url, JsonObject payload, string? editMessageId, bool wait, CancellationToken ct = default, DiscordFile? file = null)
    {
        bool edit = !string.IsNullOrEmpty(editMessageId);
        if (!TryNormalize(url, out string target) || (edit && !IsId(editMessageId)))
            return new DiscordResult(DiscordOutcome.Gone);
        // An edit changes only the fields it names: the other form (embed or text) is cleared, or a switch would leave both.
        if (edit)
        {
            payload["content"] ??= "";
            payload["embeds"] ??= new JsonArray();
        }
        HttpContent content = new StringContent(payload.ToJsonString(), Encoding.UTF8, "application/json");
        if (file is not null && !edit)
        {
            var form = new MultipartFormDataContent { { content, "payload_json" } };
            var data = new ByteArrayContent(file.Data);
            data.Headers.ContentType = new System.Net.Http.Headers.MediaTypeHeaderValue("image/png");
            form.Add(data, "files[0]", file.Name);
            content = form;
        }
        using var req = new HttpRequestMessage(edit ? HttpMethod.Patch : HttpMethod.Post,
            edit ? $"{target}/messages/{editMessageId}" : wait ? target + "?wait=true" : target)
        {
            Content = content,
        };
        req.Headers.TryAddWithoutValidation("User-Agent", "Bloxkeep");
        try
        {
            using var resp = await Http.SendAsync(req, ct);
            string body = await resp.Content.ReadAsStringAsync(ct);
            JsonElement json = default;
            try
            {
                if (body.Length > 0)
                {
                    using var doc = JsonDocument.Parse(body);
                    json = doc.RootElement.Clone();
                }
            }
            catch (JsonException)
            {
                // An error page from a proxy: the status code says enough.
            }
            int code = json.ValueKind == JsonValueKind.Object && json.TryGetProperty("code", out var c) && c.TryGetInt32(out int n) ? n : 0;
            string? reason = json.ValueKind == JsonValueKind.Object && json.TryGetProperty("message", out var msg) && msg.ValueKind == JsonValueKind.String ? msg.GetString() : null;
            switch ((int)resp.StatusCode)
            {
                case >= 200 and < 300:
                    string? id = json.ValueKind == JsonValueKind.Object && json.TryGetProperty("id", out var idEl) && idEl.ValueKind == JsonValueKind.String ? idEl.GetString() : null;
                    // The bucket is empty after this one: the next post waits for it rather than earning a 429.
                    var refill = Header(resp, "X-RateLimit-Remaining") == "0" && double.TryParse(Header(resp, "X-RateLimit-Reset-After"), System.Globalization.CultureInfo.InvariantCulture, out double after)
                        ? TimeSpan.FromSeconds(Math.Clamp(after, 0, 600)) : TimeSpan.Zero;
                    return new DiscordResult(DiscordOutcome.Sent, id ?? editMessageId, refill);
                case 429:
                    double seconds = json.ValueKind == JsonValueKind.Object && json.TryGetProperty("retry_after", out var ra) && ra.TryGetDouble(out double s) ? s
                        : double.TryParse(Header(resp, "Retry-After"), System.Globalization.CultureInfo.InvariantCulture, out double h) ? h : 5;
                    return new DiscordResult(DiscordOutcome.RateLimited, Wait: TimeSpan.FromSeconds(Math.Clamp(seconds, 1, 3600)));
                // Discord's own word that the webhook is gone: 50027 Invalid Webhook Token (its link was reset), 10015
                // Unknown Webhook. Any other 401/403 (a proxy, a firewall) is a connection problem, tried again later.
                case 401 or 403:
                    return code is 50027 or 10015
                        ? new DiscordResult(DiscordOutcome.Gone, Error: reason)
                        : new DiscordResult(DiscordOutcome.Offline, Error: $"HTTP {(int)resp.StatusCode}");
                case 404:
                    // 10008 Unknown Message: the board message was deleted, the webhook is fine. 10015 Unknown Webhook.
                    return new DiscordResult(code == 10008 || (edit && code != 10015) ? DiscordOutcome.MessageGone : DiscordOutcome.Gone, Error: reason);
                case >= 500:
                    return new DiscordResult(DiscordOutcome.Offline, Error: $"HTTP {(int)resp.StatusCode}");
                default:
                    return new DiscordResult(DiscordOutcome.Rejected, Error: Cut(reason ?? $"HTTP {(int)resp.StatusCode}", 200));
            }
        }
        catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException or IOException)
        {
            // HttpClient's messages name the host at most ("discord.com:443"), never the path with the token. They are in
            // Windows' language and say little to the person, so they go to the log; the caller says it plainly (no Error).
            Log.Note($"Discord: {ex.GetType().Name}: {ex.Message}");
            return new DiscordResult(DiscordOutcome.Offline);
        }
    }

    static string? Header(HttpResponseMessage resp, string name) =>
        resp.Headers.TryGetValues(name, out var values) ? values.FirstOrDefault() : null;
}
