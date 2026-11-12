// Tests/Bloxkeep.Tests/RobloxUsageSuite.cs: Roblox's share of the GPU from Windows' counter instances
// (roblox_usage::gpu_percent), as Task Manager adds it up: only its processes, the shares of one engine together, then
// the busiest engine, never above 100.
use super::Checks;
use crate::core::roblox_usage;

const LUID: &str = "luid_0x00000000_0x00014D4D_phys_0";

pub async fn run(c: &mut Checks, _dir: &str) {
    let gpu = |pids: &[i32], rows: &[(String, f32)]| roblox_usage::gpu_percent(rows, pids);
    let row = |pid: i32, eng: &str, share: f32| (format!("pid_{pid}_{LUID}_{eng}"), share);

    c.check("nothing read: 0", gpu(&[12], &[]) == 0, gpu(&[12], &[]));
    c.check("another program's share isn't Roblox's", gpu(&[12], &[row(99, "eng_0_engtype_3D", 80.0)]) == 0, "");
    let two = gpu(&[12, 13], &[row(12, "eng_0_engtype_3D", 20.0), row(13, "eng_0_engtype_3D", 14.0)]);
    c.check("two windows on one engine add up", two == 34, two);
    let busiest = gpu(&[12], &[row(12, "eng_0_engtype_3D", 30.0), row(12, "eng_4_engtype_Copy", 25.0)]);
    c.check("different engines: the busiest, not their sum", busiest == 30, busiest);
    let capped = gpu(&[12, 13], &[row(12, "eng_0_engtype_3D", 70.0), row(13, "eng_0_engtype_3D", 60.0)]);
    c.check("never above 100", capped == 100, capped);
    let other_gpu = gpu(&[12], &[row(12, "eng_0_engtype_3D", 30.0), ("pid_12_luid_0x00000000_0x0001AAAA_phys_0_eng_0_engtype_3D".to_string(), 30.0)]);
    c.check("the same engine number on another GPU is another engine", other_gpu == 30, other_gpu);
    c.check("a name in another form is skipped", gpu(&[12], &[("something_else".to_string(), 50.0)]) == 0, "");
}
