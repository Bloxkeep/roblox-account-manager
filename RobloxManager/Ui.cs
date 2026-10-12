using System.Runtime.InteropServices;

namespace RobloxManager;

static class Ui
{
    public const string Title = "Bloxkeep";

    const int SHCNE_UPDATEITEM = 0x2000, SHCNE_ASSOCCHANGED = 0x08000000;
    const uint SHCNF_IDLIST = 0, SHCNF_PATHW = 5;

    [DllImport("shell32.dll", CharSet = CharSet.Unicode)]
    static extern void SHChangeNotify(int eventId, uint flags, string? item1, IntPtr item2);

    /// <summary>
    /// Makes Explorer redraw the icons of our shortcuts and exe. An update replaces the program folder the shortcuts
    /// point into, and Explorer can keep showing the blank icon it cached meanwhile (on the desktop and taskbar).
    /// </summary>
    public static void RefreshShellIcons()
    {
        string[] paths =
        [
            Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.DesktopDirectory), Title + ".lnk"),
            Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.Programs), Title + ".lnk"),
            Application.ExecutablePath,
        ];
        // Explorer handles these itself; never hold up the app's start for it.
        Task.Run(() =>
        {
            foreach (string path in paths.Where(File.Exists))
                SHChangeNotify(SHCNE_UPDATEITEM, SHCNF_PATHW, path, IntPtr.Zero);
            SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, null, IntPtr.Zero);
        });
    }
    static System.Windows.Forms.Timer? _clipboardTimer;

    public static Icon? AppIcon { get; } = LoadAppIcon();

    static Icon? LoadAppIcon()
    {
        using var stream = typeof(Ui).Assembly.GetManifestResourceStream("app.ico");
        return stream is null ? null : new Icon(stream);
    }

    [DllImport("dwmapi.dll")]
    static extern int DwmSetWindowAttribute(IntPtr hwnd, int attribute, ref int value, int size);

    /// <summary>Dark title bar tinted to match the page (caption colour is ignored before Windows 11).</summary>
    public static void UseDarkTitleBar(Form form, Color caption)
    {
        const int DWMWA_USE_IMMERSIVE_DARK_MODE = 20, DWMWA_CAPTION_COLOR = 35;
        int on = 1;
        DwmSetWindowAttribute(form.Handle, DWMWA_USE_IMMERSIVE_DARK_MODE, ref on, sizeof(int));
        int color = ColorTranslator.ToWin32(caption);
        DwmSetWindowAttribute(form.Handle, DWMWA_CAPTION_COLOR, ref color, sizeof(int));
    }

    public static void Error(IWin32Window? owner, string message) =>
        MessageBox.Show(owner, message, Title, MessageBoxButtons.OK, MessageBoxIcon.Error);

    public static void Info(IWin32Window? owner, string message) =>
        MessageBox.Show(owner, message, Title, MessageBoxButtons.OK, MessageBoxIcon.Information);

    /// <summary>
    /// Secrets are flagged so Windows keeps them out of clipboard history and cloud clipboard sync,
    /// and are wiped from the clipboard after 30 seconds.
    /// </summary>
    public static bool CopyToClipboard(string text, bool secret)
    {
        if (string.IsNullOrEmpty(text))
            return false;
        var data = new DataObject();
        data.SetData(DataFormats.UnicodeText, text);
        if (secret)
        {
            data.SetData("ExcludeClipboardContentFromMonitorProcessing", new MemoryStream(new byte[4]));
            data.SetData("CanIncludeInClipboardHistory", new MemoryStream(BitConverter.GetBytes(0)));
            data.SetData("CanUploadToCloudClipboard", new MemoryStream(BitConverter.GetBytes(0)));
        }
        try
        {
            Clipboard.SetDataObject(data, copy: true, retryTimes: 5, retryDelay: 100);
        }
        catch (ExternalException)
        {
            return false;
        }
        if (secret)
        {
            _clipboardTimer?.Dispose();
            _clipboardTimer = new System.Windows.Forms.Timer { Interval = 30_000 };
            _clipboardTimer.Tick += (_, _) =>
            {
                _clipboardTimer?.Dispose();
                _clipboardTimer = null;
                try
                {
                    if (Clipboard.ContainsText() && Clipboard.GetText() == text)
                        Clipboard.Clear();
                }
                catch (ExternalException)
                {
                }
            };
            _clipboardTimer.Start();
        }
        return true;
    }
}
