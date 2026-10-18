using System.Runtime.InteropServices;

namespace RobloxManager;

/// <summary>
/// The tray icon's quick panel, opened with a left click (the right click keeps the menu, <see cref="TrayMenu"/>), like
/// Windows 11's own quick settings: Bloxkeep's name with how many Roblox windows run and their RAM, Multi-Roblox's
/// switch, the accounts playing now (click one to bring its window up; the eye and the cross on hover), and three
/// tiles: Play (a page of accounts), Presets (a page of presets) and a new Roblox window to sign in to. Drawn here in
/// the app's theme, like the mini window: it costs next to nothing while Roblox plays, and opens at once. It goes
/// away when something else is clicked, or with Esc.
/// </summary>
sealed class TrayPanel : Form
{
    const int LogicalWidth = 340, Pad = 14, MaxClients = 6;
    const TextFormatFlags One = TextFormatFlags.SingleLine | TextFormatFlags.EndEllipsis | TextFormatFlags.NoPrefix | TextFormatFlags.NoPadding;

    readonly System.Windows.Forms.Timer _timer = new() { Interval = 2000 };
    readonly ToolTip _tip = new() { ShowAlways = true };
    readonly List<(Rectangle Box, string Key, Action Click, string? Tip)> _hits = [];
    AppController.TrayInfo? _state;
    Themes.Theme _theme = Themes.Current;
    Font _text = null!, _small = null!, _title = null!;
    int _fontsDpi;
    Point _mouse = new(-1, -1);
    string? _tipKey;
    /// <summary>"home", or the page a tile opened: "accounts" or "presets".</summary>
    string _page = "home";
    long _hiddenAt, _confirmCloseUntil;
    bool _fromBottom = true;

    public event Action? OpenClicked;
    public event Action? MiniClicked;
    /// <summary>Multi-Roblox's switch, with whether it is on now (the owner decides, as for the menu's item).</summary>
    public event Action<bool>? MultiClicked;
    public event Action<Guid>? AccountClicked;
    public event Action<Guid>? PresetClicked;
    public event Action? NewRobloxClicked;
    public event Action? CloseAllClicked;
    public event Action<int>? ShowClientClicked;
    public event Action<Guid>? CloseClientClicked;

    /// <summary>What Bloxkeep has to offer now (<see cref="AppController.TrayState"/>), read when it opens and every two seconds.</summary>
    [System.ComponentModel.DesignerSerializationVisibility(System.ComponentModel.DesignerSerializationVisibility.Hidden)]
    public Func<AppController.TrayInfo>? State { get; set; }

    [System.ComponentModel.DesignerSerializationVisibility(System.ComponentModel.DesignerSerializationVisibility.Hidden)]
    public Func<bool>? MiniOpen { get; set; }

    public TrayPanel()
    {
        FormBorderStyle = FormBorderStyle.None;
        ShowInTaskbar = false;
        TopMost = true;
        StartPosition = FormStartPosition.Manual;
        KeyPreview = true;
        Text = Ui.Title;
        SetStyle(ControlStyles.OptimizedDoubleBuffer | ControlStyles.AllPaintingInWmPaint | ControlStyles.UserPaint | ControlStyles.ResizeRedraw, true);
        _timer.Tick += (_, _) =>
        {
            try
            {
                Reload();
            }
            catch (Exception ex)
            {
                Log.Write(ex);
                _timer.Stop();
            }
        };
    }

    protected override CreateParams CreateParams
    {
        get
        {
            const int WS_EX_TOOLWINDOW = 0x80, CS_DROPSHADOW = 0x20000;
            var p = base.CreateParams;
            p.ExStyle |= WS_EX_TOOLWINDOW; // not in Alt+Tab
            p.ClassStyle |= CS_DROPSHADOW;
            return p;
        }
    }

    int Px(int logical) => (int)Math.Round(logical * DeviceDpi / 96f);

    /// <summary>The tray icon was clicked just as the panel went away (the click itself took it away): don't bring it back.</summary>
    public bool RecentlyHidden => Environment.TickCount64 - _hiddenAt < 400;

    /// <summary>Opens the panel by the tray icon (at <paramref name="anchor"/>, the pointer), or closes it when open.</summary>
    public void Toggle(Point anchor)
    {
        if (Visible)
        {
            HidePanel();
            return;
        }
        if (RecentlyHidden)
            return;
        _page = "home";
        _confirmCloseUntil = 0;
        Place(anchor);
        Show();
        Activate();
        SetForegroundWindow(Handle);
        Place(anchor); // again, at the scale of the screen it opened on
        _timer.Start();
    }

    public void HidePanel()
    {
        if (!Visible)
            return;
        _hiddenAt = Environment.TickCount64;
        _timer.Stop();
        _tip.Hide(this);
        Hide();
    }

    [DllImport("user32.dll")]
    static extern bool SetForegroundWindow(IntPtr hWnd);

    [DllImport("dwmapi.dll")]
    static extern int DwmSetWindowAttribute(IntPtr hwnd, int attribute, ref int value, int size);

    protected override void OnHandleCreated(EventArgs e)
    {
        base.OnHandleCreated(e);
        Frame();
    }

    /// <summary>Windows 11's rounded corners and a border in the theme's line colour.</summary>
    void Frame()
    {
        const int DWMWA_WINDOW_CORNER_PREFERENCE = 33, DWMWA_BORDER_COLOR = 34, DWMWCP_ROUND = 2;
        int round = DWMWCP_ROUND;
        DwmSetWindowAttribute(Handle, DWMWA_WINDOW_CORNER_PREFERENCE, ref round, sizeof(int));
        int border = ColorTranslator.ToWin32(_theme.Line);
        DwmSetWindowAttribute(Handle, DWMWA_BORDER_COLOR, ref border, sizeof(int));
    }

    protected override void OnDeactivate(EventArgs e)
    {
        base.OnDeactivate(e);
        HidePanel();
    }

    protected override void OnKeyDown(KeyEventArgs e)
    {
        if (e.KeyCode == Keys.Escape)
        {
            e.Handled = true;
            if (_page != "home")
                Go("home");
            else
                HidePanel();
            return;
        }
        base.OnKeyDown(e);
    }

    protected override void OnDpiChanged(DpiChangedEventArgs e)
    {
        base.OnDpiChanged(e);
        Reload();
    }

    /// <summary>Above the taskbar by the pointer (or beside it, when the taskbar is at a side or the top), inside the screen.</summary>
    void Place(Point anchor)
    {
        Reload(sized: false);
        var screen = Screen.FromPoint(anchor);
        Rectangle work = screen.WorkingArea, all = screen.Bounds;
        int gap = Px(12), w = Px(LogicalWidth), h = Measure();
        int x = Math.Clamp(anchor.X - w / 2, work.Left + gap, work.Right - w - gap);
        int y = work.Bottom - h - gap;
        _fromBottom = true;
        if (work.Top > all.Top)
        {
            y = work.Top + gap;
            _fromBottom = false;
        }
        else if (work.Left > all.Left || work.Right < all.Right)
        {
            x = work.Left > all.Left ? work.Left + gap : work.Right - w - gap;
            y = Math.Clamp(anchor.Y - h + gap, work.Top + gap, work.Bottom - h - gap);
        }
        Bounds = new Rectangle(x, y, w, h);
    }

    /// <summary>Takes the state again and redraws; a change in height keeps the edge by the taskbar where it is.</summary>
    void Reload(bool sized = true)
    {
        _state = State?.Invoke();
        var theme = Themes.Current;
        if (theme != _theme)
        {
            _theme = theme;
            if (IsHandleCreated)
                Frame();
        }
        BackColor = _theme.Surface;
        if (_page == "accounts" && _state is not { Open: true, Accounts.Count: > 0 } || _page == "presets" && _state is not { Open: true, Presets.Count: > 0 })
            _page = "home";
        if (sized)
        {
            int h = Measure();
            if (h != Height)
            {
                var old = Bounds;
                Bounds = new Rectangle(old.X, _fromBottom ? old.Bottom - h : old.Y, Px(LogicalWidth), h);
            }
        }
        Invalidate();
    }

    void Go(string page)
    {
        _page = page;
        _confirmCloseUntil = 0;
        Reload();
    }

    int Measure() => Build(null);

    protected override void OnPaint(PaintEventArgs e)
    {
        e.Graphics.Clear(_theme.Surface);
        Build(e.Graphics);
    }

    protected override void OnMouseMove(MouseEventArgs e)
    {
        base.OnMouseMove(e);
        _mouse = e.Location;
        var hit = HitAt(e.Location);
        Cursor = hit is null ? Cursors.Default : Cursors.Hand;
        if (hit?.Key != _tipKey)
        {
            _tipKey = hit?.Key;
            if (hit?.Tip is { } tip)
                _tip.Show(tip, this, e.X + Px(12), e.Y + Px(18), 2500);
            else
                _tip.Hide(this);
        }
        Invalidate();
    }

    protected override void OnMouseLeave(EventArgs e)
    {
        base.OnMouseLeave(e);
        _mouse = new Point(-1, -1);
        _tipKey = null;
        _tip.Hide(this);
        Invalidate();
    }

    protected override void OnMouseUp(MouseEventArgs e)
    {
        base.OnMouseUp(e);
        if (e.Button == MouseButtons.Left && HitAt(e.Location) is { } hit)
            hit.Click();
    }

    (Rectangle Box, string Key, Action Click, string? Tip)? HitAt(Point p)
    {
        // The last added is on top (a row's buttons over the row).
        for (int i = _hits.Count - 1; i >= 0; i--)
        {
            if (_hits[i].Box.Contains(p))
                return _hits[i];
        }
        return null;
    }

    bool Over(Rectangle r) => r.Contains(_mouse);

    void Fonts()
    {
        if (_fontsDpi == DeviceDpi && _text is not null)
            return;
        _text?.Dispose();
        _small?.Dispose();
        _title?.Dispose();
        float s = DeviceDpi / 96f;
        _text = new Font("Segoe UI", 13 * s, GraphicsUnit.Pixel);
        _small = new Font("Segoe UI", 12 * s, GraphicsUnit.Pixel);
        _title = new Font("Segoe UI Semibold", 14 * s, GraphicsUnit.Pixel);
        _fontsDpi = DeviceDpi;
    }

    static string Ram(long mb) => mb >= 1024 ? $"{mb / 1024.0:0.0} GB" : $"{Math.Max(0, mb)} MB";

    /// <summary>Lays the panel out, and draws it when <paramref name="g"/> is given; the height it needs.</summary>
    int Build(Graphics? g)
    {
        Fonts();
        _hits.Clear();
        var t = _theme;
        var s = _state;
        int pad = Px(Pad), w = Px(LogicalWidth), inner = w - pad * 2, y = pad;
        int lineH = TextRenderer.MeasureText("Ag", _text).Height, smallH = TextRenderer.MeasureText("Ag", _small).Height;
        Color card = Drawn.Mix(t.Surface, t.Text, t.Dark ? .05f : .035f);

        // ── The header: logo, name, what runs; the mini window and Bloxkeep itself on the right.
        int logo = Px(34);
        if (g is not null)
            Drawn.Logo(g, new Rectangle(pad, y, logo, logo));
        int buttons = Px(32), gap = Px(6);
        var open = new Rectangle(w - pad - buttons, y + (logo - buttons) / 2, buttons, buttons);
        var mini = new Rectangle(open.X - gap - buttons, open.Y, buttons, buttons);
        int textX = pad + logo + Px(10), textW = mini.X - Px(8) - textX;
        string status = s is null ? "" : s is { Open: false, MultiOnly: false } ? L.T("Locked")
            : s.RobloxCount > 0 ? L.P(s.RobloxCount, "1 Roblox window · {1} RAM", "{0} Roblox windows · {1} RAM", s.RobloxCount, Ram(s.RamMb))
            : L.T("No Roblox window open");
        if (g is not null)
        {
            int ty = y + (logo - lineH - smallH) / 2;
            TextRenderer.DrawText(g, "Bloxkeep", _title, new Rectangle(textX, ty, textW, lineH), t.Text, One);
            TextRenderer.DrawText(g, status, _small, new Rectangle(textX, ty + lineH, textW, smallH), t.Text2, One);
            bool miniOn = MiniOpen?.Invoke() == true;
            IconButton(g, mini, Drawn.Glyph.Mini, miniOn ? Drawn.Mix(t.Surface, t.Text, t.Dark ? .14f : .1f) : null);
            IconButton(g, open, Drawn.Glyph.Open, card);
        }
        Hit(mini, "mini", () => { MiniClicked?.Invoke(); HidePanel(); }, L.T("Mini window"));
        Hit(open, "open", () => { HidePanel(); OpenClicked?.Invoke(); }, L.T("Open Bloxkeep"));
        y += logo + pad;

        if (s is null)
            return y;
        if (_page != "home")
            return Page(g, y, pad, inner, lineH, smallH);

        // ── Multi-Roblox: a card in its state's colour, with the switch.
        bool on = s.Multi != "off";
        Color tone = s.Multi switch { "ready" => t.Good, "notReady" => TrayMenu.Amber, _ => t.Text2 };
        var multi = new Rectangle(pad, y, inner, Px(60));
        if (g is not null)
        {
            Color fill = s.Multi == "off" ? card : Drawn.Mix(t.Surface, tone, t.Dark ? .13f : .1f);
            if (Over(multi))
                fill = Drawn.Mix(fill, t.Text, .04f);
            Drawn.FillRound(g, multi, Px(10), fill, s.Multi == "off" ? t.Line : Drawn.Mix(t.Surface, tone, .35f));
            Drawn.Icon(g, Drawn.Glyph.Multi, new Rectangle(multi.X + Px(12), multi.Y, Px(22), multi.Height), s.Multi == "off" ? t.Text2 : tone, Px(18));
            int mx = multi.X + Px(44), mw = multi.Width - Px(44) - Px(60);
            int my = multi.Y + (multi.Height - lineH - smallH) / 2;
            TextRenderer.DrawText(g, "Multi-Roblox", _text, new Rectangle(mx, my, mw, lineH), t.Text, One);
            string sub = !TrayMenu.CanSwitch(s, on) && !s.Open && !s.MultiOnly ? L.T("Unlock Bloxkeep to change it")
                : L.T(s.Multi switch { "ready" => "On, several windows allowed", "notReady" => "Not working yet: open Bloxkeep to fix it", _ => "Off: a new window closes the last one" });
            TextRenderer.DrawText(g, sub, _small, new Rectangle(mx, my + lineH, mw, smallH),
                s.Multi == "off" ? t.Text2 : Drawn.Mix(tone, t.Text, .35f), One);
            Drawn.Switch(g, new RectangleF(multi.Right - Px(12) - Px(38), multi.Y + (multi.Height - Px(22)) / 2f, Px(38), Px(22)), on, t, Drawn.Accent(t));
        }
        Hit(multi, "multi", () =>
        {
            // Switched here when it can be (off with a vault; on and off with Multi-Roblox only); otherwise Bloxkeep
            // opens: its sidebar switch asks first, its card fixes "not ready", or the lock screen.
            if (TrayMenu.CanSwitch(s, on) && !(s.Multi == "notReady" && s.Open))
            {
                MultiClicked?.Invoke(on);
                Reload();
                return;
            }
            HidePanel();
            OpenClicked?.Invoke();
        });
        y += multi.Height + pad;

        // ── The Roblox windows running now.
        bool canClose = s.RobloxCount > 0 && (s.Open || s.MultiOnly);
        var clients = s.Clients ?? [];
        if (s is { Open: false, MultiOnly: false })
        {
            // Locked: nothing is shown about the accounts, only the way back in.
            var box = new Rectangle(pad, y, inner, Px(52));
            if (g is not null)
            {
                Drawn.FillRound(g, box, Px(10), card);
                Drawn.Icon(g, Drawn.Glyph.Lock, new Rectangle(box.X + Px(12), box.Y, Px(22), box.Height), t.Text2, Px(17));
                TextRenderer.DrawText(g, L.T("Bloxkeep is locked"), _text, new Rectangle(box.X + Px(44), box.Y, box.Width - Px(140), box.Height), t.Text,
                    One | TextFormatFlags.VerticalCenter);
            }
            var unlock = new Rectangle(box.Right - Px(10) - Px(84), box.Y + (box.Height - Px(30)) / 2, Px(84), Px(30));
            if (g is not null)
                TextButton(g, unlock, L.T("Unlock"), primary: false);
            Hit(unlock, "unlock", () => { HidePanel(); OpenClicked?.Invoke(); });
            y += box.Height + pad;
        }
        else
        {
            var section = new Rectangle(pad + Px(4), y, inner - Px(8), smallH);
            if (g is not null)
                TextRenderer.DrawText(g, L.T("Running"), _small, section, t.Text2, One);
            if (canClose)
            {
                bool confirming = Environment.TickCount64 < _confirmCloseUntil;
                string label = confirming ? L.P(s.RobloxCount, "Close 1 window?", "Close {0} windows?", s.RobloxCount) : L.T("Close all");
                int lw = TextRenderer.MeasureText(label, _small).Width;
                var link = new Rectangle(section.Right - lw - Px(6), y - Px(3), lw + Px(12), smallH + Px(6));
                if (g is not null)
                {
                    if (Over(link) || confirming)
                        Drawn.FillRound(g, link, Px(6), confirming ? Drawn.Mix(t.Surface, t.Bad, .16f) : Drawn.Hover(t));
                    TextRenderer.DrawText(g, label, _small, link, confirming || Over(link) ? t.Bad : t.Text2,
                        One | TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter);
                }
                Hit(link, "closeall", () =>
                {
                    if (Environment.TickCount64 < _confirmCloseUntil)
                    {
                        _confirmCloseUntil = 0;
                        CloseAllClicked?.Invoke();
                    }
                    else
                        _confirmCloseUntil = Environment.TickCount64 + 4000;
                    Reload();
                });
            }
            y += smallH + Px(6);

            if (clients.Count == 0)
            {
                string empty = s.RobloxCount > 0 ? L.P(s.RobloxCount, "1 Roblox window open", "{0} Roblox windows open", s.RobloxCount) : L.T("No Roblox window open");
                var box = new Rectangle(pad, y, inner, Px(44));
                if (g is not null)
                    TextRenderer.DrawText(g, empty, _text, box, t.Text2, One | TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter);
                y += box.Height;
            }
            int rowH = Px(48);
            foreach (var c in clients.Take(clients.Count > MaxClients ? MaxClients - 1 : MaxClients))
            {
                var row = new Rectangle(pad, y, inner, rowH);
                bool hover = Over(row);
                var close = new Rectangle(row.Right - Px(8) - Px(30), row.Y + (rowH - Px(30)) / 2, Px(30), Px(30));
                var show = new Rectangle(close.X - Px(4) - Px(30), close.Y, Px(30), Px(30));
                if (g is not null)
                {
                    if (hover)
                        Drawn.FillRound(g, row, Px(9), Drawn.Hover(t));
                    Drawn.Avatar(g, new RectangleF(row.X + Px(8), row.Y + (rowH - Px(32)) / 2f, Px(32), Px(32)), s.PictureOf?.Invoke(c.Id), c.Name);
                    int cx = row.X + Px(50), cw = (hover ? show.X : row.Right) - Px(8) - cx;
                    int cy = row.Y + (rowH - lineH - smallH) / 2;
                    TextRenderer.DrawText(g, c.Name, _text, new Rectangle(cx, cy, cw, lineH), t.Text, One);
                    TextRenderer.DrawText(g, $"{c.Game} · {Ram(c.RamMb)}", _small, new Rectangle(cx, cy + lineH, cw, smallH), t.Text2, One);
                    if (hover)
                    {
                        IconButton(g, show, Drawn.Glyph.Show, null);
                        IconButton(g, close, Drawn.Glyph.Close, null, danger: true);
                    }
                }
                var (id, pid) = (c.Id, c.Pid);
                Hit(row, "client:" + id, () => { HidePanel(); ShowClientClicked?.Invoke(pid); });
                if (hover)
                {
                    Hit(show, "show:" + id, () => { HidePanel(); ShowClientClicked?.Invoke(pid); }, L.T("Show"));
                    Hit(close, "close:" + id, () => { CloseClientClicked?.Invoke(id); Reload(); }, L.T("Close"));
                }
                y += rowH;
            }
            if (clients.Count > MaxClients)
            {
                var more = new Rectangle(pad, y, inner, Px(36));
                if (g is not null)
                {
                    if (Over(more))
                        Drawn.FillRound(g, more, Px(9), Drawn.Hover(t));
                    TextRenderer.DrawText(g, L.F("+{0} more", clients.Count - MaxClients + 1), _small, more, t.Text2,
                        One | TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter);
                }
                Hit(more, "more", () => { HidePanel(); OpenClicked?.Invoke(); });
                y += more.Height;
            }
            y += Px(10);
        }

        // ── The tiles: Play and Presets open their page here; a new window to sign in to.
        var tiles = new List<(string Key, string Glyph, string Label, bool Primary, Action Click)>();
        if (s is { Open: true, Accounts.Count: > 0 })
            tiles.Add(("play", Drawn.Glyph.Play, L.T("Play"), true, () => Go("accounts")));
        if (s is { Open: true, Presets.Count: > 0 })
            tiles.Add(("presets", Drawn.Glyph.Presets, L.T("Presets"), false, () => Go("presets")));
        tiles.Add(("new", Drawn.Glyph.SignIn, L.T("New Roblox"), false, () => { HidePanel(); NewRobloxClicked?.Invoke(); }));
        int tileGap = Px(8), tileW = (inner - tileGap * (tiles.Count - 1)) / tiles.Count, tileH = Px(64);
        for (int i = 0; i < tiles.Count; i++)
        {
            var tile = tiles[i];
            var box = new Rectangle(pad + i * (tileW + tileGap), y, tileW, tileH);
            if (g is not null)
            {
                bool hover = Over(box);
                Color accent = Drawn.Accent(t);
                Color fill = tile.Primary ? (hover ? Drawn.Mix(accent, Color.White, .12f) : accent) : hover ? Drawn.Hover(t) : card;
                Color ink = tile.Primary ? Drawn.AccentInk(t) : t.Text;
                Drawn.FillRound(g, box, Px(10), fill, tile.Primary ? null : t.Line);
                Drawn.Icon(g, tile.Glyph, new Rectangle(box.X, box.Y + Px(10), box.Width, Px(24)), tile.Primary ? ink : t.Text2, Px(18));
                TextRenderer.DrawText(g, tile.Label, _small, new Rectangle(box.X + Px(4), box.Y + Px(36), box.Width - Px(8), smallH), ink,
                    One | TextFormatFlags.HorizontalCenter);
            }
            Hit(box, tile.Key, tile.Click, tile.Key == "new"
                ? L.T("Sign in separately in each new window. Keep Bloxkeep running; sign-ins are not saved when these windows close.") : null);
        }
        y += tileH + pad;
        return y;
    }

    /// <summary>A tile's page: a back row with its title, then the accounts (with their pictures) or the presets.</summary>
    int Page(Graphics? g, int y, int pad, int inner, int lineH, int smallH)
    {
        var t = _theme;
        var s = _state!;
        bool accounts = _page == "accounts";
        var back = new Rectangle(pad, y, inner, Px(36));
        if (g is not null)
        {
            if (Over(back))
                Drawn.FillRound(g, back, Px(8), Drawn.Hover(t));
            Drawn.Icon(g, "", new Rectangle(back.X + Px(6), back.Y, Px(24), back.Height), t.Text2, Px(14));
            TextRenderer.DrawText(g, accounts ? L.T("Play as") : L.T("Run preset"), _text, new Rectangle(back.X + Px(36), back.Y, back.Width - Px(40), back.Height), t.Text,
                One | TextFormatFlags.VerticalCenter);
        }
        Hit(back, "back", () => Go("home"));
        y += back.Height + Px(6);
        int rowH = Px(accounts ? 44 : 40);
        if (accounts)
        {
            foreach (var (id, name, running) in s.Accounts ?? [])
            {
                var row = new Rectangle(pad, y, inner, rowH);
                if (g is not null)
                {
                    if (!running && Over(row))
                        Drawn.FillRound(g, row, Px(9), Drawn.Hover(t));
                    Drawn.Avatar(g, new RectangleF(row.X + Px(8), row.Y + (rowH - Px(28)) / 2f, Px(28), Px(28)), s.PictureOf?.Invoke(id), name);
                    string playing = running ? L.T("Playing") : "";
                    int pw = playing.Length > 0 ? TextRenderer.MeasureText(playing, _small).Width + Px(10) : 0;
                    TextRenderer.DrawText(g, name, _text, new Rectangle(row.X + Px(46), row.Y, row.Width - Px(54) - pw, rowH), running ? t.Text2 : t.Text,
                        One | TextFormatFlags.VerticalCenter);
                    if (playing.Length > 0)
                        TextRenderer.DrawText(g, playing, _small, new Rectangle(row.X, row.Y, row.Width - Px(10), rowH), t.Good,
                            One | TextFormatFlags.Right | TextFormatFlags.VerticalCenter);
                }
                if (!running)
                    Hit(row, "account:" + id, () => { HidePanel(); AccountClicked?.Invoke(id); });
                y += rowH;
            }
        }
        else
        {
            foreach (var (id, name) in s.Presets)
            {
                var row = new Rectangle(pad, y, inner, rowH);
                if (g is not null)
                {
                    if (Over(row))
                        Drawn.FillRound(g, row, Px(9), Drawn.Hover(t));
                    Drawn.Icon(g, Drawn.Glyph.Presets, new Rectangle(row.X + Px(8), row.Y, Px(28), rowH), t.Text2, Px(16));
                    TextRenderer.DrawText(g, name, _text, new Rectangle(row.X + Px(46), row.Y, row.Width - Px(54), rowH), t.Text,
                        One | TextFormatFlags.VerticalCenter);
                }
                Hit(row, "preset:" + id, () => { HidePanel(); PresetClicked?.Invoke(id); });
                y += rowH;
            }
        }
        return y + pad;
    }

    void Hit(Rectangle box, string key, Action click, string? tip = null) => _hits.Add((box, key, click, tip));

    void IconButton(Graphics g, Rectangle box, string glyph, Color? fill, bool danger = false)
    {
        var t = _theme;
        bool hover = Over(box);
        if (hover)
            Drawn.FillRound(g, box, Px(8), danger ? Drawn.Mix(t.Surface, t.Bad, .18f) : Drawn.Hover(t, pressed: fill is not null));
        else if (fill is { } f)
            Drawn.FillRound(g, box, Px(8), f);
        Drawn.Icon(g, glyph, box, hover && danger ? t.Bad : t.Text2, Px(15));
    }

    void TextButton(Graphics g, Rectangle box, string text, bool primary)
    {
        var t = _theme;
        bool hover = Over(box);
        Drawn.FillRound(g, box, Px(8), hover ? Drawn.Hover(t, pressed: true) : Drawn.Mix(t.Surface, t.Text, t.Dark ? .08f : .05f), t.Line);
        TextRenderer.DrawText(g, text, _small, box, t.Text, One | TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter);
    }

    protected override void Dispose(bool disposing)
    {
        if (disposing)
        {
            _timer.Dispose();
            _tip.Dispose();
            _text?.Dispose();
            _small?.Dispose();
            _title?.Dispose();
        }
        base.Dispose(disposing);
    }
}
