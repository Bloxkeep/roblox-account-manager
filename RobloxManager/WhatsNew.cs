using System.Text.Json;

namespace RobloxManager;

/// <summary>Release notes shipped in the app (whatsnew.json, newest first, EN and VI), shown after an update; build.bat
/// also turns them into the GitHub notes.</summary>
public static class WhatsNew
{
    /// <param name="En">Notes in no group (releases before 1.1.1 have only these), and <paramref name="Vi"/> the same.</param>
    /// <param name="Highlights">The release's headline features, shown first (none for most releases).</param>
    /// <param name="New">What is new, <paramref name="Improved"/> what works better, <paramref name="Fixed"/> what was
    /// wrong and is fixed: each shown under its own heading.</param>
    public sealed record Release(string Version, string Date, string[]? En = null, string[]? Vi = null, Highlight[]? Highlights = null,
        Notes? New = null, Notes? Improved = null, Notes? Fixed = null, Notes? Removed = null);

    /// <summary>A group of notes, in each language.</summary>
    public sealed record Notes(string[] En, string[] Vi);

    /// <summary>A headline feature: an icon (no longer shown), [title, one or two sentences, button] per language, where the
    /// button leads ("settings:&lt;page or group&gt;", "guide", "web", "feedback", "discord"; none = no button), and its
    /// picture under wwwroot/whatsnew ("{lang}" is en or vi).</summary>
    public sealed record Highlight(string Icon, string? Go, string[] En, string[] Vi, string? Image = null);

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

    /// <summary>Releases newer than <paramref name="after"/> (all when null) up to <paramref name="upTo"/>, in the order of
    /// <see cref="Older"/>.</summary>
    public static Release[] Between(string? after, string upTo) =>
        Releases.Value.Where(r => (after is null || Older(after, r.Version)) && !Older(upTo, r.Version)).ToArray();

    /// <summary>Notes shown once after an update: the releases in between, none for a beta copy's next beta (it comes in
    /// quietly, see <see cref="IsQuietUpdate"/>).</summary>
    public static Release[] AfterUpdate(string last, string current) =>
        !Older(last, current) || (IsBeta(last) && IsBeta(current) && Parse(last) == Parse(current)) ? [] : Between(last, current);

    /// <summary>A quiet fix ("1.0.7-alpha.2"): a fix after a public version (1.0.6) put inside that version's GitHub release
    /// (release-fix.ps1), installed in the background without "Restart to update" or notes, shown as the version it
    /// fixes. Numbered below the next version's betas so they still update over it.</summary>
    public static bool IsQuietFix(string? version) =>
        (version ?? "").Split('+')[0].Split('-', 2) is [_, var label] && label.StartsWith("alpha.", StringComparison.Ordinal);

    static bool IsBeta(string? version) =>
        (version ?? "").Split('+')[0].Split('-', 2) is [_, var label] && label.StartsWith("beta.", StringComparison.Ordinal);

    /// <summary>
    /// An update that comes in quietly, without "Restart to update": a quiet fix, or the next beta of the version a beta
    /// copy is on (the owner's wish, 2026-10-21: "cập nhật ngầm"), whose notes don't show either (<see cref="Since"/>).
    /// </summary>
    public static bool IsQuietUpdate(string? next, string? current) =>
        IsQuietFix(next) || (IsBeta(current) && IsBeta(next) && Parse(next) == Parse(current));

    /// <summary>Velopack's order: a number's quiet fixes, then its betas (and other labels), then the version itself. A test
    /// build between two fixes (1.0.8-alpha.3.1) counts as the first. Until 2026-10-25 a beta and its version counted as
    /// the same, so 1.0.10-beta.3 → 1.0.10 showed no notes.</summary>
    public static bool Older(string a, string b)
    {
        static (Version Version, int Kind, int N) Key(string v)
        {
            string label = v.Split('+')[0].Split('-', 2) is [_, var l] ? l : "";
            int kind = label.Length == 0 ? 3 : IsQuietFix(v) ? 0 : IsBeta(v) ? 1 : 2;
            return (Parse(v), kind, kind < 3 && label.Split('.') is [_, var n, ..] && int.TryParse(n, out int at) ? at : int.MaxValue);
        }
        var (x, y) = (Key(a), Key(b));
        return x.Version < y.Version || (x.Version == y.Version && (x.Kind < y.Kind || (x.Kind == y.Kind && x.N < y.N)));
    }

    /// <summary>The version as people see it: a quiet fix shows as the public version it fixes (1.0.7-alpha.2 is 1.0.6).</summary>
    public static string Shown(string version) => !IsQuietFix(version) ? version
        : Releases.Value.Select(r => r.Version).FirstOrDefault(v => !v.Contains('-') && Parse(v) < Parse(version)) ?? version;
}
