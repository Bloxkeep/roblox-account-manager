using System.Reflection;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// An account's own frame rate limit in Roblox (RobloxGameSettings.ForLaunch, PrepareFor, Restore), on a stand-in Roblox
/// folder (never the real one): the limit goes over the shared one and works without it, light AFK takes only a lower
/// one; the file holds it for that account's opening, the next account gets the person's own, and the person's own comes
/// back even after a client saves the limit; only the choices offered are taken.
/// </summary>
static class FpsCapSuite
{
    public static async Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        string roblox = Path.Combine(dir, "Roblox");
        Directory.CreateDirectory(roblox);
        typeof(RobloxGameSettings).GetField("RobloxDir", BindingFlags.Static | BindingFlags.NonPublic)!.SetValue(null, roblox);
        string file = Path.Combine(roblox, "GlobalBasicSettings_13.xml");
        const string xml = """
            <roblox><Item class="UserGameSettings"><Properties>
            <int name="FramerateCap">240</int><int name="GraphicsQualityLevel">19</int><token name="SavedQualityLevel">9</token>
            <float name="MasterVolume">0.5</float><bool name="Fullscreen">true</bool><string name="Other">kept</string>
            </Properties></Item></roblox>
            """;
        File.WriteAllText(file, xml);
        string Fps() => System.Text.RegularExpressions.Regex.Match(File.ReadAllText(file), "FramerateCap\">([^<]*)<").Groups[1].Value;

        var shared = new RobloxConfig { Mode = "all", Graphics = 3, Fps = 60, Volume = 20 };
        var usesBoth = RobloxGameSettings.ForLaunch(shared, afk: false, usesShared: true, accountFps: 15)!;
        c.Check("the account's limit goes over the shared one, the rest of the shared setup stays",
            usesBoth is { Fps: 15, Graphics: 3, Volume: 20 } && shared.Fps == 60);
        var alone = RobloxGameSettings.ForLaunch(shared, afk: false, usesShared: false, accountFps: 45)!;
        c.Check("…it works without the shared setup, which is left out", alone is { Fps: 45, Graphics: 0, Volume: -1 });
        c.Check("no limit and no shared setup: nothing of Bloxkeep's", RobloxGameSettings.ForLaunch(shared, false, false, 0) is null);
        c.Check("light AFK takes a lower limit", RobloxGameSettings.ForLaunch(shared, true, false, 15) is { Fps: 15, Graphics: 1, Volume: 0 });
        c.Check("…but stays light under a higher one", RobloxGameSettings.ForLaunch(shared, true, false, 240) is { Fps: 30 });

        c.Check("opening the account: the file holds its limit", RobloxGameSettings.PrepareFor(alone, true, false) && Fps() == "45");
        c.Check("…nothing else changes", File.ReadAllText(file) == xml.Replace(">240<", ">45<"));
        c.Check("the next account without one gets the person's own", RobloxGameSettings.PrepareFor(shared, false, false) && Fps() == "240");

        RobloxGameSettings.PrepareFor(usesBoth, true, false);
        c.Check("a different limit for the next one", Fps() == "15");
        RobloxGameSettings.Restore(final: false);
        c.Check("once started, the person's own come back", File.ReadAllText(file) == xml);

        File.WriteAllText(file, xml.Replace(">240<", ">15<")); // Roblox saves what it runs at as it joins a game
        RobloxGameSettings.Restore(final: false);
        c.Check("…and again after a client saves the limit", Fps() == "240");

        RobloxGameSettings.PrepareFor(alone, true, false);
        File.WriteAllText(file, xml.Replace(">240<", ">144<")); // changed in Roblox's own menu
        RobloxGameSettings.Restore(final: true);
        c.Check("a value the person chose in Roblox is kept", Fps() == "144" && !RobloxGameSettings.HasBaseline);

        // A last put-back that can't write (the file held, read-only): the person's own are kept for the next try.
        string copy = Path.Combine(dir, "roblox-settings.json");
        File.WriteAllText(file, xml);
        RobloxGameSettings.PrepareFor(alone, true, false);
        File.SetAttributes(file, FileAttributes.ReadOnly);
        RobloxGameSettings.Restore(final: true);
        c.Check("a last put-back that can't write keeps the person's own", RobloxGameSettings.HasBaseline && Fps() == "45");
        File.SetAttributes(file, FileAttributes.Normal);
        RobloxGameSettings.Restore(final: true);
        c.Check("…and puts them back at the next try", File.ReadAllText(file) == xml && !RobloxGameSettings.HasBaseline, Fps());

        // The copy can't be read for a moment: the file isn't touched, and the copy isn't replaced by Bloxkeep's values.
        File.WriteAllText(file, xml);
        RobloxGameSettings.PrepareFor(alone, true, false);
        using (new FileStream(copy, FileMode.Open, FileAccess.Write, FileShare.Write | FileShare.Delete))
            c.Check("a copy that can't be read: nothing written", !RobloxGameSettings.PrepareFor(usesBoth, true, false) && Fps() == "45", Fps());
        c.Check("…and the person's own still come back after", RobloxGameSettings.PrepareFor(shared, false, false) && Fps() == "240", Fps());
        RobloxGameSettings.Restore(final: true);

        // A damaged copy: set aside as it was, and Light AFK's values in the file are never taken for the person's own.
        File.WriteAllText(file, xml);
        RobloxGameSettings.PrepareFor(RobloxGameSettings.LightAfk, true, false);
        File.WriteAllText(copy, "{ damaged");
        bool rebased = RobloxGameSettings.PrepareFor(alone, true, false);
        var aside = Directory.GetFiles(dir, "roblox-settings.json.damaged-*");
        c.Check("a damaged copy is set aside as it was", aside.Length == 1 && File.ReadAllText(aside[0]) == "{ damaged", string.Join(", ", aside));
        c.Check("…and Light AFK's values aren't taken for the person's own", !rebased && !RobloxGameSettings.HasBaseline && Fps() == "30",
            $"{rebased} {Fps()}");
        File.WriteAllText(file, xml); // the person sets their own again in Roblox
        c.Check("…once the file holds the person's own, it works as before",
            RobloxGameSettings.PrepareFor(alone, true, false) && Fps() == "45" && RobloxGameSettings.HasBaseline);
        RobloxGameSettings.Restore(final: true);
        c.Check("…and puts them back", File.ReadAllText(file) == xml && !RobloxGameSettings.HasBaseline);

        using (var seed = Vault.Create(Harness.Password))
            seed.Save();
        using var app = new AppController(new Form(), _ => { });
        SynchronizationContext.SetSynchronizationContext(null);
        await Harness.Unlock(app);
        var vault = Harness.Get<Vault>(app, "_vault");
        var one = new Account { Id = Guid.NewGuid() };
        var two = new Account { Id = Guid.NewGuid() };
        vault.Data.Accounts.AddRange([one, two]);

        await Harness.Call(app, "setFpsCap", new { ids = new[] { one.Id, two.Id }, fps = 20 });
        c.Check("a limit is set on several accounts at once", one.FpsCap == 20 && two.FpsCap == 20);
        await Harness.Call(app, "setFpsCap", new { ids = new[] { one.Id }, fps = 17 });
        c.Check("…only the choices offered are taken", one.FpsCap == 20);
        await Harness.Call(app, "setFpsCap", new { ids = new[] { one.Id }, fps = 0 });
        c.Check("…and taken off one account alone", one.FpsCap == 0 && two.FpsCap == 20);

        await Harness.Call(app, "setSettings", new { robloxConfig = new { fps = 15 } });
        c.Check("the shared limit offers the low ones too", vault.Data.Settings.RobloxConfig.Fps == 15);
    }
}
