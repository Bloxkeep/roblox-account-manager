// Talking to Discord over HTTP: checking that an interaction really comes from Discord (its Ed25519 signature, with the
// application's public key), the REST calls the bot makes with its token (posting news, editing the board, putting its
// commands in place), and the commands themselves.
const API = 'https://discord.com/api/v10';
const UA = 'DiscordBot (https://github.com/minhhoang0122/Bloxkeep, 1.0)';
const MANAGE_GUILD = String(1 << 5);
// What the bot needs in a channel: View Channel, Send Messages, Embed Links.
export const PERMISSIONS = String((1 << 10) | (1 << 11) | (1 << 14));

export const inviteUrl = env =>
  env.DISCORD_APP_ID ? `https://discord.com/oauth2/authorize?client_id=${env.DISCORD_APP_ID}&scope=bot+applications.commands&permissions=${PERMISSIONS}` : null;

const hex = s => Uint8Array.from((String(s).match(/../g) ?? []).map(b => parseInt(b, 16)));

/** Discord signs every interaction; one that doesn't verify is refused (and Discord checks that we refuse it). */
export async function verify(publicKey, signature, timestamp, body) {
  if (!publicKey || !signature || !timestamp)
    return false;
  try {
    const key = await crypto.subtle.importKey('raw', hex(publicKey), { name: 'Ed25519' }, false, ['verify']);
    return await crypto.subtle.verify('Ed25519', key, hex(signature), new TextEncoder().encode(timestamp + body));
  } catch {
    return false;
  }
}

/** A REST call with the bot's token; one 429 is waited out (up to 5 s) and tried again. */
export async function api(env, method, path, body, headers = {}) {
  const send = () => fetch((env.DISCORD_API || API) + path, {
    method,
    headers: { Authorization: `Bot ${env.DISCORD_TOKEN}`, 'Content-Type': 'application/json', 'User-Agent': UA, ...headers },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  let res = await send();
  if (res.status === 429) {
    const wait = Number((await res.json().catch(() => ({}))).retry_after) || 1;
    await new Promise(r => setTimeout(r, Math.min(5000, wait * 1000)));
    res = await send();
  }
  return res;
}

// Command descriptions are English only for now (see texts.js lang()); the Vietnamese kept here restores the
// per-locale descriptions when that option returns.
const vi = () => undefined;
const channelOption = { type: 7, name: 'channel', description: 'Where to post', description_localizations: vi('Kênh để đăng'), required: true, channel_types: [0, 5] };

/** The bot's slash commands, put in place by the bot itself whenever this list changes (worker.js). */
export const COMMANDS = [
  { name: 'help', description: "What Bloxkeep's bot can do", description_localizations: vi('Bot Bloxkeep làm được gì') },
  { name: 'download', description: 'Get Bloxkeep, the free Roblox account manager for Windows', description_localizations: vi('Tải Bloxkeep, app quản lý tài khoản Roblox miễn phí cho Windows') },
  { name: 'multi', description: 'Does Multi-Roblox work on the current Roblox, and the one coming next?', description_localizations: vi('Multi-Roblox có chạy trên bản Roblox hiện tại và bản sắp ra không') },
  { name: 'robloxstatus', description: 'Is Roblox down right now?', description_localizations: vi('Roblox có đang lỗi không') },
  {
    name: 'notify', description: 'Post Roblox updates, outages and new Bloxkeep versions in a channel',
    description_localizations: vi('Đăng tin Roblox cập nhật, Roblox lỗi và Bloxkeep ra bản mới vào một kênh'),
    default_member_permissions: MANAGE_GUILD, contexts: [0],
    options: [
      {
        type: 1, name: 'set', description: 'Choose the channel, and a role to ping', description_localizations: vi('Chọn kênh, và một role để ping'),
        options: [
          channelOption,
          { type: 8, name: 'role', description: 'Pinged with each post (optional)', description_localizations: vi('Được ping mỗi khi có tin (không bắt buộc)') },
          { type: 5, name: 'roblox', description: 'Roblox updates, also before release (on unless you turn it off)', description_localizations: vi('Roblox cập nhật, báo cả trước khi ra (mặc định bật)') },
          { type: 5, name: 'outages', description: 'Roblox outages (on unless you turn it off)', description_localizations: vi('Roblox lỗi (mặc định bật)') },
          { type: 5, name: 'bloxkeep', description: 'New Bloxkeep versions (on unless you turn it off)', description_localizations: vi('Bloxkeep ra bản mới (mặc định bật)') },
        ],
      },
      { type: 1, name: 'off', description: 'Stop posting news in this server', description_localizations: vi('Ngừng đăng tin trong server này') },
      { type: 1, name: 'show', description: 'Where news goes now', description_localizations: vi('Tin đang được đăng ở đâu') },
    ],
  },
  {
    // Seen by server admins only, and refused unless Bloxkeep's developers ask (worker.js runSetup).
    name: 'setup', description: "Set up Bloxkeep's community server (Bloxkeep's developers only)",
    default_member_permissions: String(1 << 3), contexts: [0],
  },
  {
    // Seen by server admins (and in a DM with the bot), and refused unless Bloxkeep's developers ask (worker.js).
    name: 'online', description: "How many people have Bloxkeep open right now (Bloxkeep's developers only)",
    default_member_permissions: String(1 << 3), contexts: [0, 1],
  },
  {
    name: 'board', description: 'A message that keeps itself up to date: Roblox, Multi-Roblox, outages, Bloxkeep',
    description_localizations: vi('Một tin nhắn tự cập nhật: Roblox, Multi-Roblox, sự cố, Bloxkeep'),
    default_member_permissions: MANAGE_GUILD, contexts: [0],
    options: [
      { type: 1, name: 'set', description: 'Post the board in a channel', description_localizations: vi('Đăng bảng vào một kênh'), options: [channelOption] },
      { type: 1, name: 'off', description: 'Stop updating the board', description_localizations: vi('Ngừng cập nhật bảng') },
    ],
  },
  {
    name: 'verify', description: 'A captcha newcomers solve in Discord to get a role',
    description_localizations: vi('Captcha người mới giải ngay trong Discord để nhận một vai trò'),
    default_member_permissions: MANAGE_GUILD, contexts: [0],
    options: [
      {
        type: 1, name: 'set', description: 'Post the Verify button in a channel', description_localizations: vi('Đăng nút Xác minh vào một kênh'),
        options: [
          channelOption,
          { type: 8, name: 'role', description: 'Given to whoever passes', description_localizations: vi('Cấp cho người giải đúng'), required: true },
        ],
      },
      { type: 1, name: 'off', description: 'Stop verifying in this server', description_localizations: vi('Ngừng xác minh trong server này') },
    ],
  },
];
