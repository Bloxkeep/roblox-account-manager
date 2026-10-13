using System.Drawing.Drawing2D;
using System.Runtime.InteropServices;

namespace RobloxManager;

/// <summary>
/// The tray icon's right-click menu, drawn in the app's theme: rounded corners on Windows 11, roomy rows and a soft
/// highlight instead of the stock blue-bordered Windows menu. The colours are taken again each time it opens.
/// </summary>
sealed class TrayMenu : ContextMenuStrip
{
    readonly ToolStripMenuItem _open = new(), _quit = new();
    /// <summary>The launch presets (and their separator) currently at the top of the menu.</summary>
    readonly List<ToolStripItem> _presetItems = new();

    public event Action? OpenClicked;
    public event Action? QuitClicked;
    public event Action<Guid>? PresetClicked;

    /// <summary>Launch presets to list above Open/Quit, read each time the menu opens (empty while locked).</summary>
    [System.ComponentModel.DesignerSerializationVisibility(System.ComponentModel.DesignerSerializationVisibility.Hidden)]
    public Func<List<(Guid Id, string Name)>>? Presets { get; set; }

    public TrayMenu()
    {
        ShowImageMargin = false;
        ShowCheckMargin = false;
        DropShadowEnabled = true;
        Font = new Font("Segoe UI", 9.75f);
        Padding = new Padding(4, 5, 4, 5);
        _open.Font = new Font("Segoe UI Semibold", 9.75f);
        foreach (var item in new[] { _open, _quit })
            item.Padding = new Padding(10, 6, 24, 6);
        _open.Click += (_, _) => OpenClicked?.Invoke();
        _quit.Click += (_, _) => QuitClicked?.Invoke();
        Items.AddRange([_open, new ToolStripSeparator(), _quit]);
    }

    protected override void OnOpening(System.ComponentModel.CancelEventArgs e)
    {
        // The language and the theme can both change while the app runs.
        _open.Text = L.T("Open Bloxkeep");
        _quit.Text = L.T("Quit Bloxkeep");
        ShowPresets();
        var theme = Themes.Current;
        Renderer = new ThemedRenderer(theme, RoundCorners(theme));
        base.OnOpening(e);
    }

    /// <summary>Presets can be added, renamed or removed (or the vault locked) between two openings: rebuilt each time.</summary>
    void ShowPresets()
    {
        foreach (var item in _presetItems)
        {
            Items.Remove(item);
            item.Dispose();
        }
        _presetItems.Clear();
        foreach (var (id, name) in Presets?.Invoke() ?? [])
        {
            // "&" would underline the next letter instead of showing.
            string text = (name.Length > 40 ? name[..39] + "…" : name).Replace("&", "&&");
            var item = new ToolStripMenuItem(text) { Padding = new Padding(10, 6, 24, 6) };
            item.Click += (_, _) => PresetClicked?.Invoke(id);
            _presetItems.Add(item);
        }
        if (_presetItems.Count == 0)
            return;
        _presetItems.Add(new ToolStripSeparator());
        for (int i = 0; i < _presetItems.Count; i++)
            Items.Insert(i, _presetItems[i]);
    }

    [DllImport("dwmapi.dll")]
    static extern int DwmSetWindowAttribute(IntPtr hwnd, int attribute, ref int value, int size);

    /// <summary>Windows 11 rounds the popup and draws its border; false on older Windows, where we draw the border.</summary>
    bool RoundCorners(Themes.Theme theme)
    {
        const int DWMWA_WINDOW_CORNER_PREFERENCE = 33, DWMWA_BORDER_COLOR = 34, DWMWCP_ROUND = 2;
        int round = DWMWCP_ROUND;
        if (DwmSetWindowAttribute(Handle, DWMWA_WINDOW_CORNER_PREFERENCE, ref round, sizeof(int)) != 0)
            return false;
        int border = ColorTranslator.ToWin32(theme.Line);
        DwmSetWindowAttribute(Handle, DWMWA_BORDER_COLOR, ref border, sizeof(int));
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

    sealed class ThemedRenderer(Themes.Theme theme, bool systemBorder) : ToolStripRenderer
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
