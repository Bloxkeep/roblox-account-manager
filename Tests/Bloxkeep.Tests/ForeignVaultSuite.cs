using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// A backup made somewhere else (AppController.Backup.cs, NoteRestored and DropForeignLinks): restored and unlocked, it
/// keeps its accounts but not the launcher it names nor its Discord links, which whoever made it chose; a backup of this
/// PC's own vault keeps everything. A launcher on the network is never looked at or started (RobloxLauncher.IsLocalProgram).
/// </summary>
static class ForeignVaultSuite
{
    const string OwnPassword = "own vault password", TheirPassword = "their vault password";
    const string NetworkLauncher = @"\\attacker@SSL\share\RobloxPlayerLauncher.exe";
    static readonly string TheirHook = "https://discord.com/api/webhooks/123456789012345678/" + new string('T', 68);
    static readonly string OwnHook = "https://discord.com/api/webhooks/876543210987654321/" + new string('O', 68);

    public static async Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(Path.Combine(dir, "data"));
        string theirs = Path.Combine(dir, "their-backup.rmv"), ours = Path.Combine(dir, "our-backup.rmv");
        string launcher = Path.Combine(dir, "MyLauncher.exe");
        File.WriteAllText(launcher, "");

        // Someone else's vault: an account, a launcher on their share, their Discord channel for alerts and a preset.
        var made = Vault.Create(TheirPassword);
        made.Data.Accounts.Add(new Account { Username = "TheirAlt", UserId = 77 });
        made.Data.Settings.LaunchVia = "custom";
        made.Data.Settings.CustomLauncher = NetworkLauncher;
        made.Data.Discord.Enabled = true;
        made.Data.Discord.WebhookUrl = TheirHook;
        made.Data.Settings.Presets.Add(new LaunchPreset { Name = "Farm", DiscordWebhook = TheirHook });
        made.Save();
        string theirId = made.Id;
        made.Dispose();
        File.Copy(Vault.VaultPath, theirs, overwrite: true);
        foreach (string file in Directory.GetFiles(Vault.DataDir, "vault*"))
            File.Delete(file);

        // This PC's own vault, with its own launcher and Discord channel.
        var own = Vault.Create(OwnPassword);
        own.Data.Accounts.Add(new Account { Username = "MyMain", UserId = 11 });
        own.Data.Settings.LaunchVia = "custom";
        own.Data.Settings.CustomLauncher = launcher;
        own.Data.Discord.Enabled = true;
        own.Data.Discord.WebhookUrl = OwnHook;
        own.Save();
        string ownId = own.Id;
        own.Dispose();

        using var form = new Form();
        using var app = new AppController(form, _ => { });
        Vault V() => Harness.Get<Vault>(app, "_vault");
        await Harness.Call(app, "unlock", new { password = OwnPassword });
        c.Check("a vault unlocked here is one of this PC's", Prefs.KnownVaultIds.Contains(ownId) && !Prefs.KnownVaultIds.Contains(theirId));
        File.Copy(Vault.VaultPath, ours, overwrite: true);

        // What Restore backup does once the file is picked (AppController.RestoreBackup, the dialog left out).
        async Task Restore(string file)
        {
            await Harness.Call(app, "lock", new { });
            // The copy kept aside is named to the second; two restores here can fall in the same one.
            foreach (string kept in (Directory.Exists(Vault.OldVaultsDir) ? Directory.GetFiles(Vault.OldVaultsDir, "vault-before-restore-*") : []))
                File.Delete(kept);
            typeof(AppController).Assembly.GetType("RobloxManager.Backup")!.GetMethod("Restore", Harness.F)!.Invoke(null, [file, true]);
            typeof(AppController).GetMethod("NoteRestored", Harness.F)!.Invoke(null, [file]);
        }

        // This PC's own backup: nothing dropped.
        await Restore(ours);
        await Harness.Call(app, "unlock", new { password = OwnPassword });
        var s = V().Data.Settings;
        c.Check("this PC's own backup, restored: its launcher and Discord link kept", Prefs.ForeignRestore is null
            && s.LaunchVia == "custom" && s.CustomLauncher == launcher && V().Data.Discord.Enabled && V().Data.Discord.WebhookUrl == OwnHook,
            $"{s.LaunchVia} {s.CustomLauncher} {V().Data.Discord.WebhookUrl.Length}");

        // Someone else's backup: noted at the restore, then its launcher and Discord links dropped at the first unlock.
        await Restore(theirs);
        c.Check("a backup made elsewhere is noted at the restore", Prefs.ForeignRestore == theirId, Prefs.ForeignRestore);
        await Harness.Call(app, "unlock", new { password = TheirPassword });
        s = V().Data.Settings;
        var discord = V().Data.Discord;
        c.Check("…unlocked: its accounts are there", V().Data.Accounts.Any(a => a.Username == "TheirAlt"));
        c.Check("…the launcher it named is gone, Roblox opens the usual way", s.CustomLauncher == "" && s.LaunchVia == "auto", $"{s.LaunchVia} {s.CustomLauncher}");
        c.Check("…its Discord channel is gone and Discord is off", !discord.Enabled && discord.WebhookUrl == "", discord.WebhookUrl);
        c.Check("…its presets' Discord links are gone", s.Presets.All(p => p.DiscordWebhook == ""), s.Presets.Count);
        c.Check("…and it is one of this PC's from now on", Prefs.ForeignRestore is null && Prefs.KnownVaultIds.Contains(theirId));

        // Launchers on the network: never this PC's program, never looked at.
        foreach (string path in new[] { NetworkLauncher, @"\\server\share\Bloxstrap.exe", @"\\?\UNC\server\share\a.exe", @"\\.\pipe\x", "RobloxPlayerLauncher.exe", "" })
            c.Check($"a launcher at \"{path}\" isn't a program on this PC", !RobloxLauncher.IsLocalProgram(path) && !RobloxLauncher.CustomLauncherReady(path));
        c.Check("a launcher on this PC's disk is", RobloxLauncher.IsLocalProgram(launcher) && RobloxLauncher.CustomLauncherReady(launcher));
        s.CustomLauncher = NetworkLauncher;
        await Harness.Try(app, "setSettings", new { launchVia = "custom" });
        c.Check("Settings won't switch to a launcher on the network", s.LaunchVia == "auto", s.LaunchVia);
        bool refused = false;
        try
        {
            RobloxLauncher.Launch("ticket", null, "custom", NetworkLauncher);
        }
        catch (FileNotFoundException)
        {
            refused = true;
        }
        c.Check("Play with a launcher on the network: refused as gone, nothing started", refused);
    }
}
