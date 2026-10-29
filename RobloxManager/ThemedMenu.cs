using System.Drawing.Drawing2D;
using System.Runtime.InteropServices;

namespace RobloxManager;

/// <summary>A native right-click menu in the app's theme, with Windows 11's rounded corners (the game monitor's tile menu,
/// <see cref="MonitorTab"/>).</summary>
static class ThemedMenu
{
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

    internal sealed class Renderer(Themes.Theme theme, bool systemBorder) : ToolStripRenderer
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
            // No check margin (it would indent every item): a checked item gets a small tick in its right padding.
            if (e.Item is ToolStripMenuItem { Checked: true })
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
            e.Graphics.InterpolationMode = InterpolationMode.HighQualityBicubic;
            e.Graphics.DrawImage(e.Image, e.ImageRectangle);
        }

        protected override void OnRenderItemText(ToolStripItemTextRenderEventArgs e)
        {
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
