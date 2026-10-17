using System.Text.Json;

namespace RobloxManager;

/// <summary>
/// What the tray icon's menu (<see cref="TrayMenu"/>) shows and does with Bloxkeep in the tray: Multi-Roblox's state
/// (and turning it off), the presets to run, and closing every Roblox, the same actions as the page's.
/// </summary>
public sealed partial class AppController
{
    /// <param name="Open">A vault is open and the screen isn't locked: presets and closing Roblox are offered.</param>
    /// <param name="Multi">"off", "ready" (Multi-Roblox holds Roblox's lock) or "notReady".</param>
    public sealed record TrayInfo(bool Open, string Multi, int RobloxCount, List<(Guid Id, string Name)> Presets);

    public TrayInfo TrayState()
    {
        bool open = _vault is not null && !_screenLocked;
        bool on = open ? _vault!.Data.Settings.MultiInstance : Prefs.MultiRobloxAtStart;
        var presets = open ? _vault!.Data.Settings.Presets.Select(p => (p.Id, p.Name)).ToList() : [];
        return new TrayInfo(open, !on ? "off" : MultiInstance.Armed ? "ready" : "notReady", RobloxCount, presets);
    }

    /// <summary>The menu's Multi-Roblox item while it is on: off, as the sidebar's switch does. (On asks first: the page.)</summary>
    public void TrayMultiOff()
    {
        if (_vault is null || _screenLocked)
            return;
        SetSettings(JsonSerializer.SerializeToElement(new { multiInstance = false }));
        PushState();
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
