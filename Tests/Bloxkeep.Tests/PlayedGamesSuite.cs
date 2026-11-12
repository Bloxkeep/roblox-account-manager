using System.Text.Json;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// The games played in the last 90 days, summed from the stays (AppController.PlayedGamesOf). The GPUI interface makes
/// the same sum itself (ui-gpui/src/history.rs, since 2026-11-11), so one table, Tests/played-games-cases.json, holds
/// both to the same answers: this suite runs it against the C# sum, the Rust test `agrees_with_the_app` against the
/// Rust one. The table's times are minutes from its `now` (before it when below zero).
/// </summary>
static class PlayedGamesSuite
{
    public static Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        string? folder = AppContext.BaseDirectory;
        while (folder is not null && !File.Exists(Path.Combine(folder, "Tests", "played-games-cases.json")))
            folder = Path.GetDirectoryName(folder);
        c.Check("The table of cases is found", folder is not null);
        if (folder is null)
            return Task.CompletedTask;
        using var doc = JsonDocument.Parse(File.ReadAllText(Path.Combine(folder, "Tests", "played-games-cases.json")));
        var now = DateTimeOffset.FromUnixTimeMilliseconds(doc.RootElement.GetProperty("now").GetInt64()).UtcDateTime;
        DateTime At(JsonElement minutes) => now.AddMinutes(minutes.GetDouble());
        DateTime? AtOrNone(JsonElement x, string k) => x.TryGetProperty(k, out var v) && v.ValueKind == JsonValueKind.Number ? At(v) : null;
        int n = 0;
        foreach (var x in doc.RootElement.GetProperty("cases").EnumerateArray())
        {
            n++;
            var sessions = x.GetProperty("sessions").EnumerateArray().Select(s => new PlaySession
            {
                PlaceId = s.GetProperty("place").GetInt64(), UniverseId = s.GetProperty("universe").GetInt64(), GameName = s.GetProperty("game").GetString() ?? "",
                StartedAt = At(s.GetProperty("start")), EndedAt = AtOrNone(s, "end"), LastSeenAt = AtOrNone(s, "seen"),
            }).ToList();
            var known = x.GetProperty("games").EnumerateObject().ToDictionary(g => long.Parse(g.Name),
                g => new AppController.KnownGame(g.Value.GetProperty("universe").GetInt64(), g.Value.GetProperty("name").GetString() ?? "", g.Value.GetProperty("root").GetInt64()));
            var got = AppController.PlayedGamesOf(sessions, p => known.GetValueOrDefault(p), now);
            var want = x.GetProperty("expect").EnumerateArray().ToList();
            bool same = got.Count == want.Count && got.Zip(want).All(p => p.First.Name == p.Second.GetProperty("name").GetString()
                && p.First.Place == p.Second.GetProperty("place").GetInt64() && Math.Abs(p.First.Hours - p.Second.GetProperty("hours").GetDouble()) < 1e-9
                && p.First.Last == At(p.Second.GetProperty("last")));
            c.Check(x.GetProperty("what").GetString() ?? "", same, string.Join("; ", got.Select(g => $"{g.Name} place {g.Place} {g.Hours} h")));
        }
        c.Check("The table has its cases", n >= 12, n);
        return Task.CompletedTask;
    }
}
