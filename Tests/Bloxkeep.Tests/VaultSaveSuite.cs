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
            try
            {
                using var opened = Vault.Open(password);
                return opened.Data.Accounts.FirstOrDefault()?.Alias ?? "";
            }
            catch (InvalidPasswordException)
            {
                return $"(the file doesn't open with {password})";
            }
        }
        var account = new Account { Username = "bg_save_user", Alias = "first", UserId = 77 };
        vault.Data.Accounts.Add(account);

        // A background save of the vault as it is now, held once it has taken the vault and before it reaches the file
        // (where a save made now can take the file first), while what comes meanwhile is done; false when it never got there.
        async Task<bool> HeldWhile(Action meanwhile)
        {
            var pause = typeof(Vault).GetProperty("BeforeBackgroundWrite", Harness.F)!;
            using var reached = new ManualResetEventSlim();
            using var go = new ManualResetEventSlim();
            pause.SetValue(null, (Action)(() =>
            {
                reached.Set();
                go.Wait(TimeSpan.FromSeconds(10));
            }));
            try
            {
                var save = vault.SaveInBackground();
                bool held = reached.Wait(TimeSpan.FromSeconds(10));
                meanwhile();
                go.Set();
                await save;
                return held;
            }
            finally
            {
                pause.SetValue(null, null);
            }
        }

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

        // The same with the older one already on its way: it had taken the vault but not yet the file when the save
        // made now wrote. The check above caught it by chance (once on 2026-10-22, with the UI checks running; 31 of
        // 2,000 times in a row on an idle PC); held here, it happens every time.
        account.Alias = "older, held on its way";
        bool held = await HeldWhile(() =>
        {
            account.Alias = "newer, saved while the older was held";
            vault.Save();
        });
        c.Check("…nor by an older one that was on its way to the file", held && Alias(Harness.Password) == "newer, saved while the older was held", Alias(Harness.Password));

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

        // …nor when the save under the old one was already on its way: the new password still opens the file.
        account.Alias = "under the second password, on its way";
        held = await HeldWhile(() =>
        {
            account.Alias = "under the third password";
            vault.ChangePassword(Vault.PrepareKey("harness-new-pass-3"));
        });
        c.Check("…and when a save under the old one was on its way meanwhile", held && Alias("harness-new-pass-3") == "under the third password", Alias("harness-new-pass-3"));

        // Closing right after a save was asked for: it is written before the key goes.
        account.Alias = "saved just before closing";
        _ = vault.SaveInBackground();
        vault.Dispose();
        c.Check("closing the vault finishes the save on its way", Alias("harness-new-pass-3") == "saved just before closing", Alias("harness-new-pass-3"));

        // A file held by another program (an antivirus, a sync client): the save fails and says so; the next one writes.
        var again = Vault.Open("harness-new-pass-3");
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
        c.Check("…and the next save writes it", Alias("harness-new-pass-3") == "while the file is held", Alias("harness-new-pass-3"));
        again.Dispose();
    }
}
