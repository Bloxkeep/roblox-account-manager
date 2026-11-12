// assets/i18n.json from RobloxManager/wwwroot/i18n.js (tools/i18n.mjs, with Node). Without Node the last one made stays.
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=../RobloxManager/wwwroot/i18n.js");
    println!("cargo:rerun-if-changed=tools/i18n.mjs");
    match Command::new("node").arg("tools/i18n.mjs").status() {
        Ok(s) if s.success() => {}
        other => {
            if !std::path::Path::new("assets/i18n.json").exists() {
                panic!("assets/i18n.json is missing and tools/i18n.mjs didn't run ({other:?}); install Node");
            }
            println!("cargo:warning=tools/i18n.mjs didn't run ({other:?}); using the last assets/i18n.json");
        }
    }
}
