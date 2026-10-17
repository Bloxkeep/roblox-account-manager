using System.Drawing.Drawing2D;
using System.Runtime.InteropServices;

namespace RobloxManager;

/// <summary>
/// The tray icon's right-click menu, drawn in the app's theme: rounded corners on Windows 11, roomy rows and a soft
/// highlight instead of the stock blue-bordered Windows menu. The colours are taken again each time it opens.
/// </summary>
sealed class TrayMenu : ContextMenuStrip
{
    readonly ToolStripMenuItem _mini = new(), _open = new(), _quit = new(), _multi = new(), _newRoblox = new(), _accounts = new(),
        _presets = new(), _closeAll = new();
    readonly ToolStripSeparator _multiLine = new(), _toolsLine = new();

    public event Action? MiniClicked;
    public event Action? OpenClicked;
    public event Action? QuitClicked;

    /// <summary>What Bloxkeep has to offer now (<see cref="AppController.TrayState"/>), read each time the menu opens.</summary>
    [System.ComponentModel.DesignerSerializationVisibility(System.ComponentModel.DesignerSerializationVisibility.Hidden)]
    public Func<AppController.TrayInfo>? State { get; set; }
    /// <summary>Multi-Roblox's item: off while it is on; on opens Bloxkeep, where the switch asks first.</summary>
    public event Action<bool>? MultiClicked;
    public event Action<Guid>? PresetClicked;
    public event Action? NewRobloxClicked;
    public event Action<Guid>? AccountClicked;
    public event Action? CloseAllClicked;
    /// <summary>When the menu last opened, Multi-Roblox's item could switch it from here (see <see cref="AppController.TrayMulti"/>).</summary>
    public bool CanSwitchMulti(bool on) => _multi.Tag is AppController.TrayInfo info && (info.MultiOnly || (on && info.Open));

    /// <summary>Whether the mini window is open right now, for the check mark next to its item.</summary>
    [System.ComponentModel.DesignerSerializationVisibility(System.ComponentModel.DesignerSerializationVisibility.Hidden)]
    public Func<bool>? MiniOpen { get; set; }

    public TrayMenu()
    {
        ShowImageMargin = false;
        ShowCheckMargin = false;
        DropShadowEnabled = true;
        Font = new Font("Segoe UI", 9.75f);
        Padding = new Padding(4, 5, 4, 5);
        _open.Font = new Font("Segoe UI Semibold", 9.75f);
        foreach (var item in new[] { _mini, _open, _quit, _multi, _newRoblox, _accounts, _presets, _closeAll })
            item.Padding = new Padding(10, 6, 24, 6);
        _mini.Click += (_, _) => MiniClicked?.Invoke();
        _open.Click += (_, _) => OpenClicked?.Invoke();
        _quit.Click += (_, _) => QuitClicked?.Invoke();
        _multi.Click += (_, _) => MultiClicked?.Invoke(_multi.Checked);
        _newRoblox.Click += (_, _) => NewRobloxClicked?.Invoke();
        // Closing every Roblox window is confirmed by its submenu's one item, not on the first click.
        var closeNow = new ToolStripMenuItem { Padding = new Padding(10, 6, 24, 6) };
        closeNow.Click += (_, _) => CloseAllClicked?.Invoke();
        _closeAll.DropDownItems.Add(closeNow);
        Items.AddRange([_multi, _multiLine, _newRoblox, _accounts, _presets, _closeAll, _toolsLine, _mini, _open, new ToolStripSeparator(), _quit]);
    }

    protected override void OnOpening(System.ComponentModel.CancelEventArgs e)
    {
        // The language and the theme can both change while the app runs.
        _mini.Text = L.T("Mini window");
        _mini.Checked = MiniOpen?.Invoke() == true;
        _open.Text = L.T("Open Bloxkeep");
        _quit.Text = L.T("Quit Bloxkeep");
        var state = State?.Invoke();
        _multi.Visible = _multiLine.Visible = _newRoblox.Visible = _toolsLine.Visible = state is not null;
        _newRoblox.Text = L.T("Open Roblox");
        _newRoblox.ToolTipText = L.T("Sign in separately in each new window. Keep Bloxkeep running; sign-ins are not saved when these windows close.");
        if (state is not null)
        {
            _multi.Text = L.T(state.Multi switch { "ready" => "Multi-Roblox: on", "notReady" => "Multi-Roblox: not ready", _ => "Multi-Roblox: off" });
            _multi.Checked = state.Multi != "off";
            _multi.Tag = state;
        }
        _accounts.Visible = state is { Open: true, Accounts.Count: > 0 };
        _accounts.Text = L.T("Open account");
        _accounts.DropDownItems.Clear();
        foreach (var (id, name, running) in state?.Accounts ?? [])
        {
            // One already open is ticked, not opened a second time.
            var item = new ToolStripMenuItem(name.Replace("&", "&&")) { Padding = new Padding(10, 6, 24, 6), Checked = running, Enabled = !running };
            item.Click += (_, _) => AccountClicked?.Invoke(id);
            _accounts.DropDownItems.Add(item);
        }
        _presets.Visible = state is { Open: true, Presets.Count: > 0 };
        _presets.Text = L.T("Run preset");
        _presets.DropDownItems.Clear();
        foreach (var (id, name) in state?.Presets ?? [])
        {
            var item = new ToolStripMenuItem(name.Replace("&", "&&")) { Padding = new Padding(10, 6, 24, 6) };
            item.Click += (_, _) => PresetClicked?.Invoke(id);
            _presets.DropDownItems.Add(item);
        }
        _closeAll.Visible = state is { RobloxCount: > 0 } && (state.Open || state.MultiOnly);
        _closeAll.Text = L.T("Close all Roblox");
        if (state is { RobloxCount: > 0 })
            _closeAll.DropDownItems[0].Text = state.RobloxCount == 1 ? L.T("Close 1 Roblox window") : L.F("Close {0} Roblox windows", state.RobloxCount);
        var theme = Themes.Current;
        var renderer = new ThemedRenderer(theme, RoundCorners(Handle, theme));
        Renderer = renderer;
        foreach (var sub in new[] { _accounts.DropDown, _presets.DropDown, _closeAll.DropDown })
        {
            sub.Renderer = new ThemedRenderer(theme, RoundCorners(sub.Handle, theme));
            sub.Font = Font;
            if (sub is ToolStripDropDownMenu menu)
            {
                menu.ShowImageMargin = false;
                menu.ShowCheckMargin = false;
            }
        }
        base.OnOpening(e);
    }

    [DllImport("dwmapi.dll")]
    static extern int DwmSetWindowAttribute(IntPtr hwnd, int attribute, ref int value, int size);

    /// <summary>Windows 11 rounds the popup and draws its border; false on older Windows, where we draw the border.</summary>
    internal static bool RoundCorners(IntPtr hwnd, Themes.Theme theme)
    {
        const int DWMWA_WINDOW_CORNER_PREFERENCE = 33, DWMWA_BORDER_COLOR = 34, DWMWCP_ROUND = 2;
        int round = DWMWCP_ROUND;
        if (DwmSetWindowAttribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, ref round, sizeof(int)) != 0)
            return false;
        int border = ColorTranslator.ToWin32(theme.Line);
        DwmSetWindowAttribute(hwnd, DWMWA_BORDER_COLOR, ref border, sizeof(int));
        return true;
    }

    protected override void Dispose(bool disposing)
    {
        if (disposing)
        {
            _open.Font.Dispose();
            Font.Dispose();
        }
        base.Dispose(disposing);
    }

    /// <summary>Shared with the game monitor tab's tile menu, so every native menu looks the same.</summary>
    internal sealed class ThemedRenderer(Themes.Theme theme, bool systemBorder) : ToolStripRenderer
    {
        static Color Mix(Color a, Color b, float amount) => Color.FromArgb(
            (int)(a.R + (b.R - a.R) * amount), (int)(a.G + (b.G - a.G) * amount), (int)(a.B + (b.B - a.B) * amount));

        protected override void OnRenderToolStripBackground(ToolStripRenderEventArgs e)
        {
            using var brush = new SolidBrush(theme.Surface);
            e.Graphics.FillRectangle(brush, e.AffectedBounds);
        }

        protected override void OnRenderToolStripBorder(ToolStripRenderEventArgs e)
        {
            if (systemBorder)
                return;
            using var pen = new Pen(theme.Line);
            var r = e.AffectedBounds;
            e.Graphics.DrawRectangle(pen, r.X, r.Y, r.Width - 1, r.Height - 1);
        }

        protected override void OnRenderMenuItemBackground(ToolStripItemRenderEventArgs e)
        {
            if (!e.Item.Selected || !e.Item.Enabled)
                return;
            float scale = e.ToolStrip!.DeviceDpi / 96f;
            var r = new RectangleF(2 * scale, 0, e.Item.Width - 4 * scale, e.Item.Height);
            float radius = 6 * scale;
            using var path = new GraphicsPath();
            path.AddArc(r.X, r.Y, radius * 2, radius * 2, 180, 90);
            path.AddArc(r.Right - radius * 2, r.Y, radius * 2, radius * 2, 270, 90);
            path.AddArc(r.Right - radius * 2, r.Bottom - radius * 2, radius * 2, radius * 2, 0, 90);
            path.AddArc(r.X, r.Bottom - radius * 2, radius * 2, radius * 2, 90, 90);
            path.CloseFigure();
            e.Graphics.SmoothingMode = SmoothingMode.AntiAlias;
            using var brush = new SolidBrush(Mix(theme.Surface, theme.Text, theme.Dark ? .1f : .07f));
            e.Graphics.FillPath(brush, path);
        }

        protected override void OnRenderItemText(ToolStripItemTextRenderEventArgs e)
        {
            e.TextColor = e.Item.Enabled ? theme.Text : theme.Text2;
            base.OnRenderItemText(e);
            // No check margin (it would indent every item): a checked item gets a small tick in its right padding.
            if (e.Item is ToolStripMenuItem { Checked: true })
            {
                float scale = e.ToolStrip!.DeviceDpi / 96f;
                float x = e.Item.Width - 18 * scale, y = e.Item.Height / 2f;
                using var pen = new Pen(theme.Text, 1.6f * scale) { StartCap = LineCap.Round, EndCap = LineCap.Round, LineJoin = LineJoin.Round };
                e.Graphics.SmoothingMode = SmoothingMode.AntiAlias;
                e.Graphics.DrawLines(pen, [new PointF(x, y), new PointF(x + 3 * scale, y + 3 * scale), new PointF(x + 9 * scale, y - 4 * scale)]);
            }
        }

        protected override void OnRenderSeparator(ToolStripSeparatorRenderEventArgs e)
        {
            float scale = e.ToolStrip!.DeviceDpi / 96f;
            int y = e.Item.Height / 2;
            using var pen = new Pen(theme.Line);
            e.Graphics.DrawLine(pen, 8 * scale, y, e.Item.Width - 8 * scale, y);
        }
    }
}
