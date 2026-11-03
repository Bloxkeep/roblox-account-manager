using System.Text.Json;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// The app's message box (Forms/Dialog.cs) as a page in its own window: it shows only once its page has drawn, sized to
/// it and centred; a click on the page answers (with "Don't ask again"); Esc/✕ and the timeout cancel. The answers are
/// given the way the page gives them (its "act" message), never by pressing keys on the desktop. Needs the WebView2
/// runtime. (No screen pictures here: a box shown by a background process can open behind other windows, and a capture
/// of the screen would hold whatever is there.)
/// </summary>
static class DialogSuite
{
    public static Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        var done = new TaskCompletionSource();
        var thread = new Thread(() =>
        {
            using var host = new Form { ShowInTaskbar = false, StartPosition = FormStartPosition.Manual, Location = new Point(-2000, -2000), Size = new Size(200, 200) };
            host.Shown += (_, _) => host.BeginInvoke(() =>
            {
                try
                {
                    Body(c);
                }
                catch (Exception ex)
                {
                    c.Check("the dialog checks ran to their end", false, ex);
                }
                finally
                {
                    host.Close();
                }
            });
            WindowsFormsSynchronizationContext.AutoInstall = true;
            Application.Run(host);
            done.SetResult();
        });
        thread.SetApartmentState(ApartmentState.STA);
        thread.Start();
        return done.Task;
    }

    /// <summary>Runs <paramref name="then"/> on the box once it is on screen (or after 15 s, with null).</summary>
    static void WhenShown(Action<Dialog?> then)
    {
        var clock = System.Diagnostics.Stopwatch.StartNew();
        var timer = new System.Windows.Forms.Timer { Interval = 50 };
        timer.Tick += (_, _) =>
        {
            var box = Application.OpenForms.OfType<Dialog>().FirstOrDefault();
            bool shown = box is not null && Harness.Get<bool>(box, "_shown");
            if (!shown && clock.ElapsedMilliseconds < 15000)
                return;
            timer.Dispose();
            then(shown ? box : null);
        };
        timer.Start();
    }

    static void Answer(Dialog box, string what, object? args = null)
    {
        using var doc = JsonDocument.Parse(JsonSerializer.Serialize(args ?? new { }));
        Harness.Invoke(box, "OnAct", what, doc.RootElement.Clone());
    }

    static void Body(Checks c)
    {
        // The uninstall question's fallback (Windows' own dialog, after the page failed to draw in 6 s) gets what is left of the
        // wait, not all of it again: Velopack allows the step 30 s in all.
        var six = TimeSpan.FromSeconds(6);
        c.Check("the fallback gets the timeout less the time the page took", Dialog.Remaining(TimeSpan.FromSeconds(20), six) == TimeSpan.FromSeconds(14));
        c.Check("…at least a few seconds, however long that took", Dialog.Remaining(TimeSpan.FromSeconds(5), six) == TimeSpan.FromSeconds(3));
        c.Check("…and no timeout stays none", Dialog.Remaining(null, six) is null);

        Rectangle seen = default;
        float scale = 1;
        WhenShown(box =>
        {
            c.Check("the box shows once its page has drawn", box is not null);
            if (box is null)
                return;
            seen = box.Bounds;
            scale = box.DeviceDpi / 96f;
            Answer(box, "pick", new { index = 0, remember = true });
        });
        var (yes, remember) = Dialog.ConfirmRemember(null, "Close ALL Roblox running on this PC (including Roblox in the system tray)?", "Close all", "Cancel", danger: true);
        c.Check("a click on the page answers, with Don't ask again", yes && remember, $"{yes} {remember}");
        var work = Screen.FromRectangle(seen).WorkingArea;
        c.Check("sized to its page (460 CSS px wide, as tall as its text) and centred on the screen", Math.Abs(seen.Width - 460 * scale) <= 2 && seen.Height > 120 * scale && seen.Height < 400 * scale
            && Math.Abs(seen.Left + seen.Width / 2 - (work.Left + work.Width / 2)) <= 2, seen);

        WhenShown(box =>
        {
            if (box is null)
                return;
            Answer(box, "cancel");
        });
        int picked = Dialog.Show(null, "Bloxkeep is being removed from this PC. Your accounts and settings stay unless you delete them.", "Delete your accounts too?",
            Dialog.Kind.Warning, [new Dialog.Choice("Keep", "Installing again picks up where you left off."), new Dialog.Choice("Delete", "This can't be undone.", Danger: true)],
            defaultIndex: 0, cancelIndex: 0);
        c.Check("Esc or ✕ picks the cancel choice", picked == 0, picked);

        var clock = System.Diagnostics.Stopwatch.StartNew();
        picked = Dialog.Show(null, "Kept if you don't choose.", null, Dialog.Kind.Info, [new Dialog.Choice("Yes"), new Dialog.Choice("No")], timeout: TimeSpan.FromSeconds(3));
        c.Check("the timeout picks the cancel one", picked == 1 && clock.Elapsed < TimeSpan.FromSeconds(8), $"{picked} after {clock.Elapsed.TotalSeconds:0.0} s");

        WhenShown(box =>
        {
            if (box is not null)
                Answer(box, "pick", new { index = 0, remember = true });
        });
        bool ok = Dialog.Confirm(null, "Quit anyway?", "Quit", "Cancel");
        c.Check("Confirm: yes", ok);
        c.Check("no box left open", !Application.OpenForms.OfType<Dialog>().Any());
    }
}
