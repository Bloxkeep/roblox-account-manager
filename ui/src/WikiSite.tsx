// A game's whole wiki inside its Wiki part (GamesView.tsx), read from this PC (AppController.Wiki.cs, WikiNet.cs): any
// wiki on Fandom or wiki.gg, so a game needs nothing of its own here, only its wiki, named by Bloxkeep's bot (wiki.js
// POPULAR_GAMES; never looked for or pasted by the user, the owner's call on 2026-11-06). Every article reads here, drawn as a wiki is (app.css .ws) in the game's colour; its
// links open its other pages here; the whole wiki is searched as it is typed. What the wiki itself can't do sits on top:
// each kind of thing it has a box of facts for (weapons, pets, fish…) as a table to filter, sort and compare (WikiView.tsx),
// the items most like the one open, and pages saved and seen lately without signing in anywhere.
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { Icon, useAppState, useHost } from './host';
import { boxChanges, clean, cleanPage, kindName, linkOf, wikiItem, type BoxChange, type Link, type WikiItem } from './wikiText';
import { DropOdds, ItemTable, Similar, fold } from './WikiView';
import { timeText } from './DiscordBlocks';

export type WikiPick = { id: string; name: string; site: string; place: number; img: string | null; added: boolean };
export type WikiMark = { site: string; title: string; game: string; at: number };
type About = { name: string; articles: number; mainPage: string; logo: string | null; rights: { name: string; url: string } | null };
type Kind = { template: string; name: string; count: number; pic?: string | null };
type Section = { line: string; anchor: string; level: number; number: string };
type Parsed = { title: string; html: string; sections: Section[]; cats: string[]; missing?: boolean };
type View = { kind: 'home' } | { kind: 'page'; title: string; anchor?: string | null; table?: string } | { kind: 'table'; template: string }
  | { kind: 'search'; q: string } | { kind: 'saved' };
type Api = (q: Record<string, string | number>) => Promise<any>;
type T = (k: string, v?: Record<string, string | number>) => string;

/** A kind needs this many pages to be a table; at most this many kinds; a table reads this many pages at most. */
const KIND_MIN = 8, KINDS_MAX = 14, ITEMS_MAX = 600;
/** Kept while the app runs, so going back to a wiki asks nothing again. */
const abouts = new Map<string, About>();
const kindsOf = new Map<string, Kind[]>();
const tables = new Map<string, { items: WikiItem[]; done: boolean }>();
const pages = new Map<string, Parsed>();
/** What's new on each wiki (Fresh), asked once while the app runs. */
const freshOf = new Map<string, Fresh>();
type FreshPage = { title: string; at: number; edits: number; pic: string | null };
type Fresh = { added: FreshPage[]; hot: FreshPage[]; edited: FreshPage[] };
const FRESH_DAYS = 14;
/** The numbers each wiki's pages changed in that time (Changed), asked once while the app runs. */
const changesOf = new Map<string, Changed[]>();
type Changed = { title: string; pic: string | null; lines: BoxChange[] };
/** Pages read for changed numbers: the ones edited most that the wiki had before. */
const CHANGED_PAGES = 30;

/**
 * The wiki's last two weeks: the pages added (a game's update shows up here first: on 2026-11-06 Blox Fruits' had
 * "Updates/30.1" and its Halloween items, Deepwoken's the Harvest event) and the pages edited most, as players fill
 * them in. Bots and redirects left out.
 */
async function loadFresh(api: Api): Promise<Fresh> {
  const since = new Date(Date.now() - FRESH_DAYS * 864e5).toISOString();
  const [made, edits] = await Promise.all([
    api({ action: 'query', list: 'recentchanges', rcnamespace: 0, rctype: 'new', rcshow: '!bot|!redirect', rclimit: 40, rcprop: 'title|timestamp', rcend: since }),
    api({ action: 'query', list: 'recentchanges', rcnamespace: 0, rctype: 'edit', rcshow: '!bot', rclimit: 500, rcprop: 'title|timestamp', rcend: since }),
  ]);
  const added: FreshPage[] = [], seen = new Set<string>();
  for (const c of made?.query?.recentchanges || [])
    if (typeof c.title === 'string' && !seen.has(c.title)) { seen.add(c.title); added.push({ title: c.title, at: Date.parse(c.timestamp) || 0, edits: 0, pic: null }); }
  const count = new Map<string, FreshPage>();
  for (const c of edits?.query?.recentchanges || []) {
    if (typeof c.title !== 'string') continue;
    const had = count.get(c.title);
    if (had) had.edits++;
    else count.set(c.title, { title: c.title, at: Date.parse(c.timestamp) || 0, edits: 1, pic: null });
  }
  const hot = [...count.values()].filter(p => p.edits >= 3 && p.title !== 'Main Page').sort((a, b) => b.edits - a.edits).slice(0, 8);
  const wanted = [...new Set([...added.slice(0, 16), ...hot].map(p => p.title))];
  if (wanted.length) {
    const d = await api({ action: 'query', titles: wanted.slice(0, 50).join('|'), prop: 'pageimages', piprop: 'thumbnail', pithumbsize: 160 }).catch(() => null);
    const pic = new Map<string, string>();
    for (const p of d?.query?.pages || [])
      if (typeof p.title === 'string' && typeof p.thumbnail?.source === 'string' && WIKI_IMG.test(p.thumbnail.source))
        pic.set(p.title, p.thumbnail.source);
    for (const p of [...added, ...hot]) p.pic = pic.get(p.title) || null;
  }
  const edited = [...count.values()].filter(p => !seen.has(p.title) && p.title !== 'Main Page').sort((a, b) => b.edits - a.edits)
    .slice(0, CHANGED_PAGES);
  return { added: added.slice(0, 16), hot, edited };
}

/**
 * "Buffs and nerfs": the numbers in the boxes of the pages edited lately, then and now. The wiki shows only its raw edits,
 * line by line in markup; this is what changed for a player (on 2026-11-06 GPO's True Ba'al drops went from 1% to 5%,
 * Deepwoken's Hivelord's Hubris range from 10 to 9.5). Each page's text from before is one ask; four at a time.
 */
async function loadChanges(api: Api, edited: FreshPage[]): Promise<Changed[]> {
  if (!edited.length) return [];
  const since = new Date(Date.now() - FRESH_DAYS * 864e5).toISOString();
  const text = (p: any) => p?.revisions?.[0]?.slots?.main?.content;
  const d = await api({ action: 'query', titles: edited.map(p => p.title).join('|'), prop: 'revisions|pageimages', rvprop: 'content', rvslots: 'main',
    piprop: 'thumbnail', pithumbsize: 160 }).catch(() => null);
  const now = new Map<string, { text: string; pic: string | null }>();
  for (const p of d?.query?.pages || [])
    if (typeof p.title === 'string' && typeof text(p) === 'string')
      now.set(p.title, { text: text(p), pic: typeof p.thumbnail?.source === 'string' && WIKI_IMG.test(p.thumbnail.source) ? p.thumbnail.source : null });
  const out: Changed[] = [];
  const todo = edited.filter(p => now.has(p.title));
  const next = async () => {
    for (let p = todo.shift(); p; p = todo.shift()) {
      const o = await api({ action: 'query', titles: p.title, prop: 'revisions', rvprop: 'content', rvslots: 'main', rvlimit: 1, rvstart: since,
        rvdir: 'older' }).catch(() => null);
      const old = text(o?.query?.pages?.[0]);
      if (typeof old !== 'string') continue;
      const lines = boxChanges(old, now.get(p.title)!.text);
      if (lines.length) out.push({ title: p.title, pic: now.get(p.title)!.pic, lines });
    }
  };
  await Promise.all([next(), next(), next(), next()]);
  const rank = new Map(edited.map((p, i) => [p.title, i]));
  return out.sort((a, b) => rank.get(a.title)! - rank.get(b.title)!);
}

/** The wiki home's "New on the wiki": the pages added lately as pictures in a row, the pages edited most as a ranking. */
function FreshPart({ api, site, t, onGo }: { api: Api; site: string; t: T; onGo: (title: string) => void }) {
  const { locale } = useHost();
  const [fresh, setFresh] = useState<Fresh | null>(freshOf.get(site) || null);
  const [changes, setChanges] = useState<Changed[] | null>(changesOf.get(site) || null);
  useEffect(() => {
    let alive = true;
    const more = (f: Fresh) => changesOf.has(site) ? null
      : loadChanges(api, f.edited).then(c => { changesOf.set(site, c); if (alive) setChanges(c); }, () => {});
    if (fresh) more(fresh);
    else loadFresh(api).then(f => { freshOf.set(site, f); if (alive) setFresh(f); more(f); }, () => {});
    return () => { alive = false; };
  }, [site]);
  if (!fresh || (!fresh.added.length && !fresh.hot.length)) return null;
  const now = Date.now(), most = fresh.hot[0]?.edits || 1;
  return <section className="ws-fresh">
    <h2 className="ws-h2">{t('wiki.fresh')}</h2>
    <p className="ws-dim">{t('wiki.freshText', { n: FRESH_DAYS })}</p>
    {fresh.added.length ? <div className="ws-fresh-row" role="list" aria-label={t('wiki.freshNew')}>
      {fresh.added.map(p => <button key={p.title} type="button" role="listitem" className="ws-fresh-card" data-wk={wk(p.title)} onClick={() => onGo(p.title)}>
        <span className="ws-fresh-pic">{p.pic ? <img src={p.pic} alt="" loading="lazy" referrerPolicy="no-referrer" /> : <span>{p.title.slice(0, 1)}</span>}</span>
        <b>{/^[\d.]+$/.test(p.title.split('/').pop() || '') ? p.title.replace(/\//g, ' ') : p.title.split('/').pop()}</b>
        <small><em>{t('wiki.freshNewOne')}</em> {timeText(p.at, 'R', locale, now)}</small>
      </button>)}
    </div> : null}
    {changes?.length ? <>
      <h3 className="ws-h3">{t('wiki.changed')}</h3>
      <p className="ws-dim">{t('wiki.changedText', { n: FRESH_DAYS })}</p>
      <div className="ws-changed">{changes.map(c => <button key={c.title} type="button" className="ws-changed-item" data-wk={wk(c.title)} onClick={() => onGo(c.title)}>
        {c.pic ? <img src={c.pic} alt="" loading="lazy" referrerPolicy="no-referrer" /> : <span className="wk-img-none">{c.title.slice(0, 1)}</span>}
        <span className="ws-changed-body">
          <b>{c.title.split('/').pop()}</b>
          {c.lines.map(l => <span key={l.field} className="ws-changed-line">
            <span className="ws-changed-field">{l.field}</span>
            <s>{l.was}</s>
            <span aria-hidden="true">→</span>
            <em className={l.up ? 'up' : 'down'}>{l.now}<Icon name={l.up ? 'chevronUp' : 'chevronDown'} /></em>
          </span>)}
        </span>
      </button>)}</div>
    </> : null}
    {fresh.hot.length ? <>
      <h3 className="ws-h3">{t('wiki.freshHot')}</h3>
      <ol className="ws-hot">{fresh.hot.map(p => <li key={p.title}>
        <button type="button" className="ws-hot-item" data-wk={wk(p.title)} onClick={() => onGo(p.title)}>
          {p.pic ? <img src={p.pic} alt="" loading="lazy" referrerPolicy="no-referrer" /> : <span className="wk-img-none">{p.title.slice(0, 1)}</span>}
          <span className="ws-hot-name">{p.title}</span>
          <span className="ws-hot-bar"><i style={{ '--f': p.edits / most } as React.CSSProperties} /></span>
          <small>{t('wiki.editsN', { n: p.edits })}</small>
        </button></li>)}</ol>
    </> : null}
  </section>;
}

/** Each wiki's short codes in its own words ({{Rarity|L}} → Legendary), asked once while the app runs (null: no words). */
const said = new Map<string, string | null>();
const SAY_SPLIT = '\n@@BK@@\n';

/** The lines a wiki writes as a short code (wikiText.ts codes) put in the wiki's own words, as its pages show them:
 * the templates asked of the wiki a few at a time (2026-11-06: Grand Piece Online's rarity read "L" and "LC" in Compare). */
async function sayCodes(api: Api, site: string, items: WikiItem[]) {
  const raws = [...new Set(items.flatMap(i => Object.values(i.codes || {})))].filter(r => !said.has(`${site}|${r}`));
  for (let i = 0; i < raws.length;) {
    const part: string[] = [];
    while (i < raws.length && [...part, raws[i]].join(SAY_SPLIT).length <= 1800) part.push(raws[i++]);
    if (!part.length) { said.set(`${site}|${raws[i++]}`, null); continue; }
    const d = await api({ action: 'expandtemplates', prop: 'wikitext', text: part.join(SAY_SPLIT) }).catch(() => null);
    const out = typeof d?.expandtemplates?.wikitext === 'string' ? d.expandtemplates.wikitext.split(SAY_SPLIT.trim()) : [];
    part.forEach((raw, n) => {
      const words = out.length === part.length ? clean(out[n]) : '';
      said.set(`${site}|${raw}`, words && words.length <= 40 && !words.includes('\n') ? words : null);
    });
  }
  for (const item of items)
    for (const [field, raw] of Object.entries(item.codes || {})) {
      const words = said.get(`${site}|${raw}`);
      if (words) item.fields = item.fields.map(([k, v]) => k === field ? [k, words] : [k, v]);
    }
}

function useApi(site: string): Api {
  const { call } = useHost();
  return q => call('wikiApi', { site, q }, { silent: true }) as Promise<any>;
}

/** The wiki's name, size, main page, logo and licence. */
async function loadAbout(api: Api): Promise<About | null> {
  const d = await api({ action: 'query', meta: 'siteinfo', siprop: 'general|statistics|rightsinfo' });
  const g = d?.query?.general;
  if (!g) return null;
  const logo = typeof g.logo === 'string' ? (g.logo.startsWith('//') ? 'https:' + g.logo : g.logo) : null;
  const r = d.query.rightsinfo;
  return {
    name: String(g.sitename || ''), articles: Number(d.query.statistics?.articles) || 0, mainPage: String(g.mainpage || 'Main Page'),
    logo: logo && /^https:\/\/(static\.wikia\.nocookie\.net|[a-z0-9-]+\.wiki\.gg)\//.test(logo) ? logo : null,
    rights: r?.text && typeof r.url === 'string' && r.url.startsWith('https://') ? { name: String(r.text), url: r.url } : null,
  };
}

/** The kinds of thing the wiki keeps a box of facts for: its infobox templates, by how many pages use each. */
const WIKI_IMG = /^https:\/\/(static\.wikia\.nocookie\.net|[a-z0-9-]+\.wiki\.gg)\//;
async function loadKinds(api: Api): Promise<Kind[]> {
  const [boxes, used] = await Promise.all([
    api({ action: 'query', list: 'allinfoboxes' }),
    api({ action: 'query', list: 'querypage', qppage: 'Mostlinkedtemplates', qplimit: 500 }),
  ]);
  const counts = new Map<string, number>();
  for (const r of used?.query?.querypage?.results || [])
    if (typeof r.title === 'string' && r.title.startsWith('Template:')) counts.set(fold(r.title.slice(9).replace(/_/g, ' ')), Number(r.value) || 0);
  const all: Kind[] = [];
  for (const b of boxes?.query?.allinfoboxes || []) {
    const title = String(b.title || '').replace(/_/g, ' ');
    if (!title || /user|test|sandbox|tabber|navbox|doc$|^\.|\//i.test(title)) continue;
    const n = counts.get(fold(title)) || 0;
    if (n >= KIND_MIN) all.push({ template: `Template:${title}`, name: kindName(title), count: n });
  }
  // Those counts are of every page using a box, players' own pages too ("About me"): a kind is kept only when enough
  // articles use it, asked of the biggest few at once. The same answer gives the kind its picture: the first of its
  // pages that has one (a sword for Weapon, a face for NPC), so the tiles show what's inside.
  const top = all.sort((a, b) => b.count - a.count).slice(0, KINDS_MAX + 8);
  const inArticles = await Promise.all(top.map(k => api({ action: 'query', generator: 'embeddedin', geititle: k.template, geinamespace: 0, geilimit: 50,
    prop: 'pageimages', piprop: 'thumbnail', pithumbsize: 160, pilimit: 50 })
    .then(d => {
      const pages: any[] = d?.query?.pages || [];
      const pic = pages.map(p => p?.thumbnail?.source).find(u => typeof u === 'string' && WIKI_IMG.test(u)) as string | undefined;
      return { n: pages.length, pic: pic || null };
    }, () => ({ n: 0, pic: null }))));
  const kinds: Kind[] = [], names = new Set<string>();
  top.forEach((k, n) => {
    const { n: real, pic } = inArticles[n];
    if (real < KIND_MIN || names.has(fold(k.name))) return;
    names.add(fold(k.name));
    kinds.push({ ...k, count: real < 50 ? real : k.count, pic });
  });
  return kinds.sort((a, b) => b.count - a.count).slice(0, KINDS_MAX);
}

/** A page, parsed by the wiki and made safe to draw; a category is its pages. */
async function loadPage(api: Api, site: string, title: string, t: T): Promise<Parsed | null> {
  const key = `${site}|${title}`;
  if (pages.has(key)) return pages.get(key)!;
  if (/^Category:/i.test(title)) {
    const d = await api({ action: 'query', list: 'categorymembers', cmtitle: title, cmlimit: 500, cmnamespace: '0|14', cmprop: 'title' });
    if (!d?.query) return null;
    const list = (d.query.categorymembers || []).map((m: { title: string }) => m.title as string);
    const doc = document.implementation.createHTMLDocument('');
    const ul = doc.createElement('ul');
    ul.className = 'ws-list';
    for (const name of list) {
      const li = doc.createElement('li'), a = doc.createElement('a');
      a.href = '#';
      a.setAttribute('data-wk', JSON.stringify({ kind: 'page', title: name, anchor: null }));
      a.textContent = name.replace(/^Category:/, '');
      li.append(a);
      ul.append(li);
    }
    const page: Parsed = { title, html: list.length ? ul.outerHTML : `<p class="ws-nothing">${t('wiki.nothing')}</p>`, sections: [], cats: [] };
    pages.set(key, page);
    return page;
  }
  const d = await api({ action: 'parse', page: title, prop: 'text|sections|categories|displaytitle', redirects: 1, disableeditsection: 1, disablelimitreport: 1 });
  if (d?.error) return { title, html: '', sections: [], cats: [], missing: true };
  const p = d?.parse;
  if (!p || typeof p.text !== 'string') return null;
  const sections: Section[] = (p.sections || []).filter((s: any) => s.toclevel <= 2 && s.anchor)
    .map((s: any) => ({ line: textOf(String(s.line)), anchor: String(s.anchor), level: Number(s.toclevel) || 1, number: String(s.number || '') }));
  const page: Parsed = {
    title: String(p.title || title), html: cleanPage(p.text, site, sections.length >= 3 ? { title: t('wiki.toc'), sections } : null), sections,
    cats: (p.categories || []).filter((c: any) => !c.hidden).map((c: any) => String(c.category).replace(/_/g, ' ')).slice(0, 30),
  };
  if (pages.size > 40) pages.delete(pages.keys().next().value!);
  pages.set(key, page);
  return page;
}

/** HTML's words alone. */
const textOf = (html: string) => new DOMParser().parseFromString(html, 'text/html').body.textContent || '';

/** A game's wiki: its home (tables, saved, seen), its articles, its search. */
export function WikiSite({ pick, gameId, shown }: { pick: WikiPick; gameId: string; shown: boolean }) {
  const { t, call, locale } = useHost();
  const app = useAppState();
  const site = pick.site;
  const api = useApi(site);
  const peek = usePeek(api, site);
  useEffect(() => { if (!shown) peek.hide(); }, [shown]);
  const [about, setAbout] = useState<About | null>(abouts.get(site) || null);
  const [kinds, setKinds] = useState<Kind[] | null>(kindsOf.get(site) || null);
  const [failed, setFailed] = useState(false);
  const [views, setViews] = useState<View[]>([{ kind: 'home' }]);
  const view = views[views.length - 1];
  const root = useRef<HTMLDivElement>(null);
  const scrollAt = useRef<number[]>([]);
  const scroller = () => root.current?.closest('.scroll') as HTMLElement | null;

  useEffect(() => {
    if (!shown || about) return;
    setFailed(false);
    loadAbout(api).then(a => { if (a) { abouts.set(site, a); setAbout(a); } else setFailed(true); });
  }, [shown, site, about]);
  useEffect(() => {
    if (!shown || !about || kinds) return;
    loadKinds(api).then(k => { kindsOf.set(site, k); setKinds(k); }, () => setKinds([]));
  }, [shown, site, about, kinds]);

  const go = (v: View) => {
    scrollAt.current.push(scroller()?.scrollTop || 0);
    setViews(vs => [...vs.slice(-30), v]);
    requestAnimationFrame(() => scroller()?.scrollTo({ top: 0 }));
  };
  const back = () => {
    if (views.length < 2) return;
    const top = scrollAt.current.pop() || 0;
    setViews(vs => vs.slice(0, -1));
    requestAnimationFrame(() => requestAnimationFrame(() => scroller()?.scrollTo({ top })));
  };
  const home = () => { scrollAt.current = []; setViews([{ kind: 'home' }]); scroller()?.scrollTo({ top: 0 }); };
  useEffect(() => {
    if (!shown) return;
    const key = (e: KeyboardEvent) => {
      const typing = e.target instanceof HTMLElement && e.target.closest('input, textarea');
      if (e.key === 'Escape' && !typing && !e.defaultPrevented && views.length > 1) back();
    };
    addEventListener('keydown', key);
    return () => removeEventListener('keydown', key);
  }, [shown, views.length]);

  const open = (url: string) => { call('wikiOpen', { url }); };
  const saved: WikiMark[] = (app.wikis?.saved || []).filter((m: WikiMark) => m.site === site);
  const recent: WikiMark[] = (app.wikis?.recent || []).filter((m: WikiMark) => m.site === site).slice(0, 12);
  const pageUrl = (title: string) => `${site}/wiki/${encodeURIComponent(title.replace(/ /g, '_'))}`;

  if (!about) return shown ? <div className="ws"><div className="ws-body">
    <p className="ws-nothing">{failed ? t('wiki.offline') : t('news.connecting')}</p>
    {failed ? <p><button type="button" className="ws-link" onClick={() => { setAbout(null); setFailed(false); abouts.delete(site); loadAbout(api).then(a => a ? (abouts.set(site, a), setAbout(a)) : setFailed(true)); }}>{t('wiki.retry')}</button></p> : null}
  </div></div> : null;

  return <div className="ws" ref={root} onMouseOver={peek.over} onMouseOut={peek.out} onClickCapture={peek.hide}>
    {peek.node}
    <div className="ws-top">
      <button type="button" className="ws-site" onClick={home} title={t('wiki.home')}>
        {about.logo ? <img src={about.logo} alt="" referrerPolicy="no-referrer" /> : <Icon name="book" />}<span>{about.name}</span></button>
      <WikiSearch api={api} t={t} onOpen={title => go({ kind: 'page', title })} onSearch={q => go({ kind: 'search', q })} />
    </div>
    <nav className="ws-nav" aria-label={about.name}>
      <button type="button" className={view.kind === 'home' ? 'on' : ''} onClick={home}>{t('wiki.home')}</button>
      {(kinds || []).slice(0, 8).map(k => <button key={k.template} type="button" className={view.kind === 'table' && view.template === k.template ? 'on' : ''}
        onClick={() => go({ kind: 'table', template: k.template })}>{k.name}</button>)}
      <button type="button" className={view.kind === 'saved' ? 'on' : ''} onClick={() => go({ kind: 'saved' })}>
        <Icon name="bookmark" />{t('wiki.saved')}{saved.length ? ` (${saved.length})` : ''}</button>
    </nav>
    {view.kind === 'home' ? <div className="ws-body">
      <h1 className="ws-h1">{about.name}</h1>
      <p className="ws-tagline">{t('wiki.articles', { n: about.articles.toLocaleString(locale) })}
        {' · '}<button type="button" className="ws-link" onClick={() => go({ kind: 'page', title: about.mainPage })}>{t('wiki.mainPage')}</button>
        {' · '}<button type="button" className="ws-link" onClick={() => open(site)}>{t('wiki.openPage')}<Icon name="external" /></button></p>
      <FreshPart api={api} site={site} t={t} onGo={title => go({ kind: 'page', title })} />
      {kinds === null ? <p className="ws-dim">{t('news.connecting')}</p> : kinds.length ? <section>
        <h2 className="ws-h2">{t('wiki.tables')}</h2>
        <p className="ws-dim">{t('wiki.tablesText')}</p>
        <div className="ws-kinds">{kinds.map(k => <button key={k.template} type="button" className="ws-kind" onClick={() => go({ kind: 'table', template: k.template })}>
          {k.pic ? <span className="ws-kind-pic"><img src={k.pic} alt="" referrerPolicy="no-referrer" /></span> : <Icon name="grid" />}<b>{k.name}</b><small>{t('wiki.pagesN', { n: k.count.toLocaleString(locale) })}</small></button>)}</div>
      </section> : null}
      {recent.length ? <section><h2 className="ws-h2">{t('wiki.recent')}</h2>
        <ul className="ws-list">{recent.map(m => <li key={m.title}><button type="button" className="ws-link" data-wk={wk(m.title)} onClick={() => go({ kind: 'page', title: m.title })}>{m.title}</button></li>)}</ul>
      </section> : null}
      {saved.length ? <section><h2 className="ws-h2">{t('wiki.saved')}</h2>
        <ul className="ws-list">{saved.slice(0, 12).map(m => <li key={m.title}><button type="button" className="ws-link" data-wk={wk(m.title)} onClick={() => go({ kind: 'page', title: m.title })}>{m.title}</button></li>)}</ul>
      </section> : null}
      <Credit about={about} site={site} t={t} open={open} />
    </div>
    : view.kind === 'saved' ? <div className="ws-body">
      <h1 className="ws-h1">{t('wiki.saved')}</h1>
      <p className="ws-tagline">{t('wiki.savedText')}</p>
      {saved.length ? <ul className="ws-list">{saved.map(m => <li key={m.title}>
        <button type="button" className="ws-link" data-wk={wk(m.title)} onClick={() => go({ kind: 'page', title: m.title })}>{m.title}</button>
        {' '}<button type="button" className="ws-link ws-dim" onClick={() => call('wikiMark', { list: 'saved', site, title: m.title, game: gameId, on: false }, { silent: true })}>{t('wiki.unsave')}</button></li>)}</ul>
        : <p className="ws-nothing">{t('wiki.savedNone')}</p>}
    </div>
    : view.kind === 'search' ? <SearchResults key={view.q} api={api} q={view.q} t={t} onOpen={title => go({ kind: 'page', title })} />
    : view.kind === 'table' ? <KindTable key={view.template} api={api} site={site} about={about} kind={(kinds || []).find(k => k.template === view.template)}
      template={view.template} t={t} onOpen={i => go({ kind: 'page', title: i.url ? titleOfUrl(i.url, site) || i.name : i.name, table: view.template })} />
    : <Article key={view.title} api={api} site={site} about={about} title={view.title} anchor={view.anchor || null} table={view.table || null}
      gameId={gameId} saved={saved.some(m => m.title === view.title)} t={t} open={open} pageUrl={pageUrl}
      canBack={views.length > 1} onBack={back} onGo={(title, anchor, table) => go({ kind: 'page', title, anchor, table })} />}
  </div>;
}

/** A page's title from its address on the wiki. */
function titleOfUrl(url: string, site: string) {
  const l = linkOf(url, site);
  return l?.kind === 'page' ? l.title : null;
}

function Credit({ about, site, t, open }: { about: About; site: string; t: T; open: (url: string) => void }) {
  return <p className="pl-credit wk-credit">{t('wiki.credit', { site: about.name })} <button type="button" className="pl-link" onClick={() => open(site)}>{site.replace(/^https:\/\//, '')}</button>
    {about.rights ? <> · <button type="button" className="pl-link" onClick={() => open(about.rights!.url)}>{about.rights.name}</button></> : null}</p>;
}

/** The search box: the wiki's pages named like what is typed, as it is typed; Enter searches every page's words. */
function WikiSearch({ api, t, onOpen, onSearch }: { api: Api; t: T; onOpen: (title: string) => void; onSearch: (q: string) => void }) {
  const [q, setQ] = useState('');
  const [hits, setHits] = useState<string[]>([]);
  const [at, setAt] = useState(-1);
  const [open, setOpen] = useState(false);
  useEffect(() => {
    const words = q.trim();
    if (words.length < 2) { setHits([]); return; }
    let alive = true;
    const timer = setTimeout(() => {
      api({ action: 'query', list: 'prefixsearch', pssearch: words, pslimit: 8, psnamespace: 0 })
        .then(d => { if (alive) { setHits((d?.query?.prefixsearch || []).map((p: { title: string }) => p.title)); setAt(-1); } });
    }, 220);
    return () => { alive = false; clearTimeout(timer); };
  }, [q]);
  const pick = (title: string) => { onOpen(title); setQ(''); setHits([]); setOpen(false); };
  return <div className="ws-search-wrap">
    <label className="ws-search"><Icon name="search" />
      <input type="search" value={q} placeholder={t('wiki.searchWiki')} aria-label={t('wiki.searchWiki')} maxLength={100} autoComplete="off" spellCheck={false}
        role="combobox" aria-expanded={open && hits.length > 0} aria-controls="ws-suggest" id="ws-q"
        onChange={e => { setQ(e.target.value); setOpen(true); }} onFocus={() => setOpen(true)} onBlur={() => setTimeout(() => setOpen(false), 150)}
        onKeyDown={e => {
          if (e.key === 'ArrowDown') { e.preventDefault(); setAt(n => Math.min(hits.length - 1, n + 1)); }
          else if (e.key === 'ArrowUp') { e.preventDefault(); setAt(n => Math.max(-1, n - 1)); }
          else if (e.key === 'Enter' && q.trim()) { if (at >= 0 && hits[at]) pick(hits[at]); else { onSearch(q.trim()); setOpen(false); } }
          else if (e.key === 'Escape') { e.preventDefault(); setOpen(false); }
        }} /></label>
    {open && hits.length ? <ul className="ws-suggest" id="ws-suggest" role="listbox">{hits.map((h, n) => <li key={h} role="option" aria-selected={n === at}>
      <button type="button" className={n === at ? 'on' : ''} onMouseDown={e => e.preventDefault()} onClick={() => pick(h)}>{h}</button></li>)}
      <li><button type="button" className="ws-suggest-all" onMouseDown={e => e.preventDefault()} onClick={() => { onSearch(q.trim()); setOpen(false); }}>
        <Icon name="search" />{t('wiki.searchAll', { q: q.trim() })}</button></li></ul> : null}
  </div>;
}

/** Every page with the words in it. */
function SearchResults({ api, q, t, onOpen }: { api: Api; q: string; t: T; onOpen: (title: string) => void }) {
  const [hits, setHits] = useState<{ title: string; text: string }[] | null>(null);
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    api({ action: 'query', list: 'search', srsearch: q, srlimit: 40, srnamespace: 0, srprop: 'snippet' }).then(d => {
      if (!d?.query) { setFailed(true); return; }
      setHits((d.query.search || []).map((s: { title: string; snippet?: string }) => ({ title: s.title, text: textOf(s.snippet || '') })));
    });
  }, [q]);
  return <div className="ws-body">
    <h1 className="ws-h1">{t('wiki.resultsFor', { q })}</h1>
    {!hits ? <p className="ws-dim">{failed ? t('wiki.offline') : t('news.connecting')}</p>
      : hits.length ? <ul className="ws-results">{hits.map(h => <li key={h.title}>
        <button type="button" className="ws-link" data-wk={wk(h.title)} onClick={() => onOpen(h.title)}>{h.title}</button>{h.text ? <p>{h.text}</p> : null}</li>)}</ul>
      : <p className="ws-nothing">{t('wiki.nothing')}</p>}
  </div>;
}

/** A kind's pages as a table, read 50 at a time and shown as they come. */
function KindTable({ api, site, about, kind, template, t, onOpen }: {
  api: Api; site: string; about: About; kind?: Kind; template: string; t: T; onOpen: (i: WikiItem) => void;
}) {
  const { locale } = useHost();
  const key = `${site}|${template}`;
  const [state, setState] = useState(tables.get(key) || { items: [] as WikiItem[], done: false });
  const [failed, setFailed] = useState(false);
  const name = kind?.name || kindName(template);
  useEffect(() => {
    if (tables.get(key)?.done) return;
    let alive = true;
    (async () => {
      const seen = new Set<string>(), items: WikiItem[] = [];
      let cont: Record<string, string> | null = {};
      while (alive && cont && items.length < ITEMS_MAX) {
        const d = await api({
          action: 'query', generator: 'embeddedin', geititle: template, geinamespace: 0, geilimit: 50, prop: 'revisions|pageimages|info', rvprop: 'content',
          rvslots: 'main', piprop: 'thumbnail', pithumbsize: 160, inprop: 'url', redirects: 1, ...cont,
        });
        if (!d || d.error) { if (alive) setFailed(true); return; }
        for (const p of d.query?.pages || []) {
          const item = wikiItem(p, name, template, site);
          if (item && !seen.has(item.id)) { seen.add(item.id); items.push(item); }
        }
        await sayCodes(api, site, items);
        cont = d.continue && typeof d.continue === 'object' ? d.continue : null;
        const now = { items: [...items], done: !cont || items.length >= ITEMS_MAX };
        tables.set(key, now);
        if (alive) setState(now);
      }
    })();
    return () => { alive = false; };
  }, [key]);
  const list = useMemo(() => ({ name, source: { name: about.name, url: site }, items: state.items }), [state.items, name, about.name, site]);
  if (!state.items.length) return <div className="ws-body"><h1 className="ws-h1">{name}</h1>
    <p className="ws-dim">{failed ? t('wiki.offline') : state.done ? t('wiki.nothing') : t('news.connecting')}</p></div>;
  return <ItemTable game={list} loading={state.done ? null : t('wiki.loadingN', { n: state.items.length.toLocaleString(locale) })} onOpen={onOpen} />;
}

/** An article: the wiki's own page, its links opening here, saved with one click; under it, when it was opened from a
 * table, the items of that table most like it. */
function Article({ api, site, about, title, anchor, table, gameId, saved, t, open, pageUrl, canBack, onBack, onGo }: {
  api: Api; site: string; about: About; title: string; anchor: string | null; table: string | null; gameId: string; saved: boolean; t: T;
  open: (url: string) => void; pageUrl: (title: string) => string; canBack: boolean; onBack: () => void;
  onGo: (title: string, anchor?: string | null, table?: string) => void;
}) {
  const { call } = useHost();
  const [page, setPage] = useState<Parsed | null>(pages.get(`${site}|${title}`) || null);
  const [failed, setFailed] = useState(false);
  const body = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (page) return;
    loadPage(api, site, title, t).then(p => p ? setPage(p) : setFailed(true));
  }, [title]);
  useEffect(() => {
    if (!page || page.missing) return;
    call('wikiMark', { list: 'recent', site, title: page.title, game: gameId }, { silent: true });
    if (anchor) requestAnimationFrame(() => jump(anchor));
  }, [page]);
  const jump = (to: string) => {
    const el = body.current?.querySelector(`[id="${CSS.escape('w-' + to)}"]`);
    el?.scrollIntoView({ block: 'start', behavior: 'smooth' });
  };
  // Links in the page: its own pages open here, a place in it is scrolled to, the rest open in the browser.
  const click = (e: React.MouseEvent) => {
    const a = (e.target as HTMLElement).closest('a');
    if (!a) return;
    e.preventDefault();
    let link: Link | null = null;
    try { link = JSON.parse(a.getAttribute('data-wk') || 'null'); } catch { link = null; }
    if (!link) return;
    if (link.kind === 'page') onGo(link.title, link.anchor);
    else if (link.kind === 'anchor') jump(link.anchor);
    else open(link.url);
  };
  const items = table ? tables.get(`${site}|${table}`)?.items || [] : [];
  const named = (i: WikiItem) => fold(i.name) === fold(title.split('/').pop() || title) || fold(i.name) === fold(page?.title || '');
  const item = items.find(named);
  // The item's facts for its drop odds: from its table, else any table of this wiki read already, else its hover card.
  const facts = item || [...tables].filter(([k]) => k.startsWith(site + '|')).map(([, v]) => v.items.find(named)).find(Boolean)
    || peeks.get(`${site}|${fold(title)}`) || boxOf(page?.html);
  const shownTitle = page?.title || title;
  return <article className="ws-body ws-article" aria-label={shownTitle}>
    {canBack ? <nav className="ws-crumb"><button type="button" className="ws-link" id="wk-back" onClick={onBack}><Icon name="chevronLeft" />{t('wiki.back')}</button></nav> : null}
    <h1 className="ws-h1">{shownTitle.replace(/^Category:/, `${t('wiki.cats')}: `)}</h1>
    <p className="ws-tagline">{t('wiki.from', { site: about.name })}
      {page && !page.missing ? <>{' · '}<button type="button" className={`ws-link ${saved ? 'on' : ''}`} id="wk-save"
        onClick={() => call('wikiMark', { list: 'saved', site, title: shownTitle, game: gameId, on: !saved }, { silent: true })}>
        <Icon name={saved ? 'starFill' : 'star'} />{t(saved ? 'wiki.savedOne' : 'wiki.save')}</button></> : null}
      {' · '}<button type="button" className="ws-link" onClick={() => open(pageUrl(shownTitle))}>{t('wiki.openPage')}<Icon name="external" /></button></p>
    {!page ? <p className="ws-dim">{failed ? t('wiki.offline') : t('news.connecting')}</p>
      : page.missing ? <p className="ws-nothing">{t('wiki.noPage')}</p>
      : <>{facts ? <DropOdds item={facts} /> : null}
        <div className="ws-content" ref={body} onClick={click} dangerouslySetInnerHTML={{ __html: page.html }} /></>}
    {page && item ? <Similar item={item} items={items} onOpen={i => onGo(i.url ? titleOfUrl(i.url, site) || i.name : i.name, null, table!)} /> : null}
    {page?.cats.length ? <div className="ws-catlinks"><b>{t('wiki.cats')}:</b> {page.cats.map((c, n) => <span key={c}>{n ? ' | ' : ''}
      <button type="button" className="ws-link" onClick={() => onGo(`Category:${c}`)}>{c}</button></span>)}</div> : null}
    <Credit about={about} site={site} t={t} open={open} />
  </article>;
}

/** The label and value lines of the box of facts in a page's own text, for a page no table or hover card has read. */
const boxes = new Map<string, Pick<WikiItem, 'fields'> | null>();
function boxOf(html: string | undefined) {
  if (!html) return null;
  if (boxes.has(html)) return boxes.get(html)!;
  const doc = new DOMParser().parseFromString(html, 'text/html');
  const fields = [...doc.querySelectorAll('.w-portable-infobox .w-pi-data')].map(d => [
    d.querySelector('.w-pi-data-label')?.textContent?.trim() || '', d.querySelector('.w-pi-data-value')?.textContent?.trim() || '',
  ] as [string, string]).filter(([k, v]) => k && v);
  const box = fields.length ? { fields } : null;
  boxes.set(html, box);
  return box;
}

/** What a link leads to, shown while the pointer rests on it: the page's picture, its first lines and its box of facts,
 * read from the page's own infobox (any wiki, any kind of thing). Kept per wiki once asked. */
const peeks = new Map<string, WikiItem | null>();
/** A page's name as a link the preview knows (the same mark the article's own links carry). */
const wk = (title: string) => JSON.stringify({ kind: 'page', title, anchor: null });
const PEEK_WAIT = 300, PEEK_ROWS = 6;
const PEEK_LOOK = ['--ws-bg', '--ws-box', '--ws-head', '--ws-line', '--ws-text', '--ws-dim', '--ws-link', '--ws-serif', '--gt', '--gt-ink'];
function usePeek(api: Api, site: string) {
  const [shown, setShown] = useState<{ item: WikiItem; at: DOMRect; look: Record<string, string> } | null>(null);
  const timer = useRef(0), over = useRef<string | null>(null);
  const hide = () => { clearTimeout(timer.current); over.current = null; setShown(null); };
  useEffect(() => {
    if (!shown) return;
    // Gone with any scroll or press anywhere (another tab of the app too), and when the window loses the pointer.
    addEventListener('scroll', hide, { capture: true, passive: true });
    addEventListener('mousedown', hide, true);
    document.documentElement.addEventListener('mouseleave', hide);
    return () => {
      removeEventListener('scroll', hide, { capture: true });
      removeEventListener('mousedown', hide, true);
      document.documentElement.removeEventListener('mouseleave', hide);
    };
  }, [shown]);
  useEffect(() => () => clearTimeout(timer.current), []);
  const titleOf = (e: React.MouseEvent) => {
    const a = (e.target as HTMLElement).closest<HTMLElement>('[data-wk]');
    let link: Link | null = null;
    try { link = JSON.parse(a?.getAttribute('data-wk') || 'null'); } catch { link = null; }
    return a && link?.kind === 'page' && !/^Category:/i.test(link.title) ? { a, title: link.title } : null;
  };
  const show = async (a: HTMLElement, title: string) => {
    const key = `${site}|${fold(title)}`;
    let item = peeks.get(key);
    if (item === undefined) {
      // A table already read has it: no asking.
      for (const [k, v] of tables) if (k.startsWith(site + '|')) { const hit = v.items.find(i => fold(i.name) === fold(title)); if (hit) { item = hit; break; } }
      if (item === undefined) {
        const d = await api({ action: 'query', titles: title, prop: 'revisions|pageimages|info', rvprop: 'content', rvslots: 'main', piprop: 'thumbnail',
          pithumbsize: 200, inprop: 'url', redirects: 1 });
        const p = d?.query?.pages?.[0];
        item = p && !p.missing ? wikiItem(p, '', '', site) : null;
        if (item) await sayCodes(api, site, [item]);
        if (d && !d.error) peeks.set(key, item);
      }
    }
    // The link must still be on screen: a part switched away meanwhile leaves it unlaid (a card at the window's corner).
    if (over.current !== title || !item || !a.isConnected || !a.getClientRects().length) return;
    // The card sits on the page's top layer, outside the sheet: it takes the sheet's colours along.
    const sheet = a.closest('.ws'), look: Record<string, string> = {};
    if (sheet) { const cs = getComputedStyle(sheet); for (const v of PEEK_LOOK) look[v] = cs.getPropertyValue(v); }
    setShown({ item, at: a.getBoundingClientRect(), look });
  };
  return {
    hide,
    over: (e: React.MouseEvent) => {
      const hit = titleOf(e);
      if (!hit || over.current === hit.title) return;
      clearTimeout(timer.current);
      over.current = hit.title;
      timer.current = window.setTimeout(() => show(hit.a, hit.title), PEEK_WAIT);
    },
    out: (e: React.MouseEvent) => {
      const hit = titleOf(e);
      if (hit && hit.a.contains(e.relatedTarget as Node)) return;
      hide();
    },
    node: shown ? <PeekCard item={shown.item} at={shown.at} look={shown.look} /> : null,
  };
}

function PeekCard({ item, at, look }: { item: WikiItem; at: DOMRect; look: Record<string, string> }) {
  const card = useRef<HTMLDivElement>(null);
  const [place, setPlace] = useState<{ left: number; top: number } | null>(null);
  useLayoutEffect(() => {
    const c = card.current;
    if (!c) return;
    const w = c.offsetWidth, h = c.offsetHeight;
    const below = at.bottom + 8, above = at.top - 8 - h;
    const top = below + h <= innerHeight - 8 || above < 8 ? Math.min(below, innerHeight - 8 - h) : above;
    setPlace({ left: Math.max(8, Math.min(at.left, innerWidth - 8 - w)), top: Math.max(8, top) });
  }, [at]);
  const rows = item.fields.slice(0, PEEK_ROWS);
  return createPortal(<div className="ws-peek" ref={card} role="tooltip"
    style={{ ...look, ...(place ? { left: place.left, top: place.top } : { visibility: 'hidden', left: at.left, top: at.bottom + 8 }) } as React.CSSProperties}>
    <b className="ws-peek-title">{item.name}</b>
    {item.img ? <div className="ws-peek-img"><img src={item.img} alt="" referrerPolicy="no-referrer" /></div> : null}
    {item.lead ? <p className="ws-peek-lead">{item.lead}</p> : null}
    {rows.length ? <dl>{rows.map(([k, v]) => <div key={k}><dt>{k}</dt><dd>{v.length > 90 ? v.slice(0, 89).replace(/\s+\S*$/, '') + '…' : v}</dd></div>)}</dl> : null}
  </div>, document.body);
}
