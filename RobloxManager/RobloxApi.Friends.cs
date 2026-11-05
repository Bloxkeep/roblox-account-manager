using System.Net;
using System.Text;
using System.Text.Json;

namespace RobloxManager;

/// <summary>Friends between the person's own accounts (Friend each other): where two stand, a request, its acceptance.</summary>
public static partial class RobloxApi
{
    /// <summary>Where the signed-in account stands with each of <paramref name="others"/>: "Friends", "RequestSent",
    /// "RequestReceived" or "NotFriends", by user id (one missing from Roblox's answer is left out).</summary>
    public static async Task<Dictionary<long, string>> FriendStatusesAsync(string cookie, long userId, IReadOnlyCollection<long> others,
        string realm = Realm.Global, CancellationToken ct = default)
    {
        var result = new Dictionary<long, string>();
        if (others.Count == 0)
            return result;
        string url = Realm.Api(realm, "friends") + $"/v1/users/{userId}/friends/statuses?userIds=" + string.Join(",", others);
        using var req = Request(HttpMethod.Get, url, cookie);
        using var res = await Http.SendAsync(req, ct);
        string body = await res.Content.ReadAsStringAsync(ct);
        if (res.StatusCode == HttpStatusCode.TooManyRequests)
            throw new RobloxApiException(L.T("Roblox is rate-limiting requests; try again shortly."), rateLimited: true);
        if (!res.IsSuccessStatusCode)
            throw Refused("Friend statuses", res.StatusCode, body, cookieInvalid: res.StatusCode == HttpStatusCode.Unauthorized);
        if (DecodeJson(body) is { ValueKind: JsonValueKind.Object } root && root.TryGetProperty("data", out var data) && data.ValueKind == JsonValueKind.Array)
            foreach (var d in data.EnumerateArray())
                if (d.TryGetProperty("id", out var id) && id.TryGetInt64(out long other) && d.TryGetProperty("status", out var s) && s.ValueKind == JsonValueKind.String)
                    result[other] = s.GetString() ?? "";
        return result;
    }

    /// <summary>Roblox's answer to a friend request or an acceptance: HTTP status, its error codes, and the check holding it
    /// (rblx-challenge-* headers), if any.</summary>
    public sealed record FriendAnswer(int Http, List<int> Codes, JoinChallenge? Challenge)
    {
        public bool Ok => Http is >= 200 and < 300;
    }

    /// <summary>The signed-in account asks <paramref name="targetUserId"/> to be friends; <paramref name="answered"/> carries a
    /// check passed, as roblox.com asks again once its check is solved.</summary>
    public static Task<FriendAnswer> RequestFriendshipAsync(string cookie, long targetUserId, string realm = Realm.Global,
        JoinChallenge? answered = null, CancellationToken ct = default) =>
        FriendPostAsync(Realm.Api(realm, "friends") + $"/v1/users/{targetUserId}/request-friendship",
            JsonSerializer.Serialize(new { friendshipOriginSourceType = 0 }), cookie, realm, answered, "Friend request", ct);

    /// <summary>The signed-in account accepts the request <paramref name="requesterUserId"/> sent it.</summary>
    public static Task<FriendAnswer> AcceptFriendRequestAsync(string cookie, long requesterUserId, string realm = Realm.Global,
        JoinChallenge? answered = null, CancellationToken ct = default) =>
        FriendPostAsync(Realm.Api(realm, "friends") + $"/v1/users/{requesterUserId}/accept-friend-request",
            "{}", cookie, realm, answered, "Friend accept", ct);

    static async Task<FriendAnswer> FriendPostAsync(string url, string json, string cookie, string realm, JoinChallenge? answered, string what,
        CancellationToken ct)
    {
        using var res = await PostWithCsrfAsync(url, cookie, json, answered is null ? null : req =>
        {
            req.Headers.TryAddWithoutValidation("rblx-challenge-id", answered.Id);
            req.Headers.TryAddWithoutValidation("rblx-challenge-type", answered.Type);
            req.Headers.TryAddWithoutValidation("rblx-challenge-metadata", Convert.ToBase64String(Encoding.UTF8.GetBytes(answered.MetadataJson)));
        }, ct, realm);
        string body = await res.Content.ReadAsStringAsync(ct);
        JoinChallenge? challenge = null;
        if (Header(res, "rblx-challenge-id") is { Length: > 0 } challengeId)
        {
            var meta = DecodeMetadata(Header(res, "rblx-challenge-metadata"));
            challenge = new JoinChallenge(challengeId, Header(res, "rblx-challenge-type") ?? "", meta?.GetRawText() ?? "{}");
        }
        var (codes, summary) = Errors(body);
        // Codes and Roblox's messages only: never the rest of the reply.
        Log.Note($"{what}: HTTP {(int)res.StatusCode}{(challenge is null ? "" : $", held for a check ({SafeName(challenge.Type)})")}"
            + (res.IsSuccessStatusCode ? "" : $", errors {summary}"));
        if (res.StatusCode == HttpStatusCode.Unauthorized)
            throw new RobloxApiException(L.T("This account was signed out — sign in to it again."), cookieInvalid: true);
        if (res.StatusCode == HttpStatusCode.TooManyRequests && challenge is null)
            throw new RobloxApiException(L.T("Roblox is rate-limiting requests; try again shortly."), rateLimited: true);
        return new FriendAnswer((int)res.StatusCode, codes, challenge);
    }
}
