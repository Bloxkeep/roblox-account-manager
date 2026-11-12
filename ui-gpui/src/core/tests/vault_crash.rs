// Tests/Bloxkeep.Tests/VaultCrashSuite.cs: a save cut short (a power cut, an antivirus holding the file): vault.rmv,
// vault.rmv.tmp and vault.rmv.bak each absent, whole, empty, cut in half or random bytes, all 125 states. For each,
// whether the app offers "unlock" (vault::exists), what opens (restore_if_missing and Vault::open) and that no file's
// content is gone afterwards; then eight threads writing one small file at once (durable_file, the settings' way).
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::Checks;
use crate::core::durable_file;
use crate::core::models::Account;
use crate::core::vault::{self, Vault};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum S {
    None,
    Valid,
    Empty,
    Half,
    Random,
}

const ALL: [S; 5] = [S::None, S::Valid, S::Empty, S::Half, S::Random];
const PASSWORD: &str = "crash-matrix-pass-1";

fn random_bytes(length: usize) -> Vec<u8> {
    // PORT: C#'s new Random(length); any bytes do, seeded by the length as there.
    let mut state = length as u64 ^ 0x9E37_79B9_7F4A_7C15;
    let mut bytes: Vec<u8> = (0..length)
        .map(|_| {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (state >> 33) as u8
        })
        .collect();
    if let Some(b) = bytes.first_mut() {
        *b = b'X'; // never the vault's own first bytes
    }
    bytes
}

/// Whether the app offers to unlock, and what Open then gives, for the states of the three files (from vault.rs's contract).
fn expect(mut main: S, tmp: S, mut bak: S) -> (bool, String) {
    let looks_like_tmp = matches!(tmp, S::Valid | S::Half);
    let looks_like_bak = matches!(bak, S::Valid | S::Half); // Half: the header is there and it is over 256 bytes
    let mut main_accounts = 2;
    if main == S::None {
        // Put back: the .tmp when it looks like a vault (the .bak stays as its fallback), else the previous save.
        if looks_like_tmp {
            main = tmp;
            main_accounts = 3;
        } else if looks_like_bak {
            main = bak;
            main_accounts = 1;
            bak = S::None; // it became the vault
        } else {
            return (false, String::new());
        }
    }
    let opened = |n: i32| format!("opened {n} account{}", if n == 1 { "" } else { "s" });
    match main {
        S::Valid => (true, opened(main_accounts)),
        // The header is there but the tag doesn't match: the previous save, if it is whole, else it looks like a wrong password.
        S::Half => (true, if bak == S::Valid { opened(1) } else { "wrong password".into() }),
        // No header: the previous save if whole; a half one has the header but won't open; nothing to go by otherwise.
        _ => (
            true,
            if bak == S::Valid {
                opened(1)
            } else if bak == S::Half {
                "wrong password".into()
            } else {
                "not a vault file".into()
            },
        ),
    }
}

fn open_outcome(path: &Path) -> String {
    match Vault::open(PASSWORD, Some(path.to_path_buf())) {
        Ok(v) => {
            let n = v.data().accounts.len();
            format!("opened {n} account{}", if n == 1 { "" } else { "s" })
        }
        Err(f) if vault::is_invalid_password(&f) => "wrong password".into(),
        // InvalidDataException
        Err(f) if f.message() == crate::core::l::t("The data file is not in the right format or is damaged.") => "not a vault file".into(),
        Err(f) => format!("error: {}", f.message()),
    }
}

/// The files of the folder whose name starts with `name` (Directory.GetFiles(data, name + "*")).
fn files_of(data: &Path, name: &str) -> Vec<PathBuf> {
    std::fs::read_dir(data)
        .map(|d| d.flatten().map(|e| e.path()).filter(|p| p.is_file() && p.file_name().is_some_and(|n| n.to_string_lossy().starts_with(name))).collect())
        .unwrap_or_default()
}

pub async fn run(c: &mut Checks, _dir: &str) {
    let data = vault::data_dir().to_path_buf();
    let original = vault::active_file();

    // Three saves of one vault: 1, 2 and 3 accounts (with enough in each for half of the file to still look like a vault).
    let maker = data.join("maker.rmv");
    let mut saves: Vec<Vec<u8>> = vec![Vec::new(); 4];
    {
        let v = Vault::create(PASSWORD, Some(maker.clone())).expect("a vault");
        for n in 1..=3usize {
            v.data().accounts.push(Account {
                username: format!("Crash{n}"),
                alias: std::iter::repeat_n((b'a' + n as u8) as char, 700).collect(),
                user_id: 9000 + n as i64,
                ..Default::default()
            });
            v.save().expect("saved");
            saves[n] = std::fs::read(&maker).expect("read");
        }
    }
    c.check(
        "the three saves are each big enough that half of one still looks like a vault",
        saves[1..].iter().all(|b| b.len() / 2 >= 300),
        saves[1..].iter().map(|b| b.len().to_string()).collect::<Vec<_>>().join(", "),
    );

    // The state that a swap cut short leaves: vault.rmv is the second save, .tmp the third written in full, .bak the first.
    let content = |slot: &str, state: S| -> Vec<u8> {
        let whole = match slot {
            "main" => &saves[2],
            "tmp" => &saves[3],
            _ => &saves[1],
        };
        match state {
            S::Valid => whole.clone(),
            S::Half => whole[..whole.len() / 2].to_vec(),
            S::Empty => Vec::new(),
            _ => random_bytes(whole.len() / 2 + slot.len() * 17),
        }
    };

    let mut wrong: HashMap<S, Vec<String>> = ALL.iter().map(|s| (*s, Vec::new())).collect();
    let mut lost: Vec<String> = Vec::new();
    let mut number = 0;
    for m in ALL {
        for t in ALL {
            for b in ALL {
                number += 1;
                let name = format!("vault-{number:08x}.rmv");
                let path = data.join(&name);
                let mut written: Vec<(&str, Vec<u8>)> = Vec::new();
                for (slot, state, suffix) in [("main", m, ""), ("tmp", t, ".tmp"), ("bak", b, ".bak")] {
                    if state == S::None {
                        continue;
                    }
                    let bytes = content(slot, state);
                    std::fs::write(format!("{}{suffix}", path.display()), &bytes).expect("written");
                    written.push((slot, bytes));
                }
                let (expect_exists, expect_outcome) = expect(m, t, b);
                vault::set_active_file(&name); // each state is its own file: exists puts a missing vault back once per file
                let mut outcome = String::new();
                let exists = vault::exists();
                if exists {
                    outcome = open_outcome(&path);
                }
                // Opened from the spare: the damaged vault is set aside and the spare is the vault again, which opens the same.
                if exists && outcome == "opened 1 account" && m != S::Valid && expect_outcome == outcome {
                    let again = open_outcome(&path);
                    if again != outcome {
                        outcome = format!("first {outcome}, then {again}");
                    }
                }
                if exists != expect_exists || (expect_exists && outcome != expect_outcome) {
                    wrong.get_mut(&m).unwrap().push(format!(
                        "[tmp {t:?}, bak {b:?}] exists {exists} (expected {expect_exists}), {outcome} (expected {expect_outcome})"
                    ));
                }

                // Whatever was there is still somewhere in the folder, under a name or other.
                let after: Vec<Vec<u8>> = files_of(&data, &name).iter().filter_map(|f| std::fs::read(f).ok()).collect();
                for (slot, bytes) in &written {
                    if !bytes.is_empty() && !after.contains(bytes) {
                        lost.push(format!("[main {m:?}, tmp {t:?}, bak {b:?}] the {slot} file's content"));
                    }
                }
                for f in files_of(&data, &name) {
                    let _ = std::fs::remove_file(f);
                }
            }
        }
    }
    vault::set_active_file(&original);
    for m in ALL {
        let list = &wrong[&m];
        c.check(
            &format!("vault.rmv {m:?}: with each state of .tmp and .bak, Exists and the vault that opens are as expected (25 states)"),
            list.is_empty(),
            list.iter().take(6).cloned().collect::<Vec<_>>().join(" ; "),
        );
    }
    c.check(
        "in none of the 125 states is a file's content gone afterwards (damaged and spare copies are kept)",
        lost.is_empty(),
        lost.iter().take(6).cloned().collect::<Vec<_>>().join(" ; "),
    );

    // Eight threads, 200 writes each, one file: the settings file's way of writing (a temp file beside it, then a move).
    let shared = data.join("shared.json");
    let values: Vec<String> = (0..8u8).map(|i| std::iter::repeat_n((b'A' + i) as char, 400).collect()).collect();
    let errors = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let threads: Vec<_> = values
        .iter()
        .cloned()
        .map(|value| {
            let (shared, errors) = (shared.clone(), errors.clone());
            std::thread::spawn(move || {
                for _ in 0..200 {
                    if let Err(e) = durable_file::write_text(&shared, &value) {
                        errors.lock().unwrap().push(e.message().to_string());
                    }
                }
            })
        })
        .collect();
    for t in threads {
        let _ = t.join();
    }
    let errors = errors.lock().unwrap().clone();
    let mut counts: Vec<(String, usize)> = Vec::new();
    for e in &errors {
        match counts.iter_mut().find(|(k, _)| k == e) {
            Some((_, n)) => *n += 1,
            None => counts.push((e.clone(), 1)),
        }
    }
    let distinct: Vec<String> = counts.iter().take(3).map(|(k, n)| format!("{n}x {k}")).collect();
    c.check("8 threads x 200 DurableFile writes to one file: none throws", errors.is_empty(), format!("{} failed, e.g. {}", errors.len(), distinct.join(" ; ")));
    c.check("…no temp file is left beside it", !Path::new(&format!("{}.tmp", shared.display())).exists(), ());
    let text = std::fs::read_to_string(&shared).ok();
    c.check("…and what is in the file is one of the values written, whole", text.as_ref().is_some_and(|t| values.contains(t)), ());
}
