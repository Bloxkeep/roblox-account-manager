// A game's whole wiki inside its page (the Wiki tab of games.rs), read from this PC through the app's wikiApi (any wiki
// on Fandom or wiki.gg, named by Bloxkeep's bot): the old web UI's Wiki part (ui/src/WikiSite.tsx, WikiView.tsx) in
// GPUI. Its home (what is new on the wiki and which numbers changed lately, the kinds of thing it keeps boxes of facts
// for, pages seen and saved), its search as typed, every article drawn here with its box of facts as a card and its
// links opening here, and each kind as a table to search, filter, sort and compare, with the items most like one and
// the odds of a drop. Text and pictures are the wiki's (CC BY-SA): every page names and links it.
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::{Arc, LazyLock, Mutex},
    time::Duration,
};

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::text::TextView;
use gpui_kit::{prelude::*, *};
use serde_json::{Value, json};

use crate::{
    images,
    kit::*,
    link::Link,
    shell::Root,
    theme::*,
    wikihtml::{self, Infobox},
    wikitext::{self, BoxChange, Item, fold, less_is_better, number_of, rank_of, value_of},
};

/// A kind needs this many pages to be a table; at most this many kinds; a table reads this many pages at most.
const KIND_MIN: usize = 8;
const KINDS_MAX: usize = 14;
const ITEMS_MAX: usize = 600;
const FRESH_DAYS: i64 = 14;
const CHANGED_PAGES: usize = 30;
const COMPARE_MAX: usize = 4;
const COLUMNS: usize = 5;
const ROWS_SHOWN: usize = 200;
const SIDE: f32 = 300.;

#[derive(Clone, PartialEq)]
pub enum View {
    Home,
    Page { title: String, table: Option<String> },
    Table(String),
    Search(String),
    Saved,
}

#[derive(Clone)]
pub struct About {
    name: String,
    articles: i64,
    main_page: String,
    logo: Option<String>,
    rights: Option<(String, String)>,
}

#[derive(Clone)]
struct Kind {
    template: String,
    name: String,
    count: i64,
    pic: Option<String>,
}

#[derive(Clone)]
struct FreshPage {
    title: String,
    at: i64,
    edits: i64,
    pic: Option<String>,
}

#[derive(Clone, Default)]
struct Fresh {
    added: Vec<FreshPage>,
    hot: Vec<FreshPage>,
    edited: Vec<FreshPage>,
}

#[derive(Clone)]
struct Changed {
    title: String,
    pic: Option<String>,
    lines: Vec<BoxChange>,
}

#[derive(Clone)]
struct Article {
    title: String,
    html: SharedString,
    infobox: Option<Infobox>,
    images: Vec<String>,
    cats: Vec<String>,
    missing: bool,
    /// Its parts, for its Contents (shown with 3 or more, as WikiSite.tsx's loadPage).
    sections: Vec<wikihtml::Section>,
}

#[derive(Clone, Default)]
struct Table {
    /// Shared, not copied, by each frame that draws it.
    items: Rc<Vec<Item>>,
    done: bool,
    failed: bool,
    /// Its number as last read (each read 50 more): what the views' caches go by.
    rev: u64,
}

/// The next table read's number.
static TABLE_REV: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// A table as filtered and sorted (by the fingerprint of what it was worked out from): its lines and the items shown.
struct TableCut {
    key: u64,
    lines: Rc<Vec<Line>>,
    shown: Rc<Vec<usize>>,
}

/// What an article found in the tables (by the fingerprint of the page and the tables): the item it is, its facts, and
/// the items most like it.
struct ArtFound {
    key: u64,
    item: Option<usize>,
    facts: Rc<Vec<(String, String)>>,
    sim: Rc<(Vec<wikitext::SimLine>, Vec<usize>)>,
}

fn hash_of(x: impl std::hash::Hash) -> u64 {
    use std::hash::Hasher;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    x.hash(&mut h);
    h.finish()
}

/// What the tab has read (kept while the app runs, by wiki) and what it shows.
#[derive(Default)]
pub struct Data {
    site: String,
    game: String,
    views: Vec<View>,
    abouts: HashMap<String, Option<About>>,
    asked_about: HashSet<String>,
    kinds: HashMap<String, Vec<Kind>>,
    asked_kinds: HashSet<String>,
    fresh: HashMap<String, Fresh>,
    changes: HashMap<String, Vec<Changed>>,
    asked_fresh: HashSet<String>,
    /// Pages by "site|title" (None: it couldn't be read), and when each was last shown (the one shown longest ago goes
    /// first when there are too many).
    pages: HashMap<String, Option<Rc<Article>>>,
    page_used: HashMap<String, u64>,
    page_tick: u64,
    asked_pages: HashSet<String>,
    tables: HashMap<String, Table>,
    asked_tables: HashSet<String>,
    results: HashMap<String, Option<Vec<(String, String)>>>,
    asked_results: HashSet<String>,
    /// The search box's suggestions and what they are for; a counter so only the last typing asks.
    hits: Vec<String>,
    hits_for: String,
    typed: u64,
    /// The pages marked as seen this run.
    marked: HashSet<String>,
    /// Each wiki's short codes in its own words ("site|{{Rarity|L}}" → "Legendary"; None: no words).
    said: HashMap<String, Option<String>>,
    /// The table's sort, the values it is filtered to, the items picked to compare, and whether the filters and the
    /// comparison are open.
    sort: Option<(String, bool)>,
    filters: Vec<(String, Vec<String>)>,
    filtering: bool,
    picked: Vec<String>,
    comparing: bool,
    /// The number lines' Min and Max boxes (WikiView.tsx's wk-range), by line, and the table they are for: made the
    /// first time its filters show (a box needs the window, so `settle` makes them).
    ranges: Vec<(String, Entity<InputState>, Entity<InputState>)>,
    ranges_for: String,
    /// The drop odds' "tries so far" box (WikiView.tsx's DropOdds) and the page it is for.
    tried: Option<(String, Entity<InputState>)>,
    /// The table view's and the article's work, kept between frames.
    cut: RefCell<Option<TableCut>>,
    found: RefCell<Option<ArtFound>>,
}

/// A link clicked in an article (the text view hands it back from outside the window's reach).
static CLICKED: Mutex<Option<String>> = Mutex::new(None);
/// Where each part of the shown article starts on the window (by its place in the Contents), as last drawn.
static PART_TOPS: LazyLock<Mutex<HashMap<usize, Pixels>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
/// The article's pictures loaded so far, for the text view to draw.
static LOADED: LazyLock<Mutex<HashMap<String, Arc<Image>>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
/// A clear pixel, drawn while a picture loads.
static BLANK: LazyLock<Arc<Image>> = LazyLock::new(|| {
    const PNG: [u8; 68] = [
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
        0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0b, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x60, 0x00, 0x02, 0x00,
        0x00, 0x05, 0x00, 0x01, 0x7a, 0x5e, 0xab, 0x3f, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ];
    Arc::new(Image::from_bytes(ImageFormat::Png, PNG.to_vec()))
});

fn key(site: &str, part: &str) -> String {
    format!("{site}|{part}")
}

fn view_now(d: &Data) -> View {
    d.views.last().cloned().unwrap_or(View::Home)
}

pub(crate) async fn api(link: &Link, site: &str, q: Value) -> Option<Value> {
    link.call("wikiApi", json!({ "site": site, "q": q })).await.ok().and_then(Result::ok).filter(|v| v.get("error").is_none())
}

fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or_default().to_string()
}

fn stamp(iso: &str) -> i64 {
    crate::time::iso_secs(iso).unwrap_or(0)
}

fn thumb(p: &Value) -> Option<String> {
    p.pointer("/thumbnail/source").and_then(Value::as_str).filter(|u| wikitext::wiki_img(u)).map(str::to_string)
}

fn since_iso() -> String {
    crate::time::iso(crate::time::now_secs() - FRESH_DAYS * 86_400)
}

// ---- Asking the wiki ----

/// What the shown view needs, asked once each (games.rs calls this while the Wiki tab is shown).
pub fn want(r: &mut Root, site: &str, game: &str, cx: &mut Context<Root>) {
    let Some(link) = r.link.clone() else { return };
    if r.wiki.site != site {
        r.wiki.site = site.to_string();
        r.wiki.views = vec![View::Home];
        r.wiki.sort = None;
        r.wiki.filters.clear();
        r.wiki.ranges.clear();
        r.wiki.ranges_for.clear();
        r.wiki.picked.clear();
        r.wiki.comparing = false;
        r.wiki.hits.clear();
    }
    r.wiki.game = game.to_string();
    // BLOXKEEP_SHOT_WIKI="table:Template:Fruit", "page:Dragon", "search:sword" or "saved": a hidden shot's Wiki tab on
    // that view (over its home).
    #[cfg(feature = "shot")]
    {
        static ONCE: std::sync::Once = std::sync::Once::new();
        if let Ok(v) = std::env::var("BLOXKEEP_SHOT_WIKI") {
            ONCE.call_once(|| {
                let (kind, rest) = v.split_once(':').unwrap_or((v.as_str(), ""));
                let shown = match kind {
                    "table" => Some(View::Table(rest.to_string())),
                    "page" => Some(View::Page { title: rest.to_string(), table: None }),
                    "search" => Some(View::Search(rest.to_string())),
                    "saved" => Some(View::Saved),
                    _ => None,
                };
                r.wiki.views.extend(shown);
                // BLOXKEEP_SHOT_PICKED=2: that many of the table's first items picked to compare, the comparison open.
                if let Ok(n) = std::env::var("BLOXKEEP_SHOT_PICKED") {
                    r.wiki.comparing = n.parse::<usize>().unwrap_or(0) >= 2;
                    r.wiki.picked = vec![format!("#{n}")];
                }
            });
        }
        if let Some(n) = r.wiki.picked.first().and_then(|p| p.strip_prefix('#')).and_then(|n| n.parse::<usize>().ok())
            && let View::Table(t) = view_now(&r.wiki)
            && let Some(tb) = r.wiki.tables.get(&key(site, &t)).filter(|tb| tb.done)
        {
            r.wiki.picked = tb.items.iter().take(n).map(|i| i.id.clone()).collect();
        }
    }
    let site = site.to_string();
    if r.wiki.asked_about.insert(site.clone()) {
        let (link, site) = (link.clone(), site.clone());
        cx.spawn(async move |this, cx| {
            let d = api(&link, &site, json!({ "action": "query", "meta": "siteinfo", "siprop": "general|statistics|rightsinfo" })).await;
            let about = d.as_ref().and_then(|d| {
                let g = d.pointer("/query/general")?;
                let logo = g.get("logo").and_then(Value::as_str).map(|l| if l.starts_with("//") { format!("https:{l}") } else { l.to_string() }).filter(|l| wikitext::wiki_img(l));
                let r = d.pointer("/query/rightsinfo");
                Some(About {
                    name: s(g, "sitename"),
                    articles: d.pointer("/query/statistics/articles").and_then(Value::as_i64).unwrap_or(0),
                    main_page: Some(s(g, "mainpage")).filter(|m| !m.is_empty()).unwrap_or_else(|| "Main Page".into()),
                    logo,
                    rights: r.and_then(|r| {
                        let (t, u) = (s(r, "text"), s(r, "url"));
                        (!t.is_empty() && u.starts_with("https://")).then_some((t, u))
                    }),
                })
            });
            let _ = this.update(cx, |r, cx| {
                r.wiki.abouts.insert(site, about);
                cx.notify();
            });
        })
        .detach();
        return;
    }
    if !matches!(r.wiki.abouts.get(&site), Some(Some(_))) {
        return;
    }
    if r.wiki.asked_kinds.insert(site.clone()) {
        load_kinds(link.clone(), site.clone(), cx);
    }
    if r.wiki.asked_fresh.insert(site.clone()) {
        load_fresh(link.clone(), site.clone(), cx);
    }
    match view_now(&r.wiki) {
        View::Page { title, .. } => {
            let k = key(&site, &title);
            r.wiki.page_tick += 1;
            let tick = r.wiki.page_tick;
            r.wiki.page_used.insert(k.clone(), tick);
            if r.wiki.asked_pages.insert(k.clone()) {
                load_page(link, site, title, cx);
            } else if let Some(Some(a)) = r.wiki.pages.get(&k)
                && !a.missing
                && r.wiki.marked.insert(k)
            {
                let (title, game) = (a.title.clone(), r.wiki.game.clone());
                r.send("wikiMark", json!({ "list": "recent", "site": site, "title": title, "game": game }));
            }
        }
        View::Table(template) => {
            if r.wiki.asked_tables.insert(key(&site, &template)) {
                load_table(link, site, template, r.wiki.said.clone(), cx);
            }
        }
        View::Search(q) => {
            if r.wiki.asked_results.insert(key(&site, &q)) {
                let k = key(&site, &q);
                cx.spawn(async move |this, cx| {
                    let d = api(&link, &site, json!({ "action": "query", "list": "search", "srsearch": q, "srlimit": 40, "srnamespace": 0, "srprop": "snippet" })).await;
                    let hits = d.and_then(|d| {
                        Some(
                            d.pointer("/query/search")?
                                .as_array()?
                                .iter()
                                .map(|h| (s(h, "title"), wikihtml::words_of(&s(h, "snippet"))))
                                .filter(|(t, _)| !t.is_empty())
                                .collect::<Vec<_>>(),
                        )
                    });
                    let failed = hits.is_none();
                    let _ = this.update(cx, |r, cx| {
                        r.wiki.results.insert(k.clone(), hits);
                        cx.notify();
                    });
                    // Failed (offline a moment): asked again while it is shown.
                    if failed {
                        cx.background_executor().timer(crate::pages::games::RETRY).await;
                        let _ = this.update(cx, |r, cx| {
                            r.wiki.asked_results.remove(&k);
                            cx.notify();
                        });
                    }
                })
                .detach();
            }
        }
        _ => {}
    }
}

/// The kinds of thing the wiki keeps a box of facts for: its infobox templates, by how many articles use each, each
/// with the picture of its first page that has one.
fn load_kinds(link: Link, site: String, cx: &mut Context<Root>) {
    cx.spawn(async move |this, cx| {
        let (boxes, used) = futures::join!(
            api(&link, &site, json!({ "action": "query", "list": "allinfoboxes" })),
            api(&link, &site, json!({ "action": "query", "list": "querypage", "qppage": "Mostlinkedtemplates", "qplimit": 500 })),
        );
        let mut counts: HashMap<String, i64> = HashMap::new();
        for r in used.as_ref().and_then(|u| u.pointer("/query/querypage/results")).and_then(Value::as_array).into_iter().flatten() {
            let t = s(r, "title");
            if let Some(name) = t.strip_prefix("Template:") {
                counts.insert(fold(&name.replace('_', " ")), r.get("value").and_then(|v| v.as_i64().or_else(|| v.as_str().and_then(|x| x.parse().ok()))).unwrap_or(0));
            }
        }
        let mut all: Vec<Kind> = Vec::new();
        for b in boxes.as_ref().and_then(|b| b.pointer("/query/allinfoboxes")).and_then(Value::as_array).into_iter().flatten() {
            let title = s(b, "title").replace('_', " ");
            let low = title.to_lowercase();
            if title.is_empty() || ["user", "test", "sandbox", "tabber", "navbox"].iter().any(|w| low.contains(w)) || low.ends_with("doc") || low.starts_with('.') || low.contains('/') {
                continue;
            }
            let n = counts.get(&fold(&title)).copied().unwrap_or(0);
            if n >= KIND_MIN as i64 {
                all.push(Kind { template: format!("Template:{title}"), name: wikitext::kind_name(&title), count: n, pic: None });
            }
        }
        all.sort_by(|a, b| b.count.cmp(&a.count));
        all.truncate(KINDS_MAX + 8);
        // Those counts are of every page using a box, players' own pages too: a kind is kept only when enough articles
        // use it; the same answer gives the kind its picture.
        let asks = all.iter().map(|k| {
            api(
                &link,
                &site,
                json!({ "action": "query", "generator": "embeddedin", "geititle": k.template, "geinamespace": 0, "geilimit": 50, "prop": "pageimages", "piprop": "thumbnail", "pithumbsize": 160, "pilimit": 50 }),
            )
        });
        let answers = futures::future::join_all(asks).await;
        let mut kinds = Vec::new();
        let mut names = HashSet::new();
        for (mut k, d) in all.into_iter().zip(answers) {
            let pages: Vec<Value> = d.as_ref().and_then(|d| d.pointer("/query/pages")).and_then(Value::as_array).cloned().unwrap_or_default();
            if pages.len() < KIND_MIN || !names.insert(fold(&k.name)) {
                continue;
            }
            k.pic = pages.iter().find_map(thumb);
            if pages.len() < 50 {
                k.count = pages.len() as i64;
            }
            kinds.push(k);
        }
        kinds.sort_by(|a, b| b.count.cmp(&a.count));
        kinds.truncate(KINDS_MAX);
        // (Neither asking answered: the wiki or the network failed a moment; asked again after a while.)
        let failed = boxes.is_none() && used.is_none();
        let site2 = site.clone();
        let _ = this.update(cx, |r, cx| {
            r.wiki.kinds.insert(site2, kinds);
            cx.notify();
        });
        if failed {
            cx.background_executor().timer(crate::pages::games::RETRY).await;
            let _ = this.update(cx, |r, cx| {
                r.wiki.asked_kinds.remove(&site);
                cx.notify();
            });
        }
    })
    .detach();
}

/// The wiki's last two weeks: the pages added and the pages edited most (bots and redirects left out); then the numbers
/// the boxes of the pages edited most changed in that time ("buffs and nerfs").
fn load_fresh(link: Link, site: String, cx: &mut Context<Root>) {
    cx.spawn(async move |this, cx| {
        let since = since_iso();
        let (made, edits) = futures::join!(
            api(&link, &site, json!({ "action": "query", "list": "recentchanges", "rcnamespace": 0, "rctype": "new", "rcshow": "!bot|!redirect", "rclimit": 40, "rcprop": "title|timestamp", "rcend": since })),
            api(&link, &site, json!({ "action": "query", "list": "recentchanges", "rcnamespace": 0, "rctype": "edit", "rcshow": "!bot", "rclimit": 500, "rcprop": "title|timestamp", "rcend": since })),
        );
        let rows = |d: &Option<Value>| d.as_ref().and_then(|d| d.pointer("/query/recentchanges")).and_then(Value::as_array).cloned().unwrap_or_default();
        let mut added: Vec<FreshPage> = Vec::new();
        let mut seen = HashSet::new();
        for c in rows(&made) {
            let t = s(&c, "title");
            if !t.is_empty() && seen.insert(t.clone()) {
                added.push(FreshPage { title: t, at: stamp(&s(&c, "timestamp")), edits: 0, pic: None });
            }
        }
        let mut count: Vec<FreshPage> = Vec::new();
        for c in rows(&edits) {
            let t = s(&c, "title");
            if t.is_empty() {
                continue;
            }
            match count.iter_mut().find(|p| p.title == t) {
                Some(p) => p.edits += 1,
                None => count.push(FreshPage { title: t, at: stamp(&s(&c, "timestamp")), edits: 1, pic: None }),
            }
        }
        let mut hot: Vec<FreshPage> = count.iter().filter(|p| p.edits >= 3 && p.title != "Main Page").cloned().collect();
        hot.sort_by(|a, b| b.edits.cmp(&a.edits));
        hot.truncate(8);
        added.truncate(16);
        let mut wanted: Vec<String> = Vec::new();
        for p in added.iter().chain(&hot) {
            if !wanted.contains(&p.title) {
                wanted.push(p.title.clone());
            }
        }
        if !wanted.is_empty() {
            let d = api(&link, &site, json!({ "action": "query", "titles": wanted.iter().take(50).cloned().collect::<Vec<_>>().join("|"), "prop": "pageimages", "piprop": "thumbnail", "pithumbsize": 160 })).await;
            let pics: HashMap<String, String> =
                d.as_ref().and_then(|d| d.pointer("/query/pages")).and_then(Value::as_array).into_iter().flatten().filter_map(|p| Some((s(p, "title"), thumb(p)?))).collect();
            for p in added.iter_mut().chain(hot.iter_mut()) {
                p.pic = pics.get(&p.title).cloned();
            }
        }
        let mut edited: Vec<FreshPage> = count.into_iter().filter(|p| !seen.contains(&p.title) && p.title != "Main Page").collect();
        edited.sort_by(|a, b| b.edits.cmp(&a.edits));
        edited.truncate(CHANGED_PAGES);
        let fresh = Fresh { added, hot, edited: edited.clone() };
        let site2 = site.clone();
        let _ = this.update(cx, |r, cx| {
            r.wiki.fresh.insert(site2, fresh);
            cx.notify();
        });
        // (Neither list answered: asked again after a while, not kept empty for the session.)
        if made.is_none() && edits.is_none() {
            cx.background_executor().timer(crate::pages::games::RETRY).await;
            let _ = this.update(cx, |r, cx| {
                r.wiki.asked_fresh.remove(&site);
                cx.notify();
            });
            return;
        }
        if edited.is_empty() {
            return;
        }
        // Each page's text now, then as it was two weeks ago: four asked at a time.
        let text = |p: &Value| p.pointer("/revisions/0/slots/main/content").and_then(Value::as_str).map(str::to_string);
        let d = api(
            &link,
            &site,
            json!({ "action": "query", "titles": edited.iter().map(|p| p.title.clone()).collect::<Vec<_>>().join("|"), "prop": "revisions|pageimages", "rvprop": "content", "rvslots": "main", "piprop": "thumbnail", "pithumbsize": 160 }),
        )
        .await;
        let now: HashMap<String, (String, Option<String>)> =
            d.as_ref().and_then(|d| d.pointer("/query/pages")).and_then(Value::as_array).into_iter().flatten().filter_map(|p| Some((s(p, "title"), (text(p)?, thumb(p))))).collect();
        let todo: Vec<&FreshPage> = edited.iter().filter(|p| now.contains_key(&p.title)).collect();
        let mut out: Vec<Changed> = Vec::new();
        for part in todo.chunks(4) {
            let asks = part.iter().map(|p| {
                api(&link, &site, json!({ "action": "query", "titles": p.title, "prop": "revisions", "rvprop": "content", "rvslots": "main", "rvlimit": 1, "rvstart": since, "rvdir": "older" }))
            });
            for (p, o) in part.iter().zip(futures::future::join_all(asks).await) {
                let Some(old) = o.as_ref().and_then(|o| o.pointer("/query/pages/0")).and_then(text) else { continue };
                let (new_text, pic) = &now[&p.title];
                let lines = wikitext::box_changes(&old, new_text);
                if !lines.is_empty() {
                    out.push(Changed { title: p.title.clone(), pic: pic.clone(), lines });
                }
            }
        }
        let _ = this.update(cx, |r, cx| {
            r.wiki.changes.insert(site, out);
            cx.notify();
        });
    })
    .detach();
}

/// A page, parsed by the wiki and made plain to draw; a category is its pages as a list.
fn load_page(link: Link, site: String, title: String, cx: &mut Context<Root>) {
    cx.spawn(async move |this, cx| {
        let k = key(&site, &title);
        let page = if title.to_lowercase().starts_with("category:") {
            api(&link, &site, json!({ "action": "query", "list": "categorymembers", "cmtitle": title, "cmlimit": 500, "cmnamespace": "0|14", "cmprop": "title" })).await.map(|d| {
                let list: Vec<String> = d.pointer("/query/categorymembers").and_then(Value::as_array).into_iter().flatten().map(|m| s(m, "title")).filter(|t| !t.is_empty()).collect();
                let mut html = String::from("<ul>");
                for name in &list {
                    let shown = name.strip_prefix("Category:").unwrap_or(name);
                    html.push_str(&format!("<li><a href=\"{}{}\">{}</a></li>", wikihtml::PAGE, attr_esc(name), attr_esc(shown)));
                }
                html.push_str("</ul>");
                Article { title: title.clone(), html: if list.is_empty() { SharedString::default() } else { html.into() }, infobox: None, images: Vec::new(), cats: Vec::new(), missing: list.is_empty(), sections: Vec::new() }
            })
        } else {
            let d = link
                .call("wikiApi", json!({ "site": site, "q": { "action": "parse", "page": title, "prop": "text|categories|displaytitle", "redirects": 1, "disableeditsection": 1, "disablelimitreport": 1 } }))
                .await
                .ok()
                .and_then(Result::ok);
            match d {
                Some(d) if d.get("error").is_some() => Some(Article { title: title.clone(), html: SharedString::default(), infobox: None, images: Vec::new(), cats: Vec::new(), missing: true, sections: Vec::new() }),
                Some(d) => d.get("parse").and_then(|p| {
                    let text = p.get("text").and_then(Value::as_str)?;
                    let clean = wikihtml::clean_page(text, &site);
                    let cats = p
                        .get("categories")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .filter(|c| !c.get("hidden").and_then(Value::as_bool).unwrap_or(false))
                        .map(|c| s(c, "category").replace('_', " "))
                        .filter(|c| !c.is_empty())
                        .take(30)
                        .collect();
                    Some(Article { title: Some(s(p, "title")).filter(|t| !t.is_empty()).unwrap_or(title.clone()), html: clean.html.into(), infobox: clean.infobox, images: clean.images, cats, missing: false, sections: clean.sections })
                }),
                None => None,
            }
        };
        let failed = page.is_none();
        let _ = this.update(cx, |r, cx| {
            // Too many kept: the one shown longest ago goes.
            if r.wiki.pages.len() > 40
                && let Some(old) = r.wiki.pages.keys().filter(|x| **x != k).min_by_key(|x| r.wiki.page_used.get(*x).copied().unwrap_or(0)).cloned()
            {
                r.wiki.pages.remove(&old);
                r.wiki.asked_pages.remove(&old);
                r.wiki.page_used.remove(&old);
            }
            r.wiki.pages.insert(k.clone(), page.map(Rc::new));
            cx.notify();
        });
        // Not read (offline a moment): asked again while it is shown, not only once 40 others pushed it out.
        if failed {
            cx.background_executor().timer(crate::pages::games::RETRY).await;
            let _ = this.update(cx, |r, cx| {
                r.wiki.asked_pages.remove(&k);
                cx.notify();
            });
        }
    })
    .detach();
}

fn attr_esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// A kind's pages as a table, read 50 at a time and shown as they come; each wiki's short codes said in its words.
fn load_table(link: Link, site: String, template: String, mut said: HashMap<String, Option<String>>, cx: &mut Context<Root>) {
    cx.spawn(async move |this, cx| {
        let k = key(&site, &template);
        let name = wikitext::kind_name(&template);
        let mut items: Vec<Item> = Vec::new();
        let mut seen = HashSet::new();
        let mut cont: Option<serde_json::Map<String, Value>> = Some(serde_json::Map::new());
        while let Some(c) = cont.take() {
            if items.len() >= ITEMS_MAX {
                break;
            }
            let mut q = json!({ "action": "query", "generator": "embeddedin", "geititle": template, "geinamespace": 0, "geilimit": 50, "prop": "revisions|pageimages|info", "rvprop": "content", "rvslots": "main", "piprop": "thumbnail", "pithumbsize": 160, "inprop": "url", "redirects": 1 });
            if let Some(o) = q.as_object_mut() {
                o.extend(c);
            }
            let Some(d) = api(&link, &site, q).await else {
                // What came stays, said to be cut short; read again after a while.
                let _ = this.update(cx, |r, cx| {
                    let t = r.wiki.tables.entry(k.clone()).or_default();
                    t.failed = true;
                    t.done = true;
                    cx.notify();
                });
                cx.background_executor().timer(crate::pages::games::RETRY).await;
                let _ = this.update(cx, |r, cx| {
                    r.wiki.asked_tables.remove(&k);
                    cx.notify();
                });
                return;
            };
            for p in d.pointer("/query/pages").and_then(Value::as_array).into_iter().flatten() {
                if let Some(it) = wikitext::item(p, &name, &template, &site)
                    && seen.insert(it.id.clone())
                {
                    items.push(it);
                }
            }
            say_codes(&link, &site, &mut items, &mut said).await;
            cont = d.get("continue").and_then(Value::as_object).cloned();
            let done = cont.is_none() || items.len() >= ITEMS_MAX;
            let (now, said_now) = (items.clone(), said.clone());
            let k2 = k.clone();
            let alive = this
                .update(cx, |r, cx| {
                    r.wiki.tables.insert(k2, Table { items: Rc::new(now), done, failed: false, rev: TABLE_REV.fetch_add(1, std::sync::atomic::Ordering::Relaxed) });
                    r.wiki.said.extend(said_now);
                    cx.notify();
                })
                .is_ok();
            if !alive {
                return;
            }
        }
    })
    .detach();
}

const SAY_SPLIT: &str = "\n@@BK@@\n";

/// The lines a wiki writes as a short code put in the wiki's own words, as its pages show them: the templates asked
/// of the wiki a few at a time (Grand Piece Online's rarity read "L" and "LC" before this).
async fn say_codes(link: &Link, site: &str, items: &mut [Item], said: &mut HashMap<String, Option<String>>) {
    let mut raws: Vec<String> = Vec::new();
    for i in items.iter() {
        for (_, raw) in &i.codes {
            if !said.contains_key(&key(site, raw)) && !raws.contains(raw) {
                raws.push(raw.clone());
            }
        }
    }
    let mut i = 0;
    while i < raws.len() {
        let mut part: Vec<String> = Vec::new();
        while i < raws.len() && part.iter().map(|p| p.len() + SAY_SPLIT.len()).sum::<usize>() + raws[i].len() <= 1800 {
            part.push(raws[i].clone());
            i += 1;
        }
        if part.is_empty() {
            said.insert(key(site, &raws[i]), None);
            i += 1;
            continue;
        }
        let d = api(link, site, json!({ "action": "expandtemplates", "prop": "wikitext", "text": part.join(SAY_SPLIT) })).await;
        let out: Vec<String> = d.as_ref().and_then(|d| d.pointer("/expandtemplates/wikitext")).and_then(Value::as_str).map(|t| t.split(SAY_SPLIT.trim()).map(str::to_string).collect()).unwrap_or_default();
        for (n, raw) in part.iter().enumerate() {
            let words = if out.len() == part.len() { wikitext::clean(&out[n], false) } else { String::new() };
            said.insert(key(site, raw), (!words.is_empty() && words.chars().count() <= 40 && !words.contains('\n')).then_some(words));
        }
    }
    for item in items.iter_mut() {
        for (field, raw) in item.codes.clone() {
            if let Some(Some(words)) = said.get(&key(site, &raw)) {
                for f in item.fields.iter_mut().filter(|(k, _)| *k == field) {
                    f.1 = words.clone();
                }
            }
        }
    }
}

// ---- Moving about ----

fn go(r: &mut Root, v: View) {
    if r.wiki.views.len() > 30 {
        r.wiki.views.remove(0);
    }
    if matches!(v, View::Table(_)) && view_now(&r.wiki) != v {
        r.wiki.sort = None;
        r.wiki.filters.clear();
        r.wiki.ranges.clear();
        r.wiki.ranges_for.clear();
        r.wiki.filtering = false;
        r.wiki.picked.clear();
        r.wiki.comparing = false;
    }
    r.wiki.views.push(v);
    r.wiki.hits.clear();
    r.games.page_scroll.set_offset(point(px(0.), px(0.)));
}

fn back(r: &mut Root) {
    if r.wiki.views.len() > 1 {
        r.wiki.views.pop();
    }
}

fn home(r: &mut Root) {
    r.wiki.views = vec![View::Home];
    r.games.page_scroll.set_offset(point(px(0.), px(0.)));
}

/// What was typed in the wiki's search box: suggestions after a short pause (Enter searches every page).
pub fn typed(r: &mut Root, cx: &mut Context<Root>) {
    let q = r.wiki_q.read(cx).value().trim().to_string();
    r.wiki.typed += 1;
    if q.chars().count() < 2 {
        r.wiki.hits.clear();
        r.wiki.hits_for.clear();
        cx.notify();
        return;
    }
    let (n, site) = (r.wiki.typed, r.wiki.site.clone());
    let Some(link) = r.link.clone() else { return };
    cx.spawn(async move |this, cx| {
        cx.background_executor().timer(Duration::from_millis(220)).await;
        let still = this.update(cx, |r, _| r.wiki.typed == n).unwrap_or(false);
        if !still {
            return;
        }
        let d = api(&link, &site, json!({ "action": "query", "list": "prefixsearch", "pssearch": q, "pslimit": 8, "psnamespace": 0 })).await;
        let hits: Vec<String> = d.as_ref().and_then(|d| d.pointer("/query/prefixsearch")).and_then(Value::as_array).into_iter().flatten().map(|p| s(p, "title")).filter(|t| !t.is_empty()).collect();
        let _ = this.update(cx, |r, cx| {
            if r.wiki.typed == n {
                r.wiki.hits = hits;
                r.wiki.hits_for = q;
                cx.notify();
            }
        });
    })
    .detach();
}

/// Enter in the search box: every page with the words in it.
pub fn search(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    let q = r.wiki_q.read(cx).value().trim().to_string();
    if q.is_empty() {
        return;
    }
    // (The words stay in the box while their results show, as a search page keeps them.)
    let _ = window;
    r.wiki.typed += 1;
    go(r, View::Search(q));
    cx.notify();
}

/// A link clicked in an article: its own pages open here, the rest in the browser.
pub fn settle(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    boxes(r, window, cx);
    let Some(href) = CLICKED.lock().ok().and_then(|mut c| c.take()) else { return };
    if let Some(anchor) = href.strip_prefix(wikihtml::ANCHOR) {
        // A place in this page: its part scrolled to, when it is one of its Contents (WikiSite.tsx's jump).
        let same = |a: &str| a.replace(' ', "_").to_lowercase();
        if let View::Page { title, .. } = view_now(&r.wiki)
            && let Some(Some(a)) = r.wiki.pages.get(&key(&r.wiki.site, &title))
            && let Some(at) = a.sections.iter().position(|s| s.anchor.as_deref().is_some_and(|x| same(x) == same(anchor)))
        {
            jump(r, at);
        }
    } else if let Some(title) = href.strip_prefix(wikihtml::PAGE) {
        let title = title.split('#').next().unwrap_or(title).to_string();
        let table = match view_now(&r.wiki) {
            View::Page { table, .. } => table,
            _ => None,
        };
        go(r, View::Page { title, table });
    } else if let Some(url) = href.strip_prefix(wikihtml::OUT) {
        r.send("wikiOpen", json!({ "url": url }));
    }
    let _ = window;
    cx.notify();
}

/// A text box whose typing redraws the page.
fn new_box(hint: String, window: &mut Window, cx: &mut Context<Root>) -> Entity<InputState> {
    let e = cx.new(|cx| InputState::new(window, cx).placeholder(hint));
    cx.subscribe(&e, |_, _, ev: &InputEvent, cx| {
        if matches!(ev, InputEvent::Change) {
            cx.notify();
        }
    })
    .detach();
    e
}

/// The boxes the view shown types into, made here as they are needed (a box needs the window): each number line's Min
/// and Max while a table's filters are open, the tries on a page (WikiView.tsx's wk-range and DropOdds).
fn boxes(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    match view_now(&r.wiki) {
        View::Table(t) if r.wiki.filtering => {
            let k = key(&r.wiki.site, &t);
            if r.wiki.ranges_for == k {
                return;
            }
            let Some(tb) = r.wiki.tables.get(&k).filter(|tb| !tb.items.is_empty()) else { return };
            let lines = lines_of(&tb.items);
            let wanted: Vec<String> = facets(&lines).into_iter().filter(|l| l.number).map(|l| l.field.clone()).collect();
            // (Asked again while the table still comes in: a line can turn up with more pages.)
            // (A read cut short by a failure counts as done too, and is read on later: not settled then.)
            if tb.done && !tb.failed {
                r.wiki.ranges_for = k;
            }
            for f in wanted {
                if !r.wiki.ranges.iter().any(|(x, _, _)| *x == f) {
                    let (lo, hi) = (new_box(r.tr("wiki.min"), window, cx), new_box(r.tr("wiki.max"), window, cx));
                    r.wiki.ranges.push((f, lo, hi));
                }
            }
        }
        View::Page { title, .. } if r.wiki.tried.as_ref().is_none_or(|(t, _)| *t != title) => {
            let e = new_box("0".into(), window, cx);
            r.wiki.tried = Some((title, e));
        }
        _ => {}
    }
}

/// Scrolls the page to a part of the shown article (its place in the Contents), where it was last drawn.
fn jump(r: &mut Root, at: usize) {
    let Some(top) = PART_TOPS.lock().ok().and_then(|m| m.get(&at).copied()) else { return };
    let s = &r.games.page_scroll;
    let mut off = s.offset();
    off.y = (off.y + s.bounds().top() + px(12.) - top).min(px(0.)).max(-s.max_offset().y);
    s.set_offset(off);
}

// ---- Drawing ----
// The Wiki part of a game is drawn as a wiki is, not in the app's panels (the old page's app.css ".ws", read again
// 2026-11-15 after the owner's "giao diện wiki đã đúng với bản C# cũ chưa": it was not, it had become the app's own
// chips and blocks): a sheet of its own in the game's colour, its name and search on a head bar, its parts as words
// with a rule under the one shown, serif headings over a rule, links in the game's colour, tables with a line on
// every side of every cell, an item as an article with its box of facts.

/// The sheet's colours (.ws's --ws-bg, --ws-box, --ws-head, --ws-line, --ws-text, --ws-dim, --ws-link and the game's
/// own --gt): the old page's on a dark theme; the theme's own grounds, the game's colour mixed in the same shares, on a
/// light one, where the old dark sheet's words (the theme's) would not read.
pub(crate) struct Ws {
    pub bg: Hsla,
    pub boxed: Hsla,
    pub head: Hsla,
    pub line: Hsla,
    pub text: Hsla,
    pub dim: Hsla,
    pub link: Hsla,
    /// The game's own colour, for the rule under the part shown, a bar's fill, an infobox's title.
    pub own: Hsla,
}

pub(crate) fn ws() -> Ws {
    let hex = |h: u32| -> Hsla { rgb(h).into() };
    let dark = crate::theme::lightness(c(PANEL)) < 50.;
    let game = game_tint().map(|t| Hsla { a: 1., ..t });
    let g = game.unwrap_or(hex(0x6b7280));
    let solid = |col: u32| Hsla { a: 1., ..c(col) };
    let link = if dark { mix(hex(0xffffff), game.unwrap_or(hex(0x7aa2f7)), 0.6) } else { mix(hex(0x000000), game.unwrap_or(hex(0x2f5fd0)), 0.7) };
    if dark {
        Ws { bg: mix(hex(0x15171c), g, 0.07), boxed: mix(hex(0x1d2027), g, 0.10), head: mix(hex(0x262a33), g, 0.16), line: hex(0x3a3f4b), text: hex(0xe6e7ea), dim: hex(0xa3a8b3), link, own: game.unwrap_or(link) }
    } else {
        Ws { bg: mix(solid(PANEL), g, 0.07), boxed: mix(solid(RAISED), g, 0.10), head: mix(solid(RAISED_2), g, 0.16), line: solid(LINE), text: c(TEXT), dim: c(MUTED), link, own: game.unwrap_or(link) }
    }
}

/// The sheet's headings' letters (--ws-serif: Cambria, Georgia, Times New Roman).
const SERIF: &str = "Cambria";

/// A box on the sheet (.ws-changed-item, .ws-odds, a table's ground): the sheet's own, never the app's block.
fn boxed() -> Div {
    let k = ws();
    div().rounded(px(8.)).bg(k.boxed).border_1().border_color(k.line)
}

/// A box of the sheet that can be pressed (.ws-changed-item, .ws-kind): the box's ground and line; under the mouse
/// the head's ground and a line in the link colour.
fn ws_tile(id: impl Into<ElementId>, radius: f32) -> Hot {
    let k = ws();
    hot(id).rounded(px(radius)).border_1().tones(k.boxed, k.head).edge(k.line, k.link).cursor_pointer()
}

fn note(text: String) -> Div {
    div().py(px(28.)).text_size(px(T_M)).text_color(ws().dim).child(text)
}

/// .ws-h1: 30 px serif, not bold, over a rule.
fn h1(text: impl Into<SharedString>) -> Div {
    div().w_full().pb(px(4.)).border_b_1().border_color(ws().line).font_family(SERIF).text_size(px(30.)).font_weight(FontWeight::NORMAL).line_height(px(38.)).child(text.into())
}

/// .ws-h2: 22 px serif over a rule; what it leaves unsaid under it, as .ws-tagline.
fn h2(text: String, sub: Option<String>) -> Div {
    let k = ws();
    div()
        .pt(px(8.))
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(div().w_full().pb(px(3.)).border_b_1().border_color(k.line).font_family(SERIF).text_size(px(22.)).font_weight(FontWeight::NORMAL).line_height(px(29.)).child(text))
        .when_some(sub, |d, s| d.child(div().text_size(px(T_M)).text_color(k.dim).child(s)))
}

fn link_text(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Stateful<Div> {
    // (.ws-link: the game's colour, 60 % of it; underlined under the mouse.)
    focusable(div().id(id)).cursor_pointer().text_color(ws().link).hover(|st| st.underline()).child(text.into())
}

/// A page's picture, made at the box's pixels; a page the wiki gives no picture gets its first letter on a colour of
/// its own (the same every time, from its name) rather than a grey slab.
fn pic_box(url: Option<&String>, letter: &str, w: f32, h: f32, cx: &mut App) -> Div {
    let p = url.and_then(|u| images::pic_at(u, w, h, cx));
    let hue = letter.bytes().fold(2166136261u32, |h, b| (h ^ b as u32).wrapping_mul(16777619)) % 360;
    let first: String = letter.chars().find(|c| c.is_alphanumeric()).map(|c| c.to_uppercase().collect()).unwrap_or_default();
    div()
        .w(px(w))
        .h(px(h))
        .flex_none()
        .rounded(px(R_S))
        .overflow_hidden()
        .flex()
        .items_center()
        .justify_center()
        .font_weight(FontWeight::BOLD)
        .map(|d| match p {
            Some(p) => d.bg(c(RAISED_2)).child(img(p).size_full().object_fit(ObjectFit::Contain)),
            None => d
                .bg(hsla(hue as f32 / 360., 0.32, 0.26, 1.))
                .text_color(hsla(hue as f32 / 360., 0.5, 0.78, 1.))
                .text_size(px((w.min(h) * 0.42).clamp(10., 40.)))
                .child(first),
        })
}

fn word(root: &Root, f: &str) -> String {
    if root.i18n.vietnamese { wikitext::vi_word(f).map(str::to_string).unwrap_or_else(|| f.to_string()) } else { f.to_string() }
}

fn n_text(root: &Root, n: i64) -> String {
    crate::pages::games::thousands(n, root.i18n.vietnamese)
}

/// The Wiki tab: the wiki's bar (its name, the search, its parts) over the view shown.
pub fn view(root: &Root, width: f32, cx: &mut Context<Root>) -> AnyElement {
    let w = &root.wiki;
    let site = w.site.clone();
    let about = match w.abouts.get(&site) {
        Some(Some(a)) => a.clone(),
        Some(None) => {
            return div()
                .flex()
                .flex_col()
                .gap(px(10.))
                .child(note(root.tr("wiki.offline")))
                .child(Button::new("wk-retry", root.tr("wiki.retry")).icon("refresh").on_click(cx.listener(|r, _, _, cx| {
                    let site = r.wiki.site.clone();
                    r.wiki.abouts.remove(&site);
                    r.wiki.asked_about.remove(&site);
                    cx.notify();
                })))
                .into_any_element();
        }
        None => return note(root.tr("news.connecting")).into_any_element(),
    };
    let now = view_now(w);
    let kinds = w.kinds.get(&site).cloned();
    let saved = marks(root, "saved");

    // The bar: the wiki's face (home), the search with its suggestions.
    let logo = about.logo.as_ref().and_then(|l| images::pic(l, cx));
    let face = focusable(div().id("wk-site"))
        .flex_none()
        .flex()
        .items_center()
        .gap(px(10.))
        .cursor_pointer()
        .map(|d| match logo {
            Some(p) => d.child(div().h(px(24.)).max_w(px(140.)).child(img(p).h(px(24.)).object_fit(ObjectFit::Contain))),
            None => d.child(icon("book", 16., ws().link)),
        })
        .child(div().font_family(SERIF).text_size(px(15.)).font_weight(FontWeight::BOLD).truncate().max_w(px(320.)).hover(|st| st.underline()).child(about.name.clone()))
        .on_click(cx.listener(|r, _, _, cx| {
            home(r);
            cx.notify();
        }));
    let q_now = root.wiki_q.read(cx).value().trim().to_string();
    let mut find = div().relative().w(px(340.)).flex_shrink(1.).min_w(px(180.)).child(field(&root.wiki_q, Some("search")));
    if !w.hits.is_empty() && w.hits_for == q_now {
        let mut menu = div().w(px(440.)).p(px(6.)).rounded(px(R_L)).bg(c(RAISED_2)).border_1().border_color(ws().line).shadow_lg().flex().flex_col();
        for (i, h) in w.hits.iter().enumerate() {
            let title = h.clone();
            menu = menu.child(menu_row(("wk-hit", i)).px(px(10.)).py(px(7.)).rounded(px(R_S)).text_size(px(T_M)).child(h.clone()).on_click(cx.listener(move |r, _, window, cx| {
                r.wiki_q.update(cx, |s, cx| s.set_value("", window, cx));
                r.wiki.typed += 1;
                go(r, View::Page { title: title.clone(), table: None });
                cx.notify();
            })));
        }
        let all = root.i18n.tv("wiki.searchAll", &[("q", &q_now)]);
        menu = menu.child(
            menu_row("wk-hit-all")
                .px(px(10.))
                .py(px(7.))
                .rounded(px(R_S))
                .flex()
                .items_center()
                .gap(px(8.))
                .text_size(px(T_M))
                .text_color(c(TEXT_2))
                .child(icon("search", 14., c(MUTED)))
                .child(all)
                .on_click(cx.listener(|r, _, window, cx| search(r, window, cx))),
        );
        find = find.child(div().absolute().top(px(40.)).left_0().child(deferred(anchored().snap_to_window_with_margin(px(8.)).child(menu)).with_priority(1)));
    }
    let k = ws();
    let top_bar = div().flex().items_center().gap(px(16.)).px(px(20.)).py(px(10.)).bg(k.head).border_b_1().border_color(k.line).child(face).child(div().flex_1()).child(find);

    // Its parts: home, its biggest kinds, saved (.ws-nav: words in the link colour; the one shown bold, in the
    // sheet's text colour, over a 2 px rule in the game's own).
    let part = |id: ElementId, on: bool, mark: Option<&'static str>, label: String| {
        focusable(div().id(id))
            .relative()
            .pt(px(9.))
            .pb(px(8.))
            .flex()
            .items_center()
            .gap(px(5.))
            .when(on, |d| d.child(div().absolute().left_0().right_0().bottom(px(-1.)).h(px(2.)).bg(k.own)))
            .text_size(px(13.5))
            .text_color(if on { k.text } else { k.link })
            .when(on, |d| d.font_weight(FontWeight::BOLD))
            .when(!on, |d| d.cursor_pointer().hover(|st| st.underline()))
            .when_some(mark, |d, m| d.child(icon(m, 13., if on { k.text } else { k.link })))
            .child(label)
    };
    let mut nav = div().flex().flex_wrap().items_center().gap_x(px(18.)).gap_y(px(2.)).px(px(20.)).border_b_1().border_color(k.line);
    nav = nav.child(part("wk-nav-home".into(), now == View::Home, Some("home"), root.tr("wiki.home")).on_click(cx.listener(|r, _, _, cx| {
        home(r);
        cx.notify();
    })));
    for (i, kind) in kinds.iter().flatten().take(8).enumerate() {
        let t = kind.template.clone();
        let on = now == View::Table(t.clone());
        nav = nav.child(part(("wk-nav-kind", i).into(), on, None, kind.name.clone()).on_click(cx.listener(move |r, _, _, cx| {
            go(r, View::Table(t.clone()));
            cx.notify();
        })));
    }
    let saved_on = now == View::Saved;
    let saved_label = if saved.is_empty() { root.tr("wiki.saved") } else { format!("{} ({})", root.tr("wiki.saved"), saved.len()) };
    nav = nav.child(part("wk-nav-saved".into(), saved_on, Some("bookmark"), saved_label).on_click(cx.listener(|r, _, _, cx| {
        go(r, View::Saved);
        cx.notify();
    })));

    let body: AnyElement = match &now {
        View::Home => home_view(root, &about, kinds.as_deref(), width, cx).into_any_element(),
        View::Saved => saved_view(root, &saved, width, cx).into_any_element(),
        View::Search(q) => search_view(root, q, width, cx).into_any_element(),
        View::Table(t) => table_view(root, t, width, cx),
        View::Page { title, table } => article_view(root, &about, title, table.as_deref(), width, cx),
    };
    div()
        .mt(px(4.))
        .flex()
        .flex_col()
        .rounded(px(6.))
        .bg(k.bg)
        .border_1()
        .border_color(k.line)
        .overflow_hidden()
        .text_color(k.text)
        .child(top_bar)
        .child(nav)
        .child(div().px(px(28.)).pt(px(18.)).pb(px(22.)).flex().flex_col().gap(px(16.)).child(body).child(credit(root, &about, cx)))
        .into_any_element()
}

/// The pages marked on this wiki (saved or seen lately): their titles, newest first.
fn marks(root: &Root, list: &str) -> Vec<String> {
    root.state
        .pointer(&format!("/wikis/{list}"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|m| s(m, "site") == root.wiki.site)
        .map(|m| s(m, "title"))
        .filter(|t| !t.is_empty())
        .collect()
}

fn credit(root: &Root, about: &About, cx: &mut Context<Root>) -> Div {
    let site = root.wiki.site.clone();
    let shown = site.trim_start_matches("https://").to_string();
    div()
        .pt(px(6.))
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(6.))
        .text_size(px(T_S))
        .text_color(c(MUTED))
        .child(root.i18n.tv("wiki.credit", &[("site", &about.name)]))
        .child(link_text("wk-credit-site", shown).on_click(cx.listener(move |r, _, _, _| r.send("wikiOpen", json!({ "url": site })))))
        .when_some(about.rights.clone(), |d, (name, url)| {
            d.child("·").child(link_text("wk-credit-rights", name).on_click(cx.listener(move |r, _, _, _| r.send("wikiOpen", json!({ "url": url })))))
        })
}

fn title_list(prefix: &'static str, titles: &[String], width: f32, cx: &mut Context<Root>) -> Div {
    let (_, w) = grid_fit(width, 300., GRID_GAP);
    grid(GRID_GAP).children(titles.iter().enumerate().map(|(i, t)| {
        let title = t.clone();
        result_card((prefix, i), t, None, w)
            .on_click(cx.listener(move |r, _, _, cx| {
                go(r, View::Page { title: title.clone(), table: None });
                cx.notify();
            }))
            .into_any_element()
    }))
}

fn home_view(root: &Root, about: &About, kinds: Option<&[Kind]>, width: f32, cx: &mut Context<Root>) -> Div {
    let w = &root.wiki;
    let site = w.site.clone();
    let main = about.main_page.clone();
    let open_site = site.clone();
    let tag = div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(8.))
        .text_size(px(T_M))
        .text_color(c(MUTED))
        .child(root.i18n.tv("wiki.articles", &[("n", &n_text(root, about.articles))]))
        .child("·")
        .child(link_text("wk-main", root.tr("wiki.mainPage")).on_click(cx.listener(move |r, _, _, cx| {
            go(r, View::Page { title: main.clone(), table: None });
            cx.notify();
        })))
        .child("·")
        .child(link_text("wk-open-site", root.tr("wiki.openPage")).on_click(cx.listener(move |r, _, _, _| r.send("wikiOpen", json!({ "url": open_site })))));
    let mut body = div().flex().flex_col().gap(px(14.)).child(div().flex().flex_col().gap(px(6.)).child(h1(about.name.clone())).child(tag));

    // New on the wiki: pages added as pictures in a row, numbers changed, pages edited most as a ranking.
    if let Some(f) = w.fresh.get(&site).filter(|f| !f.added.is_empty() || !f.hot.is_empty()) {
        body = body.child(h2(root.tr("wiki.fresh"), Some(root.i18n.tv("wiki.freshText", &[("n", &FRESH_DAYS.to_string())]))));
        if !f.added.is_empty() {
            // (As many a row as fit the page, ending where it does.)
            let card_w = 132.;
            let mut row = grid(10.);
            for (i, p) in f.added.iter().enumerate() {
                let title = p.title.clone();
                let last = p.title.rsplit('/').next().unwrap_or(&p.title);
                let shown = if last.chars().all(|c| c.is_ascii_digit() || c == '.') { p.title.replace('/', " ") } else { last.to_string() };
                row = row.child(
                    hot(("wk-fresh", i))
                        .w(px(card_w))
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .cursor_pointer()
                        .child(div().w(px(card_w)).h(px(104.)).rounded(px(8.)).border_1().border_color(ws().line).bg(ws().boxed).overflow_hidden().child(pic_box(p.pic.as_ref(), &p.title, card_w - 2., 102., cx)))
                        .child(div().text_size(px(13.)).line_height(px(17.)).font_weight(FontWeight::BOLD).line_clamp(2).child(shown))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(5.))
                                .text_size(px(11.5))
                                .text_color(ws().dim)
                                .child(div().px(px(5.)).rounded(px(3.)).bg(Hsla { a: 0.3, ..ws().own }).text_color(ws().text).font_weight(FontWeight::BOLD).child(root.tr("wiki.freshNewOne")))
                                .child(crate::time::ago(crate::time::now_secs() - p.at, &root.i18n)),
                        )
                        .on_click(cx.listener(move |r, _, _, cx| {
                            go(r, View::Page { title: title.clone(), table: None });
                            cx.notify();
                        })),
                );
            }
            body = body.child(row);
        }
        if let Some(ch) = w.changes.get(&site).filter(|c| !c.is_empty()) {
            body = body.child(h2(root.tr("wiki.changed"), Some(root.i18n.tv("wiki.changedText", &[("n", &FRESH_DAYS.to_string())]))));
            let col_w = if width >= 900. { (width - 12.) / 2. } else { width };
            let mut grid = div().flex().flex_wrap().gap(px(12.));
            for (i, ch1) in ch.iter().enumerate() {
                let title = ch1.title.clone();
                let mut lines = div().flex().flex_col().gap(px(4.)).min_w_0();
                for l in &ch1.lines {
                    let tone = if l.up { OK } else { BAD };
                    lines = lines.child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap(px(6.))
                            .text_size(px(T_S))
                            .child(div().text_color(c(MUTED)).child(word(root, &l.field)))
                            .child(div().text_color(c(DIM)).line_through().child(l.was.clone()))
                            .child(div().text_color(c(MUTED)).child("→"))
                            .child(div().flex().items_center().gap(px(2.)).text_color(c(tone)).font_weight(FontWeight::SEMIBOLD).child(l.now.clone()).child(icon(if l.up { "chevronUp" } else { "chevronDown" }, 12., c(tone)))),
                    );
                }
                grid = grid.child(
                    ws_tile(("wk-changed", i), 8.)
                        .w(px(col_w))
                        .p(px(10.))
                        .flex()
                        .gap(px(12.))
                        .child(pic_box(ch1.pic.as_ref(), &ch1.title, 52., 52., cx))
                        .child(div().flex_1().min_w_0().flex().flex_col().gap(px(5.)).child(div().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).truncate().child(ch1.title.rsplit('/').next().unwrap_or(&ch1.title).to_string())).child(lines))
                        .on_click(cx.listener(move |r, _, _, cx| {
                            go(r, View::Page { title: title.clone(), table: None });
                            cx.notify();
                        })),
                );
            }
            body = body.child(grid);
        }
        if !f.hot.is_empty() {
            body = body.child(h2(root.tr("wiki.freshHot"), None));
            let most = f.hot[0].edits.max(1) as f32;
            let mut list = div().flex().flex_col().gap(px(4.));
            for (i, p) in f.hot.iter().enumerate() {
                let title = p.title.clone();
                list = list.child(
                    hot(("wk-hot", i))
                        .tones(ca(WHITE, 0.), ca(WHITE, 0.04))
                        .px(px(8.))
                        .py(px(6.))
                        .rounded(px(R_M))
                        .flex()
                        .items_center()
                        .gap(px(12.))
                        .cursor_pointer()
                        .child(div().w(px(18.)).text_size(px(T_S)).text_color(c(MUTED)).font_features(tnum()).child((i + 1).to_string()))
                        .child(pic_box(p.pic.as_ref(), &p.title, 32., 32., cx))
                        .child(div().w(px(260.)).text_size(px(T_M)).truncate().child(p.title.clone()))
                        .child(div().flex_1().h(px(4.)).rounded_full().bg(ws().boxed).child(div().h_full().rounded_full().bg(ws().own).w(relative((p.edits as f32 / most).clamp(0.02, 1.)))))
                        .child(div().w(px(90.)).text_size(px(T_S)).text_color(c(MUTED)).child(root.i18n.tv("wiki.editsN", &[("n", &p.edits.to_string())])))
                        .on_click(cx.listener(move |r, _, _, cx| {
                            go(r, View::Page { title: title.clone(), table: None });
                            cx.notify();
                        })),
                );
            }
            body = body.child(list);
        }
    }

    // The kinds as tiles: what the wiki keeps a box of facts for, each a table.
    match kinds {
        None => body = body.child(note(root.tr("news.connecting"))),
        Some([]) => {}
        Some(ks) => {
            body = body.child(h2(root.tr("wiki.tables"), Some(root.tr("wiki.tablesText"))));
            let (_, tile_w) = grid_fit(width, 200., GRID_GAP);
            let mut grid = crate::kit::grid(GRID_GAP);
            for (i, k) in ks.iter().enumerate() {
                let t = k.template.clone();
                grid = grid.child(
                    ws_tile(("wk-kind", i), 4.)
                        .w(px(tile_w))
                        .p(px(10.))
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .child(pic_box(k.pic.as_ref(), &k.name, 44., 44., cx))
                        .child(
                            div()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap(px(2.))
                                .child(div().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).truncate().child(k.name.clone()))
                                .child(div().text_size(px(T_S)).text_color(c(MUTED)).child(root.i18n.tv("wiki.pagesN", &[("n", &n_text(root, k.count))]))),
                        )
                        .on_click(cx.listener(move |r, _, _, cx| {
                            go(r, View::Table(t.clone()));
                            cx.notify();
                        })),
                );
            }
            body = body.child(grid);
        }
    }
    let recent: Vec<String> = marks(root, "recent").into_iter().take(12).collect();
    if !recent.is_empty() {
        body = body.child(h2(root.tr("wiki.recent"), None)).child(title_list("wk-recent", &recent, width, cx));
    }
    let saved: Vec<String> = marks(root, "saved").into_iter().take(12).collect();
    if !saved.is_empty() {
        body = body.child(h2(root.tr("wiki.saved"), None)).child(title_list("wk-saved-home", &saved, width, cx));
    }
    body
}

fn saved_view(root: &Root, saved: &[String], width: f32, cx: &mut Context<Root>) -> Div {
    let body = div().flex().flex_col().gap(px(14.)).child(
        div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(section_head(root.tr("wiki.saved"), (!saved.is_empty()).then_some(saved.len())))
            .child(div().text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("wiki.savedText"))),
    );
    if saved.is_empty() {
        return body.child(empty_state("bookmark", root.tr("wiki.savedNone"), None));
    }
    let (_, w) = grid_fit(width, 420., GRID_GAP);
    body.child(grid(GRID_GAP).children(saved.iter().enumerate().map(|(i, t)| {
        let (title, gone) = (t.clone(), t.clone());
        div()
            .relative()
            .child(result_card(("wk-saved", i), t, None, w).pr(px(110.)).on_click(cx.listener(move |r, _, _, cx| {
                go(r, View::Page { title: title.clone(), table: None });
                cx.notify();
            })))
            .child(
                div().absolute().top_0().bottom_0().right(px(40.)).flex().items_center().child(Button::new(("wk-unsave", i), root.tr("wiki.unsave")).quiet().small().on_click(cx.listener(move |r, _, _, _| {
                    let (site, game) = (r.wiki.site.clone(), r.wiki.game.clone());
                    r.send("wikiMark", json!({ "list": "saved", "site": site, "title": gone, "game": game, "on": false }));
                }))),
            )
            .into_any_element()
    })))
}

fn search_view(root: &Root, q: &str, width: f32, cx: &mut Context<Root>) -> Div {
    let found = root.wiki.results.get(&key(&root.wiki.site, q));
    let n = match found {
        Some(Some(hits)) => Some(hits.len()),
        _ => None,
    };
    let body = div().flex().flex_col().gap(px(14.)).child(section_head(root.i18n.tv("wiki.resultsFor", &[("q", q)]), n));
    match found {
        None => body.child(note(root.tr("news.connecting"))),
        Some(None) => body.child(note(root.tr("wiki.offline"))),
        Some(Some(hits)) if hits.is_empty() => body.child(empty_state("search", root.tr("wiki.nothing"), None)),
        Some(Some(hits)) => {
            let (_, w) = grid_fit(width, 420., GRID_GAP);
            body.child(grid(GRID_GAP).children(hits.iter().enumerate().map(|(i, (t, text))| {
                let title = t.clone();
                result_card(("wk-result", i), t, Some(text.as_str()).filter(|x| !x.is_empty()), w)
                    .on_click(cx.listener(move |r, _, _, cx| {
                        go(r, View::Page { title: title.clone(), table: None });
                        cx.notify();
                    }))
                    .into_any_element()
            })))
        }
    }
}

/// A page found or kept: a mark on its own small block, its title, its lines under it, and a chevron; the whole card
/// opens it.
fn result_card(id: impl Into<ElementId>, title: &str, text: Option<&str>, w: f32) -> Hot {
    let last = title.rsplit('/').next().unwrap_or(title).to_string();
    card(id)
        .w(px(w))
        .min_h(px(64.))
        .px(px(14.))
        .py(px(12.))
        .flex()
        .items_center()
        .gap(px(12.))
        .cursor_pointer()
        .child(div().flex_none().child(icon("book", 18., c(TEXT_2))))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(3.))
                .child(div().truncate().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).child(last))
                .when_some(text.map(str::to_string), |d, t| d.child(div().text_size(px(T_M)).line_height(px(19.)).text_color(c(TEXT_2)).line_clamp(2).child(t))),
        )
        .child(icon("chevronRight", 15., c(MUTED)))
}

/// Nothing to show: a mark on a block, what is missing in words, and what to do in quieter words.
fn empty_state(mark: &'static str, title: String, sub: Option<String>) -> Div {
    div()
        .py(px(40.))
        .flex()
        .flex_col()
        .items_center()
        .gap(px(10.))
        .child(div().size(px(48.)).rounded(px(R_L)).bg(ca(WHITE, 0.06)).flex().items_center().justify_center().child(icon(mark, 22., c(MUTED))))
        .child(div().text_size(px(T_H)).font_weight(FontWeight::SEMIBOLD).child(title))
        .when_some(sub, |d, t| d.child(div().max_w(px(420.)).text_center().text_size(px(T_M)).text_color(c(MUTED)).child(t)))
}

/// A line of a kind: how many items have it, whether it is a number, its values (short ones).
struct Line {
    field: String,
    number: bool,
    values: Vec<String>,
}

fn lines_of(items: &[Item]) -> Vec<Line> {
    let mut order: Vec<String> = Vec::new();
    let mut count: HashMap<String, usize> = HashMap::new();
    let mut nums: HashMap<String, usize> = HashMap::new();
    let mut values: HashMap<String, Vec<String>> = HashMap::new();
    for i in items {
        for (k, v) in &i.fields {
            if !count.contains_key(k) {
                order.push(k.clone());
            }
            *count.entry(k.clone()).or_default() += 1;
            if number_of(Some(v)).is_some() {
                *nums.entry(k.clone()).or_default() += 1;
            }
            let vs = values.entry(k.clone()).or_default();
            if !v.contains('\n') && v.chars().count() <= 40 && !vs.contains(v) {
                vs.push(v.clone());
            }
        }
    }
    let least = (items.len() as f64 * 0.15).max(2.);
    let mut out: Vec<(usize, Line)> = order
        .into_iter()
        .filter_map(|f| {
            let n = count[&f];
            (n as f64 >= least).then(|| {
                let mut vs = values.remove(&f).unwrap_or_default();
                vs.sort();
                (n, Line { number: nums.get(&f).copied().unwrap_or(0) as f64 >= n as f64 * 0.7, values: vs, field: f })
            })
        })
        .collect();
    out.sort_by(|a, b| b.0.cmp(&a.0));
    out.into_iter().map(|(_, l)| l).collect()
}

/// What a table can be filtered by (WikiView.tsx's facets): a number line's range, or a line with a few values
/// (rarity, type, element…).
fn facets(lines: &[Line]) -> Vec<&Line> {
    lines.iter().filter(|l| l.number || (l.values.len() >= 2 && l.values.len() <= 24)).take(8).collect()
}

fn table_view(root: &Root, template: &str, width: f32, cx: &mut Context<Root>) -> AnyElement {
    let w = &root.wiki;
    let name = w.kinds.get(&w.site).and_then(|ks| ks.iter().find(|k| k.template == template)).map(|k| k.name.clone()).unwrap_or_else(|| wikitext::kind_name(template));
    let none = Table::default();
    let table = w.tables.get(&key(&w.site, template)).unwrap_or(&none);
    if table.items.is_empty() {
        let says = if table.failed { "wiki.offline" } else if table.done { "wiki.nothing" } else { "news.connecting" };
        return div().flex().flex_col().gap(px(8.)).child(h1(name)).child(note(root.tr(says))).into_any_element();
    }
    let items: &[Item] = &table.items;
    let words: Vec<String> = fold(&root.wiki_tq.read(cx).value()).split_whitespace().map(str::to_string).collect();
    // The ranges as typed (read before the lines are known: they are part of what the cut is kept by).
    let typed: Vec<(String, Option<f64>, Option<f64>)> = w
        .ranges
        .iter()
        .map(|(f, lo, hi)| (f.clone(), number_of(Some(lo.read(cx).value().trim())), number_of(Some(hi.read(cx).value().trim()))))
        .collect();
    let cut_key = hash_of((
        &w.site,
        template,
        table.rev,
        &words,
        &w.filters,
        typed.iter().map(|(f, lo, hi)| (f, lo.map(f64::to_bits), hi.map(f64::to_bits))).collect::<Vec<_>>(),
        &w.sort,
    ));
    let kept = w.cut.borrow().as_ref().filter(|c| c.key == cut_key).map(|c| (c.lines.clone(), c.shown.clone()));
    let (lines, order) = kept.unwrap_or_else(|| {
        let lines = Rc::new(lines_of(items));
        let facets = facets(&lines);
        let ranges: Vec<&(String, Option<f64>, Option<f64>)> = typed.iter().filter(|(f, lo, hi)| (lo.is_some() || hi.is_some()) && facets.iter().any(|l| l.number && l.field == *f)).collect();
        let mut shown: Vec<usize> = (0..items.len())
            .filter(|k| {
                let i = &items[*k];
                if !words.is_empty() {
                    let all = fold(&format!("{} {}", i.name, i.fields.iter().map(|f| f.1.as_str()).collect::<Vec<_>>().join(" ")));
                    if !words.iter().all(|wd| all.contains(wd.as_str())) {
                        return false;
                    }
                }
                w.filters.iter().all(|(f, vs)| vs.is_empty() || value_of(i, f).is_some_and(|v| vs.iter().any(|x| x == v)))
                    && ranges.iter().all(|(f, lo, hi)| number_of(value_of(i, f)).is_some_and(|n| lo.is_none_or(|lo| n >= lo) && hi.is_none_or(|hi| n <= hi)))
            })
            .collect();
        if let Some((field, down)) = &w.sort {
            let number = lines.iter().find(|l| l.field == *field).is_some_and(|l| l.number);
            shown.sort_by(|a, b| {
                let (va, vb) = (value_of(&items[*a], field), value_of(&items[*b], field));
                match (va, vb) {
                    (None, None) => std::cmp::Ordering::Equal,
                    (None, _) => std::cmp::Ordering::Greater,
                    (_, None) => std::cmp::Ordering::Less,
                    (Some(x), Some(y)) => {
                        let o = if number { number_of(Some(x)).unwrap_or(f64::MIN).total_cmp(&number_of(Some(y)).unwrap_or(f64::MIN)) } else { x.cmp(y) };
                        if *down { o.reverse() } else { o }
                    }
                }
            });
        }
        let shown = Rc::new(shown);
        *w.cut.borrow_mut() = Some(TableCut { key: cut_key, lines: lines.clone(), shown: shown.clone() });
        (lines, shown)
    });
    let columns: Vec<&Line> = lines.iter().filter(|l| l.number || l.values.len() <= 30).take(COLUMNS).collect();
    let facets = facets(&lines);
    // A number line's range as typed in its boxes (WikiView.tsx's Filter min/max): either end may be left empty.
    let ranges: Vec<&(String, Option<f64>, Option<f64>)> = typed.iter().filter(|(f, lo, hi)| (lo.is_some() || hi.is_some()) && facets.iter().any(|l| l.number && l.field == *f)).collect();
    // Found by words, filtered by values and ranges, sorted by a column (worked out above, once for each change).
    let shown: Vec<&Item> = order.iter().map(|k| &items[*k]).collect();

    // (A read cut short says so beside what came.)
    let loading = if table.failed {
        Some(root.tr("wiki.offline"))
    } else {
        (!table.done).then(|| root.i18n.tv("wiki.loadingN", &[("n", &n_text(root, items.len() as i64))]))
    };
    let filtered = w.filters.iter().filter(|(_, v)| !v.is_empty()).count() + ranges.len();
    let mut tag = div().flex().flex_wrap().items_center().gap(px(10.)).text_size(px(T_M)).text_color(c(MUTED)).child(root.i18n.tv("wiki.pagesN", &[("n", &n_text(root, shown.len() as i64))]));
    if let Some(l) = loading {
        tag = tag.child("·").child(l);
    }
    let tools = div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(10.))
        .child(div().w(px(300.)).child(field(&root.wiki_tq, Some("search"))))
        .when(!facets.is_empty(), |d| {
            let label = if filtered > 0 { format!("{} ({filtered})", root.tr("wiki.filter")) } else { root.tr("wiki.filter") };
            let on = w.filtering || filtered > 0;
            d.child(chip("wk-filter", on).h(px(TOOL_H)).px(px(12.)).child(icon("sliders", 14., if on { c(INK) } else { c(MUTED) })).child(label).on_click(cx.listener(|r, _, _, cx| {
                r.wiki.filtering = !r.wiki.filtering;
                cx.notify();
            })))
        })
        .when(filtered > 0, |d| {
            d.child(Button::new("wk-filter-clear", root.tr("wiki.clear")).quiet().small().on_click(cx.listener(|r, _, window, cx| {
                r.wiki.filters.clear();
                for (_, lo, hi) in r.wiki.ranges.clone() {
                    lo.update(cx, |s, cx| s.set_value("", window, cx));
                    hi.update(cx, |s, cx| s.set_value("", window, cx));
                }
                cx.notify();
            })))
        });
    let mut body = div()
        .flex()
        .flex_col()
        .gap(px(12.))
        .child(div().flex().flex_wrap().items_center().gap(px(16.)).child(div().flex().flex_col().gap(px(2.)).child(div().font_family(SERIF).text_size(px(30.)).font_weight(FontWeight::NORMAL).line_height(px(38.)).child(name)).child(tag)).child(div().flex_1()).child(tools));

    if w.filtering {
        let mut panel = boxed().p(px(14.)).flex().flex_col().gap(px(10.));
        for (fi, l) in facets.iter().enumerate() {
            // A number line: its Min – Max boxes (WikiView.tsx 166-171).
            if l.number {
                let Some((_, lo, hi)) = w.ranges.iter().find(|(f, _, _)| *f == l.field) else { continue };
                let range = div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(field(lo, None).w(px(110.)).h(px(32.)))
                    .child(div().text_color(c(MUTED)).child("–"))
                    .child(field(hi, None).w(px(110.)).h(px(32.)));
                panel = panel.child(div().flex().items_center().gap(px(14.)).child(div().w(px(140.)).flex_none().text_size(px(T_S)).text_color(c(MUTED)).child(word(root, &l.field))).child(range));
                continue;
            }
            let on_now: Vec<String> = w.filters.iter().find(|(f, _)| *f == l.field).map(|(_, v)| v.clone()).unwrap_or_default();
            let mut vals = div().flex().flex_wrap().gap(px(6.));
            for (vi, v) in l.values.iter().enumerate() {
                let on = on_now.contains(v);
                let (field, value) = (l.field.clone(), v.clone());
                vals = vals.child(chip(("wk-fv", fi * 100 + vi), on).child(v.clone()).on_click(cx.listener(move |r, _, _, cx| {
                    let slot = match r.wiki.filters.iter_mut().find(|(f, _)| *f == field) {
                        Some(s) => s,
                        None => {
                            r.wiki.filters.push((field.clone(), Vec::new()));
                            r.wiki.filters.last_mut().expect("just pushed")
                        }
                    };
                    if let Some(at) = slot.1.iter().position(|x| *x == value) {
                        slot.1.remove(at);
                    } else {
                        slot.1.push(value.clone());
                    }
                    cx.notify();
                })));
            }
            panel = panel.child(div().flex().gap(px(14.)).child(div().w(px(140.)).flex_none().pt(px(4.)).text_size(px(T_S)).text_color(c(MUTED)).child(word(root, &l.field))).child(vals));
        }
        body = body.child(panel);
    }

    // Picked to compare: a tray, and the comparison when asked.
    let picked: Vec<&Item> = w.picked.iter().filter_map(|id| items.iter().find(|i| i.id == *id)).collect();
    if !picked.is_empty() {
        let mut tray = boxed().px(px(12.)).py(px(8.)).flex().flex_wrap().items_center().gap(px(8.));
        for (i, it) in picked.iter().enumerate() {
            let id = it.id.clone();
            tray = tray.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .pl(px(4.))
                    .pr(px(2.))
                    .py(px(2.))
                    .rounded(px(R_S))
                    .bg(c(RAISED_2))
                    .text_size(px(T_M))
                    .child(pic_box(it.img.as_ref(), &it.name, 22., 22., cx))
                    .child(it.name.clone())
                    .child(IconButton::new(("wk-unpick", i), "x").size(22.).tip(root.tr("wiki.compareRemove")).on_click(cx.listener(move |r, _, _, cx| {
                        r.wiki.picked.retain(|x| *x != id);
                        if r.wiki.picked.len() < 2 {
                            r.wiki.comparing = false;
                        }
                        cx.notify();
                    }))),
            );
        }
        tray = tray
            .child(div().flex_1())
            .child(Button::new("wk-pick-clear", root.tr("wiki.clear")).quiet().small().on_click(cx.listener(|r, _, _, cx| {
                r.wiki.picked.clear();
                r.wiki.comparing = false;
                cx.notify();
            })))
            .child(Button::new("wk-compare", root.i18n.tv("wiki.compareN", &[("n", &picked.len().to_string())])).icon("grid").primary().small().disabled(picked.len() < 2).on_click(cx.listener(|r, _, _, cx| {
                r.wiki.comparing = !r.wiki.comparing;
                cx.notify();
            })));
        body = body.child(tray);
        if w.comparing && picked.len() >= 2 {
            body = body.child(compare(root, &picked, cx));
        }
    }

    // The table: pick, name, the kind's lines.
    let name_w = 260.;
    let col_w = ((width - 40. - name_w) / (columns.len().max(1) as f32 + 1.)).clamp(96., 240.);
    let k = ws();
    let cell_line = k.line;
    let mut head = div().flex().items_stretch().min_h(px(34.)).bg(k.head).text_size(px(13.5)).text_color(k.text).font_weight(FontWeight::BOLD).border_b_1().border_color(k.line);
    head = head.child(div().w(px(34.)).flex_none()).child(div().flex_1().min_w(px(name_w)).px(px(10.)).flex().items_center().border_l_1().border_color(cell_line).child(root.tr("wiki.name")));
    for (ci, l) in columns.iter().enumerate() {
        let on = w.sort.as_ref().filter(|(f, _)| *f == l.field).map(|(_, d)| *d);
        let (field, number) = (l.field.clone(), l.number);
        // (Its tip as WikiView.tsx's wk-sort title.)
        let tip = SharedString::from(root.i18n.tv("wiki.sortBy", &[("f", &word(root, &l.field))]));
        head = head.child(
            focusable(div().id(("wk-sort", ci)))
                .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
                .w(px(col_w))
                .flex_none()
                .px(px(10.))
                .flex()
                .items_center()
                .gap(px(4.))
                .border_l_1()
                .border_color(cell_line)
                .cursor_pointer()
                .hover(|st| st.underline())
                .child(div().truncate().child(word(root, &l.field)))
                .child(icon(match on { Some(true) => "chevronDown", Some(false) => "chevronUp", None => "sort" }, 12., if on.is_some() { k.link } else { Hsla { a: 0.55, ..k.text } }))
                .on_click(cx.listener(move |r, _, _, cx| {
                    // Numbers start biggest first; a second click turns it round, a third lets go.
                    r.wiki.sort = match r.wiki.sort.take() {
                        Some((f, d)) if f == field => (d == number).then_some((f, !d)),
                        _ => Some((field.clone(), number)),
                    };
                    cx.notify();
                })),
        );
    }
    let mut rows = div().flex().flex_col();
    for (ri, it) in shown.iter().take(ROWS_SHOWN).enumerate() {
        let picked_on = w.picked.contains(&it.id);
        let full = !picked_on && w.picked.len() >= COMPARE_MAX;
        let (id, open_title) = (it.id.clone(), wiki_title_of(&it.url, &root.wiki.site).unwrap_or_else(|| it.name.clone()));
        let tpl = template.to_string();
        let pick_tip = SharedString::from(root.tr("wiki.compareAdd"));
        let mut row = div()
            .id(("wk-row", ri))
            .flex()
            .items_stretch()
            .min_h(px(42.))
            .text_size(px(13.5))
            .border_b_1()
            .border_color(k.line)
            .when(picked_on, |d| d.bg(Hsla { a: 0.12, ..k.own }))
            .when(!picked_on, |d| d.hover(|st| st.bg(Hsla { a: 0.05, ..k.text })))
            .child(
                // (Its tip as WikiView.tsx's wk-check title; a full comparison takes no more, as its disabled box.)
                div().id(("wk-pick-tip", ri)).w(px(34.)).flex_none().flex().items_center().justify_center().when(full, |d| d.opacity(0.4)).tooltip(move |_, cx| cx.new(|_| Tip(pick_tip.clone())).into()).child(Check::new(("wk-pick", ri), picked_on).on_toggle({
                    let this = cx.entity().downgrade();
                    move |_, _, cx| {
                        if full {
                            return;
                        }
                        let id = id.clone();
                        let _ = this.update(cx, |r, cx| {
                            if let Some(at) = r.wiki.picked.iter().position(|x| *x == id) {
                                r.wiki.picked.remove(at);
                            } else if r.wiki.picked.len() < COMPARE_MAX {
                                r.wiki.picked.push(id);
                            }
                            cx.notify();
                        });
                    }
                })),
            )
            .child(
                focusable(div().id(("wk-item", ri)))
                    .flex_1()
                    .min_w(px(name_w))
                    .px(px(10.))
                    .py(px(5.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .border_l_1()
                    .border_color(cell_line)
                    .cursor_pointer()
                    .text_color(k.link)
                    .hover(|st| st.underline())
                    .child(pic_box(it.img.as_ref(), &it.name, 30., 30., cx))
                    .child(div().min_w_0().truncate().child(it.name.clone()))
                    .on_click(cx.listener(move |r, _, _, cx| {
                        go(r, View::Page { title: open_title.clone(), table: Some(tpl.clone()) });
                        cx.notify();
                    })),
            );
        for l in &columns {
            let v = value_of(it, &l.field).map(|v| v.lines().next().unwrap_or_default().to_string()).unwrap_or_else(|| "–".into());
            row = row.child(div().w(px(col_w)).flex_none().px(px(10.)).flex().items_center().border_l_1().border_color(cell_line).text_color(k.text).when(l.number, |d| d.font_features(tnum())).child(div().min_w_0().truncate().child(v)));
        }
        rows = rows.child(row);
    }
    let more = shown.len().saturating_sub(ROWS_SHOWN);
    body = body.child(
        // (.ws-table: square, on the box's ground, a line all round.)
        div()
            .bg(k.boxed)
            .border_1()
            .border_color(k.line)
            .overflow_hidden()
            .child(head)
            .child(rows)
            .when(shown.is_empty(), |d| d.child(note(root.tr("wiki.nothing")).px(px(14.))))
            .when(more > 0, |d| d.child(div().px(px(14.)).py(px(10.)).text_size(px(T_S)).text_color(c(MUTED)).child(root.i18n.tv("wiki.more", &[("n", &more.to_string())])))),
    );
    body.into_any_element()
}

/// A table item's page title, from its address on the wiki.
fn wiki_title_of(url: &str, site: &str) -> Option<String> {
    match wikitext::link_of(url, site) {
        Some(wikitext::Link::Page { title, .. }) => Some(title),
        _ => None,
    }
}

/// Items side by side: a column each, the better value of each line marked, how many lines each wins on top. Lines
/// where one is better come first, then the ones all of them have; lines only some have last, quieter.
fn compare(root: &Root, items: &[&Item], cx: &mut Context<Root>) -> Div {
    let mut fields: Vec<String> = Vec::new();
    for i in items {
        for (k, _) in &i.fields {
            if !fields.contains(k) {
                fields.push(k.clone());
            }
        }
    }
    let best = |f: &str| -> Option<f64> {
        let have: Vec<f64> = items.iter().filter_map(|i| rank_of(f, value_of(i, f))).collect();
        let mut uniq = have.clone();
        uniq.sort_by(f64::total_cmp);
        uniq.dedup();
        if have.len() < 2 || uniq.len() < 2 {
            return None;
        }
        Some(if less_is_better(f) { uniq[0] } else { uniq[uniq.len() - 1] })
    };
    let all = |f: &str| items.iter().all(|i| value_of(i, f).is_some());
    let ranked: Vec<&String> = fields.iter().filter(|f| best(f).is_some()).collect();
    let shared: Vec<&String> = fields.iter().filter(|f| best(f).is_none() && all(f)).collect();
    let some: Vec<&String> = fields.iter().filter(|f| best(f).is_none() && !all(f)).collect();
    let wins: Vec<usize> = items.iter().map(|i| ranked.iter().filter(|f| rank_of(f, value_of(i, f)) == best(f)).count()).collect();
    let top = wins.iter().copied().max().unwrap_or(0);
    let label_w = 170.;
    let mut head = div().flex().gap(px(10.)).pb(px(10.)).border_b_1().border_color(ws().line).child(div().w(px(label_w)).flex_none().text_size(px(T_H)).font_weight(FontWeight::SEMIBOLD).child(root.tr("wiki.compare")));
    for (n, i) in items.iter().enumerate() {
        let lead = !ranked.is_empty() && wins[n] == top;
        head = head.child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(6.))
                .child(pic_box(i.img.as_ref(), &i.name, 64., 64., cx))
                .child(div().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).truncate().max_w_full().child(i.name.clone()))
                .when(!ranked.is_empty(), |d| {
                    let words = format!("{}/{} {}", wins[n], ranked.len(), root.tr("wiki.winsOf"));
                    d.child(if lead {
                        div().h(px(22.)).px(px(8.)).flex().items_center().rounded(px(7.)).bg(c(FACE)).text_size(px(T_S)).font_weight(FontWeight::SEMIBOLD).text_color(c(INK)).child(words)
                    } else {
                        div().h(px(22.)).flex().items_center().text_size(px(T_S)).text_color(c(MUTED)).child(words)
                    })
                }),
        );
    }
    let row = |f: &String, quiet: bool| {
        let b = best(f);
        let mut r = div()
            .flex()
            .items_center()
            .gap(px(10.))
            .py(px(7.))
            .border_b_1()
            .border_color(ws().line)
            .text_size(px(T_M))
            .when(quiet, |d| d.opacity(0.7))
            .child(
                div()
                    .w(px(label_w))
                    .flex_none()
                    .flex()
                    .flex_col()
                    .child(div().text_color(c(MUTED)).child(word(root, f)))
                    .when(b.is_some() && less_is_better(f), |d| d.child(div().text_size(px(T_XS)).text_color(c(DIM)).child(root.tr("wiki.lessBetter")))),
            );
        for i in items {
            let v = value_of(i, f);
            let win = b.is_some() && rank_of(f, v) == b;
            r = r.child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .justify_center()
                    .items_center()
                    .gap(px(5.))
                    .text_color(if win { c(OK) } else if b.is_some() { c(TEXT_2) } else { c(TEXT) })
                    .when(win, |d| d.font_weight(FontWeight::SEMIBOLD).child(icon("check", 13., c(OK))))
                    .child(div().text_center().child(v.map(|v| v.replace('\n', " · ")).unwrap_or_else(|| "–".into()))),
            );
        }
        r
    };
    let mut panel = boxed().p(px(16.)).flex().flex_col().child(head);
    for f in ranked.iter().chain(&shared) {
        panel = panel.child(row(f, false));
    }
    if !some.is_empty() {
        panel = panel.child(div().pt(px(10.)).pb(px(4.)).text_size(px(T_S)).text_color(c(MUTED)).child(format!("{} · {}", root.tr("wiki.onlySome"), some.len())));
        for f in &some {
            panel = panel.child(row(f, true));
        }
    }
    panel
}

fn article_view(root: &Root, about: &About, title: &str, table: Option<&str>, width: f32, cx: &mut Context<Root>) -> AnyElement {
    // The article's own words are drawn by the kit's text view, which takes its link and table-head colours from the
    // kit's theme: set to the sheet's here (.ws-content a: the link colour; a wikitable's head on --ws-head). Nothing
    // else in the app uses that view.
    {
        let k = ws();
        gpui_kit::component::Theme::update(cx, |t| {
            let col = &mut t.colors;
            (col.link, col.link_hover, col.link_active) = (k.link, k.link, k.link);
            (col.table_head, col.table_head_foreground) = (k.head, k.text);
        });
    }
    let w = &root.wiki;
    let site = w.site.clone();
    let page = w.pages.get(&key(&site, title));
    let shown_title = match page {
        Some(Some(a)) => a.title.clone(),
        _ => title.to_string(),
    };
    let saved = marks(root, "saved").contains(&shown_title);
    let can_back = w.views.len() > 1;
    let mut top = div().flex().flex_col().gap(px(6.));
    if can_back {
        top = top.child(div().flex().child(Button::new("wk-back", root.tr("wiki.back")).icon("chevronLeft").quiet().small().on_click(cx.listener(|r, _, _, cx| {
            back(r);
            cx.notify();
        }))));
    }
    let heading = match shown_title.strip_prefix("Category:") {
        Some(rest) => format!("{}: {rest}", root.tr("wiki.cats")),
        None => shown_title.clone(),
    };
    let open_url = wikitext::page_url(&site, &shown_title);
    let mut tag = div().flex().flex_wrap().items_center().gap(px(8.)).text_size(px(T_M)).text_color(c(MUTED)).child(root.i18n.tv("wiki.from", &[("site", &about.name)]));
    if matches!(page, Some(Some(a)) if !a.missing) {
        let t = shown_title.clone();
        tag = tag.child("·").child(
            focusable(div().id("wk-save"))
                .flex()
                .items_center()
                .gap(px(5.))
                .cursor_pointer()
                .text_color(c(if saved { FACE } else { TEXT_2 }))
                .hover(|st| st.text_color(c(FACE_HI)))
                .child(icon(if saved { "starFill" } else { "star" }, 13., c(if saved { FACE } else { MUTED })))
                .child(root.tr(if saved { "wiki.savedOne" } else { "wiki.save" }))
                .on_click(cx.listener(move |r, _, _, _| {
                    let (site, game) = (r.wiki.site.clone(), r.wiki.game.clone());
                    r.send("wikiMark", json!({ "list": "saved", "site": site, "title": t, "game": game, "on": !saved }));
                })),
        );
    }
    tag = tag.child("·").child(link_text("wk-open-page", root.tr("wiki.openPage")).on_click(cx.listener(move |r, _, _, _| r.send("wikiOpen", json!({ "url": open_url })))));
    top = top.child(h1(heading)).child(tag);
    let mut body = div().flex().flex_col().gap(px(14.)).child(top);

    let article = match page {
        None => return body.child(note(root.tr("news.connecting"))).into_any_element(),
        Some(None) => return body.child(note(root.tr("wiki.offline"))).into_any_element(),
        Some(Some(a)) if a.missing => return body.child(note(root.tr("wiki.noPage"))).into_any_element(),
        Some(Some(a)) => a.clone(),
    };

    // The item as a table has it (when opened from one), else as its box shows it: its facts for the drop odds.
    let items: Rc<Vec<Item>> = table.and_then(|t| w.tables.get(&key(&site, t))).map(|t| t.items.clone()).unwrap_or_default();
    // (Looked for once for the page and the tables as they are, not on every frame.)
    let prefix = format!("{site}|");
    let mut revs: Vec<(&String, u64)> = w.tables.iter().filter(|(k, _)| k.starts_with(&prefix)).map(|(k, t)| (k, t.rev)).collect();
    revs.sort();
    // (No table: the empty list is a new one each frame, its address no key; 0 then.)
    let items_at = if items.is_empty() { 0 } else { Rc::as_ptr(&items) as usize };
    let found_key = hash_of((&site, title, table, Rc::as_ptr(&article) as usize, items_at, revs));
    let kept = w.found.borrow().as_ref().filter(|f| f.key == found_key).map(|f| (f.item, f.facts.clone(), f.sim.clone()));
    let (item_at, facts, sim) = kept.unwrap_or_else(|| {
        let (want_a, want_b) = (fold(title.rsplit('/').next().unwrap_or(title)), fold(&article.title));
        let named = |i: &Item| {
            let n = fold(&i.name);
            n == want_a || n == want_b
        };
        let item_at = items.iter().position(|i| named(i));
        let facts: Vec<(String, String)> = item_at
            .map(|k| items[k].fields.clone())
            .or_else(|| w.tables.iter().filter(|(k, _)| k.starts_with(&prefix)).find_map(|(_, t)| t.items.iter().find(|i| named(i)).map(|i| i.fields.clone())))
            .or_else(|| article.infobox.as_ref().map(|b| b.rows.clone()))
            .unwrap_or_default();
        let sim = match item_at {
            Some(k) => {
                let (lines, near) = wikitext::similar_to(&items[k], &items);
                let near = near.into_iter().filter_map(|p| items.iter().position(|x| std::ptr::eq(x, p))).collect();
                (lines, near)
            }
            None => (Vec::new(), Vec::new()),
        };
        let (facts, sim) = (Rc::new(facts), Rc::new(sim));
        *w.found.borrow_mut() = Some(ArtFound { key: found_key, item: item_at, facts: facts.clone(), sim: sim.clone() });
        (item_at, facts, sim)
    });
    if let Some(p) = wikitext::drop_chance(&facts) {
        body = body.child(odds(root, p, cx));
    }

    // The words, drawn by GPUI's text view; pictures handed to it as they load.
    if let Ok(mut loaded) = LOADED.lock() {
        // Only this article's: one article shows at a time, and the ones read before don't stay in memory.
        let wanted: std::collections::HashSet<&String> = article.images.iter().take(80).collect();
        loaded.retain(|k, _| wanted.contains(k));
        for u in article.images.iter().take(80) {
            if !loaded.contains_key(u)
                && let Some(p) = images::pic(u, cx)
            {
                loaded.insert(u.clone(), p);
            }
        }
    }
    let words = |id: String, html: SharedString| {
        TextView::html(SharedString::from(id), html)
            .selectable(true)
            .image_source(|uri| {
                let u = uri.to_string();
                ImageSource::Image(LOADED.lock().ok().and_then(|m| m.get(&u).cloned()).unwrap_or_else(|| BLANK.clone()))
            })
            .on_link_click(|href, _, _, cx| {
                if let Ok(mut c) = CLICKED.lock() {
                    *c = Some(href.to_string());
                }
                cx.refresh_windows();
            })
    };
    let art_id = format!("wk-art-{}", key(&site, &article.title));
    if let Ok(mut tops) = PART_TOPS.lock() {
        tops.clear();
    }
    let mut text = div().min_w_0().flex_1().text_size(px(T_L)).text_color(c(TEXT)).line_height(px(22.));
    // Contents with 3 or more parts (WikiSite.tsx's loadPage, wikiText.ts's ws-toc): after the first words, before the
    // first heading. GPUI's text view can't scroll its HTML to a heading, so the words are drawn a part at a time
    // (cut where each heading starts, at the top of the page's words) and each part's place on the window is noted
    // as it is drawn: a line of the Contents scrolls the page there.
    let parts = &article.sections;
    let cuts_ok = parts.len() >= 3 && parts.windows(2).all(|p| p[0].at < p[1].at) && parts.last().is_some_and(|p| p.at <= article.html.len()) && parts.iter().all(|p| article.html.is_char_boundary(p.at));
    if cuts_ok {
        let lead = &article.html[..parts[0].at];
        if !lead.trim().is_empty() {
            text = text.child(words(format!("{art_id}-lead"), SharedString::from(lead.to_string())));
        }
        text = text.child(contents(root, parts, cx));
        for (k, p) in parts.iter().enumerate() {
            let until = parts.get(k + 1).map(|n| n.at).unwrap_or(article.html.len());
            let chunk = SharedString::from(article.html[p.at..until].to_string());
            text = text.child(
                div()
                    .relative()
                    .child(canvas(move |b, _, _| {
                        if let Ok(mut tops) = PART_TOPS.lock() {
                            tops.insert(k, b.origin.y);
                        }
                    }, |_, _, _, _| {}).absolute().inset_0())
                    .child(words(format!("{art_id}-{k}"), chunk)),
            );
        }
    } else {
        text = text.child(words(art_id, article.html.clone()));
    }
    let side = article.infobox.as_ref().map(|b| infobox_card(root, b, cx));
    if width >= 980. {
        body = body.child(div().flex().items_start().gap(px(24.)).child(text).children(side.map(|s| s.w(px(SIDE)).flex_none())));
    } else {
        body = body.children(side).child(text);
    }
    if item_at.is_some() {
        body = body.child(similar(root, &sim.0, &sim.1, &items, table.unwrap_or_default(), cx));
    }
    if !article.cats.is_empty() {
        let mut cats = div().flex().flex_wrap().items_center().gap(px(8.)).text_size(px(T_S)).child(div().text_color(c(MUTED)).font_weight(FontWeight::SEMIBOLD).child(format!("{}:", root.tr("wiki.cats"))));
        for (i, ct) in article.cats.iter().enumerate() {
            let t = format!("Category:{ct}");
            cats = cats.child(chip(("wk-cat", i), false).child(ct.clone()).on_click(cx.listener(move |r, _, _, cx| {
                go(r, View::Page { title: t.clone(), table: None });
                cx.notify();
            })));
        }
        body = body.child(div().pt(px(8.)).border_t_1().border_color(ws().line).child(cats));
    }
    body.into_any_element()
}

/// An article's Contents (wikiText.ts's ws-toc): its parts numbered as the wiki numbers them (1, 1.1), the second
/// level under its first; a line scrolls the page to its part.
fn contents(root: &Root, parts: &[wikihtml::Section], cx: &mut Context<Root>) -> Div {
    let mut list = div().flex().flex_col().gap(px(2.));
    let (mut one, mut two) = (0, 0);
    for (k, p) in parts.iter().enumerate() {
        let number = if p.level == 1 {
            one += 1;
            two = 0;
            one.to_string()
        } else if one > 0 {
            two += 1;
            format!("{one}.{two}")
        } else {
            continue;
        };
        list = list.child(
            focusable(div().id(("wk-toc", k)))
                .flex()
                .gap(px(8.))
                .py(px(2.))
                .when(p.level == 2, |d| d.pl(px(18.)))
                .cursor_pointer()
                .text_color(c(TEXT_2))
                .hover(|st| st.text_color(c(TEXT)).underline())
                .child(div().flex_none().text_color(c(MUTED)).font_features(tnum()).child(number))
                .child(div().min_w_0().child(p.line.clone()))
                .on_click(cx.listener(move |r, _, _, cx| {
                    jump(r, k);
                    cx.notify();
                })),
        );
    }
    div().flex().my(px(10.)).child(
        boxed()
            .max_w(px(440.))
            .px(px(14.))
            .py(px(10.))
            .flex()
            .flex_col()
            .gap(px(6.))
            .text_size(px(T_M))
            .line_height(px(20.))
            .child(div().font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(root.tr("wiki.toc")))
            .child(list),
    )
}

/// The page's box of facts, as a wiki's infobox (.ws-infobox): square, a line all round; its title on the game's own
/// colour in serif; its picture on the sheet's ground; each line its name on the head's ground (two fifths of the
/// width, bold) and its value, a line between the lines.
fn infobox_card(root: &Root, b: &Infobox, cx: &mut Context<Root>) -> Div {
    let k = ws();
    let p = b.image.as_ref().and_then(|u| images::pic(u, cx));
    let mut card = div().bg(k.boxed).border_1().border_color(k.line).flex().flex_col().text_size(px(T_M));
    if let Some(t) = &b.title {
        // (--gt-ink: dark words on a light game colour, light on a dark one.)
        let ink: Hsla = if crate::theme::lightness(k.own) > 58. { rgb(0x15171c).into() } else { rgb(0xffffff).into() };
        card = card.child(div().px(px(10.)).py(px(7.)).bg(k.own).text_color(ink).font_family(SERIF).text_size(px(16.)).line_height(px(21.)).font_weight(FontWeight::BOLD).text_center().child(t.clone()));
    }
    if let Some(p) = p {
        card = card.child(div().h(px(204.)).p(px(12.)).bg(k.bg).border_b_1().border_color(k.line).flex().items_center().justify_center().child(img(p).size_full().object_fit(ObjectFit::Contain)));
    }
    let mut rows = div().flex().flex_col();
    for (i, (name, v)) in b.rows.iter().enumerate() {
        rows = rows.child(
            div()
                .flex()
                .items_stretch()
                .when(i > 0, |d| d.border_t_1().border_color(k.line))
                .child(div().w(relative(0.4)).flex_none().px(px(10.)).py(px(5.)).bg(k.head).font_weight(FontWeight::BOLD).child(word(root, name)))
                .child(div().flex_1().min_w_0().px(px(10.)).py(px(5.)).text_color(k.text).child(v.replace('\n', " · "))),
        );
    }
    card.child(rows)
}

/// The items most like one, as a table: each differing line with + or −, green where it's better.
fn similar(root: &Root, lines: &[wikitext::SimLine], near: &[usize], items: &[Item], table: &str, cx: &mut Context<Root>) -> Div {
    let near: Vec<&Item> = near.iter().filter_map(|k| items.get(*k)).collect();
    if near.is_empty() {
        return div();
    }
    let site = root.wiki.site.clone();
    let mut list = boxed().overflow_hidden().flex().flex_col();
    let mut head = div().flex().items_center().h(px(34.)).px(px(12.)).bg(ws().head).text_size(px(T_S)).text_color(c(MUTED)).font_weight(FontWeight::SEMIBOLD).border_b_1().border_color(ws().line).child(div().w(px(240.)).flex_none().child(root.tr("wiki.name")));
    for l in lines {
        head = head.child(div().flex_1().child(word(root, &l.field)));
    }
    list = list.child(head);
    for (i, p) in near.iter().enumerate() {
        let title = wiki_title_of(&p.url, &site).unwrap_or_else(|| p.name.clone());
        let tpl = table.to_string();
        let mut row = div().flex().items_center().min_h(px(42.)).px(px(12.)).text_size(px(T_M)).border_b_1().border_color(ws().line).child(
            focusable(div().id(("wk-sim", i)))
                .w(px(240.))
                .flex_none()
                .flex()
                .items_center()
                .gap(px(10.))
                .cursor_pointer()
                .hover(|st| st.text_color(c(FACE)))
                .child(pic_box(p.img.as_ref(), &p.name, 28., 28., cx))
                .child(div().truncate().child(p.name.clone()))
                .on_click(cx.listener(move |r, _, _, cx| {
                    go(r, View::Page { title: title.clone(), table: Some(tpl.clone()) });
                    cx.notify();
                })),
        );
        for l in lines {
            let v = value_of(p, &l.field);
            let d = number_of(v).map(|n| ((n - l.at) * 100.).round() / 100.);
            let better = d.is_some_and(|d| d != 0. && if less_is_better(&l.field) { d < 0. } else { d > 0. });
            row = row.child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .font_features(tnum())
                    .child(v.map(|v| v.lines().next().unwrap_or_default().to_string()).unwrap_or_else(|| "–".into()))
                    .when_some(d.filter(|d| *d != 0.), |el, d| {
                        el.child(div().text_size(px(T_S)).text_color(c(if better { OK } else { BAD })).child(format!("({}{})", if d > 0. { "+" } else { "−" }, d.abs())))
                    }),
            );
        }
        list = list.child(row);
    }
    div().flex().flex_col().gap(px(10.)).child(h2(root.tr("wiki.similar"), None)).child(list)
}

/// "How long to farm it": for an item with a drop chance, the tries it takes for half, nine in ten and 99 of 100
/// players to have it, on a track, and how lucky (or not) you are after the tries you made, typed in its box, your
/// tries filling the track (WikiView.tsx's DropOdds). The wiki says only "1%".
fn odds(root: &Root, p: f64, cx: &mut Context<Root>) -> Div {
    let marks: Vec<(f64, i64)> = [0.5, 0.9, 0.99].iter().map(|c| (*c, wikitext::tries_for(p, *c))).collect();
    let end = marks[2].1.max(1) as f32;
    let pct = format!("{}%", trim_num(p * 100.));
    let each = root.i18n.tv("wiki.oddsEach", &[("p", &pct), ("n", &n_text(root, (1. / p).round() as i64))]);
    let tried_box = root.wiki.tried.as_ref().map(|(_, e)| e.clone());
    // Its digits only, at most 8 of them (the old box's maxLength), as at most ten million tries.
    let n: i64 = tried_box
        .as_ref()
        .map(|e| e.read(cx).value().chars().filter(char::is_ascii_digit).take(8).collect::<String>().parse::<i64>().unwrap_or(0).min(10_000_000))
        .unwrap_or(0);
    let had = (n > 0).then(|| 1. - (1. - p).powf(n as f64));
    let share = |x: f64| if x < 0.995 { format!("{:.0}%", x * 100.) } else { format!("{}%", trim_num((x * 1000.).round() / 10.)) };
    let mut track = div().relative().h(px(56.)).mt(px(6.)).child(div().absolute().top(px(10.)).left_0().right_0().h(px(4.)).rounded_full().bg(c(RAISED_2)));
    if had.is_some() {
        track = track.child(div().absolute().top(px(10.)).left_0().h(px(4.)).rounded_full().bg(ca(FACE, 0.6)).w(relative((n as f32 / end).min(1.))));
    }
    for (i, (ch, n)) in marks.iter().enumerate() {
        let at = *n as f32 / end;
        let label = root.i18n.tv("wiki.oddsFor", &[("c", &format!("{}%", trim_num(ch * 100.)))]);
        track = track.child(
            div()
                .absolute()
                .top_0()
                .left(relative(at))
                .ml(px(if i == 2 { -110. } else { -6. }))
                .flex()
                .flex_col()
                .when(i == 2, |d| d.items_end())
                .gap(px(4.))
                .w(px(116.))
                .child(div().when(i == 2, |d| d.mr(px(0.))).size(px(12.)).rounded_full().bg(c(FACE)).border_2().border_color(c(PANEL)).mt(px(6.)))
                .child(div().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).font_features(tnum()).child(n_text(root, *n)))
                .child(div().text_size(px(T_S)).text_color(c(MUTED)).child(label)),
        );
    }
    boxed()
        .p(px(16.))
        .pr(px(24.))
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(div().flex().flex_wrap().items_center().gap(px(10.)).child(div().text_size(px(T_H)).font_weight(FontWeight::SEMIBOLD).child(root.tr("wiki.odds"))).child(div().text_size(px(T_M)).text_color(c(MUTED)).child(each)))
        .child(track)
        .when_some(tried_box, |d, e| {
            d.child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(12.))
                    .text_size(px(T_M))
                    .text_color(c(MUTED))
                    .mt(px(4.))
                    .child(root.tr("wiki.oddsTried"))
                    .child(field(&e, None).w(px(110.)).h(px(32.)))
                    .when_some(had, |d, had| {
                        // Nine in ten would have it by now: unlucky, said in the warning colour.
                        let words = root.i18n.tv(if had >= 0.5 { "wiki.oddsUnlucky" } else { "wiki.oddsNormal" }, &[("c", &share(had))]);
                        d.child(div().text_color(if had >= 0.9 { c(WARN) } else { c(TEXT) }).when(had >= 0.9, |d| d.font_weight(FontWeight::SEMIBOLD)).child(words))
                    }),
            )
        })
}

fn trim_num(x: f64) -> String {
    let s = format!("{:.3}", x);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}
