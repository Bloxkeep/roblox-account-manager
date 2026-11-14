// Marketplace › Watching: things the user keeps an eye on, read again every few minutes while Bloxkeep runs. What
// changed (back on sale, cheaper, a cheaper resale, new stock) is said as Bloxkeep's other news is: a toast, a Windows
// notification while the window is behind, and the Discord and phone copies the user has set up (core/app/notify.rs).
// Kept on this PC, the same for every account: which of them can pay for it is worked out from their Robux when shown.
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// How many things can be watched (each is one request a round).
pub const MOST: usize = 40;
/// How often the list is read again.
pub const EVERY: std::time::Duration = std::time::Duration::from_secs(180);

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Watched {
    pub id: i64,
    pub name: String,
    pub pic: String,
    /// Its maker and Roblox's id for it as a thing sold: enough to open its page again from the list.
    #[serde(default)]
    pub maker: String,
    #[serde(default)]
    pub collectible: String,
    /// The maker's price (None: not for sale there), the cheapest resale (None: none), whether it is on sale, and how
    /// many are left of a limited one (None: not counted).
    pub price: Option<i64>,
    pub resale: Option<i64>,
    pub sale: bool,
    pub left: Option<i64>,
    /// When it was put on the list, and what changed since, newest first (when, what as a Change's words key + numbers).
    pub added: i64,
    #[serde(default)]
    pub log: Vec<Change>,
    /// A change not seen on the Watching list yet.
    #[serde(default)]
    pub fresh: bool,
}

/// What changed: "sale" (on sale again), "off" (off sale), "price" (the maker's price, from → to), "resale" (the
/// cheapest resale, from → to), "stock" (more left than before: to).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Change {
    pub at: i64,
    pub kind: String,
    pub from: i64,
    pub to: i64,
}

fn path() -> PathBuf {
    crate::roblox::kept("item-watch.json")
}

pub fn list() -> Vec<Watched> {
    std::fs::read_to_string(path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

/// Written beside it and moved over it: a crash mid-write never leaves half a list.
pub fn save(list: &[Watched]) -> Result<(), String> {
    let p = path();
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let tmp = p.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string(list).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &p).map_err(|e| e.to_string())
}

pub fn watching(id: i64) -> bool {
    list().iter().any(|w| w.id == id)
}

/// On the list or off it; Err("full") at MOST. Returns whether it is watched now.
pub fn toggle(id: i64, name: &str, pic: &str, maker: &str, collectible: &str) -> Result<bool, String> {
    let mut l = list();
    if let Some(i) = l.iter().position(|w| w.id == id) {
        l.remove(i);
        save(&l)?;
        return Ok(false);
    }
    if l.len() >= MOST {
        return Err("full".into());
    }
    let mut w = Watched { id, name: name.into(), pic: pic.into(), maker: maker.into(), collectible: collectible.into(), added: crate::time::now_secs(), ..Default::default() };
    // Its numbers now, so the first change said is a real one (on a slow answer: taken at the next round).
    if let Ok(n) = read(id) {
        (w.price, w.resale, w.sale, w.left) = (n.price, n.resale, n.sale, n.left);
    }
    l.insert(0, w);
    save(&l)?;
    Ok(true)
}

/// Seen on the Watching list: no longer fresh.
pub fn seen() {
    let mut l = list();
    if l.iter().any(|w| w.fresh) {
        l.iter_mut().for_each(|w| w.fresh = false);
        let _ = save(&l);
    }
}

struct Now {
    price: Option<i64>,
    resale: Option<i64>,
    sale: bool,
    left: Option<i64>,
}

/// Roblox's numbers for a thing now (economy's asset details: signed-out, one request).
fn read(id: i64) -> Result<Now, String> {
    let v: Value = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .get(&format!("https://economy.roblox.com/v2/assets/{id}/details"))
        .call()
        .map_err(|e| match e {
            ureq::Error::Status(c, _) => format!("HTTP {c}"),
            e => e.to_string(),
        })?
        .into_string()
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .ok_or("bad answer")?;
    let sale = v.get("IsForSale").and_then(Value::as_bool).unwrap_or(false);
    let limited = v.get("IsLimited").and_then(Value::as_bool).unwrap_or(false) || v.get("IsLimitedUnique").and_then(Value::as_bool).unwrap_or(false);
    Ok(Now {
        price: v.get("PriceInRobux").and_then(Value::as_i64).filter(|_| sale),
        resale: v.pointer("/CollectiblesItemDetails/CollectibleLowestResalePrice").and_then(Value::as_i64),
        sale,
        left: v.get("Remaining").and_then(Value::as_i64).filter(|_| limited),
    })
}

/// One round: every thing read again; what changed is kept on it and returned (the thing, its new changes).
pub fn check() -> Vec<(Watched, Vec<Change>)> {
    let l = list();
    if l.is_empty() {
        return Vec::new();
    }
    let at = crate::time::now_secs();
    let mut out = Vec::new();
    let mut read_now: Vec<(i64, Now)> = Vec::new();
    for w in &l {
        if let Ok(n) = read(w.id) {
            read_now.push((w.id, n));
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    // (Read again from disk: one taken off or put on while the round ran stays so.)
    let mut l = list();
    for w in l.iter_mut() {
        let Some((_, n)) = read_now.iter().find(|(id, _)| *id == w.id) else { continue };
        let new = changes(w, n, at);
        (w.price, w.resale, w.sale, w.left) = (n.price, n.resale, n.sale, n.left);
        if !new.is_empty() {
            w.fresh = true;
            for c in new.iter().rev() {
                w.log.insert(0, c.clone());
            }
            w.log.truncate(20);
            out.push((w.clone(), new));
        }
    }
    if !read_now.is_empty() {
        let _ = save(&l);
    }
    out
}

/// What differs between a thing as kept and as read now, in the order it is said: on or off sale, or a lower price;
/// a lower (or a first) resale; more left.
fn changes(w: &Watched, n: &Now, at: i64) -> Vec<Change> {
    let mut new = Vec::new();
    let ch = |kind: &str, from: i64, to: i64| Change { at, kind: kind.into(), from, to };
    if n.sale && !w.sale {
        new.push(ch("sale", 0, n.price.unwrap_or(0)));
    } else if !n.sale && w.sale {
        new.push(ch("off", 0, 0));
    } else if let (Some(a), Some(b)) = (w.price, n.price)
        && b < a
    {
        new.push(ch("price", a, b));
    }
    match (w.resale, n.resale) {
        (Some(a), Some(b)) if b < a => new.push(ch("resale", a, b)),
        (None, Some(b)) => new.push(ch("resale", 0, b)),
        _ => {}
    }
    if let (Some(a), Some(b)) = (w.left, n.left)
        && b > a
    {
        new.push(ch("stock", a, b));
    }
    new
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(w: &Watched, n: &Now) -> Vec<String> {
        changes(w, n, 1).into_iter().map(|c| format!("{}:{}>{}", c.kind, c.from, c.to)).collect()
    }

    #[test]
    fn what_changed() {
        let was = Watched { price: Some(100), resale: Some(500), sale: true, left: Some(3), ..Default::default() };
        // Nothing moved: nothing said.
        assert!(kinds(&was, &Now { price: Some(100), resale: Some(500), sale: true, left: Some(3) }).is_empty());
        // Cheaper, a cheaper resale, restocked; dearer or fewer left is not news.
        assert_eq!(kinds(&was, &Now { price: Some(80), resale: Some(450), sale: true, left: Some(10) }), ["price:100>80", "resale:500>450", "stock:3>10"]);
        assert!(kinds(&was, &Now { price: Some(120), resale: Some(600), sale: true, left: Some(2) }).is_empty());
        // Off sale, then on sale again.
        assert_eq!(kinds(&was, &Now { price: None, resale: Some(500), sale: false, left: Some(3) }), ["off:0>0"]);
        let off = Watched { price: None, sale: false, ..Default::default() };
        assert_eq!(kinds(&off, &Now { price: Some(0), resale: None, sale: true, left: None }), ["sale:0>0"]);
        // A first resale.
        assert_eq!(kinds(&off, &Now { price: None, resale: Some(75), sale: false, left: None }), ["resale:0>75"]);
    }
}
