using Velopack;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// Quiet fixes (WhatsNew.IsQuietFix, release-fix.ps1): small fixes after a public version, numbered 1.0.7-alpha.N after
/// 1.0.6. Velopack's own comparison puts them above that version and below the next one's betas and the next version
/// itself, so copies on betas or on the next version never go back to one; the app shows the version they fix, and
/// What's new treats them as coming before the version they lead to (no notes for a fix, the next version's after it).
/// </summary>
static class QuietFixSuite
{
    public static Task Run(Checks c, string dir)
    {
        SemanticVersion V(string v) => SemanticVersion.Parse(v);
        c.Check("Velopack takes a quiet fix over the version it fixes", V("1.0.7-alpha.1") > V("1.0.6"));
        c.Check("and a later quiet fix over an earlier one", V("1.0.7-alpha.2") > V("1.0.7-alpha.1") && V("1.0.7-alpha.10") > V("1.0.7-alpha.9"));
        c.Check("a beta of the next version stays above every quiet fix (beta copies don't go back)",
            V("1.0.7-beta.1") > V("1.0.7-alpha.99"));
        c.Check("the next version stays above every quiet fix", V("1.0.7") > V("1.0.7-alpha.99"));

        c.Check("a quiet fix is told apart from betas and versions",
            WhatsNew.IsQuietFix("1.0.7-alpha.2") && WhatsNew.IsQuietFix("1.0.7-alpha.2+abc123")
            && !WhatsNew.IsQuietFix("1.0.7-beta.1") && !WhatsNew.IsQuietFix("1.0.6") && !WhatsNew.IsQuietFix(null));
        c.Check("a quiet fix shows as the version it fixes", WhatsNew.Shown("1.0.7-alpha.2") == "1.0.6", WhatsNew.Shown("1.0.7-alpha.2"));
        c.Check("a version shows as itself", WhatsNew.Shown("1.0.6") == "1.0.6" && WhatsNew.Shown("1.0.7-beta.1") == "1.0.7-beta.1");

        c.Check("versions in order: the fixed version, its quiet fixes, the next version",
            WhatsNew.Older("1.0.6", "1.0.7-alpha.1") && WhatsNew.Older("1.0.7-alpha.1", "1.0.7-alpha.2")
            && WhatsNew.Older("1.0.7-alpha.2", "1.0.7") && !WhatsNew.Older("1.0.7", "1.0.7-alpha.2"));
        c.Check("a test build between two fixes counts as the first: the next fix and the next version are newer",
            WhatsNew.Older("1.0.8-alpha.3.1", "1.0.8-alpha.4") && WhatsNew.Older("1.0.8-alpha.3.1", "1.0.8")
            && !WhatsNew.Older("1.0.8-alpha.3", "1.0.8-alpha.3.1"));
        c.Check("a beta and its version stay as they were (no notes again after the betas)",
            !WhatsNew.Older("1.0.6-beta.8", "1.0.6") && !WhatsNew.Older("1.0.6", "1.0.6-beta.8"));

        string[] Versions(string? after, string upTo) => WhatsNew.Between(after, upTo).Select(r => r.Version).ToArray();
        c.Check("updated to a quiet fix: no notes to show", Versions("1.0.6", "1.0.7-alpha.1").Length == 0, string.Join(", ", Versions("1.0.6", "1.0.7-alpha.1")));
        c.Check("from an older version to a quiet fix: the public versions in between",
            Versions("1.0.5", "1.0.7-alpha.1").SequenceEqual(["1.0.6"]), string.Join(", ", Versions("1.0.5", "1.0.7-alpha.1")));
        c.Check("from a quiet fix to the version it leads to: that version's notes",
            Versions("1.0.6-alpha.3", "1.0.6").SequenceEqual(["1.0.6"]), string.Join(", ", Versions("1.0.6-alpha.3", "1.0.6")));
        c.Check("all the notes, as Settings' What's new lists them", Versions(null, "1.0.6").FirstOrDefault() == "1.0.6");

        // Bloxkeep clicked open while an update installs (2026-10-20, 8:43): that start waits for the update's run, which
        // carries the unlocked vault over, rather than taking its place. It knows by the fresh hand-over on disk.
        Harness.UseDataFolder(dir);
        string handOver = Path.Combine(Vault.DataDir, "resume.bin");
        c.Check("no update under way: a start goes ahead", !Relaunch.HandOverWaiting());
        Relaunch.Write(new Relaunch.State { WrittenAt = DateTime.UtcNow, To = "1.0.7-alpha.3", Window = "minimized" });
        c.Check("an update handing over right now: a start by hand waits for its run", Relaunch.HandOverWaiting());
        c.Check("the update's own run (and the watch's) doesn't wait for itself",
            Relaunch.IsUpdateRun(["--resume", "ab12"]) && Relaunch.IsUpdateRun(["--after-update", "1.0.7-alpha.3"]) && !Relaunch.IsUpdateRun([]));
        File.SetLastWriteTimeUtc(handOver, DateTime.UtcNow.AddMinutes(-10));
        c.Check("a hand-over left from long ago doesn't hold a start back", !Relaunch.HandOverWaiting());
        File.Delete(handOver);
        return Task.CompletedTask;
    }
}
