// Accounts in an order to change: rows are dragged (anywhere on the row) into another order, or moved with the arrow
// buttons (keyboard). The launch-order dialog numbers them; the preset form ticks them instead (a click on the row
// ticks it too), and the ticked ones, in this order, are the preset's launch order.
import { useImperativeHandle, useRef, useState, type Ref } from 'react';
import { flushSync } from 'react-dom';
import { Icon, useHost } from './host';

export type OrderAccount = { id: string; label: string; username?: string };
export interface OrderListApi {
  /** Every row's account, top to bottom. */
  ids(): string[];
  /** The ticked ones, top to bottom. */
  ticked(): string[];
}

export function OrderList({ accounts, ticked, api, onChange }: {
  accounts: OrderAccount[]; ticked?: string[]; api?: Ref<OrderListApi>; onChange?: () => void;
}) {
  const { t, avatarHtml } = useHost();
  const byId = new Map(accounts.map(a => [a.id, a]));
  const [order, setOrder] = useState(() => accounts.map(a => a.id));
  const [checked, setChecked] = useState(() => new Set(ticked || []));
  const [dragging, setDragging] = useState<string | null>(null);
  const list = useRef<HTMLDivElement>(null);
  const latest = useRef({ order, checked });
  latest.current = { order, checked };
  useImperativeHandle(api, () => ({
    ids: () => latest.current.order,
    ticked: () => latest.current.order.filter(id => latest.current.checked.has(id)),
  }), []);

  const tick = (id: string, on: boolean) => {
    const next = new Set(checked);
    if (on) next.add(id); else next.delete(id);
    flushSync(() => setChecked(next));
    onChange?.();
  };
  const move = (id: string, dir: number, btn: HTMLElement) => {
    const i = order.indexOf(id), j = i + dir;
    if (j < 0 || j >= order.length) return;
    const next = [...order];
    [next[i], next[j]] = [next[j], next[i]];
    flushSync(() => setOrder(next));
    btn.focus();
    btn.closest('.ord-item')?.scrollIntoView({ block: 'nearest' });
    onChange?.();
  };

  return <div className="ord-list" role="list" ref={list}
    onDragOver={e => {
      if (!dragging) return;
      e.preventDefault();
      e.dataTransfer.dropEffect = 'move';
      const over = (e.target as HTMLElement).closest<HTMLElement>('.ord-item');
      if (!over || over.dataset.id === dragging) return;
      const r = over.getBoundingClientRect();
      const next = order.filter(id => id !== dragging);
      next.splice(next.indexOf(over.dataset.id!) + (e.clientY < r.top + r.height / 2 ? 0 : 1), 0, dragging);
      if (next.join() !== order.join()) setOrder(next);
    }}
    onDrop={e => e.preventDefault()}
    onDragEnd={() => { setDragging(null); onChange?.(); }}>
    {order.map((id, i) => {
      const a = byId.get(id)!;
      return <div key={id} className={`ord-item${dragging === id ? ' dragging' : ''}`} role="listitem" draggable="true" data-id={id}
        onDragStart={e => {
          setDragging(id);
          e.dataTransfer.effectAllowed = 'move';
          e.dataTransfer.setData('text/plain', id); // Chromium needs some data for the drag to start
        }}
        onClick={e => {
          // The row itself ticks its box (the box, the handle and the buttons do their own thing).
          if (ticked && !(e.target as HTMLElement).closest('input, .ord-grip, button')) tick(id, !checked.has(id));
        }}>
        <span className="ord-grip" title={t('lo.drag')}><Icon name="grip" /></span>
        {ticked ? <input type="checkbox" value={id} checked={checked.has(id)} aria-label={a.label} onChange={e => tick(id, e.target.checked)} />
          : <span className="ord-n">{i + 1}</span>}
        <div className="avatar sm" dangerouslySetInnerHTML={{ __html: avatarHtml(a) }} />
        <span className="ord-txt"><b title={a.label}>{a.label}</b><small>{a.username ? `@${a.username}` : ''}</small></span>
        <span className="ord-btns">
          <button type="button" className="icon-btn" data-move="-1" title={t('lo.up')} aria-label={t('lo.up')} onClick={e => move(id, -1, e.currentTarget)}><Icon name="chevronUp" /></button>
          <button type="button" className="icon-btn" data-move="1" title={t('lo.down')} aria-label={t('lo.down')} onClick={e => move(id, 1, e.currentTarget)}><Icon name="chevronDown" /></button>
        </span>
      </div>;
    })}
  </div>;
}

/** The ⋯ menu's "Launch order…": its text, then every account to put in the order Play opens them in. */
export function LaunchOrder({ accounts, api }: { accounts: OrderAccount[]; api: Ref<OrderListApi> }) {
  const { t } = useHost();
  return <><p className="modal-text">{t('lo.text')}</p><OrderList accounts={accounts} api={api} /></>;
}
