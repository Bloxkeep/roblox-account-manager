using System.Text.Json;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// What a game box holds, read as a place to go to (GameTarget.TryParse). The interface reads the same words its own way
/// (ui-gpui/src/target.rs, moved there from here on 2026-11-11), so one table of cases, Tests/game-target-cases.json,
/// holds both to the same answers: this suite runs it against the C# reading, the Rust test `agrees_with_the_app`
/// against the Rust one.
/// </summary>
static class GameTargetSuite
{
    public static Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        string? folder = AppContext.BaseDirectory;
        while (folder is not null && !File.Exists(Path.Combine(folder, "Tests", "game-target-cases.json")))
            folder = Path.GetDirectoryName(folder);
        c.Check("The table of cases is found", folder is not null);
        if (folder is null)
            return Task.CompletedTask;
        using var doc = JsonDocument.Parse(File.ReadAllText(Path.Combine(folder, "Tests", "game-target-cases.json")));
        int n = 0;
        foreach (var x in doc.RootElement.EnumerateArray())
        {
            n++;
            string? Text(string k) => x.TryGetProperty(k, out var v) && v.ValueKind == JsonValueKind.String ? v.GetString() : null;
            string input = Text("in") ?? "", job = Text("job") ?? "";
            bool ok = GameTarget.TryParse(input, job, out var t, out string error);
            string what = $"\"{input}\"{(job.Length > 0 ? $" with job \"{job}\"" : "")}";
            if (x.GetProperty("ok").GetBoolean())
                c.Check($"{what} is taken", ok && t is not null && t.PlaceId == x.GetProperty("place").GetInt64() && t.JobId == Text("jobId") && t.LinkCode == Text("link")
                    && t.ShareCode == Text("share") && t.AccessCode == Text("access"),
                    ok ? $"place {t!.PlaceId}, job {t.JobId}, link {t.LinkCode}, share {t.ShareCode}, access {t.AccessCode}" : error);
            else
                c.Check($"{what} is refused, with its reason", !ok && error == Text("error"), ok ? "taken" : error);
        }
        c.Check("The table has its cases", n >= 30, n);
        return Task.CompletedTask;
    }
}
