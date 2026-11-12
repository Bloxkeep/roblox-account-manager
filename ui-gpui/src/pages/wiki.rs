// A game's whole wiki inside its page (the Wiki tab of games.rs), read from this PC through the app's wikiApi (any wiki
// on Fandom or wiki.gg, named by Bloxkeep's bot): the old web UI's Wiki part (ui/src/WikiSite.tsx, WikiView.tsx) in
// GPUI. Its home (what is new on the wiki and which numbers changed lately, the kinds of thing it keeps boxes of facts
// for, pages seen and saved), its search as typed, every article drawn here with its box of facts as a card and its
// links opening here, and each kind as a table to search, filter, sort and compare, with the items most like one and
// the odds of a drop. Text and pictures are the wiki's (CC BY-SA): every page names and links it.
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, LazyLock, Mutex},
    time::Duration,
};

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
    html: String,
    infobox: Option<Infobox>,
    images: Vec<String>,
    cats: Vec<String>,
    missing: bool,
}

#[derive(Clone, Default)]
struct Table {
    items: Vec<Item>,
    done: bool,
    failed: bool,
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
    /// Pages by "site|title" (None: it couldn't be read).
    pages: HashMap<String, Option<Article>>,
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
}

/// A link clicked in an article (the text view hands it back from outside the window's reach).
static CLICKED: Mutex<Option<String>> = Mutex::new(None);
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

async fn api(link: &Link, site: &str, q: Value) -> Option<Value> {
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
                    let _ = this.update(cx, |r, cx| {
                        r.wiki.results.insert(k, hits);
                        cx.notify();
                    });
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
        let _ = this.update(cx, |r, cx| {
            r.wiki.kinds.insert(site, kinds);
            cx.notify();
        });
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
                Article { title: title.clone(), html: if list.is_empty() { String::new() } else { html }, infobox: None, images: Vec::new(), cats: Vec::new(), missing: list.is_empty() }
            })
        } else {
            let d = link
                .call("wikiApi", json!({ "site": site, "q": { "action": "parse", "page": title, "prop": "text|categories|displaytitle", "redirects": 1, "disableeditsection": 1, "disablelimitreport": 1 } }))
                .await
                .ok()
                .and_then(Result::ok);
            match d {
                Some(d) if d.get("error").is_some() => Some(Article { title: title.clone(), html: String::new(), infobox: None, images: Vec::new(), cats: Vec::new(), missing: true }),
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
                    Some(Article { title: Some(s(p, "title")).filter(|t| !t.is_empty()).unwrap_or(title.clone()), html: clean.html, infobox: clean.infobox, images: clean.images, cats, missing: false })
                }),
                None => None,
            }
        };
        let _ = this.update(cx, |r, cx| {
            if r.wiki.pages.len() > 40
                && let Some(old) = r.wiki.pages.keys().find(|x| **x != k).cloned()
            {
                r.wiki.pages.remove(&old);
                r.wiki.asked_pages.remove(&old);
            }
            r.wiki.pages.insert(k, page);
            cx.notify();
        });
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
                let _ = this.update(cx, |r, cx| {
                    r.wiki.tables.entry(k.clone()).or_default().failed = true;
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
                    r.wiki.tables.insert(k2, Table { items: now, done, failed: false });
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
    let Some(href) = CLICKED.lock().ok().and_then(|mut c| c.take()) else { return };
    if let Some(title) = href.strip_prefix(wikihtml::PAGE) {
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

// ---- Drawing ----

fn note(text: String) -> Div {
    div().py(px(28.)).text_size(px(T_M)).text_color(c(MUTED)).child(text)
}

fn h1(text: impl Into<SharedString>) -> Div {
    div().text_size(px(26.)).font_weight(FontWeight::BOLD).line_height(px(34.)).child(text.into())
}

fn h2(text: String, sub: Option<String>) -> Div {
    div()
        .pt(px(10.))
        .flex()
        .flex_col()
        .gap(px(3.))
        .child(div().text_size(px(17.)).font_weight(FontWeight::SEMIBOLD).child(text))
        .when_some(sub, |d, s| d.child(div().text_size(px(T_S)).text_color(c(MUTED)).child(s)))
}

fn link_text(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Stateful<Div> {
    // (Not yellow: yellow is what is chosen and the one main button. A link is the words, underlined.)
    focusable(div().id(id)).cursor_pointer().text_color(c(TEXT_2)).underline().hover(|st| st.text_color(c(TEXT))).child(text.into())
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
            Some(p) => d.child(div().h(px(32.)).max_w(px(140.)).child(img(p).h(px(32.)).object_fit(ObjectFit::Contain))),
            None => d.child(icon("book", 18., c(FACE))),
        })
        .child(div().text_size(px(T_H)).font_weight(FontWeight::SEMIBOLD).truncate().max_w(px(260.)).child(about.name.clone()))
        .on_click(cx.listener(|r, _, _, cx| {
            home(r);
            cx.notify();
        }));
    let q_now = root.wiki_q.read(cx).value().trim().to_string();
    let mut find = div().relative().flex_1().max_w(px(460.)).child(field(&root.wiki_q, Some("search")));
    if !w.hits.is_empty() && w.hits_for == q_now {
        let mut menu = div().w(px(440.)).p(px(6.)).rounded(px(R_L)).bg(c(RAISED_2)).border_1().border_color(ca(WHITE, 0.08)).shadow_lg().flex().flex_col();
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
    let top_bar = div().flex().items_center().gap(px(16.)).child(face).child(find).child(div().flex_1());

    // Its parts: home, its biggest kinds, saved.
    let mut nav = div().flex().flex_wrap().items_center().gap(px(4.));
    nav = nav.child(chip("wk-nav-home", now == View::Home).h(px(TOOL_H)).px(px(12.)).child(icon("home", 14., if now == View::Home { c(INK) } else { c(MUTED) })).child(root.tr("wiki.home")).on_click(cx.listener(|r, _, _, cx| {
        home(r);
        cx.notify();
    })));
    for (i, k) in kinds.iter().flatten().take(8).enumerate() {
        let t = k.template.clone();
        let on = now == View::Table(t.clone());
        nav = nav.child(chip(("wk-nav-kind", i), on).h(px(TOOL_H)).px(px(12.)).child(k.name.clone()).on_click(cx.listener(move |r, _, _, cx| {
            go(r, View::Table(t.clone()));
            cx.notify();
        })));
    }
    let saved_on = now == View::Saved;
    nav = nav.child(tool_rule()).child(
        chip("wk-nav-saved", saved_on)
            .h(px(TOOL_H))
            .px(px(12.))
            .child(icon("bookmark", 14., if saved_on { c(INK) } else { c(MUTED) }))
            .child(root.tr("wiki.saved"))
            .when(!saved.is_empty(), |d| d.child(chip_count(saved.len(), saved_on)))
            .on_click(cx.listener(|r, _, _, cx| {
        go(r, View::Saved);
        cx.notify();
    })),
    );

    let body: AnyElement = match &now {
        View::Home => home_view(root, &about, kinds.as_deref(), width, cx).into_any_element(),
        View::Saved => saved_view(root, &saved, width, cx).into_any_element(),
        View::Search(q) => search_view(root, q, width, cx).into_any_element(),
        View::Table(t) => table_view(root, t, width, cx),
        View::Page { title, table } => article_view(root, &about, title, table.as_deref(), width, cx),
    };
    div()
        .flex()
        .flex_col()
        .gap(px(16.))
        .pt(px(4.))
        .child(top_bar)
        .child(nav)
        .child(body)
        .child(credit(root, &about, cx))
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
            let (_, card_w) = grid_fit(width, 168., GRID_GAP);
            let mut row = grid(GRID_GAP);
            for (i, p) in f.added.iter().enumerate() {
                let title = p.title.clone();
                let last = p.title.rsplit('/').next().unwrap_or(&p.title);
                let shown = if last.chars().all(|c| c.is_ascii_digit() || c == '.') { p.title.replace('/', " ") } else { last.to_string() };
                row = row.child(
                    card(("wk-fresh", i))
                        .w(px(card_w))
                        .p(px(8.))
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .child(pic_box(p.pic.as_ref(), &p.title, card_w - 16., ((card_w - 16.) * 0.6).round(), cx))
                        .child(div().text_size(px(T_M)).font_weight(FontWeight::MEDIUM).line_clamp(2).child(shown))
                        .child(div().text_size(px(T_S)).text_color(c(MUTED)).child(format!("{} · {}", root.tr("wiki.freshNewOne"), crate::time::ago(crate::time::now_secs() - p.at, &root.i18n))))
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
                    card(("wk-changed", i))
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
                        .child(div().flex_1().h(px(4.)).rounded_full().bg(c(RAISED_2)).child(div().h_full().rounded_full().bg(ca(FACE, 0.55)).w(relative((p.edits as f32 / most).clamp(0.02, 1.)))))
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
                    card(("wk-kind", i))
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
        .child(div().size(px(36.)).flex_none().rounded(px(R_M)).bg(ca(WHITE, 0.06)).flex().items_center().justify_center().child(icon("book", 16., c(TEXT_2))))
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

fn table_view(root: &Root, template: &str, width: f32, cx: &mut Context<Root>) -> AnyElement {
    let w = &root.wiki;
    let name = w.kinds.get(&w.site).and_then(|ks| ks.iter().find(|k| k.template == template)).map(|k| k.name.clone()).unwrap_or_else(|| wikitext::kind_name(template));
    let table = w.tables.get(&key(&w.site, template)).cloned().unwrap_or_default();
    if table.items.is_empty() {
        let says = if table.failed { "wiki.offline" } else if table.done { "wiki.nothing" } else { "news.connecting" };
        return div().flex().flex_col().gap(px(8.)).child(h1(name)).child(note(root.tr(says))).into_any_element();
    }
    let items = &table.items;
    let lines = lines_of(items);
    let columns: Vec<&Line> = lines.iter().filter(|l| l.number || l.values.len() <= 30).take(COLUMNS).collect();
    let facets: Vec<&Line> = lines.iter().filter(|l| !l.number && l.values.len() >= 2 && l.values.len() <= 24).take(8).collect();

    // Found by words, filtered by values, sorted by a column.
    let words: Vec<String> = fold(&root.wiki_tq.read(cx).value()).split_whitespace().map(str::to_string).collect();
    let mut shown: Vec<&Item> = items
        .iter()
        .filter(|i| {
            if !words.is_empty() {
                let all = fold(&format!("{} {}", i.name, i.fields.iter().map(|f| f.1.as_str()).collect::<Vec<_>>().join(" ")));
                if !words.iter().all(|wd| all.contains(wd.as_str())) {
                    return false;
                }
            }
            w.filters.iter().all(|(f, vs)| vs.is_empty() || value_of(i, f).is_some_and(|v| vs.iter().any(|x| x == v)))
        })
        .collect();
    if let Some((field, down)) = &w.sort {
        let number = lines.iter().find(|l| l.field == *field).is_some_and(|l| l.number);
        shown.sort_by(|a, b| {
            let (va, vb) = (value_of(a, field), value_of(b, field));
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

    let loading = (!table.done).then(|| root.i18n.tv("wiki.loadingN", &[("n", &n_text(root, items.len() as i64))]));
    let filtered = w.filters.iter().filter(|(_, v)| !v.is_empty()).count();
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
            d.child(Button::new("wk-filter-clear", root.tr("wiki.clear")).quiet().small().on_click(cx.listener(|r, _, _, cx| {
                r.wiki.filters.clear();
                cx.notify();
            })))
        });
    let mut body = div()
        .flex()
        .flex_col()
        .gap(px(12.))
        .child(div().flex().flex_wrap().items_center().gap(px(16.)).child(div().flex().flex_col().gap(px(2.)).child(div().text_size(px(T_GROUP + 4.)).font_weight(FontWeight::BOLD).line_height(px(28.)).child(name)).child(tag)).child(div().flex_1()).child(tools));

    if w.filtering {
        let mut panel = surface().p(px(14.)).flex().flex_col().gap(px(10.));
        for (fi, l) in facets.iter().enumerate() {
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
        let mut tray = surface().px(px(12.)).py(px(8.)).flex().flex_wrap().items_center().gap(px(8.));
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
    let mut head = div().flex().items_center().h(px(36.)).px(px(10.)).text_size(px(T_S)).text_color(c(MUTED)).font_weight(FontWeight::SEMIBOLD).border_b_1().border_color(ca(WHITE, 0.08));
    head = head.child(div().w(px(30.)).flex_none()).child(div().flex_1().min_w(px(name_w)).child(root.tr("wiki.name")));
    for (ci, l) in columns.iter().enumerate() {
        let on = w.sort.as_ref().filter(|(f, _)| *f == l.field).map(|(_, d)| *d);
        let (field, number) = (l.field.clone(), l.number);
        head = head.child(
            focusable(div().id(("wk-sort", ci)))
                .w(px(col_w))
                .flex_none()
                .flex()
                .items_center()
                .gap(px(4.))
                .cursor_pointer()
                .when(on.is_some(), |d| d.text_color(c(TEXT)))
                .hover(|st| st.text_color(c(TEXT)))
                .child(div().truncate().child(word(root, &l.field)))
                .child(icon(match on { Some(true) => "chevronDown", Some(false) => "chevronUp", None => "sort" }, 12., c(MUTED)))
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
        let mut row = div()
            .flex()
            .items_center()
            .min_h(px(44.))
            .px(px(10.))
            .py(px(4.))
            .text_size(px(T_M))
            .border_b_1()
            .border_color(ca(WHITE, 0.04))
            .when(picked_on, |d| d.bg(tinted(PANEL, FACE, 0.08)))
            .child(
                div().w(px(30.)).flex_none().child(Check::new(("wk-pick", ri), picked_on).on_toggle({
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
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .cursor_pointer()
                    .hover(|st| st.underline())
                    .child(pic_box(it.img.as_ref(), &it.name, 32., 32., cx))
                    .child(div().min_w_0().truncate().font_weight(FontWeight::MEDIUM).child(it.name.clone()))
                    .on_click(cx.listener(move |r, _, _, cx| {
                        go(r, View::Page { title: open_title.clone(), table: Some(tpl.clone()) });
                        cx.notify();
                    })),
            );
        for l in &columns {
            let v = value_of(it, &l.field).map(|v| v.lines().next().unwrap_or_default().to_string()).unwrap_or_else(|| "–".into());
            row = row.child(div().w(px(col_w)).flex_none().pr(px(8.)).truncate().text_color(c(TEXT_2)).when(l.number, |d| d.font_features(tnum())).child(v));
        }
        rows = rows.child(row);
    }
    let more = shown.len().saturating_sub(ROWS_SHOWN);
    body = body.child(
        surface()
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
    let mut head = div().flex().gap(px(10.)).pb(px(10.)).border_b_1().border_color(ca(WHITE, 0.08)).child(div().w(px(label_w)).flex_none().text_size(px(T_H)).font_weight(FontWeight::SEMIBOLD).child(root.tr("wiki.compare")));
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
            .border_color(ca(WHITE, 0.04))
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
    let mut panel = surface().p(px(16.)).flex().flex_col().child(head);
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
    let items: Vec<Item> = table.and_then(|t| w.tables.get(&key(&site, t))).map(|t| t.items.clone()).unwrap_or_default();
    let named = |i: &Item| fold(&i.name) == fold(title.rsplit('/').next().unwrap_or(title)) || fold(&i.name) == fold(&article.title);
    let item = items.iter().find(|i| named(i)).cloned();
    let facts: Vec<(String, String)> = item
        .as_ref()
        .map(|i| i.fields.clone())
        .or_else(|| w.tables.iter().filter(|(k, _)| k.starts_with(&format!("{site}|"))).find_map(|(_, t)| t.items.iter().find(|i| named(i)).map(|i| i.fields.clone())))
        .or_else(|| article.infobox.as_ref().map(|b| b.rows.clone()))
        .unwrap_or_default();
    if let Some(p) = wikitext::drop_chance(&facts) {
        body = body.child(odds(root, p));
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
    let words = TextView::html(SharedString::from(format!("wk-art-{}", key(&site, &article.title))), article.html.clone())
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
        });
    let text = div().min_w_0().flex_1().text_size(px(T_L)).text_color(c(TEXT)).line_height(px(22.)).child(words);
    let side = article.infobox.as_ref().map(|b| infobox_card(root, b, cx));
    if width >= 980. {
        body = body.child(div().flex().items_start().gap(px(24.)).child(text).children(side.map(|s| s.w(px(SIDE)).flex_none())));
    } else {
        body = body.children(side).child(text);
    }
    if let Some(it) = item {
        body = body.child(similar(root, &it, &items, table.unwrap_or_default(), cx));
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
        body = body.child(div().pt(px(8.)).border_t_1().border_color(ca(WHITE, 0.06)).child(cats));
    }
    body.into_any_element()
}

/// The page's box of facts as a card: its title, picture and lines.
fn infobox_card(root: &Root, b: &Infobox, cx: &mut Context<Root>) -> Div {
    let p = b.image.as_ref().and_then(|u| images::pic(u, cx));
    let mut card = surface().p(px(14.)).flex().flex_col().gap(px(10.));
    if let Some(t) = &b.title {
        card = card.child(div().text_size(px(T_H)).font_weight(FontWeight::SEMIBOLD).text_center().child(t.clone()));
    }
    if let Some(p) = p {
        card = card.child(div().h(px(200.)).rounded(px(R_M)).bg(c(RAISED_2)).overflow_hidden().flex().items_center().justify_center().child(img(p).size_full().object_fit(ObjectFit::Contain)));
    }
    let mut rows = div().flex().flex_col();
    for (i, (k, v)) in b.rows.iter().enumerate() {
        rows = rows.child(
            div()
                .flex()
                .gap(px(10.))
                .py(px(7.))
                .text_size(px(T_M))
                .when(i > 0, |d| d.border_t_1().border_color(ca(WHITE, 0.05)))
                .child(div().w(px(104.)).flex_none().text_color(c(MUTED)).child(word(root, k)))
                .child(div().flex_1().min_w_0().text_color(c(TEXT)).child(v.replace('\n', " · "))),
        );
    }
    card.child(rows)
}

/// The items most like one, as a table: each differing line with + or −, green where it's better.
fn similar(root: &Root, item: &Item, items: &[Item], table: &str, cx: &mut Context<Root>) -> Div {
    let (lines, near) = wikitext::similar_to(item, items);
    if near.is_empty() {
        return div();
    }
    let site = root.wiki.site.clone();
    let mut list = surface().overflow_hidden().flex().flex_col();
    let mut head = div().flex().items_center().h(px(34.)).px(px(12.)).text_size(px(T_S)).text_color(c(MUTED)).font_weight(FontWeight::SEMIBOLD).border_b_1().border_color(ca(WHITE, 0.08)).child(div().w(px(240.)).flex_none().child(root.tr("wiki.name")));
    for l in &lines {
        head = head.child(div().flex_1().child(word(root, &l.field)));
    }
    list = list.child(head);
    for (i, p) in near.iter().enumerate() {
        let title = wiki_title_of(&p.url, &site).unwrap_or_else(|| p.name.clone());
        let tpl = table.to_string();
        let mut row = div().flex().items_center().min_h(px(42.)).px(px(12.)).text_size(px(T_M)).border_b_1().border_color(ca(WHITE, 0.04)).child(
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
        for l in &lines {
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
/// players to have it, on a track. The wiki says only "1%".
fn odds(root: &Root, p: f64) -> Div {
    let marks: Vec<(f64, i64)> = [0.5, 0.9, 0.99].iter().map(|c| (*c, wikitext::tries_for(p, *c))).collect();
    let end = marks[2].1.max(1) as f32;
    let pct = format!("{}%", trim_num(p * 100.));
    let each = root.i18n.tv("wiki.oddsEach", &[("p", &pct), ("n", &n_text(root, (1. / p).round() as i64))]);
    let mut track = div().relative().h(px(56.)).mt(px(6.)).child(div().absolute().top(px(10.)).left_0().right_0().h(px(4.)).rounded_full().bg(c(RAISED_2)));
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
    surface()
        .p(px(16.))
        .pr(px(24.))
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(div().flex().flex_wrap().items_center().gap(px(10.)).child(div().text_size(px(T_H)).font_weight(FontWeight::SEMIBOLD).child(root.tr("wiki.odds"))).child(div().text_size(px(T_M)).text_color(c(MUTED)).child(each)))
        .child(track)
}

fn trim_num(x: f64) -> String {
    let s = format!("{:.3}", x);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}
