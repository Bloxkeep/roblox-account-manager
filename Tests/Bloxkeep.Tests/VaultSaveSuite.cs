using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// Saves written in the background (Vault.SaveInBackground, AppController's usual save): what reaches the disk is the
/// vault as it was when the save was asked for, a newer save wins over one still waiting, a save made now or a changed
/// password is never overwritten by an older one still on its way, closing the vault finishes the write first, and a
/// file that can't be written is reported and written by the next save.
/// </summary>
static class VaultSaveSuite
{
    public static async Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        var vault = Vault.Create(Harness.Password);
        string Alias(string password)
        {
            using var opened = Vault.Open(password);
            return opened.Data.Accounts.FirstOrDefault()?.Alias ?? "";
        }
        var account = new Account { Username = "bg_save_user", Alias = "first", UserId = 77 };
        vault.Data.Accounts.Add(account);

        // What the save took, not what the vault became after it.
        var first = vault.SaveInBackground();
        account.Alias = "changed after the save was asked for";
        await first;
        c.Check("a background save writes the vault as it was when asked for", Alias(Harness.Password) == "first", Alias(Harness.Password));
        c.Check("…and says when it's done", !vault.SavingInBackground);

        // Many saves in a row: the newest is on disk once they are done.
        var saves = new List<Task>();
        for (int i = 1; i <= 40; i++)
        {
            account.Alias = $"save {i}";
            saves.Add(vault.SaveInBackground());
        }
        await Task.WhenAll(saves);
        c.Check("forty saves in a row: the last one is on disk", Alias(Harness.Password) == "save 40", Alias(Harness.Password));

        // A save made now, right after one asked for in the background: the older one doesn't come after it.
        account.Alias = "older, in the background";
        var older = vault.SaveInBackground();
        account.Alias = "newer, saved now";
        vault.Save();
        await older;
        await Task.Delay(200);
        c.Check("a save made now isn't overwritten by an older one from the background", Alias(Harness.Password) == "newer, saved now", Alias(Harness.Password));

        // A changed password while a save under the old one waits: the file ends under the new password only.
        account.Alias = "under the old password";
        var stale = vault.SaveInBackground();
        account.Alias = "under the new password";
        vault.ChangePassword(Vault.PrepareKey("harness-new-pass-2"));
        await stale;
        await Task.Delay(200);
        bool oldOpens;
        try
        {
            using var old = Vault.Open(Harness.Password);
            oldOpens = true;
        }
        catch (InvalidPasswordException)
        {
            oldOpens = false;
        }
        c.Check("a changed password: the old one no longer opens the file", !oldOpens);
        c.Check("…and the new one opens what was saved under it", Alias("harness-new-pass-2") == "under the new password", Alias("harness-new-pass-2"));

        // Closing right after a save was asked for: it is written before the key goes.
        account.Alias = "saved just before closing";
        _ = vault.SaveInBackground();
        vault.Dispose();
        c.Check("closing the vault finishes the save on its way", Alias("harness-new-pass-2") == "saved just before closing", Alias("harness-new-pass-2"));

        // A file held by another program (an antivirus, a sync client): the save fails and says so; the next one writes.
        var again = Vault.Open("harness-new-pass-2");
        again.Data.Accounts[0].Alias = "while the file is held";
        Exception? failure = null;
        using (new FileStream(Vault.VaultPath, FileMode.Open, FileAccess.Read, FileShare.None))
        {
            try
            {
                await again.SaveInBackground();
            }
            catch (Exception ex)
            {
                failure = ex;
            }
        }
        c.Check("a file another program holds: the save fails with the reason", failure is IOException or UnauthorizedAccessException, failure?.GetType().Name);
        await again.SaveInBackground();
        c.Check("…and the next save writes it", Alias("harness-new-pass-2") == "while the file is held", Alias("harness-new-pass-2"));
        again.Dispose();
    }
}
