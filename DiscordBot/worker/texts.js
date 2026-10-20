// What the bot says, in English and Vietnamese (a server's own language, from Discord), in Bloxkeep's Discord style
// (docs/discord-style.md): one embed, the news in the title, the colour for how it went, short fields three to a row,
// times as Discord timestamps, the Roblox watch's own words for what a build means for Multi-Roblox.
import { REPO, release } from './sources.js';

export const LOGO = 'https://raw.githubusercontent.com/minhhoang0122/bloxkeep/main/docs/brand/bloxkeep.png';
export const SITE = 'https://minhhoang0122.github.io/Bloxkeep/';
const WATCH_DOCS = 'https://github.com/minhhoang0122/bloxkeep/blob/main/docs/roblox-watch.md';
const STATUS_PAGE = 'https://status.roblox.com';
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
    ],
  },
  vi: {
    multi: 'Multi-Roblox', version: 'Phiên bản', released: 'Phát hành', notYet: 'Chưa',
    coming: v => `Roblox ${v} sắp ra`, live: v => `Roblox ${v} đã ra`,
    testChannel: 'Kiểm tra trên kênh thử nghiệm của Roblox', notes: 'Ghi chú phát hành', how: 'Cách kiểm tra',
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
    ],
  },
};
export const words = l => W[l];

/** Discord's timestamp, which each reader sees in their own time zone and language. */
export const stamp = (ms, style = 'R') => `<t:${Math.floor(ms / 1000)}:${style}>`;
const code = v => `\`${v}\``;
const notesUrl = v => `https://create.roblox.com/docs/release-notes/release-notes-${release(v)}`;

function embed(title, color, description, fields, footer, extra = {}) {
  return {
    title, color, ...(description ? { description } : {}), fields,
    thumbnail: { url: LOGO },
    footer: { text: `Bloxkeep · ${footer}`, icon_url: LOGO },
    timestamp: new Date().toISOString(),
    ...extra,
  };
}

/** A build before release (kind "coming") or as it went live ("live"). */
export function buildEmbed(l, kind, build, liveAt = Date.now()) {
  const w = W[l], [word, color, sentence] = verdict(l, build.verdict);
  const small = [...(kind === 'coming' ? [w.testChannel] : []), `[${w.notes}](${notesUrl(build.version)})`, `[${w.how}](${WATCH_DOCS})`];
  return embed(kind === 'coming' ? w.coming(build.version) : w.live(build.version), color, `${sentence}\n-# ${small.join(' · ')}`, [
    { name: w.multi, value: `**${word}**`, inline: true },
    { name: w.version, value: code(build.version), inline: true },
    { name: w.released, value: kind === 'coming' ? w.notYet : stamp(liveAt), inline: true },
  ], 'Roblox watch', { url: notesUrl(build.version) });
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
export function presenceLines(snap) {
  const live = snap?.watch?.live, next = snap?.watch?.next, status = snap?.status;
  const short = v => `0.${release(v)}`;
  return [
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
  },
};
export const replies = l => R[l];
