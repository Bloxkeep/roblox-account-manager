// Tests/Bloxkeep.Tests/RobuxSuite.cs: each account's Robux on its card: what Roblox's answers read as, that the first
// round starts at unlocking, that the card keeps the number's place until its first answer, and that the page gets the
// number. And Roblox VNG accounts open like the others. Offline: nothing is asked of Roblox.
use super::{unlock, window::quiet_app as app, Checks, PASSWORD};
use crate::core::{
    base::Stamp,
    models::Account,
    roblox_api::{play::read_robux, realm},
    roblox_installs, roblox_launcher,
    vault::Vault,
};

pub async fn run(c: &mut Checks, _dir: &str) {
    let read = read_robux;
    c.check("Roblox's answer {\"robux\": 1234} reads as 1234", read("{\"robux\":1234}") == Some(1234), read("{\"robux\":1234}"));
    c.check("…and 0 as 0", read("{\"robux\":0}") == Some(0), read("{\"robux\":0}"));
    c.check(
        "an error, or an answer that isn't the balance, reads as nothing",
        read("{\"errors\":[{\"code\":9002}]}").is_none() && read("<html>").is_none() && read("[]").is_none() && read("{\"robux\":-5}").is_none(),
        (),
    );

    let seed = Vault::create(PASSWORD, None).expect("vault");
    seed.data().accounts.push(Account { username: "Vng".into(), user_id: 31, realm: realm::VNG.to_string(), ..Default::default() });
    seed.save().expect("saved");
    drop(seed);
    let (app, _said) = app();
    app.power.network_up.set(true); // as on a PC online, whatever this one is
    unlock(&app).await;
    let account = app.vault(|d| d.accounts[0].clone()).expect("unlocked");

    let refused = app.check_can_launch(std::slice::from_ref(&account)).err().map(|e| e.to_string());
    c.check("a Roblox VNG account alone may be opened (it was refused before)", refused.is_none(), &refused);

    c.check(
        "Roblox VNG's client is told by its folder",
        roblox_installs::is_vng_client(r"C:\Users\A\AppData\Local\RobloxPCVNG\Versions\version-1\RobloxPlayerBeta.exe")
            && !roblox_installs::is_vng_client(r"C:\Users\A\AppData\Local\Roblox\Versions\version-1\RobloxPlayerBeta.exe")
            && !roblox_installs::is_vng_client(r"D:\bloxtrap\Bloxstrap.exe"),
        (),
    );
    let fallback_exe = roblox_launcher::fallback_launcher().map(|f| f.0);
    c.check(
        "…and never what stands in for the links",
        fallback_exe.as_deref().is_none_or(|e| !roblox_installs::is_vng_client(e)),
        &fallback_exe,
    );
    c.check(
        "…nor the client Default counts as its own",
        roblox_installs::client_for("auto", "").is_none_or(|own| !roblox_installs::is_vng_client(&own)),
        (),
    );

    // The unlock started the first round already: the next is half an hour away. No cookie: nothing asked of Roblox.
    let next = app.robux.next_robux.get();
    let ahead = next.secs_since(Stamp::utc_now());
    c.check("the first round starts at unlocking, alongside status and avatars", ahead > 29.0 * 60.0, ahead);

    let field = |name: &str| {
        let a = app.find_account(account.id).expect("account");
        app.account_dto(&a).get(name).cloned().unwrap_or(serde_json::Value::Null)
    };
    app.update_account(account.id, |a| a.cookie = "_|WARNING:-DO-NOT-SHARE-THIS.--test".into());
    app.power.network_up.set(true);
    c.check(
        "a signed-in account not answered yet keeps the number's place on its card",
        field("robuxWait") == serde_json::json!(true) && field("robux").is_null(),
        (field("robuxWait"), field("robux")),
    );
    app.robux.robux_asked.borrow_mut().insert(account.id);
    c.check("…given back once asked, when Roblox gave no number", field("robuxWait") == serde_json::json!(false), field("robuxWait"));
    app.robux.robux.borrow_mut().insert(account.id, 1234);
    c.check(
        "the card gets the account's Robux",
        field("robux") == serde_json::json!(1234) && field("robuxWait") == serde_json::json!(false),
        field("robux"),
    );
    app.robux.robux_asked.borrow_mut().clear();
    app.robux.robux.borrow_mut().clear();
    app.power.network_up.set(false);
    c.check("…and offline, no place kept for a number that won't come", field("robuxWait") == serde_json::json!(false), field("robuxWait"));
    app.update_account(account.id, |a| a.cookie = String::new());
}
