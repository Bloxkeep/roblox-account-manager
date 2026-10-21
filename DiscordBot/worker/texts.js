// What the bot says, in English and Vietnamese (a server's own language, from Discord), in Bloxkeep's Discord style
// (docs/discord-style.md): one embed, the news in the title, the colour for how it went, short fields three to a row,
// times as Discord timestamps, the Roblox watch's own words for what a build means for Multi-Roblox. A Roblox update is
// a card instead (Discord's newer message layout): a heading, a picture for what it means, and what changed in it.
import { REPO, release } from './sources.js';

export const LOGO = 'https://raw.githubusercontent.com/minhhoang0122/bloxkeep/main/docs/brand/bloxkeep.png';
export const SITE = 'https://minhhoang0122.github.io/Bloxkeep/';
const WATCH_DOCS = 'https://github.com/minhhoang0122/bloxkeep/blob/main/docs/roblox-watch.md';
const STATUS_PAGE = 'https://status.roblox.com';
// The pictures beside a Roblox update, one per verdict (docs/brand/bot/make-icons.py draws them).
const ICONS = 'https://raw.githubusercontent.com/minhhoang0122/bloxkeep/main/docs/brand/bot';
const ICON = { same: 'same', changed: 'changed', renamed: 'changed', gone: 'gone', first: 'first' };
/** The flag that makes a message one of Discord's newer layouts (components only: no content, no embeds). */
export const CARD = 1 << 15;
const SETUP = `${REPO}/releases/latest/download/Bloxkeep-Setup.exe`;
export const COLOR = { green: 0x57F287, yellow: 0xFEE75C, red: 0xED4245, blurple: 0x5865F2, grey: 0x4E5058 };

/**
 * The bot speaks English by default (Bloxkeep's own convention). The Vietnamese strings below are kept for a later
 * per-server language option; to turn it on, return 'vi' from the server's locale here instead of always 'en'.
 */
export const lang = () => 'en';

const VERDICT = {
  en: {
    first: ['Watching', COLOR.blurple, 'This is the first build the watch has seen, so there is nothing to compare it with yet.'],
    same: ['No change', COLOR.green, 'Nothing changed in the lock Multi-Roblox uses, so it should keep working.'],
    changed: ['May be blocked', COLOR.yellow, 'Roblox changed the code that keeps it to one window. Try two windows before counting on Multi-Roblox.'],
    renamed: ['Lock renamed', COLOR.yellow, 'Roblox renamed the lock Multi-Roblox uses. Bloxkeep holds the new name by itself; try two windows to be sure.'],
    gone: ['Likely blocked', COLOR.red, 'The lock Multi-Roblox uses is gone from this build, so Roblox will probably close each window when the next one opens.'],
    null: ['Checking', COLOR.grey, "The Roblox watch hasn't checked this build yet."],
  },
  vi: {
    first: ['Đang theo dõi', COLOR.blurple, 'Đây là bản đầu tiên Roblox watch thấy, nên chưa có gì để so.'],
    same: ['Không đổi', COLOR.green, 'Phần khoá Multi-Roblox dựa vào không đổi, nên vẫn mở được nhiều cửa sổ.'],
    changed: ['Có thể bị chặn', COLOR.yellow, 'Roblox đã sửa phần giữ một cửa sổ. Thử mở hai cửa sổ trước khi dùng Multi-Roblox.'],
    renamed: ['Khoá đổi tên', COLOR.yellow, 'Roblox đổi tên khoá Multi-Roblox dùng. Bloxkeep đã tự giữ cả tên mới; thử mở hai cửa sổ cho chắc.'],
    gone: ['Gần như bị chặn', COLOR.red, 'Bản này không còn khoá Multi-Roblox dùng, nên Roblox nhiều khả năng sẽ đóng cửa sổ cũ mỗi khi mở cửa sổ mới.'],
    null: ['Đang kiểm tra', COLOR.grey, 'Roblox watch chưa kiểm tra xong bản này.'],
  },
};
const verdict = (l, v) => VERDICT[l][v] ?? VERDICT[l].null;

const STATE = {
  en: { 100: 'Working', 200: 'Maintenance', 300: 'Slow', 400: 'Partly down', 500: 'Down', 600: 'Security issue' },
  vi: { 100: 'Bình thường', 200: 'Đang bảo trì', 300: 'Chậm', 400: 'Lỗi một phần', 500: 'Đang sập', 600: 'Sự cố bảo mật' },
};
const PART = {
  en: { User: 'Website & app', Player: 'Playing', Creator: 'Creators' },
  vi: { User: 'Website và app', Player: 'Vào game', Creator: 'Nhà sáng tạo' },
};
const state = (l, code) => STATE[l][code] ?? STATE[l][Math.floor(code / 100) * 100] ?? String(code);
const stateColor = code => code <= 100 ? COLOR.green : code < 400 ? COLOR.yellow : COLOR.red;

const W = {
  en: {
    multi: 'Multi-Roblox', version: 'Version', released: 'Released', notYet: 'Not yet',
    coming: v => `Roblox ${v} is coming`, live: v => `Roblox ${v} is live`,
    testChannel: "Checked on Roblox's test channel", notes: 'Release notes', how: 'How the check works',
    onTest: "on Roblox's test channel", wentLive: 'live', whatsNew: "What's new", better: 'Improvements', fixes: 'Fixes',
    notesLater: v => `Roblox hasn't posted its notes for ${v} yet. They'll show up here once it does.`,
    ofAll: (shown, all) => `${shown} of ${all} changes`, robloxNotes: "Roblox's notes",
    down: 'Roblox is having problems', downSaid: 'Roblox says some of its services are not working right now.',
    up: 'Roblox is working again', upSaid: 'All of Roblox is working again.', downFor: 'Down for',
    status: 'Roblox status', allGood: 'All of Roblox is working.', incident: 'Incident',
    bloxkeepOut: v => `Bloxkeep ${v} is out`, download: 'Download', site: 'Website', fullNotes: 'All the notes',
    bloxkeepIs: v => `Bloxkeep ${v}`,
    bloxkeepAbout: 'A free Windows app for your Roblox accounts: sign in once, open several Roblox windows at once, presets, auto-rejoin and play-time statistics. Everything stays on your PC.',
    next: 'Coming', nothing: 'Nothing yet', robloxNow: 'Roblox now', changedAt: 'Last change', bloxkeep: 'Bloxkeep',
    board: 'Roblox and Bloxkeep now',
    help: "What Bloxkeep's bot can do", addBot: 'Add to your server',
    cmds: [
      ['/multi', 'Does Multi-Roblox work on the current Roblox, and on the one coming next'],
      ['/robloxstatus', 'Is Roblox down right now'],
      ['/download', 'Get Bloxkeep'],
      ['/notify set', 'Post Roblox updates (also before release), outages and new Bloxkeep versions in a channel, pinging a role if you like'],
      ['/board set', 'A message in a channel that keeps itself up to date: Roblox, Multi-Roblox, outages, Bloxkeep'],
      ['/verify set', 'A captcha newcomers solve right in Discord to get a role'],
    ],
    verifyTitle: 'Verify to join', verifyAbout: 'Press **Verify**, then type the code from the picture. It takes a few seconds and keeps bots out.',
    verify: 'Verify', codeTitle: 'Type this code', codeAbout: 'Press **Enter the code** and type the 5 letters and numbers in the picture (capitals or not).',
    enterCode: 'Enter the code', codeLabel: 'The code in the picture',
  },
  vi: {
    multi: 'Multi-Roblox', version: 'Phiên bản', released: 'Phát hành', notYet: 'Chưa',
    coming: v => `Roblox ${v} sắp ra`, live: v => `Roblox ${v} đã ra`,
    testChannel: 'Kiểm tra trên kênh thử nghiệm của Roblox', notes: 'Ghi chú phát hành', how: 'Cách kiểm tra',
    onTest: 'trên kênh thử nghiệm của Roblox', wentLive: 'ra', whatsNew: 'Có gì mới', better: 'Cải tiến', fixes: 'Sửa lỗi',
    notesLater: v => `Roblox chưa đăng ghi chú cho bản ${v}. Khi có, tin này sẽ tự cập nhật.`,
    ofAll: (shown, all) => `${shown} trong ${all} thay đổi`, robloxNotes: 'Ghi chú của Roblox',
    down: 'Roblox đang gặp sự cố', downSaid: 'Roblox báo một số dịch vụ của họ đang không chạy bình thường.',
    up: 'Roblox đã hoạt động lại', upSaid: 'Mọi dịch vụ của Roblox đã chạy bình thường trở lại.', downFor: 'Lỗi trong',
    status: 'Trạng thái Roblox', allGood: 'Mọi dịch vụ của Roblox đều đang chạy bình thường.', incident: 'Sự cố',
    bloxkeepOut: v => `Bloxkeep ${v} đã ra`, download: 'Tải về', site: 'Trang web', fullNotes: 'Xem đầy đủ',
    bloxkeepIs: v => `Bloxkeep ${v}`,
    bloxkeepAbout: 'App miễn phí trên Windows để quản lý tài khoản Roblox: đăng nhập một lần, mở nhiều cửa sổ Roblox cùng lúc, Preset, tự vào lại game và thống kê thời gian chơi. Mọi thứ nằm trên máy bạn.',
    next: 'Sắp ra', nothing: 'Chưa có', robloxNow: 'Roblox hiện tại', changedAt: 'Thay đổi lần cuối', bloxkeep: 'Bloxkeep',
    board: 'Roblox và Bloxkeep lúc này',
    help: 'Bot Bloxkeep làm được gì', addBot: 'Thêm vào server của bạn',
    cmds: [
      ['/multi', 'Multi-Roblox có chạy trên bản Roblox hiện tại và bản sắp ra không'],
      ['/robloxstatus', 'Roblox có đang lỗi không'],
      ['/download', 'Tải Bloxkeep'],
      ['/notify set', 'Đăng tin Roblox cập nhật (báo cả trước khi ra), Roblox lỗi và Bloxkeep ra bản mới vào một kênh, ping một role nếu muốn'],
      ['/board set', 'Một tin nhắn trong kênh tự cập nhật: Roblox, Multi-Roblox, sự cố, Bloxkeep'],
      ['/verify set', 'Captcha người mới giải ngay trong Discord để nhận một vai trò'],
    ],
    verifyTitle: 'Xác minh để vào server', verifyAbout: 'Bấm **Xác minh**, rồi gõ mã trong ảnh. Chỉ mất vài giây, và giúp chặn bot.',
    verify: 'Xác minh', codeTitle: 'Gõ mã này', codeAbout: 'Bấm **Nhập mã** rồi gõ 5 chữ và số trong ảnh (viết hoa hay thường đều được).',
    enterCode: 'Nhập mã', codeLabel: 'Mã trong ảnh',
  },
};
export const words = l => W[l];

/** Discord's timestamp, which each reader sees in their own time zone and language. */
export const stamp = (ms, style = 'R') => `<t:${Math.floor(ms / 1000)}:${style}>`;
const code = v => `\`${v}\``;

function embed(title, color, description, fields, footer, extra = {}) {
  return {
    title, color, ...(description ? { description } : {}), fields,
    thumbnail: { url: LOGO },
    footer: { text: `Bloxkeep · ${footer}`, icon_url: LOGO },
    timestamp: new Date().toISOString(),
    ...extra,
  };
}

/** One of Roblox's changes, short: its links and API paths as plain words, cut at a word near `max` characters. */
export function change(text, max = 110) {
  let s = text.replace(/\[([^\]]+)\]\([^)]*\)/g, '$1')
    // `Class.Workspace.GlobalWind|GlobalWind` → `GlobalWind`, `Class.TeleportService:ReserveServer()` → `TeleportService:ReserveServer()`
    .replace(/`([^`]*)`/g, (_, c) => `\`${c.includes('|') ? c.split('|').pop() : c.replace(/^(Class|Enum|Datatype|Library|Global)\./i, '')}\``)
    .replace(/\s+/g, ' ').trim();
  if (s.length <= max)
    return s;
  s = s.slice(0, max).replace(/\s+\S*$/, '');
  if ((s.match(/`/g) ?? []).length % 2) // cut inside code: closed again
    s = s.endsWith('`') ? s.slice(0, -1) : `${s}\``;
  return `${s.replace(/[\s,.;:(]+$/, '')}…`;
}

// Changes for creators rather than players: Studio and its windows, Luau and its type checks, the API.
const FOR_CREATORS = /\b(Studio|Luau|Type Solver|type checking|plugins?|Assistant|API|Properties window|Explorer|Input Action|StyleEditor|Avatar Settings)\b/i;

/**
 * The few changes shown of each kind: those a player notices first (not for creators, no API names, already on), in
 * Roblox's own order.
 */
function pick(changes, kind, n) {
  const forPlayers = c => (FOR_CREATORS.test(c.text) ? 0 : 4) + (c.text.includes('`') ? 0 : 2) + (c.live ? 1 : 0);
  return changes.map((c, i) => ({ ...c, i })).filter(c => c.kind === kind)
    .sort((a, b) => forPlayers(b) - forPlayers(a) || a.i - b.i).slice(0, n).sort((a, b) => a.i - b.i);
}

const text = content => ({ type: 10, content });
const link = (label, url) => ({ type: 2, style: 5, label, url });

/**
 * A build before release (kind "coming") or as it went live ("live"), as a card: the news and what it means for
 * Multi-Roblox beside its picture, the build, then what's new from Roblox's notes (`notes`, from readNotes), or a line
 * saying they'll come (the bot edits them in once Roblox posts them).
 */
export function buildMessage(l, kind, build, { notes = null, liveAt = Date.now() } = {}) {
  const w = W[l], [word, color, sentence] = verdict(l, build.verdict), v = `0.${release(build.version)}`;
  const facts = [code(build.version), ...(build.hash ? [code(build.hash)] : []), kind === 'coming' ? w.onTest : `${w.wentLive} ${stamp(liveAt)}`];
  let changes = `-# ${w.notesLater(v)}`;
  if (notes) {
    const list = [['better', w.better], ['fix', w.fixes]].map(([k, name]) => [name, pick(notes.changes, k, 3)]).filter(([, cs]) => cs.length);
    const shown = list.reduce((n, [, cs]) => n + cs.length, 0);
    changes = [...list.map(([name, cs]) => `**${name}**\n${cs.map(c => `- ${change(c.text)}`).join('\n')}`),
      `-# ${w.ofAll(shown, notes.changes.length)}`].join('\n');
  }
  return {
    flags: CARD,
    components: [{
      type: 17, accent_color: color, components: [
        {
          type: 9,
          components: [text(`## ${kind === 'coming' ? w.coming(v) : w.live(v)}`), text(`${w.multi} · **${word}**\n${sentence}`), text(`-# ${facts.join(' · ')}`)],
          accessory: { type: 11, media: { url: `${ICONS}/update-${ICON[build.verdict] ?? 'checking'}.png` }, description: `${w.multi}: ${word}` },
        },
        { type: 14, divider: true, spacing: 1 },
        text(`### ${w.whatsNew}\n${changes}`),
        { type: 1, components: [...(notes ? [link(w.robloxNotes, notes.url)] : []), link(w.how, WATCH_DOCS)] },
      ],
    }],
  };
}

/** A post with the server's role named on top (pinged), or nobody pinged: in the content, or above a card. */
export function addressed(body, role) {
  const allowed_mentions = role ? { roles: [role] } : { parse: [] };
  if (body.flags & CARD)
    return { ...body, components: [...(role ? [text(`<@&${role}>`)] : []), ...body.components], allowed_mentions };
  return { ...body, ...(role ? { content: `<@&${role}>` } : {}), allowed_mentions };
}

function partFields(l, status) {
  return status.parts.slice(0, 6).map(p => ({ name: PART[l][p.name] ?? p.name, value: `**${state(l, p.code)}**`, inline: true }));
}

/** Roblox's status page now, as /robloxstatus shows it. */
export function statusEmbed(l, status) {
  const w = W[l], ok = status.code <= 100;
  return embed(ok ? w.status : w.down, stateColor(status.code), ok ? w.allGood : [w.downSaid, status.incident && `**${w.incident}:** ${status.incident}`].filter(Boolean).join('\n'),
    partFields(l, status), 'Roblox status', { url: STATUS_PAGE });
}

/** An outage starting ("down") or over ("up", with how long it lasted). */
export function outageEmbed(l, kind, status, downSince) {
  const w = W[l];
  if (kind === 'down')
    return statusEmbed(l, status);
  const fields = [...partFields(l, status), ...(downSince ? [{ name: w.downFor, value: duration(l, Date.now() - downSince), inline: true }] : [])];
  return embed(w.up, COLOR.green, w.upSaid, fields, 'Roblox status', { url: STATUS_PAGE });
}

function duration(l, ms) {
  const min = Math.max(1, Math.round(ms / 60_000)), h = Math.floor(min / 60), m = min % 60;
  return l === 'vi' ? (h ? `${h} giờ ${m} phút` : `${m} phút`) : (h ? `${h} h ${m} min` : `${m} min`);
}

/** A new public Bloxkeep version, with its highlights in the server's language. */
export function releaseMessage(l, rel) {
  const w = W[l], text = (l === 'vi' ? rel.vi : rel.en) || '';
  return {
    embeds: [embed(w.bloxkeepOut(rel.version), COLOR.blurple, text || null, [], 'Releases', { url: rel.url })],
    components: [buttons([[w.download, SETUP], [w.fullNotes, rel.url]])],
  };
}

/** /download: what Bloxkeep is, its newest version, and the buttons to get it. */
export function downloadMessage(l, version, inviteUrl) {
  const w = W[l];
  return {
    embeds: [embed(version ? w.bloxkeepIs(version) : 'Bloxkeep', COLOR.blurple, w.bloxkeepAbout, [], 'Download', { url: SITE })],
    components: [buttons([[w.download, SETUP], [w.site, SITE], ['GitHub', REPO], ...(inviteUrl ? [[w.addBot, inviteUrl]] : [])])],
  };
}

/** /multi: the live build and the one coming, if any, with what each means for Multi-Roblox. */
export function multiEmbed(l, watch) {
  const w = W[l];
  if (!watch?.live)
    return embed(w.multi, COLOR.grey, verdict(l, null)[2], [], 'Roblox watch');
  const [word, color, sentence] = verdict(l, watch.live.verdict);
  const fields = [
    { name: w.robloxNow, value: code(watch.live.version), inline: true },
    { name: w.multi, value: `**${word}**`, inline: true },
    { name: w.next, value: watch.next ? `${code(watch.next.version)} · **${verdict(l, watch.next.verdict)[0]}**` : w.nothing, inline: true },
  ];
  return embed(w.multi, color, `${sentence}\n-# [${w.how}](${WATCH_DOCS})`, fields, 'Roblox watch');
}

/** The board: everything at a glance, edited in place when something changes. */
export function boardEmbed(l, snap) {
  const w = W[l];
  const live = snap.watch?.live, next = snap.watch?.next, status = snap.status;
  const [word, vColor] = verdict(l, live?.verdict ?? null);
  const color = status && status.code >= 400 ? COLOR.red : status && status.code > 100 ? COLOR.yellow : vColor === COLOR.grey ? COLOR.green : vColor;
  return embed(w.board, color, null, [
    { name: w.robloxNow, value: live ? code(live.version) : '?', inline: true },
    { name: w.multi, value: `**${word}**`, inline: true },
    { name: w.next, value: next ? `${code(next.version)} · **${verdict(l, next.verdict)[0]}**` : w.nothing, inline: true },
    { name: w.status, value: status ? `**${state(l, status.code)}**` : '?', inline: true },
    { name: w.bloxkeep, value: snap.bloxkeep ? `[${code(snap.bloxkeep.version)}](${snap.bloxkeep.url})` : '?', inline: true },
    { name: w.changedAt, value: snap.changedAt ? stamp(snap.changedAt) : '?', inline: true },
  ], 'Status board');
}

/**
 * The lines the bot's custom status moves through (Discord shows one under its name): Roblox and Multi-Roblox now, the
 * build coming, Roblox's status, the newest Bloxkeep, the commands. Only what is known; at least the commands.
 */
export function presenceLines(snap, online = 0) {
  const live = snap?.watch?.live, next = snap?.watch?.next, status = snap?.status;
  const short = v => `0.${release(v)}`;
  return [
    ...(online > 0 ? [online === 1 ? '1 person using Bloxkeep now' : `${online} people using Bloxkeep now`] : []),
    ...(live ? [`Roblox ${short(live.version)} · Multi-Roblox: ${verdict('en', live.verdict)[0]}`] : []),
    ...(next ? [`Coming: Roblox ${short(next.version)} · ${verdict('en', next.verdict)[0]}`] : []),
    ...(status ? [`Roblox status: ${state('en', status.code)}`] : []),
    ...(snap?.bloxkeep ? [`Bloxkeep ${snap.bloxkeep.version} · /download`] : []),
    '/help · /notify · /board',
  ].map(line => line.slice(0, 128));
}

/** /help: the commands, and a button to add the bot elsewhere. */
export function helpMessage(l, inviteUrl) {
  const w = W[l];
  return {
    embeds: [embed(w.help, COLOR.blurple, w.cmds.map(([c, d]) => `**${c}**: ${d}`).join('\n'), [], 'Bot')],
    components: [buttons([[w.download, SETUP], ['GitHub', REPO], ...(inviteUrl ? [[w.addBot, inviteUrl]] : [])])],
  };
}

function buttons(list) {
  return { type: 1, components: list.map(([label, url]) => ({ type: 2, style: 5, label, url })) };
}

/** /verify set: the message with the Verify button, posted in the channel newcomers see. */
export function verifyPanel(l) {
  const w = W[l];
  return {
    embeds: [embed(w.verifyTitle, COLOR.green, w.verifyAbout, [], 'Verification', { timestamp: undefined })],
    components: [{ type: 1, components: [{ type: 2, style: 3, label: w.verify, custom_id: 'verify' }] }],
    allowed_mentions: { parse: [] },
  };
}

/** The captcha, seen only by who pressed Verify: its picture (attached as captcha.png) and the button to type the code. */
export function captchaMessage(l) {
  const w = W[l];
  return {
    embeds: [{ title: w.codeTitle, description: w.codeAbout, color: COLOR.blurple, image: { url: 'attachment://captcha.png' } }],
    components: [{ type: 1, components: [{ type: 2, style: 1, label: w.enterCode, custom_id: 'verify-code' }] }],
    attachments: [{ id: 0, filename: 'captcha.png' }],
  };
}

/** The form the code is typed into. */
export function codeForm(l, length) {
  const w = W[l];
  return {
    custom_id: 'verify-form', title: w.verify,
    components: [{ type: 1, components: [{ type: 4, custom_id: 'code', label: w.codeLabel, style: 1, min_length: length, max_length: length, required: true }] }],
  };
}

/** Short replies only the person who used the command sees. */
const R = {
  en: {
    guildOnly: 'This works in a server only.',
    noRight: 'Only people who can manage the server can do this.',
    cantPost: c => `I can't post in <#${c}>. Give me **View Channel**, **Send Messages** and **Embed Links** there, then try again.`,
    notifyOn: (c, role, kinds) => `Done: news goes to <#${c}>${role ? `, pinging <@&${role}>` : ''} (${kinds}).`,
    roleMuted: r => `\n-# <@&${r}> doesn't allow everyone to mention it, so I can't ping it. Turn on "Allow anyone to @mention this role" in its settings.`,
    notifyOff: 'Done: I no longer post news here.',
    notifyNone: 'Nothing is set. Use **/notify set** to choose a channel.',
    kinds: { roblox: 'Roblox updates', outages: 'Roblox outages', bloxkeep: 'new Bloxkeep versions' },
    hello: 'Bloxkeep will post Roblox updates, outages and new Bloxkeep versions here.',
    boardOn: c => `Done: the board is in <#${c}>, and keeps itself up to date.`,
    boardOff: 'Done: the board is no longer updated (you can delete its message).',
    noKinds: 'Choose at least one kind of news.',
    verifyOn: (c, role) => `Done: the Verify button is in <#${c}>; whoever passes gets <@&${role}>.\n-# I need **Manage Roles**, and my role above <@&${role}> in the role list.`,
    verifyOff: "Done: no more verification here (you can delete the button's message).",
    verifyNotSet: 'Verification is off in this server.',
    verifyAlready: "You're already verified.",
    verifySlow: 'Too many tries. Wait a few minutes, then press **Verify** again.',
    verifyExpired: 'That code has expired. Press **Verify** again for a new one.',
    verifyWrong: 'Wrong code. Press **Verify** again for a new one.',
    verifyDone: "You're in, welcome! The server's channels are open to you now.",
    verifyCant: role => `You passed, but I couldn't give you <@&${role}>. Please tell a moderator.`,
  },
  vi: {
    guildOnly: 'Lệnh này chỉ dùng trong server.',
    noRight: 'Chỉ người có quyền quản lý server mới làm được việc này.',
    cantPost: c => `Bot không đăng được vào <#${c}>. Hãy cho bot quyền **Xem kênh**, **Gửi tin nhắn** và **Nhúng liên kết** ở kênh đó rồi thử lại.`,
    notifyOn: (c, role, kinds) => `Xong: tin sẽ đăng vào <#${c}>${role ? `, ping <@&${role}>` : ''} (${kinds}).`,
    roleMuted: r => `\n-# <@&${r}> chưa cho mọi người nhắc tên nên bot không ping được. Bật "Cho phép mọi người @đề cập vai trò này" trong cài đặt của role.`,
    notifyOff: 'Xong: bot không đăng tin ở đây nữa.',
    notifyNone: 'Chưa chọn kênh nào. Dùng **/notify set** để chọn.',
    kinds: { roblox: 'Roblox cập nhật', outages: 'Roblox lỗi', bloxkeep: 'Bloxkeep ra bản mới' },
    hello: 'Bloxkeep sẽ đăng tin Roblox cập nhật, Roblox lỗi và Bloxkeep ra bản mới ở đây.',
    boardOn: c => `Xong: bảng đã nằm ở <#${c}> và sẽ tự cập nhật.`,
    boardOff: 'Xong: bảng không còn được cập nhật (bạn có thể xoá tin nhắn đó).',
    noKinds: 'Hãy chọn ít nhất một loại tin.',
    verifyOn: (c, role) => `Xong: nút Xác minh đã ở <#${c}>, ai giải đúng sẽ nhận <@&${role}>.\n-# Bot cần quyền **Quản lý vai trò**, và vai trò của bot phải nằm trên <@&${role}> trong danh sách vai trò.`,
    verifyOff: 'Xong: server này không xác minh nữa (bạn có thể xoá tin nhắn có nút).',
    verifyNotSet: 'Server này đang tắt xác minh.',
    verifyAlready: 'Bạn đã xác minh rồi.',
    verifySlow: 'Bạn thử nhiều quá. Đợi vài phút rồi bấm **Xác minh** lại.',
    verifyExpired: 'Mã này đã hết hạn. Bấm **Xác minh** lại để lấy mã mới.',
    verifyWrong: 'Sai mã. Bấm **Xác minh** lại để lấy mã mới.',
    verifyDone: 'Xong, chào mừng bạn! Giờ bạn đã xem được các kênh của server.',
    verifyCant: role => `Bạn đã giải đúng, nhưng bot không cấp được <@&${role}>. Hãy báo cho moderator.`,
  },
};
export const replies = l => R[l];
