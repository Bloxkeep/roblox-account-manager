using System.Runtime.InteropServices;
using Velopack;

namespace RobloxManager;

static class Program
{
    static DateTime _lastErrorBox;

    [STAThread]
    static void Main()
    {
        // The installer and updater run the app with special arguments for install/update/uninstall steps; those
        // are handled here (and exit) before anything else starts. Uninstalling keeps the user's accounts: it removes
        // only the program folder, the shortcuts and the interface caches, never %LOCALAPPDATA%\Bloxkeep.
        VelopackApp.Build()
            .OnBeforeUninstallFastCallback(_ =>
            {
                CloseRunningApp();
                Vault.DeleteCaches();
                // Sign-in and captcha browser profiles in %TEMP%, normally removed at the next start that never comes.
                BrowserLoginForm.CleanupLeftoverProfiles();
                CaptchaForm.CleanupLeftoverProfiles();
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
        ShellForm shell;
        try
        {
            shell = new ShellForm();
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
