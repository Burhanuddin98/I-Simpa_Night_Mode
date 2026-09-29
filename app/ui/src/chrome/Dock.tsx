// The bottom dock (design:175-275): Acoustics, Console and Runs.
// Hand-over stub from the M10 foundation (M9's dock); the scene package owns it from here.
// The Acoustics tab keeps M9's empty state: no solver-computed number before M12 (PLAN.md 2.4).
import { useState } from 'react';
import { consoleStore, useStore } from '../store';
import { ConsolePane } from './ConsolePane';
import { RunsPane } from './RunsPane';

const DOCK_TABS = [
  { key: 'acoustics', name: 'Acoustics' },
  { key: 'console', name: 'Console' },
  { key: 'runs', name: 'Runs' },
] as const;
type DockKey = (typeof DOCK_TABS)[number]['key'];

export function Dock() {
  const [tab, setTab] = useState<DockKey>('console');
  const fails = useStore(consoleStore).filter((l) => l.tag === 'FAIL').length;
  return (
    <section className="dock" aria-label="Analysis dock">
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
            {d.key === 'console' && fails > 0 && <span className="tab-badge">{fails} fail</span>}
          </button>
        ))}
      </div>
      <div className="dock-body" role="tabpanel" data-dock-panel={tab}>
        {tab === 'acoustics' && (
          <div className="dock-empty empty">Room acoustics appear here once the physics checks behind them pass.</div>
        )}
        {tab === 'console' && <ConsolePane />}
        {tab === 'runs' && <RunsPane />}
      </div>
    </section>
  );
}
