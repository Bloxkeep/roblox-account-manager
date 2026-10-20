using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// The custom theme (Settings › Appearance, AppController.CustomTheme.cs): colours checked and kept, the window's own
/// colours dark or light by the background, its picture served only under its own kind of name (never another file of
/// the data folder), and the choice surviving a restart.
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
        string picture = Path.Combine(Vault.DataDir, "theme-background-1.png");
        File.WriteAllBytes(picture, [0x89, 0x50, 0x4e, 0x47]);
        Prefs.Custom = Prefs.Custom with { Picture = "theme-background-1.png" };
        await Harness.Call(app, "clearThemePicture", new { });
        c.Check("removing the picture deletes its copy", Prefs.Custom.Picture is null && !File.Exists(picture));

        Prefs.Custom = Prefs.Custom with { Picture = "theme-background-2.png" };
        Prefs.Save();
        Prefs.Custom = new("#000000", "#000000", 0, null);
        Prefs.Theme = "dark";
        Prefs.Load();
        c.Check("the custom theme, its colours and its picture come back after a restart",
            Prefs.Theme == "custom" && Prefs.Custom == new Prefs.CustomTheme("#141a26", "#123456", 30, "theme-background-2.png"), Prefs.Custom);
    }
}
