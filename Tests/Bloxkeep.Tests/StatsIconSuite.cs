using System.Text.Json;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// Statistics with nothing in the game caches (a fresh start, games played days ago): "playStats" comes back with every
/// game's and preset's icon. Asks Roblox's public game and thumbnail servers, so it runs with --online only.
/// </summary>
static class StatsIconSuite
{
    public static async Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        var seed = Vault.Create(Harness.Password);
        var accounts = Enumerable.Range(0, 3).Select(i => new Account { Username = $"Test{i}", UserId = 1000 + i }).ToList();
        seed.Data.Accounts.AddRange(accounts);
        var farm = new LaunchPreset { Name = "Farm", AccountIds = [accounts[0].Id], PlaceInput = "142823291", GameName = "Murder Mystery 2" };
        seed.Data.Settings.Presets.Add(farm);
        var now = DateTime.UtcNow;
        void Stay(int who, long place, long universe, string game, double startHoursAgo, double hours) =>
            seed.Data.PlaySessions.Add(new PlaySession { AccountId = accounts[who].Id, AccountLabel = accounts[who].Username, PlaceId = place, UniverseId = universe, GameName = game,
                StartedAt = now.AddHours(-startHoursAgo), EndedAt = now.AddHours(-startHoursAgo + hours), EndReason = "left", PresetId = who == 0 ? farm.Id : null });
        Stay(0, 116495829188952, 6716940848, "Dead Rails", 30, 1.6);
        Stay(1, 2753915549, 994732206, "Blox Fruits", 50, 1.0);
        Stay(2, 126884695634066, 0, "Grow a Garden", 70, 0.8); // place only: its game is looked up first
        Stay(0, 920587237, 383310974, "Adopt Me!", 5, 0.5);
        seed.Save();
        using var form = new Form();
        using var app = new AppController(form, _ => { });
        await Harness.Unlock(app);
        var json = JsonSerializer.SerializeToElement(await Harness.Call(app, "playStats", new { days = 7 }));
        foreach (var g in json.GetProperty("games").EnumerateArray())
            c.Check($"icon for {g.GetProperty("name").GetString()}", g.GetProperty("iconUrl").ValueKind == JsonValueKind.String);
        foreach (var p in json.GetProperty("presets").EnumerateArray().Where(p => p.GetProperty("id").GetGuid() != Guid.Empty))
            c.Check($"icon for preset {p.GetProperty("name").GetString()}", p.GetProperty("iconUrl").ValueKind == JsonValueKind.String);
    }
}
