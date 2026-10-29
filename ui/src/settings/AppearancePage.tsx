// Settings › Appearance: the theme (swatches by group; Custom's colours, picture or video, blur, saving it by name), your
// themes (kept by name, shared as a file), the language, and hiding account names.
import { useState, useSyncExternalStore, type CSSProperties, type KeyboardEvent } from 'react';
import { Icon, useAppState, useHost, type CustomTheme, type SavedTheme } from '../host';
import { Group, Page, Row, Switch, useLocal } from './parts';
import { ColorInput } from '../ColorField';

export function AppearancePage({ title }: { title: string }) {
  const { t, tIn, lang, call, toast, busy, themes, themePending, previewTheme, subscribe, customTheme, customVars, previewCustom } = useHost();
  const app = useAppState();
  // A theme picked with the arrow keys shows at once and saves once the keys rest (app.js previewTheme); marked here meanwhile.
  const pending = useSyncExternalStore(subscribe, themePending);
  const current = pending || app.theme || 'dark';
  // The custom theme as it shows, changes being dragged included: its swatch wears its colours.
  const custom = useSyncExternalStore(subscribe, customTheme);
  const customLook = custom ? customVars(custom) : null;
  const [hide, setHide] = useLocal(!!app.hideNames);
  const saved: SavedTheme[] = app.savedThemes || [];
  const [name, setName] = useState('');
  // The kept theme of the same look as the one in use (its name goes in the box to save it again under it).
  const same = (a: CustomTheme, b: CustomTheme) => a.background === b.background && a.accent === b.accent && a.fade === b.fade
    && (a.blur || 0) === (b.blur || 0) && a.picture === b.picture;
  const inUse = current === 'custom' && custom ? saved.find(s => same(s.look, custom)) : undefined;
  const [deleting, setDeleting] = useState<string | null>(null);

  // The swatches are one Tab stop; the arrow keys (Home/End to the ends) move between them and pick as they go, like radio buttons.
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
              // data-name: shown on hover and focus; data-theme gives each swatch its theme's own colours.
              return <button key={id} className={`swatch ${on ? 'on' : ''}`} data-act="theme" data-id={id} role="radio" aria-checked={on}
                tabIndex={on ? 0 : -1} aria-label={name} data-name={name}>
                {id === 'system' ? <><span data-theme="light" /><span data-theme="dark" /></>
                  : id === 'custom' && customLook ? <span data-theme="custom" data-tone={customLook.tone} style={{ ...customLook.vars, '--backdrop': customLook.swatch } as CSSProperties}><em /></span>
                  : <span data-theme={id}><em /></span>}
              </button>;
            })}
          </div>
        </div>)}
      </div>
      {/* Custom chosen: its two colours (shown as they are dragged, saved once they rest) and a picture behind the page. */}
      {current === 'custom' && custom ? <div className="ct" id="s-custom">
        <Row title={t('ct.background')} desc={t('ct.backgroundDesc')}>
          <ColorInput id="s-ct-bg" value={custom.background} label={t('ct.background')} onChange={v => previewCustom({ background: v })} />
        </Row>
        <Row title={t('ct.accent')} desc={t('ct.accentDesc')}>
          <ColorInput id="s-ct-accent" value={custom.accent} label={t('ct.accent')} onChange={v => previewCustom({ accent: v })} />
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
        {custom.picture ? <Row title={t('ct.blur')} desc={t('ct.blurDesc')}>
          <input type="range" id="s-ct-blur" min={0} max={30} step={1} value={custom.blur || 0} aria-label={t('ct.blur')}
            onChange={e => previewCustom({ blur: +e.target.value })} />
          <span className="range-v">{custom.blur || 0}</span>
        </Row> : null}
        <Row title={t('ct.save')} desc={t('ct.saveDesc')}>
          <input className="field sm ct-name" id="s-ct-name" maxLength={40} placeholder={inUse?.name || t('ct.namePh')} aria-label={t('ct.namePh')}
            value={name} onChange={e => setName(e.target.value)}
            onKeyDown={e => { if (e.key === 'Enter' && (name.trim() || inUse)) (e.currentTarget.nextElementSibling as HTMLElement)?.click(); }} />
          <button className="btn sm" id="s-ct-save" disabled={!name.trim() && !inUse}
            onClick={e => busy(e.currentTarget, async () => {
              const named = name.trim() || inUse!.name;
              if (await call('saveTheme', { name: named }) !== undefined) {
                setName('');
                toast(t('ct.savedToast', { name: named }), 'ok');
              }
            })}>{t('ct.saveBtn')}</button>
        </Row>
      </div> : null}
      <Row title={t('ct.yours')} desc={t(saved.length ? 'ct.yoursDesc' : 'ct.yoursNone')}>
        <button className="btn sm" id="s-ct-import" onClick={e => busy(e.currentTarget, () => call('importTheme'))}>
          <Icon name="folder" />{t('ct.import')}</button>
      </Row>
      {saved.length ? <div className="ct-saved" id="s-saved-themes" role="list">
        {saved.map(s => {
          const look = customVars(s.look), on = s.id === inUse?.id;
          return <div className={`ct-theme${on ? ' on' : ''}`} role="listitem" key={s.id} data-id={s.id}>
            <button className="ct-use" aria-pressed={on} title={t('ct.use')} onClick={() => call('useSavedTheme', { id: s.id })}>
              <span className="ct-dot" style={{ background: look.swatch, '--inv': s.look.accent } as CSSProperties} />
              <span className="ct-tname">{s.name}</span>
              {s.look.video ? <span className="ct-kind">{t('ct.video')}</span> : null}
            </button>
            <button className="icon-btn sm" title={t('ct.export')} aria-label={`${t('ct.export')}: ${s.name}`}
              onClick={async e => {
                const done = await busy(e.currentTarget, () => call('exportTheme', { id: s.id })) as { saved?: string } | null | undefined;
                if (done?.saved) toast(t('ct.exported', { file: done.saved }), 'ok');
              }}><Icon name="download" /></button>
            {deleting === s.id
              ? <button className="btn sm danger" onClick={() => { setDeleting(null); call('deleteSavedTheme', { id: s.id }); }}
                  onBlur={() => setDeleting(null)} autoFocus>{t('ct.deleteSure')}</button>
              : <button className="icon-btn sm ct-del" title={t('ct.delete')} aria-label={`${t('ct.delete')}: ${s.name}`}
                  onClick={() => setDeleting(s.id)}><Icon name="trash" /></button>}
          </div>;
        })}
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
