using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// Roblox's share of the GPU from Windows' counter instances (RobloxUsage.GpuPercent), as Task Manager adds it up: only
/// its processes, the shares of one engine together, then the busiest engine, never above 100.
/// </summary>
static class RobloxUsageSuite
{
    const string Luid = "luid_0x00000000_0x00014D4D_phys_0";

    public static Task Run(Checks c, string dir)
    {
        int Gpu(int[] pids, params (string, float)[] rows) => RobloxUsage.GpuPercent(rows, pids);

        c.Check("nothing read: 0", Gpu([12]) == 0);
        c.Check("another program's share isn't Roblox's", Gpu([12], ($"pid_99_{Luid}_eng_0_engtype_3D", 80)) == 0);
        c.Check("two windows on one engine add up", Gpu([12, 13], ($"pid_12_{Luid}_eng_0_engtype_3D", 20), ($"pid_13_{Luid}_eng_0_engtype_3D", 14)) == 34);
        c.Check("different engines: the busiest, not their sum", Gpu([12], ($"pid_12_{Luid}_eng_0_engtype_3D", 30), ($"pid_12_{Luid}_eng_4_engtype_Copy", 25)) == 30);
        c.Check("never above 100", Gpu([12, 13], ($"pid_12_{Luid}_eng_0_engtype_3D", 70), ($"pid_13_{Luid}_eng_0_engtype_3D", 60)) == 100);
        c.Check("the same engine number on another GPU is another engine",
            Gpu([12], ($"pid_12_{Luid}_eng_0_engtype_3D", 30), ("pid_12_luid_0x00000000_0x0001AAAA_phys_0_eng_0_engtype_3D", 30)) == 30);
        c.Check("a name in another form is skipped", Gpu([12], ("something_else", 50)) == 0);
        return Task.CompletedTask;
    }
}
