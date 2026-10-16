using System.Diagnostics;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// Roblox's notification-area copy ("RobloxPlayerBeta.exe --launch-to-tray") is told by its command line. Checked on
/// stand-ins: a process started with that word in its command line counts as one, the same program without it doesn't,
/// and an answer is kept per process (a pid Windows gives to a later process is asked again).
/// </summary>
static class TraySuite
{
    public static Task Run(Checks c, string dir)
    {
        // ping waits as long as asked and ignores nothing it doesn't know after the target: a quiet stand-in.
        static Process Start(string args) => Process.Start(new ProcessStartInfo("ping.exe", args) { CreateNoWindow = true, UseShellExecute = false })!;
        using var tray = Start("-n 20 127.0.0.1 --launch-to-tray");
        using var plain = Start("-n 20 127.0.0.1");
        try
        {
            long created = tray.StartTime.ToFileTimeUtc();
            c.Check("a process started with --launch-to-tray is Roblox's notification-area copy", RobloxProcesses.IsTrayCopy(tray.Id, created));
            c.Check("the same program without it isn't", !RobloxProcesses.IsTrayCopy(plain.Id, plain.StartTime.ToFileTimeUtc()));
            c.Check("asked again, the same answer", RobloxProcesses.IsTrayCopy(tray.Id, created));
            c.Check("a process that isn't there isn't one", !RobloxProcesses.IsTrayCopy(int.MaxValue - 7, 1));
        }
        finally
        {
            foreach (var p in new[] { tray, plain })
            {
                try { p.Kill(); } catch (InvalidOperationException) { }
            }
        }
        return Task.CompletedTask;
    }
}
