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
    { type: 1, name: 'prices', description: "Read the games' price lists again now (they're read once a day)" },
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
const MD_MAX = 4000, BLOCKS_MAX = 40, GALLERY_MAX = 10;

/**
 * Discord's markdown as the apps show it: pings and mentions out (they mean nothing outside that server), a command's
 * mention as its name; the rest (bold, lists, quotes, code, links, times, a server's emoji) kept for the app to draw.
 */
export function md(text) {
  return cut(String(text ?? '')
    .replace(/<\/([\w -]{1,64}):\d{5,25}>/g, '/$1')
    .replace(/<(@[!&]?|#)\d{5,25}>|<id:\w{1,20}>/g, '')
    .replace(/@(everyone|here)\b/g, '')
    .replace(/[ \t]+$/gm, '')
    .replace(/\n{3,}/g, '\n\n').trim(), MD_MAX);
}

const emojiText = e => (e?.id && /^\d{5,25}$/.test(e.id) ? `<${e.animated ? 'a' : ''}:${String(e.name || 'e').replace(/\W/g, '') || 'e'}:${e.id}>` : typeof e?.name === 'string' ? e.name.slice(0, 20) : '');
const isVideo = (type, url) => /^video\//.test(type ?? '') || /\.(mp4|webm|mov)(\?|$)/i.test(url ?? '');
/** A picture or video Discord keeps (its own copy first), as a gallery's item. */
const mediaItem = (m, extra = {}) => {
  const url = discordPic(m?.proxy_url) ?? discordPic(m?.url);
  return url ? { url, video: isVideo(m?.content_type, url), w: m?.width ?? null, h: m?.height ?? null, ...extra } : null;
};
const button = b => (b?.type === 2
  ? { label: cut(String(b.label ?? ''), 80), emoji: emojiText(b.emoji), url: b.style === 5 ? httpsUrl(b.url) : null }
  : { label: cut(String(b?.placeholder ?? '…'), 80), emoji: '', url: null, menu: true });

/**
 * Discord's newer layout (components) as the apps' blocks, in order: text, a section (text with a picture or a button
 * beside it), a gallery, a file, a line between, a row of buttons. A box (container) inside is left to the caller.
 */
function componentBlocks(list, files) {
  const out = [];
  for (const c of list ?? []) {
    if (!c || typeof c !== 'object')
      continue;
    switch (c.type) {
      case 10: out.push({ t: 'md', md: md(c.content) }); break;
      case 9: {
        const text = md((c.components ?? []).filter(x => x?.type === 10).map(x => x.content).join('\n'));
        const a = c.accessory;
        out.push({ t: 'section', md: text, thumb: a?.type === 11 ? mediaItem(a.media)?.url ?? null : null, button: a?.type === 2 ? button(a) : null });
        break;
      }
      case 12: {
        const items = (c.items ?? []).map(i => mediaItem(i?.media, { alt: cut(String(i?.description ?? ''), 200), spoiler: !!i?.spoiler })).filter(Boolean).slice(0, GALLERY_MAX);
        if (items.length) out.push({ t: 'gallery', items });
        break;
      }
      case 13: {
        const name = String(c.file?.url ?? '').replace(/^attachment:\/\//, '');
        const f = files.find(x => x.name === name || x.url === c.file?.url);
        out.push({ t: 'file', name: cut(String(c.name ?? f?.name ?? (name || 'file')), 120), size: c.size ?? f?.size ?? null });
        break;
      }
      case 14: out.push({ t: 'sep', line: c.divider !== false, big: c.spacing === 2 }); break;
      case 1: {
        const items = (c.components ?? []).map(button).filter(b => b.label || b.emoji).slice(0, 5);
        if (items.length) out.push({ t: 'buttons', items });
        break;
      }
      default:
        if (typeof c.content === 'string') out.push({ t: 'md', md: md(c.content) });
    }
  }
  return out;
}

/** An embed as a list: its own title, author, link, colour and side picture, and its blocks. */
function embedList(e) {
  const blocks = [];
  if (e.description) blocks.push({ t: 'md', md: md(e.description) });
  const fields = (e.fields ?? []).filter(f => f && (f.name || f.value)).slice(0, 25)
    .map(f => ({ name: md(f.name), value: md(f.value), inline: !!f.inline }));
  if (fields.length) blocks.push({ t: 'fields', items: fields });
  const pic = (e.video?.proxy_url ? mediaItem(e.video) : null) ?? mediaItem(e.image);
  if (pic) blocks.push({ t: 'gallery', items: [pic] });
  if (e.footer?.text || e.timestamp)
    blocks.push({ t: 'footer', md: md(e.footer?.text ?? ''), icon: discordPic(e.footer?.proxy_icon_url) ?? discordPic(e.footer?.icon_url), at: Date.parse(e.timestamp ?? '') || null });
  return {
    title: e.title || '', blocks, picture: mediaItem(e.thumbnail)?.url ?? null, color: Number.isInteger(e.color) ? e.color : null, link: httpsUrl(e.url),
    author: e.author?.name ? { name: cut(plain(e.author.name), 80), icon: discordPic(e.author.proxy_icon_url) ?? discordPic(e.author.icon_url), url: httpsUrl(e.author.url) } : null,
  };
}

/**
 * What a message shows, as lists of blocks in Discord's order: its own text, files, stickers and poll, then each box of
 * the newer layout (loose components together), then each embed. The message's own part goes on top of the first list
 * (a ping and a line over an embed), or is the only one.
 */
function listsOf(r) {
  const files = r.attachments ?? [];
  // A message in the older layout has its rows of buttons under its embeds; in the newer one (flag 1 << 15) everything
  // is in components' own order, and only the files they name are shown.
  const v2 = ((r.flags ?? 0) & (1 << 15)) !== 0;
  const mine = [];
  if (r.content) mine.push({ t: 'md', md: md(r.content) });
  const media = a => /^(image|video)\//.test(a.type ?? '') || (!a.type && /\.(png|jpe?g|gif|webp|mp4|webm|mov)(\?|$)/i.test(a.url ?? ''));
  const shown = (v2 ? [] : files).map(a => (media(a) ? mediaItem({ url: a.url, proxy_url: a.proxy, content_type: a.type, width: a.w, height: a.h }, { spoiler: /^SPOILER_/.test(a.name ?? '') }) : null));
  const pics = shown.filter(Boolean).slice(0, GALLERY_MAX);
  if (pics.length) mine.push({ t: 'gallery', items: pics });
  (v2 ? [] : files).forEach((a, i) => { if (!shown[i]) mine.push({ t: 'file', name: cut(String(a.name ?? 'file'), 120), size: a.size ?? null }); });
  for (const st of (r.stickers ?? []).slice(0, 3))
    if (/^\d{5,25}$/.test(st?.id ?? '') && [1, 2, 4].includes(st.format))
      mine.push({ t: 'sticker', url: `https://media.discordapp.net/stickers/${st.id}.${st.format === 4 ? 'gif' : 'webp'}?size=160`, name: cut(String(st.name ?? ''), 40) });
  if (r.poll?.question) {
    const counts = r.poll.results?.answer_counts ?? [];
    mine.push({
      t: 'poll', q: md(r.poll.question.text), done: !!r.poll.results?.is_finalized, ends: Date.parse(r.poll.expiry ?? '') || null,
      answers: (r.poll.answers ?? []).slice(0, 10).map(a => ({ text: md(a.poll_media?.text ?? ''), emoji: emojiText(a.poll_media?.emoji), votes: counts.find(c => c.id === a.answer_id)?.count ?? 0 })),
    });
  }
  const lists = [], loose = [], rows = [];
  for (const c of r.components ?? []) {
    if (c?.type === 17)
      lists.push({ title: '', blocks: componentBlocks(c.components, files), picture: null, color: Number.isInteger(c.accent_color) ? c.accent_color : null, spoiler: !!c.spoiler });
    else if (c && typeof c === 'object')
      (c.type === 1 && !v2 ? rows : loose).push(c);
  }
  if (loose.length)
    lists.unshift({ title: '', blocks: componentBlocks(loose, files), picture: null, color: null });
  for (const e of (r.embeds ?? []).filter(e => e && typeof e === 'object')) {
    const prev = lists.at(-1);
    // Several embeds with one link and only a picture each: one embed with a gallery, as Discord draws them.
    if (prev?.link && e.url === prev.link && !e.title && !e.description && !e.fields?.length && (e.image || e.thumbnail)) {
      const item = mediaItem(e.image) ?? mediaItem(e.thumbnail), g = prev.blocks.find(b => b.t === 'gallery');
      if (item) g ? g.items.length < GALLERY_MAX && g.items.push(item) : prev.blocks.push({ t: 'gallery', items: [item] });
      continue;
    }
    const list = embedList(e);
    if (list.title || list.blocks.length || list.author || list.picture)
      lists.push(list);
  }
  if (mine.length)
    lists.length ? lists[0].blocks.unshift(...mine) : lists.push({ title: '', blocks: mine, picture: null, color: null });
  if (rows.length)
    lists.length ? lists.at(-1).blocks.push(...componentBlocks(rows, files)) : lists.push({ title: '', blocks: componentBlocks(rows, files), picture: null, color: null });
  return lists;
}

const textOf = b => (b.t === 'md' || b.t === 'section' || b.t === 'footer' ? b.md
  : b.t === 'fields' ? b.items.map(f => `${f.name} ${f.value}`).join('\n')
  : b.t === 'poll' ? [b.q, ...b.answers.map(a => a.text)].join('\n')
  : b.t === 'buttons' ? b.items.map(x => x.label).join(' ') : '');

/**
 * A list as cards: split at its bold headings when it has several, each with what is under it ("Normal Stock:" and
 * "Mirage Stock:" in one box are two cards; a heading with nothing under it before the next is left out). Lines naming
 * an item with its price become fields (tiles with the item's picture). Without a heading or a title, a short first line
 * is the card's title.
 */
function cardsOfList(l, at) {
  const groups = [];
  // An embed's own title stays: a bold line over it (the message's text) is words, not a heading.
  let g = { title: l.title, blocks: [], own: !!l.title };
  const body = b => b.t !== 'sep';
  const add = b => g.blocks.push(b);
  for (const b of l.blocks) {
    if (b.t !== 'md') {
      add(b);
      continue;
    }
    let text = [];
    const flush = () => {
      const m = text.join('\n').trim();
      if (m) add({ t: 'md', md: m });
      text = [];
    };
    for (const line of b.md.split('\n')) {
      const head = !(g.own && !g.blocks.some(body)) && HEADING.exec(line);
      if (head) {
        flush();
        if (g.blocks.some(body))
          groups.push(g);
        g = { title: head[1].trim(), blocks: [] };
        continue;
      }
      const item = !/<t:\d/.test(line) && ITEM.exec(line);
      // An item has its picture or a number (a price, a count): "Sea 3 · Castle on the Sea" is words.
      if (item && (item[1] || /\d/.test(item[4]))) {
        flush();
        const f = { name: cut(plain(`${item[1] ? `${item[1]} ` : ''}${item[2].trim()}`), 80), value: cut(plain(`${item[3].trim()}${item[4].trim()}`), 300), inline: true, item: true };
        const last = g.blocks.at(-1);
        last?.t === 'fields' && last.items[0]?.item && last.items.length < 25 ? last.items.push(f) : add({ t: 'fields', items: [f] });
        continue;
      }
      text.push(line);
    }
    flush();
  }
  if (g.blocks.some(body) || g.title)
    groups.push(g);
  return groups.map(g => {
    let title = plain(g.title);
    const blocks = g.blocks.slice();
    while (blocks.at(-1)?.t === 'sep') blocks.pop();
    if (!title && blocks[0]?.t === 'md') {
      const [head, ...rest] = blocks[0].md.split('\n');
      if (plain(head).length <= 120 && !/^\s*(```|>)/.test(head)) {
        title = plain(head);
        const more = rest.join('\n').trim();
        more ? (blocks[0] = { t: 'md', md: more }) : blocks.shift();
      }
    }
    const fields = blocks.filter(b => b.t === 'fields').flatMap(b => b.items).map(f => ({ name: cut(plain(f.name), 80), value: cut(plain(f.value), 300) })).filter(f => f.name || f.value).slice(0, 25);
    const text = cut(plain(blocks.filter(b => b.t !== 'fields').map(textOf).filter(Boolean).join('\n')), 1500);
    if (!title && !blocks.some(body))
      return null;
    const all = [g.title, ...blocks.map(textOf)].join(' ');
    const until = [...all.matchAll(/<t:(\d{9,11})(?::[tTdDfFR])?>/g)].map(m => Number(m[1]) * 1000).find(ms => ms > at) ?? null;
    const image = blocks.find(b => b.t === 'gallery')?.items.find(i => !i.video)?.url ?? null;
    return {
      title: cut(title, 200), text, fields, picture: l.picture ?? null, image, author: l.author ?? null, link: l.link ?? null, color: l.color ?? null, until,
      blocks: blocks.slice(0, BLOCKS_MAX).map(b => (b.t === 'fields' ? { t: 'fields', items: b.items.map(({ item, ...f }) => f) } : b)), ...(l.spoiler ? { spoiler: true } : {}),
    };
  }).filter(Boolean);
}

/**
 * A kept message (rawOf) as the cards the apps show, one for each thing it tells (an embed, a box of Discord's newer
 * layout, a heading's part of one). Each card has its blocks in Discord's order (what the app draws: text with Discord's
 * markdown, fields, sections, galleries, files, lines, buttons, a footer, a poll, a sticker), and, for alerts and lists,
 * a title, its words as plain text, its items as name/value, a picture, a big picture, who posted it, a colour, and when
 * it ends (the first time in it still ahead of when it came: "changes <t:…:R>"). The message's poster (its name and
 * picture) stands for a card that names none. at: when it came (ms).
 */
export function cardsOf(r, at = Date.now()) {
  const poster = r.author?.name ? { name: cut(plain(r.author.name), 80), icon: r.author.avatar && /^\d{5,25}$/.test(r.author.id ?? '') && /^(a_)?[0-9a-f]{32}$/.test(r.author.avatar)
    ? `https://cdn.discordapp.com/avatars/${r.author.id}/${r.author.avatar}.webp?size=64` : null } : null;
  return listsOf(r).flatMap(l => cardsOfList(l, at)).map(c => ({ ...c, author: c.author ?? poster })).slice(0, 10);
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
    attachments: (d.attachments ?? []).map(a => ({ name: a.filename, url: a.url, proxy: a.proxy_url ?? null, type: a.content_type ?? null, size: a.size ?? null, w: a.width ?? null, h: a.height ?? null })),
    stickers: (d.sticker_items ?? []).map(t => ({ id: t.id, name: t.name, format: t.format_type })), poll: d.poll ?? null,
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
