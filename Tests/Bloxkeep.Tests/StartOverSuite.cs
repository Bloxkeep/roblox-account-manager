using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// A forgotten master password (the lock screen's Start over, Vault.SetAside): the vault and every spare copy of it are
/// moved aside, never deleted, so nothing brings the old vault back at the next start and a new master password sets up
/// afresh; the set-aside file still opens with the old password, the way a backup is restored.
/// </summary>
static class StartOverSuite
{
    const string OldPassword = "old password nobody remembers";

    public static async Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        var old = Vault.Create(OldPassword);
        old.Data.Accounts.Add(new Account { Username = "Kept", UserId = 41 });
        old.Save();
        old.Save(); // a second save leaves the first as vault.rmv.bak
        old.Dispose();
        string data = Vault.DataDir;
        c.Check("before: a vault and its spare copy", File.Exists(Vault.VaultPath) && File.Exists(Path.Combine(data, "vault.rmv.bak")));

        using var form = new Form();
        using var app = new AppController(form, _ => { });
        await Harness.Call(app, "startOver", new { });
        var set = Directory.GetFiles(data, "vault-before-start-over-*").Select(f => Path.GetFileName(f)!).ToList();
        c.Check("Start over moves the vault and its spare copy aside", !File.Exists(Vault.VaultPath)
            && !File.Exists(Path.Combine(data, "vault.rmv.bak")) && set.Count == 2, string.Join(", ", set));
        c.Check("and nothing brings it back: Bloxkeep sets up afresh", !Vault.Exists);

        // Remembered later: the set-aside file opens with the old password, accounts and all.
        string kept = Path.Combine(data, set.Single(n => !n.Contains("-spare")));
        File.Copy(kept, Vault.VaultPath);
        bool opens;
        try
        {
            using var back = Vault.Open(OldPassword);
            opens = back.Data.Accounts.Any(a => a.Username == "Kept");
        }
        catch (Exception)
        {
            opens = false;
        }
        c.Check("the set-aside file is the old vault, opened with the old password", opens);
        File.Delete(Vault.VaultPath);

        await Harness.Call(app, "createVault", new { password = "a brand new password" });
        c.Check("a new master password starts an empty vault", Harness.Get<Vault>(app, "_vault").Data.Accounts.Count == 0);
        c.Check("the set-aside files are still there", set.All(n => File.Exists(Path.Combine(data, n))));
    }
}
