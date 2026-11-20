// Roblox's binary model files (.rbxm, "<roblox!"), read far enough for an emote: an emote's asset holds an Animation
// whose AnimationId names its KeyframeSequence, another such file of Keyframes (each at its Time) holding a tree of
// Poses (a joint's name and the turn and shift of it, a CFrame).
//
// The file: a header, then chunks, each named (INST: the instances of one class; PROP: one property of every
// instance of a class; PRNT: who is whose parent; END), its body LZ4-compressed when its compressed length is not 0.
// Numbers in a chunk are stored "interleaved" (all first bytes, then all second bytes…), big-endian, signed ones
// zigzagged and floats with the sign bit last; referents are each the one before plus a difference.

use std::collections::HashMap;

/// A property's value, of the kinds read (others are left out).
#[derive(Clone, Debug)]
pub enum Value {
    Text(Vec<u8>),
    Float(f32),
    /// A turn (row-major, as Roblox's R00 R01 R02 R10 …) and a shift.
    Frame([f32; 9], [f32; 3]),
}

/// The instances: each one's class, properties and parent (none at the top), by referent.
#[derive(Default)]
pub struct Doc {
    pub class: HashMap<i32, String>,
    pub props: HashMap<i32, HashMap<String, Value>>,
    pub parent: HashMap<i32, i32>,
}

impl Doc {
    pub fn text(&self, at: i32, name: &str) -> Option<&[u8]> {
        match self.props.get(&at)?.get(name)? {
            Value::Text(t) => Some(t),
            _ => None,
        }
    }

    pub fn float(&self, at: i32, name: &str) -> Option<f32> {
        match self.props.get(&at)?.get(name)? {
            Value::Float(f) => Some(*f),
            _ => None,
        }
    }

    pub fn frame(&self, at: i32, name: &str) -> Option<([f32; 9], [f32; 3])> {
        match self.props.get(&at)?.get(name)? {
            Value::Frame(r, p) => Some((*r, *p)),
            _ => None,
        }
    }

    /// The instances of a class.
    pub fn all(&self, class: &str) -> Vec<i32> {
        let mut v: Vec<i32> = self.class.iter().filter(|(_, c)| c.as_str() == class).map(|(r, _)| *r).collect();
        v.sort_unstable();
        v
    }
}

fn u32_at(d: &[u8], i: usize) -> Result<u32, String> {
    d.get(i..i + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).ok_or_else(|| "cut short".to_string())
}

/// An LZ4 block, to `n` bytes.
fn lz4(src: &[u8], n: usize) -> Result<Vec<u8>, String> {
    let bad = || "bad LZ4".to_string();
    let mut out = Vec::with_capacity(n);
    let mut i = 0;
    let more = |i: &mut usize, mut l: usize| -> Result<usize, String> {
        loop {
            let b = *src.get(*i).ok_or_else(bad)?;
            *i += 1;
            l += b as usize;
            if b != 255 {
                return Ok(l);
            }
        }
    };
    while i < src.len() {
        let t = src[i];
        i += 1;
        let mut l = (t >> 4) as usize;
        if l == 15 {
            l = more(&mut i, l)?;
        }
        out.extend_from_slice(src.get(i..i + l).ok_or_else(bad)?);
        i += l;
        if i >= src.len() {
            break;
        }
        let off = *src.get(i).ok_or_else(bad)? as usize | (*src.get(i + 1).ok_or_else(bad)? as usize) << 8;
        i += 2;
        let mut m = (t & 15) as usize;
        if m == 15 {
            m = more(&mut i, m)?;
        }
        m += 4;
        if off == 0 || off > out.len() || out.len() + m > n {
            return Err(bad());
        }
        let s = out.len() - off;
        for k in 0..m {
            out.push(out[s + k]);
        }
    }
    if out.len() != n {
        return Err(bad());
    }
    Ok(out)
}

/// `n` interleaved big-endian words.
fn words(d: &[u8], n: usize) -> Result<Vec<u32>, String> {
    if d.len() < 4 * n {
        return Err("cut short".into());
    }
    Ok((0..n).map(|i| u32::from_be_bytes([d[i], d[n + i], d[2 * n + i], d[3 * n + i]])).collect())
}

fn ints(d: &[u8], n: usize) -> Result<Vec<i32>, String> {
    Ok(words(d, n)?.into_iter().map(|x| ((x >> 1) as i32) ^ -((x & 1) as i32)).collect())
}

fn floats(d: &[u8], n: usize) -> Result<Vec<f32>, String> {
    Ok(words(d, n)?.into_iter().map(|x| f32::from_bits(x.rotate_right(1))).collect())
}

fn referents(d: &[u8], n: usize) -> Result<Vec<i32>, String> {
    let mut acc = 0i32;
    Ok(ints(d, n)?
        .into_iter()
        .map(|x| {
            acc = acc.wrapping_add(x);
            acc
        })
        .collect())
}

/// One of the 24 square turns a CFrame names by number (the right and up ways, each one of +X +Y +Z -X -Y -Z).
fn square_turn(id: u8) -> [f32; 9] {
    let axis = |k: u8| -> [f32; 3] {
        let mut v = [0.; 3];
        v[(k % 3) as usize] = if k < 3 { 1. } else { -1. };
        v
    };
    let k = id.saturating_sub(1);
    let (r, u) = (axis(k / 6), axis(k % 6));
    let b = [r[1] * u[2] - r[2] * u[1], r[2] * u[0] - r[0] * u[2], r[0] * u[1] - r[1] * u[0]];
    // (Columns: right, up, back.)
    [r[0], u[0], b[0], r[1], u[1], b[1], r[2], u[2], b[2]]
}

/// The file's instances, or why it can't be read.
pub fn read(b: &[u8]) -> Result<Doc, String> {
    if !b.starts_with(b"<roblox!") || b.len() < 32 {
        return Err("not a binary Roblox file".into());
    }
    let mut doc = Doc::default();
    let mut ids: HashMap<u32, (String, Vec<i32>)> = HashMap::new();
    let mut i = 32;
    while i + 16 <= b.len() {
        let name = &b[i..i + 4];
        let (cl, ul) = (u32_at(b, i + 4)? as usize, u32_at(b, i + 8)? as usize);
        i += 16;
        let len = if cl > 0 { cl } else { ul };
        let raw = b.get(i..i + len).ok_or("cut short")?;
        i += len;
        if name == b"END\0" {
            break;
        }
        let d = if cl > 0 {
            if raw.starts_with(&[0x28, 0xB5, 0x2F, 0xFD]) {
                return Err("zstd-compressed (not read)".into());
            }
            lz4(raw, ul)?
        } else {
            raw.to_vec()
        };
        match name {
            b"INST" => {
                let cid = u32_at(&d, 0)?;
                let ln = u32_at(&d, 4)? as usize;
                let cname = String::from_utf8_lossy(d.get(8..8 + ln).ok_or("cut short")?).into_owned();
                let j = 8 + ln + 1;
                let n = u32_at(&d, j)? as usize;
                let refs = referents(&d[j + 4..], n)?;
                for &r in &refs {
                    doc.class.insert(r, cname.clone());
                }
                ids.insert(cid, (cname, refs));
            }
            b"PROP" => {
                let cid = u32_at(&d, 0)?;
                let ln = u32_at(&d, 4)? as usize;
                let pname = String::from_utf8_lossy(d.get(8..8 + ln).ok_or("cut short")?).into_owned();
                let j = 8 + ln;
                let ty = *d.get(j).ok_or("cut short")?;
                let body = &d[j + 1..];
                let Some((_, refs)) = ids.get(&cid) else { continue };
                let n = refs.len();
                let vals: Vec<Value> = match ty {
                    0x01 => {
                        let mut v = Vec::with_capacity(n);
                        let mut k = 0;
                        for _ in 0..n {
                            let l = u32_at(body, k)? as usize;
                            v.push(Value::Text(body.get(k + 4..k + 4 + l).ok_or("cut short")?.to_vec()));
                            k += 4 + l;
                        }
                        v
                    }
                    0x04 => floats(body, n)?.into_iter().map(Value::Float).collect(),
                    0x10 => {
                        let mut turns = Vec::with_capacity(n);
                        let mut k = 0;
                        for _ in 0..n {
                            let id = *body.get(k).ok_or("cut short")?;
                            k += 1;
                            if id == 0 {
                                let mut m = [0f32; 9];
                                for (q, x) in m.iter_mut().enumerate() {
                                    let o = k + 4 * q;
                                    *x = f32::from_le_bytes(body.get(o..o + 4).ok_or("cut short")?.try_into().unwrap_or([0; 4]));
                                }
                                k += 36;
                                turns.push(m);
                            } else {
                                turns.push(square_turn(id));
                            }
                        }
                        let xs = floats(&body[k..], n)?;
                        let ys = floats(&body[k + 4 * n..], n)?;
                        let zs = floats(&body[k + 8 * n..], n)?;
                        turns.into_iter().enumerate().map(|(q, m)| Value::Frame(m, [xs[q], ys[q], zs[q]])).collect()
                    }
                    _ => continue,
                };
                for (r, v) in refs.iter().zip(vals) {
                    doc.props.entry(*r).or_default().insert(pname.clone(), v);
                }
            }
            b"PRNT" => {
                let n = u32_at(&d, 1)? as usize;
                let kids = referents(&d[5..], n)?;
                let parents = referents(&d[5 + 4 * n..], n)?;
                for (k, p) in kids.into_iter().zip(parents) {
                    if p >= 0 {
                        doc.parent.insert(k, p);
                    }
                }
            }
            _ => {}
        }
    }
    Ok(doc)
}

/// The number at the end of an asset link ("rbxassetid://123", "http://www.roblox.com/asset/?id=123").
pub fn asset_id(link: &[u8]) -> Option<i64> {
    let s = String::from_utf8_lossy(link);
    let digits: String = s.chars().rev().take_while(|c| c.is_ascii_digit()).collect::<Vec<_>>().into_iter().rev().collect();
    digits.parse().ok().filter(|&n: &i64| n > 0)
}
