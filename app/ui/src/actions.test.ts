// node --test suite for actions.ts's run lifecycle, the real module under a stand-in backend
// (testing/hooks.mjs). The stand-in follows the app's run slot (src-tauri/src/runs.rs): run_start
// registers the run only once its checks (`plan()`) are done and answers after that; run_cancel
// cancels the registered run and answers true, or answers false with the slot empty; a second
// run_start while a run is registered is refused with RUN_ACTIVE. Timings are the suite's own.
import { strict as assert } from 'node:assert';
import { register } from 'node:module';
import { beforeEach, test } from 'node:test';
import type { RunRow, RunStreamBatch, RunStreamEvent, SceneState } from './bindings/ipc.ts';

register('./testing/hooks.mjs', import.meta.url);
const actions = await import('./actions.ts');
const store = await import('./store.ts');

const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

async function until(what: string, pred: () => boolean, ms = 2_000): Promise<void> {
  const t0 = Date.now();
  while (!pred()) {
    if (Date.now() - t0 > ms) assert.fail(`not within ${ms} ms: ${what}`);
    await sleep(1);
  }
}

interface Channelish {
  onmessage: (b: RunStreamBatch) => void;
}

/** One run the stand-in backend holds in its slot. */
interface SlotRun {
  name: string;
  cancelled: boolean;
  channel: Channelish;
}

function scene(name: string, path: string | null, dirty: boolean): SceneState {
  return {
    groups: [],
    info: { name, path, dirty, active_variant: null, geometry_rev: 1, undo_depth: dirty ? 1 : 0 },
    issues: [],
    lines: [],
    run_blockers: [],
  } as unknown as SceneState;
}

function row(run: string, status: RunRow['status']): RunRow {
  return {
    run,
    number: 1,
    status,
    stage: 'solve',
    solver: 'spps',
    variant: null,
    reasons: [],
    warnings: [],
    exports: [],
  } as unknown as RunRow;
}

/** The stand-in backend. */
class Backend {
  calls: string[] = [];
  slot: SlotRun | null = null;
  runs = 0;
  saveMs = 0;
  solversMs = 0;
  /** While set, run_start waits on it before registering the run: its `plan()` in flight. */
  plan: Promise<void> | null = null;
  scene = scene('room', 'C:/p/room.simpa', false);
  rows: RunRow[] = [];
  exampleIds: string[] = [];

  count(cmd: string): number {
    return this.calls.filter((c) => c === cmd).length;
  }

  async ipc(cmd: string, args: Record<string, unknown>): Promise<unknown> {
    this.calls.push(cmd);
    switch (cmd) {
      case 'project_save':
        await sleep(this.saveMs);
        this.scene = scene(this.scene.info.name, (args.path as string | null) ?? this.scene.info.path ?? null, false);
        return this.scene;
      case 'solvers_status':
        await sleep(this.solversMs);
        return { checks: [], blockers: [] };
      case 'runs_list':
        return { root: 'C:/p/runs', rows: this.rows, other_projects: 0, active: this.slot?.name ?? null };
      case 'run_start': {
        if (this.plan) await this.plan;
        if (this.slot) throw { code: 'RUN_ACTIVE', message: 'a run is active: cancel it first' };
        const name = `20260930-0100${String(++this.runs).padStart(2, '0')}-000-spps`;
        this.slot = { name, cancelled: false, channel: args.on_event as Channelish };
        return { solver: 'spps', variant: null, project_path: this.scene.info.path, runs_root: 'C:/p/runs' };
      }
      case 'run_cancel':
        if (!this.slot) return false;
        this.slot.cancelled = true;
        return true;
      case 'scene_new':
        this.scene = scene(String(args.name), null, true);
        return this.scene;
      case 'proj_import':
        this.scene = scene('tutorial_1', null, true);
        return this.scene;
      case 'example_open':
        this.exampleIds.push(String(args.id));
        this.scene = scene(String(args.id), `C:/Users/u/Documents/Night Mode/Examples/${String(args.id)}.simpa`, false);
        return this.scene;
      default:
        throw { code: 'NO_STANDIN', message: `the stand-in backend has no ${cmd}` };
    }
  }

  /** Streams the registered run as run_thread does: started, a stage, `progress` PROGRESS lines,
   * then ended (CANCELLED once cancelled), the slot freed before the last event. */
  async stream(progress: number): Promise<string> {
    const r = this.slot;
    assert.ok(r, 'a run is registered');
    let seq = 0;
    let batch = 0;
    const send = (events: RunStreamEvent[], last = false) => r.channel.onmessage({ batch: batch++, events, last });
    send([
      { kind: 'started', seq: seq++, t_ms: 0, run: r.name, folder: `C:/p/runs/${r.name}` },
      { kind: 'stage', seq: seq++, t_ms: 1, stage: 'solve' },
    ] as RunStreamEvent[]);
    for (let i = 1; i <= progress && !r.cancelled; i++) {
      await sleep(1);
      send([
        {
          kind: 'line', seq: seq++, t_ms: 1 + i, source: 'solver', stream: 'stdout', class: 'PROGRESS', rule: 'spps_progress',
          solver_seq: i, progress: i, continuation: false, text: `#${i}`,
        },
      ] as RunStreamEvent[]);
    }
    const status = r.cancelled ? 'CANCELLED' : 'OK';
    this.slot = null;
    this.rows = [...this.rows, row(r.name, status)];
    send([{ kind: 'ended', seq: seq++, t_ms: 999, row: row(r.name, status) }] as RunStreamEvent[], true);
    return status;
  }
}

let backend = new Backend();

beforeEach(() => {
  backend = new Backend();
  (globalThis as Record<string, unknown>).__TAURI_TEST_IPC__ = (cmd: string, args: Record<string, unknown>) => backend.ipc(cmd, args);
  store.sceneStore.set(backend.scene);
  // accept() fetches the mesh only when the geometry revision moved: it never does here.
  store.meshStore.set({ geometryRev: 1 } as unknown as NonNullable<ReturnType<typeof store.meshStore.get>>);
  store.runStore.set(null);
  store.runLinesStore.set(new Map());
  store.consoleStore.set([]);
  store.selectedRunStore.set(null);
  store.resultsStore.set(new Map());
});

const infoLines = () => store.consoleStore.get().filter((l) => l.tag === 'INFO').map((l) => l.text);
const failLines = () => store.consoleStore.get().filter((l) => l.tag === 'FAIL').map((l) => l.text);

test('a second Run while the first is saving never reaches run_start, and the first run keeps its state (review 2, M3 and E1)', async () => {
  backend.scene = scene('room', 'C:/p/room.simpa', true);
  store.sceneStore.set(backend.scene);
  backend.saveMs = 40;
  const first = actions.runStart('spps');
  await sleep(5); // inside the save: a double click, or F5 after the click
  const second = await actions.runStart('spps').catch((e: unknown) => e);
  assert.equal(second, null, 'the second Run is refused in the page');
  assert.ok(infoLines().includes('Run: a run is starting'), JSON.stringify(infoLines()));
  await first;
  assert.equal(backend.count('run_start'), 1, 'run_start was called once');
  assert.deepEqual(failLines(), [], 'no RUN_ACTIVE failure');
  const live = store.runStore.get();
  assert.equal(live?.status, 'starting');
  const status = await backend.stream(5);
  assert.equal(status, 'OK');
  const name = backend.rows[0].run;
  const log = store.runLinesStore.get().get(name);
  assert.equal(log?.counts.PROGRESS, 5, 'every line of the run was folded under its own name');
  await until('the run ended in the page', () => store.runStore.get() === null);
});

test('a second Run while the solvers are checked on a clean project is refused the same way', async () => {
  backend.solversMs = 30;
  const first = actions.runStart('spps');
  await sleep(5);
  assert.equal(await actions.runStart('spps').catch((e: unknown) => e), null);
  await first;
  assert.equal(backend.count('run_start'), 1);
  assert.equal(store.runStore.get()?.status, 'starting');
  await backend.stream(2);
  await until('the run ended in the page', () => store.runStore.get() === null);
});

test('a start the backend refuses clears its own state only, and the refusal is a FAIL line', async () => {
  backend.slot = { name: 'held', cancelled: false, channel: { onmessage: () => {} } };
  const refused = await actions.runStart('spps').catch((e: unknown) => e);
  assert.equal((refused as { code?: string }).code, 'RUN_ACTIVE');
  assert.equal(store.runStore.get(), null);
  assert.deepEqual(failLines(), ['Could not start the run: a run is active: cancel it first (RUN_ACTIVE)']);
  // Nothing is left starting: the next Run goes through.
  backend.slot = null;
  await actions.runStart('spps');
  assert.equal(backend.count('run_start'), 2);
  await backend.stream(1);
  await until('the run ended in the page', () => store.runStore.get() === null);
});

test("a run's stream is filed under the run its own channel named, whatever runStore holds (review 2, E1)", async () => {
  await actions.runStart('spps');
  const r = backend.slot;
  assert.ok(r);
  let seq = 0;
  const line = (i: number) =>
    ({
      kind: 'line', seq: seq++, t_ms: i, source: 'solver', stream: 'stdout', class: i % 2 ? 'WARN' : 'PROGRESS', rule: 'x',
      solver_seq: i, progress: i, continuation: false, text: `#${i}`,
    }) as RunStreamEvent;
  r.channel.onmessage({ batch: 0, events: [{ kind: 'started', seq: seq++, t_ms: 0, run: r.name, folder: r.name } as RunStreamEvent, line(1)], last: false });
  assert.equal(store.runStore.get()?.run, r.name);
  // Something else takes runStore (what a refused second start's catch used to do, with null).
  const other = { ...store.runStore.get()!, id: 999, run: 'another-run' };
  store.runStore.set(other);
  r.channel.onmessage({ batch: 1, events: [line(2), line(3)], last: false });
  const log = store.runLinesStore.get().get(r.name);
  assert.equal(log?.seqs.size, 3, "three solver lines, all under the channel's own run");
  assert.deepEqual([log?.lastEventSeq, log?.gaps], [3, 0], 'every event of the stream, none skipped');
  assert.deepEqual([log?.counts.WARN, log?.counts.PROGRESS], [2, 1], 'the lines after runStore moved are counted');
  assert.equal(store.runStore.get(), other, 'runStore, which no longer holds this run, is left alone');
  assert.equal(store.runLinesStore.get().get('another-run'), undefined);
  store.runStore.set(null);
});

test('a Cancel pressed while the run is starting reaches the run once it is registered (review 2, M2)', async () => {
  let release!: () => void;
  backend.plan = new Promise<void>((r) => (release = r));
  const started = actions.runStart('spps');
  await until('the run is starting', () => store.runStore.get()?.status === 'starting' && backend.count('run_start') === 1);
  // The Cancel button is on screen from here (SimulatePanel renders RunningBlock on runStore).
  assert.equal(await actions.runCancel(), false, 'the backend has no run registered yet');
  assert.equal(store.runStore.get()?.status, 'cancelling');
  release(); // plan() done: run_start registers a fresh run and answers
  backend.plan = null;
  await started;
  await until('the Cancel reached the registered run', () => backend.slot?.cancelled === true);
  assert.equal(await backend.stream(50), 'CANCELLED', 'the run ended cancelled, not after 50 progress lines');
  await until('the run ended in the page', () => store.runStore.get() === null);
});

test("New, a .proj import and a Save as into another folder forget the previous project's run and verdicts (review 2, app 3)", async () => {
  const verified = { run: 'R-old', verified: true, refusal: null };
  const seed = () => {
    store.selectedRunStore.set('R-old');
    store.resultsStore.set(new Map([['R-old', verified]]));
  };
  const forgotten = (what: string) => {
    assert.equal(store.selectedRunStore.get(), null, `${what}: the Results step still shows the previous run`);
    assert.equal(store.resultsStore.get().size, 0, `${what}: a previous verdict is still cached`);
  };
  seed();
  assert.ok(await actions.newProject('Untitled'));
  forgotten('File > New');
  seed();
  await actions.importProj('C:/u/tutorial_1.proj');
  forgotten('File > Open of a .proj');
  // Saved, then Save as into another folder: another runs root.
  backend.scene = scene('room', 'C:/p/room.simpa', false);
  store.sceneStore.set(backend.scene);
  seed();
  await actions.saveAs('C:/q/room.simpa');
  forgotten('Save as into another folder');
  // The control: Save as beside the file keeps the runs root, and the selection with it.
  seed();
  await actions.saveAs('C:/Q/room-2.simpa');
  assert.equal(store.selectedRunStore.get(), 'R-old', 'Save as in the same folder keeps the run');
  assert.equal(store.resultsStore.get().get('R-old'), verified);
  await sleep(5); // the refreshRuns() each action fires
});

test('after a reload mid-run the page takes the run back, New and Run wait, and the run is let go when it ends (review 2, app 2)', async () => {
  // A fresh page (runStore null) and a backend still running a run.
  backend.slot = { name: 'R-live', cancelled: false, channel: { onmessage: () => {} } };
  backend.rows = [row('R-live', 'RUNNING')];
  await actions.refreshRuns();
  const r = store.runStore.get();
  assert.deepEqual([r?.run, r?.status], ['R-live', 'running'], 'the page took the run back');
  assert.ok(
    store.consoleStore.get().some((l) => l.tag === 'WARN' && l.text.includes('page was reloaded')),
    'a line tells the user',
  );
  assert.equal(await actions.newProject('Untitled'), null, 'New waits');
  assert.equal(backend.count('scene_new'), 0);
  assert.equal(await actions.runStart('spps'), null, 'Run waits');
  assert.equal(backend.count('run_start'), 0);
  assert.equal(await actions.runCancel(), true, "Cancel reaches the backend's run");
  assert.equal(backend.slot?.cancelled, true);
  // The run ends: the page, which receives no stream for it, sees it through runs_list.
  backend.slot = null;
  backend.rows = [row('R-live', 'CANCELLED')];
  await until('the page let the ended run go', () => store.runStore.get() === null, 3_000);
  assert.equal(store.selectedRunStore.get(), 'R-live');
});

test('a runs_list answer from before the end of a run this page streamed does not take the run back', async () => {
  await actions.runStart('spps');
  const name = backend.slot?.name;
  assert.ok(name);
  // The Runs tab asks while the run is live; its answer names the run as active.
  const stale = { root: 'C:/p/runs', rows: [row(name, 'RUNNING')], other_projects: 0, active: name };
  await backend.stream(2);
  await until('the run ended in the page', () => store.runStore.get() === null);
  const ipc = (globalThis as Record<string, unknown>).__TAURI_TEST_IPC__ as (cmd: string, args: Record<string, unknown>) => Promise<unknown>;
  (globalThis as Record<string, unknown>).__TAURI_TEST_IPC__ = (cmd: string, args: Record<string, unknown>) =>
    cmd === 'runs_list' ? Promise.resolve(stale) : ipc(cmd, args);
  try {
    await actions.refreshRuns();
    assert.equal(store.runStore.get(), null, 'the ended run was taken back as live');
    assert.ok(!store.consoleStore.get().some((l) => l.text.includes('page was reloaded')), 'no reload line');
  } finally {
    // A run wrongly taken back is watched until runs_list stops naming it, which this stale
    // answer never does: let it go, so a failure cannot hold the suite open.
    store.runStore.set(null);
    (globalThis as Record<string, unknown>).__TAURI_TEST_IPC__ = ipc;
  }
});

test('an example opens through example_open after the save prompt, and forgets the previous run; a run or a cancelled prompt stops it', async () => {
  store.selectedRunStore.set('R-old');
  const opened = await actions.openExample('bras-cr4');
  assert.deepEqual(backend.exampleIds, ['bras-cr4']);
  assert.equal(opened?.info.path, 'C:/Users/u/Documents/Night Mode/Examples/bras-cr4.simpa');
  assert.equal(store.sceneStore.get(), opened, 'the copy is the open project');
  assert.equal(store.selectedRunStore.get(), null, "the previous project's run is forgotten");

  // A run is active: nothing is copied or opened.
  store.runStore.set({ id: 1, run: 'R-live', status: 'running' } as unknown as NonNullable<ReturnType<typeof store.runStore.get>>);
  assert.equal(await actions.openExample('elmia'), null);
  store.runStore.set(null);

  // A changed project asks first; Cancel leaves it open and copies nothing.
  backend.scene = scene('room', 'C:/p/room.simpa', true);
  store.sceneStore.set(backend.scene);
  const pending = actions.openExample('elmia');
  await until('the save prompt is up', () => store.promptStore.get() !== null);
  store.promptStore.get()!.resolve('cancel');
  assert.equal(await pending, null);
  assert.deepEqual(backend.exampleIds, ['bras-cr4'], 'example_open was not called again');
  assert.equal(store.sceneStore.get()?.info.name, 'room');
  await sleep(5); // the refreshRuns() the first open fired
});
