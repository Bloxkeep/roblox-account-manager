// Bloxkeep's Discord bot, online around the clock (the green dot) without a server of its own: a Durable Object holds the
// bot's Gateway connection on Cloudflare's free plan. One object, awake all day, is about 11,000 of the 13,000 GB-s a day
// the plan includes. Heartbeats run on the object's alarm rather than a timer, so they survive Cloudflare restarting it;
// after a restart the next alarm, or the minute's cron at the latest, connects again and resumes the session. Every
// connect, close and restart is logged, and /status says how long the bot has been online, to prove it holds up.
//
// The same object reads the news every minute (sources.js): a Roblox build coming (from the Roblox watch, before release)
// or live, Roblox having problems or working again, a new public Bloxkeep version. Each is posted once to the channels
// servers chose with /notify, and every board (/board) is edited when what it shows changes. Slash commands come over
// HTTP (/interactions, signed by Discord), so they work even while the object reconnects.
import { DurableObject } from 'cloudflare:workers';
import { COMMANDS, api, inviteUrl, verify } from './discord.js';
import { newer, readLatestTag, readRelease, readStatus, readWatch, release } from './sources.js';
import { boardEmbed, buildEmbed, downloadMessage, helpMessage, lang, multiEmbed, outageEmbed, releaseMessage, replies, statusEmbed } from './texts.js';

const GATEWAY = 'wss://gateway.discord.gg';
const QUERY = '/?v=10&encoding=json';
// Closes after which connecting again can't help (a wrong token, intents not allowed): the bot stops and says why.
const FATAL = { 4004: 'token refused', 4010: 'bad shard', 4011: 'sharding required', 4012: 'bad API version', 4013: 'bad intents', 4014: 'intents not allowed' };
// Closes that end the session: the next connection identifies again instead of resuming.
const SESSION_GONE = new Set([4007, 4009]);
const MAX_EVENTS = 2000;
const MAX_BACKOFF = 60_000;
// Bloxkeep's releases are looked at every ten minutes (the others every minute).
const RELEASE_EVERY = 10 * 60_000;
// An outage counts once two looks in a row say so, and is over after two that say all is well (no posts for a blip).
const OUTAGE_LOOKS = 2;

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
      // Where each server wants its news, and which kinds; its language for the posts.
      ctx.storage.sql.exec(`CREATE TABLE IF NOT EXISTS subs (guild TEXT PRIMARY KEY, channel TEXT NOT NULL, role TEXT,
        roblox INTEGER NOT NULL, outages INTEGER NOT NULL, bloxkeep INTEGER NOT NULL, locale TEXT)`);
      // Each server's board: the message edited in place, and a fingerprint of what it shows now.
      ctx.storage.sql.exec(`CREATE TABLE IF NOT EXISTS boards (guild TEXT PRIMARY KEY, channel TEXT NOT NULL, message TEXT NOT NULL,
        locale TEXT, shown TEXT)`);
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
    await this.registerCommands();
    await this.poll();
  }

  // ───────────────────────────── commands ─────────────────────────────

  /** The slash commands, put in place with Discord again whenever their list (discord.js) changes. */
  async registerCommands() {
    if (!this.env.DISCORD_APP_ID)
      return;
    const want = await tokenHash(JSON.stringify(COMMANDS));
    if ((await this.ctx.storage.get('commands')) === want)
      return;
    const res = await api(this.env, 'PUT', `/applications/${this.env.DISCORD_APP_ID}/commands`, COMMANDS);
    if (res.ok)
      await this.ctx.storage.put('commands', want);
    this.log('commands', res.ok ? 'registered' : `HTTP ${res.status}`);
  }

  /** A slash command (worker.js checked Discord's signature): the reply Discord shows. */
  async interaction(i) {
    const l = lang(i.locale ?? i.guild_locale), r = replies(l), name = i.data?.name;
    const reply = (data, mine = false) => ({ type: 4, data: { ...data, allowed_mentions: { parse: [] }, ...(mine ? { flags: 64 } : {}) } });
    const snap = await this.snapshot();
    switch (name) {
      case 'help':
        return reply(helpMessage(l, inviteUrl(this.env)));
      case 'download':
        return reply(downloadMessage(l, snap.bloxkeep?.version, inviteUrl(this.env)));
      case 'multi':
        return reply({ embeds: [multiEmbed(l, snap.watch)] });
      case 'robloxstatus':
        return reply({ embeds: [statusEmbed(l, snap.status ?? { code: 100, parts: [], incident: null })] });
    }
    if (!i.guild_id)
      return reply({ content: r.guildOnly }, true);
    // Discord shows these to people who can manage the server only; checked again here all the same.
    if (!(BigInt(i.member?.permissions ?? 0) & (1n << 5n)))
      return reply({ content: r.noRight }, true);
    const sub = i.data.options?.[0], opt = n => sub?.options?.find(o => o.name === n)?.value;
    const sql = this.ctx.storage.sql, guild = i.guild_id, locale = i.guild_locale ?? i.locale ?? 'en-US';
    if (name === 'notify' && sub?.name === 'set') {
      const channel = opt('channel'), role = opt('role') ?? null;
      const kinds = { roblox: opt('roblox') ?? true, outages: opt('outages') ?? true, bloxkeep: opt('bloxkeep') ?? true };
      if (!kinds.roblox && !kinds.outages && !kinds.bloxkeep)
        return reply({ content: r.noKinds }, true);
      // A first post there says what will come, and proves the bot may post in that channel.
      const hello = await api(this.env, 'POST', `/channels/${channel}/messages`, { content: replies(lang(locale)).hello, allowed_mentions: { parse: [] } });
      if (!hello.ok)
        return reply({ content: r.cantPost(channel) }, true);
      sql.exec('INSERT OR REPLACE INTO subs (guild, channel, role, roblox, outages, bloxkeep, locale) VALUES (?, ?, ?, ?, ?, ?, ?)',
        guild, channel, role, +kinds.roblox, +kinds.outages, +kinds.bloxkeep, locale);
      const muted = role && i.data.resolved?.roles?.[role]?.mentionable === false;
      const list = Object.entries(kinds).filter(([, on]) => on).map(([k]) => r.kinds[k]).join(', ');
      return reply({ content: r.notifyOn(channel, role, list) + (muted ? r.roleMuted(role) : '') }, true);
    }
    if (name === 'notify' && sub?.name === 'off') {
      sql.exec('DELETE FROM subs WHERE guild = ?', guild);
      return reply({ content: r.notifyOff }, true);
    }
    if (name === 'notify' && sub?.name === 'show') {
      const s = [...sql.exec('SELECT * FROM subs WHERE guild = ?', guild)][0];
      if (!s)
        return reply({ content: r.notifyNone }, true);
      const list = ['roblox', 'outages', 'bloxkeep'].filter(k => s[k]).map(k => r.kinds[k]).join(', ');
      return reply({ content: r.notifyOn(s.channel, s.role, list) }, true);
    }
    if (name === 'board' && sub?.name === 'set') {
      const channel = opt('channel'), board = { embeds: [boardEmbed(lang(locale), snap)], allowed_mentions: { parse: [] } };
      const res = await api(this.env, 'POST', `/channels/${channel}/messages`, board);
      if (!res.ok)
        return reply({ content: r.cantPost(channel) }, true);
      const message = (await res.json()).id;
      sql.exec('INSERT OR REPLACE INTO boards (guild, channel, message, locale, shown) VALUES (?, ?, ?, ?, ?)',
        guild, channel, message, locale, await tokenHash(JSON.stringify(boardShown(lang(locale), snap))));
      return reply({ content: r.boardOn(channel) }, true);
    }
    if (name === 'board' && sub?.name === 'off') {
      sql.exec('DELETE FROM boards WHERE guild = ?', guild);
      return reply({ content: r.boardOff }, true);
    }
    return reply({ content: '?' }, true);
  }

  // ───────────────────────────── news ─────────────────────────────

  /** What the sources said last; read now if never yet (a first command before the first minute's look). */
  async snapshot() {
    return (await this.ctx.storage.get('snap')) ?? await this.poll();
  }

  /**
   * Every minute: the sources read again, each change posted once to the servers that want that kind of news, and the
   * boards edited where what they show has changed. The first look only takes note (nothing old is announced).
   */
  async poll() {
    const prev = (await this.ctx.storage.get('snap')) ?? null, now = Date.now();
    const snap = prev ? structuredClone(prev) : { announced: {}, outage: { down: false, bad: 0, good: 0, since: null } };
    const releaseDue = !prev || now - (prev.releaseAt ?? 0) >= (Number(this.env.RELEASE_EVERY_MS) || RELEASE_EVERY);
    const [watch, status, tag] = await Promise.all([
      readWatch(this.env).catch(() => null),
      readStatus(this.env).catch(() => null),
      releaseDue ? readLatestTag(this.env).catch(() => null) : Promise.resolve(null),
    ]);
    const posts = [];
    let changed = false;
    if (watch) {
      changed ||= JSON.stringify(watch) !== JSON.stringify(prev?.watch);
      snap.watch = watch;
      const a = snap.announced;
      // A build coming, once the watch has checked it; a build gone live. A new weekly release always, a build within
      // the same release (a fix Roblox pushes) only when it changes something for Multi-Roblox.
      const worth = (build, before) => !before || release(build.version) !== release(before) || (build.verdict && build.verdict !== 'same');
      if (watch.next?.verdict && a.coming !== watch.next.version && (!a.live || newer(watch.next.version, a.live))) {
        if (prev && worth(watch.next, a.coming ?? a.live))
          posts.push({ kind: 'roblox', make: l => ({ embeds: [buildEmbed(l, 'coming', watch.next)] }) });
        a.coming = watch.next.version;
      }
      if (watch.live && a.live !== watch.live.version && (!a.live || newer(watch.live.version, a.live))) {
        if (prev && worth(watch.live, a.live))
          posts.push({ kind: 'roblox', make: l => ({ embeds: [buildEmbed(l, 'live', watch.live)] }) });
        a.live = watch.live.version;
      }
    }
    if (status) {
      changed ||= JSON.stringify(status) !== JSON.stringify(prev?.status);
      snap.status = status;
      const o = snap.outage, bad = status.code >= 300;
      o.bad = bad ? o.bad + 1 : 0;
      o.good = bad ? 0 : o.good + 1;
      if (!o.down && o.bad >= OUTAGE_LOOKS) {
        o.down = true;
        o.since = now;
        if (prev)
          posts.push({ kind: 'outages', make: l => ({ embeds: [outageEmbed(l, 'down', status)] }) });
      } else if (o.down && o.good >= OUTAGE_LOOKS) {
        const since = o.since;
        o.down = false;
        o.since = null;
        posts.push({ kind: 'outages', make: l => ({ embeds: [outageEmbed(l, 'up', status, since)] }) });
      }
    }
    if (releaseDue)
      snap.releaseAt = now;
    if (tag && tag !== snap.announced.release && !/-/.test(tag)) {
      const rel = await readRelease(this.env, tag);
      changed = true;
      snap.bloxkeep = { version: rel.version, url: rel.url };
      if (prev && (!snap.announced.release || newer(tag, snap.announced.release)))
        posts.push({ kind: 'bloxkeep', make: l => releaseMessage(l, rel) });
      snap.announced.release = tag;
    }
    if (changed || !snap.changedAt)
      snap.changedAt = now;
    await this.ctx.storage.put('snap', snap);
    for (const p of posts)
      await this.broadcast(p.kind, p.make);
    await this.updateBoards(snap);
    return snap;
  }

  /** One post to every server that wants this kind of news, in its language, pinging its role if it chose one. */
  async broadcast(kind, make) {
    if (!this.env.DISCORD_TOKEN)
      return;
    const sql = this.ctx.storage.sql;
    for (const s of [...sql.exec(`SELECT * FROM subs WHERE ${kind} = 1`)]) {
      const body = make(lang(s.locale));
      const res = await api(this.env, 'POST', `/channels/${s.channel}/messages`, {
        ...body,
        ...(s.role ? { content: `<@&${s.role}>`, allowed_mentions: { roles: [s.role] } } : { allowed_mentions: { parse: [] } }),
      });
      if (res.status === 404) // the channel is gone: so is the subscription
        sql.exec('DELETE FROM subs WHERE guild = ?', s.guild);
      this.log('posted', `${kind} ${s.guild} ${res.status}`);
    }
  }

  /** Each board edited when what it shows has changed; one whose message was deleted is forgotten. */
  async updateBoards(snap) {
    if (!this.env.DISCORD_TOKEN)
      return;
    const sql = this.ctx.storage.sql;
    for (const b of [...sql.exec('SELECT * FROM boards')]) {
      const l = lang(b.locale), shown = await tokenHash(JSON.stringify(boardShown(l, snap)));
      if (shown === b.shown)
        continue;
      const res = await api(this.env, 'PATCH', `/channels/${b.channel}/messages/${b.message}`, { embeds: [boardEmbed(l, snap)] });
      if (res.ok)
        sql.exec('UPDATE boards SET shown = ? WHERE guild = ?', shown, b.guild);
      else if (res.status === 404)
        sql.exec('DELETE FROM boards WHERE guild = ?', b.guild);
    }
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

/** What a board shows, without its timestamp: the board is edited only when this changes. */
function boardShown(l, snap) {
  const { timestamp, ...rest } = boardEmbed(l, snap);
  return rest.fields;
}

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    const gateway = env.GATEWAY.get(env.GATEWAY.idFromName('bot'));
    if (url.pathname === '/status')
      return Response.json(await gateway.status(), { headers: { 'Cache-Control': 'no-store' } });
    if (url.pathname === '/interactions' && request.method === 'POST') {
      const body = await request.text();
      if (!await verify(env.DISCORD_PUBLIC_KEY, request.headers.get('X-Signature-Ed25519'), request.headers.get('X-Signature-Timestamp'), body))
        return new Response('bad signature', { status: 401 });
      const i = JSON.parse(body);
      if (i.type === 1) // Discord checking the address
        return Response.json({ type: 1 });
      if (i.type === 2)
        return Response.json(await gateway.interaction(i));
      return Response.json({ error: 'unsupported' }, { status: 400 });
    }
    return Response.json({ error: 'not found' }, { status: 404 });
  },

  async scheduled(event, env, ctx) {
    const gateway = env.GATEWAY.get(env.GATEWAY.idFromName('bot'));
    ctx.waitUntil(gateway.ensure());
  },
};
