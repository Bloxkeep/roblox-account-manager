// AppController.SavedServers.cs: servers saved to join again (the game picker's Saved tab): a private server's link (put
// in the game box, so Play sends the accounts there), or a code a game asks for inside it (puts the game in the box and
// copies the code; Bloxkeep never types into Roblox).
use serde_json::{Value, json};

use super::App;
use super::controller::{long_of, str_of};
use super::presets::utf16_len;
use crate::core::{
    base::{Fail, Guid, Res},
    game_target::GameTarget,
    l,
    models::SavedServer,
    ui,
};

pub const MAX_SAVED_SERVERS: usize = 100;
pub const MAX_SERVER_NAME: usize = 60;
pub const MAX_SERVER_CODE: usize = 100;

impl App {
    pub fn saved_server_dto(&self, s: &SavedServer) -> Value {
        let icon = self
            .controller
            .games
            .borrow()
            .get(&s.place_id)
            .cloned()
            .flatten()
            .and_then(|g| g.icon_url)
            .or_else(|| self.game_picker.icon_by_universe.borrow().get(&s.universe_id).cloned());
        json!({
            "id": s.id,
            "name": s.name,
            "placeId": s.place_id,
            "universeId": s.universe_id,
            "gameName": s.game_name,
            "link": s.link,
            "code": s.code,
            "iconUrl": icon,
        })
    }

    /// Saves a private server's link, a game's code, or both; the same link saved again updates the existing one.
    pub fn save_server(&self, a: &Value) -> Res<Value> {
        self.vault(|_| ())?;
        let link = str_of(a, "link").trim().to_string();
        let code = str_of(a, "code").trim().to_string();
        let mut name = str_of(a, "name").trim().to_string();
        let game_name = str_of(a, "gameName").trim().to_string();
        let mut place_id = long_of(a, "placeId");
        let universe_id = long_of(a, "universeId");
        if !link.is_empty() {
            let target = GameTarget::try_parse(&link, "").map_err(Fail::user)?;
            if !target.is_private() && target.share_code.is_none() {
                return Err(Fail::user(l::t("That link isn't a private server's. Paste the private server's link, or enter the code the game asks for.")));
            }
            if target.place_id > 0 {
                place_id = target.place_id;
            }
        }
        if link.is_empty() && code.is_empty() {
            return Err(Fail::user(l::t("Enter the code the game asks for, or put a private server's link in the game box first.")));
        }
        if utf16_len(&code) > MAX_SERVER_CODE {
            return Err(Fail::user(l::t("That code is too long.")));
        }
        if link.is_empty() && place_id <= 0 {
            return Err(Fail::user(l::t("Put the game in the game box first, then save its code.")));
        }
        if name.is_empty() {
            name = if !game_name.is_empty() { game_name.clone() } else { l::t("Private server") };
        }
        if utf16_len(&name) > MAX_SERVER_NAME {
            let units: Vec<u16> = name.encode_utf16().take(MAX_SERVER_NAME).collect();
            name = String::from_utf16_lossy(&units);
        }

        let saved: Res<SavedServer> = self.vault_mut(|d| {
            let list = &mut d.settings.saved_servers;
            let at = if !link.is_empty() { list.iter().position(|s| s.link == link) } else { None };
            let at = match at {
                Some(i) => i,
                None => {
                    if list.len() >= MAX_SAVED_SERVERS {
                        return Err(Fail::user(l::f("You can save up to {0} servers. Delete one first.", &[&MAX_SAVED_SERVERS])));
                    }
                    list.insert(0, SavedServer::default());
                    0
                }
            };
            let server = &mut list[at];
            server.name = name;
            server.link = link;
            server.code = code;
            server.place_id = place_id;
            server.universe_id = universe_id;
            server.game_name = game_name;
            Ok(server.clone())
        })?;
        let server = saved?;
        self.persist();
        self.push_state();
        Ok(self.saved_server_dto(&server))
    }

    pub fn delete_server(&self, id: Guid) -> Res<Value> {
        let removed = self.vault_mut(|d| {
            let list = &mut d.settings.saved_servers;
            let before = list.len();
            list.retain(|s| s.id != id);
            before - list.len()
        })?;
        if removed == 0 {
            return Err(Fail::user(l::t("That saved server isn't there any more.")));
        }
        self.persist();
        self.push_state();
        Ok(Value::Null)
    }

    /// The code a saved server's game asks for, onto the clipboard: the person pastes it in the game.
    pub fn copy_server_code(&self, id: Guid) -> Res<Value> {
        let server = self
            .vault(|d| d.settings.saved_servers.iter().find(|s| s.id == id).cloned())?
            .ok_or_else(|| Fail::user(l::t("That saved server isn't there any more.")))?;
        if server.code.is_empty() {
            return Err(Fail::user(l::t("This saved server has no code.")));
        }
        if !ui::copy_to_clipboard(&server.code, false) {
            return Err(Fail::user(l::t("Couldn't access the clipboard; try again.")));
        }
        Ok(json!({ "code": server.code }))
    }
}
