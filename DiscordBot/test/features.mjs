// The bot's commands and news (DiscordBot/worker) against a fake Discord (its REST API and Gateway) and fake sources (the
// Roblox watch, Roblox's status page, Bloxkeep's releases), run by wrangler dev: each command's reply, a signature that
// doesn't verify refused, and each kind of news posted once, in English, only where it was asked for.
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

// ── the fake Gateway: hello, ready, heartbeats answered ──
const gw = new WebSocketServer({ port: GW });
gw.on('connection', ws => {
  ws.send(JSON.stringify({ op: 10, d: { heartbeat_interval: 30000 } }));
  ws.on('message', raw => {
    const m = JSON.parse(raw);
    if (m.op === 1) ws.send(JSON.stringify({ op: 11 }));
    if (m.op === 2 || m.op === 6) ws.send(JSON.stringify({ op: 0, t: 'READY', s: 1, d: { session_id: 's1', resume_gateway_url: `ws://127.0.0.1:${GW}`, user: { username: 'TestBot' } } }));
  });
});

// ── the fake REST API: every call kept; channel "nochan" refuses posts, message "gone" was deleted ──
const calls = [];
let msgNo = 0;
const rest = http.createServer((req, res) => {
  let body = '';
  req.on('data', c => { body += c; });
  req.on('end', () => {
    const call = { method: req.method, url: req.url, auth: req.headers.authorization, body: body ? JSON.parse(body) : null };
    calls.push(call);
    const send = (status, value) => { res.writeHead(status, { 'Content-Type': 'application/json' }); res.end(JSON.stringify(value)); };
    if (req.method === 'POST' && req.url === '/channels/nochan/messages') return send(403, { message: 'Missing Access', code: 50001 });
    if (req.method === 'PATCH' && /\/messages\/gone$/.test(req.url)) return send(404, { message: 'Unknown Message', code: 10008 });
    if (req.method === 'POST') return send(200, { id: `m${++msgNo}` });
    send(200, req.method === 'PUT' ? [] : {});
  });
}).listen(REST);
const posts = channel => calls.filter(c => c.method === 'POST' && c.url === `/channels/${channel}/messages`);
const patches = () => calls.filter(c => c.method === 'PATCH');

// ── the fake sources ──
const build = (version, hash, verdict, channel = 'live') => ({ version, hash, verdict, channel, seenAt: '2026-10-20T16:26:42Z' });
let watch = {
  schema: 1, builds: [build('0.744.0.7441080', 'h744', 'same')],
  live: { version: '0.744.0.7441080', hash: 'h744' }, beta: { version: '0.744.0.7441080', hash: 'h744' },
};
let status = { code: 100 };
let latest = 'v1.0.7';
const NOTES = '- **Robux at once**: shown as you unlock.\n\n#### New\n- details\n\n**Tiếng Việt**\n\n- **Robux hiện ngay**: lúc mở khoá.\n\n#### Mới\n- chi tiết';
const src = http.createServer((req, res) => {
  const json = v => { res.writeHead(200, { 'Content-Type': 'application/json' }); res.end(JSON.stringify(v)); };
  if (req.url === '/watch.json') return json(watch);
  if (req.url === '/status.json') return json({ result: { status_overall: { status_code: status.code }, status: ['User', 'Player', 'Creator'].map(name => ({ name, status_code: status.code })), incidents: status.code > 100 ? [{ name: 'Players can\'t join' }] : [] } });
  if (req.url === '/releases/latest') { res.writeHead(302, { Location: `http://127.0.0.1:${SRC}/releases/tag/${latest}` }); return res.end(); }
  const m = /^\/api\/tags\/(.+)$/.exec(req.url);
  if (m) return json({ html_url: `https://github.com/minhhoang0122/Bloxkeep/releases/tag/${m[1]}`, body: NOTES });
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
  return { status: res.status, json: res.status === 200 ? await res.json() : null };
}
const command = (name, { guild = 'g1', locale = 'vi', perms = '32', sub, options = [], resolved } = {}) => ({
  type: 2, id: 'i', application_id: 'app1', token: 't', locale, guild_locale: locale,
  ...(guild ? { guild_id: guild, member: { permissions: perms, user: { id: 'u1' } } } : { user: { id: 'u1' } }),
  data: { id: 'c', name, type: 1, options: sub ? [{ type: 1, name: sub, options }] : [], ...(resolved ? { resolved } : {}) },
});
const opt = (name, value, type = 7) => ({ name, value, type });

// ── wrangler dev ──
let dev;
function startDev() {
  const out = fs.openSync(path.join(PERSIST, 'wrangler-dev.log'), 'a');
  const vars = {
    GATEWAY_URL: `ws://127.0.0.1:${GW}`, DISCORD_TOKEN: 'test-token', DISCORD_API: `http://127.0.0.1:${REST}`, DISCORD_APP_ID: 'app1', DISCORD_PUBLIC_KEY: PUBLIC_HEX,
    WATCH_URL: `http://127.0.0.1:${SRC}/watch.json`, STATUS_URL: `http://127.0.0.1:${SRC}/status.json`, LATEST_URL: `http://127.0.0.1:${SRC}/releases/latest`,
    RELEASE_API_URL: `http://127.0.0.1:${SRC}/api/tags/`, RELEASE_EVERY_MS: '1',
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
    && puts[0].body.map(c => c.name).join() === 'help,download,multi,robloxstatus,notify,board', JSON.stringify(puts.map(p => p.url)));
  check('…and the first look announces nothing', calls.filter(c => c.method === 'POST').length === 0, calls.length);
  await minute();
  check('…nor are the commands put again when unchanged', calls.filter(c => c.method === 'PUT').length === 1);

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

  // A build on Roblox's test channel, checked by the watch: "coming", to both news channels, pinging r1 in g1.
  const base = () => ({ c1: posts('c1').length, c3: posts('c3').length, patches: patches().length });
  let before = base();
  watch = { ...watch, builds: [...watch.builds, build('0.745.0.7451000', 'h745', 'changed', 'beta')], beta: { version: '0.745.0.7451000', hash: 'h745' } };
  await minute();
  const coming = posts('c1').at(-1);
  check('a build coming: posted in English, pinging the role', posts('c1').length === before.c1 + 1 && embedOf(coming)?.title === 'Roblox 0.745.0.7451000 is coming'
    && coming.body.content === '<@&r1>' && JSON.stringify(coming.body.allowed_mentions) === '{"roles":["r1"]}', JSON.stringify(coming?.body));
  check('…with what it means for Multi-Roblox', embedOf(coming)?.fields?.[0]?.value === '**May be blocked**' && embedOf(coming)?.fields?.[2]?.value === 'Not yet', JSON.stringify(embedOf(coming)?.fields));
  check('…the same in the other server, where nobody is pinged', embedOf(posts('c3').at(-1))?.title === 'Roblox 0.745.0.7451000 is coming' && JSON.stringify(posts('c3').at(-1).body.allowed_mentions) === '{"parse":[]}', JSON.stringify(posts('c3').at(-1)?.body));
  check('…and the board edited to show it', patches().length === before.patches + 1 && JSON.stringify(embedOf(patches().at(-1))).includes('0.745.0.7451000'), JSON.stringify(patches().at(-1)?.body));
  before = base();
  await minute();
  check('nothing new: no post, no edit', posts('c1').length === before.c1 && patches().length === before.patches);

  // It goes live: "live", once.
  watch = { ...watch, live: { version: '0.745.0.7451000', hash: 'h745' } };
  before = base();
  await minute();
  check('the build goes live: posted once', posts('c1').length === before.c1 + 1 && embedOf(posts('c1').at(-1))?.title === 'Roblox 0.745.0.7451000 is live', embedOf(posts('c1').at(-1))?.title);
  // A fix within the same release that changes nothing for Multi-Roblox: the board shows it, nobody is told.
  watch = { ...watch, builds: [...watch.builds, build('0.745.0.7451005', 'h7455', 'same')], live: { version: '0.745.0.7451005', hash: 'h7455' }, beta: { version: '0.745.0.7451005', hash: 'h7455' } };
  before = base();
  await minute();
  check('a fix build with no change for Multi-Roblox: no post, the board edited', posts('c1').length === before.c1 && patches().length === before.patches + 1, `${posts('c1').length - before.c1} ${patches().length - before.patches}`);

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
  watch = { ...watch, builds: [...watch.builds, build('0.746.0.7460001', 'h746', 'same', 'beta')], beta: { version: '0.746.0.7460001', hash: 'h746' } };
  before = base();
  await minute();
  check('/notify off: no more posts there, the others still get theirs', posts('c1').length === before.c1 && posts('c3').length === before.c3 + 1, `${posts('c1').length - before.c1} ${posts('c3').length - before.c3}`);

  const s = await (await fetch(`http://127.0.0.1:${DEV}/status`)).json();
  check('the bot stayed online throughout', s.state === 'online', s.state);
  check('no token in what /status shows', !JSON.stringify(s).includes('test-token'));
} finally {
  stopDev();
  gw.close();
  rest.close();
  src.close();
}
console.log(failures ? `${failures} check(s) failed` : 'All checks passed');
process.exit(failures ? 1 : 0);
