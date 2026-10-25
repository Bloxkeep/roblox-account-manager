// Bloxkeep's Discord bot, online around the clock (the green dot) without a server of its own: a Durable Object holds the
// bot's Gateway connection on Cloudflare's free plan. One object, awake all day, is about 11,000 of the 13,000 GB-s a day
// the plan includes. Heartbeats run on the object's alarm rather than a timer, so they survive Cloudflare restarting it;
// after a restart the next alarm, or the minute's cron at the latest, connects again and resumes the session. Every
// connect, close and restart is logged, and /status says how long the bot has been online, to prove it holds up.
//
// The same object reads the news every minute (sources.js): a Roblox build coming (from the Roblox watch, before release)
// or live, Roblox having problems or working again, a new public Bloxkeep version. Each is posted once to the channels
// servers chose with /notify (a Roblox update as a card, edited once Roblox's release notes are out), and every board
// (/board) is edited when what it shows changes. Slash commands come over
// HTTP (/interactions, signed by Discord), so they work even while the object reconnects; so do the Verify button and
// its form (/verify: a captcha drawn by captcha.js, solved in Discord, for a role). It also counts how many people have
// Bloxkeep open right now (online.js): the app says so to /online/ping while it's open.
import { DurableObject } from 'cloudflare:workers';
import { onceMore } from './calls.js';
import { CODE_LENGTH, captcha } from './captcha.js';
import { COMMANDS, api, inviteUrl, verify } from './discord.js';
import { DAYS_SHOWN, GRACE, GRACE_SLACK_SECONDS, beatEvery, scramble, dashboardKey, dashboardPage, dayOf, onlineMessage, readBeat } from './online.js';
import { setupServer } from './setup.js';
import { newer, readInstallDownloads, readLatestTag, readNotes, readRelease, readStatus, readWatch, release } from './sources.js';
import {
  COUNT_CHANNEL, SITE, addressed, boardEmbed, buildMessage, captchaMessage, codeForm, countChannelName, downloadMessage, helpMessage, lang,
  multiEmbed, outageEmbed, presenceLines, releaseMessage, replies, statusEmbed, verifyPanel,
} from './texts.js';

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
// How many looks a new release waits for its notes to be readable before it is posted without them (an hour).
const RELEASE_TRIES = 6;
// An outage counts once two looks in a row say so, and is over after two that say all is well (no posts for a blip).
const OUTAGE_LOOKS = 2;
// A Roblox update posted before Roblox's notes were out: they're looked for this often, for this long, and edited in.
const NOTES_EVERY = 10 * 60_000, NOTES_FOR = 14 * 24 * 3600_000;
// The community server's channel that shows how many have Bloxkeep open: renamed at most this often (Discord allows two
// renames of a channel in ten minutes), and looked for this often while there is none (made by hand, say).
const COUNT_EVERY = 6 * 60_000, COUNT_LOOK = 10 * 60_000;
// The bot's object, kept in eastern North America, next to Discord: slash commands come from Discord's servers there,
// and the posts go back to them. The first object ("bot") was made where the first look at /status came from (Asia),
// which put about 200 ms of ocean on every command; this one takes over its servers' choices and boards (migrate()).
const OBJECT = 'bot-na', OLD_OBJECT = 'bot', PLACE = 'enam';
// The worker's own address: the dashboard's link (/online) points here.
const PUBLIC_URL = 'https://bloxkeep-bot.bloxkeep-app.workers.dev';
// The custom status under the bot's name changes this often, through what's true right now (texts.js presenceLines).
const PRESENCE_EVERY = 20_000;
// What the bot hears from Discord: messages in servers (who posted where, not what: no privileged intent), for the trap.
// A session made with other intents is not resumed but identified again.
const INTENTS = 1 << 9;
// A captcha is good for this long, and once: a wrong answer means a new picture. Pictures a person may ask for, per window.
const CAPTCHA_FOR = 5 * 60_000, CAPTCHAS_PER = 5, CAPTCHA_WINDOW = 10 * 60_000;
// The community server's invite (/discord) is asked Discord whether it still works at most this often.
const INVITE_RECHECK = 60_000;
// The README's downloads badge: GitHub is asked for the count at most once an hour, and 5 minutes after it said no.
const DOWNLOADS_EVERY = 60 * 60_000, DOWNLOADS_RETRY = 5 * 60_000;
// Who Bloxkeep's developers are (the application's owner, or its team's owner and admins) is asked Discord this often.
const DEVELOPERS_RECHECK = 10 * 60_000;

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
      // Roblox updates posted before Roblox's notes were out, to edit them in: which release, which post, as it was made.
      ctx.storage.sql.exec(`CREATE TABLE IF NOT EXISTS waiting (release INTEGER NOT NULL, kind TEXT NOT NULL, build TEXT NOT NULL,
        channel TEXT NOT NULL, message TEXT NOT NULL, locale TEXT, role TEXT, at INTEGER NOT NULL)`);
      // Each server's trap channel (/setup): who posts there is banned, unless an owner or one of these roles.
      ctx.storage.sql.exec('CREATE TABLE IF NOT EXISTS traps (guild TEXT PRIMARY KEY, channel TEXT NOT NULL, exempt TEXT NOT NULL, owner TEXT, log TEXT)');
      this.traps = new Map([...ctx.storage.sql.exec('SELECT * FROM traps')].map(t => [t.guild, { ...t, exempt: JSON.parse(t.exempt) }]));
      // Each server's verification (/verify): the role whoever solves the captcha gets, and the button's message.
      ctx.storage.sql.exec('CREATE TABLE IF NOT EXISTS verify (guild TEXT PRIMARY KEY, channel TEXT NOT NULL, role TEXT NOT NULL, message TEXT)');
      // The captchas waiting for an answer, and when each person asked for theirs (kept in memory: after a restart,
      // pressing Verify again is all it takes).
      this.captchas = new Map();
      this.asked = new Map();
      // The Bloxkeep apps open now (online.js): each one's id, until when it counts as online (its next beat is due well
      // before), its version and language. Kept on disk only so that a restart doesn't make everyone vanish until their
      // next beat; a row goes once it's stale.
      ctx.storage.sql.exec('CREATE TABLE IF NOT EXISTS online (id TEXT PRIMARY KEY, until INTEGER NOT NULL, app TEXT NOT NULL, lang TEXT NOT NULL)');
      // The most online at once on each day (Vietnam time), and when.
      ctx.storage.sql.exec('CREATE TABLE IF NOT EXISTS peaks (day TEXT PRIMARY KEY, peak INTEGER NOT NULL, at INTEGER NOT NULL)');
      // How many different PCs had Bloxkeep open on each day (Vietnam time). The day's list holds each one's id scrambled
      // with that day's key (dayKey: random, made for the day), and the list and the key go once the day is over, so
      // nothing leads back to an id or links one day's list to another's: only each day's number stays.
      ctx.storage.sql.exec('CREATE TABLE IF NOT EXISTS seen (day TEXT NOT NULL, key TEXT NOT NULL, PRIMARY KEY (day, key))');
      ctx.storage.sql.exec('CREATE TABLE IF NOT EXISTS people (day TEXT PRIMARY KEY, n INTEGER NOT NULL)');
      this.dayKey = (await ctx.storage.get('dayKey')) ?? null; // { day, key }
      this.here = new Map([...ctx.storage.sql.exec('SELECT * FROM online')].map(r => [r.id, { until: r.until, app: r.app, lang: r.lang }]));
      this.session = (await ctx.storage.get('session')) ?? null; // { id, url, seq }
      this.stopped = (await ctx.storage.get('stopped')) ?? null;
      this.retired = (await ctx.storage.get('retired')) ?? false; // handed over to the object that replaced it
      // A new instance: the first one, or Cloudflare restarted the object (the connection went with it). The time of the
      // old one's last heartbeat answer marks when it stopped being online.
      this.log('woke', String((await ctx.storage.get('lastBeat')) ?? ''));
      // Woken by anything (a deploy, a restart, a look at /status): connect in a moment rather than at the next cron.
      if (env.DISCORD_TOKEN && !this.stopped && !this.retired && (await ctx.storage.getAlarm()) == null)
        await ctx.storage.setAlarm(Date.now() + 1000);
    });
  }

  /** From the cron, every minute: connected, or connecting now. Stopped by a refused token, it starts over once the token is another. */
  async ensure(version) {
    // After a deploy the object goes on with the code it started with for as long as its connection is open (20 minutes
    // and more); the cron, already the new code, says which version that is, and an older object starts over on it.
    const mine = this.env.CF_VERSION?.id;
    if (version && mine && version !== mine) {
      this.ctx.abort('a newer version of the bot was deployed');
      return;
    }
    this.countHere(); // the apps that stopped saying they're open, forgotten
    if (!this.env.DISCORD_TOKEN || this.retired)
      return; // not set yet (the worker's settings): nothing to connect with
    if (this.stopped && this.stopped.token !== await tokenHash(this.env.DISCORD_TOKEN))
      await this.restart();
    if (this.stopped)
      return;
    if (!this.ws)
      await this.connect();
    else if ((await this.ctx.storage.getAlarm()) == null)
      await this.ctx.storage.setAlarm(Date.now() + this.interval);
    // No news posted before the old object handed over its servers' choices (it then posts nothing more).
    if (!await this.migrate())
      return;
    await this.registerCommands();
    await this.poll();
  }

  // ───────────────────────────── moving ─────────────────────────────

  /**
   * Once: the old object's servers' choices, boards, news state and log copied here, and the old one told to stop (it
   * drops its connection and never connects again). Tried again every minute until the old one answers: just after a
   * deploy it may still run code from before this existed. True once done (or when there is nothing to take over).
   */
  async migrate() {
    if (await this.ctx.storage.get('migrated'))
      return true;
    const name = this.env.OLD_OBJECT_NAME ?? OLD_OBJECT;
    const oldId = name ? this.env.GATEWAY.idFromName(name) : null;
    if (oldId && !oldId.equals(this.ctx.id)) {
      let data;
      try {
        data = await this.env.GATEWAY.get(oldId).retire();
      } catch (e) {
        this.log('migrate failed', String(e?.message ?? e).slice(0, 200));
        return false;
      }
      const sql = this.ctx.storage.sql;
      // Choices made here meanwhile win over the old ones.
      for (const s of data.subs)
        sql.exec('INSERT OR IGNORE INTO subs (guild, channel, role, roblox, outages, bloxkeep, locale) VALUES (?, ?, ?, ?, ?, ?, ?)',
          s.guild, s.channel, s.role, s.roblox, s.outages, s.bloxkeep, s.locale);
      for (const b of data.boards)
        sql.exec('INSERT OR IGNORE INTO boards (guild, channel, message, locale, shown) VALUES (?, ?, ?, ?, ?)', b.guild, b.channel, b.message, b.locale, b.shown);
      for (const e of data.events)
        sql.exec('INSERT INTO events (at, kind, detail) VALUES (?, ?, ?)', e.at, e.kind, e.detail);
      if (data.snap && !(await this.ctx.storage.get('snap')))
        await this.ctx.storage.put('snap', data.snap);
      this.log('migrated', `${data.subs.length} subscriptions, ${data.boards.length} boards`);
    }
    await this.ctx.storage.put('migrated', true);
    return true;
  }

  /** The old object's side: everything it holds, handed over, and it stops for good. */
  async retire() {
    this.retired = true;
    await this.ctx.storage.put('retired', true);
    await this.ctx.storage.deleteAlarm();
    this.drop(1000);
    this.log('retired', 'handed over');
    const sql = this.ctx.storage.sql;
    return {
      subs: [...sql.exec('SELECT * FROM subs')],
      boards: [...sql.exec('SELECT * FROM boards')],
      events: [...sql.exec('SELECT at, kind, detail FROM events ORDER BY at, rowid')],
      snap: (await this.ctx.storage.get('snap')) ?? null,
    };
  }

  /**
   * Where the app's Discord button goes (/discord): an invite into Bloxkeep's community server that never expires, made
   * by the bot in the channel newcomers see (the /verify channel). Made again whenever it stops working (someone deleted
   * it, or Discord dropped it); while that can't be done, Bloxkeep's website.
   */
  async invite() {
    const guild = this.env.COMMUNITY_GUILD;
    if (!guild || !this.env.DISCORD_TOKEN)
      return SITE;
    const recheck = Number(this.env.INVITE_RECHECK_MS ?? INVITE_RECHECK);
    const kept = await this.ctx.storage.get('invite'); // { code, checked }
    if (kept && Date.now() - kept.checked < recheck)
      return `https://discord.gg/${kept.code}`;
    if (kept) {
      // Only "unknown invite" means it's gone; Discord being slow or busy doesn't.
      const check = await api(this.env, 'GET', `/invites/${kept.code}`);
      if (check.status !== 404) {
        if (check.ok)
          await this.ctx.storage.put('invite', { code: kept.code, checked: Date.now() });
        return `https://discord.gg/${kept.code}`;
      }
    }
    const channel = [...this.ctx.storage.sql.exec('SELECT channel FROM verify WHERE guild = ?', guild)][0]?.channel;
    if (!channel)
      return SITE;
    const res = await api(this.env, 'POST', `/channels/${channel}/invites`, { max_age: 0, max_uses: 0, unique: true },
      { 'X-Audit-Log-Reason': encodeURIComponent("The invite behind the Bloxkeep app's Discord button") });
    if (!res.ok) {
      this.log('invite failed', `HTTP ${res.status}`);
      return SITE;
    }
    const code = (await res.json()).code;
    await this.ctx.storage.put('invite', { code, checked: Date.now() });
    this.log('invite', kept ? `${kept.code} stopped working, made ${code}` : `made ${code}`);
    return `https://discord.gg/${code}`;
  }

  /**
   * The install files' downloads for the README's badge (null before GitHub first answered). GitHub's API takes 60
   * unsigned calls an hour from one address, and Cloudflare's addresses are shared with everyone else's workers, so it
   * often says no: the last count stays until it answers again, rather than the badge going to "unknown".
   */
  async downloads() {
    const every = Number(this.env.DOWNLOADS_EVERY_MS ?? DOWNLOADS_EVERY);
    const kept = (await this.ctx.storage.get('downloads')) ?? { n: null, next: 0 };
    if (Date.now() < kept.next)
      return kept.n;
    try {
      kept.n = await readInstallDownloads(this.env);
      kept.next = Date.now() + every;
    } catch (e) {
      kept.next = Date.now() + Math.min(every, DOWNLOADS_RETRY);
      this.log('downloads', e.message);
    }
    await this.ctx.storage.put('downloads', kept);
    return kept.n;
  }

  /** How long a slash command took between Cloudflare's edge and this object, and where it entered (for /status). */
  noteInteraction(colo, name, ms) {
    (this.timings ??= []).push({ at: new Date().toISOString(), colo, name, ms });
    this.timings.splice(0, Math.max(0, this.timings.length - 20));
  }

  /** Where this object runs (Cloudflare's code for the place, "IAD", "SIN"…), asked once per instance. */
  async place() {
    if (this.colo === undefined) {
      try {
        const trace = await (await fetch(this.env.TRACE_URL || 'https://www.cloudflare.com/cdn-cgi/trace')).text();
        this.colo = /^colo=(\w+)/m.exec(trace)?.[1] ?? null;
      } catch {
        this.colo = null;
      }
    }
    return this.colo;
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
    // Seen only by who asked (mine): flag 64, besides a card's own.
    const reply = (data, mine = false) => ({ type: 4, data: { ...data, allowed_mentions: { parse: [] }, ...(mine ? { flags: (data.flags ?? 0) | 64 } : {}) } });
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
      case 'online': { // Bloxkeep's developers only, wherever they ask; seen only by them
        if (!await this.isDeveloper(i.member?.user?.id ?? i.user?.id))
          return reply({ content: "Only Bloxkeep's developers can see this." }, true);
        const key = await dashboardKey(this.env.DISCORD_TOKEN);
        return reply(onlineMessage(this.onlineNow(), key ? `${this.env.PUBLIC_URL || PUBLIC_URL}/online?key=${key}` : null), true);
      }
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
    if (name === 'verify' && sub?.name === 'set') {
      const channel = opt('channel'), role = opt('role');
      if (!await mayGive(this.env, i, role))
        return reply({ content: r.verifyNoRight(role) }, true);
      const res = await api(this.env, 'POST', `/channels/${channel}/messages`, verifyPanel(lang(locale)));
      if (!res.ok)
        return reply({ content: r.cantPost(channel) }, true);
      sql.exec('INSERT OR REPLACE INTO verify (guild, channel, role, message) VALUES (?, ?, ?, ?)', guild, channel, role, (await res.json()).id);
      return reply({ content: r.verifyOn(channel, role) }, true);
    }
    if (name === 'verify' && sub?.name === 'off') {
      sql.exec('DELETE FROM verify WHERE guild = ?', guild);
      return reply({ content: r.verifyOff }, true);
    }
    return reply({ content: '?' }, true);
  }

  /**
   * A press on Verify (a new captcha, its picture in `png`), on "Enter the code" (the form), or the code typed in (the
   * role, when right). Everything is seen only by that person; worker.js sends Discord the answer, with the picture.
   */
  async component(i) {
    const l = lang(i.locale ?? i.guild_locale), r = replies(l), id = i.data?.custom_id;
    const mine = data => ({ type: 4, data: { ...data, flags: 64, allowed_mentions: { parse: [] } } });
    // After the form: the captcha's message itself is changed to say how it went (its picture and button gone).
    const instead = content => ({ type: 7, data: { content, embeds: [], components: [], attachments: [] } });
    const v = i.guild_id ? [...this.ctx.storage.sql.exec('SELECT * FROM verify WHERE guild = ?', i.guild_id)][0] : null;
    const user = i.member?.user?.id, key = `${i.guild_id}:${user}`;
    if (!v || !user)
      return { reply: mine({ content: r.verifyNotSet }) };
    if (i.member.roles?.includes(v.role))
      return { reply: id === 'verify-form' ? instead(r.verifyAlready) : mine({ content: r.verifyAlready }) };
    if (id === 'verify') {
      const now = Date.now(), recent = (this.asked.get(key) ?? []).filter(t => now - t < CAPTCHA_WINDOW);
      if (recent.length >= CAPTCHAS_PER)
        return { reply: mine({ content: r.verifySlow }) };
      this.asked.set(key, [...recent, now]);
      const { code, png } = await captcha(this.env.CAPTCHA_CODE);
      this.captchas.set(key, { code, until: now + CAPTCHA_FOR });
      return { reply: mine(captchaMessage(l)), png };
    }
    if (id === 'verify-code')
      return { reply: { type: 9, data: codeForm(l, CODE_LENGTH) } };
    if (id === 'verify-form') {
      const typed = String(i.data.components?.[0]?.components?.[0]?.value ?? '').replace(/\s/g, '').toUpperCase();
      const waiting = this.captchas.get(key);
      this.captchas.delete(key);
      if (!waiting || Date.now() > waiting.until)
        return { reply: instead(r.verifyExpired) };
      if (typed !== waiting.code)
        return { reply: instead(r.verifyWrong) };
      const res = await api(this.env, 'PUT', `/guilds/${i.guild_id}/members/${user}/roles/${v.role}`, undefined,
        { 'X-Audit-Log-Reason': encodeURIComponent('Solved the captcha (/verify)') });
      this.log('verified', `${i.guild_id} ${res.status}`);
      return { reply: instead(res.ok ? r.verifyDone : r.verifyCant(v.role)) };
    }
    return { reply: mine({ content: '?' }) };
  }

  /** Bloxkeep's developers: the application's owner, or its team's owner and admins (asked Discord now and then). */
  async isDeveloper(who) {
    if (!who)
      return false;
    if (!this.developers || Date.now() - this.developers.at > DEVELOPERS_RECHECK) {
      const app = await (await api(this.env, 'GET', '/oauth2/applications/@me')).json().catch(() => ({}));
      const ids = app.team
        ? [app.team.owner_user_id, ...(app.team.members ?? []).filter(m => m.role === 'admin' && m.membership_state === 2).map(m => m.user.id)]
        : [app.owner?.id];
      this.developers = { ids: ids.filter(Boolean), at: Date.now() };
    }
    return this.developers.ids.includes(who);
  }

  // ───────────────────────────── online now ─────────────────────────────

  /**
   * A beat from a Bloxkeep app (online.js readBeat): open now, or closing. The answer says when to send the next: later
   * the more are online, so that all the beats together stay within the bot's daily budget (online.js beatEvery).
   */
  async beat(b) {
    const now = Date.now();
    if (b.gone) {
      if (this.here.delete(b.id))
        this.ctx.storage.sql.exec('DELETE FROM online WHERE id = ?', b.id);
      return {};
    }
    // Counted among the day's people once (a restart forgets that it was: the day's list then says so).
    const counted = this.here.get(b.id)?.counted;
    if (counted !== dayOf(now))
      await this.notePerson(b.id, now);
    this.here.set(b.id, { until: now, app: b.app, lang: b.lang, counted: dayOf(now) });
    const count = this.countHere(now), next = this.beatEvery(count), until = now + this.onlineFor(next);
    this.here.get(b.id).until = until;
    this.ctx.storage.sql.exec(`INSERT INTO online (id, until, app, lang) VALUES (?, ?, ?, ?)
      ON CONFLICT (id) DO UPDATE SET until = excluded.until, app = excluded.app, lang = excluded.lang`, b.id, until, b.app, b.lang);
    this.notePeak(count, now);
    return { next };
  }

  beatEvery(count) {
    return beatEvery(count, Number(this.env.BEAT_BUDGET) || undefined);
  }

  /** One of the day's people: their id scrambled with the day's key, counted if the day's list hasn't it yet. */
  async notePerson(id, now) {
    const day = dayOf(now);
    if (this.dayKey?.day !== day) {
      this.dayKey = { day, key: crypto.randomUUID() + crypto.randomUUID() };
      await this.ctx.storage.put('dayKey', this.dayKey);
      this.ctx.storage.sql.exec('DELETE FROM seen WHERE day < ?', day);
    }
    const key = await scramble(this.dayKey.key, id);
    if (this.ctx.storage.sql.exec('INSERT OR IGNORE INTO seen (day, key) VALUES (?, ?)', day, key).rowsWritten > 0)
      this.ctx.storage.sql.exec('INSERT INTO people (day, n) VALUES (?, 1) ON CONFLICT (day) DO UPDATE SET n = n + 1', day);
  }

  /** How long a beat that says the next comes in `next` seconds keeps its app online (shorter in the checks). */
  onlineFor(next) {
    return Number(this.env.ONLINE_FOR_MS) || (next * GRACE + GRACE_SLACK_SECONDS) * 1000;
  }

  /** How many are online now; the ones that stopped saying so are forgotten on the way. */
  countHere(now = Date.now()) {
    for (const [id, h] of this.here) {
      if (now > h.until) {
        this.here.delete(id);
        this.ctx.storage.sql.exec('DELETE FROM online WHERE id = ?', id);
      }
    }
    return this.here.size;
  }

  /** Today's peak, raised when more are online than ever before today. */
  notePeak(count, now) {
    const day = dayOf(now);
    if (this.peak?.day !== day)
      this.peak = [...this.ctx.storage.sql.exec('SELECT * FROM peaks WHERE day = ?', day)][0] ?? { day, peak: 0, at: now };
    if (count <= this.peak.peak)
      return;
    this.peak = { day, peak: count, at: now };
    this.ctx.storage.sql.exec('INSERT OR REPLACE INTO peaks (day, peak, at) VALUES (?, ?, ?)', day, count, now);
  }

  /** What /online and the dashboard show: online now, by version and language, today's peak and the last days' peaks. */
  onlineNow() {
    const now = Date.now(), count = this.countHere(now);
    const tally = key => [...[...this.here.values()].reduce((m, h) => m.set(h[key], (m.get(h[key]) ?? 0) + 1), new Map())]
      .map(([name, n]) => ({ name, n })).sort((a, b) => b.n - a.n || String(a.name).localeCompare(String(b.name), 'en', { numeric: true }));
    const days = [];
    for (let n = DAYS_SHOWN - 1; n >= 0; n--)
      days.push(dayOf(now - n * 86_400_000));
    const peaks = new Map([...this.ctx.storage.sql.exec('SELECT * FROM peaks WHERE day >= ?', days[0])].map(p => [p.day, p]));
    const people = new Map([...this.ctx.storage.sql.exec('SELECT * FROM people WHERE day >= ?', days[0])].map(p => [p.day, p.n]));
    // People: none counted yet today is 0; an earlier day without a number is one from before they were counted (null).
    const today = days.at(-1);
    const shown = days.map(day => ({ day, peak: peaks.get(day)?.peak ?? 0, at: peaks.get(day)?.at ?? null,
      people: people.get(day) ?? (day === today ? 0 : null) }));
    return {
      now: count, at: new Date(now).toISOString(), forMinutes: Math.round(this.onlineFor(this.beatEvery(count)) / 60_000),
      today: shown.at(-1), days: shown, apps: tally('app'), languages: tally('lang'),
    };
  }

  // ───────────────────────────── the community server ─────────────────────────────

  /** /setup: answered "thinking…" at once (worker.js); the work goes on here and the answer is edited when it's done. */
  startSetup(i) {
    const work = this.runSetup(i).catch(e => this.log('setup failed', String(e?.message ?? e).slice(0, 200)));
    this.ctx.waitUntil?.(work);
  }

  async runSetup(i) {
    const answer = content => api(this.env, 'PATCH', `/webhooks/${i.application_id}/${i.token}/messages/@original`, { content: content.slice(0, 2000), allowed_mentions: { parse: [] } });
    // Only Bloxkeep's developers, whoever else manages the server.
    const who = i.member?.user?.id ?? i.user?.id;
    if (!await this.isDeveloper(who)) {
      await answer("Only Bloxkeep's developers can set up a server with this.");
      return;
    }
    const bot = (await (await api(this.env, 'GET', '/users/@me')).json()).id;
    const result = await setupServer(this.env, i.guild_id, bot, who);
    this.ctx.storage.sql.exec('INSERT OR REPLACE INTO traps (guild, channel, exempt, owner, log) VALUES (?, ?, ?, ?, ?)',
      i.guild_id, result.trap.channel, JSON.stringify(result.trap.exempt), result.trap.owner, result.modLog);
    this.traps.set(i.guild_id, { guild: i.guild_id, channel: result.trap.channel, exempt: result.trap.exempt, owner: result.trap.owner, log: result.modLog });
    if (result.count && i.guild_id === this.env.COMMUNITY_GUILD)
      await this.ctx.storage.put('countChannel', { ...result.count, at: 0, looked: Date.now() });
    // The bot's own news in #roblox-updates, pinging @Roblox updates, and its board there, pinned.
    if (result.updates) {
      const sql = this.ctx.storage.sql;
      sql.exec('INSERT OR REPLACE INTO subs (guild, channel, role, roblox, outages, bloxkeep, locale) VALUES (?, ?, ?, 1, 1, 1, ?)',
        i.guild_id, result.updates, result.pings, 'en-US');
      if (![...sql.exec('SELECT guild FROM boards WHERE guild = ?', i.guild_id)].length) {
        const snap = await this.snapshot(), l = lang('en-US');
        const res = await api(this.env, 'POST', `/channels/${result.updates}/messages`, { embeds: [boardEmbed(l, snap)], allowed_mentions: { parse: [] } });
        if (res.ok) {
          const message = (await res.json()).id;
          sql.exec('INSERT OR REPLACE INTO boards (guild, channel, message, locale, shown) VALUES (?, ?, ?, ?, ?)',
            i.guild_id, result.updates, message, 'en-US', await tokenHash(JSON.stringify(boardShown(l, snap))));
          await api(this.env, 'PUT', `/channels/${result.updates}/pins/${message}`);
          result.made.push('board in #roblox-updates');
        } else {
          result.failed.push(`board: HTTP ${res.status}`);
        }
      }
    }
    this.log('setup', `${result.made.length} made, ${result.failed.length} failed`);
    await answer([
      result.made.length ? `Done. Made: ${result.made.join(', ')}.` : 'Done: everything was already there.',
      ...(result.failed.length ? [`Couldn't: ${result.failed.join('; ')}`] : []),
    ].join('\n'));
  }

  /**
   * A message in a server: posted in that server's trap channel by anyone but its owner, a moderator, an admin or a
   * bot, it gets its sender banned, with their messages from the last hour deleted, and says so in #mod-log.
   */
  async trap(d) {
    const t = d.guild_id ? this.traps.get(d.guild_id) : null;
    if (!t || d.channel_id !== t.channel || d.author?.bot || d.webhook_id)
      return;
    const user = d.author?.id;
    if (!user || user === t.owner || (d.member?.roles ?? []).some(r => t.exempt.includes(r)))
      return;
    const key = `${d.guild_id}/${user}`;
    if ((this.banned ??= new Set()).has(key))
      return; // several messages at once: one ban
    this.banned.add(key);
    setTimeout(() => this.banned.delete(key), 10 * 60_000);
    const res = await api(this.env, 'PUT', `/guilds/${d.guild_id}/bans/${user}`, { delete_message_seconds: 3600 },
      { 'X-Audit-Log-Reason': encodeURIComponent('Posted in #dont-type-here, the trap for scam bots and hacked accounts') });
    this.log('trap', `${d.guild_id} HTTP ${res.status}`);
    if (!t.log)
      return;
    const name = String(d.author?.username ?? '').replace(/[\\*_`~|>]/g, m => `\\${m}`);
    await api(this.env, 'POST', `/channels/${t.log}/messages`, {
      embeds: [{
        title: res.ok ? 'Banned by the trap' : "The trap couldn't ban",
        color: res.ok ? 0xED4245 : 0xFEE75C,
        description: `<@${user}> (${name}) posted in <#${t.channel}>.` + (res.ok
          ? ' Their messages from the last hour were deleted.'
          : ` Discord answered HTTP ${res.status}: the bot needs Ban Members, and its role above the member's roles.`),
        footer: { text: 'Bloxkeep · Trap' }, timestamp: new Date().toISOString(),
      }],
      allowed_mentions: { parse: [] },
    });
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
      // With Roblox's notes when they're out already; else posted without, and the notes edited in later (addNotes).
      const update = async (kind, build) => {
        const notes = await readNotes(this.env, release(build.version)).catch(() => null);
        posts.push({ kind: 'roblox', make: l => buildMessage(l, kind, build, { notes, liveAt: now }), waiting: !notes && { kind, build } });
      };
      if (watch.next?.verdict && a.coming !== watch.next.version && (!a.live || newer(watch.next.version, a.live))) {
        if (prev && worth(watch.next, a.coming ?? a.live))
          await update('coming', watch.next);
        a.coming = watch.next.version;
      }
      if (watch.live && a.live !== watch.live.version && (!a.live || newer(watch.live.version, a.live))) {
        if (prev && worth(watch.live, a.live))
          await update('live', watch.live);
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
      // Its notes not readable yet (GitHub not answering): asked again at the next look, for up to RELEASE_TRIES looks,
      // rather than posted without its highlights.
      const tries = snap.releaseTries?.tag === tag ? snap.releaseTries.n + 1 : 1;
      if (!rel.read && tries < RELEASE_TRIES) {
        snap.releaseTries = { tag, n: tries };
      } else {
        delete snap.releaseTries;
        changed = true;
        snap.bloxkeep = { version: rel.version, url: rel.url };
        if (prev && (!snap.announced.release || newer(tag, snap.announced.release)))
          posts.push({ kind: 'bloxkeep', make: l => releaseMessage(l, rel) });
        snap.announced.release = tag;
      }
    }
    if (changed || !snap.changedAt)
      snap.changedAt = now;
    await this.ctx.storage.put('snap', snap);
    for (const p of posts)
      await this.broadcast(p.kind, p.make, p.waiting);
    await this.addNotes(now);
    await this.updateBoards(snap);
    await this.updateCountChannel(now);
    return snap;
  }

  /**
   * The community server's channel that shows how many have Bloxkeep open now ("🟢 Using Bloxkeep: 3": /setup makes it,
   * or one named so by hand that the bot may manage): renamed when the number changes, at most every six minutes, and
   * looked for every ten minutes while there is none.
   */
  async updateCountChannel(now) {
    if (!this.env.DISCORD_TOKEN || !this.env.COMMUNITY_GUILD)
      return;
    const was = (await this.ctx.storage.get('countChannel')) ?? { id: null, name: null, at: 0, looked: 0 }, c = { ...was };
    if (!c.id && now - c.looked >= (Number(this.env.COUNT_LOOK_MS) || COUNT_LOOK)) {
      c.looked = now;
      const res = await api(this.env, 'GET', `/guilds/${this.env.COMMUNITY_GUILD}/channels`);
      const list = res.ok ? await res.json() : [];
      const found = Array.isArray(list) ? list.find(x => x.type === 2 && COUNT_CHANNEL.test(x.name ?? '')) : null;
      if (found)
        Object.assign(c, { id: found.id, name: found.name });
    }
    const want = countChannelName(this.countHere(now));
    if (c.id && want !== c.name && now - c.at >= (Number(this.env.COUNT_EVERY_MS) || COUNT_EVERY)) {
      c.at = now;
      const res = await api(this.env, 'PATCH', `/channels/${c.id}`, { name: want });
      if (res.ok)
        c.name = want;
      else if (res.status === 404) // deleted: looked for again within the hour
        Object.assign(c, { id: null, name: null });
      this.log('count channel', `${want} ${res.status}`);
    }
    if (JSON.stringify(c) !== JSON.stringify(was))
      await this.ctx.storage.put('countChannel', c);
  }

  /**
   * One post to every server that wants this kind of news, in its language, pinging its role if it chose one. A Roblox
   * update still `waiting` for Roblox's notes is kept, to edit them in once they're out.
   */
  async broadcast(kind, make, waiting) {
    if (!this.env.DISCORD_TOKEN)
      return;
    const sql = this.ctx.storage.sql;
    for (const s of [...sql.exec(`SELECT * FROM subs WHERE ${kind} = 1`)]) {
      const res = await api(this.env, 'POST', `/channels/${s.channel}/messages`, addressed(make(lang(s.locale)), s.role));
      if (res.status === 404) // the channel is gone: so is the subscription
        sql.exec('DELETE FROM subs WHERE guild = ?', s.guild);
      else if (res.ok) {
        const id = (await res.json()).id;
        if (waiting)
          sql.exec('INSERT INTO waiting VALUES (?, ?, ?, ?, ?, ?, ?, ?)', release(waiting.build.version), waiting.kind, JSON.stringify(waiting.build),
            s.channel, id, s.locale ?? null, s.role ?? null, Date.now());
        // Published, for the servers following an announcement channel (a text channel: Discord says no). An edit
        // reaches them too.
        await api(this.env, 'POST', `/channels/${s.channel}/messages/${id}/crosspost`);
      }
      this.log('posted', `${kind} ${s.guild} ${res.status}`);
    }
  }

  /**
   * Roblox's notes looked for (every ten minutes, for two weeks) for each release posted without them; once they're out,
   * each of its posts is edited to show them. A post that was deleted, or can't be edited any more, is let go.
   */
  async addNotes(now) {
    const sql = this.ctx.storage.sql;
    sql.exec('DELETE FROM waiting WHERE at < ?', now - NOTES_FOR);
    const releases = [...sql.exec('SELECT DISTINCT release FROM waiting')].map(r => r.release);
    const asked = Object.fromEntries(Object.entries((await this.ctx.storage.get('notesAsked')) ?? {}).filter(([n]) => releases.includes(Number(n))));
    for (const n of releases) {
      if (now - (asked[n] ?? 0) < (Number(this.env.NOTES_EVERY_MS) || NOTES_EVERY))
        continue;
      asked[n] = now;
      const notes = await readNotes(this.env, n).catch(() => null);
      if (!notes)
        continue;
      for (const p of [...sql.exec('SELECT rowid, * FROM waiting WHERE release = ?', n)]) {
        const body = addressed(buildMessage(lang(p.locale), p.kind, JSON.parse(p.build), { notes, liveAt: p.at }), p.role);
        const res = await api(this.env, 'PATCH', `/channels/${p.channel}/messages/${p.message}`, body);
        if (res.ok || (res.status >= 400 && res.status < 500 && res.status !== 429))
          sql.exec('DELETE FROM waiting WHERE rowid = ?', p.rowid);
        this.log('notes', `${n} ${p.channel} ${res.status}`);
      }
    }
    await this.ctx.storage.put('notesAsked', asked);
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
    if (this.stopped || this.retired)
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
    const resume = !!(this.session?.id && this.session?.url && this.session.intents === INTENTS);
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
      case 11: // Heartbeat answered: the session (with its sequence number) saved now and then, not at every message.
        this.acked = true;
        this.ctx.storage.put('lastBeat', Date.now());
        if (this.session)
          this.ctx.storage.put('session', this.session);
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
        if (msg.t === 'MESSAGE_CREATE') {
          // Every message in every server the bot is in: kept cheap, and the session saved only now and then (below).
          this.trap(msg.d).catch(() => {});
          break;
        }
        if (msg.t === 'READY') {
          this.session = { id: msg.d.session_id, url: msg.d.resume_gateway_url, seq: msg.s, intents: INTENTS };
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
    this.rotatePresence();
  }

  /** The custom status under the bot's name, moving through what's true right now; stops with the connection. */
  rotatePresence() {
    clearInterval(this.presenceTimer);
    let n = 0;
    const show = async () => {
      if (!this.ws || !this.ready) {
        clearInterval(this.presenceTimer);
        return;
      }
      const lines = presenceLines(await this.ctx.storage.get('snap'), this.countHere());
      this.send({ op: 3, d: { since: null, activities: [{ type: 4, name: 'Custom Status', state: lines[n++ % lines.length] }], status: 'online', afk: false } });
    };
    this.presenceTimer = setInterval(show, Number(this.env.PRESENCE_EVERY_MS) || PRESENCE_EVERY);
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
    clearInterval(this.presenceTimer);
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
      intents: INTENTS,
      properties: { os: 'linux', browser: 'bloxkeep', device: 'bloxkeep' },
      presence: { since: null, activities: [{ type: 4, name: 'Custom Status', state: presenceLines(null)[0] }], status: 'online', afk: false },
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
      state: !this.env.DISCORD_TOKEN ? 'no token' : this.retired ? 'retired' : this.stopped ? 'stopped' : this.ready ? 'online' : this.ws ? 'connecting' : 'offline',
      stopped: this.stopped,
      place: await this.place(),
      interactions: this.timings ?? [],
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

/**
 * Whether whoever used the command could give this role by hand, by Discord's own rule: Manage Roles (or Administrator)
 * and the role below their highest one, the server's owner above all; never @everyone or a role an app manages. The bot
 * gives /verify's role to anyone who solves the captcha, so without this Manage Server alone would hand out any role
 * under the bot's own (a moderator's, say).
 */
async function mayGive(env, i, role) {
  const perms = BigInt(i.member?.permissions ?? 0);
  if (!(perms & (1n << 28n)) && !(perms & (1n << 3n)))
    return false;
  const res = await api(env, 'GET', `/guilds/${i.guild_id}`);
  if (!res.ok)
    return false;
  const guild = await res.json();
  const target = guild.roles?.find(x => x.id === role);
  if (!target || target.managed || role === i.guild_id)
    return false;
  if (guild.owner_id === i.member?.user?.id)
    return true;
  const mine = new Set(i.member?.roles ?? []);
  const top = Math.max(0, ...guild.roles.filter(x => mine.has(x.id)).map(x => x.position));
  return target.position < top;
}

/** What a board shows, without its timestamp: the board is edited only when this changes. */
function boardShown(l, snap) {
  const { timestamp, ...rest } = boardEmbed(l, snap);
  return rest.fields;
}

/**
 * A Bloxkeep app saying it's open (or closing). Small bodies only, and a few dozen a minute from one address at most
 * (an internet café's PCs share one); the address is only the rate limit's key, never kept.
 */
async function ping(request, env, gateway) {
  if (env.PING_LIMITER && !(await env.PING_LIMITER.limit({ key: request.headers.get('CF-Connecting-IP') || 'unknown' })).success)
    return Response.json({ error: 'rate' }, { status: 429 });
  const raw = await request.text();
  if (raw.length > 1000)
    return Response.json({ error: 'too large' }, { status: 413 });
  let beat;
  try {
    beat = readBeat(JSON.parse(raw));
  } catch {
    beat = null;
  }
  if (!beat)
    return Response.json({ error: 'bad beat' }, { status: 400 });
  try {
    return Response.json(await gateway.beat(beat));
  } catch {
    return Response.json({ error: 'busy' }, { status: 503 }); // the object starting over (a deploy): the next beat counts
  }
}

/**
 * The README's downloads badge (shields.io's endpoint badge reads this): the install files' downloads, as the bot's
 * object last had them from GitHub (Gateway.downloads), kept an hour per Cloudflare location. "unknown" only before
 * GitHub ever answered.
 */
async function downloadsBadge(env, ctx) {
  const key = new Request('https://bloxkeep-bot.invalid/badge/downloads'), cache = caches.default;
  const hit = await cache.match(key);
  if (hit)
    return hit;
  const n = await ask(env, g => g.downloads()).catch(() => null);
  if (n == null)
    return Response.json({ schemaVersion: 1, label: 'downloads', message: 'unknown', color: 'lightgrey', isError: true },
      { headers: { 'Cache-Control': 'max-age=300' } });
  const message = n >= 1000 ? `${(n / 1000).toFixed(n >= 10_000 ? 0 : 1).replace(/\.0$/, '')}k` : String(n);
  const seconds = Math.floor(Number(env.DOWNLOADS_EVERY_MS ?? DOWNLOADS_EVERY) / 1000);
  const res = Response.json({ schemaVersion: 1, label: 'downloads', message, color: 'brightgreen' },
    { headers: { 'Cache-Control': `public, max-age=${seconds}` } });
  if (seconds > 0)
    ctx.waitUntil(cache.put(key, res.clone()));
  return res;
}

/** The bot's object: made in eastern North America the first time, then wherever it is. */
const bot = env => env.GATEWAY.get(env.GATEWAY.idFromName(env.OBJECT_NAME || OBJECT), { locationHint: PLACE });
/** A call someone waits for, made once more if the object was starting over (a deploy: calls.js). */
const ask = (env, call) => onceMore(() => bot(env), call);

export default {
  async fetch(request, env, ctx) {
    const url = new URL(request.url);
    const gateway = bot(env);
    if (url.pathname === '/status')
      return Response.json(await gateway.status(), { headers: { 'Cache-Control': 'no-store' } });
    if (url.pathname === '/discord') // the app's Discord button
      return new Response(null, { status: 302, headers: { Location: await gateway.invite(), 'Cache-Control': 'no-store' } });
    if (url.pathname === '/badge/downloads') // the README's downloads badge
      return downloadsBadge(env, ctx);
    if (url.pathname === '/online/ping' && request.method === 'POST') // a Bloxkeep app, open now (online.js)
      return ping(request, env, gateway);
    if (url.pathname === '/online' || url.pathname === '/online.json') { // the dashboard, with the key /online gives
      const key = await dashboardKey(env.DISCORD_TOKEN);
      if (!key || url.searchParams.get('key') !== key)
        return Response.json({ error: 'not found' }, { status: 404 });
      const noStore = { 'Cache-Control': 'no-store', 'X-Robots-Tag': 'noindex' };
      return url.pathname === '/online'
        ? new Response(dashboardPage(), { headers: { 'Content-Type': 'text/html; charset=utf-8', ...noStore } })
        : Response.json(await ask(env, g => g.onlineNow()), { headers: noStore });
    }
    if (url.pathname === '/interactions' && request.method === 'POST') {
      const body = await request.text();
      if (!await verify(env.DISCORD_PUBLIC_KEY, request.headers.get('X-Signature-Ed25519'), request.headers.get('X-Signature-Timestamp'), body))
        return new Response('bad signature', { status: 401 });
      const i = JSON.parse(body);
      if (i.type === 1) // Discord checking the address
        return Response.json({ type: 1 });
      if (i.type === 2 && i.data?.name === 'setup') {
        // Many calls to Discord: "thinking…" now (seen only by them), the result when it's done.
        ctx.waitUntil(gateway.startSetup(i));
        return Response.json({ type: 5, data: { flags: 64 } });
      }
      if (i.type === 2) {
        const started = Date.now(), reply = await ask(env, g => g.interaction(i));
        ctx.waitUntil(gateway.noteInteraction(request.cf?.colo ?? null, i.data?.name ?? '', Date.now() - started));
        return Response.json(reply);
      }
      if (i.type === 3 || i.type === 5) { // a button pressed, a form sent (/verify)
        const started = Date.now(), { reply, png } = await ask(env, g => g.component(i));
        ctx.waitUntil(gateway.noteInteraction(request.cf?.colo ?? null, i.data?.custom_id ?? '', Date.now() - started));
        if (!png)
          return Response.json(reply);
        // The captcha's picture goes with the answer, as the file its embed shows.
        const form = new FormData();
        form.append('payload_json', JSON.stringify(reply));
        form.append('files[0]', new Blob([png], { type: 'image/png' }), 'captcha.png');
        return new Response(form);
      }
      return Response.json({ error: 'unsupported' }, { status: 400 });
    }
    return Response.json({ error: 'not found' }, { status: 404 });
  },

  async scheduled(event, env, ctx) {
    ctx.waitUntil(bot(env).ensure(env.CF_VERSION?.id).catch(() => {})); // an object starting over rejects this call
  },
};
