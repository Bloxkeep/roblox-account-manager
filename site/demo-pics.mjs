// Real pictures for the try-it page, fetched once by hand (`node site/demo-pics.mjs`) and kept in site/demo-pics/: the
// games' icons and covers from Roblox, Roblox's own famous accounts' faces for the made-up accounts, and the pictures
// the wiki, the price list and the news show. demo.mjs puts them beside the demo, and ui/dev/demo.js shows them instead
// of the drawn stand-ins, so the demo looks like the real app while the page still fetches nothing from outside.
// map.json: each address the mock uses → its file; avatars.json: the faces' files in the accounts' order.
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const out = path.join(root, 'site', 'demo-pics');
fs.mkdirSync(out, { recursive: true });
const mock = fs.readFileSync(path.join(root, 'ui', 'dev', 'mock.js'), 'utf8');
const wiki = fs.readFileSync(path.join(root, 'ui', 'dev', 'wiki-sample.json'), 'utf8');

/** Roblox's own accounts (Roblox, John Doe, Jane Doe, builderman, Shedletsky, Stickmasterluke): public faces, no one private. */
const FACES = [1, 2, 3, 156, 261, 80254];

const json = async url => { const r = await fetch(url); if (!r.ok) throw new Error(`${r.status} ${url}`); return r.json(); };
const map = {};
const ext = type => /jpe?g/.test(type) ? '.jpg' : /webp/.test(type) ? '.webp' : /gif/.test(type) ? '.gif' : /svg/.test(type) ? '.svg' : '.png';
/** Saves a picture under a name made from `key` (the address the mock uses) and maps the key to it. */
async function save(key, from = key) {
  const r = await fetch(from, { headers: { 'user-agent': 'Bloxkeep-site-build' } });
  const type = r.headers.get('content-type') || '';
  if (!r.ok || !/^image\//.test(type)) throw new Error(`${r.status} ${type} ${from}`);
  const name = createHash('sha1').update(key).digest('hex').slice(0, 16) + ext(type);
  fs.writeFileSync(path.join(out, name), Buffer.from(await r.arrayBuffer()));
  map[key] = name;
  return name;
}
async function pool(items, n, fn) {
  const failed = [];
  let i = 0;
  await Promise.all(Array.from({ length: n }, async () => { while (i < items.length) { const it = items[i++]; try { await fn(it); } catch (e) { failed.push(e.message); } } }));
  return failed;
}

// The games: every place the mock names, with the icon and cover addresses it uses for it.
const cdn = 'https://tr.rbxcdn.com/';
const places = new Map();
const place = id => places.get(id) || places.set(id, { icons: [], covers: [] }).get(id);
for (const m of mock.matchAll(/(\d{6,}): \{ name: [^\n]*?iconUrl: `\$\{cdn\}([^`]+)`/g)) place(m[1]).icons.push(cdn + m[2]);
for (const m of mock.matchAll(/(\d{6,}): \{ icon: rbx\('([^']+)'\), cover: rbx\('([^']+)'\)/g)) {
  place(m[1]).icons.push(`${cdn}180DAY-${m[2]}/Image/Png/noFilter`);
  place(m[1]).covers.push(`${cdn}180DAY-${m[3]}/Image/Png/noFilter`);
}
const ids = [...places.keys()];
const universes = {};
await pool(ids, 6, async p => { universes[p] = (await json(`https://apis.roblox.com/universes/v1/places/${p}/universe`)).universeId; });
const us = ids.map(p => universes[p]).filter(Boolean);
const icons = (await json(`https://thumbnails.roblox.com/v1/games/icons?universeIds=${us.join(',')}&size=150x150&format=Png`)).data;
const covers = (await json(`https://thumbnails.roblox.com/v1/games/multiget/thumbnails?universeIds=${us.join(',')}&size=768x432&format=Png&countPerUniverse=1`)).data;
const games = {};
for (const p of ids) {
  const u = universes[p];
  const icon = icons.find(x => x.targetId === u)?.imageUrl, cover = covers.find(x => x.universeId === u)?.thumbnails?.[0]?.imageUrl;
  games[p] = {};
  if (icon) { games[p].icon = await save(`game-icon-${p}`, icon); for (const k of places.get(p).icons) map[k] = games[p].icon; }
  if (cover) { games[p].cover = await save(`game-cover-${p}`, cover); for (const k of places.get(p).covers) map[k] = games[p].cover; }
}

// The faces.
const faces = (await json(`https://thumbnails.roblox.com/v1/users/avatar-headshot?userIds=${FACES.join(',')}&size=150x150&format=Png`)).data;
const avatars = [];
for (const id of FACES) {
  const f = faces.find(x => x.targetId === id);
  if (f?.state === 'Completed') avatars.push(await save(`avatar-${id}`, f.imageUrl));
}

// Everything else from outside: the wiki's, the price list's and the news' pictures.
const OUTSIDE = /https?:\/\/(?:(?:[a-z0-9-]+\.)*(?:wikia\.nocookie\.net|postimg\.cc|discordapp\.(?:com|net))\/|bloxfruitsvalues\.com\/api\/)[^\s"'<>)\]`\\]*/gi;
const rest = [...new Set([...(mock + wiki).matchAll(OUTSIDE)].map(m => m[0]))].filter(u => !map[u]);
const failed = await pool(rest, 12, u => save(u));
const keep = new Set([...Object.values(games).flatMap(g => [g.icon, g.cover]), ...avatars]);
// A wiki's moving pictures come as megabytes each: those keep the drawn stand-in, so the site stays small (~14 MB).
for (const [k, f] of Object.entries(map))
  if (!/^(game|avatar)-/.test(k) && !keep.has(f) && fs.statSync(path.join(out, f)).size > 120_000) { fs.unlinkSync(path.join(out, f)); delete map[k]; }

fs.writeFileSync(path.join(out, 'map.json'), JSON.stringify({ games, avatars, map }, null, 0));
const size = fs.readdirSync(out).reduce((s, f) => s + fs.statSync(path.join(out, f)).size, 0);
console.log(`${Object.keys(map).length} addresses, ${avatars.length} faces, ${(size / 1048576).toFixed(1)} MB; ${failed.length} failed`);
for (const f of failed.slice(0, 15)) console.log('  ' + f);
