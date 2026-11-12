using System.Text.Json;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// The accounts that played a game in 7 days, summed from the stays (AppController.GameAccountsOf). The GPUI interface
/// makes the same sum itself (ui-gpui/src/history.rs, since 2026-11-11), so one table, Tests/game-accounts-cases.json,
/// holds both to the same answers: this suite runs it against the C# sum, the Rust test
/// `game_accounts_agree_with_the_app` against the Rust one. Its times are minutes from its `now`.
/// </summary>
static class GameAccountsSuite
{
    public static Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        string? folder = AppContext.BaseDirectory;
        while (folder is not null && !File.Exists(Path.Combine(folder, "Tests", "game-accounts-cases.json")))
            folder = Path.GetDirectoryName(folder);
        c.Check("The table of cases is found", folder is not null);
        if (folder is null)
            return Task.CompletedTask;
        using var doc = JsonDocument.Parse(File.ReadAllText(Path.Combine(folder, "Tests", "game-accounts-cases.json")));
        var now = DateTimeOffset.FromUnixTimeMilliseconds(doc.RootElement.GetProperty("now").GetInt64()).UtcDateTime;
        DateTime At(JsonElement minutes) => now.AddMinutes(minutes.GetDouble());
        string? Text(JsonElement x, string k) => x.TryGetProperty(k, out var v) && v.ValueKind == JsonValueKind.String ? v.GetString() : null;
        int n = 0;
        foreach (var x in doc.RootElement.GetProperty("cases").EnumerateArray())
        {
            n++;
            var sessions = x.GetProperty("sessions").EnumerateArray().Select(s => new PlaySession
            {
                AccountId = Guid.Parse(s.GetProperty("account").GetString()!), AccountLabel = Text(s, "label"),
                PlaceId = s.GetProperty("place").GetInt64(), UniverseId = s.GetProperty("universe").GetInt64(),
                StartedAt = At(s.GetProperty("start")), EndedAt = s.GetProperty("end").ValueKind == JsonValueKind.Number ? At(s.GetProperty("end")) : null,
                EndReason = Text(s, "reason"), ReasonCode = s.GetProperty("code").ValueKind == JsonValueKind.Number ? s.GetProperty("code").GetInt32() : null,
            }).ToList();
            var known = x.GetProperty("games").EnumerateObject().ToDictionary(g => long.Parse(g.Name),
                g => new AppController.KnownGame(g.Value.GetProperty("universe").GetInt64(), g.Value.GetProperty("name").GetString() ?? "", g.Value.GetProperty("root").GetInt64()));
            var got = AppController.GameAccountsOf(sessions, p => known.GetValueOrDefault(p), x.GetProperty("place").GetInt64(), x.GetProperty("universe").GetInt64(), now);
            var want = x.GetProperty("expect").EnumerateArray().ToList();
            bool same = got.Count == want.Count && got.Zip(want).All(p => p.First.Id == Guid.Parse(p.Second.GetProperty("account").GetString()!)
                && Math.Abs(p.First.Seconds - p.Second.GetProperty("seconds").GetDouble()) < 1e-6 && p.First.Kicks == p.Second.GetProperty("kicks").GetInt32()
                && p.First.Idle == p.Second.GetProperty("idle").GetBoolean() && p.First.Drops == p.Second.GetProperty("drops").GetInt32()
                && p.First.Now == p.Second.GetProperty("now").GetBoolean() && p.First.Last == At(p.Second.GetProperty("last")) && p.First.Label == Text(p.Second, "label"));
            c.Check(x.GetProperty("what").GetString() ?? "", same,
                string.Join("; ", got.Select(g => $"{g.Id.ToString()[..4]} {g.Seconds} s, {g.Kicks} kicks{(g.Idle ? " idle" : "")}, {g.Drops} drops{(g.Now ? ", now" : "")}, last {(g.Last - now).TotalMinutes} min, \"{g.Label}\"")));
        }
        c.Check("The table has its cases", n >= 10, n);
        return Task.CompletedTask;
    }
}
