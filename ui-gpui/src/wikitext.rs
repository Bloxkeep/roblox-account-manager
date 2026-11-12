// Wiki markup read the way the old Wiki part read it (ui/src/wikiText.ts, WikiView.tsx), for the Wiki tab of a game's
// page (pages/wiki.rs): a page's box of facts (its infobox: damage, rarity, where to get it…) as lines of plain text and
// its first words, the numbers a page's box changed between two versions, what a link of a page does, and the sums the
// tables make (similar items, drop odds). Wiki text is CC BY-SA: the tab names the wiki under every page and links it.
use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use crate::kit::folded;

/// A page as an item of a kind (a thing the wiki keeps a box of facts for).
#[derive(Clone, Debug)]
pub struct Item {
    pub id: String,
    pub name: String,
    pub cat: String,
    pub fields: Vec<(String, String)>,
    pub lead: String,
    pub img: Option<String>,
    pub url: String,
    /// Lines the wiki writes as a short code in a template ({{Rarity|L}}): line → template, for the wiki to say in words.
    pub codes: Vec<(String, String)>,
}

const FIELDS_MAX: usize = 24;
const FIELD_MAX: usize = 500;
const LEAD_MAX: usize = 420;

fn re(s: &str) -> Regex {
    Regex::new(s).expect("wiki pattern")
}

/// A line that is one template around a short code: {{Rarity|L}}, {{Tier|S}}.
static CODE: LazyLock<Regex> = LazyLock::new(|| re(r"^\s*\{\{\s*[^{}|]+\|\s*[^{}|=]{1,4}\s*\}\}\s*$"));
/// Infobox lines that are about the page, not the item.
static SKIP: LazyLock<Regex> = LazyLock::new(|| {
    re(r"(?i)^(image\d*|images?|caption\d*|name\d*|title\d*|alloyed name|description|desc|quote|gallery|imagewidth|width|m\.equip|ref|id|category|killmessage|music|gender|theme|imagecaption|image_caption|tabber|navbox|icon|thumbnail|model|render|tab\d+|link\d+)$")
});
static EMPTY: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)^(tba|tbd|n/?a|none|\?+|-|unknown)$"));
static FILE: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)^[\w .-]+\.(png|jpe?g|gif|webp)$"));
static WIKI_IMG: LazyLock<Regex> = LazyLock::new(|| re(r"^https://(static\.wikia\.nocookie\.net|[a-z0-9-]+\.wiki\.gg)/"));

/// A picture on the wiki's own servers (the only ones drawn).
pub fn wiki_img(u: &str) -> bool {
    WIKI_IMG.is_match(u)
}

/// Wikis' short line names, said in full.
fn short_label(k: &str) -> Option<&'static str> {
    Some(match k {
        "hp" => "HP",
        "xp" | "exp" => "XP",
        "def" => "Defense",
        "dmg" => "Damage",
        "atk" => "Attack",
        "spd" => "Speed",
        "desc" => "Description",
        "reqs" | "req" => "Requirements",
        "mantratype" => "Mantra type",
        "ethercost" => "Ether cost",
        "dmgtype" => "Damage type",
        "dmgtaken" => "Damage taken",
        "poisebreakable" => "Poise breakable",
        "cd" => "Cooldown",
        "lvl" => "Level",
        _ => return None,
    })
}

/// A page as an item of a kind (its infobox template), or None when it has no box and no words. `page` is a page of a
/// MediaWiki query (formatversion 2) with revisions (content), pageimages and info (url).
pub fn item(page: &Value, cat: &str, template: &str, site: &str) -> Option<Item> {
    let text = page.pointer("/revisions/0/slots/main/content").and_then(Value::as_str)?;
    let name = page.get("title").and_then(Value::as_str).unwrap_or_default().trim().to_string();
    if name.is_empty() || text.get(..9).is_some_and(|h| h.eq_ignore_ascii_case("#redirect")) {
        return None;
    }
    let bx = infobox(text, Some(template));
    let mut fields = Vec::new();
    let mut codes = Vec::new();
    let mut about = String::new();
    for (k, v) in bx.as_ref().map(|b| b.params.as_slice()).unwrap_or_default() {
        if about.is_empty() && (k.eq_ignore_ascii_case("desc") || k.eq_ignore_ascii_case("description")) {
            about = clean(v, true);
        }
        if SKIP.is_match(k) || fields.len() >= FIELDS_MAX {
            continue;
        }
        let value = clean(v, false);
        if !value.is_empty() && !EMPTY.is_match(&value) && !FILE.is_match(&value) {
            fields.push((label(k), cut(&value, FIELD_MAX)));
            if CODE.is_match(v) {
                codes.push((label(k), v.trim().to_string()));
            }
        }
    }
    let lead = Some(lead_of(text, bx.as_ref())).filter(|l| !l.is_empty()).unwrap_or(about);
    if fields.is_empty() && lead.is_empty() {
        return None;
    }
    let img = page.pointer("/thumbnail/source").and_then(Value::as_str).filter(|u| wiki_img(u)).map(str::to_string);
    let url = page
        .get("fullurl")
        .and_then(Value::as_str)
        .filter(|u| u.starts_with("https://"))
        .map(str::to_string)
        .unwrap_or_else(|| page_url(site, &name));
    let id = page.get("pageid").map(|v| v.to_string()).filter(|v| v != "null").unwrap_or_else(|| name.clone());
    let short: String = name.rsplit('/').next().unwrap_or(&name).chars().take(100).collect();
    Some(Item { id, name: short, cat: cat.to_string(), fields, lead: cut(&lead, LEAD_MAX), img, url, codes })
}

/// A page's address on its wiki.
pub fn page_url(site: &str, title: &str) -> String {
    let t = title.replace(' ', "_");
    format!("{site}/wiki/{}", percent_encoding::utf8_percent_encode(&t, percent_encoding::NON_ALPHANUMERIC))
}

/// At most n characters, cut after a sentence or between words (…).
fn cut(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        return s.to_string();
    }
    let part: String = s.chars().take(n).collect();
    let stop = part.rfind(". ").into_iter().chain(part.rfind('\n')).max();
    match stop {
        Some(at) if part[..at].chars().count() as f32 > n as f32 * 0.6 => part[..=at].trim().to_string(),
        _ => match part.rfind(char::is_whitespace) {
            Some(at) => format!("{}…", part[..at].trim_end()),
            None => format!("{part}…"),
        },
    }
}

/// "posture damage" → "Posture damage", "drop_rate" → "Drop rate", "hp" → "HP".
fn label(k: &str) -> String {
    let s = k.replace('_', " ").split_whitespace().collect::<Vec<_>>().join(" ");
    if let Some(l) = short_label(&s.to_lowercase()) {
        return l.to_string();
    }
    let mut ch = s.chars();
    let first = ch.next().map(|c| c.to_uppercase().collect::<String>()).unwrap_or_default();
    format!("{first}{}", ch.as_str()).chars().take(40).collect()
}

#[derive(Clone)]
pub struct Template {
    pub name: String,
    pub start: usize,
    pub end: usize,
    pub params: Vec<(String, String)>,
}

fn same(a: &str, b: &str) -> bool {
    a.replace('_', " ").trim().to_lowercase() == b.replace('_', " ").trim().to_lowercase()
}

/// The page's box: the template the kind is (Template:Weapon Infobox), else the first named Infobox, else the first
/// with three named lines.
pub fn infobox(text: &str, want: Option<&str>) -> Option<Template> {
    let wanted = want.map(|w| if w.get(..9).is_some_and(|p| p.eq_ignore_ascii_case("template:")) { &w[9..] } else { w });
    let mut named: Option<Template> = None;
    let mut first: Option<Template> = None;
    for (name, body, start, end) in templates(text) {
        let params: Vec<(String, String)> = parse_params(body).into_iter().filter(|(k, _)| !k.is_empty()).collect();
        let bx = Template { name: name.clone(), start, end, params };
        if wanted.is_some_and(|w| same(&name, w)) {
            return Some(bx);
        }
        if first.is_none() && bx.params.len() >= 3 {
            first = Some(bx.clone());
        }
        if named.is_none() && name.to_lowercase().contains("infobox") {
            named = Some(bx);
        }
    }
    named.or(first)
}

/// A changed line of a page's box: was, now, and whether the first number that changed went up.
#[derive(Clone, Debug)]
pub struct BoxChange {
    pub field: String,
    pub was: String,
    pub now: String,
    pub up: bool,
}

static NUMBER: LazyLock<Regex> = LazyLock::new(|| re(r"-?\d[\d,]*(?:\.\d+)?"));
static EXP: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)\de[+-]?\d"));
static PICTURE_LINE: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)image|file|gif|caption|music"));

/// A number in a line: "1,200", "9.5", "-30". Written as 1e+420 (a joke edit) it isn't one.
fn numbers_of(s: &str) -> Vec<f64> {
    if EXP.is_match(s) {
        return Vec::new();
    }
    NUMBER.find_iter(s).filter_map(|m| m.as_str().replace(',', "").parse().ok()).collect()
}

/// The lines of a page's box whose numbers changed between two versions of the page (drop chance 1% → 5%, range 10 →
/// 9.5): only lines both versions have, with a few numbers each, the same count of them.
pub fn box_changes(old_text: &str, new_text: &str) -> Vec<BoxChange> {
    let Some(now) = infobox(new_text, None) else { return Vec::new() };
    let Some(was) = infobox(old_text, Some(&now.name)) else { return Vec::new() };
    let lines = |b: &Template| -> Vec<(String, String)> {
        b.params
            .iter()
            .filter(|(k, _)| !SKIP.is_match(k) && !PICTURE_LINE.is_match(k))
            .map(|(k, v)| (label(k), clean(v, false).replace('\n', " ")))
            .collect()
    };
    let before = lines(&was);
    let mut out = Vec::new();
    for (field, b) in lines(&now) {
        let Some(a) = before.iter().find(|(f, _)| *f == field).map(|(_, v)| v.clone()) else { continue };
        if a == b || a.chars().count() > 60 || b.chars().count() > 60 {
            continue;
        }
        let (x, y) = (numbers_of(&a), numbers_of(&b));
        if x.is_empty() || x.len() != y.len() || x.len() > 4 {
            continue;
        }
        let sorted = |n: &[f64]| {
            let mut v = n.to_vec();
            v.sort_by(f64::total_cmp);
            v
        };
        if sorted(&x) == sorted(&y) {
            continue;
        }
        let Some(i) = x.iter().zip(&y).position(|(p, q)| p != q) else { continue };
        out.push(BoxChange { field, was: a, now: b.clone(), up: y[i] > x[i] });
        if out.len() >= 6 {
            break;
        }
    }
    out
}

/// The page's top-level templates ({{…}} not inside another): name, body, start, end.
fn templates(text: &str) -> Vec<(String, &str, usize, usize)> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(at) = text[i..].find("{{").map(|f| f + i) {
        let Some(end) = match_end(text, at) else { break };
        let body = &text[at + 2..end - 2];
        out.push((body.split('|').next().unwrap_or_default().trim().to_string(), body, at, end));
        i = end;
    }
    out
}

/// Where the {{ at i closes (just after its }}).
fn match_end(text: &str, i: usize) -> Option<usize> {
    let b = text.as_bytes();
    let mut depth = 0i32;
    let mut j = i;
    while j + 1 < b.len() {
        if b[j] == b'{' && b[j + 1] == b'{' {
            depth += 1;
            j += 1;
        } else if b[j] == b'}' && b[j + 1] == b'}' {
            depth -= 1;
            j += 1;
            if depth == 0 {
                return Some(j + 1);
            }
        }
        j += 1;
    }
    None
}

/// A template's lines: (name, value) (name "" for an unnamed one), split at | outside {{ }} and [[ ]].
fn parse_params(body: &str) -> Vec<(String, String)> {
    let b = body.as_bytes();
    let (mut depth, mut link) = (0i32, 0i32);
    let mut parts = Vec::new();
    let (mut start, mut j) = (0, 0);
    while j < b.len() {
        if j + 1 < b.len() {
            match (b[j], b[j + 1]) {
                (b'{', b'{') => {
                    depth += 1;
                    j += 2;
                    continue;
                }
                (b'}', b'}') => {
                    depth -= 1;
                    j += 2;
                    continue;
                }
                (b'[', b'[') => {
                    link += 1;
                    j += 2;
                    continue;
                }
                (b']', b']') => {
                    link -= 1;
                    j += 2;
                    continue;
                }
                _ => {}
            }
        }
        if b[j] == b'|' && depth == 0 && link == 0 {
            parts.push(&body[start..j]);
            start = j + 1;
        }
        j += 1;
    }
    parts.push(&body[start..]);
    parts
        .into_iter()
        .skip(1)
        .map(|p| {
            // A name has no markup before its "=" (a value can hold one: "[[a|b=c]]").
            if let Some(eq) = p.find('=').filter(|eq| *eq > 0 && !p[..*eq].contains(['{', '[', '<'])) {
                return (p[..eq].trim().to_string(), p[eq + 1..].trim().to_string());
            }
            (String::new(), p.trim().to_string())
        })
        .collect()
}

static HEADING_LINE: LazyLock<Regex> = LazyLock::new(|| re(r"(?m)^==[^=].*?==\s*$"));
static PARAGRAPHS: LazyLock<Regex> = LazyLock::new(|| re(r"\n\s*\n"));

/// The words before the first heading under the infobox; else the first paragraph of the first heading.
fn lead_of(text: &str, bx: Option<&Template>) -> String {
    let rest = match bx {
        Some(b) => format!("{}{}", &text[..b.start], &text[b.end..]),
        None => text.to_string(),
    };
    let blocks: Vec<&str> = HEADING_LINE.split(&rest).collect();
    for block in blocks.iter().take(if blocks.len() > 1 { 2 } else { 1 }) {
        if let Some(p) = PARAGRAPHS.split(block).map(|p| clean(p, true)).find(|p| p.chars().count() > 25) {
            return p;
        }
    }
    String::new()
}

static CLEAN_DROP: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [r"(?s)<!--.*?-->", r"__[A-Z]+__", r"(?is)<gallery.*?</gallery>", r"(?is)<tabber.*?</tabber>", r"(?i)<ref[^>]*/>", r"(?is)<ref.*?</ref>", r"(?s)\{\|.*?\|\}"]
        .into_iter()
        .map(re)
        .collect()
});
static BR: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)<br\s*/?>"));
static LONE_TEMPLATE: LazyLock<Regex> = LazyLock::new(|| re(r"(?m)^[ \t]*\{\{[^\n]*\}\}[ \t]*$"));
static INNER_TEMPLATE: LazyLock<Regex> = LazyLock::new(|| re(r"\{\{([^{}]*)\}\}"));
static FILE_LINK: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)\[\[(?:File|Image|Category|Datei):[^\]]*\]\]"));
static PIPED_LINK: LazyLock<Regex> = LazyLock::new(|| re(r"\[\[[^\]|]*\|([^\]]*)\]\]"));
static PLAIN_LINK: LazyLock<Regex> = LazyLock::new(|| re(r"\[\[([^\]]*)\]\]"));
static NAMED_OUT: LazyLock<Regex> = LazyLock::new(|| re(r"\[https?://[^\s\]]+\s+([^\]]+)\]"));
static BARE_OUT: LazyLock<Regex> = LazyLock::new(|| re(r"\[https?://[^\]]+\]"));
static QUOTES: LazyLock<Regex> = LazyLock::new(|| re(r"'''?"));
static TAG: LazyLock<Regex> = LazyLock::new(|| re(r"<[^>]+>"));
static LIST_MARK: LazyLock<Regex> = LazyLock::new(|| re(r"^[*#:;]+\s*"));
static SPACES: LazyLock<Regex> = LazyLock::new(|| re(r"[ \t]+"));
static BEFORE_STOP: LazyLock<Regex> = LazyLock::new(|| re(r"\s+([,.;:])"));

/// Wiki markup as plain text: links as their words, files, galleries, references and tables gone, a small template as
/// its last unnamed value ({{cl|alloyed|75}} → 75), list items on their own lines. In prose a template standing alone on
/// its line is dropped (a box drawn there, not words).
pub fn clean(s: &str, prose: bool) -> String {
    let mut t = s.to_string();
    for r in CLEAN_DROP.iter() {
        t = r.replace_all(&t, "").into_owned();
    }
    t = BR.replace_all(&t, "\n").into_owned();
    if prose {
        t = LONE_TEMPLATE.replace_all(&t, "").into_owned();
    }
    for _ in 0..50 {
        if !t.contains("{{") {
            break;
        }
        let next = INNER_TEMPLATE
            .replace_all(&t, |c: &regex::Captures| {
                parse_params(&c[1]).into_iter().filter(|(k, v)| k.is_empty() && !v.is_empty()).map(|(_, v)| v).next_back().unwrap_or_default()
            })
            .into_owned();
        if next == t {
            break;
        }
        t = next;
    }
    t = t.replace("{{", "").replace("}}", "");
    t = FILE_LINK.replace_all(&t, "").into_owned();
    t = PIPED_LINK.replace_all(&t, "$1").into_owned();
    t = PLAIN_LINK.replace_all(&t, "$1").into_owned();
    t = NAMED_OUT.replace_all(&t, "$1").into_owned();
    t = BARE_OUT.replace_all(&t, "").into_owned();
    t = QUOTES.replace_all(&t, "").into_owned();
    t = TAG.replace_all(&t, "").into_owned();
    t = t.replace("&nbsp;", " ").replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"");
    let lines: Vec<String> = t
        .split('\n')
        .map(|l| {
            let l = LIST_MARK.replace(l, |c: &regex::Captures| if c[0].contains(['*', '#']) { "• ".to_string() } else { String::new() });
            SPACES.replace_all(&l, " ").trim().to_string()
        })
        .filter(|l| !l.is_empty() && l != "•")
        .collect();
    let joined = lines.join(if prose { " " } else { "\n" });
    BEFORE_STOP.replace_all(&joined, "$1").trim().to_string()
}

static KIND_END: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)\s*(info ?box|info ?template|template|box)\s*$"));
static KIND_START: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)^(info ?box)\s*"));
static CAMEL: LazyLock<Regex> = LazyLock::new(|| re(r"([a-z])([A-Z])"));

/// A kind's name from its template: "Weapon_Infobox" → "Weapon", "SeedPackInfoTemplate" → "Seed Pack".
pub fn kind_name(template: &str) -> String {
    let t = if template.get(..9).is_some_and(|p| p.eq_ignore_ascii_case("template:")) { &template[9..] } else { template };
    let s = t.replace('_', " ");
    let s = KIND_END.replace(&s, "");
    let s = KIND_START.replace(&s, "");
    let s = CAMEL.replace_all(&s, "$1 $2").trim().to_string();
    if s.is_empty() { template.to_string() } else { s }
}

/// What a page's link does: another page of the same wiki (opened here), a place in this page, or a link out.
#[derive(Clone, Debug, PartialEq)]
pub enum Link {
    Page { title: String, anchor: Option<String> },
    Anchor(String),
    Out(String),
}

static NOT_HERE: LazyLock<Regex> = LazyLock::new(|| {
    re(r"(?i)^(File|Image|User|User talk|Special|Talk|Message Wall|Board|Thread|MediaWiki|Template|Module|Help|Forum|Blog|User blog)( talk)?:")
});

fn decode(s: &str) -> String {
    percent_encoding::percent_decode_str(s).decode_utf8_lossy().into_owned()
}

/// What a page's link does, by where it points.
pub fn link_of(href: &str, site: &str) -> Option<Link> {
    if href.is_empty() {
        return None;
    }
    if let Some(a) = href.strip_prefix('#') {
        return Some(Link::Anchor(decode(a)));
    }
    let base = url::Url::parse(&format!("{site}/")).ok()?;
    let u = base.join(href).ok()?;
    if u.scheme() != "https" && u.scheme() != "http" {
        return None;
    }
    // A picture's own file: nothing to open.
    if u.host_str() == Some("static.wikia.nocookie.net") || u.path().contains("/images/") {
        return None;
    }
    let lang = base.path().trim_end_matches('/').to_string();
    let prefix = format!("{lang}/wiki/");
    if u.host_str() == base.host_str() && u.path().starts_with(&prefix) {
        let title = decode(&u.path()[prefix.len()..]).replace('_', " ");
        // Its own pages open here; files, users, specials and talk open on the wiki.
        if !title.is_empty() && !NOT_HERE.is_match(&title) && u.query().is_none() {
            return Some(Link::Page { title, anchor: u.fragment().map(decode) });
        }
    }
    let out = u.to_string();
    Some(Link::Out(out.strip_prefix("http:").map(|r| format!("https:{r}")).unwrap_or(out)))
}

// ---- The tables' sums (WikiView.tsx) ----

/// Lower case, without accents (đ as d).
pub fn fold(s: &str) -> String {
    folded(s)
}

static LEAD_NUMBER: LazyLock<Regex> = LazyLock::new(|| re(r"^[+]?(-?\d[\d,]*(?:\.\d+)?)"));
static LESS_BETTER: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)duration|endlag|startup|cost|cooldown|requirement|weight|delay|cast time|mana|ether|price|level"));

/// The number a value starts with ("35 HVY / 75 HVY" 35, "0.85x" 0.85, "+12" 12, "1,200" 1200).
pub fn number_of(v: Option<&str>) -> Option<f64> {
    let c = LEAD_NUMBER.captures(v?.trim())?;
    c[1].replace(',', "").parse().ok()
}

/// Lines where less is better (an attack's duration, a cost, what it asks for).
pub fn less_is_better(field: &str) -> bool {
    LESS_BETTER.is_match(field)
}

pub fn value_of<'a>(i: &'a Item, f: &str) -> Option<&'a str> {
    i.fields.iter().find(|(k, _)| k == f).map(|(_, v)| v.as_str())
}

/// A line's words in Vietnamese, for the lines most wikis have (the rest stay as the wiki says them).
pub fn vi_word(f: &str) -> Option<&'static str> {
    Some(match f {
        "Damage" => "Sát thương",
        "Rarity" => "Độ hiếm",
        "Type" => "Loại",
        "Obtainment" => "Cách lấy",
        "Obtained from" => "Lấy từ",
        "Requirements" => "Yêu cầu",
        "Selling price" | "Sell" => "Giá bán",
        "Range" => "Tầm đánh",
        "Swing speed" => "Tốc độ vung",
        "Posture damage" => "Sát thương posture",
        "Location" => "Vị trí",
        "Drops" => "Rơi ra",
        "Defense" => "Phòng thủ",
        "Gold" => "Vàng",
        "Element" => "Nguyên tố",
        "Cost" => "Giá",
        "Cooldown" => "Hồi chiêu",
        "Innate stats" => "Chỉ số gốc",
        "Attack duration" => "Thời gian đánh",
        "Penetration" => "Xuyên giáp",
        "Chip damage" => "Sát thương xuyên đỡ",
        "Mana cost" => "Tốn mana",
        "Class" => "Lớp",
        "Subtype" => "Loại phụ",
        "Droppable" => "Rơi khi chết",
        "Bankable" => "Gửi kho được",
        "Health" => "Máu",
        "Weight" => "Trọng lượng",
        "Damage type" => "Kiểu sát thương",
        "Range type" => "Kiểu tầm",
        "Endlag" => "Độ trễ sau đòn",
        "Effect" => "Hiệu ứng",
        "Innate talent" => "Talent sẵn có",
        "Special effect" => "Hiệu ứng đặc biệt",
        "Species" => "Loài",
        "Affiliation" | "Faction" => "Phe",
        "Damage taken" => "Sát thương nhận",
        "Ether cost" => "Tốn ether",
        "Mantra type" => "Loại mantra",
        "Usage" => "Cách dùng",
        "Rewards" => "Phần thưởng",
        "Quest giver" => "Người giao nhiệm vụ",
        "Value" => "Giá trị",
        "Stack" => "Chồng tối đa",
        "Knockback" => "Đẩy lùi",
        "Crit chance" => "Tỉ lệ chí mạng",
        "Velocity" => "Tốc độ bay",
        "Heavy cooldown" => "Hồi đòn nặng",
        _ => return None,
    })
}

/// A number line the similar items are measured on: the item's value and the spread of the kind's.
pub struct SimLine {
    pub field: String,
    pub at: f64,
    pub span: f64,
}

/// The items most like one: same kind, nearest on the number lines it has that most of the kind has too, a different
/// Type counting against.
pub fn similar_to<'a>(item: &Item, items: &'a [Item]) -> (Vec<SimLine>, Vec<&'a Item>) {
    let peers: Vec<&Item> = items.iter().filter(|i| i.id != item.id).collect();
    let mut lines = Vec::new();
    for (k, v) in &item.fields {
        let Some(n) = number_of(Some(v)) else { continue };
        let have: Vec<f64> = peers.iter().filter_map(|p| number_of(value_of(p, k))).collect();
        if (have.len() as f64) < peers.len() as f64 * 0.5 || have.len() < 2 {
            continue;
        }
        let hi = have.iter().copied().fold(n, f64::max);
        let lo = have.iter().copied().fold(n, f64::min);
        lines.push(SimLine { field: k.clone(), at: n, span: if hi - lo == 0. { 1. } else { hi - lo } });
        if lines.len() >= 3 {
            break;
        }
    }
    let ty = value_of(item, "Type");
    let mut near: Vec<(&Item, f64)> = peers
        .into_iter()
        .filter_map(|p| {
            let (mut d, mut known) = (0., 0usize);
            for l in &lines {
                if let Some(n) = number_of(value_of(p, &l.field)) {
                    d += (n - l.at).abs() / l.span;
                    known += 1;
                }
            }
            (known > 0).then(|| {
                let other = ty.is_some() && value_of(p, "Type") != ty;
                (p, d / known as f64 + (lines.len() - known) as f64 * 0.5 + if other { 0.25 } else { 0. })
            })
        })
        .collect();
    near.sort_by(|a, b| a.1.total_cmp(&b.1));
    (lines, near.into_iter().take(6).map(|(p, _)| p).collect())
}

/// Rarities from lowest to highest, as most Roblox wikis name them.
const RARITIES: [&[&str]; 7] = [&["common"], &["uncommon"], &["rare"], &["epic"], &["legendary"], &["mythic", "mythical"], &["godly", "divine", "secret", "exotic", "celestial"]];

/// What a value ranks as on its line: its number, or on a rarity line its rarity.
pub fn rank_of(field: &str, v: Option<&str>) -> Option<f64> {
    if let Some(n) = number_of(v) {
        return Some(n);
    }
    let v = v?;
    let f = field.to_lowercase();
    if !f.contains("rarity") && !f.contains("tier") {
        return None;
    }
    let words: Vec<String> = fold(v).split(|c: char| !c.is_ascii_lowercase()).map(str::to_string).collect();
    let mut rank = None;
    for (r, names) in RARITIES.iter().enumerate() {
        if names.iter().any(|x| words.iter().any(|w| w == x)) {
            rank = Some(r as f64);
        }
    }
    rank
}

static ONE_IN: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)\b1\s*(?:/|in)\s*([\d,]+(?:\.\d+)?)"));
static PERCENT: LazyLock<Regex> = LazyLock::new(|| re(r"([\d.]+)\s*%"));
static CHANCE_LINE: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)drop|chance|rate|odds"));

/// An item's chance to drop each try, from its "drop chance" line: "1%", "0.5 %", "1/250", "1 in 40".
pub fn drop_chance(fields: &[(String, String)]) -> Option<f64> {
    for (k, v) in fields {
        if !CHANCE_LINE.is_match(k) {
            continue;
        }
        let p = if let Some(c) = ONE_IN.captures(v) {
            c[1].replace(',', "").parse::<f64>().ok().map(|n| 1. / n)
        } else {
            PERCENT.captures(v).and_then(|c| c[1].parse::<f64>().ok()).map(|n| n / 100.)
        };
        if let Some(p) = p.filter(|p| *p > 0. && *p < 1.) {
            return Some(p);
        }
    }
    None
}

/// Tries for a given chance to have had it at least once.
pub fn tries_for(p: f64, want: f64) -> i64 {
    ((1. - want).ln() / (1. - p).ln()).ceil() as i64
}
