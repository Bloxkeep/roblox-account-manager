using System.Net.Http.Headers;
using System.Security.Cryptography;
using System.Text;
using System.Text.RegularExpressions;

namespace RobloxManager;

/// <summary>
/// Alerts on the phone through ntfy (ntfy.sh, a free app for Android and iPhone, no account): Bloxkeep posts to a "topic"
/// the phone subscribes to. Anyone who knows a topic's name can read it, so Bloxkeep makes a long random one. A person
/// with their own ntfy server pastes its https address with the topic instead.
/// </summary>
static class PhoneAlert
{
    const string Server = "https://ntfy.sh/";
    static readonly Regex Topic = new("^[A-Za-z0-9_-]{6,64}$", RegexOptions.CultureInvariant);
    static readonly HttpClient Http = new(new SocketsHttpHandler { UseCookies = false, AllowAutoRedirect = false })
    {
        Timeout = TimeSpan.FromSeconds(10),
    };

    /// <summary>A new topic no one could guess: "bloxkeep-" and 16 letters and digits.</summary>
    public static string NewTopic()
    {
        const string chars = "abcdefghijkmnpqrstuvwxyz23456789";
        return "bloxkeep-" + string.Concat(Enumerable.Range(0, 16).Select(_ => chars[RandomNumberGenerator.GetInt32(chars.Length)]));
    }

    /// <summary>A topic's name, or https://server/topic, kept as given (trimmed); false for anything else.</summary>
    public static bool TryNormalize(string text, out string topic)
    {
        topic = text.Trim();
        if (Topic.IsMatch(topic))
            return true;
        return Uri.TryCreate(topic, UriKind.Absolute, out var uri) && uri.Scheme == Uri.UriSchemeHttps && string.IsNullOrEmpty(uri.Query)
            && Topic.IsMatch(uri.AbsolutePath.Trim('/'));
    }

    /// <summary>Where a topic's messages are posted.</summary>
    public static string UrlOf(string topic) => topic.StartsWith("https://", StringComparison.OrdinalIgnoreCase) ? topic : Server + topic;

    /// <summary>Posts one alert; null when sent, else why not (for Settings to show).</summary>
    public static async Task<string?> SendAsync(string topic, string text, bool urgent, CancellationToken ct = default)
    {
        using var req = new HttpRequestMessage(HttpMethod.Post, UrlOf(topic))
        {
            Content = new StringContent(text.Length > 1000 ? text[..1000] : text, Encoding.UTF8, "text/plain"),
        };
        req.Headers.UserAgent.Add(new ProductInfoHeaderValue("Bloxkeep", "1"));
        req.Headers.Add("Title", "Bloxkeep");
        req.Headers.Add("Tags", urgent ? "warning" : "video_game");
        if (urgent)
            req.Headers.Add("Priority", "high");
        try
        {
            using var res = await Http.SendAsync(req, ct);
            if (res.IsSuccessStatusCode)
                return null;
            Log.Note($"Phone alert: HTTP {(int)res.StatusCode}.");
            return (int)res.StatusCode == 429 ? L.T("ntfy says too many alerts went out. They go again in a while.")
                : L.F("ntfy didn't take the alert (error {0}).", (int)res.StatusCode);
        }
        catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException or IOException)
        {
            Log.Note($"Phone alert: no answer ({ex.GetType().Name}).");
            return L.T("ntfy didn't answer. Check the internet connection.");
        }
    }
}
