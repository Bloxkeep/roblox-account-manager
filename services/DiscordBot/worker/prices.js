// Prices of a game's items (Game news › Values in the app): read by the bot from a values website once a day and sent to
// the apps with the games (worker.js feedsNow, then { t: 'prices' } when they change). The apps never call the website:
// one read a day here, whatever the number of apps. Pictures go through the bot (/prices/img), so the apps load nothing
// from the website's picture host. A game's prices show while that game is shown in Bloxkeep (/feed show).

export const PRICES_EVERY = 24 * 3600_000, PRICES_RETRY = 60 * 60_000;
const PAGES_MAX = 20, ITEMS_MAX = 400;
const UA = 'Bloxkeep-bot/1.0 (+https://bloxkeep.github.io/roblox-account-manager)';

/**
 * Where each game's prices come from. bloxfruitsvalues.com: the list its own pages load (24 items a page). Prices from
 * there are theirs: the app names and links the site under the list.
 */
export const PRICE_SOURCES = [
  { game: 'blox-fruits', title: 'Blox Fruits', place: 2753915549, name: 'bloxfruitsvalues.com', site: 'https://bloxfruitsvalues.com', api: 'https://bloxfruitsvalues.com/api/v1/items/bloxfruits/all' },
];

const num = v => (typeof v === 'number' && Number.isFinite(v) && v > 0 ? v : null);
const str = (v, n = 40) => (typeof v === 'string' && v.trim() && v !== 'N/A' ? v.trim().slice(0, n) : null);

/** An item as the apps get it: its name, kind, picture, values, demand (of 10), trend, prices in Beli and Robux, its page. */
export function priceItem(i, site) {
  if (!i || typeof i !== 'object' || !str(i.name, 80))
    return null;
  const m = i.metadata && typeof i.metadata === 'object' ? i.metadata : {};
  const pics = String(i.image ?? '').split(',').map(s => s.trim());
  // The picture host's copy first (made to be linked to), else the site's own.
  const img = pics.find(u => /^https:\/\/i\.postimg\.cc\/[\w-]{1,40}\/[\w.%()-]{1,120}\.(png|jpe?g|webp|gif)$/i.test(u))
    ?? pics.find(u => /^https:\/\/bloxfruitsvalues\.com\/api\/bloxfruits\/item-image\/\d{1,6}$/.test(u)) ?? null;
  const name = str(i.name, 80), category = str(i.category) ?? 'Other';
  return {
    id: String(i.itemId ?? i.id ?? name).slice(0, 60), name, category, rarity: str(m.tier) ?? str(i.rarity), type: str(m.fruitType),
    img, value: num(m.regValue), perm: num(m.permValue), demand: num(m.regDemand), permDemand: num(m.permDemand),
    trend: str(m.regTrend), permTrend: str(m.permTrend), beli: num(m.beliPrice), robux: num(m.robuxPrice), bestFor: str(m.bestUsedFor),
    updated: Date.parse(i.updatedAt ?? '') || null,
    // The site's page for it: its kind and name, lower case, spaces as dashes ("limiteds/vibe-(frame-break)").
    url: site ? `${site}/values/${encodeURIComponent(category.toLowerCase().replace(/\s+/g, '-'))}/${encodeURIComponent(name.toLowerCase().replace(/\s+/g, '-'))}` : null,
  };
}

/** Every item of a source, page by page. Throws when the site can't be read (the last list then stays). */
export async function readPrices(source, apiUrl = source.api) {
  const items = [];
  for (let page = 1; page <= PAGES_MAX; page++) {
    const res = await fetch(`${apiUrl}?page=${page}&limit=24`, { headers: { 'User-Agent': UA, Accept: 'application/json' } });
    if (!res.ok)
      throw new Error(`HTTP ${res.status}`);
    const d = await res.json();
    if (!Array.isArray(d?.items))
      throw new Error('no items in the answer');
    items.push(...d.items.map(i => priceItem(i, source.site)).filter(Boolean));
    if (!d.pagination?.hasMore || items.length >= ITEMS_MAX)
      break;
  }
  if (!items.length)
    throw new Error('no items');
  // One of each (a page read while the list moved can repeat one).
  return [...new Map(items.map(i => [i.id, i])).values()].slice(0, ITEMS_MAX);
}
