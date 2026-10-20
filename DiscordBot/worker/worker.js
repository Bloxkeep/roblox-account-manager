// Bloxkeep's Discord bot, online around the clock (the green dot) without a server of its own: a Durable Object holds the
// bot's Gateway connection on Cloudflare's free plan. One object, awake all day, is about 11,000 of the 13,000 GB-s a day
// the plan includes. Heartbeats run on the object's alarm rather than a timer, so they survive Cloudflare restarting it;
// after a restart the next alarm, or the minute's cron at the latest, connects again and resumes the session. Every
// connect, close and restart is logged, and /status says how long the bot has been online, to prove it holds up.
import { DurableObject } from 'cloudflare:workers';

const GATEWAY = 'wss://gateway.discord.gg';
const QUERY = '/?v=10&encoding=json';
// Closes after which connecting again can't help (a wrong token, intents not allowed): the bot stops and says why.
const FATAL = { 4004: 'token refused', 4010: 'bad shard', 4011: 'sharding required', 4012: 'bad API version', 4013: 'bad intents', 4014: 'intents not allowed' };
// Closes that end the session: the next connection identifies again instead of resuming.
const SESSION_GONE = new Set([4007, 4009]);
const MAX_EVENTS = 2000;
const MAX_BACKOFF = 60_000;

export class Gateway extends DurableObject {
  constructor(ctx, env) {
    super(ctx, env);
    this.ws = null;
    this.ready = false;
    this.acked = true;
    this.interval = 41_250;
    this.backoff = 0;
    this.connecting = null;
    ctx.blockConcurrencyWhile(async () => {
      ctx.storage.sql.exec('CREATE TABLE IF NOT EXISTS events (at INTEGER NOT NULL, kind TEXT NOT NULL, detail TEXT)');
      this.session = (await ctx.storage.get('session')) ?? null; // { id, url, seq }
      this.stopped = (await ctx.storage.get('stopped')) ?? null;
      // A new instance: the first one, or Cloudflare restarted the object (the connection went with it). The time of the
      // old one's last heartbeat answer marks when it stopped being online.
      this.log('woke', String((await ctx.storage.get('lastBeat')) ?? ''));
      // Woken by anything (a deploy, a restart, a look at /status): connect in a moment rather than at the next cron.
      if (env.DISCORD_TOKEN && !this.stopped && (await ctx.storage.getAlarm()) == null)
        await ctx.storage.setAlarm(Date.now() + 1000);
    });
  }

  /** From the cron, every minute: connected, or connecting now. Stopped by a refused token, it starts over once the token is another. */
  async ensure() {
    if (!this.env.DISCORD_TOKEN)
      return; // not set yet (the worker's settings): nothing to connect with
    if (this.stopped && this.stopped.token !== await tokenHash(this.env.DISCORD_TOKEN))
      await this.restart();
    if (this.stopped)
      return;
    if (!this.ws)
      await this.connect();
    else if ((await this.ctx.storage.getAlarm()) == null)
      await this.ctx.storage.setAlarm(Date.now() + this.interval);
  }

  /** Heartbeats, and connecting again after a drop (after a pause that grows while it keeps failing). */
  async alarm() {
    if (this.stopped)
      return;
    if (!this.ws) {
      await this.connect();
      return;
    }
    if (!this.acked) {
      // Discord stopped answering heartbeats: the connection is dead even if it looks open.
      this.log('zombie', '');
      this.drop(4000);
      await this.connect();
      return;
    }
    this.acked = false;
    this.send({ op: 1, d: this.session?.seq ?? null });
    await this.ctx.storage.setAlarm(Date.now() + this.interval);
  }

  connect() {
    // One attempt at a time: the cron and the alarm may both ask.
    return this.connecting ??= this.open().finally(() => { this.connecting = null; });
  }

  async open() {
    if (this.stopped || this.ws || !this.env.DISCORD_TOKEN)
      return;
    const resume = !!(this.session?.id && this.session?.url);
    const base = this.env.GATEWAY_URL || (resume ? this.session.url : GATEWAY);
    try {
      const res = await fetch(base.replace(/^ws/, 'http').replace(/\/+$/, '') + QUERY, { headers: { Upgrade: 'websocket' } });
      const ws = res.webSocket;
      if (!ws)
        throw new Error(`HTTP ${res.status}`);
      ws.accept();
      this.ws = ws;
      this.ready = false;
      this.acked = true;
      this.resuming = resume;
      ws.addEventListener('message', e => this.onMessage(ws, e.data));
      ws.addEventListener('close', e => this.onClose(ws, e.code, e.reason));
      ws.addEventListener('error', () => this.onClose(ws, 1006, 'error'));
      this.log('connect', resume ? 'resume' : 'identify');
    } catch (e) {
      this.log('connect failed', String(e?.message ?? e).slice(0, 200));
      await this.retry();
    }
  }

  async onMessage(ws, data) {
    if (ws !== this.ws)
      return;
    let msg;
    try {
      msg = JSON.parse(typeof data === 'string' ? data : new TextDecoder().decode(data));
    } catch {
      return;
    }
    if (msg.s != null && this.session)
      this.session.seq = msg.s;
    switch (msg.op) {
      case 10: // Hello: start heartbeating (the first one after a random part of the interval), then identify or resume.
        this.interval = msg.d?.heartbeat_interval || this.interval;
        if (this.resuming)
          this.send({ op: 6, d: { token: this.env.DISCORD_TOKEN, session_id: this.session.id, seq: this.session.seq ?? null } });
        else
          this.send({ op: 2, d: this.identity() });
        await this.ctx.storage.setAlarm(Date.now() + Math.floor(this.interval * Math.random()));
        break;
      case 11: // Heartbeat answered.
        this.acked = true;
        this.ctx.storage.put('lastBeat', Date.now());
        break;
      case 1: // Discord asks for a heartbeat now.
        this.send({ op: 1, d: this.session?.seq ?? null });
        break;
      case 7: // Discord asks to reconnect (and resume).
        this.log('reconnect asked', '');
        this.drop(4000);
        await this.connect();
        break;
      case 9: // Invalid session: resumable or not, said by d.
        this.log('invalid session', msg.d ? 'resumable' : 'new session');
        if (!msg.d)
          await this.forgetSession();
        this.drop(4000);
        this.backoff = 1000 + Math.floor(Math.random() * 4000); // Discord: wait 1–5 s before identifying again
        await this.ctx.storage.setAlarm(Date.now() + this.backoff);
        break;
      case 0:
        if (msg.t === 'READY') {
          this.session = { id: msg.d.session_id, url: msg.d.resume_gateway_url, seq: msg.s };
          this.online('ready', msg.d.user?.username ?? '');
        } else if (msg.t === 'RESUMED') {
          this.online('resumed', '');
        }
        if (this.session)
          this.ctx.storage.put('session', this.session);
        break;
    }
  }

  online(kind, detail) {
    this.ready = true;
    this.backoff = 0;
    this.log(kind, detail);
  }

  async onClose(ws, code, reason) {
    if (ws !== this.ws)
      return;
    this.ws = null;
    this.ready = false;
    this.log('closed', `${code} ${reason || ''}`.trim());
    if (FATAL[code]) {
      this.stopped = { code, why: FATAL[code], at: Date.now(), token: await tokenHash(this.env.DISCORD_TOKEN) };
      await this.ctx.storage.put('stopped', this.stopped);
      await this.ctx.storage.deleteAlarm();
      this.log('stopped', `${code} ${FATAL[code]}`);
      return;
    }
    if (SESSION_GONE.has(code))
      await this.forgetSession();
    await this.retry();
  }

  /** Connects again after a pause: 1 s, doubling while it keeps failing, at most a minute. */
  async retry() {
    this.backoff = Math.min(this.backoff ? this.backoff * 2 : 1000, MAX_BACKOFF);
    await this.ctx.storage.setAlarm(Date.now() + this.backoff);
  }

  /** Closes the connection on our side; its close event is then ignored (the caller connects again itself). */
  drop(code) {
    const ws = this.ws;
    this.ws = null;
    this.ready = false;
    try {
      ws?.close(code, 'reconnect');
    } catch {
      // Already closed.
    }
  }

  async forgetSession() {
    this.session = null;
    await this.ctx.storage.delete('session');
  }

  identity() {
    return {
      token: this.env.DISCORD_TOKEN,
      intents: 0, // nothing to listen to yet: the bot only has to be there
      properties: { os: 'linux', browser: 'bloxkeep', device: 'bloxkeep' },
      presence: { since: null, activities: [{ name: 'Roblox updates', type: 3 }], status: 'online', afk: false },
    };
  }

  send(payload) {
    try {
      this.ws?.send(JSON.stringify(payload));
    } catch {
      // Closing meanwhile: the close event takes it from here.
    }
  }

  log(kind, detail) {
    const sql = this.ctx.storage.sql;
    sql.exec('INSERT INTO events (at, kind, detail) VALUES (?, ?, ?)', Date.now(), kind, detail ?? '');
    sql.exec('DELETE FROM events WHERE rowid IN (SELECT rowid FROM events ORDER BY at DESC LIMIT -1 OFFSET ?)', MAX_EVENTS);
  }

  /**
   * How the bot is doing, and how long it has been online since it first connected (to judge the green dot over days;
   * the time before, without a token, isn't counted).
   */
  async status() {
    const all = [...this.ctx.storage.sql.exec('SELECT at, kind, detail FROM events ORDER BY at, rowid')];
    const first = all.findIndex(e => e.kind === 'connect');
    const events = first < 0 ? [] : all.slice(first);
    const now = Date.now();
    let onlineMs = 0, since = null, restarts = 0, connects = 0, longestOffMs = 0, offSince = events[0]?.at ?? now;
    const goOffline = at => {
      if (since == null)
        return;
      onlineMs += Math.max(0, at - since);
      since = null;
      offSince = at;
    };
    for (const e of events) {
      if (e.kind === 'ready' || e.kind === 'resumed') {
        if (since == null) {
          longestOffMs = Math.max(longestOffMs, e.at - offSince);
          since = e.at;
        }
      } else if (e.kind === 'woke') {
        restarts++;
        // The old instance was online until its last answered heartbeat.
        const lastBeat = Number(e.detail) || e.at;
        goOffline(since != null ? Math.max(since, Math.min(lastBeat, e.at)) : e.at);
      } else if (e.kind === 'closed' || e.kind === 'zombie' || e.kind === 'stopped') {
        goOffline(e.at);
      } else if (e.kind === 'connect') {
        connects++;
      }
    }
    if (since != null)
      onlineMs += now - since;
    else
      longestOffMs = Math.max(longestOffMs, now - offSince);
    const spanMs = events.length ? now - events[0].at : 0;
    return {
      state: !this.env.DISCORD_TOKEN ? 'no token' : this.stopped ? 'stopped' : this.ready ? 'online' : this.ws ? 'connecting' : 'offline',
      stopped: this.stopped,
      heartbeatMs: this.interval,
      lastBeat: (await this.ctx.storage.get('lastBeat')) ?? null,
      log: {
        from: events[0] ? new Date(events[0].at).toISOString() : null,
        hours: Math.round(spanMs / 36e4) / 10,
        onlinePct: spanMs ? Math.round(onlineMs / spanMs * 10000) / 100 : null,
        longestOfflineSec: Math.round(longestOffMs / 1000),
        restarts, // Cloudflare starting the object again since the first connect
        connects,
      },
      recent: all.slice(-40).map(e => ({ at: new Date(e.at).toISOString(), kind: e.kind, detail: e.kind === 'woke' ? undefined : e.detail || undefined })),
    };
  }

  /** Starts over after a fatal close, a new token set since: the log is kept. */
  async restart() {
    this.stopped = null;
    this.backoff = 0;
    await this.ctx.storage.delete('stopped');
    await this.forgetSession();
    this.drop(1000);
    this.log('restart', 'new token');
  }
}

/** A token's fingerprint, so a stop remembers which token was refused without keeping the token itself. */
async function tokenHash(token) {
  const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(token ?? ''));
  return [...new Uint8Array(digest)].slice(0, 8).map(b => b.toString(16).padStart(2, '0')).join('');
}

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    const gateway = env.GATEWAY.get(env.GATEWAY.idFromName('bot'));
    if (url.pathname === '/status')
      return Response.json(await gateway.status(), { headers: { 'Cache-Control': 'no-store' } });
    return Response.json({ error: 'not found' }, { status: 404 });
  },

  async scheduled(event, env, ctx) {
    const gateway = env.GATEWAY.get(env.GATEWAY.idFromName('bot'));
    ctx.waitUntil(gateway.ensure());
  },
};
