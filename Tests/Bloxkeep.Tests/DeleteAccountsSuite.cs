using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// Deleting accounts (AppController.Accounts.cs): what refers to them goes too. A macro aimed at the accounts picked by hand
/// loses the deleted ones; one left with none is turned off (its triggers would fire into nothing, and its next edit would
/// fail with "Pick at least one account"), as a preset left empty has its schedule turned off.
/// </summary>
static class DeleteAccountsSuite
{
    public static async Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        Account a = new() { Username = "AlphaOne", UserId = 101 }, b = new() { Username = "BetaTwo", UserId = 102 }, other = new() { Username = "GammaThree", UserId = 103 };
        var seed = Vault.Create(Harness.Password);
        seed.Data.Accounts.AddRange([a, b, other]);
        Macro Make(string name, string targets, params Account[] on) => new()
        {
            Id = Guid.NewGuid(), Name = name, Targets = targets, Enabled = true, AccountIds = on.Select(x => x.Id).ToList(),
            Trigger = new MacroTrigger { Kind = "join" }, Steps = [new MacroStep { Kind = "wait", Ms = 100 }],
        };
        var onlyA = Make("only A", "accounts", a);
        var aAndB = Make("A and B", "accounts", a, b);
        var onlyOther = Make("only the other", "accounts", other);
        var everyone = Make("all, with an old id", "all", a);
        seed.Data.Settings.Macros.AddRange([onlyA, aAndB, onlyOther, everyone]);
        seed.Save();
        seed.Dispose();

        using var form = new Form();
        using var app = new AppController(form, _ => { });
        await Harness.Unlock(app);
        var vault = (Vault)typeof(AppController).GetField("_vault", Harness.F)!.GetValue(app)!;
        await Harness.Call(app, "deleteAccounts", new { ids = new[] { a.Id } });
        Macro Find(Guid id) => vault.Data.Settings.Macros.Single(m => m.Id == id);

        c.Check("a macro aimed at the deleted account alone: no accounts left, and turned off",
            Find(onlyA.Id).AccountIds.Count == 0 && !Find(onlyA.Id).Enabled, $"{Find(onlyA.Id).AccountIds.Count} {Find(onlyA.Id).Enabled}");
        c.Check("a macro aimed at it and another: keeps the other, stays on",
            Find(aAndB.Id).AccountIds.SequenceEqual([b.Id]) && Find(aAndB.Id).Enabled);
        c.Check("a macro aimed at others: untouched", Find(onlyOther.Id).AccountIds.SequenceEqual([other.Id]) && Find(onlyOther.Id).Enabled);
        c.Check("a macro for every account: the old id is dropped, it stays on",
            Find(everyone.Id).AccountIds.Count == 0 && Find(everyone.Id).Enabled && Find(everyone.Id).Targets == "all");
        c.Check("the accounts themselves are gone", vault.Data.Accounts.Select(x => x.Id).SequenceEqual([b.Id, other.Id]));
    }
}
