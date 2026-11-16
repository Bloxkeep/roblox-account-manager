import { spawn } from 'node:child_process';
import { existsSync } from 'node:fs';

const maxReplySize = 4 * 1024 * 1024;

export function createCapture(executable) {
  let worker;
  let pending;
  let bytes = Buffer.alloc(0);

  function settle(body) {
    const reply = pending;
    if (!reply) return;
    pending = undefined;
    clearTimeout(reply.timer);
    reply.resolve(body);
  }

  function stop() {
    settle(null);
    bytes = Buffer.alloc(0);
    // EOF lets a capture in progress put a minimized window back before exiting.
    worker?.stdin.end();
  }

  function start() {
    const child = spawn(executable, ['--core-monitor-frames'], {
      windowsHide: true,
      stdio: ['pipe', 'pipe', 'ignore'],
    });
    worker = child;
    const reset = () => {
      if (worker !== child) return;
      worker = undefined;
      bytes = Buffer.alloc(0);
      settle(null);
    };
    child.on('error', reset);
    child.on('close', reset);
    child.stdin.on('error', () => { if (worker === child) stop(); });
    child.stdout.on('data', (chunk) => {
      if (worker !== child || !pending) return;
      bytes = Buffer.concat([bytes, chunk]);
      if (bytes.length < 4) return;
      const size = bytes.readUInt32LE(0);
      if (size > maxReplySize || bytes.length > size + 4) {
        stop();
        return;
      }
      if (bytes.length === size + 4) {
        const body = bytes.subarray(4);
        bytes = Buffer.alloc(0);
        settle(body);
      }
    });
  }

  function call(command) {
    if (pending || worker?.stdin.writableEnded || !existsSync(executable)) return Promise.resolve(null);
    if (!worker) start();
    return new Promise((resolve) => {
      const timeout = command.startsWith('frame-slow ') ? 23_000 : 6500;
      pending = { resolve, timer: setTimeout(stop, timeout) };
      worker.stdin.write(`${command}\n`);
    });
  }

  return { call, stop, get installed() { return existsSync(executable); } };
}
