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

    /// <summary>
    /// Releases newer than <paramref name="after"/> (all of them when null) up to <paramref name="upTo"/>. A quiet fix
    /// comes before the version it leads to: after one, that version's notes are new; up to one, they aren't yet.
    /// </summary>
    public static Release[] Between(string? after, string upTo) =>
        Releases.Value.Where(r => (after is null || Parse(r.Version) > Parse(after) || (IsQuietFix(after) && Parse(r.Version) == Parse(after)))
            && (Parse(r.Version) < Parse(upTo) || (Parse(r.Version) == Parse(upTo) && !IsQuietFix(upTo)))).ToArray();

    /// <summary>
    /// A quiet fix ("1.0.7-alpha.2"): a small fix after a public version (1.0.6), put inside that version's GitHub
    /// release instead of a release of its own (release-fix.ps1), installed in the background without "Restart to
    /// update" or notes, and shown as the version it fixes. Numbered below the next version's betas and the next
    /// version itself, so both still update over it.
    /// </summary>
    public static bool IsQuietFix(string? version) =>
        (version ?? "").Split('+')[0].Split('-', 2) is [_, var label] && label.StartsWith("alpha.", StringComparison.Ordinal);

    /// <summary>Whether <paramref name="a"/> is older than <paramref name="b"/>: a quiet fix before the version it leads to, an earlier quiet fix before a later one.</summary>
    public static bool Older(string a, string b)
    {
        static (Version Version, int Fix) Key(string v) =>
            (Parse(v), IsQuietFix(v) && int.TryParse(v.Split('+')[0].Split("-alpha.")[1], out int n) ? n : int.MaxValue);
        var (x, y) = (Key(a), Key(b));
        return x.Version < y.Version || (x.Version == y.Version && x.Fix < y.Fix);
    }

    /// <summary>The version as people see it: a quiet fix shows as the public version it fixes (1.0.7-alpha.2 is 1.0.6).</summary>
    public static string Shown(string version) => !IsQuietFix(version) ? version
        : Releases.Value.Select(r => r.Version).FirstOrDefault(v => !v.Contains('-') && Parse(v) < Parse(version)) ?? version;
}
