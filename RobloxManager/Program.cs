using Velopack;

namespace RobloxManager;

static class Program
{
    static DateTime _lastErrorBox;

    [STAThread]
    static void Main()
    {
        // The installer and updater run the app with special arguments for install/update/uninstall steps; those
        // are handled here (and exit) before anything else starts. Uninstalling keeps the user's accounts.
        VelopackApp.Build()
            .OnBeforeUninstallFastCallback(_ =>
            {
                CloseRunningApp();
                Vault.DeleteCaches();
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
            activate.Set();
            return;
        }

        Prefs.Load();
        BrowserLoginForm.CleanupLeftoverProfiles();
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
        var wait = ThreadPool.RegisterWaitForSingleObject(activate, (_, _) =>
        {
            if (shell.IsHandleCreated && !shell.IsDisposed)
                shell.BeginInvoke(shell.BringToFront);
        }, null, Timeout.Infinite, executeOnlyOnce: false);
        // The uninstaller asks the running app to quit (even when it would only hide to the tray), so nothing it uses
        // stays locked.
        var quitWait = ThreadPool.RegisterWaitForSingleObject(quit, (_, _) =>
        {
            if (shell.IsHandleCreated && !shell.IsDisposed)
                shell.BeginInvoke(shell.Quit);
        }, null, Timeout.Infinite, executeOnlyOnce: true);
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

    /// <summary>
    /// Before uninstalling: asks a running Bloxkeep to close and waits (up to 15 s of the hook's 30) until it has,
    /// so its files and browser cache can be removed. The updater can't always see the app's process itself.
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
