// B3 (decision 71): the live layer's controller. actions.ts hands it a run's LIVE v1 batches while SPPS
// runs on the GPU; it keeps them in a LiveSet (live.ts), rebuilds the view's tables at most every
// LIVE_REBUILD_MS, and runs the live clock from animation frames. When the run ends the layer clears;
// the Results step's particles are a separate layer and are never touched.
import { setLiveSink } from '../../actions';
import { stepStore, Store } from '../../store';
import { registerHook } from '../../testhooks';
import { liveState, setLiveTime, showLive } from './engine';
import { decodeLiveBatch, liveCaption, LiveRun, type LiveSummary } from './live';
import type { ParticleLook } from './rays';
import { resultsViewStore } from './resultsView';

/** The tables are rebuilt at most this often while batches arrive. */
export const LIVE_REBUILD_MS = 250;

export interface LiveUi {
  /** The run the layer shows (actions.ts's run id). */
  runId: number;
  summary: LiveSummary | null;
  caption: string;
  look: ParticleLook;
  /** Why the chosen look is not drawn, or null. */
  lookNote: string | null;
  /** Batches that could not be read (each logged once to the console). */
  errors: number;
  /** Table rebuilds so far, and the last one's main-thread time, ms (the cost of drawing live). */
  builds: number;
  buildMs: number;
}

/** Non-null while a run of SPPS on the GPU is live. */
export const liveStore = new Store<LiveUi | null>(null);

let run: LiveRun | null = null;
let raf = 0;
let timer: ReturnType<typeof setTimeout> | null = null;
let builtAt = 0;

const now = () => performance.now();
/** The layer is drawn on the Simulate step only (engine.ts `liveShown`). */
const shown = () => stepStore.get() === 'simulate';

function patch(p: Partial<LiveUi>): void {
  const cur = liveStore.get();
  if (cur) liveStore.set({ ...cur, ...p });
}

function rebuild(): void {
  timer = null;
  const ui = liveStore.get();
  if (!run || !ui || !run.wantsBuild(shown())) return;
  run.built();
  builtAt = now();
  const want = ui.look;
  const inForce = showLive(run.set.particles(), want, resultsViewStore.get().trails);
  patch({ lookNote: inForce === want ? null : `${want} is not available here: drawn as ${inForce}`, builds: ui.builds + 1, buildMs: now() - builtAt });
  tick();
}

function schedule(): void {
  if (timer || !shown()) return;
  const wait = Math.max(0, LIVE_REBUILD_MS - (now() - builtAt));
  timer = setTimeout(rebuild, wait);
}

/** The live clock from animation frames, while the layer is shown. */
function tick(): void {
  if (raf || !run || !shown()) return;
  raf = requestAnimationFrame(() => {
    raf = 0;
    const t = run?.set.clock(now());
    if (t === null || t === undefined || !shown()) return;
    setLiveTime(t);
    tick();
  });
}

/** A run of SPPS on the GPU has started: the layer waits for its first batch. */
export function liveBegin(runId: number): void {
  liveEnd();
  run = new LiveRun(runId);
  builtAt = 0;
  liveStore.set({ runId, summary: null, caption: liveCaption(null), look: resultsViewStore.get().look, lookNote: null, errors: 0, builds: 0, buildMs: 0 });
}

/** One LIVE v1 batch of run `runId`: kept on any step (the caption follows it); anything for another run, or after its end, is dropped. */
export function liveBatch(runId: number, buf: ArrayBuffer): void {
  const ui = liveStore.get();
  if (!run || !ui) return;
  let kept: boolean;
  try {
    kept = run.accept(runId, decodeLiveBatch(buf), now());
  } catch (e) {
    if (ui.errors === 0) console.warn(`live particles: ${e instanceof Error ? e.message : String(e)}`);
    patch({ errors: ui.errors + 1 });
    return;
  }
  if (!kept) return;
  const summary = run.set.summary();
  patch({ summary, caption: liveCaption(summary) });
  schedule();
}

/** The layer clears: the run ended (or failed, or never started), on whatever step is shown. */
export function liveEnd(): void {
  if (timer) clearTimeout(timer);
  timer = null;
  if (raf) cancelAnimationFrame(raf);
  raf = 0;
  if (run) showLive(null, 'dots', 0);
  run = null;
  liveStore.set(null);
}

/** How the live particles are drawn (the Results card's looks, chosen here for the live layer only). */
export function setLiveLook(look: ParticleLook): void {
  if (!liveStore.get() || !run) return;
  patch({ look });
  run.invalidate();
  rebuild();
}

// Back on the Simulate step during a run: what arrived meanwhile is drawn at once, at the clock's now.
stepStore.subscribe(() => {
  if (!run || !shown()) return;
  run.invalidate();
  rebuild();
  tick();
});

setLiveSink({
  begin: liveBegin,
  batch: liveBatch,
  end: (runId) => {
    if (run?.end(runId)) liveEnd();
  },
});

registerHook('liveView', () => ({ ui: liveStore.get(), view: liveState(), clock: run?.set.clock(now()) ?? null }));
