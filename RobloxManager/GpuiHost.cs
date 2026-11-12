using System.Diagnostics;
using System.IO.Pipes;
using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;
using System.Threading.Channels;

namespace RobloxManager;

/// <summary>
/// Bloxkeep 1.1's interface drawn with GPUI (..\ui-gpui, bloxkeep-ui.exe) in place of the WebView2 page, while it's being
/// built: only with the hidden switch BLOXKEEP_UI=gpui and the exe there. It gets the page's messages over two one-way
/// named pipes (<c>name</c> to it, <c>name-up</c> from it), one JSON message per line, the same ones
/// <see cref="AppController"/> trades with the page; once its window says its handle, that window is laid inside
/// <paramref name="parent"/> as a child, so the shell keeps its title bar, tray, hotkeys and cover.
/// </summary>
sealed class GpuiHost(Control parent, Func<string, Task> onMessage) : IDisposable
{
    /// <summary>The switch, and where the exe is: beside the app (ui-gpui\bloxkeep-ui.exe), or BLOXKEEP_UI_EXE.</summary>
    public static string? Exe
    {
        get
        {
            if (!string.Equals(Environment.GetEnvironmentVariable("BLOXKEEP_UI"), "gpui", StringComparison.OrdinalIgnoreCase))
                return null;
            string path = Environment.GetEnvironmentVariable("BLOXKEEP_UI_EXE") is { Length: > 0 } own
                ? own
                : Path.Combine(AppContext.BaseDirectory, "ui-gpui", "bloxkeep-ui.exe");
            return File.Exists(path) ? path : null;
        }
    }

    readonly string _name = $"bloxkeep-ui-{Environment.ProcessId}-{Guid.NewGuid():N}";
    readonly Channel<string> _out = Channel.CreateUnbounded<string>(new() { SingleReader = true });
    readonly CancellationTokenSource _stop = new();
    Process? _process;
    IntPtr _window;

    /// <summary>The window is inside the shell and showing.</summary>
    public bool Placed => _window != IntPtr.Zero;

    /// <summary>The GPUI side quit or its line broke (not when the shell closed it).</summary>
    public event Action? Gone;

    /// <summary>Starts the exe and waits for it to connect; throws when it can't start.</summary>
    public async Task StartAsync(string exe)
    {
        // (With room in them: a pipe made with none holds each write until the other side has read it.)
        var down = new NamedPipeServerStream(_name, PipeDirection.Out, 1, PipeTransmissionMode.Byte, PipeOptions.Asynchronous, PipeBuffer, PipeBuffer);
        var up = new NamedPipeServerStream(_name + "-up", PipeDirection.In, 1, PipeTransmissionMode.Byte, PipeOptions.Asynchronous, PipeBuffer, PipeBuffer);
        var info = new ProcessStartInfo(exe, ["--pipe", _name, "--cache", Vault.CacheDir, "--pictures", Vault.PicturesDir]) { UseShellExecute = false, WorkingDirectory = Path.GetDirectoryName(exe)! };
        _process = Process.Start(info) ?? throw new InvalidOperationException("bloxkeep-ui didn't start");
        _process.EnableRaisingEvents = true;
        _process.Exited += (_, _) => Lost($"bloxkeep-ui exited ({SafeExitCode()})");
        using (var wait = CancellationTokenSource.CreateLinkedTokenSource(_stop.Token))
        {
            wait.CancelAfter(TimeSpan.FromSeconds(15));
            await Task.WhenAll(down.WaitForConnectionAsync(wait.Token), up.WaitForConnectionAsync(wait.Token));
        }
        // Off the window's thread, both of them: started from it they would carry on there after every line, and the
        // reading would stop whenever that thread waits for the other window (Place does) while the other window waits
        // for its lines to be read. That was a start that stood still on the cover (2026-11-11).
        _ = Task.Run(() => WriteLoopAsync(down));
        _ = Task.Run(() => ReadLoopAsync(up));
    }

    const int PipeBuffer = 1 << 20;

    string SafeExitCode()
    {
        try
        {
            return _process!.ExitCode.ToString();
        }
        catch (InvalidOperationException)
        {
            return "?";
        }
    }

    /// <summary>A message for the interface (AppController's postJson); kept in order, sent on the writer's thread.</summary>
    public void Post(string json) => _out.Writer.TryWrite(json);

    async Task WriteLoopAsync(NamedPipeServerStream pipe)
    {
        await using var writer = new StreamWriter(pipe, new UTF8Encoding(false)) { AutoFlush = false, NewLine = "\n" };
        try
        {
            await foreach (string json in _out.Reader.ReadAllAsync(_stop.Token))
            {
                await writer.WriteLineAsync(json);
                if (!_out.Reader.TryPeek(out _))
                    await writer.FlushAsync();
            }
        }
        catch (Exception ex) when (ex is IOException or OperationCanceledException or ObjectDisposedException)
        {
            Lost($"bloxkeep-ui line closed: {ex.Message}");
        }
    }

    async Task ReadLoopAsync(NamedPipeServerStream pipe)
    {
        using var reader = new StreamReader(pipe, Encoding.UTF8);
        try
        {
            while (await reader.ReadLineAsync(_stop.Token) is { } line)
            {
                if (line.Length == 0)
                    continue;
                if (HelloOf(line) is { } hwnd)
                {
                    parent.BeginInvoke(() => Place(hwnd));
                    continue;
                }
                // As the page's messages: outside the reading, on the window's thread, so a command may open a modal window.
                parent.BeginInvoke(new Action(async () => await onMessage(line)));
            }
            Lost("bloxkeep-ui closed its line");
        }
        catch (Exception ex) when (ex is IOException or OperationCanceledException or ObjectDisposedException)
        {
            Lost($"bloxkeep-ui line closed: {ex.Message}");
        }
    }

    static IntPtr? HelloOf(string line)
    {
        if (!line.Contains("\"hello\"", StringComparison.Ordinal))
            return null;
        try
        {
            using var doc = JsonDocument.Parse(line);
            var root = doc.RootElement;
            return root.TryGetProperty("type", out var t) && t.ValueEquals("hello") && root.TryGetProperty("hwnd", out var h)
                ? new IntPtr(h.GetInt64())
                : null;
        }
        catch (JsonException)
        {
            return null;
        }
    }

    bool _lost;

    void Lost(string why)
    {
        if (_lost || _stop.IsCancellationRequested)
            return;
        _lost = true;
        Log.Note(why);
        if (!parent.IsDisposed)
            parent.BeginInvoke(() => Gone?.Invoke());
    }

    int _placeTries;

    /// <summary>Makes the GPUI window a child of the shell's client area, under the cover, and shows it.</summary>
    void Place(IntPtr hwnd)
    {
        if (parent.IsDisposed || !IsWindow(hwnd))
            return;
        // Restyling and moving another program's window waits for that window's thread. One that isn't answering (still
        // starting, or stuck) is asked again shortly, never waited on from here; after four seconds it counts as gone.
        if (SendMessageTimeout(hwnd, WM_NULL, IntPtr.Zero, IntPtr.Zero, SMTO_ABORTIFHUNG, 150, out _) == IntPtr.Zero)
        {
            if (++_placeTries > 40)
            {
                Lost("bloxkeep-ui's window didn't answer");
                return;
            }
            var again = new System.Windows.Forms.Timer { Interval = 100 };
            again.Tick += (_, _) =>
            {
                again.Dispose();
                if (!_stop.IsCancellationRequested)
                    Place(hwnd);
            };
            again.Start();
            return;
        }
        long style = GetWindowLongPtr(hwnd, GWL_STYLE).ToInt64();
        style = (style & ~(WS_POPUP | WS_CAPTION | WS_THICKFRAME | WS_SYSMENU | WS_MINIMIZEBOX | WS_MAXIMIZEBOX)) | WS_CHILD | WS_CLIPSIBLINGS;
        SetWindowLongPtr(hwnd, GWL_STYLE, new IntPtr(style));
        long ex = GetWindowLongPtr(hwnd, GWL_EXSTYLE).ToInt64();
        SetWindowLongPtr(hwnd, GWL_EXSTYLE, new IntPtr(ex & ~(WS_EX_APPWINDOW | WS_EX_DLGMODALFRAME | WS_EX_WINDOWEDGE | WS_EX_CLIENTEDGE)));
        SetParent(hwnd, parent.Handle);
        _window = hwnd;
        Fit();
        ShowWindow(hwnd, SW_SHOWNA);
        // The app is the one in front: what is typed goes to the interface from the start (its lock screen's box).
        if (GetForegroundWindow() == parent.Handle)
            SetFocus(hwnd);
    }

    /// <summary>Fills the shell's client area (call on every resize), below the shell's own controls (the cover).</summary>
    public void Fit()
    {
        if (_window == IntPtr.Zero || parent.IsDisposed)
            return;
        var area = parent.ClientRectangle;
        SetWindowPos(_window, HWND_BOTTOM, 0, 0, area.Width, area.Height, SWP_NOACTIVATE | SWP_FRAMECHANGED);
    }

    /// <summary>Gives the keyboard to the interface (after a click on the shell, or the window coming to the front).
    /// Not while it already has it, nor while another window of the app is in front (a dialog).</summary>
    public void Focus()
    {
        if (_window != IntPtr.Zero && !parent.IsDisposed && GetForegroundWindow() == parent.Handle && GetFocus() != _window)
            SetFocus(_window);
    }

    public void Dispose()
    {
        _stop.Cancel();
        _out.Writer.TryComplete();
        try
        {
            if (_process is { HasExited: false } p && !p.WaitForExit(1500))
                p.Kill();
        }
        catch (Exception ex) when (ex is InvalidOperationException or System.ComponentModel.Win32Exception)
        {
        }
        _process?.Dispose();
        _stop.Dispose();
    }

    /// <summary>Has Windows measure the window's frame again (after the title bar goes or comes back).</summary>
    public static void FrameChanged(IntPtr form) =>
        SetWindowPos(form, IntPtr.Zero, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED);

    /// <summary>How far a maximised window's frame reaches past the screen's edge, at the window's scale.</summary>
    public static int FrameThickness(IntPtr form)
    {
        uint dpi = GetDpiForWindow(form);
        return GetSystemMetricsForDpi(SM_CYSIZEFRAME, dpi) + GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi);
    }

    const int SM_CYSIZEFRAME = 33, SM_CXPADDEDBORDER = 92;
    const uint SWP_NOSIZE = 0x1, SWP_NOMOVE = 0x2, SWP_NOZORDER = 0x4;

    [DllImport("user32.dll")]
    static extern uint GetDpiForWindow(IntPtr hWnd);

    [DllImport("user32.dll")]
    static extern int GetSystemMetricsForDpi(int index, uint dpi);

    const int GWL_STYLE = -16, GWL_EXSTYLE = -20, SW_SHOWNA = 8;
    const long WS_CHILD = 0x40000000, WS_POPUP = 0x80000000, WS_CAPTION = 0x00C00000, WS_THICKFRAME = 0x00040000,
        WS_SYSMENU = 0x00080000, WS_MINIMIZEBOX = 0x00020000, WS_MAXIMIZEBOX = 0x00010000, WS_CLIPSIBLINGS = 0x04000000;
    const long WS_EX_APPWINDOW = 0x40000, WS_EX_DLGMODALFRAME = 0x1, WS_EX_WINDOWEDGE = 0x100, WS_EX_CLIENTEDGE = 0x200;
    const uint SWP_NOACTIVATE = 0x10, SWP_FRAMECHANGED = 0x20;
    static readonly IntPtr HWND_BOTTOM = new(1);

    [DllImport("user32.dll", EntryPoint = "GetWindowLongPtrW")]
    static extern IntPtr GetWindowLongPtr(IntPtr hWnd, int nIndex);

    [DllImport("user32.dll", EntryPoint = "SetWindowLongPtrW")]
    static extern IntPtr SetWindowLongPtr(IntPtr hWnd, int nIndex, IntPtr value);

    [DllImport("user32.dll")]
    static extern IntPtr SetParent(IntPtr child, IntPtr parent);

    [DllImport("user32.dll")]
    static extern bool SetWindowPos(IntPtr hWnd, IntPtr after, int x, int y, int cx, int cy, uint flags);

    [DllImport("user32.dll")]
    static extern bool ShowWindow(IntPtr hWnd, int cmd);

    [DllImport("user32.dll")]
    static extern bool IsWindow(IntPtr hWnd);

    [DllImport("user32.dll")]
    static extern IntPtr SetFocus(IntPtr hWnd);

    [DllImport("user32.dll")]
    static extern IntPtr GetForegroundWindow();

    [DllImport("user32.dll")]
    static extern IntPtr GetFocus();

    const uint WM_NULL = 0, SMTO_ABORTIFHUNG = 0x2;

    [DllImport("user32.dll")]
    static extern IntPtr SendMessageTimeout(IntPtr hWnd, uint msg, IntPtr wParam, IntPtr lParam, uint flags, uint timeout, out IntPtr result);
}
