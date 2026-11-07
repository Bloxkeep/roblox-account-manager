// Asking the app for something a part shows, again and again while the part is shown and still has no answer. Asked
// once only, a part stayed on "Connecting…" when that one ask came back empty (refused while Bloxkeep was locked, as it
// is for a moment after Restart to update, or the bot or the wiki not answering): only leaving the tab and coming
// back asked again (2026-11-07).
import { useEffect, useRef, useState } from 'react';

/** How long an ask may go unanswered before another is sent all the same. */
const STALE = 15_000;
/** How often it looks again while still without an answer. */
const EVERY = 4_000;

/** While shown and needed: asks now, then every few seconds until needed turns false (ask sets what it got). */
export function useAskWhile(shown: boolean, needed: boolean, ask: () => Promise<unknown>) {
  const since = useRef(0);
  const [round, setRound] = useState(0);
  useEffect(() => {
    if (!shown || !needed) return;
    const now = Date.now();
    if (!since.current || now - since.current > STALE) {
      since.current = now;
      ask().catch(() => {}).finally(() => { since.current = 0; });
    }
    const timer = setTimeout(() => setRound(n => n + 1), EVERY);
    return () => clearTimeout(timer);
  }, [shown, needed, round]);
}
