// Tests/Bloxkeep.Tests/PlayHeatSuite.cs: Statistics' hour map: a stay's hours go to the local day and hour they were
// played in; an account's run of whole days (23 hours or more) up to yesterday.
use super::Checks;
use crate::core::app::stats::{add_heat, full_day_streaks};
use crate::core::base::{Guid, Stamp, new_guid};
use crate::core::models::PlaySession;

pub async fn run(c: &mut Checks, _dir: &str) {
    let first_day = Stamp::from_local(2026, 11, 1, 0, 0, 0);
    let local = |day: f64, hour: f64| first_day.plus_secs((day * 24.0 + hour) * 3600.0).to_utc();
    let heat = || -> Vec<Vec<f64>> { (0..3).map(|_| vec![0.0; 24]).collect() };

    let mut whole = heat();
    add_heat(&mut whole, first_day, local(0.0, 14.0), local(0.0, 15.0));
    c.check("an hour played fills its cell", whole[0][14] == 1.0 && whole[0].iter().sum::<f64>() == 1.0, &whole[0]);

    let mut half = heat();
    add_heat(&mut half, first_day, local(1.0, 9.5), local(1.0, 11.0));
    c.check("a stay split by the hours it covers", half[1][9] == 0.5 && half[1][10] == 1.0, &half[1]);

    let mut night = heat();
    add_heat(&mut night, first_day, local(0.0, 23.5), local(1.0, 1.0));
    c.check("past midnight goes on the next day", night[0][23] == 0.5 && night[1][0] == 1.0, (&night[0], &night[1]));

    let mut outside = heat();
    add_heat(&mut outside, first_day, local(-1.0, 10.0), local(-1.0, 12.0));
    add_heat(&mut outside, first_day, local(3.0, 10.0), local(3.0, 12.0));
    c.check("outside the period: nothing", outside.iter().all(|day| day.iter().sum::<f64>() == 0.0), &outside);

    // Full days in a row, counted back from yesterday (today isn't over): now is 4 November, 15:00.
    let now = local(3.0, 15.0);
    let (steady, broken, short) = (new_guid(), new_guid(), new_guid());
    let stay = |id: Guid, from: Stamp, to: Option<Stamp>| PlaySession { account_id: id, started_at: from, ended_at: to, ..Default::default() };
    let streaks = full_day_streaks(
        &[
            stay(steady, local(0.0, 0.0), None), // since 1 November, still on
            stay(broken, local(1.0, 0.0), Some(local(2.0, 0.0))),
            stay(broken, local(2.0, 8.0), Some(local(2.0, 18.0))), // yesterday 10 hours only
            stay(short, local(0.0, 1.0), Some(local(1.0, 0.0))),
            stay(short, local(1.0, 1.0), Some(local(2.0, 0.0))),
            stay(short, local(2.0, 1.0), Some(local(3.0, 0.0))), // 23 hours a day
        ],
        now,
    );
    c.check("three whole days up to yesterday: 3", streaks.get(&steady) == Some(&3), streaks.get(&steady));
    c.check("yesterday not whole: no streak", streaks.get(&broken) == Some(&0), streaks.get(&broken));
    c.check("23 hours a day counts as whole", streaks.get(&short) == Some(&3), streaks.get(&short));
}
