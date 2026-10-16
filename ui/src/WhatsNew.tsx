// What's new: the release notes, shown once after an update and any time from Settings › Bloxkeep updates. The body of
// the dialog app.js opens (its title, close button and Got it stay there). The newest release first: its headline
// features as cards with a button to try each, its other notes, then New, Improved and Fixed under their own headings;
// the older releases skipped over are folded below; last, a line asking for a GitHub star.
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
}
/** A headline feature: [title, one or two sentences, button] in each language, and where the button leads. */
export interface Highlight { icon?: string; go?: string | null; en: string[]; vi: string[] }
export interface Notes { en: string[]; vi: string[] }

/** A note's icon, from the words in it (either language); the first match wins. */
const NOTE_ICONS: [RegExp, string][] = [
  [/microsoft store/i, 'gamepad'], [/website|roblox\.com|trang web/i, 'globe'], [/discord/i, 'message'],
  [/guide|hướng dẫn/i, 'book'], [/feedback|góp ý/i, 'message'], [/lock app|khóa app/i, 'lock'],
  [/before you open|question|câu hỏi|lưu ý/i, 'alert'], [/multi-roblox/i, 'layers'], [/theme|giao diện/i, 'palette'],
  [/faster|nhanh hơn/i, 'gauge'], [/vietnamese|tiếng việt/i, 'languages'], [/github|download|tải về/i, 'shield'],
  [/preset|schedule|hẹn giờ/i, 'calendar'], [/password|mật khẩu/i, 'key'],
];
const KINDS = [['new', 'plus'], ['improved', 'gauge'], ['fixed', 'wrench']] as const;

/** "Title: what it does" as [title, text]; a note without a short title before its first ": " is all text. */
function splitNote(note: string, locale: string): [string, string] {
  const i = note.indexOf(': ');
  if (!(i > 0 && i <= 60 && !note.slice(0, i).includes('.'))) return ['', note];
  const text = note.slice(i + 2); // under its own title it starts a sentence: "choose it…" reads "Choose it…"
  return [note.slice(0, i), text.charAt(0).toLocaleUpperCase(locale) + text.slice(1)];
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
  const groups = (r: Release) => KINDS.map(([kind, icon]) => {
    const list = pick(r[kind]?.en, r[kind]?.vi);
    return list.length ? [
      <h4 key={`${kind}-h`} className={`wn-sub wn-kind ${kind}`}><Icon name={icon} />{t(`wn.kind.${kind}`)}</h4>,
      <ul key={`${kind}-l`} className={`wn-also ${kind}`}>{list.map((n, i) => <li key={i}><Icon name={icon} /><span>{n}</span></li>)}</ul>,
    ] : null;
  });
  const [latest, ...older] = releases;
  const highlights = latest.highlights || [];
  const go = (target: string) => { close(); host.go(target); };

  return <>
    {highlights.length ? <>
      <ul className="wn-cards">{highlights.map((h, i) => {
        const [title, text, button] = pick(h.en, h.vi);
        return <li key={i} className="wn-card">
          <span className="wn-ic"><Icon name={h.icon || 'check'} /></span><b>{title}</b><p>{text}</p>
          {h.go && button ? <button type="button" className="wn-go" onClick={() => go(h.go!)}>{button}<Icon name="chevronRight" /></button> : null}
        </li>;
      })}</ul>
      {notes(latest).length ? <>
        <h4 className="wn-sub">{t('wn.also')}</h4>
        <ul className="wn-also">{notes(latest).map((n, i) => <li key={i}><Icon name="check" /><span>{n}</span></li>)}</ul>
      </> : null}
    </> : notes(latest).length ? <ul className="wn-list">{notes(latest).map((n, i) => {
      const [title, text] = splitNote(n, locale);
      const icon = NOTE_ICONS.find(([re]) => re.test(title || text))?.[1] || 'check';
      return <li key={i} className="wn-item"><span className="wn-ic"><Icon name={icon} /></span><div>{title ? <b>{title}</b> : null}<p>{text}</p></div></li>;
    })}</ul> : null}
    {groups(latest)}
    {older.length ? <details className="wn-older">
      <summary><Icon name="chevronRight" />{tn('wn.older', older.length)}</summary>
      {older.map(r => <section key={r.version} className="wn">
        <div className="wn-head"><b>{r.version}</b><span>{day(r.date)}</span></div>
        {notes(r).length ? <ul>{notes(r).map((n, i) => <li key={i}>{n}</li>)}</ul> : null}
        {groups(r)}
      </section>)}
    </details> : null}
    <p className="wn-star"><Icon name="star" /><span>{t('wn.star')}</span>
      <button type="button" className="wn-go" onClick={() => host.go('github')}>{t('star.open')}<Icon name="chevronRight" /></button></p>
  </>;
}
