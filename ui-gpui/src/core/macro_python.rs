// A Python macro script (pyautogui, pydirectinput, keyboard, time) read into a Bloxkeep macro: its presses, holds,
// clicks, moves, scrolls, typing and waits become steps, `while True:` and `for _ in range(N):` become loops, a
// function the script defines is played where it is called. Nothing of the script is run: it is only read, line by
// line. What can't be turned into a step (an `if`, a screenshot, a library Bloxkeep doesn't know) is left out and
// named, so the player knows what to redo by hand in the editor.
//
// Screen pixels become places on the window: the script's points are taken as on a screen of the usual size that
// holds them all (1920 × 1080 unless a point lies past it), the size a full-screen Roblox window has.
use std::collections::HashMap;

use crate::core::base::{Fail, Res};
use crate::core::l;
use crate::core::macro_model::{Macro, MacroSize, MacroStep, macro_keys};

/// Functions inside functions, at most this deep (a script calling itself stops here).
const DEEPEST: usize = 8;
/// The screen sizes a script's points are measured against, smallest first.
const SCREENS: [(i32, i32); 6] = [(1280, 720), (1366, 768), (1600, 900), (1920, 1080), (2560, 1440), (3840, 2160)];

#[derive(Clone, Debug)]
struct Line {
    no: usize,
    indent: usize,
    code: String,
}

#[derive(Clone, Debug)]
enum Val {
    Num(f64),
    Text(String),
}

#[derive(Default)]
struct Ctx {
    /// What each name a module was imported as stands for ("kb" → "keyboard"), and the bare names
    /// `from x import y` brought in ("sleep" → "time").
    alias: HashMap<String, String>,
    vars: HashMap<String, Val>,
    defs: HashMap<String, Vec<Line>>,
    /// Where the mouse is (screen pixels), for a click that names no point.
    at: Option<(f64, f64)>,
    left_out: Vec<String>,
}

/// The script as a macro named `name`, and the lines left out ("12: if hp < 50:").
pub fn from_python(text: &str, name: &str) -> Res<(Macro, Vec<String>)> {
    let lines = lines_of(text);
    let mut ctx = Ctx::default();
    let mut steps = Vec::new();
    run(&lines, &mut ctx, &mut steps, 0);
    if !steps.iter().any(|s| s.kind != "loop" && s.kind != "endLoop") {
        return Err(Fail::user(l::t("That script has nothing Bloxkeep can play.")));
    }
    // The points as places on the window: of the smallest usual screen that holds them all.
    let (mut wide, mut tall) = (0f64, 0f64);
    for s in &steps {
        if let (Some(x), Some(y)) = (s.x, s.y) {
            wide = wide.max(x);
            tall = tall.max(y);
        }
    }
    let (w, h) = SCREENS.iter().copied().find(|(w, h)| wide < *w as f64 && tall < *h as f64 && *w >= 1920).unwrap_or(*SCREENS.last().unwrap_or(&(1920, 1080)));
    for s in steps.iter_mut() {
        if let (Some(x), Some(y)) = (s.x, s.y) {
            s.x = Some((x / w as f64).clamp(0., 1.));
            s.y = Some((y / h as f64).clamp(0., 1.));
        }
    }
    steps.truncate(crate::core::macro_model::macro_rules::MAX_STEPS as usize);
    // (A loop the cut left open is closed.)
    let open = steps.iter().fold(0i32, |n, s| n + (s.kind == "loop") as i32 - (s.kind == "endLoop") as i32);
    for _ in 0..open.max(0) {
        steps.push(step("endLoop"));
    }
    let m = Macro { name: name.into(), steps, recorded_size: Some(MacroSize { w, h }), max_minutes: 240, ..Default::default() };
    Ok((m, ctx.left_out))
}

/// The script's lines with their indent, comments and blank lines gone; a line ending in "\" or inside open brackets
/// joined with the next.
fn lines_of(text: &str) -> Vec<Line> {
    let mut out: Vec<Line> = Vec::new();
    let mut held: Option<Line> = None;
    for (i, raw) in text.lines().enumerate() {
        let code = strip_comment(raw);
        if let Some(mut h) = held.take() {
            h.code.push(' ');
            h.code.push_str(code.trim());
            if open_brackets(&h.code) > 0 || h.code.ends_with('\\') {
                h.code = h.code.trim_end_matches('\\').to_string();
                held = Some(h);
            } else {
                out.push(h);
            }
            continue;
        }
        if code.trim().is_empty() {
            continue;
        }
        let indent = code.len() - code.trim_start().len() + code.chars().take_while(|c| *c == '\t').count() * 3;
        let line = Line { no: i + 1, indent, code: code.trim().to_string() };
        if open_brackets(&line.code) > 0 || line.code.ends_with('\\') {
            held = Some(Line { code: line.code.trim_end_matches('\\').to_string(), ..line });
        } else {
            out.push(line);
        }
    }
    out.extend(held);
    out
}

fn strip_comment(s: &str) -> String {
    let mut quote: Option<char> = None;
    let mut out = String::new();
    for c in s.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None if c == '\'' || c == '"' => quote = Some(c),
            None if c == '#' => break,
            None => {}
        }
        out.push(c);
    }
    out.trim_end().to_string()
}

fn open_brackets(s: &str) -> i32 {
    let mut n = 0;
    let mut quote: Option<char> = None;
    for c in s.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None if c == '\'' || c == '"' => quote = Some(c),
            None if matches!(c, '(' | '[' | '{') => n += 1,
            None if matches!(c, ')' | ']' | '}') => n -= 1,
            None => {}
        }
    }
    n
}

fn step(kind: &str) -> MacroStep {
    MacroStep { kind: kind.into(), ..Default::default() }
}

/// The lines under a header line `at`: those after it indented further.
fn body(lines: &[Line], at: usize) -> (&[Line], usize) {
    let end = lines[at + 1..].iter().position(|l| l.indent <= lines[at].indent).map(|p| at + 1 + p).unwrap_or(lines.len());
    (&lines[at + 1..end], end)
}

fn run(lines: &[Line], ctx: &mut Ctx, out: &mut Vec<MacroStep>, depth: usize) {
    let mut i = 0;
    while i < lines.len() {
        let code = lines[i].code.clone();
        let no = lines[i].no;
        if code.ends_with(':') {
            let (inner, end) = body(lines, i);
            let head = code.trim_end_matches(':').trim();
            let word = head.split(|c: char| !c.is_alphanumeric() && c != '_').next().unwrap_or_default();
            match word {
                "def" => {
                    let name = head[3..].trim().split('(').next().unwrap_or_default().trim().to_string();
                    ctx.defs.insert(name, inner.to_vec());
                }
                "while" => {
                    let what = head[5..].trim();
                    if !matches!(what, "True" | "1" | "(True)") {
                        ctx.left_out.push(format!("{no}: {code} → {}", l::t("played until stopped")));
                    }
                    let mut s = step("loop");
                    s.times = Some(0);
                    out.push(s);
                    run(inner, ctx, out, depth);
                    out.push(step("endLoop"));
                }
                "for" => {
                    let n = head.split(" in ").nth(1).and_then(|r| range_count(r.trim(), ctx));
                    let mut s = step("loop");
                    s.times = Some(n.unwrap_or(0).clamp(0, 100_000) as i32);
                    if n.is_none() {
                        ctx.left_out.push(format!("{no}: {code} → {}", l::t("played until stopped")));
                    }
                    out.push(s);
                    run(inner, ctx, out, depth);
                    out.push(step("endLoop"));
                }
                // The script's own start, and blocks that only guard what they hold: played as they are.
                "if" if head.contains("__name__") => run(inner, ctx, out, depth),
                "try" | "with" | "finally" => run(inner, ctx, out, depth),
                "class" => {}
                _ => ctx.left_out.push(format!("{no}: {code}")),
            }
            i = end;
            continue;
        }
        line(&code, no, ctx, out, depth);
        i += 1;
    }
}

fn line(code: &str, no: usize, ctx: &mut Ctx, out: &mut Vec<MacroStep>, depth: usize) {
    let first = code.split_whitespace().next().unwrap_or_default();
    match first {
        "import" => {
            for part in code[6..].split(',') {
                let mut w = part.split(" as ").map(str::trim);
                let module = w.next().unwrap_or_default().to_string();
                let alias = w.next().map(str::to_string).unwrap_or_else(|| module.clone());
                ctx.alias.insert(alias, module);
            }
            return;
        }
        "from" => {
            let mut w = code[4..].splitn(2, " import ");
            let module = w.next().unwrap_or_default().trim().to_string();
            for name in w.next().unwrap_or_default().trim_matches(|c| c == '(' || c == ')').split(',') {
                let mut n = name.split(" as ").map(str::trim);
                let real = n.next().unwrap_or_default();
                let alias = n.next().unwrap_or(real);
                if real == "*" {
                    ctx.alias.insert("*".into(), module.clone());
                } else {
                    ctx.alias.insert(alias.to_string(), format!("{module}.{real}"));
                }
            }
            return;
        }
        "pass" | "global" | "nonlocal" | "return" | "continue" | "print" | "assert" | "del" => return,
        _ if code.starts_with("print(") || code.starts_with("logging.") || code.starts_with("os.system(\"cls") => return,
        _ => {}
    }
    // A library's own setting (pyautogui.FAILSAFE = False): nothing to play.
    if let Some((left, _)) = code.split_once('=')
        && left.trim().contains('.')
        && !left.contains('(')
        && !code.contains("==")
    {
        return;
    }
    // name = value
    if let Some((name, value)) = assignment(code) {
        if let Some(v) = value_of(value, ctx) {
            ctx.vars.insert(name.to_string(), v);
        }
        return;
    }
    let Some((func, args)) = call(code) else {
        ctx.left_out.push(format!("{no}: {code}"));
        return;
    };
    // A function of the script's own: its lines, here.
    if let Some(def) = ctx.defs.get(func).cloned() {
        if depth < DEEPEST {
            run(&def, ctx, out, depth + 1);
        }
        return;
    }
    let (module, method) = match func.rsplit_once('.') {
        Some((obj, m)) => (ctx.alias.get(obj).cloned().unwrap_or_else(|| obj.to_string()), m.to_string()),
        None => match ctx.alias.get(func) {
            Some(full) => match full.rsplit_once('.') {
                Some((m, f)) => (m.to_string(), f.to_string()),
                None => (full.clone(), func.to_string()),
            },
            None => (ctx.alias.get("*").cloned().unwrap_or_default(), func.to_string()),
        },
    };
    let (pos, kw) = split_args(args, ctx);
    let arg = |i: usize, key: &str| kw.get(key).cloned().or_else(|| pos.get(i).cloned());
    let num = |v: Option<Val>| match v {
        Some(Val::Num(n)) => Some(n),
        _ => None,
    };
    let text = |v: Option<Val>| match v {
        Some(Val::Text(t)) => Some(t),
        _ => None,
    };
    let keyboard_lib = module == "keyboard";
    let before = out.len();
    match method.as_str() {
        "sleep" => {
            if let Some(s) = num(arg(0, "secs")) {
                wait(out, s);
            }
        }
        // keyboard: press holds, release lets go, send taps.
        "press" if keyboard_lib => keys(out, &text(arg(0, "hotkey")).unwrap_or_default(), Some("down")),
        "release" if keyboard_lib => keys(out, &text(arg(0, "hotkey")).unwrap_or_default(), Some("up")),
        "send" | "press_and_release" if keyboard_lib => keys(out, &text(arg(0, "hotkey")).unwrap_or_default(), None),
        "write" if keyboard_lib => typed(out, &text(arg(0, "text")).unwrap_or_default()),
        // pyautogui / pydirectinput
        "press" => {
            let presses = num(arg(1, "presses")).unwrap_or(1.).clamp(1., 100.) as usize;
            let gap = num(arg(2, "interval")).unwrap_or(0.);
            let names: Vec<String> = match arg(0, "keys") {
                Some(Val::Text(t)) => vec![t],
                _ => list_texts(args),
            };
            for n in 0..presses {
                for k in &names {
                    tap(out, k);
                }
                if gap > 0. && n + 1 < presses {
                    wait(out, gap);
                }
            }
        }
        "keyDown" | "key_down" => {
            if let Some(k) = text(arg(0, "key")).and_then(|k| key_name(&k)) {
                let mut s = step("down");
                s.key = Some(k);
                out.push(s);
            }
        }
        "keyUp" | "key_up" => {
            if let Some(k) = text(arg(0, "key")).and_then(|k| key_name(&k)) {
                let mut s = step("up");
                s.key = Some(k);
                out.push(s);
            }
        }
        "hotkey" => {
            let names: Vec<String> = pos.iter().filter_map(|v| if let Val::Text(t) = v { key_name(t) } else { None }).collect();
            for k in &names {
                let mut s = step("down");
                s.key = Some(k.clone());
                out.push(s);
            }
            for k in names.iter().rev() {
                let mut s = step("up");
                s.key = Some(k.clone());
                out.push(s);
            }
        }
        "typewrite" | "write" | "type" => typed(out, &text(arg(0, "message")).unwrap_or_default()),
        "moveTo" | "move_to" | "dragTo" => {
            if let (Some(x), Some(y)) = (num(arg(0, "x")), num(arg(1, "y"))) {
                ctx.at = Some((x, y));
                let mut s = step("move");
                (s.x, s.y) = (Some(x), Some(y));
                out.push(s);
            }
        }
        // A move by an amount: in Roblox, the camera turning (the cursor held in place).
        "moveRel" | "move" | "move_rel" | "drag" | "dragRel" => {
            let (dx, dy) = (num(arg(0, "xOffset")).unwrap_or(0.), num(arg(1, "yOffset")).unwrap_or(0.));
            if dx != 0. || dy != 0. {
                let mut s = step("camera");
                (s.dx, s.dy) = (Some(dx.round() as i32), Some(dy.round() as i32));
                s.button = Some(if method.starts_with("drag") { "right" } else { "none" }.into());
                s.ms = Some((num(arg(2, "duration")).unwrap_or(0.1) * 1000.).clamp(0., 10_000.) as i32);
                out.push(s);
            }
        }
        "click" | "leftClick" | "rightClick" | "middleClick" | "doubleClick" | "tripleClick" => {
            let point = match (num(arg(0, "x")), num(arg(1, "y"))) {
                (Some(x), Some(y)) => Some((x, y)),
                _ => ctx.at,
            };
            let Some((x, y)) = point else {
                ctx.left_out.push(format!("{no}: {code} → {}", l::t("no point to click")));
                return;
            };
            ctx.at = Some((x, y));
            let button = match method.as_str() {
                "rightClick" => "right".to_string(),
                "middleClick" => "middle".to_string(),
                _ => text(kw.get("button").cloned()).filter(|b| matches!(b.as_str(), "left" | "right" | "middle")).unwrap_or_else(|| "left".into()),
            };
            let clicks = match method.as_str() {
                "doubleClick" => 2,
                "tripleClick" => 3,
                _ => num(kw.get("clicks").cloned()).unwrap_or(1.).clamp(1., 20.) as usize,
            };
            let mut n = clicks;
            while n > 0 {
                let mut s = step("click");
                (s.x, s.y) = (Some(x), Some(y));
                s.button = Some(button.clone());
                if n >= 2 {
                    s.double = Some(true);
                    n -= 2;
                } else {
                    n -= 1;
                }
                out.push(s);
            }
        }
        "mouseDown" | "mouseUp" | "mouse_down" | "mouse_up" => {
            let button = text(arg(2, "button")).or_else(|| text(arg(0, "button"))).unwrap_or_else(|| "left".into());
            let key = match button.as_str() {
                "right" => "MouseRight",
                "middle" => "MouseMiddle",
                _ => "MouseLeft",
            };
            let mut s = step(if method.to_lowercase().ends_with("down") { "down" } else { "up" });
            s.key = Some(key.into());
            out.push(s);
        }
        "scroll" | "vscroll" => {
            if let Some(n) = num(arg(0, "clicks")) {
                // (Windows' wheel moves in 120s: a script may give either.)
                let notches = if n.abs() >= 120. { n / 120. } else { n }.round().clamp(-50., 50.) as i32;
                if notches != 0 {
                    let mut s = step("scroll");
                    s.notches = Some(notches);
                    out.push(s);
                }
            }
        }
        _ => {}
    }
    if out.len() == before && !matches!(method.as_str(), "sleep" | "FAILSAFE" | "PAUSE") {
        ctx.left_out.push(format!("{no}: {code}"));
    }
}

fn wait(out: &mut Vec<MacroStep>, secs: f64) {
    let ms = (secs * 1000.).round().clamp(0., 3_600_000.) as i32;
    if ms == 0 {
        return;
    }
    // (Back-to-back waits are one.)
    if let Some(last) = out.last_mut().filter(|s| s.kind == "wait") {
        last.ms = Some((last.ms.unwrap_or(0) + ms).min(3_600_000));
        return;
    }
    let mut s = step("wait");
    s.ms = Some(ms);
    out.push(s);
}

fn tap(out: &mut Vec<MacroStep>, name: &str) {
    if let Some(k) = key_name(name) {
        let mut s = step("key");
        s.key = Some(k);
        s.ms = Some(60);
        out.push(s);
    }
}

/// keyboard's "ctrl+c" or "w": held together (`how` "down" / "up"), or tapped (None).
fn keys(out: &mut Vec<MacroStep>, combo: &str, how: Option<&str>) {
    let names: Vec<String> = combo.split('+').filter_map(|k| key_name(k.trim())).collect();
    match how {
        Some(kind) => {
            for k in names {
                let mut s = step(kind);
                s.key = Some(k);
                out.push(s);
            }
        }
        None if names.len() == 1 => tap(out, &names[0]),
        None => {
            for k in &names {
                let mut s = step("down");
                s.key = Some(k.clone());
                out.push(s);
            }
            for k in names.iter().rev() {
                let mut s = step("up");
                s.key = Some(k.clone());
                out.push(s);
            }
        }
    }
}

fn typed(out: &mut Vec<MacroStep>, text: &str) {
    if text.is_empty() {
        return;
    }
    let mut s = step("type");
    s.text = Some(text.chars().take(500).collect());
    out.push(s);
}

/// A key's name in Python's libraries as Bloxkeep's ("w" → "W", "space" → "Space", "shiftleft" → "Shift").
fn key_name(k: &str) -> Option<String> {
    let low = k.trim().to_lowercase();
    let named = match low.as_str() {
        "space" | " " => "Space",
        "enter" | "return" | "\n" => "Enter",
        "tab" | "\t" => "Tab",
        "backspace" => "Backspace",
        "esc" | "escape" => "Escape",
        "shift" | "shiftleft" | "shiftright" | "left shift" | "right shift" => "Shift",
        "ctrl" | "ctrlleft" | "ctrlright" | "control" | "left ctrl" | "right ctrl" => "Ctrl",
        "alt" | "altleft" | "altright" | "left alt" | "right alt" => "Alt",
        "up" | "down" | "left" | "right" | "insert" | "delete" | "home" | "end" => return Some(cap(&low)),
        "del" => "Delete",
        "pageup" | "pgup" | "page up" => "PageUp",
        "pagedown" | "pgdn" | "page down" => "PageDown",
        "-" => "Minus",
        "=" => "Equals",
        "[" => "LeftBracket",
        "]" => "RightBracket",
        ";" => "Semicolon",
        "'" => "Quote",
        "," => "Comma",
        "." => "Period",
        "/" => "Slash",
        "\\" => "Backslash",
        "`" => "Backquote",
        _ => "",
    };
    let name = if !named.is_empty() {
        named.to_string()
    } else if let Some(n) = low.strip_prefix("num").filter(|n| n.len() == 1 && n.chars().all(|c| c.is_ascii_digit())) {
        format!("Numpad{n}")
    } else if low.starts_with('f') && low.len() <= 3 && low[1..].parse::<u8>().is_ok() {
        low.to_uppercase()
    } else if low.chars().count() == 1 {
        low.to_uppercase()
    } else {
        return None;
    };
    macro_keys::contains(&name).then_some(name)
}

fn cap(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// "x = 5" → ("x", "5"); not "x == 5" nor a keyword argument inside a call.
fn assignment(code: &str) -> Option<(&str, &str)> {
    let (name, value) = code.split_once('=')?;
    let name = name.trim();
    if value.starts_with('=') || name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_') || name.chars().next()?.is_ascii_digit() {
        return None;
    }
    Some((name, value.trim()))
}

/// "pyautogui.click(10, 20)" → ("pyautogui.click", "10, 20").
fn call(code: &str) -> Option<(&str, &str)> {
    let open = code.find('(')?;
    let func = code[..open].trim();
    if func.is_empty() || !func.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.') {
        return None;
    }
    let close = code.rfind(')')?;
    (close > open && code[close + 1..].trim().is_empty()).then(|| (func, &code[open + 1..close]))
}

fn split_top(args: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0;
    let mut quote: Option<char> = None;
    for c in args.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None if c == '\'' || c == '"' => quote = Some(c),
            None if matches!(c, '(' | '[' | '{') => depth += 1,
            None if matches!(c, ')' | ']' | '}') => depth -= 1,
            None if c == ',' && depth == 0 => {
                out.push(std::mem::take(&mut cur).trim().to_string());
                continue;
            }
            None => {}
        }
        cur.push(c);
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

fn split_args(args: &str, ctx: &Ctx) -> (Vec<Val>, HashMap<String, Val>) {
    let (mut pos, mut kw) = (Vec::new(), HashMap::new());
    for a in split_top(args) {
        match assignment(&a) {
            Some((k, v)) => {
                if let Some(v) = value_of(v, ctx) {
                    kw.insert(k.to_string(), v);
                }
            }
            // (A value Bloxkeep can't work out keeps its place.)
            None => pos.push(value_of(&a, ctx).unwrap_or(Val::Text(String::new()))),
        }
    }
    (pos, kw)
}

/// The texts of a list argument: press(['w', 'a']).
fn list_texts(args: &str) -> Vec<String> {
    let inner = args.trim().trim_start_matches('[').trim_end_matches(']');
    split_top(inner).into_iter().filter_map(|t| unquote(&t)).collect()
}

fn unquote(s: &str) -> Option<String> {
    let s = s.trim();
    let s = s.strip_prefix(['r', 'f', 'b', 'u']).filter(|r| r.starts_with(['\'', '"'])).unwrap_or(s);
    let q = s.chars().next().filter(|c| *c == '\'' || *c == '"')?;
    let inner = s.strip_prefix(q)?.strip_suffix(q)?;
    Some(inner.replace("\\n", "\n").replace("\\t", "\t").replace("\\'", "'").replace("\\\"", "\"").replace("\\\\", "\\"))
}

/// A value worked out by hand, never run: a number, a text, a name set before, random.uniform / randint (their middle), or two of those joined
/// by + - * /.
fn value_of(s: &str, ctx: &Ctx) -> Option<Val> {
    let s = s.trim();
    let s = s.strip_prefix('(').and_then(|r| r.strip_suffix(')')).filter(|r| open_brackets(r) == 0).unwrap_or(s).trim();
    if let Some(t) = unquote(s) {
        return Some(Val::Text(t));
    }
    if let Ok(n) = s.replace('_', "").parse::<f64>() {
        return Some(Val::Num(n));
    }
    if let Some(v) = ctx.vars.get(s) {
        return Some(v.clone());
    }
    if let Some((f, args)) = call(s)
        && matches!(f.rsplit('.').next(), Some("uniform" | "randint" | "randrange"))
    {
        let parts: Vec<f64> = split_top(args).iter().filter_map(|a| if let Some(Val::Num(n)) = value_of(a, ctx) { Some(n) } else { None }).collect();
        if parts.len() >= 2 {
            return Some(Val::Num((parts[0] + parts[1]) / 2.));
        }
    }
    for op in ['+', '-', '*', '/'] {
        if let Some((a, b)) = s.rsplit_once(op).filter(|(a, _)| !a.trim().is_empty()) {
            if let (Some(Val::Num(x)), Some(Val::Num(y))) = (value_of(a, ctx), value_of(b, ctx)) {
                return Some(Val::Num(match op {
                    '+' => x + y,
                    '-' => x - y,
                    '*' => x * y,
                    _ if y != 0. => x / y,
                    _ => return None,
                }));
            }
        }
    }
    None
}

/// How many times "range(5)", "range(2, 7)" or a number set before runs.
fn range_count(r: &str, ctx: &Ctx) -> Option<i64> {
    let (f, args) = call(r)?;
    if f != "range" {
        return None;
    }
    let n: Vec<f64> = split_top(args).iter().filter_map(|a| if let Some(Val::Num(n)) = value_of(a, ctx) { Some(n) } else { None }).collect();
    match n.as_slice() {
        [b] => Some(*b as i64),
        [a, b] => Some((*b - *a) as i64),
        [a, b, s] if *s != 0. => Some(((*b - *a) / *s).ceil() as i64),
        _ => None,
    }
    .map(|n| n.max(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(m: &Macro) -> Vec<String> {
        m.steps
            .iter()
            .map(|s| match s.kind.as_str() {
                "key" | "down" | "up" => format!("{}:{}", s.kind, s.key.clone().unwrap_or_default()),
                "wait" => format!("wait:{}", s.ms.unwrap_or(0)),
                "loop" => format!("loop:{}", s.times.unwrap_or(-1)),
                "click" => format!("click:{}:{:.2},{:.2}", s.button.clone().unwrap_or_default(), s.x.unwrap_or(0.), s.y.unwrap_or(0.)),
                k => k.to_string(),
            })
            .collect()
    }

    #[test]
    fn a_farming_script() {
        let script = r#"
import pyautogui as pag
import time, random
from pydirectinput import keyDown, keyUp

DELAY = 0.5  # seconds

def attack():
    pag.click(960, 540)
    time.sleep(DELAY)

if __name__ == "__main__":
    time.sleep(3)
    while True:
        keyDown('w')
        time.sleep(random.uniform(1, 2))
        keyUp('w')
        for _ in range(3):
            attack()
        pag.press('e')
        if pag.locateOnScreen('boss.png'):
            pag.press('q')
"#;
        let (m, left) = from_python(script, "Farm").unwrap();
        assert_eq!(
            kinds(&m),
            [
                "wait:3000", "loop:0", "down:W", "wait:1500", "up:W", "loop:3", "click:left:0.50,0.50", "wait:500", "endLoop", "key:E", "endLoop"
            ]
        );
        assert_eq!(m.recorded_size.as_ref().map(|s| (s.w, s.h)), Some((1920, 1080)));
        assert_eq!(left.len(), 1);
        assert!(left[0].starts_with("21: if pag.locateOnScreen"), "{left:?}");
    }

    #[test]
    fn the_keyboard_library() {
        let script = "import keyboard\nkeyboard.press('shift')\nkeyboard.send('ctrl+c')\nkeyboard.release('shift')\nkeyboard.write('gg')\n";
        let (m, left) = from_python(script, "k").unwrap();
        assert_eq!(kinds(&m), ["down:Shift", "down:Ctrl", "down:C", "up:C", "up:Ctrl", "up:Shift", "type"]);
        assert!(left.is_empty());
    }

    /// What it makes is a macro Bloxkeep saves as it is (camera turns, scrolls, held keys, typing).
    #[test]
    fn passes_the_save_check() {
        let script = "import pydirectinput as pdi
import pyautogui
pyautogui.FAILSAFE = False
for i in range(0, 10, 2):
    pdi.moveRel(300, 0, duration=0.2)
    pyautogui.scroll(-240)
    pyautogui.hotkey('ctrl', 'shift', 'f5')
    pyautogui.rightClick(2400, 1300)
    pyautogui.write('hello')
";
        let (mut m, left) = from_python(script, "Cam").unwrap();
        assert!(left.is_empty(), "{left:?}");
        assert_eq!(m.steps[0].times, Some(5));
        assert_eq!(m.recorded_size.as_ref().map(|s| (s.w, s.h)), Some((2560, 1440)));
        crate::core::macro_model::macro_rules::check(&mut m, &std::collections::HashSet::new(), &[]).unwrap();
    }

    #[test]
    fn nothing_to_play() {
        assert!(from_python("import os\nprint('hi')\n", "x").is_err());
    }
}
