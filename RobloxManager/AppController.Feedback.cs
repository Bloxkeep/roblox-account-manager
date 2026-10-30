using System.Globalization;
using System.Net;
using System.Text;
using System.Text.Json;
using System.Text.RegularExpressions;

namespace RobloxManager;

/// <summary>
/// Settings › Bloxkeep › Send feedback: an idea or bug report with Bloxkeep's version, Windows' and the language, and on
/// request the end of error.log, scrubbed first (<see cref="Scrub"/>); Preview shows exactly what goes. A Cloudflare
/// Worker (services/Feedback/worker) posts it to the developer's Discord channel and keeps that webhook to itself.
/// </summary>
public sealed partial class AppController
{
    /// <summary>The feedback Worker (BLOXKEEP_FEEDBACK_URL points elsewhere: the checks' own little server).</summary>
    const string FeedbackUrl = "https://bloxkeep-feedback.bloxkeep-app.workers.dev/feedback";
    const int FeedbackTextMax = 2000, FeedbackContactMax = 100, FeedbackLogMax = 12_000;
    static readonly TimeSpan FeedbackGap = TimeSpan.FromSeconds(60);
    static readonly HttpClient FeedbackHttp = new(new SocketsHttpHandler { UseCookies = false, AllowAutoRedirect = false })
    {
        Timeout = TimeSpan.FromSeconds(20),
    };
    static readonly Regex CookieText = new(@"_\|WARNING:-DO-NOT-SHARE-THIS[^\s""']*", RegexOptions.CultureInvariant);
    static readonly Regex WebhookText = new(@"https?://(?:[a-z]+\.)?discord(?:app)?\.com/api/webhooks/\S+", RegexOptions.IgnoreCase | RegexOptions.CultureInvariant);
    static readonly Regex EmailText = new(@"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}", RegexOptions.CultureInvariant);
    static readonly Regex LongToken = new(@"[A-Za-z0-9+/=_-]{60,}", RegexOptions.CultureInvariant);

    DateTime _feedbackSentAt = DateTime.MinValue;

    sealed record Feedback(string Kind, string Text, string Contact, string App, string Windows, string Language, string? Log);

    Feedback ReadFeedback(JsonElement a)
    {
        string kind = Str(a, "kind") == "bug" ? "bug" : "idea";
        string text = Str(a, "text").Trim();
        string contact = Str(a, "contact").Trim();
        string? log = null;
        if (Bool(a, "log"))
        {
            // Account names are only known once the vault is open: without them the log can't go.
            if (_vault is null)
                throw new UserError(L.T("Unlock Bloxkeep to add the error log: account names are taken out of it first."));
            log = ScrubbedLog();
        }
        // Cut between whole characters: half an emoji at the limit made the request's JSON fail (2026-10-30 review).
        return new Feedback(kind, CutText(text, FeedbackTextMax), CutText(contact, FeedbackContactMax),
            _updater.CurrentVersion, WindowsText(), L.Language, log);
    }

    static string WindowsText()
    {
        var v = Environment.OSVersion.Version;
        string name = v.Major == 10 ? (v.Build >= 22000 ? "11" : "10") : v.ToString(2);
        return $"Windows {name} ({v.Major}.{v.Minor}.{v.Build})";
    }

    /// <summary>The end of error.log (whole lines) with what could tell who someone is taken out; null when there's none.</summary>
    string? ScrubbedLog()
    {
        string text;
        try
        {
            string path = Path.Combine(Vault.DataDir, "error.log");
            if (!File.Exists(path))
                return null;
            text = File.ReadAllText(path);
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            return null;
        }
        if (text.Length > FeedbackLogMax)
        {
            text = text[^FeedbackLogMax..];
            int lineEnd = text.IndexOf('\n');
            if (lineEnd >= 0)
                text = text[(lineEnd + 1)..];
        }
        return text.Trim().Length == 0 ? null : Scrub(text);
    }

    /// <summary>Takes out what could tell whose a text is: each account's password, cookie, names and id, any cookies and
    /// Discord links, e-mail addresses, long tokens, and the Windows user and PC names.</summary>
    string Scrub(string text)
    {
        var words = new List<(string Word, string Mark)>();
        void Add(string? word, string mark)
        {
            if (word is { Length: >= 3 })
                words.Add((word, mark));
        }
        foreach (var account in _vault?.Data.Accounts ?? [])
        {
            Add(account.Password, "[password]");
            Add(account.Cookie, "[cookie]");
            Add(account.Username, "[account]");
            Add(account.DisplayName, "[account]");
            Add(account.Alias, "[account]");
            if (account.UserId > 0)
                Add(account.UserId.ToString(CultureInfo.InvariantCulture), "[account id]");
        }
        Add(Environment.UserName, "[user]");
        Add(Environment.MachineName, "[pc]");
        text = CookieText.Replace(text, "[cookie]");
        text = WebhookText.Replace(text, "[webhook]");
        text = EmailText.Replace(text, "[email]");
        // Longest first, so a name that holds a shorter one isn't left half replaced.
        foreach (var (word, mark) in words.OrderByDescending(w => w.Word.Length))
        {
            text = Regex.Replace(text, $@"(?<![\p{{L}}\p{{N}}_]){Regex.Escape(word)}(?![\p{{L}}\p{{N}}_])", mark,
                RegexOptions.IgnoreCase | RegexOptions.CultureInvariant);
        }
        return LongToken.Replace(text, "[token]");
    }

    /// <summary>Exactly what Send would send, as the person reads it.</summary>
    object FeedbackPreview(JsonElement a)
    {
        var f = ReadFeedback(a);
        var lines = new StringBuilder();
        lines.AppendLine(f.Kind == "bug" ? L.T("Bug report") : L.T("Idea"));
        lines.AppendLine($"Bloxkeep {f.App} · {f.Windows} · {(f.Language == "vi" ? "Tiếng Việt" : "English")}");
        if (f.Contact.Length > 0)
            lines.AppendLine(L.F("Contact: {0}", f.Contact));
        lines.AppendLine().AppendLine(f.Text.Length > 0 ? f.Text : "…");
        lines.AppendLine().AppendLine(f.Log is null ? L.T("(no error log)") : $"── error-log.txt ──{Environment.NewLine}{f.Log.TrimEnd()}");
        return new { preview = lines.ToString().TrimEnd() };
    }

    async Task<object?> SendFeedbackAsync(JsonElement a)
    {
        var f = ReadFeedback(a);
        if (f.Text.Length < 3)
            throw new UserError(L.T("Write a few words first."));
        if (DateTime.UtcNow - _feedbackSentAt < FeedbackGap)
            throw new UserError(L.T("Feedback was just sent. Wait a minute before sending more."));
        string url = Environment.GetEnvironmentVariable("BLOXKEEP_FEEDBACK_URL") is { Length: > 0 } other ? other : FeedbackUrl;
        string body = JsonSerializer.Serialize(new { kind = f.Kind, text = f.Text, contact = f.Contact, app = f.App, windows = f.Windows, language = f.Language, log = f.Log });
        using var content = new StringContent(body, Encoding.UTF8, "application/json");
        HttpResponseMessage res;
        try
        {
            res = await FeedbackHttp.PostAsync(url, content);
        }
        catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException)
        {
            throw new UserError(L.T("Couldn't reach the feedback service. Check the internet connection and try again."));
        }
        using (res)
        {
            if (res.StatusCode == HttpStatusCode.TooManyRequests)
                throw new UserError(L.T("A lot of feedback is arriving right now. Try again in a minute."));
            if (!res.IsSuccessStatusCode)
                throw new UserError(L.F("Couldn't send it: the feedback service answered {0}. Try again later.", (int)res.StatusCode));
        }
        _feedbackSentAt = DateTime.UtcNow;
        return new { sent = true };
    }
}
