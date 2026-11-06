using System.Drawing;

namespace RobloxManager;

/// <summary>How a game looks in the Wiki tab: its icon and wide picture from its Roblox page, and a colour taken from the
/// icon (its strongest hue, made bright enough to read on), with black or white words for on top of that colour.</summary>
public sealed record WikiLookInfo(string? Icon, string? Cover, string? Tint, string? Ink);

/// <summary>
/// Each game's look in the Wiki tab (<see cref="WikiLookInfo"/>), from the Roblox place the bot names for it. The pictures
/// come back as soon as Roblox gives their addresses; the colour, which needs each icon downloaded, is worked out after
/// (the tab asks again a moment later). All of it is kept on disk for a week, so the tab opens with its pictures at once
/// (2026-11-06: the owner found them "super slow", every game's icon downloaded before any picture showed, each app start).
/// Nothing when Roblox doesn't answer: the tab then shows the game in the app's own colours, and it is asked again later.
/// </summary>
static class WikiLook
{
    static readonly HttpClient Http = new(new SocketsHttpHandler { UseCookies = false }) { Timeout = TimeSpan.FromSeconds(10) };
    static readonly TimeSpan KeptFor = TimeSpan.FromDays(7);
    /// <summary>A place's look and when it was asked of Roblox; Tinted once its icon's colour is worked out (none is a
    /// colour too: an icon with next to none).</summary>
    sealed record Kept(string? Icon, string? Cover, string? Tint, string? Ink, bool Tinted, DateTime At);
    static readonly Dictionary<long, Kept> Known = [];
    static readonly SemaphoreSlim Gate = new(1, 1);
    static bool loaded;
    static string FilePath => Path.Combine(Vault.CacheDir, "game-looks.json");

    public static async Task<Dictionary<long, WikiLookInfo>> OfAsync(IEnumerable<long> places)
    {
        var wanted = places.Where(p => p > 0).Distinct().ToList();
        await Gate.WaitAsync();
        try
        {
            Load();
            List<long> missing;
            lock (Known)
                missing = wanted.Where(p => !Known.TryGetValue(p, out var k) || DateTime.UtcNow - k.At > (k.Icon is null && k.Cover is null ? TimeSpan.FromMinutes(5) : KeptFor)).ToList();
            if (missing.Count > 0)
            {
                var universes = await Task.WhenAll(missing.Select(async p => (Place: p, Universe: await Try(() => RobloxApi.GetUniverseIdAsync(p), 0L))));
                var ids = universes.Select(u => u.Universe).Where(u => u > 0).ToList();
                var icons = Try(() => RobloxApi.GetGameIconsAsync(ids), new Dictionary<long, string>());
                var covers = Try(() => RobloxApi.GetGameCoversAsync(ids), new Dictionary<long, string>());
                await Task.WhenAll(icons, covers);
                lock (Known)
                    foreach (var u in universes)
                    {
                        string? icon = icons.Result.GetValueOrDefault(u.Universe), cover = covers.Result.GetValueOrDefault(u.Universe);
                        // The colour stays while the icon does.
                        var had = Known.GetValueOrDefault(u.Place);
                        Known[u.Place] = had is { Tinted: true } && had.Icon == icon && icon is not null
                            ? had with { Cover = cover, At = DateTime.UtcNow }
                            : new Kept(icon, cover, null, null, icon is null, DateTime.UtcNow);
                    }
                Save();
            }
            List<(long Place, string Icon)> untinted;
            lock (Known)
                untinted = wanted.Where(p => Known.TryGetValue(p, out var k) && !k.Tinted && k.Icon is not null).Select(p => (p, Known[p].Icon!)).ToList();
            if (untinted.Count > 0)
                _ = Task.Run(() => TintAllAsync(untinted));
            lock (Known)
                return wanted.Where(Known.ContainsKey).ToDictionary(p => p, p => new WikiLookInfo(Known[p].Icon, Known[p].Cover, Known[p].Tint, Known[p].Ink));
        }
        finally
        {
            Gate.Release();
        }
    }

    /// <summary>The icons' colours, a few at a time, kept as each is worked out.</summary>
    static async Task TintAllAsync(List<(long Place, string Icon)> icons)
    {
        using var few = new SemaphoreSlim(6);
        await Task.WhenAll(icons.Select(async x =>
        {
            await few.WaitAsync();
            try
            {
                var (tint, ink) = await Try(() => TintAsync(x.Icon), ((string?)null, (string?)null));
                lock (Known)
                    if (Known.TryGetValue(x.Place, out var k) && k.Icon == x.Icon)
                        Known[x.Place] = k with { Tint = tint, Ink = ink, Tinted = true };
            }
            finally
            {
                few.Release();
            }
        }));
        Save();
    }

    static void Load()
    {
        if (loaded)
            return;
        loaded = true;
        try
        {
            if (File.Exists(FilePath) && System.Text.Json.JsonSerializer.Deserialize<Dictionary<long, Kept>>(File.ReadAllText(FilePath)) is { } kept)
                lock (Known)
                    foreach (var (place, k) in kept)
                        Known.TryAdd(place, k);
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException or System.Text.Json.JsonException or NotSupportedException)
        {
            Log.Note($"Game pictures: the kept list couldn't be read ({e.Message}); asked of Roblox again.");
        }
    }

    /// <summary>Only what Roblox answered is kept (a game it didn't is asked again next time).</summary>
    static void Save()
    {
        try
        {
            string text;
            lock (Known)
                text = System.Text.Json.JsonSerializer.Serialize(Known.Where(k => k.Value.Icon is not null || k.Value.Cover is not null).ToDictionary());
            Directory.CreateDirectory(Vault.CacheDir);
            string temp = FilePath + ".tmp";
            File.WriteAllText(temp, text);
            File.Move(temp, FilePath, overwrite: true);
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException)
        {
            Log.Note($"Game pictures: the list couldn't be kept ({e.Message}).");
        }
    }

    static async Task<T> Try<T>(Func<Task<T>> get, T otherwise)
    {
        try { return await get(); }
        catch (Exception e) when (e is HttpRequestException or TaskCanceledException or ArgumentException or InvalidOperationException) { return otherwise; }
    }

    static async Task<(string?, string?)> TintAsync(string iconUrl)
    {
        await using var stream = new MemoryStream(await Http.GetByteArrayAsync(iconUrl));
        using var bitmap = new Bitmap(stream);
        return Tint(bitmap);
    }

    /// <summary>The icon's strongest hue: each pixel's hue weighted by how coloured and how bright it is (greys, black and
    /// white count for nothing), as a colour of medium lightness. None for an icon with next to no colour.</summary>
    internal static (string? Tint, string? Ink) Tint(Bitmap bitmap)
    {
        double x = 0, y = 0, weight = 0, sat = 0, seen = 0;
        int step = Math.Max(1, Math.Min(bitmap.Width, bitmap.Height) / 60);
        for (int i = 0; i < bitmap.Width; i += step)
            for (int j = 0; j < bitmap.Height; j += step)
            {
                var c = bitmap.GetPixel(i, j);
                if (c.A < 128)
                    continue;
                seen++;
                double max = Math.Max(c.R, Math.Max(c.G, c.B)) / 255.0, min = Math.Min(c.R, Math.Min(c.G, c.B)) / 255.0;
                double s = max == 0 ? 0 : (max - min) / max, w = s * s * max;
                if (w < 0.02)
                    continue;
                double h = c.GetHue() * Math.PI / 180;
                x += Math.Cos(h) * w;
                y += Math.Sin(h) * w;
                sat += s * w;
                weight += w;
            }
        if (seen == 0 || weight / seen < 0.03)
            return (null, null);
        double hue = (Math.Atan2(y, x) * 180 / Math.PI + 360) % 360;
        double saturation = Math.Clamp(sat / weight, 0.45, 0.8);
        var (r, g, b) = FromHsl(hue, saturation, 0.55);
        double luminance = (0.2126 * Linear(r) + 0.7152 * Linear(g) + 0.0722 * Linear(b));
        return ($"#{r:x2}{g:x2}{b:x2}", luminance > 0.2 ? "#111111" : "#ffffff");
    }

    static double Linear(int c)
    {
        double v = c / 255.0;
        return v <= 0.04045 ? v / 12.92 : Math.Pow((v + 0.055) / 1.055, 2.4);
    }

    static (int, int, int) FromHsl(double h, double s, double l)
    {
        double c = (1 - Math.Abs(2 * l - 1)) * s, hp = h / 60, x = c * (1 - Math.Abs(hp % 2 - 1)), m = l - c / 2;
        var (r, g, b) = hp switch { < 1 => (c, x, 0d), < 2 => (x, c, 0d), < 3 => (0d, c, x), < 4 => (0d, x, c), < 5 => (x, 0d, c), _ => (c, 0d, x) };
        return ((int)Math.Round((r + m) * 255), (int)Math.Round((g + m) * 255), (int)Math.Round((b + m) * 255));
    }
}
