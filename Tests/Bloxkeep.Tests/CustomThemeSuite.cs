using System.IO.Compression;
using System.Text;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// The custom theme (Settings › Appearance, AppController.CustomTheme.cs): colours checked and kept, the window's own
/// colours dark or light by the background, its picture served only under its own kind of name (never another file of
/// the data folder), and the choice surviving a restart. A video can be the picture, either blurred; custom themes kept
/// by name (Your themes) keep their pictures; a theme shared as a file comes back the same, and a file from someone else
/// is trusted in nothing (its picture only of a kind and size the page shows, under a name of Bloxkeep's own).
/// </summary>
static class CustomThemeSuite
{
    public static async Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        c.Check("the custom theme is a theme of its own", Themes.Normalize("custom") == "custom" && Themes.Normalize("nonsense") == Themes.Default);
        c.Check("colours are #rrggbb, kept in lower case; anything else is refused",
            Themes.Hex("#A1B2C3") == "#a1b2c3" && Themes.Hex("red") is null && Themes.Hex("#12345g") is null && Themes.Hex("#1234567") is null && Themes.Hex(null) is null);
        c.Check("only a picture of its own name is served from the data folder",
            Themes.IsPictureName("theme-background-638700000000000000.png") && Themes.IsPictureName("theme-background-1.jpeg")
            && !Themes.IsPictureName("vault.rmv") && !Themes.IsPictureName("theme-background-1.exe")
            && !Themes.IsPictureName("theme-background-1.png/../vault.rmv") && !Themes.IsPictureName("../theme-background-1.png"));
        c.Check("…a video (MP4, WebM) as the custom theme's, never as a vault's picture",
            Themes.IsPictureName("theme-background-1.mp4") && Themes.IsThemeMediaName("theme-background-1.webm") && Themes.IsVideo("theme-background-1.webm")
            && !Themes.IsPictureName("vault-picture-0123abcd-1.mp4") && !Themes.IsThemeMediaName("vault-picture-0123abcd-1.png") && !Themes.IsVideo("theme-background-1.png"));
        c.Check("a theme's name: control characters gone, trimmed, at most 40 characters, none when empty",
            Themes.ThemeName("  Night\u0007 drive \n") == "Night drive" && Themes.ThemeName(new string('x', 60))!.Length == 40 && Themes.ThemeName(" \t ") is null);

        var seed = Vault.Create(Harness.Password);
        seed.Save();
        using var form = new Form();
        using var app = new AppController(form, _ => { });
        await Harness.Unlock(app);

        await Harness.Call(app, "setCustomTheme", new { background = "#F1E9DA", accent = "#f59e0b", fade = 120 });
        c.Check("new colours make the custom theme the one in use, the fade kept within 0 to 90",
            Prefs.Theme == "custom" && Prefs.Custom is { Background: "#f1e9da", Accent: "#f59e0b", Fade: 90 }, Prefs.Custom);
        c.Check("on a light background the window and title bar are light", Themes.Current is { Id: "custom", Dark: false } light
            && light.Background.ToArgb() == Color.FromArgb(0xf1, 0xe9, 0xda).ToArgb(), Themes.Current);

        await Harness.Call(app, "setCustomTheme", new { background = "not a colour", accent = "#123456" });
        c.Check("a colour that isn't one leaves the old one", Prefs.Custom is { Background: "#f1e9da", Accent: "#123456", Fade: 90 }, Prefs.Custom);
        await Harness.Call(app, "setCustomTheme", new { background = "#141a26", accent = "#123456", fade = 30 });
        c.Check("on a dark background they are dark", Themes.Current is { Id: "custom", Dark: true }, Themes.Current);

        // A picture chosen earlier (as "Choose a picture…" leaves it), then removed: the file goes too.
        Directory.CreateDirectory(Vault.PicturesDir);
        string picture = Path.Combine(Vault.PicturesDir, "theme-background-1.png");
        File.WriteAllBytes(picture, [0x89, 0x50, 0x4e, 0x47]);
        Prefs.Custom = Prefs.Custom with { Picture = "theme-background-1.png" };
        await Harness.Call(app, "clearThemePicture", new { });
        c.Check("removing the picture deletes its copy", Prefs.Custom.Picture is null && !File.Exists(picture));

        await Harness.Call(app, "setCustomTheme", new { background = "#141a26", accent = "#123456", fade = 30, blur = 99 });
        c.Check("the blur kept within 0 to 30 px", Prefs.Custom.Blur == 30, Prefs.Custom);
        await Harness.Call(app, "setCustomTheme", new { background = "#141a26", accent = "#123456", fade = 30, blur = 6 });

        // ── Your themes ──
        string Pic(string name) => Path.Combine(Vault.PicturesDir, name);
        File.WriteAllBytes(Pic("theme-background-10.png"), [1, 2, 3]);
        Prefs.Custom = Prefs.Custom with { Picture = "theme-background-10.png" };
        await Harness.Call(app, "saveTheme", new { name = " Ha Long " });
        var kept = Prefs.SavedThemes.SingleOrDefault();
        c.Check("the custom theme saved under its name, as it is", kept is { Name: "Ha Long" } && kept.Look == Prefs.Custom, kept);
        c.Check("…and in the app's state for Settings", Harness.Get<object>(app, "_vault") is not null
            && Harness.Invoke(app, "SavedThemesDto") is object[] { Length: 1 });
        await Harness.Call(app, "setCustomTheme", new { background = "#0a3d62", accent = "#123456", fade = 30 });
        await Harness.Call(app, "saveTheme", new { name = "ha long" });
        c.Check("the same name again saves over it (one theme, the new look)",
            Prefs.SavedThemes.Count == 1 && Prefs.SavedThemes[0].Look.Background == "#0a3d62", Prefs.SavedThemes.Count);
        c.Check("a theme with no name is refused", await Harness.Try(app, "saveTheme", new { name = "  " }) is UserError);
        await Harness.Call(app, "clearThemePicture", new { });
        c.Check("removing the picture in use keeps it while a kept theme shows it", File.Exists(Pic("theme-background-10.png")));
        await Harness.Call(app, "useSavedTheme", new { id = Prefs.SavedThemes[0].Id });
        c.Check("a kept theme picked is the custom theme in use, its picture with it",
            Prefs.Theme == "custom" && Prefs.Custom == Prefs.SavedThemes[0].Look && Prefs.Custom.Picture == "theme-background-10.png", Prefs.Custom);

        // Shared as a file and added back: the same look, its picture copied under a new name, the same bytes.
        string file = Path.Combine(dir, "Ha Long.bloxkeeptheme");
        Harness.Invoke(app, "WriteThemeFile", Prefs.SavedThemes[0], file);
        await Harness.Call(app, "setCustomTheme", new { background = "#222222", accent = "#ffffff", fade = 10 });
        Harness.Invoke(app, "AddThemeFile", file);
        var added = Prefs.SavedThemes[0];
        c.Check("a theme's file added back: kept first, in use, the same name and look",
            Prefs.SavedThemes.Count == 2 && added.Name == Prefs.SavedThemes[1].Name && Prefs.Custom == added.Look && added.Look with { Picture = null } == Prefs.SavedThemes[1].Look with { Picture = null },
            added);
        c.Check("…its picture copied out under a new name of Bloxkeep's own, the same bytes",
            added.Look.Picture is { } copy && copy != "theme-background-10.png" && Themes.IsThemeMediaName(copy) && File.ReadAllBytes(Pic(copy)).SequenceEqual(new byte[] { 1, 2, 3 }),
            added.Look.Picture);

        // Files from someone else.
        string Zip(string name, string json, params (string Name, byte[] Data)[] files)
        {
            string path = Path.Combine(dir, name);
            File.Delete(path);
            using var zip = ZipFile.Open(path, ZipArchiveMode.Create);
            using (var w = new StreamWriter(zip.CreateEntry("theme.json").Open()))
                w.Write(json);
            foreach (var (entry, data) in files)
                using (var stream = zip.CreateEntry(entry).Open())
                    stream.Write(data);
            return path;
        }
        Exception? Add(string path)
        {
            try
            {
                Harness.Invoke(app, "AddThemeFile", path);
                return null;
            }
            catch (System.Reflection.TargetInvocationException e)
            {
                return e.InnerException;
            }
        }
        int before = Prefs.SavedThemes.Count;
        var pictures = Directory.GetFiles(Vault.PicturesDir).Length;
        string notZip = Path.Combine(dir, "plain.bloxkeeptheme");
        File.WriteAllText(notZip, "not a zip");
        c.Check("a file that isn't a theme is refused, nothing kept",
            Add(notZip) is UserError && Add(Zip("nojson.bloxkeeptheme", "[1]")) is UserError
            && Add(Zip("other.bloxkeeptheme", "{\"name\":\"x\"}")) is UserError && Prefs.SavedThemes.Count == before);
        c.Check("a picture of a kind the page doesn't show (a program) is refused, nothing written",
            Add(Zip("exe.bloxkeeptheme", "{\"bloxkeepTheme\":1,\"name\":\"x\",\"picture\":\"background.exe\"}", ("background.exe", [0x4d, 0x5a])))
                is UserError && Prefs.SavedThemes.Count == before && Directory.GetFiles(Vault.PicturesDir).Length == pictures);
        var traversal = Add(Zip("path.bloxkeeptheme", "{\"bloxkeepTheme\":1,\"name\":\"Sneaky\",\"background\":\"red\",\"accent\":\"#00ff00\",\"fade\":500,\"blur\":-4,\"picture\":\"../../evil.png\"}",
            ("../../evil.png", [1])));
        var sneaky = Prefs.SavedThemes[0];
        c.Check("a picture named with a folder is never read, nothing written outside the pictures folder; colours checked, numbers kept in range",
            traversal is null && sneaky.Name == "Sneaky" && sneaky.Look.Picture is null && sneaky.Look.Accent == "#00ff00" && sneaky.Look.Fade == 90
            && sneaky.Look.Blur == 0 && Themes.Hex(sneaky.Look.Background) is not null && !File.Exists(Path.Combine(dir, "evil.png"))
            && !File.Exists(Path.Combine(Vault.DataDir, "evil.png")), sneaky);
        c.Check("a video over 50 MB is refused before anything is read",
            Harness.Invoke(app, "CheckThemeMedia", ".mp4", 50L * 1024 * 1024) is null
            && Catch(() => Harness.Invoke(app, "CheckThemeMedia", ".mp4", 50L * 1024 * 1024 + 1)) is UserError
            && Catch(() => Harness.Invoke(app, "CheckThemeMedia", ".png", 20L * 1024 * 1024 + 1)) is UserError);

        // Deleted: its picture goes, unless the theme in use or another kept one shows it.
        string addedPicture = added.Look.Picture!;
        Prefs.Custom = Prefs.Custom with { Picture = null };
        await Harness.Call(app, "deleteSavedTheme", new { id = added.Id });
        c.Check("a kept theme deleted, its picture with it", Prefs.SavedThemes.All(t => t.Id != added.Id) && !File.Exists(Pic(addedPicture)));
        c.Check("…a picture another kept theme shows is kept", File.Exists(Pic("theme-background-10.png")));

        Prefs.Custom = Prefs.Custom with { Picture = "theme-background-2.png" };
        Prefs.Save();
        var customBefore = Prefs.Custom;
        var savedBefore = Prefs.SavedThemes.ToList();
        Prefs.SavedThemes = [];
        Prefs.Custom = new("#000000", "#000000", 0, null);
        Prefs.Theme = "dark";
        Prefs.Load();
        c.Check("the custom theme, its colours, its picture and its blur come back after a restart",
            Prefs.Theme == "custom" && Prefs.Custom == customBefore && customBefore.Picture == "theme-background-2.png", Prefs.Custom);
        c.Check("…and your themes", Prefs.SavedThemes.SequenceEqual(savedBefore), string.Join(", ", Prefs.SavedThemes));
    }

    static Exception? Catch(Action act)
    {
        try
        {
            act();
            return null;
        }
        catch (System.Reflection.TargetInvocationException e)
        {
            return e.InnerException;
        }
    }
}
