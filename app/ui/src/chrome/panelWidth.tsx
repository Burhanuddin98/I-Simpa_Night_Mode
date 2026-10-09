// The side panels' widths, dragged by a grip on their inner edge (GUI audit 2026-10-09 B1: both panels were the same
// 248 and 344 px at 1280, 1536 and 1920 px windows, and nothing could be made wider). The width is kept in this
// browser profile, as the dock's height is; never narrower than the design's width, never wider than 45 % of the
// window. A double-click on the grip gives the design's width back. While a grip is held, html[data-resizing] is set:
// the panel's fold transition and the hover tilt (tilt.ts) stand still.
import { useEffect, useRef, useState, type CSSProperties, type PointerEvent, type UIEvent } from 'react';

const MAX_SHARE = 0.45;

function stored(key: string, base: number): number {
  try {
    const n = Number(localStorage.getItem(key));
    return Number.isFinite(n) && n >= base ? Math.round(n) : base;
  } catch {
    return base;
  }
}

function clamp(w: number, base: number): number {
  return Math.round(Math.max(base, Math.min(w, Math.max(base, window.innerWidth * MAX_SHARE))));
}

/** The panel's width and its grip. `edge` is the panel's inner edge: 'right' for the scene list, 'left' for the properties. */
export function usePanelWidth(key: string, base: number, edge: 'left' | 'right') {
  const [width, setWidth] = useState(() => clamp(stored(key, base), base));
  const drag = useRef<{ x: number; w: number } | null>(null);
  useEffect(() => {
    const fit = () => setWidth((w) => clamp(w, base));
    window.addEventListener('resize', fit);
    return () => window.removeEventListener('resize', fit);
  }, [base]);
  const save = (w: number) => {
    try {
      localStorage.setItem(key, String(w));
    } catch {
      // A profile that keeps nothing keeps the design's width next time.
    }
  };
  const grip = {
    className: `panel-grip ${edge}`,
    role: 'separator',
    'aria-orientation': 'vertical' as const,
    'aria-label': 'Drag to resize; double-click for the default width',
    title: 'Drag to resize · double-click for the default width',
    onPointerDown: (e: PointerEvent<HTMLDivElement>) => {
      if (e.button !== 0) return;
      e.preventDefault();
      e.currentTarget.setPointerCapture(e.pointerId);
      drag.current = { x: e.clientX, w: width };
      document.documentElement.setAttribute('data-resizing', '');
    },
    onPointerMove: (e: PointerEvent<HTMLDivElement>) => {
      const d = drag.current;
      if (!d) return;
      const dx = e.clientX - d.x;
      setWidth(clamp(edge === 'right' ? d.w + dx : d.w - dx, base));
    },
    onPointerUp: () => {
      if (!drag.current) return;
      drag.current = null;
      document.documentElement.removeAttribute('data-resizing');
      setWidth((w) => {
        save(w);
        return w;
      });
    },
    onDoubleClick: () => {
      setWidth(base);
      save(base);
    },
  };
  // The grip sits on the panel's edge inside the panel, which scrolls: it is moved back by what the panel scrolled.
  const gripRef = useRef<HTMLDivElement>(null);
  const onScroll = (e: UIEvent<HTMLElement>) => {
    if (gripRef.current) gripRef.current.style.transform = `translateY(${e.currentTarget.scrollTop}px)`;
  };
  const style = { '--panel-w': `${width}px` } as CSSProperties;
  return { style, grip: { ...grip, ref: gripRef }, onScroll };
}
