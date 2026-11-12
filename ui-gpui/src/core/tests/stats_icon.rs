// Tests/Bloxkeep.Tests/StatsIconSuite.cs: Statistics with nothing in the game caches (a fresh start, games played days
// ago): "playStats" comes back with every game's and preset's icon. The C# asks Roblox's public game and thumbnail
// servers (an --online suite), so each of its checks is skipped here: the checks never reach the internet.
use super::Checks;

pub async fn run(c: &mut Checks, _dir: &str) {
    for game in ["Dead Rails", "Blox Fruits", "Grow a Garden", "Adopt Me!"] {
        c.skip(&format!("icon for {game}"), "needs the network");
    }
    c.skip("icon for preset Farm", "needs the network");
}
