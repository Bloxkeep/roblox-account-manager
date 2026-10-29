// What's new: the body of the dialog app.js opens after an update or from Settings › Bloxkeep updates. Newest release
// first: headline features with a picture or short demo and a button to try it, then New, Improved and Fixed lists. No
// icon beside each note (the owner found the icon tiles old-fashioned, 2026-10-26). Older skipped releases fold below.
import { useState } from 'react';
import { Icon, useHost } from './host';

/** whatsnew.json, as the app hands it over (RobloxManager/WhatsNew.cs). */
export interface Release {
  version: string;
  date: string;
  en?: string[] | null;
  vi?: string[] | null;
  highlights?: Highlight[] | null;
  new?: Notes | null;
  improved?: Notes | null;
  fixed?: Notes | null;
  removed?: Notes | null;
}
/** A headline feature: [title, text, button] per language, where the button leads, and its picture ("{lang}": en or vi). */
export interface Highlight { icon?: string; go?: string | null; image?: string | null; en: string[]; vi: string[] }
export interface Notes { en: string[]; vi: string[] }

const KINDS = ['new', 'improved', 'fixed', 'removed'] as const;

/** "Title: what it does" as [title, text]; a note without a short title before its first ": " is all text. */
function splitNote(note: string, locale: string): [string, string] {
  const i = note.indexOf(': ');
  if (!(i > 0 && i <= 60 && !note.slice(0, i).includes('.'))) return ['', note];
  const text = note.slice(i + 2); // under its own title it starts a sentence: "choose it…" reads "Choose it…"
  return [note.slice(0, i), text.charAt(0).toLocaleUpperCase(locale) + text.slice(1)];
}

/** The pictures are made at 1.5 times the app's size (sharp on a scaled screen): shown at the size they are in the app. */
const SHOT_SCALE = 1.5;
/** The tallest a picture is shown (app.css .wn-shot img max-height). */
const SHOT_MAX_HEIGHT = 360;

/** One headline feature: a wide picture (a row of settings, a message) above the words, a tall one (a menu) beside them. */
function Feature({ image, eager, children }: { image?: string | null; eager: boolean; children: React.ReactNode }) {
  const [size, setSize] = useState<{ w: number; wide: boolean } | null>(null);
  return <article className={`wn-feat${image ? (size?.wide ? ' wide' : '') : ' plain'}`}>
    {image ? <div className="wn-shot"><img src={image} alt="" loading={eager ? 'eager' : 'lazy'} style={size ? { width: size.w } : undefined}
      onLoad={e => {
        // Never wider than its height cap allows (app.css .wn-shot img): a tall picture set to its full width was squashed.
        const i = e.currentTarget, w = Math.min(i.naturalWidth / SHOT_SCALE, SHOT_MAX_HEIGHT * i.naturalWidth / i.naturalHeight);
        setSize({ w: Math.round(w), wide: i.naturalWidth >= i.naturalHeight * 2 });
      }} /></div> : null}
    <div className="wn-feat-text">{children}</div>
  </article>;
}

export function WhatsNew({ releases, close }: { releases: Release[]; close: () => void }) {
  const host = useHost();
  const { t, tn, lang, locale } = host;
  const pick = <T,>(en?: T[] | null, vi?: T[] | null): T[] => (lang === 'vi' && vi?.length ? vi : en) || [];
  const notes = (r: Release) => pick(r.en, r.vi);
  const day = (d: string) => {
    const date = new Date(`${d}T00:00:00`);
    return isNaN(+date) ? '' : date.toLocaleDateString(locale, { day: 'numeric', month: 'long', year: 'numeric' });
  };
  /** A list of notes; one with a short title before ": " shows it in bold. */
  const list = (items: string[], className: string) => <ul className={className}>{items.map((n, i) => {
    const [title, text] = splitNote(n, locale);
    return <li key={i}>{title ? <><b>{title}</b> </> : null}{text}</li>;
  })}</ul>;
  const groups = (r: Release) => KINDS.map(kind => {
    const items = pick(r[kind]?.en, r[kind]?.vi);
    return items.length ? <section key={kind} className={`wn-group ${kind}`}>
      <h4 className="wn-kind">{t(`wn.kind.${kind}`)}</h4>
      {list(items, 'wn-notes')}
    </section> : null;
  });
  const [latest, ...older] = releases;
  const highlights = latest.highlights || [];
  const go = (target: string) => { close(); host.go(target); };

  return <>
    {highlights.length ? <div className="wn-feats">{highlights.map((h, i) => {
      const [title, text, button] = pick(h.en, h.vi);
      const image = h.image?.replace('{lang}', lang === 'vi' ? 'vi' : 'en');
      return <Feature key={i} image={image} eager={i === 0}>
        <h3>{title}</h3>
        <p>{text}</p>
        {h.go && button ? <button type="button" className="wn-go" onClick={() => go(h.go!)}>{button}<Icon name="chevronRight" /></button> : null}
      </Feature>;
    })}</div> : null}
    {notes(latest).length ? <section className="wn-group">
      {highlights.length ? <h4 className="wn-kind">{t('wn.also')}</h4> : null}
      {list(notes(latest), 'wn-notes')}
    </section> : null}
    {groups(latest)}
    {older.length ? <details className="wn-older">
      <summary><Icon name="chevronRight" />{tn('wn.older', older.length)}</summary>
      {older.map(r => <section key={r.version} className="wn">
        <div className="wn-head"><b>{r.version}</b><span>{day(r.date)}</span></div>
        {notes(r).length ? list(notes(r), 'wn-notes') : null}
        {groups(r)}
      </section>)}
    </details> : null}
    <p className="wn-star"><span>{t('wn.star')}</span>
      <button type="button" className="wn-go" onClick={() => host.go('github')}>{t('star.open')}<Icon name="chevronRight" /></button></p>
  </>;
}
