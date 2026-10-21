// The bot's commands and news (DiscordBot/worker) against a fake Discord (its REST API and Gateway) and fake sources (the
// Roblox watch, Roblox's status page, Bloxkeep's releases, Roblox's release notes), run by wrangler dev: each command's
// reply, a signature that doesn't verify refused, each kind of news posted once, in English, only where it was asked for
// (a Roblox update as a card, Roblox's notes edited in once they're out), and /verify's captcha (a PNG only that person
// sees, the code typed into a form, the role given once it's right), /discord (the app's Discord button: an invite that
// never expires, made again once it stops working), and how many people have Bloxkeep open now (the apps' beats,
// /online and its dashboard for Bloxkeep's developers only, the line in the bot's status).
// No Discord account or token needed. In this folder: npm install, then node features.mjs (about a minute).
import { spawn, execSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { WebSocketServer } from 'ws';

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
const waitFor = async (cond, ms) => { const end = Date.now() + ms; while (Date.now() < end) { if (cond()) return true; await sleep(50); } return cond(); };

// ── the fake Gateway: hello, ready, heartbeats answered; the custom statuses sent kept, open connections counted ──
const gw = new WebSocketServer({ port: GW });
const statuses = [];
let open = 0;
gw.on('connection', ws => {
  open++;
  ws.on('close', () => { open--; });
  ws.send(JSON.stringify({ op: 10, d: { heartbeat_interval: 30000 } }));
  ws.on('message', raw => {
    const m = JSON.parse(raw);
    if (m.op === 3) statuses.push(m.d.activities?.[0]);
    if (m.op === 1) ws.send(JSON.stringify({ op: 11 }));
    if (m.op === 2 || m.op === 6) ws.send(JSON.stringify({ op: 0, t: 'READY', s: 1, d: { session_id: 's1', resume_gateway_url: `ws://127.0.0.1:${GW}`, user: { username: 'TestBot' } } }));
  });
});

// ── the fake REST API: every call kept; channel "nochan" refuses posts, message "gone" was deleted, invites in
//    deadInvites were deleted ──
const calls = [];
const deadInvites = new Set();
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
    if (req.method === 'PATCH' && /\/messages\/gone$/.test(req.url)) return send(404, { message: 'Unknown Message', code: 10008 });
    if (req.method === 'POST' && /^\/channels\/\w+\/invites$/.test(req.url)) return send(200, { code: `inv${++inviteNo}` });
    // The community server's channels: one shows how many have Bloxkeep open.
    if (req.method === 'GET' && req.url === '/guilds/g1/channels') return send(200, [{ id: 'c1', type: 0, name: 'news' }, { id: 'cnt', type: 2, name: '🟢 Using Bloxkeep: 0' }]);
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
const src = http.createServer((req, res) => {
  const json = v => { res.writeHead(200, { 'Content-Type': 'application/json' }); res.end(JSON.stringify(v)); };
  if (req.url === '/watch.json') return json(watch);
  if (req.url === '/status.json') return json({ result: { status_overall: { status_code: status.code }, status: ['User', 'Player', 'Creator'].map(name => ({ name, status_code: status.code })), incidents: status.code > 100 ? [{ name: 'Players can\'t join' }] : [] } });
  if (req.url === '/releases/latest') { res.writeHead(302, { Location: `http://127.0.0.1:${SRC}/releases/tag/${latest}` }); return res.end(); }
  const m = /^\/api\/tags\/(.+)$/.exec(req.url);
  if (m) return json({ html_url: `https://github.com/minhhoang0122/Bloxkeep/releases/tag/${m[1]}`, body: NOTES });
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
const command = (name, { guild = 'g1', locale = 'vi', perms = '32', sub, options = [], resolved } = {}) => ({
  type: 2, id: 'i', application_id: 'app1', token: 't', locale, guild_locale: locale,
  ...(guild ? { guild_id: guild, member: { permissions: perms, user: { id: 'u1' } } } : { user: { id: 'u1' } }),
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
    RELEASE_API_URL: `http://127.0.0.1:${SRC}/api/tags/`, RELEASE_EVERY_MS: '1', CAPTCHA_CODE: 'K7MXP', COMMUNITY_GUILD: 'g1', INVITE_RECHECK_MS: '0',
    ONLINE_FOR_MS: '8000', BEAT_BUDGET: '200', PUBLIC_URL: `http://127.0.0.1:${DEV}`,
    NOTES_URL: `http://127.0.0.1:${SRC}/notes/release-notes-`, NOTES_EVERY_MS: '1', COUNT_EVERY_MS: '3500', ...extra,
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
    && puts[0].body.map(c => c.name).join() === 'help,download,multi,robloxstatus,notify,setup,online,board,verify', JSON.stringify(puts[0]?.body.map(c => c.name)));
  check('…and the first look announces nothing', calls.filter(c => c.method === 'POST').length === 0, calls.length);
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
    && helpButtons[1].url === 'https://github.com/minhhoang0122/Bloxkeep' && helpButtons[2].url.includes('client_id=app1') && helpButtons[2].url.includes('permissions=19456'), JSON.stringify(helpButtons));
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

  // /verify in g1: the Verify button in v1, role rv for whoever solves the captcha (its code fixed for the test).
  r = await interact(command('verify', { sub: 'set', options: [opt('channel', 'v1'), opt('role', 'rv', 8)] }));
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

  // Online now: two Bloxkeep apps say they're open (online for 8 s here; for real, one and a half gaps between beats). The
  // bot takes 200 beats a day here (40,000 for real): one app online beats every ten minutes, two every 864 s.
  const beat = async body => {
    const res = await fetch(`http://127.0.0.1:${DEV}/online/ping`, { method: 'POST', body: JSON.stringify(body), headers: { 'Content-Type': 'application/json' } });
    return { status: res.status, json: await res.json().catch(() => null) };
  };
  const A = 'a'.repeat(32), B = 'B'.repeat(32);
  let b = await beat({ id: A, app: '1.0.8', language: 'vi' });
  check('a beat from a Bloxkeep app: taken, the answer says to send the next in ten minutes', b.status === 200 && b.json?.next === 600, JSON.stringify(b));
  check("…one that isn't a beat is refused (an id that isn't 32 hex digits, a version that isn't one)",
    (await beat({ id: 'me', app: '1.0.8' })).status === 400 && (await beat({ id: B, app: 'x' })).status === 400 && (await beat({ id: B })).status === 400);
  await beat({ id: B, app: '1.0.8-alpha.2', language: 'en' });
  b = await beat({ id: A, app: '1.0.8', language: 'vi' }); // the same app again: still one
  check('…and with more online, beats further apart, so that they stay within the day\'s budget', b.json?.next === 864, JSON.stringify(b));
  const asDeveloper = command('online', { guild: null, locale: 'en-US' });
  const asSomeoneElse = { ...command('online', { locale: 'en-US', perms: '8' }), member: { permissions: '8', user: { id: 'u2' } } };
  r = await interact(asSomeoneElse);
  check('/online from someone who is not one of Bloxkeep\'s developers (a server admin): refused, seen only by them',
    r.json?.data.flags === 64 && /developers/.test(r.json.data.content ?? '') && !r.json.data.embeds, JSON.stringify(r.json));
  r = await interact(asDeveloper);
  const oe = r.json?.data.embeds?.[0], field = name => oe?.fields?.find(f => f.name === name)?.value ?? '';
  check('/online from a developer, in a DM with the bot: 2 online now, seen only by them', r.json?.data.flags === 64 && oe?.title === '2 online now', JSON.stringify(r.json));
  check('…with today\'s peak, and who by version and language', /^\*\*2\*\*, at <t:\d+:t>$/.test(field('Peak today'))
    && field('Version') === '1.0.8: **1**\n1.0.8-alpha.2: **1**' && field('Language') === 'en: **1**\nvi: **1**', JSON.stringify(oe?.fields));
  const dashboard = r.json?.data.components?.[0]?.components?.[0]?.url ?? '';
  check('…and a Dashboard button, its link carrying the key', /^http:\/\/127\.0\.0\.1:\d+\/online\?key=[\w-]{24}$/.test(dashboard), dashboard);
  let page = await fetch(dashboard);
  check('the dashboard opens with that key', page.status === 200 && /text\/html/.test(page.headers.get('content-type')) && (await page.text()).includes('<title>Bloxkeep online</title>'), page.status);
  const numbers = async (key = new URL(dashboard).search) => { const res = await fetch(`http://127.0.0.1:${DEV}/online.json${key}`); return res.ok ? res.json() : res.status; };
  let o = await numbers();
  check('…and its numbers: 2 now, today\'s peak 2, the last 30 days', o.now === 2 && o.today?.peak === 2 && o.days?.length === 30 && o.days.at(-1).day === o.today.day, JSON.stringify(o).slice(0, 300));
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
