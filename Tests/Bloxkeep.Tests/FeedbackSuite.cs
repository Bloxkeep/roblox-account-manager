using System.Net;
using System.Text.Json;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// Send feedback: what leaves the PC (the preview and the request match, the error log without account names, ids,
/// passwords, cookies, Discord links, e-mail addresses or the Windows user), and the refusals (no text, the log while
/// locked, twice in a minute, a busy or unreachable service). Sent to a little server of the test's own, never the
/// real one; and the real address must be set before a build goes out.
/// </summary>
static class FeedbackSuite
{
    public static async Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        var account = new Account
        {
            Username = "NoobMaster99", DisplayName = "Noobie", Alias = "Farm one", UserId = 4815162342, Password = "hunter2pass",
            Cookie = "_|WARNING:-DO-NOT-SHARE-THIS.--Sharing-this-will-allow-someone-to-log-in-as-you.|_ABC123", CookieValid = true,
        };
        var seed = Vault.Create(Harness.Password);
        seed.Data.Accounts.Add(account);
        seed.Save();
        string user = Environment.UserName;
        File.WriteAllText(Path.Combine(dir, "error.log"), string.Join(Environment.NewLine,
            "[2026-10-16 02:10:44] System.Net.Http.HttpRequestException: 401 for NoobMaster99 (noobmaster99) / Noobie, user 4815162342",
            $"   at RobloxManager.RobloxApi.GetPresenceAsync() in C:\\Users\\{user}\\Bloxkeep\\RobloxApi.cs:line 512",
            "   cookie _|WARNING:-DO-NOT-SHARE-THIS.--other|_XYZ, password hunter2pass, Farm one, Farm ones",
            "   webhook https://discord.com/api/webhooks/123/abcDEF and mail someone@example.com",
            "   token " + new string('Q', 80)));

        using var form = new Form();
        using var app = new AppController(form, _ => { });
        object Args(string text, bool log = true, string kind = "bug") => new { kind, text, contact = "me#1", log };

        // Locked: the log can't go (the names to take out aren't known), the rest can.
        var locked = await Harness.Try(app, "feedbackPreview", Args("The mini window forgets its size."));
        c.Check("locked: the log is refused", locked is UserError, locked);
        await Harness.Unlock(app);

        string preview = Text(await Harness.Call(app, "feedbackPreview", Args("The mini window forgets its size.")), "preview");
        string[] secrets = ["NoobMaster99", "noobmaster99", "Noobie", "4815162342", "hunter2pass", "ABC123", "_|WARNING", "abcDEF", "someone@example.com", new string('Q', 80)];
        var leaked = secrets.Where(s => preview.Contains(s, StringComparison.Ordinal)).ToList();
        if (user.Length >= 3 && preview.Contains($"\\{user}\\", StringComparison.OrdinalIgnoreCase))
            leaked.Add("the Windows user");
        c.Check("preview: nothing that tells who it is", leaked.Count == 0, string.Join(", ", leaked));
        c.Check("preview: the marks in their place", preview.Contains("[account]") && preview.Contains("[account id]") && preview.Contains("[password]")
            && preview.Contains("[cookie]") && preview.Contains("[webhook]") && preview.Contains("[email]") && preview.Contains("[token]"), preview);
        c.Check("preview: a name inside a longer word is left alone", preview.Contains("Farm ones"), preview);
        c.Check("preview: the message, contact and version", preview.Contains("The mini window forgets its size.") && preview.Contains("me#1") && preview.Contains("Bloxkeep "));
        string noLog = Text(await Harness.Call(app, "feedbackPreview", Args("An idea for presets.", log: false, kind: "idea")), "preview");
        c.Check("no log asked: none in the preview", !noLog.Contains("error-log") && !noLog.Contains("NoobMaster99"), noLog);
        // An emoji across the length limits (2026-10-30 review): left out whole, never cut in half (that can't go as JSON).
        var cut = Harness.Invoke(app, "ReadFeedback", JsonSerializer.SerializeToElement(new
        {
            kind = "idea", text = new string('a', 1999) + "\U0001F600", contact = new string('b', 99) + "\U0001F600", log = false,
        }))!;
        string cutText = (string)cut.GetType().GetProperty("Text")!.GetValue(cut)!, cutContact = (string)cut.GetType().GetProperty("Contact")!.GetValue(cut)!;
        c.Check("an emoji at the text's and the contact's limit is left out whole", cutText.Length == 1999 && cutContact.Length == 99,
            $"{cutText.Length} {cutContact.Length}");

        // The service: a little server here that answers what each step needs.
        var (listener, url) = Listen();
        Environment.SetEnvironmentVariable("BLOXKEEP_FEEDBACK_URL", url);
        var received = new List<JsonDocument>();
        int answer = 200;
        var serving = Task.Run(async () =>
        {
            while (listener.IsListening)
            {
                HttpListenerContext ctx;
                try { ctx = await listener.GetContextAsync(); }
                catch (Exception) { break; }
                using var reader = new StreamReader(ctx.Request.InputStream);
                received.Add(JsonDocument.Parse(await reader.ReadToEndAsync()));
                ctx.Response.StatusCode = answer;
                ctx.Response.Close();
            }
        });

        var tooShort = await Harness.Try(app, "sendFeedback", Args("ok"));
        c.Check("too short: refused before sending", tooShort is UserError && received.Count == 0, tooShort);
        answer = 429;
        var busy = await Harness.Try(app, "sendFeedback", Args("The mini window forgets its size."));
        c.Check("a busy service: said so", busy is UserError e429 && e429.Message.Contains("minute"), busy);
        answer = 200;
        var sent = await Harness.Try(app, "sendFeedback", Args("The mini window forgets its size."));
        c.Check("sent", sent is not UserError && received.Count == 2, sent);
        if (received.Count == 2)
        {
            var body = received[1].RootElement;
            string log = body.GetProperty("log").GetString() ?? "";
            c.Check("sent: kind, text, contact, version, Windows, language",
                body.GetProperty("kind").GetString() == "bug" && body.GetProperty("text").GetString() == "The mini window forgets its size."
                && body.GetProperty("contact").GetString() == "me#1" && body.GetProperty("app").GetString()!.Length > 0
                && body.GetProperty("windows").GetString()!.StartsWith("Windows") && body.GetProperty("language").GetString() is "en" or "vi", body);
            c.Check("sent: the same log as the preview", log.Length > 0 && preview.Contains(log.Trim().Split('\n')[0].Trim()) && !secrets.Any(log.Contains));
        }
        var again = await Harness.Try(app, "sendFeedback", Args("And another thing."));
        c.Check("twice in a minute: refused", again is UserError && received.Count == 2, again);

        listener.Stop();
        await serving;
        Harness.Set(app, "_feedbackSentAt", DateTime.MinValue);
        var down = await Harness.Try(app, "sendFeedback", Args("The mini window forgets its size."));
        c.Check("service unreachable: said so", down is UserError, down);
        Environment.SetEnvironmentVariable("BLOXKEEP_FEEDBACK_URL", null);

        // The real address: a build mustn't go out before it's set (services/Feedback/worker's workers.dev address).
        string real = (string)typeof(AppController).GetField("FeedbackUrl", Harness.F)!.GetValue(null)!;
        c.Check("the feedback service's address is set", Uri.TryCreate(real, UriKind.Absolute, out var uri) && uri.Scheme == "https"
            && uri.Host.EndsWith(".workers.dev") && !uri.Host.Contains("subdomain", StringComparison.OrdinalIgnoreCase) && uri.AbsolutePath == "/feedback", real);
    }

    static string Text(object? dto, string name) => JsonSerializer.SerializeToElement(dto).GetProperty(name).GetString() ?? "";

    /// <summary>An HttpListener on a free port of this PC only.</summary>
    static (HttpListener Listener, string Url) Listen()
    {
        for (int port = 48731; ; port++)
        {
            var listener = new HttpListener();
            string url = $"http://127.0.0.1:{port}/feedback/";
            listener.Prefixes.Add(url);
            try
            {
                listener.Start();
                return (listener, url);
            }
            catch (HttpListenerException)
            {
                listener.Close();
            }
        }
    }
}
