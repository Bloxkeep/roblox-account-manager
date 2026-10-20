using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// Each account's Robux on its card (AppController.Robux.cs, RobloxApi.GetRobuxAsync): what Roblox's answers read as,
/// that the first round starts at unlocking, that the card keeps the number's place until its first answer, and that
/// the page gets the number. And Roblox VNG accounts open like
/// the others now (a ticket from Roblox VNG's sign-in), never with Roblox VNG's own client. Offline: nothing is asked of
/// Roblox.
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
        Harness.Set(app, "_networkUp", true); // as on a PC online, whatever this one is
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

        // Roblox VNG's client is never what Default opens Roblox with, even when its installer took the links.
        c.Check("Roblox VNG's client is told by its folder",
            RobloxInstalls.IsVngClient(@"C:\Users\A\AppData\Local\RobloxPCVNG\Versions\version-1\RobloxPlayerBeta.exe")
            && !RobloxInstalls.IsVngClient(@"C:\Users\A\AppData\Local\Roblox\Versions\version-1\RobloxPlayerBeta.exe")
            && !RobloxInstalls.IsVngClient(@"D:\bloxtrap\Bloxstrap.exe"));
        var fallback = typeof(RobloxLauncher).GetMethod("FallbackLauncher", Harness.F)!.Invoke(null, null);
        string? fallbackExe = fallback?.GetType().GetField("Item1")!.GetValue(fallback) as string;
        c.Check("…and never what stands in for the links", fallbackExe is null || !RobloxInstalls.IsVngClient(fallbackExe), fallbackExe);
        c.Check("…nor the client Default counts as its own", RobloxInstalls.ClientFor("auto") is not { } own || !RobloxInstalls.IsVngClient(own));

        // The unlock started the first round already (not 20 s later, as before): the next is half an hour away. The
        // account has no cookie, so nothing was asked of Roblox.
        var next = Harness.Get<DateTime>(app, "_nextRobux");
        c.Check("the first round starts at unlocking, alongside status and avatars", next - DateTime.UtcNow > TimeSpan.FromMinutes(29), next - DateTime.UtcNow);

        object? Field(string name) { object dto = Harness.Invoke(app, "AccountDto", account)!; return dto.GetType().GetProperty(name)!.GetValue(dto); }
        account.Cookie = "_|WARNING:-DO-NOT-SHARE-THIS.--test";
        Harness.Set(app, "_networkUp", true);
        c.Check("a signed-in account not answered yet keeps the number's place on its card", Field("robuxWait") is true && Field("robux") is null);
        Harness.Get<HashSet<Guid>>(app, "_robuxAsked").Add(account.Id);
        c.Check("…given back once asked, when Roblox gave no number", Field("robuxWait") is false);
        Harness.Get<Dictionary<Guid, long>>(app, "_robux")[account.Id] = 1234;
        c.Check("the card gets the account's Robux", Field("robux") is long n && n == 1234 && Field("robuxWait") is false, Field("robux"));
        Harness.Get<HashSet<Guid>>(app, "_robuxAsked").Clear();
        Harness.Get<Dictionary<Guid, long>>(app, "_robux").Clear();
        Harness.Set(app, "_networkUp", false);
        c.Check("…and offline, no place kept for a number that won't come", Field("robuxWait") is false);
        account.Cookie = "";
    }
}
