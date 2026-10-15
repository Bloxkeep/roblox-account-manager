// Bloxkeep's installer and uninstaller windows. Velopack still does the work underneath (copying the files, the
// shortcuts, the Installed apps entry, updates, removing it all), run silently by these windows:
//   Bloxkeep-Setup.exe       has Velopack's installer embedded: choose a folder (any drive) › progress › done.
//   BloxkeepUninstall.exe    the same program without it, shipped inside the app (current\) and set as the
//                            Installed apps uninstall command: confirm, optionally delete the accounts › progress › done.
// Accounts and settings live apart from the program, in %LOCALAPPDATA%\Bloxkeep. build.bat compiles this with the
// csc.exe that ships with Windows (.NET Framework 4.x, so C# 5): it runs on every Windows 10/11 as it is.
//
// Command line: --installto <folder> [--silent] installs without the window; the uninstaller takes --silent and
// --delete-data. --shot <png> --page <name> [--lang en|vi] saves a page as a picture (for the README).
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Globalization;
using System.IO;
using System.Reflection;
using System.Runtime.InteropServices;
using System.Text.RegularExpressions;
using System.Threading;
using System.Windows.Forms;
using Microsoft.Win32;

namespace BloxkeepSetup
{
    static class Program
    {
        [STAThread]
        static int Main(string[] args)
        {
            Application.EnableVisualStyles();
            Application.SetCompatibleTextRenderingDefault(false);
            T.Vi = PickVietnamese(Arg(args, "--lang"));
            bool silent = Has(args, "--silent");
            string shot = Arg(args, "--shot");
            // Started by the app for "Restart to update": the "Updating Bloxkeep" window (run from a copy in %TEMP%).
            string updating = Arg(args, "--updating");
            bool uninstall = updating == null && (Has(args, "--uninstall") || !Ops.HasPayload);

            if (uninstall && shot == null && Arg(args, "--root") == null)
            {
                // Windows › Apps runs the uninstaller inside the program folder, which is about to be deleted: carry on
                // from a copy in %TEMP%.
                string root = Ops.RootOf(Path.GetDirectoryName(Application.ExecutablePath)) ?? Ops.ExistingInstall();
                if (root == null)
                {
                    if (!silent)
                        Message(T.S("Bloxkeep isn't installed on this PC.", "Máy này chưa cài Bloxkeep."));
                    return 3;
                }
                return Ops.RelaunchFromTemp(root, args);
            }

            bool first;
            using (var only = new Mutex(true, @"Local\Bloxkeep.Setup", out first))
            using (var activate = new EventWaitHandle(false, EventResetMode.AutoReset, @"Local\Bloxkeep.Setup.Activate"))
            {
                // Started twice (Edge's download panel can, so can a double double-click): two installs at once would
                // trip over each other's files, so the second one just brings the first window to the front.
                if (!first)
                {
                    AllowSetForegroundWindow(-1);
                    activate.Set();
                    return 1;
                }
                Activate = activate;
                if (updating != null)
                {
                    var window = new SetupWindow(false, null, updating);
                    if (shot != null)
                        return window.SaveShot(shot, Arg(args, "--page"));
                    Application.Run(window);
                    Ops.RemoveTempCopy();
                    return 0;
                }
                if (uninstall)
                {
                    string root = Arg(args, "--root") ?? @"D:\Bloxkeep";
                    int code;
                    if (shot != null)
                        code = new SetupWindow(true, root).SaveShot(shot, Arg(args, "--page"));
                    else if (silent)
                        code = Ops.Uninstall(root, Has(args, "--delete-data"));
                    else
                    {
                        var window = new SetupWindow(true, root);
                        Application.Run(window);
                        code = window.Result;
                    }
                    Ops.RemoveTempCopy();
                    return code;
                }

                string target = Arg(args, "--installto");
                if (target != null)
                {
                    string error = Ops.Check(target, Ops.ExistingInstall());
                    if (error != null)
                    {
                        if (!silent)
                            Message(error);
                        return 2;
                    }
                    Ops.CloseRunningApp();
                    return Ops.Install(Ops.Full(target), null);
                }
                var form = new SetupWindow(false, null);
                if (shot != null)
                    return form.SaveShot(shot, Arg(args, "--page"));
                Application.Run(form);
                return form.Result;
            }
        }

        /// <summary>Set by a second copy of the program, to have this one's window come to the front.</summary>
        public static EventWaitHandle Activate;

        [DllImport("user32.dll")]
        static extern bool AllowSetForegroundWindow(int processId);

        static void Message(string text)
        {
            MessageBox.Show(text, "Bloxkeep", MessageBoxButtons.OK, MessageBoxIcon.Information);
        }

        /// <summary>The language Bloxkeep already uses on this PC, else Windows' display language.</summary>
        static bool PickVietnamese(string forced)
        {
            if (forced != null)
                return forced == "vi";
            try
            {
                string prefs = Path.Combine(Ops.DataDir, "prefs.json");
                if (File.Exists(prefs))
                {
                    var m = Regex.Match(File.ReadAllText(prefs), "\"language\"\\s*:\\s*\"(\\w+)\"");
                    if (m.Success)
                        return m.Groups[1].Value == "vi";
                }
            }
            catch (IOException) { }
            catch (UnauthorizedAccessException) { }
            return CultureInfo.CurrentUICulture.TwoLetterISOLanguageName == "vi";
        }

        public static string Arg(string[] args, string name)
        {
            for (int i = 0; i < args.Length - 1; i++)
                if (string.Equals(args[i], name, StringComparison.OrdinalIgnoreCase))
                    return args[i + 1];
            return null;
        }

        public static bool Has(string[] args, string name)
        {
            foreach (string a in args)
                if (string.Equals(a, name, StringComparison.OrdinalIgnoreCase))
                    return true;
            return false;
        }
    }

    /// <summary>English and Vietnamese, like the app.</summary>
    static class T
    {
        public static bool Vi;
        public static string S(string en, string vi) { return Vi ? vi : en; }
    }

    /// <summary>What the windows do: checks, running Velopack, the Installed apps entry.</summary>
    static class Ops
    {
        const string UninstallKey = @"Software\Microsoft\Windows\CurrentVersion\Uninstall\BloxkeepApp";
        const string UninstallerName = "BloxkeepUninstall.exe";
        public static readonly string LocalAppData = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
        public static readonly string DefaultDir = Path.Combine(LocalAppData, "BloxkeepApp");
        public static readonly string VelopackLog = Path.Combine(LocalAppData, "velopack", "velopack.log");

        /// <summary>Accounts and settings, as the app finds them: never inside the program folder, which uninstalling
        /// deletes whole.</summary>
        public static string DataDir
        {
            get
            {
                string custom = Environment.GetEnvironmentVariable("BLOXKEEP_DATA");
                return string.IsNullOrEmpty(custom) ? Path.Combine(LocalAppData, "Bloxkeep") : custom;
            }
        }

        public static bool HasData { get { return File.Exists(Path.Combine(DataDir, "vault.rmv")); } }

        public static bool HasPayload { get { return Assembly.GetExecutingAssembly().GetManifestResourceInfo("VelopackSetup.exe") != null; } }

        public static string Version
        {
            get
            {
                var v = (AssemblyInformationalVersionAttribute)Attribute.GetCustomAttribute(Assembly.GetExecutingAssembly(), typeof(AssemblyInformationalVersionAttribute));
                return v != null ? v.InformationalVersion : "";
            }
        }

        /// <summary>Program files, the package Velopack keeps for updates and room to unpack (build.bat measures them).</summary>
        public static long NeededBytes
        {
            get
            {
                foreach (AssemblyMetadataAttribute a in Attribute.GetCustomAttributes(Assembly.GetExecutingAssembly(), typeof(AssemblyMetadataAttribute)))
                {
                    long n;
                    if (a.Key == "InstallBytes" && long.TryParse(a.Value, out n))
                        return n;
                }
                return 240L * 1024 * 1024;
            }
        }

        /// <summary>Where Bloxkeep is installed now, if it is.</summary>
        public static string ExistingInstall()
        {
            using (var k = Registry.CurrentUser.OpenSubKey(UninstallKey))
            {
                string at = k == null ? null : k.GetValue("InstallLocation") as string;
                if (string.IsNullOrEmpty(at) || !IsBloxkeepFolder(at))
                    return null;
                return Full(at);
            }
        }

        public static bool IsBloxkeepFolder(string dir)
        {
            return File.Exists(Path.Combine(dir, "Update.exe")) && Directory.Exists(Path.Combine(dir, "current"));
        }

        /// <summary>The install folder when <paramref name="dir"/> is its "current" folder (where the app runs from).</summary>
        public static string RootOf(string dir)
        {
            string parent = Path.GetDirectoryName(dir);
            return parent != null && string.Equals(Path.GetFileName(dir), "current", StringComparison.OrdinalIgnoreCase) && File.Exists(Path.Combine(parent, "Update.exe"))
                ? parent : null;
        }

        public static string Full(string path)
        {
            string full = Path.GetFullPath(path.Trim().Trim('"'));
            return full.Length > 3 ? full.TrimEnd('\\') : full;
        }

        static bool Same(string a, string b) { return string.Equals(Full(a), Full(b), StringComparison.OrdinalIgnoreCase); }

        static bool Inside(string path, string folder) { return Full(path).StartsWith(Full(folder) + "\\", StringComparison.OrdinalIgnoreCase); }

        /// <summary>
        /// The program's own folder inside the one the person picked: picking D:\ or D:\Games gives D:\Bloxkeep or
        /// D:\Games\Bloxkeep, so uninstalling (which deletes the program folder) never touches anything else.
        /// </summary>
        public static string ProgramFolderIn(string picked)
        {
            string full = Full(picked);
            string name = Path.GetFileName(full);
            if (IsBloxkeepFolder(full) || ((name.Equals("Bloxkeep", StringComparison.OrdinalIgnoreCase) || name.Equals("BloxkeepApp", StringComparison.OrdinalIgnoreCase)) && !Same(full, DataDir)))
                return full;
            string inside = Path.Combine(full, "Bloxkeep");
            return Same(inside, DataDir) ? Path.Combine(full, "BloxkeepApp") : inside;
        }

        /// <summary>Why Bloxkeep can't be installed in this folder, or null when it can.</summary>
        public static string Check(string target, string existing)
        {
            string anyDrive = T.S("Choose a folder on one of this PC's own drives.", "Hãy chọn thư mục trên ổ đĩa của máy này.");
            string full;
            try
            {
                if (string.IsNullOrWhiteSpace(target) || !Path.IsPathRooted(target.Trim().Trim('"')))
                    return T.S("Choose a folder.", "Hãy chọn một thư mục.");
                full = Full(target);
            }
            catch (Exception ex)
            {
                if (ex is ArgumentException || ex is NotSupportedException || ex is PathTooLongException)
                    return T.S("That isn't a valid folder path.", "Đường dẫn thư mục không hợp lệ.");
                throw;
            }
            if (existing != null && !Same(full, existing))
                return string.Format(T.S("Bloxkeep is already installed in {0}. To move it, uninstall it first (your accounts can stay).",
                    "Bloxkeep đã được cài ở {0}. Muốn chuyển chỗ thì gỡ cài đặt trước (tài khoản có thể giữ lại)."), existing);
            if (full.StartsWith(@"\\", StringComparison.Ordinal))
                return anyDrive;
            if (Path.GetPathRoot(full).Length >= full.Length)
                return T.S("Choose a folder, not a whole drive.", "Hãy chọn một thư mục, không chọn cả ổ đĩa.");
            if (Same(full, DataDir) || Inside(full, DataDir) || Inside(DataDir, full))
                return T.S("That folder holds Bloxkeep's accounts and settings. Choose another one.", "Thư mục đó chứa tài khoản và cài đặt của Bloxkeep. Hãy chọn thư mục khác.");
            string windows = Environment.GetFolderPath(Environment.SpecialFolder.Windows);
            if (Same(full, windows) || Inside(full, windows))
                return T.S("Choose a folder outside Windows' own folder.", "Hãy chọn thư mục nằm ngoài thư mục Windows.");

            DriveInfo drive;
            try { drive = new DriveInfo(Path.GetPathRoot(full)); }
            catch (ArgumentException) { return anyDrive; }
            if (!drive.IsReady || (drive.DriveType != DriveType.Fixed && drive.DriveType != DriveType.Removable))
                return anyDrive;
            bool reinstall = Directory.Exists(full) && IsBloxkeepFolder(full);
            if (drive.AvailableFreeSpace < (reinstall ? NeededBytes / 4 : NeededBytes))
                return string.Format(T.S("Not enough free space on {0} (needs about {1}).", "Ổ {0} không đủ chỗ trống (cần khoảng {1})."), drive.Name.TrimEnd('\\'), Size(NeededBytes));
            if (Directory.Exists(full) && !reinstall && Directory.GetFileSystemEntries(full).Length > 0)
                return T.S("That folder already has other files in it. Choose an empty or new folder: uninstalling deletes the program folder.",
                    "Thư mục đó đã có file khác. Hãy chọn thư mục trống hoặc thư mục mới, vì gỡ cài đặt sẽ xóa cả thư mục chương trình.");
            if (!CanWrite(full))
                return T.S("Bloxkeep installs without admin rights, so it can't use this folder. Choose another one, for example D:\\Bloxkeep.",
                    "Bloxkeep cài không cần quyền admin nên không dùng được thư mục này. Hãy chọn thư mục khác, ví dụ D:\\Bloxkeep.");
            return null;
        }

        /// <summary>Tries a file in the folder (made for the test and removed again when it didn't exist).</summary>
        static bool CanWrite(string dir)
        {
            bool made = !Directory.Exists(dir);
            try
            {
                Directory.CreateDirectory(dir);
                string probe = Path.Combine(dir, ".bloxkeep-write-test");
                File.WriteAllText(probe, "");
                File.Delete(probe);
                return true;
            }
            catch (UnauthorizedAccessException) { return false; }
            catch (IOException) { return false; }
            finally
            {
                // Velopack must find no folder, else it treats it as an earlier install.
                if (made)
                    try { Directory.Delete(dir); } catch (IOException) { } catch (UnauthorizedAccessException) { }
            }
        }

        public static string Size(long bytes)
        {
            if (bytes >= 1024L * 1024 * 1024)
                return (bytes / (1024.0 * 1024 * 1024)).ToString("0.#", CultureInfo.CurrentCulture) + " GB";
            return (Math.Ceiling(bytes / (1024.0 * 1024 * 10)) * 10).ToString("0", CultureInfo.CurrentCulture) + " MB";
        }

        public static string FreeOn(string path)
        {
            try
            {
                string root = Path.GetPathRoot(Full(path));
                var d = new DriveInfo(root);
                if (d.IsReady)
                    return string.Format(T.S("{0} has {1} free.", "Ổ {0} còn trống {1}."), root.TrimEnd('\\'), Size(d.AvailableFreeSpace));
            }
            catch (Exception ex)
            {
                if (!(ex is ArgumentException || ex is NotSupportedException || ex is IOException || ex is UnauthorizedAccessException))
                    throw;
            }
            return "";
        }

        public static bool AppRunning()
        {
            Mutex single;
            if (!Mutex.TryOpenExisting(@"Local\Bloxkeep.SingleInstance", out single))
                return false;
            single.Dispose();
            return true;
        }

        /// <summary>Asks a running Bloxkeep to quit (the signal its own uninstall step uses) and waits until it has.</summary>
        public static void CloseRunningApp()
        {
            EventWaitHandle quit;
            if (!EventWaitHandle.TryOpenExisting(@"Local\Bloxkeep.Quit", out quit))
                return;
            using (quit)
                quit.Set();
            Mutex single;
            if (!Mutex.TryOpenExisting(@"Local\Bloxkeep.SingleInstance", out single))
                return;
            using (single)
            {
                try
                {
                    if (single.WaitOne(TimeSpan.FromSeconds(15)))
                        single.ReleaseMutex();
                }
                catch (AbandonedMutexException)
                {
                    single.ReleaseMutex();
                }
            }
            // Its browser processes end right after it.
            Thread.Sleep(1500);
        }

        /// <summary>
        /// Runs the embedded Velopack installer silently into the folder and returns its exit code; <paramref name="progress"/>
        /// gets 0–99 from how much has been written so far, or -1 while nothing has been yet.
        /// </summary>
        public static int Install(string target, Action<int> progress)
        {
            string dir = Path.Combine(Path.GetTempPath(), "Bloxkeep-Setup-" + Guid.NewGuid().ToString("N").Substring(0, 8));
            Directory.CreateDirectory(dir);
            try
            {
                string exe = Path.Combine(dir, "Bloxkeep-Install.exe");
                using (Stream s = Assembly.GetExecutingAssembly().GetManifestResourceStream("VelopackSetup.exe"))
                using (FileStream f = File.Create(exe))
                    s.CopyTo(f);
                var info = new ProcessStartInfo(exe, "--silent --installto \"" + target + "\"");
                info.UseShellExecute = false;
                info.WorkingDirectory = dir;
                int code;
                using (Process p = Process.Start(info))
                {
                    long expected = Math.Max(1, NeededBytes - 58L * 1024 * 1024);
                    while (!p.WaitForExit(200))
                        if (progress != null)
                        {
                            long have = SizeOf(target);
                            progress(have <= 0 ? -1 : (int)Math.Min(99, have * 100 / expected));
                        }
                    code = p.ExitCode;
                }
                if (code == 0)
                    UseOurUninstaller(target);
                return code;
            }
            finally
            {
                try { Directory.Delete(dir, true); } catch (IOException) { } catch (UnauthorizedAccessException) { }
            }
        }

        static long SizeOf(string dir)
        {
            long total = 0;
            try
            {
                if (Directory.Exists(dir))
                    foreach (string f in Directory.EnumerateFiles(dir, "*", SearchOption.AllDirectories))
                        try { total += new FileInfo(f).Length; } catch (IOException) { }
            }
            catch (IOException) { }
            catch (UnauthorizedAccessException) { }
            return total;
        }

        /// <summary>Installed apps › Uninstall opens Bloxkeep's uninstaller window instead of Velopack's bare one (the app
        /// sets this again at every start, in case an update rewrote it).</summary>
        static void UseOurUninstaller(string root)
        {
            string uninstaller = Path.Combine(root, "current", UninstallerName);
            if (!File.Exists(uninstaller))
                return;
            using (var k = Registry.CurrentUser.OpenSubKey(UninstallKey, true))
            {
                if (k == null)
                    return;
                k.SetValue("UninstallString", "\"" + uninstaller + "\"");
                k.SetValue("QuietUninstallString", "\"" + uninstaller + "\" --silent");
            }
        }

        public static int RelaunchFromTemp(string root, string[] args)
        {
            string dir = Path.Combine(Path.GetTempPath(), "Bloxkeep-Uninstall-" + Guid.NewGuid().ToString("N").Substring(0, 8));
            Directory.CreateDirectory(dir);
            string copy = Path.Combine(dir, UninstallerName);
            File.Copy(Application.ExecutablePath, copy);
            string line = "--uninstall --root \"" + root + "\" --lang " + (T.Vi ? "vi" : "en");
            if (Program.Has(args, "--silent"))
                line += " --silent";
            if (Program.Has(args, "--delete-data"))
                line += " --delete-data";
            Process.Start(new ProcessStartInfo(copy, line) { UseShellExecute = false, WorkingDirectory = dir }).Dispose();
            return 0;
        }

        /// <summary>The copy in %TEMP% can't delete itself while it runs: a moment after it ends, cmd does.</summary>
        public static void RemoveTempCopy()
        {
            string dir = Path.GetDirectoryName(Application.ExecutablePath);
            string name = Path.GetFileName(dir);
            if (!(name.StartsWith("Bloxkeep-Uninstall-", StringComparison.Ordinal) || name.StartsWith("Bloxkeep-Updating-", StringComparison.Ordinal))
                || !Same(Path.GetDirectoryName(dir), Path.GetTempPath()))
                return;
            var info = new ProcessStartInfo("cmd.exe", "/C choice /C Y /N /D Y /T 3 >NUL & rmdir /s /q \"" + dir + "\"");
            info.UseShellExecute = false;
            info.CreateNoWindow = true;
            info.WorkingDirectory = Path.GetTempPath();
            Process.Start(info).Dispose();
        }

        /// <summary>
        /// Closes Bloxkeep and has Velopack remove the program (folder, shortcuts, Installed apps entry). The app's own
        /// uninstall step reads BLOXKEEP_UNINSTALL_DATA and deletes the accounts and settings too when asked.
        /// </summary>
        public static int Uninstall(string root, bool deleteData)
        {
            CloseRunningApp();
            var info = new ProcessStartInfo(Path.Combine(root, "Update.exe"), "--uninstall --silent");
            info.UseShellExecute = false;
            info.CreateNoWindow = true;
            info.WorkingDirectory = Path.GetTempPath();
            info.EnvironmentVariables["BLOXKEEP_UNINSTALL_DATA"] = deleteData ? "delete" : "keep";
            int code;
            using (Process p = Process.Start(info))
            {
                if (!p.WaitForExit(180000))
                    return -2;
                code = p.ExitCode;
            }
            // Velopack removes the folder itself a few seconds after it ends.
            for (int i = 0; i < 40 && Directory.Exists(root); i++)
                Thread.Sleep(250);
            return code;
        }
    }

    /// <summary>The installer's colours: those of the splash screen and the app's dark theme.</summary>
    static class Theme
    {
        public static readonly Color Bg = Color.FromArgb(21, 24, 30);
        public static readonly Color Footer = Color.FromArgb(16, 18, 23);
        public static readonly Color Field = Color.FromArgb(29, 33, 41);
        public static readonly Color FieldHover = Color.FromArgb(38, 43, 53);
        public static readonly Color Line = Color.FromArgb(48, 54, 66);
        public static readonly Color Text = Color.FromArgb(233, 236, 243);
        public static readonly Color Muted = Color.FromArgb(150, 156, 173);
        public static readonly Color Accent = Color.FromArgb(245, 201, 51);
        public static readonly Color AccentHover = Color.FromArgb(255, 216, 87);
        public static readonly Color Ink = Color.FromArgb(27, 22, 0);
        public static readonly Color Danger = Color.FromArgb(229, 72, 77);
        public static readonly Color DangerHover = Color.FromArgb(242, 92, 97);
        public static readonly Color DangerText = Color.FromArgb(255, 138, 142);
        public static readonly Color Ok = Color.FromArgb(52, 211, 153);

        public static GraphicsPath Round(RectangleF r, float radius)
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

        public static Color Mix(Color a, Color b, float t)
        {
            return Color.FromArgb((int)(a.R + (b.R - a.R) * t), (int)(a.G + (b.G - a.G) * t), (int)(a.B + (b.B - a.B) * t));
        }
    }

    /// <summary>A rounded, flat button drawn in the theme's colours.</summary>
    sealed class Btn : Control, IButtonControl
    {
        Color _fill, _hoverFill, _ink;
        bool _hover, _down;
        public DialogResult DialogResult { get; set; }

        public Btn(Color fill, Color hoverFill, Color ink)
        {
            SetStyle(ControlStyles.AllPaintingInWmPaint | ControlStyles.OptimizedDoubleBuffer | ControlStyles.UserPaint | ControlStyles.ResizeRedraw | ControlStyles.Selectable | ControlStyles.StandardClick, true);
            Colors(fill, hoverFill, ink);
            Cursor = Cursors.Hand;
            TabStop = true;
        }

        public void Colors(Color fill, Color hoverFill, Color ink)
        {
            _fill = fill;
            _hoverFill = hoverFill;
            _ink = ink;
            Invalidate();
        }

        public void NotifyDefault(bool value) { }

        public void PerformClick()
        {
            if (Enabled && Visible)
                OnClick(EventArgs.Empty);
        }

        protected override bool IsInputKey(Keys key) { return key == Keys.Enter || key == Keys.Space || base.IsInputKey(key); }

        protected override void OnKeyDown(KeyEventArgs e)
        {
            if (e.KeyCode == Keys.Enter || e.KeyCode == Keys.Space)
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
        protected override void OnEnabledChanged(EventArgs e) { Invalidate(); base.OnEnabledChanged(e); }
        protected override void OnTextChanged(EventArgs e) { Invalidate(); base.OnTextChanged(e); }

        protected override void OnPaint(PaintEventArgs e)
        {
            Graphics g = e.Graphics;
            g.Clear(Parent != null ? Parent.BackColor : Theme.Bg);
            g.SmoothingMode = SmoothingMode.AntiAlias;
            Color fill = !Enabled ? Theme.Mix(_fill, Parent != null ? Parent.BackColor : Theme.Bg, .45f) : _hover || _down ? _hoverFill : _fill;
            float radius = Height * .22f;
            using (GraphicsPath path = Theme.Round(new RectangleF(.5f, .5f, Width - 1.5f, Height - 1.5f), radius))
            using (var brush = new SolidBrush(fill))
                g.FillPath(brush, path);
            if (Focused && ShowFocusCues)
                using (GraphicsPath ring = Theme.Round(new RectangleF(1.5f, 1.5f, Width - 3.5f, Height - 3.5f), radius - 1))
                using (var pen = new Pen(Theme.Mix(_ink, fill, .35f), 1.5f))
                    g.DrawPath(pen, ring);
            TextRenderer.DrawText(g, Text, Font, ClientRectangle, Enabled ? _ink : Theme.Mix(_ink, fill, .5f), fill,
                TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter | TextFormatFlags.SingleLine);
        }
    }

    /// <summary>A checkbox drawn in the theme's colours, the whole line clickable.</summary>
    sealed class Check : Control
    {
        bool _checked, _hover;
        public event EventHandler Changed;
        public Color Box = Theme.Accent;

        public Check()
        {
            SetStyle(ControlStyles.AllPaintingInWmPaint | ControlStyles.OptimizedDoubleBuffer | ControlStyles.UserPaint | ControlStyles.ResizeRedraw | ControlStyles.Selectable | ControlStyles.StandardClick, true);
            Cursor = Cursors.Hand;
            TabStop = true;
        }

        public bool Checked
        {
            get { return _checked; }
            set
            {
                if (_checked == value)
                    return;
                _checked = value;
                Invalidate();
                if (Changed != null)
                    Changed(this, EventArgs.Empty);
            }
        }

        protected override void OnClick(EventArgs e) { Focus(); Checked = !Checked; base.OnClick(e); }

        protected override void OnKeyDown(KeyEventArgs e)
        {
            if (e.KeyCode == Keys.Space)
                Checked = !Checked;
            base.OnKeyDown(e);
        }

        protected override void OnMouseEnter(EventArgs e) { _hover = true; Invalidate(); base.OnMouseEnter(e); }
        protected override void OnMouseLeave(EventArgs e) { _hover = false; Invalidate(); base.OnMouseLeave(e); }
        protected override void OnGotFocus(EventArgs e) { Invalidate(); base.OnGotFocus(e); }
        protected override void OnLostFocus(EventArgs e) { Invalidate(); base.OnLostFocus(e); }
        protected override void OnTextChanged(EventArgs e) { Invalidate(); base.OnTextChanged(e); }

        protected override void OnPaint(PaintEventArgs e)
        {
            Graphics g = e.Graphics;
            g.Clear(Parent != null ? Parent.BackColor : Theme.Bg);
            g.SmoothingMode = SmoothingMode.AntiAlias;
            float size = Font.Height * 1.05f, top = (Height - size) / 2;
            var box = new RectangleF(1, top, size, size);
            using (GraphicsPath path = Theme.Round(box, size * .22f))
            {
                if (_checked)
                    using (var b = new SolidBrush(Box))
                        g.FillPath(b, path);
                else
                {
                    using (var b = new SolidBrush(_hover ? Theme.FieldHover : Theme.Field))
                        g.FillPath(b, path);
                    using (var pen = new Pen(Focused && ShowFocusCues ? Theme.Muted : Theme.Line, 1.2f))
                        g.DrawPath(pen, path);
                }
            }
            if (_checked)
                using (var pen = new Pen(Theme.Ink, size * .13f))
                {
                    pen.StartCap = pen.EndCap = LineCap.Round;
                    pen.LineJoin = LineJoin.Round;
                    g.DrawLines(pen, new[] {
                        new PointF(box.X + size * .26f, top + size * .52f),
                        new PointF(box.X + size * .43f, top + size * .69f),
                        new PointF(box.X + size * .75f, top + size * .33f) });
                }
            var text = new Rectangle((int)(size + size * .6f), 0, Width - (int)(size * 1.6f), Height);
            TextRenderer.DrawText(g, Text, Font, text, ForeColor, TextFormatFlags.VerticalCenter | TextFormatFlags.SingleLine | TextFormatFlags.EndEllipsis);
            if (Focused && ShowFocusCues)
                using (var pen = new Pen(Theme.Line) { DashStyle = DashStyle.Dot })
                    g.DrawRectangle(pen, text.X - 2, 1, Math.Min(text.Width, TextRenderer.MeasureText(Text, Font).Width) + 3, Height - 3);
        }
    }

    /// <summary>The progress bar: a share filled, or a moving stripe while the length is unknown.</summary>
    sealed class Bar : Control
    {
        int _value = -1;
        float _phase;
        readonly System.Windows.Forms.Timer _tick = new System.Windows.Forms.Timer();

        public Bar()
        {
            SetStyle(ControlStyles.AllPaintingInWmPaint | ControlStyles.OptimizedDoubleBuffer | ControlStyles.UserPaint | ControlStyles.ResizeRedraw, true);
            _tick.Interval = 30;
            _tick.Tick += delegate { _phase = (_phase + .018f) % 1.4f; Invalidate(); };
            _tick.Start();
        }

        /// <summary>0–100, or -1 for "working, length unknown".</summary>
        public int Value
        {
            get { return _value; }
            set
            {
                _value = value;
                _tick.Enabled = value < 0;
                Invalidate();
            }
        }

        protected override void Dispose(bool disposing)
        {
            if (disposing)
                _tick.Dispose();
            base.Dispose(disposing);
        }

        protected override void OnPaint(PaintEventArgs e)
        {
            Graphics g = e.Graphics;
            g.Clear(Parent != null ? Parent.BackColor : Theme.Bg);
            g.SmoothingMode = SmoothingMode.AntiAlias;
            float r = Height / 2f;
            using (GraphicsPath track = Theme.Round(new RectangleF(0, 0, Width - 1, Height - 1), r))
            using (var b = new SolidBrush(Theme.Field))
                g.FillPath(b, track);
            RectangleF fill;
            if (_value >= 0)
                fill = new RectangleF(0, 0, Math.Max(Height, (Width - 1) * _value / 100f), Height - 1);
            else
            {
                float w = Width * .3f, x = (Width + w) * _phase - w;
                fill = RectangleF.Intersect(new RectangleF(x, 0, w, Height - 1), new RectangleF(0, 0, Width - 1, Height - 1));
            }
            if (fill.Width >= 1)
                using (GraphicsPath path = Theme.Round(fill, Math.Min(r, fill.Width / 2)))
                using (var b = new SolidBrush(Theme.Accent))
                    g.FillPath(b, path);
        }
    }

    /// <summary>The installer and uninstaller window: one page at a time.</summary>
    sealed class SetupWindow : Form
    {
        public int Result = 1;
        readonly bool _uninstall;
        readonly string _root;
        readonly string _existing;
        readonly float _k;
        readonly Panel _body = new Panel();
        readonly Panel _footer = new Panel();
        string _page, _target, _error;
        bool _deleteData, _openApp = true;
        Bar _bar;
        Label _status;
        TextBox _path;
        Label _space, _problem;

        /// <summary>Drawn to a picture (--shot): nothing is watched or run.</summary>
        bool _shooting;

        /// <summary>The version "Restart to update" is installing (the "Updating Bloxkeep" window), else null.</summary>
        readonly string _updatingTo;

        public SetupWindow(bool uninstall, string root, string updatingTo = null)
        {
            _uninstall = uninstall;
            _root = root;
            _updatingTo = updatingTo;
            // The screen's scale, read without creating the window: a window made this early ignores StartPosition
            // and opens in Windows' default corner.
            using (Graphics g = Graphics.FromHwnd(IntPtr.Zero))
                _k = g.DpiX / 96f;
            Font = new Font("Segoe UI", 9.75f);
            AutoScaleMode = AutoScaleMode.None;
            FormBorderStyle = FormBorderStyle.FixedSingle;
            MaximizeBox = false;
            StartPosition = FormStartPosition.CenterScreen;
            BackColor = Theme.Bg;
            ForeColor = Theme.Text;
            Icon = AppIcon(S(32));
            ClientSize = new Size(S(600), S(400));
            _footer.Dock = DockStyle.Bottom;
            _footer.Height = S(68);
            _footer.BackColor = Theme.Footer;
            _footer.Paint += delegate (object o, PaintEventArgs e)
            {
                using (var pen = new Pen(Theme.Line))
                    e.Graphics.DrawLine(pen, 0, 0, _footer.Width, 0);
            };
            _body.Dock = DockStyle.Fill;
            _body.BackColor = Theme.Bg;
            Controls.Add(_body);
            Controls.Add(_footer);
            if (updatingTo != null)
            {
                // Only a moment on screen: no buttons, a smaller window.
                _footer.Visible = false;
                ClientSize = new Size(S(600), S(214));
                ShowPage("updating");
                return;
            }
            if (!uninstall)
            {
                _existing = Ops.ExistingInstall();
                _target = _existing ?? Ops.DefaultDir;
            }
            ShowPage(uninstall ? "uninstall" : "choose");
        }

        int S(float px) { return (int)Math.Round(px * _k); }

        protected override void OnHandleCreated(EventArgs e)
        {
            base.OnHandleCreated(e);
            // A dark title bar to go with the window (Windows 10 2004+ and 11; ignored elsewhere).
            int on = 1;
            if (DwmSetWindowAttribute(Handle, 20, ref on, 4) != 0)
                DwmSetWindowAttribute(Handle, 19, ref on, 4);
        }

        protected override void OnLoad(EventArgs e)
        {
            base.OnLoad(e);
            // In the middle of the screen the mouse is on (with several screens, the one the person is looking at).
            if (StartPosition == FormStartPosition.CenterScreen)
            {
                Rectangle area = Screen.FromPoint(Cursor.Position).WorkingArea;
                Location = new Point(area.Left + (area.Width - Width) / 2, area.Top + (area.Height - Height) / 2);
            }
        }

        protected override void OnShown(EventArgs e)
        {
            base.OnShown(e);
            if (_page == "updating" && !_shooting)
                WatchForNewVersion();
            if (Program.Activate != null)
                _activateWait = ThreadPool.RegisterWaitForSingleObject(Program.Activate, delegate
                {
                    try { BeginInvoke((MethodInvoker)BringUp); }
                    catch (InvalidOperationException) { } // Closing.
                }, null, Timeout.Infinite, false);
        }

        RegisteredWaitHandle _activateWait;

        void BringUp()
        {
            if (WindowState == FormWindowState.Minimized)
                WindowState = FormWindowState.Normal;
            Activate();
        }

        protected override void OnFormClosing(FormClosingEventArgs e)
        {
            // Not while Velopack is busy: closing then would leave a half-done install.
            if (_page == "installing" || _page == "removing")
                e.Cancel = true;
            else if (_activateWait != null)
                _activateWait.Unregister(null);
            base.OnFormClosing(e);
        }

        [DllImport("dwmapi.dll")]
        static extern int DwmSetWindowAttribute(IntPtr hwnd, int attribute, ref int value, int size);

        [DllImport("user32.dll")]
        static extern bool DrawIconEx(IntPtr hdc, int x, int y, IntPtr icon, int width, int height, int step, IntPtr brush, int flags);

        /// <summary>Bloxkeep's icon at this size, from the .ico embedded by build.bat (it has every size).</summary>
        static Icon AppIcon(int size)
        {
            using (Stream s = Assembly.GetExecutingAssembly().GetManifestResourceStream("app.ico"))
                return s != null ? new Icon(s, size, size) : new Icon(SystemIcons.Application, size, size);
        }

        // ---- building blocks -------------------------------------------------------------------------------------

        void ShowPage(string page)
        {
            _page = page;
            Text = _updatingTo != null ? "Bloxkeep" : _uninstall ? T.S("Uninstall Bloxkeep", "Gỡ cài đặt Bloxkeep") : T.S("Bloxkeep Setup", "Cài đặt Bloxkeep");
            SuspendLayout();
            foreach (Control c in new List<Control>(Ctrls(_body)))
                c.Dispose();
            foreach (Control c in new List<Control>(Ctrls(_footer)))
                c.Dispose();
            _path = null;
            _bar = null;
            _status = _space = _problem = null;
            AcceptButton = CancelButton = null;
            switch (page)
            {
                case "choose": Choose(); break;
                case "installing": Installing(); break;
                case "installed": Installed(); break;
                case "failed": Failed(); break;
                case "uninstall": Confirm(); break;
                case "removing": Removing(); break;
                case "removed": Removed(); break;
                case "updating": Updating(); break;
                case "updateSlow": UpdateSlow(); break;
            }
            ResumeLayout();
        }

        static IEnumerable<Control> Ctrls(Control parent)
        {
            foreach (Control c in parent.Controls)
                yield return c;
        }

        /// <summary>The top of every page: an icon, the title and a line under it, and the language switch.</summary>
        void Header(string icon, string title, string sub, bool language)
        {
            var pic = new PictureBox();
            pic.SetBounds(S(32), S(32), S(44), S(44));
            pic.Image = icon == "logo" ? Logo(S(44)) : Mark(S(44), icon);
            _body.Controls.Add(pic);
            Add(new Label { Text = title, Font = new Font("Segoe UI Semibold", 15f), ForeColor = Theme.Text, AutoSize = true, Location = new Point(S(90), S(30)) });
            Add(new Label { Text = sub, ForeColor = Theme.Muted, AutoSize = true, Location = new Point(S(92), S(60)) });
            if (!language)
                return;
            var lang = new LinkLabel { Text = T.Vi ? "English" : "Tiếng Việt", AutoSize = true, LinkBehavior = LinkBehavior.HoverUnderline };
            lang.LinkColor = lang.VisitedLinkColor = Theme.Muted;
            lang.ActiveLinkColor = Theme.Text;
            lang.LinkClicked += delegate { T.Vi = !T.Vi; ShowPage(_page); };
            Add(lang);
            lang.Location = new Point(ClientSize.Width - S(32) - lang.PreferredWidth, S(36));
        }

        Label Para(string text, int y, Color color)
        {
            var l = new Label { Text = text, ForeColor = color, AutoSize = true, MaximumSize = new Size(ClientSize.Width - S(64), 0), Location = new Point(S(32), y) };
            Add(l);
            return l;
        }

        void Add(Control c) { _body.Controls.Add(c); }

        Btn Button(string text, bool primary, EventHandler click)
        {
            var b = primary ? new Btn(Theme.Accent, Theme.AccentHover, Theme.Ink) : new Btn(Theme.Field, Theme.FieldHover, Theme.Text);
            b.Text = text;
            b.Font = new Font("Segoe UI Semibold", 9.75f);
            b.Size = new Size(Math.Max(S(104), TextRenderer.MeasureText(text, b.Font).Width + S(36)), S(36));
            b.Click += click;
            // Right-aligned, in the order added (the first one rightmost: Cancel, then the main button left of it).
            int right = ClientSize.Width - S(32);
            foreach (Control c in _footer.Controls)
                right = Math.Min(right, c.Left - S(10));
            b.Location = new Point(right - b.Width, (_footer.Height - b.Height) / 2);
            _footer.Controls.Add(b);
            if (primary)
                AcceptButton = b;
            return b;
        }

        Bitmap Logo(int size)
        {
            // Drawn by Windows itself: .NET Framework's own icon drawing fails on the .ico's PNG-compressed sizes.
            var bmp = new Bitmap(size, size);
            using (Icon icon = AppIcon(size))
            using (Graphics g = Graphics.FromImage(bmp))
            {
                g.Clear(Theme.Bg);
                IntPtr hdc = g.GetHdc();
                try { DrawIconEx(hdc, 0, 0, icon.Handle, size, size, 0, IntPtr.Zero, 3); }
                finally { g.ReleaseHdc(hdc); }
            }
            return bmp;
        }

        /// <summary>A round "done" tick or "problem" mark.</summary>
        static Bitmap Mark(int size, string kind)
        {
            var bmp = new Bitmap(size, size);
            using (Graphics g = Graphics.FromImage(bmp))
            {
                g.Clear(Theme.Bg);
                g.SmoothingMode = SmoothingMode.AntiAlias;
                Color c = kind == "ok" ? Theme.Ok : Theme.Danger;
                using (var b = new SolidBrush(Color.FromArgb(40, c)))
                    g.FillEllipse(b, 1, 1, size - 3, size - 3);
                using (var pen = new Pen(c, size * .07f))
                {
                    pen.StartCap = pen.EndCap = LineCap.Round;
                    pen.LineJoin = LineJoin.Round;
                    g.DrawEllipse(pen, size * .06f, size * .06f, size * .86f, size * .86f);
                    if (kind == "ok")
                        g.DrawLines(pen, new[] { new PointF(size * .3f, size * .52f), new PointF(size * .44f, size * .65f), new PointF(size * .7f, size * .37f) });
                    else
                    {
                        g.DrawLine(pen, size * .49f, size * .3f, size * .49f, size * .55f);
                        g.DrawLine(pen, size * .49f, size * .69f, size * .49f, size * .7f);
                    }
                }
            }
            return bmp;
        }

        static string VersionLine { get { return T.S("Version ", "Phiên bản ") + Ops.Version; } }

        // ---- installer pages -------------------------------------------------------------------------------------

        void Choose()
        {
            bool again = _existing != null;
            Header("logo", again ? T.S("Reinstall Bloxkeep", "Cài lại Bloxkeep") : T.S("Install Bloxkeep", "Cài đặt Bloxkeep"),
                T.S("Roblox account manager", "Quản lý tài khoản Roblox") + " · " + VersionLine, true);
            Add(new Label { Text = T.S("Install location", "Nơi cài đặt"), Font = new Font("Segoe UI Semibold", 9.75f), ForeColor = Theme.Text, AutoSize = true, Location = new Point(S(32), S(108)) });

            var field = new Panel { BackColor = Theme.Field, Bounds = new Rectangle(S(32), S(134), ClientSize.Width - S(64) - S(118), S(38)), Padding = new Padding(S(12), S(9), S(10), 0) };
            field.Paint += delegate (object o, PaintEventArgs e)
            {
                using (var pen = new Pen(Theme.Line))
                    e.Graphics.DrawRectangle(pen, 0, 0, field.Width - 1, field.Height - 1);
            };
            _path = new TextBox { BorderStyle = BorderStyle.None, BackColor = Theme.Field, ForeColor = Theme.Text, Dock = DockStyle.Fill, Text = _target, ReadOnly = again };
            field.Controls.Add(_path);
            Add(field);
            var browse = new Btn(Theme.Field, Theme.FieldHover, Theme.Text) { Text = T.S("Browse…", "Chọn…"), Bounds = new Rectangle(ClientSize.Width - S(32) - S(108), S(134), S(108), S(38)), Enabled = !again };
            browse.Font = new Font("Segoe UI Semibold", 9.75f);
            browse.Click += delegate { Browse(); };
            Add(browse);

            _space = Para("", S(182), Theme.Muted);
            var note = Para(again
                ? T.S("Bloxkeep is already installed here, so this repairs or updates it. To move it to another drive, uninstall it first; your accounts can stay.",
                    "Bloxkeep đã được cài ở đây, cài lại sẽ sửa hoặc cập nhật nó. Muốn chuyển sang ổ khác thì gỡ cài đặt trước; tài khoản có thể giữ lại.")
                : T.S("Pick any drive. Your accounts and settings are kept apart from the program, in AppData, so they stay when you uninstall or reinstall it.",
                    "Chọn ổ nào cũng được. Tài khoản và cài đặt được lưu riêng với chương trình, trong AppData, nên gỡ hay cài lại vẫn còn."), S(212), Theme.Muted);
            _problem = Para(_error ?? "", note.Bottom + S(12), Theme.DangerText);
            _error = null;
            _path.TextChanged += delegate { _target = _path.Text; ShowSpace(); _problem.Text = ""; };
            ShowSpace();

            CancelButton = Button(T.S("Cancel", "Hủy"), false, delegate { Close(); });
            Button(again ? T.S("Reinstall", "Cài lại") : T.S("Install", "Cài đặt"), true, delegate { Install(); });
        }

        void ShowSpace()
        {
            if (_space != null && _path != null)
                _space.Text = string.Format(T.S("Needs about {0}.", "Cần khoảng {0}."), Ops.Size(Ops.NeededBytes)) + " " + Ops.FreeOn(_path.Text);
        }

        void Browse()
        {
            string start = _path.Text;
            while (!string.IsNullOrEmpty(start) && !Directory.Exists(start))
                start = Path.GetDirectoryName(start);
            string picked = FolderPicker.Pick(this, T.S("Choose where to install Bloxkeep", "Chọn nơi cài Bloxkeep"), start);
            if (picked != null)
                _path.Text = Ops.ProgramFolderIn(picked);
        }

        void Install()
        {
            string error = Ops.Check(_path.Text, _existing);
            if (error != null)
            {
                _problem.Text = error;
                return;
            }
            if (Ops.AppRunning() && MessageBox.Show(this,
                    T.S("Bloxkeep is running. Setup will close it first.", "Bloxkeep đang chạy. Bộ cài sẽ đóng nó trước."),
                    Text, MessageBoxButtons.OKCancel, MessageBoxIcon.Information) != DialogResult.OK)
                return;
            string target = Ops.Full(_path.Text);
            _target = target;
            ShowPage("installing");
            Work(delegate
            {
                Ops.CloseRunningApp();
                return Ops.Install(target, delegate (int percent) { BeginInvoke((MethodInvoker)delegate { Progress(percent); }); });
            }, delegate (int code)
            {
                if (code == 0)
                    ShowPage("installed");
                else
                {
                    _error = string.Format(T.S("The installer stopped with code {0}. The setup log below says why. If Bloxkeep was open, close it and try again.",
                        "Trình cài dừng lại với mã {0}. Nhật ký cài đặt bên dưới ghi lý do. Nếu Bloxkeep đang mở, hãy đóng nó rồi thử lại."), code);
                    ShowPage("failed");
                }
            });
        }

        void Installing()
        {
            Header("logo", T.S("Installing Bloxkeep", "Đang cài Bloxkeep"), _target, false);
            _status = Para(T.S("Getting ready…", "Đang chuẩn bị…"), S(118), Theme.Text);
            _bar = new Bar { Bounds = new Rectangle(S(32), S(146), ClientSize.Width - S(64), S(8)), Value = -1 };
            Add(_bar);
            Para(T.S("This takes a few seconds. Microsoft Edge WebView2 is installed first if this PC lacks it, which takes longer.",
                "Việc này mất vài giây. Nếu máy chưa có Microsoft Edge WebView2 thì nó được cài trước, sẽ lâu hơn một chút."), S(172), Theme.Muted);
        }

        void Progress(int percent)
        {
            if (_bar == null || _status == null)
                return;
            _bar.Value = percent;
            _status.Text = percent < 0 ? T.S("Getting ready…", "Đang chuẩn bị…") : string.Format(T.S("Copying files… {0}%", "Đang chép file… {0}%"), percent);
        }

        void Installed()
        {
            Header("ok", T.S("Bloxkeep is installed", "Đã cài xong Bloxkeep"), _target, true);
            Para(T.S("Open it from the shortcut on your Desktop or in the Start menu. It keeps itself up to date.",
                "Mở bằng shortcut trên Desktop hoặc trong Start menu. App tự cập nhật."), S(112), Theme.Text);
            var open = new Check { Text = T.S("Open Bloxkeep now", "Mở Bloxkeep ngay"), ForeColor = Theme.Text, Bounds = new Rectangle(S(30), S(160), S(420), S(28)) };
            open.Checked = _openApp;
            open.Changed += delegate { _openApp = open.Checked; };
            Add(open);
            Button(T.S("Finish", "Xong"), true, delegate
            {
                Result = 0;
                if (_openApp)
                    try { Process.Start(new ProcessStartInfo(Path.Combine(_target, "Bloxkeep.exe")) { WorkingDirectory = _target }).Dispose(); }
                    catch (System.ComponentModel.Win32Exception) { }
                Close();
            });
        }

        void Failed()
        {
            Header("problem", _uninstall ? T.S("Bloxkeep wasn't removed", "Chưa gỡ được Bloxkeep") : T.S("Setup didn't finish", "Cài đặt chưa xong"), _uninstall ? _root : _target, true);
            var text = Para(_error ?? "", S(112), Theme.Text);
            var log = new LinkLabel { Text = T.S("Open the setup log", "Mở nhật ký cài đặt"), AutoSize = true, LinkBehavior = LinkBehavior.HoverUnderline, Location = new Point(S(32), text.Bottom + S(14)) };
            log.LinkColor = log.VisitedLinkColor = Theme.Accent;
            log.ActiveLinkColor = Theme.AccentHover;
            log.LinkClicked += delegate
            {
                try { Process.Start(new ProcessStartInfo("notepad.exe", "\"" + Ops.VelopackLog + "\"")).Dispose(); }
                catch (System.ComponentModel.Win32Exception) { }
            };
            Add(log);
            Para(Ops.VelopackLog, log.Bottom + S(6), Theme.Muted);
            CancelButton = Button(T.S("Close", "Đóng"), _uninstall, delegate { Close(); });
            if (!_uninstall)
                Button(T.S("Try again", "Thử lại"), true, delegate { _error = null; ShowPage("choose"); });
        }

        // ---- uninstaller pages -----------------------------------------------------------------------------------

        void Confirm()
        {
            Header("logo", T.S("Uninstall Bloxkeep", "Gỡ cài đặt Bloxkeep"), VersionLine, true);
            var what = Para(string.Format(T.S("This removes Bloxkeep from {0}, with its shortcuts and its Installed apps entry.",
                "Bloxkeep sẽ được gỡ khỏi {0}, cùng shortcut và mục trong Installed apps."), _root), S(108), Theme.Text);
            CancelButton = Button(T.S("Cancel", "Hủy"), false, delegate { Close(); });
            Btn go = null;
            if (Directory.Exists(Ops.DataDir))
            {
                var delete = new Check { Text = T.S("Also delete my accounts and settings", "Xóa luôn tài khoản và cài đặt"), ForeColor = Theme.Text, Box = Theme.Danger, Bounds = new Rectangle(S(30), what.Bottom + S(18), S(500), S(28)) };
                Add(delete);
                var detail = Para("", delete.Bottom + S(4), Theme.Muted);
                detail.Left = S(60);
                detail.MaximumSize = new Size(ClientSize.Width - S(92), 0);
                Action show = delegate
                {
                    detail.Text = delete.Checked
                        ? string.Format(T.S("Deletes the vault with your saved accounts, your settings and logs in {0}. This can't be undone. Backups you saved in another folder stay.",
                            "Xóa vault chứa các tài khoản đã lưu, cài đặt và nhật ký trong {0}. Không khôi phục lại được. Các bản sao lưu ở thư mục khác vẫn còn."), Ops.DataDir)
                        : string.Format(T.S("Kept in {0}, so installing Bloxkeep again picks up where you left off.",
                            "Được giữ trong {0}, cài lại Bloxkeep là dùng tiếp như cũ."), Ops.DataDir);
                    detail.ForeColor = delete.Checked ? Theme.DangerText : Theme.Muted;
                    if (go != null)
                    {
                        go.Text = delete.Checked ? T.S("Uninstall and delete", "Gỡ và xóa dữ liệu") : T.S("Uninstall", "Gỡ cài đặt");
                        go.Width = Math.Max(S(104), TextRenderer.MeasureText(go.Text, go.Font).Width + S(36));
                        if (delete.Checked)
                            go.Colors(Theme.Danger, Theme.DangerHover, Color.White);
                        else
                            go.Colors(Theme.Accent, Theme.AccentHover, Theme.Ink);
                        LayoutFooter();
                    }
                };
                delete.Checked = _deleteData;
                delete.Changed += delegate { _deleteData = delete.Checked; show(); };
                go = Button(T.S("Uninstall", "Gỡ cài đặt"), true, delegate { Remove(); });
                show();
            }
            else
            {
                Para(T.S("No accounts are saved on this PC.", "Máy này không có tài khoản nào được lưu."), what.Bottom + S(14), Theme.Muted);
                go = Button(T.S("Uninstall", "Gỡ cài đặt"), true, delegate { Remove(); });
            }
            if (Ops.AppRunning())
                Para(T.S("Bloxkeep is running; it will be closed.", "Bloxkeep đang chạy; nó sẽ được đóng lại."), S(300) - S(28), Theme.Muted);
        }

        /// <summary>Lines the footer buttons up again after one changed width.</summary>
        void LayoutFooter()
        {
            int right = ClientSize.Width - S(32);
            var buttons = new List<Control>(Ctrls(_footer));
            buttons.Sort(delegate (Control a, Control b) { return b.Left.CompareTo(a.Left); });
            foreach (Control c in buttons)
            {
                c.Left = right - c.Width;
                right = c.Left - S(10);
            }
        }

        void Remove()
        {
            bool delete = _deleteData;
            ShowPage("removing");
            Work(delegate { return Ops.Uninstall(_root, delete); }, delegate (int code)
            {
                if (code == 0)
                    ShowPage("removed");
                else
                {
                    _error = string.Format(T.S("Uninstalling stopped with code {0}. The setup log below says why.",
                        "Gỡ cài đặt dừng lại với mã {0}. Nhật ký cài đặt bên dưới ghi lý do."), code);
                    ShowPage("failed");
                }
            });
        }

        void Removing()
        {
            Header("logo", T.S("Removing Bloxkeep", "Đang gỡ Bloxkeep"), _root, false);
            _status = Para(_deleteData ? T.S("Removing the program and your data…", "Đang gỡ chương trình và xóa dữ liệu…") : T.S("Removing the program…", "Đang gỡ chương trình…"), S(118), Theme.Text);
            _bar = new Bar { Bounds = new Rectangle(S(32), S(146), ClientSize.Width - S(64), S(8)), Value = -1 };
            Add(_bar);
        }

        void Removed()
        {
            Header("ok", T.S("Bloxkeep was removed", "Đã gỡ Bloxkeep"), _root, true);
            bool kept = Ops.HasData;
            Para(kept
                ? string.Format(T.S("Your accounts and settings are still in {0}, so installing Bloxkeep again picks up where you left off.",
                    "Tài khoản và cài đặt vẫn còn trong {0}, cài lại Bloxkeep là dùng tiếp như cũ."), Ops.DataDir)
                : _deleteData ? T.S("Your accounts and settings were deleted too.", "Tài khoản và cài đặt cũng đã được xóa.")
                : T.S("Thanks for using Bloxkeep.", "Cảm ơn bạn đã dùng Bloxkeep."), S(112), Theme.Text);
            Button(T.S("Close", "Đóng"), true, delegate { Result = 0; Close(); });
        }

        // ---- "Restart to update" ----------------------------------------------------------------------------------

        void Updating()
        {
            Header("logo", T.S("Updating Bloxkeep", "Đang cập nhật Bloxkeep"), string.Format(T.S("To version {0}", "Lên phiên bản {0}"), _updatingTo), false);
            Para(T.S("Bloxkeep opens again by itself in a few seconds, as it was. Your Roblox windows keep running.",
                "Bloxkeep sẽ tự mở lại sau vài giây, y như cũ. Các cửa sổ Roblox vẫn chạy bình thường."), S(106), Theme.Muted);
            _bar = new Bar { Bounds = new Rectangle(S(32), S(166), ClientSize.Width - S(64), S(8)), Value = -1 };
            Add(_bar);
        }

        void UpdateSlow()
        {
            _footer.Visible = true;
            ClientSize = new Size(S(600), S(300));
            Header("problem", T.S("Bloxkeep hasn't opened again yet", "Bloxkeep chưa mở lại"), string.Format(T.S("Version {0}", "Phiên bản {0}"), _updatingTo), false);
            var text = Para(T.S("It's taking longer than usual. If it doesn't open in a moment, start it from the Start menu or the Desktop shortcut; your accounts are safe.",
                "Lâu hơn bình thường. Nếu lát nữa vẫn chưa mở, hãy mở từ Start menu hoặc shortcut trên Desktop; tài khoản của bạn vẫn an toàn."), S(106), Theme.Text);
            var log = new LinkLabel { Text = T.S("Open the setup log", "Mở nhật ký cài đặt"), AutoSize = true, LinkBehavior = LinkBehavior.HoverUnderline, Location = new Point(S(32), text.Bottom + S(12)) };
            log.LinkColor = log.VisitedLinkColor = Theme.Accent;
            log.ActiveLinkColor = Theme.AccentHover;
            log.LinkClicked += delegate
            {
                try { Process.Start(new ProcessStartInfo("notepad.exe", "\"" + Ops.VelopackLog + "\"")).Dispose(); }
                catch (System.ComponentModel.Win32Exception) { }
            };
            Add(log);
            CancelButton = Button(T.S("Close", "Đóng"), true, delegate { Close(); });
        }

        /// <summary>
        /// Closes the "Updating" window once the new version is up: the old run lets go of Bloxkeep's single-instance
        /// name as it quits, the new one takes it again. After a minute without that, says so instead.
        /// </summary>
        void WatchForNewVersion()
        {
            var worker = new Thread(delegate ()
            {
                var clock = Stopwatch.StartNew();
                bool gone = false, back = false;
                while (clock.Elapsed < TimeSpan.FromSeconds(60))
                {
                    bool running = Ops.AppRunning();
                    if (!running)
                        gone = true;
                    else if (gone)
                    {
                        back = true;
                        Thread.Sleep(700); // its window comes up meanwhile
                        break;
                    }
                    Thread.Sleep(150);
                }
                try
                {
                    BeginInvoke((MethodInvoker)delegate
                    {
                        if (back)
                            Close();
                        else
                            ShowPage("updateSlow");
                    });
                }
                catch (InvalidOperationException) { } // already closed by the person
            });
            worker.IsBackground = true;
            worker.Start();
        }

        /// <summary>Runs <paramref name="job"/> off the window's thread, then <paramref name="done"/> on it.</summary>
        void Work(Func<int> job, Action<int> done)
        {
            var worker = new Thread(delegate ()
            {
                int code;
                try { code = job(); }
                catch (Exception ex)
                {
                    BeginInvoke((MethodInvoker)delegate { _error = ex.Message; ShowPage("failed"); });
                    return;
                }
                BeginInvoke((MethodInvoker)delegate { done(code); });
            });
            worker.IsBackground = true;
            worker.Start();
        }

        /// <summary>Saves a page as a PNG (checking the layout, and the README's pictures).</summary>
        public int SaveShot(string path, string page)
        {
            if (page == "installing")
            {
                _target = @"D:\Bloxkeep";
                ShowPage(page);
                Progress(62);
            }
            else if (page == "picked")
            {
                _target = Ops.ProgramFolderIn(@"D:\");
                ShowPage("choose");
            }
            else if (page == "delete")
            {
                _deleteData = true;
                ShowPage("uninstall");
            }
            else if (page != null)
            {
                _target = @"D:\Bloxkeep";
                _error = page == "failed" ? string.Format(T.S("The installer stopped with code {0}. The setup log below says why. If Bloxkeep was open, close it and try again.",
                    "Trình cài dừng lại với mã {0}. Nhật ký cài đặt bên dưới ghi lý do. Nếu Bloxkeep đang mở, hãy đóng nó rồi thử lại."), 1) : null;
                ShowPage(page);
            }
            _shooting = true;
            StartPosition = FormStartPosition.Manual;
            Location = new Point(-20000, -20000);
            ShowInTaskbar = false;
            Show();
            Application.DoEvents();
            using (var bmp = new Bitmap(Width, Height))
            {
                DrawToBitmap(bmp, new Rectangle(0, 0, Width, Height));
                bmp.Save(path, System.Drawing.Imaging.ImageFormat.Png);
            }
            _page = null;
            Close();
            return 0;
        }
    }

    /// <summary>Windows' own folder picker (the modern one), with the old tree dialog as a fallback.</summary>
    static class FolderPicker
    {
        public static string Pick(IWin32Window owner, string title, string start)
        {
            try
            {
                var dialog = (IFileOpenDialog)new FileOpenDialogRCW();
                try
                {
                    uint options;
                    dialog.GetOptions(out options);
                    dialog.SetOptions(options | 0x20 | 0x40 | 0x800); // FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST
                    dialog.SetTitle(title);
                    if (!string.IsNullOrEmpty(start))
                    {
                        IShellItem folder;
                        if (SHCreateItemFromParsingName(start, IntPtr.Zero, typeof(IShellItem).GUID, out folder) == 0)
                            dialog.SetFolder(folder);
                    }
                    if (dialog.Show(owner.Handle) != 0)
                        return null; // Cancelled.
                    IShellItem result;
                    dialog.GetResult(out result);
                    string path;
                    result.GetDisplayName(0x80058000, out path); // SIGDN_FILESYSPATH
                    return path;
                }
                finally
                {
                    Marshal.ReleaseComObject(dialog);
                }
            }
            catch (COMException)
            {
                using (var old = new FolderBrowserDialog())
                {
                    old.Description = title;
                    old.SelectedPath = start ?? "";
                    return old.ShowDialog(owner) == DialogResult.OK ? old.SelectedPath : null;
                }
            }
        }

        [DllImport("shell32.dll", CharSet = CharSet.Unicode)]
        static extern int SHCreateItemFromParsingName(string path, IntPtr pbc, [MarshalAs(UnmanagedType.LPStruct)] Guid riid, out IShellItem item);

        [ComImport, Guid("DC1C5A9C-E88A-4dde-A5A1-60F82A20AEF7")]
        class FileOpenDialogRCW { }

        // Only the methods up to GetResult are declared; their order must match the COM vtable.
        [ComImport, Guid("d57c7288-d4ad-4768-be02-9d969532d960"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
        interface IFileOpenDialog
        {
            [PreserveSig] int Show(IntPtr parent);
            void SetFileTypes(uint count, IntPtr specs);
            void SetFileTypeIndex(uint index);
            void GetFileTypeIndex(out uint index);
            void Advise(IntPtr events, out uint cookie);
            void Unadvise(uint cookie);
            void SetOptions(uint options);
            void GetOptions(out uint options);
            void SetDefaultFolder(IShellItem item);
            void SetFolder(IShellItem item);
            void GetFolder(out IShellItem item);
            void GetCurrentSelection(out IShellItem item);
            void SetFileName([MarshalAs(UnmanagedType.LPWStr)] string name);
            void GetFileName([MarshalAs(UnmanagedType.LPWStr)] out string name);
            void SetTitle([MarshalAs(UnmanagedType.LPWStr)] string title);
            void SetOkButtonLabel([MarshalAs(UnmanagedType.LPWStr)] string text);
            void SetFileNameLabel([MarshalAs(UnmanagedType.LPWStr)] string text);
            void GetResult(out IShellItem item);
        }

        [ComImport, Guid("43826D1E-E718-42EE-BC55-A1E261C37BFE"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
        interface IShellItem
        {
            void BindToHandler(IntPtr pbc, ref Guid bhid, ref Guid riid, out IntPtr ppv);
            void GetParent(out IShellItem parent);
            void GetDisplayName(uint sigdn, [MarshalAs(UnmanagedType.LPWStr)] out string name);
            void GetAttributes(uint mask, out uint attributes);
            void Compare(IShellItem other, uint hint, out int order);
        }
    }
}
