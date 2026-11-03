using System.Security.Cryptography;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// A save cut short (a power cut, an antivirus holding the file): vault.rmv, vault.rmv.tmp and vault.rmv.bak each absent,
/// whole, empty, cut in half or random bytes, all 125 states. For each, whether the app offers "unlock" (Vault.Exists), what
/// opens (Vault.RestoreIfMissing and Vault.Open) and that no file's content is gone afterwards; then eight threads writing
/// one small file at once (DurableFile, the settings' way of writing).
/// </summary>
static class VaultCrashSuite
{
    enum S { None, Valid, Empty, Half, Random }

    const string Password = "crash-matrix-pass-1";

    public static Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        string data = Vault.DataDir;
        string original = Vault.ActiveFile;

        // Three saves of one vault: 1, 2 and 3 accounts (with enough in each for half of the file to still look like a vault).
        string maker = Path.Combine(data, "maker.rmv");
        byte[][] saves = new byte[4][];
        using (var vault = Vault.Create(Password, maker))
        {
            for (int n = 1; n <= 3; n++)
            {
                vault.Data.Accounts.Add(new Account { Username = "Crash" + n, Alias = new string((char)('a' + n), 700), UserId = 9000 + n });
                vault.Save();
                saves[n] = File.ReadAllBytes(maker);
            }
        }
        c.Check("the three saves are each big enough that half of one still looks like a vault", saves.Skip(1).All(b => b.Length / 2 >= 300), string.Join(", ", saves.Skip(1).Select(b => b.Length)));

        // The state that a swap cut short leaves: vault.rmv is the second save, .tmp the third written in full, .bak the first.
        byte[] Content(string slot, S state)
        {
            byte[] whole = slot switch { "main" => saves[2], "tmp" => saves[3], _ => saves[1] };
            return state switch
            {
                S.Valid => whole,
                S.Half => whole[..(whole.Length / 2)],
                S.Empty => [],
                _ => RandomBytes(whole.Length / 2 + (slot.Length * 17)),
            };
        }
        static byte[] RandomBytes(int length)
        {
            var bytes = new byte[length];
            new Random(length).NextBytes(bytes);
            bytes[0] = (byte)'X'; // never the vault's own first bytes
            return bytes;
        }

        var all = Enum.GetValues<S>();
        var wrong = all.ToDictionary(s => s, _ => new List<string>());
        var lost = new List<string>();
        int number = 0;
        foreach (S m in all)
        {
            foreach (S t in all)
            {
                foreach (S b in all)
                {
                    string name = $"vault-{++number:x8}.rmv";
                    string path = Path.Combine(data, name);
                    var written = new Dictionary<string, byte[]>();
                    foreach (var (slot, state, suffix) in new[] { ("main", m, ""), ("tmp", t, ".tmp"), ("bak", b, ".bak") })
                    {
                        if (state == S.None)
                            continue;
                        byte[] bytes = Content(slot, state);
                        File.WriteAllBytes(path + suffix, bytes);
                        written[slot] = bytes;
                    }
                    var (expectExists, expectOutcome) = Expect(m, t, b);
                    Vault.ActiveFile = name; // each state is its own file: Exists puts a missing vault back once per file
                    string outcome = "";
                    bool exists = Vault.Exists;
                    if (exists)
                        outcome = OpenOutcome(path);
                    // Opened from the spare: the damaged vault is set aside and the spare is the vault again, which opens the same.
                    if (exists && outcome == "opened 1 account" && m != S.Valid && expectOutcome == outcome)
                    {
                        string again = OpenOutcome(path);
                        if (again != outcome)
                            outcome = "first " + outcome + ", then " + again;
                    }
                    if (exists != expectExists || (expectExists && outcome != expectOutcome))
                        wrong[m].Add($"[tmp {t}, bak {b}] exists {exists} (expected {expectExists}), {outcome} (expected {expectOutcome})");

                    // Whatever was there is still somewhere in the folder, under a name or other.
                    var after = Directory.GetFiles(data, name + "*").Select(f => Convert.ToHexString(SHA256.HashData(File.ReadAllBytes(f)))).ToHashSet();
                    foreach (var (slot, bytes) in written)
                    {
                        if (bytes.Length > 0 && !after.Contains(Convert.ToHexString(SHA256.HashData(bytes))))
                            lost.Add($"[main {m}, tmp {t}, bak {b}] the {slot} file's content");
                    }
                    foreach (string f in Directory.GetFiles(data, name + "*"))
                        File.Delete(f);
                }
            }
        }
        Vault.ActiveFile = original;
        foreach (S m in all)
            c.Check($"vault.rmv {m}: with each state of .tmp and .bak, Exists and the vault that opens are as expected (25 states)", wrong[m].Count == 0, string.Join(" ; ", wrong[m].Take(6)));
        c.Check("in none of the 125 states is a file's content gone afterwards (damaged and spare copies are kept)", lost.Count == 0, string.Join(" ; ", lost.Take(6)));

        // Eight threads, 200 writes each, one file: the settings file's way of writing (a temp file beside it, then a move).
        string shared = Path.Combine(data, "shared.json");
        var values = Enumerable.Range(0, 8).Select(i => new string((char)('A' + i), 400)).ToArray();
        var errors = new List<string>();
        var threads = values.Select(value => new Thread(() =>
        {
            for (int i = 0; i < 200; i++)
            {
                try
                {
                    DurableFile.WriteText(shared, value);
                }
                catch (Exception ex)
                {
                    lock (errors)
                        errors.Add(ex.GetType().Name + ": " + ex.Message);
                }
            }
        })).ToArray();
        foreach (var thread in threads)
            thread.Start();
        foreach (var thread in threads)
            thread.Join();
        string[] distinct;
        lock (errors)
            distinct = errors.GroupBy(e => e).Select(g => $"{g.Count()}x {g.Key}").Take(3).ToArray();
        c.Check("8 threads x 200 DurableFile writes to one file: none throws", errors.Count == 0, $"{errors.Count} failed, e.g. {string.Join(" ; ", distinct)}");
        c.Check("…no temp file is left beside it", !File.Exists(shared + ".tmp"));
        c.Check("…and what is in the file is one of the values written, whole", File.Exists(shared) && values.Contains(File.ReadAllText(shared)));
        return Task.CompletedTask;
    }

    /// <summary>Whether the app offers to unlock, and what Open then gives, for the states of the three files (from Vault.cs's contract).</summary>
    static (bool Exists, string Outcome) Expect(S main, S tmp, S bak)
    {
        bool looksLikeTmp = tmp is S.Valid or S.Half, looksLikeBak = bak is S.Valid or S.Half; // Half: the header is there and it is over 256 bytes
        int mainAccounts = 2;
        if (main == S.None)
        {
            // Put back: the .tmp when it looks like a vault (the .bak stays as its fallback), else the previous save.
            if (looksLikeTmp)
            {
                main = tmp;
                mainAccounts = 3;
            }
            else if (looksLikeBak)
            {
                main = bak;
                mainAccounts = 1;
                bak = S.None; // it became the vault
            }
            else
                return (false, "");
        }
        string Opened(int accounts) => $"opened {accounts} account{(accounts == 1 ? "" : "s")}";
        return main switch
        {
            S.Valid => (true, Opened(mainAccounts)),
            // The header is there but the tag doesn't match: the previous save, if it is whole, else it looks like a wrong password.
            S.Half => (true, bak == S.Valid ? Opened(1) : "wrong password"),
            // No header: the previous save if whole; a half one has the header but won't open; nothing to go by otherwise.
            _ => (true, bak == S.Valid ? Opened(1) : bak == S.Half ? "wrong password" : "not a vault file"),
        };
    }

    static string OpenOutcome(string path)
    {
        try
        {
            using var vault = Vault.Open(Password, path);
            int n = vault.Data.Accounts.Count;
            return $"opened {n} account{(n == 1 ? "" : "s")}";
        }
        catch (InvalidPasswordException)
        {
            return "wrong password";
        }
        catch (InvalidDataException)
        {
            return "not a vault file";
        }
        catch (Exception ex)
        {
            return ex.GetType().Name + ": " + ex.Message;
        }
    }
}
