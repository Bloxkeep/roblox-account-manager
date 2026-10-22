using System.Diagnostics;
using Bloxkeep.Tests;

// Bloxkeep's checks. Each suite runs in a process of its own with a fresh data folder under %TEMP% (Bloxkeep keeps its
// data folder and settings in static state), so nothing touches the real data, and no Roblox process is touched.
//   Bloxkeep.Tests                  every suite that needs no internet
//   Bloxkeep.Tests --online         also the ones that ask Roblox's servers
//   Bloxkeep.Tests afk restart      only these
// The exit code is the number of failed checks (or 1 when a suite crashed).

var suites = new (string Name, bool Online, Func<Checks, string, Task> Run)[]
{
    ("afk", false, AfkSuite.Run),
    ("restart", false, RestartSuite.Run),
    ("rejoin", false, RejoinSuite.Run),
    ("follow", false, FollowSuite.Run),
    ("private-servers", false, PrivateServerSuite.Run),
    ("claims", false, ClaimsSuite.Run),
    ("presets", false, PresetSuite.Run),
    ("window", false, WindowSuite.Run),
    ("arrange", false, ArrangeSuite.Run),
    ("watch", false, WatchSuite.Run),
    ("feedback", false, FeedbackSuite.Run),
    ("online", false, OnlineSuite.Run),
    ("discord", false, DiscordSuite.Run),
    ("website", false, WebsiteSuite.Run),
    ("tray", false, TraySuite.Run),
    ("autolock", false, AutoLockSuite.Run),
    ("vault-save", false, VaultSaveSuite.Run),
    ("main-browser", false, MainBrowserSuite.Run),
    ("cores", false, CoresSuite.Run),
    ("robux", false, RobuxSuite.Run),
    ("saved-servers", false, SavedServersSuite.Run),
    ("discord-presence", false, DiscordPresenceSuite.Run),
    ("quiet-fixes", false, QuietFixSuite.Run),
    ("custom-theme", false, CustomThemeSuite.Run),
    ("vaults", false, VaultsSuite.Run),
    ("window-icons", false, WindowIconsSuite.Run),
    ("taskbar-groups", false, TaskbarGroupsSuite.Run),
    ("home-session", false, HomeSessionSuite.Run),
    ("sign-ins", false, SignInsSuite.Run),
    ("stats-icons", true, StatsIconSuite.Run),
    ("watch-online", true, WatchSuite.RunOnline),
};

if (args is ["--suite", var only, var dataDir])
{
    // A child run: one suite, in this process.
    var checks = new Checks();
    try
    {
        await suites.Single(s => s.Name == only).Run(checks, dataDir);
    }
    catch (Exception ex)
    {
        checks.Check("the suite ran to its end", false, ex);
    }
    Console.WriteLine(checks.Failures == 0 ? "ALL PASS" : $"{checks.Failures} FAILED");
    return checks.Failures;
}

bool online = args.Contains("--online");
var wanted = args.Where(a => !a.StartsWith("--")).ToHashSet();
int failed = 0;
var clock = Stopwatch.StartNew();
foreach (var suite in suites)
{
    if (wanted.Count > 0 ? !wanted.Contains(suite.Name) : suite.Online && !online)
        continue;
    string dir = Path.Combine(Path.GetTempPath(), "bloxkeep-tests", suite.Name);
    var info = new ProcessStartInfo(Environment.ProcessPath!, ["--suite", suite.Name, dir]) { RedirectStandardOutput = true, RedirectStandardError = true };
    using var child = Process.Start(info)!;
    string output = await child.StandardOutput.ReadToEndAsync() + await child.StandardError.ReadToEndAsync();
    await child.WaitForExitAsync();
    Console.WriteLine($"── {suite.Name} ──");
    foreach (var line in output.Split('\n', StringSplitOptions.RemoveEmptyEntries))
        Console.WriteLine("  " + line.TrimEnd('\r'));
    // A crash outside the checks (exit code not a count) is one failure.
    failed += child.ExitCode is >= 0 and < 1000 ? child.ExitCode : 1;
}
Console.WriteLine(failed == 0 ? $"All checks passed ({clock.Elapsed.TotalSeconds:0.0} s)." : $"{failed} check(s) failed.");
return failed;
