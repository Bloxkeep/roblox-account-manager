using System.Net;
using System.Text;
using System.Text.Json;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// "Solve the bot check" on a window Roblox holds for its check (AppController.BotCheck.cs): the join asked with the
/// account's sign-in gets the check (rblx-challenge-* headers), a proof of work is computed and traded for its token,
/// the answer goes to Roblox's challenge/continue with the account's cookie, the join is asked again carrying it, and
/// once let in the held window is closed and opened again into its game. A kind not handled is said and nothing is sent.
/// Roblox here is a stand-in (RobloxApi.StandIn); no window is closed.
/// </summary>
static class BotCheckSuite
{
    const long Place = 4242;

    public static async Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        var seed = Vault.Create(Harness.Password);
        seed.Data.Accounts.Add(new Account { Username = "Held", UserId = 31, Cookie = "cookie-held", CookieValid = true });
        seed.Save();
        using var form = new Form();
        using var app = new AppController(form, _ => { });
        await Harness.Unlock(app);
        var account = Harness.Get<Vault>(app, "_vault").Data.Accounts.Single();

        // The held window: its log says joining, held for the check.
        var t = typeof(AppController);
        var running = Harness.Get<System.Collections.IDictionary>(app, "_running");
        var launchType = t.GetNestedType("PendingLaunch", Harness.F)!;
        var tupleType = running.GetType().GetGenericArguments()[1];
        var watcher = Harness.Get<RobloxLogWatcher>(app, "_logs");
        var tails = Harness.Get<System.Collections.IDictionary>(watcher, "_tails");
        var tailType = typeof(RobloxLogWatcher).GetNestedType("Tail", Harness.F)!;
        const int Pid = 777001;
        var tail = Activator.CreateInstance(tailType, "stand-in.log", Pid)!;
        tailType.GetField("State")!.SetValue(tail, new RobloxLogWatcher.ClientState("joining", Place, null, null, DateTime.UtcNow));
        tails[Pid] = tail;
        void Held(bool held) => tailType.GetField("JoinChallenged")!.SetValue(tail, held ? DateTime.UtcNow : null);
        var launch = Activator.CreateInstance(launchType, account.Id, new HashSet<int>(), DateTime.UtcNow.AddMinutes(-1), new GameTarget(Place), false, null)!;
        running[account.Id] = Activator.CreateInstance(tupleType, Pid, launch);
        var closed = new List<int>();
        Harness.Set(app, "_closeForReopen", (Action<int, bool>)((pid, _) => closed.Add(pid)));
        var restarts = Harness.Get<System.Collections.IDictionary>(app, "_restarts");

        // Roblox: the join is held until it carries the answered check.
        string type = "proofofwork";
        bool stillHeld = false;
        var seen = new List<string>();
        string? continued = null, continueCookie = null, retryHeader = null;
        static HttpResponseMessage Json(HttpStatusCode code, string json) => new(code) { Content = new StringContent(json, Encoding.UTF8, "application/json") };
        Task<HttpResponseMessage> Roblox(HttpRequestMessage req)
        {
            string path = req.RequestUri!.AbsolutePath;
            lock (seen) seen.Add(path);
            if (path.EndsWith("/v1/join-game", StringComparison.Ordinal))
            {
                if (req.Headers.TryGetValues("rblx-challenge-id", out var id) && !stillHeld)
                {
                    retryHeader = id.First();
                    return Task.FromResult(Json(HttpStatusCode.OK, """{"status":2,"message":"ok"}"""));
                }
                var held = Json(HttpStatusCode.Forbidden, """{"errors":[{"code":0,"message":"Challenge is required to authorize the request"}]}""");
                held.Headers.Add("rblx-challenge-id", "check-1");
                held.Headers.Add("rblx-challenge-type", type);
                held.Headers.Add("rblx-challenge-metadata", Convert.ToBase64String(Encoding.UTF8.GetBytes("""{"sessionId":"pow-1","userId":"31"}""")));
                return Task.FromResult(held);
            }
            if (path.EndsWith("/pow-puzzle", StringComparison.Ordinal) && req.Method == HttpMethod.Get)
                return Task.FromResult(Json(HttpStatusCode.OK, """{"puzzleType":"1","artifacts":"{\"N\":\"77\",\"A\":\"5\",\"T\":\"3\"}"}"""));
            if (path.EndsWith("/pow-puzzle", StringComparison.Ordinal))
            {
                // 5 squared three times modulo 77 is 4.
                string body = req.Content!.ReadAsStringAsync().Result;
                return Task.FromResult(Json(HttpStatusCode.OK, body.Contains("\"solution\":\"4\"") ? """{"answerCorrect":true,"redemptionToken":"token-1"}""" : """{"answerCorrect":false}"""));
            }
            if (path.EndsWith("/challenge/v1/continue", StringComparison.Ordinal))
            {
                continued = req.Content!.ReadAsStringAsync().Result;
                continueCookie = req.Headers.TryGetValues("Cookie", out var ck) ? string.Join(";", ck) : null;
                return Task.FromResult(Json(HttpStatusCode.OK, """{"challengeId":"check-1","challengeType":"proofofwork","challengeMetadata":"{\"sessionId\":\"pow-1\",\"redemptionToken\":\"token-1\"}"}"""));
            }
            return Task.FromResult(Json(HttpStatusCode.NotFound, "{}"));
        }
        var standIn = typeof(RobloxApi).GetProperty("StandIn", Harness.F)!;
        standIn.SetValue(null, (Func<HttpRequestMessage, Task<HttpResponseMessage>>)Roblox);
        try
        {
            Held(false);
            var notHeld = await Harness.Try(app, "solveBotCheck", new { id = account.Id });
            c.Check("a window not held by a check: said, nothing asked", notHeld is UserError && seen.Count == 0, notHeld);

            Held(true);
            var unknown = "unknownkind";
            type = unknown;
            var refused = await Harness.Try(app, "solveBotCheck", new { id = account.Id });
            c.Check("a kind of check not handled: said with its kind, nothing answered", refused is UserError u && u.Message.Contains(unknown)
                && continued is null && closed.Count == 0, refused);

            type = "proofofwork";
            stillHeld = true;
            seen.Clear();
            var notLet = await Harness.Try(app, "solveBotCheck", new { id = account.Id });
            c.Check("answered but Roblox still holds the join: said, the window left as it is", notLet is UserError && closed.Count == 0, notLet);

            stillHeld = false;
            seen.Clear();
            continued = null;
            var done = await Harness.Try(app, "solveBotCheck", new { id = account.Id });
            c.Check("a proof of work: computed and traded for its token", done is not UserError && seen.Count(p => p.EndsWith("/pow-puzzle")) == 2, string.Join(",", seen));
            c.Check("…the answer sent to Roblox with the account's sign-in, carrying the token",
                continued?.Contains("token-1") == true && continued.Contains("check-1") && continueCookie?.Contains("cookie-held") == true, continued);
            c.Check("…the join asked again carrying the check", retryHeader == "check-1", retryHeader);
            c.Check("…let in: the held window closed and opened again into its game", closed.SequenceEqual([Pid]) && restarts.Contains(account.Id), string.Join(",", closed));
        }
        finally
        {
            standIn.SetValue(null, null);
        }
    }
}
