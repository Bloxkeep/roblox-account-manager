// Bloxkeep from Discord (/pc, services/DiscordBot/worker/remote.js) against a fake Discord and a fake Bloxkeep app, run by
// wrangler dev: an app connects with its PC's secret and asks for a code; /pc link with it links that PC to the person who
// typed it (once, for ten minutes); their commands reach their PC only and its answers come back seen only by them (edited
// in when slow, followed up when done); autocomplete offers the PC's presets and accounts; a PC that isn't connected, a
// person with no PC, a wrong code are each told so; unlinking, from Discord or from the app, ends it.
// No Discord account or token needed. In this folder: npm install, then node remote.mjs (about half a minute).
import { spawn, execSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import WebSocket, { WebSocketServer } from 'ws';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const WORKER = path.join(HERE, '..', 'worker');
const WRANGLER = path.join(HERE, 'node_modules', 'wrangler', 'bin', 'wrangler.js');
const PERSIST = path.join(process.env.TEMP || HERE, 'bkr');
const GW = 8794, REST = 8795, DEV = 8789;
fs.rmSync(PERSIST, { recursive: true, force: true });
fs.mkdirSync(PERSIST, { recursive: true });

const sleep = ms => new Promise(r => setTimeout(r, ms));
let failures = 0;
const check = (name, ok, detail = '') => { console.log(`${ok ? 'PASS' : 'FAIL'}  ${name}${ok ? '' : `  [${String(detail).slice(0, 300)}]`}`); if (!ok) failures++; };
const waitFor = async (cond, ms) => { const end = Date.now() + ms; while (Date.now() < end) { if (cond()) return true; await sleep(50); } return cond(); };

// The fake Gateway: enough for the bot to be online.
const gw = new WebSocketServer({ port: GW });
gw.on('connection', ws => {
  ws.send(JSON.stringify({ op: 10, d: { heartbeat_interval: 30000 } }));
  ws.on('message', raw => {
    const m = JSON.parse(raw);
    if (m.op === 1) ws.send(JSON.stringify({ op: 11 }));
    if (m.op === 2 || m.op === 6) ws.send(JSON.stringify({ op: 0, t: 'READY', s: 1, d: { session_id: 's1', resume_gateway_url: `ws://127.0.0.1:${GW}`, user: { username: 'TestBot' } } }));
  });
});
// The fake REST API: every call kept.
const calls = [];
let userInstall = false;
const rest = http.createServer((req, res) => {
  const chunks = [];
  req.on('data', c => { chunks.push(c); });
  req.on('end', async () => {
    const raw = Buffer.concat(chunks), multipart = /^multipart\/form-data/.test(req.headers['content-type'] ?? '');
    // With files (a screenshot): its payload_json as the body, and the files' names, types and sizes.
    const form = multipart ? await new Response(raw, { headers: { 'Content-Type': req.headers['content-type'] } }).formData() : null;
    const call = form ? { method: req.method, url: req.url, body: JSON.parse(form.get('payload_json')),
      files: [...form.entries()].filter(([k]) => k.startsWith('files[')).map(([, f]) => ({ name: f.name, type: f.type, size: f.size })) }
      : { method: req.method, url: req.url, body: raw.length ? JSON.parse(raw.toString()) : null };
    calls.push(call);
    // An application not set up for installs on people's own accounts: commands that ask for it are refused.
    if (req.method === 'PUT' && !userInstall && call.body?.some(c => c.integration_types?.includes(1))) {
      res.writeHead(400, { 'Content-Type': 'application/json' });
      return res.end(JSON.stringify({ message: 'Invalid Form Body', code: 50035 }));
    }
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify(req.method === 'PUT' ? [] : { id: 'm1' }));
  });
}).listen(REST);

// Signing like Discord.
const { publicKey, privateKey } = crypto.generateKeyPairSync('ed25519');
const PUBLIC_HEX = publicKey.export({ format: 'der', type: 'spki' }).subarray(-32).toString('hex');
async function interact(payload) {
  const body = JSON.stringify(payload), ts = String(Math.floor(Date.now() / 1000));
  const sig = crypto.sign(null, Buffer.from(ts + body), privateKey).toString('hex');
  const res = await fetch(`http://127.0.0.1:${DEV}/interactions`, { method: 'POST', body, headers: { 'Content-Type': 'application/json', 'X-Signature-Timestamp': ts, 'X-Signature-Ed25519': sig } });
  return res.status === 200 ? res.json() : { status: res.status };
}
// /pc in a DM with the bot (no server), by `user`; type 4 = autocomplete, with the option being typed in.
let tokenNo = 0;
const pc = (sub, options = {}, { user = 'u5', name = 'Five', type = 2, focused } = {}) => ({
  type, id: 'i', application_id: 'app1', token: `tok${++tokenNo}`, locale: 'en-US', user: { id: user, global_name: name },
  data: { id: 'c', name: 'pc', type: 1, options: [{ type: 1, name: sub, options: Object.entries(options).map(([n, value]) => ({ type: 3, name: n, value, ...(n === focused ? { focused: true } : {}) })) }] },
});
const contentOf = r => r?.data?.content ?? '';

/** A fake Bloxkeep app: connects with its secret, keeps what it hears, answers commands with `answer(command)`. */
function app(secret, answer = () => null) {
  const ws = new WebSocket(`ws://127.0.0.1:${DEV}/remote/connect`, { headers: { Authorization: `Bearer ${secret}` } });
  const heard = [];
  const me = { ws, heard, closed: null, status: null, last: t => heard.filter(m => m.t === t).at(-1), send: m => ws.send(JSON.stringify(m)) };
  let settle = () => {};
  ws.on('unexpected-response', (req, res) => { me.status = res.statusCode; req.destroy(); settle(false); });
  ws.on('error', () => {});
  ws.on('close', code => { me.closed = code; });
  ws.on('message', async raw => {
    const m = JSON.parse(raw);
    heard.push(m);
    if (m.t === 'command') {
      const a = await answer(m);
      if (a) me.send({ t: 'answer', id: m.id, ...a });
    }
  });
  me.open = () => new Promise(r => { settle = r; ws.once('open', () => r(true)); ws.once('close', () => r(false)); });
  return me;
}

let dev;
function startDev() {
  const out = fs.openSync(path.join(PERSIST, 'wrangler-dev.log'), 'a');
  const vars = {
    OBJECT_NAME: 'bot-remote', OLD_OBJECT_NAME: '', GATEWAY_URL: `ws://127.0.0.1:${GW}`, DISCORD_TOKEN: 'test-token', DISCORD_API: `http://127.0.0.1:${REST}`,
    DISCORD_APP_ID: 'app1', DISCORD_PUBLIC_KEY: PUBLIC_HEX, TRACE_URL: 'http://127.0.0.1:9/', ANSWER_WAIT_MS: '800', ANSWER_LATE_MS: '3000', COMMANDS_RETRY_MS: '1000',
    WATCH_URL: 'http://127.0.0.1:9/', STATUS_URL: 'http://127.0.0.1:9/', LATEST_URL: 'http://127.0.0.1:9/', RELEASE_API_URL: 'http://127.0.0.1:9/',
    DOWNLOADS_URL: 'http://127.0.0.1:9/', RELEASE_FILE_URL: 'http://127.0.0.1:9/', NOTES_URL: 'http://127.0.0.1:9/', COMMUNITY_GUILD: 'g1',
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

const SECRET_A = crypto.randomBytes(32).toString('base64url'), SECRET_B = crypto.randomBytes(32).toString('base64url');
try {
  startDev();
  check('wrangler dev serves the worker', await devUp());
  await fetch(`http://127.0.0.1:${DEV}/__scheduled?cron=*+*+*+*+*`).catch(() => {});
  const puts = () => calls.filter(c => c.method === 'PUT' && c.url === '/applications/app1/commands');
  check("Discord refusing /pc on people's own accounts: the commands go in all the same, /pc for servers and DMs only",
    await waitFor(() => puts().length >= 2, 8000) && puts()[1].body.find(c => c.name === 'pc')?.integration_types.join() === '0'
    && !puts()[1].body.find(c => c.name === 'pc').contexts.includes(2), JSON.stringify(puts().map(p => p.body.find(c => c.name === 'pc'))));
  // Installs on people’s own accounts turned on in the Developer Portal later: taken up at the next try, no deploy.
  userInstall = true;
  await sleep(1200);
  await fetch(`http://127.0.0.1:${DEV}/__scheduled?cron=*+*+*+*+*`).catch(() => {});
  check('…turned on later in the Developer Portal: /pc goes in for people’s own accounts at the next try',
    await waitFor(() => puts().length >= 3, 8000) && puts()[2].body.find(c => c.name === 'pc')?.integration_types.join() === '0,1', puts().length);

  // ── connecting ──
  const short = app('too-short');
  await short.open();
  check('an app without a proper secret is refused', short.status === 401, short.status);

  const commands = [];
  let slow = false;
  const answers = m => {
    commands.push(m);
    if (m.command === 'status') return { content: '2 of 2 accounts in game', embeds: [{ title: '2 accounts in game', color: 1 }], extra: 'dropped' };
    if (m.command === 'preset') return { content: `Opening ${m.value}…` };
    if (m.command === 'switch') return slow ? sleep(1500).then(() => ({ content: `Switched ${m.value}` })) : { content: `Switched ${m.value}` };
    if (m.command === 'close') return { content: `Closed ${m.value}` };
    if (m.command === 'screenshot') return m.value === 'Beta' ? { content: 'Beta has no Roblox window open.' } : null; // pictures go up on their own
    return null;
  };
  /** The app's pictures for a screenshot, as Bloxkeep sends them: straight to the answer link it was handed (Discord's). */
  const sendShot = (reply, n = 2) => {
    const f = new FormData();
    f.append('payload_json', JSON.stringify({ content: 'Alpha · Beta', allowed_mentions: { parse: [] }, attachments: [...Array(n).keys()].map(i => ({ id: i, filename: `screenshot-${i + 1}.jpg` })) }));
    for (let i = 0; i < n; i++)
      f.append(`files[${i}]`, new Blob([new Uint8Array(2048)], { type: 'image/jpeg' }), `screenshot-${i + 1}.jpg`);
    return fetch(reply, { method: 'PATCH', body: f }).then(res => res.status);
  };
  let a = app(SECRET_A, answers);
  check('an app connects with its secret', await a.open());
  check('…and is told at once that it is not linked yet', await waitFor(() => a.last('link'), 3000) && a.last('link').linked === false, JSON.stringify(a.heard));
  a.send({ t: 'hello', name: 'GAMING-PC', accounts: ['Alpha', 'Beta'], presets: ['Farm', 'Daily'], lang: 'en', ask: true });
  a.send({ t: 'code' });
  check('…asks for a code: six letters and digits', await waitFor(() => a.last('code'), 3000) && /^[A-Z2-9]{6}$/.test(a.last('code').code), JSON.stringify(a.last('code')));
  const code = a.last('code').code;

  // ── linking ──
  let r = await interact(pc('status'));
  check('/pc status with no PC linked: told how to link one, seen only by them', /Control from Discord/.test(contentOf(r)) && r.data.flags === 64, JSON.stringify(r));
  r = await interact(pc('link', { code: 'ZZZZZZ' }));
  check('/pc link with a wrong code: refused', /doesn't work/.test(contentOf(r)), contentOf(r));
  r = await interact(pc('link', { code: `${code.slice(0, 3)}-${code.slice(3).toLowerCase()}` }));
  const linkToken = `tok${tokenNo}`;
  check('/pc link with its code (typed with a dash, in lower case): not linked yet, the PC asked to allow it', /asking whether to allow/.test(contentOf(r))
    && /GAMING-PC/.test(contentOf(r)), contentOf(r));
  check('…the app is asked, with the name Discord shows them by', await waitFor(() => a.last('ask'), 3000) && a.last('ask').user === 'Five' && a.last('ask').replacing === null,
    JSON.stringify(a.last('ask')));
  r = await interact(pc('status'));
  check('…until it is allowed, commands reach nothing', /No PC is linked/.test(contentOf(r)), contentOf(r));
  a.send({ t: 'allow' });
  check('allowed in the app: linked, the app told', await waitFor(() => a.last('link')?.linked, 3000) && a.last('link').user === 'Five', JSON.stringify(a.last('link')));
  check('…and the Discord answer edited to say so', await waitFor(() => calls.some(c => c.method === 'PATCH' && c.url === `/webhooks/app1/${linkToken}/messages/@original`
    && /Linked: \*\*GAMING-PC\*\*/.test(c.body.content)), 3000), JSON.stringify(calls.slice(-2)));
  r = await interact(pc('link', { code }, { user: 'u6', name: 'Six' }));
  check('…the same code again (someone else): refused, it was used', /doesn't work/.test(contentOf(r)), contentOf(r));
  // Someone who got a new code (asked the person for it, saw it on a stream): the PC says who, and whom it replaces;
  // refused there, nothing changes.
  a.send({ t: 'code' });
  await waitFor(() => a.last('code')?.code !== code, 3000);
  r = await interact(pc('link', { code: a.last('code').code }, { user: 'u8', name: 'Stranger' }));
  const strangerToken = `tok${tokenNo}`;
  check('a stranger with a new code: the PC is asked, told it would replace the link', await waitFor(() => a.last('ask')?.user === 'Stranger', 3000)
    && a.last('ask').replacing === 'Five', JSON.stringify(a.last('ask')));
  a.send({ t: 'deny' });
  check('…refused in the app: the stranger is told, nothing linked', await waitFor(() => calls.some(c => c.method === 'PATCH' && c.url === `/webhooks/app1/${strangerToken}/messages/@original`
    && /didn't allow/.test(c.body.content)), 3000), JSON.stringify(calls.slice(-1)));
  r = await interact(pc('status', {}, { user: 'u8', name: 'Stranger' }));
  check("…and the stranger can't reach the PC", /No PC is linked/.test(contentOf(r)), contentOf(r));
  // Wrong codes: a few, then a wait.
  let wrongs = '';
  for (let n = 0; n < 6; n++)
    wrongs += /Too many wrong codes/.test(contentOf(await interact(pc('link', { code: 'QQQQQ2' }, { user: 'u9', name: 'Guesser' })))) ? 'W' : '.';
  check('guessing codes: refused after 5 wrong ones', wrongs === '.....W', wrongs);

  // ── commands ──
  r = await interact(pc('status'));
  check('/pc status: the PC\'s answer, seen only by them', r.type === 4 && contentOf(r) === '2 of 2 accounts in game' && r.data.embeds?.[0]?.title === '2 accounts in game'
    && r.data.flags === 64 && r.data.extra === undefined && r.data.allowed_mentions?.parse?.length === 0, JSON.stringify(r));
  r = await interact(pc('preset', { name: 'Farm' }));
  check('/pc preset: the preset named reaches the PC', contentOf(r) === 'Opening Farm…' && commands.at(-1)?.command === 'preset' && commands.at(-1)?.value === 'Farm', JSON.stringify(commands.at(-1)));
  const presetId = commands.at(-1).id, presetToken = `tok${tokenNo}`;
  a.send({ t: 'later', id: presetId, content: 'Farm: 3 of 3 accounts opened' });
  check('…how it went, once done: followed up in Discord', await waitFor(() => calls.some(c => c.method === 'POST' && c.url === `/webhooks/app1/${presetToken}`), 3000)
    && calls.find(c => c.url === `/webhooks/app1/${presetToken}`).body.flags === 64, JSON.stringify(calls.slice(-2)));
  r = await interact(pc('status', {}, { user: 'u6', name: 'Six' }));
  check('someone else: no PC of theirs, nothing reaches this one', /No PC is linked/.test(contentOf(r)) && commands.length === 2, `${contentOf(r)} ${commands.length}`);
  slow = true;
  r = await interact(pc('switch', { account: 'Beta' }));
  const switchToken = `tok${tokenNo}`;
  check('a slow answer: "thinking…" first (seen only by them)', r.type === 5 && r.data?.flags === 64, JSON.stringify(r));
  check('…then the answer edited in', await waitFor(() => calls.some(c => c.method === 'PATCH' && c.url === `/webhooks/app1/${switchToken}/messages/@original`), 4000)
    && calls.find(c => c.url === `/webhooks/app1/${switchToken}/messages/@original`).body.content === 'Switched Beta', JSON.stringify(calls.slice(-2)));
  slow = false;

  // ── screenshot ──
  r = await interact(pc('screenshot', { account: 'All accounts' }));
  check('/pc screenshot to an app that never said it can: told to update it, nothing sent', /update it there first/.test(contentOf(r)) && r.data.flags === 64
    && commands.at(-1)?.command !== 'screenshot', contentOf(r));
  a.send({ t: 'hello', name: 'GAMING-PC', accounts: ['Alpha', 'Beta'], presets: ['Farm', 'Daily'], lang: 'en', ask: true, shots: true });
  await sleep(300);
  r = await interact(pc('screenshot', { account: 'All accounts' }));
  const shotToken = `tok${tokenNo}`;
  check('/pc screenshot: "thinking…" at once (seen only by them), the command reaches the PC', r.type === 5 && r.data?.flags === 64
    && await waitFor(() => commands.at(-1)?.command === 'screenshot', 3000) && commands.at(-1).value === 'All accounts', JSON.stringify(r));
  const shotCommand = commands.at(-1);
  check("…with this answer's own link to send the pictures to (Discord's, no bot token), which other commands never get",
    shotCommand.reply === `http://127.0.0.1:${REST}/webhooks/app1/${shotToken}/messages/@original` && commands.filter(m => m.command !== 'screenshot').every(m => m.reply === undefined), JSON.stringify(shotCommand));
  check('…the PC sends its pictures there itself (none pass through the bot)', await sendShot(shotCommand.reply) === 200);
  a.send({ t: 'shown', id: shotCommand.id });
  const shotCall = calls.find(c => c.method === 'PATCH' && c.url === `/webhooks/app1/${shotToken}/messages/@original` && c.files);
  check('…in place of "thinking…": both pictures, the line about them', shotCall?.files.length === 2 && shotCall.body.content === 'Alpha · Beta', JSON.stringify(shotCall));
  await sleep(3500);
  check("…said done: no 'didn't answer in time' put over them later", !calls.some(c => c.url === `/webhooks/app1/${shotToken}/messages/@original` && /didn't answer/.test(c.body?.content ?? '')),
    JSON.stringify(calls.filter(c => c.url === `/webhooks/app1/${shotToken}/messages/@original`)));
  r = await interact(pc('screenshot', { account: 'Beta' }));
  const noShotToken = `tok${tokenNo}`;
  check('/pc screenshot of an account with no window: what the PC says is edited in', r.type === 5 && await waitFor(() => calls.some(c => c.method === 'PATCH'
    && c.url === `/webhooks/app1/${noShotToken}/messages/@original` && c.body.content === 'Beta has no Roblox window open.'), 3000), JSON.stringify(calls.slice(-1)));
  let shotsRefused = 0;
  for (let n = 0; n < 3; n++)
    if (/screenshots a minute at most/.test(contentOf(await interact(pc('screenshot', { account: 'Beta' }))))) shotsRefused++;
  check('more than 4 screenshots in a minute: the rest are refused', shotsRefused === 1, shotsRefused);

  // ── autocomplete ──
  r = await interact(pc('preset', { name: 'fa' }, { type: 4, focused: 'name' }));
  check('autocomplete: the PC\'s presets that match', r.type === 8 && JSON.stringify(r.data.choices) === JSON.stringify([{ name: 'Farm', value: 'Farm' }]), JSON.stringify(r));
  r = await interact(pc('close', { account: '' }, { type: 4, focused: 'account' }));
  check('…and for /pc close, "All accounts" then its accounts', JSON.stringify(r.data.choices.map(c => c.name)) === JSON.stringify(['All accounts', 'Alpha', 'Beta']), JSON.stringify(r));
  r = await interact(pc('close', { account: '' }, { type: 4, focused: 'account', user: 'u6' }));
  check('…someone else sees none of them', r.data.choices.length === 1 && r.data.choices[0].name === 'All accounts', JSON.stringify(r));
  r = await interact(pc('switch', { account: '' }, { type: 4, focused: 'account' }));
  const r2 = await interact(pc('screenshot', { account: '' }, { type: 4, focused: 'account' }));
  check('…"All accounts" for /pc switch and /pc screenshot too', [r, r2].every(x => JSON.stringify(x.data.choices.map(c => c.name)) === JSON.stringify(['All accounts', 'Alpha', 'Beta'])),
    JSON.stringify([r, r2]));

  // ── one socket per PC, a second PC ──
  const a2 = app(SECRET_A, answers);
  await a2.open();
  check('the same PC connecting again: the older socket is closed', await waitFor(() => a.closed === 4000, 3000), a.closed);
  a = a2;
  a.send({ t: 'hello', name: 'GAMING-PC', accounts: ['Alpha', 'Beta'], presets: ['Farm', 'Daily'], ask: true });
  const b = app(SECRET_B, () => ({ content: 'from the laptop' }));
  await b.open();
  b.send({ t: 'hello', name: 'LAPTOP', accounts: [], presets: [], ask: true });
  b.send({ t: 'code' });
  await waitFor(() => b.last('code'), 3000);
  r = await interact(pc('link', { code: b.last('code').code }));
  await waitFor(() => b.last('ask'), 3000);
  b.send({ t: 'allow' });
  check('a second PC linked to the same person', /LAPTOP/.test(contentOf(r)) && await waitFor(() => b.last('link')?.linked, 3000), contentOf(r));
  r = await interact(pc('status'));
  check('…a command without saying which PC: asked to pick one', /more than one PC/.test(contentOf(r)) && /GAMING-PC/.test(contentOf(r)) && /LAPTOP/.test(contentOf(r)), contentOf(r));
  r = await interact(pc('status', { pc: 'laptop' }));
  check('…with the PC named: that PC answers', contentOf(r) === 'from the laptop', contentOf(r));
  r = await interact(pc('status', { pc: '' }, { type: 4, focused: 'pc' }));
  check('…autocomplete offers both PCs', JSON.stringify(r.data.choices.map(c => c.name)) === JSON.stringify(['GAMING-PC', 'LAPTOP']), JSON.stringify(r));

  // ── offline, unlinking ──
  b.ws.close();
  await sleep(500);
  r = await interact(pc('status'));
  check('one PC closed: commands go to the one connected', contentOf(r) === '2 of 2 accounts in game', contentOf(r));
  r = await interact(pc('status', { pc: 'LAPTOP' }));
  check('…the closed one, named: said to be not connected', /LAPTOP\*\* isn't connected/.test(contentOf(r)), contentOf(r));
  r = await interact(pc('unlink', { pc: 'LAPTOP' }));
  check('/pc unlink: the PC unlinked', /Unlinked: \*\*LAPTOP\*\*/.test(contentOf(r)), contentOf(r));
  const b2 = app(SECRET_B);
  await b2.open();
  check('…its app, connecting again, hears it isn\'t linked', await waitFor(() => b2.last('link'), 3000) && b2.last('link').linked === false, JSON.stringify(b2.heard));
  b2.ws.close();
  a.send({ t: 'unlink' });
  check('unlinked from the app (Control from Discord turned off): told so', await waitFor(() => a.last('link')?.linked === false, 3000), JSON.stringify(a.last('link')));
  r = await interact(pc('status'));
  check('…and Discord can\'t reach it any more', /No PC is linked/.test(contentOf(r)), contentOf(r));
  a.ws.close();

  // ── an app from 1.0.12 (it never says it can ask): linked at once, as before ──
  const old = app(crypto.randomBytes(32).toString('base64url'));
  await old.open();
  old.send({ t: 'hello', name: 'OLD-PC', accounts: [], presets: [] });
  old.send({ t: 'code' });
  await waitFor(() => old.last('code'), 3000);
  r = await interact(pc('link', { code: old.last('code').code }, { user: 'u10', name: 'Ten' }));
  check('an app from before links could be allowed (1.0.12): linked at once, as before', /Linked: \*\*OLD-PC\*\*/.test(contentOf(r))
    && await waitFor(() => old.last('link')?.linked, 3000) && !old.last('ask'), contentOf(r));
  old.ws.close();

  // ── an app sending far more than an app does: its socket is closed ──
  const flood = app(crypto.randomBytes(32).toString('base64url'));
  await flood.open();
  for (let n = 0; n < 70; n++) flood.send({ t: 'hello', name: `x${n}`, accounts: [], presets: [] });
  check('a socket sending more than 60 messages a minute is closed', await waitFor(() => flood.closed === 1008, 3000), flood.closed);

  // ── too many at once ──
  let refused = 0;
  for (let n = 0; n < 22; n++)
    if (/Too many commands/.test(contentOf(await interact(pc('status', {}, { user: 'u7' }))))) refused++;
  check('more than 20 commands in a minute: the rest are refused', refused === 2, refused);
} finally {
  stopDev();
  gw.close();
  rest.close();
}
console.log(failures ? `${failures} check(s) failed` : 'All checks passed');
process.exit(failures ? 1 : 0);
