import { spawn, spawnSync } from 'node:child_process';
import { once } from 'node:events';
import { createInterface } from 'node:readline';
import { setTimeout as wait } from 'node:timers/promises';
import { fileURLToPath } from 'node:url';

const abort = new AbortController();
const { signal } = abort;
const stop = () => abort.abort();
process.stdin.once('end', stop);
process.stdin.once('error', stop);
process.stdin.resume();

async function reachable(origin, key) {
  const url = new URL('/api/frame/demo', origin);
  const options = { signal: AbortSignal.any([signal, AbortSignal.timeout(5000)]), redirect: 'error' };
  const denied = await fetch(url, options);
  await denied.body?.cancel();
  if (denied.status !== 401) return false;
  const frame = await fetch(url, { ...options, headers: { Authorization: `Bearer ${key}` } });
  return frame.ok && frame.headers.get('content-type') === 'image/jpeg' && (await frame.arrayBuffer()).byteLength > 0;
}

let origin;
let tunnel;
let expiration;
const startup = setTimeout(() => abort.abort(new Error('Tunnel timeout')), 75_000).unref();
try {
  const probe = spawnSync(process.env.BK_MONITOR_FFMPEG, [
    '-hide_banner', '-loglevel', 'error', '-f', 'lavfi', '-i', 'color=size=128x128',
    '-frames:v', '1', '-c:v', 'h264_nvenc', '-f', 'null', '-',
  ], { windowsHide: true, stdio: 'ignore', timeout: 5000 });
  process.env.BK_MONITOR_NVENC = probe.status === 0 ? '1' : '0';
  origin = await (await import('./server.mjs')).start();
  signal.throwIfAborted();
  tunnel = spawn(process.env.BK_MONITOR_TUNNEL, [
    'tunnel', '--config', fileURLToPath(new URL('./remote-tunnel.yml', import.meta.url)),
    '--no-autoupdate', '--url', origin.address,
  ], { windowsHide: true, stdio: ['ignore', 'ignore', 'pipe'] });
  tunnel.on('error', (error) => abort.abort(error));
  tunnel.on('exit', () => abort.abort(new Error('Tunnel stopped')));
  const lines = createInterface({ input: tunnel.stderr });
  signal.addEventListener('abort', () => lines.close(), { once: true });
  let address;
  for await (const line of lines) {
    const match = /https:\/\/[a-z0-9-]+\.trycloudflare\.com(?![.a-z0-9-])/.exec(line);
    if (match) {
      address = match[0];
      break;
    }
  }
  tunnel.stderr.resume();
  if (!address) throw new Error('Tunnel did not return a URL');
  while (!(await reachable(address, origin.key).catch(() => false))) {
    await wait(1250, undefined, { signal });
  }
  clearTimeout(startup);
  console.log(`READY ${address}/#${origin.key}`);
  expiration = setTimeout(stop, 120 * 60_000).unref();
  if (!signal.aborted) await once(signal, 'abort');
  if (signal.reason?.name !== 'AbortError') throw signal.reason;
} catch (error) {
  if (error?.name !== 'AbortError' && signal.reason?.name !== 'AbortError') {
    console.log('FAILED');
    process.exitCode = 1;
  }
} finally {
  clearTimeout(startup);
  clearTimeout(expiration);
  process.stdin.pause();
  tunnel?.kill();
  origin?.close();
}
