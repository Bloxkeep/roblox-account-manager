// Roblox mods, as Bloxstrap's mods folder has them: files laid over the Roblox client (a cursor, a death sound, a
// font, textures, a sky), kept here and put into every Roblox build before a window opens (core/roblox_mods.rs), so a
// Roblox update doesn't lose them. The library is GameBanana's Roblox section (game 2879), the place Bloxstrap's own
// pages send people for mods; its public API (apiv11) was tried 2026-11-14: Mod/Index with Generic_Game and
// Generic_Category filters, Util/Search/Results for words, Mod/<id>/DownloadPage for the files.
//
// Only what changes how Roblox looks or sounds is taken out of a mod: pictures, sounds, fonts and meshes under
// Roblox's own content folders. Nothing that runs (exe, dll, scripts) and no Fast Flags file is ever kept, and a file
// GameBanana's virus check didn't call clean is never downloaded.
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

const GAME: i64 = 2879;
/// The kinds shown, as GameBanana files Roblox's mods (its Fleasion, Places and Game Files are left out: those need
/// another tool, aren't mods of the client, or hold programs).
pub const KINDS: [(i64, &str); 4] = [(1670, "GUIs"), (598, "Textures"), (8780, "Skins"), (1114, "Effects")];
/// Roblox's folders a mod may lay files in, under a build (version-…\content\sounds\ouch.ogg).
const ROOTS: [&str; 3] = ["content", "PlatformContent", "ExtraContent"];
const KEEP: [&str; 13] = ["png", "jpg", "jpeg", "webp", "tga", "dds", "ktx", "ogg", "mp3", "wav", "ttf", "otf", "mesh"];
/// The biggest download taken (GameBanana's largest Roblox packs are about 80 MB).
const MOST: u64 = 300 * 1024 * 1024;

/// Where mods are kept: %LOCALAPPDATA%\Bloxkeep\mods, each in a folder of its id holding its files as they go into a
/// build (content\…), and mods.json, the list.
pub fn dir() -> PathBuf {
    crate::roblox::kept("mods")
}

/// A mod in the list: GameBanana's id and name, its picture, whether it is on, and its files (as under a build).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Installed {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub pic: String,
    #[serde(default)]
    pub kind: String,
    pub on: bool,
    pub files: Vec<String>,
    /// When it was put in (unix seconds).
    #[serde(default)]
    pub at: i64,
}

/// The list, in order: a later mod's file wins over an earlier one's.
pub fn installed() -> Vec<Installed> {
    crate::net::saved("Mods list", &dir().join("mods.json")).unwrap_or_default()
}

fn save(list: &[Installed]) -> Result<(), String> {
    std::fs::create_dir_all(dir()).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(list).map_err(|e| e.to_string())?;
    let temp = dir().join("mods.json.new");
    std::fs::write(&temp, text).and_then(|_| std::fs::rename(&temp, dir().join("mods.json"))).map_err(|e| e.to_string())
}

/// Turned on or off (put in or taken out of Roblox when a window next opens).
pub fn set_on(id: i64, on: bool) -> Result<(), String> {
    let mut list = installed();
    for m in list.iter_mut().filter(|m| m.id == id) {
        m.on = on;
    }
    save(&list)
}

/// The list in a new order (the same mods).
pub fn reorder(list: Vec<Installed>) -> Result<(), String> {
    save(&list)
}

/// Gone from the list and from the disk (Roblox gets its own files back when a window next opens).
pub fn remove(id: i64) -> Result<(), String> {
    let mut list = installed();
    list.retain(|m| m.id != id);
    save(&list)?;
    let _ = std::fs::remove_dir_all(dir().join(id.to_string()));
    Ok(())
}

/// The files of every mod on, by where they go under a build, and where each is kept (a later mod wins).
pub fn wanted() -> BTreeMap<String, PathBuf> {
    let mut out = BTreeMap::new();
    for m in installed().into_iter().filter(|m| m.on) {
        for f in m.files {
            let from = dir().join(m.id.to_string()).join(&f);
            if from.is_file() {
                out.insert(f.to_ascii_lowercase().replace('/', "\\"), from);
            }
        }
    }
    out
}

// ───────────────────────── the library ─────────────────────────

/// A mod in GameBanana's list.
#[derive(Clone, Debug, Default)]
pub struct Listed {
    pub id: i64,
    pub name: String,
    pub pic: String,
    pub kind: String,
    pub by: String,
    pub views: i64,
    pub updated: i64,
    /// When it was put up (unix seconds): All's order for Newest.
    pub added: i64,
}

fn agent() -> ureq::Agent {
    crate::net::builder().timeout(Duration::from_secs(30)).user_agent("Bloxkeep").build()
}

fn get(url: &str) -> Result<Value, String> {
    let out = (|| serde_json::from_str(&agent().get(url).call().map_err(|e| e.to_string())?.into_string().map_err(|e| e.to_string())?).map_err(|e| e.to_string()))();
    if let Err(why) = &out {
        crate::net::note_failed("Mods", url, why);
    }
    out
}

fn listed(r: &Value) -> Option<Listed> {
    // (Marked for grown-ups on GameBanana: not shown.)
    if r.get("_bHasContentRatings").and_then(Value::as_bool).unwrap_or(false) || r.get("_bIsObsolete").and_then(Value::as_bool).unwrap_or(false) {
        return None;
    }
    if r.pointer("/_aGame/_idRow").and_then(Value::as_i64).is_some_and(|g| g != GAME) {
        return None;
    }
    let kind = r.pointer("/_aRootCategory/_sName").and_then(Value::as_str).unwrap_or_default().to_string();
    if !kind.is_empty() && !KINDS.iter().any(|(_, k)| *k == kind) {
        return None;
    }
    let img = r.pointer("/_aPreviewMedia/_aImages/0");
    let pic = img
        .map(|i| {
            let base = i.get("_sBaseUrl").and_then(Value::as_str).unwrap_or_default();
            let file = i.get("_sFile530").or(i.get("_sFile")).and_then(Value::as_str).unwrap_or_default();
            if base.is_empty() || file.is_empty() { String::new() } else { format!("{base}/{file}") }
        })
        .unwrap_or_default();
    Some(Listed {
        id: r.get("_idRow")?.as_i64()?,
        name: r.get("_sName").and_then(Value::as_str).unwrap_or_default().trim().to_string(),
        pic,
        kind,
        by: r.pointer("/_aSubmitter/_sName").and_then(Value::as_str).unwrap_or_default().to_string(),
        views: r.get("_nViewCount").and_then(Value::as_i64).unwrap_or(0),
        updated: r.get("_tsDateUpdated").or(r.get("_tsDateModified")).or(r.get("_tsDateAdded")).and_then(Value::as_i64).unwrap_or(0),
        added: r.get("_tsDateAdded").and_then(Value::as_i64).unwrap_or(0),
    })
}

/// A page of the library: `kind` 0 for all, `sort` "popular", "new" or "updated", or `words` to search for (sort and
/// kind then left to GameBanana's best match). The mods, and whether there are more pages.
pub fn browse(kind: i64, sort: &str, words: &str, page: u32) -> Result<(Vec<Listed>, bool), String> {
    let url = if !words.trim().is_empty() {
        let q: String = url_word(words.trim());
        format!("https://gamebanana.com/apiv11/Util/Search/Results?_sModelName=Mod&_sOrder=best_match&_idGameRow={GAME}&_sSearchString={q}&_nPage={page}")
    } else if kind > 0 {
        return index(kind, sort, page, 30);
    } else {
        // All: each kind shown asked on its own and the four put together. Asked as one, GameBanana's newest Roblox
        // mods are mostly the kinds left out (Fleasion, Game Files: 27 of 30 on 2026-11-17), so a page showed 3.
        let got: Vec<Result<(Vec<Listed>, bool), String>> = crate::threads::scope(|s| {
            let asks: Vec<_> = KINDS.iter().map(|(k, _)| s.spawn(move || index(*k, sort, page, 15))).collect();
            asks.into_iter().map(|a| a.join().unwrap_or_else(|_| Err("stopped".into()))).collect()
        });
        if got.iter().all(Result::is_err) {
            return got.into_iter().next().unwrap_or_else(|| Err(String::new()));
        }
        let more = got.iter().any(|g| g.as_ref().is_ok_and(|(_, m)| *m));
        let lists: Vec<Vec<Listed>> = got.into_iter().filter_map(Result::ok).map(|(l, _)| l).collect();
        let mut all: Vec<Listed> = Vec::new();
        match sort {
            "new" | "updated" => {
                all = lists.into_iter().flatten().collect();
                if sort == "new" {
                    all.sort_by_key(|x| std::cmp::Reverse(x.added));
                } else {
                    all.sort_by_key(|x| std::cmp::Reverse(x.updated));
                }
            }
            // Most downloaded: GameBanana's lists don't carry the count, so each kind's order is kept, taken in turn.
            _ => {
                let longest = lists.iter().map(Vec::len).max().unwrap_or(0);
                for i in 0..longest {
                    all.extend(lists.iter().filter_map(|l| l.get(i).cloned()));
                }
            }
        }
        return Ok((all, more));
    };
    let d = get(&url)?;
    let more = !d.pointer("/_aMetadata/_bIsComplete").and_then(Value::as_bool).unwrap_or(true);
    let list = d.get("_aRecords").and_then(Value::as_array).into_iter().flatten().filter_map(listed).collect();
    Ok((list, more))
}

/// A page of one kind of mod in `sort`'s order, `per` to a page.
fn index(kind: i64, sort: &str, page: u32, per: u32) -> Result<(Vec<Listed>, bool), String> {
    let sort = match sort {
        "new" => "Generic_Newest",
        "updated" => "Generic_LatestUpdated",
        _ => "Generic_MostDownloaded",
    };
    let d = get(&format!("https://gamebanana.com/apiv11/Mod/Index?_nPage={page}&_nPerpage={per}&_aFilters%5BGeneric_Game%5D={GAME}&_aFilters%5BGeneric_Category%5D={kind}&_sSort={sort}"))?;
    let more = !d.pointer("/_aMetadata/_bIsComplete").and_then(Value::as_bool).unwrap_or(true);
    let list = d.get("_aRecords").and_then(Value::as_array).into_iter().flatten().filter_map(listed).collect();
    Ok((list, more))
}

fn url_word(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            b' ' => "+".into(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// A mod's own page on GameBanana: its pictures, what its maker wrote, who made it, its numbers and its files.
#[derive(Clone, Debug, Default)]
pub struct Profile {
    pub pics: Vec<String>,
    pub text: String,
    /// Each group of credits ("Creators", "Special Thanks") and the names in it with what each did.
    pub credits: Vec<(String, Vec<(String, String)>)>,
    pub likes: i64,
    pub views: i64,
    pub downloads: i64,
    pub added: i64,
    pub updated: i64,
    pub version: String,
    pub files: Vec<(String, u64)>,
    pub category: String,
}

/// The words of GameBanana's marked-up text: a line break where a line, paragraph, list item or heading ends, the tags
/// dropped, the common entities read, no more than one empty line in a row.
fn words_of(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut tag = String::new();
    let mut inside = false;
    for ch in html.chars() {
        match ch {
            '<' => {
                inside = true;
                tag.clear();
            }
            '>' if inside => {
                inside = false;
                let t = tag.trim_start_matches('/').split_whitespace().next().unwrap_or_default().to_ascii_lowercase();
                if matches!(t.as_str(), "br" | "br/" | "p" | "div" | "li" | "ul" | "ol" | "tr" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6") {
                    out.push('\n');
                }
            }
            _ if inside => tag.push(ch),
            _ => out.push(ch),
        }
    }
    let text = out.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'").replace("&nbsp;", " ");
    let mut lines: Vec<&str> = Vec::new();
    for line in text.lines().map(str::trim) {
        if line.is_empty() && lines.last().is_none_or(|l| l.is_empty()) {
            continue;
        }
        lines.push(line);
    }
    lines.join("\n").trim().to_string()
}

/// A mod's page (GameBanana's ProfilePage), for showing it inside Bloxkeep.
pub fn profile(id: i64) -> Result<Profile, String> {
    let d = get(&format!("https://gamebanana.com/apiv11/Mod/{id}/ProfilePage"))?;
    let text_of = |k: &str| d.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
    let int_of = |k: &str| d.get(k).and_then(Value::as_i64).unwrap_or(0);
    let pics = d
        .pointer("/_aPreviewMedia/_aImages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|i| {
            let base = i.get("_sBaseUrl").and_then(Value::as_str).filter(|b| !b.is_empty())?;
            let file = i.get("_sFile").and_then(Value::as_str).filter(|f| !f.is_empty())?;
            Some(format!("{base}/{file}"))
        })
        .take(12)
        .collect();
    let credits = d
        .get("_aCredits")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|g| {
            let names: Vec<(String, String)> = g
                .get("_aAuthors")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|a| Some((a.get("_sName").and_then(Value::as_str)?.to_string(), a.get("_sRole").and_then(Value::as_str).unwrap_or_default().to_string())))
                .collect();
            (!names.is_empty()).then(|| (g.get("_sGroupName").and_then(Value::as_str).unwrap_or_default().to_string(), names))
        })
        .collect();
    let files = d
        .get("_aFiles")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|f| Some((f.get("_sFile").and_then(Value::as_str)?.to_string(), f.get("_nFilesize").and_then(Value::as_u64).unwrap_or(0))))
        .collect();
    let written = words_of(&text_of("_sText"));
    Ok(Profile {
        pics,
        text: if written.is_empty() { text_of("_sDescription") } else { written },
        credits,
        likes: int_of("_nLikeCount"),
        views: int_of("_nViewCount"),
        downloads: int_of("_nDownloadCount"),
        added: int_of("_tsDateAdded"),
        updated: d.get("_tsDateUpdated").or(d.get("_tsDateModified")).and_then(Value::as_i64).unwrap_or(0),
        version: text_of("_sVersion"),
        files,
        category: d.pointer("/_aCategory/_sName").and_then(Value::as_str).unwrap_or_default().to_string(),
    })
}

/// One of a mod's downloads that could be put in: many mods offer a few versions to pick one of (six Wii cursors),
/// not parts to take together.
#[derive(Clone, Debug, Default)]
pub struct Choice {
    pub file: String,
    pub about: String,
    pub size: u64,
    url: String,
}

/// A mod's downloads Bloxkeep could take: an archive or a picture, sound or font, that GameBanana's virus check
/// called clean, not too big.
pub fn choices(id: i64) -> Result<Vec<Choice>, String> {
    let d = get(&format!("https://gamebanana.com/apiv11/Mod/{id}/DownloadPage"))?;
    Ok(d.get("_aFiles")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|f| {
            let file = f.get("_sFile").and_then(Value::as_str)?.to_string();
            let ext = Path::new(&file).extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
            let size = f.get("_nFilesize").and_then(Value::as_u64).unwrap_or(0);
            let ok = (matches!(ext.as_str(), "zip" | "rar" | "7z") || KEEP.contains(&ext.as_str())) && f.get("_sAvResult").and_then(Value::as_str) == Some("clean") && size <= MOST;
            ok.then(|| Choice {
                about: f.get("_sDescription").and_then(Value::as_str).unwrap_or_default().trim().to_string(),
                url: f.get("_sDownloadUrl").and_then(Value::as_str).unwrap_or_default().to_string(),
                file,
                size,
            })
        })
        .collect())
}

/// Roblox's own files by name (lower case) and where each is under a build, from the newest Roblox build here: a mod
/// that holds only "ArrowCursor.png", with no folders, is put where Roblox keeps its file of that name.
fn roblox_index() -> std::collections::HashMap<String, Vec<String>> {
    let mut out: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    let versions = PathBuf::from(std::env::var("LOCALAPPDATA").unwrap_or_default()).join("Roblox").join("Versions");
    let newest = std::fs::read_dir(&versions)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().join("RobloxPlayerBeta.exe").is_file())
        .max_by_key(|e| e.metadata().and_then(|m| m.modified()).ok());
    let Some(build) = newest.map(|e| e.path()) else { return out };
    for root in ROOTS {
        for (_, rel) in walk(&build.join(root)) {
            let name = rel.rsplit('/').next().unwrap_or_default().to_ascii_lowercase();
            out.entry(name).or_default().push(format!("{root}\\{}", rel.replace('/', "\\")));
        }
    }
    out
}

/// Where a file with no Roblox folder around it goes: Roblox's one file of that name, or of several the one in a
/// folder of the same name as the mod's; else a few well-known ones when no Roblox build is here to look in.
fn guess(rel: &str, index: &std::collections::HashMap<String, Vec<String>>) -> Option<String> {
    let parts: Vec<&str> = rel.split('/').collect();
    let name = parts.last()?.to_ascii_lowercase();
    let ext = name.rsplit('.').next().unwrap_or_default();
    if !KEEP.contains(&ext) {
        return None;
    }
    if index.is_empty() {
        return match name.as_str() {
            "ouch.ogg" => Some("content\\sounds\\ouch.ogg".into()),
            "arrowcursor.png" | "arrowfarcursor.png" | "ibeamcursor.png" => Some(format!("content\\textures\\Cursors\\KeyboardMouse\\{}", parts.last()?)),
            _ => None,
        };
    }
    let found = index.get(&name)?;
    if found.len() == 1 {
        return Some(found[0].clone());
    }
    let folder = parts.len().checked_sub(2).map(|i| parts[i].to_ascii_lowercase())?;
    let near: Vec<&String> = found.iter().filter(|p| p.to_ascii_lowercase().rsplit('\\').nth(1) == Some(folder.as_str())).collect();
    (near.len() == 1).then(|| near[0].clone())
}

/// Downloads `pick` of a mod's downloads (`choices`), takes out what can go into Roblox and keeps it, on. Its name and
/// picture are kept for the list. Err("none") when nothing in it is a file Bloxkeep puts in (a mod for another tool,
/// or only Fast Flags).
pub fn install(m: &Listed, pick: &Choice) -> Result<usize, String> {
    let files = [pick];
    let work = std::env::temp_dir().join(format!("bloxkeep-mod-{}", m.id));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let out = dir().join(m.id.to_string());
    let _ = std::fs::remove_dir_all(&out);
    let mut kept: Vec<String> = Vec::new();
    let mut tried = 0;
    let index = roblox_index();
    for (n, f) in files.iter().enumerate() {
        let name = f.file.clone();
        let ext = Path::new(&name).extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
        let archive = matches!(ext.as_str(), "zip" | "rar" | "7z");
        if !archive && !KEEP.contains(&ext.as_str()) {
            continue;
        }
        let url = f.url.as_str();
        if url.is_empty() {
            continue;
        }
        tried += 1;
        let got = work.join(format!("{n}.{ext}"));
        download(url, &got)?;
        let base = if archive {
            let into = work.join(format!("x{n}"));
            std::fs::create_dir_all(&into).map_err(|e| e.to_string())?;
            unpack(&got, &into)?;
            into
        } else {
            // A loose file: where it goes is known from its name only.
            if let Some(to) = guess(&name, &index) {
                copy_in(&got, &out, &to, &mut kept)?;
            }
            continue;
        };
        for (path, rel) in walk(&base) {
            if let Some(to) = place_of(&rel).or_else(|| guess(&rel, &index)) {
                copy_in(&path, &out, &to, &mut kept)?;
            }
        }
    }
    let _ = std::fs::remove_dir_all(&work);
    if kept.is_empty() {
        let _ = std::fs::remove_dir_all(&out);
        return Err(if tried == 0 { "nofile".into() } else { "none".into() });
    }
    kept.sort();
    kept.dedup();
    let n = kept.len();
    let mut list = installed();
    list.retain(|x| x.id != m.id);
    list.push(Installed { id: m.id, name: m.name.clone(), pic: m.pic.clone(), kind: m.kind.clone(), on: true, files: kept, at: crate::time::now_secs() });
    save(&list)?;
    Ok(n)
}

/// A mod the user has on this PC (a .zip, .rar or .7z, or one picture, sound, font or mesh) put in as GameBanana's
/// are: its files placed by Roblox's own folder names, or by name against the Roblox build here. Kept under a
/// negative id, so it never meets a GameBanana one. Returns its name and how many files went in.
pub fn import(path: &Path) -> Result<(String, usize), String> {
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if !meta.is_file() {
        return Err("nofile".into());
    }
    if meta.len() > MOST {
        return Err("big".into());
    }
    let file = path.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
    let name = path.file_stem().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
    let archive = matches!(ext.as_str(), "zip" | "rar" | "7z");
    if !archive && !KEEP.contains(&ext.as_str()) {
        return Err("kind".into());
    }
    let mut list = installed();
    let mut id = -crate::time::now_secs().max(1);
    while list.iter().any(|m| m.id == id) {
        id -= 1;
    }
    let out = dir().join(id.to_string());
    let _ = std::fs::remove_dir_all(&out);
    let mut kept: Vec<String> = Vec::new();
    let index = roblox_index();
    if archive {
        let work = std::env::temp_dir().join(format!("bloxkeep-mod{id}"));
        let _ = std::fs::remove_dir_all(&work);
        std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
        let done = unpack(path, &work).and_then(|_| {
            for (p, rel) in walk(&work) {
                if let Some(to) = place_of(&rel).or_else(|| guess(&rel, &index)) {
                    copy_in(&p, &out, &to, &mut kept)?;
                }
            }
            Ok(())
        });
        let _ = std::fs::remove_dir_all(&work);
        done?;
    } else if let Some(to) = guess(&file, &index) {
        copy_in(path, &out, &to, &mut kept)?;
    }
    if kept.is_empty() {
        let _ = std::fs::remove_dir_all(&out);
        return Err("none".into());
    }
    kept.sort();
    kept.dedup();
    let n = kept.len();
    list.push(Installed { id, name: name.clone(), pic: String::new(), kind: "local".into(), on: true, files: kept, at: crate::time::now_secs() });
    save(&list)?;
    Ok((name, n))
}

fn download(url: &str, to: &Path) -> Result<(), String> {
    let out = download_once(url, to);
    if let Err(why) = &out {
        crate::net::note_failed("Mod download", url, why);
    }
    out
}

fn download_once(url: &str, to: &Path) -> Result<(), String> {
    let res = agent().get(url).timeout(Duration::from_secs(300)).call().map_err(|e| e.to_string())?;
    let mut body = Vec::new();
    res.into_reader().take(MOST + 1).read_to_end(&mut body).map_err(|e| e.to_string())?;
    if body.len() as u64 > MOST {
        return Err("big".into());
    }
    std::fs::write(to, body).map_err(|e| e.to_string())
}

/// Windows' own tar (libarchive: zip, 7z and rar), out of sight.
fn unpack(file: &Path, into: &Path) -> Result<(), String> {
    let tar = PathBuf::from(std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into())).join("System32").join("tar.exe");
    let mut cmd = std::process::Command::new(tar);
    cmd.arg("-xf").arg(file).arg("-C").arg(into);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let out = cmd.output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err("unpack".into());
    }
    Ok(())
}

/// Every file under `base`, with its path from there ("/"-joined).
fn walk(base: &Path) -> Vec<(PathBuf, String)> {
    let mut out = Vec::new();
    let mut todo = vec![base.to_path_buf()];
    while let Some(d) = todo.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            // (Never followed out of the folder.)
            if e.file_type().is_ok_and(|t| t.is_symlink()) {
                continue;
            }
            if p.is_dir() {
                todo.push(p);
            } else if let Ok(rel) = p.strip_prefix(base) {
                out.push((p.clone(), rel.to_string_lossy().replace('\\', "/")));
            }
        }
    }
    out
}

/// Where a file of a mod goes under a build: from Roblox's own folder name on (a mod's own top folders dropped), only
/// a picture, sound, font or mesh, and a font family's list (content\fonts\families\*.json). None for anything else.
fn place_of(rel: &str) -> Option<String> {
    let parts: Vec<&str> = rel.split('/').filter(|p| !p.is_empty()).collect();
    let at = parts.iter().position(|p| ROOTS.iter().any(|r| r.eq_ignore_ascii_case(p)))?;
    let rest = &parts[at..];
    if rest.len() < 2 || rest.iter().any(|p| *p == ".." || p.contains(':')) {
        return None;
    }
    let file = rest.last()?.to_ascii_lowercase();
    let ext = file.rsplit('.').next().unwrap_or_default();
    let family = ext == "json" && rest.len() >= 3 && rest[rest.len() - 2].eq_ignore_ascii_case("families");
    if !KEEP.contains(&ext) && !family {
        return None;
    }
    let root = ROOTS.iter().find(|r| r.eq_ignore_ascii_case(rest[0]))?;
    Some(std::iter::once(*root).chain(rest[1..].iter().copied()).collect::<Vec<_>>().join("\\"))
}

fn copy_in(from: &Path, out: &Path, to: &str, kept: &mut Vec<String>) -> Result<(), String> {
    let dest = out.join(to);
    if let Some(p) = dest.parent() {
        std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
    }
    std::fs::copy(from, &dest).map_err(|e| e.to_string())?;
    kept.push(to.to_string());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{guess, place_of};

    #[test]
    fn places() {
        assert_eq!(place_of("MyMod/content/sounds/ouch.ogg").as_deref(), Some("content\\sounds\\ouch.ogg"));
        assert_eq!(place_of("a/b/PlatformContent/pc/textures/sky/sky512_bk.tex.png").as_deref(), Some("PlatformContent\\pc\\textures\\sky\\sky512_bk.tex.png"));
        assert_eq!(place_of("content/fonts/families/BuilderSans.json").as_deref(), Some("content\\fonts\\families\\BuilderSans.json"));
        assert_eq!(place_of("content/run.exe"), None);
        assert_eq!(place_of("ClientSettings/ClientAppSettings.json"), None);
        assert_eq!(place_of("readme.txt"), None);
        assert_eq!(place_of("content/../x.png"), None);
    }

    #[test]
    fn loose_files() {
        let bs = |p: &str| p.replace('/', "\\");
        let mut index = std::collections::HashMap::new();
        index.insert("arrowcursor.png".to_string(), vec![bs("content/textures/Cursors/KeyboardMouse/ArrowCursor.png")]);
        index.insert("ouch.ogg".to_string(), vec![bs("content/sounds/ouch.ogg")]);
        index.insert("cursor.png".to_string(), vec![bs("content/textures/Cursors/KeyboardMouse/Cursor.png"), bs("content/textures/Cursors/Gamepad/Cursor.png")]);
        // One file of that name in Roblox: there.
        assert_eq!(guess("Creeper cursor (MODERN)/ArrowCursor.png", &index), Some(bs("content/textures/Cursors/KeyboardMouse/ArrowCursor.png")));
        // Several: the one in a folder of the same name, else none.
        assert_eq!(guess("pack/Gamepad/Cursor.png", &index), Some(bs("content/textures/Cursors/Gamepad/Cursor.png")));
        assert_eq!(guess("pack/Cursor.png", &index), None);
        // Not a kind of file Bloxkeep puts in, or not a Roblox file.
        assert_eq!(guess("readme.txt", &index), None);
        assert_eq!(guess("mine/ArrowCursor2.png", &index), None);
        // No Roblox build to look in: the well-known names only.
        let none = std::collections::HashMap::new();
        assert_eq!(guess("x/ouch.ogg", &none), Some(bs("content/sounds/ouch.ogg")));
        assert_eq!(guess("x/sky.png", &none), None);
    }
}
