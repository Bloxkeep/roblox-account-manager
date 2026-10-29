// Bloxkeep from Discord (/pc): a person links the Bloxkeep on their PC to their Discord account, then checks its accounts,
// runs a preset, closes windows or switches server from anywhere. The app keeps a WebSocket to the bot's object while
// Settings › Discord › Control from Discord is on (Gateway.fetch, hibernation API: a quiet socket costs nothing).
//
// What the bot keeps: which Discord account each PC is linked to (the PC by a hash of a secret only that app has),
// on disk; a six-letter code while a link is being made, in memory for ten minutes; the names of the PC, its accounts
// and presets that its app sent, for autocomplete, with the socket only (gone when it closes). Commands and answers
// pass through without being kept.

// A code to link a PC: six letters and digits, no look-alikes (0/O, 1/I/L), good for ten minutes and once.
export const CODE_FOR = 10 * 60_000;
const CODE_CHARS = 'ABCDEFGHJKMNPQRSTUVWXYZ23456789';
export const LINK_CODE_LENGTH = 6;
// How long the bot waits for the PC's answer before it says "working on it" and edits that once the answer comes.
export const ANSWER_WAIT = 2500;
// Discord lets a command's answer be changed (and followed up) for 15 minutes.
export const TOKEN_FOR = 15 * 60_000;
// Commands one person may send in a minute (all their PCs together).
export const COMMANDS_PER_MINUTE = 20;
export const MAX_PCS = 10;
// A link waits this long for the person at the PC to allow it in Bloxkeep (a code seen on a stream links nothing by itself).
export const ALLOW_FOR = 5 * 60_000;
// Wrong codes one Discord account may type in ten minutes.
export const WRONG_CODES = 5, WRONG_WINDOW = 10 * 60_000;
// What the object holds for apps at most: codes waiting, apps connected that aren't linked, messages a minute from one.
export const MAX_CODES = 5000, MAX_UNLINKED = 3000, MESSAGES_PER_MINUTE = 60;
// What one message from an app may be at most (a status card with 25 accounts is about 5 KB).
export const MAX_MESSAGE = 16_000;
const MAX_NAME = 60, MAX_LIST = 200;

const option = (name, description, extra = {}) => ({ type: 3, name, description, ...extra });
const pcOption = option('pc', 'Which PC (only needed when you linked more than one)', { autocomplete: true });

/** /pc and its subcommands: usable in servers, in a DM with the bot, and anywhere once the app is added to an account. */
export const PC_COMMAND = {
  name: 'pc', description: 'Bloxkeep on your PC, from Discord: status, presets, windows',
  integration_types: [0, 1], contexts: [0, 1, 2],
  options: [
    { type: 1, name: 'link', description: 'Link Bloxkeep on a PC to your Discord (Settings › Discord gives the code)', options: [option('code', 'The code Bloxkeep shows', { required: true, min_length: LINK_CODE_LENGTH, max_length: LINK_CODE_LENGTH + 2 })] },
    { type: 1, name: 'status', description: 'Which accounts are in a game right now', options: [pcOption] },
    { type: 1, name: 'preset', description: 'Run a preset', options: [option('name', 'The preset', { required: true, autocomplete: true }), pcOption] },
    { type: 1, name: 'close', description: "Close an account's Roblox window, or all of them", options: [option('account', 'The account, or "All accounts"', { required: true, autocomplete: true }), pcOption] },
    { type: 1, name: 'switch', description: 'Move an account to another server of its game', options: [option('account', 'The account', { required: true, autocomplete: true }), pcOption] },
    { type: 1, name: 'unlink', description: 'Stop controlling a PC from Discord', options: [pcOption] },
  ],
};

/** The commands the app is sent (the subcommand's name), and the option each needs. */
export const APP_COMMANDS = { status: null, preset: 'name', close: 'account', switch: 'account' };

export function makeCode(taken) {
  for (;;) {
    const bytes = crypto.getRandomValues(new Uint8Array(LINK_CODE_LENGTH));
    const code = [...bytes].map(b => CODE_CHARS[b % CODE_CHARS.length]).join('');
    if (!taken(code))
      return code;
  }
}

/** A code as typed: upper case, spaces and dashes left out. */
export const readCode = text => String(text ?? '').toUpperCase().replace(/[\s-]/g, '');

/** The PC's id: a hash of the secret its app holds (the secret itself is never kept). */
export async function pcId(secret) {
  if (typeof secret !== 'string' || secret.length < 32 || secret.length > 200)
    return null;
  const hash = await crypto.subtle.digest('SHA-256', new TextEncoder().encode('bloxkeep-remote:' + secret));
  return [...new Uint8Array(hash)].slice(0, 16).map(b => b.toString(16).padStart(2, '0')).join('');
}

const text = (v, max) => typeof v === 'string' ? v.slice(0, max) : '';
const list = v => Array.isArray(v) ? v.slice(0, MAX_LIST).map(x => text(x, MAX_NAME)).filter(Boolean) : [];

/** What the app says about itself when it connects (and when its names change): only names, cut short. */
export const readHello = m => ({ name: text(m.name, MAX_NAME) || 'PC', accounts: list(m.accounts), presets: list(m.presets), lang: m.lang === 'vi' ? 'vi' : 'en', ask: m.ask === true });

/** The app's answer as Discord takes it: text and embeds only (no mentions, files or components), seen by the asker only. */
export function readAnswer(m) {
  const data = { content: text(m.content, 2000), allowed_mentions: { parse: [] }, flags: 64 };
  if (Array.isArray(m.embeds))
    data.embeds = m.embeds.slice(0, 10).filter(e => e && typeof e === 'object');
  if (!data.content && !data.embeds?.length)
    data.content = '…';
  return data;
}

/** Autocomplete's choices: the names that contain what was typed so far, first those that start with it (25 at most). */
export function choices(names, typed) {
  const t = String(typed ?? '').toLowerCase();
  const hits = names.filter(n => n.toLowerCase().includes(t));
  hits.sort((a, b) => +!a.toLowerCase().startsWith(t) - +!b.toLowerCase().startsWith(t));
  return hits.slice(0, 25).map(n => ({ name: n, value: n }));
}

/** The bot's own words for /pc (the app answers the rest in its own language). */
export const PC_TEXT = {
  linked: pc => `Linked: **${pc}** can now be controlled from here. Try \`/pc status\`.`,
  allowFirst: pc => `Bloxkeep on **${pc}** is asking whether to allow this. Press **Allow** there to finish linking (within 5 minutes).`,
  refusedLink: pc => `Bloxkeep on **${pc}** didn't allow it. Nothing was linked.`,
  allowLate: pc => `Nobody allowed it on **${pc}** within 5 minutes. Nothing was linked: get a new code in Bloxkeep.`,
  tooManyWrong: 'Too many wrong codes. Wait 10 minutes, then try again with the code Bloxkeep shows.',
  busy: 'Bloxkeep\'s bot is too busy for a new code right now. Try again in a minute.',
  badCode: "That code doesn't work: it may be mistyped or more than 10 minutes old. Bloxkeep shows a new one in Settings › Discord.",
  tooManyPcs: `You have ${MAX_PCS} PCs linked already. Unlink one first with \`/pc unlink\`.`,
  noPc: 'No PC is linked to your Discord yet. In Bloxkeep, open Settings › Discord › Control from Discord, then type `/pc link` with the code it shows.',
  whichPc: names => `You have more than one PC linked: pick one in the \`pc\` box (${names}).`,
  unknownPc: pc => `None of your linked PCs is called **${pc}**.`,
  offline: pc => `**${pc}** isn't connected right now: Bloxkeep is closed, the PC is off or offline, or Control from Discord is turned off there.`,
  unlinked: pc => `Unlinked: **${pc}** can't be controlled from Discord any more.`,
  slow: 'Asked Bloxkeep. Its answer will show here.',
  noAnswer: pc => `**${pc}** didn't answer in time. It may still have done it: check with \`/pc status\`.`,
  tooFast: 'Too many commands in a minute. Wait a moment, then try again.',
  allAccounts: 'All accounts',
};
