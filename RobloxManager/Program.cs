using System.Runtime.InteropServices;
using Velopack;

namespace RobloxManager;

static class Program
{
    static DateTime _lastErrorBox;

    [STAThread]
    static void Main(string[] args)
    {
        // Out of the program folder first: programs Bloxkeep starts inherit it, and one still running (Bloxstrap stays
        // while Roblox does) made updates fail, as Velopack can't move a folder a process is in. No relative paths here.
        try
        {
            Environment.CurrentDirectory = Path.GetTempPath();
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or System.Security.SecurityException)
        {
        }
        // Install/update/uninstall steps from Velopack are handled (and exit) before anything else. Uninstalling keeps
        // the accounts and settings in %LOCALAPPDATA%\Bloxkeep unless the person chooses otherwise.
        // A downloaded update is applied at start (Velopack's default) only when no Bloxkeep runs: a second start (the
        // shortcut clicked to bring the window forward) would otherwise apply it, and its Update.exe kills the running one
        // mid-launch, unsaved (2026-10-30 review). That one applies it itself, at a quiet moment or on quitting.
        bool anotherRuns = Mutex.TryOpenExisting(SingleInstanceMutex, out var already);
        already?.Dispose();
        VelopackApp.Build()
            .SetAutoApplyOnStartup(!anotherRuns)
            .OnBeforeUninstallFastCallback(_ =>
            {
                // Velopack ends this step after 30 s, counted from before this process started: the question below is
                // answered by 22 s, which leaves time to delete the data (2026-10-30 review: 50 s were planned).
                var deadline = DateTime.UtcNow.AddSeconds(22);
                CloseRunningApp();
                Vault.DeleteCaches();
                // Sign-in and captcha browser profiles in %TEMP%, normally removed at the next start that never comes.
                BrowserLoginForm.CleanupLeftoverProfiles();
                CaptchaForm.CleanupLeftoverProfiles();
                WebsiteTab.CleanupLeftovers();
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

        if (!Relaunch.IsUpdateRun(args))
            WaitForUpdateRun(UpdateRunWait);

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

        Vault.Tidy(); // before anything opens the files it moves
        RobloxProcesses.ShowLeftHidden(); // Roblox windows a run that ended suddenly had hidden
        WindowIcons.ClearLeftovers(); // …and the icons it had put on them (its taskbar groups: at the first window check)
        Prefs.Load();
        Vaults.Load(); // the vault the lock screen offers (and an update's restart opens): the last one used
        BrowserLoginForm.CleanupLeftoverProfiles();
        CaptchaForm.CleanupLeftoverProfiles();
        WebsiteTab.CleanupLeftovers();
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
        // Listened for once the window has a handle: a start by hand that waited for an update's run (WaitForUpdateRun)
        // asks as soon as that run takes the hand-over, before its window exists, and OnShell would drop the request.
        RegisteredWaitHandle? wait = null;
        void Listen() => wait ??= ThreadPool.RegisterWaitForSingleObject(activate, (_, _) => OnShell(shell, shell.BringToFront), null,
            Timeout.Infinite, executeOnlyOnce: false);
        // Its handle may be there already (made while building it): then at once, or HandleCreated would never come.
        if (shell.IsHandleCreated)
            Listen();
        else
            shell.HandleCreated += (_, _) => Listen();
        // The uninstaller asks the app to quit (not hide to the tray), so nothing it uses stays locked.
        // A downloaded update isn't applied on that quit: the installer or uninstaller runs Velopack's Update.exe next, and
        // a second one applying the update would race it for the folder (2026-10-30 review).
        var quitWait = ThreadPool.RegisterWaitForSingleObject(quit, (_, _) =>
        {
            AppUpdater.HeldForInstaller = true;
            OnShell(shell, shell.Quit);
        }, null, Timeout.Infinite, executeOnlyOnce: true);
        try
        {
            Application.Run(shell);
        }
        finally
        {
            // Nobody could show them once Bloxkeep is gone.
            RobloxProcesses.ShowWindows();
            wait?.Unregister(null);
            quitWait.Unregister(null);
        }
    }

    const string ActivateEvent = @"Local\Bloxkeep.Activate";
    const string QuitEvent = @"Local\Bloxkeep.Quit";
    const string SingleInstanceMutex = @"Local\Bloxkeep.SingleInstance";
    /// <summary>How long a start by hand waits for the run an update is starting; after that it starts as usual.</summary>
    static readonly TimeSpan UpdateRunWait = TimeSpan.FromSeconds(90);
    const uint ASFW_ANY = unchecked((uint)-1);

    [DllImport("user32.dll")]
    static extern bool AllowSetForegroundWindow(uint processId);

    /// <summary>Opened during a background update: waits for the run the update starts, to bring it forward (as any second
    /// launch) rather than take its place, which made that run (carrying the unlocked vault) give way, and Bloxkeep came back
    /// locked (seen 2026-10-20). Over once that run took the hand-over (or the watch removed it), or after
    /// <paramref name="limit"/>. Not as soon as some Bloxkeep holds the single-instance mutex: right after "Restart to
    /// update" that is the run on its way out, which got the request and vanished, and the update's run then came back in
    /// the tray with nothing on screen (2026-10-30 review).</summary>
    static void WaitForUpdateRun(TimeSpan limit)
    {
        var waiting = System.Diagnostics.Stopwatch.StartNew();
        while (waiting.Elapsed < limit && Relaunch.HandOverWaiting())
            Thread.Sleep(200);
    }

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

    /// <summary>Before uninstalling: asks a running Bloxkeep to close and waits up to 10 s (of the hook's 30). Velopack 1.2
    /// already ends processes in the program folder; this covers one it couldn't see (running elevated).</summary>
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
                if (single.WaitOne(TimeSpan.FromSeconds(10)))
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

    /// <summary>Velopack's uninstall run without Bloxkeep's uninstaller window: asks whether the accounts go too; kept when
    /// unanswered by <paramref name="deadline"/>, before Velopack's 30 s for this step run out.</summary>
    static bool AskToDeleteData(DateTime deadline)
    {
        Prefs.Load();
        Vaults.Load();
        if (Prefs.VaultList.Count == 0 && !File.Exists(Vault.VaultPath))
            return false;
        ApplicationConfiguration.Initialize();
        int picked = Dialog.Show(null, L.T("Bloxkeep is being removed from this PC. Your accounts and settings stay unless you delete them."),
            L.T("Delete your accounts too?"), Dialog.Kind.Warning,
            [
                new Dialog.Choice(L.T("Keep my accounts and settings"), L.T("Installing Bloxkeep again picks up where you left off.")),
                new Dialog.Choice(L.T("Delete my accounts and settings"),
                    L.F("Deletes the vault with your saved accounts, your settings and logs in {0}. This can't be undone.", Vault.DataDir), Danger: true),
            ],
            defaultIndex: 0, cancelIndex: 0, footnote: L.T("Kept if you don't choose within 20 seconds."),
            timeout: TimeSpan.FromMilliseconds(Math.Clamp((deadline - DateTime.UtcNow).TotalMilliseconds, 5000, 20000)));
        return picked == 1;
    }

    /// <summary>Installed apps › Uninstall opens BloxkeepUninstall.exe rather than Velopack's bare one. Set at every start
    /// because an update rewrites the entry; nothing to do for the portable copy or a development build.</summary>
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
            // A quiet fix shows in Installed apps as the version it fixes (Velopack writes the full number at each update).
            if (typeof(Program).Assembly.GetCustomAttributes(typeof(System.Reflection.AssemblyInformationalVersionAttribute), false)
                    .OfType<System.Reflection.AssemblyInformationalVersionAttribute>().FirstOrDefault()?.InformationalVersion.Split('+')[0] is { } version
                && WhatsNew.IsQuietFix(version) && key.GetValue("DisplayVersion") as string != WhatsNew.Shown(version))
                key.SetValue("DisplayVersion", WhatsNew.Shown(version));
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
