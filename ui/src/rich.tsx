// The few interface strings with markup in them (i18n.js: "<b>Close every Roblox</b>, including…") as React nodes,
// laid out exactly as app.js puts them in with innerHTML: the text and its tags straight inside the parent, no wrapper.
// Only for the app's own strings: the values filled in ({name}) are escaped first, so a name can't add markup.
import { Fragment, type ReactNode } from 'react';
import { useHost } from './host';

const escape = (s: string) => s.replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]!));
const unescape = (s: string) => s.replace(/&(amp|lt|gt|quot|#39);/g, (_, e: string) => ({ amp: '&', lt: '<', gt: '>', quot: '"', '#39': "'" }[e]!));

/** The tags the strings use (b, span, button, br); attributes kept as they are (class → className). */
export function richNodes(html: string): ReactNode[] {
  type Node = { tag: string; attrs: Record<string, string>; kids: ReactNode[] };
  const root: Node = { tag: '', attrs: {}, kids: [] };
  const stack = [root];
  const re = /<(b|i|span|button|code|small)(\s[^>]*)?>|<\/(b|i|span|button|code|small)>|<br\s*\/?>/g;
  let at = 0, key = 0, m: RegExpExecArray | null;
  const text = (s: string) => { if (s) stack[stack.length - 1].kids.push(unescape(s)); };
  while ((m = re.exec(html))) {
    text(html.slice(at, m.index));
    at = re.lastIndex;
    if (m[0].startsWith('<br')) stack[stack.length - 1].kids.push(<br key={key++} />);
    else if (m[1]) {
      const attrs: Record<string, string> = {};
      for (const a of (m[2] || '').matchAll(/([\w-]+)="([^"]*)"/g)) attrs[a[1] === 'class' ? 'className' : a[1]] = unescape(a[2]);
      stack.push({ tag: m[1], attrs, kids: [] });
    } else if (stack.length > 1) {
      const node = stack.pop()!;
      const Tag = node.tag as 'b';
      stack[stack.length - 1].kids.push(<Tag key={key++} {...node.attrs}>{node.kids}</Tag>);
    }
  }
  text(html.slice(at));
  return root.kids;
}

/** An interface string with markup, its {values} escaped: <Rich k="mh.s1" /> */
export function Rich({ k, vars }: { k: string; vars?: Record<string, string | number> }) {
  const { t } = useHost();
  const safe = vars && Object.fromEntries(Object.entries(vars).map(([n, v]) => [n, typeof v === 'string' ? escape(v) : v]));
  return <Fragment>{richNodes(t(k, safe))}</Fragment>;
}
