// Tests/Bloxkeep.Tests/DeleteAccountsSuite.cs: deleting accounts (app/accounts.rs): what refers to them goes too. A
// macro aimed at the accounts picked by hand loses the deleted ones; one left with none is turned off, as a preset left
// empty has its schedule turned off.
use serde_json::json;

use super::{
    Checks, call,
    rejoin::{no_network, quiet_app, seed_with},
    unlock,
};
use crate::core::{
    base::{Guid, new_guid},
    macro_model::{Macro, MacroStep, MacroTrigger},
    models::Account,
};

fn make(name: &str, targets: &str, on: &[Guid]) -> Macro {
    Macro {
        id: new_guid(),
        name: name.into(),
        targets: targets.into(),
        enabled: true,
        account_ids: on.to_vec(),
        trigger: MacroTrigger { kind: "join".into(), ..Default::default() },
        steps: vec![MacroStep { kind: "wait".into(), ms: Some(100), ..Default::default() }],
        ..Default::default()
    }
}

pub async fn run(c: &mut Checks, _dir: &str) {
    no_network();
    let made = |name: &str, user_id: i64| Account { username: name.into(), user_id, ..Default::default() };
    let (a, b, other) = (made("AlphaOne", 101), made("BetaTwo", 102), made("GammaThree", 103));
    let (a, b, other_id) = (a.id, b.id, other.id);
    let only_a = make("only A", "accounts", &[a]);
    let a_and_b = make("A and B", "accounts", &[a, b]);
    let only_other = make("only the other", "accounts", &[other_id]);
    let everyone = make("all, with an old id", "all", &[a]);
    let ids = (only_a.id, a_and_b.id, only_other.id, everyone.id);
    let accounts = vec![
        Account { id: a, username: "AlphaOne".into(), user_id: 101, ..Default::default() },
        Account { id: b, username: "BetaTwo".into(), user_id: 102, ..Default::default() },
        other,
    ];
    seed_with(accounts, |d| d.settings.macros.extend([only_a, a_and_b, only_other, everyone]));
    let (app, _said) = quiet_app();
    unlock(&app).await;
    let deleted = call(&app, "deleteAccounts", json!({ "ids": [a] })).await;
    if let Err(e) = &deleted {
        println!("      (deleteAccounts refused: {e})");
    }
    let find = |id: Guid| app.vault(|d| d.settings.macros.iter().find(|m| m.id == id).cloned()).ok().flatten().unwrap_or_default();

    let m = find(ids.0);
    c.check(
        "a macro aimed at the deleted account alone: no accounts left, and turned off",
        m.account_ids.is_empty() && !m.enabled,
        format!("{} {}", m.account_ids.len(), m.enabled),
    );
    let m = find(ids.1);
    c.check("a macro aimed at it and another: keeps the other, stays on", m.account_ids == [b] && m.enabled, ());
    let m = find(ids.2);
    c.check("a macro aimed at others: untouched", m.account_ids == [other_id] && m.enabled, ());
    let m = find(ids.3);
    c.check("a macro for every account: the old id is dropped, it stays on", m.account_ids.is_empty() && m.enabled && m.targets == "all", ());
    let left: Vec<Guid> = app.vault(|d| d.accounts.iter().map(|x| x.id).collect()).unwrap_or_default();
    c.check("the accounts themselves are gone", left == [b, other_id], ());
}
