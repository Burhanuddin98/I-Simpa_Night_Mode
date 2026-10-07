// The bottom dock (design:175-275, M11 PLAN.md 3.3): Acoustics, Console and Runs. Owned by the
// dock package (PLAN.md 9.2); mounted by App.tsx with no props.
//
// - Badges, as text: the Console reads "live" while a run is active, else "<n> fail" (its FAIL
//   lines); the Runs tab shows its row count.
// - The Acoustics tab (M12 P2, features/acoustics): the selected run's report, only on the Results
//   step; on every other step it holds no number.
// - The run list is re-read when a run's folder appears, so its Running row shows at once; a
//   run's end re-reads it too (actions.ts).
// - Its height (C2, decision-log row 74; the rules are model.ts's): the top edge is a handle,
//   dragged or moved with the keys, from the tab strip alone (the fold) to the whole work area,
//   where the 3D view is hidden. The 3D view's region follows live as the edge moves (its
//   ResizeObserver, engine.ts). The maximise button and a double-click on the tab strip toggle
//   full height; Escape restores. The height is kept across steps and launches, maximised for
//   the session only.
import { useEffect, useMemo, useRef, useState } from 'react';
import * as actions from '../../actions';
import { consoleStore, runsStore, runStore, stepStore, Store, useStore } from '../../store';
import { registerHook } from '../../testhooks';
import { AcousticsPane, useAcousticsDock } from '../acoustics/AcousticsPane';
import { ConsolePane } from './ConsolePane';
import {
  consoleBadge,
  DOCK_MIN,
  type DockPlace,
  type DockSize,
  dockHeight,
  dockHeightText,
  dockTall,
  dragTo,
  escapeMax,
  keyTo,
  parseDockHeight,
  runsBadge,
  toggleMax,
  VIEW_MIN,
} from './model';
import { RunsPane } from './RunsPane';
import { FoldButton, foldStore, openDockFor, setFold, useFold } from '../../chrome/fold';
import './dock.css';

const DOCK_TABS = [
  { key: 'acoustics', name: 'Acoustics' },
  { key: 'console', name: 'Console' },
  { key: 'runs', name: 'Runs' },
] as const;
type DockKey = (typeof DOCK_TABS)[number]['key'];

// ---- the height ----------------------------------------------------------------------------------

const HEIGHT_KEY = 'nm-dock-height';

function loadHeight(): number {
  try {
    return parseDockHeight(localStorage.getItem(HEIGHT_KEY));
  } catch {
    return parseDockHeight(null);
  }
}

/** The dock's size: its height (kept in this browser profile, as the fold is) and maximised (this session). */
const sizeStore = new Store<DockSize>({ height: loadHeight(), max: false });

function placeNow(): DockPlace {
  return { size: sizeStore.get(), folded: foldStore.get().dock };
}

/** Applies a place; the height is stored when `store` (at the end of a drag, on a key). */
function place(p: DockPlace, store = true): void {
  const was = sizeStore.get();
  if (p.size.height !== was.height || p.size.max !== was.max) sizeStore.set(p.size);
  if (p.folded !== foldStore.get().dock) setFold('dock', p.folded);
  if (!store) return;
  try {
    localStorage.setItem(HEIGHT_KEY, dockHeightText(p.size.height));
  } catch {
    // Storage refused: the height still holds for this session.
  }
}

/** The height the dock takes maximised: its column, less the gap above it. */
function roomOf(dock: HTMLElement | null): number {
  const col = dock?.parentElement;
  if (!col) return 0;
  const gap = parseFloat(getComputedStyle(col).getPropertyValue('--gap')) || 0;
  return col.clientHeight - gap;
}

/** Something else that Escape closes is open: it takes the key, not the dock. */
function escapeTaken(e: KeyboardEvent): boolean {
  if (e.defaultPrevented) return true;
  const t = e.target;
  if (t instanceof HTMLElement && (t.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(t.tagName))) return true;
  if (document.documentElement.classList.contains('present')) return true;
  return document.querySelector('.rw, [aria-modal="true"], [role="dialog"], [role="menu"], [role="listbox"], .dropdown') !== null;
}

const MaxIcon = ({ max }: { max: boolean }) =>
  max ? (
    <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden>
      <rect x="1.5" y="3.5" width="7" height="7" stroke="currentColor" strokeWidth="1.2" />
      <path d="M3.5 3.5V1.5h7v7h-2" stroke="currentColor" strokeWidth="1.2" />
    </svg>
  ) : (
    <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden>
      <rect x="1.5" y="1.5" width="9" height="9" stroke="currentColor" strokeWidth="1.2" />
    </svg>
  );

export function Dock() {
  const [tab, setTab] = useState<DockKey>('console');
  const lines = useStore(consoleStore);
  const active = useStore(runStore);
  const view = useStore(runsStore);
  const body = useRef<HTMLDivElement>(null);
  const dockRef = useRef<HTMLElement>(null);
  const size = useStore(sizeStore);
  const [dragging, setDragging] = useState(false);
  // The column's height, for the handle's value only; the drag measures its own at its start.
  const [room, setRoom] = useState(0);
  const consoleTag = consoleBadge(lines, active !== null);
  const runs = runsBadge(view);
  useAcousticsDock();

  const activeRun = active?.run;
  useEffect(() => {
    if (activeRun) actions.fire(actions.refreshRuns());
  }, [activeRun]);

  // Test hooks (PLAN.md 3.5): show a tab without a click, and read the Console's scroll state.
  useEffect(() => {
    const offTab = registerHook('dockTab', (key: string) => {
      if (!DOCK_TABS.some((d) => d.key === key)) throw new Error(`dockTab: no tab '${key}'`);
      setTab(key as DockKey);
      return key;
    });
    const offScroll = registerHook('consoleScroll', () => {
      const el = body.current;
      if (!el || el.getAttribute('data-dock-panel') !== 'console') return null;
      return {
        top: el.scrollTop,
        height: el.scrollHeight,
        client: el.clientHeight,
        atBottom: el.scrollHeight - el.scrollTop - el.clientHeight < 24,
      };
    });
    // C2: set the dock's height ('fold', 'max', 'restore', or a height in pixels, as a drag to it
    // would) and read it back with the 3D view's region beside it.
    const offSize = registerHook('dockSize', (to?: string | number) => {
      const p = placeNow();
      const room = roomOf(dockRef.current);
      let next: DockPlace | null = null;
      if (to === 'fold') next = keyTo(p, 'Home', false, room);
      else if (to === 'max') next = p.size.max && !p.folded ? null : toggleMax({ ...p, size: { ...p.size, max: false } });
      else if (to === 'restore') next = escapeMax(p) ?? (p.folded ? { size: p.size, folded: false } : null);
      else if (typeof to === 'number') next = dragTo(p.size, to, room);
      else if (to !== undefined) throw new Error(`dockSize: '${to}' is not fold, max, restore or a height`);
      if (next) place(next);
      const box = dockRef.current?.getBoundingClientRect();
      const vp = document.querySelector('.center > .viewport')?.getBoundingClientRect();
      const q = placeNow();
      return { height: box?.height ?? 0, stored: q.size.height, max: q.size.max, folded: q.folded, room, view: vp?.height ?? 0 };
    });
    return () => {
      offTab();
      offScroll();
      offSize();
    };
  }, []);
  const folded = useFold('dock');
  // A folded dock opens itself on the Results step (the Acoustics tab) and when an error is logged.
  useEffect(() => {
    const offStep = stepStore.subscribe(() => openDockFor({ step: stepStore.get() }));
    const offLog = consoleStore.subscribe(() => openDockFor({ tag: consoleStore.get().at(-1)?.tag }));
    openDockFor({ step: stepStore.get() });
    return () => {
      offStep();
      offLog();
    };
  }, []);

  useEffect(() => {
    const col = dockRef.current?.parentElement;
    if (!col) return;
    const ro = new ResizeObserver(() => setRoom(roomOf(dockRef.current)));
    ro.observe(col);
    return () => ro.disconnect();
  }, []);

  // Escape restores a maximised dock, unless something open takes the key.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== 'Escape' || e.ctrlKey || e.altKey || e.metaKey || escapeTaken(e)) return;
      const next = escapeMax(placeNow());
      if (!next) return;
      e.preventDefault();
      place(next);
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  /** The top edge, dragged: the height follows the pointer each frame, and is stored on release. */
  const onGripDown = (e: React.PointerEvent<HTMLDivElement>) => {
    if (e.button !== 0 || !dockRef.current) return;
    e.preventDefault();
    const grip = e.currentTarget;
    grip.setPointerCapture(e.pointerId);
    grip.focus({ preventScroll: true });
    // The edge moves by what the pointer moves, from the height drawn at the start (no jump where it was grabbed).
    const from = dockRef.current.getBoundingClientRect().height;
    const room = roomOf(dockRef.current);
    const start = sizeStore.get();
    let y = e.clientY;
    let frame = 0;
    let moved = false;
    const apply = (store: boolean) => {
      frame = 0;
      place(dragTo(start, from + e.clientY - y, room), store);
    };
    const move = (ev: PointerEvent) => {
      if (!moved && Math.abs(ev.clientY - e.clientY) < 2) return;
      moved = true;
      y = ev.clientY;
      if (!frame) frame = requestAnimationFrame(() => apply(false));
    };
    const end = () => {
      if (frame) cancelAnimationFrame(frame);
      if (moved) apply(true);
      setDragging(false);
      grip.removeEventListener('pointermove', move);
      grip.removeEventListener('pointerup', end);
      grip.removeEventListener('pointercancel', end);
    };
    setDragging(true);
    grip.addEventListener('pointermove', move);
    grip.addEventListener('pointerup', end);
    grip.addEventListener('pointercancel', end);
  };

  const onGripKey = (e: React.KeyboardEvent<HTMLDivElement>) => {
    if (e.ctrlKey || e.altKey || e.metaKey) return;
    const next = keyTo(placeNow(), e.key, e.shiftKey, roomOf(dockRef.current));
    if (!next) return;
    e.preventDefault();
    e.stopPropagation();
    place(next);
  };

  const p: DockPlace = { size, folded };
  const tall = dockTall(p);
  // Not maximised: the stored height, never past the 3D view's minimum in this window (CSS does
  // the clamp, so a resized window needs no measuring). Maximised: dock.css's, the work area.
  const height = folded || size.max ? undefined : `clamp(${DOCK_MIN}px, ${size.height}px, calc(100% - ${VIEW_MIN}px - var(--gap)))`;
  const maxLabel = size.max && !folded ? 'Restore the dock (Esc)' : 'Fill the window with the dock';
  // The panes are made once per tab, so a drag's re-renders stop at the dock's own box.
  const pane = useMemo(() => (tab === 'acoustics' ? <AcousticsPane /> : tab === 'console' ? <ConsolePane /> : <RunsPane />), [tab]);

  return (
    <section
      className="dock"
      aria-label="Analysis dock"
      data-folded={folded}
      data-max={size.max && !folded}
      data-tall={tall}
      data-dragging={dragging || undefined}
      ref={dockRef}
      style={height === undefined ? undefined : { height }}
    >
      <div
        className="dock-grip"
        role="separator"
        aria-orientation="horizontal"
        aria-label="Dock height: drag, or use the arrow keys; Home folds, End fills the window"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={folded ? 0 : size.max ? 100 : Math.round((100 * dockHeight(size, room)) / Math.max(1, room))}
        title="Drag to resize the dock; double-click to fill the window"
        tabIndex={0}
        onPointerDown={onGripDown}
        onKeyDown={onGripKey}
        onDoubleClick={() => place(toggleMax(placeNow()))}
      />
      <button
        type="button"
        className="fold dock-max"
        data-dock-max={size.max && !folded}
        aria-label={maxLabel}
        title={maxLabel}
        aria-pressed={size.max && !folded}
        onClick={() => place(toggleMax(placeNow()))}
      >
        <MaxIcon max={size.max && !folded} />
      </button>
      <FoldButton panel="dock" />
      <div
        className="dock-tabs"
        role="tablist"
        aria-label="Dock"
        onDoubleClick={(e) => {
          if ((e.target as HTMLElement).closest('.tab-badge')) return;
          place(toggleMax(placeNow()));
        }}
      >
        {DOCK_TABS.map((d) => (
          <button
            key={d.key}
            className="dock-tab"
            role="tab"
            data-dock-tab={d.key}
            aria-selected={d.key === tab}
            onClick={() => {
              setTab(d.key);
              // A tab clicked on the folded strip is a tab asked for: the dock opens to show it.
              if (foldStore.get().dock) setFold('dock', false);
            }}
          >
            {d.name}
            {d.key === 'console' && consoleTag && (
              <span
                className={`tab-badge ${consoleTag.kind}`}
                data-part={consoleTag.kind === 'live' ? 'live' : 'fail-count'}
              >
                {consoleTag.text}
              </span>
            )}
            {d.key === 'runs' && runs !== null && (
              <span className="tab-badge" data-part="runs-count">
                {runs}
              </span>
            )}
          </button>
        ))}
      </div>
      <div className="dock-body" role="tabpanel" data-dock-panel={tab} ref={body}>
        {pane}
      </div>
    </section>
  );
}
