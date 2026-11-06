// The Wiki tab in the app: each game's items (weapons, gear, bosses…) read by the bot once a day from the game's own
// fan wiki (MediaWiki's API on Fandom) and kept here, so the apps never call the wiki and a game is added here, without
// an app update. A page becomes an item: its name, the box of facts at its top (the infobox: damage, rarity, where to
// get it…), its first lines and its picture. Wiki text is CC BY-SA: the app names the wiki under every item, links its
// page and says the licence. Blox Fruits' price list (prices.js) shows in the same tab.

export const WIKI_EVERY = 24 * 3600_000, WIKI_RETRY = 60 * 60_000;
/** Pages read in one go (MediaWiki gives a page's text for 50 pages at most), and requests at most per bot tick. */
const BATCH = 50;
export const WIKI_REQUESTS_PER_TICK = 6;
const ITEMS_MAX = 1500, FIELDS_MAX = 24, FIELD_MAX = 500, LEAD_MAX = 420;
const UA = 'Bloxkeep-bot/1.0 (+https://bloxkeep.github.io/roblox-account-manager)';

/**
 * The games and the wiki categories read for each, in order: a page in two of them goes under the first. The names are
 * the wiki's own (and the game's: players know them in English).
 */
export const WIKI_SOURCES = [
  {
    game: 'deepwoken', name: 'Deepwoken', place: 4111023553, wiki: 'https://deepwoken.fandom.com', title: 'Deepwoken Wiki',
    cats: ['Weapons', 'Equipment', 'Enchantments', 'Oaths', 'Mantras', 'Resonances', 'Murmurs', 'Enemies', 'Items'],
  },
  {
    game: 'pilgrammed', name: 'Pilgrammed', place: 6735572261, wiki: 'https://pilgrammed-rblx.fandom.com', title: 'Pilgrammed Wiki',
    cats: ['Weapons', 'Accessories', 'Armor', 'Bosses', 'Enemies', 'Skills', 'Techniques', 'Materials', 'Quests'],
  },
];

/**
 * Games the Games tab shows to everyone, each with its wiki named here (every one opened and checked 2026-11-06: the
 * biggest wiki of the game on Fandom), so the app never has to look for one. Names as players say them; place is the
 * game's start place on Roblox, for its picture and colour. Added or taken out here, the apps have it within the hour,
 * no update.
 */
export const POPULAR_GAMES = [
  ['blox-fruits', 'Blox Fruits', 2753915549, 'https://blox-fruits.fandom.com'],
  ['shindo-life', 'Shindo Life', 4616652839, 'https://shindo-life-rell.fandom.com'],
  ['king-legacy', 'King Legacy', 4520749081, 'https://king-legacy-official.fandom.com'],
  ['grand-piece-online', 'Grand Piece Online', 1730877806, 'https://grand-piece-online.fandom.com'],
  ['fruit-battlegrounds', 'Fruit Battlegrounds', 9224601490, 'https://official-fruit-battlegrounds.fandom.com'],
  ['anime-adventures', 'Anime Adventures', 8304191830, 'https://animeadventures.fandom.com'],
  ['anime-vanguards', 'Anime Vanguards', 16146832113, 'https://animevanguards.fandom.com'],
  ['jujutsu-infinite', 'Jujutsu Infinite', 10450270085, 'https://jujutsuinfinite.fandom.com'],
  ['the-strongest-battlegrounds', 'The Strongest Battlegrounds', 10449761463, 'https://the-strongest-battlegrounds-rblx.fandom.com'],
  ['deepwoken', 'Deepwoken', 4111023553, 'https://deepwoken.fandom.com'],
  ['peroxide', 'Peroxide', 9096881148, 'https://peroxide-roblox.fandom.com'],
  ['pilgrammed', 'Pilgrammed', 6735572261, 'https://pilgrammed-rblx.fandom.com'],
  ['pet-simulator-99', 'Pet Simulator 99', 8737899170, 'https://pet-simulator.fandom.com'],
  ['grow-a-garden', 'Grow a Garden', 126884695634066, 'https://growagarden.fandom.com'],
  ['ride-a-pet', 'Ride A Pet', 124216119978534, 'https://ride-a-pet.fandom.com'], // checked 2026-11-06: universe 10035204815
  ['fisch', 'Fisch', 16732694052, 'https://fisch.fandom.com'],
  ['bee-swarm-simulator', 'Bee Swarm Simulator', 1537690962, 'https://bee-swarm-simulator.fandom.com'],
  ['bubble-gum-simulator-infinity', 'Bubble Gum Simulator Infinity', 85896571713843, 'https://bgs-infinity.fandom.com'],
  ['tap-simulator-reborn', 'Tap Simulator REBORN', 72858262610964, 'https://tap-simulator-reborn.fandom.com'], // checked 2026-11-07: universe 10629695169, the only wiki (small)
  ['sols-rng', "Sol's RNG", 15532962292, 'https://sol-rng.fandom.com'],
  ['steal-a-brainrot', 'Steal a Brainrot', 109983668079237, 'https://stealabrainrot.fandom.com'],
  ['99-nights-in-the-forest', '99 Nights in the Forest', 79546208627805, 'https://99-nights-in-the-forest.fandom.com'],
  ['adopt-me', 'Adopt Me!', 920587237, 'https://adoptme.fandom.com'],
  ['dress-to-impress', 'Dress to Impress', 15101393044, 'https://dti-dress-to-impress.fandom.com'],
  ['royale-high', 'Royale High', 735030788, 'https://royale-high.fandom.com'],
  ['doors', 'Doors', 6516141723, 'https://doors-game.fandom.com'],
  ['forsaken', 'Forsaken', 18687417158, 'https://forsaken2024.fandom.com'],
  ['murder-mystery-2', 'Murder Mystery 2', 142823291, 'https://murder-mystery-2.fandom.com'],
  ['rivals', 'Rivals', 17625359962, 'https://robloxrivals.fandom.com'],
  ['blade-ball', 'Blade Ball', 13772394625, 'https://bladeball.fandom.com'],
  ['bedwars', 'BedWars', 6872265039, 'https://robloxbedwars.fandom.com'],
  ['arsenal', 'Arsenal', 286090429, 'https://robloxarsenal.fandom.com'],
  ['jailbreak', 'Jailbreak', 606849621, 'https://jailbreak.fandom.com'],
  ['toilet-tower-defense', 'Toilet Tower Defense', 13775256536, 'https://toilet-tower-defense-game.fandom.com'],
].map(([game, name, place, wiki]) => ({ game, name, place, wiki }));

/** Infobox lines that are about the page, not the item. */
const SKIP = /^(image\d*|images?|caption\d*|name|title|alloyed name|description|desc|quote|gallery|imagewidth|width|m\.equip|ref|id|category|killmessage|music|gender|theme)$/i;
/** Wikis' short line names, said in full. */
const LABELS = {
  hp: 'HP', xp: 'XP', exp: 'XP', def: 'Defense', dmg: 'Damage', atk: 'Attack', spd: 'Speed', desc: 'Description', reqs: 'Requirements',
  req: 'Requirements', mantratype: 'Mantra type', ethercost: 'Ether cost', dmgtype: 'Damage type', dmgtaken: 'Damage taken',
  poisebreakable: 'Poise breakable', cd: 'Cooldown', lvl: 'Level',
};

/** The wiki's API address. */
export const apiOf = source => `${source.wiki}/api.php`;

/**
 * One request's worth of a category: up to 50 pages with their text and picture, and where the next one starts (null
 * when the category is done). Throws when the wiki can't be read.
 */
export async function readBatch(source, cat, cont, api = apiOf(source)) {
  const q = new URLSearchParams({
    action: 'query', format: 'json', formatversion: '2', generator: 'categorymembers', gcmtitle: `Category:${cat}`,
    gcmnamespace: '0', gcmlimit: String(BATCH), prop: 'revisions|pageimages|info', rvprop: 'content', rvslots: 'main',
    piprop: 'thumbnail', pithumbsize: '240', inprop: 'url', redirects: '1',
  });
  for (const [k, v] of Object.entries(cont ?? {}))
    q.set(k, String(v));
  const res = await fetch(`${api}?${q}`, { headers: { 'User-Agent': UA, Accept: 'application/json' } });
  if (!res.ok)
    throw new Error(`HTTP ${res.status}`);
  const d = await res.json();
  if (d?.error)
    throw new Error(String(d.error.info ?? d.error.code ?? 'error').slice(0, 120));
  const pages = Array.isArray(d?.query?.pages) ? d.query.pages : [];
  const items = pages.map(p => wikiItem(p, cat, source)).filter(Boolean);
  // Revisions come for 50 pages at most: the same category goes on (gcmcontinue) once they all came (no rvcontinue).
  return { items, next: d?.continue && typeof d.continue === 'object' ? d.continue : null };
}

/** A page as an item, or null when it has no text (a missing page) or is a list of other pages. */
export function wikiItem(page, cat, source) {
  const text = page?.revisions?.[0]?.slots?.main?.content ?? page?.revisions?.[0]?.slots?.main?.['*'];
  const name = typeof page?.title === 'string' ? page.title.trim() : '';
  if (!name || typeof text !== 'string' || /^#redirect/i.test(text))
    return null;
  const box = infobox(text);
  const fields = [];
  let about = '';
  for (const [k, v] of box?.params ?? []) {
    if (/^desc(ription)?$/i.test(k) && !about)
      about = clean(v, { prose: true });
    if (SKIP.test(k) || fields.length >= FIELDS_MAX)
      continue;
    const value = clean(v);
    if (value && !/^(tba|tbd|n\/?a|none|\?+|-|unknown)$/i.test(value))
      fields.push([label(k), cut(value, FIELD_MAX)]);
  }
  const lead = leadOf(text, box) || about;
  if (!fields.length && !lead)
    return null;
  const thumb = page.thumbnail?.source;
  return {
    // A subpage ("Admin-Exclusive Content/Ardorblade") is called by its own name.
    id: String(page.pageid ?? name), name: (name.split('/').pop() || name).slice(0, 100), cat, fields,
    lead: cut(lead, LEAD_MAX),
    img: typeof thumb === 'string' && /^https:\/\/static\.wikia\.nocookie\.net\//.test(thumb) ? thumb : null,
    url: typeof page.fullurl === 'string' && page.fullurl.startsWith(source.wiki + '/') ? page.fullurl : `${source.wiki}/wiki/${encodeURIComponent(name.replace(/ /g, '_'))}`,
  };
}

/** At most n characters, cut after a sentence or between words (…). */
function cut(s, n) {
  if (s.length <= n)
    return s;
  const part = s.slice(0, n), stop = Math.max(part.lastIndexOf('. '), part.lastIndexOf('\n'));
  return stop > n * 0.6 ? part.slice(0, stop + 1).trim() : part.replace(/\s+\S*$/, '') + '…';
}

/** "posture damage" → "Posture damage", "drop_rate" → "Drop rate", "hp" → "HP". */
const label = k => {
  const s = k.replace(/[_]+/g, ' ').replace(/\s+/g, ' ').trim();
  if (LABELS[s.toLowerCase()])
    return LABELS[s.toLowerCase()];
  return (s.charAt(0).toUpperCase() + s.slice(1)).slice(0, 40);
};

/** The first template on the page whose name says Infobox (else the first with three named lines): its lines in order. */
export function infobox(text) {
  let first = null;
  for (const t of templates(text)) {
    const params = parseParams(t.body);
    const named = params.filter(([k]) => k);
    if (/infobox/i.test(t.name))
      return { ...t, params: named };
    if (!first && named.length >= 3)
      first = { ...t, params: named };
  }
  return first;
}

/** The page's top-level templates ({{…}} not inside another), with where they start and end. */
function* templates(text) {
  let i = 0;
  while ((i = text.indexOf('{{', i)) >= 0) {
    const end = matchEnd(text, i);
    if (end < 0)
      return;
    const body = text.slice(i + 2, end - 2);
    const name = body.split('|')[0].trim();
    yield { name, body, start: i, end };
    i = end;
  }
}

/** Where the {{ at i closes (just after its }}), or -1. */
function matchEnd(text, i) {
  let depth = 0;
  for (let j = i; j < text.length - 1; j++) {
    if (text[j] === '{' && text[j + 1] === '{') {
      depth++;
      j++;
    } else if (text[j] === '}' && text[j + 1] === '}') {
      depth--;
      j++;
      if (depth === 0)
        return j + 1;
    }
  }
  return -1;
}

/** A template's lines: [name, value] (name '' for an unnamed one), split at | outside {{ }} and [[ ]]. */
function parseParams(body) {
  const parts = [];
  let depth = 0, link = 0, cur = '';
  for (let j = 0; j < body.length; j++) {
    const two = body.slice(j, j + 2);
    if (two === '{{') { depth++; cur += two; j++; continue; }
    if (two === '}}') { depth--; cur += two; j++; continue; }
    if (two === '[[') { link++; cur += two; j++; continue; }
    if (two === ']]') { link--; cur += two; j++; continue; }
    if (body[j] === '|' && depth === 0 && link === 0) { parts.push(cur); cur = ''; continue; }
    cur += body[j];
  }
  parts.push(cur);
  return parts.slice(1).map(p => {
    const eq = p.indexOf('=');
    // A name has no markup before its "=" (a value can hold one: "[[a|b=c]]").
    if (eq > 0 && !/[{[<]/.test(p.slice(0, eq)))
      return [p.slice(0, eq).trim(), p.slice(eq + 1).trim()];
    return ['', p.trim()];
  });
}

/** The words before the first heading under the infobox; else the first paragraph of the first heading. */
function leadOf(text, box) {
  const rest = box ? text.slice(0, box.start) + text.slice(box.end) : text;
  const blocks = rest.split(/^==[^=].*?==\s*$/m);
  for (const block of blocks.length > 1 ? [blocks[0], blocks[1]] : [blocks[0]]) {
    const paras = block.split(/\n\s*\n/).map(p => clean(p, { prose: true })).filter(p => p.length > 25);
    if (paras.length)
      return paras[0];
  }
  return '';
}

/**
 * Wiki markup as plain text: links as their words, files, galleries, references and tables gone, a small template as
 * its last unnamed value ({{cl|alloyed|75}} → 75), list items on their own lines. In prose (prose) a template standing
 * alone on its line is dropped (a box drawn there, not words).
 */
export function clean(s, { prose = false } = {}) {
  let t = String(s ?? '');
  t = t.replace(/<!--[\s\S]*?-->/g, '').replace(/__[A-Z]+__/g, '')
    .replace(/<gallery[\s\S]*?<\/gallery>/gi, '')
    .replace(/<ref[^>]*\/>/gi, '').replace(/<ref[\s\S]*?<\/ref>/gi, '')
    .replace(/\{\|[\s\S]*?\|\}/g, '')
    .replace(/<br\s*\/?>/gi, '\n');
  if (prose)
    t = t.replace(/^\s*\{\{[^\n]*\}\}\s*$/gm, '');
  // Templates, innermost first.
  for (let n = 0; n < 50 && /\{\{/.test(t); n++) {
    const next = t.replace(/\{\{([^{}]*)\}\}/g, (_, body) => {
      const parts = parseParams(body).filter(([k]) => !k).map(([, v]) => v).filter(Boolean);
      return parts.length ? parts[parts.length - 1] : '';
    });
    if (next === t)
      break;
    t = next;
  }
  t = t.replace(/\{\{|\}\}/g, '')
    .replace(/\[\[(?:File|Image|Category|Datei):[^\]]*\]\]/gi, '')
    .replace(/\[\[[^\]|]*\|([^\]]*)\]\]/g, '$1')
    .replace(/\[\[([^\]]*)\]\]/g, '$1')
    .replace(/\[https?:\/\/[^\s\]]+\s+([^\]]+)\]/g, '$1')
    .replace(/\[https?:\/\/[^\]]+\]/g, '')
    .replace(/'''?/g, '')
    .replace(/<[^>]+>/g, '')
    .replace(/&nbsp;/g, ' ').replace(/&amp;/g, '&').replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&quot;/g, '"');
  const lines = t.split('\n').map(l => l.replace(/^[*#:;]+\s*/, m => (/[*#]/.test(m) ? '• ' : '')).replace(/[ \t]+/g, ' ').trim()).filter(l => l && l !== '•');
  return (prose ? lines.join(' ') : lines.join('\n')).replace(/\s+([,.;:])/g, '$1').trim();
}

/** Every category of a source, read whole (for tools and checks; the bot reads a few requests per tick). */
export async function readAll(source, api = apiOf(source)) {
  const seen = new Map();
  for (const cat of source.cats) {
    let cont = null;
    for (let n = 0; n < 40; n++) {
      const { items, next } = await readBatch(source, cat, cont, api);
      for (const i of items)
        if (!seen.has(i.id))
          seen.set(i.id, i);
      if (!next)
        break;
      cont = next;
    }
  }
  return [...seen.values()].slice(0, ITEMS_MAX);
}

export { ITEMS_MAX as WIKI_ITEMS_MAX };
