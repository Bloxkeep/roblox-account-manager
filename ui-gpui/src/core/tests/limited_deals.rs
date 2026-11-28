// Limited deals, asked of the real Rolimons and Roblox (no sign-in): Limiteds come back, each with a price Roblox sells
// at now, a RAP and a Value, at least 10 % below one of them, none projected, the cheapest against the RAP first; a
// second read only follows Rolimons' feed of the last minutes, far quicker. Needs the internet, so it runs only when
// named.
use std::time::Instant;

use super::Checks;
use crate::roblox::{self, LimitedDeal};

pub async fn run(c: &mut Checks, _dir: &str) {
    let first = Instant::now();
    let got = crate::core::rt::blocking(|| roblox::limited_deals(10)).await;
    let took = first.elapsed().as_secs_f32();
    let list: Vec<LimitedDeal> = match got {
        Ok(l) => l,
        Err(e) => {
            c.check("Limited deals are read", false, e);
            return;
        }
    };
    c.check("Limited deals are read, some found", !list.is_empty(), (list.len(), took));
    c.check("…each with a price, a RAP and a Value", list.iter().all(|d| d.ware.price.is_some_and(|p| p > 0) && d.rap > 0 && d.value > 0), ());
    c.check("…each at least 10 % below its RAP or Value", list.iter().all(|d| d.off(false) >= 10 || d.off(true) >= 10), ());
    c.check("…none projected", list.iter().all(|d| !d.projected), ());
    c.check("…the cheapest against the RAP first", list.windows(2).all(|w| w[0].off(false) >= w[1].off(false)), ());
    let pics = list.iter().filter(|d| !d.ware.pic.is_empty()).count();
    c.check("…with their pictures", pics * 10 >= list.len() * 9, (pics, list.len()));
    let top: Vec<String> = list.iter().take(5).map(|d| format!("{} {} R$ (RAP {}, Value {}, -{}%)", d.ware.name, d.ware.price.unwrap_or(0), d.rap, d.value, d.off(false))).collect();
    c.check("the best five, as read", true, top);
    let again = Instant::now();
    let _ = crate::core::rt::blocking(|| roblox::limited_deals(10)).await;
    c.check("a second read follows Rolimons' feed only, in a few seconds", again.elapsed().as_secs_f32() < 5.0, (again.elapsed().as_secs_f32(), took));
}
