// Runs presence.ts's rule for one menu (Burhan's 10-05 UI list, item 14): the value it shows,
// kept while it closes so the menu leaves with what it held, and whether it is closing. A closing
// menu carries `data-closing` (motion.css plays the way out from it) and is inert: no click or
// key reaches it, and nothing that looks for an open menu (the dock's Escape) counts it.
import { useEffect, useRef, useState } from 'react';
import { CLOSE_MS, closeAnimates, presenceAfter, type Presence } from './presence';

function animates(): boolean {
  try {
    return closeAnimates(matchMedia('(prefers-reduced-motion: reduce)').matches, navigator.webdriver === true);
  } catch {
    return false;
  }
}

/**
 * `value` is what the menu shows while open (`true`, a menu's name, a position), `null` or
 * `false` when closed. Returns the value to render (the last open one while it closes; `null`
 * once gone) and whether it is on its way out.
 */
export function usePresence<T>(value: T | null | false): { shown: T | null; closing: boolean } {
  const open = value !== null && value !== false;
  const last = useRef<T | null>(open ? (value as T) : null);
  if (open) last.current = value as T;
  const [state, setState] = useState<Presence>(open ? 'shown' : 'gone');
  const next = presenceAfter(state, open, animates());
  // A change of state during render: React re-renders at once with it, before painting.
  if (next !== state) setState(next);
  useEffect(() => {
    if (next !== 'closing') return;
    const t = setTimeout(() => setState((s) => (s === 'closing' ? 'gone' : s)), CLOSE_MS);
    return () => clearTimeout(t);
  }, [next]);
  if (next === 'gone') last.current = null;
  return { shown: next === 'gone' ? null : last.current, closing: next === 'closing' };
}

/** The props a closing menu carries: the mark motion.css plays its way out from, and inert. */
export function closingProps(closing: boolean): { 'data-closing'?: ''; inert?: boolean; 'aria-hidden'?: true } {
  return closing ? { 'data-closing': '', inert: true, 'aria-hidden': true } : {};
}
