// "Player sees": the macro's panel as the app draws it beside the game (its own small dark window): the name, Run with
// its hotkey, one status line with the counters, its tabs, the settings the maker handed to the player, Edit areas and
// Reset. Here the settings' names are typed in place, their order and tab changed, and switches, choices and headings
// added; the values are set in the steps.
import { useState, type ReactNode } from 'react';
import { Icon, useHost } from './host';
import { asPoint, keyName, msText, type Macro, type Option } from './macros';

export function PlayerSees({ m, options, counts, areas, setLabel, move, remove, tabs, tab, setTab, addTab, renameTab, removeTab, moveTo, add, setChoices, children }: {
  m: Macro; options: Option[]; counts: string[]; areas: number; setLabel: (name: string, label: string) => void;
  move: (name: string, dir: number) => void; remove: (name: string) => void;
  tabs: string[]; tab: string | null; setTab: (tab: string) => void; addTab: () => void; renameTab: (old: string, name: string) => void;
  removeTab: (tab: string) => void; moveTo: (name: string, tab: string) => void;
  add: (kind: 'check' | 'choice' | 'list' | 'heading') => void; setChoices: (name: string, choices: string[]) => void; children?: ReactNode;
}) {
  const host = useHost();
  const { t } = host;
  // Times are kept in milliseconds and shown in seconds; a share kept as 0–1 is shown as a percentage.
  const numText = (o: Option) => typeof o.value !== 'number' ? String(o.value)
    : o.unit === 's' || o.unit === 'ms' ? msText(t, o.value)
    : o.unit === '%' ? `${o.max !== undefined && o.max <= 1 ? Math.round(o.value * 100) : o.value} %`
    : `${o.value}${o.unit ? ' ' + o.unit : ''}`;
  const value = (o: Option) => {
    switch (o.kind) {
      case 'colour': return <><i className="mp-swatch" style={{ background: String(o.value) }} /><span className="mp-mini">{t('macro.panel.pick')}</span></>;
      case 'check': return <span className={`mp-switch${o.value ? ' on' : ''}`}><span /></span>;
      case 'key': return <><kbd>{keyName(t, String(o.value))}</kbd><span className="mp-mini">{t('macro.panel.change')}</span></>;
      case 'choice': return <Choices o={o} set={list => setChoices(o.name, list)} />;
      case 'list': return <><Choices o={o} set={list => setChoices(o.name, list)} />
        <span className="mp-mini" title={t('macro.opt.listTip')}>{t('macro.opt.ticked', { n: Array.isArray(o.value) ? o.value.length : 0, all: o.choices?.length || 0 })}</span></>;
      case 'point': {
        const p = asPoint(o.value) || { x: 0.5, y: 0.5 };
        return <><span className="mp-field num">{Math.round(p.x * 100)}%, {Math.round(p.y * 100)}%</span><span className="mp-mini">{t('macro.panel.set')}</span></>;
      }
      default: return <><span className="mp-mini">−</span>
        <span className="mp-field num">{numText(o)}</span>
        <span className="mp-mini">+</span></>;
    }
  };
  const tabOf = (o: Option) => o.tab && tabs.includes(o.tab) ? o.tab : tabs[0];
  const shown = tabs.length ? options.filter(o => tabOf(o) === tab) : options;
  const addMenu = (btn: HTMLElement) => host.menu(btn, [
    { icon: 'check', text: t('macro.opt.add.check'), run: () => add('check') },
    { icon: 'chevronDown', text: t('macro.opt.add.choice'), run: () => add('choice') },
    { icon: 'rows', text: t('macro.opt.add.list'), run: () => add('list') },
    { icon: 'info', text: t('macro.opt.add.heading'), run: () => add('heading') },
    '-', { icon: 'plus', text: t('macro.tab.add'), run: addTab },
  ]);
  const tabMenu = (btn: HTMLElement, o: Option) => host.menu(btn, [
    { head: true, text: t('macro.tab.moveTo') },
    ...tabs.filter(x => x !== tabOf(o)).map(x => ({ icon: 'chevronRight', text: x, run: () => moveTo(o.name, x) })),
  ]);
  return <section className="mc-sees" aria-labelledby="mc-sees-h">
    <div className="mc-sees-head"><b id="mc-sees-h">{t('macro.panel.title')}</b><small>{t('macro.panel.sub')}</small>
      <button type="button" className="btn sm" id="mc-sees-add" onClick={e => addMenu(e.currentTarget)}><Icon name="plus" /><span>{t('macro.opt.add')}</span></button></div>
    <div className="mp-win" aria-label={t('macro.panel.preview')}>
      <div className="mp-title"><b>{m.name.trim() || t('macro.namePh')}</b><em>{t('macro.beta')}</em>
        <span className="mp-x" aria-hidden="true">–</span><span className="mp-x" aria-hidden="true"><Icon name="x" /></span></div>
      <div className="mp-run"><Icon name="play" /><span>{t('macro.run')}</span>{m.hotkey ? <kbd>Ctrl+Alt+{m.hotkey}</kbd> : null}</div>
      <p className="mp-status">{[t('macro.panel.idle'), ...counts.map(c => `${c}: 0`)].join(' · ')}</p>
      {tabs.length ? <div className="mp-tabs" role="tablist" aria-label={t('macro.tab.list')}>
        {tabs.map(x => x === tab
          ? <span key={x} className="mp-tab on" role="tab" aria-selected="true">
            <TabName name={x} others={tabs.filter(y => y !== x)} rename={nu => { renameTab(x, nu); setTab(nu); }} label={t('macro.tab.nameOf', { name: x })} />
            <button type="button" className="mp-tab-x" title={t('macro.tab.remove')} aria-label={`${t('macro.tab.remove')}: ${x}`} onClick={() => removeTab(x)}><Icon name="x" /></button></span>
          : <button key={x} type="button" className="mp-tab" role="tab" aria-selected="false" onClick={() => setTab(x)}>{x}</button>)}
        {tabs.length < 6 ? <button type="button" className="mp-tab add" title={t('macro.tab.add')} aria-label={t('macro.tab.add')} onClick={addTab}><Icon name="plus" /></button> : null}
      </div> : null}
      {shown.length ? <ul className="mp-opts">{shown.map((o, k) => <li key={o.name} className={o.kind === 'heading' ? 'mp-head' : ''}>
        <input className="mp-label" value={o.label} maxLength={30} aria-label={t('macro.panel.labelOf', { n: k + 1 })} title={t('macro.panel.labelTip')}
          onChange={e => setLabel(o.name, e.target.value)} />
        {o.kind === 'heading' ? null : <span className="mp-val">{value(o)}</span>}
        <span className="mp-acts">
          {tabs.length ? <button type="button" className="icon-btn" title={t('macro.tab.moveTo')} aria-label={`${t('macro.tab.moveTo')}: ${o.label}`}
            onClick={e => tabMenu(e.currentTarget, o)}><Icon name="chevronRight" /></button> : null}
          <button type="button" className="icon-btn" title={t('lo.up')} aria-label={`${t('lo.up')}: ${o.label}`} disabled={k === 0} onClick={() => move(o.name, -1)}><Icon name="chevronUp" /></button>
          <button type="button" className="icon-btn" title={t('lo.down')} aria-label={`${t('lo.down')}: ${o.label}`} disabled={k === shown.length - 1} onClick={() => move(o.name, 1)}><Icon name="chevronDown" /></button>
          <button type="button" className="icon-btn" title={t('macro.panel.remove')} aria-label={`${t('macro.panel.remove')}: ${o.label}`} onClick={() => remove(o.name)}><Icon name="x" /></button>
        </span>
      </li>)}</ul> : <p className="mp-none">{t(tabs.length ? 'macro.tab.empty' : 'macro.panel.none')}</p>}
      <div className="mp-foot">{areas ? <span className="mp-btn">{t('macro.panel.areas')}</span> : null}
        {options.length ? <span className="mp-btn">{t('macro.panel.reset')}</span> : null}</div>
    </div>
    {children}
  </section>;
}

/** The shown tab's name, typed in place: kept when it leaves the box, unless empty or another tab's. */
function TabName({ name, others, rename, label }: { name: string; others: string[]; rename: (name: string) => void; label: string }) {
  const [text, setText] = useState(name);
  const done = () => {
    const nu = text.trim();
    if (nu && nu !== name && !others.some(x => x.toLowerCase() === nu.toLowerCase())) rename(nu);
    else setText(name);
  };
  return <input className="mp-tab-name" value={text} maxLength={20} aria-label={label} size={Math.max(4, text.length)}
    onChange={e => setText(e.target.value)} onBlur={done} onKeyDown={e => { if (e.key === 'Enter') e.currentTarget.blur(); }} />;
}

/** A choice's options, typed with commas between them; kept when the box is left. */
function Choices({ o, set }: { o: Option; set: (list: string[]) => void }) {
  const { t } = useHost();
  const [text, setText] = useState((o.choices || []).join(', '));
  const done = () => {
    const list = [...new Map(text.split(',').map(x => x.trim().slice(0, 30)).filter(Boolean).map(x => [x.toLowerCase(), x])).values()].slice(0, 20);
    if (list.length) set(list);
    setText((list.length ? list : o.choices || []).join(', '));
  };
  return <input className="mp-field mp-choices" value={text} aria-label={t('macro.opt.choicesOf', { name: o.label })} title={t('macro.opt.choicesTip')}
    onChange={e => setText(e.target.value)} onBlur={done} onKeyDown={e => { if (e.key === 'Enter') e.currentTarget.blur(); }} />;
}
