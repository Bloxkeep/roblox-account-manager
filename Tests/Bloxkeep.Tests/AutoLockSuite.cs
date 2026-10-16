using System.Text.Json;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// The lock screen after a while without use (AppController.AutoLock.cs): when it is due, what the page gets and may do
/// behind it, the master password to take it down, the Lock button from it, and a restart for an update behind it. The
/// idle time is handed in (Windows' own input clock isn't something a test can wind on).
/// </summary>
static class AutoLockSuite
{
    public static async Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);

        // When it is due.
        c.Check("0 minutes (Never): never due, whatever the wait", !AppController.AutoLockDue(0, TimeSpan.FromDays(3)));
        c.Check("a negative number is never due", !AppController.AutoLockDue(-5, TimeSpan.FromHours(1)));
        c.Check("15 minutes: not at 14:59", !AppController.AutoLockDue(15, TimeSpan.FromMinutes(15) - TimeSpan.FromSeconds(1)));
        c.Check("15 minutes: due at 15:00", AppController.AutoLockDue(15, TimeSpan.FromMinutes(15)));
        c.Check("15 minutes: due after a night", AppController.AutoLockDue(15, TimeSpan.FromHours(9)));

        var seed = Vault.Create(Harness.Password);
        var secret = new Account { Username = "SecretUser42", Alias = "HiddenAlias42", UserId = 4242 };
        seed.Data.Accounts.Add(secret);
        seed.Save();
        seed.Dispose();

        var posts = new List<string>();
        using var form = new Form();
        using var app = new AppController(form, json => { lock (posts) posts.Add(json); });
        await Harness.Unlock(app);
        app.PageAttached = true;
        bool ScreenLocked() => Harness.Get<bool>(app, "_screenLocked");
        Vault? VaultNow() => (Vault?)typeof(AppController).GetField("_vault", Harness.F)!.GetValue(app);
        void Check(TimeSpan idle, DateTime? now = null) => Harness.Invoke(app, "CheckAutoLock", idle, now ?? DateTime.UtcNow);
        void LongAgoActive() => Harness.Set(app, "_activeAt", DateTime.UtcNow.AddHours(-3));
        string LastState() { lock (posts) return posts.Last(p => p.Contains("\"name\":\"state\"")); }
        int Posts() { lock (posts) return posts.Count; }
        JsonElement StateOf(object? reply) => JsonSerializer.SerializeToElement(reply, new JsonSerializerOptions { PropertyNamingPolicy = JsonNamingPolicy.CamelCase });

        // The setting: only the choices offered.
        await Harness.Call(app, "setSettings", new { autoLockMinutes = 15 });
        var vault = VaultNow()!;
        c.Check("Lock automatically: 15 minutes saved", vault.Data.Settings.AutoLockMinutes == 15, vault.Data.Settings.AutoLockMinutes);
        await Harness.Call(app, "setSettings", new { autoLockMinutes = 7 });
        c.Check("a time Settings doesn't offer (7) is ignored", vault.Data.Settings.AutoLockMinutes == 15, vault.Data.Settings.AutoLockMinutes);
        await Harness.Call(app, "setSettings", new { autoLockMinutes = -1 });
        c.Check("a negative time is ignored", vault.Data.Settings.AutoLockMinutes == 15, vault.Data.Settings.AutoLockMinutes);

        // Choosing the setting counts as using the PC: a long idle reading right after doesn't lock.
        Check(TimeSpan.FromHours(2));
        c.Check("just chosen: Windows' long idle reading doesn't lock yet", !ScreenLocked());
        LongAgoActive();
        Check(TimeSpan.FromMinutes(15) - TimeSpan.FromSeconds(1));
        c.Check("14:59 without input: still open", !ScreenLocked());

        await Harness.Call(app, "setSettings", new { autoLockMinutes = 0 });
        LongAgoActive();
        Check(TimeSpan.FromDays(1));
        c.Check("Never: a day without input doesn't lock", !ScreenLocked());
        await Harness.Call(app, "setSettings", new { autoLockMinutes = 15 });

        // Due: the lock screen, the vault open behind it.
        LongAgoActive();
        Check(TimeSpan.FromMinutes(15));
        c.Check("15:00 without input: the lock screen is up", ScreenLocked());
        c.Check("the vault stays open behind it (the same one: auto-rejoin, schedules and Discord carry on)", ReferenceEquals(VaultNow(), vault));
        string locked = LastState();
        c.Check("the page is told: locked, with the screen lock's minutes", locked.Contains("\"locked\":true") && locked.Contains("\"screenLock\":{\"minutes\":15}"), locked.Length);
        c.Check("the state the page gets names no account", !locked.Contains("SecretUser42") && !locked.Contains("HiddenAlias42") && locked.Contains("\"accounts\":[]"));
        c.Check("nor its settings or Discord", locked.Contains("\"settings\":null") && locked.Contains("\"discord\":null"));
        int before = Posts();
        Check(TimeSpan.FromHours(5));
        c.Check("already up: the check doesn't put it up again (no new state)", Posts() == before);

        // What the page may do behind it.
        foreach (var (cmd, args) in new (string, object)[]
        {
            ("reveal", new { id = secret.Id, field = "password" }), ("copy", new { id = secret.Id, field = "username" }),
            ("setSettings", new { autoLockMinutes = 0 }), ("launch", new { ids = new[] { secret.Id }, placeInput = "", jobId = "" }),
            ("exportActivity", new { }), ("totpCode", new { id = secret.Id }), ("toggleMini", new { }), ("setDiscord", new { enabled = true }),
            ("closeAll", new { }), ("playStats", new { days = 7 }),
        })
        {
            var r = await Harness.Try(app, cmd, args);
            c.Check($"behind the lock screen, {cmd} is refused", r is UserError e && e.Message.Length > 0, r);
        }
        c.Check("…and Lock automatically wasn't turned off by the refused setSettings", vault.Data.Settings.AutoLockMinutes == 15);
        var got = StateOf(((dynamic?)await Harness.Call(app, "getState", new { }))!);
        c.Check("getState answers: locked, no accounts, no view to restore",
            got.GetProperty("state").GetProperty("locked").GetBoolean() && got.GetProperty("state").GetProperty("accounts").GetArrayLength() == 0
            && got.GetProperty("view").ValueKind == JsonValueKind.Null, got);
        var lang = await Harness.Try(app, "setLanguage", new { language = "en" });
        c.Check("the lock screen's language switch still works", lang is not UserError, lang);
        before = Posts();
        Harness.Invoke(app, "Toast", "Farm was kicked", "warn");
        Harness.Invoke(app, "Progress", "Opening SecretUser42");
        c.Check("no toast or progress reaches the page meanwhile (it could name an account)", Posts() == before);
        c.Check("the mini window lists no client meanwhile", app.RunningClients().Count == 0);

        // A command the page sends anyway is answered with the refusal, through the real reply path.
        before = Posts();
        await app.HandleMessageAsync("{\"id\":7,\"cmd\":\"reveal\",\"args\":{\"id\":\"" + secret.Id + "\",\"field\":\"password\"}}");
        string reply;
        lock (posts) reply = posts.Skip(before).Last();
        c.Check("its reply: refused, \"Bloxkeep is locked.\"", reply.Contains("\"ok\":false") && reply.Contains("Bloxkeep is locked."), reply);

        // The password.
        var empty = await Harness.Try(app, "unlock", new { password = "" });
        c.Check("an empty password: asked for", empty is UserError, empty);
        var wrong = await Harness.Try(app, "unlock", new { password = "not-the-password" });
        c.Check("a wrong password: refused, in the app's words", wrong is UserError { Message: "Wrong master password." }, wrong);
        c.Check("…and the lock screen stays up", ScreenLocked());
        var right = StateOf(await Harness.Call(app, "unlock", new { password = Harness.Password }));
        c.Check("the right password takes it down", !ScreenLocked() && !right.GetProperty("locked").GetBoolean(), right.GetProperty("locked"));
        c.Check("…and the page gets the accounts again", right.GetProperty("accounts").GetArrayLength() == 1);
        c.Check("…the vault never closed", ReferenceEquals(VaultNow(), vault));
        Check(TimeSpan.FromHours(3));
        c.Check("just unlocked: Windows' old idle reading doesn't put it straight back up", !ScreenLocked());
        var reveal = await Harness.Try(app, "reveal", new { id = secret.Id, field = "password" });
        c.Check("commands go through again", reveal is not UserError { Message: "Bloxkeep is locked." }, reveal);

        // A command that started before the lock screen came up and ends behind it: what it did stands, its answer doesn't
        // reach the lock screen. Changing the master password takes a moment (key stretching), long enough to lock meanwhile.
        const string NewPassword = "harness-pass-2";
        var changing = Harness.Try(app, "changePassword", new { current = Harness.Password, next = NewPassword });
        LongAgoActive();
        Check(TimeSpan.FromHours(1));
        var changed = await changing;
        c.Check("a command still running when it locked: its answer is withheld", changed is UserError { Message: "Bloxkeep is locked." }, changed);
        var oldPw = await Harness.Try(app, "unlock", new { password = Harness.Password });
        c.Check("…what it did stands: the old password no longer opens the lock screen", oldPw is UserError && ScreenLocked(), oldPw);
        await Harness.Call(app, "unlock", new { password = NewPassword });
        c.Check("…and the new one does", !ScreenLocked());
        await Harness.Call(app, "changePassword", new { current = NewPassword, next = Harness.Password });

        // The Lock button from the lock screen: Bloxkeep closes for real, and a normal unlock opens it again.
        LongAgoActive();
        Check(TimeSpan.FromHours(1));
        c.Check("locked again after a while", ScreenLocked());
        await Harness.Call(app, "lock", new { });
        c.Check("Lock from the lock screen closes the vault", VaultNow() is null);
        c.Check("…and isn't a screen lock any more", !ScreenLocked());
        await Harness.Unlock(app);
        vault = VaultNow()!;
        c.Check("a normal unlock afterwards opens it, screen not locked", vault is not null && !ScreenLocked());

        // Locked for real while the lock screen's password is still being checked: the lock stands.
        LongAgoActive();
        Check(TimeSpan.FromHours(1));
        var unlocking = Harness.Try(app, "unlock", new { password = Harness.Password });
        await Harness.Call(app, "lock", new { });
        var late = await unlocking;
        c.Check("an unlock that finishes after Lock doesn't open anything", VaultNow() is null && !ScreenLocked(), late);
        await Harness.Unlock(app);

        // A restart for an update behind the lock screen comes back behind it (the key travels for the background).
        vault = VaultNow()!;
        var (key, salt, iterations) = vault.ExportKey();
        foreach (bool screen in new[] { true, false })
        {
            using var form2 = new Form();
            using var next = new AppController(form2, _ => { });
            next.Resume(new Relaunch.State
            {
                Key = Convert.ToBase64String(key), Salt = Convert.ToBase64String(salt), Iterations = iterations,
                ScreenLocked = screen, View = "{\"view\":\"accounts\"}",
            });
            bool nextLocked = Harness.Get<bool>(next, "_screenLocked");
            bool open = typeof(AppController).GetField("_vault", Harness.F)!.GetValue(next) is not null;
            var first = StateOf(Harness.Invoke(next, "InitialState"));
            if (screen)
            {
                c.Check("restarted behind the lock screen: still behind it, the vault open", nextLocked && open);
                c.Check("…the page gets no accounts and no view back", first.GetProperty("state").GetProperty("locked").GetBoolean()
                    && first.GetProperty("view").ValueKind == JsonValueKind.Null);
            }
            else
            {
                c.Check("restarted unlocked: unlocked again, as before", !nextLocked && open && !first.GetProperty("state").GetProperty("locked").GetBoolean());
            }
        }
        System.Security.Cryptography.CryptographicOperations.ZeroMemory(key);
    }
}
