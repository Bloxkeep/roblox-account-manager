// AppController.CustomTheme.cs: Settings › Appearance › Custom (Prefs.Custom): the person's background colour, accent and
// optional picture or video (MP4, WebM) behind glass; the page works out the other colours (app.js customVars). The
// picture is copied into the data folder and served from there (AppPages.ServeAsset, "user/…"), so the original can go.
// Kept by name (Prefs.SavedThemes) and shared as a .bloxkeeptheme file: a ZIP of theme.json and its picture.
use std::collections::HashSet;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use serde_json::{Value, json};

use super::App;
use super::settings::{CSIDL_MYDOCUMENTS, CSIDL_MYPICTURES, open_file_dialog, save_file_dialog, special_folder};
use crate::core::base::{Fail, Res, new_guid};
use crate::core::models::to_net_json_string;
use crate::core::prefs::{self, CustomTheme, SavedTheme};
use crate::core::{l, log, themes, vault};

/// Bigger than this is refused: the page holds the whole picture (or video) while it shows.
pub const MAX_THEME_PICTURE: u64 = 20 * 1024 * 1024;
pub const MAX_THEME_VIDEO: u64 = 50 * 1024 * 1024;

pub const PICTURE_TYPES: &[&str] = &[".png", ".jpg", ".jpeg", ".webp", ".gif", ".bmp"];
pub const VIDEO_TYPES: &[&str] = &[".mp4", ".webm"];

/// A theme file's own name ending, and the most its theme.json may hold.
pub const THEME_FILE_TYPE: &str = ".bloxkeeptheme";
pub const MAX_THEME_JSON: usize = 16 * 1024;

/// AppController.CustomTheme.cs declares no instance fields.
#[derive(Default)]
pub struct CustomThemeState {}

/// Path.GetExtension, lower-cased (".png"), "" when there is none.
pub fn extension_lower(path: &str) -> String {
    let name = path.rsplit(['\\', '/']).next().unwrap_or(path);
    match name.rfind('.') {
        Some(dot) => name[dot..].to_lowercase(),
        None => String::new(),
    }
}

/// Path.GetFileName.
pub fn file_name(path: &str) -> String {
    path.rsplit(['\\', '/']).next().unwrap_or(path).to_string()
}

/// DateTime.UtcNow.Ticks: 100 ns since 0001-01-01.
fn utc_ticks() -> i64 {
    let d = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    d.as_secs() as i64 * 10_000_000 + (d.subsec_nanos() / 100) as i64 + 621_355_968_000_000_000
}

pub fn look_dto(look: &CustomTheme) -> Value {
    json!({
        "background": look.background,
        "accent": look.accent,
        "fade": look.fade,
        "blur": look.blur,
        "picture": look.picture.as_ref().map(|p| format!("user/{p}")),
        "video": themes::is_video(look.picture.as_deref()),
    })
}

fn not_theme() -> Fail {
    Fail::user(l::t("That file isn't a Bloxkeep theme."))
}

/// A picture or video of a kind and size the page can show, or a message saying why not.
pub fn check_theme_media(kind: &str, length: u64) -> Res<()> {
    let video = VIDEO_TYPES.contains(&kind);
    if !video && !PICTURE_TYPES.contains(&kind) {
        return Err(Fail::user(l::t("Pick a picture (PNG, JPG, WebP, GIF, BMP) or a video (MP4, WebM).")));
    }
    if length > (if video { MAX_THEME_VIDEO } else { MAX_THEME_PICTURE }) {
        return Err(Fail::user(if video { l::t("That video is over 50 MB. Pick a smaller one.") } else { l::t("That picture is over 20 MB. Pick a smaller one.") }));
    }
    Ok(())
}

/// A new name each time, so the page loads the new picture rather than the one it already shows.
pub fn new_media_name(kind: &str) -> Res<String> {
    let dir = vault::pictures_dir();
    std::fs::create_dir_all(&dir)?;
    let mut ticks = utc_ticks();
    while dir.join(format!("theme-background-{ticks}{kind}")).exists() {
        ticks += 1;
    }
    Ok(format!("theme-background-{ticks}{kind}"))
}

/// A theme's file: theme.json (its name and look) and its picture or video, if it has one.
pub fn write_theme_file(theme: &SavedTheme, path: &str) -> Res<()> {
    let pictures = vault::pictures_dir();
    let media = theme.look.picture.clone().filter(|p| pictures.join(p).exists());
    let in_file = media.as_ref().map(|m| format!("background{}", extension_lower(m)));
    let temp = format!("{path}.tmp");
    {
        let mut zip = zip::ZipWriter::new(File::create(&temp)?);
        let json = json!({
            "bloxkeepTheme": 1, "name": theme.name, "background": theme.look.background, "accent": theme.look.accent,
            "fade": theme.look.fade, "blur": theme.look.blur, "picture": in_file,
        });
        // JsonSerializer.Serialize of the anonymous object: its names as written, in this order.
        let text = to_net_json_string(&ordered_theme_json(&json))?;
        zip.start_file("theme.json", zip::write::SimpleFileOptions::default()).map_err(Fail::other)?;
        zip.write_all(text.as_bytes())?;
        if let (Some(media), Some(in_file)) = (&media, &in_file) {
            zip.start_file(in_file.as_str(), zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored)).map_err(Fail::other)?;
            let mut from = File::open(pictures.join(media))?;
            std::io::copy(&mut from, &mut zip)?;
        }
        zip.finish().map_err(Fail::other)?;
    }
    std::fs::rename(&temp, path)?;
    Ok(())
}

/// serde_json's map is sorted; the C# wrote the anonymous object's members in their order.
fn ordered_theme_json(v: &Value) -> OrderedTheme {
    OrderedTheme(v.clone())
}

struct OrderedTheme(Value);

impl serde::Serialize for OrderedTheme {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        const ORDER: [&str; 7] = ["bloxkeepTheme", "name", "background", "accent", "fade", "blur", "picture"];
        let mut map = s.serialize_map(Some(ORDER.len()))?;
        for key in ORDER {
            map.serialize_entry(key, &self.0[key])?;
        }
        map.end()
    }
}

/// A theme file, maybe someone else's, so nothing in it is trusted: colours checked, numbers kept in range, name
/// cleaned; the picture only of a kind and size the page shows, copied out under a name of Bloxkeep's own (never a path
/// from the file), read no further than that size whatever the file claims.
pub fn read_theme_file(path: &str) -> Res<SavedTheme> {
    if std::fs::metadata(path)?.len() > MAX_THEME_VIDEO + MAX_THEME_JSON as u64 + 64 * 1024 {
        return Err(not_theme());
    }
    let mut zip = match zip::ZipArchive::new(File::open(path)?) {
        Ok(z) => z,
        Err(zip::result::ZipError::Io(e)) => return Err(e.into()),
        Err(_) => return Err(not_theme()),
    };
    if zip.len() > 4 {
        return Err(not_theme());
    }
    let root: Value = {
        let mut entry = match zip.by_name("theme.json") {
            Ok(e) => e,
            Err(_) => return Err(not_theme()),
        };
        if entry.size() > MAX_THEME_JSON as u64 {
            return Err(not_theme());
        }
        let mut bytes = Vec::new();
        if (&mut entry).take(MAX_THEME_JSON as u64 * 4 + 4).read_to_end(&mut bytes).is_err() {
            return Err(not_theme());
        }
        let text = String::from_utf8_lossy(&bytes);
        let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
        if text.encode_utf16().count() > MAX_THEME_JSON {
            return Err(not_theme());
        }
        serde_json::from_str(text).map_err(|_| not_theme())?
    };
    let Some(obj) = root.as_object().filter(|o| o.contains_key("bloxkeepTheme")) else {
        return Err(not_theme());
    };
    let text = |name: &str| obj.get(name).and_then(Value::as_str).map(str::to_string);
    let number = |name: &str| obj.get(name).and_then(Value::as_i64).and_then(|n| i32::try_from(n).ok());
    let mut media: Option<String> = None;
    // Only a file at the top of the ZIP, by a name with no folder in it.
    if let Some(picture) = text("picture").filter(|p| !p.contains(['/', '\\', ':']))
        && let Ok(mut file) = zip.by_name(&picture)
    {
        let kind = extension_lower(&picture);
        check_theme_media(&kind, file.size())?;
        let limit = if VIDEO_TYPES.contains(&kind.as_str()) { MAX_THEME_VIDEO } else { MAX_THEME_PICTURE };
        let name = new_media_name(&kind)?;
        let to = vault::pictures_dir().join(&name);
        let copied: Res<()> = (|| {
            let mut output = File::create(&to)?;
            let mut buffer = vec![0u8; 81920];
            let mut total: u64 = 0;
            loop {
                let n = file.read(&mut buffer).map_err(|_| not_theme())?;
                if n == 0 {
                    break;
                }
                total += n as u64;
                if total > limit {
                    return Err(not_theme());
                }
                output.write_all(&buffer[..n])?;
            }
            Ok(())
        })();
        if let Err(e) = copied {
            let _ = std::fs::remove_file(&to);
            return Err(match e {
                Fail::User(_) => e,
                _ => not_theme(),
            });
        }
        media = Some(name);
    }
    let custom = prefs::lock().custom.clone();
    let look = CustomTheme {
        background: themes::hex(text("background").as_deref()).unwrap_or(custom.background),
        accent: themes::hex(text("accent").as_deref()).unwrap_or(custom.accent),
        fade: number("fade").unwrap_or(45).clamp(0, 90),
        picture: media,
        blur: number("blur").unwrap_or(0).clamp(0, themes::MAX_BLUR),
    };
    let stem = Path::new(path).file_stem().map(|s| s.to_string_lossy().into_owned());
    let name = themes::theme_name(text("name").as_deref()).or_else(|| themes::theme_name(stem.as_deref())).unwrap_or_else(|| l::t("Theme"));
    Ok(SavedTheme { id: new_guid().simple().to_string(), name, look })
}

/// The theme pictures (and videos) neither the theme in use nor a kept one shows any more, deleted.
pub fn delete_theme_pictures() {
    let used: HashSet<String> = {
        let p = prefs::lock();
        p.saved_themes.iter().filter_map(|t| t.look.picture.clone()).chain(p.custom.picture.clone()).map(|n| n.to_lowercase()).collect()
    };
    let result: std::io::Result<()> = (|| {
        // The top of the data folder too: one Vault.Tidy couldn't move yet.
        let mut files = Vec::new();
        for dir in [vault::pictures_dir(), vault::data_dir().to_path_buf()] {
            if !dir.is_dir() {
                continue;
            }
            for entry in std::fs::read_dir(&dir)? {
                let entry = entry?;
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.to_lowercase().starts_with("theme-background-") && entry.file_type()?.is_file() {
                    files.push((entry.path(), name));
                }
            }
        }
        for (file, name) in files {
            if !used.contains(&name.to_lowercase()) && themes::is_theme_media_name(&name) {
                std::fs::remove_file(file)?;
            }
        }
        Ok(())
    })();
    if let Err(e) = result {
        log::write(&e); // an old picture left behind: replaced or removed next time
    }
}

impl App {
    pub fn custom_theme_dto() -> Value {
        let custom = prefs::lock().custom.clone();
        look_dto(&custom)
    }

    pub fn saved_themes_dto() -> Value {
        let saved = prefs::lock().saved_themes.clone();
        Value::Array(saved.iter().map(|t| json!({ "id": t.id, "name": t.name, "look": look_dto(&t.look) })).collect())
    }

    /// New colours, fade or blur; the theme becomes the custom one.
    pub fn set_custom_theme(&self, background: &str, accent: &str, fade: Option<i32>, blur: Option<i32>) -> Value {
        {
            let mut p = prefs::lock();
            let c = p.custom.clone();
            p.custom = CustomTheme {
                background: themes::hex(Some(background)).unwrap_or(c.background),
                accent: themes::hex(Some(accent)).unwrap_or(c.accent),
                fade: fade.map(|f| f.clamp(0, 90)).unwrap_or(c.fade),
                blur: blur.map(|b| b.clamp(0, themes::MAX_BLUR)).unwrap_or(c.blur),
                picture: c.picture,
            };
        }
        self.use_custom_theme()
    }

    /// "Choose a picture or video…": one from the PC, copied into the data folder, behind the page from now on.
    /// The box waits on a worker thread (as tray.rs's), so the core keeps answering while it is open.
    pub async fn pick_theme_picture(&self) -> Res<Value> {
        let owner = self.controller.owner.get();
        let title = l::t("Choose a background picture or video");
        let filter = l::t("Pictures and videos")
            + " (*.png;*.jpg;*.jpeg;*.webp;*.gif;*.bmp;*.mp4;*.webm)|*.png;*.jpg;*.jpeg;*.webp;*.gif;*.bmp;*.mp4;*.webm";
        let initial = special_folder(CSIDL_MYPICTURES);
        let Some(chosen) = crate::core::rt::blocking(move || crate::core::ui::modal(|| open_file_dialog(owner, &title, &filter, &initial))).await else {
            return Ok(Value::Null);
        };
        let kind = extension_lower(&chosen);
        check_theme_media(&kind, std::fs::metadata(&chosen)?.len())?;
        let name = new_media_name(&kind)?;
        // File.Copy without overwrite: the name is new.
        let to = vault::pictures_dir().join(&name);
        if to.exists() {
            return Err(Fail::other(format!("The file '{}' already exists.", to.display())));
        }
        std::fs::copy(&chosen, &to)?;
        prefs::lock().custom.picture = Some(name);
        delete_theme_pictures();
        Ok(self.use_custom_theme())
    }

    pub fn clear_theme_picture(&self) -> Value {
        prefs::lock().custom.picture = None;
        delete_theme_pictures();
        self.use_custom_theme()
    }

    /// "Save": the custom theme kept under a name; the same name replaces that theme, a new one goes first. At
    /// most `themes::MAX_SAVED`.
    pub fn save_theme(&self, name: &str) -> Res<Value> {
        let named = themes::theme_name(Some(name)).ok_or_else(|| Fail::user(l::t("Give the theme a name.")))?;
        {
            let mut p = prefs::lock();
            let custom = p.custom.clone();
            let lower = named.to_lowercase();
            if let Some(same) = p.saved_themes.iter().position(|t| t.name.to_lowercase() == lower) {
                let t = &mut p.saved_themes[same];
                t.name = named.clone();
                t.look = custom;
            } else if p.saved_themes.len() >= themes::MAX_SAVED {
                drop(p);
                return Err(Fail::user(l::f("You can keep up to {0} themes. Delete one first.", &[&themes::MAX_SAVED])));
            } else {
                p.saved_themes.insert(0, SavedTheme { id: new_guid().simple().to_string(), name: named.clone(), look: custom });
            }
        }
        prefs::save();
        self.push_state();
        Ok(Value::Null)
    }

    pub fn use_saved_theme(&self, id: &str) -> Res<Value> {
        let theme = prefs::lock().saved_themes.iter().find(|t| t.id == id).cloned();
        let theme = theme.ok_or_else(|| Fail::user(l::t("That theme isn't there any more.")))?;
        prefs::lock().custom = theme.look;
        delete_theme_pictures();
        Ok(self.use_custom_theme())
    }

    /// A kept theme deleted, with its picture unless the theme in use (or another kept one) shows it too.
    pub fn delete_saved_theme(&self, id: &str) -> Value {
        prefs::lock().saved_themes.retain(|t| t.id != id);
        delete_theme_pictures();
        prefs::save();
        self.push_state();
        Value::Null
    }

    pub async fn export_theme(&self, id: &str) -> Res<Value> {
        let theme = prefs::lock().saved_themes.iter().find(|t| t.id == id).cloned();
        let theme = theme.ok_or_else(|| Fail::user(l::t("That theme isn't there any more.")))?;
        let owner = self.controller.owner.get();
        let title = l::t("Save the theme as a file");
        let filter = l::t("Bloxkeep theme") + &format!(" (*{THEME_FILE_TYPE})|*{THEME_FILE_TYPE}");
        // Path.GetInvalidFileNameChars: '"' '<' '>' '|' ':' '*' '?' '\\' '/' and the control characters.
        let file: String = theme.name.chars().map(|ch| if matches!(ch, '"' | '<' | '>' | '|' | ':' | '*' | '?' | '\\' | '/') || (ch as u32) < 32 { '-' } else { ch }).collect();
        let file = file + THEME_FILE_TYPE;
        let initial = special_folder(CSIDL_MYDOCUMENTS);
        let Some(chosen) = crate::core::rt::blocking(move || crate::core::ui::modal(|| save_file_dialog(owner, &title, &filter, &file, &initial))).await else {
            return Ok(Value::Null);
        };
        write_theme_file(&theme, &chosen)?;
        Ok(json!({ "saved": file_name(&chosen) }))
    }

    pub async fn import_theme(&self) -> Res<Value> {
        if prefs::lock().saved_themes.len() >= themes::MAX_SAVED {
            return Err(Fail::user(l::f("You can keep up to {0} themes. Delete one first.", &[&themes::MAX_SAVED])));
        }
        let owner = self.controller.owner.get();
        let title = l::t("Add a theme from a file");
        let filter = l::t("Bloxkeep theme") + &format!(" (*{THEME_FILE_TYPE})|*{THEME_FILE_TYPE}");
        let initial = special_folder(CSIDL_MYDOCUMENTS);
        let Some(chosen) = crate::core::rt::blocking(move || crate::core::ui::modal(|| open_file_dialog(owner, &title, &filter, &initial))).await else {
            return Ok(Value::Null);
        };
        self.add_theme_file(&chosen)
    }

    pub fn add_theme_file(&self, path: &str) -> Res<Value> {
        let theme = read_theme_file(path)?;
        {
            let mut p = prefs::lock();
            // Asked again here: a theme may have been saved while Import's box was open (the core keeps running).
            if p.saved_themes.len() >= themes::MAX_SAVED {
                return Err(Fail::user(l::f("You can keep up to {0} themes. Delete one first.", &[&themes::MAX_SAVED])));
            }
            p.saved_themes.insert(0, theme.clone());
            p.custom = theme.look;
        }
        delete_theme_pictures();
        Ok(self.use_custom_theme())
    }

    pub fn use_custom_theme(&self) -> Value {
        prefs::lock().theme = themes::CUSTOM.to_string();
        prefs::save();
        let changed = self.controller.appearance_changed.borrow().clone();
        if let Some(changed) = changed {
            changed();
        }
        self.push_state();
        Value::Null
    }
}
