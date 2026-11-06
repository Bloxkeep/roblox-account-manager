// A piece of game news drawn the way Discord drew it (services/DiscordBot/worker/feeds.js cardsOf makes the blocks):
// Discord's markdown, fields, a section with a picture or a button beside it, a gallery, a file, a line between, a row of
// buttons, a footer, a poll, a sticker. Everything another game's bot can post is one of these, so a game added later
// shows without an app update. Nothing here is HTML from outside: every piece becomes React's own elements, pictures come
// only from Discord's servers (the page's rules), and a link opens through the app (feedOpen), which checks it is the
// news's own.
import { createContext, Fragment, useContext, useState, type ReactNode } from 'react';
import { Icon } from './host';

export type Field = { name: string; value: string; inline?: boolean };
type Media = { url: string; video?: boolean; alt?: string; spoiler?: boolean; w?: number | null; h?: number | null };
type Btn = { label: string; emoji?: string; url?: string | null; menu?: boolean };
export type Block =
  | { t: 'md'; md: string }
  | { t: 'section'; md: string; thumb?: string | null; button?: Btn | null }
  | { t: 'fields'; items: Field[] }
  | { t: 'gallery'; items: Media[] }
  | { t: 'file'; name: string; size?: number | null }
  | { t: 'sep'; line?: boolean; big?: boolean }
  | { t: 'buttons'; items: Btn[] }
  | { t: 'footer'; md: string; icon?: string | null; at?: number | null }
  | { t: 'sticker'; url: string; name?: string }
  | { t: 'poll'; q: string; answers: { text: string; emoji?: string; votes: number }[]; ends?: number | null; done?: boolean }
  | { t: string; md?: string };

type T = (k: string, v?: Record<string, string | number>) => string;
/** What drawing needs: the person's language and clock, and how to open a link. */
export const NewsCtx = createContext<{ t: T; locale: string; now: number; open: (url: string) => void }>({ t: k => k, locale: 'en', now: 0, open: () => {} });

/** Discord's times (<t:1762400000:R>), in the person's own clock and language. */
export function timeText(ms: number, style: string, locale: string, now: number): string {
  const d = new Date(ms);
  switch (style) {
    case 'R': {
      const s = Math.round((ms - now) / 1000), a = Math.abs(s);
      const rtf = new Intl.RelativeTimeFormat(locale, { numeric: 'auto' });
      return a < 60 ? rtf.format(s, 'second') : a < 3600 ? rtf.format(Math.round(s / 60), 'minute') : a < 86400 ? rtf.format(Math.round(s / 3600), 'hour') : rtf.format(Math.round(s / 86400), 'day');
    }
    case 't': return d.toLocaleTimeString(locale, { hour: '2-digit', minute: '2-digit' });
    case 'T': return d.toLocaleTimeString(locale);
    case 'd': return d.toLocaleDateString(locale);
    case 'D': return d.toLocaleDateString(locale, { dateStyle: 'long' });
    case 'F': return d.toLocaleString(locale, { dateStyle: 'full', timeStyle: 'short' });
    default: return d.toLocaleString(locale, { dateStyle: 'long', timeStyle: 'short' });
  }
}
export const emojiSrc = (id: string, animated: boolean) => `https://cdn.discordapp.com/emojis/${id}.${animated ? 'gif' : 'webp'}?size=64`;

// ───────────────────────── Discord's markdown ─────────────────────────

// One inline piece, earliest first: code, a time, a server's emoji, a masked link, a bare link, a spoiler, then the marks.
const INLINE = new RegExp([
  /(`+)([^`][\s\S]*?)\1(?!`)/.source, // 1, 2 code
  /<t:(-?\d{1,13})(?::([tTdDfFR]))?>/.source, // 3, 4 time
  /<(a?):(\w{1,40}):(\d{5,25})>/.source, // 5, 6, 7 emoji
  /\[([^\]\n]{1,300})\]\(<?(https?:\/\/[^\s)>]+)>?(?:\s+"[^"]*")?\)/.source, // 8, 9 masked link
  /<?(https?:\/\/[^\s<>]*[^\s<>.,:;"')\]])>?/.source, // 10 link
  /\|\|([\s\S]+?)\|\|/.source, // 11 spoiler
  /\*\*\*([\s\S]+?)\*\*\*/.source, // 12 bold italic
  /\*\*([\s\S]+?)\*\*/.source, // 13 bold
  /__([\s\S]+?)__/.source, // 14 underline
  /~~([\s\S]+?)~~/.source, // 15 strike
  /\*(?=\S)([\s\S]*?\S)\*(?!\*)/.source, // 16 italic
  String.raw`(?<![\p{L}\p{N}_])_(?=\S)([\s\S]*?\S)_(?![\p{L}\p{N}_])`, // 17 italic
  /\\([\\*_~`|<>[\]()#-])/.source, // 18 an escaped mark
].join('|'), 'gu');

function Spoiler({ children }: { children: ReactNode }) {
  const [shown, setShown] = useState(false);
  return <span className={`md-spoiler ${shown ? 'on' : ''}`} role={shown ? undefined : 'button'} tabIndex={shown ? undefined : 0}
    onClick={() => setShown(true)} onKeyDown={e => { if (e.key === 'Enter' || e.key === ' ') setShown(true); }}>{children}</span>;
}

function Link({ url, children }: { url: string; children: ReactNode }) {
  const { open } = useContext(NewsCtx);
  if (!url.startsWith('https://')) return <>{children}</>;
  return <a href={url} title={url} onClick={e => { e.preventDefault(); open(url); }}>{children}</a>;
}

function Time({ ms, style }: { ms: number; style: string }) {
  const { locale, now } = useContext(NewsCtx);
  return <time className="md-time" dateTime={new Date(ms).toISOString()} title={timeText(ms, 'F', locale, now)}>{timeText(ms, style, locale, now)}</time>;
}

/** A line's words with Discord's inline marks drawn. */
export function Inline({ text }: { text: string }): ReactNode {
  const out: ReactNode[] = [];
  let at = 0, k = 0;
  INLINE.lastIndex = 0;
  for (let m = INLINE.exec(text); m; m = INLINE.exec(text)) {
    if (m.index > at) out.push(text.slice(at, m.index));
    at = INLINE.lastIndex;
    const key = k++;
    if (m[2] !== undefined) out.push(<code key={key} className="md-code">{m[2].replace(/^ (.*) $/, '$1')}</code>);
    else if (m[3] !== undefined) out.push(<Time key={key} ms={Number(m[3]) * 1000} style={m[4] || 'f'} />);
    else if (m[7] !== undefined) out.push(<img key={key} className="emj" src={emojiSrc(m[7], m[5] === 'a')} alt={`:${m[6]}:`} title={`:${m[6]}:`} decoding="async" />);
    else if (m[9] !== undefined) out.push(<Link key={key} url={m[9]}><Inline text={m[8]} /></Link>);
    else if (m[10] !== undefined) out.push(<Link key={key} url={m[10]}>{m[10]}</Link>);
    else if (m[11] !== undefined) out.push(<Spoiler key={key}><Inline text={m[11]} /></Spoiler>);
    else if (m[12] !== undefined) out.push(<strong key={key}><em><Inline text={m[12]} /></em></strong>);
    else if (m[13] !== undefined) out.push(<strong key={key}><Inline text={m[13]} /></strong>);
    else if (m[14] !== undefined) out.push(<u key={key}><Inline text={m[14]} /></u>);
    else if (m[15] !== undefined) out.push(<s key={key}><Inline text={m[15]} /></s>);
    else if (m[16] !== undefined || m[17] !== undefined) out.push(<em key={key}><Inline text={m[16] ?? m[17]} /></em>);
    else if (m[18] !== undefined) out.push(m[18]);
    else out.push(m[0]);
    if (INLINE.lastIndex === m.index) INLINE.lastIndex++; // never stuck on an empty match
  }
  // Each nested call reuses the one pattern: take up again where this one is.
  if (at < text.length) out.push(text.slice(at));
  return <>{out.map((n, i) => <Fragment key={i}>{n}</Fragment>)}</>;
}

/** Lines with Discord's line breaks kept. */
const Lines = ({ lines }: { lines: string[] }) => <>{lines.map((l, i) => <Fragment key={i}>{i ? <br /> : null}<Inline text={l} /></Fragment>)}</>;

type MdPart =
  | { k: 'p'; lines: string[] } | { k: 'h'; level: number; text: string } | { k: 'sub'; text: string }
  | { k: 'quote'; text: string } | { k: 'code'; text: string } | { k: 'list'; ordered: boolean; start: number; items: { depth: number; text: string }[] };

function mdParts(text: string): MdPart[] {
  const parts: MdPart[] = [];
  const lines = text.replace(/\r\n?/g, '\n').split('\n');
  const last = () => parts[parts.length - 1];
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const fence = /^\s*```(\w{0,20})\s*(.*)$/.exec(line);
    if (fence) {
      // Code to its closing marks (on the same line, or a later one).
      const sameLine = /^(.*?)```\s*$/.exec(fence[2]);
      if (sameLine && fence[2].length) { parts.push({ k: 'code', text: sameLine[1] }); continue; }
      const body: string[] = fence[2] ? [fence[2]] : [];
      for (i++; i < lines.length && !/```\s*$/.test(lines[i]); i++) body.push(lines[i]);
      if (i < lines.length) { const end = lines[i].replace(/```\s*$/, ''); if (end) body.push(end); }
      parts.push({ k: 'code', text: body.join('\n') });
      continue;
    }
    if (/^>>> /.test(line)) { parts.push({ k: 'quote', text: [line.slice(4), ...lines.slice(i + 1)].join('\n') }); break; }
    const q = /^> ?(.*)$/.exec(line);
    if (q && line.startsWith('>')) {
      const prev = last();
      prev?.k === 'quote' ? (prev.text += `\n${q[1]}`) : parts.push({ k: 'quote', text: q[1] });
      continue;
    }
    const h = /^(#{1,3}) +(.+)$/.exec(line);
    if (h) { parts.push({ k: 'h', level: h[1].length, text: h[2] }); continue; }
    const sub = /^-# +(.+)$/.exec(line);
    if (sub) { parts.push({ k: 'sub', text: sub[1] }); continue; }
    const li = /^(\s*)(?:([-*])|(\d{1,9})\.) +(.*)$/.exec(line);
    if (li) {
      const ordered = !!li[3], depth = Math.min(3, Math.floor(li[1].replace(/\t/g, '  ').length / 2)), prev = last();
      if (prev?.k === 'list' && (prev.ordered === ordered || depth > 0)) prev.items.push({ depth, text: li[4] });
      else parts.push({ k: 'list', ordered, start: ordered ? Number(li[3]) : 1, items: [{ depth, text: li[4] }] });
      continue;
    }
    const prev = last();
    if (!line.trim()) { if (prev?.k === 'p') prev.lines.push(''); continue; }
    prev?.k === 'p' ? prev.lines.push(line) : parts.push({ k: 'p', lines: [line] });
  }
  // A paragraph's empty last lines are only room before what follows.
  for (const p of parts) if (p.k === 'p') while (p.lines.length && !p.lines[p.lines.length - 1].trim()) p.lines.pop();
  return parts.filter(p => p.k !== 'p' || p.lines.length);
}

/** Text with Discord's markdown: headings, small print, quotes, lists, code, and the inline marks. */
export function Md({ text, className = '' }: { text: string; className?: string }) {
  if (!text) return null;
  return <div className={`md ${className}`}>{mdParts(text).map((p, i) => {
    switch (p.k) {
      case 'h': return <div key={i} className={`md-h md-h${p.level}`} role="heading" aria-level={p.level + 3}><Inline text={p.text} /></div>;
      case 'sub': return <div key={i} className="md-sub"><Inline text={p.text} /></div>;
      case 'quote': return <blockquote key={i} className="md-quote"><Md text={p.text} /></blockquote>;
      case 'code': return <pre key={i} className="md-pre"><code>{p.text}</code></pre>;
      case 'list': {
        const List = p.ordered ? 'ol' : 'ul';
        return <List key={i} className="md-list" start={p.ordered ? p.start : undefined}>
          {p.items.map((it, j) => <li key={j} style={it.depth ? { marginInlineStart: `${it.depth * 18}px` } : undefined}><Inline text={it.text} /></li>)}
        </List>;
      }
      default: return <p key={i} className="md-p"><Lines lines={p.lines} /></p>;
    }
  })}</div>;
}

// ───────────────────────── the blocks ─────────────────────────

/** A field's name that starts with an emoji (most stock and spawn bots name items so): that emoji as the tile's picture. */
function splitIcon(name: string): { icon: ReactNode; rest: string } {
  const custom = /^\s*<(a?):(\w+):(\d+)>\s*/.exec(name);
  if (custom) return { icon: <img src={emojiSrc(custom[3], !!custom[1])} alt="" decoding="async" />, rest: name.slice(custom[0].length) };
  const uni = /^\s*((?:\p{Extended_Pictographic}|\p{Regional_Indicator})(?:️|‍(?:\p{Extended_Pictographic})|\p{Emoji_Modifier}|\p{Regional_Indicator})*)\s*/u.exec(name);
  if (uni) return { icon: <span className="uni">{uni[1]}</span>, rest: name.slice(uni[0].length) };
  return { icon: null, rest: name };
}

function Fields({ items }: { items: Field[] }) {
  return <dl className="nc-fields">{items.map((f, i) => {
    const inline = f.inline !== false;
    const { icon, rest } = inline ? splitIcon(f.name) : { icon: null, rest: f.name };
    return <div key={i} className={`${icon ? 'has-icon' : ''} ${inline ? '' : 'nc-wide'}`}>
      {icon ? <span className="nc-ico">{icon}</span> : null}
      <span className="nc-fv"><dt><Inline text={rest} /></dt><dd><Md text={f.value} /></dd></span>
    </div>;
  })}</dl>;
}

function Hidden({ children, className = '' }: { children: ReactNode; className?: string }) {
  const { t } = useContext(NewsCtx);
  const [shown, setShown] = useState(false);
  if (shown) return <>{children}</>;
  return <button type="button" className={`nb-hidden ${className}`} onClick={() => setShown(true)}>
    <span className="nb-hidden-in" aria-hidden="true">{children}</span><b><Icon name="eye" />{t('news.spoiler')}</b>
  </button>;
}

function Gallery({ items }: { items: Media[] }) {
  const n = Math.min(items.length, 5);
  return <div className={`nb-gallery n${n}`}>{items.map((m, i) => {
    const media = m.video
      ? <video src={m.url} controls muted preload="metadata" playsInline title={m.alt || undefined} />
      : <img src={m.url} alt={m.alt || ''} title={m.alt || undefined} loading="lazy" />;
    return <div key={i} className="nb-media" style={items.length === 1 && m.w && m.h ? { aspectRatio: `${m.w} / ${m.h}` } : undefined}>
      {m.spoiler ? <Hidden className="nb-media-hidden">{media}</Hidden> : media}
    </div>;
  })}</div>;
}

const sizeText = (b?: number | null) => (b == null ? '' : b < 1024 ? `${b} B` : b < 1048576 ? `${(b / 1024).toFixed(b < 10240 ? 1 : 0)} KB` : `${(b / 1048576).toFixed(1)} MB`);

function Button({ b }: { b: Btn }) {
  const { t, open } = useContext(NewsCtx);
  const face = <>{b.emoji ? <Inline text={b.emoji} /> : null}{b.label ? <span>{b.label}</span> : null}</>;
  if (b.url) return <button type="button" className="nb-btn nb-link" title={b.url} onClick={() => open(b.url!)}>{face}<Icon name="external" /></button>;
  // Another bot's own button or menu: it answers only in Discord.
  return <span className={`nb-btn nb-off ${b.menu ? 'nb-menu' : ''}`} title={t('news.onlyDiscord')}>{face}{b.menu ? <Icon name="chevronDown" /> : null}</span>;
}

function Poll({ p }: { p: Extract<Block, { t: 'poll' }> }) {
  const { t, locale, now } = useContext(NewsCtx);
  const total = p.answers.reduce((s, a) => s + (a.votes || 0), 0);
  const top = Math.max(0, ...p.answers.map(a => a.votes || 0));
  return <div className="nb-poll">
    <div className="nb-poll-q"><Inline text={p.q} /></div>
    {p.answers.map((a, i) => {
      const pct = total ? Math.round((a.votes / total) * 100) : 0;
      return <div key={i} className={`nb-poll-a ${p.done && a.votes === top && top ? 'nb-win' : ''}`}>
        <span className="nb-poll-bar" style={{ width: `${pct}%` }} />
        <span className="nb-poll-t">{a.emoji ? <Inline text={a.emoji} /> : null}<Inline text={a.text} /></span>
        <span className="nb-poll-n">{pct}%</span>
      </div>;
    })}
    <div className="nb-poll-foot">{t('news.votes', { n: total })}{p.done ? ` · ${t('news.pollDone')}` : p.ends ? ` · ${t('news.pollEnds', { t: timeText(p.ends, 'R', locale, now) })}` : ''}</div>
  </div>;
}

function BlockView({ b }: { b: Block }) {
  const { locale, now } = useContext(NewsCtx);
  switch (b.t) {
    case 'md': return <Md text={(b as { md: string }).md} className="nb-md" />;
    case 'section': {
      const s = b as Extract<Block, { t: 'section' }>;
      return <div className="nb-section"><Md text={s.md} />{s.thumb ? <img className="nb-thumb" src={s.thumb} alt="" loading="lazy" /> : s.button ? <Button b={s.button} /> : null}</div>;
    }
    case 'fields': return <Fields items={(b as { items: Field[] }).items} />;
    case 'gallery': return <Gallery items={(b as { items: Media[] }).items} />;
    case 'file': {
      const f = b as Extract<Block, { t: 'file' }>;
      return <div className="nb-file"><Icon name="download" /><span className="nb-file-name">{f.name}</span>{f.size != null ? <span className="nb-file-size">{sizeText(f.size)}</span> : null}</div>;
    }
    case 'sep': {
      const s = b as Extract<Block, { t: 'sep' }>;
      return <hr className={`nb-sep ${s.line === false ? 'nb-blank' : ''} ${s.big ? 'nb-big' : ''}`} />;
    }
    case 'buttons': return <div className="nb-buttons">{(b as { items: Btn[] }).items.map((x, i) => <Button key={i} b={x} />)}</div>;
    case 'footer': {
      const f = b as Extract<Block, { t: 'footer' }>;
      return <div className="nb-footer">{f.icon ? <img src={f.icon} alt="" loading="lazy" /> : null}
        {f.md ? <span><Inline text={f.md} /></span> : null}
        {f.md && f.at ? <span aria-hidden="true">•</span> : null}
        {f.at ? <time dateTime={new Date(f.at).toISOString()}>{timeText(f.at, 'f', locale, now)}</time> : null}</div>;
    }
    case 'sticker': {
      const s = b as Extract<Block, { t: 'sticker' }>;
      return <img className="nb-sticker" src={s.url} alt={s.name || ''} title={s.name || undefined} loading="lazy" />;
    }
    case 'poll': return <Poll p={b as Extract<Block, { t: 'poll' }>} />;
    // A kind Discord adds later: its words, if it has some.
    default: {
      const words = (b as { md?: unknown }).md;
      return typeof words === 'string' ? <Md text={words} className="nb-md" /> : null;
    }
  }
}

/** A card's blocks in order; a box Discord hides as a spoiler stays hidden until clicked. */
export function Blocks({ blocks, spoiler }: { blocks: Block[]; spoiler?: boolean }) {
  const body = <div className="nb-blocks">{blocks.map((b, i) => <BlockView key={i} b={b} />)}</div>;
  return spoiler ? <Hidden className="nb-blocks-hidden">{body}</Hidden> : body;
}
