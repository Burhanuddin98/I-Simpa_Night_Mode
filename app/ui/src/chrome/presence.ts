// Burhan's 10-05 UI list, item 14: a drop-down or menu animates closing as it animates opening
// (motion.css). Closing needs the menu to stay mounted while it fades and lifts away, then go:
// `presenceAfter` is that rule, `usePresence` (usePresence.ts) runs it. Under
// prefers-reduced-motion or WebDriver nothing moves (motion.css), so a closed menu unmounts at
// once there, as it did before: the gates still find it gone the moment it closes. A pure
// module: no store, no DOM, only erasable TypeScript, tested by presence.test.ts under `node --test`.

/** Where a menu is: open, on its way out, or gone. */
export type Presence = 'shown' | 'closing' | 'gone';

/** How long a closing menu stays mounted: the longest of its two transitions (motion.css, the
 * transform's 170 ms; the opacity's 150 ms ends first). */
export const CLOSE_MS = 170;

/**
 * The next state of a menu that was `prev`, now `open` or not. Opening always shows it, a
 * closing one included (it turns back); a shown one that closes plays its way out when `animate`,
 * else goes at once; a gone one stays gone. The end of the way out (`CLOSE_MS` later) is the
 * caller's to make `gone`.
 */
export function presenceAfter(prev: Presence, open: boolean, animate: boolean): Presence {
  if (open) return 'shown';
  if (prev === 'shown') return animate ? 'closing' : 'gone';
  if (prev === 'closing' && !animate) return 'gone';
  return prev;
}

/** Whether a closing menu animates: not under prefers-reduced-motion, nor under WebDriver. */
export function closeAnimates(reducedMotion: boolean, webdriver: boolean): boolean {
  return !reducedMotion && !webdriver;
}
