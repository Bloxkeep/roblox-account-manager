using System.Net;
using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;
using System.Text.RegularExpressions;

namespace RobloxManager;

/// <summary>What a message says, before it is shaped: the same content goes out as an embed or as plain text.</summary>
/// <param name="Timestamp">Shown by Discord under an embed, in each reader's own time zone.</param>
public sealed record DiscordMessage(string Title, string Text, IReadOnlyList<DiscordField> Fields, string? Footer = null, int Color = DiscordWebhook.Blue, DateTime? Timestamp = null);

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

/// <summary>
/// Posts to a Discord webhook the person pasted in Settings › Discord: builds the message within Discord's limits,
/// with <c>allowed_mentions</c> naming exactly who may be pinged, and reports how the post went. Only discord.com is
/// ever contacted, and the link (whose token is the only key to the webhook) never reaches a log or an error text.
/// </summary>
public static partial class DiscordWebhook
{
    // Discord's limits (docs.discord.com, Resources › Message / Webhook).
    public const int MaxContent = 2000, MaxTitle = 256, MaxDescription = 4096, MaxFields = 25, MaxFieldName = 256,
        MaxFieldValue = 1024, MaxFooter = 2048, MaxEmbedTotal = 6000, MaxUsername = 80;
    public const int Blue = 0x5865F2, Green = 0x2EA043, Amber = 0xD29922, Red = 0xD1453B, Grey = 0x6E7681;

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

    /// <summary>
    /// Names and game titles are shown as typed: Discord's formatting characters in them are escaped (and a name
    /// like "@everyone" or "&lt;@123&gt;" shows as text; allowed_mentions already keeps it from pinging).
    /// </summary>
    public static string Escape(string? text) =>
        Regex.Replace(text ?? "", @"[\\*_~`|>\[\]<@]", @"\$0");

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

    /// <summary>
    /// The JSON Discord gets: an embed (or plain text) within every limit, the pings first in <c>content</c>, and
    /// <c>allowed_mentions</c> listing exactly those, so a name like "@everyone" inside the text pings nobody.
    /// </summary>
    public static JsonObject Build(DiscordMessage message, DiscordPing ping, bool embed, string? username, string? avatarUrl)
    {
        var mentions = ping.Users.Select(u => $"<@{u}>").Concat(ping.Roles.Select(r => $"<@&{r}>")).ToList();
        if (ping.Here)
            mentions.Add("@here");
        if (ping.Everyone)
            mentions.Add("@everyone");
        string pings = string.Join(' ', mentions);
        var payload = new JsonObject();
        if (!string.IsNullOrWhiteSpace(username))
            payload["username"] = Cut(username.Trim(), MaxUsername);
        if (!string.IsNullOrWhiteSpace(avatarUrl))
            payload["avatar_url"] = avatarUrl.Trim();
        if (embed)
        {
            if (pings.Length > 0)
                payload["content"] = pings;
            payload["embeds"] = new JsonArray(Embed(message));
        }
        else
        {
            string body = PlainText(message);
            payload["content"] = pings.Length > 0 ? pings + "\n" + Cut(body, MaxContent - pings.Length - 1) : Cut(body, MaxContent);
        }
        var allowed = new JsonObject { ["parse"] = ping.Here || ping.Everyone ? new JsonArray("everyone") : new JsonArray() };
        if (ping.Users.Count > 0)
            allowed["users"] = new JsonArray(ping.Users.Take(100).Select(u => (JsonNode)u!).ToArray());
        if (ping.Roles.Count > 0)
            allowed["roles"] = new JsonArray(ping.Roles.Take(100).Select(r => (JsonNode)r!).ToArray());
        payload["allowed_mentions"] = allowed;
        return payload;
    }

    static JsonObject Embed(DiscordMessage m)
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
        if (description.Length > 0)
            embed["description"] = description;
        if (fields.Count > 0)
            embed["fields"] = new JsonArray(fields.Select(f => (JsonNode)new JsonObject { ["name"] = f.Name, ["value"] = f.Value, ["inline"] = f.Inline }).ToArray());
        if (footer.Length > 0)
            embed["footer"] = new JsonObject { ["text"] = footer };
        if (m.Timestamp is { } at)
            embed["timestamp"] = at.ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ss.fffZ");
        return embed;
    }

    /// <summary>Discord refuses an empty field name or value.</summary>
    static string Blank(string text) => string.IsNullOrWhiteSpace(text) ? "​" : text;

    static string PlainText(DiscordMessage m)
    {
        var sb = new StringBuilder();
        sb.Append("**").Append(m.Title).Append("**");
        if (m.Text.Length > 0)
            sb.Append('\n').Append(m.Text);
        foreach (var f in m.Fields)
            sb.Append('\n').Append("**").Append(f.Name).Append("**").Append(f.Value.Contains('\n') ? "\n" : ": ").Append(f.Value);
        if (!string.IsNullOrEmpty(m.Footer))
            sb.Append("\n-# ").Append(m.Footer);
        return sb.ToString();
    }

    /// <summary>
    /// Posts a new message (<paramref name="wait"/>: Discord answers with it, for its id) or edits one this webhook
    /// sent. Never throws for Discord's or the network's answers: the outcome says what happened.
    /// </summary>
    public static async Task<DiscordResult> SendAsync(string url, JsonObject payload, string? editMessageId, bool wait, CancellationToken ct = default)
    {
        bool edit = !string.IsNullOrEmpty(editMessageId);
        if (!TryNormalize(url, out string target) || (edit && !IsId(editMessageId)))
            return new DiscordResult(DiscordOutcome.Gone);
        // An edit changes only the fields it names: the other form (embed or plain text) is cleared, or a switch
        // between them would leave both on the message.
        if (edit)
        {
            payload["content"] ??= "";
            payload["embeds"] ??= new JsonArray();
        }
        using var req = new HttpRequestMessage(edit ? HttpMethod.Patch : HttpMethod.Post,
            edit ? $"{target}/messages/{editMessageId}" : wait ? target + "?wait=true" : target)
        {
            Content = new StringContent(payload.ToJsonString(), Encoding.UTF8, "application/json"),
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
