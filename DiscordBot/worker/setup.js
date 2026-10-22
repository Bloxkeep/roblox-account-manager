// Bloxkeep's community server, set up by the bot itself with /setup (Bloxkeep's developers only): its roles, categories
// and channels, who may post where, Community on, AutoMod
// (spam, mention raids, bad words, and a Roblox cookie or a free-Robux scam pasted in chat), #dont-type-here (a trap:
// whoever posts there is banned, worker.js trap), its icon, the rules and welcome posts. Run again, it only adds what is missing and updates its own posts: nothing is made twice. A step
// Discord refuses is reported, and the others still run.
import { api } from './discord.js';
import { REPO } from './sources.js';
import { COUNT_CHANNEL, LOGO, SITE, countChannelName } from './texts.js';

const B = n => 1n << BigInt(n);
const P = {
  KICK: B(1), BAN: B(2), ADMIN: B(3), MANAGE_CHANNELS: B(4), AUDIT_LOG: B(7), VIEW: B(10), SEND: B(11), MANAGE_MESSAGES: B(13), EMBED: B(14), HISTORY: B(16),
  MANAGE_NICKNAMES: B(27), MANAGE_THREADS: B(34), PUBLIC_THREADS: B(35), PRIVATE_THREADS: B(36), SEND_IN_THREADS: B(38),
  CONNECT: B(20), MODERATE: B(40),
};
const TEXT = 0, VOICE = 2, CATEGORY = 4, NEWS = 5, FORUM = 15;
// The categories Discord makes with a new server, in English and in Vietnamese (the creator's language decides).
const DISCORD_CATEGORIES = ['Text Channels', 'Voice Channels', 'Kênh Chat', 'Kênh đàm thoại'];
const ICON = 'https://raw.githubusercontent.com/Bloxkeep/roblox-account-manager/main/docs/brand/server-icon.png';
const SETUP = `${REPO}/releases/latest/download/Bloxkeep-Setup.exe`;
const DESCRIPTION = "Bloxkeep: the free Roblox account manager for Windows. Help, ideas, and Roblox update alerts.";

// Permission overwrites: a role (type 0) or the bot itself (type 1, by its user id).
const ow = (id, type, allow = 0n, deny = 0n) => ({ id, type, allow: String(allow), deny: String(deny) });
const botCan = bot => ow(bot, 1, P.VIEW | P.SEND | P.EMBED | P.HISTORY);
// Everyone reads, nobody but moderators and the bot posts.
const readOnly = (everyone, mod, bot) => [
  ow(everyone, 0, 0n, P.SEND | P.PUBLIC_THREADS | P.PRIVATE_THREADS | P.SEND_IN_THREADS), ow(mod, 0, P.SEND), botCan(bot)];
// Seen by these roles only.
const only = (everyone, roles, bot) => [ow(everyone, 0, 0n, P.VIEW), ...roles.map(r => ow(r, 0, P.VIEW)), botCan(bot)];

const RULES = [
  '**Be kind.** No harassment, hate or NSFW.',
  "**Never share your Roblox cookie, password or 2-step codes**, not even with staff. Nobody from Bloxkeep will ever ask. A cookie pasted here is blocked.",
  '**No scams.** No free-Robux or Nitro offers, and no selling or buying accounts or items for real money.',
  '**Right place.** Questions in #help, bugs in #bug-reports, ideas in #ideas, bot commands in #bot-commands.',
  "**Don't post in #dont-type-here.** It bans you on the spot: it's a trap for scam bots and hacked accounts.",
  "Keep to [Discord's Terms](https://discord.com/terms) and [Roblox's Terms of Use](https://www.roblox.com/info/terms). Moderators may remove anything that breaks these.",
];

/** What /setup made (or found) that the bot keeps using, and what it did; runner: who ran it (made an Admin). */
export async function setupServer(env, guildId, bot, runner) {
  const made = [], failed = [];
  const call = async (method, path, body) => {
    const res = await api(env, method, path, body);
    if (!res.ok)
      throw new Error(`HTTP ${res.status} ${(await res.text()).slice(0, 160)}`);
    return res.status === 204 ? null : res.json();
  };
  const step = async (name, fn) => {
    try {
      return await fn();
    } catch (e) {
      failed.push(`${name}: ${e.message}`);
      return null;
    }
  };
  const g = `/guilds/${guildId}`, everyone = guildId; // @everyone's role id is the server's id
  const guild = await call('GET', g);
  const roles = await call('GET', `${g}/roles`);
  const channels = await call('GET', `${g}/channels`);

  // A role by its name, with or without an emoji after it: made with ours, found with whichever the server gave it.
  async function role(name, icon, body) {
    let r = roles.find(x => x.name === name || x.name.startsWith(`${name} `));
    if (!r) {
      r = await call('POST', `${g}/roles`, { name: `${name} ${icon}`, ...body });
      roles.push(r);
      made.push(`@${name}`);
    }
    return r;
  }
  async function channel(name, type, parent, body = {}) {
    let c = channels.find(x => x.name === name && x.type === type);
    if (!c) {
      c = await call('POST', `${g}/channels`, { name, type, ...(parent ? { parent_id: parent.id } : {}), ...body });
      channels.push(c);
      made.push(type === CATEGORY ? name : `#${name}`);
    } else if (parent && c.parent_id !== parent.id) {
      Object.assign(c, await call('PATCH', `/channels/${c.id}`, { parent_id: parent.id }));
    }
    return c;
  }

  // Roles, top to bottom: Admin (everything), Moderator, then the one members take for a ping when Roblox updates.
  const admin = await role('Admin', '👑', { permissions: String(P.ADMIN), color: 0xED4245, hoist: true });
  const mod = await role('Moderator', '🛡️', {
    permissions: String(P.KICK | P.BAN | P.AUDIT_LOG | P.MANAGE_MESSAGES | P.MANAGE_NICKNAMES | P.MANAGE_THREADS | P.MODERATE),
    color: 0x5865F2, hoist: true,
  });
  const pings = await role('Roblox updates', '🔔', { permissions: '0', color: 0xF5C933, mentionable: true });
  const top = Math.max(...[admin, mod, pings].map(r => r.position ?? 1));
  await step('role order', () => call('PATCH', `${g}/roles`, [admin, mod, pings].map((r, i) => ({ id: r.id, position: top - i }))));
  if (runner)
    await step('Admin for you', () => call('PUT', `${g}/members/${runner}/roles/${admin.id}`));

  // What Discord made with the server: its first text channel ("chung" in Vietnamese) becomes #general, its voice
  // channel "General"; both move into Community below, and Discord's categories go once empty.
  const defaults = channels.filter(c => c.type === CATEGORY && DISCORD_CATEGORIES.includes(c.name));
  const theirs = type => channels.find(c => c.type === type && defaults.some(d => d.id === c.parent_id));
  if (!channels.some(c => c.type === TEXT && c.name === 'general') && theirs(TEXT))
    await step('rename #general', async () => Object.assign(theirs(TEXT), await call('PATCH', `/channels/${theirs(TEXT).id}`, { name: 'general' })));
  if (theirs(VOICE) && theirs(VOICE).name !== 'General')
    await step('rename General', async () => Object.assign(theirs(VOICE), await call('PATCH', `/channels/${theirs(VOICE).id}`, { name: 'General' })));

  // Categories and the channels that need nothing more than a server.
  const info = await channel('Info', CATEGORY, null);
  const community = await channel('Community', CATEGORY, null);
  const support = await channel('Support', CATEGORY, null);
  const staff = await channel('Staff', CATEGORY, null, { permission_overwrites: only(everyone, [mod.id], bot) });
  // The trap: everyone may post, and whoever does is banned (scam bots and hacked accounts post in every channel).
  // How many have Bloxkeep open now, as a voice channel's name at the very top, in no category (Discord puts a category's
  // voice channels after its text ones): seen by everyone, joined by nobody, renamed by the bot (worker.js
  // updateCountChannel), which may manage this channel alone. One found (by its name) stays where it is.
  const count = channels.find(x => x.type === VOICE && COUNT_CHANNEL.test(x.name)) ?? await step('online count', async () => {
    const c = await call('POST', `${g}/channels`, {
      name: countChannelName(0), type: VOICE,
      permission_overwrites: [ow(everyone, 0, 0n, P.CONNECT), ow(bot, 1, P.VIEW | P.CONNECT | P.MANAGE_CHANNELS)],
    });
    channels.push(c);
    made.push(c.name);
    return c;
  });
  const trap = await channel('dont-type-here', TEXT, info, { topic: "Don't type here: posting bans you at once. A trap for scam bots and hacked accounts." });
  const welcome = await channel('welcome', TEXT, info, { topic: 'What Bloxkeep is, and where to go here', permission_overwrites: readOnly(everyone, mod.id, bot) });
  const rules = await channel('rules', TEXT, info, { topic: 'Read these before you post', permission_overwrites: readOnly(everyone, mod.id, bot) });
  const general = await channel('general', TEXT, community, { topic: 'Anything about Bloxkeep and Roblox' });
  const generalVn = await channel('general-vn', TEXT, community, { topic: 'Trò chuyện bằng tiếng Việt' });
  const commands = await channel('bot-commands', TEXT, community, { topic: "Bot commands here: /help, /download, /robloxstatus and the other bots' commands" });
  const modChat = await channel('mod-chat', TEXT, staff, { permission_overwrites: only(everyone, [mod.id], bot) });
  const modLog = await channel('mod-log', TEXT, staff, { topic: 'AutoMod alerts and Discord updates', permission_overwrites: only(everyone, [mod.id], bot) });

  // Discord's voice channel into Community, its categories gone once empty.
  for (const v of channels.filter(c => c.type === VOICE && defaults.some(d => d.id === c.parent_id)))
    await step(`move ${v.name}`, async () => Object.assign(v, await call('PATCH', `/channels/${v.id}`, { parent_id: community.id })));
  for (const d of defaults) {
    if (!channels.some(c => c.parent_id === d.id))
      await step(`remove ${d.name}`, async () => {
        await call('DELETE', `/channels/${d.id}`);
        channels.splice(channels.indexOf(d), 1);
      });
  }

  // Community: announcement channels and forums. Members verified (an account older than 5 minutes), media
  // scanned for everyone, notifications for mentions only.
  if (!guild.features.includes('COMMUNITY')) {
    await step('Community', async () => {
      await call('PATCH', g, {
        features: [...guild.features, 'COMMUNITY'], verification_level: 2, explicit_content_filter: 2, default_message_notifications: 1,
        rules_channel_id: rules.id, public_updates_channel_id: modLog.id, description: DESCRIPTION, preferred_locale: 'en-US',
      });
      made.push('Community');
    });
  }
  const announcements = await step('#announcements', () => channel('announcements', NEWS, info,
    { topic: 'New Bloxkeep versions and news', permission_overwrites: readOnly(everyone, mod.id, bot) }));
  const updates = await step('#roblox-updates', () => channel('roblox-updates', NEWS, info,
    { topic: 'Roblox builds coming and live, outages, new Bloxkeep versions. Follow it from your own server too.', permission_overwrites: readOnly(everyone, mod.id, bot) }));
  const forum = (name, topic, tags) => step(`#${name}`, () => channel(name, FORUM, support, { topic, available_tags: tags.map(t => ({ name: t })), default_sort_order: 0 }));
  const help = await forum('help', 'One question per post. Say what you did, what happened, and your Bloxkeep version.', ['Question', 'Multi-Roblox', 'Accounts', 'Launching', 'Solved']);
  const bugs = await forum('bug-reports', "One bug per post: the steps, what happened, what you expected. Settings › Send feedback can attach the error log.", ['Bug', 'Crash', 'Fixed', "Can't reproduce"]);
  const ideas = await forum('ideas', 'One idea per post. React to the ones you want.', ['Idea', 'Planned', 'Done', 'Not planned']);

  // The order people see.
  await step('order', () => call('PATCH', `${g}/channels`, [
    [info, community, support, staff],
    [trap, welcome, rules, announcements, updates],
    [general, generalVn, commands, ...channels.filter(c => c.type === VOICE && c.parent_id === community.id)],
    [help, bugs, ideas],
    [modChat, modLog],
  ].flatMap(list => list.filter(Boolean).map((c, position) => ({ id: c.id, position })))));

  // No onboarding: newcomers get in through the bot's own captcha in #verify (/verify set, by hand: its Keeper role sees
  // the server), and Discord's onboarding wants channels open to everyone.

  // AutoMod, alerts to #mod-log.
  await step('AutoMod', async () => {
    const have = (await call('GET', `${g}/auto-moderation/rules`)).map(r => r.name);
    const block = message => ({ type: 1, ...(message ? { metadata: { custom_message: message } } : {}) });
    const alert = { type: 2, metadata: { channel_id: modLog.id } };
    const list = [
      {
        name: 'Roblox cookies and scams', trigger_type: 1,
        trigger_metadata: {
          // A Roblox cookie, wherever it is in a message; free Robux or Nitro and their tools.
          regex_patterns: ['_\\|WARNING:-DO-NOT-SHARE-THIS', '\\.ROBLOSECURITY'],
          keyword_filter: ['free robux', '*freerobux*', 'robux generator', 'robux hack', 'free nitro', 'nitro giveaway', 'cookie logger'],
        },
        actions: [block('Blocked: never share your Roblox cookie, and no free-Robux or Nitro offers here.'), alert],
      },
      { name: 'Spam', trigger_type: 3, actions: [block(), alert] },
      { name: 'Mention spam', trigger_type: 5, trigger_metadata: { mention_total_limit: 5, mention_raid_protection_enabled: true }, actions: [block(), alert] },
      { name: 'Bad words', trigger_type: 4, trigger_metadata: { presets: [1, 2, 3] }, actions: [block()] },
    ];
    for (const rule of list) {
      if (!have.includes(rule.name))
        await step(`AutoMod ${rule.name}`, async () => {
          await call('POST', `${g}/auto-moderation/rules`, { event_type: 1, enabled: true, ...rule });
          made.push(`AutoMod: ${rule.name}`);
        });
    }
  });

  // The server's icon, the logo, unless it has one.
  if (!guild.icon) {
    await step('icon', async () => {
      const bytes = new Uint8Array(await (await fetch(env.ICON_URL || ICON)).arrayBuffer());
      let text = '';
      for (let i = 0; i < bytes.length; i += 0x8000)
        text += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
      await call('PATCH', g, { icon: `data:image/png;base64,${btoa(text)}` });
      made.push('icon');
    });
  }

  // The bot's posts in #welcome and #rules: updated in place when run again.
  const links = { type: 1, components: [['Download', SETUP], ['GitHub', REPO], ['Website', SITE]].map(([label, url]) => ({ type: 2, style: 5, label, url })) };
  const embed = (title, description, color) => ({ title, description, color, thumbnail: { url: LOGO }, footer: { text: 'Bloxkeep', icon_url: LOGO } });
  async function post(c, body) {
    if (!c)
      return;
    await step(`post in #${c.name}`, async () => {
      const mine = (await call('GET', `/channels/${c.id}/messages?limit=20`)).find(m => m.author?.id === bot);
      if (mine)
        await call('PATCH', `/channels/${c.id}/messages/${mine.id}`, body);
      else {
        await call('POST', `/channels/${c.id}/messages`, { ...body, allowed_mentions: { parse: [] } });
        made.push(`post in #${c.name}`);
      }
    });
  }
  await post(trap, { embeds: [embed("Don't type here", [
    '**Anything posted here bans its sender at once**, and deletes their messages from the last hour in every channel.',
    'Scam bots and hacked accounts post in every channel they can, this one included, so this is where they get caught. People reading this: just leave it be.',
    '-# Moderators and bots are not affected.',
  ].join('\n\n'), 0xED4245)] });
  await post(rules, { embeds: [embed('Rules', RULES.map((r, i) => `**${i + 1}.** ${r}`).join('\n\n'), 0xED4245)] });
  await post(welcome, {
    embeds: [embed('Welcome to Bloxkeep', [
      'Bloxkeep is a free Windows app for your Roblox accounts: sign in once, open several Roblox windows at once, presets, auto-rejoin and play-time statistics. Everything stays on your PC.',
      `**Start here:** ${rules ? `<#${rules.id}>` : '#rules'}, then download Bloxkeep below.`,
      `**Need help?** Post in ${help ? `<#${help.id}>` : '#help'}. **Found a bug?** ${bugs ? `<#${bugs.id}>` : '#bug-reports'}. **An idea?** ${ideas ? `<#${ideas.id}>` : '#ideas'}.`,
      `**Roblox updates:** ${updates ? `<#${updates.id}>` : '#roblox-updates'} says when a Roblox build is coming, and whether Multi-Roblox still works on it.`,
      `-# English in every channel; Vietnamese in ${generalVn ? `<#${generalVn.id}>` : '#general-vn'}.`,
    ].join('\n\n'), 0x5865F2)],
    components: [links],
  });

  return {
    made, failed, updates: updates?.id ?? null, pings: pings.id, modLog: modLog.id, count: count ? { id: count.id, name: count.name } : null,
    trap: { channel: trap.id, exempt: [admin.id, mod.id], owner: guild.owner_id ?? null },
  };
}
