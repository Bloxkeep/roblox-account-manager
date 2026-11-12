// GameTarget.cs: where to send an account, and the player it follows.
use std::collections::HashMap;
use std::sync::LazyLock;

use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};

use crate::core::l;
use crate::core::roblox_api::{contains_ci, unescape_data_string};

/// A Roblox player an account follows: its rejoins go to the server that player is in by then.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct FollowedPlayer {
    pub user_id: i64,
    pub name: String,
}

/// Where to send an account: a place, optionally a specific server (job) or a private server.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct GameTarget {
    pub place_id: i64,
    pub job_id: Option<String>,
    pub link_code: Option<String>,
    pub share_code: Option<String>,
    /// The player this account follows, kept through rejoins, restarts and saved sessions: each rejoin looks the
    /// player up again and goes to their current server instead of this place.
    pub follow: Option<FollowedPlayer>,
    /// A private server's access code (Browse servers): only accounts that own it or were invited get in, unlike a
    /// link code, which lets in anyone with the link.
    pub access_code: Option<String>,
}

static GAME_PATH: LazyLock<Regex> = LazyLock::new(|| RegexBuilder::new(r"/games/(\d+)").case_insensitive(true).build().unwrap());
// A pair also right after "roblox://" or at the start ("placeId=…&gameInstanceId=…"), not only after ? or &.
static QUERY_PAIR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?:[?&]|//|^)([^=&#\s/?]+)=([^&#\s]*)").unwrap());

impl GameTarget {
    pub fn new(place_id: i64, job_id: Option<String>, link_code: Option<String>, share_code: Option<String>) -> Self {
        GameTarget { place_id, job_id, link_code, share_code, follow: None, access_code: None }
    }

    /// A private server, by its link or by its access code.
    pub fn is_private(&self) -> bool {
        self.link_code.is_some() || self.access_code.is_some()
    }

    /// Accepts a place id, a roblox.com/games/… link (privateServerLinkCode, gameInstanceId), a
    /// roblox.com/share?code=…&type=Server link (also roblox://navigation/share_links or wrapped in ro.blox.com), or
    /// a roblox:// deep link with placeId, also with accessCode (what Browse servers puts in the game box).
    /// Err holds the sentence to show.
    pub fn try_parse(place_input: &str, job_input: &str) -> Result<GameTarget, String> {
        Self::try_parse_at(place_input, job_input, 0)
    }

    fn try_parse_at(place_input: &str, job_input: &str, depth: i32) -> Result<GameTarget, String> {
        let input = place_input.trim();
        let job = job_input.trim();
        if input.is_empty() {
            return Err(l::t("Enter a Place ID or paste a Roblox game link."));
        }

        let mut place_id: i64 = 0;
        let (mut link_code, mut share_code, mut job_id, mut access_code): (Option<String>, Option<String>, Option<String>, Option<String>) =
            (None, None, None, None);
        match input.parse::<i64>() {
            Ok(n) => {
                place_id = n;
                if place_id <= 0 {
                    return Err(l::t("Invalid Place ID."));
                }
            }
            Err(_) => {
                // Words, not a link (a game typed by name and not picked from the search): say what to do instead.
                if !input.contains('/') && !input.contains('?') && !contains_ci(input, "roblox") {
                    return Err(l::t("Pick the game from the search list, or paste a game link or Place ID."));
                }
                // Keys compared ignoring case; the first of a name wins (TryAdd).
                let mut query: HashMap<String, String> = HashMap::new();
                for m in QUERY_PAIR.captures_iter(input) {
                    let key = unescape_data_string(&m[1]);
                    query.entry(key.to_uppercase()).or_insert_with(|| unescape_data_string(&m[2]));
                }
                let get = |name: &str| query.get(&name.to_uppercase()).cloned();

                // A ro.blox.com link from the Roblox app wraps the real link in af_web_dp (or af_dp); read that one.
                if depth < 2
                    && let Some(wrapped) = get("af_web_dp").or_else(|| get("af_dp"))
                    && contains_ci(&wrapped, "roblox")
                {
                    return Self::try_parse_at(&wrapped, job_input, depth + 1);
                }

                if let Some(path) = GAME_PATH.captures(input) {
                    place_id = path[1].parse().unwrap_or(0);
                } else if let Some(p) = get("placeId") {
                    place_id = p.trim().parse().unwrap_or(0);
                }

                let share = contains_ci(input, "roblox.com/share") || contains_ci(input, "share_links");
                if share && let Some(code) = get("code") {
                    if let Some(kind) = get("type")
                        && !kind.eq_ignore_ascii_case("Server")
                    {
                        return Err(l::t("Only private-server share links (type=Server) are supported. For a regular game, paste a roblox.com/games/… link."));
                    }
                    share_code = Some(code);
                }

                if let Some(lc) = get("privateServerLinkCode").or_else(|| get("linkCode")) {
                    link_code = Some(lc);
                }
                if let Some(gi) = get("gameInstanceId").or_else(|| get("jobId")) {
                    job_id = Some(gi);
                }
                if let Some(ac) = get("accessCode")
                    && !ac.trim().is_empty()
                {
                    match guid_try_parse(&ac) {
                        Some(access) => access_code = Some(access),
                        None => return Err(l::t("Invalid private server access code.")),
                    }
                }

                if place_id <= 0 && share_code.is_none() {
                    return Err(l::t("This link has no Place ID."));
                }
            }
        }

        if !job.is_empty() {
            job_id = Some(job.to_string());
        }
        if let Some(j) = job_id.as_deref().filter(|j| !j.is_empty()) {
            match guid_try_parse(j) {
                // Guid also accepts {…} and undashed forms, which Roblox and the log comparison don't
                Some(guid) => job_id = Some(guid),
                None => return Err(l::t("Invalid Job ID (expected xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx).")),
            }
        }

        Ok(GameTarget { place_id, job_id: null_if_empty(job_id), link_code: null_if_empty(link_code), share_code, follow: None, access_code })
    }
}

/// Guid.TryParse, written back as "D" (lower case, dashed). Takes the dashed, undashed, {…} and (…) forms.
// PORT: .NET's Guid.TryParse also takes the "X" form ({0x…,0x…,…}); it isn't taken here.
pub(crate) fn guid_try_parse(text: &str) -> Option<String> {
    let t = text.trim();
    let inner = if t.starts_with('(') && t.ends_with(')') && t.len() >= 2 { &t[1..t.len() - 1] } else { t };
    if inner.starts_with("urn:") {
        return None;
    }
    uuid::Uuid::parse_str(inner).ok().map(|g| g.hyphenated().to_string())
}

fn null_if_empty(s: Option<String>) -> Option<String> {
    s.filter(|v| !v.trim().is_empty()).map(|v| v.trim().to_string())
}
