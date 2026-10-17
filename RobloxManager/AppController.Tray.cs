using System.Text.Json;
using Microsoft.Win32;

namespace RobloxManager;

/// <summary>
/// What the tray icon's menu (<see cref="TrayMenu"/>) shows and does with Bloxkeep in the tray: Multi-Roblox's state
/// (and turning it off), a new Roblox window, the accounts and presets to open, and closing every Roblox, the same
/// actions as the page's. With Multi-Roblox only (no vault, see <see cref="MultiOnly"/>) Multi-Roblox turns on and off
/// from here as well.
/// </summary>
public sealed partial class AppController
{
    /// <param name="Open">A vault is open and the screen isn't locked: accounts, presets and closing Roblox are offered.</param>
    /// <param name="Multi">"off", "ready" (Multi-Roblox holds Roblox's lock) or "notReady".</param>
    /// <param name="Accounts">The accounts to open, most recently used first (at most 12), and whether each is open.</param>
    /// <param name="MultiOnly">No vault on this PC (Multi-Roblox only): Multi-Roblox turns on and off from the menu.</param>
    public sealed record TrayInfo(bool Open, string Multi, int RobloxCount, List<(Guid Id, string Name)> Presets,
        List<(Guid Id, string Name, bool Running)>? Accounts = null, bool MultiOnly = false);

    const int TrayAccounts = 12;

    public TrayInfo TrayState()
    {
        bool open = _vault is not null && !_screenLocked;
        bool on = open ? _vault!.Data.Settings.MultiInstance : Prefs.MultiRobloxAtStart;
        var presets = open ? _vault!.Data.Settings.Presets.Select(p => (p.Id, p.Name)).ToList() : [];
        var accounts = open
            ? _vault!.Data.Accounts.Where(a => a.UserId > 0 && a.Banned != true && a.Realm != Realm.Vng).OrderByDescending(a => a.LastUsed ?? DateTime.MinValue)
                .Take(TrayAccounts).Select(a => (a.Id, NameText(a), _running.ContainsKey(a.Id))).ToList()
            : [];
        return new TrayInfo(open, !on ? "off" : MultiInstance.Armed ? "ready" : "notReady", RobloxCount, presets, accounts, !Vault.Exists);
    }

    /// <summary>
    /// The menu's Multi-Roblox item: off while it is on, as the sidebar's switch does (on asks first: the page). With
    /// Multi-Roblox only, on and off both (its warning was answered when that was chosen).
    /// </summary>
    public void TrayMulti(bool on)
    {
        if (!Vault.Exists)
        {
            MultiOnly(on);
            return;
        }
        if (on || _vault is null || _screenLocked)
            return;
        SetSettings(JsonSerializer.SerializeToElement(new { multiInstance = false }));
        PushState();
    }

    /// <summary>
    /// First start › "Just use Multi-Roblox": Multi-Roblox without a vault or any account, for someone who signs in to
    /// Roblox only in their own browser. It is kept on from start-up (<see cref="Prefs.MultiRobloxAtStart"/>); a vault
    /// made later takes the setting over.
    /// </summary>
    object? MultiOnly(bool on)
    {
        if (Vault.Exists)
            throw new UserError(L.T("Bloxkeep has a vault on this PC: Multi-Roblox is turned on or off inside it."));
        Prefs.MultiRobloxAtStart = on;
        Prefs.Save();
        if (on)
            MultiInstance.Enable();
        else
            MultiInstance.Disable();
        _lastArmed = MultiInstance.Armed;
        PushState();
        return null;
    }

    /// <summary>
    /// "Open Roblox": Roblox's home screen with whatever account Roblox's own app is signed in with (no account of
    /// Bloxkeep's, no sign-in ticket), through the program Windows opens roblox: links with, so the person's launcher
    /// and its mods are used. Only with no Roblox window open: Roblox hands any start without a ticket (a plain one, or
    /// a game link) to the window already open, which then goes where it was sent (seen 2026-10-18, 0.743: "App
    /// NoReload receiving deeplink", and the open window joined the game). A second window comes only from a start
    /// with an account's ticket: Open account, or Play on roblox.com in another browser; that is said instead.
    /// </summary>
    public void TrayNewRoblox()
    {
        void Tell(string text)
        {
            Toast(text, "warn");
            ShowNotification?.Invoke(text, "warn");
        }
        RobloxProcesses.CloseTrayCopies(); // Roblox's windowless copy would take the start over and open nothing
        var players = RobloxProcesses.GetPlayerPids();
        if (RobloxProcesses.VisibleWindows(players).Values.Any(list => list.Count > 0))
        {
            Tell(L.T("Roblox is already open, and Roblox opens another window only for a sign-in: open an account (from Bloxkeep or this menu), or press Play on roblox.com in another browser."));
            return;
        }
        if (RobloxLinkCommand() is not { } command)
        {
            Tell(L.T("Roblox isn't installed on this PC."));
            return;
        }
        string args = command.Args.Replace("\"%1\"", "").Replace("%1", "").Trim();
        var startedAt = DateTime.UtcNow;
        try
        {
            System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo(command.Exe, args)
            {
                UseShellExecute = false,
                WorkingDirectory = Path.GetDirectoryName(command.Exe)!,
            })?.Dispose();
        }
        catch (Exception ex) when (ex is System.ComponentModel.Win32Exception or IOException)
        {
            Tell(ex.Message);
            return;
        }
        _ = TellIfHandedOverAsync(startedAt, Tell);
    }

    /// <summary>The program and its arguments Windows runs for roblox: links (else roblox-player: ones); null without Roblox.</summary>
    static (string Exe, string Args)? RobloxLinkCommand()
    {
        foreach (string scheme in new[] { "roblox", "roblox-player" })
        {
            using var key = Registry.ClassesRoot.OpenSubKey($@"{scheme}\shell\open\command");
            if (key?.GetValue(null) is not string line || (line = line.Trim()).Length == 0)
                continue;
            string exe, args;
            if (line.StartsWith('"'))
            {
                int end = line.IndexOf('"', 1);
                if (end < 2)
                    continue;
                exe = line[1..end];
                args = line[(end + 1)..].Trim();
            }
            else
            {
                int space = line.IndexOf(' ');
                exe = space > 0 ? line[..space] : line;
                args = space > 0 ? line[(space + 1)..].Trim() : "";
            }
            if (File.Exists(exe))
                return (exe, args.Contains("%1") ? args : (args + " \"%1\"").Trim());
        }
        return null;
    }

    /// <summary>A client started since then whose log says Roblox handed it over to the home screen already open.</summary>
    static async Task TellIfHandedOverAsync(DateTime since, Action<string> tell)
    {
        await Task.Delay(TimeSpan.FromSeconds(12));
        foreach (string dir in RobloxLogWatcher.Folders)
        {
            try
            {
                foreach (var file in new DirectoryInfo(dir).EnumerateFiles("*_Player_*.log"))
                {
                    if (file.CreationTimeUtc < since.AddSeconds(-2) || file.Name.Contains("CrashHandler", StringComparison.OrdinalIgnoreCase))
                        continue;
                    using var stream = new FileStream(file.FullName, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete);
                    using var reader = new StreamReader(stream);
                    if ((await reader.ReadToEndAsync()).Contains(NoReloadMark, StringComparison.Ordinal))
                    {
                        tell(L.T("Roblox brought its open home screen forward instead of opening another one: it runs one home screen at a time. To open more windows, press Play on roblox.com in another browser, or open an account from Bloxkeep."));
                        return;
                    }
                }
            }
            catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
            {
                // A log Roblox is replacing, or no log folder: nothing to say.
            }
        }
    }

    /// <summary>What a client logs when Roblox hands it over to the one already open (seen 2026-10-13, 0.742).</summary>
    const string NoReloadMark = "App NoReload!";

    /// <summary>
    /// An account from the menu, as the account card's Play: into the game chosen in the dock, else Roblox's home
    /// screen. A launch with something to ask first isn't opened from here: that is said, to be done from Bloxkeep.
    /// </summary>
    public async void TrayOpenAccount(Guid id)
    {
        void Tell(string text)
        {
            Toast(text, "warn");
            ShowNotification?.Invoke(text, "warn");
        }
        try
        {
            if (_vault is not { } vault || _screenLocked || FindAccount(id) is not { } account)
                return;
            var s = vault.Data.Settings;
            GameTarget? target = null;
            if (s.LastPlaceInput.Length > 0 && !GameTarget.TryParse(s.LastPlaceInput, s.LastJobId, out target, out string error))
                throw new UserError(error);
            CheckCanLaunch([account]);
            if (await LaunchAccountsAsync(vault, [account], target, force: false, s.LaunchDelaySeconds) is { NeedsConfirm: true } result)
                Tell(L.T("Opening this account asks something first. Open it from Bloxkeep.") + "\n\n" + result.Message);
        }
        catch (UserError ex)
        {
            Tell(ex.Message);
        }
        catch (Exception ex)
        {
            Tell(ErrorText(ex));
        }
    }

    /// <summary>
    /// A preset from the menu, as the Presets tab's Run. A launch with something to ask first (Multi-Roblox off, low
    /// RAM…) isn't run from here: that is said, for it to be run from Bloxkeep, where the question shows.
    /// </summary>
    public async void TrayRunPreset(Guid id)
    {
        void Tell(string text)
        {
            Toast(text, "warn");
            ShowNotification?.Invoke(text, "warn");
        }
        try
        {
            if (_vault is null || _screenLocked)
                return;
            if (await StartPresetAsync(id, force: false) is { NeedsConfirm: true } result)
                Tell(L.T("This preset asks something before it opens. Run it from Bloxkeep's Presets tab.") + "\n\n" + result.Message);
        }
        catch (UserError ex)
        {
            Tell(ex.Message);
        }
        catch (Exception ex)
        {
            Tell(ErrorText(ex));
        }
    }

    /// <summary>The vault's Multi-Roblox setting, copied for the next start (<see cref="Prefs.MultiRobloxAtStart"/>).</summary>
    static void KeepMultiAtStart(bool on)
    {
        if (Prefs.MultiRobloxAtStart == on)
            return;
        Prefs.MultiRobloxAtStart = on;
        Prefs.Save();
    }
}
