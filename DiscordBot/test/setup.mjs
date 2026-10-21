// /setup (DiscordBot/worker/setup.js) against a fake Discord server, run by wrangler dev: refused to anyone but
// Bloxkeep's developers; then the roles, channels, Community, AutoMod, icon and posts made (no onboarding), Discord's
// own categories tidied away, nothing made twice when run again, its roles renamed or not; and #dont-type-here banning whoever posts in it (not
// moderators, not bots, once per person). No Discord account or token needed. In this folder: npm install, then
// node setup.mjs (about a minute).
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
const PERSIST = path.join(process.env.TEMP || HERE, 'bks'); // short: see gateway.mjs
const GW = 8790, REST = 8791, DEV = 8788, G = '900', OWNER = '111', BOT = '222';
fs.rmSync(PERSIST, { recursive: true, force: true });
fs.mkdirSync(PERSIST, { recursive: true });

const sleep = ms => new Promise(r => setTimeout(r, ms));
let failures = 0;
const check = (name, ok, detail = '') => { console.log(`${ok ? 'PASS' : 'FAIL'}  ${name}${ok ? '' : `  [${String(detail).slice(0, 400)}]`}`); if (!ok) failures++; };
const waitFor = async (cond, ms) => { const end = Date.now() + ms; while (Date.now() < end) { if (cond()) return true; await sleep(50); } return cond(); };

// ── the fake Gateway, which can also send the bot a message event ──
let socket = null, seq = 1;
const identifies = [];
const gw = new WebSocketServer({ port: GW });
gw.on('connection', ws => {
  socket = ws;
  ws.send(JSON.stringify({ op: 10, d: { heartbeat_interval: 30000 } }));
  ws.on('message', raw => {
    const m = JSON.parse(raw);
    if (m.op === 1) ws.send(JSON.stringify({ op: 11 }));
    if (m.op === 2) identifies.push(m.d);
    if (m.op === 2 || m.op === 6) ws.send(JSON.stringify({ op: 0, t: 'READY', s: seq++, d: { session_id: 's1', resume_gateway_url: `ws://127.0.0.1:${GW}`, user: { username: 'Bloxkeep' } } }));
  });
});
const message = (channel, author, roles = [], bot = false) =>
  socket.send(JSON.stringify({ op: 0, t: 'MESSAGE_CREATE', s: seq++, d: { id: `m${seq}`, guild_id: G, channel_id: channel, author: { id: author, username: `user_${author}`, bot }, member: { roles }, content: '' } }));

// ── the fake server: what a new server has, made by someone whose Discord is in Vietnamese (Discord's own categories,
//    #chung and the Chung voice channel) ──
let next = 1000;
const id = () => String(next++);
const guild = { id: G, owner_id: OWNER, features: [], icon: null };
const roles = [{ id: G, name: '@everyone', position: 0, permissions: '0' }];
const channels = [
  { id: 't1', name: 'Kênh Chat', type: 4, parent_id: null }, { id: 'c1', name: 'chung', type: 0, parent_id: 't1' },
  { id: 't2', name: 'Kênh đàm thoại', type: 4, parent_id: null }, { id: 'v1', name: 'Chung', type: 2, parent_id: 't2' },
];
const messages = {}, automod = [], bans = [], memberRoles = [], answers = [];
let onboarding = null;
const calls = [];
const rest = http.createServer((req, res) => {
  let raw = '';
  req.on('data', c => { raw += c; });
  req.on('end', () => {
    const body = raw ? JSON.parse(raw) : null, u = req.url.split('?')[0], m = req.method;
    calls.push({ m, u, body, reason: req.headers['x-audit-log-reason'] });
    const send = (status, value) => { res.writeHead(status, { 'Content-Type': 'application/json' }); res.end(value === undefined ? '' : JSON.stringify(value)); };
    const community = guild.features.includes('COMMUNITY');
    let x;
    if (m === 'GET' && u === '/oauth2/applications/@me') return send(200, { id: 'app1', team: { owner_user_id: OWNER, members: [{ user: { id: OWNER }, role: 'admin', membership_state: 2 }] } });
    if (m === 'GET' && u === '/users/@me') return send(200, { id: BOT, username: 'Bloxkeep' });
    if (u === `/guilds/${G}`) { if (m === 'PATCH') { if (body.features?.includes('COMMUNITY') && !(body.rules_channel_id && body.public_updates_channel_id && body.verification_level >= 1 && body.explicit_content_filter === 2)) return send(400, { message: 'community needs rules, updates, verification, filter' }); Object.assign(guild, body); } return send(200, guild); }
    if (u === `/guilds/${G}/roles`) {
      if (m === 'GET') return send(200, roles);
      if (m === 'POST') { roles.forEach(r => { if (r.id !== G) r.position++; }); roles.push(x = { id: id(), position: 1, ...body }); return send(200, x); }
      if (m === 'PATCH') { body.forEach(p => { roles.find(r => r.id === p.id).position = p.position; }); return send(200, roles); }
    }
    if ((x = new RegExp(`^/guilds/${G}/members/(\\d+)/roles/(\\d+)$`).exec(u)) && m === 'PUT') { memberRoles.push([x[1], x[2]]); return send(204); }
    if (u === `/guilds/${G}/channels`) {
      if (m === 'GET') return send(200, channels);
      if (m === 'POST') { if ([5, 15].includes(body.type) && !community) return send(400, { message: 'needs Community' }); channels.push(x = { id: id(), parent_id: null, ...body }); return send(200, x); }
      if (m === 'PATCH') { body.forEach(p => { channels.find(c => c.id === p.id).position = p.position; }); return send(204); }
    }
    if ((x = /^\/channels\/(\w+)$/.exec(u))) {
      const c = channels.find(c => c.id === x[1]);
      if (!c) return send(404, {});
      if (m === 'PATCH') { Object.assign(c, body); return send(200, c); }
      if (m === 'DELETE') { channels.splice(channels.indexOf(c), 1); return send(200, c); }
    }
    if (u === `/guilds/${G}/onboarding` && m === 'PUT') {
      if (!community) return send(400, { message: 'needs Community' });
      // Like Discord: an id (a snowflake) on each question and each answer, new ones too.
      if (!body.prompts.every(p => /^\d{17,20}$/.test(p.id ?? '') && p.options.every(o => /^\d{17,20}$/.test(o.id ?? ''))))
        return send(400, { message: 'Invalid Form Body', code: 50035, errors: { prompts: { 0: { id: { _errors: [{ code: 'BASE_TYPE_REQUIRED' }] } } } } });
      onboarding = body; return send(200, body);
    }
    if (u === `/guilds/${G}/auto-moderation/rules`) { if (m === 'GET') return send(200, automod); automod.push(x = { id: id(), ...body }); return send(200, x); }
    if ((x = /^\/channels\/(\w+)\/messages$/.exec(u))) {
      messages[x[1]] ??= [];
      if (m === 'GET') return send(200, [...messages[x[1]]].reverse());
      messages[x[1]].push(x = { id: id(), author: { id: BOT }, ...body }); return send(200, x);
    }
    if ((x = /^\/channels\/(\w+)\/messages\/(\w+)$/.exec(u)) && m === 'PATCH') { Object.assign(messages[x[1]].find(p => p.id === x[2]), body); return send(200, {}); }
    if (/^\/channels\/\w+\/pins\/\w+$/.test(u)) return send(204);
    if (/^\/webhooks\/app1\/\w+\/messages\/@original$/.test(u)) { answers.push(body.content); return send(200, {}); }
    if ((x = new RegExp(`^/guilds/${G}/bans/(\\d+)$`).exec(u)) && m === 'PUT') { bans.push({ user: x[1], ...body, reason: decodeURIComponent(req.headers['x-audit-log-reason'] ?? '') }); return send(204); }
    if (m === 'PUT' && u.startsWith('/applications/')) return send(200, []);
    if (u === '/icon.png') { res.writeHead(200, { 'Content-Type': 'image/png' }); return res.end(Buffer.from('89504e470d0a1a0a', 'hex')); }
    send(404, { message: `fake: ${m} ${u}` });
  });
}).listen(REST);
const named = (name, type) => channels.find(c => c.name === name && (type === undefined || c.type === type));
const role = name => roles.find(x => x.name === name || x.name.startsWith(`${name} `));

// ── signing like Discord ──
const { publicKey, privateKey } = crypto.generateKeyPairSync('ed25519');
const PUBLIC_HEX = publicKey.export({ format: 'der', type: 'spki' }).subarray(-32).toString('hex');
async function interact(payload) {
  const body = JSON.stringify(payload), ts = String(Math.floor(Date.now() / 1000));
  const sig = crypto.sign(null, Buffer.from(ts + body), privateKey).toString('hex');
  const res = await fetch(`http://127.0.0.1:${DEV}/interactions`, { method: 'POST', body, headers: { 'Content-Type': 'application/json', 'X-Signature-Timestamp': ts, 'X-Signature-Ed25519': sig } });
  return res.json();
}
const setup = (user, token) => interact({ type: 2, id: 'i', application_id: 'app1', token, guild_id: G, guild_locale: 'en-US', locale: 'en-US',
  member: { permissions: String(1 << 3), user: { id: user } }, data: { id: 'c', name: 'setup', type: 1 } });

// ── wrangler dev ──
let dev;
function startDev() {
  const out = fs.openSync(path.join(PERSIST, 'wrangler-dev.log'), 'a');
  const vars = {
    OBJECT_NAME: 'bot-setup', OLD_OBJECT_NAME: '', TRACE_URL: 'http://127.0.0.1:9/', GATEWAY_URL: `ws://127.0.0.1:${GW}`, DISCORD_TOKEN: 'test-token',
    DISCORD_API: `http://127.0.0.1:${REST}`, DISCORD_APP_ID: 'app1', DISCORD_PUBLIC_KEY: PUBLIC_HEX, ICON_URL: `http://127.0.0.1:${REST}/icon.png`,
    WATCH_URL: 'http://127.0.0.1:9/', STATUS_URL: 'http://127.0.0.1:9/', LATEST_URL: 'http://127.0.0.1:9/',
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

try {
  startDev();
  check('wrangler dev serves the worker', await devUp());
  await fetch(`http://127.0.0.1:${DEV}/__scheduled?cron=*+*+*+*+*`);
  check('the bot connects, asking for message events (who posted where) only', await waitFor(() => identifies.length > 0, 8000) && identifies[0].intents === 512, JSON.stringify(identifies[0]?.intents));

  // Anyone else: refused, after "thinking…".
  let r = await setup('333', 'tok1');
  check('/setup answers "thinking…" at once, seen only by them', r.type === 5 && r.data?.flags === 64, JSON.stringify(r));
  check('…and refuses anyone but Bloxkeep\'s developers', await waitFor(() => answers.length === 1, 8000) && /Only Bloxkeep's developers/.test(answers[0]) && roles.length === 1, answers[0]);

  // The owner: the server set up.
  await setup(OWNER, 'tok2');
  check('the owner\'s /setup finishes, with nothing it couldn\'t do', await waitFor(() => answers.length === 2, 20000) && /^Done\. Made: /.test(answers[1]) && !/Couldn't/.test(answers[1]), answers[1]);
  const names = roles.filter(x => x.id !== G).sort((a, b) => b.position - a.position).map(x => x.name).join(', ');
  check('roles, top to bottom, each with its emoji: Admin, Moderator, Roblox updates', names === 'Admin 👑, Moderator 🛡️, Roblox updates 🔔', names);
  check('…Admin for whoever ran it', memberRoles.some(([u, r]) => u === OWNER && r === role('Admin').id));
  check('…"Roblox updates" may be pinged', role('Roblox updates')?.mentionable === true);
  const cat = name => named(name, 4)?.id;
  const under = (cat, list) => list.every(([name, type]) => named(name, type)?.parent_id === cat);
  check('Info: the trap, welcome, rules, announcements, roblox-updates', under(cat('Info'), [['dont-type-here', 0], ['welcome', 0], ['rules', 0], ['announcements', 5], ['roblox-updates', 5]]), JSON.stringify(channels.map(c => [c.name, c.type, c.parent_id])));
  const count = named('🟢 Using Bloxkeep: 0', 2), ows = count?.permission_overwrites ?? [];
  check('Info starts with a voice channel saying how many use Bloxkeep: seen by all, joined by nobody, renamed by the bot alone',
    count?.parent_id === cat('Info') && count.position === 0 && named('dont-type-here').position === 1
    && ows.some(o => o.id === G && BigInt(o.deny) & (1n << 20n)) && ows.some(o => o.id === BOT && o.type === 1 && BigInt(o.allow) & (1n << 4n)), JSON.stringify(count));
  check('Community: general (Discord\'s #chung, renamed), general-vn, bot-commands, the voice channel (renamed General), no showcase or off-topic',
    under(cat('Community'), [['general', 0], ['general-vn', 0], ['bot-commands', 0], ['General', 2]])
    && !named('showcase') && !named('off-topic') && named('general').id === 'c1' && named('General').id === 'v1');
  check('Support: the help, bug-reports and ideas forums, with tags', under(cat('Support'), [['help', 15], ['bug-reports', 15], ['ideas', 15]]) && named('help').available_tags.some(t => t.name === 'Solved'));
  check('Staff: mod-chat and mod-log, hidden from everyone else', under(cat('Staff'), [['mod-chat', 0], ['mod-log', 0]])
    && named('mod-log').permission_overwrites.some(o => o.id === G && BigInt(o.deny) & (1n << 10n)));
  check('Discord\'s own categories are gone', !named('Kênh Chat') && !named('Kênh đàm thoại') && !named('chung'));
  check('rules and welcome: everyone reads, only moderators and the bot post', ['rules', 'welcome', 'roblox-updates'].every(n => named(n).permission_overwrites.some(o => o.id === G && BigInt(o.deny) & (1n << 11n))
    && named(n).permission_overwrites.some(o => o.id === BOT && o.type === 1 && BigInt(o.allow) & (1n << 11n))));
  check('Community on: rules channel, updates to #mod-log, verification, media scanned', guild.features.includes('COMMUNITY') && guild.rules_channel_id === named('rules').id
    && guild.public_updates_channel_id === named('mod-log').id && guild.verification_level === 2 && guild.explicit_content_filter === 2);
  check('no onboarding (newcomers get in through Captcha.bot\'s #verify)', onboarding === null && !calls.some(c => c.u.endsWith('/onboarding')));
  const cookie = automod.find(a => a.name === 'Roblox cookies and scams');
  check('AutoMod: four rules, a pasted Roblox cookie blocked and reported in #mod-log', automod.length === 4 && cookie?.trigger_metadata.regex_patterns.includes('_\\|WARNING:-DO-NOT-SHARE-THIS')
    && cookie.actions.some(a => a.type === 2 && a.metadata.channel_id === named('mod-log').id), JSON.stringify(automod.map(a => a.name)));
  check('the server\'s icon', /^data:image\/png;base64,/.test(guild.icon ?? ''));
  check('posts in #dont-type-here, #rules and #welcome', ['dont-type-here', 'rules', 'welcome'].every(n => messages[named(n).id]?.length === 1)
    && messages[named('welcome').id][0].components[0].components.map(b => b.label).join() === 'Download,GitHub,Website');
  const rulesText = messages[named('rules').id]?.[0]?.embeds[0].description ?? '';
  check('…the rules: six, bot commands in #bot-commands, nothing about exploits', (rulesText.match(/^\*\*\d+\.\*\*/gm) ?? []).length === 6
    && rulesText.includes('#bot-commands') && !/exploit/i.test(rulesText), rulesText);
  check('…the welcome post points Vietnamese speakers to #general-vn', messages[named('welcome').id][0].embeds[0].description.includes(`<#${named('general-vn').id}>`));
  check('the board in #roblox-updates, pinned', messages[named('roblox-updates').id]?.some(p => p.embeds?.[0]?.title === 'Roblox and Bloxkeep now') && calls.some(c => c.m === 'PUT' && c.u.startsWith(`/channels/${named('roblox-updates').id}/pins/`)));

  // Again, one role's emoji taken off by hand and the count channel renamed by the bot since: nothing new.
  const counts = () => [roles.length, channels.length, automod.length, Object.values(messages).flat().length].join();
  const before = counts();
  role('Moderator').name = 'Moderator';
  count.name = '🟢 Using Bloxkeep: 3';
  await setup(OWNER, 'tok3');
  check('run again: nothing made twice, its posts updated in place', await waitFor(() => answers.length === 3, 20000) && /everything was already there/.test(answers[2]) && counts() === before, `${answers[2]} ${before} → ${counts()}`);

  // The trap.
  const trap = named('dont-type-here').id, modRole = role('Moderator').id;
  message(trap, '444');
  check('a message in #dont-type-here: its sender banned, the last hour of their messages deleted', await waitFor(() => bans.length === 1, 5000)
    && bans[0].user === '444' && bans[0].delete_message_seconds === 3600 && /dont-type-here/.test(bans[0].reason), JSON.stringify(bans));
  check('…and #mod-log told', await waitFor(() => messages[named('mod-log').id]?.some(p => p.embeds?.[0]?.title === 'Banned by the trap'), 3000));
  message(trap, '444');
  message(trap, '555', [modRole]);
  message(trap, OWNER);
  message(trap, '666', [], true);
  message(named('general').id, '777');
  await sleep(1500);
  check('the same person again, a moderator, the owner, a bot, or a message elsewhere: nobody else banned', bans.length === 1, JSON.stringify(bans.map(b => b.user)));
} finally {
  stopDev();
  gw.close();
  rest.close();
}
console.log(failures ? `${failures} check(s) failed` : 'All checks passed');
process.exit(failures ? 1 : 0);
