// Wiki markup read the way Bloxkeep's bot reads it (services/DiscordBot/worker/wiki.js), for the Wiki part reading a
// wiki itself (WikiSite.tsx): a page's box of facts (its infobox: damage, rarity, where to get it…) as lines of plain
// text, and its first words. Wiki text is CC BY-SA: the part names the wiki under every page and links it.

export type WikiItem = { id: string; name: string; cat: string; fields: [string, string][]; lead: string; img: string | null; url: string | null;
  /** Lines the wiki writes as a short code in a template ({{Rarity|L}}), by line: the template, for the wiki to say in words
   * (WikiSite.tsx sayCodes). */
  codes?: Record<string, string> };

const FIELDS_MAX = 24, FIELD_MAX = 500, LEAD_MAX = 420;
/** Infobox lines that are about the page, not the item. */
/** A line that is one template around a short code: {{Rarity|L}}, {{Tier|S}}. */
const CODE = /^\s*\{\{\s*[^{}|]+\|\s*[^{}|=]{1,4}\s*\}\}\s*$/;
const SKIP = /^(image\d*|images?|caption\d*|name\d*|title\d*|alloyed name|description|desc|quote|gallery|imagewidth|width|m\.equip|ref|id|category|killmessage|music|gender|theme|imagecaption|image_caption|tabber|navbox|icon|thumbnail|model|render)$/i;
/** Wikis' short line names, said in full. */
const LABELS: Record<string, string> = {
  hp: 'HP', xp: 'XP', exp: 'XP', def: 'Defense', dmg: 'Damage', atk: 'Attack', spd: 'Speed', desc: 'Description', reqs: 'Requirements',
  req: 'Requirements', mantratype: 'Mantra type', ethercost: 'Ether cost', dmgtype: 'Damage type', dmgtaken: 'Damage taken',
  poisebreakable: 'Poise breakable', cd: 'Cooldown', lvl: 'Level',
};

type Page = { pageid?: number; title?: string; fullurl?: string; thumbnail?: { source?: string }; revisions?: { slots?: { main?: { content?: string } } }[] };

/** A page as an item of a kind (its infobox template), or null when it has no box and no words. */
export function wikiItem(page: Page, cat: string, template: string, site: string): WikiItem | null {
  const text = page?.revisions?.[0]?.slots?.main?.content;
  const name = typeof page?.title === 'string' ? page.title.trim() : '';
  if (!name || typeof text !== 'string' || /^#redirect/i.test(text)) return null;
  const box = infobox(text, template);
  const fields: [string, string][] = [];
  const codes: Record<string, string> = {};
  let about = '';
  for (const [k, v] of box?.params ?? []) {
    if (/^desc(ription)?$/i.test(k) && !about) about = clean(v, true);
    if (SKIP.test(k) || fields.length >= FIELDS_MAX) continue;
    const value = clean(v);
    if (value && !/^(tba|tbd|n\/?a|none|\?+|-|unknown)$/i.test(value) && !/^[\w .-]+\.(png|jpe?g|gif|webp)$/i.test(value)) {
      fields.push([label(k), cut(value, FIELD_MAX)]);
      if (CODE.test(v)) codes[label(k)] = v.trim();
    }
  }
  const lead = leadOf(text, box) || about;
  if (!fields.length && !lead) return null;
  const thumb = page.thumbnail?.source;
  return {
    ...(Object.keys(codes).length ? { codes } : {}),
    id: String(page.pageid ?? name), name: (name.split('/').pop() || name).slice(0, 100), cat, fields, lead: cut(lead, LEAD_MAX),
    img: typeof thumb === 'string' && /^https:\/\/(static\.wikia\.nocookie\.net|[a-z0-9-]+\.wiki\.gg)\//.test(thumb) ? thumb : null,
    url: typeof page.fullurl === 'string' && page.fullurl.startsWith('https://') ? page.fullurl : `${site}/wiki/${encodeURIComponent(name.replace(/ /g, '_'))}`,
  };
}

/** At most n characters, cut after a sentence or between words (…). */
function cut(s: string, n: number) {
  if (s.length <= n) return s;
  const part = s.slice(0, n), stop = Math.max(part.lastIndexOf('. '), part.lastIndexOf('\n'));
  return stop > n * 0.6 ? part.slice(0, stop + 1).trim() : part.replace(/\s+\S*$/, '') + '…';
}

/** "posture damage" → "Posture damage", "drop_rate" → "Drop rate", "hp" → "HP". */
function label(k: string) {
  const s = k.replace(/[_]+/g, ' ').replace(/\s+/g, ' ').trim();
  if (LABELS[s.toLowerCase()]) return LABELS[s.toLowerCase()];
  return (s.charAt(0).toUpperCase() + s.slice(1)).slice(0, 40);
}

type Template = { name: string; body: string; start: number; end: number; params: [string, string][] };
const same = (a: string, b: string) => a.replace(/_/g, ' ').trim().toLowerCase() === b.replace(/_/g, ' ').trim().toLowerCase();

/** The page's box: the template the kind is (Template:Weapon Infobox), else the first named Infobox, else the first
 * with three named lines. */
export function infobox(text: string, want?: string): Template | null {
  const wanted = want?.replace(/^Template:/i, '');
  let named: Template | null = null, first: Template | null = null;
  for (const t of templates(text)) {
    const params = parseParams(t.body).filter(([k]) => k) as [string, string][];
    const box = { ...t, params };
    if (wanted && same(t.name, wanted)) return box;
    if (!named && /infobox/i.test(t.name)) named = box;
    if (!first && params.length >= 3) first = box;
  }
  return named || first;
}

export type BoxChange = { field: string; was: string; now: string; up: boolean };
/** A number in a line: "1,200", "9.5", "-30". Written as 1e+420 (a joke edit) it isn't one. */
const NUMBER = /-?\d[\d,]*(?:\.\d+)?/g;
const numbersOf = (s: string) => /\de[+-]?\d/i.test(s) ? [] : [...s.matchAll(NUMBER)].map(m => Number(m[0].replace(/,/g, '')));

/**
 * The lines of a page's box whose numbers changed between two versions of the page (drop chance 1% → 5%, range 10 →
 * 9.5): only lines both versions have, with a few numbers each, the same count of them, so a long list or a line just
 * filled in isn't one. up: the first number that changed went up.
 */
export function boxChanges(oldText: string, newText: string): BoxChange[] {
  const now = infobox(newText);
  if (!now) return [];
  const was = infobox(oldText, now.name);
  if (!was) return [];
  const lines = (box: Template) => new Map(box.params.filter(([k]) => !SKIP.test(k) && !/image|file|gif|caption|music/i.test(k))
    .map(([k, v]) => [label(k), clean(v).replace(/\n/g, ' ')] as const));
  const before = lines(was), after = lines(now);
  const out: BoxChange[] = [];
  for (const [field, b] of after) {
    const a = before.get(field);
    if (!a || a === b || a.length > 60 || b.length > 60) continue;
    const x = numbersOf(a), y = numbersOf(b);
    if (!x.length || x.length !== y.length || x.length > 4) continue;
    // The same numbers in another order ("50 Strength, Power 15" → "Power 15, 50 Strength") changed nothing.
    const sorted = (n: number[]) => [...n].sort((p, q) => p - q).join();
    if (sorted(x) === sorted(y)) continue;
    const i = x.findIndex((n, j) => n !== y[j]);
    if (i < 0) continue;
    out.push({ field, was: a, now: b, up: y[i] > x[i] });
  }
  return out.slice(0, 6);
}

/** The page's top-level templates ({{…}} not inside another), with where they start and end. */
function* templates(text: string) {
  let i = 0;
  while ((i = text.indexOf('{{', i)) >= 0) {
    const end = matchEnd(text, i);
    if (end < 0) return;
    const body = text.slice(i + 2, end - 2);
    yield { name: body.split('|')[0].trim(), body, start: i, end };
    i = end;
  }
}

/** Where the {{ at i closes (just after its }}), or -1. */
function matchEnd(text: string, i: number) {
  let depth = 0;
  for (let j = i; j < text.length - 1; j++) {
    if (text[j] === '{' && text[j + 1] === '{') { depth++; j++; }
    else if (text[j] === '}' && text[j + 1] === '}') { depth--; j++; if (depth === 0) return j + 1; }
  }
  return -1;
}

/** A template's lines: [name, value] (name '' for an unnamed one), split at | outside {{ }} and [[ ]]. */
function parseParams(body: string): [string, string][] {
  const parts: string[] = [];
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
    if (eq > 0 && !/[{[<]/.test(p.slice(0, eq))) return [p.slice(0, eq).trim(), p.slice(eq + 1).trim()];
    return ['', p.trim()];
  });
}

/** The words before the first heading under the infobox; else the first paragraph of the first heading. */
function leadOf(text: string, box: Template | null) {
  const rest = box ? text.slice(0, box.start) + text.slice(box.end) : text;
  const blocks = rest.split(/^==[^=].*?==\s*$/m);
  for (const block of blocks.length > 1 ? [blocks[0], blocks[1]] : [blocks[0]]) {
    const paras = block.split(/\n\s*\n/).map(p => clean(p, true)).filter(p => p.length > 25);
    if (paras.length) return paras[0];
  }
  return '';
}

/**
 * Wiki markup as plain text: links as their words, files, galleries, references and tables gone, a small template as
 * its last unnamed value ({{cl|alloyed|75}} → 75), list items on their own lines. In prose a template standing alone on
 * its line is dropped (a box drawn there, not words).
 */
export function clean(s: string, prose = false) {
  let t = String(s ?? '');
  t = t.replace(/<!--[\s\S]*?-->/g, '').replace(/__[A-Z]+__/g, '')
    .replace(/<gallery[\s\S]*?<\/gallery>/gi, '').replace(/<tabber[\s\S]*?<\/tabber>/gi, '')
    .replace(/<ref[^>]*\/>/gi, '').replace(/<ref[\s\S]*?<\/ref>/gi, '')
    .replace(/\{\|[\s\S]*?\|\}/g, '')
    .replace(/<br\s*\/?>/gi, '\n');
  if (prose) t = t.replace(/^\s*\{\{[^\n]*\}\}\s*$/gm, '');
  for (let n = 0; n < 50 && /\{\{/.test(t); n++) {
    const next = t.replace(/\{\{([^{}]*)\}\}/g, (_, body: string) => {
      const parts = parseParams(body).filter(([k]) => !k).map(([, v]) => v).filter(Boolean);
      return parts.length ? parts[parts.length - 1] : '';
    });
    if (next === t) break;
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

/** A kind's name from its template: "Weapon_Infobox" → "Weapon", "SeedPackInfoTemplate" → "Seed Pack", "Pets" → "Pets". */
export function kindName(template: string) {
  const s = template.replace(/^Template:/i, '').replace(/_/g, ' ')
    .replace(/\s*(info ?box|info ?template|template|box)\s*$/i, '').replace(/^(info ?box)\s*/i, '')
    .replace(/([a-z])([A-Z])/g, '$1 $2').trim();
  return s || template;
}

// ---- A parsed page made safe to draw ----

const KEEP_TAGS = new Set(['a', 'abbr', 'b', 'bdi', 'big', 'blockquote', 'br', 'caption', 'center', 'cite', 'code', 'col', 'colgroup', 'dd', 'del', 'dfn', 'div', 'dl', 'dt',
  'em', 'figcaption', 'figure', 'font', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'hr', 'i', 'img', 'ins', 'kbd', 'li', 'mark', 'ol', 'p', 'pre', 'q', 's', 'small', 'span',
  'strong', 'sub', 'sup', 'table', 'tbody', 'td', 'tfoot', 'th', 'thead', 'tr', 'u', 'ul', 'aside', 'section', 'header', 'footer', 'nav', 'details', 'summary', 'time', 'wbr']);
/** Gone with all they hold: code, forms, frames, the wiki's edit links and boxes that need its scripts. */
const DROP_TAGS = new Set(['script', 'style', 'link', 'meta', 'iframe', 'object', 'embed', 'form', 'input', 'button', 'select', 'textarea', 'video', 'audio', 'source',
  'svg', 'math', 'noscript', 'template', 'canvas', 'map', 'area', 'base', 'frame', 'frameset', 'dialog', 'portal']);
const DROP_CLASSES = /\b(mw-editsection|toc|navbox|noprint|mw-empty-elt|reference|reflist|references|printfooter|catlinks|mw-cite-backlink|wds-tabs__wrapper|article-thumb-placeholder|fandom-community-header|mobileHide)\b/;

/** What a link in a page does: another page of the same wiki (opened here), a place in this page, or a link out. */
export type Link = { kind: 'page'; title: string; anchor: string | null } | { kind: 'anchor'; anchor: string } | { kind: 'out'; url: string };

/**
 * A parsed page's HTML with only what reads safely: no code, no frames, no forms, no styles of its own (the Wiki part
 * draws it in its own look), pictures only from the wiki's own servers, every link marked with what it does (data-wk)
 * so the page opens it, none followed by the browser itself.
 */
export function cleanPage(html: string, site: string, toc: { title: string; sections: { line: string; anchor: string; level: number; number: string }[] } | null = null): string {
  const doc = new DOMParser().parseFromString(html, 'text/html');
  const root = doc.body;
  const host = new URL(site).host;
  const imgOk = (u: string) => {
    try {
      const x = new URL(u, site);
      return x.protocol === 'https:' && (x.host === 'static.wikia.nocookie.net' || x.host === host || /\.wiki\.gg$/.test(x.host)) ? x.href : null;
    } catch { return null; }
  };
  const walk = (el: Element) => {
    for (const child of [...el.children]) {
      const tag = child.tagName.toLowerCase();
      const cls = child.getAttribute('class') || '';
      if (DROP_TAGS.has(tag) || DROP_CLASSES.test(cls) || child.id === 'toc') { child.remove(); continue; }
      if (!KEEP_TAGS.has(tag)) { child.replaceWith(...[...child.childNodes]); continue; }
      walk(child);
      tidy(child, tag, cls);
    }
  };
  const tidy = (el: Element, tag: string, cls: string) => {
    const keep: Record<string, string> = {};
    const get = (n: string) => el.getAttribute(n);
    // Only the wiki's own words for what a box is (its classes stay, so app.css can draw infoboxes, tables, tabs).
    // Under a w- of their own: a wiki's box can't take on the app's own look (an overlay, a button) by naming it.
    if (cls) keep.class = cls.split(/\s+/).filter(c => /^[\w-]{1,60}$/.test(c)).slice(0, 8).map(c => `w-${c}`).join(' ');
    if (get('id') && get('id')!.length <= 200) keep.id = `w-${get('id')}`;
    for (const n of ['colspan', 'rowspan', 'title', 'alt', 'scope', 'data-source'])
      if (get(n) && get(n)!.length <= 200) keep[n] = get(n)!;
    if (tag === 'img') {
      const src = imgOk(get('data-src') || get('src') || '');
      if (!src || src.startsWith('data:')) { el.remove(); return; }
      keep.src = src;
      keep.loading = 'lazy';
      keep.referrerpolicy = 'no-referrer';
      for (const n of ['width', 'height']) if (/^\d{1,4}$/.test(get(n) || '')) keep[n] = get(n)!;
    }
    if (tag === 'a') {
      const link = linkOf(get('href') || '', site);
      if (link) keep['data-wk'] = JSON.stringify(link);
      keep.href = '#';
    }
    for (const a of [...el.attributes]) el.removeAttribute(a.name);
    for (const [k, v] of Object.entries(keep)) el.setAttribute(k, v);
  };
  walk(root);
  // Contents, as a wiki puts it: after the first words, before the first heading.
  if (toc) {
    const box = doc.createElement('div');
    box.className = 'ws-toc';
    const b = doc.createElement('b');
    b.textContent = toc.title;
    const ol = doc.createElement('ol');
    for (const s of toc.sections.filter(x => x.level === 1)) {
      const li = doc.createElement('li'), a = doc.createElement('a');
      a.href = '#';
      a.setAttribute('data-wk', JSON.stringify({ kind: 'anchor', anchor: s.anchor }));
      a.textContent = s.line;
      li.append(a);
      const subs = toc.sections.filter(x => x.level === 2 && x.number.startsWith(s.number + '.'));
      if (subs.length) {
        const sub = doc.createElement('ol');
        for (const x of subs) {
          const li2 = doc.createElement('li'), a2 = doc.createElement('a');
          a2.href = '#';
          a2.setAttribute('data-wk', JSON.stringify({ kind: 'anchor', anchor: x.anchor }));
          a2.textContent = x.line;
          li2.append(a2);
          sub.append(li2);
        }
        li.append(sub);
      }
      ol.append(li);
    }
    box.append(b, ol);
    // The first heading of the words, not one inside a box of facts (its title is an h2 too).
    const first = [...root.querySelectorAll('h2')].find(h => !h.closest('aside, table, .w-portable-infobox, .w-infobox'));
    const at = first?.closest('.w-mw-heading2') || first;
    if (at?.parentElement) at.parentElement.insertBefore(box, at);
    else root.append(box);
  }
  return root.innerHTML;
}

/** What a page's link does, by where it points. */
export function linkOf(href: string, site: string): Link | null {
  if (!href) return null;
  if (href.startsWith('#')) return { kind: 'anchor', anchor: decodeURIComponent(href.slice(1)) };
  let u: URL;
  try { u = new URL(href, site + '/'); } catch { return null; }
  if (u.protocol !== 'https:' && u.protocol !== 'http:') return null;
  // A picture's own file: nothing to open.
  if (u.host === 'static.wikia.nocookie.net' || /\/images\//.test(u.pathname)) return null;
  const base = new URL(site);
  const lang = base.pathname.replace(/\/$/, '');
  if (u.host === base.host && u.pathname.startsWith(`${lang}/wiki/`)) {
    const title = decodeURIComponent(u.pathname.slice(lang.length + 6)).replace(/_/g, ' ');
    // Its own pages open here; files, users, specials and talk open on the wiki.
    if (title && !/^(File|Image|User|User talk|Special|Talk|Message Wall|Board|Thread|MediaWiki|Template|Module|Help|Forum|Blog|User blog)( talk)?:/i.test(title) && !u.search)
      return { kind: 'page', title, anchor: u.hash ? decodeURIComponent(u.hash.slice(1)) : null };
  }
  return { kind: 'out', url: u.href.replace(/^http:/, 'https:') };
}
