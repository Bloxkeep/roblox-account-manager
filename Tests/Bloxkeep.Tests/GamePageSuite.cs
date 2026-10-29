using System.Net;
using System.Text.Json;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// A game's page (AppController.GamePage.cs, GameHours), against a stand-in Roblox: its figures (likes from the votes,
/// the game's own last update over the universe's, nothing without a cookie but what anyone sees); your accounts in it
/// over 7 days, any of its places (a stay outside the 7 days or in another game left out; kicks by the game or for
/// idling apart from other drops; one playing it now); the hours it's busy, noted and kept; a game Roblox doesn't know.
/// </summary>
static class GamePageSuite
{
    const long Place = 2753915549, OtherPlace = 4442272183, Universe = 994732206, OtherGame = 1;

    /// <summary>GameHours forgets what it read (it is read again from the data folder).</summary>
    static void ResetHours() => typeof(GameHours).GetMethod("Reset", Harness.F)!.Invoke(null, null);

    public static async Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        ResetHours();

        // ── the hours ──
        var noon = new DateTime(2026, 10, 29, 12, 0, 0, DateTimeKind.Local).ToUniversalTime();
        GameHours.Note(Universe, 1000, noon);
        GameHours.Note(Universe, 2000, noon.AddDays(1));
        GameHours.Note(Universe, 0, noon.AddHours(1)); // nothing said: not noted
        var hours = GameHours.Of(Universe);
        c.Check("an hour's figure leans on the newest (1000, then 2000: 1300), other hours unknown",
            hours is { Length: 24 } && hours[12] == 1300 && hours[13] is null && hours.Count(h => h is not null) == 1, hours is null ? "null" : string.Join(",", hours));
        GameHours.Save();
        ResetHours();
        c.Check("…kept in the data folder", GameHours.Of(Universe)?[12] == 1300 && File.Exists(Path.Combine(dir, "game-hours.json")));
        for (long g = 100; g < 145; g++)
            GameHours.Note(g, 5, noon.AddDays(2).AddMinutes(g));
        c.Check("…for the 40 games noted last", GameHours.Of(Universe) is null && GameHours.Of(144) is not null && GameHours.Of(104) is null && GameHours.Of(105) is not null);
        GameHours.Note(Universe, 1300, noon);

        // ── the accounts ──
        var now = DateTime.UtcNow;
        var seed = Vault.Create(Harness.Password);
        var farm = new Account { Username = "Farm", UserId = 4001, Cookie = "_TEST_gp_A", CookieValid = true };
        var main = new Account { Username = "Main", UserId = 4002, Cookie = "_TEST_gp_B", CookieValid = true };
        seed.Data.Accounts.AddRange([farm, main]);
        PlaySession Stay(Account a, long place, long universe, double hoursAgo, double length, string? end = "left", int? code = null) => new()
        {
            AccountId = a.Id, AccountLabel = a.Username, PlaceId = place, UniverseId = universe, GameName = "Blox Fruits",
            StartedAt = now.AddHours(-hoursAgo), EndedAt = end is null ? null : now.AddHours(-hoursAgo + length), EndReason = end, ReasonCode = code,
        };
        seed.Data.PlaySessions.AddRange([
            Stay(farm, Place, Universe, 30, 10, "disconnected", 278),         // 10 h, kicked for idling
            Stay(farm, OtherPlace, 0, 20, 2, "disconnected", 277),            // another place of the game (universe learned below): 2 h, a drop
            Stay(farm, OtherPlace, Universe, 15, 1),                          // 1 h
            Stay(main, Place, Universe, 9 * 24, 5),                           // more than 7 days ago
            Stay(main, Place, Universe, 50, 0.5, "disconnected", 267),        // 30 min, kicked by the game
            Stay(main, 999, OtherGame, 3, 2),                                 // another game
        ]);
        seed.Save();
        seed.Dispose();
        using var form = new Form();
        using var app = new AppController(form, _ => { });
        SynchronizationContext.SetSynchronizationContext(null);
        await Harness.Unlock(app);
        // In it now (1 h so far): added once unlocked, or the unlock ends it (no client of it is running here).
        Harness.Get<Vault>(app, "_vault").Data.PlaySessions.Add(Stay(farm, Place, Universe, 1, 0, end: null));
        var rows = JsonSerializer.SerializeToElement(Harness.Invoke(app, "GameAccounts", Place, Universe, now)).EnumerateArray().ToList();
        string Row(JsonElement r) => $"{r.GetProperty("name").GetString()} {r.GetProperty("minutes").GetInt32()} k{r.GetProperty("kicks").GetInt32()} d{r.GetProperty("drops").GetInt32()} {r.GetProperty("now").GetString()}";
        c.Check("your accounts in it over 7 days, any of its places, most played first: time, kicks, drops, now",
            rows.Count == 2 && Row(rows[0]) == "Farm 840 k1 d1 ingame" && Row(rows[1]) == "Main 30 k1 d0 ", string.Join(" | ", rows.Select(Row)));
        c.Check("…an idle kick said as AFK, a kick by the game not", rows[0].GetProperty("kickWhy").GetString() == "AFK"
            && rows[1].GetProperty("kickWhy").ValueKind == JsonValueKind.Null);
        c.Check("…when one not in it now last played", rows[1].GetProperty("lastPlayed").ValueKind == JsonValueKind.String
            && rows[0].GetProperty("lastPlayed").ValueKind == JsonValueKind.Null);

        // ── Roblox's figures ──
        var asked = new List<string>();
        typeof(RobloxApi).GetProperty("StandIn", Harness.F)!.SetValue(null, (Func<HttpRequestMessage, Task<HttpResponseMessage>>)(req =>
        {
            string url = req.RequestUri!.ToString();
            bool signedIn = req.Headers.TryGetValues("Cookie", out var cookies) && cookies.Any(v => v.Contains("_TEST_gp_"));
            lock (asked)
                asked.Add((signedIn ? "+" : "-") + req.RequestUri.Host);
            string json = url switch
            {
                _ when url.Contains("/universes/v1/places/123/") => """{"universeId":0}""",
                _ when url.Contains("/universes/v1/places/") => $$"""{"universeId":{{Universe}}}""",
                _ when url.Contains("develop.roblox.com") => $$"""{"data":[{"id":{{Universe}},"name":"Blox Fruits","description":"Become a master swordsman.","rootPlaceId":{{Place}},"creatorName":"Gamer Robot Inc","created":"2019-01-15T09:29:19.417Z","updated":"2026-09-21T18:36:21.337Z"}]}""",
                _ when url.Contains("/v1/games/votes") => $$"""{"data":[{"id":{{Universe}},"upVotes":900,"downVotes":100}]}""",
                _ when url.Contains("games.roblox.com/v1/games") => signedIn
                    ? $$"""{"data":[{"id":{{Universe}},"playing":412873,"visits":48216000000,"favoritedCount":8700000,"genre":"Adventure","updated":"2026-10-28T10:00:00Z"}]}"""
                    : """{"data":[{"id":0,"playing":0,"visits":0}]}""",
                _ when url.Contains("thumbnails") => $$"""{"data":[{"targetId":{{Universe}},"state":"Completed","imageUrl":"https://tr.rbxcdn.com/icon.png"}]}""",
                _ => "{}",
            };
            return Task.FromResult(new HttpResponseMessage(HttpStatusCode.OK) { Content = new StringContent(json) });
        }));
        var page = JsonSerializer.SerializeToElement(await Harness.Call(app, "gameDetails", new { placeId = Place }));
        c.Check("Roblox's figures: name, maker, players, visits, likes from the votes (90 %), genre",
            page.GetProperty("name").GetString() == "Blox Fruits" && page.GetProperty("creator").GetString() == "Gamer Robot Inc"
            && page.GetProperty("playing").GetInt64() == 412873 && page.GetProperty("visits").GetInt64() == 48216000000
            && page.GetProperty("likes").GetInt32() == 90 && page.GetProperty("genre").GetString() == "Adventure", page.ToString());
        c.Check("…the game's own last update (signed in) over the universe's", page.GetProperty("updated").GetDateTime() == new DateTime(2026, 10, 28, 10, 0, 0, DateTimeKind.Utc), page.GetProperty("updated").ToString());
        c.Check("…the player count asked with an account's sign-in (Roblox hides it otherwise), the rest without",
            asked.Contains("+games.roblox.com") && asked.Contains("-develop.roblox.com"), string.Join(",", asked));
        c.Check("…its hours, with the count just seen noted too", page.GetProperty("hours")[12].GetInt64() == 1300 && page.GetProperty("hours").GetArrayLength() == 24
            && page.GetProperty("accounts").GetArrayLength() == 2, page.GetProperty("hours").ToString());
        c.Check("a Place ID Roblox doesn't know: said so", await Harness.Try(app, "gameDetails", new { placeId = 123 }) is UserError e
            && e.Message == "Roblox has no game with this Place ID.");
        c.Check("an invalid one for its roblox.com page: refused, nothing opened", await Harness.Try(app, "openGamePage", new { placeId = 0 }) is UserError);
        c.Check("a wiki for no name: nothing opened", await Harness.Call(app, "openGameWiki", new { name = " " }) is null);
        ResetHours();
    }
}
