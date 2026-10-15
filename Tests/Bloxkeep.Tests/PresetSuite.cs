using System.Text.Json;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>Presets' options, a preset's own Discord link, closing a preset's windows, deleting accounts, signing in again.</summary>
static class PresetSuite
{
    public static async Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        var seed = Vault.Create(Harness.Password);
        var a1 = new Account { Username = "farm1", Cookie = "x1", UserId = 1001, CookieValid = true };
        var a2 = new Account { Username = "farm2", Cookie = "x2", UserId = 1002, CookieValid = true };
        var a3 = new Account { Username = "main", Cookie = "x3", UserId = 1003, CookieValid = true };
        seed.Data.Accounts.AddRange([a1, a2, a3]);
        seed.Save();
        using var form = new Form();
        using var app = new AppController(form, _ => { });
        Task<object?> Call(string cmd, object args) => Harness.Try(app, cmd, args);
        Vault V() => Harness.Get<Vault>(app, "_vault");
        var camel = new JsonSerializerOptions { PropertyNamingPolicy = JsonNamingPolicy.CamelCase };

        await Harness.Unlock(app);
        c.Check("unlocked", V() is not null);

        string hook = "https://discordapp.com/api/v10/webhooks/123456789012345678/" + new string('A', 68);
        var idObj = await Call("savePreset", new { name = "Farm", ids = new[] { a1.Id, a2.Id }, placeInput = "", arrange = true, discordWebhook = hook,
            schedule = new { enabled = true, time = "20:00", days = new[] { 1, 2, 3 }, date = "", stop = "never", stopHours = 0, stopTime = "" } });
        c.Check("savePreset returns an id", idObj is Guid, idObj);
        var id = (Guid)idObj!;
        var p = V().Data.Settings.Presets.Single(x => x.Id == id);
        c.Check("link normalised to discord.com", p.DiscordWebhook == "https://discord.com/api/webhooks/123456789012345678/" + new string('A', 68), p.DiscordWebhook);
        c.Check("arrange saved", p.Arrange == true);
        var dto = JsonSerializer.SerializeToElement(Harness.Invoke(app, "PresetDto", p), camel);
        c.Check("dto: masked link only", dto.GetProperty("discord").GetString() == "discord.com/api/webhooks/…5678/••••••", dto.GetProperty("discord").GetString());
        c.Check("dto: no raw link anywhere", !dto.GetRawText().Contains(new string('A', 20)));
        c.Check("dto: arrange", dto.GetProperty("arrange").GetBoolean());
        c.Check("dto: running 0", dto.GetProperty("running").GetInt32() == 0);

        await Call("savePreset", new { id, name = "Farm", ids = new[] { a1.Id, a2.Id }, placeInput = "", arrange = (bool?)null });
        p = V().Data.Settings.Presets.Single(x => x.Id == id);
        c.Check("no discordWebhook key keeps the link", p.DiscordWebhook.Length > 0);
        c.Check("arrange back to follow", p.Arrange is null);

        var bad = await Call("savePreset", new { id, name = "Farm", ids = new[] { a1.Id }, placeInput = "", discordWebhook = "https://example.com/hook" });
        c.Check("a link that isn't Discord's is refused", bad is UserError, (bad as Exception)?.Message);
        c.Check("refused save changed nothing", V().Data.Settings.Presets.Single(x => x.Id == id).AccountIds.Count == 2);

        await Call("savePreset", new { id, name = "Farm", ids = new[] { a1.Id, a2.Id }, placeInput = "", discordWebhook = "" });
        c.Check("'' goes back to Settings' channel", V().Data.Settings.Presets.Single(x => x.Id == id).DiscordWebhook == "");

        c.Check("test with a bad link says so", await Call("presetDiscordTest", new { id, url = "nope" }) is UserError);
        c.Check("test without any link says so", await Call("presetDiscordTest", new { id, url = "" }) is UserError);

        var closed = await Call("closePreset", new { id });
        c.Check("closePreset with nothing open: 0", closed is int n0 && n0 == 0, closed);

        var dup = await Call("duplicatePreset", new { id });
        var copy = V().Data.Settings.Presets.Single(x => x.Id == (Guid)dup!);
        c.Check("duplicate keeps arrange and webhook", copy.Arrange == p.Arrange && copy.DiscordWebhook == V().Data.Settings.Presets.Single(x => x.Id == id).DiscordWebhook);

        V().Data.Settings.LaunchOrder.AddRange([a1.Id, a2.Id, a3.Id]);
        V().Data.Discord.SummaryAccounts.AddRange([a1.Id, a3.Id]);
        await Call("deleteAccounts", new { ids = new[] { a1.Id, a2.Id } });
        var after = V().Data.Settings.Presets.Single(x => x.Id == id);
        c.Check("deleted accounts gone from the preset", after.AccountIds.Count == 0, after.AccountIds.Count);
        c.Check("empty preset's schedule switched off", after.Schedule is { Enabled: false }, after.Schedule?.Enabled);
        c.Check("launch order cleaned", !V().Data.Settings.LaunchOrder.Contains(a1.Id) && V().Data.Settings.LaunchOrder.Contains(a3.Id));
        c.Check("Discord summary accounts cleaned", V().Data.Discord.SummaryAccounts.SequenceEqual([a3.Id]));

        var acc = V().Data.Accounts.Single();
        acc.AutoRejoin = false;
        acc.RejoinOffBySignOut = true;
        typeof(AppController).GetMethod("SignedInAgain", Harness.F)!.Invoke(null, [acc]);
        c.Check("signed in again: auto-rejoin back on", acc.AutoRejoin && !acc.RejoinOffBySignOut);
        acc.RejoinOffBySignOut = true;
        await Call("setAutoRejoin", new { ids = new[] { acc.Id }, enabled = false });
        c.Check("own choice clears the flag", !acc.AutoRejoin && !acc.RejoinOffBySignOut);

        var state = JsonSerializer.SerializeToElement(Harness.Invoke(app, "BuildState"), camel);
        c.Check("state has lastSignInCheck", state.TryGetProperty("lastSignInCheck", out _));
        c.Check("state has the questions answered Don't ask again", state.TryGetProperty("skipConfirm", out _));
        await Call("skipConfirm", new { key = "deletePreset" });
        c.Check("Don't ask again is kept", Prefs.SkipConfirm.Contains("deletePreset"));
        c.Check("an unknown question is refused", await Call("skipConfirm", new { key = "deleteAccounts" }) is UserError && !Prefs.SkipConfirm.Contains("deleteAccounts"));
    }
}
