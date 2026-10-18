using System.Drawing.Drawing2D;
using System.Runtime.InteropServices;

namespace RobloxManager;

/// <summary>
/// The tray icon's right-click menu (a left click opens the quick panel, <see cref="TrayPanel"/>), drawn in the app's
/// theme: rounded corners on Windows 11, an icon on every row, roomy rows and a soft highlight instead of the stock
/// Windows menu. Top to bottom: Bloxkeep's name with what is going on (Multi-Roblox, how many Roblox windows); opening
/// Bloxkeep and the mini window; what opens Roblox (an account, a preset, a window to sign in to) and Multi-Roblox,
/// switched in its submenu so a stray click can't turn it off mid-game; closing every Roblox; Quit. The colours and
/// words are taken again each time it opens.
/// </summary>
sealed class TrayMenu : ContextMenuStrip
{
    readonly HeaderItem _header = new();
    readonly RowItem _open = new(Drawn.Glyph.Open), _mini = new(Drawn.Glyph.Mini), _accounts = new(Drawn.Glyph.Play),
        _presets = new(Drawn.Glyph.Presets), _newRoblox = new(Drawn.Glyph.SignIn), _multi = new(Drawn.Glyph.Multi),
        _closeAll = new(Drawn.Glyph.Close), _quit = new(Drawn.Glyph.Quit);
    readonly ToolStripMenuItem _multiSwitch = new(), _multiFix = new();
    readonly ToolStripSeparator _playLine = new();
    readonly Font _bold = new("Segoe UI Semibold", 9.75f);

    public event Action? MiniClicked;
    public event Action? OpenClicked;
    public event Action? QuitClicked;

    /// <summary>What Bloxkeep has to offer now (<see cref="AppController.TrayState"/>), read each time the menu opens.</summary>
    [System.ComponentModel.DesignerSerializationVisibility(System.ComponentModel.DesignerSerializationVisibility.Hidden)]
    public Func<AppController.TrayInfo>? State { get; set; }
    /// <summary>Multi-Roblox's switch (in its submenu): off while it is on; on opens Bloxkeep, where the switch asks first.</summary>
    public event Action<bool>? MultiClicked;
    public event Action<Guid>? PresetClicked;
    public event Action? NewRobloxClicked;
    public event Action<Guid>? AccountClicked;
    public event Action? CloseAllClicked;
    /// <summary>When the menu last opened, Multi-Roblox's item could switch it from here (see <see cref="AppController.TrayMulti"/>).</summary>
    public bool CanSwitchMulti(bool on) => _multi.Tag is AppController.TrayInfo info && CanSwitch(info, on);

    internal static bool CanSwitch(AppController.TrayInfo info, bool on) => info.MultiOnly || (on && info.Open);

    /// <summary>Whether the mini window is open right now, for the check mark next to its item.</summary>
    [System.ComponentModel.DesignerSerializationVisibility(System.ComponentModel.DesignerSerializationVisibility.Hidden)]
    public Func<bool>? MiniOpen { get; set; }

    /// <summary>A row with an icon (its image, drawn from <see cref="Drawn.IconFont"/>) and, on its right, a short value ("On", "3").</summary>
    internal sealed class RowItem(string glyph) : ToolStripMenuItem
    {
        public string Glyph { get; } = glyph;
        [System.ComponentModel.DesignerSerializationVisibility(System.ComponentModel.DesignerSerializationVisibility.Hidden)]
        public string? Value { get; set; }
    }

    /// <summary>The menu's first row, not a command: the logo, "Bloxkeep" and a line on what is going on, with a coloured dot.</summary>
    internal sealed class HeaderItem : ToolStripMenuItem
    {
        [System.ComponentModel.DesignerSerializationVisibility(System.ComponentModel.DesignerSerializationVisibility.Hidden)]
        public string Status { get; set; } = "";
        [System.ComponentModel.DesignerSerializationVisibility(System.ComponentModel.DesignerSerializationVisibility.Hidden)]
        public Color Dot { get; set; }
        public override bool CanSelect => false;
    }

    /// <summary>The sidebar's yellow light: Multi-Roblox is on but not working yet.</summary>
    internal static readonly Color Amber = Color.FromArgb(245, 158, 11);

    static readonly Padding Row = new(8, 7, 24, 7);

    public TrayMenu()
    {
        ShowImageMargin = true;
        ShowCheckMargin = false;
        DropShadowEnabled = true;
        Font = new Font("Segoe UI", 9.75f);
        Padding = new Padding(5, 6, 5, 6);
        _open.Font = _bold;
        _header.Enabled = false;
        foreach (var item in new[] { _open, _mini, _accounts, _presets, _newRoblox, _multi, _closeAll, _quit, _multiSwitch, _multiFix })
            item.Padding = Row;
        _open.Click += (_, _) => OpenClicked?.Invoke();
        _mini.Click += (_, _) => MiniClicked?.Invoke();
        _quit.Click += (_, _) => QuitClicked?.Invoke();
        _newRoblox.Click += (_, _) => NewRobloxClicked?.Invoke();
        _multiSwitch.Click += (_, _) => MultiClicked?.Invoke(_multi.Checked);
        _multiFix.Click += (_, _) => OpenClicked?.Invoke();
        _multi.DropDownItems.AddRange([_multiFix, _multiSwitch]);
        // Closing every Roblox window is confirmed by its submenu's one item, not on the first click.
        var closeNow = new ToolStripMenuItem { Padding = Row };
        closeNow.Click += (_, _) => CloseAllClicked?.Invoke();
        _closeAll.DropDownItems.Add(closeNow);
        Items.AddRange([_header, new ToolStripSeparator(), _open, _mini, new ToolStripSeparator(), _accounts, _presets, _newRoblox, _multi,
            _playLine, _closeAll, _quit]);
    }

    int Px(int logical) => (int)Math.Round(logical * DeviceDpi / 96f);

    protected override void OnOpening(System.ComponentModel.CancelEventArgs e)
    {
        // The language, the theme and the screen's scale can all change while the app runs.
        var theme = Themes.Current;
        var state = State?.Invoke();
        ImageScalingSize = new Size(Px(16), Px(16));

        _open.Text = L.T("Open Bloxkeep");
        _mini.Text = L.T("Mini window");
        _mini.Checked = MiniOpen?.Invoke() == true;
        _quit.Text = L.T("Quit Bloxkeep");
        _newRoblox.Text = L.T("New Roblox window to sign in");
        _newRoblox.ToolTipText = L.T("Sign in separately in each new window. Keep Bloxkeep running; sign-ins are not saved when these windows close.");
        _newRoblox.Visible = _multi.Visible = state is not null;

        // What is going on, under the name: Multi-Roblox and how many Roblox windows run.
        string multiShort = state?.Multi switch { "ready" => L.T("Multi-Roblox on"), "notReady" => L.T("Multi-Roblox not ready"), _ => L.T("Multi-Roblox off") };
        _header.Status = state is null ? "" : state is { Open: false, MultiOnly: false } ? $"{L.T("Locked")} · {multiShort}"
            : state.RobloxCount > 0 ? $"{multiShort} · {L.P(state.RobloxCount, "1 Roblox window", "{0} Roblox windows", state.RobloxCount)}" : multiShort;
        _header.Dot = state?.Multi switch { "ready" => theme.Good, "notReady" => Amber, _ => theme.Text2 };

        if (state is not null)
        {
            bool on = state.Multi != "off";
            _multi.Text = "Multi-Roblox";
            _multi.Value = L.T(state.Multi switch { "ready" => "On", "notReady" => "Not ready", _ => "Off" });
            _multi.Checked = on; // what its switch acts on; the value says it, not a tick
            _multi.Tag = state;
            _multiFix.Visible = state.Multi == "notReady" && state.Open && !state.MultiOnly;
            _multiFix.Text = L.T("Open Bloxkeep to fix it");
            _multiSwitch.Text = CanSwitch(state, on) ? L.T(on ? "Turn off Multi-Roblox" : "Turn on Multi-Roblox")
                : !state.Open ? L.T("Unlock Bloxkeep to change it") : L.T("Open Bloxkeep to turn it on");
        }

        _accounts.Visible = state is { Open: true, Accounts.Count: > 0 };
        _accounts.Text = L.T("Play as");
        _accounts.DropDownItems.Clear();
        foreach (var (id, name, running) in state?.Accounts ?? [])
        {
            // One already open is ticked, not opened a second time.
            var item = new ToolStripMenuItem(name.Replace("&", "&&")) { Padding = Row, Checked = running, Enabled = !running,
                Image = Drawn.AvatarBitmap(state!.PictureOf?.Invoke(id), name, Px(20)) };
            item.Click += (_, _) => AccountClicked?.Invoke(id);
            _accounts.DropDownItems.Add(item);
        }
        _presets.Visible = state is { Open: true, Presets.Count: > 0 };
        _presets.Text = L.T("Run preset");
        _presets.DropDownItems.Clear();
        foreach (var (id, name) in state?.Presets ?? [])
        {
            var item = new ToolStripMenuItem(name.Replace("&", "&&")) { Padding = Row };
            item.Click += (_, _) => PresetClicked?.Invoke(id);
            _presets.DropDownItems.Add(item);
        }
        _closeAll.Visible = _playLine.Visible = state is { RobloxCount: > 0 } && (state.Open || state.MultiOnly);
        _closeAll.Text = L.T("Close all Roblox");
        if (state is { RobloxCount: > 0 })
        {
            _closeAll.Value = state.RobloxCount.ToString();
            _closeAll.DropDownItems[0].Text = state.RobloxCount == 1 ? L.T("Close 1 Roblox window") : L.F("Close {0} Roblox windows", state.RobloxCount);
        }

        // Icons in the theme's second text colour. A row's value goes in the menu's shortcut column, which Windows
        // makes room for (it draws it only on rows without a submenu: the renderer draws the rest).
        foreach (var row in new[] { _open, _mini, _accounts, _presets, _newRoblox, _multi, _closeAll, _quit })
        {
            row.Image?.Dispose();
            row.Image = Drawn.IconBitmap(row.Glyph, Px(16), theme.Text2);
            row.ShortcutKeyDisplayString = row.Value is { Length: > 0 } ? row.Value : null;
            row.ShowShortcutKeys = true;
        }
        // The header's own text is its status line and its "shortcut" a few wide spaces, so the menu makes room for
        // the line as the renderer draws it: after the logo and a dot, on two lines (the top and bottom padding).
        _header.Text = _header.Status.Length > 0 ? _header.Status : "Bloxkeep";
        _header.ShortcutKeyDisplayString = "   ";
        _header.ShowShortcutKeys = true;
        _header.Padding = new Padding(Px(8), Px(17), Px(8), Px(17));

        Renderer = new ThemedRenderer(theme, RoundCorners(Handle, theme), _bold);
        foreach (var sub in new[] { _multi.DropDown, _accounts.DropDown, _presets.DropDown, _closeAll.DropDown })
        {
            sub.Renderer = new ThemedRenderer(theme, RoundCorners(sub.Handle, theme), _bold);
            sub.Font = Font;
            sub.Padding = Padding;
            if (sub is ToolStripDropDownMenu menu)
            {
                menu.ShowImageMargin = sub == _accounts.DropDown; // the accounts' pictures
                menu.ShowCheckMargin = false;
                menu.ImageScalingSize = new Size(Px(20), Px(20));
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
            _bold.Dispose();
            Font.Dispose();
        }
        base.Dispose(disposing);
    }

    /// <summary>Shared with the game monitor tab's tile menu, so every native menu looks the same.</summary>
    internal sealed class ThemedRenderer(Themes.Theme theme, bool systemBorder, Font? bold = null) : ToolStripRenderer
    {
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

        // The icons sit on the menu itself: no separate strip behind them.
        protected override void OnRenderImageMargin(ToolStripRenderEventArgs e) { }

        // A checked row's tick is drawn on its right (below), not over its icon.
        protected override void OnRenderItemCheck(ToolStripItemImageRenderEventArgs e) { }

        protected override void OnRenderMenuItemBackground(ToolStripItemRenderEventArgs e)
        {
            float scale = e.ToolStrip!.DeviceDpi / 96f;
            var g = e.Graphics;
            if (e.Item.Selected && e.Item.Enabled)
                Drawn.FillRound(g, new RectangleF(2 * scale, 0, e.Item.Width - 4 * scale, e.Item.Height), 6 * scale, Drawn.Hover(theme));
            // A row with a submenu: its value where the shortcut column is (Windows draws it only on rows without one).
            if (e.Item is RowItem { Value: { Length: > 0 } value, HasDropDownItems: true })
            {
                int right = e.Item.Width - (int)(34 * scale);
                TextRenderer.DrawText(g, value, e.Item.Font, new Rectangle(0, 0, right, e.Item.Height), theme.Text2,
                    TextFormatFlags.Right | TextFormatFlags.VerticalCenter | TextFormatFlags.NoPrefix | TextFormatFlags.SingleLine);
            }
            // No check margin (it would indent every item): a checked item gets a small tick in its right padding.
            if (e.Item is ToolStripMenuItem { Checked: true } and not RowItem { Glyph: Drawn.Glyph.Multi })
            {
                float x = e.Item.Width - 20 * scale, y = e.Item.Height / 2f;
                using var pen = new Pen(e.Item.Enabled ? theme.Text : Drawn.Accent(theme), 1.6f * scale) { StartCap = LineCap.Round, EndCap = LineCap.Round, LineJoin = LineJoin.Round };
                g.SmoothingMode = SmoothingMode.AntiAlias;
                g.DrawLines(pen, [new PointF(x, y), new PointF(x + 3 * scale, y + 3 * scale), new PointF(x + 9 * scale, y - 4 * scale)]);
            }
        }

        protected override void OnRenderItemImage(ToolStripItemImageRenderEventArgs e)
        {
            if (e.Image is null)
                return;
            // Drawn as it is, not greyed: an account already playing shows its picture with a tick.
            e.Graphics.InterpolationMode = InterpolationMode.HighQualityBicubic;
            e.Graphics.DrawImage(e.Image, e.ImageRectangle);
        }

        protected override void OnRenderItemText(ToolStripItemTextRenderEventArgs e)
        {
            float scale = e.ToolStrip!.DeviceDpi / 96f;
            var g = e.Graphics;
            if (e.Item is HeaderItem header)
            {
                int pad = (int)(12 * scale), logo = (int)(30 * scale);
                Drawn.Logo(g, new Rectangle(pad, (header.Height - logo) / 2, logo, logo));
                int x = pad + logo + (int)(11 * scale), lineH = TextRenderer.MeasureText("Ag", e.TextFont).Height;
                int y = (header.Height - lineH * 2) / 2;
                TextRenderer.DrawText(g, "Bloxkeep", bold ?? e.TextFont, new Point(x, y), theme.Text, TextFormatFlags.NoPrefix | TextFormatFlags.NoPadding);
                float d = 7 * scale;
                using (var dot = new SolidBrush(header.Dot))
                {
                    g.SmoothingMode = SmoothingMode.AntiAlias;
                    g.FillEllipse(dot, x, y + lineH + (lineH - d) / 2f, d, d);
                }
                TextRenderer.DrawText(g, header.Status, e.TextFont, new Point(x + (int)(d + 6 * scale), y + lineH), theme.Text2,
                    TextFormatFlags.NoPrefix | TextFormatFlags.NoPadding);
                return;
            }
            // A row's value (in the shortcut column) in the second text colour, like a label's value.
            bool value = e.Item is ToolStripMenuItem { ShortcutKeyDisplayString: { Length: > 0 } shortcut } && e.Text == shortcut;
            e.TextColor = e.Item.Enabled && !value ? theme.Text : theme.Text2;
            base.OnRenderItemText(e);
        }

        // A submenu's arrow in the theme's second text colour: the stock one is near black, lost on a dark menu.
        protected override void OnRenderArrow(ToolStripArrowRenderEventArgs e)
        {
            e.ArrowColor = e.Item is { Enabled: false } ? theme.Line : theme.Text2;
            base.OnRenderArrow(e);
        }

        protected override void OnRenderSeparator(ToolStripSeparatorRenderEventArgs e)
        {
            float scale = e.ToolStrip!.DeviceDpi / 96f;
            int y = e.Item.Height / 2;
            using var pen = new Pen(theme.Line);
            e.Graphics.DrawLine(pen, 10 * scale, y, e.Item.Width - 10 * scale, y);
        }
    }
}
