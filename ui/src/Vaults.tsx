// Several vaults (RobloxManager/Vaults.cs): each one's face (its picture, or its name's first letter on a colour of its
// own), and the lock screen's picker, Steam-like ("Who's using Bloxkeep?"): a tile per vault, + to add one, and whether
// it asks every time Bloxkeep starts. The app decides when the picker shows (its vaults.pick); app.js handles the
// buttons (data-act vault-*).
import { useState } from 'react';
import { Icon, useAppState, useHost } from './host';

/** A vault as the app lists it: its picture is a "user/…" file it serves, or none. */
export interface VaultInfo { id: string; name: string; picture: string | null }

/** Solid colours for a vault without a picture, one per vault by its id (the same one every time). */
const FACE_COLORS = ['#2563eb', '#16a34a', '#d97706', '#dc2626', '#7c3aed', '#0891b2', '#db2777', '#0d9488'];
// FNV-1a: ids that differ by a character or two still get different colours.
const faceColor = (id: string) => {
  let h = [...id].reduce((h, c) => Math.imul(h ^ c.charCodeAt(0), 16777619), 2166136261);
  h ^= h >>> 16; h = Math.imul(h, 0x45d9f3b); h ^= h >>> 16; // mixed: FNV's low bits alone repeat for similar ids
  return FACE_COLORS[(h >>> 0) % FACE_COLORS.length];
};

/** A vault's picture, or the first letter of its name on its colour; a picture that doesn't load falls back to the letter. */
export function VaultFace({ vault, className = '' }: { vault: VaultInfo; className?: string }) {
  const [failed, setFailed] = useState('');
  return vault.picture && failed !== vault.picture
    ? <span className={`vault-face ${className}`}><img src={vault.picture} alt="" onError={() => setFailed(vault.picture!)} /></span>
    : <span className={`vault-face letter ${className}`} style={{ background: faceColor(vault.id) }} aria-hidden="true">
      {[...vault.name.trim()][0]?.toLocaleUpperCase() || '?'}</span>;
}

/** The lock screen's picker (inside its frame, Lock.tsx): the vaults on this PC, + to add one, and the switch for asking at every start. */
export function VaultPicker() {
  const { t, logo, call } = useHost();
  const vaults = useAppState().vaults || {};
  const list: VaultInfo[] = vaults.list || [];
  const [ask, setAsk] = useState(!!vaults.ask);
  return <div className="vp" id="vault-picker">
    <img className="logo-img" src={logo} alt="" />
    <h2>{t('vault.who')}</h2>
    <ul className="vp-tiles">
      {list.map(v => <li key={v.id} className="vp-item">
        <button className={`vp-tile ${v.id === vaults.current ? 'last' : ''}`} data-act="vault-select" data-id={v.id} title={v.name}>
          <VaultFace vault={v} /></button>
        <span className="vp-name" title={v.name}>{v.name}</span>
        {/* Not the vault open behind the picker: switch away from it first. */}
        {v.id === vaults.open ? null : <button className="icon-btn vp-remove" data-act="vault-remove" data-id={v.id} title={t('vault.remove')}
          aria-label={`${t('vault.remove')}: ${v.name}`}><Icon name="x" /></button>}
      </li>)}
      <li className="vp-item">
        <button className="vp-tile vp-add" data-act="vault-add" title={t('vault.add')} aria-label={t('vault.add')}><Icon name="plus" /></button>
        <span className="vp-name">{t('vault.add')}</span>
      </li>
    </ul>
    <label className="vp-ask">
      <span>{t('vault.ask')}</span>
      <span className="switch"><input type="checkbox" id="vault-ask" checked={ask}
        onChange={e => { setAsk(e.target.checked); call('setAskVault', { enabled: e.target.checked }); }} /><span /></span>
    </label>
  </div>;
}
