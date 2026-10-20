// The bot Worker (DiscordBot/worker) against a fake Discord Gateway, run by wrangler dev: every way a real connection
// ends, and Cloudflare restarting the object, must end with the bot online again (or stopped, for a refused token). No
// Discord account or token needed. In this folder: npm install, then node gateway.mjs (about a minute).
import { spawn, execSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { WebSocketServer } from 'ws';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const WORKER = path.join(HERE, '..', 'worker');
const WRANGLER = path.join(HERE, 'node_modules', 'wrangler', 'bin', 'wrangler.js');
// Short: wrangler's object files under a long folder pass Windows' 260-character limit and fail as 'internal error'.
const PERSIST = path.join(process.env.TEMP || HERE, 'bkp');
const GW_PORT = 8790, DEV_PORT = 8788;
fs.rmSync(PERSIST, { recursive: true, force: true });

const sleep = ms => new Promise(r => setTimeout(r, ms));
let failures = 0;
const check = (name, ok, detail = '') => { console.log(`${ok ? 'PASS' : 'FAIL'}  ${name}${ok ? '' : `  [${detail}]`}`); if (!ok) failures++; };

// ── fake Gateway ──
const conns = []; // { ws, ops: [], hello, mute, resumeRefuse }
const sessions = new Set();
let sessionNo = 0, seq = 0;
const wss = new WebSocketServer({ port: GW_PORT });
let refuseNextResume = false;
wss.on('connection', ws => {
  const c = { ws, ops: [], closed: null, mute: false, n: conns.length + 1 };
  conns.push(c);
  ws.on('close', code => { c.closed = code; });
  ws.send(JSON.stringify({ op: 10, d: { heartbeat_interval: 1000 } }));
  ws.on('message', raw => {
    const m = JSON.parse(raw);
    c.ops.push(m);
    if (m.op === 1 && !c.mute) ws.send(JSON.stringify({ op: 11 }));
    if (m.op === 2) {
      c.identify = m.d;
      const id = `s-${++sessionNo}`;
      sessions.add(id);
      ws.send(JSON.stringify({ op: 0, t: 'READY', s: ++seq, d: { session_id: id, resume_gateway_url: `ws://127.0.0.1:${GW_PORT}`, user: { username: 'TestBot' } } }));
    }
    if (m.op === 6) {
      c.resume = m.d;
      if (refuseNextResume || !sessions.has(m.d.session_id)) {
        refuseNextResume = false;
        ws.send(JSON.stringify({ op: 9, d: false }));
      } else {
        ws.send(JSON.stringify({ op: 0, t: 'RESUMED', s: ++seq, d: {} }));
      }
    }
  });
});
const last = () => conns[conns.length - 1];
const waitFor = async (cond, ms) => { const end = Date.now() + ms; while (Date.now() < end) { if (cond()) return true; await sleep(50); } return cond(); };

// ── wrangler dev ──
let dev = null;
function startDev(token) {
  fs.mkdirSync(PERSIST, { recursive: true });
  const out = fs.openSync(path.join(PERSIST, 'wrangler-dev.log'), 'a'); // what wrangler said, if a check fails
  dev = spawn(process.execPath, [WRANGLER, 'dev', '--ip', '127.0.0.1', '--port', String(DEV_PORT), '--test-scheduled', '--persist-to', PERSIST,
    '--var', `GATEWAY_URL:ws://127.0.0.1:${GW_PORT}`, '--var', `DISCORD_TOKEN:${token}`, '--show-interactive-dev-session=false'],
  { cwd: WORKER, stdio: ['ignore', out, out], env: { ...process.env, WRANGLER_SEND_METRICS: 'false', CI: '1' } });
}
function stopDev() {
  try { execSync(`taskkill /pid ${dev.pid} /t /f`, { stdio: 'ignore' }); } catch {}
}
async function devUp() {
  for (let i = 0; i < 120; i++) { try { const r = await fetch(`http://127.0.0.1:${DEV_PORT}/status`); if (r.ok) return true; } catch {} await sleep(500); }
  return false;
}
const cron = () => fetch(`http://127.0.0.1:${DEV_PORT}/__scheduled?cron=*+*+*+*+*`).then(r => r.text()).catch(e => String(e));
const status = () => fetch(`http://127.0.0.1:${DEV_PORT}/status`).then(r => r.json());

try {
  startDev('test-token');
  check('wrangler dev serves the worker', await devUp());

  // A. The minute's cron connects: identify with the token, no intents, a presence; Discord answers READY.
  await cron();
  check('cron connects and identifies', await waitFor(() => conns.length === 1 && conns[0].identify, 8000), conns.length);
  const id = conns[0]?.identify;
  check('…with the token, no intents and an online presence', id?.token === 'test-token' && id?.intents === 0 && id?.presence?.status === 'online', JSON.stringify(id));
  await sleep(3500);
  const beats = conns[0].ops.filter(o => o.op === 1);
  check('heartbeats keep coming, about one a second (the alarm)', beats.length >= 2, beats.length);
  check('…carrying the last sequence number', beats.at(-1)?.d === 1, JSON.stringify(beats.at(-1)));
  let s = await status();
  check('status: online', s.state === 'online', JSON.stringify(s));

  // B. Discord asks to reconnect: a new connection resumes the session.
  conns[0].ws.send(JSON.stringify({ op: 7, d: null }));
  check('asked to reconnect, it resumes on a new connection', await waitFor(() => conns.length === 2 && conns[1].resume, 8000), conns.length);
  check('…the same session, with its sequence number', conns[1]?.resume?.session_id === 's-1' && conns[1]?.resume?.seq === 1, JSON.stringify(conns[1]?.resume));
  check('…and the old connection is closed', await waitFor(() => conns[0].closed != null, 3000), conns[0].closed);

  // C. Heartbeats stop being answered: the dead connection is dropped and the session resumed.
  conns[1].mute = true;
  check('no heartbeat answers: dropped and resumed', await waitFor(() => conns.length === 3 && conns[2].resume, 8000), conns.length);

  // D. Session timed out (4009): identify again rather than resume.
  await sleep(1500);
  conns[2].ws.close(4009, 'session timed out');
  check('session ended (4009): a new identify', await waitFor(() => conns.length === 4 && conns[3].identify, 8000), `${conns.length} ${JSON.stringify(last()?.resume)}`);
  await sleep(1500);

  // E. Cloudflare restarts the object (wrangler dev killed and started again, storage kept): the saved alarm connects
  //    again and resumes the saved session without waiting for the cron.
  stopDev();
  check('restart: the old connection ends with the process', await waitFor(() => conns[3].closed != null, 5000), conns[3].closed);
  const before = conns.length;
  startDev('test-token');
  check('wrangler dev back', await devUp());
  const backByAlarm = await waitFor(() => conns.length > before && conns[before].resume, 10000);
  if (!backByAlarm) await cron();
  check('after the restart it connects again by itself (alarm, not cron)', backByAlarm, conns.length - before);
  check('…resuming the saved session', await waitFor(() => conns.length > before && conns[before].resume?.session_id === 's-2', 8000), JSON.stringify(conns[before]?.resume));
  await sleep(1500);

  // F. A resume refused (invalid session, not resumable): identify after a short wait.
  refuseNextResume = true;
  const f0 = conns.length;
  last().ws.close(4000, 'unknown error');
  check('resume refused: a fresh identify within a few seconds', await waitFor(() => conns.slice(f0).some(c => c.identify), 9000), conns.slice(f0).map(c => c.identify ? 'identify' : c.resume ? 'resume' : '?').join(','));
  await sleep(1500);

  // G. Token refused (4004): it stops, and the cron doesn't start it again with the same token.
  const g0 = conns.length;
  last().ws.close(4004, 'Authentication failed.');
  await sleep(3000);
  await cron();
  await sleep(2000);
  s = await status();
  check('token refused: stopped, no new connection', conns.length === g0 && s.state === 'stopped' && s.stopped?.code === 4004, `${conns.length - g0} ${s.state}`);
  check('…and the stop keeps no token, only a fingerprint', !JSON.stringify(s).includes('test-token'), JSON.stringify(s.stopped));

  // H. A new token (redeploy): the cron starts it again.
  stopDev();
  startDev('test-token-2');
  check('wrangler dev back with a new token', await devUp());
  await cron();
  check('new token: connects and identifies again', await waitFor(() => conns.length > g0 && conns.at(-1).identify?.token === 'test-token-2', 10000), conns.length - g0);
  await sleep(2500);

  s = await status();
  console.log(JSON.stringify({ state: s.state, log: s.log }, null, 1));
  check('status at the end: online, with the restarts counted', s.state === 'online' && s.log.restarts >= 2 && s.log.onlinePct > 0, JSON.stringify(s.log));
  check('nothing in the log holds a token', !JSON.stringify(s).includes('test-token'));
} finally {
  stopDev();
  wss.close();
}
console.log(failures ? `${failures} check(s) failed` : 'All checks passed');
process.exit(failures ? 1 : 0);
