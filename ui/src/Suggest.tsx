// A text box with Bloxkeep's own suggestions under it (an account's group, a preset's player to follow). It took the
// browser's <datalist>, whose list came up in the browser's own colours and style (review, 2026-10-30). Like it, the list
// shows what was used before that matches what is typed; a click in the box opens it and a second click closes it,
// arrows move through it, Enter takes one, Escape, a click outside or leaving the box closes it.
import { useEffect, useId, useRef, useState, type InputHTMLAttributes, type KeyboardEvent, type Ref } from 'react';
import { Popup } from './TimeField';

type Props = Omit<InputHTMLAttributes<HTMLInputElement>, 'value' | 'onChange' | 'list'> & {
  ref?: Ref<HTMLInputElement>;
  value: string;
  setValue: (v: string) => void;
  options: string[];
  /** What the list is (read out by a screen reader). */
  label: string;
};

export function SuggestInput({ ref, value, setValue, options, label, onKeyDown, onBlur, ...attrs }: Props) {
  const box = useRef<HTMLInputElement | null>(null);
  const [open, setOpen] = useState(false);
  const [index, setIndex] = useState(-1);
  const id = useId();
  const typed = value.trim().toLowerCase();
  // What is typed already, exactly, isn't offered again.
  const rows = options.filter(o => o.toLowerCase().includes(typed) && o.toLowerCase() !== typed);
  const shown = open && rows.length > 0;
  const close = () => { setOpen(false); setIndex(-1); };
  const pick = (v: string) => { setValue(v); close(); };
  // The one the arrows reached stays in sight in a long list.
  useEffect(() => { if (index >= 0) document.getElementById(`${id}-${index}`)?.scrollIntoView({ block: 'nearest' }); }, [index]);

  const keys = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      if (!shown) { setOpen(true); setIndex(-1); return; }
      const n = rows.length;
      setIndex(i => e.key === 'ArrowDown' ? (i + 1) % n : i <= 0 ? n - 1 : i - 1);
    } else if (e.key === 'Enter' && shown) {
      // The pick, not the dialog's Save (it leaves a box with its list open alone); none picked: the list goes, and the
      // next Enter saves.
      e.preventDefault();
      if (index >= 0 && index < rows.length) pick(rows[index]); else close();
    }
    onKeyDown?.(e);
  };

  return <>
    <input {...attrs} ref={el => {
      box.current = el;
      if (typeof ref === 'function') ref(el); else if (ref) ref.current = el;
    }} value={value} autoComplete="off" role="combobox" aria-autocomplete="list" aria-haspopup="listbox" aria-expanded={shown}
      aria-controls={shown ? id : undefined} aria-activedescendant={shown && index >= 0 ? `${id}-${index}` : undefined}
      onChange={e => { setValue(e.target.value); setOpen(true); setIndex(-1); }}
      onClick={() => { if (open) close(); else setOpen(true); }}
      onKeyDown={keys}
      onBlur={e => { close(); onBlur?.(e); }} />
    {shown && box.current ? <Popup anchor={box.current} close={close} label={label} role="listbox" fit id={id}>
      {/* mousedown kept from the box: it stays focused while a suggestion is taken. */}
      <div className="sg-list" onMouseDown={e => e.preventDefault()}>
        {rows.map((o, i) => <div key={o} id={`${id}-${i}`} role="option" aria-selected={i === index} className={i === index ? 'on' : ''}
          title={o} onClick={() => pick(o)}>{o}</div>)}
      </div>
    </Popup> : null}
  </>;
}
