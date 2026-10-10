// A chart opened large (Burhan 2026-10-10 04:11, "those plots and little itsy bitsy graphs ... need to be
// expanded in window size so we can see them better"): every chart of the app has an Expand button that
// opens the same chart in a floating window on the response window's frame (`useFrame`: moved by its title
// bar, resized by its corner), at 80 % of the app's window. Several can be open, cascaded so none hides
// another; a click raises one, Escape closes the one on top (and only it), the × closes it. The chart in it
// is the small one's, drawn from the same arrays and with the same controls; this file is only the frame.
// Nothing here moves on its own, so prefers-reduced-motion and WebDriver need nothing of it.
import { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { useFrame } from './ResponseWindow';

/** The open windows, the top one last. Escape closes the top one. */
const stack: { id: number; close: () => void }[] = [];
let nextId = 1;
let nextZ = 1;

/** Escape, in the capture phase, so the window under it (the response window, a menu) keeps its own. A
 * key typed into a field is the field's. */
function onKey(e: KeyboardEvent) {
  if (e.key !== 'Escape' || !stack.length) return;
  if ((e.target as HTMLElement | null)?.closest?.('input, select, textarea')) return;
  e.stopImmediatePropagation();
  e.preventDefault();
  stack[stack.length - 1].close();
}

/** R62/R63/R72's controls apply to the large chart as to the small: this opens it, pressed while it is open. */
export function ExpandButton({ open, chart, onClick }: { open: boolean; chart: string; onClick: () => void }) {
  return (
    <button
      type="button"
      className="small-button ac-copy"
      data-action="expand-chart"
      data-chart={chart}
      aria-pressed={open}
      title={open ? 'Close the large window of this chart' : 'Open this chart in a large window, moved by its title bar and resized by its corner'}
      onClick={onClick}
    >
      Expand
    </button>
  );
}

export function ChartWindow({
  chart,
  title,
  sub,
  onClose,
  className = '',
  children,
}: {
  /** The chart's `data-part`; the window is `[data-chart-window=<chart>]`. */
  chart: string;
  title: string;
  sub?: React.ReactNode;
  onClose: () => void;
  /** More classes on the window (`cw-legend`: a colour legend, lower, no chart). */
  className?: string;
  children: React.ReactNode;
}) {
  const frame = useFrame();
  const close = useRef(onClose);
  close.current = onClose;
  const [me] = useState(() => ({ id: nextId++, close: () => close.current() }));
  // Cascaded by how many were open when it opened, so a second window does not sit exactly on the first.
  const [shift] = useState(() => stack.length % 6);
  const [z, setZ] = useState(() => nextZ++);
  useEffect(() => {
    stack.push(me);
    if (stack.length === 1) window.addEventListener('keydown', onKey, true);
    return () => {
      const i = stack.indexOf(me);
      if (i >= 0) stack.splice(i, 1);
      if (!stack.length) window.removeEventListener('keydown', onKey, true);
    };
  }, [me]);
  const raise = () => {
    const i = stack.indexOf(me);
    if (i >= 0 && i !== stack.length - 1) {
      stack.splice(i, 1);
      stack.push(me);
    }
    setZ(nextZ++);
  };
  // It holds the run's numbers and is open only on the Results step: one of its regions (m10-h, m11-h).
  return createPortal(
    <div
      ref={frame.ref}
      className={`rw cw glass${className ? ` ${className}` : ''}${frame.pos ? ' rw-moved' : ''}`}
      style={{ zIndex: 31 + z, ['--cw-shift' as string]: String(shift), ...(frame.pos ? { left: frame.pos.x, top: frame.pos.y } : {}) }}
      role="dialog"
      aria-label={title}
      data-chart-window={chart}
      data-results-region
      onPointerDownCapture={raise}
    >
      <div className="rw-head" onPointerDown={frame.onHeadDown} onPointerMove={frame.onHeadMove} onPointerUp={frame.onHeadUp} onPointerCancel={frame.onHeadUp}>
        <span className="rw-title" data-part="chart-window-title">
          {title}
        </span>
        {sub ? <span className="rw-sub">{sub}</span> : null}
        <button type="button" className="rw-close" data-action="close-chart-window" aria-label="Close" title="Close (Esc)" onClick={onClose}>
          ×
        </button>
      </div>
      <div className="cw-body">{children}</div>
    </div>,
    document.body,
  );
}
