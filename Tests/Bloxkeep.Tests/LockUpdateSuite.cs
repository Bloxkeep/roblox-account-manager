using System.Text.Json;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// An update found before the vault is opened installs at the lock screen (AppController.Relaunch.cs, 2026-10-25): the
/// screen is offered it while it downloads, gives it its place when nothing was typed, and it then goes with nobody's
/// use holding it back. Found after the unlock, it waits for a quiet moment rather than cutting in: until then one found
/// in the first 5 minutes after opening installed at once, unlocked or not (1.0.10 did so 26 s after the owner opened
/// Bloxkeep). Nothing is installed here: the decisions are asked, not acted on.
/// </summary>
static class LockUpdateSuite
{
    public static async Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        Vault.Create(Harness.Password).Save();
        using var form = new Form();
        string window = "shown";
        using var app = new AppController(form, _ => { }) { WindowNow = () => (window, true, false) };
        var updater = Harness.Get<AppUpdater>(app, "_updater");
        void Status(string status) => typeof(AppUpdater).GetField("<Status>k__BackingField", Harness.F)!.SetValue(updater, status);
        bool Offered() => (bool)typeof(AppController).GetProperty("UpdateOffersAtLock", Harness.F)!.GetValue(app)!;
        bool Goes() => (bool)Harness.Invoke(app, "UpdateGoesAtLock")!;
        bool Quiet(bool atLock) => (bool)Harness.Invoke(app, "QuietForRelaunch", DateTime.UtcNow, atLock)!;
        JsonElement LockUpdate()
        {
            var state = JsonSerializer.SerializeToElement(Harness.Invoke(app, "BuildState"), new JsonSerializerOptions(JsonSerializerDefaults.Web));
            return state.GetProperty("lockUpdate");
        }

        c.Check("no update found: the lock screen isn't offered one", !Offered() && LockUpdate().ValueKind == JsonValueKind.Null);
        Status("downloading");
        c.Check("an update downloading before the unlock: offered to the lock screen, not answered yet",
            Offered() && LockUpdate().GetProperty("go").ValueKind == JsonValueKind.Null);
        c.Check("…and nothing goes while the screen, on screen, hasn't given it its place", !Goes());
        await Harness.Call(app, "updateAtLock", new { go = true });
        c.Check("the lock screen gave its place: the update goes once downloaded, nobody's use holding it back",
            Goes() && Quiet(true) && LockUpdate().GetProperty("go").GetBoolean());
        await Harness.Call(app, "updateAtLock", new { go = false });
        c.Check("its link took the form back: nothing goes while the window is on screen, and the answer stays (a redraw keeps it)",
            !Goes() && !LockUpdate().GetProperty("go").GetBoolean());
        window = "hidden";
        c.Check("in the tray (opened at Windows' sign-in), still locked: it goes without asking", Goes());
        window = "shown";
        Status("error");
        Harness.Invoke(app, "OnUpdaterChanged");
        c.Check("a download that failed: no offer", !Offered());

        Status("ready");
        await Harness.Call(app, "updateAtLock", new { go = true });
        await Harness.Unlock(app);
        c.Check("unlocked: the offer and its answer go (fails on the old code: installed at once in the first 5 minutes)",
            !Offered() && !Goes() && LockUpdate().ValueKind == JsonValueKind.Null);
        c.Check("…and the update waits for a quiet moment: not in the first minutes, nor with the window on screen and the PC in use",
            !Quiet(false));
        Status("idle");
    }
}
