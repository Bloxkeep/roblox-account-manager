import { createConnection } from 'node:net';

/** Names come from the running, unlocked app. Untracked windows remain unnamed. */
export function loadRoster(key, connect = createConnection) {
  if (!key) return Promise.resolve({ state: 'unavailable' });
  return new Promise((resolve) => {
    const socket = connect({ host: '127.0.0.1', port: Number(process.env.BK_MONITOR_ROSTER_PORT) }, () => socket.write(key + '\n'));
    let bytes = Buffer.alloc(0);
    const finish = (roster) => {
      socket.destroy();
      resolve(roster ?? { state: 'unavailable' });
    };
    socket.setTimeout(1400, finish);
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
