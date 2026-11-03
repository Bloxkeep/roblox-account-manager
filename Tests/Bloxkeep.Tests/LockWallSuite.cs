using System.Collections;
using System.Runtime.CompilerServices;
using System.Security.Cryptography;
using System.Text.Json;
using System.Text.RegularExpressions;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// The wall behind the lock screen (AppController.AutoLock.cs, ScreenLockCommands): every command the page can send, but the
/// few the lock screen itself uses, is refused with "Bloxkeep is locked." and does nothing: the page's list is read from the
/// source (so a command added later is under the check at once), the vault, the settings file, the posts and the running and
/// pending launches are the same afterwards, the tray's entry points stay no-ops. A command that is no command is refused
/// the same way (the wall doesn't tell which names exist).
/// </summary>
static class LockWallSuite
{
    const string Locked = "Bloxkeep is locked.";

    static string ThisFile([CallerFilePath] string path = "") => path;

    public static async Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);

        // The command list, as the source has it: the switch in DispatchCommandAsync.
        string source = Path.Combine(Path.GetDirectoryName(ThisFile())!, "..", "..", "RobloxManager", "AppController.cs");
        string[] commands = [];
        if (File.Exists(source))
        {
            string text = File.ReadAllText(source);
            int start = text.IndexOf("Task<object?> DispatchCommandAsync(", StringComparison.Ordinal);
            int end = start < 0 ? -1 : text.IndexOf("\n    };", start, StringComparison.Ordinal);
            if (start >= 0 && end > start)
                commands = Regex.Matches(text[start..end], "^        \"(\\w+)\" =>", RegexOptions.Multiline).Select(m => m.Groups[1].Value).Distinct().ToArray();
        }
        c.Check("the command list was read from AppController.cs (more than 100 names)", commands.Length > 100, $"{commands.Length} names from {Path.GetFullPath(source)}");
        var allowed = (HashSet<string>)typeof(AppController).GetField("ScreenLockCommands", Harness.F)!.GetValue(null)!;
        c.Check("every name the lock screen may use is a command (no typo in the allow-list)", allowed.All(commands.Contains), string.Join(", ", allowed.Where(a => !commands.Contains(a))));
        c.Check("the allow-list is the few the lock screen needs (at most 12), none of them reading or changing accounts",
            allowed.Count is > 0 and <= 12 && !allowed.Any(a => a.Contains("ccount") || a.StartsWith("reveal") || a.StartsWith("launch") || a.StartsWith("export")), string.Join(", ", allowed.OrderBy(a => a)));
        var walled = commands.Where(n => !allowed.Contains(n)).ToArray();

        // A vault with an account and a preset to find on the page, a settings file written.
        var seed = Vault.Create(Harness.Password);
        var secret = new Account { Username = "WallUser77", Alias = "WallAlias77", UserId = 7777, Cookie = "_TEST_WALL_" + Guid.NewGuid().ToString("N") };
        seed.Data.Accounts.Add(secret);
        var preset = new LaunchPreset { Name = "WallPreset", AccountIds = [secret.Id] };
        seed.Data.Settings.Presets.Add(preset);
        seed.Save();
        seed.Dispose();
        Prefs.Save();

        var posts = new List<string>();
        int notices = 0;
        using var form = new Form();
        using var app = new AppController(form, json => { lock (posts) posts.Add(json); }) { ShowNotification = (_, _) => { notices++; return true; } };
        await Harness.Unlock(app);
        app.PageAttached = true;
        var vault = Harness.Get<Vault>(app, "_vault");

        // The harness's own lookups, on the object these checks use.
        c.Check("Harness: a dotted path goes through a field (_discordPresence._disposed)", !Harness.Get<bool>(app, "_discordPresence._disposed"));
        c.Check("Harness: a name only one of the parts has is found there (_nextTry, in the Discord presence)", Harness.Get<DateTime>(app, "_nextTry") == Harness.Get<DateTime>(app, "_discordPresence._nextTry"));
        c.Check("Harness: a name the owner has itself is its own (_screenLocked)", !Harness.Get<bool>(app, "_screenLocked"));
        string? missing = null;
        try { Harness.Get<object>(app, "_noSuchField"); } catch (InvalidOperationException e) { missing = e.Message; }
        c.Check("Harness: a name nothing has is said clearly", missing is not null && missing.Contains("_noSuchField") && missing.Contains("AppController"), missing);

        Harness.Invoke(app, "ScreenLock");
        c.Check("the lock screen is up, the vault open behind it", Harness.Get<bool>(app, "_screenLocked") && ReferenceEquals(Harness.Get<Vault?>(app, "_vault"), vault));

        string Hash(string path) => File.Exists(path) ? Convert.ToHexString(SHA256.HashData(File.ReadAllBytes(path))) : "absent";
        string Snapshot() => string.Join(" | ", Hash(Vault.VaultPath), Hash(Path.Combine(Vault.DataDir, "prefs.json")),
            Convert.ToHexString(SHA256.HashData(JsonSerializer.SerializeToUtf8Bytes(vault.Data))),
            string.Join(",", Directory.GetFiles(Vault.DataDir).Select(Path.GetFileName).OrderBy(n => n, StringComparer.Ordinal)),
            Harness.Get<ICollection>(app, "_running").Count, Harness.Get<ICollection>(app, "_pending").Count);
        int Posts() { lock (posts) return posts.Count; }
        // The app's own timers may push the lock screen's state at any moment (that is allowed: it names nothing); a toast,
        // another event or a state with accounts in it is not.
        static bool LockScreenState(string post) => post.StartsWith("{\"type\":\"event\",\"name\":\"state\"", StringComparison.Ordinal)
            && post.Contains("\"locked\":true") && post.Contains("\"accounts\":[]");
        string[] Since(int count) { lock (posts) return posts.Skip(count).ToArray(); }

        string before = Snapshot();
        int postsBefore = Posts();

        // Each by itself (an empty answer, whatever the command would have needed).
        var notRefused = new List<string>();
        foreach (string name in walled)
        {
            Exception? outcome = null;
            try
            {
                await Harness.Call(app, name, new { });
            }
            catch (Exception ex)
            {
                outcome = ex;
            }
            if (outcome is not UserError { Message: Locked })
                notRefused.Add($"{name}: {outcome?.GetType().Name ?? "answered"} {outcome?.Message}");
        }
        c.Check($"behind the lock screen, each of the {walled.Length} other commands is refused with \"{Locked}\"", notRefused.Count == 0 && walled.Length > 100, string.Join("; ", notRefused));
        c.Check("…and a name that is no command is refused the same way (the wall doesn't tell which names exist)",
            await Harness.Try(app, "noSuchCommand", new { }) is UserError { Message: Locked });
        c.Check("…the vault file, the settings file, the vault in memory, the files in the folder and the launches are the same afterwards", Snapshot() == before, Snapshot());
        c.Check("…and nothing reached the page but the lock screen's own state", Since(postsBefore).All(LockScreenState), string.Join(" // ", Since(postsBefore).Where(p => !LockScreenState(p)).Select(p => p.Length > 100 ? p[..100] : p)));
        c.Check("…and no Windows notification", notices == 0, notices);

        // The same through the page's own door: the reply says why.
        var wrongReply = new List<string>();
        int id = 100;
        foreach (string name in walled)
        {
            int at = Posts();
            await app.HandleMessageAsync($"{{\"id\":{++id},\"cmd\":\"{name}\",\"args\":{{}}}}");
            var got = Since(at);
            string[] replies = got.Where(p => p.StartsWith("{\"type\":\"reply\",\"id\":" + id + ",", StringComparison.Ordinal)).ToArray();
            if (replies.Length != 1 || !(replies[0].Contains("\"ok\":false") && replies[0].Contains(Locked)) || !got.Except(replies).All(LockScreenState))
                wrongReply.Add($"{name}: {string.Join(" // ", got.Select(p => p.Length > 80 ? p[..80] : p))}");
        }
        c.Check("sent through the page's message door, each gets one reply: not ok, \"" + Locked + "\"", wrongReply.Count == 0, string.Join("; ", wrongReply));
        c.Check("…and still nothing changed", Snapshot() == before, Snapshot());
        string answers;
        lock (posts) answers = string.Join("", posts.Skip(postsBefore));
        c.Check("…and no account name or cookie is in anything the page got", !answers.Contains("WallUser77") && !answers.Contains("WallAlias77") && !answers.Contains(secret.Cookie));

        // The tray, the mini window and the panels have doors of their own.
        int postsAfterReplies = Posts();
        app.TrayOpenAccount(secret.Id);
        app.TrayRunPreset(preset.Id);
        app.TrayTogglePanel(Guid.NewGuid());
        await Task.Delay(300); // the first two are async void: a start would show within this
        // The app's own timers may push the lock screen's state meanwhile (that is allowed); nothing else may reach the page.
        var later = Since(postsAfterReplies);
        bool onlyLockedState = later.All(LockScreenState);
        c.Check("the tray's Open and Run preset do nothing behind the lock screen: no launch, no change, no toast", Snapshot() == before && notices == 0 && onlyLockedState,
            $"same {Snapshot() == before}, notices {notices}, {later.Length} posts, only the locked state {onlyLockedState}");
        var tray = app.TrayState();
        c.Check("the tray menu counts as closed, with no presets or clients", !tray.Open && tray.Presets.Count == 0, $"open {tray.Open}, {tray.Presets.Count} presets");
        c.Check("…and lists no macros and no running clients", app.TrayMacros().Count == 0 && app.RunningClients().Count == 0 && !app.Unlocked);

        // Those it may answer: not the wall's refusal (their own answer, or their own complaint).
        var refusedByWall = new List<string>();
        foreach (string name in allowed.Where(a => a != "lock").OrderBy(a => a, StringComparer.Ordinal))
        {
            try
            {
                await Harness.Call(app, name, new { });
            }
            catch (UserError e) when (e.Message == Locked)
            {
                refusedByWall.Add(name);
            }
            catch (Exception)
            {
                // its own complaint about the empty arguments
            }
        }
        c.Check("the lock screen's own commands are not refused by the wall", refusedByWall.Count == 0, string.Join(", ", refusedByWall));
        c.Check("getState behind it names no account and no cookie", await Harness.Try(app, "getState", new { }) is { } state
            && JsonSerializer.Serialize(state) is var json && !json.Contains("WallUser77") && !json.Contains("WallAlias77") && !json.Contains(secret.Cookie));
    }
}
