import { spawn } from 'node:child_process';
import { randomBytes } from 'node:crypto';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { createConnection } from 'node:net';
import { tmpdir } from 'node:os';
import { basename, dirname, join, resolve } from 'node:path';
import { setTimeout as sleep } from 'node:timers/promises';
const ffmpeg = process.env.BK_MONITOR_FFMPEG;
const fullHd = process.env.BK_MONITOR_COMPAT !== '1';
const frameRate = fullHd ? 30 : 18;
const width = fullHd ? 1920 : 960;
const height = fullHd ? 1080 : 540;
const sourceCodec = fullHd ? 'bmp' : 'mjpeg';
const maxBitrate = fullHd ? '6000k' : '2200k';
const bufferSize = fullHd ? '6000k' : '1100k';
const segmentSeconds = fullHd ? 1 : 0.5;
const nvenc = process.env.BK_MONITOR_NVENC === '1';
const captureOnGpu = fullHd;
const encoding = nvenc
  ? ['-c:v', 'h264_nvenc', '-preset', 'p4', '-tune', 'll', '-zerolatency', '1',
    '-rc', 'vbr', '-cq', '19', '-b:v', '0', '-bf', '0', '-no-scenecut', '1', '-forced-idr', '1']
  : ['-c:v', 'libx264', '-preset', 'ultrafast', '-tune', 'zerolatency',
    '-sc_threshold', '0', '-crf', fullHd ? '19' : '21'];
const idleMs = 15_000;
const limit = (fullHd ? 64 : 4) * 1024 * 1024;

async function removeMedia(folder) {
  const target = resolve(folder);
  if (dirname(target) !== resolve(tmpdir()) || !basename(target).startsWith('bloxkeep-live-')) {
    throw new Error('Unexpected live segment directory.');
  }
  await rm(target, { recursive: true, force: true, maxRetries: 3, retryDelay: 120 });
}

async function attach(id, key) {
  const socket = createConnection({ host: '127.0.0.1', port: Number(process.env.BK_MONITOR_VIDEO_PORT) });
  let receive;
  let closed = false;
  const header = Buffer.alloc(4);
  let headerBytes = 0;
  let frame;
  let frameBytes = 0;
  function finish() {
    closed = true;
    receive?.(null);
    receive = undefined;
  }
  socket.setNoDelay();
  socket.on('error', finish);
  socket.on('close', finish);
  socket.setTimeout(14_000, () => socket.destroy());
  socket.on('data', (chunk) => {
    if (!receive) return;
    if (!frame) {
      const read = chunk.copy(header, headerBytes);
      headerBytes += read;
      if (headerBytes < header.length) return;
      const size = header.readUInt32LE();
      if (size > limit) {
        socket.destroy();
        return;
      }
      frame = Buffer.allocUnsafe(size);
      chunk = chunk.subarray(read);
    }
    if (chunk.length > frame.length - frameBytes) {
      socket.destroy();
      return;
    }
    frameBytes += chunk.copy(frame, frameBytes);
    if (frameBytes === frame.length) {
      const resolve = receive;
      const result = frame;
      receive = frame = undefined;
      headerBytes = frameBytes = 0;
      resolve(result);
    }
  });
  const ask = (line) => new Promise((resolve) => {
    if (closed || receive) {
      resolve(null);
      return;
    }
    receive = resolve;
    socket.write(line);
  });
  try {
    const mode = captureOnGpu ? ' direct' : '';
    const answer = await ask(`${key}\n${id.replace('-', ' ')}${mode}\n`);
    const result = answer?.toString();
    const window = captureOnGpu ? /^ok ([1-9]\d*)$/.exec(result ?? '')?.[1] : undefined;
    if (captureOnGpu ? !window : result !== 'ok') {
      const messages = { busy: 'Bloxkeep đang thao tác với cửa sổ. Thử lại sau.',
        locked: 'Mở khóa Bloxkeep trên PC để xem live.', window: 'Cửa sổ Roblox không còn mở.' };
      const code = Object.hasOwn(messages, result) ? result : 'bridgeUnavailable';
      throw Object.assign(new Error(messages[code] ?? 'Không kết nối được với Bloxkeep.'), { code });
    }
    return { window, capture: () => ask(window ? 'check\n' : fullHd ? 'bmp\n' : 'frame\n'), close: () => socket.end() };
  } catch (error) {
    socket.destroy();
    throw error;
  }
}

export function createLive(key, connect = attach) {
  let current;
  let starting = false;
  let stopping = false;
  let closed = false;

  async function start(source) {
    if (closed || current || starting || stopping) throw Object.assign(new Error('Another live viewer is active.'), { code: 'inUse' });
    starting = true;
    let client;
    let folder;
    try {
      client = await connect(source, key);
      folder = await mkdtemp(join(tmpdir(), 'bloxkeep-live-'));
      if (closed) throw new Error('The remote session ended.');
      const session = { id: randomBytes(24).toString('hex'), client, folder, seen: Date.now(), frame: null, stopped: false };
      const window = client.window;
      const cpuScale = `scale=${width}:${height}:force_original_aspect_ratio=decrease:flags=lanczos,pad=${width}:${height}:(ow-iw)/2:(oh-ih)/2`;
      const gpuScale = `gfxcapture=hwnd=${window}:max_framerate=${frameRate}:width=${width}:height=${height}:resize_mode=scale_aspect:capture_cursor=0:dup_frames=1,fps=${frameRate}`;
      const inputArgs = window
        ? ['-filter_complex', nvenc ? gpuScale : gpuScale + ',hwdownload,format=bgra,format=yuv420p', '-colorspace', 'bt709', '-color_primaries', 'bt709', '-color_trc', 'bt709']
        : ['-f', 'image2pipe', '-framerate', String(frameRate), '-c:v', sourceCodec,
          '-probesize', '16384', '-analyzeduration', '0', '-i', 'pipe:0', '-vf', cpuScale, '-pix_fmt', 'yuv420p'];
      const encoder = spawn(ffmpeg, [
        '-hide_banner', '-loglevel', 'error', ...inputArgs, '-an', ...encoding, '-g', String(frameRate * segmentSeconds),
        '-maxrate', maxBitrate, '-bufsize', bufferSize,
        '-f', 'hls', '-hls_time', String(segmentSeconds), '-hls_list_size', '8', '-hls_segment_type', 'fmp4',
        '-hls_flags', 'delete_segments+independent_segments+temp_file+omit_endlist',
        '-hls_segment_filename', 'chunk-%06d.m4s', 'index.m3u8',
      ], { cwd: folder, windowsHide: true, stdio: ['pipe', 'ignore', 'pipe'] });
      encoder.stderr.on('data', (message) => console.error(`Live encoder: ${String(message).trim()}`));
      session.encoder = encoder;
      current = session;
      encoder.on('error', () => stop(session));
      encoder.on('close', () => stop(session));
      encoder.stdin.on('error', () => stop(session));
      session.clock = setInterval(() => {
        if (Date.now() - session.seen > idleMs) {
          stop(session);
        } else if (!window && session.frame && !encoder.stdin.writableNeedDrain) {
          encoder.stdin.write(session.frame);
        }
      }, 1000 / frameRate);
      if (window) void keepWindow(session);
      else void sample(session);
      return session.id;
    } catch (error) {
      client?.close();
      if (folder) await removeMedia(folder);
      throw error;
    } finally {
      starting = false;
    }
  }

  async function keepWindow(session) {
    try {
      while (!session.stopped && await session.client.capture() !== null) {
        await sleep(200);
      }
    } finally {
      stop(session);
    }
  }

  async function sample(session) {
    try {
      while (!session.stopped) {
        const begin = performance.now();
        const frame = await session.client.capture();
        if (frame === null) break;
        if (frame.length) {
          session.frame = frame;
        }
        await sleep(Math.max(1, 1000 / frameRate - (performance.now() - begin)));
      }
    } catch {
      // A disconnected bridge ends the session like a closed viewer.
    } finally {
      stop(session);
    }
  }

  function stop(session = current) {
    if (!session || session.stopped) return;
    session.stopped = true;
    stopping = true;
    current = undefined;
    clearInterval(session.clock);
    if (!session.client.window) session.client.close();
    session.encoder.stdin.end(session.client.window ? 'q\n' : undefined);
    const fallback = setTimeout(() => session.encoder.kill(), 1800).unref();
    const clean = async () => {
      clearTimeout(fallback);
      if (session.client.window) session.client.close();
      try {
        await removeMedia(session.folder).catch((error) => console.error(error.message));
      } finally {
        stopping = false;
      }
    };
    if (session.encoder.exitCode !== null || session.encoder.signalCode !== null) void clean();
    else session.encoder.once('close', () => void clean());
  }

  function heartbeat(id) {
    if (!current || current.id !== id) return false;
    current.seen = Date.now();
    return true;
  }

  function media(id, file) {
    if (!current || current.id !== id || !/^(index\.m3u8|init\.mp4|chunk-\d{6}\.m4s)$/.test(file)) return null;
    return readFile(join(current.folder, file)).catch(() => null);
  }

  return { start, stop, media, heartbeat, close() { closed = true; stop(); } };
}
