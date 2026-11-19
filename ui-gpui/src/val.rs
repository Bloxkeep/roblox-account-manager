// Reading the app's JSON state: a field, or nothing when it isn't there or is of another kind (bad data never crashes
// the window). One set for every page (they each had their own copy: 23 of `s`, 2026-11-17).
use serde_json::Value;

/// A text field ("" when missing).
pub fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or_default()
}

/// A name while names are hidden (Settings › Appearance): its first letter and "…", as the accounts' are shown.
pub fn first_only(name: &str) -> String {
    name.trim().chars().next().map(|c| format!("{c}…")).unwrap_or_else(|| "…".into())
}

/// A whole-number field (0 when missing).
pub fn int(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(Value::as_i64).unwrap_or(0)
}

/// A yes/no field (no when missing).
pub fn flag(v: &Value, k: &str) -> bool {
    v.get(k).and_then(Value::as_bool).unwrap_or(false)
}

/// A list field (empty when missing).
pub fn arr<'a>(v: &'a Value, k: &str) -> &'a [Value] {
    v.get(k).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}

/// A text field as an owned string ("" when missing).
pub fn string(v: &Value, k: &str) -> String {
    s(v, k).to_string()
}
