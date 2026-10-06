// The bottom dock (design:175-275, M11 PLAN.md 3.3): Acoustics, Console and Runs. Owned by the
// dock package (PLAN.md 9.2); mounted by App.tsx with no props.
//
// - Badges, as text: the Console reads "live" while a run is active, else "<n> fail" (its FAIL
//   lines); the Runs tab shows its row count.
// - The Acoustics tab (M12 P2, features/acoustics): the selected run's report, only on the Results
//   step; on every other step it holds no number.
// - The run list is re-read when a run's folder appears, so its Running row shows at once; a
//   run's end re-reads it too (actions.ts).
import { useEffect, useRef, useState } from 'react';
import * as actions from '../../actions';
import { consoleStore, runsStore, runStore, stepStore, useStore } from '../../store';
import { registerHook } from '../../testhooks';
import { AcousticsPane, useAcousticsDock } from '../acoustics/AcousticsPane';
import { ConsolePane } from './ConsolePane';
import { consoleBadge, runsBadge } from './model';
import { RunsPane } from './RunsPane';
import { FoldButton, openDockFor, useFold } from '../../chrome/fold';
import './dock.css';

const DOCK_TABS = [
  { key: 'acoustics', name: 'Acoustics' },
  { key: 'console', name: 'Console' },
  { key: 'runs', name: 'Runs' },
] as const;
type DockKey = (typeof DOCK_TABS)[number]['key'];

export function Dock() {
  const [tab, setTab] = useState<DockKey>('console');
  const lines = useStore(consoleStore);
  const active = useStore(runStore);
  const view = useStore(runsStore);
  const body = useRef<HTMLDivElement>(null);
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
    return () => {
      offTab();
      offScroll();
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

  return (
    <section className="dock" aria-label="Analysis dock" data-folded={folded}>
      <FoldButton panel="dock" />
      <div className="dock-tabs" role="tablist" aria-label="Dock">
        {DOCK_TABS.map((d) => (
          <button
            key={d.key}
            className="dock-tab"
            role="tab"
            data-dock-tab={d.key}
            aria-selected={d.key === tab}
            onClick={() => setTab(d.key)}
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
        {tab === 'acoustics' && <AcousticsPane />}
        {tab === 'console' && <ConsolePane />}
        {tab === 'runs' && <RunsPane />}
      </div>
    </section>
  );
}
