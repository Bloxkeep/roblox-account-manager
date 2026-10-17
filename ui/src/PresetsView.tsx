// The Presets tab: the saved presets, each a row with its game, what it does in one plain line, when it runs next and
// last, its accounts, its schedule switch, Run (or Close while its windows are open) and Edit / Duplicate / Delete; a
// search over names and games; the order Play's menu shows them in, changed with the arrows or by dragging the handle.
import { Fragment, useEffect, useRef, useState, type DragEvent } from 'react';
import { flushSync } from 'react-dom';
import { Ago, Icon, useAppState, useHost } from './host';
import { richNodes } from './rich';
import { fold } from './text';
import { Avatar } from './Avatar';

/** The app's own limit (MaxPresets); app.js checks it too, so a full list doesn't open an empty form. */
const MAX_PRESETS = 50;
const AVATARS_SHOWN = 4;

type Schedule = { enabled: boolean; time: string; date?: string; days?: number[] };
export type Preset = {
  id: string; name: string; accountIds: string[]; placeInput: string; gameName?: string; server: string;
  delaySeconds: number | null; schedule: Schedule | null; nextRun?: string; lastRun?: string; running?: number; iconUrl?: string;
};
type Account = { id: string; label: string };
const NONE: Preset[] = [];

/** Waits for the page to be drawn with the app's answer (its state comes on its own, around the reply). */
const drawn = () => new Promise(res => setTimeout(res));

export function PresetsView() {
  const host = useHost();
  const { t, tn, call, busy, toast, on } = host;
  const app = useAppState();
  const all: Preset[] = app.settings?.presets || NONE;
  const byId = new Map<string, Account>((app.accounts || []).map((a: Account) => [a.id, a]));
  /** The preset's accounts that still exist (deleted ones are skipped when it runs). */
  const accountsOf = (p: Preset) => p.accountIds.map(id => byId.get(id)).filter((a): a is Account => !!a);
  const accCount = (n: number) => t(n === 1 ? 'ps.acc1' : 'ps.accN', { n });
  const list = useRef<HTMLDivElement>(null);
  const rowOf = (id: string) => [...list.current!.querySelectorAll<HTMLElement>('.pv-item')].find(r => r.dataset.id === id);

  // The search: kept by app.js too (a page restarted comes back with it); the list follows the box a moment later.
  const [query, setQuery] = useState(host.presetQuery());
  const [shownQuery, setShownQuery] = useState(query);
  useEffect(() => on('presetQuery', () => { const q = host.presetQuery(); setQuery(q); setShownQuery(q); }), [on, host]);
  useEffect(() => {
    if (shownQuery === query) return;
    const timer = setTimeout(() => setShownQuery(query), 120);
    return () => clearTimeout(timer);
  }, [query, shownQuery]);
  const q = fold(shownQuery.trim());
  const shown = q ? all.filter(p => fold(`${p.name}\n${p.placeInput ? p.gameName || p.placeInput : t('ps.noGame')}`).includes(q)) : all;

  // The tab just opened: game icons not known yet are asked for (at most every few minutes).
  const iconsAt = useRef(0), latest = useRef(all);
  latest.current = all;
  useEffect(() => on('presetsShown', () => {
    if (!latest.current.some(p => p.placeInput && !p.iconUrl) || Date.now() - iconsAt.current <= 5 * 60_000) return;
    iconsAt.current = Date.now();
    call('presetIcons', {}, { silent: true });
  }), [on, call]);

  // Dragging a row by its handle (only the handle starts a drag, so the row's buttons and switch work as usual): the
  // rows show the new order as it goes and keep it until the app answers; Esc, or a drop outside, puts them back.
  const [armed, setArmed] = useState<string | null>(null);
  const [drag, setDrag] = useState<{ id: string; order: string[]; done?: boolean } | null>(null);
  const rows = drag ? drag.order.map(id => all.find(p => p.id === id)).filter((p): p is Preset => !!p) : shown;
  const dragOver = (e: DragEvent<HTMLDivElement>) => {
    if (!drag || drag.done) return;
    e.preventDefault();
    e.dataTransfer.dropEffect = 'move';
    const over = (e.target as HTMLElement).closest<HTMLElement>('.pv-item');
    if (!over || over.dataset.id === drag.id) return;
    const r = over.getBoundingClientRect();
    const order = drag.order.filter(id => id !== drag.id);
    order.splice(order.indexOf(over.dataset.id!) + (e.clientY < r.top + r.height / 2 ? 0 : 1), 0, drag.id);
    if (order.join() !== drag.order.join()) setDrag({ ...drag, order });
  };
  const dragEnd = async (e: DragEvent<HTMLDivElement>) => {
    setArmed(null);
    if (!drag) return;
    const ids = drag.order;
    if (e.dataTransfer.dropEffect === 'none' || ids.join() === all.map(p => p.id).join()) return setDrag(null);
    setDrag({ ...drag, done: true });
    await call('setPresetOrder', { ids });
    setDrag(null);
  };

  // The arrows: the focus stays on the moved row's arrow; once the row reaches an end (that arrow is off) on the row
  // itself, not on the other arrow, where the next Enter would move it straight back.
  const move = async (id: string, dir: number) => {
    const ids = all.map(p => p.id);
    const i = ids.indexOf(id), j = i + dir;
    if (i < 0 || j < 0 || j >= ids.length) return;
    [ids[i], ids[j]] = [ids[j], ids[i]];
    if (await call('setPresetOrder', { ids }) === undefined) return;
    await drawn();
    const row = rowOf(id);
    const same = row?.querySelector<HTMLElement>(`[data-move="${dir}"]:not(:disabled)`);
    if (same) same.focus();
    else if (row) { row.tabIndex = -1; row.focus(); }
    row?.scrollIntoView({ block: 'nearest' });
  };

  // A double click makes one copy: the row is drawn again before the second click lands, on an enabled button.
  const dupLast = useRef(new Map<string, number>());
  const duplicate = async (p: Preset, btn: HTMLElement) => {
    if (Date.now() - (dupLast.current.get(p.id) || 0) < 600) return;
    dupLast.current.set(p.id, Date.now());
    const id = await busy(btn, () => call('duplicatePreset', { id: p.id })) as string | undefined;
    dupLast.current.set(p.id, Date.now());
    if (id === undefined) return;
    toast(t(p.schedule?.enabled ? 'pv.duplicatedOff' : 'pv.duplicated'), 'ok');
    // The copy is drawn right under the original (or its other copies); the focus stays on the original's Duplicate.
    await drawn();
    rowOf(id)?.scrollIntoView({ block: 'nearest' });
  };

  const scheduled = all.filter(p => p.schedule?.enabled).length;
  // Counts only, like the other tabs; the empty state says what a preset is.
  const sub = all.length ? [t(all.length === 1 ? 'pv.count1' : 'pv.countN', { n: all.length }),
    scheduled ? t('pv.scheduled', { n: scheduled }) : '', all.length >= MAX_PRESETS ? t('pv.atMax') : ''].filter(Boolean) : [];
  const ordered = !q && all.length > 1;

  return <>
    <header className="top">
      <div className="top-title"><h1>{t('ps.listTitle')}</h1><p id="pv-sub">{sub.map((s, i) => <Fragment key={i}>{i ? <span className="sep">·</span> : null}{richNodes(s)}</Fragment>)}</p></div>
      {/* The empty state has its own New button. */}
      <div className="top-tools" id="pv-tools" hidden={!all.length}>
        <label className="search"><Icon name="search" /><input id="pv-q" placeholder={t('pv.search')} maxLength={100} aria-label={t('pv.search')} value={query}
          onChange={e => { setQuery(e.target.value); host.setPresetQuery(e.target.value); }} /></label>
        <button className="btn primary" data-act="pv-new"><Icon name="plus" /><span>{t('ps.new')}</span></button>
      </div>
    </header>
    <div className="scroll">
      <div className="pv-list" id="pv-list" role="list" ref={list} onPointerUp={() => setArmed(null)} onDragOver={dragOver}
        onDrop={e => e.preventDefault()} onDragEnd={dragEnd}>
        {!all.length ? <div className="empty"><span className="pv-empty-icon"><Icon name="layers" /></span><h2>{t('pv.emptyTitle')}</h2><p>{t('pv.emptyText')}</p>
          <div className="row"><button className="btn primary" data-act="pv-new"><Icon name="plus" /><span>{t('ps.new')}</span></button></div></div>
          : !rows.length ? <div className="empty"><p>{t('pv.noMatch', { q: shownQuery.trim() })}</p></div>
          : rows.map((p, i) => <PresetRow key={p.id} p={p} i={i} count={rows.length} ordered={ordered} accounts={accountsOf(p)} accCount={accCount}
            launching={!!app.launching} delayDefault={app.settings?.launchDelaySeconds ?? 8} dragging={drag?.id === p.id && !drag.done}
            draggable={armed === p.id} arm={() => setArmed(p.id)} move={move} duplicate={duplicate}
            dragStart={e => {
              if (armed !== p.id) return;
              setDrag({ id: p.id, order: rows.map(x => x.id) });
              e.dataTransfer.effectAllowed = 'move';
              e.dataTransfer.setData('text/plain', p.id); // Chromium needs some data for the drag to start
            }}
            close={async btn => busy(btn, async () => {
              const n = await call('closePreset', { id: p.id }) as number | undefined;
              if (n !== undefined) toast(n ? tn('t.closing', n) : t('t.noneRunning'), n ? 'ok' : 'info');
            })} />)}
      </div>
    </div>
  </>;
}

function PresetRow({ p, i, count, ordered, accounts, accCount, launching, delayDefault, dragging, draggable, arm, dragStart, move, duplicate, close }: {
  p: Preset; i: number; count: number; ordered: boolean; accounts: Account[]; accCount: (n: number) => string; launching: boolean;
  delayDefault: number; dragging: boolean; draggable: boolean; arm: () => void; dragStart: (e: DragEvent<HTMLDivElement>) => void;
  move: (id: string, dir: number) => void; duplicate: (p: Preset, btn: HTMLElement) => void; close: (btn: HTMLElement) => void;
}) {
  const host = useHost();
  const { t, tn, call, busy, toast } = host;
  const game = p.placeInput ? p.gameName || p.placeInput : t('ps.noGame');
  const sc = p.schedule;
  const delay = p.delaySeconds ?? null;
  // What it does, in one plain line: server · wait · schedule · options set differently from Settings.
  const summary = [
    p.placeInput ? t(`ps.srv.${p.server}`) : '',
    delay === null ? t('pv.delayDefault', { n: delayDefault }) : delay === 0 ? t('pv.delay0') : t('pv.delayN', { n: delay }),
    !sc?.time ? t('pv.noSchedule') : !sc.enabled ? `${t('pv.schOff')} · ${host.scheduleText(sc)}` : host.scheduleText(sc),
    host.presetOptionsText(p),
  ].filter(Boolean).join(' · ');
  // Then, quieter: when it runs next (a one-off's date is already above) and when it ran last.
  const next = [
    p.running ? <span className="pv-live" key="live">{tn('pv.running', p.running)}</span> : null,
    sc?.enabled && p.nextRun && !sc.date ? host.nextRunText(p) : null,
    p.lastRun ? <Ago key="last" iso={p.lastRun} tpl="pv.lastRun" /> : null,
  ].filter(Boolean);
  const accTitle = [accCount(accounts.length), ...accounts.map(a => a.label)].join('\n');

  // The schedule switch: off while the app answers; refused (a one-off whose date has passed), it goes back.
  const [schOn, setSchOn] = useState(!!sc?.enabled);
  const [schBusy, setSchBusy] = useState(false);
  useEffect(() => setSchOn(!!sc?.enabled), [sc?.enabled]);
  const schBox = useRef<HTMLInputElement>(null);
  const [iconFailed, setIconFailed] = useState(false);
  useEffect(() => setIconFailed(false), [p.iconUrl]);

  return <div className={`pv-item${dragging ? ' dragging' : ''}`} role="listitem" data-id={p.id} draggable={draggable} onDragStart={dragStart}>
    {ordered ? <span className="ord-grip pv-grip" title={t('lo.drag')} onPointerDown={arm}><Icon name="grip" /></span> : null}
    {p.iconUrl && !iconFailed ? <img className="pv-icon" src={p.iconUrl} alt="" draggable="false" loading="lazy" onError={() => setIconFailed(true)} />
      : <span className="pv-icon"><Icon name={p.placeInput ? 'gamepad' : 'monitor'} /></span>}
    <div className="pv-main">
      <b title={p.name}>{p.name}</b>
      <small title={game}>{game}</small>
      <span className="pv-sum" title={summary}>{summary}</span>
      <span className="pv-next">{next.map((x, k) => <Fragment key={k}>{k ? ' · ' : null}{x}</Fragment>)}</span>
    </div>
    {accounts.length
      ? <div className="pv-accs" title={accTitle} aria-label={accCount(accounts.length)}>
        {accounts.slice(0, AVATARS_SHOWN).map(a => <span key={a.id} className="avatar sm"><Avatar a={a} /></span>)}
        {accounts.length > AVATARS_SHOWN ? <em>+{accounts.length - AVATARS_SHOWN}</em> : null}
      </div>
      : <div className="pv-accs none" title={t('pv.noAccountsTip')}><Icon name="alert" /><small>{t('pv.noAccounts')}</small></div>}
    <div className="pv-sw">{sc?.time ? <>
      <small aria-hidden="true">{t('ps.sch')}</small>
      <label className="switch" title={t(sc.enabled ? 'pv.schTurnOff' : 'pv.schTurnOn')}>
        <input type="checkbox" data-pv="sched" ref={schBox} checked={schOn} disabled={schBusy} aria-label={`${t('ps.sch')}: ${p.name}`} onChange={async e => {
          const want = e.target.checked;
          setSchOn(want);
          setSchBusy(true);
          const r = await call('setPresetSchedule', { id: p.id, enabled: want });
          // On again (drawn now, so it can take the focus back: a switch turned off lost it).
          flushSync(() => setSchBusy(false));
          if (!document.activeElement || document.activeElement === document.body) schBox.current?.focus();
          if (r === undefined) setSchOn(!want);
          else toast(t(want ? 't.schOn' : 't.schOff', { name: p.name }), 'ok');
        }} /><span />
      </label>
    </> : null}</div>
    {/* Run and Close are two buttons (keyed), not one changing its words: the focus doesn't stay on Run after Close,
        where the next Enter would open the windows just closed. */}
    {p.running
      ? <button key="close" className="btn sm pv-run pv-close" data-pv="close" title={tn('pv.runningTip', p.running)} aria-label={`${t('pv.close')}: ${p.name}`}
        onClick={e => close(e.currentTarget)}><Icon name="power" /><span>{t('pv.close')}</span></button>
      : <button key="run" className="btn sm pv-run" data-pv="run" aria-label={`${t('ps.run')}: ${p.name}`} disabled={!accounts.length || launching}
        onClick={e => busy(e.currentTarget, () => host.runPreset(p.id))}><Icon name="play" /><span>{t('ps.run')}</span></button>}
    <span className="pv-acts">
      <button className="icon-btn" data-pv="edit" title={t('ps.edit')} aria-label={`${t('ps.edit')}: ${p.name}`} onClick={() => host.editPreset(p)}><Icon name="edit" /></button>
      <button className="icon-btn" data-pv="dup" title={t('pv.duplicate')} aria-label={`${t('pv.duplicate')}: ${p.name}`} onClick={e => duplicate(p, e.currentTarget)}><Icon name="copy" /></button>
      <button className="icon-btn" data-pv="del" title={t('ps.delete')} aria-label={`${t('ps.delete')}: ${p.name}`} onClick={() => host.deletePreset(p)}><Icon name="trash" /></button>
    </span>
    {ordered ? <span className="ord-btns">
      <button className="icon-btn" data-move="-1" title={t('lo.up')} aria-label={`${t('lo.up')}: ${p.name}`} disabled={i === 0} onClick={() => move(p.id, -1)}><Icon name="chevronUp" /></button>
      <button className="icon-btn" data-move="1" title={t('lo.down')} aria-label={`${t('lo.down')}: ${p.name}`} disabled={i >= count - 1} onClick={() => move(p.id, 1)}><Icon name="chevronDown" /></button>
    </span> : null}
  </div>;
}
