using System.Text.Json;

namespace RobloxManager;

/// <summary>
/// Release notes shipped inside the app (whatsnew.json, newest first, English and Vietnamese). After an update the
/// page shows what changed since the version the user ran before; build.bat also turns them into the GitHub notes.
/// </summary>
public static class WhatsNew
{
    public sealed record Release(string Version, string Date, string[] En, string[] Vi);

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
