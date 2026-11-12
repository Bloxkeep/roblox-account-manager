// Tests/Bloxkeep.Tests/MonitorSuite.cs: the game monitor's picture of a window (WindowThumbnails), on two windows of the
// test's own shown on the screen and read back from it: none of it runs here.
use super::Checks;

pub async fn run(c: &mut Checks, _dir: &str) {
    c.skip("the picture shows the whole window, its orange edge too", "needs a window");
    c.skip("grown, the picture follows within a moment and still shows the orange edge", "needs a window");
}
