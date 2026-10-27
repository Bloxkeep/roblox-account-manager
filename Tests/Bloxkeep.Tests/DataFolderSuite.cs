using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// The data folder's layout (Vault.Tidy): a folder from before 1.0.11, with everything at the top, has its old vault
/// copies, pictures and caches moved into old-vaults, pictures and cache at start; the vaults, their spare copies,
/// settings, the log and anything not Bloxkeep's stay where they are, and the pictures still show.
/// </summary>
static class DataFolderSuite
{
    public static Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        string data = Vault.DataDir;
        void Put(string name, string text = "x")
        {
            Directory.CreateDirectory(Path.GetDirectoryName(Path.Combine(data, name))!);
            File.WriteAllText(Path.Combine(data, name), text);
        }
        bool At(string name) => File.Exists(Path.Combine(data, name));

        string[] stay = ["vault.rmv", "vault.rmv.bak", "vault.rmv.tmp", "vault-a1b2c3d4.rmv", "prefs.json", "error.log",
            "roblox-settings.json", "notes.txt", "vault-picture-a1b2c3d4-1.png.txt"];
        foreach (string name in stay)
            Put(name);
        Put("vault-before-restore-20261023-103313.rmv", "before");
        Put("vault-removed-20261020-090000.rmv", "removed");
        Put("vault-removed-20261020-090000-spare.rmv");
        Put("theme-background-638700000000000000.png");
        Put("vault-picture-a1b2c3d4-639280667182096585.png", "picture");
        Put(@"webview\EBWebView\Local State", "cache");
        Put(@"captcha\Default\Cookies");
        Directory.CreateDirectory(Path.Combine(data, "roblox-client-paths"));

        Vault.Tidy();

        c.Check("old vault copies move into old-vaults, unchanged",
            File.ReadAllText(Path.Combine(data, @"old-vaults\vault-before-restore-20261023-103313.rmv")) == "before"
            && File.ReadAllText(Path.Combine(data, @"old-vaults\vault-removed-20261020-090000.rmv")) == "removed"
            && At(@"old-vaults\vault-removed-20261020-090000-spare.rmv") && !At("vault-before-restore-20261023-103313.rmv"));
        c.Check("pictures move into pictures",
            At(@"pictures\theme-background-638700000000000000.png") && At(@"pictures\vault-picture-a1b2c3d4-639280667182096585.png")
            && !At("theme-background-638700000000000000.png"));
        c.Check("caches move into cache with what's in them",
            File.ReadAllText(Path.Combine(data, @"cache\webview\EBWebView\Local State")) == "cache" && At(@"cache\captcha\Default\Cookies")
            && Directory.Exists(Path.Combine(data, @"cache\roblox-client-paths")) && !Directory.Exists(Path.Combine(data, "webview")));
        var moved = stay.Where(n => !At(n)).ToList();
        c.Check("the vaults with their .bak and .tmp, settings, the log and others' files stay at the top", moved.Count == 0, string.Join(", ", moved));
        var top = Directory.EnumerateDirectories(data).Select(Path.GetFileName).Order().ToList();
        c.Check("the top holds only the three folders", top.SequenceEqual(["cache", "old-vaults", "pictures"]), string.Join(", ", top));

        // A cache made fresh since (an older build run again): the old one at the top goes, the new one stays.
        Put(@"webview\old", "old");
        Vault.Tidy();
        c.Check("an old cache next to a new one is dropped", !Directory.Exists(Path.Combine(data, "webview"))
            && At(@"cache\webview\EBWebView\Local State") && !At(@"cache\webview\old"));
        // An old copy whose name is already taken in old-vaults is left alone, never overwritten.
        Put("vault-before-restore-20261023-103313.rmv", "second");
        Vault.Tidy();
        c.Check("a copy whose name is taken stays at the top, neither one lost",
            File.ReadAllText(Path.Combine(data, "vault-before-restore-20261023-103313.rmv")) == "second"
            && File.ReadAllText(Path.Combine(data, @"old-vaults\vault-before-restore-20261023-103313.rmv")) == "before");
        File.Delete(Path.Combine(data, "vault-before-restore-20261023-103313.rmv"));

        // Asked for at uninstall: Bloxkeep's files in the subfolders go too; others' files keep the folder.
        Vault.DeleteAllData();
        var left = Directory.EnumerateFileSystemEntries(data, "*", SearchOption.AllDirectories).Select(p => Path.GetRelativePath(data, p)).Order().ToList();
        c.Check("delete everything takes the subfolders' files and the emptied folders, leaving others' files",
            left.SequenceEqual(["notes.txt", "vault-picture-a1b2c3d4-1.png.txt"]), string.Join(", ", left));
        return Task.CompletedTask;
    }
}
