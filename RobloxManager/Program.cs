using System.Runtime.InteropServices;
using Velopack;

namespace RobloxManager;

static class Program
{
    static DateTime _lastErrorBox;

    [STAThread]
    static void Main(string[] args)
    {
        // The installer and updater run the app with special arguments for install/update/uninstall steps; those
        // are handled here (and exit) before anything else starts. Uninstalling removes the program folder, the
        // shortcuts and the interface caches; the accounts and settings in %LOCALAPPDATA%\Bloxkeep go only when the
        // person chooses so.
        VelopackApp.Build()
            .OnBeforeUninstallFastCallback(_ =>
            {
                var deadline = DateTime.UtcNow.AddSeconds(50);
                CloseRunningApp();
                Vault.DeleteCaches();
                // Sign-in and captcha browser profiles in %TEMP%, normally removed at the next start that never comes.
                BrowserLoginForm.CleanupLeftoverProfiles();
                CaptchaForm.CleanupLeftoverProfiles();
                // Bloxkeep's uninstaller window has already asked and says so here; when Velopack's uninstall runs on
                // its own (an entry an update reset), ask now.
                bool delete = Environment.GetEnvironmentVariable("BLOXKEEP_UNINSTALL_DATA") switch
                {
                    "delete" => true,
                    "keep" => false,
                    _ => AskToDeleteData(deadline),
                };
                if (delete)
                    Vault.DeleteAllData();
            })
            .Run();

        ApplicationConfiguration.Initialize();
        Application.SetUnhandledExceptionMode(UnhandledExceptionMode.CatchException);
        Application.ThreadException += (_, e) => OnUnexpected(e.Exception);
        AppDomain.CurrentDomain.UnhandledException += (_, e) =>
        {
            if (e.ExceptionObject is Exception ex)
                Log.Write(ex);
        };
        TaskScheduler.UnobservedTaskException += (_, e) =>
        {
            Log.Write(e.Exception);
            e.SetObserved();
        };

        // A second launch just brings the running window to the front.
        using var activate = new EventWaitHandle(false, EventResetMode.AutoReset, ActivateEvent);
        using var quit = new EventWaitHandle(false, EventResetMode.ManualReset, QuitEvent);
        using var single = new Mutex(true, SingleInstanceMutex, out bool first);
        if (!first)
        {
            // This process was just started by the person, so it may pass its right to come to the front on to the
            // running one; without that Windows only flashes the running window's taskbar button.
            AllowSetForegroundWindow(ASFW_ANY);
            activate.Set();
            return;
        }

        Prefs.Load();
        BrowserLoginForm.CleanupLeftoverProfiles();
        CaptchaForm.CleanupLeftoverProfiles();
        UseOurUninstaller();
        // Started again by an update in the background: what the last run handed over (also removed when not).
        var resume = Relaunch.Take(args);
        ShellForm shell;
        try
        {
            shell = new ShellForm(resume);
        }
        // Nothing is running yet, so report why the window could not be created instead of vanishing.
        catch (Exception ex)
        {
            Log.Write(ex);
            Ui.Error(null, L.F("Unexpected error: {0}", ex.Message));
            return;
        }
        var wait = ThreadPool.RegisterWaitForSingleObject(activate, (_, _) => OnShell(shell, shell.BringToFront), null, Timeout.Infinite, executeOnlyOnce: false);
        // The uninstaller asks the running app to quit (even when it would only hide to the tray), so nothing it uses
        // stays locked.
        var quitWait = ThreadPool.RegisterWaitForSingleObject(quit, (_, _) => OnShell(shell, shell.Quit), null, Timeout.Infinite, executeOnlyOnce: true);
        try
        {
            Application.Run(shell);
        }
        finally
        {
            wait.Unregister(null);
            quitWait.Unregister(null);
        }
    }

    const string ActivateEvent = @"Local\Bloxkeep.Activate";
    const string QuitEvent = @"Local\Bloxkeep.Quit";
    const string SingleInstanceMutex = @"Local\Bloxkeep.SingleInstance";
    const uint ASFW_ANY = unchecked((uint)-1);

    [DllImport("user32.dll")]
    static extern bool AllowSetForegroundWindow(uint processId);

    /// <summary>Runs a request from another process on the window's thread; the window may be going away meanwhile.</summary>
    static void OnShell(ShellForm shell, Action action)
    {
        try
        {
            if (shell.IsHandleCreated && !shell.IsDisposed)
                shell.BeginInvoke(action);
        }
        catch (Exception ex) when (ex is InvalidOperationException or ObjectDisposedException)
        {
            // Closed between the check and the call: nothing left to bring to the front or quit.
        }
    }

    /// <summary>
    /// Before uninstalling: asks a running Bloxkeep to close and waits (up to 15 s of the hook's 60) until it has,
    /// so its files and browser cache can be removed. Velopack 1.2 ends every process running from the program folder
    /// before this hook, so normally there is nothing left to ask; this covers a copy it couldn't see (one running
    /// elevated), and it cannot stop a killed copy's tray icon from lingering until the mouse passes over it.
    /// </summary>
    static void CloseRunningApp()
    {
        if (!EventWaitHandle.TryOpenExisting(QuitEvent, out var quit))
            return;
        using (quit)
            quit.Set();
        if (!Mutex.TryOpenExisting(SingleInstanceMutex, out var single))
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
                // The app ended without releasing it: it is gone either way, and the mutex is now ours.
                single.ReleaseMutex();
            }
        }
        // Its browser processes end right after it.
        Thread.Sleep(1500);
    }

    /// <summary>
    /// Velopack's uninstall run directly, without Bloxkeep's uninstaller window: asks whether the accounts go too. Kept
    /// when there is no answer by <paramref name="deadline"/>, before Velopack's minute for this step runs out.
    /// </summary>
    static bool AskToDeleteData(DateTime deadline)
    {
        if (!File.Exists(Vault.VaultPath))
            return false;
        Prefs.Load();
        ApplicationConfiguration.Initialize();
        var keep = new TaskDialogCommandLinkButton(L.T("Keep my accounts and settings"), L.T("Installing Bloxkeep again picks up where you left off."));
        var delete = new TaskDialogCommandLinkButton(L.T("Delete my accounts and settings"),
            L.F("Deletes the vault with your saved accounts, your settings and logs in {0}. This can't be undone.", Vault.DataDir));
        var page = new TaskDialogPage
        {
            Caption = L.T("Uninstall Bloxkeep"),
            Heading = L.T("Delete your accounts too?"),
            Text = L.T("Bloxkeep is being removed from this PC. Your accounts and settings stay unless you delete them."),
            Buttons = { keep, delete },
            DefaultButton = keep,
            Footnote = new TaskDialogFootnote(L.T("Kept if you don't choose within a minute.")),
            AllowCancel = true,
        };
        using var timer = new System.Windows.Forms.Timer { Interval = (int)Math.Clamp((deadline - DateTime.UtcNow).TotalMilliseconds, 5000, 50000) };
        timer.Tick += (_, _) =>
        {
            timer.Stop();
            page.BoundDialog?.Close();
        };
        page.Created += (_, _) => timer.Start();
        return TaskDialog.ShowDialog(page) == delete;
    }

    /// <summary>
    /// Installed apps › Uninstall opens Bloxkeep's uninstaller window (BloxkeepUninstall.exe, shipped with the app)
    /// rather than Velopack's bare one. Bloxkeep's installer sets this; doing it at every start keeps it after an update
    /// rewrites the entry. Nothing to do for the portable copy or a development build (no entry, no uninstaller).
    /// </summary>
    static void UseOurUninstaller()
    {
        try
        {
            string appDir = Path.TrimEndingDirectorySeparator(AppContext.BaseDirectory);
            string uninstaller = Path.Combine(appDir, "BloxkeepUninstall.exe");
            if (Path.GetDirectoryName(appDir) is not { } root || !File.Exists(uninstaller) || !File.Exists(Path.Combine(root, "Update.exe")))
                return;
            using var key = Microsoft.Win32.Registry.CurrentUser.OpenSubKey(@"Software\Microsoft\Windows\CurrentVersion\Uninstall\BloxkeepApp", writable: true);
            if (key?.GetValue("InstallLocation") is not string at
                || !string.Equals(Path.TrimEndingDirectorySeparator(Path.GetFullPath(at)), root, StringComparison.OrdinalIgnoreCase))
                return;
            string command = $"\"{uninstaller}\"";
            if (key.GetValue("UninstallString") as string == command)
                return;
            key.SetValue("UninstallString", command);
            key.SetValue("QuietUninstallString", command + " --silent");
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or System.Security.SecurityException)
        {
            Log.Write(ex);
        }
    }

    /// <summary>Logs the error; shows it at most every few seconds so a repeating failure can't flood the screen.</summary>
    static void OnUnexpected(Exception ex)
    {
        Log.Write(ex);
        if (DateTime.Now - _lastErrorBox < TimeSpan.FromSeconds(10))
            return;
        _lastErrorBox = DateTime.Now;
        Ui.Error(null, L.F("Unexpected error: {0}", ex.Message));
    }
}
