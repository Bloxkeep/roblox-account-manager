using System.Globalization;
using System.Net;
using System.Text;
using System.Text.Json;

namespace RobloxManager;

/// <summary>Signing in with a username and password: Roblox's checks on the way (captcha, proof of work, two-step).</summary>
public static partial class RobloxApi
{
    /// <summary>A password sign-in's outcome: a cookie (done), a two-step prompt or captcha to answer, or a check this app
    /// can't show (<see cref="Blocked"/>: what to tell the person; the attempt ends).</summary>
    public sealed record LoginResult(string? Cookie, TwoStepPrompt? TwoStep, CaptchaPrompt? Captcha = null, string? Blocked = null,
        ProofOfWorkPrompt? ProofOfWork = null, HumanCaptchaPrompt? HumanCaptcha = null);

    /// <summary>Roblox's "captchav2", HUMAN Security's press-and-hold check: fetched from apis.roblox.com/v2/captcha with
    /// <see cref="CaptchaChallengeId"/> and solved in <see cref="CaptchaForm"/>, which hands back a redemption token.</summary>
    /// <param name="Metadata">The challenge's own metadata, sent back with the token filled in.</param>
    public sealed record HumanCaptchaPrompt(string Realm, string LoginChallengeId, string CaptchaChallengeId, string Metadata,
        string Username, string Password);

    /// <summary>A proof-of-work check: a puzzle of a few CPU seconds, solved in the background as roblox.com does.</summary>
    /// <param name="Metadata">The challenge's own metadata (session, request path…); it goes back to Roblox as-is, with the token filled in.</param>
    public sealed record ProofOfWorkPrompt(string Realm, string LoginChallengeId, string SessionId, string Metadata, string Username, string Password);

    /// <summary>What's needed to finish a two-step sign-in: the user picks where the code comes from and types it.</summary>
    /// <param name="MediaType">Set when the login answer carried "twoStepVerificationData" (a 200, no cookie) instead of a
    /// challenge header: the code went that way already (e.g. "Email"), and the sign-in finishes through
    /// /v3/users/{id}/two-step-verification/login with the ticket rather than by repeating the login.</param>
    public sealed record TwoStepPrompt(string Realm, long UserId, string ChallengeId, string LoginChallengeId, string Username, string Password,
        string? MediaType = null, string ActionType = "Login")
    {
        /// <summary>Where this account can get a code (authenticator, email, sms), the one Roblox prefers first.</summary>
        public string[] Methods { get; init; } = ["authenticator", "email"];

        /// <summary>Set once Roblox accepted the code, so a retry after a failed finish doesn't send the spent code again.</summary>
        public string? VerificationToken { get; set; }
    }

    /// <summary>The 2-step methods Bloxkeep can answer: a code typed by the user, checked at challenges/{method}/verify.</summary>
    static readonly string[] CodeMethods = ["authenticator", "email", "sms"];

    /// <summary>A 2-step method's name from either spelling Roblox uses: "Email", or its TwoStepVerificationMediaType number
    /// (Email 0, SMS 1, Authenticator 2, RecoveryCode 3, SecurityKey 4, CrossDevice 5, Password 6, Passkey 7).</summary>
    static string MediaName(JsonElement e) => e.ValueKind switch
    {
        JsonValueKind.String => e.GetString() ?? "",
        JsonValueKind.Number when e.TryGetInt32(out int n) && n >= 0 && n < MediaTypes.Length => MediaTypes[n],
        _ => "",
    };

    static readonly string[] MediaTypes = ["Email", "SMS", "Authenticator", "RecoveryCode", "SecurityKey", "CrossDevice", "Password", "Passkey"];

    /// <returns>The path name of a method answered with a typed code, or null for one Bloxkeep can't do (security key…).</returns>
    static string? CodeMethod(string mediaType) => CodeMethods.FirstOrDefault(m => m.Equals(mediaType, StringComparison.OrdinalIgnoreCase));

    /// <summary>A captcha on a sign-in: Arkose Labs' FunCaptcha (see apis.roblox.com/captcha/v1/metadata);
    /// <see cref="Blob"/> ties it to this attempt.</summary>
    /// <param name="ChallengeType">"captcha", or "captchav2" when Roblox asks for one after another check.</param>
    /// <param name="Metadata">The challenge's own metadata, sent back with the token filled in.</param>
    public sealed record CaptchaPrompt(string Realm, string LoginChallengeId, string UnifiedCaptchaId, string Blob, string ActionType,
        string Username, string Password, string ChallengeType = "captcha", string Metadata = "{}");

    static string? CookieFrom(HttpResponseMessage res) =>
        res.Headers.TryGetValues("Set-Cookie", out var values)
            ? values.Where(v => v.StartsWith(CookieName + "=", StringComparison.Ordinal)).Select(NormalizeCookie).FirstOrDefault(v => v.Length > 0)
            : null;

    /// <summary>Signs in as the Roblox client does: username and password go only to Roblox's (or VNG's) official auth
    /// host over HTTPS and are never stored or logged by the app.</summary>
    public static Task<LoginResult> LoginWithPasswordAsync(string username, string password, string realm, CancellationToken ct = default) =>
        SendLoginAsync(username, password, realm, null, ct);

    /// <summary>POST /v2/login, optionally carrying a solved challenge, and sorts out what Roblox answered.</summary>
    static async Task<LoginResult> SendLoginAsync(string username, string password, string realm, Action<HttpRequestMessage>? configure,
        CancellationToken ct)
    {
        string json = JsonSerializer.Serialize(new { ctype = "Username", cvalue = username, password });
        using var res = await PostWithCsrfAsync(Realm.Api(realm, "auth") + "/v2/login", "", json, configure, ct, realm);
        if (CookieFrom(res) is { } cookie && res.IsSuccessStatusCode)
            return new LoginResult(cookie, null);

        string step = configure is null ? "login" : "login after a check";
        if (Header(res, "rblx-challenge-id") is { Length: > 0 } challengeId)
            return ParseChallenge(realm, challengeId, Header(res, "rblx-challenge-type") ?? "",
                DecodeMetadata(Header(res, "rblx-challenge-metadata")), username, password, step, res);

        string body = await res.Content.ReadAsStringAsync(ct);
        if (res.IsSuccessStatusCode)
        {
            if (TwoStepFromBody(realm, body, username, password) is { } twoStep)
                return new LoginResult(null, twoStep);
            var root = DecodeJson(body);
            // Accepted, but Roblox wants an ID check first (done only in its own apps and site).
            if (Field(root, "identityVerificationLoginTicket") is { Length: > 0 })
                return new LoginResult(null, null, Blocked: CantShow(realm, "identity verification"));
            if (root is { ValueKind: JsonValueKind.Object } r && r.TryGetProperty("isBanned", out var banned) && banned.ValueKind == JsonValueKind.True)
                throw new RobloxApiException(L.T("Roblox reports a problem with this account (it may be restricted or banned). Check it on roblox.com."), attemptOver: true);
            return new LoginResult(null, null, Blocked: Unsupported(step, realm, res, root));
        }
        throw LoginRefused(step, res.StatusCode, body);
    }

    static string? Field(JsonElement? obj, string name) =>
        obj is { ValueKind: JsonValueKind.Object } o && o.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.String ? v.GetString() : null;

    /// <summary>A check Roblox's site or app does that Bloxkeep can't (yet); <paramref name="kind"/> names it.</summary>
    static string CantShow(string realm, string kind)
    {
        Log.Note($"Sign-in check from Roblox{(realm == Realm.Vng ? " VNG" : "")} that Bloxkeep can't show: {kind}");
        return L.F("Roblox asked for a check Bloxkeep can't show yet ({0}). Try again later, or sign in with your phone.", kind);
    }

    /// <summary>A sign-in answer of unknown shape: logs only its layout (step, HTTP status, challenge header names, field
    /// names and value kinds, error codes), never a value (password, cookie, ticket, token, name, id). Returns what to
    /// tell the person.</summary>
    static string Unsupported(string step, string realm, HttpResponseMessage? res, JsonElement? json, string? type = null)
    {
        var headers = res is null ? [] : res.Headers.Select(h => h.Key).Where(k => k.StartsWith("rblx-", StringComparison.OrdinalIgnoreCase)).ToList();
        var codes = json is { ValueKind: JsonValueKind.Object } o && o.TryGetProperty("errors", out var errors) && errors.ValueKind == JsonValueKind.Array
            ? errors.EnumerateArray().Select(e => e.ValueKind == JsonValueKind.Object && e.TryGetProperty("code", out var c) ? c.GetRawText() : "?").ToList()
            : [];
        Log.Note($"Sign-in answer not understood{(realm == Realm.Vng ? " (VNG)" : "")}: step '{step}'"
            + (res is null ? "" : $", HTTP {(int)res.StatusCode}")
            + (type is null ? "" : $", type '{SafeName(type)}'")
            + $", headers [{string.Join(", ", headers)}]"
            + $", body {(json is { } j ? Shape(j) : "not JSON")}"
            + (codes.Count > 0 ? $", errors [{string.Join(", ", codes)}]" : ""));
        return L.T("Roblox asked for a sign-in check Bloxkeep doesn't support yet. Sign in with your phone or the browser instead. The details were saved in error.log (Settings › Backup & data › Open folder).");
    }

    /// <summary>A name from Roblox (a check's type, a field) as it may go to the log: short and plain, else "#".</summary>
    static string SafeName(string name) =>
        name.Length == 0 ? "" : name.Length <= 40 && !name.All(char.IsAsciiDigit) && name.All(c => char.IsAsciiLetterOrDigit(c) || c is '-' or '_' or '.')
            ? name : "#";

    /// <summary>Checks Roblox's site knows but that only it (or the Roblox app) can show.</summary>
    static readonly string[] BrowserOnlyChecks =
    [
        "securityquestions", "security-questions", "reauthentication", "rostile", "privateaccesstoken", "deviceintegrity",
        "proofofspace", "phoneverification", "emailverification", "biometric", "chef", "turnstile",
    ];

    /// <summary>Turns a challenge Roblox put in front of the sign-in into what the user (or the app) has to answer.</summary>
    /// <param name="res">The answer the challenge came in, when it came in headers (for the log of an unknown one).</param>
    static LoginResult ParseChallenge(string realm, string challengeId, string type, JsonElement? meta, string username, string password,
        string step, HttpResponseMessage? res = null)
    {
        string? Text(string name) => Field(meta, name);
        bool Is(params string[] kinds) => kinds.Any(k => type.Equals(k, StringComparison.OrdinalIgnoreCase));
        if (Known() is { } known)
        {
            // Only the kind of check and the layout of its fields: enough to follow a change on Roblox's side.
            Log.Note($"Sign-in check from Roblox{(realm == Realm.Vng ? " VNG" : "")}: type '{SafeName(type)}', fields "
                + (meta is { } shape ? Shape(shape) : "none"));
            return known;
        }
        // An unknown check, or a known one in a shape that changed: one line with its layout, for a later version.
        return new LoginResult(null, null, Blocked: Unsupported(step, realm, res, meta, type));

        LoginResult? Known()
        {
            if (Is("twostepverification")
                && Text("challengeId") is { Length: > 0 } twoStepId
                && meta!.Value.TryGetProperty("userId", out var uid)
                && (uid.ValueKind == JsonValueKind.Number ? uid.TryGetInt64(out long id) ? id : 0
                    : uid.ValueKind == JsonValueKind.String && long.TryParse(uid.GetString(), out var n) ? n : 0) is > 0 and var userId)
            {
                return new LoginResult(null, new TwoStepPrompt(realm, userId, twoStepId, challengeId, username, password,
                    ActionType: Text("actionType") is { Length: > 0 } action ? action : "Login"));
            }
            if (Is("captcha", "captchav2")
                && Text("unifiedCaptchaId") is { Length: > 0 } captchaId
                && Text("dataExchangeBlob") is { Length: > 0 } blob)
            {
                return new LoginResult(null, null, new CaptchaPrompt(realm, challengeId, captchaId, blob, Text("actionType") ?? "Login",
                    username, password, type, meta!.Value.GetRawText()));
            }
            if (Is("captchav2") && Text("challengeId") is { Length: > 0 } humanId)
                return new LoginResult(null, null, HumanCaptcha: new HumanCaptchaPrompt(realm, challengeId, humanId, meta!.Value.GetRawText(), username, password));
            if (Is("proofofwork") && Text("sessionId") is { Length: > 0 } powSession)
                return new LoginResult(null, null, ProofOfWork: new ProofOfWorkPrompt(realm, challengeId, powSession, meta!.Value.GetRawText(), username, password));
            // The account has to turn on 2-step (or an authenticator) before it may sign in: roblox.com sends it to settings.
            if (Is("forcetwostepverification", "forceauthenticator"))
                return new LoginResult(null, null, Blocked: L.T("Roblox wants this account to turn on 2-step verification before it signs in. Sign in on roblox.com or in the Roblox app, turn it on there, then try again, or sign in with your phone."));
            if (Is("blocksession", "denied"))
                return new LoginResult(null, null, Blocked: L.T("Roblox blocked this sign-in. Sign in with your phone or the browser instead."));
            if (Is(BrowserOnlyChecks))
                return new LoginResult(null, null, Blocked: L.F("Roblox asked for a check Bloxkeep can't show yet ({0}). Try again later, or sign in with your phone.", SafeName(type)));
            return null;
        }
    }

    /// <summary>2-step asked in a 200 answer's body ("twoStepVerificationData": { mediaType, ticket }), seen after a solved
    /// captcha. Null when the body isn't that.</summary>
    static TwoStepPrompt? TwoStepFromBody(string realm, string body, string username, string password)
    {
        try
        {
            using var doc = JsonDocument.Parse(body);
            var root = doc.RootElement;
            if (root.ValueKind != JsonValueKind.Object
                || !root.TryGetProperty("twoStepVerificationData", out var data) || data.ValueKind != JsonValueKind.Object
                || !data.TryGetProperty("ticket", out var ticket) || ticket.GetString() is not { Length: > 0 } ticketId
                || !root.TryGetProperty("user", out var user) || !user.TryGetProperty("id", out var id) || !id.TryGetInt64(out long userId))
                return null;
            string media = data.TryGetProperty("mediaType", out var m) && MediaName(m) is { Length: > 0 } kind ? kind : "Email";
            Log.Note($"Sign-in check from Roblox{(realm == Realm.Vng ? " VNG" : "")}: two-step in the answer, by {SafeName(media)}");
            return new TwoStepPrompt(realm, userId, ticketId, "", username, password, media);
        }
        catch (Exception ex) when (ex is JsonException or InvalidOperationException)
        {
            // A field of another kind than expected (GetString on a number…): not this shape.
            return null;
        }
    }

    static JsonElement? DecodeMetadata(string? base64)
    {
        if (string.IsNullOrEmpty(base64))
            return null;
        try
        {
            using var doc = JsonDocument.Parse(Encoding.UTF8.GetString(Convert.FromBase64String(base64)));
            return doc.RootElement.Clone();
        }
        catch (Exception ex) when (ex is FormatException or JsonException)
        {
            return null;
        }
    }

    /// <summary>The FunCaptcha key Roblox (or VNG) uses for sign-ins, from its public captcha settings.</summary>
    public static async Task<string> GetLoginCaptchaKeyAsync(string realm, CancellationToken ct = default)
    {
        using var req = Request(HttpMethod.Get, Realm.Api(realm, "apis") + "/captcha/v1/metadata", null);
        using var res = await Http.SendAsync(req, ct);
        string body = await res.Content.ReadAsStringAsync(ct);
        if (!res.IsSuccessStatusCode)
            throw Refused("Captcha settings", res.StatusCode, body);
        using var doc = JsonDocument.Parse(body);
        if (doc.RootElement.TryGetProperty("funCaptchaPublicKeys", out var keys)
            && keys.TryGetProperty("ACTION_TYPE_WEB_LOGIN", out var key) && key.GetString() is { Length: > 0 } value)
            return value;
        throw new RobloxApiException(L.T("Roblox didn't say which captcha to show."));
    }

    /// <summary>Hands Roblox a solved captcha's token and retries the sign-in with it, as roblox.com does; the answer can
    /// still be a two-step prompt (or, rarely, another captcha).</summary>
    public static Task<LoginResult> FinishCaptchaLoginAsync(CaptchaPrompt prompt, string captchaToken, CancellationToken ct = default) =>
        ContinueLoginAsync(prompt.Realm, prompt.LoginChallengeId, prompt.ChallengeType,
            prompt.ChallengeType == "captcha"
                ? JsonSerializer.Serialize(new { unifiedCaptchaId = prompt.UnifiedCaptchaId, captchaToken, actionType = prompt.ActionType })
                : WithField(prompt.Metadata, "captchaToken", captchaToken),
            prompt.Username, prompt.Password, ct);

    /// <summary>Carries on with the sign-in once the press-and-hold check handed back its redemption token.</summary>
    public static Task<LoginResult> FinishHumanCaptchaLoginAsync(HumanCaptchaPrompt prompt, string redemptionToken, CancellationToken ct = default) =>
        ContinueLoginAsync(prompt.Realm, prompt.LoginChallengeId, "captchav2", WithField(prompt.Metadata, "redemptionToken", redemptionToken),
            prompt.Username, prompt.Password, ct);

    /// <summary>Solves a proof-of-work check as roblox.com does, then carries on with the sign-in.</summary>
    public static async Task<LoginResult> FinishProofOfWorkLoginAsync(ProofOfWorkPrompt prompt, CancellationToken ct = default)
    {
        string redemptionToken = await ProofOfWorkTokenAsync(prompt.Realm, prompt.SessionId, ct);
        return await ContinueLoginAsync(prompt.Realm, prompt.LoginChallengeId, "proofofwork",
            WithField(prompt.Metadata, "redemptionToken", redemptionToken), prompt.Username, prompt.Password, ct);
    }

    /// <summary>Fetches a proof-of-work puzzle, computes its answer and trades it for the redemption token the check wants.</summary>
    public static async Task<string> ProofOfWorkTokenAsync(string realm, string sessionId, CancellationToken ct = default)
    {
        string service = Realm.Api(realm, "apis") + "/proof-of-work-service/v1/pow-puzzle";
        string puzzleType, artifacts;
        using (var req = Request(HttpMethod.Get, $"{service}?sessionID={Uri.EscapeDataString(sessionId)}", null))
        using (var res = await Http.SendAsync(req, ct))
        {
            string body = await res.Content.ReadAsStringAsync(ct);
            if (!res.IsSuccessStatusCode)
                throw Refused("Proof-of-work puzzle", res.StatusCode, body);
            using var doc = JsonDocument.Parse(body);
            var root = doc.RootElement;
            puzzleType = !root.TryGetProperty("puzzleType", out var p) ? ""
                : p.ValueKind == JsonValueKind.String ? p.GetString() ?? "" : p.GetRawText();
            artifacts = root.TryGetProperty("artifacts", out var a) ? (a.ValueKind == JsonValueKind.String ? a.GetString() ?? "" : a.GetRawText()) : "";
            Log.Note($"Proof-of-work puzzle: type '{SafeName(puzzleType)}', fields {string.Join(", ", root.EnumerateObject().Select(x => x.Name))}"
                + (DecodeJson(artifacts) is { ValueKind: JsonValueKind.Object } art ? $", artifacts {string.Join(", ", art.EnumerateObject().Select(x => x.Name))}" : ""));
        }

        string solution = await Task.Run(() => SolvePuzzle(puzzleType, artifacts, ct), ct);

        string redemptionToken;
        using (var res = await PostWithCsrfAsync(service, "", JsonSerializer.Serialize(new { sessionID = sessionId, solution }), null, ct, realm))
        {
            string body = await res.Content.ReadAsStringAsync(ct);
            if (!res.IsSuccessStatusCode)
                throw Refused("Proof-of-work answer", res.StatusCode, body);
            using var doc = JsonDocument.Parse(body);
            var root = doc.RootElement;
            if (root.TryGetProperty("answerCorrect", out var ok) && ok.ValueKind == JsonValueKind.False)
                throw new RobloxApiException(L.T("Roblox didn't accept the sign-in check. Please try again."));
            redemptionToken = root.TryGetProperty("redemptionToken", out var r) ? r.GetString() ?? "" : "";
            if (redemptionToken.Length == 0)
            {
                Log.Note($"Proof-of-work answer reply fields: {string.Join(", ", root.EnumerateObject().Select(x => x.Name))}");
                throw new RobloxApiException(L.T("Roblox didn't accept the sign-in check. Please try again."));
            }
        }
        return redemptionToken;
    }

    /// <summary>Roblox's time-lock puzzle: square A modulo N, T times. Inherently serial: a few seconds of one CPU core per
    /// sign-in.</summary>
    static string SolvePuzzle(string puzzleType, string artifacts, CancellationToken ct)
    {
        // Roblox names the type as a number: 1 is the time-lock puzzle (seen Oct 2026, with artifacts N, A, T).
        if (!(puzzleType == "1" || puzzleType.Equals("TimeLockPuzzle", StringComparison.OrdinalIgnoreCase))
            || DecodeJson(artifacts) is not { ValueKind: JsonValueKind.Object } art
            || Number(art, "N") is not { } n || Number(art, "A") is not { } x || Number(art, "T") is not { } t)
            throw new UserError(L.F("Roblox asked for a check Bloxkeep can't show yet ({0}). Try again later, or sign in with your phone.",
                SafeName(puzzleType) is { Length: > 0 } kind && kind != "#" ? kind : "proofofwork"));
        for (System.Numerics.BigInteger i = 0; i < t; i++)
        {
            if ((i & 0xFFF) == 0)
                ct.ThrowIfCancellationRequested();
            x = System.Numerics.BigInteger.ModPow(x, 2, n);
        }
        return x.ToString(CultureInfo.InvariantCulture);

        static System.Numerics.BigInteger? Number(JsonElement obj, string name)
        {
            var prop = obj.EnumerateObject().FirstOrDefault(p => p.Name.Equals(name, StringComparison.OrdinalIgnoreCase));
            string? text = prop.Value.ValueKind switch
            {
                JsonValueKind.String => prop.Value.GetString(),
                JsonValueKind.Number => prop.Value.GetRawText(),
                _ => null,
            };
            return System.Numerics.BigInteger.TryParse(text, NumberStyles.None, CultureInfo.InvariantCulture, out var v) ? v : null;
        }
    }

    /// <summary>A JSON value's layout for the log: field names and value kinds (text by length), never values. Strings
    /// holding (base64) JSON are opened too; three levels at most; a field name that looks like data is written "#".</summary>
    static string Shape(JsonElement e, int depth = 0)
    {
        if (depth >= 3 && e.ValueKind is JsonValueKind.Object or JsonValueKind.Array)
            return "…";
        switch (e.ValueKind)
        {
            case JsonValueKind.Object:
                return "{" + string.Join(", ", e.EnumerateObject().Select(p => $"{SafeName(p.Name)}: {Shape(p.Value, depth + 1)}")) + "}";
            case JsonValueKind.Array:
                return $"[{e.GetArrayLength()} x {(e.GetArrayLength() > 0 ? Shape(e[0], depth + 1) : "-")}]";
            case JsonValueKind.String:
                string s = e.GetString() ?? "";
                if ((DecodeJson(s) ?? (s.Length > 0 && s.Length % 4 == 0 ? DecodeMetadata(s) : null)) is { ValueKind: JsonValueKind.Object or JsonValueKind.Array } inner)
                    return "json" + Shape(inner, depth + 1);
                return $"text({s.Length})";
            case JsonValueKind.True or JsonValueKind.False:
                return "bool";
            default:
                return e.ValueKind.ToString().ToLowerInvariant();
        }
    }

    /// <summary>The JSON object with one string field set (added or replaced); every other field is kept as it was.</summary>
    static string WithField(string json, string name, string value)
    {
        var obj = System.Text.Json.Nodes.JsonNode.Parse(json) as System.Text.Json.Nodes.JsonObject ?? new System.Text.Json.Nodes.JsonObject();
        obj[name] = value;
        return obj.ToJsonString();
    }

    static JsonElement? DecodeJson(string json)
    {
        try
        {
            using var doc = JsonDocument.Parse(json);
            return doc.RootElement.Clone();
        }
        catch (JsonException)
        {
            return null;
        }
    }

    /// <summary>Tells Roblox a challenge was answered, then retries the sign-in carrying the answer, as roblox.com does.</summary>
    static async Task<LoginResult> ContinueLoginAsync(string realm, string challengeId, string type, string metadataJson,
        string username, string password, CancellationToken ct)
    {
        string json = JsonSerializer.Serialize(new { challengeId, challengeType = type, challengeMetadata = metadataJson });
        string retryId = challengeId, retryType = type, retryMeta = metadataJson;
        using (var res = await PostWithCsrfAsync(Realm.Api(realm, "apis") + "/challenge/v1/continue", "", json, null, ct, realm))
        {
            if (res.IsSuccessStatusCode)
            {
                // Roblox answers with the challenge the retried request must carry; use it when it's there.
                string reply = await res.Content.ReadAsStringAsync(ct);
                var answer = DecodeJson(reply);
                // roblox.com reads challengeType from every reply ("" when no check is left); without it, the answer is new.
                if (answer is not { ValueKind: JsonValueKind.Object } r || Field(r, "challengeType") is null)
                    return new LoginResult(null, null, Blocked: Unsupported($"{SafeName(type)} continue", realm, res, answer));
                string? Text(string name) => Field(r, name);
                Log.Note($"Challenge continue ({SafeName(type)}) reply: fields {string.Join(", ", r.EnumerateObject().Select(x => SafeName(x.Name)))}, type '{SafeName(Text("challengeType")!)}'");
                // Roblox can chain checks (e.g. a captcha after proof-of-work): answer the next one before retrying.
                if (Text("challengeType") is { Length: > 0 } next && !next.Equals(type, StringComparison.OrdinalIgnoreCase))
                {
                    string raw = Text("challengeMetadata") ?? "{}";
                    var nextMeta = DecodeJson(raw) ?? DecodeMetadata(raw);
                    return ParseChallenge(realm, Text("challengeId") is { Length: > 0 } nextId ? nextId : challengeId, next, nextMeta, username, password,
                        $"{SafeName(type)} continue");
                }
                if (Text("challengeId") is { Length: > 0 } id) retryId = id;
                if (Text("challengeType") is { Length: > 0 } t) retryType = t;
                if (Text("challengeMetadata") is { Length: > 0 } m) retryMeta = m;
            }
            else
            {
                // The answered check is spent: this attempt can only start over.
                string body = await res.Content.ReadAsStringAsync(ct);
                throw LoginRefused($"the {type} check", res.StatusCode, body);
            }
        }
        string meta = Convert.ToBase64String(Encoding.UTF8.GetBytes(retryMeta));
        try
        {
            return await SendLoginAsync(username, password, realm, req =>
            {
                req.Headers.TryAddWithoutValidation("rblx-challenge-id", retryId);
                req.Headers.TryAddWithoutValidation("rblx-challenge-type", retryType);
                req.Headers.TryAddWithoutValidation("rblx-challenge-metadata", meta);
            }, ct);
        }
        catch (RobloxApiException ex)
        {
            Log.Note($"Sign-in retry after {type} refused{(ex.RateLimited ? " (rate-limited)" : "")}."); // its message may carry Roblox's reply
            throw;
        }
    }

    static string TwoStepUrl(TwoStepPrompt prompt, string path) =>
        $"{Realm.Api(prompt.Realm, "twostepverification")}/v1/users/{prompt.UserId}/{path}";

    /// <summary>Learns the account's 2-step methods as roblox.com does (no cookie: the challenge id proves the sign-in).
    /// Only typed-code methods are kept, Roblox's preferred first; with none left (security key, passkey, another
    /// device's approval) the sign-in can't go on here.</summary>
    public static async Task<LoginResult> WithTwoStepMethodsAsync(TwoStepPrompt prompt, CancellationToken ct = default)
    {
        List<string> enabled = [];
        string primary = "";
        try
        {
            string url = TwoStepUrl(prompt, "configuration")
                + $"?challengeId={Uri.EscapeDataString(prompt.ChallengeId)}&actionType={Uri.EscapeDataString(prompt.ActionType)}";
            using var req = Request(HttpMethod.Get, url, null, fromWebsite: true, prompt.Realm);
            using var res = await Http.SendAsync(req, ct);
            string body = await res.Content.ReadAsStringAsync(ct);
            var config = DecodeJson(body);
            if (res.IsSuccessStatusCode && config is { ValueKind: JsonValueKind.Object } c
                && c.TryGetProperty("methods", out var methods) && methods.ValueKind == JsonValueKind.Array)
            {
                primary = c.TryGetProperty("primaryMediaType", out var p) ? MediaName(p) : "";
                enabled = methods.EnumerateArray()
                    .Where(m => m.ValueKind == JsonValueKind.Object && m.TryGetProperty("enabled", out var on) && on.ValueKind == JsonValueKind.True
                        && m.TryGetProperty("mediaType", out _))
                    .Select(m => MediaName(m.GetProperty("mediaType"))).Where(m => m.Length > 0).ToList();
                Log.Note($"Two-step methods: {string.Join(", ", enabled.Select(SafeName))}, preferred {SafeName(primary)}");
            }
            else
            {
                // Not needed to go on: the code screen then offers the usual methods.
                Log.Note($"Two-step methods unknown: HTTP {(int)res.StatusCode}, body {(config is { } j ? Shape(j) : "not JSON")}");
            }
        }
        catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException && !ct.IsCancellationRequested)
        {
            Log.Note($"Two-step methods unknown: {ex.GetType().Name}");
        }
        return PickTwoStepMethods(prompt, enabled, primary);
    }

    /// <param name="enabled">The account's enabled methods as Roblox names them; empty when unknown.</param>
    static LoginResult PickTwoStepMethods(TwoStepPrompt prompt, List<string> enabled, string primary)
    {
        // Unknown: the method the answer named, then the ones most accounts have (unless it named one without a code).
        if (enabled.Count == 0)
        {
            if (prompt.MediaType is { } sent)
                enabled.Add(sent);
            if (prompt.MediaType is null || CodeMethod(prompt.MediaType) is not null)
                enabled.AddRange(["Authenticator", "Email"]);
        }
        var usable = enabled.Select(CodeMethod).OfType<string>().Distinct().ToList();
        if (usable.Count == 0)
        {
            Log.Note($"Two-step by {string.Join(", ", enabled.Select(SafeName))} only: not a typed code.");
            return new LoginResult(null, null, Blocked: L.T("This account confirms sign-ins with a security key, a passkey or another device, which Bloxkeep can't do. Sign in with your phone or the browser instead."));
        }
        // The method the code was already sent by (or Roblox's preferred one) comes first.
        string? first = CodeMethod(prompt.MediaType ?? "") ?? CodeMethod(primary);
        if (first is not null && usable.Remove(first))
            usable.Insert(0, first);
        return new LoginResult(null, prompt with { Methods = [.. usable] });
    }

    /// <summary>Has Roblox send a (new) code by email or SMS; the answer is just whether it was sent.</summary>
    public static async Task SendTwoStepCodeAsync(TwoStepPrompt prompt, string method, CancellationToken ct = default)
    {
        string body = JsonSerializer.Serialize(new { challengeId = prompt.ChallengeId, actionType = prompt.ActionType });
        using var res = await PostWithCsrfAsync(TwoStepUrl(prompt, $"challenges/{method}/send-code"), "", body, null, ct, prompt.Realm);
        if (res.IsSuccessStatusCode)
            return;
        throw TwoStepRefused("Two-step code", res.StatusCode, await res.Content.ReadAsStringAsync(ct));
    }

    /// <summary>A twostepverification.roblox.com refusal by its documented codes: 10 wrong code (typed again), 18
    /// authenticator code used already, 1 challenge expired (the attempt is over), 5 or 429 too many tries.</summary>
    static Exception TwoStepRefused(string what, HttpStatusCode status, string body)
    {
        var (codes, _) = Errors(body);
        if (status == HttpStatusCode.TooManyRequests || codes.Contains(5))
            return new RobloxApiException(L.T("Too many sign-in attempts. Wait a few minutes and try again."), rateLimited: true);
        if (codes.Contains(10))
            return new UserError(L.T("That verification code is wrong or expired."));
        if (codes.Contains(18))
            return new UserError(L.T("That code was used already. Wait for the next one in the authenticator app."));
        if (codes.Contains(1))
        {
            Log.Note($"{what}: HTTP {(int)status}, the challenge expired");
            return new RobloxApiException(L.T("This sign-in has expired. Sign in again."), attemptOver: true);
        }
        return Refused(what, status, body);
    }

    /// <summary>Verifies the code, then finishes the way 2-step was asked: from the login's answer, through
    /// /v3/users/{id}/two-step-verification/login with the ticket; from a challenge header, via /challenge/v1/continue
    /// and a repeated login (as roblox.com does), which may end in another check.</summary>
    public static async Task<LoginResult> VerifyTwoStepAsync(TwoStepPrompt prompt, string method, string code, CancellationToken ct = default)
    {
        string token;
        if (prompt.VerificationToken is { Length: > 0 } accepted)
        {
            token = accepted;
        }
        else
        {
            string verifyBody = JsonSerializer.Serialize(new { challengeId = prompt.ChallengeId, actionType = prompt.ActionType, code });
            using (var res = await PostWithCsrfAsync(TwoStepUrl(prompt, $"challenges/{method}/verify"), "", verifyBody, null, ct, prompt.Realm))
            {
                string body = await res.Content.ReadAsStringAsync(ct);
                if (!res.IsSuccessStatusCode)
                    throw TwoStepRefused("Two-step verification", res.StatusCode, body);
                var answer = DecodeJson(body);
                if (Field(answer, "verificationToken") is not { Length: > 0 } verified)
                    throw new RobloxApiException(Unsupported("two-step verify", prompt.Realm, res, answer), attemptOver: true);
                token = verified;
            }
            prompt.VerificationToken = token;
        }

        if (prompt.MediaType is null)
            return await ContinueLoginAsync(prompt.Realm, prompt.LoginChallengeId, "twostepverification",
                JsonSerializer.Serialize(new { verificationToken = token, rememberDevice = false, challengeId = prompt.ChallengeId, actionType = prompt.ActionType }),
                prompt.Username, prompt.Password, ct);

        string finish = JsonSerializer.Serialize(new { challengeId = prompt.ChallengeId, verificationToken = token, rememberDevice = false });
        using var done = await PostWithCsrfAsync($"{Realm.Api(prompt.Realm, "auth")}/v3/users/{prompt.UserId}/two-step-verification/login",
            "", finish, null, ct, prompt.Realm);
        string doneBody = await done.Content.ReadAsStringAsync(ct);
        if (CookieFrom(done) is { } signedIn && done.IsSuccessStatusCode)
            return new LoginResult(signedIn, null);
        if (done.IsSuccessStatusCode)
        {
            var answer = DecodeJson(doneBody);
            return new LoginResult(null, null, Blocked: Field(answer, "identityVerificationLoginTicket") is { Length: > 0 }
                ? CantShow(prompt.Realm, "identity verification")
                : Unsupported("two-step finish", prompt.Realm, done, answer));
        }
        // 400: the ticket (1, 5) or the verification (10) is no longer good; only a new sign-in helps.
        if (done.StatusCode == HttpStatusCode.BadRequest)
        {
            Log.Note($"Sign-in refused at login after two-step: HTTP 400, errors {Errors(doneBody).Summary}");
            throw new RobloxApiException(L.T("This sign-in has expired. Sign in again."), attemptOver: true);
        }
        throw LoginRefused("login after two-step", done.StatusCode, doneBody);
    }
}
