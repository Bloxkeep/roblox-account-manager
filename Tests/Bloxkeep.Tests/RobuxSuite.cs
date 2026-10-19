using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// Each account's Robux on its card (AppController.Robux.cs, RobloxApi.GetRobuxAsync): what Roblox's answers read as,
/// that a round waits a little after unlocking, and that the page gets the number. And Roblox VNG accounts open like
/// the others now (a ticket from Roblox VNG's sign-in). Offline: nothing is asked of Roblox.
/// </summary>
static class RobuxSuite
{
    public static async Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        long? Read(string body) => (long?)typeof(RobloxApi).GetMethod("ReadRobux", Harness.F)!.Invoke(null, [body]);
        c.Check("Roblox's answer {\"robux\": 1234} reads as 1234", Read("{\"robux\":1234}") == 1234);
        c.Check("…and 0 as 0", Read("{\"robux\":0}") == 0);
        c.Check("an error, or an answer that isn't the balance, reads as nothing",
            Read("{\"errors\":[{\"code\":9002}]}") is null && Read("<html>") is null && Read("[]") is null && Read("{\"robux\":-5}") is null);

        var seed = Vault.Create(Harness.Password);
        seed.Data.Accounts.Add(new Account { Username = "Vng", UserId = 31, Realm = Realm.Vng });
        seed.Save();
        using var form = new Form();
        using var app = new AppController(form, _ => { });
        await Harness.Unlock(app);
        var account = Harness.Get<Vault>(app, "_vault").Data.Accounts.Single();

        string? refused = null;
        try
        {
            Harness.Invoke(app, "CheckCanLaunch", new List<Account> { account });
        }
        catch (System.Reflection.TargetInvocationException ex)
        {
            refused = ex.InnerException?.Message;
        }
        c.Check("a Roblox VNG account alone may be opened (it was refused before)", refused is null, refused);

        var now = DateTime.UtcNow;
        Harness.Invoke(app, "RefreshRobuxIfDue", now);
        var next = Harness.Get<DateTime>(app, "_nextRobux");
        c.Check("right after unlocking, the first round waits its turn", next > now && next - now <= TimeSpan.FromMinutes(1) && Harness.Get<bool>(app, "_robuxBusy") == false,
            next - now);

        Harness.Get<Dictionary<Guid, long>>(app, "_robux")[account.Id] = 1234;
        object dto = Harness.Invoke(app, "AccountDto", account)!;
        object? robux = dto.GetType().GetProperty("robux")!.GetValue(dto);
        c.Check("the card gets the account's Robux", robux is long n && n == 1234, robux);
    }
}
