import { createServer } from 'node:http';
import { randomBytes, timingSafeEqual } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import jpeg from 'jpeg-js';
import { createCapture } from './capture.mjs';
import { loadRoster, nameWindows, sendCommand } from './window-labels.mjs';
import { createLive } from './live-video.mjs';

const key = randomBytes(24).toString('hex');
const expected = Buffer.from(`Bearer ${key}`);
const bridgeKey = process.env.BK_MONITOR_KEY;
const capture = createCapture(process.env.BK_MONITOR_APP);
// The capture worker answers one command at a time (a second one while it works gets nothing): the window list and the
// tiles' pictures wait their turn.
let captureTurn = Promise.resolve();
function inTurn(command) {
  const asked = captureTurn.then(() => capture.call(command));
  captureTurn = asked.catch(() => {});
  return asked;
}
const live = createLive(bridgeKey);
const web = new Map();
for (const [path, file, type] of [
  ['/', 'index.html', 'text/html; charset=utf-8'],
  ['/monitor.css', 'monitor.css', 'text/css; charset=utf-8'],
  ['/monitor.js', 'monitor.js', 'text/javascript; charset=utf-8'],
  ['/monitor-language.js', 'monitor-language.js', 'text/javascript; charset=utf-8'],
  ['/bloxkeep.svg', 'bloxkeep.svg', 'image/svg+xml'],
  ['/regular.ttf', 'BeVietnamPro-Regular.ttf', 'font/ttf'],
  ['/bold.ttf', 'BeVietnamPro-Bold.ttf', 'font/ttf'],
]) {
  web.set(path, { type, data: await readFile(new URL(`./public/${file}`, import.meta.url)) });
}
const hls = new URL('./node_modules/hls.js/dist/hls.light.min.mjs', import.meta.url);
web.set('/hls.js', { type: 'text/javascript; charset=utf-8', data: await readFile(hls) });

const width = 640;
const height = 360;
const rgba = Buffer.alloc(width * height * 4);
let demo = { at: 0, jpeg: undefined };
function sampleFrame() {
  const now = Date.now();
  if (now - demo.at < 250) return demo.jpeg;
  const t = Math.floor(now / 125) % 64;
  const colors = [[94, 174, 228], [55, 109, 145], [195, 161, 79], [27, 52, 67]];
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const i = (y * width + x) * 4;
      const wave = Math.sin((x + t * 10) * 0.013 + y * 0.022);
      const color = y > 311 ? (x < t * 10 ? colors[2] : colors[3])
        : y > 180 + wave * 35 ? colors[3] : y > 112 + wave * 22 ? colors[1] : colors[0];
      rgba[i] = color[0];
      rgba[i + 1] = color[1];
      rgba[i + 2] = color[2];
      rgba[i + 3] = 255;
    }
  }
  demo = { at: now, jpeg: jpeg.encode({ data: rgba, width, height }, 72).data };
  return demo.jpeg;
}

// A command's body: {do, window?, preset?}, at most 1 KB, each part as the app expects it; else undefined.
async function readOrder(req) {
  let body = '';
  for await (const chunk of req) {
    body += chunk;
    if (body.length > 1024) return undefined;
  }
  let order;
  try {
    order = JSON.parse(body);
  } catch {
    return undefined;
  }
  const window = /^\d{1,10}-\d{1,19}$/;
  const preset = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
  switch (order?.do) {
    case 'rejoin':
    case 'switch':
    case 'close':
      return typeof order.window === 'string' && window.test(order.window) ? { do: order.do, window: order.window } : undefined;
    case 'preset':
      return typeof order.preset === 'string' && preset.test(order.preset) ? { do: 'preset', preset: order.preset } : undefined;
    case 'ram':
      return { do: 'ram' };
    default:
      return undefined;
  }
}

function reply(res, code, type, body) {
  res.writeHead(code, { 'Content-Type': type });
  res.end(body);
}

async function handle(req, res) {
  res.setHeader('Cache-Control', 'no-store');
  res.setHeader('X-Content-Type-Options', 'nosniff');
  res.setHeader('Referrer-Policy', 'no-referrer');
  res.setHeader('X-Robots-Tag', 'noindex, nofollow');
  res.setHeader('Content-Security-Policy', "default-src 'self'; img-src 'self' blob:; media-src 'self' blob:; worker-src 'self' blob:; object-src 'none'; frame-ancestors 'none'; base-uri 'none'");
  const path = new URL(req.url, 'http://localhost').pathname;
  if (req.method !== 'GET' && !(req.method === 'POST' && (path.startsWith('/api/live/') || path === '/api/command'))) {
    res.writeHead(405, { Allow: 'GET' }).end();
    return;
  }
  if (web.has(path)) {
    const { type, data } = web.get(path);
    reply(res, 200, type, data);
    return;
  }
  const videoId = /(?:^|;\s*)bk_live=([0-9a-f]{48})(?:;|$)/.exec(req.headers.cookie ?? '')?.[1];
  const piece = req.method === 'GET' && /^\/api\/live\/(index\.m3u8|init\.mp4|chunk-\d{6}\.m4s)$/.exec(path);
  if (piece) {
    const file = piece[1];
    const pending = live.media(videoId, file);
    if (!pending) {
      res.writeHead(401).end();
      return;
    }
    const body = await pending;
    if (body) reply(res, 200, file.endsWith('.m3u8') ? 'application/vnd.apple.mpegurl' : 'video/mp4', body);
    else res.writeHead(404).end();
    return;
  }
  const auth = Buffer.from(req.headers.authorization ?? '');
  if (auth.length !== expected.length || !timingSafeEqual(auth, expected)) {
    res.writeHead(401).end();
    return;
  }
  if (path === '/api/windows') {
    const [body, roster] = await Promise.all([
      inTurn('list'),
      loadRoster(bridgeKey),
    ]);
    if (!body) {
      res.writeHead(503).end();
      return;
    }
    const windows = nameWindows(JSON.parse(body.toString()), roster);
    reply(res, 200, 'application/json', JSON.stringify({
      windows, accountState: roster.state, captureReady: capture.installed, video: true,
      // For the commands: presets to open a game with, the windows that are an account's (Rejoin, Another server,
      // Close), whether Clean RAM runs without a question at the PC, and the commands that ended lately.
      presets: Array.isArray(roster.presets) ? roster.presets : [],
      commands: Array.isArray(roster.commands) ? roster.commands : [],
      ram: roster.ram === true,
      done: Array.isArray(roster.done) ? roster.done : [],
      // The app's language, for a page whose language wasn't picked on it.
      ...(typeof roster.language === 'string' && /^[a-z]{2,3}$/.test(roster.language) ? { language: roster.language } : {}),
    }));
    return;
  }
  // A command for the app (remote_page.rs): only these, each part checked before it goes on.
  if (req.method === 'POST' && path === '/api/command') {
    const order = await readOrder(req);
    if (!order) {
      res.writeHead(400).end();
      return;
    }
    reply(res, 200, 'application/json', JSON.stringify(await sendCommand(bridgeKey, order)));
    return;
  }
  const open = req.method === 'POST' && /^\/api\/live\/start\/(\d{1,10}-\d{1,19})$/.exec(path);
  if (open) {
    try {
      const sharp = new URL(req.url, 'http://localhost').searchParams.get('sharp') === '1';
      const id = await live.start(open[1], sharp);
      const secure = req.headers['x-forwarded-proto'] === 'https' ? '; Secure' : '';
      res.setHeader('Set-Cookie', `bk_live=${id}; Path=/api/live; HttpOnly; SameSite=Strict${secure}`);
      reply(res, 200, 'application/json', JSON.stringify({ playlist: '/api/live/index.m3u8' }));
    } catch (error) {
      reply(res, 409, 'application/json', JSON.stringify({ code: error.code ?? 'openFailed' }));
    }
    return;
  }
  if (req.method === 'POST' && ['/api/live/keep', '/api/live/stop'].includes(path)) {
    const matched = live.heartbeat(videoId);
    if (path.endsWith('/stop') && matched) live.stop();
    res.writeHead(matched ? 204 : 404).end();
    return;
  }
  // A tile's picture of a window: what it shows now, read only (a minimized or hidden window has none, never shown for it).
  const thumb = /^\/api\/thumb\/(\d{1,10})-(\d{1,19})$/.exec(path);
  if (thumb) {
    const body = await inTurn(`frame ${thumb[1]} ${thumb[2]}`);
    if (body?.length) reply(res, 200, 'image/jpeg', body);
    else res.writeHead(body ? 204 : 503).end();
    return;
  }
  if (path === '/api/frame/demo') {
    reply(res, 200, 'image/jpeg', sampleFrame());
    return;
  }
  res.writeHead(404).end();
}

export async function start() {
  const server = createServer((req, res) => {
    handle(req, res).catch(() => {
      if (!res.headersSent) res.writeHead(500);
      res.end();
    });
  });
  await new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', resolve);
  });
  const address = `http://127.0.0.1:${server.address().port}`;
  return { address, key, close() {
    server.close();
    server.closeAllConnections();
    live.close();
    capture.stop();
  } };
}
