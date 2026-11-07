using System.Runtime.InteropServices;
using Velopack;

namespace RobloxManager;

static class Program
{
    static DateTime _lastErrorBox;
    static bool _inputLanguageLogged;

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
        bool anotherRuns;
        try
        {
            anotherRuns = Mutex.TryOpenExisting(SingleInstanceMutex, out var already);
            already?.Dispose();
        }
        catch (UnauthorizedAccessException)
        {
            anotherRuns = true; // one started as administrator
        }
        VelopackApp.Build()
            .SetAutoApplyOnStartup(!anotherRuns)
            .OnBeforeUninstallFastCallback(_ =>
            {
                // Velopack ends this step after 30 s, counted from before this process started: the question below is
                // answered by 22 s, which leaves time to delete the data (2026-10-30 review: 50 s were planned).
                var deadline = DateTime.UtcNow.AddSeconds(22);
                CloseRunningApp();
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
                // The caches go once, after the answer (DeleteAllData clears them itself): before the question, held files could
                // take up to 24 s of the 30 that Velopack allows, and the question's own page makes the webview cache again.
                if (delete)
                    Vault.DeleteAllData();
                else
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

        if (!Relaunch.IsUpdateRun(args))
            WaitForUpdateRun(UpdateRunWait);

        // A second launch just brings the running window to the front.
        // One started as administrator owns these names, and a normal start can't open them: it says so rather than crash.
        var activate = Guarded(() => new EventWaitHandle(false, EventResetMode.AutoReset, ActivateEvent));
        var quit = Guarded(() => new EventWaitHandle(false, EventResetMode.ManualReset, QuitEvent));
        bool first = false;
        var single = Guarded(() => new Mutex(true, SingleInstanceMutex, out first));
        if (activate is null || quit is null || single is null)
        {
            activate?.Dispose();
            quit?.Dispose();
            single?.Dispose();
            Log.Note("Another Bloxkeep, started as administrator, owns the single-instance handles; this start gave way.");
            Ui.Error(null, L.T("Bloxkeep is already running as administrator. Close it, then start Bloxkeep again."));
            return;
        }
        using var activateHandle = activate;
        using var quitHandle = quit;
        using var singleHandle = single;
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
        StartWithWindows.KeepPathCurrent();
        ShellForm shell;
        try
        {
            shell = new ShellForm(resume, startup: args.Contains(StartWithWindows.Arg));
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
        void Listen() => wait ??= ThreadPool.RegisterWaitForSingleObject(activate, (_, _) => OnShell(shell, () => shell.ShowInFront("second start")), null,
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

    /// <summary>A named handle the way <paramref name="open"/> gets it; null when another Bloxkeep started as administrator owns it
    /// and this process has no right to it (opening or creating it throws UnauthorizedAccessException, which would end a start
    /// with an error box, or the uninstall before it deleted anything).</summary>
    internal static T? Guarded<T>(Func<T?> open) where T : class
    {
        try
        {
            return open();
        }
        catch (UnauthorizedAccessException)
        {
            return null;
        }
    }

    /// <summary>Before uninstalling: asks a running Bloxkeep to close and waits up to 10 s (of the hook's 30). Velopack 1.2
    /// already ends processes in the program folder; this covers one it couldn't see (running elevated): that one can't be asked
    /// either (no access), and the uninstall goes on without it.</summary>
    static void CloseRunningApp() => CloseRunningApp(
        () => EventWaitHandle.TryOpenExisting(QuitEvent, out var quit) ? quit : null,
        () => Mutex.TryOpenExisting(SingleInstanceMutex, out var single) ? single : null);

    internal static void CloseRunningApp(Func<EventWaitHandle?> openQuit, Func<Mutex?> openSingle)
    {
        if (Guarded(openQuit) is not { } quit)
            return;
        using (quit)
            quit.Set();
        if (Guarded(openSingle) is not { } single)
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
        // Another keyboard language picked (Alt+Shift) while Bloxkeep has the keyboard: WinForms names it for an event
        // Bloxkeep doesn't use, which can't be done without culture data (InvariantGlobalization), so each switch showed
        // "Unexpected error" (a user's error log, Portuguese 1046, 2026-11-06). The switch itself has already happened.
        if (ex is System.Globalization.CultureNotFoundException && ex.StackTrace?.Contains("System.Windows.Forms.InputLanguage", StringComparison.Ordinal) == true)
        {
            if (!_inputLanguageLogged)
                Log.Write(ex);
            _inputLanguageLogged = true;
            return;
        }
        Log.Write(ex);
        if (DateTime.Now - _lastErrorBox < TimeSpan.FromSeconds(10))
            return;
        _lastErrorBox = DateTime.Now;
        Ui.Error(null, L.F("Unexpected error: {0}", ex.Message));
    }
}
