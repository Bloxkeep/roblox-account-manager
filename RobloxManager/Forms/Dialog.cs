using System.Runtime.InteropServices;
using System.Text.Json;
using Microsoft.Web.WebView2.Core;
using Microsoft.Web.WebView2.WinForms;

namespace RobloxManager;

/// <summary>
/// The app's own message box: dialog.html (ui/src/dialog/DialogView.tsx), the page's .modal in a small window of its own,
/// in the app's theme (until 2026-11 hand-drawn WinForms). Buttons side by side, or stacked when they are choices
/// (<see cref="Choice.Detail"/>). Enter presses the default; Esc, ✕, Alt+F4 and the timeout pick the cancel one (the last
/// by default). <see cref="Ui.Modal"/> shows it above the always-on-top windows.
/// <para>It waits unseen (cloaked) until its page has drawn, so it never flashes empty. Where the page can't start (no
/// WebView2 yet: the question that offers to install it; a browser that won't start), Windows' own task dialog asks
/// the same question instead.</para>
/// </summary>
sealed class Dialog : WebToolWindow
{
    public enum Kind { Info, Warning, Error }

    /// <summary>A button: its text, an optional line under it (a choice), and whether it destroys something.</summary>
    public sealed record Choice(string Text, string? Detail = null, bool Danger = false);

    /// <summary>How long the page may take to draw before Windows' own dialog asks instead.</summary>
    static readonly TimeSpan StartLimit = TimeSpan.FromSeconds(6);

    readonly object _data;
    readonly bool _owned;
    readonly Point _center;
    readonly System.Windows.Forms.Timer _limit = new();
    int _picked;
    bool _remembered, _shown, _failed;
    CoreWebView2Environment? _env;

    Dialog(string text, string? heading, Kind kind, IReadOnlyList<Choice> choices, int defaultIndex, int cancelIndex, string? footnote,
        IWin32Window? owner, string? remember)
    {
        _picked = cancelIndex;
        _owned = owner is not null;
        // Without an owner (the uninstaller's question) it is a window of its own on the taskbar, so it can't get lost.
        ShowInTaskbar = !_owned;
        _center = CenterFor(owner);
        // Unseen and tiny until the page has said its size; there already, so it starts at that monitor's scale.
        Bounds = new Rectangle(_center, new Size(1, 1));
        _data = new
        {
            text,
            heading,
            kind = kind switch { Kind.Error => "error", Kind.Warning => "warning", _ => "info" },
            choices = choices.Select(c => new { text = c.Text, detail = c.Detail, danger = c.Danger }),
            def = Math.Clamp(defaultIndex, 0, choices.Count - 1),
            cancel = cancelIndex,
            footnote,
            remember,
            theme = PageLook.Theme,
            customTheme = PageLook.CustomTheme,
            language = L.Language,
        };
        _limit.Interval = (int)StartLimit.TotalMilliseconds;
        _limit.Tick += (_, _) =>
        {
            _limit.Stop();
            if (!_shown)
                Fail();
        };
    }

    protected override string PageFile => "dialog.html";
    protected override TimeSpan KeepBrowser => TimeSpan.FromSeconds(1);

    protected override CreateParams CreateParams
    {
        get
        {
            const int WS_EX_TOOLWINDOW = 0x80, WS_EX_APPWINDOW = 0x40000;
            var p = base.CreateParams;
            if (!_owned)
            {
                p.ExStyle &= ~WS_EX_TOOLWINDOW;
                p.ExStyle |= WS_EX_APPWINDOW;
            }
            return p;
        }
    }

    /// <summary>Over the owner's window when it is on screen, else in the middle of the screen the pointer is on.</summary>
    static Point CenterFor(IWin32Window? owner)
    {
        var form = owner is null ? null : (Control.FromHandle(owner.Handle) as Control)?.FindForm();
        if (form is { Visible: true, IsDisposed: false } && form.WindowState != FormWindowState.Minimized)
            return new Point(form.Left + form.Width / 2, form.Top + form.Height / 2);
        var work = Screen.FromPoint(Cursor.Position).WorkingArea;
        return new Point(work.Left + work.Width / 2, work.Top + work.Height / 2);
    }

    [DllImport("dwmapi.dll")]
    static extern int DwmSetWindowAttribute(IntPtr hwnd, int attribute, ref int value, int size);

    const int DWMWA_CLOAK = 13;

    void Cloak(bool on)
    {
        int value = on ? 1 : 0;
        try
        {
            DwmSetWindowAttribute(Handle, DWMWA_CLOAK, ref value, sizeof(int));
        }
        catch (Exception ex) when (ex is DllNotFoundException or EntryPointNotFoundException)
        {
        }
    }

    protected override void OnHandleCreated(EventArgs e)
    {
        base.OnHandleCreated(e);
        Cloak(true);
    }

    protected override async void OnShown(EventArgs e)
    {
        base.OnShown(e);
        _limit.Start();
        if (!await EnsureBrowserAsync() || IsDisposed)
        {
            Fail();
            return;
        }
        if (Controls.OfType<WebView2>().FirstOrDefault()?.CoreWebView2 is { } core)
            _env = core.Environment;
        SendEvent("dialog", _data);
    }

    protected override void OnPageReady()
    {
        if (!IsDisposed && !_failed)
            SendEvent("dialog", _data);
    }

    /// <summary>The page has drawn: sized to it (never taller than the screen; the text then scrolls), centred, shown.</summary>
    protected override void OnPageSize(Size css, JsonElement args)
    {
        if (_failed || IsDisposed)
            return;
        float scale = ScaleNow;
        var work = Screen.FromPoint(_center).WorkingArea;
        var size = Scaled(css, scale);
        size.Width = Math.Min(size.Width, work.Width - (int)(16 * scale));
        size.Height = Math.Min(size.Height, work.Height - (int)(40 * scale));
        if (_shown)
        {
            if (Size != size)
                Size = size;
            return;
        }
        var at = Ui.Inside(new Rectangle(new Point(_center.X - size.Width / 2, _center.Y - size.Height / 2), size), work, 0);
        // The place first, then the size (TrayWindow, 2026-10-30: set together on a screen at another scale, Windows
        // resized it again by the ratio).
        Location = at;
        Bounds = new Rectangle(at, size);
        _shown = true;
        _limit.Stop();
        Cloak(false);
        Activate();
        Controls.OfType<WebView2>().FirstOrDefault()?.Focus();
    }

    protected override void OnAct(string what, JsonElement args)
    {
        switch (what)
        {
            case "pick":
                int index = args.TryGetProperty("index", out var i) && i.ValueKind == JsonValueKind.Number ? i.GetInt32() : -1;
                if (index < 0)
                    return;
                _picked = index;
                _remembered = args.TryGetProperty("remember", out var r) && r.ValueKind == JsonValueKind.True;
                Close();
                break;
            case "cancel":
                Close();
                break;
        }
    }

    /// <summary>Its page crashed: Windows' own dialog asks the question instead.</summary>
    protected override void OnBrowserFailed() => Fail();

    void Fail()
    {
        if (_failed || IsDisposed)
            return;
        _failed = true;
        Close();
    }

    protected override void Dispose(bool disposing)
    {
        if (disposing)
            _limit.Dispose();
        base.Dispose(disposing);
    }

    // ───────────────────────── asking ─────────────────────────

    /// <returns>The index of the button clicked; <paramref name="cancelIndex"/> (the last by default) for Esc, the X or the timeout.</returns>
    public static int Show(IWin32Window? owner, string text, string? heading, Kind kind, IReadOnlyList<Choice> choices,
        int defaultIndex = 0, int? cancelIndex = null, string? footnote = null, TimeSpan? timeout = null) =>
        Ask(owner, text, heading, kind, choices, defaultIndex, cancelIndex ?? choices.Count - 1, footnote, timeout, null).Picked;

    public static void Error(IWin32Window? owner, string text) =>
        Show(owner, text, null, Kind.Error, [new Choice(L.T("OK"))]);

    /// <summary><see cref="Confirm"/> with a "Don't ask again" box; Remember is true only with a yes.</summary>
    public static (bool Yes, bool Remember) ConfirmRemember(IWin32Window? owner, string text, string yes, string no, bool danger = false)
    {
        var (picked, remember) = Ask(owner, text, null, danger ? Kind.Warning : Kind.Info, [new Choice(yes, Danger: danger), new Choice(no)],
            danger ? 1 : 0, 1, null, null, L.T("Don't ask again"));
        return (picked == 0, picked == 0 && remember);
    }

    /// <summary>Yes or no; with <paramref name="danger"/> the "yes" is red and Enter means "no".</summary>
    public static bool Confirm(IWin32Window? owner, string text, string yes, string no, bool danger = false) =>
        Show(owner, text, null, danger ? Kind.Warning : Kind.Info, [new Choice(yes, Danger: danger), new Choice(no)], defaultIndex: danger ? 1 : 0) == 0;

    static (int Picked, bool Remember) Ask(IWin32Window? owner, string text, string? heading, Kind kind, IReadOnlyList<Choice> choices,
        int defaultIndex, int cancelIndex, string? footnote, TimeSpan? timeout, string? remember)
    {
        // A page needs a window thread: asked from another one, the question goes to the app's.
        if (Thread.CurrentThread.GetApartmentState() != ApartmentState.STA)
        {
            var ui = Application.OpenForms.Cast<Form>().FirstOrDefault(f => f.IsHandleCreated && !f.IsDisposed);
            if (ui is not null && ui.InvokeRequired)
                return ((int, bool))ui.Invoke(() => Ask(owner, text, heading, kind, choices, defaultIndex, cancelIndex, footnote, timeout, remember));
            return Plain(owner, text, heading, kind, choices, defaultIndex, cancelIndex, footnote, timeout, remember);
        }
        if (!WebView2Installed())
            return Plain(owner, text, heading, kind, choices, defaultIndex, cancelIndex, footnote, timeout, remember);

        var asked = System.Diagnostics.Stopwatch.StartNew();
        CoreWebView2Environment? env;
        (int, bool) answer;
        bool failed;
        using (var dialog = new Dialog(text, heading, kind, choices, defaultIndex, cancelIndex, footnote, owner, remember))
        using (var timer = new System.Windows.Forms.Timer())
        {
            if (timeout is { } after)
            {
                timer.Interval = (int)Math.Clamp(after.TotalMilliseconds, 1000, int.MaxValue);
                timer.Tick += (_, _) =>
                {
                    timer.Stop();
                    dialog._picked = cancelIndex;
                    dialog.Close();
                };
                dialog.Shown += (_, _) => timer.Start();
            }
            dialog.ShowDialog(owner);
            failed = dialog._failed;
            answer = (dialog._picked, dialog._remembered);
            env = dialog._env;
        }
        if (failed)
        {
            Log.Note("Message box: its page didn't start; Windows' dialog asked instead.");
            return Plain(owner, text, heading, kind, choices, defaultIndex, cancelIndex, footnote, Remaining(timeout, asked.Elapsed), remember);
        }
        WaitForBrowserExit(env);
        return answer;
    }

    /// <summary>The least the fallback dialog is left up, whatever the first wait used.</summary>
    static readonly TimeSpan FallbackLeast = TimeSpan.FromSeconds(3);

    /// <summary>What is left of <paramref name="timeout"/> after the page's failed attempt took <paramref name="elapsed"/>: the
    /// fallback dialog gets that, not the whole timeout again (the uninstall question must be answered within the seconds
    /// Velopack allows the step; unanswered, it keeps the data). null stays null: no timeout.</summary>
    internal static TimeSpan? Remaining(TimeSpan? timeout, TimeSpan elapsed) =>
        timeout is { } total ? TimeSpan.FromTicks(Math.Max((total - elapsed).Ticks, FallbackLeast.Ticks)) : null;

    static bool _hasWebView2;

    static bool WebView2Installed()
    {
        if (_hasWebView2)
            return true;
        try
        {
            _hasWebView2 = !string.IsNullOrEmpty(CoreWebView2Environment.GetAvailableBrowserVersionString());
        }
        catch (Exception ex) when (ex is WebView2RuntimeNotFoundException or COMException or DllNotFoundException or BadImageFormatException)
        {
            _hasWebView2 = false;
        }
        return _hasWebView2;
    }

    /// <summary>
    /// Asked with no other window of the app open (the uninstaller's question, a failed start): its browser is let finish
    /// exiting, a moment at most, so what comes next (deleting the app's data) finds no file of it held open.
    /// </summary>
    static void WaitForBrowserExit(CoreWebView2Environment? env)
    {
        if (env is null || Application.OpenForms.Count > 0)
            return;
        bool exited = false;
        try
        {
            env.BrowserProcessExited += (_, _) => exited = true;
        }
        catch (Exception ex) when (ex is COMException or InvalidOperationException or NotImplementedException)
        {
            return;
        }
        var clock = System.Diagnostics.Stopwatch.StartNew();
        while (!exited && clock.ElapsedMilliseconds < 3000)
        {
            Application.DoEvents();
            Thread.Sleep(20);
        }
    }

    /// <summary>The same question in Windows' own task dialog, for when the page can't be shown.</summary>
    static (int Picked, bool Remember) Plain(IWin32Window? owner, string text, string? heading, Kind kind, IReadOnlyList<Choice> choices,
        int defaultIndex, int cancelIndex, string? footnote, TimeSpan? timeout, string? remember)
    {
        bool listed = choices.Any(c => c.Detail is not null);
        var page = new TaskDialogPage
        {
            Caption = Ui.Title,
            Heading = heading,
            Text = text,
            AllowCancel = true,
            SizeToContent = true,
            Icon = kind switch { Kind.Error => TaskDialogIcon.Error, Kind.Warning => TaskDialogIcon.Warning, _ => TaskDialogIcon.Information },
        };
        var buttons = choices.Select(c => listed ? new TaskDialogCommandLinkButton(c.Text, c.Detail) : new TaskDialogButton(c.Text)).ToList();
        foreach (var b in buttons)
            page.Buttons.Add(b);
        page.DefaultButton = buttons[Math.Clamp(defaultIndex, 0, buttons.Count - 1)];
        if (footnote is not null)
            page.Footnote = new TaskDialogFootnote(footnote);
        if (remember is not null)
            page.Verification = new TaskDialogVerificationCheckBox(remember);
        using var timer = new System.Windows.Forms.Timer();
        if (timeout is { } after)
        {
            timer.Interval = (int)Math.Clamp(after.TotalMilliseconds, 1000, int.MaxValue);
            timer.Tick += (_, _) =>
            {
                timer.Stop();
                page.BoundDialog?.Close();
            };
            page.Created += (_, _) => timer.Start();
        }
        var result = owner is null
            ? TaskDialog.ShowDialog(page, TaskDialogStartupLocation.CenterScreen)
            : TaskDialog.ShowDialog(owner, page, TaskDialogStartupLocation.CenterOwner);
        int picked = result is TaskDialogButton b2 ? buttons.IndexOf(b2) : -1;
        return (picked >= 0 ? picked : cancelIndex, picked >= 0 && page.Verification?.Checked == true);
    }
}
