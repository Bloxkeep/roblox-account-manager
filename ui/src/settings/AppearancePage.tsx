// Settings › Appearance: the theme (its name, then one small swatch per theme, a line per group; with Custom chosen,
// its colours and picture), the language, and hiding account names.
import { useSyncExternalStore, type CSSProperties, type KeyboardEvent } from 'react';
import { Icon, useAppState, useHost } from '../host';
import { Group, Page, Row, Switch, useLocal } from './parts';

export function AppearancePage({ title }: { title: string }) {
  const { t, tIn, lang, call, themes, themePending, previewTheme, subscribe, customTheme, customVars, previewCustom } = useHost();
  const app = useAppState();
  // A theme picked with the arrow keys shows at once and is saved once the keys rest (app.js previewTheme); until then
  // it is the one marked here.
  const pending = useSyncExternalStore(subscribe, themePending);
  const current = pending || app.theme || 'dark';
  // The custom theme as it shows, changes being dragged included: its swatch wears its colours.
  const custom = useSyncExternalStore(subscribe, customTheme);
  const customLook = custom ? customVars(custom) : null;
  const [hide, setHide] = useLocal(!!app.hideNames);

  // The swatches are one Tab stop; the arrow keys (Home/End to the ends) move between them and pick as they go, like
  // radio buttons.
  const keys = (e: KeyboardEvent<HTMLDivElement>) => {
    const step = ({ ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 } as Record<string, number>)[e.key];
    const from = (e.target as HTMLElement).closest<HTMLElement>('.swatch');
    if (!from || (!step && e.key !== 'Home' && e.key !== 'End')) return;
    e.preventDefault();
    const all = [...e.currentTarget.querySelectorAll<HTMLElement>('.swatch')];
    const next = e.key === 'Home' ? all[0] : e.key === 'End' ? all[all.length - 1] : all[(all.indexOf(from) + step + all.length) % all.length];
    previewTheme(next.dataset.id!);
    next.focus();
  };

  return <Page id="appearance" title={title}>
    <Group id="appearance">
      <Row title={t('set.theme')} desc={<span id="s-theme-name">{t(`theme.${current}`)}</span>} />
      <div className="themes" id="s-themes" role="radiogroup" aria-label={t('set.theme')} onKeyDown={keys}>
        {Object.entries(themes).map(([group, ids]) => <div className="tg" key={group}>
          <span className="tg-label">{t(`theme.group.${group}`)}</span>
          <div className="tg-list">
            {ids.map(id => {
              const on = id === current, name = t(`theme.${id}`);
              // data-name: the name shown on hover and focus. Each swatch carries data-theme, so it shows that theme's
              // own colours.
              return <button key={id} className={`swatch ${on ? 'on' : ''}`} data-act="theme" data-id={id} role="radio" aria-checked={on}
                tabIndex={on ? 0 : -1} aria-label={name} data-name={name}>
                {id === 'system' ? <><span data-theme="light" /><span data-theme="dark" /></>
                  : id === 'custom' && customLook ? <span data-theme="custom" data-tone={customLook.tone} style={customLook.vars as CSSProperties}><em /></span>
                  : <span data-theme={id}><em /></span>}
              </button>;
            })}
          </div>
        </div>)}
      </div>
      {/* Custom chosen: its two colours (shown as they are dragged, saved once they rest) and a picture behind the page. */}
      {current === 'custom' && custom ? <div className="ct" id="s-custom">
        <Row title={t('ct.background')} desc={t('ct.backgroundDesc')}>
          <input type="color" className="ct-color" id="s-ct-bg" value={custom.background} aria-label={t('ct.background')}
            onChange={e => previewCustom({ background: e.target.value })} />
        </Row>
        <Row title={t('ct.accent')} desc={t('ct.accentDesc')}>
          <input type="color" className="ct-color" id="s-ct-accent" value={custom.accent} aria-label={t('ct.accent')}
            onChange={e => previewCustom({ accent: e.target.value })} />
        </Row>
        <Row title={t('ct.picture')} desc={t('ct.pictureDesc')}>
          {custom.picture ? <button className="btn sm ghost" data-act="ct-clear">{t('ct.remove')}</button> : null}
          <button className="btn sm" data-act="ct-pick"><Icon name="image" />{t(custom.picture ? 'ct.change' : 'ct.pick')}</button>
        </Row>
        {custom.picture ? <Row title={t('ct.fade')} desc={t('ct.fadeDesc')}>
          <input type="range" id="s-ct-fade" min={0} max={90} step={5} value={custom.fade} aria-label={t('ct.fade')}
            onChange={e => previewCustom({ fade: +e.target.value })} />
          <span className="range-v">{custom.fade}%</span>
        </Row> : null}
      </div> : null}
      <Row title={t('set.language')}>
        <div className="lang" role="group" aria-label="Language">
          {(['en', 'vi'] as const).map(l => <button key={l} data-act="lang" data-lang={l} className={l === lang ? 'on' : ''} aria-pressed={l === lang}
            title={tIn(l, 'lang.name')}>{l.toUpperCase()}</button>)}
        </div>
      </Row>
      <Row title={t('set.hideNames')} desc={t('set.hideNamesDesc')}>
        <Switch id="s-hidenames" label={t('set.hideNames')} checked={hide} onChange={on => { setHide(on); call('setHideNames', { enabled: on }); }} />
      </Row>
    </Group>
  </Page>;
}
