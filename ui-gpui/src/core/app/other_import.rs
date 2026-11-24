//! Accounts from elsewhere, shown in the same preview as another vault file's (vault_import.rs) and imported the same
//! way: Roblox Account Manager's AccountData.json, and a pasted list of cookies.
//!
//! Roblox Account Manager (ic3w0lf22's, and niccdevs' continuation) keeps its accounts in AccountData.json next to its
//! exe, in one of three ways (its AccountManager.cs, LoadAccounts / SaveAccounts):
//! - with its own password: a 64-byte header, a 16-byte salt, a 24-byte nonce, then libsodium's secretbox
//!   (XSalsa20-Poly1305, the tag first) under Argon2i ("Moderate": 128 MiB, 6 passes) of SHA-512 of the password;
//! - without one: the JSON wrapped with Windows' DPAPI for this PC, with its fixed entropy;
//! - plain JSON, when its "NoEncryption" file is there.
//! The JSON is a list of its accounts: SecurityToken (the cookie), Username, UserID, Password, Alias, Description, Group.
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use serde_json::{Value, json};

use super::App;
use crate::core::base::{Cancel, Fail, Res, Stamp};
use crate::core::models::{Account, VaultData};
use crate::core::roblox_api::{self, realm};
use crate::core::vault::Vault;
use crate::core::{l, log, rt};

/// Bigger than any real AccountData.json (about 1 KB an account).
pub const MAX_RAM_FILE: u64 = 64 * 1024 * 1024;
/// Lines of a pasted list checked at once, and in all.
const LIST_AT_ONCE: usize = 4;
pub const MAX_LIST_LINES: usize = 1000;

const RAM_HEADER: &[u8] = b"Roblox Account Manager created by ic3w0lf22 @ github.com .......";
/// The header niccdevs' continuation writes.
const RAM_HEADER_NEW: &[u8] = b"Roblox Account Manager created by ic3w0lf2 and continued by niccdevs @ github.com .......";
/// "ROBLOX ACCOUNT MANAGER | :) | BROUGHT TO YOU BUY ic3w0lf": its DPAPI entropy.
const RAM_ENTROPY: &[u8] = b"ROBLOX ACCOUNT MANAGER | :) | BROUGHT TO YOU BUY ic3w0lf";

/// How a Roblox Account Manager file is kept, by its first bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RamKind {
    Password,
    Windows,
    Plain,
}

pub fn ram_kind(bytes: &[u8]) -> RamKind {
    if bytes.starts_with(RAM_HEADER) || bytes.starts_with(RAM_HEADER_NEW) {
        RamKind::Password
    } else if bytes.trim_ascii_start().first() == Some(&b'[') {
        RamKind::Plain
    } else {
        RamKind::Windows
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum RamFail {
    WrongPassword,
    /// DPAPI refused: the file was made on another PC (or Windows account).
    OtherPc,
    NotRam,
}

/// The file's JSON, opened as Roblox Account Manager opens it.
pub fn ram_open(bytes: &[u8], password: &str) -> Result<Vec<u8>, RamFail> {
    match ram_kind(bytes) {
        RamKind::Plain => Ok(bytes.to_vec()),
        RamKind::Windows => dpapi_open(bytes).ok_or(if bytes.len() < 32 { RamFail::NotRam } else { RamFail::OtherPc }),
        RamKind::Password => {
            let header = if bytes.starts_with(RAM_HEADER) { RAM_HEADER.len() } else { RAM_HEADER_NEW.len() };
            let rest = &bytes[header..];
            if rest.len() < 16 + 24 + 16 {
                return Err(RamFail::NotRam);
            }
            let (salt, rest) = rest.split_at(16);
            let (nonce, sealed) = rest.split_at(24);
            let key = ram_key(password, salt).ok_or(RamFail::NotRam)?;
            ram_unseal(sealed, nonce, &key).ok_or(RamFail::WrongPassword)
        }
    }
}

/// libsodium's crypto_pwhash (Argon2i v1.3, "Moderate") of SHA-512 of the password, as its Cryptography.cs derives it.
fn ram_key(password: &str, salt: &[u8]) -> Option<zeroize::Zeroizing<[u8; 32]>> {
    use argon2::{Algorithm, Argon2, Params, Version};
    use sha2::{Digest, Sha512};
    let hash = zeroize::Zeroizing::new(Sha512::digest(password.as_bytes()));
    let params = Params::new(131_072, 6, 1, Some(32)).ok()?;
    let mut key = zeroize::Zeroizing::new([0u8; 32]);
    Argon2::new(Algorithm::Argon2i, Version::V0x13, params).hash_password_into(&hash, salt, key.as_mut()).ok()?;
    Some(key)
}

/// libsodium's crypto_secretbox_open_easy: the 16-byte tag, then the text (RustCrypto's crypto_secretbox reads that order).
fn ram_unseal(sealed: &[u8], nonce: &[u8], key: &[u8; 32]) -> Option<Vec<u8>> {
    use crypto_secretbox::aead::{Aead, KeyInit};
    let cipher = crypto_secretbox::XSalsa20Poly1305::new(key.into());
    cipher.decrypt(crypto_secretbox::Nonce::from_slice(nonce), sealed).ok()
}

/// libsodium's crypto_secretbox_easy, for the tests' made-up files.
#[cfg(test)]
fn ram_seal(text: &[u8], password: &str, salt: &[u8; 16], nonce: &[u8; 24]) -> Vec<u8> {
    use crypto_secretbox::aead::{Aead, KeyInit};
    let key = ram_key(password, salt).unwrap();
    let cipher = crypto_secretbox::XSalsa20Poly1305::new((&*key).into());
    let sealed = cipher.encrypt(crypto_secretbox::Nonce::from_slice(nonce), text).unwrap();
    [RAM_HEADER, salt, nonce, &sealed].concat()
}

/// ProtectedData.Unprotect with its entropy; it saved for the whole PC (LocalMachine), which this reads too.
fn dpapi_open(data: &[u8]) -> Option<Vec<u8>> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptUnprotectData};
    let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
    let entropy = CRYPT_INTEGER_BLOB { cbData: RAM_ENTROPY.len() as u32, pbData: RAM_ENTROPY.as_ptr() as *mut u8 };
    let mut output = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
    // SAFETY: the blobs point at slices that live through the call; Windows only reads `input` and `entropy` and fills
    // `output`, freed with LocalFree below.
    let ok = unsafe {
        CryptUnprotectData(&input, std::ptr::null_mut(), &entropy, std::ptr::null(), std::ptr::null(), CRYPTPROTECT_UI_FORBIDDEN, &mut output)
    } != 0;
    if !ok || output.pbData.is_null() {
        return None;
    }
    // SAFETY: Windows gave `cbData` bytes at `pbData`; copied out before they are freed.
    let bytes = unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    // SAFETY: `pbData` was allocated by Windows with LocalAlloc for this call.
    unsafe { LocalFree(output.pbData as _) };
    Some(bytes)
}

/// ProtectedData.Protect for the whole PC with its entropy: how it saves without a password (the tests' files).
#[cfg(test)]
fn dpapi_seal(data: &[u8]) -> Option<Vec<u8>> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{CRYPT_INTEGER_BLOB, CRYPTPROTECT_LOCAL_MACHINE, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData};
    let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
    let entropy = CRYPT_INTEGER_BLOB { cbData: RAM_ENTROPY.len() as u32, pbData: RAM_ENTROPY.as_ptr() as *mut u8 };
    let mut output = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
    // SAFETY: as in dpapi_open.
    let ok = unsafe {
        CryptProtectData(&input, std::ptr::null(), &entropy, std::ptr::null(), std::ptr::null(), CRYPTPROTECT_UI_FORBIDDEN | CRYPTPROTECT_LOCAL_MACHINE, &mut output)
    } != 0;
    if !ok || output.pbData.is_null() {
        return None;
    }
    // SAFETY: as in dpapi_open.
    let bytes = unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    // SAFETY: as in dpapi_open.
    unsafe { LocalFree(output.pbData as _) };
    Some(bytes)
}

/// Its accounts as Bloxkeep's: the cookie, name, id, password, alias, description (the note) and group ("Default", its
/// own word for none, left out). Entries without a cookie are skipped; None when the text isn't its list.
pub fn ram_accounts(json: &[u8]) -> Option<Vec<Account>> {
    let list: Vec<Value> = serde_json::from_slice(json).ok()?;
    let text = |v: &Value, key: &str| v.get(key).and_then(Value::as_str).unwrap_or("").trim().to_string();
    let accounts = list
        .iter()
        .filter_map(|v| {
            let cookie = roblox_api::normalize_cookie(&text(v, "SecurityToken"));
            if cookie.is_empty() || !roblox_api::cookie_text_valid(&cookie) {
                return None;
            }
            let group = text(v, "Group");
            // (.NET's empty date, year 1, is none.)
            let last_used = v.get("LastUse").and_then(Value::as_str).and_then(Stamp::parse).filter(|s| *s > Stamp::from_unix_secs(1_262_304_000));
            Some(Account {
                cookie,
                user_id: v.get("UserID").and_then(Value::as_i64).unwrap_or(0).max(0),
                username: text(v, "Username"),
                password: v.get("Password").and_then(Value::as_str).unwrap_or("").to_string(),
                alias: App::cut_text(&text(v, "Alias"), 50),
                note: text(v, "Description"),
                group: if group.eq_ignore_ascii_case("Default") { String::new() } else { group },
                cookie_valid: match v.get("Valid") {
                    Some(Value::Bool(false)) => Some(false),
                    _ => None,
                },
                last_used,
                ..Account::default()
            })
        })
        .collect();
    Some(accounts)
}

/// One line of a pasted list: `cookie`, or `username:password:cookie`. A cookie's own text has colons
/// ("_|WARNING:-DO-NOT-SHARE-THIS…"), so where it starts is found first; what is before it gives the name and password.
#[derive(Debug, PartialEq, Eq)]
pub struct ListLine {
    pub username: String,
    pub password: String,
    pub cookie: String,
}

pub fn parse_list_line(line: &str) -> Option<ListLine> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let lower = line.to_ascii_lowercase();
    let start = ["_|warning", ".roblosecurity="].iter().filter_map(|m| lower.find(m)).min();
    let (before, cookie) = match start {
        Some(i) => (&line[..i], &line[i..]),
        // An old cookie without the warning: the part after the second colon, or the whole line.
        None => match line.splitn(3, ':').collect::<Vec<_>>()[..] {
            [user, pass, cookie] => return Some(ListLine { username: user.trim().into(), password: pass.into(), cookie: roblox_api::normalize_cookie(cookie) }),
            [_, _] => return None, // username:password, no cookie
            _ => ("", line),
        },
    };
    let before = before.trim_end_matches([':', ' ', '\t']);
    let (username, password) = before.split_once(':').map(|(u, p)| (u.trim().to_string(), p.to_string())).unwrap_or_default();
    let cookie = roblox_api::normalize_cookie(cookie);
    (!cookie.is_empty()).then_some(ListLine { username, password, cookie })
}

impl App {
    /// Roblox Account Manager's file, picked: whether it is password-locked (else it opens with no password).
    pub fn ram_file_kind(path: &str) -> Res<RamKind> {
        let refuse = || Fail::user(l::t("This isn't Roblox Account Manager's AccountData.json."));
        let size = std::fs::metadata(path).map_err(|ex| Fail::user(l::f("Couldn't open the data file: {0}", &[&super::controller::error_text(&Fail::from(ex))])))?.len();
        if size > MAX_RAM_FILE || size < 2 {
            return Err(refuse());
        }
        let bytes = std::fs::read(path).map_err(|ex| Fail::user(l::f("Couldn't open the data file: {0}", &[&super::controller::error_text(&Fail::from(ex))])))?;
        Ok(ram_kind(&bytes))
    }

    /// Opens the picked Roblox Account Manager file (with its password when it has one) and shows its accounts.
    pub(super) async fn ram_import_open(self: &Rc<Self>, vault: Arc<Vault>, path: String, password: &str) -> Res<Value> {
        let wait = self.import_wait_seconds(&path);
        if wait > 0 {
            return Ok(json!({ "status": "wrong", "wait": wait, "error": l::f("Too many wrong tries. Try again in {0} seconds.", &[&wait]) }));
        }
        let (p, pw) = (path.clone(), password.to_string());
        let opened = rt::blocking(move || -> Res<Result<Vec<u8>, RamFail>> {
            let bytes = std::fs::read(&p).map_err(Fail::from)?;
            if bytes.len() as u64 > MAX_RAM_FILE {
                return Ok(Err(RamFail::NotRam));
            }
            if ram_kind(&bytes) == RamKind::Password && pw.is_empty() {
                return Ok(Err(RamFail::WrongPassword));
            }
            Ok(ram_open(&bytes, &pw))
        })
        .await
        .map_err(|_| Fail::user(l::t("Windows couldn't read or write the file (another program may be using it).")))?;
        if !self.is_vault(&vault) {
            return Err(Fail::user(l::t("Bloxkeep was locked.")));
        }
        let json = match opened {
            Ok(json) => json,
            Err(RamFail::WrongPassword) if password.is_empty() => return Err(Fail::user(l::t("Enter Roblox Account Manager's password."))),
            Err(RamFail::WrongPassword) => return Ok(self.import_wrong_try(&path, l::t("Wrong password for this file."))),
            Err(RamFail::OtherPc) => {
                log::note("Import from Roblox Account Manager: Windows couldn't open its file (made on another PC).");
                return Err(Fail::user(l::t(
                    "This file was saved without a password on another PC; only that PC can open it. Set a password in Roblox Account Manager there, then import the file again.",
                )));
            }
            Err(RamFail::NotRam) => return Err(Fail::user(l::t("This isn't Roblox Account Manager's AccountData.json."))),
        };
        self.vault_import.vault_import_tries.borrow_mut().remove(&path.to_lowercase());
        let Some(accounts) = ram_accounts(&json) else {
            return Err(Fail::user(l::t("This isn't Roblox Account Manager's AccountData.json.")));
        };
        log::note(&format!("Import from Roblox Account Manager: {} accounts in the file.", accounts.len()));
        if accounts.is_empty() {
            return Err(Fail::user(l::t("There are no accounts in this file.")));
        }
        let data = VaultData { accounts, ..VaultData::default() };
        let name = Path::new(&path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        self.show_import(vault, data, name, Some(path)).await
    }

    /// A pasted list: each line's cookie is checked with Roblox (a few at once), and those Roblox takes are shown in the
    /// preview with the name Roblox gives. The reply also counts lines left out: no cookie, or one Roblox refused or
    /// couldn't be asked about.
    pub async fn list_import_open(self: &Rc<Self>, text: &str) -> Res<Value> {
        let vault = self.require_vault_ref()?;
        let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
        if lines.is_empty() {
            return Err(Fail::user(l::t("Paste one cookie a line.")));
        }
        if lines.len() > MAX_LIST_LINES {
            return Err(Fail::user(l::f("At most {0} lines at a time.", &[&MAX_LIST_LINES])));
        }
        if self.vault_import.vault_import_opening.get() {
            return Err(Fail::user(l::t("Still opening the file — please wait.")));
        }
        self.drop_vault_import(false);
        let _opening = crate::core::base::Busy::set(&self.vault_import.vault_import_opening);
        let parsed: Vec<Option<ListLine>> = lines.iter().map(|l| parse_list_line(l)).collect();
        let no_cookie = parsed.iter().filter(|p| p.as_ref().is_none_or(|p| !roblox_api::cookie_text_valid(&p.cookie))).count();
        let mut seen = std::collections::HashSet::new();
        let to_check: Vec<ListLine> =
            parsed.into_iter().flatten().filter(|p| roblox_api::cookie_text_valid(&p.cookie)).filter(|p| seen.insert(p.cookie.clone())).collect();
        let (mut refused, mut unasked) = (0usize, 0usize);
        let mut accounts: Vec<Account> = Vec::new();
        for chunk in to_check.chunks(LIST_AT_ONCE) {
            let answers = futures::future::join_all(chunk.iter().map(|line| {
                let cookie = line.cookie.clone();
                rt::blocking(move || roblox_api::get_authenticated_user(&cookie, realm::GLOBAL, &Cancel::new()))
            }))
            .await;
            if !self.is_vault(&vault) {
                return Err(Fail::user(l::t("Bloxkeep was locked.")));
            }
            for (line, answer) in chunk.iter().zip(answers) {
                match answer {
                    Ok(Some(user)) => accounts.push(Account {
                        cookie: line.cookie.clone(),
                        user_id: user.id,
                        username: user.name,
                        display_name: user.display_name,
                        password: line.password.clone(),
                        cookie_valid: Some(true),
                        last_checked: Some(Stamp::now()),
                        ..Account::default()
                    }),
                    Ok(None) => refused += 1,
                    Err(_) => unasked += 1,
                }
            }
        }
        log::note(&format!(
            "Import a cookie list: {} lines, {} taken, {} refused by Roblox, {} not asked (error), {} without a cookie.",
            lines.len(),
            accounts.len(),
            refused,
            unasked,
            no_cookie
        ));
        if accounts.is_empty() {
            return Err(Fail::user(if unasked > 0 {
                l::t("Couldn't reach Roblox to check the cookies. Check your connection and try again.")
            } else {
                l::t("Roblox didn't accept any of these cookies.")
            }));
        }
        let shown = accounts.len();
        let data = VaultData { accounts, ..VaultData::default() };
        let mut reply = self.show_import(vault, data, l::t("Pasted list"), None).await?;
        // Lines not in the preview: no cookie, refused, not asked, or a cookie already on an earlier line.
        if let Some(o) = reply.as_object_mut() {
            o.insert("skipped".into(), json!(lines.len().saturating_sub(shown)));
        }
        Ok(reply)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const COOKIE: &str = "_|WARNING:-DO-NOT-SHARE-THIS.--Sharing-this-will-allow-someone-to-log-in-as-you-and-to-steal-your-ROBUX-and-items.|_ABCDEF0123";

    #[test]
    fn list_lines() {
        assert_eq!(parse_list_line(COOKIE), Some(ListLine { username: String::new(), password: String::new(), cookie: COOKIE.into() }));
        let full = format!("someone:pa:ss:{COOKIE}");
        assert_eq!(parse_list_line(&full), Some(ListLine { username: "someone".into(), password: "pa:ss".into(), cookie: COOKIE.into() }));
        let named = format!(".ROBLOSECURITY={COOKIE}; path=/");
        assert_eq!(parse_list_line(&named).unwrap().cookie, COOKIE);
        assert_eq!(parse_list_line("someone:password"), None);
        assert_eq!(parse_list_line("   "), None);
        assert_eq!(parse_list_line("a:b:OLDCOOKIE").unwrap().cookie, "OLDCOOKIE");
    }

    fn sample() -> Vec<u8> {
        serde_json::to_vec(&json!([
            { "Valid": true, "SecurityToken": COOKIE, "Username": "made_up_one", "UserID": 1234, "Alias": "Main", "Description": "a note",
              "Password": "pw1", "Group": "Default", "LastUse": "2025-05-01T10:20:30.123+07:00", "Fields": {} },
            { "Valid": false, "SecurityToken": "", "Username": "no_cookie", "UserID": 5 },
            { "Valid": true, "SecurityToken": format!("{COOKIE}X"), "Username": "made_up_two", "UserID": 99, "Group": "Farm", "LastUse": "0001-01-01T00:00:00" },
        ]))
        .unwrap()
    }

    #[test]
    fn ram_password_file() {
        let file = ram_seal(&sample(), "secret pass", &[7; 16], &[9; 24]);
        assert_eq!(ram_kind(&file), RamKind::Password);
        assert_eq!(ram_open(&file, "wrong"), Err(RamFail::WrongPassword));
        let accounts = ram_accounts(&ram_open(&file, "secret pass").unwrap()).unwrap();
        assert_eq!(accounts.len(), 2);
        let a = &accounts[0];
        assert_eq!((a.username.as_str(), a.user_id, a.alias.as_str(), a.note.as_str(), a.password.as_str(), a.group.as_str()), ("made_up_one", 1234, "Main", "a note", "pw1", ""));
        assert!(a.last_used.is_some());
        assert_eq!((accounts[1].group.as_str(), accounts[1].last_used), ("Farm", None));
    }

    /// A file sealed by libsodium itself (PyNaCl 1.6.2: crypto_pwhash Argon2i with opslimit 6 and memlimit 128 MiB of
    /// SHA-512("made up pass"), then crypto_secretbox), as Roblox Account Manager's Sodium writes it; a made-up account.
    #[test]
    fn ram_file_from_libsodium() {
        const FILE: &str = "526f626c6f78204163636f756e74204d616e6167657220637265617465642062792069633377306c6632322040206769746875622e636f6d202e2e2e2e2e2e2e000102030405060708090a0b0c0d0e0f6465666768696a6b6c6d6e6f707172737475767778797a7b45d2d8e1c430c7613fa0324b32340e9a9794e50af71e640030c7d50d486e5844cb2c5b01589ad47711a69acdc479cf3d7f23ef85c6b221283f5125c3d4af734a46f56315bf728c362fa59a0f84a76fe116e7f7940bb32436c2fedb61475b5cc1fa2f90d8077b42e5f685454c3c4576d56352895581afd7aad49065b4f5f28df5771183ff71ea958c842e69d0e327ec8b17239e7dd6acc28eb685d8b992e807373e52b8a718c2aab4ad02739b2997fc59c17382f4652a59bd96b247b8aaecf4e9b77453819d7cb677887e6018a2aa2ffb736a0e3d961d76258281ca75bc0effd5ef";
        let bytes: Vec<u8> = (0..FILE.len()).step_by(2).map(|i| u8::from_str_radix(&FILE[i..i + 2], 16).unwrap()).collect();
        assert_eq!(ram_kind(&bytes), RamKind::Password);
        assert_eq!(ram_open(&bytes, "made up pas"), Err(RamFail::WrongPassword));
        let accounts = ram_accounts(&ram_open(&bytes, "made up pass").unwrap()).unwrap();
        assert_eq!((accounts.len(), accounts[0].username.as_str(), accounts[0].user_id), (1, "made_up_user", 42));
    }

    #[test]
    fn ram_windows_and_plain_files() {
        let sealed = dpapi_seal(&sample()).unwrap();
        assert_eq!(ram_kind(&sealed), RamKind::Windows);
        assert_eq!(ram_accounts(&ram_open(&sealed, "").unwrap()).unwrap().len(), 2);
        let plain = sample();
        assert_eq!(ram_kind(&plain), RamKind::Plain);
        assert_eq!(ram_accounts(&ram_open(&plain, "").unwrap()).unwrap().len(), 2);
        assert_eq!(ram_open(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33], ""), Err(RamFail::OtherPc));
    }
}
