// The built-in starting points for a new macro: generic (no game's name), each running as it is once its marked steps
// are done on the game (an area moved, a colour picked). Each comes with the settings the player changes on its panel
// ("$o1") and its areas ("@a1"), and a note on its steps saying why each is there (they teach as they're used). Kept as
// data: the page lists them and opens a copy in the editor.
use crate::core::base::{Fail, Json, Res};
use crate::core::l;
use crate::core::macro_model::macro_rules::{self, page_json};
use crate::core::macro_model::{Macro, MacroStep};

/// A template: its macro and what it does.
#[derive(Clone, Debug)]
pub struct Template {
    pub id: String,
    pub name: String,
    pub about: String,
    /// A plain word the page draws an icon for: keyboard, pointer, bar, rejoin, blank.
    pub icon: String,
    /// The macro a new macro starts from (Blank: no steps).
    pub r#macro: Macro,
}

/// For the page: { macros: (Macro & {about, icon})[] }, each new (its id ""). A step the person still has to
/// finish on the game carries "todo" (its checklist: the fields to set, "region", "colour", "target", "marker", "x").
pub fn all() -> Res<Json> {
    let macros: Vec<Json> = build()?
        .into_iter()
        .map(|t| {
            let mut node = page_json::write(&t.r#macro, page_json::Shape::Macro);
            if let Json::Object(o) = &mut node {
                o.insert("id".into(), Json::String(String::new()));
                o.insert("about".into(), Json::String(t.about));
                o.insert("icon".into(), Json::String(t.icon));
            }
            node
        })
        .collect();
    Ok(serde_json::json!({ "macros": macros }))
}

/// The person's to-do on a step (the editor's checklist): the fields still to set for their game, kept with
/// the step like any field it doesn't know.
fn todo(step: &mut MacroStep, fields: &[&str]) {
    step.extra.insert("todo".into(), Json::Array(fields.iter().map(|f| Json::String(f.to_string())).collect()));
}

/// Why a step is there, shown under it in the editor.
fn note(step: &mut MacroStep, note: String) {
    step.extra.insert("note".into(), Json::String(note));
}

pub fn find(id: Option<&str>) -> Res<Option<Template>> {
    Ok(build()?.into_iter().find(|t| Some(t.id.as_str()) == id))
}

/// Built each time (names in the language now; nothing shared to change by mistake).
pub fn build() -> Res<Vec<Template>> {
    Ok(vec![keys()?, click()?, cast()?, rejoin()?, blank()])
}

fn macro_of(name: &str, max_minutes: i32, steps: &str, options: &str, areas: &str, targets: &str, trigger: &str) -> Res<Macro> {
    let text = format!(
        r#"{{"name":{},"targets":{},"trigger":{},"maxMinutes":{},"steps":{},"options":{},"areas":{},
             "panel":{{"autoShow":false,"placeId":null}} }}"#,
        s(name),
        s(targets),
        trigger,
        max_minutes,
        steps,
        options,
        areas
    );
    let json: Json = serde_json::from_str(&text).map_err(Fail::from)?;
    macro_rules::read(&json)
}

fn s(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_else(|_| "\"\"".into())
}

const MANUAL: &str = r#"{"kind":"manual"}"#;

fn keys() -> Res<Template> {
    let name = l::t("Press a key every few minutes");
    let options = r##"
        [
          {"name":"o1","label":%EVERY%,"kind":"number","value":300000,"default":300000,"min":1000,"max":1080000,"unit":"s"},
          {"name":"o2","label":%KEY%,"kind":"key","value":"Space","default":"Space"}
        ]
        "##
    .replace("%EVERY%", &s(&l::t("Every")))
    .replace("%KEY%", &s(&l::t("Key")));
    let mut m = macro_of(
        &name,
        240,
        r##"
        [
          {"kind":"loop","times":0},
          {"kind":"key","key":"$o2"},
          {"kind":"wait","ms":"$o1"},
          {"kind":"endLoop"}
        ]
        "##,
        &options,
        "[]",
        "selected",
        MANUAL,
    )?;
    note(&mut m.steps[1], l::t("Roblox kicks idle players after 20 minutes; this presses a key before that."));
    note(&mut m.steps[2], l::t("The player can change how long on the panel"));
    Ok(Template { id: "keys".into(), name, about: l::t("Presses a key now and then, so the game doesn't take you for away."), icon: "keyboard".into(), r#macro: m })
}

fn click() -> Res<Template> {
    let name = l::t("Click a button when it shows");
    let options = r##"
        [
          {"name":"o1","label":%COLOR%,"kind":"colour","value":"#22c55e","default":"#22c55e"},
          {"name":"o2","label":%WAIT%,"kind":"number","value":500,"default":500,"min":0,"max":60000,"unit":"s"}
        ]
        "##
    .replace("%COLOR%", &s(&l::t("Button color")))
    .replace("%WAIT%", &s(&l::t("Then wait")));
    let areas = r##"
        [ {"name":"a1","label":%AREA%,"colour":"#22c55e","rect":{"x":0.25,"y":0.25,"w":0.5,"h":0.5} } ]
        "##
    .replace("%AREA%", &s(&l::t("Button area")));
    let mut m = macro_of(
        &name,
        120,
        r##"
        [
          {"kind":"loop","times":0},
          {"kind":"waitColour","colour":"$o1","tolerance":24,"region":"@a1","timeoutMs":600000},
          {"kind":"clickColour","colour":"$o1","tolerance":24,"region":"@a1","minSize":6,"button":"left","timeoutMs":2000},
          {"kind":"addVar","name":"clicks","value":1},
          {"kind":"wait","ms":"$o2"},
          {"kind":"endLoop"}
        ]
        "##,
        &options,
        &areas,
        "selected",
        MANUAL,
    )?;
    todo(&mut m.steps[1], &["region", "colour"]);
    note(&mut m.steps[1], l::t("Move the box to where the button appears"));
    note(&mut m.steps[2], l::t("Pick the button's color on the game"));
    note(&mut m.steps[3], l::t("Counts the clicks, shown on the panel"));
    Ok(Template {
        id: "click".into(),
        name,
        about: l::t("Waits for a button of a color to show up in an area, clicks it, and waits for the next one."),
        icon: "pointer".into(),
        r#macro: m,
    })
}

fn cast() -> Res<Template> {
    let name = l::t("Cast and reel");
    let options = r##"
        [
          {"name":"o1","label":%HOLD%,"kind":"number","value":800,"default":800,"min":0,"max":10000,"unit":"s"},
          {"name":"o2","label":%TARGET%,"kind":"colour","value":"#22c55e","default":"#22c55e"},
          {"name":"o3","label":%MARKER%,"kind":"colour","value":"#ffffff","default":"#ffffff"},
          {"name":"o4","label":%STRENGTH%,"kind":"number","value":1,"default":1,"min":0,"max":10,"unit":""},
          {"name":"o5","label":%BAR%,"kind":"number","value":60000,"default":60000,"min":1000,"max":600000,"unit":"s"}
        ]
        "##
    .replace("%HOLD%", &s(&l::t("Hold to cast")))
    .replace("%TARGET%", &s(&l::t("Target color")))
    .replace("%MARKER%", &s(&l::t("Marker color")))
    .replace("%STRENGTH%", &s(&l::t("Strength")))
    .replace("%BAR%", &s(&l::t("Wait for the bar")));
    let areas = r##"
        [ {"name":"a1","label":%AREA%,"colour":"#3b82f6","rect":{"x":0.3,"y":0.75,"w":0.4,"h":0.06} } ]
        "##
    .replace("%AREA%", &s(&l::t("Bar area")));
    let mut m = macro_of(
        &name,
        240,
        r##"
        [
          {"kind":"loop","times":0},
          {"kind":"click","x":0.5,"y":0.5,"button":"left","holdMs":"$o1"},
          {"kind":"follow","region":"@a1","target":"$o2","marker":"$o3","tolerance":24,
           "button":"left","mode":"smooth","kp":"$o4","kd":0.4,"predict":true,"fps":60,"untilGoneMs":1000,"timeoutMs":"$o5"},
          {"kind":"addVar","name":"catches","value":1},
          {"kind":"wait","ms":1500},
          {"kind":"endLoop"}
        ]
        "##,
        &options,
        &areas,
        "selected",
        MANUAL,
    )?;
    todo(&mut m.steps[1], &["x"]);
    todo(&mut m.steps[2], &["region", "target", "marker"]);
    note(&mut m.steps[1], l::t("Holds to cast: drag dot 2 to where you cast"));
    note(&mut m.steps[2], l::t("Keeps the marker on the target until the bar goes"));
    note(&mut m.steps[3], l::t("Counts each catch"));
    note(&mut m.steps[4], l::t("A short break before the next cast"));
    Ok(Template {
        id: "cast".into(),
        name,
        about: l::t("Holds the mouse to cast, waits for the bar, then keeps the marker on the target until the bar goes."),
        icon: "bar".into(),
        r#macro: m,
    })
}

fn rejoin() -> Res<Template> {
    let name = l::t("Rejoin and walk back");
    let mut m = macro_of(
        &name,
        10,
        r##"
        [
          {"kind":"wait","ms":8000},
          {"kind":"key","key":"W","ms":2000},
          {"kind":"wait","ms":1000},
          {"kind":"key","key":"E"}
        ]
        "##,
        "[]",
        "[]",
        "all",
        r#"{"kind":"rejoin"}"#,
    )?;
    note(&mut m.steps[0], l::t("Give the game time to load"));
    note(&mut m.steps[1], l::t("Walks forward for 2 s: change it to your way back"));
    note(&mut m.steps[2], l::t("Stops for a moment"));
    note(&mut m.steps[3], l::t("Uses what is there"));
    Ok(Template { id: "rejoin".into(), name, about: l::t("After a rejoin, waits for the game to load, walks back and presses E."), icon: "rejoin".into(), r#macro: m })
}

fn blank() -> Template {
    let m = Macro { name: l::t("New macro"), targets: "selected".into(), ..Macro::default() };
    Template { id: "blank".into(), name: l::t("Blank"), about: l::t("Start from nothing and build your own."), icon: "blank".into(), r#macro: m }
}
