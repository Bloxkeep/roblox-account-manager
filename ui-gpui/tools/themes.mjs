// The web page's themes for the GPUI window: app.css's colour tokens, resolved per theme the way the browser cascades
// them (the shared :root rules, then each [data-theme="…"] block in order), written to src/theme_table.rs. Run by hand
// after app.css's themes change:  node ui-gpui/tools/themes.mjs
// It also lists the themes whose words would be hard to read on the GPUI panel (contrast under 4.5, meta under 3).
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const www = path.resolve(here, '../../RobloxManager/wwwroot');
const css = fs.readFileSync(path.join(www, 'app.css'), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
const out = path.resolve(here, '../src/theme_table.rs');

// The groups, as Settings shows them (app.js THEMES).
const appJs = fs.readFileSync(path.join(www, 'app.js'), 'utf8');
const groupsSrc = appJs.match(/const THEMES = (\{[\s\S]*?\n {2}\});/)[1];
const GROUPS = vm.runInNewContext(`(${groupsSrc})`);
const ids = Object.values(GROUPS).flat().filter(id => id !== 'system' && id !== 'custom');

// Top-level rules only (an @media block is skipped whole): [selectors, declarations].
const rules = [];
{
  let i = 0;
  while (i < css.length) {
    const open = css.indexOf('{', i);
    if (open < 0) break;
    const sel = css.slice(i, open).trim();
    let depth = 1, j = open + 1;
    while (j < css.length && depth) { if (css[j] === '{') depth++; else if (css[j] === '}') depth--; j++; }
    if (!sel.startsWith('@')) rules.push([sel, css.slice(open + 1, j - 1)]);
    i = j;
  }
}
const simple = s => s === ':root' || s === '[data-theme]' || /^\[data-theme="[a-z]+"\]$/.test(s);
const decls = body => {
  const out = []; let depth = 0, start = 0, q = null;
  for (let k = 0; k <= body.length; k++) {
    const ch = body[k];
    if (q) { if (ch === q) q = null; continue; }
    if (ch === '"' || ch === "'") q = ch;
    else if (ch === '(') depth++;
    else if (ch === ')') depth--;
    else if ((ch === ';' || k === body.length) && depth === 0) {
      const d = body.slice(start, k).trim(); start = k + 1;
      const c = d.indexOf(':');
      if (c > 0) out.push([d.slice(0, c).trim(), d.slice(c + 1).trim()]);
    }
  }
  return out;
};
function vars(id) {
  const v = {};
  for (const [sel, body] of rules) {
    // Only the plain theme selectors count (the custom theme's light tone shares the light block).
    const list = sel.split(',').map(s => s.trim()).filter(simple);
    if (!list.length) continue;
    if (!list.some(s => s === ':root' || s === '[data-theme]' || s === `[data-theme="${id}"]`)) continue;
    for (const [k, val] of decls(body)) if (k.startsWith('--') || k === 'color-scheme') v[k] = val;
  }
  return v;
}
function resolve(v, text, seen = new Set()) {
  return text.replace(/var\((--[a-z0-9-]+)(?:,\s*([^)]*))?\)/g, (_, name, fb) => {
    if (seen.has(name)) throw new Error(`loop at ${name}`);
    const raw = v[name] ?? fb;
    if (raw === undefined) throw new Error(`no ${name}`);
    return resolve(v, raw, new Set([...seen, name]));
  });
}
// A colour as [r, g, b, a].
function colour(t) {
  t = t.trim();
  if (t === 'transparent') return [0, 0, 0, 0];
  let m = t.match(/^#([0-9a-f]{3,8})$/i);
  if (m) {
    let h = m[1];
    if (h.length <= 4) h = [...h].map(x => x + x).join('');
    const n = [0, 2, 4].map(i => parseInt(h.slice(i, i + 2), 16));
    return [...n, h.length === 8 ? parseInt(h.slice(6, 8), 16) / 255 : 1];
  }
  m = t.match(/^rgba?\(([^)]*)\)$/);
  if (m) {
    const p = m[1].split(/[\s,/]+/).filter(Boolean).map(Number);
    return [p[0], p[1], p[2], p.length > 3 ? p[3] : 1];
  }
  m = t.match(/^color-mix\(in srgb,\s*(.+?)\s+([\d.]+)%,\s*(.+)\)$/);
  if (m) {
    const a = colour(m[1]), b = colour(m[3]), p = +m[2] / 100;
    const al = a[3] * p + b[3] * (1 - p);
    if (!al) return [0, 0, 0, 0];
    return [0, 1, 2].map(i => (a[i] * a[3] * p + b[i] * b[3] * (1 - p)) / al).concat(al);
  }
  throw new Error(`colour? ${t}`);
}
const over = (top, under) => {
  const a = top[3] + under[3] * (1 - top[3]);
  return [0, 1, 2].map(i => (top[i] * top[3] + under[i] * under[3] * (1 - top[3])) / (a || 1)).concat(a);
};
const lum = c => {
  const [r, g, b] = c.slice(0, 3).map(x => (x /= 255) <= 0.04045 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4);
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
};
const contrast = (a, b) => { const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p); return (x + 0.05) / (y + 0.05); };

const KEYS = ['--bg', '--bg-2', '--surface', '--surface-2', '--surface-3', '--raised', '--field', '--card', '--line-2', '--text', '--text-2',
  '--muted', '--dim', '--accent', '--accent-hover', '--accent-ink', '--ok', '--warn', '--online', '--bad', '--knob'];
const rs = c => `(0x${c.slice(0, 3).map(x => Math.round(Math.max(0, Math.min(255, x))).toString(16).padStart(2, '0')).join('')}, ${f32(c[3])})`;
function f32(x) { const s = String(+x.toFixed(3)); return s.includes(".") ? s : s + "."; }
const rows = [];
const warn = [];
for (const id of ids) {
  const v = vars(id);
  const get = k => colour(resolve(v, v[k]));
  const tint = resolve(v, v['--tint']).split(',').map(Number);
  const backdrop = resolve(v, v['--backdrop'] || 'var(--bg)');
  const scene = backdrop.match(/url\("((?:scene|photo)-[a-z]+\.(?:svg|jpg))"\)/)?.[1] || null;
  // The colour under the picture (the last layer), and a veil over it (snowfall's linear-gradient of one colour).
  const under = scene ? colour(backdrop.match(/,\s*(#[0-9a-f]{3,8})\s*$/i)?.[1] || '#000000') : null;
  const veil = scene ? backdrop.match(/^linear-gradient\((rgba\([^)]*\)),/)?.[1] : null;
  // backdrop-filter: blur(Npx) saturate(M%) behind the panels, when the theme has a picture.
  const bm = scene ? String(v['--blur'] || '').match(/blur\(\s*([\d.]+)px\s*\)(?:\s*saturate\(\s*([\d.]+)%\s*\))?/) : null;
  const blur = !!bm;
  const frost = bm ? `Some((${f32(Number(bm[1]))}, ${f32(Number(bm[2] || 100) / 100)}))` : 'None';
  const dark = (v['color-scheme'] || 'dark').trim() === 'dark';
  const c = Object.fromEntries(KEYS.map(k => [k, get(k)]));
  rows.push(`    Look { id: "${id}", dark: ${dark}, scene: ${scene ? `Some("${scene}")` : 'None'}, under: ${under ? rs(under) : '(0, 0.)'}, veil: ${veil ? `Some(${rs(colour(veil))})` : 'None'}, frost: ${frost}, tint: 0x${tint.map(x => x.toString(16).padStart(2, '0')).join('')}, web: [${KEYS.map(k => rs(c[k])).join(', ')}] },`);
  // As theme.rs lays it: a picture theme's panel is --bg-2 (at least FLOOR) over the picture's colour.
  const base = scene ? over(c['--bg'], under) : [...c['--bg'].slice(0, 3), 1];
  const panel = scene ? over([...c['--bg-2'].slice(0, 3), blur ? c['--bg-2'][3] : Math.max(c['--bg-2'][3], 0.72)], base) : over(c['--surface'], base);
  const t = contrast(c['--text'], panel), m = contrast(c['--muted'], panel);
  if (t < 4.5 || m < 3) warn.push(`${id}: text ${t.toFixed(1)}, muted ${m.toFixed(1)}`);
}

const groupRs = Object.entries(GROUPS).map(([g, l]) => `    ("${g}", &[${l.map(x => `"${x}"`).join(', ')}]),`).join('\n');
const scenes = [...new Set(rows.map(r => r.match(/Some\("((?:scene|photo)-[a-z]+\.(?:svg|jpg))"\)/)?.[1]).filter(Boolean))].sort();
const sceneRs = scenes.map(s => `        "${s}" => include_bytes!("../../RobloxManager/wwwroot/${s}"),`).join('\n');
fs.writeFileSync(out, `// Generated by tools/themes.mjs from RobloxManager/wwwroot/app.css (the web page's themes): don't edit by hand.
// Each theme's colour tokens as the browser resolves them, (rgb, alpha), in the order of \`WEB\` in theme.rs.
#![allow(clippy::all)]

pub struct Look {
    pub id: &'static str,
    pub dark: bool,
    /// The picture behind the window (wwwroot's scene-*.svg or photo-*.jpg), the colour under it and a veil over it.
    pub scene: Option<&'static str>,
    pub under: (u32, f32),
    pub veil: Option<(u32, f32)>,
    /// What the web page did to the picture behind its panels (backdrop-filter): blur (px) and saturate (1 = as is).
    pub frost: Option<(f32, f32)>,
    /// --tint: the colour of the hairlines and the faint grounds (white on dark themes).
    pub tint: u32,
    pub web: [(u32, f32); ${KEYS.length}],
}

/// Settings › Appearance's groups, in order (app.js THEMES).
pub static GROUPS: &[(&str, &[&str])] = &[
${groupRs}
];

pub static LOOKS: &[Look] = &[
${rows.join('\n')}
];

/// A picture's file, built into the program.
pub fn scene_bytes(name: &str) -> Option<&'static [u8]> {
    Some(match name {
${sceneRs}
        _ => return None,
    })
}
`);
console.log(`theme_table.rs: ${rows.length} themes, ${scenes.length} pictures`);
if (warn.length) console.log('hard to read:\n  ' + warn.join('\n  '));
