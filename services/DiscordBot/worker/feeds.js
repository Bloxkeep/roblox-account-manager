// Game feeds (docs/superpowers/specs/2026-11-04-game-feeds-design.md): other games' alert bots post into hidden channels
// of Bloxkeep's server (a Follow, the bot itself invited, or a webhook), one channel per source, and the bot keeps what
// arrives there as it came (/feed raw). A channel /feed show puts under a game reaches the apps too: each message as a
// card (cardOf), sent down the apps' /feeds/connect sockets as it comes. Games and sources live here, so they come and go
// without an app update. /feed is for Bloxkeep's developers only.

// A feed channel keeps this many messages (new ones and edits), the oldest going first.
export const RAW_KEPT = 20;
// And this many cards for the apps. Apps listening at once, at most (each a hibernating socket: nothing while quiet).
export const CARDS_KEPT = 30, MAX_FEED_SOCKETS = 20_000;
export const CATEGORY_NAME = 'Feeds';

const B = n => 1n << BigInt(n);
const VIEW = B(10), SEND = B(11), EMBED = B(14), HISTORY = B(16);
const ow = (id, type, allow = 0n, deny = 0n) => ({ id, type, allow: String(allow), deny: String(deny) });
// Hidden from everyone but admins; the bot sees it. Only what the bot already has may be put here (Discord refuses more),
// so making these needs Manage Channels, nothing else.
const hidden = (everyone, bot) => [ow(everyone, 0, 0n, VIEW), ow(bot, 1, VIEW | SEND | EMBED | HISTORY)];
// Another game's bot invited to post there.
const poster = id => ow(id, 1, VIEW | SEND | EMBED | HISTORY);

export const FEED_COMMAND = {
  // Seen by server admins only, and refused unless Bloxkeep's developers ask (worker.js feedCommand).
  name: 'feed', description: "Game feeds: other games' alert bots (Bloxkeep's developers only)",
  default_member_permissions: String(1 << 3), contexts: [0],
  options: [
    {
      type: 1, name: 'new', description: 'Make a hidden channel for one source, under Feeds',
      options: [
        { type: 3, name: 'name', description: 'Short, like shindo-spawn or sae-egg', required: true, max_length: 40 },
        { type: 6, name: 'bot', description: "The game's bot, if you invite it to post there (optional)" },
      ],
    },
    { type: 1, name: 'list', description: 'The feed channels, and what arrived in each' },
    {
      type: 1, name: 'raw', description: 'The messages a feed channel got, as they came',
      options: [{ type: 7, name: 'channel', description: 'The feed channel', required: true, channel_types: [0, 5] }],
    },
    {
      type: 1, name: 'show', description: "Show a feed channel's messages in Bloxkeep, under a game",
      options: [
        { type: 7, name: 'channel', description: 'The feed channel', required: true, channel_types: [0, 5] },
        { type: 3, name: 'game', description: 'The game, as people know it: Blox Fruits', required: true, max_length: 40 },
        { type: 3, name: 'title', description: 'What this source is: Stock, Fruit spawn…', required: true, max_length: 40 },
        { type: 4, name: 'place', description: "The game's place id on Roblox (its picture and Play)", min_value: 1 },
      ],
    },
    {
      type: 1, name: 'hide', description: 'Stop showing a feed channel in Bloxkeep (still kept here)',
      options: [{ type: 7, name: 'channel', description: 'The feed channel', required: true, channel_types: [0, 5] }],
    },
    {
      type: 1, name: 'remove', description: 'Stop keeping what a feed channel gets (the channel stays)',
      options: [{ type: 7, name: 'channel', description: 'The feed channel', required: true, channel_types: [0, 5] }],
    },
  ],
};

/** A game's id for the apps (what they remember following): its name, lower case, dashes. */
export function gameId(name) {
  return String(name).toLowerCase().normalize('NFD').replace(/[\u0300-\u036f]/g, '').replace(/đ/g, 'd')
    .replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '').slice(0, 40);
}

/**
 * Discord's text as plain lines: mentions and pings gone, links as their words, markdown marks gone. Times (<t:…>) and
 * a server's own emoji (<:fruit:123…>, an item's picture) stay: the app shows times in the person's own clock and
 * language, and the emoji as their pictures.
 */
export function plain(text) {
  return String(text ?? '')
    .replace(/<(@[!&]?|#)\d{5,25}>/g, '')
    .replace(/@(everyone|here)\b/g, '')
    .replace(/\[([^\]\n]{1,200})\]\(<?https?:\/\/[^\s)>]+>?\)/g, '$1')
    .replace(/^[ \t]{0,3}(#{1,3}|-#)[ \t]+/gm, '')
    .replace(/^>[ \t]?/gm, '')
    .replace(/\*\*|__|~~|\|\||`{1,3}/g, '')
    .split('\n').map(l => l.replace(/[ \t]+/g, ' ').trim()).join('\n')
    .replace(/\n{3,}/g, '\n\n').trim();
}

const cut = (s, n) => (s.length > n ? `${s.slice(0, n - 1)}…` : s);
const httpsUrl = u => (typeof u === 'string' && /^https:\/\/\S{1,1000}$/.test(u) ? u : null);
// Pictures only from Discord's own servers (its copy of an embed's picture, proxy_url): the app loads nothing from
// wherever another game's bot points, which would see each person's address.
const discordPic = u => (httpsUrl(u) && /^https:\/\/(media\.discordapp\.net|cdn\.discordapp\.com|images-ext-\d+\.discordapp\.net)\//.test(u) ? u : null);

// A line naming one item: its picture (a server's emoji or an emoji), its name, a bullet, then its price or count, the way
// stock and spawn bots write them ("<:flame:…> **Flame • <:money:…>`250,000`**").
const ITEM = /^\s*(<a?:\w{1,40}:\d{5,25}>|\p{Extended_Pictographic}\uFE0F?)?\s*\**\s*([^*•·<`\n]{1,60}?)\s*\**\s*[•·]\s*\**\s*((?:<a?:\w{1,40}:\d{5,25}>\s*)?)`?([^`*\n]{1,60}?)`?\s*\**\s*$/u;
// A line that is a heading: all of it bold ("**Current Mirage Stock**", "**Normal Stock:**").
const HEADING = /^\s*\*\*([^*\n]{1,120}?):?\*\*\s*$/;

/**
 * What a message shows, as blocks: each embed, or each container of Discord's newer layout (components: text, sections
 * with a picture, galleries), or else its text. A block: its own title (an embed's), its lines, its fields, pictures and
 * colour.
 */
function blocksOf(r) {
  const blocks = [];
  const fromParts = (parts, color) => {
    const block = { title: '', lines: [], fields: [], picture: null, image: null, color: Number.isInteger(color) ? color : null };
    const walk = list => {
      for (const c of list ?? []) {
        if (!c || typeof c !== 'object')
          continue;
        if (c.type === 10 && typeof c.content === 'string')
          block.lines.push(c.content);
        else if (c.type === 9) {
          walk(c.components);
          block.picture ??= discordPic(c.accessory?.media?.proxy_url) ?? discordPic(c.accessory?.media?.url);
        } else if (c.type === 12)
          block.image ??= (c.items ?? []).map(i => discordPic(i?.media?.proxy_url) ?? discordPic(i?.media?.url)).find(Boolean) ?? null;
        else if (c.type === 14)
          block.lines.push('');
      }
    };
    walk(parts);
    return block;
  };
  const loose = [];
  for (const c of r.components ?? []) {
    if (c?.type === 17)
      blocks.push(fromParts(c.components, c.accent_color));
    else if (c && [9, 10, 12].includes(c.type))
      loose.push(c);
  }
  if (loose.length)
    blocks.unshift(fromParts(loose, null));
  for (const e of (r.embeds ?? []).filter(e => e && typeof e === 'object')) {
    if (!(e.title || e.description || e.fields?.length || e.author?.name || e.image || e.thumbnail))
      continue;
    blocks.push({
      title: e.title || '', lines: e.description ? [e.description] : [], fields: (e.fields ?? []).map(f => ({ name: f?.name ?? '', value: f?.value ?? '' })),
      picture: discordPic(e.thumbnail?.proxy_url) ?? discordPic(e.thumbnail?.url), image: discordPic(e.image?.proxy_url) ?? discordPic(e.image?.url),
      color: Number.isInteger(e.color) ? e.color : null, link: httpsUrl(e.url),
      author: e.author?.name ? { name: cut(plain(e.author.name), 80), icon: discordPic(e.author.proxy_icon_url) ?? discordPic(e.author.icon_url) } : null,
    });
  }
  // The message's own text goes before its first block (a ping and a line over an embed), or is the only block.
  if (r.content)
    blocks.length ? blocks[0].lines.unshift(r.content) : blocks.push({ title: '', lines: [r.content], fields: [], picture: null, image: null, color: null });
  if (!blocks.some(b => b.picture))
    for (const b of blocks.slice(0, 1))
      b.picture = discordPic(r.attachments?.find(a => /^image\//.test(a.type ?? ''))?.url);
  return blocks;
}

/**
 * A block as cards: split at its headings when it has several, each with items under it ("Normal Stock:" and "Mirage
 * Stock:" in one box are two cards); a heading with nothing under it before the next is left out.
 */
function cardsOfBlock(b, at) {
  const groups = [];
  let group = { title: b.title, lines: [], fields: [...b.fields] };
  for (const line of b.lines.flatMap(l => String(l).split('\n'))) {
    const head = HEADING.exec(line);
    if (head) {
      if (group.lines.some(l => l.trim()) || group.fields.length)
        groups.push(group);
      // A heading right after the block's own title, with nothing between: the title it is.
      group = { title: head[1].trim(), lines: [], fields: [] };
      continue;
    }
    const item = ITEM.exec(line);
    if (item && !/<t:\d/.test(line)) {
      group.fields.push({ name: `${item[1] ? `${item[1]} ` : ''}${item[2].trim()}`, value: `${item[3].trim()}${item[4].trim()}` });
      continue;
    }
    group.lines.push(line);
  }
  if (group.lines.some(l => l.trim()) || group.fields.length || group.title)
    groups.push(group);
  return groups.map(g => {
    let title = plain(g.title), text = plain(g.lines.join('\n'));
    if (!title && text) {
      const [head, ...rest] = text.split('\n');
      title = head;
      text = rest.join('\n').trim();
    }
    const fields = g.fields.map(f => ({ name: cut(plain(f.name), 80), value: cut(plain(f.value), 300) })).filter(f => f.name || f.value).slice(0, 25);
    if (!title && !text && !fields.length)
      return null;
    const all = [g.title, ...g.lines, ...g.fields.flatMap(f => [f.name, f.value])].join(' ');
    const until = [...all.matchAll(/<t:(\d{9,11})(?::[tTdDfFR])?>/g)].map(m => Number(m[1]) * 1000).find(ms => ms > at) ?? null;
    return {
      title: cut(title, 200), text: cut(text, 1500), fields, picture: b.picture ?? null, image: b.image ?? null, author: b.author ?? null,
      link: b.link ?? null, color: b.color ?? null, until,
    };
  }).filter(Boolean);
}

/**
 * A kept message (rawOf) as the cards the apps show, one for each thing it tells (an embed, a box of Discord's newer
 * layout, a heading's part of one): a title, lines of text, items as name/value (with their pictures), a picture, a big
 * picture, who posted it, a colour, and when it ends (the first time in it still ahead of when it came: "changes
 * <t:…:R>"). The message's poster (its name and picture) stands for a card that names none. at: when it came (ms).
 */
export function cardsOf(r, at = Date.now()) {
  const poster = r.author?.name ? { name: cut(plain(r.author.name), 80), icon: r.author.avatar && /^\d{5,25}$/.test(r.author.id ?? '') && /^(a_)?[0-9a-f]{32}$/.test(r.author.avatar)
    ? `https://cdn.discordapp.com/avatars/${r.author.id}/${r.author.avatar}.webp?size=64` : null } : null;
  return blocksOf(r).flatMap(b => cardsOfBlock(b, at)).map(c => ({ ...c, author: c.author ?? poster })).slice(0, 10);
}

/** The first card of a message, or null (kept for what read one card before). */
export function cardOf(r, at = Date.now()) {
  return cardsOf(r, at)[0] ?? null;
}

/** A channel name Discord takes: lower case, dashes, "feed-" in front. */
export function feedChannelName(name) {
  const slug = String(name).toLowerCase().normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/đ/g, 'd')
    .replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '').slice(0, 40);
  return slug ? `feed-${slug.replace(/^feed-/, '')}` : null;
}

/** What is kept of a message in a feed channel: what a source's map may read, and how it came (Follow, webhook, bot). */
export function rawOf(d, kind) {
  return {
    kind, id: d.id, at: d.edited_timestamp ?? d.timestamp ?? null,
    author: d.author ? { id: d.author.id, name: d.author.global_name ?? d.author.username, avatar: d.author.avatar ?? null, bot: !!d.author.bot } : null,
    webhook: d.webhook_id ?? null, flags: d.flags ?? 0, from: d.message_reference ?? null,
    content: d.content ?? '', embeds: d.embeds ?? [], components: d.components ?? [],
    attachments: (d.attachments ?? []).map(a => ({ name: a.filename, url: a.url, type: a.content_type ?? null })),
  };
}

/**
 * /feed new: the hidden Feeds category (made once), and a channel in it for the source; the game's bot, if named, may
 * post there. api(method, path, body): a REST call with the bot's token. Returns { channel, name, made } or { error }.
 */
export async function makeFeedChannel(api, guild, bot, name, theirBot) {
  const channelName = feedChannelName(name);
  if (!channelName)
    return { error: 'name' };
  const list = await api('GET', `/guilds/${guild}/channels`);
  if (!list.ok)
    return { error: `HTTP ${list.status}` };
  const channels = await list.json();
  if (!Array.isArray(channels))
    return { error: 'no channel list' };
  let category = channels.find(c => c.type === 4 && c.name.toLowerCase() === CATEGORY_NAME.toLowerCase());
  if (!category) {
    const res = await api('POST', `/guilds/${guild}/channels`, { name: CATEGORY_NAME, type: 4, permission_overwrites: hidden(guild, bot) });
    if (!res.ok)
      return { error: `HTTP ${res.status}`, refused: res.status === 403 };
    category = await res.json();
  }
  const found = channels.find(c => c.type === 0 && c.name === channelName && c.parent_id === category.id);
  if (found) { // there already: the game's bot, if named, may post there from now on
    if (theirBot) {
      const { id, type, allow, deny } = poster(theirBot);
      const res = await api('PUT', `/channels/${found.id}/permissions/${id}`, { type, allow, deny });
      if (!res.ok)
        return { error: `HTTP ${res.status}`, refused: res.status === 403 };
    }
    return { channel: found.id, name: channelName, made: false, bot: theirBot };
  }
  const overwrites = [...hidden(guild, bot), ...(theirBot ? [poster(theirBot)] : [])];
  const res = await api('POST', `/guilds/${guild}/channels`, {
    name: channelName, type: 0, parent_id: category.id, topic: 'Game feed: what arrives here is kept as it came (/feed raw)', permission_overwrites: overwrites,
  });
  if (!res.ok)
    return { error: `HTTP ${res.status}`, refused: res.status === 403 };
  return { channel: (await res.json()).id, name: channelName, made: true, bot: theirBot };
}

/** A kept message in a few words, for /feed list and /feed raw. */
export function rawLine(r) {
  const e = r.embeds?.[0], how = r.from ? 'Follow' : r.webhook ? 'webhook' : r.author?.bot ? 'bot' : 'person';
  const what = e?.title || e?.author?.name || r.content || cardsOf(r)[0]?.title || (r.attachments?.length ? 'file' : '(empty)');
  return `${r.kind === 'edit' ? 'edited' : 'new'} · ${how} · ${String(what).replace(/\s+/g, ' ').slice(0, 80)}`;
}
