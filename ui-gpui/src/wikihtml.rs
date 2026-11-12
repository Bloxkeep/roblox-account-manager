// A wiki page as the wiki parsed it (MediaWiki's action=parse), made safe and plain for the Wiki tab (pages/wiki.rs) to
// draw with GPUI's text view: no code, frames, forms or the wiki's own styles; pictures only from the wiki's own servers;
// every link rewritten to say what it does (another page here, a place in this page, or out to the browser). The page's
// box of facts (its infobox) is taken out and handed back on its own, so the tab draws it as a card beside the words
// (ui/src/wikiText.ts cleanPage did the same in the browser).
use std::{cell::RefCell, collections::HashSet};

use html5ever::{parse_document, tendril::TendrilSink};
use markup5ever_rcdom::{Handle, NodeData, RcDom};

use crate::wikitext::{Link, link_of};

/// A page's box of facts: its title, picture and lines (label, value).
#[derive(Clone, Debug, Default)]
pub struct Infobox {
    pub title: Option<String>,
    pub image: Option<String>,
    pub rows: Vec<(String, String)>,
}

/// A page ready to draw: its HTML, its box of facts, and the pictures the HTML shows.
#[derive(Clone, Debug, Default)]
pub struct Clean {
    pub html: String,
    pub infobox: Option<Infobox>,
    pub images: Vec<String>,
}

/// How a rewritten link says what it does (the text view hands the address back on a click).
pub const PAGE: &str = "bk-wiki-page:";
pub const ANCHOR: &str = "bk-wiki-anchor:";
pub const OUT: &str = "bk-wiki-out:";

const KEEP_TAGS: &[&str] = &[
    "a", "b", "big", "blockquote", "br", "caption", "cite", "code", "dd", "del", "dl", "dt", "em", "h1", "h2", "h3", "h4", "h5", "h6", "hr", "i", "ins",
    "kbd", "li", "mark", "ol", "p", "pre", "s", "small", "strong", "sub", "sup", "table", "tbody", "td", "tfoot", "th", "thead", "tr", "u", "ul",
];
/// Gone with all they hold: code, forms, frames, and media the view can't play.
const DROP_TAGS: &[&str] = &[
    "script", "style", "link", "meta", "iframe", "object", "embed", "form", "input", "button", "select", "textarea", "video", "audio", "source", "svg",
    "math", "noscript", "template", "canvas", "map", "area", "base", "frame", "frameset", "dialog", "portal", "head", "title",
];
/// The wiki's edit links, contents box, navigation boxes and references: gone.
const DROP_CLASSES: &[&str] = &[
    "mw-editsection", "toc", "navbox", "noprint", "mw-empty-elt", "reference", "reflist", "references", "printfooter", "catlinks", "mw-cite-backlink",
    "wds-tabs__wrapper", "article-thumb-placeholder", "fandom-community-header", "mobileHide",
];
/// The widest a picture is drawn in the words.
const IMG_MAX: u32 = 420;

fn attr(attrs: &RefCell<Vec<html5ever::Attribute>>, name: &str) -> Option<String> {
    attrs.borrow().iter().find(|a| &*a.name.local == name).map(|a| a.value.to_string())
}

fn classes(attrs: &RefCell<Vec<html5ever::Attribute>>) -> Vec<String> {
    attr(attrs, "class").map(|c| c.split_whitespace().map(str::to_string).collect()).unwrap_or_default()
}

fn esc(s: &str, out: &mut String) {
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
}

/// A node's words alone, spaces squeezed.
fn text_of(node: &Handle) -> String {
    fn walk(n: &Handle, out: &mut String) {
        match &n.data {
            NodeData::Text { contents } => out.push_str(&contents.borrow()),
            NodeData::Element { name, attrs, .. } => {
                let tag = &*name.local;
                if DROP_TAGS.contains(&tag) || classes(attrs).iter().any(|c| DROP_CLASSES.contains(&c.as_str())) {
                    return;
                }
                if tag == "br" || tag == "li" || tag == "p" || tag == "div" {
                    out.push('\n');
                }
                for c in n.children.borrow().iter() {
                    walk(c, out);
                }
            }
            _ => {
                for c in n.children.borrow().iter() {
                    walk(c, out);
                }
            }
        }
    }
    let mut s = String::new();
    walk(node, &mut s);
    s.lines().map(|l| l.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n")
}

/// A picture's address on the wiki's own servers, absolute; None for any other.
fn img_ok(src: &str, site: &str) -> Option<String> {
    if src.starts_with("data:") {
        return None;
    }
    let base = url::Url::parse(&format!("{site}/")).ok()?;
    let u = base.join(src).ok()?;
    let host = u.host_str()?;
    (u.scheme() == "https" && (host == "static.wikia.nocookie.net" || Some(host) == base.host_str() || host.ends_with(".wiki.gg"))).then(|| u.to_string())
}

/// The real picture of an img: Fandom puts a blank in src and the picture in data-src until it is scrolled to.
fn img_src(attrs: &RefCell<Vec<html5ever::Attribute>>, site: &str) -> Option<String> {
    attr(attrs, "data-src").and_then(|s| img_ok(&s, site)).or_else(|| attr(attrs, "src").and_then(|s| img_ok(&s, site)))
}

fn find_all(node: &Handle, want: &dyn Fn(&str, &[String]) -> bool, out: &mut Vec<Handle>) {
    for c in node.children.borrow().iter() {
        if let NodeData::Element { name, attrs, .. } = &c.data {
            if want(&name.local, &classes(attrs)) {
                out.push(c.clone());
                continue;
            }
        }
        find_all(c, want, out);
    }
}

fn first(node: &Handle, want: &dyn Fn(&str, &[String]) -> bool) -> Option<Handle> {
    let mut v = Vec::new();
    find_all(node, want, &mut v);
    v.into_iter().next()
}

fn has(cls: &[String], c: &str) -> bool {
    cls.iter().any(|x| x == c)
}

/// A Fandom "portable infobox" (aside.portable-infobox): its title, first picture and data lines.
fn portable(node: &Handle, site: &str) -> Option<Infobox> {
    let title = first(node, &|_, c| has(c, "pi-title")).map(|t| text_of(&t)).filter(|t| !t.is_empty());
    let image = first(node, &|t, _| t == "img").and_then(|i| match &i.data {
        NodeData::Element { attrs, .. } => img_src(attrs, site),
        _ => None,
    });
    let mut datas = Vec::new();
    find_all(node, &|_, c| has(c, "pi-data"), &mut datas);
    let rows: Vec<(String, String)> = datas
        .iter()
        .filter_map(|d| {
            let label = first(d, &|_, c| has(c, "pi-data-label")).map(|l| text_of(&l))?;
            let value = first(d, &|_, c| has(c, "pi-data-value")).map(|v| text_of(&v))?;
            (!label.is_empty() && !value.is_empty()).then_some((label, value))
        })
        .collect();
    (image.is_some() || !rows.is_empty()).then_some(Infobox { title, image, rows })
}

/// A table infobox (wiki.gg and older wikis: table.infobox): a row's header and cell as a line.
fn table_box(node: &Handle, site: &str) -> Option<Infobox> {
    let image = first(node, &|t, _| t == "img").and_then(|i| match &i.data {
        NodeData::Element { attrs, .. } => img_src(attrs, site),
        _ => None,
    });
    let mut trs = Vec::new();
    find_all(node, &|t, _| t == "tr", &mut trs);
    let mut title = None;
    let mut rows = Vec::new();
    for tr in &trs {
        let cells: Vec<Handle> = tr.children.borrow().iter().filter(|c| matches!(&c.data, NodeData::Element { name, .. } if &*name.local == "th" || &*name.local == "td")).cloned().collect();
        match cells.as_slice() {
            [one] if title.is_none() && rows.is_empty() => {
                let t = text_of(one);
                if !t.is_empty() && t.chars().count() <= 80 {
                    title = Some(t);
                }
            }
            [k, v] => {
                let (k, v) = (text_of(k), text_of(v));
                if !k.is_empty() && !v.is_empty() && k.chars().count() <= 60 {
                    rows.push((k, v));
                }
            }
            _ => {}
        }
    }
    (image.is_some() || rows.len() >= 2).then_some(Infobox { title, image, rows })
}

struct Ctx<'a> {
    site: &'a str,
    infobox: Option<Infobox>,
    images: Vec<String>,
    seen: HashSet<String>,
}

fn walk(node: &Handle, out: &mut String, cx: &mut Ctx) {
    match &node.data {
        NodeData::Text { contents } => esc(&contents.borrow(), out),
        NodeData::Element { name, attrs, .. } => {
            let tag: &str = &name.local;
            let cls = classes(attrs);
            if DROP_TAGS.contains(&tag) || cls.iter().any(|c| DROP_CLASSES.contains(&c.as_str())) || attr(attrs, "id").as_deref() == Some("toc") {
                return;
            }
            // The first box of facts is drawn by the tab as a card.
            if cx.infobox.is_none() {
                let got = if has(&cls, "portable-infobox") {
                    portable(node, cx.site)
                } else if tag == "table" && cls.iter().any(|c| c.contains("infobox")) {
                    table_box(node, cx.site)
                } else {
                    None
                };
                if let Some(b) = got {
                    cx.infobox = Some(b);
                    return;
                }
            }
            if tag == "img" {
                let Some(src) = img_src(attrs, cx.site) else { return };
                let num = |n: &str| attr(attrs, n).and_then(|v| v.parse::<u32>().ok()).filter(|v| *v > 0 && *v < 10_000);
                let (mut w, mut h) = (num("width"), num("height"));
                // Little icons beside words (a coin, an element) stay as small as the wiki draws them.
                if let (Some(ww), Some(hh)) = (w, h)
                    && ww > IMG_MAX
                {
                    h = Some(hh * IMG_MAX / ww);
                    w = Some(IMG_MAX);
                }
                out.push_str("<img src=\"");
                esc(&src, out);
                out.push('"');
                if let Some(w) = w {
                    out.push_str(&format!(" width=\"{w}\""));
                }
                if let Some(h) = h {
                    out.push_str(&format!(" height=\"{h}\""));
                }
                if let Some(alt) = attr(attrs, "alt") {
                    out.push_str(" alt=\"");
                    esc(&alt, out);
                    out.push('"');
                }
                out.push('>');
                if cx.seen.insert(src.clone()) {
                    cx.images.push(src);
                }
                return;
            }
            let keep = KEEP_TAGS.contains(&tag);
            // A block the view doesn't know (div, section, figure…) keeps its words apart from the next.
            let block = !keep && matches!(tag, "div" | "section" | "figure" | "aside" | "center" | "header" | "footer" | "nav" | "details" | "summary" | "figcaption");
            if keep {
                out.push('<');
                out.push_str(tag);
                if tag == "a" {
                    let href = match attr(attrs, "href").and_then(|h| link_of(&h, cx.site)) {
                        Some(Link::Page { title, anchor }) => format!("{PAGE}{title}{}", anchor.map(|a| format!("#{a}")).unwrap_or_default()),
                        Some(Link::Anchor(a)) => format!("{ANCHOR}{a}"),
                        Some(Link::Out(u)) => format!("{OUT}{u}"),
                        None => String::new(),
                    };
                    if !href.is_empty() {
                        out.push_str(" href=\"");
                        esc(&href, out);
                        out.push('"');
                    }
                }
                for n in ["colspan", "rowspan"] {
                    if let Some(v) = attr(attrs, n).filter(|v| v.len() <= 3 && v.chars().all(|c| c.is_ascii_digit())) {
                        out.push_str(&format!(" {n}=\"{v}\""));
                    }
                }
                out.push('>');
            } else if block {
                out.push_str("<div>");
            }
            // A wiki's own row of links to a page's parts (Blox Fruits' "Article Skins Combos") reads as one word
            // when drawn plain: the links are kept apart.
            let row = block && cls.iter().any(|c| !c.starts_with("wds") && (c.ends_with("tabber") || c.ends_with("tabs")));
            let mut first = true;
            for c in node.children.borrow().iter() {
                if row && matches!(c.data, NodeData::Element { .. }) {
                    if !first {
                        out.push_str(" · ");
                    }
                    first = false;
                }
                walk(c, out, cx);
            }
            if keep && !matches!(tag, "br" | "hr") {
                out.push_str(&format!("</{tag}>"));
            } else if block {
                out.push_str("</div>");
            }
        }
        _ => {
            for c in node.children.borrow().iter() {
                walk(c, out, cx);
            }
        }
    }
}

/// The page's HTML made safe and plain, its box of facts taken out.
pub fn clean_page(html: &str, site: &str) -> Clean {
    let dom: RcDom = parse_document(RcDom::default(), Default::default()).one(html);
    let mut cx = Ctx { site, infobox: None, images: Vec::new(), seen: HashSet::new() };
    let mut out = String::with_capacity(html.len() / 2);
    walk(&dom.document, &mut out, &mut cx);
    Clean { html: out, infobox: cx.infobox, images: cx.images }
}

/// A short HTML snippet's words (a search hit's snippet, a heading's line).
pub fn words_of(html: &str) -> String {
    let dom: RcDom = parse_document(RcDom::default(), Default::default()).one(html);
    text_of(&dom.document).replace('\n', " ")
}
