// Tests/Bloxkeep.Tests/CustomThemeSuite.cs: the custom theme (Settings › Appearance, app/custom_theme.rs): colours
// checked and kept, the window's own colours dark or light by the background, its picture served only under its own kind
// of name (never another file of the data folder), and the choice surviving a restart. A video can be the picture,
// either blurred; custom themes kept by name (Your themes) keep their pictures; a theme shared as a file comes back the
// same, and a file from someone else is trusted in nothing (its picture only of a kind and size the page shows, under a
// name of Bloxkeep's own).
use std::io::Write;
use std::path::Path;

use serde_json::json;

use super::rejoin::quiet_app;
use super::{Checks, PASSWORD, call, unlock};
use crate::core::app::App;
use crate::core::app::custom_theme::{check_theme_media, write_theme_file};
use crate::core::base::{Fail, Res};
use crate::core::prefs::{self, CustomTheme};
use crate::core::themes::{self, Color};
use crate::core::vault::{self, Vault};

fn is_user<T>(r: &Res<T>) -> bool {
    matches!(r, Err(Fail::User(_)))
}

fn zip(dir: &str, name: &str, json: &str, files: &[(&str, &[u8])]) -> String {
    let path = Path::new(dir).join(name);
    let _ = std::fs::remove_file(&path);
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).expect("a file"));
    zip.start_file("theme.json", zip::write::SimpleFileOptions::default()).expect("an entry");
    zip.write_all(json.as_bytes()).expect("written");
    for (entry, data) in files {
        zip.start_file(*entry, zip::write::SimpleFileOptions::default()).expect("an entry");
        zip.write_all(data).expect("written");
    }
    zip.finish().expect("closed");
    path.to_string_lossy().into_owned()
}

fn count_files(dir: &Path) -> usize {
    std::fs::read_dir(dir).map(|d| d.flatten().filter(|e| e.path().is_file()).count()).unwrap_or(0)
}

pub async fn run(c: &mut Checks, dir: &str) {
    c.check(
        "the custom theme is a theme of its own",
        themes::normalize(Some("custom")) == "custom" && themes::normalize(Some("nonsense")) == themes::DEFAULT,
        (),
    );
    c.check(
        "colours are #rrggbb, kept in lower case; anything else is refused",
        themes::hex(Some("#A1B2C3")).as_deref() == Some("#a1b2c3")
            && themes::hex(Some("red")).is_none()
            && themes::hex(Some("#12345g")).is_none()
            && themes::hex(Some("#1234567")).is_none()
            && themes::hex(None).is_none(),
        (),
    );
    c.check(
        "only a picture of its own name is served from the data folder",
        themes::is_picture_name("theme-background-638700000000000000.png")
            && themes::is_picture_name("theme-background-1.jpeg")
            && !themes::is_picture_name("vault.rmv")
            && !themes::is_picture_name("theme-background-1.exe")
            && !themes::is_picture_name("theme-background-1.png/../vault.rmv")
            && !themes::is_picture_name("../theme-background-1.png"),
        (),
    );
    c.check(
        "…a video (MP4, WebM) as the custom theme's, never as a vault's picture",
        themes::is_picture_name("theme-background-1.mp4")
            && themes::is_theme_media_name("theme-background-1.webm")
            && themes::is_video(Some("theme-background-1.webm"))
            && !themes::is_picture_name("vault-picture-0123abcd-1.mp4")
            && !themes::is_theme_media_name("vault-picture-0123abcd-1.png")
            && !themes::is_video(Some("theme-background-1.png")),
        (),
    );
    c.check(
        "a theme's name: control characters gone, trimmed, at most 40 characters, none when empty",
        themes::theme_name(Some("  Night\u{0007} drive \n")).as_deref() == Some("Night drive")
            && themes::theme_name(Some(&"x".repeat(60))).is_some_and(|n| n.encode_utf16().count() == 40)
            && themes::theme_name(Some(" \t ")).is_none(),
        (),
    );

    Vault::create(PASSWORD, None).expect("a vault").save().expect("saved");
    let (app, _said) = quiet_app();
    unlock(&app).await;
    let custom = || prefs::lock().custom.clone();
    let theme = || prefs::lock().theme.clone();
    let saved = || prefs::lock().saved_themes.clone();

    let _ = call(&app, "setCustomTheme", json!({ "background": "#F1E9DA", "accent": "#f59e0b", "fade": 120 })).await;
    let now = custom();
    c.check(
        "new colours make the custom theme the one in use, the fade kept within 0 to 90",
        theme() == "custom" && now.background == "#f1e9da" && now.accent == "#f59e0b" && now.fade == 90,
        &now,
    );
    let light = themes::current();
    c.check(
        "on a light background the window and title bar are light",
        light.id == "custom" && !light.dark && light.background == Color { r: 0xf1, g: 0xe9, b: 0xda },
        &light,
    );

    let _ = call(&app, "setCustomTheme", json!({ "background": "not a colour", "accent": "#123456" })).await;
    let now = custom();
    c.check(
        "a colour that isn't one leaves the old one",
        now.background == "#f1e9da" && now.accent == "#123456" && now.fade == 90,
        &now,
    );
    let _ = call(&app, "setCustomTheme", json!({ "background": "#141a26", "accent": "#123456", "fade": 30 })).await;
    let dark = themes::current();
    c.check("on a dark background they are dark", dark.id == "custom" && dark.dark, &dark);

    // A picture chosen earlier (as "Choose a picture…" leaves it), then removed: the file goes too.
    let pictures = vault::pictures_dir();
    std::fs::create_dir_all(&pictures).expect("the pictures folder");
    let picture = pictures.join("theme-background-1.png");
    std::fs::write(&picture, [0x89, 0x50, 0x4e, 0x47]).expect("a picture");
    prefs::lock().custom.picture = Some("theme-background-1.png".into());
    let _ = call(&app, "clearThemePicture", json!({})).await;
    c.check("removing the picture deletes its copy", custom().picture.is_none() && !picture.exists(), ());

    let _ = call(&app, "setCustomTheme", json!({ "background": "#141a26", "accent": "#123456", "fade": 30, "blur": 99 })).await;
    c.check("the blur kept within 0 to 30 px", custom().blur == 30, custom());
    let _ = call(&app, "setCustomTheme", json!({ "background": "#141a26", "accent": "#123456", "fade": 30, "blur": 6 })).await;

    // ── Your themes ──
    let pic = |name: &str| pictures.join(name);
    std::fs::write(pic("theme-background-10.png"), [1, 2, 3]).expect("a picture");
    prefs::lock().custom.picture = Some("theme-background-10.png".into());
    let _ = call(&app, "saveTheme", json!({ "name": " Ha Long " })).await;
    let all = saved();
    let kept = if all.len() == 1 { all.first().cloned() } else { None };
    c.check(
        "the custom theme saved under its name, as it is",
        kept.as_ref().is_some_and(|k| k.name == "Ha Long" && k.look == custom()),
        &kept,
    );
    c.check(
        "…and in the app's state for Settings",
        app.vault_ref().is_some() && App::saved_themes_dto().as_array().is_some_and(|a| a.len() == 1),
        (),
    );
    let _ = call(&app, "setCustomTheme", json!({ "background": "#0a3d62", "accent": "#123456", "fade": 30 })).await;
    let _ = call(&app, "saveTheme", json!({ "name": "ha long" })).await;
    c.check(
        "the same name again saves over it (one theme, the new look)",
        saved().len() == 1 && saved()[0].look.background == "#0a3d62",
        saved().len(),
    );
    c.check("a theme with no name is refused", is_user(&call(&app, "saveTheme", json!({ "name": "  " })).await), ());
    let _ = call(&app, "clearThemePicture", json!({})).await;
    c.check("removing the picture in use keeps it while a kept theme shows it", pic("theme-background-10.png").exists(), ());
    let first_id = saved()[0].id.clone();
    let _ = call(&app, "useSavedTheme", json!({ "id": first_id })).await;
    let now = custom();
    c.check(
        "a kept theme picked is the custom theme in use, its picture with it",
        theme() == "custom" && now == saved()[0].look && now.picture.as_deref() == Some("theme-background-10.png"),
        &now,
    );

    // Shared as a file and added back: the same look, its picture copied under a new name, the same bytes.
    let file = Path::new(dir).join("Ha Long.bloxkeeptheme").to_string_lossy().into_owned();
    let written = write_theme_file(&saved()[0], &file);
    if let Err(e) = &written {
        println!("NOTE  the theme file wasn't written: {e:?}");
    }
    let _ = call(&app, "setCustomTheme", json!({ "background": "#222222", "accent": "#ffffff", "fade": 10 })).await;
    let _ = app.add_theme_file(&file);
    let added = saved()[0].clone();
    let without_picture = |l: &CustomTheme| CustomTheme { picture: None, ..l.clone() };
    c.check(
        "a theme's file added back: kept first, in use, the same name and look",
        saved().len() == 2 && added.name == saved()[1].name && custom() == added.look && without_picture(&added.look) == without_picture(&saved()[1].look),
        &added,
    );
    c.check(
        "…its picture copied out under a new name of Bloxkeep's own, the same bytes",
        added.look.picture.as_ref().is_some_and(|copy| {
            copy != "theme-background-10.png" && themes::is_theme_media_name(copy) && std::fs::read(pic(copy)).is_ok_and(|b| b == [1, 2, 3])
        }),
        &added.look.picture,
    );

    // Files from someone else.
    let add = |path: &str| app.add_theme_file(path);
    let before = saved().len();
    let picture_count = count_files(&pictures);
    let not_zip = Path::new(dir).join("plain.bloxkeeptheme");
    std::fs::write(&not_zip, "not a zip").expect("a file");
    let refused_plain = is_user(&add(&not_zip.to_string_lossy()));
    let refused_no_json = is_user(&add(&zip(dir, "nojson.bloxkeeptheme", "[1]", &[])));
    let refused_other = is_user(&add(&zip(dir, "other.bloxkeeptheme", "{\"name\":\"x\"}", &[])));
    c.check(
        "a file that isn't a theme is refused, nothing kept",
        refused_plain && refused_no_json && refused_other && saved().len() == before,
        format!("{refused_plain} {refused_no_json} {refused_other}"),
    );
    let exe = add(&zip(
        dir,
        "exe.bloxkeeptheme",
        "{\"bloxkeepTheme\":1,\"name\":\"x\",\"picture\":\"background.exe\"}",
        &[("background.exe", &[0x4d, 0x5a])],
    ));
    c.check(
        "a picture of a kind the page doesn't show (a program) is refused, nothing written",
        is_user(&exe) && saved().len() == before && count_files(&pictures) == picture_count,
        &exe,
    );
    let traversal = add(&zip(
        dir,
        "path.bloxkeeptheme",
        "{\"bloxkeepTheme\":1,\"name\":\"Sneaky\",\"background\":\"red\",\"accent\":\"#00ff00\",\"fade\":500,\"blur\":-4,\"picture\":\"../../evil.png\"}",
        &[("../../evil.png", &[1])],
    ));
    let sneaky = saved()[0].clone();
    c.check(
        "a picture named with a folder is never read, nothing written outside the pictures folder; colours checked, numbers kept in range",
        traversal.is_ok()
            && sneaky.name == "Sneaky"
            && sneaky.look.picture.is_none()
            && sneaky.look.accent == "#00ff00"
            && sneaky.look.fade == 90
            && sneaky.look.blur == 0
            && themes::hex(Some(&sneaky.look.background)).is_some()
            && !Path::new(dir).join("evil.png").exists()
            && !vault::data_dir().join("evil.png").exists(),
        &sneaky,
    );
    c.check(
        "a video over 50 MB is refused before anything is read",
        check_theme_media(".mp4", 50 * 1024 * 1024).is_ok()
            && is_user(&check_theme_media(".mp4", 50 * 1024 * 1024 + 1))
            && is_user(&check_theme_media(".png", 20 * 1024 * 1024 + 1)),
        (),
    );

    // Deleted: its picture goes, unless the theme in use or another kept one shows it.
    let added_picture = added.look.picture.clone().unwrap_or_default();
    prefs::lock().custom.picture = None;
    let _ = call(&app, "deleteSavedTheme", json!({ "id": added.id })).await;
    c.check(
        "a kept theme deleted, its picture with it",
        saved().iter().all(|t| t.id != added.id) && !pic(&added_picture).exists(),
        (),
    );
    c.check("…a picture another kept theme shows is kept", pic("theme-background-10.png").exists(), ());

    prefs::lock().custom.picture = Some("theme-background-2.png".into());
    prefs::save();
    let custom_before = custom();
    let saved_before = saved();
    {
        let mut p = prefs::lock();
        p.saved_themes = Vec::new();
        p.custom = CustomTheme { background: "#000000".into(), accent: "#000000".into(), fade: 0, picture: None, blur: 0 };
        p.theme = "dark".into();
    }
    prefs::load();
    c.check(
        "the custom theme, its colours, its picture and its blur come back after a restart",
        theme() == "custom" && custom() == custom_before && custom_before.picture.as_deref() == Some("theme-background-2.png"),
        custom(),
    );
    c.check(
        "…and your themes",
        saved() == saved_before,
        saved().iter().map(|t| format!("{t:?}")).collect::<Vec<_>>().join(", "),
    );

    // The default theme became Sunset (2026-11-18): a prefs.json from before that, still on Dark, opens on Sunset,
    // once; Dark picked after that stays Dark.
    let file = crate::core::vault::data_dir().join("prefs.json");
    let kept = std::fs::read_to_string(&file).unwrap_or_default();
    let before: String = serde_json::from_str::<serde_json::Value>(&kept)
        .ok()
        .map(|mut v| {
            if let Some(o) = v.as_object_mut() {
                o.remove("sunsetDefault");
                o.insert("theme".into(), "dark".into());
            }
            v.to_string()
        })
        .unwrap_or_default();
    let _ = std::fs::write(&file, before);
    prefs::load();
    let moved = theme();
    prefs::lock().theme = "dark".into();
    prefs::save();
    prefs::load();
    c.check(
        "a prefs.json from before Sunset was the default, still on Dark, opens on Sunset once; Dark picked after that stays",
        moved == "sunset" && theme() == "dark",
        format!("{moved}, then {}", theme()),
    );

    // The first open's language step (pages/lock.rs): asked until someone picks one; a prefs.json from before it was
    // asked has its language as a pick; with no language saved, Windows' display language, read by name.
    let edited = |edit: &dyn Fn(&mut serde_json::Map<String, serde_json::Value>)| {
        let mut v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap_or_default()).unwrap_or_else(|_| json!({}));
        if let Some(o) = v.as_object_mut() {
            edit(o);
        }
        let _ = std::fs::write(&file, v.to_string());
        prefs::load();
    };
    let before_step = std::fs::read_to_string(&file).unwrap_or_default();
    let system = prefs::system_language();
    c.check("Windows' display language is one of Bloxkeep's, or English", crate::lang::by_code(system).is_some(), system);
    edited(&|o| {
        o.remove("languageChosen");
        o.insert("language".into(), "de".into());
    });
    c.check("a prefs.json from before the step, with a language: that language, picked", prefs::language() == "de" && prefs::language_chosen(), prefs::language());
    edited(&|o| {
        o.remove("language");
        o.remove("languageChosen");
    });
    c.check("no language saved: Windows' own, not picked (the step asks)", prefs::language() == system && !prefs::language_chosen(), prefs::language());
    crate::core::l::try_language("ja");
    prefs::load();
    c.check("a language tried in the step is shown and kept, still not picked", prefs::language() == "ja" && !prefs::language_chosen(), prefs::language());
    crate::core::l::set("fr");
    prefs::load();
    c.check("…Continue picks it, for good", prefs::language() == "fr" && prefs::language_chosen(), prefs::language());
    edited(&|o| {
        o.insert("language".into(), "pt-BR".into());
    });
    c.check("a language saved with its region (pt-BR) is Bloxkeep's pt", prefs::language() == "pt", prefs::language());
    let _ = std::fs::write(&file, before_step);
    prefs::load();
    let _ = std::fs::write(&file, kept);
    prefs::load();
}
