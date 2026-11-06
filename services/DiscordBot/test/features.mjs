// The bot's commands and news (services/DiscordBot/worker) against a fake Discord (its REST API and Gateway) and fake sources (the
// Roblox watch, Roblox's status page, Bloxkeep's releases, Roblox's release notes), run by wrangler dev: each command's
// reply, a signature that doesn't verify refused, each kind of news posted once, in English, only where it was asked for
// (a Roblox update as a card, Roblox's notes edited in once they're out), and /verify's captcha (a PNG only that person
// sees, the code typed into a form, the role given once it's right), /discord (the app's Discord button: an invite that
// never expires, made again once it stops working), and how many people have Bloxkeep open now (the apps' beats,
// /online for anyone, in full with its dashboard for Bloxkeep's developers, the line in the bot's status), and /feed (a
// hidden channel per game feed, what arrives there kept as it came, Message Content refused without the bot stopping).
// No Discord account or token needed. In this folder: npm install, then node features.mjs (about a minute).
import { spawn, execSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { WebSocket, WebSocketServer } from 'ws';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const WORKER = path.join(HERE, '..', 'worker');
const WRANGLER = path.join(HERE, 'node_modules', 'wrangler', 'bin', 'wrangler.js');
// Short: wrangler's object files under a long folder pass Windows' 260-character limit and fail as "internal error".
const PERSIST = path.join(process.env.TEMP || HERE, 'bkf');
const GW = 8790, REST = 8791, SRC = 8792, DEV = 8788;
fs.rmSync(PERSIST, { recursive: true, force: true });
fs.mkdirSync(PERSIST, { recursive: true });

const sleep = ms => new Promise(r => setTimeout(r, ms));
let failures = 0;
const check = (name, ok, detail = '') => { console.log(`${ok ? 'PASS' : 'FAIL'}  ${name}${ok ? '' : `  [${String(detail).slice(0, 300)}]`}`); if (!ok) failures++; };

// A command during a deploy (worker/calls.js): made once more on a new stub when the object was starting over, not for
// any other error.
{
  const { onceMore } = await import('../worker/calls.js');
  let made = 0;
  const reset = Object.assign(new Error('Durable Object reset because its code was updated.'), { retryable: true });
  const got = await onceMore(() => ++made, n => { if (n === 1) throw reset; return `answered by stub ${n}`; });
  check('a command while the bot starts over after a deploy: answered by the new object', got === 'answered by stub 2', got);
  made = 0;
  const other = await onceMore(() => ++made, () => { throw new Error('Discord said no'); }).catch(e => e.message);
  check('…another error: not made again', other === 'Discord said no' && made === 1, `${other} ${made}`);
}
const waitFor = async (cond, ms) => { const end = Date.now() + ms; while (Date.now() < end) { if (cond()) return true; await sleep(50); } return cond(); };

// ── the fake Gateway: hello, ready, heartbeats answered; the custom statuses sent kept, open connections counted ──
const gw = new WebSocketServer({ port: GW });
const statuses = [];
// Each identify's intents; refuseContent: Message Content not turned on (Discord closes 4014 when it's asked for).
const identified = [];
let open = 0, gwSocket = null, gwSeq = 1, refuseContent = false;
// A close as Discord makes it: the connection gone at once, whether or not the other side answers the close.
const closeHard = (ws, code, why) => { ws.close(code, why); setTimeout(() => ws.terminate(), 200); };
const dispatch = (t, d) => gwSocket?.send(JSON.stringify({ op: 0, t, s: ++gwSeq, d }));
gw.on('connection', ws => {
  open++;
  gwSocket = ws;
  ws.on('close', () => { open--; });
  ws.send(JSON.stringify({ op: 10, d: { heartbeat_interval: 30000 } }));
  ws.on('message', raw => {
    const m = JSON.parse(raw);
    if (m.op === 3) statuses.push(m.d.activities?.[0]);
    if (m.op === 1) ws.send(JSON.stringify({ op: 11 }));
    if (m.op === 2) identified.push(m.d.intents);
    if (m.op === 2 && refuseContent && (m.d.intents & (1 << 15))) return closeHard(ws, 4014, 'Disallowed intent(s).');
    if (m.op === 2 || m.op === 6) ws.send(JSON.stringify({ op: 0, t: 'READY', s: 1, d: { session_id: 's1', resume_gateway_url: `ws://127.0.0.1:${GW}`, user: { username: 'TestBot' } } }));
  });
});

// ── the fake REST API: every call kept; channel "nochan" refuses posts, message "gone" was deleted, invites in
//    deadInvites were deleted ──
const calls = [];
// Channels /feed new made in g1, listed from then on.
const madeChannels = [];
const deadInvites = new Set();
// Channels deleted in Discord: asked about, Discord says Unknown Channel.
const goneChannels = new Set();
let msgNo = 0, inviteNo = 0;
const rest = http.createServer((req, res) => {
  let body = '';
  req.on('data', c => { body += c; });
  req.on('end', () => {
    const call = { method: req.method, url: req.url, auth: req.headers.authorization, body: body ? JSON.parse(body) : null, reason: req.headers['x-audit-log-reason'] };
    calls.push(call);
    const send = (status, value) => { res.writeHead(status, { 'Content-Type': 'application/json' }); res.end(JSON.stringify(value)); };
    if (req.method === 'POST' && req.url === '/channels/nochan/messages') return send(403, { message: 'Missing Access', code: 50001 });
    // u1 owns the application: one of Bloxkeep's developers.
    if (req.method === 'GET' && req.url === '/oauth2/applications/@me') return send(200, { id: 'app1', owner: { id: 'u1' } });
    if (req.method === 'GET' && req.url === '/applications/@me') return send(200, { id: 'app1', approximate_guild_count: 3, approximate_user_install_count: 1 });
    if (req.method === 'PATCH' && /\/messages\/gone$/.test(req.url)) return send(404, { message: 'Unknown Message', code: 10008 });
    if (req.method === 'POST' && /^\/channels\/\w+\/invites$/.test(req.url)) return send(200, { code: `inv${++inviteNo}` });
    if (req.method === 'GET' && req.url === '/users/@me') return send(200, { id: 'bot1', username: 'TestBot' });
    // /feed new: the Feeds category, then a channel in it (g2 refuses: the bot can't manage channels there).
    if (req.method === 'GET' && req.url === '/guilds/g2/channels') return send(200, []);
    if (req.method === 'POST' && req.url === '/guilds/g2/channels') return send(403, { message: 'Missing Permissions', code: 50013 });
    if (req.method === 'POST' && req.url === '/guilds/g1/channels') { const c = { ...call.body, id: call.body?.type === 4 ? 'fcat' : `fch${++msgNo}` }; madeChannels.push(c); return send(200, c); }
    // g1's roles, low to high: rv (/verify's), an app's own, rme (u1's), rmod; u0 owns it.
    if (req.method === 'GET' && req.url === '/guilds/g1') return send(200, { id: 'g1', owner_id: 'u0', roles: [
      { id: 'g1', position: 0 }, { id: 'rv', position: 1 }, { id: 'rapp', position: 2, managed: true }, { id: 'rme', position: 3 }, { id: 'rmod', position: 5 }] });
    // The community server's channels: one shows how many have Bloxkeep open.
    if (req.method === 'GET' && req.url === '/guilds/g1/channels') return send(200, [{ id: 'c1', type: 0, name: 'news' }, { id: 'cnt', type: 2, name: '🟢 Using Bloxkeep: 0' }, ...madeChannels]);
    if (req.method === 'GET' && goneChannels.has(req.url.replace('/channels/', ''))) return send(404, { message: 'Unknown Channel', code: 10003 });
    if (req.method === 'GET' && deadInvites.has(req.url.replace('/invites/', ''))) return send(404, { message: 'Unknown Invite', code: 10006 });
    // c3 is a text channel: nothing there can be published.
    if (req.method === 'POST' && /^\/channels\/c3\/messages\/\w+\/crosspost$/.test(req.url)) return send(400, { message: 'Cannot execute action on this channel type', code: 50024 });
    if (req.method === 'POST') return send(200, { id: `m${++msgNo}` });
    send(200, req.method === 'PUT' ? [] : {});
  });
}).listen(REST);
const posts = channel => calls.filter(c => c.method === 'POST' && c.url === `/channels/${channel}/messages`);
// Edits: the boards' (an embed), and Roblox updates given Roblox's notes later (a card).
const patches = () => calls.filter(c => c.method === 'PATCH' && c.body?.embeds);
const noteEdits = () => calls.filter(c => c.method === 'PATCH' && c.body?.flags === 1 << 15);
const published = channel => calls.filter(c => c.method === 'POST' && new RegExp(`^/channels/${channel}/messages/m\\d+/crosspost$`).test(c.url));

// ── the fake sources ──
const build = (version, hash, verdict, channel = 'live') => ({ version, hash, verdict, channel, seenAt: '2026-10-20T16:26:42Z' });
let watch = {
  schema: 1, builds: [build('0.744.0.7441080', 'h744', 'same')],
  live: { version: '0.744.0.7441080', hash: 'h744' }, beta: { version: '0.744.0.7441080', hash: 'h744' },
};
let status = { code: 100 };
let latest = 'v1.0.7';
// GitHub's API not answering (as for 1.0.9 on 2026-10-23) and the notes file at a tag not there yet.
let apiDown = false, fileDown = false;
// The Roblox watch's downloads.json not readable (not written yet, or GitHub's raw files having trouble).
let downloadsDown = false;
// A values website's item list (prices.js), 24 items a page: 30 items, their values times the round; refusing while down.
let priceRound = 1, pricesDown = false;
const priceItems = () => Array.from({ length: 30 }, (_, n) => ({
  itemId: `it${n}`, name: n === 0 ? 'Doghouse (Frame Break)' : `Fruit ${n}`, category: n < 20 ? 'Fruits' : 'Limiteds', rarity: 'Rare',
  image: `https://i.postimg.cc/abc${n}/Fruit-${n}.png,https://bloxfruitsvalues.com/api/bloxfruits/item-image/${n}`,
  metadata: { regValue: 1000 * (n + 1) * priceRound, permValue: n % 2 ? 'N/A' : 5000 * (n + 1), regDemand: 7, regTrend: 'Stable', beliPrice: 250000, robuxPrice: 0, tier: 'Mythical' },
}));
// Roblox's notes, per weekly release, as its Creator Hub page carries them; a release not here has no page yet.
const notes = new Map();
const CHANGES = [
  { ReleaseNotesType: 'Improvements', Status: 'Pending', ReleaseNotesText: 'Adds `Class.RunService:BindToSimulation()|BindToSimulation` to the [API](https://create.roblox.com/docs).' },
  { ReleaseNotesType: 'Improvements', Status: 'Live', ReleaseNotesText: 'Moved `Class.Workspace.GlobalWind|GlobalWind` to the AirProperties category.' },
  { ReleaseNotesType: 'Improvements', Status: 'Pending', ReleaseNotesText: 'Improves frame rate stability when dense areas are streaming in.' },
  { ReleaseNotesType: 'Improvements', Status: 'Pending', ReleaseNotesText: 'In the New Luau Type Solver, improves error messages.' },
  { ReleaseNotesType: 'Fixes', Status: 'Pending', ReleaseNotesText: 'Fixes rare crash accessing networking Stats properties.' },
  { ReleaseNotesType: 'Fixes', Status: 'Live', ReleaseNotesText: 'In Studio, fixes the Explorer losing its place.' },
];
const notesPage = list => `<html><body><script id="__NEXT_DATA__" type="application/json">${JSON.stringify({ props: { pageProps: { data: { releaseNoteContents: { title: 'Release notes', content: list } } } } })}</script></body></html>`;
const NOTES = '- **Robux at once**: shown as you unlock.\n\n#### New\n- details\n\n**Tiếng Việt**\n\n- **Robux hiện ngay**: lúc mở khoá.\n\n#### Mới\n- chi tiết';
// A game's wiki (wiki.js, MediaWiki's API): Category:Weapons has three pages, two a request (the second through
// continue), the third a redirect; other categories are empty. Refusing while down.
let wikiDown = false;
const gamesAsked = [];
const wikiPage = (id, title, text) => ({ pageid: id, title, fullurl: `https://deepwoken.fandom.com/wiki/${title.replace(/ /g, '_')}`,
  thumbnail: { source: `https://static.wikia.nocookie.net/x/images/${id}.png/revision/latest/scale-to-width-down/240` }, revisions: [{ slots: { main: { content: text } } }] });
const wikiAnswer = q => {
  if (q.get('gcmtitle') !== 'Category:Weapons')
    return { batchcomplete: true };
  if (!q.get('gcmcontinue'))
    return { continue: { gcmcontinue: 'page|B|2', continue: 'gcmcontinue||' }, query: { pages: [
      wikiPage(1, 'Adretian Axe', `{{Weapon Infobox\n|image=<gallery>\nA.png|Base\n</gallery>\n|type=Greataxe\n|damage=24\n|scaling=HVY: 7 / HVY: {{cl|alloyed|8}}\n|obtainment=*[[Duke Ishamon Erisia]]\n*Bought from [[Traan Zakshun|Traan]] for {{c|Notes|300}}\n|rarity=TBA\n|hp=50\n}}\n__NOTOC__\nThe '''Adretian Axe''' is a [[Weapons|Heavy Weapon]].\n{{DefaultCrit|Greataxe}}\n==Gallery==\nx`),
      wikiPage(2, 'Bronze Sword', '{{Weapon Infobox|type=Sword|damage=10|rarity=Common}}\n==Description==\nA plain sword that every new player starts the game with.'),
    ] } };
  return { batchcomplete: true, query: { pages: [wikiPage(3, 'Old Axe', '#REDIRECT [[Adretian Axe]]')] } };
};

const src = http.createServer((req, res) => {
  const json = v => { res.writeHead(200, { 'Content-Type': 'application/json' }); res.end(JSON.stringify(v)); };
  if (req.url === '/watch.json') return json(watch);
  if (req.url === '/status.json') return json({ result: { status_overall: { status_code: status.code }, status: ['User', 'Player', 'Creator'].map(name => ({ name, status_code: status.code })), incidents: status.code > 100 ? [{ name: 'Players can\'t join' }] : [] } });
  if (req.url.startsWith('/games?universeIds=')) {
    gamesAsked.push(req.url);
    res.writeHead(200, { 'Content-Type': 'application/json' });
    return res.end(JSON.stringify({ data: req.url.includes('994732206') ? [{ id: 994732206, rootPlaceId: 2753915549, name: '[🎃] Blox Fruits ✨' }] : [] }));
  }
  if (req.url === '/releases/latest') { res.writeHead(302, { Location: `http://127.0.0.1:${SRC}/releases/tag/${latest}` }); return res.end(); }
  // The install files' downloads, as the Roblox watch workflow counts them for the badge.
  if (req.url === '/downloads.json' && downloadsDown) { res.writeHead(404); return res.end('404: Not Found'); }
  if (req.url === '/downloads.json') return json({ installs: 45 });
  if (req.url.startsWith('/wikiapi?')) {
    if (wikiDown) { res.writeHead(503); return res.end(); }
    return json(wikiAnswer(new URL(req.url, 'http://x').searchParams));
  }
  const pg = /^\/prices\?page=(\d+)&limit=24$/.exec(req.url);
  if (pg && pricesDown) { res.writeHead(503); return res.end(); }
  if (pg) { const all = priceItems(), p = Number(pg[1]); return json({ items: all.slice((p - 1) * 24, p * 24), pagination: { page: p, limit: 24, totalItems: all.length, hasMore: p * 24 < all.length } }); }
  const m = /^\/api\/tags\/(.+)$/.exec(req.url);
  if (m && apiDown) { res.writeHead(403); return res.end('{"message":"API rate limit exceeded"}'); }
  if (m) return json({ html_url: `https://github.com/Bloxkeep/roblox-account-manager/releases/tag/${m[1]}`, body: NOTES });
  const f = /^\/raw\/([^/]+)\/RobloxManager\/whatsnew\.json$/.exec(req.url);
  if (f && !fileDown) return json([{ version: f[1].replace(/^v/, ''), highlights: [{ icon: 'lock', en: ['Private servers', 'listed.'], vi: ['Server riêng', 'đã có.'] }] }]);
  const n = /^\/notes\/release-notes-(\d+)$/.exec(req.url);
  if (n && notes.has(Number(n[1]))) { res.writeHead(200, { 'Content-Type': 'text/html' }); return res.end(notesPage(notes.get(Number(n[1])))); }
  res.writeHead(404); res.end();
}).listen(SRC);

// ── signing like Discord ──
const { publicKey, privateKey } = crypto.generateKeyPairSync('ed25519');
const PUBLIC_HEX = publicKey.export({ format: 'der', type: 'spki' }).subarray(-32).toString('hex');
async function interact(payload, { badSignature = false } = {}) {
  const body = JSON.stringify(payload), ts = String(Math.floor(Date.now() / 1000));
  const sig = crypto.sign(null, Buffer.from(ts + body), privateKey).toString('hex');
  const res = await fetch(`http://127.0.0.1:${DEV}/interactions`, {
    method: 'POST', body,
    headers: { 'Content-Type': 'application/json', 'X-Signature-Timestamp': ts, 'X-Signature-Ed25519': badSignature ? sig.replace(/^./, c => (c === 'a' ? 'b' : 'a')) : sig },
  });
  if (res.status !== 200)
    return { status: res.status, json: null };
  // An answer with a file (the captcha's picture) comes as multipart: the JSON part, and the file's bytes.
  if ((res.headers.get('content-type') ?? '').startsWith('multipart/form-data')) {
    const form = await res.formData();
    return { status: res.status, json: JSON.parse(form.get('payload_json')), file: new Uint8Array(await form.get('files[0]').arrayBuffer()) };
  }
  return { status: res.status, json: await res.json() };
}
// A button pressed in a server, and the captcha's form sent, by member u9 unless said otherwise.
const press = (customId, { guild = 'g1', user = 'u9', roles = [] } = {}) => ({
  type: 3, id: 'i', application_id: 'app1', token: 't', locale: 'en-US', guild_locale: 'en-US', guild_id: guild,
  member: { permissions: '0', user: { id: user }, roles }, data: { custom_id: customId, component_type: 2 },
});
const typed = (code, { guild = 'g1', user = 'u9', roles = [] } = {}) => ({
  type: 5, id: 'i', application_id: 'app1', token: 't', locale: 'en-US', guild_locale: 'en-US', guild_id: guild,
  member: { permissions: '0', user: { id: user }, roles },
  data: { custom_id: 'verify-form', components: [{ type: 1, components: [{ type: 4, custom_id: 'code', value: code }] }] },
});
const command = (name, { guild = 'g1', locale = 'vi', perms = '32', roles = [], sub, options = [], resolved } = {}) => ({
  type: 2, id: 'i', application_id: 'app1', token: 't', locale, guild_locale: locale,
  ...(guild ? { guild_id: guild, member: { permissions: perms, user: { id: 'u1' }, roles } } : { user: { id: 'u1' } }),
  data: { id: 'c', name, type: 1, options: sub ? [{ type: 1, name: sub, options }] : [], ...(resolved ? { resolved } : {}) },
});
const opt = (name, value, type = 7) => ({ name, value, type });

// ── wrangler dev ──
let dev;
function startDev(extra = {}) {
  const out = fs.openSync(path.join(PERSIST, 'wrangler-dev.log'), 'a');
  const vars = {
    OBJECT_NAME: 'bot-test', OLD_OBJECT_NAME: '', PRESENCE_EVERY_MS: '300', TRACE_URL: 'http://127.0.0.1:9/',
    GATEWAY_URL: `ws://127.0.0.1:${GW}`, DISCORD_TOKEN: 'test-token', DISCORD_API: `http://127.0.0.1:${REST}`, DISCORD_APP_ID: 'app1', DISCORD_PUBLIC_KEY: PUBLIC_HEX,
    WATCH_URL: `http://127.0.0.1:${SRC}/watch.json`, STATUS_URL: `http://127.0.0.1:${SRC}/status.json`, LATEST_URL: `http://127.0.0.1:${SRC}/releases/latest`,
    RELEASE_API_URL: `http://127.0.0.1:${SRC}/api/tags/`, DOWNLOADS_URL: `http://127.0.0.1:${SRC}/downloads.json`, RELEASE_FILE_URL: `http://127.0.0.1:${SRC}/raw/`, RELEASE_EVERY_MS: '1', DOWNLOADS_EVERY_MS: '0', CAPTCHA_CODE: 'K7MXP', COMMUNITY_GUILD: 'g1', INVITE_RECHECK_MS: '0',
    ONLINE_FOR_MS: '8000', BEAT_BUDGET: '1440', PUBLIC_URL: `http://127.0.0.1:${DEV}`,
    NOTES_URL: `http://127.0.0.1:${SRC}/notes/release-notes-`, NOTES_EVERY_MS: '1', COUNT_EVERY_MS: '3500', LATEST_CACHE_S: '0',
    ROBLOX_GAMES_URL: `http://127.0.0.1:${SRC}/games?universeIds=`, COMMUNITY_EVERY_MS: '0',
    PRICES_API: `http://127.0.0.1:${SRC}/prices`, WIKI_API: `http://127.0.0.1:${SRC}/wikiapi`, WIKI_REQUESTS: '60', WIKI_CACHE_S: '0', ...extra,
  };
  dev = spawn(process.execPath, [WRANGLER, 'dev', '--ip', '127.0.0.1', '--port', String(DEV), '--test-scheduled', '--persist-to', PERSIST,
    ...Object.entries(vars).flatMap(([k, v]) => ['--var', `${k}:${v}`]), '--show-interactive-dev-session=false'],
  { cwd: WORKER, stdio: ['ignore', out, out], env: { ...process.env, WRANGLER_SEND_METRICS: 'false', CI: '1' } });
}
const stopDev = () => { try { execSync(`taskkill /pid ${dev.pid} /t /f`, { stdio: 'ignore' }); } catch {} };
async function devUp() {
  for (let i = 0; i < 120; i++) { try { if ((await fetch(`http://127.0.0.1:${DEV}/status`)).ok) return true; } catch {} await sleep(500); }
  return false;
}
/** The minute's cron, then a moment for what it posts. */
async function minute(wait = 2500) {
  await fetch(`http://127.0.0.1:${DEV}/__scheduled?cron=*+*+*+*+*`).catch(() => {});
  await sleep(wait);
}
const embedOf = call => call?.body?.embeds?.[0];
// A card (a Roblox update): its texts in order, its picture, its buttons' links.
const parts = call => {
  const all = [];
  const walk = c => { all.push(c); (c.components ?? []).forEach(walk); if (c.accessory) walk(c.accessory); };
  (call?.body?.components ?? []).forEach(walk);
  return all;
};
const textsOf = call => parts(call).filter(c => c.type === 10).map(c => c.content);
const headingOf = call => textsOf(call).find(t => t.startsWith('## '))?.slice(3);
const pictureOf = call => parts(call).find(c => c.type === 11)?.media.url.split('/').pop();
const linksOf = call => parts(call).filter(c => c.type === 2).map(c => c.url);

try {
  startDev();
  check('wrangler dev serves the worker', await devUp());

  // Signatures: Discord checks that a wrong one is refused before it accepts the address.
  check('an interaction with a wrong signature is refused', (await interact({ type: 1 }, { badSignature: true })).status === 401);
  check("Discord's check (PING) is answered", (await interact({ type: 1 })).json?.type === 1);

  // The first minute: commands put in place, the sources read, nothing old announced.
  await minute(3500);
  const puts = calls.filter(c => c.method === 'PUT');
  check('the commands are put in place, with the bot token', puts.length === 1 && puts[0].url === '/applications/app1/commands' && puts[0].auth === 'Bot test-token'
    && puts[0].body.map(c => c.name).join() === 'help,download,multi,robloxstatus,notify,setup,online,board,verify,pc,feed', JSON.stringify(puts[0]?.body.map(c => c.name)));
  check('…and the first look announces nothing', calls.filter(c => c.method === 'POST').length === 0, calls.length);
  let latestNow = await (await fetch(`http://127.0.0.1:${DEV}/latest`)).json();
  check('/latest says the newest public Bloxkeep, for the apps to update at once', latestNow.version === '1.0.7', JSON.stringify(latestNow));
  await minute();
  check('…nor are the commands put again when unchanged', calls.filter(c => c.method === 'PUT').length === 1);

  // The custom status under the bot's name moves through what's true now.
  await sleep(1500);
  const lines = [...new Set(statuses.map(a => a?.state))];
  check('the custom status changes, a custom status (type 4) each time', statuses.length >= 3 && statuses.every(a => a?.type === 4) && lines.length >= 3, JSON.stringify(lines));
  check('…with Roblox and Multi-Roblox now, Roblox\'s status, the newest Bloxkeep and the commands',
    ['Roblox 0.744 · Multi-Roblox: No change', 'Roblox status: Working', 'Bloxkeep 1.0.7 · /download', '/help · /notify · /board'].every(l => lines.includes(l)), JSON.stringify(lines));

  // Commands anyone can use; English whatever the server's language (texts.js lang).
  let r = await interact(command('help', { locale: 'vi' }));
  check('/help is in English, even for a Vietnamese server', r.json?.type === 4 && r.json.data.embeds[0].title === "What Bloxkeep's bot can do", JSON.stringify(r.json));
  const helpButtons = r.json?.data.components[0].components ?? [];
  check('…with Download, GitHub and add-to-server buttons', helpButtons.map(b => b.label).join() === 'Download,GitHub,Add to your server'
    && helpButtons[1].url === 'https://github.com/Bloxkeep/roblox-account-manager' && helpButtons[2].url.includes('client_id=app1') && helpButtons[2].url.includes('permissions=19456'), JSON.stringify(helpButtons));
  r = await interact(command('download', { locale: 'en-US' }));
  check('/download names the newest Bloxkeep', r.json?.data.embeds[0].title === 'Bloxkeep 1.0.7', JSON.stringify(r.json?.data.embeds[0]));
  check('…with Download, Website, GitHub and add-to-server buttons', (r.json?.data.components[0].components ?? []).map(b => b.label).join() === 'Download,Website,GitHub,Add to your server', JSON.stringify(r.json?.data.components));
  r = await interact(command('multi', { locale: 'en-US' }));
  const mf = r.json?.data.embeds[0].fields ?? [];
  check('/multi: the live build, "No change", nothing coming', mf[0]?.value === '`0.744.0.7441080`' && mf[1]?.value === '**No change**' && mf[2]?.value === 'Nothing yet', JSON.stringify(mf));
  r = await interact(command('robloxstatus', { locale: 'en-US', guild: null }));
  check('/robloxstatus works in a DM too: all working', r.json?.data.embeds[0].description === 'All of Roblox is working.', JSON.stringify(r.json?.data));

  // Settings: only for people who can manage the server, in a server.
  r = await interact(command('notify', { perms: '0', sub: 'set', options: [opt('channel', 'c1')] }));
  check('/notify set without Manage Server is refused', r.json?.data.flags === 64 && /manage the server/.test(r.json.data.content), JSON.stringify(r.json?.data));
  r = await interact(command('notify', { guild: null, sub: 'show' }));
  check('/notify outside a server is refused', /in a server only/.test(r.json?.data.content ?? ''), JSON.stringify(r.json?.data));

  // g1 (a Vietnamese server, answered in English all the same): everything to c1, pinging r1, a role nobody may mention.
  r = await interact(command('notify', { sub: 'set', options: [opt('channel', 'c1'), opt('role', 'r1', 8)], resolved: { roles: { r1: { id: 'r1', mentionable: false } } } }));
  check('/notify set: a first post in the channel', posts('c1').length === 1 && /Bloxkeep will post/.test(posts('c1')[0].body.content), JSON.stringify(posts('c1')));
  check('…and the reply, seen only by them, says where and warns the role can\'t be pinged', r.json?.data.flags === 64 && r.json.data.content.includes('<#c1>') && r.json.data.content.includes('<@&r1>') && /can't ping it/.test(r.json.data.content), r.json?.data.content);
  r = await interact(command('notify', { sub: 'set', options: [opt('channel', 'nochan')] }));
  check('a channel the bot may not post in: said, and the old choice kept', /can't post in <#nochan>/.test(r.json?.data.content ?? '') && /<#c1>/.test((await interact(command('notify', { sub: 'show' }))).json?.data.content ?? ''), r.json?.data.content);
  // g3 (English): Roblox updates only, to c3.
  await interact(command('notify', { guild: 'g3', locale: 'en-US', sub: 'set', options: [opt('channel', 'c3'), opt('outages', false, 5), opt('bloxkeep', false, 5)] }));
  // g2 (English): a board in b1.
  r = await interact(command('board', { guild: 'g2', locale: 'en-US', sub: 'set', options: [opt('channel', 'b1')] }));
  check('/board set posts the board', posts('b1').length === 1 && embedOf(posts('b1')[0])?.title === 'Roblox and Bloxkeep now' && /<#b1>/.test(r.json?.data.content ?? ''), JSON.stringify(posts('b1')[0]?.body));

  // /verify set only for someone who could give the role by hand: not with Manage Server alone, not a role at or above
  // their highest one, never a role an app manages; nothing posted.
  const MANAGE_ROLES = String(32 + 2 ** 28);
  for (const [why, perms, role] of [['Manage Server alone', '32', 'rv'], ['a role above their highest', MANAGE_ROLES, 'rmod'], ['their highest role itself', MANAGE_ROLES, 'rme'], ['an app\'s role', MANAGE_ROLES, 'rapp']]) {
    r = await interact(command('verify', { perms, roles: ['rme'], sub: 'set', options: [opt('channel', 'v1'), opt('role', role, 8)] }));
    check(`/verify set refused: ${why}`, /Manage Roles/.test(r.json?.data.content ?? '') && r.json.data.flags === 64 && posts('v1').length === 0, r.json?.data.content);
  }
  // /verify in g1: the Verify button in v1, role rv for whoever solves the captcha (its code fixed for the test).
  r = await interact(command('verify', { perms: MANAGE_ROLES, roles: ['rme'], sub: 'set', options: [opt('channel', 'v1'), opt('role', 'rv', 8)] }));
  const panel = posts('v1')[0];
  check('/verify set posts the Verify button, and says the bot needs Manage Roles above the role', panel?.body.components?.[0]?.components?.[0]?.custom_id === 'verify'
    && r.json?.data.flags === 64 && /<#v1>/.test(r.json.data.content) && /<@&rv>/.test(r.json.data.content) && /Manage Roles/.test(r.json.data.content), `${JSON.stringify(panel?.body)} ${r.json?.data.content}`);
  r = await interact(press('verify'));
  const PNG = [137, 80, 78, 71, 13, 10, 26, 10];
  check('Verify: a captcha only they see, its picture a PNG attached to the embed, and a button to type the code',
    r.json?.type === 4 && r.json.data.flags === 64 && r.json.data.embeds[0].image.url === 'attachment://captcha.png' && r.json.data.attachments?.[0]?.filename === 'captcha.png'
    && r.json.data.components[0].components[0].custom_id === 'verify-code' && r.file?.length > 2000 && PNG.every((b, n) => r.file[n] === b), JSON.stringify(r.json));
  if (r.file)
    fs.writeFileSync(path.join(PERSIST, 'captcha.png'), r.file); // to look at
  r = await interact(press('verify-code'));
  check('"Enter the code": the form, for 5 characters', r.json?.type === 9 && r.json.data.custom_id === 'verify-form' && r.json.data.components[0].components[0].max_length === 5, JSON.stringify(r.json));
  const roleGiven = () => calls.filter(c => c.method === 'PUT' && c.url === '/guilds/g1/members/u9/roles/rv');
  r = await interact(typed('WRONG'));
  check('a wrong code: said in place of the captcha (its picture gone), no role', r.json?.type === 7 && /Wrong code/.test(r.json.data.content)
    && r.json.data.attachments?.length === 0 && roleGiven().length === 0, JSON.stringify(r.json));
  r = await interact(typed('K7MXP'));
  check('…and that captcha is used up: the right code now asks for a new one', r.json?.type === 7 && /expired/.test(r.json.data.content) && roleGiven().length === 0, JSON.stringify(r.json));
  await interact(press('verify'));
  r = await interact(typed('k7m xp'));
  check('a new captcha and the right code (any case, spaces left out): the role given, the audit log saying why', r.json?.type === 7 && /welcome/.test(r.json.data.content)
    && roleGiven().length === 1 && /captcha/.test(decodeURIComponent(roleGiven()[0].reason ?? '')), JSON.stringify(r.json));
  r = await interact(press('verify', { roles: ['rv'] }));
  check('someone who has the role already: told so, no captcha', /already verified/.test(r.json?.data.content ?? '') && !r.file, JSON.stringify(r.json));
  for (let n = 0; n < 5; n++)
    await interact(press('verify', { user: 'u8' }));
  r = await interact(press('verify', { user: 'u8' }));
  check('a sixth picture within ten minutes: refused', /Too many tries/.test(r.json?.data.content ?? '') && !r.file, JSON.stringify(r.json));
  r = await interact(press('verify', { guild: 'g3' }));
  check('a server without /verify: the button says verification is off', /Verification is off/.test(r.json?.data.content ?? ''), JSON.stringify(r.json));

  // /discord, the app's Discord button: g1 is the community server, and its /verify channel (v1) where newcomers land.
  const discord = async () => (await fetch(`http://127.0.0.1:${DEV}/discord`, { redirect: 'manual' })).headers.get('location');
  const invitesMade = () => calls.filter(c => c.method === 'POST' && c.url === '/channels/v1/invites');
  let link = await discord();
  check('/discord: to a new invite into the verify channel, one that never expires', link === 'https://discord.gg/inv1' && invitesMade().length === 1
    && invitesMade()[0].body.max_age === 0 && invitesMade()[0].body.max_uses === 0, `${link} ${JSON.stringify(invitesMade().map(c => c.body))}`);
  link = await discord();
  check('…the same invite next time, once Discord says it still works', link === 'https://discord.gg/inv1' && invitesMade().length === 1
    && calls.some(c => c.method === 'GET' && c.url === '/invites/inv1'), link);
  deadInvites.add('inv1');
  link = await discord();
  check('…and a new one once it has stopped working (deleted)', link === 'https://discord.gg/inv2' && invitesMade().length === 2, link);

  // The README's downloads badge: the Roblox watch's count of the install files' downloads.
  const badge = await (await fetch(`http://127.0.0.1:${DEV}/badge/downloads`)).json().catch(() => null);
  check("the downloads badge shows the Roblox watch's count (45)",
    badge?.schemaVersion === 1 && badge.label === 'downloads' && badge.message === '45', JSON.stringify(badge));
  // …and keeps it while that count can't be read (asked every time here), rather than turning to "unknown".
  downloadsDown = true;
  const badgeDown = await (await fetch(`http://127.0.0.1:${DEV}/badge/downloads`)).json().catch(() => null);
  check('…while the count can\'t be read: the last count (45), not "unknown"', badgeDown?.message === '45' && !badgeDown.isError, JSON.stringify(badgeDown));
  downloadsDown = false;

  // Online now: two Bloxkeep apps say they're open (online for 8 s here; for real, one and a half gaps between beats and a
  // minute). The bot takes 1,440 beats a day here (40,000 for real): one app online beats every minute, two every 2.
  const beat = async body => {
    const res = await fetch(`http://127.0.0.1:${DEV}/online/ping`, { method: 'POST', body: JSON.stringify(body), headers: { 'Content-Type': 'application/json' } });
    return { status: res.status, json: await res.json().catch(() => null) };
  };
  const A = 'a'.repeat(32), B = 'B'.repeat(32);
  let b = await beat({ id: A, app: '1.0.8', language: 'vi' });
  check('a beat from a Bloxkeep app: taken, the answer says to send the next in a minute', b.status === 200 && b.json?.next === 60, JSON.stringify(b));
  check("…one that isn't a beat is refused (an id that isn't 32 hex digits, a version that isn't one)",
    (await beat({ id: 'me', app: '1.0.8' })).status === 400 && (await beat({ id: B, app: 'x' })).status === 400 && (await beat({ id: B })).status === 400);
  await beat({ id: B, app: '1.0.8-alpha.2', language: 'en' });
  b = await beat({ id: A, app: '1.0.8', language: 'vi' }); // the same app again: still one
  check('…and with more online, beats further apart, so that they stay within the day\'s budget', b.json?.next === 120, JSON.stringify(b));
  const asDeveloper = command('online', { guild: null, locale: 'en-US' });
  const asSomeoneElse = { ...command('online', { locale: 'en-US', perms: '8' }), member: { permissions: '8', user: { id: 'u2' } } };
  r = await interact(asSomeoneElse);
  const short = { body: r.json?.data }, shortSaid = textsOf(short).join('\n');
  check('/online from someone who is not one of Bloxkeep\'s developers: the card in the channel, how many now, today\'s peak and the chart only',
    r.json?.data.flags === 1 << 15 && headingOf(short)?.startsWith('2 using Bloxkeep now') && /Peak today \*\*2\*\*/.test(shortSaid)
    && parts(short).some(c => c.type === 12) && !/Versions|removed|install|\*\*Bot\*\*/.test(shortSaid) && !JSON.stringify(r.json).includes('Dashboard'), JSON.stringify(r.json));
  r = await interact(asDeveloper);
  const card = { body: r.json?.data }, said = textsOf(card).join('\n');
  check('/online from a developer, in a DM with the bot: a card saying 2 use Bloxkeep now, seen only by them',
    r.json?.data.flags === (64 | 1 << 15) && headingOf(card)?.startsWith('2 using Bloxkeep now'), JSON.stringify(r.json));
  const chart = parts(card).find(c => c.type === 12)?.items?.[0]?.media.url ?? '';
  const drawn = JSON.parse(new URL(chart || 'https://x/?c={}').searchParams.get('c'));
  check('…with today\'s peak and people, the versions (no languages), and the last 7 days\' peaks and people as a chart, today the last bars',
    /Peak today \*\*2\*\*, at <t:\d+:t> · \*\*2\*\* people today/.test(said) && said.includes('**Versions** 1.0.8 **1** · 1.0.8-alpha.2 **1**') && !/Language|\bvi\b/.test(said)
    && chart.startsWith('https://quickchart.io/chart?v=4&') && JSON.stringify(drawn.data?.datasets?.[0]?.data) === '[0,0,0,0,0,0,2]'
    // Days before people were counted: no bar (null), not 0.
    && JSON.stringify(drawn.data?.datasets?.[1]?.data) === '[null,null,null,null,null,null,2]' && drawn.data.labels.at(-1) === 'Today', `${said} ${chart.slice(0, 200)}`);
  check('…and how far the bot reaches: servers and user installs from Discord, the PCs linked',
    /\*\*Bot\*\* \*\*3\*\* servers · \*\*1\*\* user install · \*\*\d+\*\* PCs? linked/.test(said), said);
  const dashboard = linksOf(card)[0] ?? '';
  check('…and a Dashboard button, its link carrying the key', /^http:\/\/127\.0\.0\.1:\d+\/online\?key=[\w-]{24}$/.test(dashboard), dashboard);
  let page = await fetch(dashboard);
  check('the dashboard opens with that key', page.status === 200 && /text\/html/.test(page.headers.get('content-type')) && (await page.text()).includes('<title>Bloxkeep online</title>'), page.status);
  const numbers = async (key = new URL(dashboard).search) => { const res = await fetch(`http://127.0.0.1:${DEV}/online.json${key}`); return res.ok ? res.json() : res.status; };
  let o = await numbers();
  check('…and its numbers: 2 now, today\'s peak 2 and 2 people, the last 30 days', o.now === 2 && o.today?.peak === 2 && o.today?.people === 2
    && o.days?.length === 30 && o.days.at(-1).day === o.today.day, JSON.stringify(o).slice(0, 300));
  check('…but not without the key, or with another', (await fetch(`http://127.0.0.1:${DEV}/online`)).status === 404 && await numbers('?key=nope') === 404 && await numbers('') === 404);
  check('no ids in what the dashboard reads', !JSON.stringify(o).includes(A) && !JSON.stringify(o).toLowerCase().includes(B.toLowerCase()));
  await sleep(1500);
  check('the bot\'s status says how many are using Bloxkeep now', statuses.some(a => a?.state === '2 people using Bloxkeep now'), JSON.stringify([...new Set(statuses.map(a => a?.state))]));
  const renames = () => calls.filter(c => c.method === 'PATCH' && c.url === '/channels/cnt').map(c => c.body.name);
  await minute();
  check('the community server\'s count channel, found by its name, renamed: "🟢 Using Bloxkeep: 2"', JSON.stringify(renames()) === '["🟢 Using Bloxkeep: 2"]', JSON.stringify(renames()));
  b = await beat({ id: B, gone: true });
  await beat({ id: A, app: '1.0.8', language: 'vi' });
  o = await numbers();
  check('an app closing says goodbye: gone at once, the peak stays', b.status === 200 && o.now === 1 && o.today.peak === 2 && o.languages.map(l => l.name).join() === 'vi', JSON.stringify(o).slice(0, 300));
  await minute();
  check('…the count channel not renamed again so soon (Discord allows two renames in ten minutes)', renames().length === 1, JSON.stringify(renames()));
  await sleep(1500);
  await minute();
  check('…and once that time is up: "🟢 Using Bloxkeep: 1"', renames().at(-1) === '🟢 Using Bloxkeep: 1' && renames().length === 2, JSON.stringify(renames()));
  await sleep(8500);
  o = await numbers();
  check('an app that stopped saying so (crashed, PC asleep) is no longer online once its time is up', o.now === 0 && o.today.peak === 2 && o.apps.length === 0, JSON.stringify(o).slice(0, 300));
  // Today's people: each PC once, however often it said so or came back; a new one is one more.
  const C = 'c'.repeat(32);
  await beat({ id: B, app: '1.0.8-alpha.2', language: 'en' });
  await beat({ id: C, app: '1.0.8', language: 'en' });
  await beat({ id: C, app: '1.0.8', language: 'en' });
  o = await numbers();
  check("today's people: each PC once however often it says so or comes back, one more for a new one (3)", o.today.people === 3 && o.days.at(-1).people === 3, JSON.stringify(o.today));
  check('…and no ids among them', !JSON.stringify(o).includes(C) && !JSON.stringify(o).toLowerCase().includes(B.toLowerCase()));
  await beat({ id: B, gone: true });
  await beat({ id: C, gone: true });
  // Who stays: a new install's first open, the day's people by how long they've had it, removals by reason. Each counts
  // once a day per PC, whatever is sent again; nothing per PC is kept.
  const D = 'd'.repeat(32), E = 'e'.repeat(32);
  await beat({ id: D, app: '1.0.13', language: 'en', age: 'new', first: true });
  await beat({ id: D, app: '1.0.13', language: 'en', age: 'new', first: true }); // sent again (no answer came)
  await beat({ id: E, app: '1.0.13', language: 'vi', age: 'old', first: false });
  await beat({ id: D, gone: true, removed: true, why: 'hard' });
  await beat({ id: D, gone: true, removed: true, why: 'hard' });
  await beat({ id: E, gone: true, removed: true });
  await beat({ id: E, gone: true });
  o = await numbers();
  check('new installs, how long they have had it and removals by reason: each PC once a day',
    o.today.installs === 1 && o.today.ages.new === 1 && o.today.ages.old === 1 && o.today.removed === 2 && o.today.why.hard === 1 && o.today.why.none === 1
    && o.today.people === 5, JSON.stringify(o.today));
  check('…shown in /online too', textsOf({ body: (await interact(asDeveloper)).json?.data }).join('\n').includes('**Today** 1 new install · had it a day 1, a week 0, longer 1 · removed 2 (too hard 1)'));
  check('…and no ids among them', !JSON.stringify(o).includes(D) && !JSON.stringify(o).includes(E));
  // After "It didn't work", what didn't: its own reason (an unknown one is no reason).
  const F = 'f'.repeat(32), G = '9'.repeat(32);
  await beat({ id: F, gone: true, removed: true, why: 'antivirus' });
  await beat({ id: G, gone: true, removed: true, why: 'made-up' });
  o = await numbers();
  check("what didn't work counted by its own reason", o.today.removed === 4 && o.today.why.antivirus === 1 && o.today.why.none === 2, JSON.stringify(o.today));
  check('…and named in /online', textsOf({ body: (await interact(asDeveloper)).json?.data }).join('\n').includes('antivirus 1'));

  // What the apps play (online.js communityOf): three PCs in Blox Fruits (universe 994732206), one also in a game only it
  // plays; two on the alts board. A game shows with 3 PCs or more, a radar line with 3 PCs dropped the same way.
  const P1 = '1'.repeat(32), P2 = '2'.repeat(32), P3 = '3'.repeat(32);
  await beat({ id: P1, app: '1.0.19', games: [994732206, 555], alts: 30, board: '  Máy   của Minh ' });
  await beat({ id: P2, app: '1.0.19', games: [994732206], alts: 45, board: 'Farm_PC', drops: [{ game: 994732206, why: 'lost' }] });
  b = await beat({ id: P3, app: '1.0.19', games: [994732206], alts: 4, drops: [{ game: 994732206, why: 'lost' }, { game: 994732206, why: 'nope' }] });
  let c = b.json?.community;
  check('a beat tells what Bloxkeep users play now: games with 3 PCs or more, accounts running in all',
    c?.games?.length === 1 && c.games[0].game === 994732206 && c.games[0].pcs === 3 && c.alts === 79 && !JSON.stringify(c).includes('555'), JSON.stringify(c));
  check('…the alts board: only the PCs that joined it, most first, their names tidied; the one not on it told nothing',
    JSON.stringify(c?.board) === JSON.stringify([{ name: 'Farm_PC', alts: 45 }, { name: 'Máy của Minh', alts: 30 }]) && c.you === null, JSON.stringify(c));
  b = await beat({ id: P1, app: '1.0.19', games: [994732206], alts: 30, board: 'Máy của Minh', drops: [{ game: 994732206, why: 'lost' }] });
  c = b.json?.community;
  check('…a PC on the board is told its place; the radar shows 3 PCs that lost connection there; no id in any of it',
    c?.you?.rank === 2 && c.you.of === 2 && c.radar.length === 1 && c.radar[0].why === 'lost' && c.radar[0].pcs === 3 && c.radar[0].minutes === 15
    && ![P1, P2, P3].some(id => JSON.stringify(c).includes(id)), JSON.stringify(c));
  check("…a board name that could ping isn't taken", (await beat({ id: P3, app: '1.0.19', games: [994732206], alts: 99, board: '@everyone' })).json?.community?.board?.length === 2);
  await minute();
  b = await beat({ id: P2, app: '1.0.19', games: [994732206], alts: 45, board: 'Farm_PC' });
  check('…the game named from Roblox (once), without its emoji and marks', b.json?.community?.games?.[0]?.name === 'Blox Fruits' && b.json.community.games[0].place === 2753915549
    && gamesAsked.length === 1, JSON.stringify(b.json?.community?.games) + ' ' + gamesAsked.length);
  const playCard = textsOf({ body: (await interact(asSomeoneElse)).json?.data }).join('\n');
  check('/online shows them to everyone: accounts running, playing now, most alts now, the radar',
    playCard.includes('**174** accounts running') && playCard.includes('**Playing now** Blox Fruits 3') && playCard.includes('**Most alts now** 1. Farm\\_PC **45** · 2. Máy của Minh **30**')
    && playCard.includes('**Radar** Blox Fruits: 3 PCs lost connection in the last 15 minutes'), playCard);
  for (const id of [P1, P2, P3])
    await beat({ id, gone: true });

  // A build on Roblox's test channel, checked by the watch: "coming", to both news channels, pinging r1 in g1.
  const base = () => ({ c1: posts('c1').length, c3: posts('c3').length, patches: patches().length });
  let before = base();
  watch = { ...watch, builds: [...watch.builds, build('0.745.0.7451000', 'h745', 'changed', 'beta')], beta: { version: '0.745.0.7451000', hash: 'h745' } };
  await minute();
  const coming = posts('c1').at(-1);
  check('a build coming: a card in English, the role pinged above it', posts('c1').length === before.c1 + 1 && coming.body.flags === 1 << 15
    && headingOf(coming) === 'Roblox 0.745 is coming' && textsOf(coming)[0] === '<@&r1>' && coming.body.content === undefined
    && JSON.stringify(coming.body.allowed_mentions) === '{"roles":["r1"]}', JSON.stringify(coming?.body));
  check('…with what it means for Multi-Roblox, its picture and the build', textsOf(coming).includes("Multi-Roblox · **May be blocked**\nRoblox changed the code that keeps it to one window. Try two windows before counting on Multi-Roblox.")
    && pictureOf(coming) === 'update-changed.png' && textsOf(coming).includes("-# `0.745.0.7451000` · `h745` · on Roblox's test channel"), JSON.stringify(textsOf(coming)));
  check("…and, Roblox's notes not out yet, a line saying they'll come", textsOf(coming).some(t => t.includes("Roblox hasn't posted its notes for 0.745 yet"))
    && !linksOf(coming).some(u => u.includes('release-notes')), JSON.stringify(textsOf(coming)));
  check('…the same in the other server, where nobody is pinged', headingOf(posts('c3').at(-1)) === 'Roblox 0.745 is coming' && textsOf(posts('c3').at(-1))[0].startsWith('## ')
    && JSON.stringify(posts('c3').at(-1).body.allowed_mentions) === '{"parse":[]}', JSON.stringify(posts('c3').at(-1)?.body));
  check('…published, so servers following the channel get it too (in a text channel Discord refuses: tried once, nothing more)',
    published('c1').length === 1 && published('c3').length === 1, `${published('c1').length} ${published('c3').length}`);
  check('…and the board edited to show it', patches().length === before.patches + 1 && JSON.stringify(embedOf(patches().at(-1))).includes('0.745.0.7451000'), JSON.stringify(patches().at(-1)?.body));
  before = base();
  await minute();
  check('nothing new: no post, no edit', posts('c1').length === before.c1 && patches().length === before.patches);

  // It goes live: "live", once.
  watch = { ...watch, live: { version: '0.745.0.7451000', hash: 'h745' } };
  before = base();
  await minute();
  check('the build goes live: posted once', posts('c1').length === before.c1 + 1 && headingOf(posts('c1').at(-1)) === 'Roblox 0.745 is live'
    && textsOf(posts('c1').at(-1)).some(t => /^-# `0\.745\.0\.7451000` · `h745` · live <t:\d+:R>$/.test(t)), JSON.stringify(textsOf(posts('c1').at(-1))));
  // A fix within the same release that changes nothing for Multi-Roblox: the board shows it, nobody is told.
  watch = { ...watch, builds: [...watch.builds, build('0.745.0.7451005', 'h7455', 'same')], live: { version: '0.745.0.7451005', hash: 'h7455' }, beta: { version: '0.745.0.7451005', hash: 'h7455' } };
  before = base();
  await minute();
  check('a fix build with no change for Multi-Roblox: no post, the board edited', posts('c1').length === before.c1 && patches().length === before.patches + 1, `${posts('c1').length - before.c1} ${patches().length - before.patches}`);

  // Roblox posts its notes for 0.745: each 0.745 post (coming and live, in both servers) is edited to show them.
  check("no notes yet: no post edited", noteEdits().length === 0, noteEdits().length);
  notes.set(745, CHANGES);
  await minute();
  const edits = noteEdits(), edit = edits.find(c => c.url.startsWith('/channels/c1/') && headingOf(c) === 'Roblox 0.745 is live');
  check("Roblox's notes out: the four 0.745 posts edited", edits.length === 4 && edits.filter(c => c.url.startsWith('/channels/c3/')).length === 2, edits.map(c => c.url).join(' '));
  const whatsNew = textsOf(edit).find(t => t.startsWith("### What's new")) ?? '';
  check('…with the changes a player notices first, links and API paths as plain words', whatsNew === [
    "### What's new", '**Improvements**', '- Moved `GlobalWind` to the AirProperties category.', '- Improves frame rate stability when dense areas are streaming in.',
    '- In the New Luau Type Solver, improves error messages.', '**Fixes**', '- Fixes rare crash accessing networking Stats properties.',
    '- In Studio, fixes the Explorer losing its place.', '-# 5 of 6 changes'].join('\n'), whatsNew);
  check("…a button to Roblox's own notes, the role still named, the time it went live kept", linksOf(edit).includes('https://create.roblox.com/docs/release-notes/release-notes-745')
    && textsOf(edit)[0] === '<@&r1>' && textsOf(edit).some(t => /· live <t:\d+:R>$/.test(t)) && edit.body.flags === 1 << 15, JSON.stringify(edit?.body));
  await minute();
  check('…once only', noteEdits().length === 4, noteEdits().length);

  // Roblox down: posted after two looks say so; working again after two looks, with how long it was down.
  status = { code: 500 };
  before = base();
  await minute();
  check('one look at an outage: not posted yet', posts('c1').length === before.c1);
  await minute();
  const down = posts('c1').at(-1);
  check('two looks: "Roblox is having problems" posted, with the incident', posts('c1').length === before.c1 + 1 && embedOf(down)?.title === 'Roblox is having problems' && embedOf(down)?.description.includes("Players can't join"), JSON.stringify(embedOf(down)));
  check('…not where outages were turned off', posts('c3').length === before.c3);
  status = { code: 100 };
  await minute();
  await minute();
  const up = posts('c1').at(-1);
  check('working again: posted, with how long it was down', embedOf(up)?.title === 'Roblox is working again' && embedOf(up)?.fields.some(f => f.name === 'Down for'), JSON.stringify(embedOf(up)));

  // A new Bloxkeep: its English highlights, with Download and the full notes.
  latest = 'v1.0.8';
  before = base();
  await minute();
  const rel = posts('c1').at(-1);
  check('a new Bloxkeep: posted with its English highlights', posts('c1').length === before.c1 + 1 && embedOf(rel)?.title === 'Bloxkeep 1.0.8 is out'
    && embedOf(rel)?.description === '- **Robux at once**: shown as you unlock.' && rel.body.components?.[0]?.components?.length === 2, JSON.stringify(rel?.body));
  check('…not where Bloxkeep news was turned off', posts('c3').length === before.c3);
  r = await interact(command('download', { locale: 'en-US' }));
  check('/download now names 1.0.8', r.json?.data.embeds[0].title === 'Bloxkeep 1.0.8');

  // GitHub's API not answering, as when 1.0.9 went out with no highlights: nothing readable at first, so nothing
  // posted; once the notes file at its tag answers, posted with that file's highlights.
  apiDown = fileDown = true;
  latest = 'v1.0.9';
  before = base();
  await minute();
  latestNow = await (await fetch(`http://127.0.0.1:${DEV}/latest`)).json();
  check('…and /latest has the new one as soon as its tag is out, before its notes can be read', latestNow.version === '1.0.9', JSON.stringify(latestNow));
  before = base();
  await minute();
  check("a new Bloxkeep whose notes can't be read yet: not posted empty, it waits", posts('c1').length === before.c1);
  fileDown = false;
  await minute();
  const rel9 = posts('c1').at(-1);
  check("…GitHub's API still not answering: posted with the highlights of its notes file", posts('c1').length === before.c1 + 1
    && embedOf(rel9)?.title === 'Bloxkeep 1.0.9 is out' && embedOf(rel9)?.description === '- **Private servers**: listed.', JSON.stringify(rel9?.body));
  apiDown = false;

  // Turned off: nothing more there.
  await interact(command('notify', { sub: 'off' }));
  notes.set(746, CHANGES.slice(4));
  watch = { ...watch, builds: [...watch.builds, build('0.746.0.7460001', 'h746', 'same', 'beta')], beta: { version: '0.746.0.7460001', hash: 'h746' } };
  before = base();
  await minute();
  check('/notify off: no more posts there, the others still get theirs', posts('c1').length === before.c1 && posts('c3').length === before.c3 + 1, `${posts('c1').length - before.c1} ${posts('c3').length - before.c3}`);
  const ready = posts('c3').at(-1);
  check("a build whose notes are out already: posted with them, its picture for no change", textsOf(ready).some(t => t.startsWith("### What's new\n**Fixes**\n- Fixes rare crash"))
    && pictureOf(ready) === 'update-same.png' && linksOf(ready).includes('https://create.roblox.com/docs/release-notes/release-notes-746'), JSON.stringify(textsOf(ready)));
  await minute();
  check('…and never edited', noteEdits().length === 4, noteEdits().length);
  // A fix within the release that changes Multi-Roblox: posted; a newer one that doesn't: not; then GitHub's raw file
  // answering from its older copy for a while (the build before read again): not posted a second time.
  const fix = build('0.746.0.7460003', 'h7463', 'changed', 'beta'), after = build('0.746.0.7460004', 'h7464', 'same', 'beta');
  watch = { ...watch, builds: [...watch.builds, fix], beta: { version: fix.version, hash: fix.hash } };
  before = base();
  await minute();
  check('a fix within a release that changes Multi-Roblox: posted', posts('c3').length === before.c3 + 1, posts('c3').length - before.c3);
  const newerWatch = { ...watch, builds: [...watch.builds, after], beta: { version: after.version, hash: after.hash } }, olderWatch = watch;
  watch = newerWatch;
  await minute();
  watch = olderWatch;
  await minute();
  watch = newerWatch;
  await minute();
  check('…a newer one that changes nothing: not; and the older one read again (a stale copy): not posted twice', posts('c3').length === before.c3 + 1, posts('c3').length - before.c3);

  // Game feeds (feeds.js): /feed for Bloxkeep's developers only; a hidden channel per source under Feeds; what arrives
  // there kept as it came (the last 20, edits too), nothing from other channels; Message Content asked for, and the bot
  // going on without it while Discord refuses it.
  check('the bot asks for Message Content', (identified.at(-1) & (1 << 15)) !== 0, identified.at(-1));
  const stranger = { ...command('feed', { sub: 'new', options: [opt('name', 'x', 3)] }), member: { permissions: '8', user: { id: 'u2' }, roles: [] } };
  r = await interact(stranger);
  check("/feed: refused to anyone but Bloxkeep's developers", /Only Bloxkeep's developers/.test(r.json?.data.content ?? '') && r.json?.data.flags === 64, r.json?.data.content);
  const madeBefore = calls.filter(c => c.url === '/guilds/g1/channels' && c.method === 'POST').length;
  r = await interact(command('feed', { sub: 'new', options: [opt('name', 'Shindo Spawn!', 3), opt('bot', 'b77', 6)] }));
  const made = calls.filter(c => c.url === '/guilds/g1/channels' && c.method === 'POST').slice(madeBefore);
  const [cat, chan] = made.map(c => c.body);
  check('/feed new: a hidden Feeds category, then feed-shindo-spawn in it', made.length === 2 && cat.name === 'Feeds' && cat.type === 4
    && cat.permission_overwrites.some(o => o.id === 'g1' && BigInt(o.deny) & (1n << 10n)) && chan.name === 'feed-shindo-spawn' && chan.parent_id === 'fcat', JSON.stringify(made.map(c => c.body)));
  check("…the bot and the game's bot may see it", ['bot1', 'b77'].every(id => chan.permission_overwrites.some(o => o.id === id && BigInt(o.allow) & (1n << 10n))), JSON.stringify(chan?.permission_overwrites));
  const feed = /<#(fch\d+)>/.exec(r.json?.data.content ?? '')?.[1];
  check('…and says where, seen only by them', !!feed && r.json?.data.flags === 64, r.json?.data.content);
  // Run again for the same name with the game's bot: the channel kept, that bot let in (its own permission there).
  r = await interact(command('feed', { sub: 'new', options: [opt('name', 'shindo spawn', 3), opt('bot', 'b88', 6)] }));
  const letIn = calls.filter(c => c.method === 'PUT' && c.url === `/channels/${feed}/permissions/b88`);
  check("/feed new again with the game's bot: the channel kept, the bot let in", /Already there: <#/.test(r.json?.data.content ?? '') && /<@b88> may post there/.test(r.json?.data.content ?? '')
    && letIn.length === 1 && letIn[0].body.type === 1 && BigInt(letIn[0].body.allow) & (1n << 10n)
    && calls.filter(c => c.url === '/guilds/g1/channels' && c.method === 'POST').length === madeBefore + 2, r.json?.data.content);
  r = await interact(command('feed', { guild: 'g2', sub: 'new', options: [opt('name', 'sae-egg', 3)] }));
  check('/feed new where Discord refuses: says the bot needs Manage Channels', /Manage Channels/.test(r.json?.data.content ?? ''), r.json?.data.content);
  const embed = { title: 'Riser Akuma Blade', description: 'Spawning in ~5 minutes', fields: [{ name: 'Location', value: 'Tempest' }], thumbnail: { url: 'https://x/i.png' } };
  dispatch('MESSAGE_CREATE', { id: 'fm1', channel_id: feed, guild_id: 'g1', webhook_id: 'w1', author: { id: 'w1', username: 'Shindo Spawn Alert', bot: true }, flags: 2, message_reference: { channel_id: 'their', guild_id: 'their-g' }, content: '', embeds: [embed], timestamp: '2026-11-04T12:00:00Z' });
  dispatch('MESSAGE_CREATE', { id: 'other', channel_id: 'c1', guild_id: 'g1', author: { id: 'u5', username: 'someone' }, content: 'hi', embeds: [] });
  dispatch('MESSAGE_UPDATE', { id: 'fm1', channel_id: feed, guild_id: 'g1', webhook_id: 'w1', author: { id: 'w1', username: 'Shindo Spawn Alert', bot: true }, message_reference: { channel_id: 'their' }, content: '', embeds: [{ ...embed, description: 'Spawned' }], edited_timestamp: '2026-11-04T12:05:00Z' });
  await sleep(600);
  r = await interact(command('feed', { sub: 'raw', options: [opt('channel', feed)] }));
  const rawText = r.json?.data.content ?? '';
  check('/feed raw: the message and its edit, newest first, from a Follow', /edited · Follow · Riser Akuma Blade[\s\S]*new · Follow · Riser Akuma Blade/.test(rawText), rawText);
  const { dashboardKey } = await import('../worker/online.js');
  const rawUrl = `http://127.0.0.1:${DEV}/feeds/raw.json?key=${await dashboardKey('test-token')}&channel=${feed}`;
  check('…with a link to all of it', rawText.includes(rawUrl), rawText);
  let kept = await (await fetch(rawUrl)).json();
  check('…which has the embed as it came, and nothing from other channels', kept.length === 2 && kept[1].embeds[0].fields[0].value === 'Tempest'
    && kept[1].from?.channel_id === 'their' && !JSON.stringify(kept).includes('"hi"'), JSON.stringify(kept).slice(0, 300));
  check('…without the key: not found', (await fetch(`http://127.0.0.1:${DEV}/feeds/raw.json?channel=${feed}`)).status === 404);
  for (let n = 0; n < 22; n++)
    dispatch('MESSAGE_CREATE', { id: `fx${n}`, channel_id: feed, guild_id: 'g1', author: { id: 'b77', username: 'SenZ V2', bot: true }, content: '', embeds: [{ title: `Egg ${n}` }] });
  await sleep(800);
  kept = await (await fetch(rawUrl)).json();
  check('…only the last 20 kept', kept.length === 20 && kept[0].embeds[0].title === 'Egg 21' && kept.at(-1).embeds[0].title === 'Egg 2', `${kept.length} ${kept[0]?.embeds[0]?.title} ${kept.at(-1)?.embeds[0]?.title}`);
  r = await interact(command('feed', { sub: 'list' }));
  check('/feed list: each feed channel and how much it kept', new RegExp(`<#${feed}>: 20 kept`).test(r.json?.data.content ?? ''), r.json?.data.content);
  r = await interact(command('feed', { sub: 'raw', options: [opt('channel', 'c1')] }));
  check("/feed raw on another channel: says it isn't a feed", /isn't a feed channel/.test(r.json?.data.content ?? ''), r.json?.data.content);

  // /feed show: a feed channel shown in the apps under a game. An app's /feeds/connect socket is told the games and their
  // cards at once (what was kept already among them), then each new card as it comes (words plain: no pings, no
  // markdown), a deleted message's card gone, and the games again once a feed is hidden. It only listens.
  const heard = [];
  const app = new WebSocket(`ws://127.0.0.1:${DEV}/feeds/connect`);
  app.on('message', m => heard.push(JSON.parse(String(m))));
  await waitFor(() => heard.length > 0, 5000);
  check('an app listening for game feeds: told at once, nothing shown yet', heard[0]?.t === 'feeds' && heard[0].games.length === 0 && heard[0].cards.length === 0, JSON.stringify(heard[0]));
  r = await interact(command('feed', { sub: 'show', options: [opt('channel', feed), opt('game', 'Blox Fruits', 3), opt('title', 'Stock', 3), opt('place', 2753915549, 4)] }));
  check('/feed show: under the game, what it kept already shown', /shows in Bloxkeep under Blox Fruits as “Stock” \(20 already there\)/.test(r.json?.data.content ?? ''), r.json?.data.content);
  await waitFor(() => heard.some(m => m.t === 'feeds' && m.games.length), 3000);
  let told = heard.findLast(m => m.t === 'feeds');
  check('…and open apps are told: the game, its place, its source, its cards newest first', told?.games[0]?.id === 'blox-fruits' && told.games[0].name === 'Blox Fruits'
    && told.games[0].place === 2753915549 && told.games[0].sources[0]?.id === feed && told.games[0].sources[0].title === 'Stock'
    && told.cards.length === 20 && told.cards[0].title === 'Egg 21' && told.cards[0].game === 'blox-fruits', JSON.stringify(told).slice(0, 400));
  dispatch('MESSAGE_CREATE', { id: 'bf1', channel_id: feed, guild_id: 'g1', author: { id: 'b77', username: 'Stock Bot', bot: true },
    content: '<@&555555555555> **Stock reset!**', embeds: [{ title: 'Normal stock', description: 'Next reset <t:4102444800:R>',
      fields: [{ name: '<:flame:123456789012> Flame', value: '**$250,000**' }, { name: 'Ice', value: '$350,000' }], thumbnail: { url: 'https://x/flame.png', proxy_url: 'https://images-ext-1.discordapp.net/external/abc/https/x/flame.png' } }] });
  await waitFor(() => heard.some(m => m.t === 'card'), 3000);
  const bfCard = heard.find(m => m.t === 'card')?.card;
  check('a new message: its card to open apps, in plain words', bfCard?.id === 'bf1' && bfCard.title === 'Normal stock' && bfCard.text === 'Stock reset!\nNext reset <t:4102444800:R>'
    && bfCard.fields[0]?.name === '<:flame:123456789012> Flame' && bfCard.fields[0].value === '$250,000' && bfCard.picture === 'https://images-ext-1.discordapp.net/external/abc/https/x/flame.png' && bfCard.until === 4102444800000 && bfCard.game === 'blox-fruits', JSON.stringify(bfCard));
  dispatch('MESSAGE_DELETE', { id: 'bf1', channel_id: feed, guild_id: 'g1' });
  await waitFor(() => heard.some(m => m.t === 'gone'), 3000);
  check('…deleted there: gone from the apps', heard.find(m => m.t === 'gone')?.id === 'bf1', JSON.stringify(heard.at(-1)));
  // Discord's newer layout (Vulcan's stock: a box per stock, a line per fruit, the change time): a card per box, a field
  // per fruit with its picture, and the countdown; deleted there, both go.
  const box = (head, lines, when) => ({ type: 17, accent_color: null, components: [{ type: 10, content: `**${head}**` }, { type: 14, divider: true },
    { type: 10, content: lines.map(([e, n, v]) => `<:${e}:1300167881337864224> **${n} • <:money:1456628926276173845>\`${v}\`**`).join('\n') }, { type: 14, divider: true },
    { type: 10, content: `-# <:clock:1182328726185312336> **${head.replace('Current ', '')} Change in** - <t:${when}:R> <@&1562231587914121338>` }] });
  heard.length = 0;
  dispatch('MESSAGE_CREATE', { id: 'bf2', channel_id: feed, guild_id: 'g1', webhook_id: '1567631153991524372', author: { id: '1567631153991524372', username: 'Vulcan • Live Stock Bot', avatar: 'a'.repeat(32), bot: true }, flags: 32768,
    content: '', embeds: [], components: [box('Current Normal Stock', [['chop', 'Blade', '30,000'], ['dark', 'Dark', '500,000']], 4102444800), box('Current Mirage Stock', [['spring', 'Spring', '60,000']], 4102448400)] });
  await waitFor(() => heard.filter(m => m.t === 'card').length >= 2, 3000);
  const [normal, mirage] = heard.filter(m => m.t === 'card').map(m => m.card);
  check("Discord's newer layout: a card per box, a field per item with its picture, the countdown, the poster's picture",
    normal?.id === 'bf2' && normal.title === 'Current Normal Stock' && normal.fields.length === 2 && normal.fields[0].name === '<:chop:1300167881337864224> Blade'
    && normal.fields[0].value === '<:money:1456628926276173845>30,000' && normal.until === 4102444800000 && !normal.text.includes('<@&')
    && normal.author?.icon === `https://cdn.discordapp.com/avatars/1567631153991524372/${'a'.repeat(32)}.webp?size=64` && mirage?.id === 'bf2:1' && mirage.title === 'Current Mirage Stock'
    && mirage.fields[0].name.endsWith('Spring') && mirage.at < normal.at, JSON.stringify([normal, mirage]).slice(0, 500));
  dispatch('MESSAGE_DELETE', { id: 'bf2', channel_id: feed, guild_id: 'g1' });
  await waitFor(() => heard.filter(m => m.t === 'gone').length >= 2, 3000);
  check('…deleted there: both its cards gone', heard.filter(m => m.t === 'gone').map(m => m.id).sort().join() === 'bf2,bf2:1', JSON.stringify(heard.filter(m => m.t === 'gone')));
  // Discord's notice that a channel was followed into it (type 12, an empty card in the apps on 2026-11-06): no card.
  heard.length = 0;
  dispatch('MESSAGE_CREATE', { id: 'fol1', type: 12, channel_id: feed, guild_id: 'g1', author: { id: 'u9', username: 'H' },
    message_reference: { channel_id: 'their', guild_id: 'their-g' }, content: 'Deepwoken Info #traan-stock', embeds: [] });
  await sleep(1500);
  check("Discord's \"has added … to this channel\" notice: no card in the apps", !heard.some(m => m.t === 'card'), JSON.stringify(heard));
  // Price lists: read once a day (the minute's cron read it already), /feed prices reads again now; the apps' Wiki tab
  // asks for them (/wiki), not sent with the feeds. The website down keeps the last list. Pictures only through the bot,
  // and only those a list has.
  priceRound = 2;
  r = await interact(command('feed', { sub: 'prices' }));
  const index = await (await fetch(`http://127.0.0.1:${DEV}/wiki`)).json().catch(() => null);
  const pr = await (await fetch(`http://127.0.0.1:${DEV}/wiki/blox-fruits`)).json().catch(() => null);
  check("/feed prices: read now, the changed list in the Wiki tab's games", /bloxfruitsvalues\.com: 30 items, new in the apps' Wiki tab/.test(r.json?.data.content ?? '')
    && index?.games?.[0]?.id === 'blox-fruits' && index.games[0].kind === 'prices' && index.games[0].name === 'Blox Fruits' && index.games[0].count === 30
    && pr?.kind === 'prices' && pr.items.length === 30 && pr.source?.name === 'bloxfruitsvalues.com' && pr.source.url === 'https://bloxfruitsvalues.com'
    && pr.items[0].value === 2000 && pr.items[1].perm === null && pr.items[0].demand === 7 && pr.items[0].category === 'Fruits'
    && pr.items[0].url === 'https://bloxfruitsvalues.com/values/fruits/doghouse-(frame-break)'
    && pr.items[0].img === `http://127.0.0.1:${DEV}/prices/img?u=${encodeURIComponent('https://i.postimg.cc/abc0/Fruit-0.png')}`, `${r.json?.data.content} ${JSON.stringify(index)} ${JSON.stringify(pr?.items?.[0])}`);
  r = await interact(command('feed', { sub: 'prices' }));
  check('…the same list again: nothing changed', /30 items, nothing changed/.test(r.json?.data.content ?? ''), r.json?.data.content);
  pricesDown = true;
  r = await interact(command('feed', { sub: 'prices' }));
  pricesDown = false;
  check("…the website down: the last list stays", /couldn't read it \(HTTP 503\)\. The last list stays \(30 items\)/.test(r.json?.data.content ?? ''), r.json?.data.content);
  const app2 = new WebSocket(`ws://127.0.0.1:${DEV}/feeds/connect`), heard2 = [];
  app2.on('message', m => heard2.push(JSON.parse(String(m))));
  await waitFor(() => heard2.length > 0, 5000);
  check("…a new app's feeds come without prices", heard2[0]?.t === 'feeds' && !('prices' in heard2[0]), JSON.stringify(heard2[0] ?? null).slice(0, 200));
  app2.close();
  await sleep(500);
  // Wikis (wiki.js): read whole by the minute's cron (the first already did), a page's infobox as its lines in plain
  // words, its first lines, its picture; a redirect left out. The wiki down keeps the last items.
  await minute(1500);
  const wi = await (await fetch(`http://127.0.0.1:${DEV}/wiki`)).json().catch(() => null);
  const dw = await (await fetch(`http://127.0.0.1:${DEV}/wiki/deepwoken`)).json().catch(() => null);
  const axe = dw?.items?.find(i => i.name === 'Adretian Axe');
  check("Wiki: the games read, each game's items with their lines in plain words",
    wi?.games?.slice(0, 3).map(g => g.id).join() === 'blox-fruits,deepwoken,pilgrammed' && wi.games[1].kind === 'wiki' && wi.games[1].count === 2 && wi.games[1].place === 4111023553
    // Then the popular games (wiki.js POPULAR_GAMES), each once; every game comes with its wiki, so the app never looks for one.
    && wi.games.length === 33 && new Set(wi.games.map(g => g.id)).size === 33 && wi.games.slice(3).every(g => g.kind === 'game' && g.place > 0)
    && wi.games.every(g => /^https:\/\/[a-z0-9-]+\.fandom\.com$/.test(g.wiki ?? '')) && wi.games.find(g => g.id === 'shindo-life')?.wiki === 'https://shindo-life-rell.fandom.com'
    && wi.games[1].wiki === 'https://deepwoken.fandom.com'
    && dw?.kind === 'wiki' && dw.cats.join() === 'Weapons' && dw.items.length === 2 && dw.source?.url === 'https://deepwoken.fandom.com' && dw.license?.name === 'CC BY-SA'
    && axe?.cat === 'Weapons' && axe.lead === 'The Adretian Axe is a Heavy Weapon.'
    && JSON.stringify(axe.fields) === JSON.stringify([['Type', 'Greataxe'], ['Damage', '24'], ['Scaling', 'HVY: 7 / HVY: 8'], ['Obtainment', '• Duke Ishamon Erisia\n• Bought from Traan for 300'], ['HP', '50']])
    && axe.img === 'https://static.wikia.nocookie.net/x/images/1.png/revision/latest/scale-to-width-down/240' && axe.url === 'https://deepwoken.fandom.com/wiki/Adretian_Axe'
    && dw.items[1].lead === 'A plain sword that every new player starts the game with.', `${JSON.stringify(wi)} ${JSON.stringify(dw)?.slice(0, 900)}`);
  const unknown = await fetch(`http://127.0.0.1:${DEV}/wiki/no-such-game`);
  const bad = await fetch(`http://127.0.0.1:${DEV}/wiki/..%2Fstatus`);
  check("…a game it doesn't have: not found", unknown.status === 404 && bad.status === 404, `${unknown.status} ${bad.status}`);
  const imgOther = await fetch(`http://127.0.0.1:${DEV}/prices/img?u=${encodeURIComponent('https://i.postimg.cc/zzz/Other.png')}`);
  const imgPlain = await fetch(`http://127.0.0.1:${DEV}/prices/img?u=${encodeURIComponent(`http://127.0.0.1:${SRC}/downloads.json`)}`);
  check("…a picture no list has isn't fetched", imgOther.status === 404 && imgPlain.status === 404, `${imgOther.status} ${imgPlain.status}`);
  r = await interact(command('feed', { sub: 'list' }));
  check('/feed list: which feeds show in Bloxkeep, and how many apps listen', /in Bloxkeep: Blox Fruits › Stock/.test(r.json?.data.content ?? '') && /1 apps listening/.test(r.json?.data.content ?? ''), r.json?.data.content);
  const heardBefore = heard.length;
  r = await interact(command('feed', { sub: 'hide', options: [opt('channel', feed)] }));
  await waitFor(() => heard.length > heardBefore, 3000);
  check('/feed hide: no longer in the apps', /no longer shows in Bloxkeep/.test(r.json?.data.content ?? '') && heard.at(-1)?.t === 'feeds' && heard.at(-1).games.length === 0, JSON.stringify(heard.at(-1)));
  let closed = false;
  app.on('close', () => { closed = true; });
  app.send('{"t":"hello"}');
  await waitFor(() => closed, 3000);
  check('…an app socket only listens: one that talks is closed', closed);

  // Message Content not turned on: Discord closes 4014; the bot goes on without it, says so at /feed, and asks again then.
  refuseContent = true;
  const before4014 = identified.length;
  closeHard(gwSocket, 4009, 'Session timed out'); // a new session: identified again, asking for Message Content
  await waitFor(() => identified.slice(before4014).some(i => !(i & (1 << 15))), 15000);
  await sleep(1500);
  check('Message Content refused: the bot goes on without it, still online', identified.slice(before4014).some(i => i & (1 << 15)) && !(identified.at(-1) & (1 << 15)) && open === 1,
    `${identified.slice(before4014)} open ${open}`);
  refuseContent = false;
  const beforeAsk = identified.length;
  r = await interact(command('feed', { sub: 'list' }));
  check('…/feed says Message Content is off', /Message Content is off/.test(r.json?.data.content ?? ''), r.json?.data.content);
  await waitFor(() => identified.slice(beforeAsk).some(i => i & (1 << 15)), 10000);
  check('…and the bot asks for it again then (turned on since: it keeps it)', identified.slice(beforeAsk).some(i => i & (1 << 15)), identified.slice(beforeAsk));
  await sleep(1000);
  r = await interact(command('feed', { sub: 'list' }));
  check("…after which /feed no longer says it's off", !/Message Content is off/.test(r.json?.data.content ?? ''), r.json?.data.content);
  // A feed channel deleted in Discord (2026-11-06, Deepwoken's): /feed remove can't pick it, so the bot forgets it itself.
  r = await interact(command('feed', { sub: 'new', options: [opt('name', 'soon gone', 3)] }));
  const gone = /<#(fch\d+)>/.exec(r.json?.data.content ?? '')?.[1];
  goneChannels.add(gone);
  r = await interact(command('feed', { sub: 'list' }));
  const goneAfter = await interact(command('feed', { sub: 'raw', options: [opt('channel', gone)] }));
  check('a feed channel deleted in Discord: forgotten (not in /feed list, no longer a feed), the others kept', !!gone && !r.json?.data.content.includes(gone)
    && r.json?.data.content.includes(feed) && /isn't a feed channel/.test(goneAfter.json?.data.content ?? ''), `${r.json?.data.content} | ${goneAfter.json?.data.content}`);
  r = await interact(command('feed', { sub: 'remove', options: [opt('channel', feed)] }));
  check('/feed remove: no longer a feed, what it got forgotten', /no longer a feed/.test(r.json?.data.content ?? '') && (await (await fetch(rawUrl)).json()).length === 0, r.json?.data.content);

  let s = await (await fetch(`http://127.0.0.1:${DEV}/status`)).json();
  check('the bot stayed online throughout', s.state === 'online', s.state);
  check('/status keeps how long each command took and where it came in', s.interactions?.length > 5 && s.interactions.every(t => typeof t.ms === 'number' && t.name), JSON.stringify(s.interactions?.slice(-2)));
  check('no token in what /status shows', !JSON.stringify(s).includes('test-token'));

  // The move to a new object (as from "bot" in Asia to "bot-na" by Discord): it takes over the servers' choices and
  // boards, the old one stops for good, and the news goes on from the new one.
  stopDev();
  await waitFor(() => open === 0, 5000);
  startDev({ OBJECT_NAME: 'bot-next', OLD_OBJECT_NAME: 'bot-test' });
  check('wrangler dev back, with the new object', await devUp());
  await minute(3500);
  s = await (await fetch(`http://127.0.0.1:${DEV}/status`)).json();
  check('the new object took over 1 server\'s choices and 1 board', s.recent.some(e => e.kind === 'migrated' && e.detail === '1 subscriptions, 1 boards'), JSON.stringify(s.recent.slice(-6)));
  r = await interact(command('notify', { guild: 'g3', locale: 'en-US', sub: 'show' }));
  check('…so /notify show answers from it as before', /<#c3>/.test(r.json?.data.content ?? ''), r.json?.data.content);
  await sleep(2000);
  check('…and only one connection to Discord is left (the old object stopped)', open === 1, open);
  watch = { ...watch, builds: [...watch.builds, build('0.747.0.7470001', 'h747', 'same', 'beta')], beta: { version: '0.747.0.7470001', hash: 'h747' } };
  before = base();
  await minute();
  check('…and the news goes on from the new object, once', posts('c3').length === before.c3 + 1 && headingOf(posts('c3').at(-1)) === 'Roblox 0.747 is coming'
    && patches().length === before.patches + 1, `${posts('c3').length - before.c3} ${patches().length - before.patches}`);
  await minute();
  check('…the old one never connects again', open === 1, open);
} finally {
  stopDev();
  gw.close();
  rest.close();
  src.close();
}
console.log(failures ? `${failures} check(s) failed` : 'All checks passed');
process.exit(failures ? 1 : 0);
