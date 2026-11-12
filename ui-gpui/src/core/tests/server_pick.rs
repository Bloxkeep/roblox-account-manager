// Tests/Bloxkeep.Tests/ServerPickSuite.cs: each account in a server of its own, and leaving crowded servers
// (app/server_hop.rs).
use std::{
    cell::Cell,
    collections::HashSet,
};

use super::Checks;
use crate::core::{
    app::server_hop::{HOP_ABOVE_CHOICES, crowded_servers, quiet_server, server_apart},
    roblox_api::ServerInfo,
};

fn s(job: &str, playing: i32, max: i32) -> ServerInfo {
    ServerInfo { job_id: job.into(), playing, max_players: max, ping: 0, fps: 60.0 }
}

fn set(items: &[&str]) -> HashSet<String> {
    items.iter().map(|s| s.to_string()).collect()
}

pub async fn run(c: &mut Checks, _dir: &str) {
    // A seeded stand-in for C#'s Random(7): any index below n.
    let seed = Cell::new(7u64);
    let random = |n: usize| {
        seed.set(seed.get().wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407));
        ((seed.get() >> 33) as usize) % n
    };

    // Own server each: the list's first with room that none of the accounts is in.
    let list = vec![s("a", 2, 12), s("b", 12, 12), s("c", 5, 12), s("d", 11, 12)];
    let mut taken = set(&["a"]);
    c.check("apart: the first free one with room", server_apart(&list, &taken, "fewest", random).map(|s| s.job_id.as_str()) == Some("c"), ());
    taken.insert("c".into());
    c.check("apart: a full one is skipped", server_apart(&list, &taken, "most", random).map(|s| s.job_id.as_str()) == Some("d"), ());
    taken.insert("d".into());
    c.check("apart: none left", server_apart(&list, &taken, "fewest", random).is_none(), ());
    let picked: HashSet<String> = (0..30).filter_map(|_| server_apart(&list, &HashSet::new(), "random", random).map(|s| s.job_id.clone())).collect();
    c.check("apart: random picks among those with room", picked == set(&["a", "c", "d"]), format!("{picked:?}"));

    // Crowded: more than 3 other players, read emptiest first.
    let seen = vec![s("q", 1, 12), s("r", 3, 12), s("ours1", 4, 12), s("t", 6, 12)];
    let ours = vec![("ours1".to_string(), 1)];
    c.check("in the list: its players less the accounts'", crowded_servers(&seen, false, &ours, 3).is_empty(), ());
    c.check("…one more and it's crowded", crowded_servers(&seen, false, &ours, 2).contains("ours1"), ());
    let two = vec![("ours1".to_string(), 2)];
    c.check("two of the accounts there don't count", crowded_servers(&seen, false, &two, 2).is_empty(), ());
    let missing = vec![("gone".to_string(), 1)];
    c.check("not in the list, which went past: crowded", crowded_servers(&seen, false, &missing, 3).contains("gone"), ());
    let short = vec![s("q", 1, 12), s("r", 2, 12)];
    c.check("not in the list, which stopped early: unknown, left", crowded_servers(&short, false, &missing, 3).is_empty(), ());
    c.check("not in a list that ended: full", crowded_servers(&short, true, &missing, 3).contains("gone"), ());
    c.check("…full but small enough: left", crowded_servers(&[s("q", 1, 4)], true, &missing, 3).is_empty(), ());

    // A quiet one: at most that many there, room, not where the accounts are.
    let avoid = set(&["q"]);
    c.check("quiet: at most N there, not avoided", quiet_server(&seen, 3, &avoid, random).map(|s| s.job_id.as_str()) == Some("r"), ());
    c.check("quiet: none", quiet_server(&seen, 0, &avoid, random).is_none(), ());
    c.check("the setting's choices start at off", HOP_ABOVE_CHOICES[0] == 0 && HOP_ABOVE_CHOICES.contains(&5), ());
}
