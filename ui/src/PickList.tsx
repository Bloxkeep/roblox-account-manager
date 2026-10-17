// A short list to tick, under a line saying what it is for: the body of the pickers app.js opens (what the Discord
// summary lists, which accounts it covers, which accounts use the Roblox settings). Their Done and Cancel stay in the
// dialog's footer (app.js), which reads what is ticked through PickListApi.
import { useImperativeHandle, useRef, useState, type Ref } from 'react';
import { useHost } from './host';
import { Avatar } from './Avatar';

export interface PickItem {
  value: string;
  checked: boolean;
  label: string;
  /** A second line (an account's @username), and the account whose picture goes with it. */
  sub?: string;
  avatar?: { avatarUrl?: string | null; username?: string; label?: string };
}
export interface PickListApi { values(): string[] }

export function PickList({ text, items, api }: { text: string; items: PickItem[]; api: Ref<PickListApi> }) {
  useHost();
  const [checked, setChecked] = useState(() => new Set(items.filter(i => i.checked).map(i => i.value)));
  const current = useRef(checked);
  current.current = checked;
  useImperativeHandle(api, () => ({ values: () => items.map(i => i.value).filter(v => current.current.has(v)) }), [items]);
  const toggle = (value: string, on: boolean) => setChecked(old => {
    const next = new Set(old);
    if (on) next.add(value); else next.delete(value);
    return next;
  });

  return <>
    <p className="modal-text">{text}</p>
    <div className="rc-list">{items.map(i => <label key={i.value} className="rc-item">
      <input type="checkbox" value={i.value} checked={checked.has(i.value)} onChange={e => toggle(i.value, e.target.checked)} />
      {i.avatar ? <div className="avatar sm"><Avatar a={i.avatar} /></div> : null}
      <span><b title={i.sub !== undefined ? i.label : undefined}>{i.label}</b>{i.sub !== undefined ? <small>{i.sub}</small> : null}</span>
    </label>)}</div>
  </>;
}
