// Game feeds, first step (docs/superpowers/specs/2026-11-04-game-feeds-design.md): other games' alert bots post into
// hidden channels of Bloxkeep's server (a Follow, the bot itself invited, or a webhook), one channel per source, and the
// bot keeps what arrives there as it came, to build each source's field map from real messages. Nothing reaches the app
// yet. Bloxkeep's developers only.

// A feed channel keeps this many messages (new ones and edits), the oldest going first.
export const RAW_KEPT = 20;
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
      type: 1, name: 'remove', description: 'Stop keeping what a feed channel gets (the channel stays)',
      options: [{ type: 7, name: 'channel', description: 'The feed channel', required: true, channel_types: [0, 5] }],
    },
  ],
};

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
    author: d.author ? { id: d.author.id, name: d.author.username, bot: !!d.author.bot } : null,
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
  if (found)
    return { channel: found.id, name: channelName, made: false };
  const overwrites = [...hidden(guild, bot), ...(theirBot ? [poster(theirBot)] : [])];
  const res = await api('POST', `/guilds/${guild}/channels`, {
    name: channelName, type: 0, parent_id: category.id, topic: 'Game feed: what arrives here is kept as it came (/feed raw)', permission_overwrites: overwrites,
  });
  if (!res.ok)
    return { error: `HTTP ${res.status}`, refused: res.status === 403 };
  return { channel: (await res.json()).id, name: channelName, made: true };
}

/** A kept message in a few words, for /feed list and /feed raw. */
export function rawLine(r) {
  const e = r.embeds?.[0], how = r.from ? 'Follow' : r.webhook ? 'webhook' : r.author?.bot ? 'bot' : 'person';
  const what = e?.title || e?.author?.name || r.content || (r.attachments?.length ? 'file' : '(empty)');
  return `${r.kind === 'edit' ? 'edited' : 'new'} · ${how} · ${String(what).replace(/\s+/g, ' ').slice(0, 80)}`;
}
