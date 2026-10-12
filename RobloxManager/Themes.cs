using Microsoft.Win32;

namespace RobloxManager;

/// <summary>
/// The colour themes. The page draws itself from the tokens in app.css; the window frame, its title bar and the
/// sign-in window need a few of the same colours, which are repeated here (keep the two in step).
/// </summary>
public static class Themes
{
    public sealed record Theme(string Id, bool Dark, Color Background, Color Surface, Color Text, Color Text2, Color Line, Color Good, Color Bad);

    static readonly Theme[] All =
    [
        new("light", false, Rgb(0xf3f4f7), Rgb(0xffffff), Rgb(0x14171f), Rgb(0x3d4352), Rgb(0xd5d8df), Rgb(0x047857), Rgb(0xbe123c)),
        new("paper", false, Rgb(0xf3eee3), Rgb(0xfbf8f1), Rgb(0x2a241a), Rgb(0x50473a), Rgb(0xdcd3c1), Rgb(0x047857), Rgb(0xbe123c)),
        new("dim", true, Rgb(0x1d2026), Rgb(0x2a2e36), Rgb(0xe6e8ec), Rgb(0xbabec8), Rgb(0x464b56), Rgb(0x86efac), Rgb(0xfda4af)),
        new("dark", true, Rgb(0x0a0c11), Rgb(0x161b26), Rgb(0xe9ecf3), Rgb(0xc8cedc), Rgb(0x3c4456), Rgb(0x86efac), Rgb(0xfda4af)),
        new("black", true, Rgb(0x000000), Rgb(0x121318), Rgb(0xeceef2), Rgb(0xb4b8c2), Rgb(0x34363d), Rgb(0x86efac), Rgb(0xfda4af)),
        new("frost", true, Rgb(0x2e3440), Rgb(0x3b4252), Rgb(0xeceff4), Rgb(0xd8dee9), Rgb(0x4c566a), Rgb(0xc5d9b4), Rgb(0xe8a9af)),
        new("ocean", true, Rgb(0x0a1622), Rgb(0x13283b), Rgb(0xe3eef7), Rgb(0xa9c1d4), Rgb(0x24425a), Rgb(0xa7f3d0), Rgb(0xfecdd3)),
        new("forest", true, Rgb(0x0d1410), Rgb(0x19271f), Rgb(0xe4efe6), Rgb(0xb3c7b8), Rgb(0x2e4435), Rgb(0xa7f3d0), Rgb(0xfecdd3)),
        new("mocha", true, Rgb(0x1b1512), Rgb(0x2c231e), Rgb(0xf2e8df), Rgb(0xcdbcae), Rgb(0x4a3d34), Rgb(0xbfe3bb), Rgb(0xf4b4b4)),
        new("grape", true, Rgb(0x17141f), Rgb(0x272235), Rgb(0xefeaf9), Rgb(0xc4bbdb), Rgb(0x403856), Rgb(0xb0f5cf), Rgb(0xffb8c6)),
        new("retro", true, Rgb(0x282828), Rgb(0x3c3836), Rgb(0xebdbb2), Rgb(0xd5c4a1), Rgb(0x665c54), Rgb(0xd5d77a), Rgb(0xfc9a8f)),
        new("terminal", true, Rgb(0x030603), Rgb(0x0c170c), Rgb(0xb9f7c3), Rgb(0x86d996), Rgb(0x1f4a27), Rgb(0x9dfab0), Rgb(0xffb0b0)),
        new("contrast", true, Rgb(0x000000), Rgb(0x0d0d0d), Rgb(0xffffff), Rgb(0xffffff), Rgb(0x8a8a8a), Rgb(0x4dff88), Rgb(0xff8a8a)),
        new("aurora", true, Rgb(0x07101f), Rgb(0x141b2c), Rgb(0xf1f4fa), Rgb(0xc9d0de), Rgb(0x3a4458), Rgb(0xa7f3d0), Rgb(0xfecdd3)),
        new("sunset", true, Rgb(0x3a1d4f), Rgb(0x3a1a40), Rgb(0xfff1ea), Rgb(0xf0cfc4), Rgb(0x6a3f5c), Rgb(0xa7f3d0), Rgb(0xfecdd3)),
        new("city", true, Rgb(0x0b0c22), Rgb(0x161830), Rgb(0xf1f4fa), Rgb(0xc9d0de), Rgb(0x363a5c), Rgb(0xa7f3d0), Rgb(0xfecdd3)),
        new("space", true, Rgb(0x05040f), Rgb(0x141223), Rgb(0xf1f4fa), Rgb(0xc9d0de), Rgb(0x34304a), Rgb(0xa7f3d0), Rgb(0xfecdd3)),
        new("mist", false, Rgb(0xdfe7e2), Rgb(0xeef2ef), Rgb(0x15241f), Rgb(0x3a4c45), Rgb(0xc3cfc9), Rgb(0x047857), Rgb(0xbe123c)),
        new("snow", false, Rgb(0xd9e8f5), Rgb(0xeef4fa), Rgb(0x122033), Rgb(0x36475f), Rgb(0xc2d0e0), Rgb(0x047857), Rgb(0xbe123c)),
        new("desert", false, Rgb(0xf6dcb2), Rgb(0xfbeedb), Rgb(0x2b1a0b), Rgb(0x56391d), Rgb(0xe2c8a4), Rgb(0x047857), Rgb(0xbe123c)),
        new("meadow", false, Rgb(0xcfeaf8), Rgb(0xeaf6fc), Rgb(0x13240f), Rgb(0x344b2e), Rgb(0xbcd8c4), Rgb(0x047857), Rgb(0xbe123c)),
        new("lava", true, Rgb(0x140806), Rgb(0x2a130d), Rgb(0xfbe9e1), Rgb(0xe0b9a8), Rgb(0x4d2a1f), Rgb(0xcdf0b8), Rgb(0xffb3b3)),
        new("neon", true, Rgb(0x0b0820), Rgb(0x1c1642), Rgb(0xf3eaff), Rgb(0xc7b8f0), Rgb(0x3a2f6e), Rgb(0xa9f8d3), Rgb(0xffb3c1)),
        new("gameboy", true, Rgb(0x0f380f), Rgb(0x245524), Rgb(0xc6e05a), Rgb(0x9bbc0f), Rgb(0x3a6a2a), Rgb(0xdff09a), Rgb(0xf8b8a8)),
        new("blueprint", true, Rgb(0x0f3a68), Rgb(0x175084), Rgb(0xeef6ff), Rgb(0xc0dbf5), Rgb(0x3b77ad), Rgb(0xa7f3d0), Rgb(0xfecdd3)),
        new("candy", false, Rgb(0xfdf4fb), Rgb(0xfbeff8), Rgb(0x3b2340), Rgb(0x6b4a73), Rgb(0xecd3e6), Rgb(0x047857), Rgb(0xbe123c)),
        new("matcha", false, Rgb(0xf1f3e6), Rgb(0xf0f3e3), Rgb(0x263018), Rgb(0x4b5636), Rgb(0xd6dcbf), Rgb(0x047857), Rgb(0xbe123c)),
        new("newspaper", false, Rgb(0xf4f1ea), Rgb(0xf1eee6), Rgb(0x111111), Rgb(0x333333), Rgb(0xcfc9bb), Rgb(0x047857), Rgb(0xbe123c)),
        new("halloween", true, Rgb(0x141018), Rgb(0x251d2b), Rgb(0xf4ece4), Rgb(0xcfc0b4), Rgb(0x3e3346), Rgb(0xcdf3ac), Rgb(0xffb3bc)),
        new("lunar", true, Rgb(0x2a0d0f), Rgb(0x421a1d), Rgb(0xfbeee0), Rgb(0xe6c9b0), Rgb(0x6e3a33), Rgb(0xc4ecc7), Rgb(0xffc4bd)),
        new("spring", false, Rgb(0xf2f7ec), Rgb(0xf1f7ea), Rgb(0x1f2d1a), Rgb(0x45573c), Rgb(0xcfdfbf), Rgb(0x047857), Rgb(0xbe123c)),
        new("sakura", false, Rgb(0xfbf0f2), Rgb(0xfcf1f3), Rgb(0x3a1f27), Rgb(0x6a4450), Rgb(0xe9cdd4), Rgb(0x047857), Rgb(0xbe123c)),
    ];

    /// <summary>Follows the Windows "app mode" (light or dark).</summary>
    public const string FollowWindows = "system";
    public const string Default = "dark";

    static Color Rgb(int rgb) => Color.FromArgb((rgb >> 16) & 0xff, (rgb >> 8) & 0xff, rgb & 0xff);

    /// <summary>A known theme id (or <see cref="FollowWindows"/>); anything else falls back to the default.</summary>
    public static string Normalize(string? id) =>
        id == FollowWindows || All.Any(t => t.Id == id) ? id! : Default;

    /// <summary>The theme in use: the chosen one, or for <see cref="FollowWindows"/> light or dark like Windows apps.</summary>
    public static Theme Current
    {
        get
        {
            string id = Prefs.Theme == FollowWindows ? (WindowsUsesLight() ? "light" : "dark") : Prefs.Theme;
            return All.FirstOrDefault(t => t.Id == id) ?? All.First(t => t.Id == Default);
        }
    }

    static bool WindowsUsesLight()
    {
        try
        {
            using var key = Registry.CurrentUser.OpenSubKey(@"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");
            return key?.GetValue("AppsUseLightTheme") is int light && light != 0;
        }
        catch (Exception ex) when (ex is System.Security.SecurityException or UnauthorizedAccessException or IOException)
        {
            return false;
        }
    }
}
