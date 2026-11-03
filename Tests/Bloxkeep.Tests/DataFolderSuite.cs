using System.Runtime.CompilerServices;
using System.Text.RegularExpressions;
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

        // A prefs.json that isn't JSON any more (this one is "x"): kept aside before the next save writes the defaults over
        // it, with the vault list in it (review, 2026-10-30).
        Prefs.Load();
        var damaged = Directory.GetFiles(data, "prefs.json.damaged-*");
        c.Check("a prefs.json that can't be read is kept aside as it was before the defaults are saved",
            damaged.Length == 1 && File.ReadAllText(damaged[0]) == "x", damaged.Length);
        // Held by another program as Bloxkeep starts: not damaged, so this run's defaults aren't saved over it either.
        string prefsFile = Path.Combine(data, "prefs.json");
        using (new FileStream(prefsFile, FileMode.Open, FileAccess.ReadWrite, FileShare.None))
            Prefs.Load();
        Prefs.Save();
        c.Check("a prefs.json held by another program at the start isn't overwritten with defaults", File.ReadAllText(prefsFile) == "x", File.ReadAllText(prefsFile));
        typeof(Prefs).GetField("_unreadable", System.Reflection.BindingFlags.NonPublic | System.Reflection.BindingFlags.Static)!.SetValue(null, false);

        // Every uninstall drops the browser caches; in a folder BLOXKEEP_DATA points at (as here), a "webview" folder at the
        // top may be the person's own and stays (review, 2026-10-30: it was deleted, even with "keep my data").
        Put(@"webview\mine.txt", "mine");
        Put(@"cache\captcha\Default\Cookies");
        Vault.DeleteCaches();
        c.Check("uninstall's cache clean-up in a folder BLOXKEEP_DATA points at: Bloxkeep's cache goes, a webview folder at the top stays",
            At(@"webview\mine.txt") && !Directory.Exists(Path.Combine(data, @"cache\captcha")));
        Directory.Delete(Path.Combine(data, "webview"), true);

        // Asked for at uninstall: Bloxkeep's files in the subfolders go too; others' files keep the folder.
        Vault.DeleteAllData();
        var left = Directory.EnumerateFileSystemEntries(data, "*", SearchOption.AllDirectories).Select(p => Path.GetRelativePath(data, p)).Order().ToList();
        c.Check("delete everything takes the subfolders' files and the emptied folders, leaving others' files",
            left.SequenceEqual(["notes.txt", "vault-picture-a1b2c3d4-1.png.txt"]), string.Join(", ", left));

        EveryWriterIsCovered(c, data);
        LogRotates(c, data);
        PrefsSaveFailure(c, data);
        return Task.CompletedTask;
    }

    /// <summary>The names every writer gives its file in the data folder, read out of the program's own source.</summary>
    static List<string> WrittenNames([CallerFilePath] string here = "")
    {
        string source = Path.GetFullPath(Path.Combine(Path.GetDirectoryName(here)!, "..", "..", "RobloxManager"));
        var names = new SortedSet<string>(StringComparer.OrdinalIgnoreCase);
        foreach (string file in Directory.EnumerateFiles(source, "*.cs", SearchOption.AllDirectories).Where(f => !f.Contains($"{Path.DirectorySeparatorChar}obj{Path.DirectorySeparatorChar}")))
        {
            foreach (Match m in Regex.Matches(File.ReadAllText(file), @"Path\.Combine\(\s*Vault\.DataDir\s*,\s*""([^""{}\\]+)""\s*\)"))
                names.Add(m.Groups[1].Value);
        }
        return names.ToList();
    }

    /// <summary>"Delete my accounts and settings" leaves no file of Bloxkeep's, whichever part of the program wrote it (the list
    /// was kept by hand and fell behind: game-hours.json, roblox-flags.json, hidden-windows.txt and window-icons.txt stayed, and
    /// with them the folder), and still nothing that isn't Bloxkeep's.</summary>
    static void EveryWriterIsCovered(Checks c, string data)
    {
        if (Directory.Exists(data))
            Directory.Delete(data, true);
        Directory.CreateDirectory(data);
        var written = WrittenNames();
        c.Check("the source names the files Bloxkeep writes into the data folder (the scan isn't empty)",
            written.Count >= 8 && written.Contains("game-hours.json") && written.Contains("roblox-flags.json") && written.Contains("hidden-windows.txt")
            && written.Contains("window-icons.txt"), string.Join(", ", written));
        var made = new List<string>();
        foreach (string name in written.Concat(["error.log.1", "prefs.json.damaged-20261101-120000", "vault-a1b2c3d4.rmv", "vault-a1b2c3d4.rmv.bak"]))
        {
            File.WriteAllText(Path.Combine(data, name), "x");
            made.Add(name);
            // The temp file a write goes through, and one a crash left behind.
            if (written.Contains(name))
            {
                File.WriteAllText(Path.Combine(data, name + ".tmp"), "x");
                made.Add(name + ".tmp");
            }
        }
        // Not Bloxkeep's, though they look alike (BLOXKEEP_DATA may be a folder of the person's own).
        string[] others = ["game-hours.json5", "hidden-windows.txt.bak", "my-error.log", "error.log.2", "roblox-flags.json.old", "window-icons.txt.tmp.old"];
        foreach (string name in others)
            File.WriteAllText(Path.Combine(data, name), "mine");
        Vault.DeleteAllData();
        var remains = Directory.EnumerateFileSystemEntries(data).Select(Path.GetFileName).Order().ToList();
        c.Check("delete everything takes a file of every writer (and its .tmp, error.log.1, the damaged copies) and no other",
            remains.SequenceEqual(others.Order()), string.Join(", ", remains));
        foreach (string name in others)
            File.Delete(Path.Combine(data, name));
        foreach (string name in made.Where(n => File.Exists(Path.Combine(data, n))))
            File.Delete(Path.Combine(data, name));
        foreach (string name in made.Take(5))
            File.WriteAllText(Path.Combine(data, name), "x");
        Vault.DeleteAllData();
        c.Check("…and with only Bloxkeep's files in it, the folder goes too", !Directory.Exists(data));
        Directory.CreateDirectory(data);
    }

    /// <summary>error.log at its limit is kept as error.log.1 (the history a bug report needs), not deleted.</summary>
    static void LogRotates(Checks c, string data)
    {
        string log = Path.Combine(data, "error.log");
        for (int i = 0; i < 300; i++)
            Log.Note($"line{i} " + new string('a', 1000));
        bool kept = File.Exists(log + ".1");
        c.Check("an error.log over its limit is kept as error.log.1", kept && File.ReadAllText(log + ".1").Contains("line0 "), kept);
        c.Check("…and the log goes on in a new error.log with the newest lines",
            File.Exists(log) && File.ReadAllText(log).Contains("line299 ") && !File.ReadAllText(log).Contains("line0 ") && new FileInfo(log).Length < 256 * 1024);
        for (int i = 300; i < 700; i++)
            Log.Note($"line{i} " + new string('b', 1000));
        c.Check("a second time the spare is replaced, not added to", File.Exists(log + ".1") && new FileInfo(log + ".1").Length < 300 * 1024
            && !File.ReadAllText(log + ".1").Contains("line0 "));
        File.Delete(log);
        File.Delete(log + ".1"); // (a missing file is no error to Delete)
    }

    /// <summary>prefs.json held by another program (an antivirus, a sync client) for a moment: Save tries again, says so once,
    /// and tells the caller; it keeps trying on the next Save.</summary>
    static void PrefsSaveFailure(Checks c, string data)
    {
        string file = Path.Combine(data, "prefs.json"), log = Path.Combine(data, "error.log");
        int Lines() => File.Exists(log) ? File.ReadAllLines(log).Count(l => l.Contains("prefs.json couldn't be saved")) : 0;
        Prefs.TrayHintShown = false;
        c.Check("a normal Save says it worked", Prefs.Save() && !Prefs.SaveFailed);
        Prefs.TrayHintShown = true;
        bool ok;
        using (new FileStream(file, FileMode.Open, FileAccess.ReadWrite, FileShare.None))
        {
            ok = Prefs.Save();
            c.Check("a prefs.json held for good: Save says it failed", !ok && Prefs.SaveFailed);
            c.Check("…and writes it in error.log once", Lines() == 1, Lines());
            Prefs.Save();
            c.Check("…not again for the next failed Save", Lines() == 1, Lines());
        }
        c.Check("let go: the next Save works, with what was changed meanwhile", Prefs.Save() && !Prefs.SaveFailed
            && File.ReadAllText(file).Contains("\"trayHintShown\":true"));
        // Let go within a moment: the retries get through, and nothing is reported.
        int before = Lines();
        Prefs.TrayHintShown = false;
        var hold = new FileStream(file, FileMode.Open, FileAccess.ReadWrite, FileShare.None);
        var release = Task.Run(async () =>
        {
            await Task.Delay(120);
            hold.Dispose();
        });
        ok = Prefs.Save();
        release.Wait();
        c.Check("held for a moment only: Save tries again and works", ok && !Prefs.SaveFailed && File.ReadAllText(file).Contains("\"trayHintShown\":false"));
        c.Check("…and reports nothing", Lines() == before, Lines());
    }
}
