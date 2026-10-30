using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// Several vaults (Vaults.cs, AppController.Vaults.cs): the vault from before joins the list with its backup state; a
/// vault is added with its own name and master password; each keeps its own accounts and file (an open vault writes
/// only its own), the one last used comes back after a restart, the picker shows at start when asked for, and one taken
/// off the list is set aside, never deleted, but not the one open.
/// </summary>
static class VaultsSuite
{
    const string MainPassword = "main vault password", FarmPassword = "farm vault password";

    public static async Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        string data = Vault.DataDir;
        // The first vault set up on a PC: vault.rmv, and on the list as that file (review, 2026-10-30: the entry was made once
        // the file existed, so it named another file). Cleared after, for the PC from before below.
        using (var first = new AppController(new Form(), _ => { }))
        {
            await Harness.Call(first, "createVault", new { password = MainPassword, name = "" });
            string? firstFile = Harness.Get<Vault?>(first, "_vault")?.FilePath is { } fp ? Path.GetFileName(fp) : null;
            c.Check("the first vault on a PC: vault.rmv, on the list as that file and offered", firstFile == "vault.rmv"
                && Prefs.VaultList.SingleOrDefault()?.File == "vault.rmv" && Vaults.Current?.File == "vault.rmv",
                $"{firstFile} / {string.Join(", ", Prefs.VaultList.Select(v => v.File))}");
            await Harness.Call(first, "lock", new { });
        }
        foreach (string file in Directory.GetFiles(data))
            File.Delete(file);
        Prefs.VaultList.Clear();
        Prefs.LastVault = null;
        Vaults.Use(null);
        WindowsFormsSynchronizationContext.AutoInstall = false; // on this thread too (the awaits above moved to the pool)
        // A PC from before several vaults: vault.rmv, and its backup state in prefs.json's old keys.
        var old = Vault.Create(MainPassword);
        old.Data.Accounts.Add(new Account { Username = "MainOne", UserId = 51 });
        old.Save();
        old.Dispose();
        var backedUp = DateTime.UtcNow.AddHours(-3);
        Prefs.LastBackup = backedUp;
        Prefs.Save();
        Prefs.VaultList.Clear();
        Prefs.Load();
        Vaults.Load();
        var main = Prefs.VaultList.SingleOrDefault();
        c.Check("the vault from before joins the list, with its backup state", main is { File: "vault.rmv" }
            && Vaults.Current == main && main.LastBackup is { } at && Math.Abs((at - backedUp).TotalSeconds) < 2, main?.Name);

        using var form = new Form();
        using var app = new AppController(form, _ => { });
        await Harness.Call(app, "unlock", new { password = MainPassword });
        c.Check("it opens as before", Harness.Get<Vault>(app, "_vault").Data.Accounts.Any(a => a.Username == "MainOne"));

        await Harness.Call(app, "pickVault", new { });
        c.Check("Switch vault shows the picker over the open vault, which stays open", Harness.Get<Vault?>(app, "_vault") is not null && Harness.Get<bool>(app, "_vaultPick"));
        await Harness.Call(app, "closeVaultPicker", new { });
        c.Check("Back returns to it without the password", Harness.Get<Vault?>(app, "_vault") is not null && !Harness.Get<bool>(app, "_vaultPick"));
        await Harness.Call(app, "pickVault", new { });
        await Harness.Call(app, "selectVault", new { id = main!.Id });
        c.Check("picking the open vault in the picker returns to it as well", Harness.Get<Vault?>(app, "_vault") is not null && !Harness.Get<bool>(app, "_vaultPick"));

        await Harness.Call(app, "pickVault", new { });
        await Harness.Call(app, "addVault", new { });
        c.Check("adding one: the open vault stays open behind the form, the new one isn't on the list yet",
            Harness.Get<Vault?>(app, "_vault") is not null && Vault.ActiveFile == "vault.rmv" && Prefs.VaultList.Count == 1);
        await Harness.Call(app, "cancelAddVault", new { });
        c.Check("Cancel goes back to the picker, the open vault still open", Harness.Get<Vault?>(app, "_vault") is not null && Harness.Get<bool>(app, "_vaultPick"));
        await Harness.Call(app, "addVault", new { });
        bool shortRefused = await Harness.Try(app, "createVault", new { password = "short", name = "Farm" }) is UserError;
        c.Check("a password too short is refused before the open vault locks", shortRefused && Harness.Get<Vault?>(app, "_vault") is not null && Vault.ActiveFile == "vault.rmv");
        await Harness.Call(app, "createVault", new { password = FarmPassword, name = "  Farm  " });
        var farm = Prefs.VaultList.FirstOrDefault(v => v.Name == "Farm");
        c.Check("set up with its name and its own master password, it joins the list and opens empty",
            farm is not null && Vaults.Current == farm && Prefs.LastVault == farm.Id && Harness.Get<Vault>(app, "_vault").Data.Accounts.Count == 0
            && Path.GetFileName(Harness.Get<Vault>(app, "_vault").FilePath) == farm.File,
            string.Join(", ", Prefs.VaultList.Select(v => $"{v.Name} {v.File}")));
        Harness.Get<Vault>(app, "_vault").Data.Accounts.Add(new Account { Username = "FarmOne", UserId = 52 });
        Harness.Get<Vault>(app, "_vault").Save();
        c.Check("a new vault has no backup yet (each has its own)", Prefs.LastBackup is null);

        byte[] farmBytes = File.ReadAllBytes(Path.Combine(data, farm!.File));
        await Harness.Call(app, "selectVault", new { id = main.Id });
        c.Check("picking the other one locks this one and offers that one", Harness.Get<Vault?>(app, "_vault") is null && Vault.ActiveFile == "vault.rmv");
        bool refused = await Harness.Try(app, "unlock", new { password = FarmPassword }) is UserError;
        c.Check("each vault has its own master password", refused && Harness.Get<Vault?>(app, "_vault") is null);
        await Harness.Call(app, "unlock", new { password = MainPassword });
        var mainOpen = Harness.Get<Vault>(app, "_vault");
        c.Check("and its own accounts", mainOpen.Data.Accounts.Any(a => a.Username == "MainOne") && !mainOpen.Data.Accounts.Any(a => a.Username == "FarmOne"));
        mainOpen.Data.Accounts.Add(new Account { Username = "MainTwo", UserId = 53 });
        mainOpen.Save();
        c.Check("an open vault writes only its own file", File.ReadAllBytes(Path.Combine(data, farm.File)).SequenceEqual(farmBytes));

        // The other vault picked while this one's key is still being worked out (the unlock takes a moment): this unlock
        // is let go, and nothing of this vault ever lands in the other's file (2026-10-30 review: it saved over it).
        await Harness.Call(app, "lock", new { });
        var unlocking = Harness.Call(app, "unlock", new { password = MainPassword });
        await Harness.Call(app, "selectVault", new { id = farm.Id });
        await unlocking;
        c.Check("another vault picked during an unlock: that unlock is let go, the picked one waits for its own password",
            Harness.Get<Vault?>(app, "_vault") is null && Vault.ActiveFile == farm.File, $"{Harness.Get<Vault?>(app, "_vault")?.FilePath} {Vault.ActiveFile}");
        c.Check("…and the other vault's file is untouched", File.ReadAllBytes(Path.Combine(data, farm.File)).SequenceEqual(farmBytes));
        await Harness.Call(app, "selectVault", new { id = main.Id });
        await Harness.Call(app, "unlock", new { password = MainPassword });
        mainOpen = Harness.Get<Vault>(app, "_vault");

        // Back after a restart: the list, and the last vault used is the one offered.
        Prefs.Save();
        Prefs.VaultList.Clear();
        Prefs.LastVault = null;
        Prefs.Load();
        Vaults.Load();
        c.Check("after a restart the list is back and the last vault used is offered",
            Prefs.VaultList.Count == 2 && Vaults.Current?.Id == main.Id, string.Join(", ", Prefs.VaultList.Select(v => v.Name)));

        await Harness.Call(app, "setAskVault", new { enabled = true });
        using (var second = new AppController(new Form(), _ => { }))
            c.Check("asked for: with two vaults the picker shows at start", Harness.Get<bool>(second, "_vaultPick"));

        bool kept = await Harness.Try(app, "removeVault", new { id = main.Id }) is UserError && Prefs.VaultList.Any(v => v.Id == main.Id);
        c.Check("the vault open right now can't be taken off the list", kept);
        await Harness.Call(app, "removeVault", new { id = farm.Id });
        var setAside = Directory.GetFiles(Vault.OldVaultsDir, "vault-removed-*.rmv").Select(f => Path.GetFileName(f)!).ToList();
        c.Check("one taken off the list is set aside, never deleted", Prefs.VaultList.Count == 1 && !File.Exists(Path.Combine(data, farm.File))
            && setAside.Any(n => !n.Contains("-spare")), string.Join(", ", setAside));
        using (var third = new AppController(new Form(), _ => { }))
            c.Check("with one vault left the picker doesn't show at start", !Harness.Get<bool>(third, "_vaultPick"));

        c.Check("a vault's picture is served by its own kind of name only",
            Themes.IsPictureName("vault-picture-a1b2c3d4-638700000000000000.png") && !Themes.IsPictureName("vault-picture-a1b2c3d4-1.png/../vault.rmv")
            && !Themes.IsPictureName("vault-picture-xyz-1.png"));

        // Back pressed while a new vault is being set up (its key takes a moment): the vault written is the one that joins the
        // list, not another entry pointing at a file never written (review, 2026-10-30: the new vault was left an orphan).
        // The forms made above (on a pool thread, where WinForms installs its context) would have the set-up's last part wait
        // for a message loop this test doesn't run.
        SynchronizationContext.SetSynchronizationContext(null);
        await Harness.Call(app, "addVault", new { });
        var creating =Harness.Call(app, "createVault", new { password = FarmPassword, name = "Third" });
        await Harness.Call(app, "cancelAddVault", new { });
        await creating;
        string? openFile = Harness.Get<Vault?>(app, "_vault")?.FilePath is { } p ? Path.GetFileName(p) : null;
        var thirdVault = Prefs.VaultList.FirstOrDefault(v => v.Name == "Third");
        c.Check("Back during a new vault's set-up: the vault written joins the list with its own file, and is the one offered",
            thirdVault is not null && thirdVault.File == openFile && File.Exists(Path.Combine(data, thirdVault.File)) && Vaults.Current == thirdVault,
            $"{openFile} / {string.Join(", ", Prefs.VaultList.Select(v => $"{v.Name} {v.File}"))}");
        // A set-up that started and failed, then Cancel: the lock screen offers the vault from before, not the unwritten file.
        await Harness.Call(app, "addVault", new { });
        Harness.Invoke(app, "SwitchToNewVault");
        await Harness.Call(app, "cancelAddVault", new { });
        c.Check("…a set-up that failed, then Cancel: the vault from before is offered again", Vaults.Current == thirdVault && Vault.ActiveFile == thirdVault!.File,
            Vault.ActiveFile);

        // A backup cut short (review, 2026-10-30): one only the size of a header isn't taken as a vault to restore; one that
        // got through and won't open says where the vault it replaced is kept.
        string thirdPath = Path.Combine(data, thirdVault.File), cut = Path.Combine(dir, "cut-short.rmv");
        File.WriteAllBytes(cut, File.ReadAllBytes(thirdPath)[..100]);
        c.Check("a backup cut short to 100 bytes isn't taken as a vault; a whole one is", !Vault.LooksLikeVault(cut) && Vault.LooksLikeVault(thirdPath));
        Directory.CreateDirectory(Vault.OldVaultsDir);
        string replaced = Path.Combine(Vault.OldVaultsDir, "vault-before-restore-20261030-120000.rmv");
        File.Copy(thirdPath, replaced, overwrite: true);
        Harness.Set(app, "_restoredOver", (thirdVault.File, replaced));
        var failedUnlock = await Harness.Try(app, "unlock", new { password = "not this vault's password" });
        c.Check("…the first unlock after a restore fails: says where the vault it replaced is kept",
            failedUnlock is UserError e && e.Message.Contains(replaced, StringComparison.Ordinal), (failedUnlock as Exception)?.Message);
        await Harness.Call(app, "unlock", new { password = FarmPassword });
        c.Check("…once it opens, not said again", Harness.Get<Vault?>(app, "_vault") is not null && Harness.Get<object?>(app, "_restoredOver") is null);
    }
}
