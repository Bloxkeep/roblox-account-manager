using System.Drawing.Drawing2D;

namespace RobloxManager;

/// <summary>
/// The app's own message box, in the current theme: Windows' own (a white box with old-style buttons) looked out of
/// place next to the page. An optional heading, the text, and buttons: side by side, or one under the other with a
/// line each when they are choices (<see cref="Choice.Detail"/>). Enter presses the default one; Esc, the title bar's
/// X and the timeout pick the cancel one (the last, unless said otherwise). <see cref="Ui.Modal"/> shows it above the always-on-top windows.
/// </summary>
sealed class Dialog : Form
{
    public enum Kind { Info, Warning, Error }

    /// <summary>A button: its text, an optional line under it (a choice), and whether it destroys something.</summary>
    public sealed record Choice(string Text, string? Detail = null, bool Danger = false);

    int _picked;
    readonly int _cancel;
    readonly float _k;

    /// <summary>"Don't ask again", when the question offers it.</summary>
    readonly DialogCheck? _remember;

    Dialog(string text, string? heading, Kind kind, IReadOnlyList<Choice> choices, int defaultIndex, int cancelIndex, string? footnote, bool owned,
        string? remember = null)
    {
        var theme = Themes.Current;
        _cancel = cancelIndex;
        _picked = _cancel;
        _k = DeviceDpi / 96f;
        Text = Ui.Title;
        Font = new Font("Segoe UI", 9.75f);
        AutoScaleMode = AutoScaleMode.None;
        FormBorderStyle = FormBorderStyle.FixedDialog;
        MinimizeBox = MaximizeBox = false;
        ShowIcon = false;
        ShowInTaskbar = !owned;
        StartPosition = owned ? FormStartPosition.CenterParent : FormStartPosition.CenterScreen;
        BackColor = theme.Background;
        ForeColor = theme.Text;
        KeyPreview = true;

        int width = S(460), pad = S(24), y = pad;
        int textLeft = pad + S(44), textWidth = width - textLeft - pad;
        var mark = new PictureBox { Image = Mark(S(32), kind, theme), Bounds = new Rectangle(pad, y, S(32), S(32)) };
        Controls.Add(mark);
        if (heading is not null)
        {
            var h = Label(heading, new Font("Segoe UI Semibold", 12f), theme.Text, textLeft, y + S(4), textWidth);
            y = h.Bottom + S(8);
        }
        else
            y += S(6);
        var body = Label(text, Font, heading is null ? theme.Text : theme.Text2, textLeft, y, textWidth);
        y = Math.Max(body.Bottom, mark.Bottom) + S(20);
        if (remember is not null)
        {
            _remember = new DialogCheck(theme, remember, Font, S(16), S(7)) { TabIndex = choices.Count };
            _remember.Location = new Point(textLeft, y - S(8));
            Controls.Add(_remember);
            y = _remember.Bottom + S(14);
        }

        bool listed = choices.Any(c => c.Detail is not null);
        // Filled: a destructive button (red), else the first one; which one Enter presses is defaultIndex's to say.
        bool anyDanger = choices.Any(c => c.Danger);
        var buttons = new List<DialogButton>();
        for (int i = 0; i < choices.Count; i++)
        {
            var c = choices[i];
            int index = i;
            bool filled = !listed && (anyDanger ? c.Danger : i == 0);
            var b = new DialogButton(theme, c, filled, listed) { TabIndex = i };
            b.Click += (_, _) =>
            {
                _picked = index;
                Close();
            };
            buttons.Add(b);
            Controls.Add(b);
        }
        if (listed)
        {
            foreach (var b in buttons)
            {
                b.Bounds = new Rectangle(textLeft - S(8), y, textWidth + S(8), b.PreferredHeight(textWidth + S(8)));
                y = b.Bottom + S(8);
            }
        }
        else
        {
            int right = width - pad;
            // Windows' order: the main button first (left of Cancel).
            for (int i = buttons.Count - 1; i >= 0; i--)
            {
                var b = buttons[i];
                int w = Math.Max(S(96), TextRenderer.MeasureText(b.Text, b.Font).Width + S(32));
                b.Bounds = new Rectangle(right - w, y, w, S(34));
                right -= w + S(10);
            }
            y += S(34) + S(8);
        }
        if (footnote is not null)
        {
            var f = Label(footnote, new Font("Segoe UI", 8.75f), theme.Text2, textLeft, y + S(4), textWidth);
            y = f.Bottom;
        }
        ClientSize = new Size(width, y + pad - S(8));
        Shown += (_, _) => buttons[Math.Clamp(defaultIndex, 0, buttons.Count - 1)].Focus();
    }

    int S(float px) => (int)Math.Round(px * _k);

    Label Label(string text, Font font, Color color, int x, int y, int width)
    {
        var label = new Label { Text = text, Font = font, ForeColor = color, AutoSize = true, MaximumSize = new Size(width, 0), Location = new Point(x, y) };
        Controls.Add(label);
        return label;
    }

    protected override void OnHandleCreated(EventArgs e)
    {
        base.OnHandleCreated(e);
        Ui.UseThemedTitleBar(this, Themes.Current);
    }

    protected override void OnKeyDown(KeyEventArgs e)
    {
        if (e.KeyCode == Keys.Escape)
        {
            _picked = _cancel;
            Close();
        }
        base.OnKeyDown(e);
    }

    /// <returns>The index of the button clicked; <paramref name="cancelIndex"/> (the last by default) for Esc, the X or the timeout.</returns>
    public static int Show(IWin32Window? owner, string text, string? heading, Kind kind, IReadOnlyList<Choice> choices,
        int defaultIndex = 0, int? cancelIndex = null, string? footnote = null, TimeSpan? timeout = null)
    {
        using var dialog = new Dialog(text, heading, kind, choices, defaultIndex, cancelIndex ?? choices.Count - 1, footnote, owner is not null);
        using var timer = new System.Windows.Forms.Timer();
        if (timeout is { } after)
        {
            timer.Interval = (int)Math.Clamp(after.TotalMilliseconds, 1000, int.MaxValue);
            timer.Tick += (_, _) =>
            {
                timer.Stop();
                dialog._picked = dialog._cancel;
                dialog.Close();
            };
            dialog.Shown += (_, _) => timer.Start();
        }
        dialog.ShowDialog(owner);
        return dialog._picked;
    }

    public static void Error(IWin32Window? owner, string text) =>
        Show(owner, text, null, Kind.Error, [new Choice(L.T("OK"))]);

    /// <summary>
    /// <see cref="Confirm"/> with a "Don't ask again" box: Remember is whether it was ticked, and only with a yes.
    /// </summary>
    public static (bool Yes, bool Remember) ConfirmRemember(IWin32Window? owner, string text, string yes, string no, bool danger = false)
    {
        using var dialog = new Dialog(text, null, danger ? Kind.Warning : Kind.Info, [new Choice(yes, Danger: danger), new Choice(no)],
            danger ? 1 : 0, 1, null, owner is not null, L.T("Don't ask again"));
        dialog.ShowDialog(owner);
        bool picked = dialog._picked == 0;
        return (picked, picked && dialog._remember?.Checked == true);
    }

    /// <summary>Yes or no; with <paramref name="danger"/> the "yes" is red and Enter means "no".</summary>
    public static bool Confirm(IWin32Window? owner, string text, string yes, string no, bool danger = false) =>
        Show(owner, text, null, danger ? Kind.Warning : Kind.Info, [new Choice(yes, Danger: danger), new Choice(no)], defaultIndex: danger ? 1 : 0) == 0;

    /// <summary>A round mark: "i", "!" or "×" in the colour of its kind.</summary>
    static Bitmap Mark(int size, Kind kind, Themes.Theme theme)
    {
        var bmp = new Bitmap(size, size);
        using var g = Graphics.FromImage(bmp);
        g.Clear(theme.Background);
        g.SmoothingMode = SmoothingMode.AntiAlias;
        Color c = kind switch { Kind.Error => theme.Bad, Kind.Warning => Color.FromArgb(232, 168, 46), _ => theme.Good };
        using (var fill = new SolidBrush(Color.FromArgb(theme.Dark ? 40 : 30, c)))
            g.FillEllipse(fill, 1, 1, size - 3, size - 3);
        using var pen = new Pen(c, size * .08f) { StartCap = LineCap.Round, EndCap = LineCap.Round };
        g.DrawEllipse(pen, size * .07f, size * .07f, size * .84f, size * .84f);
        float m = size / 2f - .5f;
        if (kind == Kind.Error)
        {
            g.DrawLine(pen, size * .36f, size * .36f, size * .62f, size * .62f);
            g.DrawLine(pen, size * .62f, size * .36f, size * .36f, size * .62f);
        }
        else if (kind == Kind.Warning)
        {
            g.DrawLine(pen, m, size * .29f, m, size * .55f);
            g.DrawLine(pen, m, size * .70f, m, size * .71f);
        }
        else
        {
            g.DrawLine(pen, m, size * .30f, m, size * .31f);
            g.DrawLine(pen, m, size * .45f, m, size * .70f);
        }
        return bmp;
    }

    /// <summary>A rounded button in the theme's colours; as a choice (with a line under it), a wide one.</summary>
    sealed class DialogButton : Control, IButtonControl
    {
        readonly Themes.Theme _theme;
        readonly Choice _choice;
        readonly bool _primary, _listed;
        bool _hover, _down;
        [System.ComponentModel.DesignerSerializationVisibility(System.ComponentModel.DesignerSerializationVisibility.Hidden)]
        public DialogResult DialogResult { get; set; }

        public DialogButton(Themes.Theme theme, Choice choice, bool primary, bool listed)
        {
            SetStyle(ControlStyles.AllPaintingInWmPaint | ControlStyles.OptimizedDoubleBuffer | ControlStyles.UserPaint
                | ControlStyles.ResizeRedraw | ControlStyles.Selectable | ControlStyles.StandardClick, true);
            _theme = theme;
            _choice = choice;
            _primary = primary;
            _listed = listed;
            Text = choice.Text;
            Font = new Font("Segoe UI Semibold", 9.75f);
            Cursor = Cursors.Hand;
            TabStop = true;
        }

        /// <summary>The height for the button at this width: measured as it is drawn (see OnPaint).</summary>
        public int PreferredHeight(int width)
        {
            int h = TextRenderer.MeasureText(Text, Font).Height + LogicalToDeviceUnits(22);
            if (_choice.Detail is { } detail)
                h += TextRenderer.MeasureText(detail, _detailFont, new Size(width - 2 * LogicalToDeviceUnits(14), 0), DetailFlags).Height + LogicalToDeviceUnits(2);
            return h;
        }

        /// <summary>Wrapped like a text box: a word too long for the line (a folder path) breaks too, instead of running off the edge.</summary>
        const TextFormatFlags DetailFlags = TextFormatFlags.Left | TextFormatFlags.WordBreak | TextFormatFlags.TextBoxControl;

        readonly Font _detailFont = new("Segoe UI", 9f);

        protected override void Dispose(bool disposing)
        {
            if (disposing)
                _detailFont.Dispose();
            base.Dispose(disposing);
        }

        public void NotifyDefault(bool value) { }

        public void PerformClick()
        {
            if (Enabled && Visible)
                OnClick(EventArgs.Empty);
        }

        protected override bool IsInputKey(Keys key) => key is Keys.Enter or Keys.Space || base.IsInputKey(key);

        protected override void OnKeyDown(KeyEventArgs e)
        {
            if (e.KeyCode is Keys.Enter or Keys.Space)
            {
                e.Handled = true;
                PerformClick();
            }
            base.OnKeyDown(e);
        }

        protected override void OnMouseEnter(EventArgs e) { _hover = true; Invalidate(); base.OnMouseEnter(e); }
        protected override void OnMouseLeave(EventArgs e) { _hover = _down = false; Invalidate(); base.OnMouseLeave(e); }
        protected override void OnMouseDown(MouseEventArgs e) { _down = true; Invalidate(); base.OnMouseDown(e); }
        protected override void OnMouseUp(MouseEventArgs e) { _down = false; Invalidate(); base.OnMouseUp(e); }
        protected override void OnGotFocus(EventArgs e) { Invalidate(); base.OnGotFocus(e); }
        protected override void OnLostFocus(EventArgs e) { Invalidate(); base.OnLostFocus(e); }

        protected override void OnPaint(PaintEventArgs e)
        {
            var g = e.Graphics;
            g.Clear(_theme.Background);
            g.SmoothingMode = SmoothingMode.AntiAlias;
            Color fill = _primary ? (_choice.Danger ? _theme.Bad : _theme.Good) : _theme.Surface;
            Color ink = _primary ? InkOn(fill) : _choice.Danger && _listed ? _theme.Bad : _theme.Text;
            if (_hover || _down)
                fill = Mix(fill, _primary ? Color.White : _theme.Text, _down ? .14f : .08f);
            float radius = LogicalToDeviceUnits(7);
            var rect = new RectangleF(.5f, .5f, Width - 1.5f, Height - 1.5f);
            using (var path = Round(rect, radius))
            {
                using (var brush = new SolidBrush(fill))
                    g.FillPath(brush, path);
                if (!_primary)
                    using (var line = new Pen(_theme.Line))
                        g.DrawPath(line, path);
                if (Focused && ShowFocusCues)
                    using (var ring = new Pen(Mix(ink, fill, .35f), 1.5f))
                    using (var inner = Round(new RectangleF(2, 2, Width - 4.5f, Height - 4.5f), radius - 1))
                        g.DrawPath(ring, inner);
            }
            if (!_listed)
            {
                TextRenderer.DrawText(g, Text, Font, ClientRectangle, ink, fill, TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter | TextFormatFlags.SingleLine);
                return;
            }
            int x = LogicalToDeviceUnits(14), top = LogicalToDeviceUnits(10);
            var title = new Rectangle(x, top, Width - 2 * x, TextRenderer.MeasureText(Text, Font).Height);
            TextRenderer.DrawText(g, Text, Font, title, ink, fill, TextFormatFlags.Left | TextFormatFlags.SingleLine | TextFormatFlags.EndEllipsis);
            if (_choice.Detail is { } detail)
            {
                var under = new Rectangle(x, title.Bottom + LogicalToDeviceUnits(2), Width - 2 * x, Height - title.Bottom);
                TextRenderer.DrawText(g, detail, _detailFont, under, _theme.Text2, fill, DetailFlags);
            }
        }
    }

    /// <summary>A check box in the theme's colours: Windows' own keeps a pale box on a dark theme.</summary>
    sealed class DialogCheck : CheckBox
    {
        readonly Themes.Theme _theme;
        readonly int _box, _gap;
        bool _hover;

        public DialogCheck(Themes.Theme theme, string text, Font font, int box, int gap)
        {
            SetStyle(ControlStyles.AllPaintingInWmPaint | ControlStyles.OptimizedDoubleBuffer | ControlStyles.UserPaint | ControlStyles.ResizeRedraw, true);
            _theme = theme;
            _box = box;
            _gap = gap;
            Text = text;
            Font = font;
            Cursor = Cursors.Hand;
            AutoSize = false;
            var size = TextRenderer.MeasureText(text, font);
            Size = new Size(box + gap + size.Width + 2, Math.Max(box, size.Height) + 4);
        }

        protected override void OnMouseEnter(EventArgs e) { _hover = true; Invalidate(); base.OnMouseEnter(e); }
        protected override void OnMouseLeave(EventArgs e) { _hover = false; Invalidate(); base.OnMouseLeave(e); }
        protected override void OnGotFocus(EventArgs e) { Invalidate(); base.OnGotFocus(e); }
        protected override void OnLostFocus(EventArgs e) { Invalidate(); base.OnLostFocus(e); }

        protected override void OnPaint(PaintEventArgs e)
        {
            var g = e.Graphics;
            g.Clear(_theme.Background);
            g.SmoothingMode = SmoothingMode.AntiAlias;
            var box = new RectangleF(1.5f, (Height - _box) / 2f + .5f, _box - 2, _box - 2);
            Color fill = Checked ? _theme.Good : _theme.Surface;
            if (_hover)
                fill = Mix(fill, Checked ? Color.White : _theme.Text, .08f);
            using (var path = Round(box, _box / 4.5f))
            {
                using (var brush = new SolidBrush(fill))
                    g.FillPath(brush, path);
                if (!Checked)
                    using (var line = new Pen(_hover ? _theme.Text2 : _theme.Line))
                        g.DrawPath(line, path);
                if (Focused && ShowFocusCues)
                    using (var ring = new Pen(Mix(_theme.Text, _theme.Background, .35f), 1.5f))
                    using (var outer = Round(RectangleF.Inflate(box, 2.5f, 2.5f), _box / 3.5f))
                        g.DrawPath(ring, outer);
            }
            if (Checked)
            {
                using var tick = new Pen(InkOn(fill), Math.Max(1.5f, _box / 8f)) { StartCap = LineCap.Round, EndCap = LineCap.Round, LineJoin = LineJoin.Round };
                g.DrawLines(tick, [
                    new PointF(box.X + box.Width * .24f, box.Y + box.Height * .52f),
                    new PointF(box.X + box.Width * .43f, box.Y + box.Height * .71f),
                    new PointF(box.X + box.Width * .77f, box.Y + box.Height * .31f),
                ]);
            }
            var text = new Rectangle(_box + _gap, 0, Width - _box - _gap, Height);
            TextRenderer.DrawText(g, Text, Font, text, _theme.Text2, _theme.Background,
                TextFormatFlags.Left | TextFormatFlags.VerticalCenter | TextFormatFlags.SingleLine);
        }
    }

    static Color Mix(Color a, Color b, float t) =>
        Color.FromArgb((int)(a.R + (b.R - a.R) * t), (int)(a.G + (b.G - a.G) * t), (int)(a.B + (b.B - a.B) * t));

    static Color InkOn(Color fill) => fill.GetBrightness() > .55f ? Color.FromArgb(10, 22, 14) : Color.White;

    static GraphicsPath Round(RectangleF r, float radius)
    {
        var p = new GraphicsPath();
        float d = radius * 2;
        p.AddArc(r.X, r.Y, d, d, 180, 90);
        p.AddArc(r.Right - d, r.Y, d, d, 270, 90);
        p.AddArc(r.Right - d, r.Bottom - d, d, d, 0, 90);
        p.AddArc(r.X, r.Bottom - d, d, d, 90, 90);
        p.CloseFigure();
        return p;
    }
}
