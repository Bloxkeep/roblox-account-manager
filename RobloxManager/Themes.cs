using Microsoft.Win32;

namespace RobloxManager;

/// <summary>
/// The colour themes. The page draws itself from the tokens in app.css; the window frame, its title bar and the
/// sign-in window need a few of the same colours, which are repeated here (keep the two in step).
/// </summary>
public static class Themes
{
    public sealed record Theme(string Id, bool Dark, Color Background, Color Surface, Color Text, Color Text2, Color Line, Color Good, Color Bad);

    // Background and Surface are app.css's --bg and --surface; Text/Text2 its --text/--text-2. For the glass themes
    // (a scene behind a translucent page) Background is the page's top edge as it actually shows — the scene's top
    // colour under the page's white or dark wash — so the title bar runs into it; their Surface is a solid stand-in.
    static readonly Theme[] All =
    [
        new("light", false, Rgb(0xf3f4f7), Rgb(0xffffff), Rgb(0x14171f), Rgb(0x3d4352), Rgb(0xd5d8df), Rgb(0x047857), Rgb(0x9f1239)),
        new("paper", false, Rgb(0xf3eee3), Rgb(0xfbf8f1), Rgb(0x2a241a), Rgb(0x50473a), Rgb(0xdcd3c1), Rgb(0x047857), Rgb(0x9f1239)),
        new("dim", true, Rgb(0x1d2026), Rgb(0x24272e), Rgb(0xe6e8ec), Rgb(0xbabec8), Rgb(0x464b56), Rgb(0x86efac), Rgb(0xfda4af)),
        new("dark", true, Rgb(0x0a0c11), Rgb(0x11151e), Rgb(0xe9ecf3), Rgb(0xb6bccc), Rgb(0x3c4456), Rgb(0x86efac), Rgb(0xfda4af)),
        new("black", true, Rgb(0x000000), Rgb(0x0c0d10), Rgb(0xeceef2), Rgb(0xb4b8c2), Rgb(0x34363d), Rgb(0x86efac), Rgb(0xfda4af)),
        new("frost", true, Rgb(0x2e3440), Rgb(0x3b4252), Rgb(0xeceff4), Rgb(0xd8dee9), Rgb(0x4c566a), Rgb(0xc5d9b4), Rgb(0xf0b9be)),
        new("ocean", true, Rgb(0x0a1622), Rgb(0x0f2030), Rgb(0xe3eef7), Rgb(0xa9c1d4), Rgb(0x24425a), Rgb(0xa7f3d0), Rgb(0xfecdd3)),
        new("forest", true, Rgb(0x0d1410), Rgb(0x142019), Rgb(0xe4efe6), Rgb(0xb3c7b8), Rgb(0x2e4435), Rgb(0xa7f3d0), Rgb(0xfecdd3)),
        new("mocha", true, Rgb(0x1b1512), Rgb(0x241c18), Rgb(0xf2e8df), Rgb(0xcdbcae), Rgb(0x4a3d34), Rgb(0xbfe3bb), Rgb(0xf4b4b4)),
        new("grape", true, Rgb(0x17141f), Rgb(0x201c2b), Rgb(0xefeaf9), Rgb(0xc4bbdb), Rgb(0x403856), Rgb(0xb0f5cf), Rgb(0xffb8c6)),
        new("retro", false, Rgb(0x008080), Rgb(0xc0c0c0), Rgb(0x000000), Rgb(0x1a1a1a), Rgb(0x808080), Rgb(0x007a00), Rgb(0xc00000)),
        new("terminal", true, Rgb(0x030603), Rgb(0x081008), Rgb(0xb9f7c3), Rgb(0x86d996), Rgb(0x1f4a27), Rgb(0x9dfab0), Rgb(0xffb0b0)),
        // Text2 must differ from Text: it is what a disabled menu item is drawn in.
        new("contrast", true, Rgb(0x000000), Rgb(0x000000), Rgb(0xffffff), Rgb(0xc4c4c4), Rgb(0x8a8a8a), Rgb(0x4dff88), Rgb(0xff8a8a)),
        new("aurora", true, Rgb(0x040813), Rgb(0x141b2c), Rgb(0xf1f4fa), Rgb(0xc9d0de), Rgb(0x3a4458), Rgb(0xa7f3d0), Rgb(0xfecdd3)),
        new("sunset", true, Rgb(0x22133f), Rgb(0x3a1a40), Rgb(0xfff1ea), Rgb(0xf0cfc4), Rgb(0x6a3f5c), Rgb(0xa7f3d0), Rgb(0xfecdd3)),
        new("city", true, Rgb(0x050718), Rgb(0x161830), Rgb(0xf1f4fa), Rgb(0xc9d0de), Rgb(0x363a5c), Rgb(0xa7f3d0), Rgb(0xfecdd3)),
        new("space", true, Rgb(0x05040f), Rgb(0x141223), Rgb(0xf1f4fa), Rgb(0xc9d0de), Rgb(0x34304a), Rgb(0xa7f3d0), Rgb(0xfecdd3)),
        new("mist", false, Rgb(0xcfdad5), Rgb(0xeef2ef), Rgb(0x15241f), Rgb(0x3a4c45), Rgb(0xc3cfc9), Rgb(0x047857), Rgb(0x9f1239)),
        new("snow", false, Rgb(0x8cb5dd), Rgb(0xeef4fa), Rgb(0x122033), Rgb(0x36475f), Rgb(0xc2d0e0), Rgb(0x047857), Rgb(0x9f1239)),
        new("desert", false, Rgb(0xf2c595), Rgb(0xfbeedb), Rgb(0x2b1a0b), Rgb(0x56391d), Rgb(0xe2c8a4), Rgb(0x047857), Rgb(0x9f1239)),
        new("meadow", false, Rgb(0x89c9f0), Rgb(0xeaf6fc), Rgb(0x13240f), Rgb(0x344b2e), Rgb(0xbcd8c4), Rgb(0x047857), Rgb(0x9f1239)),
        new("lava", true, Rgb(0x140806), Rgb(0x1f0e0a), Rgb(0xfbe9e1), Rgb(0xe0b9a8), Rgb(0x4d2a1f), Rgb(0xcdf0b8), Rgb(0xffb3b3)),
        new("neon", true, Rgb(0x0b0820), Rgb(0x151033), Rgb(0xf3eaff), Rgb(0xc7b8f0), Rgb(0x3a2f6e), Rgb(0xa9f8d3), Rgb(0xffb3c1)),
        new("gameboy", false, Rgb(0x9bbc0f), Rgb(0x8bac0f), Rgb(0x0f380f), Rgb(0x1d4a1d), Rgb(0x306230), Rgb(0x0f380f), Rgb(0x8a2a10)),
        new("blueprint", true, Rgb(0x0f3a68), Rgb(0x134577), Rgb(0xeef6ff), Rgb(0xc0dbf5), Rgb(0x3b77ad), Rgb(0xa7f3d0), Rgb(0xfecdd3)),
        new("candy", false, Rgb(0xfdf4fb), Rgb(0xfffafe), Rgb(0x3b2340), Rgb(0x6b4a73), Rgb(0xecd3e6), Rgb(0x047857), Rgb(0x9f1239)),
        new("matcha", false, Rgb(0xf1f3e6), Rgb(0xfafbf3), Rgb(0x263018), Rgb(0x4b5636), Rgb(0xd6dcbf), Rgb(0x047857), Rgb(0x9f1239)),
        new("newspaper", false, Rgb(0xf4f1ea), Rgb(0xfbf9f4), Rgb(0x111111), Rgb(0x333333), Rgb(0xcfc9bb), Rgb(0x047857), Rgb(0x9f1239)),
        new("halloween", true, Rgb(0x0d0714), Rgb(0x1d1222), Rgb(0xf6eee6), Rgb(0xd6c7bb), Rgb(0x3e3346), Rgb(0xcdf3ac), Rgb(0xffb3bc)),
        new("lunar", true, Rgb(0x2c0709), Rgb(0x3a0c0f), Rgb(0xfbeee0), Rgb(0xe6c9b0), Rgb(0x6e3a33), Rgb(0xc4ecc7), Rgb(0xffc4bd)),
        new("spring", false, Rgb(0xdaeefa), Rgb(0xf3f8fb), Rgb(0x2a1d22), Rgb(0x5a4048), Rgb(0xe2d2d8), Rgb(0x047857), Rgb(0x9f1239)),
        new("sakura", false, Rgb(0xfbf0f2), Rgb(0xfffafb), Rgb(0x3a1f27), Rgb(0x6a4450), Rgb(0xe9cdd4), Rgb(0x047857), Rgb(0x9f1239)),
        new("summer", false, Rgb(0xa0ddf5), Rgb(0xeef8fc), Rgb(0x0f2a3a), Rgb(0x2e4d5e), Rgb(0xc6dde6), Rgb(0x047857), Rgb(0x9f1239)),
        new("autumn", false, Rgb(0xf7dfbe), Rgb(0xfbf0e0), Rgb(0x2f1a0c), Rgb(0x5a3a22), Rgb(0xe6cfb4), Rgb(0x047857), Rgb(0x9f1239)),
        new("midautumn", true, Rgb(0x07112c), Rgb(0x13224a), Rgb(0xeef2fb), Rgb(0xc4cde2), Rgb(0x34406a), Rgb(0xa7f3d0), Rgb(0xfecdd3)),
        new("christmas", true, Rgb(0x091426), Rgb(0x14263d), Rgb(0xeef4fb), Rgb(0xc6d3e2), Rgb(0x33465e), Rgb(0x6fdc8c), Rgb(0xfecdd3)),
        new("newyear", true, Rgb(0x070714), Rgb(0x14112e), Rgb(0xf3f1fb), Rgb(0xcbc6e2), Rgb(0x36325a), Rgb(0xa7f3d0), Rgb(0xfecdd3)),
        new("valentine", true, Rgb(0x260f2d), Rgb(0x41153f), Rgb(0xfdeef3), Rgb(0xecc6d4), Rgb(0x764d6e), Rgb(0xa7f3d0), Rgb(0xfecdd3)),
        new("rainy", true, Rgb(0x0b141e), Rgb(0x14222f), Rgb(0xe8eff6), Rgb(0xb8c7d6), Rgb(0x495765), Rgb(0xa7f3d0), Rgb(0xfecdd3)),
        new("skylanterns", true, Rgb(0x080c22), Rgb(0x12193f), Rgb(0xeef0fb), Rgb(0xc4c9e2), Rgb(0x4c5275), Rgb(0xa7f3d0), Rgb(0xfecdd3)),
        new("hoian", true, Rgb(0x0d0f25), Rgb(0x181737), Rgb(0xfbf1e4), Rgb(0xe2cdb2), Rgb(0x595362), Rgb(0xa7f3d0), Rgb(0xfecdd3)),
        new("underwater", true, Rgb(0x09556e), Rgb(0x073452), Rgb(0xe8f6fc), Rgb(0xb4d6e6), Rgb(0x406982), Rgb(0xa7f3d0), Rgb(0xfecdd3)),
        new("jungle", true, Rgb(0x0b261e), Rgb(0x103224), Rgb(0xe8f6ee), Rgb(0xb6d6c2), Rgb(0x49695a), Rgb(0xa7f3d0), Rgb(0xfecdd3)),
        new("moon", true, Rgb(0x03040b), Rgb(0x0e101e), Rgb(0xf0f2f8), Rgb(0xc8ccd8), Rgb(0x494b59), Rgb(0xa7f3d0), Rgb(0xfecdd3)),
        new("winter", false, Rgb(0xd6e4f5), Rgb(0xf7f8fc), Rgb(0x14233a), Rgb(0x3a4c66), Rgb(0xdbdee3), Rgb(0x047857), Rgb(0x9f1239)),
        new("school", false, Rgb(0xb4dff7), Rgb(0xeef8fd), Rgb(0x10283a), Rgb(0x2e4a5e), Rgb(0xd9e0e5), Rgb(0x047857), Rgb(0x9f1239)),
        new("kites", false, Rgb(0x99d3f3), Rgb(0xe9f6fc), Rgb(0x0f2a3a), Rgb(0x2e4d5e), Rgb(0xd9e0e5), Rgb(0x047857), Rgb(0x9f1239)),
        new("dalat", false, Rgb(0xe3eff1), Rgb(0xf3f8f6), Rgb(0x12291f), Rgb(0x2f4a3f), Rgb(0xd9e0dd), Rgb(0x047857), Rgb(0x9f1239)),
        new("savanna", false, Rgb(0xf7d6ad), Rgb(0xfdf5e8), Rgb(0x2a1a08), Rgb(0x54391b), Rgb(0xe4dccf), Rgb(0x047857), Rgb(0x9f1239)),
        new("phongnha", true, Rgb(0x101e22), Rgb(0x0c1e20), Rgb(0xe6f4ec), Rgb(0xb4d2c4), Rgb(0x44605a), Rgb(0xa7f3d0), Rgb(0xfecdd3)),
        new("nightbeach", true, Rgb(0x070f25), Rgb(0x0e1832), Rgb(0xeaeef8), Rgb(0xbcc6dc), Rgb(0x454f6a), Rgb(0xa7f3d0), Rgb(0xfecdd3)),
        new("coral", false, Rgb(0xfbefe9), Rgb(0xfff9f5), Rgb(0x2e1a14), Rgb(0x5c3c32), Rgb(0xecd8cd), Rgb(0x047857), Rgb(0x9f1239)),
        new("rose", true, Rgb(0x1d1216), Rgb(0x27181e), Rgb(0xf6e7ec), Rgb(0xd6bcc6), Rgb(0x4d3540), Rgb(0xa7f3d0), Rgb(0xfecdd3)),
        new("olive", true, Rgb(0x17180f), Rgb(0x202216), Rgb(0xeeeedd), Rgb(0xc8c9ad), Rgb(0x42452f), Rgb(0xa7f3d0), Rgb(0xfecdd3)),
        new("solarized", true, Rgb(0x002b36), Rgb(0x073642), Rgb(0xeee8d5), Rgb(0xc3c8bd), Rgb(0x24535f), Rgb(0xa7f3d0), Rgb(0xfecdd3)),
        new("blockworld", false, Rgb(0xa0dcf7), Rgb(0xf4faf6), Rgb(0x13241a), Rgb(0x2f4a36), Rgb(0xd9e0dc), Rgb(0x047857), Rgb(0x9f1239)),
        new("peachfall", false, Rgb(0xfdf1f2), Rgb(0xfff8f9), Rgb(0x2e141c), Rgb(0x5a3040), Rgb(0xe6ddde), Rgb(0x047857), Rgb(0x9f1239)),
        new("greatwave", false, Rgb(0xd7ccaf), Rgb(0xf8f4ea), Rgb(0x0f2340), Rgb(0x2c4568), Rgb(0xdcdbd6), Rgb(0x047857), Rgb(0x9f1239)),
        new("waterlilies", false, Rgb(0x799ea2), Rgb(0xf2f7f8), Rgb(0x10242e), Rgb(0x2c4756), Rgb(0xd7dee0), Rgb(0x047857), Rgb(0x9f1239)),
        new("blocksart", false, Rgb(0xf7f3e7), Rgb(0xfbf9f2), Rgb(0x141414), Rgb(0x333333), Rgb(0xdfded7), Rgb(0x047857), Rgb(0x9f1239)),
        new("miaminight", true, Rgb(0x1f2356), Rgb(0x14103a), Rgb(0xf2eefb), Rgb(0xcbc4e6), Rgb(0x423c66), Rgb(0xa7f3d0), Rgb(0xfecdd3)),
        new("lanternriver", true, Rgb(0x08091a), Rgb(0x0e0f22), Rgb(0xf1eff6), Rgb(0xcac7d8), Rgb(0x3f3e52), Rgb(0xa7f3d0), Rgb(0xfecdd3)),
        new("deepsea", true, Rgb(0x0d3560), Rgb(0x061c3a), Rgb(0xe8f4fc), Rgb(0xb9d5e8), Rgb(0x36506a), Rgb(0xa7f3d0), Rgb(0xfecdd3)),
        new("snowfall", true, Rgb(0x4a525c), Rgb(0x121c28), Rgb(0xeef4fa), Rgb(0xc4d4e4), Rgb(0x3e4e60), Rgb(0xa7f3d0), Rgb(0xfecdd3)),
        new("nightharbour", true, Rgb(0x3a3238), Rgb(0x18121e), Rgb(0xf3eef6), Rgb(0xcfc6d6), Rgb(0x463e50), Rgb(0xa7f3d0), Rgb(0xfecdd3)),
        new("koipond", true, Rgb(0x0e4646), Rgb(0x0a2a2c), Rgb(0xe8f5f1), Rgb(0xb8d6cf), Rgb(0x3b5757), Rgb(0xa7f3d0), Rgb(0xfecdd3)),
        new("fireflies", true, Rgb(0x091b28), Rgb(0x0c1a1c), Rgb(0xeef5df), Rgb(0xc3d4ae), Rgb(0x3e4a47), Rgb(0xa7f3d0), Rgb(0xfecdd3)),
        new("rainglass", true, Rgb(0x0b101f), Rgb(0x101626), Rgb(0xe9eef8), Rgb(0xbcc6da), Rgb(0x404654), Rgb(0xa7f3d0), Rgb(0xfecdd3)),
        new("starrynight", true, Rgb(0x122c66), Rgb(0x0e1a3e), Rgb(0xeef1fa), Rgb(0xc2cbe2), Rgb(0x3f4967), Rgb(0xa7f3d0), Rgb(0xfecdd3)),
        new("cubism", true, Rgb(0x352c23), Rgb(0x26201a), Rgb(0xf3ead8), Rgb(0xd2c6ae), Rgb(0x534c44), Rgb(0xa7f3d0), Rgb(0xfecdd3)),
        new("goldleaf", true, Rgb(0x191109), Rgb(0x20160a), Rgb(0xf7ecd0), Rgb(0xd8c89e), Rgb(0x4f4536), Rgb(0xa7f3d0), Rgb(0xfecdd3)),
        new("flametree", false, Rgb(0xb3def6), Rgb(0xf0f9fd), Rgb(0x2a1612), Rgb(0x5a3a30), Rgb(0xe5dbd9), Rgb(0x047857), Rgb(0x9f1239)),
        new("halong", false, Rgb(0xcde8f0), Rgb(0xf3f9fa), Rgb(0x102a26), Rgb(0x2f4c47), Rgb(0xd9e0df), Rgb(0x047857), Rgb(0x9f1239)),
        new("terraces", false, Rgb(0xfae0bb), Rgb(0xfdf5e8), Rgb(0x262a10), Rgb(0x4b5230), Rgb(0xe1e0d9), Rgb(0x047857), Rgb(0x9f1239)),
        new("fuji", false, Rgb(0xf9dcdf), Rgb(0xfdf4f4), Rgb(0x2a1a22), Rgb(0x5a404a), Rgb(0xe1dbdd), Rgb(0x047857), Rgb(0x9f1239)),
        new("skyislands", false, Rgb(0xabd8f6), Rgb(0xe9f5fc), Rgb(0x0f2a3a), Rgb(0x2e4d5e), Rgb(0xd9e0e5), Rgb(0x047857), Rgb(0x9f1239)),
        new("gray", false, Rgb(0xe9eaec), Rgb(0xf4f4f5), Rgb(0x18191c), Rgb(0x3c3e44), Rgb(0xd2d2d4), Rgb(0x047857), Rgb(0x9f1239)),
        new("midnight", true, Rgb(0x0b1020), Rgb(0x111833), Rgb(0xe6ebfa), Rgb(0xb3bedc), Rgb(0x38405c), Rgb(0x86efac), Rgb(0xfda4af)),
        new("contrastlight", false, Rgb(0xffffff), Rgb(0xffffff), Rgb(0x000000), Rgb(0x1a1a1a), Rgb(0xd6d6d6), Rgb(0x047857), Rgb(0x9f1239)),
        new("indigo", true, Rgb(0x12122a), Rgb(0x1a1a3a), Rgb(0xececfb), Rgb(0xc0c0e6), Rgb(0x424261), Rgb(0x86efac), Rgb(0xfda4af)),
        new("crimson", true, Rgb(0x1c0b10), Rgb(0x261018), Rgb(0xfbeaf0), Rgb(0xe2bcc8), Rgb(0x513941), Rgb(0x86efac), Rgb(0xfda4af)),
        new("teal", true, Rgb(0x071a1a), Rgb(0x0c2626), Rgb(0xe2f7f4), Rgb(0xa8d4cf), Rgb(0x34504f), Rgb(0x86efac), Rgb(0xfda4af)),
        new("amber", true, Rgb(0x1a1406), Rgb(0x241b0a), Rgb(0xfbf2e0), Rgb(0xe2cfa8), Rgb(0x504632), Rgb(0x86efac), Rgb(0xfda4af)),
        new("sky", false, Rgb(0xeef6fc), Rgb(0xfafdff), Rgb(0x10243a), Rgb(0x34506a), Rgb(0xd5dee6), Rgb(0x047857), Rgb(0x9f1239)),
        new("lavender", false, Rgb(0xf5f2fc), Rgb(0xfdfcff), Rgb(0x241a38), Rgb(0x4e4068), Rgb(0xded8e6), Rgb(0x047857), Rgb(0x9f1239)),
        new("peach", false, Rgb(0xfdf3ec), Rgb(0xfffaf6), Rgb(0x36200f), Rgb(0x64422a), Rgb(0xe6dad2), Rgb(0x047857), Rgb(0x9f1239)),
        new("mint", false, Rgb(0xeffaf5), Rgb(0xfbfffd), Rgb(0x0f2a1e), Rgb(0x2e5442), Rgb(0xd6e3dd), Rgb(0x047857), Rgb(0x9f1239)),
        new("notebook", false, Rgb(0xfdfcf6), Rgb(0xfffffb), Rgb(0x1f2a44), Rgb(0x3e4a66), Rgb(0xdedcd2), Rgb(0x047857), Rgb(0x9f1239)),
        new("chalkboard", true, Rgb(0x2a3d34), Rgb(0x2f443a), Rgb(0xf4f4ee), Rgb(0xdcdcd2), Rgb(0x6e7e74), Rgb(0xa8e6a1), Rgb(0xf7a8a8)),
        new("brutal", false, Rgb(0xfff6dc), Rgb(0xffffff), Rgb(0x000000), Rgb(0x1a1a1a), Rgb(0x000000), Rgb(0x047857), Rgb(0x9f1239)),
        new("arcade", true, Rgb(0x0a0a12), Rgb(0x14142a), Rgb(0xffffff), Rgb(0xd8d8f0), Rgb(0x3a3a60), Rgb(0x38c838), Rgb(0xe83a3a)),
        new("cyberpunk", true, Rgb(0x0b0b10), Rgb(0x15151d), Rgb(0xf2f2f8), Rgb(0xc8c8d6), Rgb(0x3a3a4a), Rgb(0x86efac), Rgb(0xfda4af)),
        new("macclassic", false, Rgb(0x808080), Rgb(0xffffff), Rgb(0x000000), Rgb(0x000000), Rgb(0x000000), Rgb(0x000000), Rgb(0x000000)),
        new("macos", false, Rgb(0xf5f5f7), Rgb(0xffffff), Rgb(0x1d1d1f), Rgb(0x3c3c43), Rgb(0xd8d8dc), Rgb(0x047857), Rgb(0x9f1239)),
        new("macosdark", true, Rgb(0x1e1e1e), Rgb(0x2c2c2e), Rgb(0xf5f5f7), Rgb(0xd1d1d6), Rgb(0x48484a), Rgb(0x86efac), Rgb(0xfda4af)),
        new("gnome", false, Rgb(0xfafafa), Rgb(0xffffff), Rgb(0x2e2e2e), Rgb(0x464646), Rgb(0xdedede), Rgb(0x047857), Rgb(0x9f1239)),
        new("ubuntu", false, Rgb(0xfafafa), Rgb(0xffffff), Rgb(0x3d3d3d), Rgb(0x4f4f4f), Rgb(0xd6d6d6), Rgb(0x047857), Rgb(0x9f1239)),
        new("kde", true, Rgb(0x202326), Rgb(0x292c30), Rgb(0xfcfcfc), Rgb(0xd6d8da), Rgb(0x4a4f54), Rgb(0x86efac), Rgb(0xfda4af)),
    ];

    /// <summary>Follows the Windows "app mode" (light or dark).</summary>
    public const string FollowWindows = "system";
    public const string Default = "dark";
    /// <summary>The person's own colours and picture (<see cref="Prefs.Custom"/>); the page works out the rest (app.js customVars).</summary>
    public const string Custom = "custom";

    static Color Rgb(int rgb) => Color.FromArgb((rgb >> 16) & 0xff, (rgb >> 8) & 0xff, rgb & 0xff);

    /// <summary>A known theme id (or <see cref="FollowWindows"/>); anything else falls back to the default.</summary>
    public static string Normalize(string? id) =>
        id == FollowWindows || id == Custom || All.Any(t => t.Id == id) ? id! : Default;

    /// <summary>The theme in use: the chosen one, or for <see cref="FollowWindows"/> light or dark like Windows apps.</summary>
    public static Theme Current
    {
        get
        {
            string id = Prefs.Theme == FollowWindows ? (WindowsUsesLight() ? "light" : "dark") : Prefs.Theme;
            return id == Custom ? FromCustom(Prefs.Custom) : All.FirstOrDefault(t => t.Id == id) ?? All.First(t => t.Id == Default);
        }
    }

    /// <summary>The custom theme for native parts (frame, title bar, mini window): dark or light by its colour as the page
    /// decides, surfaces a step from it, text and status colours from Dark or Light.</summary>
    static Theme FromCustom(Prefs.CustomTheme custom)
    {
        var bg = Rgb(Convert.ToInt32(custom.Background[1..], 16));
        bool dark = IsDark(bg);
        var basis = All.First(t => t.Id == (dark ? "dark" : "light"));
        return new(Custom, dark, bg, Mix(bg, Color.White, dark ? .06 : .7), basis.Text, basis.Text2,
            Mix(bg, dark ? Color.White : Color.Black, .2), basis.Good, basis.Bad);
    }

    /// <summary>White words read better on it than black ones (relative luminance under 0.179); app.js decides the same way.</summary>
    public static bool IsDark(Color c)
    {
        static double Linear(int v) => v / 255.0 is var s && s <= .04045 ? s / 12.92 : Math.Pow((s + .055) / 1.055, 2.4);
        return .2126 * Linear(c.R) + .7152 * Linear(c.G) + .0722 * Linear(c.B) < .179;
    }

    static Color Mix(Color a, Color b, double t) =>
        Color.FromArgb((int)Math.Round(a.R + (b.R - a.R) * t), (int)Math.Round(a.G + (b.G - a.G) * t), (int)Math.Round(a.B + (b.B - a.B) * t));

    /// <summary>"#rrggbb" in lower case, or null for anything else.</summary>
    public static string? Hex(string? text) =>
        text is { Length: 7 } && text[0] == '#' && text.Skip(1).All(Uri.IsHexDigit) ? text.ToLowerInvariant() : null;

    /// <summary>The only files the page may load from the pictures folder: the custom theme's (theme-background-&lt;time&gt;.png…)
    /// and each vault's (vault-picture-&lt;id&gt;-&lt;time&gt;.png…).</summary>
    public static bool IsPictureName(string name) =>
        IsThemeMediaName(name) || System.Text.RegularExpressions.Regex.IsMatch(name, @"^vault-picture-[0-9a-f]{8}-\d{1,20}\.(png|jpe?g|webp|gif|bmp)$");

    /// <summary>The custom theme's picture or video in the data folder, by its own kind of name.</summary>
    public static bool IsThemeMediaName(string name) =>
        System.Text.RegularExpressions.Regex.IsMatch(name, @"^theme-background-\d{1,20}\.(png|jpe?g|webp|gif|bmp|mp4|webm)$");

    /// <summary>A video, not a picture, behind the page.</summary>
    public static bool IsVideo(string? name) => name is not null && (name.EndsWith(".mp4", StringComparison.OrdinalIgnoreCase) || name.EndsWith(".webm", StringComparison.OrdinalIgnoreCase));

    /// <summary>How many custom themes are kept by name, and the most the picture is blurred (px).</summary>
    public const int MaxSaved = 30, MaxBlur = 30;

    /// <summary>A saved theme's name: trimmed, without control characters, 1 to 40 characters; null when there is none.</summary>
    public static string? ThemeName(string? text)
    {
        string name = new string((text ?? "").Where(ch => !char.IsControl(ch)).ToArray()).Trim();
        return name.Length == 0 ? null : name.Length > 40 ? name[..40].TrimEnd() : name;
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
