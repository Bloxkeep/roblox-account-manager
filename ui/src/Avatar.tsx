// An account's picture as the page draws it everywhere (cards, rows, pickers, lists): the image, or the account's
// initials on a colour of its own when it has none or the image doesn't load (offline, gone from Roblox's CDN).
import { useState } from 'react';

type Account = { avatarUrl?: string | null; username?: string; label?: string };

const hue = (s: string) => { let h = 7; for (const c of String(s)) h = (h * 31 + c.charCodeAt(0)) >>> 0; return h % 360; };
const initials = (s: string) => (String(s).match(/[\p{L}\p{N}]/gu) || ['?']).slice(0, 2).join('').toUpperCase();

export function Avatar({ a }: { a: Account }) {
  const [failed, setFailed] = useState('');
  const style = { '--h': hue(a.username || a.label || '') } as React.CSSProperties;
  return a.avatarUrl && failed !== a.avatarUrl
    ? <img src={a.avatarUrl} alt="" draggable="false" loading="lazy" decoding="async" style={style} onError={() => setFailed(a.avatarUrl!)} />
    : <span className="initials" style={style}>{initials(a.label || a.username || '')}</span>;
}

