using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// Anti-AFK's decisions with made-up process ids, window handles, idle times, lock and full-screen states: which windows
/// are nudged, why the others wait, retries, and windows already open when Bloxkeep starts. The nudge itself never runs
/// (_nudging set, AntiAfkDue called directly), so nothing on the desktop is touched.
/// </summary>
static class AfkSuite
{
    public static async Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        var seed = Vault.Create(Harness.Password);
        seed.Data.Settings.AntiAfk = true;
        seed.Save();
        using var form = new Form();
        using var app = new AppController(form, _ => { });
        await Harness.Unlock(app);
        Harness.Set(app, "_nudging", true);
        var t = typeof(AppController);
        var run = t.GetMethod("RunAntiAfk", Harness.F)!;
        var plan = t.GetMethod("AntiAfkDue", Harness.F)!;
        var retryAt = t.GetMethod("NudgeRetryAt", Harness.F)!;
        var lastInput = Harness.Get<Dictionary<int, DateTime>>(app, "_lastInput");
        var retry = Harness.Get<Dictionary<int, DateTime>>(app, "_nudgeRetryAt");
        var held = Harness.Get<System.Collections.IDictionary>(app, "_afkHeld");
        var afk = typeof(Vault).Assembly.GetType("RobloxManager.AntiAfk")!;
        var lastChance = (TimeSpan)afk.GetField("LastChance", Harness.F)!.GetValue(null)!;

        // Windows already open when Bloxkeep starts (an update's restart).
        var now = DateTime.UtcNow;
        run.Invoke(app, [now, new Dictionary<int, List<IntPtr>> { [990001] = [1], [990002] = [2] }]);
        c.Check("open at start: due at once, even for a nudge while the PC is in use", lastInput.TryGetValue(990001, out var a) && a == now - lastChance, a);
        c.Check("both windows open at start", lastInput.TryGetValue(990002, out var b) && b == now - lastChance, b);
        var later = now.AddSeconds(30);
        run.Invoke(app, [later, new Dictionary<int, List<IntPtr>> { [990001] = [1], [990002] = [2], [990003] = [3] }]);
        c.Check("opened later: counts from now", lastInput.TryGetValue(990003, out var cc) && cc == later, cc);
        c.Check("start windows keep their time", lastInput[990001] == now - lastChance);
        run.Invoke(app, [later.AddSeconds(1), new Dictionary<int, List<IntPtr>> { [990003] = [3] }]);
        c.Check("closed windows forgotten", !lastInput.ContainsKey(990001) && lastInput.ContainsKey(990003));

        // The decision. Windows idle 5 min (A), 13 min (B), 19 min (C); D is a Roblox window that may be in front.
        const int A = 1001, B = 1002, C = 1003, D = 1004, Other = 4242;
        var wins = new Dictionary<int, List<IntPtr>> { [A] = [11], [B] = [12], [C] = [13], [D] = [14] };
        var on = new HashSet<int> { A, B, C, D };
        bool fullAsked = false;
        List<int> Decide(TimeSpan idle, bool locked = false, int foreground = Other, bool fullScreen = false, HashSet<int>? only = null)
        {
            var at = DateTime.UtcNow;
            lastInput.Clear(); retry.Clear(); held.Clear();
            lastInput[A] = at.AddMinutes(-5); lastInput[B] = at.AddMinutes(-13); lastInput[C] = at.AddMinutes(-19); lastInput[D] = at.AddMinutes(-1);
            fullAsked = false;
            return ((List<int>)plan.Invoke(app, [at, only ?? on, wins, locked, idle, foreground, (Func<bool>)(() => { fullAsked = true; return fullScreen; })])!).Order().ToList();
        }
        string Held(int pid) => held.Contains(pid) ? held[pid]!.ToString()! : "-";

        var r = Decide(TimeSpan.FromSeconds(90));
        c.Check("away: windows idle 12+ min nudged, the 5-min one not", r.SequenceEqual([B, C]), string.Join(",", r));
        r = Decide(TimeSpan.FromSeconds(20));
        c.Check("in use, 20 s since input: the window about to be kicked goes, the 13-min one waits", r.SequenceEqual([C]), string.Join(",", r));
        c.Check("in use: due windows noted as held (PC in use)", Held(B) == "InUse" && Held(C) == "InUse" && Held(A) == "-", $"{Held(A)} {Held(B)} {Held(C)}");
        r = Decide(TimeSpan.FromSeconds(20), only: [A, B]);
        c.Check("in use, nothing near its kick: nothing nudged", r.Count == 0, string.Join(",", r));
        c.Check("no last-chance window: full-screen state not asked", !fullAsked);
        r = Decide(TimeSpan.FromSeconds(1));
        c.Check("in use, 1 s pause: last-chance window waits", r.Count == 0, string.Join(",", r));
        r = Decide(TimeSpan.FromSeconds(3));
        c.Check("in use, 3 s pause: only the window about to be kicked", r.SequenceEqual([C]), string.Join(",", r));
        c.Check("full-screen state asked only when it matters", fullAsked);
        r = Decide(TimeSpan.FromSeconds(3), fullScreen: true);
        c.Check("full-screen app in front: not interrupted", r.Count == 0 && Held(C) == "FullScreen", $"{string.Join(",", r)} {Held(C)}");
        r = Decide(TimeSpan.FromSeconds(3), foreground: D, fullScreen: true);
        c.Check("a Roblox window in front (even full screen): the other one about to be kicked goes", r.SequenceEqual([C]) && !fullAsked, $"{string.Join(",", r)} asked={fullAsked}");
        c.Check("the Roblox window in front counts as played in", (DateTime.UtcNow - lastInput[D]).TotalSeconds < 5, lastInput[D]);
        r = Decide(TimeSpan.FromMinutes(5), locked: true);
        c.Check("locked: nothing tried, noted as locked", r.Count == 0 && Held(B) == "Locked" && Held(C) == "Locked", $"{string.Join(",", r)} {Held(B)} {Held(C)}");

        // A refused window waits before its next try, but not past its last chance.
        var at0 = DateTime.UtcNow;
        double In(int misses, double idleMin) => ((DateTime)retryAt.Invoke(null, [at0, misses, at0.AddMinutes(-idleMin)])! - at0).TotalMinutes;
        c.Check("miss 1 at 13 min: next try in 1 min", Math.Abs(In(1, 13) - 1) < .01, In(1, 13));
        c.Check("miss 4 at 13 min: not after the last chance (18 min)", Math.Abs(In(4, 13) - 5) < .01, In(4, 13));
        c.Check("past the last chance: tried again within a minute", Math.Abs(In(9, 19) - 1) < .01, In(9, 19));
        c.Check("long run of misses early on: 15 min at most", Math.Abs(In(5, 2) - 15) < .01, In(5, 2));

        // A last-chance window keeps its wait while the PC is in use; the others' waits are cleared.
        lastInput.Clear(); retry.Clear(); held.Clear();
        var t1 = DateTime.UtcNow;
        lastInput[B] = t1.AddMinutes(-13); lastInput[C] = t1.AddMinutes(-19);
        retry[B] = t1.AddMinutes(3); retry[C] = t1.AddSeconds(40);
        var rw = (List<int>)plan.Invoke(app, [t1, new HashSet<int> { B, C }, wins, false, TimeSpan.FromSeconds(3), Other, (Func<bool>)(() => false)])!;
        c.Check("last-chance window still waiting: not tried yet", rw.Count == 0, string.Join(",", rw));
        c.Check("in use clears the other windows' waits only", !retry.ContainsKey(B) && retry.ContainsKey(C), string.Join(",", retry.Keys));

        Unseen(c, afk);
    }

    /// <summary>
    /// The nudge's parts on this process's own windows, off-screen and never activated: made unseen and click-through and
    /// back, put back in their place among the windows, and left unseen by a nudge cut short. No key is pressed and no
    /// window is brought to the front. All on one thread, with no await in between: a style change on another thread's
    /// window would wait for that thread to answer.
    /// </summary>
    static void Unseen(Checks c, Type afk)
    {
        c.Check("INPUT is the size SendInput expects (40 bytes in a 64-bit process)",
            (int)afk.GetProperty("InputSize", Harness.F)!.GetValue(null)! == (Environment.Is64BitProcess ? 40 : 28));
        var hide = afk.GetMethod("Hide", Harness.F)!;
        var show = afk.GetMethod("Show", Harness.F)!;
        var putBack = afk.GetMethod("PutBack", Harness.F)!;
        var place = afk.GetMethod("Place", Harness.F)!;
        var recover = afk.GetMethod("Recover", Harness.F)!;
        var nudge = afk.GetMethod("NudgeAsync", Harness.F)!;
        byte hidden = (byte)afk.GetField("HiddenAlpha", Harness.F)!.GetValue(null)!;
        const int ours = WS_EX_LAYERED | WS_EX_TRANSPARENT;

        using var x = new OffScreen();
        using var y = new OffScreen();
        using var z = new OffScreen();
        using var faded = new OffScreen { Opacity = 0.8 };
        using var top = new OffScreen();
        using var ghost = new OffScreen(); // never shown
        foreach (var f in new[] { x, y, z, faded, top })
            f.Show();
        Application.DoEvents(); // a window whose thread hasn't looked at its messages for 5 s counts as not answering
        IntPtr X = x.Handle, Y = y.Handle, Z = z.Handle, Faded = faded.Handle, Top = top.Handle;
        (byte Alpha, uint Flags) Alpha(IntPtr w) => GetLayeredWindowAttributes(w, out _, out byte a, out uint fl) ? (a, fl) : ((byte)0, 0u);
        object Hide(IntPtr w, bool minimized = false) => hide.Invoke(null, [w, place.Invoke(null, [w]), minimized])!;

        // Unseen and click-through, then back as it was.
        int plain = GetWindowLong(X, GWL_EXSTYLE);
        var before = Hide(X);
        c.Check("hidden: layered and click-through", (GetWindowLong(X, GWL_EXSTYLE) & ours) == ours, GetWindowLong(X, GWL_EXSTYLE).ToString("x"));
        c.Check("hidden: all but invisible", Alpha(X) is var ha && ha.Alpha == hidden && (ha.Flags & LWA_ALPHA) != 0, Alpha(X));
        show.Invoke(null, [before]);
        c.Check("shown again: its own style back", GetWindowLong(X, GWL_EXSTYLE) == plain, $"{GetWindowLong(X, GWL_EXSTYLE):x} vs {plain:x}");

        // A window that has an opacity of its own keeps it.
        var own = Alpha(Faded);
        int fadedStyle = GetWindowLong(Faded, GWL_EXSTYLE);
        before = Hide(Faded);
        c.Check("own opacity: hidden all the same", Alpha(Faded).Alpha == hidden, Alpha(Faded));
        show.Invoke(null, [before]);
        c.Check("own opacity: back to its own, still layered, not click-through",
            Alpha(Faded) == own && GetWindowLong(Faded, GWL_EXSTYLE) == fadedStyle, $"{Alpha(Faded)} vs {own}, {GetWindowLong(Faded, GWL_EXSTYLE):x} vs {fadedStyle:x}");

        // Back in its place: x over y over z, with a hidden window between y and z (like the input-method window each
        // program has, which comes along to the front with it); z comes to the front for its nudge, then goes back under y.
        IntPtr Ghost = ghost.Handle;
        SetWindowPos(X, HWND_TOP, 0, 0, 0, 0, Keep);
        SetWindowPos(Y, X, 0, 0, 0, 0, Keep);
        SetWindowPos(Ghost, Y, 0, 0, 0, 0, Keep);
        SetWindowPos(Z, Ghost, 0, 0, 0, 0, Keep);
        var zPlace = place.Invoke(null, [Z])!;
        c.Check("its place: under the shown window that was above it", (IntPtr)zPlace.GetType().GetField("Item1")!.GetValue(zPlace)! == Y,
            zPlace.GetType().GetField("Item1")!.GetValue(zPlace));
        before = Hide(Z);
        SetWindowPos(Z, HWND_TOP, 0, 0, 0, 0, Keep);
        SetWindowPos(Ghost, HWND_TOP, 0, 0, 0, 0, Keep);
        putBack.Invoke(null, [before]);
        show.Invoke(null, [before]);
        c.Check("put back under the same window", GetWindow(Z, GW_HWNDPREV) == Y && GetWindow(Y, GW_HWNDPREV) == X,
            $"above z {GetWindow(Z, GW_HWNDPREV)}, y {Y}");
        // Never under a window that is always on top: it would become always on top too.
        SetWindowPos(Top, HWND_TOPMOST, 0, 0, 0, 0, Keep);
        SetWindowPos(X, HWND_TOP, 0, 0, 0, 0, Keep);
        var xPlace = place.Invoke(null, [X])!;
        // What is above it besides the always-on-top window: nothing, or the app the person is using (Windows keeps a
        // program in the background from putting its window over the one in front), never an always-on-top one.
        var xAbove = (IntPtr)xPlace.GetType().GetField("Item1")!.GetValue(xPlace)!;
        c.Check("first of the usual windows: goes under none of the always-on-top ones",
            xAbove != Top && (xAbove == IntPtr.Zero || (GetWindowLong(xAbove, GWL_EXSTYLE) & WS_EX_TOPMOST) == 0), xPlace);
        SetWindowPos(Top, HWND_NOTOPMOST, 0, 0, 0, 0, Keep);

        // Left unseen by a nudge cut short (Bloxkeep closed): seen again; a window faded by its own app is left alone.
        Hide(Y);
        int fadedAndThrough = fadedStyle | WS_EX_TRANSPARENT;
        SetWindowLong(Faded, GWL_EXSTYLE, fadedAndThrough);
        recover.Invoke(null, [new[] { Y, Faded }]);
        c.Check("recovered: neither layered nor click-through", (GetWindowLong(Y, GWL_EXSTYLE) & ours) == 0, GetWindowLong(Y, GWL_EXSTYLE).ToString("x"));
        c.Check("another app's faded window untouched", GetWindowLong(Faded, GWL_EXSTYLE) == fadedAndThrough && Alpha(Faded) == own);

        // A nudge that touches nothing: turned off, or no window that answers. Nothing pressed, nothing changed.
        int pressed = 0;
        Func<IntPtr, Task> press = _ => { pressed++; return Task.CompletedTask; };
        int xStyle = GetWindowLong(X, GWL_EXSTYLE);
        var off = ((Task<List<IntPtr>>)nudge.Invoke(null, [new List<IntPtr> { X }, (Func<bool>)(() => false), press])!).GetAwaiter().GetResult();
        c.Check("turned off: no window reached, its style untouched", off.Count == 0 && GetWindowLong(X, GWL_EXSTYLE) == xStyle);
        var dead = new IntPtr(0x7FFFFFF0);
        if (!IsWindow(dead))
        {
            var none = ((Task<List<IntPtr>>)nudge.Invoke(null, [new List<IntPtr> { dead }, (Func<bool>)(() => true), press])!).GetAwaiter().GetResult();
            c.Check("a window that's gone: skipped", none.Count == 0);
        }
        c.Check("no key pressed", pressed == 0, pressed);
    }

    /// <summary>A window off-screen, out of the taskbar, that never takes the keyboard when shown.</summary>
    sealed class OffScreen : Form
    {
        public OffScreen()
        {
            ShowInTaskbar = false;
            FormBorderStyle = FormBorderStyle.None;
            StartPosition = FormStartPosition.Manual;
            Location = new Point(-3000, -3000);
            Size = new Size(120, 80);
        }

        protected override bool ShowWithoutActivation => true;
    }

    const int GWL_EXSTYLE = -20, WS_EX_TRANSPARENT = 0x20, WS_EX_LAYERED = 0x80000, WS_EX_TOPMOST = 0x8;
    const uint LWA_ALPHA = 0x2, GW_HWNDPREV = 3;
    const uint Keep = 0x1 | 0x2 | 0x10; // SWP_NOSIZE | SWP_NOMOVE | SWP_NOACTIVATE
    static readonly IntPtr HWND_TOP = 0, HWND_TOPMOST = -1, HWND_NOTOPMOST = -2;

    [System.Runtime.InteropServices.DllImport("user32.dll", EntryPoint = "GetWindowLongW")] static extern int GetWindowLong(IntPtr hWnd, int index);
    [System.Runtime.InteropServices.DllImport("user32.dll", EntryPoint = "SetWindowLongW")] static extern int SetWindowLong(IntPtr hWnd, int index, int value);
    [System.Runtime.InteropServices.DllImport("user32.dll")] static extern bool GetLayeredWindowAttributes(IntPtr hWnd, out uint key, out byte alpha, out uint flags);
    [System.Runtime.InteropServices.DllImport("user32.dll")] static extern bool SetWindowPos(IntPtr hWnd, IntPtr after, int x, int y, int cx, int cy, uint flags);
    [System.Runtime.InteropServices.DllImport("user32.dll")] static extern IntPtr GetWindow(IntPtr hWnd, uint command);
    [System.Runtime.InteropServices.DllImport("user32.dll")] static extern bool IsWindow(IntPtr hWnd);
}
