using System.Text.Json;

namespace RobloxManager;

/// <summary>
/// Release notes shipped inside the app (whatsnew.json, newest first, English and Vietnamese). After an update the
/// page shows what changed since the version the user ran before; build.bat also turns them into the GitHub notes.
/// </summary>
public static class WhatsNew
{
    /// <param name="En">Notes in no group (releases before 1.1.1 have only these), and <paramref name="Vi"/> the same.</param>
    /// <param name="Highlights">The release's headline features, shown first (none for most releases).</param>
    /// <param name="New">What is new, <paramref name="Improved"/> what works better, <paramref name="Fixed"/> what was
    /// wrong and is fixed: each shown under its own heading.</param>
    public sealed record Release(string Version, string Date, string[]? En = null, string[]? Vi = null, Highlight[]? Highlights = null,
        Notes? New = null, Notes? Improved = null, Notes? Fixed = null);

    /// <summary>A group of notes, in each language.</summary>
    public sealed record Notes(string[] En, string[] Vi);

    /// <summary>
    /// A headline feature: an icon (one of the page's), [title, one or two sentences, button] in each language, and where
    /// the button leads ("settings:&lt;page or group&gt;", "guide", "web", "feedback"; none = no button).
    /// </summary>
    public sealed record Highlight(string Icon, string? Go, string[] En, string[] Vi);

    static readonly Lazy<Release[]> Releases = new(() =>
    {
        try
        {
            using var stream = typeof(WhatsNew).Assembly.GetManifestResourceStream("whatsnew.json");
            return stream is null ? [] : JsonSerializer.Deserialize<Release[]>(stream, new JsonSerializerOptions(JsonSerializerDefaults.Web)) ?? [];
        }
        catch (JsonException ex)
        {
            Log.Write(ex);
            return [];
        }
    });

    /// <summary>"1.2.3" as a comparable version (a "-beta" or "+build" suffix is ignored); unreadable text is 0.0.</summary>
    public static Version Parse(string? version) =>
        Version.TryParse((version ?? "").Split('-', '+')[0], out var v) ? v : new Version(0, 0);

    /// <summary>Releases newer than <paramref name="after"/> (all of them when null) up to <paramref name="upTo"/>.</summary>
    public static Release[] Between(string? after, string upTo) =>
        Releases.Value.Where(r => (after is null || Parse(r.Version) > Parse(after)) && Parse(r.Version) <= Parse(upTo)).ToArray();
}
