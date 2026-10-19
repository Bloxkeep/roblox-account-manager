using System.Text.Json;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// Saved servers (AppController.SavedServers.cs): a private server's link is saved with its game, a link that isn't a
/// private server's is refused, the same link again updates the one saved, a code needs its game, and one is deleted.
/// Offline: nothing is asked of Roblox.
/// </summary>
static class SavedServersSuite
{
    public static async Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        var seed = Vault.Create(Harness.Password);
        seed.Save();
        using var form = new Form();
        using var app = new AppController(form, _ => { });
        await Harness.Unlock(app);
        var list = Harness.Get<Vault>(app, "_vault").Data.Settings.SavedServers;

        string? Save(object args)
        {
            try
            {
                Harness.Invoke(app, "SaveServer", JsonSerializer.SerializeToElement(args));
                return null;
            }
            catch (System.Reflection.TargetInvocationException ex)
            {
                return ex.InnerException?.Message ?? "refused";
            }
        }

        const string Private = "https://www.roblox.com/games/2753915549/Blox-Fruits?privateServerLinkCode=12345678901234567890";
        string? said = Save(new { name = "VIP farm", link = Private, code = "", gameName = "Blox Fruits" });
        c.Check("a private server's link is saved, with its game from the link", said is null && list is [{ Name: "VIP farm", PlaceId: 2753915549, Link: Private }], said);
        said = Save(new { name = "Renamed", link = Private, code = "4F7K2Q" });
        c.Check("…the same link again updates the one saved (name, code), not a second", said is null && list is [{ Name: "Renamed", Code: "4F7K2Q" }], said);
        said = Save(new { name = "Public", link = "https://www.roblox.com/games/2753915549/Blox-Fruits", code = "" });
        c.Check("a game's link that isn't a private server's is refused", said is not null && list.Count == 1, said);
        said = Save(new { name = "Code only", link = "", code = "ZX81" });
        c.Check("a code without its game is refused", said is not null && list.Count == 1, said);
        said = Save(new { name = "", link = "", code = "ZX81", placeId = 920587237L, gameName = "Adopt Me!" });
        c.Check("…with its game it is saved, named after the game, newest first",
            said is null && list.Count == 2 && list[0] is { Name: "Adopt Me!", Code: "ZX81", PlaceId: 920587237, Link: "" }, said);
        said = Save(new { name = "Nothing", link = "", code = "", placeId = 920587237L });
        c.Check("neither a link nor a code: refused", said is not null && list.Count == 2, said);
        Harness.Invoke(app, "DeleteServer", list[0].Id);
        c.Check("a saved server is deleted", list is [{ Name: "Renamed" }]);
    }
}
