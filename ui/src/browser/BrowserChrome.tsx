// The top of Bloxkeep's browser windows (browser.html in a WebView2 of the app's own, docked above the roblox.com one):
// Forms/WebsiteForm.cs's tab strip (each account's avatar and name, ✕, +) with back, forward, reload, the site, the tab's
// status line, Retry and Save to Bloxkeep; Forms/BrowserLoginForm.cs's bar (the site, what to do, Retry, Done).
// It is told only what it draws (names as the app shows them, the site's host name, the status words); the roblox.com
// pages, their cookies and what is typed there never come here. Until 2026-11 it was WinForms.
import { useEffect, useImperativeHandle, useLayoutEffect, useRef, useState, type Ref } from 'react';
import { Icon, useHost } from '../host';

export interface BrowserTab {
  id: string;
  /** The account's name as the app shows it (Hide account names holds), whoever signed in on a blank tab, or Roblox. */
  title: string;
  /** Its Roblox avatar (none while names are hidden, or for a blank tab). */
  avatar: string | null;
  /** A saved account's tab (else a blank one, for signing in on the site). */
  saved: boolean;
  selected: boolean;
}

/** The app's "browser" event: the chosen tab's state, the theme and the language. */
export interface BrowserData {
  /** tabs: the roblox.com window; login: the sign-in window (no tabs, Done). */
  mode: 'tabs' | 'login';
  tabs: BrowserTab[];
  back: boolean;
  forward: boolean;
  /** The host name of the page shown (www.roblox.com), null before one loads. */
  host: string | null;
  /** The status line and its colour: '' (plain), 'ok' or 'bad'. */
  message: string;
  tone: '' | 'ok' | 'bad';
  retry: boolean;
  /** Save to Bloxkeep: a blank tab someone signed in on. */
  save: boolean;
  theme: string;
  customTheme: unknown;
  language: string;
}

export interface BrowserOn {
  /** select {id}, close {id}, newTab, back, forward, reload, retry, save, done. */
  act(what: string, args?: Record<string, unknown>): void;
  /** Its height in CSS pixels (the window gives it that much room above the page). */
  size(width: number, height: number): void;
}

export interface BrowserChromeApi { show(data: BrowserData): void }

export function BrowserRoot({ on, handle }: { on: BrowserOn; handle: Ref<BrowserChromeApi> }) {
  const [data, setData] = useState<BrowserData | null>(null);
  useImperativeHandle(handle, () => ({ show: d => setData(d) }), []);
  return data ? <BrowserChrome d={data} on={on} /> : null;
}

export function BrowserChrome({ d, on }: { d: BrowserData; on: BrowserOn }) {
  const { t } = useHost();
  const root = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const el = root.current;
    if (!el) return;
    const measure = () => on.size(el.offsetWidth, el.offsetHeight);
    measure();
    const watch = new ResizeObserver(measure);
    watch.observe(el);
    return () => watch.disconnect();
  }, [d.mode]);

  const site = d.host ? <span className="bw-site" title={d.host}><Icon name="lock" /><span>{d.host}</span></span> : null;
  const status = <span className={`bw-msg${d.tone ? ` ${d.tone}` : ''}`} role="status" title={d.message || undefined}>{d.message}</span>;
  const retry = d.retry ? <button type="button" className="btn sm" onClick={() => on.act('retry')}>{t('bw.retry')}</button> : null;

  if (d.mode === 'login') {
    return <div ref={root} className="bw login">
      <div className="bw-bar">
        {site}{status}{retry}
        <button type="button" className="btn sm primary" onClick={() => on.act('done')}>{t('common.done')}</button>
      </div>
    </div>;
  }
  return <div ref={root} className="bw">
    <Tabs tabs={d.tabs} on={on} />
    <div className="bw-bar">
      <button type="button" className="icon-btn" disabled={!d.back} title={t('bw.back')} aria-label={t('bw.back')} onClick={() => on.act('back')}><Icon name="chevronLeft" /></button>
      <button type="button" className="icon-btn" disabled={!d.forward} title={t('bw.forward')} aria-label={t('bw.forward')} onClick={() => on.act('forward')}><Icon name="chevronRight" /></button>
      <button type="button" className="icon-btn" title={t('bw.reload')} aria-label={t('bw.reload')} onClick={() => on.act('reload')}><Icon name="refresh" /></button>
      {site}{status}{retry}
      {d.save ? <button type="button" className="btn sm primary" title={t('bw.saveTip')} onClick={() => on.act('save')}>{t('bw.save')}</button> : null}
    </div>
  </div>;
}

/** The tab strip: wide tabs when few, narrower when many (then avatars only), + right after the last one. */
function Tabs({ tabs, on }: { tabs: BrowserTab[]; on: BrowserOn }) {
  const { t } = useHost();
  const strip = useRef<HTMLDivElement>(null);
  const chosen = tabs.find(x => x.selected)?.id;
  useEffect(() => { (strip.current?.querySelector('.bw-tab.on') as HTMLElement | null)?.scrollIntoView({ block: 'nearest', inline: 'nearest' }); }, [chosen, tabs.length]);
  return <div className="bw-tabs"><div ref={strip} className="bw-list" role="tablist" aria-label={t('bw.tabs')}
    onWheel={e => { if (strip.current && !e.shiftKey) strip.current.scrollLeft += e.deltaY; }}
    onKeyDown={e => {
      const i = tabs.findIndex(x => x.selected);
      const to = e.key === 'ArrowRight' ? i + 1 : e.key === 'ArrowLeft' ? i - 1 : e.key === 'Home' ? 0 : e.key === 'End' ? tabs.length - 1 : -2;
      if (to === -2 || !tabs.length || !(e.target as Element).closest('.bw-pick')) return;
      e.preventDefault();
      const next = tabs[(to + tabs.length) % tabs.length];
      on.act('select', { id: next.id });
      requestAnimationFrame(() => (strip.current?.querySelector(`[data-id="${next.id}"] .bw-pick`) as HTMLElement | null)?.focus());
    }}>
    {tabs.map(x => <div key={x.id} data-id={x.id} className={`bw-tab${x.selected ? ' on' : ''}`}
      onMouseDown={e => { if (e.button === 1) e.preventDefault(); }}
      onAuxClick={e => { if (e.button === 1) on.act('close', { id: x.id }); }}>
      <button type="button" role="tab" className="bw-pick" aria-selected={x.selected} tabIndex={x.selected ? 0 : -1} title={x.title}
        onClick={() => on.act('select', { id: x.id })}>
        <Avatar tab={x} />
        <span className="bw-name">{x.title}</span>
      </button>
      <button type="button" className="bw-x" tabIndex={-1} title={t('bw.close')} aria-label={t('bw.close')} onClick={() => on.act('close', { id: x.id })}><Icon name="x" /></button>
    </div>)}
    </div>
    <button type="button" className="icon-btn bw-new" title={t('bw.newTip')} aria-label={t('bw.new')} onClick={() => on.act('newTab')}><Icon name="plus" /></button>
  </div>;
}

/** The account's Roblox avatar; a person (a hidden name, a picture that didn't load) or the web (a blank tab) without one. */
function Avatar({ tab }: { tab: BrowserTab }) {
  const [failed, setFailed] = useState(false);
  useEffect(() => setFailed(false), [tab.avatar]);
  return <span className="bw-av">
    {tab.avatar && !failed ? <img src={tab.avatar} alt="" onError={() => setFailed(true)} /> : <Icon name={tab.saved ? 'user' : 'globe'} />}
  </span>;
}
