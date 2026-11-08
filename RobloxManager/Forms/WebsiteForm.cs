using System.ComponentModel;
using System.Text.Json;

namespace RobloxManager;

/// <summary>Bloxkeep's browser for www.roblox.com: one window, one tab per account (<see cref="WebsiteTab"/>, each its own
/// private sign-in), plus blank tabs (+) for signing in on the site. Its top (the tabs, back/forward/reload, the site,
/// the status line, Save to Bloxkeep) is the app's own page (<see cref="ChromeView"/>, apart from the roblox.com ones).</summary>
public sealed class WebsiteForm : Form
{
    readonly Themes.Theme _theme = Themes.Current;
    readonly Panel _pages;
    readonly ChromeView _chrome;
    readonly List<(WebsiteTab Tab, string Key)> _tabs = new();
    WebsiteTab? _selected;
    bool _closingAll, _done, _pushSoon;
    int _keys;

    /// <summary>Opens a blank tab in this window (the + button, Ctrl+T); set by the app, which keeps the list of tabs.</summary>
    [DesignerSerializationVisibility(DesignerSerializationVisibility.Hidden)]
    public Action? NewBlankTab { get; set; }

    /// <summary>The saved accounts the top's account picker lists (as the app shows them); set by the app.</summary>
    [DesignerSerializationVisibility(DesignerSerializationVisibility.Hidden)]
    public Func<IReadOnlyList<WebsiteAccount>>? Accounts { get; set; }

    /// <summary>An account picked on the top for this tab; set by the app, which keeps one tab per account.</summary>
    [DesignerSerializationVisibility(DesignerSerializationVisibility.Hidden)]
    public Action<WebsiteTab, Guid>? SwitchAccount { get; set; }

    public IReadOnlyList<WebsiteTab> Tabs => _tabs.Select(t => t.Tab).ToList();
    public WebsiteTab? SelectedTab => _selected;

    public WebsiteForm()
    {
        Text = "Roblox";
        Icon = Ui.AppIcon;
        BackColor = _theme.Background;
        StartPosition = FormStartPosition.Manual;
        KeyPreview = true;

        _chrome = new ChromeView(this, 84) { Relayout = LayOut };
        _chrome.Act += OnAct;
        // Placed by hand (LayOut), not docked: the top's account menu reaches over the page below while it's open.
        _chrome.Control.Dock = DockStyle.None;
        _pages = new Panel { BackColor = _theme.Background };
        Controls.Add(_pages);
        Controls.Add(_chrome.Control);
        // Back from the main window (where the theme may have changed): drawn again in it.
        Activated += (_, _) => Push();
        // After the top and the page exist: a minimum above the form's size resizes it, which lays it out (LayOut).
        MinimumSize = new Size(LogicalToDeviceUnits(480), LogicalToDeviceUnits(360));
    }

    protected override async void OnHandleCreated(EventArgs e)
    {
        base.OnHandleCreated(e);
        Ui.UseThemedTitleBar(this, _theme);
        if (await _chrome.StartAsync())
            Push();
    }

    Form? _placeNear;

    /// <summary>Opens beside the main window, centred on its screen.</summary>
    public void PlaceNear(Form owner)
    {
        _placeNear = owner;
        if (IsHandleCreated)
            Place();
    }

    protected override void OnLoad(EventArgs e)
    {
        base.OnLoad(e);
        Place();
    }

    /// <summary>
    /// Only with the window real, and its place before its size: set together, or before the handle, on a screen at
    /// another scale than the main one, the window was sized at the wrong scale and Windows' scale change resized it by
    /// the ratio again (review 2026-10-30; as ShellForm.RestorePlace).
    /// </summary>
    void Place()
    {
        if (_placeNear is not { IsDisposed: false } owner)
            return;
        var area = Screen.FromControl(owner).WorkingArea;
        Location = new Point(area.Left + Math.Max(0, (area.Width - Width) / 2), area.Top + Math.Max(0, (area.Height - Height) / 2));
        var size = new Size(Math.Min(LogicalToDeviceUnits(1280), area.Width), Math.Min(LogicalToDeviceUnits(860), area.Height));
        Bounds = new Rectangle(area.Left + (area.Width - size.Width) / 2, area.Top + (area.Height - size.Height) / 2, size.Width, size.Height);
    }

    protected override void OnLayout(LayoutEventArgs levent)
    {
        base.OnLayout(levent);
        LayOut();
    }

    protected override void OnSizeChanged(EventArgs e)
    {
        base.OnSizeChanged(e);
        Push(); // the room the account menu has
    }

    /// <summary>
    /// The top above the page, at its own height; with its account menu open, taller by the menu and over the page, but
    /// cut to the bar and the menu, so the page shows beside it. Sibling windows: the top, in front, draws over the page.
    /// </summary>
    void LayOut()
    {
        // Laid out while the constructor is still making them (a size set there), or after they're gone.
        if (IsDisposed || _chrome is null || _pages is null)
            return;
        var area = ClientSize;
        int bar = Math.Min(_chrome.BarPixels, area.Height);
        var menu = _chrome.MenuPixels;
        int height = menu is { } m ? Math.Min(Math.Max(bar, m.Bottom), area.Height) : bar;
        var top = _chrome.Control;
        top.Bounds = new Rectangle(0, 0, area.Width, height);
        var old = top.Region;
        if (menu is { } open)
        {
            var region = new Region(new Rectangle(0, 0, area.Width, bar));
            region.Union(open);
            top.Region = region;
        }
        else
            top.Region = null;
        old?.Dispose();
        _pages.Bounds = new Rectangle(0, bar, area.Width, Math.Max(0, area.Height - bar));
        top.BringToFront();
    }

    public void AddTab(WebsiteTab tab)
    {
        tab.TitleChanged += () =>
        {
            if (_selected == tab)
                SyncTitle();
            Push();
        };
        tab.StateChanged += () =>
        {
            if (_selected == tab)
                Push();
        };
        tab.CloseRequested += () => _ = CloseTabAsync(tab);
        tab.Visible = false;
        _tabs.Add((tab, "t" + ++_keys));
        _pages.Controls.Add(tab);
        SelectTab(tab);
        tab.Start();
    }

    public void SelectTab(WebsiteTab tab)
    {
        if (!_tabs.Any(t => t.Tab == tab))
            return;
        _selected = tab;
        foreach (var (other, _) in _tabs)
        {
            if (other != tab)
                other.Visible = false;
        }
        tab.Visible = true;
        tab.BringToFront();
        SyncTitle();
        Push();
        if (ContainsFocus)
            tab.FocusPage();
    }

    void SyncTitle() => Text = _selected is { } tab && tab.Title != "Roblox" ? L.F("{0} — Roblox", tab.Title) : "Roblox";

    /// <summary>The top again: the saved accounts or their names changed.</summary>
    public void RefreshTop() => Push();

    /// <summary>What the top shows now, once per round of changes (a tab added and chosen is one).</summary>
    void Push()
    {
        if (_pushSoon || IsDisposed || !IsHandleCreated)
            return;
        _pushSoon = true;
        BeginInvoke(() =>
        {
            _pushSoon = false;
            if (IsDisposed)
                return;
            var tab = _selected;
            _chrome.Show(new
            {
                mode = "tabs",
                accounts = (Accounts?.Invoke() ?? []).Select(a => new { id = a.Id.ToString(), name = a.Name, avatar = a.Avatar }),
                account = tab?.AccountId?.ToString(),
                // How tall the account menu may be (CSS pixels): the window below the top's own height.
                room = Math.Max(0, (int)((ClientSize.Height - _chrome.BarPixels) * 96f / DeviceDpi)),
                tabs = _tabs.Select(t => new { id = t.Key, title = t.Tab.Title, avatar = t.Tab.Avatar, saved = t.Tab.AccountId is not null, selected = t.Tab == tab }),
                back = tab?.CanGoBack ?? false,
                forward = tab?.CanGoForward ?? false,
                host = tab?.Host,
                message = tab?.Message ?? "",
                tone = tab?.Tone ?? "",
                retry = tab?.ShowRetry ?? false,
                save = tab?.CanSave ?? false,
                theme = PageLook.Theme,
                customTheme = PageLook.CustomTheme,
                language = L.Language,
            });
        });
    }

    /// <summary>Clicked on the top. Only this window's own buttons: the page can't send a tab anywhere.</summary>
    void OnAct(string what, JsonElement args)
    {
        string key = args.TryGetProperty("id", out var id) && id.ValueKind == JsonValueKind.String ? id.GetString()! : "";
        var byKey = _tabs.FirstOrDefault(t => t.Key == key).Tab;
        switch (what)
        {
            case "select" when byKey is not null:
                SelectTab(byKey);
                byKey.FocusPage();
                break;
            case "close" when byKey is not null:
                _ = CloseTabAsync(byKey);
                break;
            case "newTab":
                NewBlankTab?.Invoke();
                break;
            case "back":
                _selected?.GoBack();
                _selected?.FocusPage();
                break;
            case "forward":
                _selected?.GoForward();
                _selected?.FocusPage();
                break;
            case "reload":
                _selected?.Reload();
                _selected?.FocusPage();
                break;
            case "retry":
                _selected?.Retry();
                break;
            case "save":
                _selected?.Save();
                break;
            case "account" when _selected is { } current && args.TryGetProperty("account", out var a) && a.ValueKind == JsonValueKind.String
                    && Guid.TryParse(a.GetString(), out var account):
                SwitchAccount?.Invoke(current, account);
                break;
        }
    }

    /// <summary>Closes one tab (after a last read of a saved account's session); the window goes with its last tab.</summary>
    public async Task CloseTabAsync(WebsiteTab tab)
    {
        int index = _tabs.FindIndex(t => t.Tab == tab);
        if (index < 0)
            return;
        _tabs.RemoveAt(index);
        if (_selected == tab)
        {
            _selected = null;
            if (_tabs.Count > 0)
                SelectTab(_tabs[Math.Min(index, _tabs.Count - 1)].Tab);
        }
        Push();
        tab.Visible = false;
        await tab.CloseAsync();
        _pages.Controls.Remove(tab);
        tab.Dispose();
        if (_tabs.Count == 0 && !_closingAll && !IsDisposed)
        {
            _done = true;
            Close();
        }
    }

    /// <summary>Closes every tab at once, without the last read of the sessions (the vault is locking, or the app quitting).</summary>
    public void CloseNow()
    {
        _closingAll = _done = true;
        foreach (var (tab, _) in _tabs.ToList())
            tab.CloseNow();
        _tabs.Clear();
        Close();
    }

    protected override async void OnFormClosing(FormClosingEventArgs e)
    {
        base.OnFormClosing(e);
        if (e.Cancel || _done || _tabs.Count == 0)
            return;
        // Each saved account's session is read once more before its tab goes, a moment at most.
        e.Cancel = true;
        _done = _closingAll = true;
        Hide();
        try
        {
            await Task.WhenAll(_tabs.Select(t => t.Tab.CloseAsync()).ToList());
        }
        finally
        {
            _tabs.Clear();
            if (!IsDisposed)
                Close();
        }
    }

    protected override void OnFormClosed(FormClosedEventArgs e)
    {
        foreach (var (tab, _) in _tabs.ToList())
            tab.CloseNow();
        _tabs.Clear();
        base.OnFormClosed(e);
    }

    /// <summary>Ctrl+T a blank tab, Ctrl+W closes this one, Ctrl+Tab / Ctrl+Shift+Tab (or Ctrl+PgDn / PgUp) the next or previous.</summary>
    protected override bool ProcessCmdKey(ref Message msg, Keys keyData)
    {
        switch (keyData)
        {
            case Keys.Control | Keys.T:
                NewBlankTab?.Invoke();
                return true;
            case Keys.Control | Keys.W when _selected is { } tab:
                _ = CloseTabAsync(tab);
                return true;
            case Keys.Control | Keys.Tab or Keys.Control | Keys.PageDown:
                Step(1);
                return true;
            case Keys.Control | Keys.Shift | Keys.Tab or Keys.Control | Keys.PageUp:
                Step(-1);
                return true;
        }
        return base.ProcessCmdKey(ref msg, keyData);
    }

    void Step(int by)
    {
        if (_tabs.Count < 2 || _selected is null)
            return;
        int i = _tabs.FindIndex(t => t.Tab == _selected);
        SelectTab(_tabs[(i + by + _tabs.Count) % _tabs.Count].Tab);
        _selected?.FocusPage();
    }
}

/// <summary>A saved account as the website window's account picker shows it.</summary>
public sealed record WebsiteAccount(Guid Id, string Name, string? Avatar);
