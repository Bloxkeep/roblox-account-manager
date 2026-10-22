using System.Net;
using System.Text;
using System.Text.Json;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// Saved sign-ins kept up to date as Roblox renews them (AppController.SignIns.cs, RobloxApi's cookie watch): a new
/// .ROBLOSECURITY in the answer to any request made with an account's cookie is saved to that account, on disk at once,
/// but only when Roblox says it signs in that same user; a cookie deleted or already expired, or set in an answer to a
/// request without a cookie, changes nothing. Bloxkeep never renews one on purpose (auth's /v1/session/refresh ends the
/// session it renews). Roblox here is a stand-in answering in this process (RobloxApi.StandIn), with made-up cookies:
/// nothing is asked of Roblox's servers.
/// </summary>
static class SignInsSuite
{
    public static async Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        string mark = Guid.NewGuid().ToString("N")[..10];
        string Cookie(string name) => $"_TEST_{name}_{mark}";
        var roblox = new StandIn();
        typeof(RobloxApi).GetProperty("StandIn", Harness.F)!.SetValue(null, (Func<HttpRequestMessage, Task<HttpResponseMessage>>)roblox.Answer);

        var seed = Vault.Create(Harness.Password);
        Account Add(string name, long userId, bool works = true, Action<Account>? also = null)
        {
            var account = new Account { Username = name, UserId = userId, Cookie = Cookie(name), CookieValid = true };
            also?.Invoke(account);
            seed.Data.Accounts.Add(account);
            if (works)
                roblox.Users[account.Cookie] = userId;
            return account;
        }
        Add("A", 2001);
        Add("B", 2002);
        Add("V", 2003, also: a => a.Realm = Realm.Vng);
        Add("S", 2004, works: false, also: a => a.CookieValid = false); // signed out
        Add("X", 2005, also: a => a.Banned = true);
        Add("C", 2006);
        Add("W", 2007); // open on the website
        Add("D", 2008, works: false); // Roblox ended it, Bloxkeep doesn't know yet
        seed.Save();

        using var form = new Form();
        using var app = new AppController(form, _ => { });
        Harness.Set(app, "_networkUp", true); // as on a PC online, whatever this one is
        await Harness.Unlock(app);
        var vault = Harness.Get<Vault>(app, "_vault");
        Account Of(string name) => vault.Data.Accounts.Single(a => a.Username == name);
        var (a, b, v, s, x, cc, w, d) = (Of("A"), Of("B"), Of("V"), Of("S"), Of("X"), Of("C"), Of("W"), Of("D"));
        // Unlocking asks Roblox for status, avatars and Robux: those answers are let through first.
        await Until(() => Field(app, "_boot") is null && Field(app, "_presenceBusy") is false && Field(app, "_robuxBusy") is false);
        string OnDisk(Account account)
        {
            using var disk = Vault.Open(Harness.Password);
            return disk.Data.Accounts.Single(k => k.Id == account.Id).Cookie;
        }

        // ── A new cookie in an answer ──
        // Whatever the request made with the account's cookie, the new one in its answer is saved: one after another,
        // each answer renewing the cookie the one before saved.
        var asks = new (string What, Func<string, Task> Ask)[]
        {
            ("its Robux", k => RobloxApi.GetRobuxAsync(k, Realm.Global, 2001)),
            ("presence", k => RobloxApi.GetPresenceAsync(k, [2001])),
            ("the sign-in check", k => RobloxApi.GetAuthenticatedUserAsync(k)),
            ("a sign-in ticket to open Roblox", k => RobloxApi.GetAuthTicketAsync(k)),
            ("a join check", k => RobloxApi.JoinCheckAsync(k, 1818, null, Realm.Global)),
        };
        for (int i = 0; i < asks.Length; i++)
        {
            string before = a.Cookie, next = Cookie($"A{i + 2}");
            roblox.Users[next] = 2001;
            roblox.Rotate[before] = next;
            await Quietly(asks[i].Ask(before));
            await Renewals(app);
            c.Check($"a new cookie in the answer to {asks[i].What} is saved to the account", a.Cookie == next, a.Cookie == before ? "kept the old one" : "another");
            if (i > 0)
                continue;
            c.Check("…and on disk at once", OnDisk(a) == next, OnDisk(a) == before ? "the old one on disk" : "another");
            // Only that check asks anything with the new cookie here (the Robux request went with the old one).
            c.Check("…once Roblox said the new one signs in that same user", roblox.AskedWith(next).Any(r => r.Path == "/v1/users/authenticated"),
                string.Join(", ", roblox.AskedWith(next).Select(r => r.Path)));
        }
        c.Check("…and noted as renewed", Renewed(a) is { } at && DateTime.Now - at < TimeSpan.FromMinutes(1), Renewed(a));

        // Never another user's sign-in in an account's place, nor one that doesn't work.
        roblox.Users[Cookie("B-other")] = 2001;
        roblox.Rotate[b.Cookie] = Cookie("B-other");
        await Quietly(RobloxApi.GetRobuxAsync(b.Cookie, Realm.Global, 2002));
        await Renewals(app);
        c.Check("a new cookie that signs in another user is not saved", b.Cookie == Cookie("B") && OnDisk(b) == Cookie("B"));
        string held = b.Cookie;
        roblox.Rotate[held] = Cookie("B-dead");
        await Quietly(RobloxApi.GetRobuxAsync(held, Realm.Global, 2002));
        await Renewals(app);
        c.Check("…nor one Roblox doesn't accept", b.Cookie == held);

        // A cookie deleted (signing out sets an empty one, expired in 1970) or already expired is no renewal, even when
        // it would sign in the account.
        held = b.Cookie;
        roblox.Users[Cookie("B-expired")] = 2002;
        foreach (string header in new[]
                 {
                     ".ROBLOSECURITY=; domain=.roblox.com; expires=Thu, 01-Jan-1970 00:00:00 GMT; path=/; secure; HttpOnly",
                     $".ROBLOSECURITY={Cookie("B-expired")}; domain=.roblox.com; expires=Thu, 01-Jan-1970 00:00:00 GMT; path=/; secure; HttpOnly",
                     $".ROBLOSECURITY={Cookie("B-expired")}; domain=.roblox.com; Max-Age=0; path=/; secure; HttpOnly",
                 })
        {
            roblox.Extra = header;
            await Quietly(RobloxApi.GetRobuxAsync(held, Realm.Global, 2002));
        }
        roblox.Extra = null;
        await Renewals(app);
        c.Check("a cookie deleted or already expired in the answer changes nothing", b.Cookie == held);
        // An answer to a request made without a cookie (a sign-in, a public lookup) renews nobody's.
        held = b.Cookie;
        roblox.Users[Cookie("anon")] = 2002;
        roblox.Extra = $".ROBLOSECURITY={Cookie("anon")}; domain=.roblox.com; path=/; secure; HttpOnly";
        await Quietly(RobloxApi.GetBannedAsync(2002));
        roblox.Extra = null;
        await Renewals(app);
        c.Check("a cookie in the answer to a request without one renews no account", b.Cookie == held);
        // Undone, should a check above have let one through, so the rest start from B's own sign-in.
        b.Cookie = Cookie("B");

        // Roblox VNG: its answer renews its account, checked with Roblox VNG.
        roblox.Users[Cookie("V2")] = 2003;
        roblox.Rotate[v.Cookie] = Cookie("V2");
        await Quietly(RobloxApi.GetRobuxAsync(v.Cookie, Realm.Vng, 2003));
        await Renewals(app);
        c.Check("Roblox VNG's new cookie is saved to its account", v.Cookie == Cookie("V2"));
        c.Check("…once Roblox VNG said whose it is", roblox.AskedWith(Cookie("V2")).Any(r => r.Host == "users.robloxapp.vnggames.com"),
            string.Join(", ", roblox.AskedWith(Cookie("V2")).Select(r => r.Host)));

        // ── Never renewed on purpose ──
        // Roblox's session refresh ends the session it renews: an account pasted from a browser, or shared with another
        // app, would be signed out there. Only what Roblox sends on its own is saved (the owner's choice, 2026-10-22).
        c.Check("Bloxkeep has no way to end a session to renew it (no call to auth's /v1/session/refresh)",
            typeof(RobloxApi).GetMethod("RefreshSessionAsync", Harness.F) is null && typeof(AppController).GetMethod("RenewSignInsIfDue", Harness.F) is null);
        c.Check("…and nothing in this run asked for one", roblox.Asked.All(r => r.Path != "/v1/session/refresh"),
            roblox.Asked.Count(r => r.Path == "/v1/session/refresh"));

        // ── Signed out, or Roblox's sign-in service down ──
        // An outage of Roblox's sign-in service answers 401 to everyone: outside the daily check (which has its own test),
        // an account is marked signed out only when Roblox accepts another account's sign-in at the same time. Before,
        // each call that met an outage marked one more account, and a rejoin turned auto-rejoin off.
        async Task<bool> Confirm(Account who, bool trusted = false) => await (Task<bool>)Harness.Invoke(app, "ConfirmSignedOutAsync", who, CancellationToken.None, trusted)!;
        roblox.Down = true;
        bool markedInOutage = await Confirm(cc);
        c.Check("Roblox's sign-in service rejecting everyone: an account isn't marked signed out", !markedInOutage && cc.CookieValid == true, cc.CookieValid);
        roblox.Down = false;
        bool markedForReal = await Confirm(d);
        c.Check("…one Roblox rejects while it accepts another account: marked signed out", markedForReal && d.CookieValid == false, d.CookieValid);
        // The daily check's word (it ruled an outage out, or trusts its second run): no other account asked.
        roblox.Down = true;
        roblox.Clear();
        bool trustedMark = await Confirm(w, trusted: true);
        c.Check("…the daily check's own confirmation asks no other account", trustedMark && w.CookieValid == false && roblox.Asked.All(r => r.Cookie == w.Cookie),
            string.Join(", ", roblox.Asked.Select(r => r.Cookie == w.Cookie ? "W" : "another")));
        // Nobody else to ask (one account, the rest signed out): no telling, so marked as before.
        var signedIn = vault.Data.Accounts.Where(k => k != cc && k.CookieValid != false).ToList();
        foreach (var k in signedIn)
            k.CookieValid = false;
        bool alone = await Confirm(cc);
        foreach (var k in signedIn)
            k.CookieValid = true;
        c.Check("…with no other account to ask: marked as before", alone && cc.CookieValid == false, cc.CookieValid);
        // The account menu's check in an outage: nobody accepted, nothing marked; not checked rather than signed out.
        var both = await Harness.Call(app, "checkAccounts", new { ids = new[] { a.Id, b.Id } });
        c.Check("the account menu's check while Roblox's sign-in service rejects everyone: nobody marked signed out",
            a.CookieValid == true && b.CookieValid == true && both?.GetType().GetProperty("bad")?.GetValue(both) is 0, both);
        roblox.Down = false;
        // Signed in again: an account marked signed out that Roblox accepts again is signed in, auto-rejoin back on.
        cc.AutoRejoin = false;
        cc.RejoinOffBySignOut = true;
        await Harness.Call(app, "checkAccounts", new { ids = new[] { cc.Id } });
        c.Check("the account menu's check: an account marked signed out that Roblox accepts again is signed in, auto-rejoin back on",
            cc.CookieValid == true && cc.AutoRejoin && !cc.RejoinOffBySignOut, $"{cc.CookieValid} {cc.AutoRejoin}");
        // The daily check asks the accounts marked signed out again too, quietly.
        w.AutoRejoin = false;
        w.RejoinOffBySignOut = true;
        bool healed = await (Task<bool>)Harness.Invoke(app, "RecheckSignedOutAsync", vault, new List<Account>())!;
        c.Check("the daily check asks again an account marked signed out: accepted now, it is signed in, auto-rejoin back on",
            healed && w.CookieValid == true && w.AutoRejoin, $"{healed} {w.CookieValid} {w.AutoRejoin}");
        c.Check("…ones Roblox still rejects stay signed out", d.CookieValid == false && s.CookieValid == false);
    }

    /// <summary>The renewals Roblox's answers brought, saved as the process timer saves them.</summary>
    static async Task Renewals(AppController app)
    {
        if (typeof(AppController).GetMethod("SaveRenewedSignInsAsync", Harness.F) is { } save)
            await (Task)save.Invoke(app, null)!;
    }

    static DateTime? Renewed(Account account) => typeof(Account).GetProperty("CookieRenewed")?.GetValue(account) as DateTime?;

    static object? Field(object owner, string name) => owner.GetType().GetField(name, Harness.F)?.GetValue(owner);

    /// <summary>Roblox's refusals are answers here too: what counts is the cookie the answer set.</summary>
    static async Task Quietly(Task ask)
    {
        try
        {
            await ask;
        }
        catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
        {
        }
    }

    static async Task<bool> Until(Func<bool> done, int seconds = 30)
    {
        for (var until = DateTime.UtcNow.AddSeconds(seconds); DateTime.UtcNow < until; await Task.Delay(100))
            if (done())
                return true;
        return done();
    }

    sealed record Request(string Method, string Host, string Path, string? Cookie, bool Csrf, long BodyLength);

    /// <summary>
    /// Roblox as far as these checks go: who each made-up cookie signs in, and a new cookie to set in the next answer
    /// to a request carrying a given one.
    /// </summary>
    sealed class StandIn
    {
        public readonly Dictionary<string, long> Users = new();
        public readonly Dictionary<string, string> Rotate = new();
        /// <summary>A Set-Cookie header added to every answer while set.</summary>
        public string? Extra;
        /// <summary>Roblox's sign-in service down: "who am I" rejects every cookie.</summary>
        public bool Down;
        readonly List<Request> _asked = new();

        public List<Request> Asked
        {
            get { lock (_asked) return _asked.ToList(); }
        }

        public List<Request> AskedWith(string cookie) => Asked.Where(r => r.Cookie == cookie).ToList();

        public void Clear()
        {
            lock (_asked)
                _asked.Clear();
        }

        public async Task<HttpResponseMessage> Answer(HttpRequestMessage request)
        {
            var uri = request.RequestUri!;
            string? cookie = request.Headers.TryGetValues("Cookie", out var sent) && sent.FirstOrDefault() is { } header
                ? header.Replace(".ROBLOSECURITY=", "") : null;
            bool csrf = request.Headers.Contains("x-csrf-token");
            long body = request.Content is null ? 0 : (await request.Content.ReadAsByteArrayAsync()).Length;
            lock (_asked)
                _asked.Add(new Request(request.Method.Method, uri.Host, uri.AbsolutePath, cookie, csrf, body));

            HttpResponseMessage res;
            lock (Users)
            {
                long? user = cookie is not null && Users.TryGetValue(cookie, out long id) ? id : null;
                res = uri.AbsolutePath switch
                {
                    "/v1/users/authenticated" => user is { } u && !Down
                        ? Json(HttpStatusCode.OK, new { id = u, name = $"user{u}", displayName = $"User {u}" })
                        : Refusal(HttpStatusCode.Unauthorized, "Authorization has been denied for this request."),
                    "/v1/authentication-ticket/" when !csrf => Csrf(),
                    "/v1/authentication-ticket/" => Ticket(),
                    "/v1/join-game" when !csrf => Csrf(),
                    "/v1/join-game" => Json(HttpStatusCode.OK, new { status = 12, message = "Unable to join Game 211" }),
                    "/v1/presence/users" when !csrf => Csrf(),
                    "/v1/presence/users" => Json(HttpStatusCode.OK, new { userPresences = Array.Empty<object>() }),
                    _ when uri.AbsolutePath.EndsWith("/currency", StringComparison.Ordinal) => Json(HttpStatusCode.OK, new { robux = 0 }),
                    _ when uri.Host.StartsWith("thumbnails.", StringComparison.Ordinal) => Json(HttpStatusCode.OK, new { data = Array.Empty<object>() }),
                    _ => Json(HttpStatusCode.NotFound, new { }),
                };
                // Only an answer that went through (not the CSRF refusal before it) renews, as Roblox's would.
                if (cookie is not null && res.StatusCode != HttpStatusCode.Forbidden && Rotate.Remove(cookie, out string? next))
                    SetCookie(res, next, uri.Host);
            }
            if (Extra is { } extra)
                res.Headers.TryAddWithoutValidation("Set-Cookie", extra);
            return res;
        }

        static void SetCookie(HttpResponseMessage res, string value, string host)
        {
            string domain = host.EndsWith(".robloxapp.vnggames.com", StringComparison.Ordinal) ? ".robloxapp.vnggames.com" : ".roblox.com";
            res.Headers.TryAddWithoutValidation("Set-Cookie",
                $".ROBLOSECURITY={value}; domain={domain}; expires={DateTime.UtcNow.AddDays(30):R}; path=/; secure; HttpOnly");
        }

        static HttpResponseMessage Csrf()
        {
            var res = Refusal(HttpStatusCode.Forbidden, "Token Validation Failed");
            res.Headers.TryAddWithoutValidation("x-csrf-token", "stand-in-csrf");
            return res;
        }

        static HttpResponseMessage Ticket()
        {
            var res = Json(HttpStatusCode.OK, new { });
            res.Headers.TryAddWithoutValidation("rbx-authentication-ticket", "stand-in-ticket");
            return res;
        }

        static HttpResponseMessage Refusal(HttpStatusCode status, string message) =>
            Json(status, new { errors = new[] { new { code = 0, message } } });

        static HttpResponseMessage Json(HttpStatusCode status, object body) =>
            new(status) { Content = new StringContent(JsonSerializer.Serialize(body), Encoding.UTF8, "application/json") };
    }
}
