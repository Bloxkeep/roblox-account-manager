import { createConnection } from 'node:net';

/** Names come from the running, unlocked app. Untracked windows remain unnamed. */
export function loadRoster(key, connect = createConnection) {
  return ask(key, 'roster', 1400, connect).then((roster) => roster ?? { state: 'unavailable' });
}

/**
 * A command for the app (Rejoin, Close, a preset…): what it did, or { code: 'working' } when it takes longer (its
 * outcome then comes with the roster's `done`).
 */
export function sendCommand(key, order, connect = createConnection) {
  const line = 'do ' + JSON.stringify(order);
  if (Buffer.byteLength(line) > 150) return Promise.resolve({ code: 'bad' });
  return ask(key, line, 4000, connect).then((answer) => answer ?? { code: 'unavailable' });
}

// One line asked of the app after the key; one JSON answer back, its length first (4 bytes).
function ask(key, line, wait, connect) {
  if (!key) return Promise.resolve(undefined);
  return new Promise((resolve) => {
    const socket = connect({ host: '127.0.0.1', port: Number(process.env.BK_MONITOR_ROSTER_PORT) }, () => socket.write(key + '\n' + line + '\n'));
    let bytes = Buffer.alloc(0);
    const finish = (answer) => {
      socket.destroy();
      resolve(answer);
    };
    socket.setTimeout(wait, () => finish());
    socket.on('error', () => finish());
    socket.on('close', () => finish());
    socket.on('data', (chunk) => {
      if (bytes.length + chunk.length > 128 * 1024) return finish();
      bytes = Buffer.concat([bytes, chunk]);
      if (bytes.length < 4) return;
      const total = 4 + bytes.readUInt32LE();
      if (bytes.length < total) return;
      if (bytes.length !== total) return finish();
      try {
        finish(JSON.parse(bytes.subarray(4)));
      } catch {
        finish();
      }
    });
  });
}

export function nameWindows(windows, roster) {
  return windows.map(({ id }) => {
    const label = roster.state === 'ready' && roster.labels?.[id];
    return { id, ...(typeof label === 'string' && label.trim() ? { label } : {}) };
  });
}
