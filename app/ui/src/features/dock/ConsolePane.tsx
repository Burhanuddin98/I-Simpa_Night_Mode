// The Console (design:247-257, M11 PLAN.md 3.3). Every line carries its class as a text label,
// FAIL, WARN, INFO, OK or PROGRESS, never colour alone, with `data-run` and `data-source`.
//
// - Solver and TetGen text is shown exactly as it came, in a `data-verbatim="<run>:<source>"`
//   element that keeps its spaces (PLAN.md 3.4 rule 2).
// - A run's PROGRESS lines are counted, never rendered one by one (T15): the active run has one
//   live PROGRESS line, updated in place, at the bottom.
// - Each run streamed in this session gets a counts strip, `[data-run-counts=<run>]` with one
//   `[data-count=<CLASS>]` per class, computed from the lines received (`runLinesStore`), and,
//   once the run has ended, checked against its run.json.
// - A number with a unit sits only in a diagnostic span (`data-diagnostic`, PLAN.md 3.4 rule 1).
//
// It follows new lines while scrolled to the bottom, and stays put while the user reads higher up.
import { memo, useLayoutEffect, useMemo, useRef } from 'react';
import type { ConsoleLine, LinePart } from '../../store';
import { consoleStore, runLinesStore, runsStore, runStore, useStore } from '../../store';
import { CLASSES, type ConsoleItem, consoleItems, stageLabel } from './model';

/** Within this many pixels of the bottom counts as at the bottom. */
const PIN_PX = 24;

function Parts({ parts }: { parts: readonly LinePart[] }) {
  return (
    <>
      {parts.map((p, i) =>
        'diagnostic' in p ? (
          <span key={i} data-diagnostic={p.diagnostic} data-run={p.run}>
            {p.text}
          </span>
        ) : (
          <span key={i}>{p.text}</span>
        ),
      )}
    </>
  );
}

/** A logged line. Memoised on the line itself, which the store never changes once logged, so a
 * batch of a run re-renders only what it added (gate (b) measures frames during a solve). */
const Line = memo(function Line({ line: l }: { line: ConsoleLine }) {
  const source = l.source ?? 'app';
  // Verbatim only when it can be traced: a run's solver or TetGen line.
  const verbatim = l.verbatim && l.run !== undefined && source !== 'app' ? `${l.run}:${source}` : undefined;
  return (
    <div className={`console-line ${l.tag}`} data-run={l.run} data-source={source}>
      <span className="time">{l.time}</span>
      <span className="tag">{l.tag}</span>
      {verbatim ? (
        <span className="text verbatim" data-verbatim={verbatim}>
          {l.text}
        </span>
      ) : (
        <span className="text">{l.parts ? <Parts parts={l.parts} /> : l.text}</span>
      )}
    </div>
  );
});

function Live({ item }: { item: Extract<ConsoleItem, { kind: 'live' }> }) {
  const note = item.cancelling ? 'cancelling' : `${item.solver} progress, updated in place`;
  return (
    <div
      className="console-line PROGRESS live"
      data-run={item.run}
      data-source={item.verbatim ? 'solver' : 'app'}
      data-progress-line={item.run}
      aria-live="off"
    >
      <span className="time">now</span>
      <span className="tag">PROGRESS</span>
      <span className="text">
        {item.verbatim ? (
          <span className="verbatim" data-verbatim={`${item.run}:solver`}>
            {item.verbatim}
          </span>
        ) : (
          <span>{stageLabel(item.stage)}</span>
        )}
        <span className="note">{` · ${note}`}</span>
      </span>
    </div>
  );
}

function Counts({ item }: { item: Extract<ConsoleItem, { kind: 'counts' }> }) {
  const lost = item.gaps > 0 || item.dupes > 0;
  return (
    <div className="console-counts" data-run-counts={item.run}>
      <span className="time" />
      <span className="tag" />
      <span className="who">{item.label} lines</span>
      {/* A class word takes its colour only when its count is above 0: a red "FAIL 0" read as a
          failure (M11 review B.10; decision-log row 28). */}
      {CLASSES.map((c) => (
        <span key={c} className={`count ${c}${item.counts[c] === 0 ? ' zero' : ''}`}>
          <span className="k">{c}</span> <span data-count={c}>{item.counts[c]}</span>
        </span>
      ))}
      {item.check &&
        (item.check.match ? (
          <span className="counts-check ok" data-part="counts-check" data-match="true" title="Equal to the counts in run.json">
            = run.json
          </span>
        ) : (
          <span className="counts-check fail" data-part="counts-check" data-match="false">
            <span className="flag">FAIL</span>
            {` run.json counts ${CLASSES.map((c) => `${c} ${item.check?.manifest[c]}`).join(' · ')}`}
          </span>
        ))}
      {lost && (
        <span className="counts-check fail" data-part="stream-check">
          <span className="flag">FAIL</span>
          {` stream: ${item.gaps} gap${item.gaps === 1 ? '' : 's'}, ${item.dupes} repeated`}
        </span>
      )}
    </div>
  );
}

export function ConsolePane() {
  const lines = useStore(consoleStore);
  const logs = useStore(runLinesStore);
  const active = useStore(runStore);
  const view = useStore(runsStore);
  const items = useMemo(() => consoleItems(lines, logs, active, view?.rows ?? null), [lines, logs, active, view]);
  const root = useRef<HTMLDivElement>(null);
  const pinned = useRef(true);

  useLayoutEffect(() => {
    const scroller = root.current?.parentElement;
    if (!scroller) return;
    const onScroll = () => {
      pinned.current = scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight < PIN_PX;
    };
    scroller.addEventListener('scroll', onScroll, { passive: true });
    return () => scroller.removeEventListener('scroll', onScroll);
  }, []);

  // After every render: a new line, the live line's update, a count. Only while pinned.
  useLayoutEffect(() => {
    const scroller = root.current?.parentElement;
    if (scroller && pinned.current) scroller.scrollTop = scroller.scrollHeight;
  });

  return (
    <div className="console" role="log" aria-live="polite" ref={root}>
      {items.length === 0 && <div className="console-empty empty">Nothing logged yet.</div>}
      {items.map((item) =>
        item.kind === 'line' ? (
          <Line key={item.key} line={item.line} />
        ) : item.kind === 'live' ? (
          <Live key={item.key} item={item} />
        ) : (
          <Counts key={item.key} item={item} />
        ),
      )}
    </div>
  );
}
