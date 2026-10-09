// Hover tilt (UI study handoff §7b D; Burhan 2026-10-06 19:33: "I WISH IT COULD ONLY BEHAVE THAT WAY ONCE THE MOUSE IS
// HOVERING ON TOP OF IT, WHERE IT SHIFTS PERSPECTIVE A BIT AND REMAINS LIKE THAT UNTIL THE MOUSE HOVERING IS STOPPED").
// At rest a panel is flat and still; while the pointer is over it, it carries data-tilt and depth.css tilts it.
//
// Not :hover. A tilt pulls the panel's far edge in by a few pixels, so a still pointer near that edge would fall off
// the panel, the panel would settle back under it, and it would tilt again: the juggling he refused. The pointer is
// held against the panel's untransformed layout box instead, with a small margin.
//
// The plan inset does not tilt: the engine draws the plan into the canvas under the inset's box and redraws only on
// demand, so a moving box would leave the plan behind. Menus do not tilt either (motion.css owns their transform).
// Nothing tilts under prefers-reduced-motion or WebDriver: the gates click through the view's centre and measure
// panels, and a transformed panel must not move under them.

const PANELS = ".scene, .props, .stepbar, .statusbar, .viewport .float-panel:not(button):not([role='menu'])";
const MARGIN = 4;
const reduced = matchMedia('(prefers-reduced-motion: reduce)');

let tilted: HTMLElement | null = null;

function set(el: HTMLElement | null): void {
  if (el === tilted) return;
  tilted?.removeAttribute('data-tilt');
  tilted = el;
  el?.setAttribute('data-tilt', '');
}

/** The element's box as laid out, in client pixels, ignoring its transform (offset* never sees one). */
function layoutBox(el: HTMLElement): { x: number; y: number; w: number; h: number } {
  let x = 0;
  let y = 0;
  for (let n: HTMLElement | null = el; n; n = n.offsetParent as HTMLElement | null) {
    x += n.offsetLeft + n.clientLeft;
    y += n.offsetTop + n.clientTop;
  }
  return { x, y, w: el.offsetWidth, h: el.offsetHeight };
}

export function installTilt(): void {
  if (navigator.webdriver) return;
  document.addEventListener(
    'pointermove',
    (e) => {
      if (reduced.matches || e.pointerType === 'touch') return set(null);
      const over = e.target instanceof Element ? e.target.closest<HTMLElement>(PANELS) : null;
      if (over) return set(over);
      if (tilted?.isConnected) {
        const b = layoutBox(tilted);
        const inside = e.clientX >= b.x - MARGIN && e.clientX <= b.x + b.w + MARGIN && e.clientY >= b.y - MARGIN && e.clientY <= b.y + b.h + MARGIN;
        if (inside) return;
      }
      set(null);
    },
    { passive: true },
  );
  document.documentElement.addEventListener('pointerleave', () => set(null));
  window.addEventListener('blur', () => set(null));
  reduced.addEventListener('change', () => set(null));
}
