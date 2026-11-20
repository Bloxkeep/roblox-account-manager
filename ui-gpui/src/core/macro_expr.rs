// Formulas in a Set variable step: numbers, counters and settings by name ("coins", or "{coins}" as in an alert's
// text), + - * / % ^, brackets, and a few functions: random(a, b), round(x), round(x, digits), floor, ceil, abs, min,
// max, sqrt. Comparisons give 1 or 0 (coins >= 100), as do and, or, not. Parsed at once into a tree, so a mistake shows
// when the macro is saved, not when it plays.
use std::sync::LazyLock;

use rand::{Rng, RngCore};
use regex::Regex;

use crate::core::base::Fail;
use crate::core::l;
use crate::core::macro_model::net;

/// A formula that can't be worked out while a macro plays (a division by 0, a missing number).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MacroExprError(pub String);

impl std::fmt::Display for MacroExprError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for MacroExprError {}

impl From<MacroExprError> for Fail {
    fn from(e: MacroExprError) -> Fail {
        Fail::Other(e.0)
    }
}

pub const MAX_LENGTH: i32 = 200;

#[derive(Clone, Debug)]
enum Node {
    Num(f64),
    Ref(String),
    Neg(Box<Node>),
    Not(Box<Node>),
    Bin(char, Box<Node>, Box<Node>),
    Cmp(String, Box<Node>, Box<Node>),
    Call(String, Vec<Node>),
}

/// Each function and how many numbers it takes (looked up case aside).
const FUNCTIONS: [(&str, usize, usize); 8] =
    [("random", 0, 2), ("round", 1, 2), ("floor", 1, 1), ("ceil", 1, 1), ("abs", 1, 1), ("min", 1, 20), ("max", 1, 20), ("sqrt", 1, 1)];

/// The formula's mistake, near where it is, or null when it reads well.
pub fn problem(text: &str) -> Option<String> {
    parse(text).err()
}

/// Works the formula out; `value` gives a name's number (an error when there's none: the caller's own kind, which
/// takes a MacroExprError too). `random`: None for the thread's own.
pub fn eval<E: From<MacroExprError>>(text: &str, value: &mut dyn FnMut(&str) -> Result<f64, E>, random: Option<&mut dyn RngCore>) -> Result<f64, E> {
    let tree = parse(text).map_err(|m| E::from(MacroExprError(m)))?;
    let mut local;
    let random: &mut dyn RngCore = match random {
        Some(r) => r,
        None => {
            local = rand::thread_rng();
            &mut local
        }
    };
    let result = run(&tree, value, random)?;
    if !result.is_finite() {
        return Err(E::from(MacroExprError(l::t("The math divides by 0 or gets too big."))));
    }
    Ok(result)
}

fn run<E: From<MacroExprError>>(n: &Node, value: &mut dyn FnMut(&str) -> Result<f64, E>, random: &mut dyn RngCore) -> Result<f64, E> {
    Ok(match n {
        Node::Num(x) => *x,
        Node::Ref(name) => value(name)?,
        Node::Neg(x) => -run(x, value, random)?,
        Node::Not(x) => {
            if run(x, value, random)? == 0.0 {
                1.0
            } else {
                0.0
            }
        }
        Node::Bin('&', l, r) => {
            if run(l, value, random)? != 0.0 && run(r, value, random)? != 0.0 {
                1.0
            } else {
                0.0
            }
        }
        Node::Bin('|', l, r) => {
            if run(l, value, random)? != 0.0 || run(r, value, random)? != 0.0 {
                1.0
            } else {
                0.0
            }
        }
        Node::Bin(op, l, r) => {
            let a = run(l, value, random)?;
            let b = run(r, value, random)?;
            apply(*op, a, b).map_err(E::from)?
        }
        Node::Cmp(op, l, r) => {
            let a = run(l, value, random)?;
            let b = run(r, value, random)?;
            if crate::core::macro_player::compare(a, op, b) { 1.0 } else { 0.0 }
        }
        Node::Call(fn_name, args) => {
            let mut a = Vec::with_capacity(args.len());
            for arg in args {
                a.push(run(arg, value, random)?);
            }
            function(fn_name, &a, random).map_err(E::from)?
        }
    })
}

fn apply(op: char, a: f64, b: f64) -> Result<f64, MacroExprError> {
    Ok(match op {
        '+' => a + b,
        '-' => a - b,
        '*' => a * b,
        '/' => {
            if b == 0.0 {
                return Err(MacroExprError(l::t("The math divides by 0 or gets too big.")));
            }
            a / b
        }
        // A remainder that is never below 0 (-1 % 5 = 4), as a counter going round wants.
        '%' => {
            if b == 0.0 {
                return Err(MacroExprError(l::t("The math divides by 0 or gets too big.")));
            }
            ((a % b) + b) % b
        }
        '^' => a.powf(b),
        _ => 0.0,
    })
}

/// Math.Round(x, digits, MidpointRounding.AwayFromZero), as .NET works it out.
fn round_away(x: f64, digits: i32) -> f64 {
    if x.abs() < 1e16 {
        let power10 = 10f64.powi(digits);
        (x * power10).round() / power10
    } else {
        x
    }
}

fn function(fn_name: &str, a: &[f64], random: &mut dyn RngCore) -> Result<f64, MacroExprError> {
    Ok(match fn_name.to_lowercase().as_str() {
        "random" => {
            if a.is_empty() {
                return Ok(random.r#gen::<f64>());
            }
            let (mut lo, mut hi) = if a.len() == 1 { (1.0, a[0]) } else { (a[0], a[1]) };
            if lo > hi {
                (lo, hi) = (hi, lo);
            }
            // Whole numbers both ends: a whole number between them, both included (random(1, 6) is a die).
            if lo == lo.floor() && hi == hi.floor() && hi - lo < i32::MAX as f64 {
                return Ok(lo + random.gen_range(0..((hi - lo) as i64 + 1)) as f64);
            }
            lo + random.r#gen::<f64>() * (hi - lo)
        }
        "round" => {
            if a.len() == 2 {
                round_away(a[0], (a[1] as i32).clamp(0, 10))
            } else {
                a[0].round()
            }
        }
        "floor" => a[0].floor(),
        "ceil" => a[0].ceil(),
        "abs" => a[0].abs(),
        // Enumerable.Min: NaN as soon as one is.
        "min" => {
            let mut m = a[0];
            for &x in a {
                if x.is_nan() {
                    return Ok(x);
                }
                if x < m {
                    m = x;
                }
            }
            m
        }
        // Enumerable.Max: NaN only when every one is.
        "max" => {
            let mut m = a[0];
            for &x in &a[1..] {
                if x > m || m.is_nan() {
                    m = x;
                }
            }
            m
        }
        "sqrt" => {
            if a[0] < 0.0 {
                return Err(MacroExprError(l::t("The math divides by 0 or gets too big.")));
            }
            a[0].sqrt()
        }
        _ => 0.0,
    })
}

// ---- Reading -------------------------------------------------------------------------------------------------

fn parse(text: &str) -> Result<Node, String> {
    if net::len16(text) > MAX_LENGTH as usize {
        return Err(l::f("The math can be up to {0} characters.", &[&MAX_LENGTH]));
    }
    let mut p = Parser { s: text.chars().collect(), at: 0, depth: 0 };
    let tree = p.or()?;
    p.skip();
    if !p.end() {
        return Err(p.wrong());
    }
    Ok(tree)
}

/// char.IsLetter.
fn is_letter(c: char) -> bool {
    c.is_alphabetic()
}

/// char.IsDigit.
fn is_digit(c: char) -> bool {
    c.is_numeric()
}

struct Parser {
    s: Vec<char>,
    at: usize,
    depth: i32,
}

impl Parser {
    fn end(&self) -> bool {
        self.at >= self.s.len()
    }

    fn skip(&mut self) {
        while self.at < self.s.len() && self.s[self.at].is_whitespace() {
            self.at += 1;
        }
    }

    fn wrong(&self) -> String {
        if self.end() {
            return l::t("The math stops too soon.");
        }
        let near: String = self.s[self.at..self.s.len().min(self.at + 12)].iter().collect();
        l::f("The math is wrong near “{0}”.", &[&near])
    }

    fn eat(&mut self, token: &str) -> bool {
        self.skip();
        let token: Vec<char> = token.chars().collect();
        if self.at + token.len() > self.s.len() || self.s[self.at..self.at + token.len()] != token[..] {
            return false;
        }
        // A word (and, or, not) is a whole word only.
        let after = self.at + token.len();
        if is_letter(token[0]) && after < self.s.len() && (self.s[after].is_alphanumeric() || self.s[after] == '_') {
            return false;
        }
        self.at += token.len();
        true
    }

    fn or(&mut self) -> Result<Node, String> {
        let mut left = self.and()?;
        while self.eat("||") || self.eat("or") {
            left = Node::Bin('|', Box::new(left), Box::new(self.and()?));
        }
        Ok(left)
    }

    fn and(&mut self) -> Result<Node, String> {
        let mut left = self.compare()?;
        while self.eat("&&") || self.eat("and") {
            left = Node::Bin('&', Box::new(left), Box::new(self.compare()?));
        }
        Ok(left)
    }

    fn compare(&mut self) -> Result<Node, String> {
        let left = self.sum()?;
        for op in ["<=", ">=", "!=", "<>", "==", "<", ">", "="] {
            if self.eat(op) {
                let op = match op {
                    "<>" => "!=",
                    "==" => "=",
                    _ => op,
                };
                return Ok(Node::Cmp(op.to_string(), Box::new(left), Box::new(self.sum()?)));
            }
        }
        Ok(left)
    }

    fn sum(&mut self) -> Result<Node, String> {
        let mut left = self.product()?;
        loop {
            if self.eat("+") {
                left = Node::Bin('+', Box::new(left), Box::new(self.product()?));
            } else if self.eat("-") {
                left = Node::Bin('-', Box::new(left), Box::new(self.product()?));
            } else {
                return Ok(left);
            }
        }
    }

    fn product(&mut self) -> Result<Node, String> {
        let mut left = self.unary()?;
        loop {
            if self.eat("*") || self.eat("×") {
                left = Node::Bin('*', Box::new(left), Box::new(self.unary()?));
            } else if self.eat("/") || self.eat("÷") {
                left = Node::Bin('/', Box::new(left), Box::new(self.unary()?));
            } else if self.eat("%") {
                left = Node::Bin('%', Box::new(left), Box::new(self.unary()?));
            } else {
                return Ok(left);
            }
        }
    }

    fn unary(&mut self) -> Result<Node, String> {
        self.depth += 1;
        if self.depth > 40 {
            return Err(l::t("The math has too many brackets."));
        }
        let node = self.unary_inner();
        self.depth -= 1;
        node
    }

    fn unary_inner(&mut self) -> Result<Node, String> {
        if self.eat("-") {
            return Ok(Node::Neg(Box::new(self.unary()?)));
        }
        if self.eat("+") {
            return self.unary();
        }
        if self.eat("!") || self.eat("not") {
            return Ok(Node::Not(Box::new(self.unary()?)));
        }
        let atom = self.atom()?;
        if self.eat("^") { Ok(Node::Bin('^', Box::new(atom), Box::new(self.unary()?))) } else { Ok(atom) }
    }

    fn atom(&mut self) -> Result<Node, String> {
        self.skip();
        if self.end() {
            return Err(self.wrong());
        }
        let c = self.s[self.at];
        if c == '(' {
            self.at += 1;
            let inner = self.or()?;
            if !self.eat(")") {
                return Err(self.wrong());
            }
            return Ok(inner);
        }
        if c == '{' {
            self.at += 1;
            let name = self.word();
            if name.is_empty() || !self.eat("}") {
                return Err(self.wrong());
            }
            return Ok(Node::Ref(name));
        }
        if is_digit(c) || c == '.' {
            let start = self.at;
            while self.at < self.s.len() && (is_digit(self.s[self.at]) || self.s[self.at] == '.') {
                self.at += 1;
            }
            let digits: String = self.s[start..self.at].iter().collect();
            // NumberStyles.AllowDecimalPoint: digits and one point only.
            let parsed = if digits.chars().all(|d| d.is_ascii_digit() || d == '.') { digits.parse::<f64>().ok() } else { None };
            let Some(n) = parsed else {
                self.at = start;
                return Err(self.wrong());
            };
            return Ok(Node::Num(n));
        }
        if is_letter(c) || c == '_' {
            let start = self.at;
            let name = self.word();
            if self.eat("(") {
                let Some(&(_, min, max)) = FUNCTIONS.iter().find(|(f, _, _)| net::eq_ci(f, &name)) else {
                    self.at = start;
                    return Err(l::f("Can’t read “{0}”. It can use random, round, min, max.", &[&name]));
                };
                let mut args = Vec::new();
                if !self.eat(")") {
                    loop {
                        args.push(self.or()?);
                        if !self.eat(",") {
                            break;
                        }
                    }
                    if !self.eat(")") {
                        return Err(self.wrong());
                    }
                }
                if args.len() < min || args.len() > max {
                    return Err(l::f("“{0}” takes {1} to {2} numbers.", &[&name, &min, &max]));
                }
                return Ok(Node::Call(name, args));
            }
            if net::len16(&name) > 32 {
                self.at = start;
                return Err(self.wrong());
            }
            return Ok(Node::Ref(name));
        }
        Err(self.wrong())
    }

    fn word(&mut self) -> String {
        self.skip();
        let start = self.at;
        if self.at < self.s.len() && (is_letter(self.s[self.at]) || self.s[self.at] == '_') {
            self.at += 1;
            while self.at < self.s.len() && (self.s[self.at].is_alphanumeric() || self.s[self.at] == '_') {
                self.at += 1;
            }
        }
        self.s[start..self.at].iter().collect()
    }
}

// ---- Numbers read off the screen ------------------------------------------------------------------------------

/// C#'s `(?<sign>-)?(?<n>\d[\d,\.]*\d|\d)\s*(?<k>[kKmMbBtT](?![\p{L}]))?`: the look-ahead (not in the regex crate) is
/// checked by hand in `number_in`; when it fails the match is the same without `k`.
static NUMBER_TEXT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?P<sign>-)?(?P<n>\d[\d,\.]*\d|\d)\s*(?P<k>[kKmMbBtT])?").unwrap());
static LETTER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\p{L}").unwrap());

/// The first number in a text read off the screen, as games write them: "1,234", "1.234.567", "12.5", "3.2K",
/// "1.5M", "2B", "Coins: 450". A comma or dot followed by exactly three digits (and no other kind) groups thousands;
/// otherwise the last one is the decimal point. Null when there's none.
pub fn number_in(text: &str) -> Option<f64> {
    let mut from = 0;
    while from <= text.len() {
        let Some(m) = NUMBER_TEXT.captures_at(text, from) else {
            return None;
        };
        let whole = m.get(0)?;
        // (?![\p{L}]) after the suffix: a letter after it means no suffix (the match ends before it).
        let k = m.name("k").filter(|k| !LETTER.is_match(&text[k.end()..]));
        let end = match (m.name("k"), k) {
            (Some(kk), None) => kk.start(),
            _ => whole.end(),
        };
        from = if end > whole.start() { end } else { whole.start() + text[whole.start()..].chars().next().map_or(1, char::len_utf8) };
        let raw = m.name("n").map_or("", |n| n.as_str());
        let Some(n) = number(raw) else {
            continue;
        };
        let scale = match k.map(|k| k.as_str().to_lowercase()).as_deref() {
            Some("k") => 1e3,
            Some("m") => 1e6,
            Some("b") => 1e9,
            Some("t") => 1e12,
            _ => 1.0,
        };
        let n = n * scale;
        return Some(if m.name("sign").is_some() { -n } else { n });
    }
    None
}

fn number(raw: &str) -> Option<f64> {
    let comma = raw.rfind(',');
    let dot = raw.rfind('.');
    let digits = match (comma, dot) {
        (None, None) => raw.to_string(),
        (Some(c), Some(d)) => {
            // Both: the later one is the decimal point.
            let point = if c > d { ',' } else { '.' };
            let group = if point == ',' { '.' } else { ',' };
            raw.replace(group, "").replace(point, ".")
        }
        _ => {
            let mark = if comma.is_some() { ',' } else { '.' };
            let parts: Vec<&str> = raw.split(mark).collect();
            let grouped = parts.iter().skip(1).all(|p| net::len16(p) == 3) && (parts.len() > 2 || mark == ',');
            if grouped {
                parts.concat()
            } else if parts.len() == 2 {
                format!("{}.{}", parts[0], parts[1])
            } else {
                parts.concat()
            }
        }
    };
    // NumberStyles.AllowDecimalPoint: ASCII digits and one point only.
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return None;
    }
    digits.parse::<f64>().ok()
}
