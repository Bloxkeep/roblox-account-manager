// Where the bot's news comes from, read every minute by its object (worker.js): Bloxkeep's Roblox watch (the build Roblox
// serves, the one on its test channel before release, and what each means for Multi-Roblox), Roblox's own status page,
// and Bloxkeep's newest public release; Roblox's release notes once a release has been announced, until they're out.
// A source that can't be read is left as it was; nothing is announced from it.
export const REPO = 'https://github.com/Bloxkeep/roblox-account-manager';
const UA = 'BloxkeepBot (https://github.com/Bloxkeep/roblox-account-manager)';
// Each one's address can be pointed elsewhere (DiscordBot/test runs them against its own fakes).
const URLS = {
  WATCH_URL: 'https://raw.githubusercontent.com/Bloxkeep/roblox-account-manager/roblox-status/multi-status.json',
  STATUS_URL: 'https://api.status.io/1.0/status/59db90dbcdeb2f04dadcf16d', // status.roblox.com is a status.io page
  LATEST_URL: `${REPO}/releases/latest`,
  RELEASE_API_URL: 'https://api.github.com/repos/Bloxkeep/roblox-account-manager/releases/tags/',
  NOTES_URL: 'https://create.roblox.com/docs/release-notes/release-notes-',
};
const url = (env, key) => env[key] || URLS[key];

/** "0.744.0.7441080" as numbers, to compare. */
export function versionKey(v) {
  return String(v ?? '').replace(/^v/, '').split(/[.-]/).map(n => parseInt(n, 10) || 0);
}

/** a is a later version than b. */
export function newer(a, b) {
  const x = versionKey(a), y = versionKey(b);
  for (let i = 0; i < Math.max(x.length, y.length); i++) {
    if ((x[i] ?? 0) !== (y[i] ?? 0))
      return (x[i] ?? 0) > (y[i] ?? 0);
  }
  return false;
}

/** Roblox's weekly release a build belongs to: "0.744.0.7441080" is 744. */
export const release = v => versionKey(v)[1] ?? 0;

async function getJson(address) {
  const res = await fetch(address, { headers: { 'User-Agent': UA, Accept: 'application/json' } });
  if (!res.ok)
    throw new Error(`HTTP ${res.status}`);
  return res.json();
}

/**
 * The Roblox watch's file: the live build and, when Roblox's test channel offers a newer one, that one, each with its
 * verdict for Multi-Roblox (same, changed, renamed, gone, first; null while the watch hasn't checked it).
 */
export async function readWatch(env) {
  const d = await getJson(url(env, 'WATCH_URL'));
  const entry = hash => (d.builds ?? []).find(b => b.hash === hash);
  const build = b => b?.version ? { version: b.version, hash: b.hash ?? '', verdict: entry(b.hash)?.verdict ?? null } : null;
  const live = build(d.live);
  const beta = build(d.beta);
  return { live, next: beta && (!live || newer(beta.version, live.version)) ? beta : null };
}

/**
 * Roblox's status page: the overall state and each part (User, Player, Creator) with status.io's code (100 working,
 * 200 maintenance, 300 slow, 400 partly down, 500 down), and the incident's name when there is one.
 */
export async function readStatus(env) {
  const d = (await getJson(url(env, 'STATUS_URL'))).result ?? {};
  const code = Number(d.status_overall?.status_code) || 100;
  return {
    code,
    parts: (d.status ?? []).map(s => ({ name: String(s.name ?? ''), code: Number(s.status_code) || 100 })),
    incident: d.incidents?.find(i => i?.name)?.name ?? null,
  };
}

/** Bloxkeep's newest public release ("v1.0.7"): where /releases/latest sends you, no API limit involved. */
export async function readLatestTag(env) {
  const res = await fetch(url(env, 'LATEST_URL'), { redirect: 'manual', headers: { 'User-Agent': UA } });
  const m = /\/releases\/tag\/(v[^/?#]+)$/.exec(res.headers.get('Location') ?? '');
  if (!m)
    throw new Error(`no release (HTTP ${res.status})`);
  return decodeURIComponent(m[1]);
}

/**
 * A release's highlights in English and Vietnamese: the lines before its first heading, on each side of the
 * "**Tiếng Việt**" line its notes have (release-notes.ps1). Asked only when a new release appears.
 */
export async function readRelease(env, tag) {
  const page = `${REPO}/releases/tag/${tag}`;
  try {
    const d = await getJson(url(env, 'RELEASE_API_URL') + encodeURIComponent(tag));
    const body = String(d.body ?? '').replace(/\r/g, '');
    const [en, vi = ''] = body.split(/^\*\*Tiếng Việt\*\*\s*$/m);
    const highlights = text => text.split(/^#{1,6} /m)[0].trim().slice(0, 1500);
    return { tag, version: tag.replace(/^v/, ''), url: d.html_url || page, en: highlights(en), vi: highlights(vi) || highlights(en) };
  } catch {
    return { tag, version: tag.replace(/^v/, ''), url: page, en: '', vi: '' };
  }
}

/** Where Roblox's notes for a weekly release are read (744 → .../release-notes-744). */
export const notesPage = (env, n) => url(env, 'NOTES_URL') + n;

/**
 * Roblox's release notes for a weekly release (744), each change with its kind ("Improvements" or "Fixes") and whether
 * it is on already ("Live") or still waits behind a switch ("Pending"); null while Roblox hasn't posted them (the page
 * is missing, or has no changes yet). They come from the data the Creator Hub page carries for its own drawing.
 */
export async function readNotes(env, n) {
  const res = await fetch(notesPage(env, n), { headers: { 'User-Agent': UA, Accept: 'text/html' } });
  if (res.status === 404)
    return null;
  if (!res.ok)
    throw new Error(`HTTP ${res.status}`);
  const m = /<script id="__NEXT_DATA__"[^>]*>([\s\S]*?)<\/script>/.exec(await res.text());
  const list = m ? JSON.parse(m[1])?.props?.pageProps?.data?.releaseNoteContents?.content : null;
  if (!Array.isArray(list) || !list.length)
    return null;
  return {
    url: notesPage({}, n),
    changes: list.filter(c => c?.ReleaseNotesText).map(c => ({ kind: c.ReleaseNotesType === 'Fixes' ? 'fix' : 'better', live: c.Status === 'Live', text: String(c.ReleaseNotesText) })),
  };
}
