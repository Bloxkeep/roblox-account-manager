// Tests/Bloxkeep.Tests/SavedServersSuite.cs: saved servers (app/saved_servers.rs). A private server's link is saved
// with its game, a link that isn't a private server's is refused, the same link again updates the one saved, a code
// needs its game, and one is deleted. Offline: nothing is asked of Roblox.
use serde_json::{Value, json};

use super::{
    Checks,
    rejoin::{no_network, quiet_app, seed},
    unlock,
};
use crate::core::models::SavedServer;

const PRIVATE: &str = "https://www.roblox.com/games/2753915549/Blox-Fruits?privateServerLinkCode=12345678901234567890";

pub async fn run(c: &mut Checks, _dir: &str) {
    no_network();
    seed(Vec::new());
    let (app, _said) = quiet_app();
    unlock(&app).await;
    let list = || -> Vec<SavedServer> { app.vault(|d| d.settings.saved_servers.clone()).unwrap_or_default() };
    let save = |args: Value| app.save_server(&args).err().map(|e| e.to_string());

    let said = save(json!({ "name": "VIP farm", "link": PRIVATE, "code": "", "gameName": "Blox Fruits" }));
    let l = list();
    c.check(
        "a private server's link is saved, with its game from the link",
        said.is_none() && l.len() == 1 && l[0].name == "VIP farm" && l[0].place_id == 2753915549 && l[0].link == PRIVATE,
        &said,
    );
    let said = save(json!({ "name": "Renamed", "link": PRIVATE, "code": "4F7K2Q" }));
    let l = list();
    c.check(
        "…the same link again updates the one saved (name, code), not a second",
        said.is_none() && l.len() == 1 && l[0].name == "Renamed" && l[0].code == "4F7K2Q",
        &said,
    );
    let said = save(json!({ "name": "Public", "link": "https://www.roblox.com/games/2753915549/Blox-Fruits", "code": "" }));
    c.check("a game's link that isn't a private server's is refused", said.is_some() && list().len() == 1, &said);
    let said = save(json!({ "name": "Code only", "link": "", "code": "ZX81" }));
    c.check("a code without its game is refused", said.is_some() && list().len() == 1, &said);
    let said = save(json!({ "name": "", "link": "", "code": "ZX81", "placeId": 920587237i64, "gameName": "Adopt Me!" }));
    let l = list();
    c.check(
        "…with its game it is saved, named after the game, newest first",
        said.is_none() && l.len() == 2 && l[0].name == "Adopt Me!" && l[0].code == "ZX81" && l[0].place_id == 920587237 && l[0].link.is_empty(),
        &said,
    );
    let said = save(json!({ "name": "Nothing", "link": "", "code": "", "placeId": 920587237i64 }));
    c.check("neither a link nor a code: refused", said.is_some() && list().len() == 2, &said);
    let first = list()[0].id;
    let _ = app.delete_server(first);
    let l = list();
    c.check("a saved server is deleted", l.len() == 1 && l[0].name == "Renamed", ());
}
