// m11-focus's judge (docs/investigations/2026-09-29-m11/PLAN.md 4.2, item 3): pure, over the JSON
// lines tools/gates/focus-watch.ps1 writes. Burhan, 2026-09-29 11:26: the test windows stay
// visible; they must never take keyboard focus from what he is using.
//
// It fails when any of these holds:
//   - the watcher was not live: no start or stop record, a hook that did not install, an error;
//   - a foreground change to a window of a process descended from the gate (app.exe, WebView2,
//     msedgedriver, tauri-driver, node, a console host, anything the gate started) with no real
//     input in the `inputWindowMs` (1,000 ms) before it. A real button-down inside that window's
//     rectangle, or a real key, excuses it: that is a person clicking or Alt+Tabbing to it. Input
//     WebDriver sends never passes through the low-level hooks, and injected input does not count;
//   - a gate app.exe window, once first seen visible, sampled minimised or on no monitor, or
//     invisible while it still exists (a window seen invisible and gone within `closeGraceMs` was
//     being destroyed: DestroyWindow hides a window first);
//   - an app.exe session (a process with a titled window) in which no window was ever seen
//     visible, or fewer sessions than the gate opened.
// What it cannot see (PLAN.md 4.2): a steal within 1 s of the person's own key press anywhere,
// and a hide or minimise shorter than one sample.

export interface WindowSample {
  hwnd: string;
  pid: number;
  title: string;
  cls: string;
  visible: boolean;
  iconic: boolean;
  monitor: boolean;
}

export type FocusRecord =
  | { kind: 'start'; t: number; wall: string; root: number; sample_ms: number; hooks: { foreground: boolean; mouse: boolean; keyboard: boolean } }
  | { kind: 'fg'; t: number; hwnd: string; pid: number; name: string; title: string; cls: string; rect: [number, number, number, number]; gate: boolean; chain: string[] }
  | { kind: 'mouse'; t: number; injected: boolean; x: number; y: number }
  | { kind: 'key'; t: number; injected: boolean }
  | { kind: 'sample'; t: number; apps: number[]; windows: WindowSample[] }
  | { kind: 'stop'; t: number; reason: string; fg: number; mouse: number; key: number; samples: number }
  | { kind: 'error'; t: number; message: string };

export interface JudgeOptions {
  /** How long before a foreground change a person's input excuses it. */
  inputWindowMs?: number;
  /** How soon an invisible window must be gone to count as closing. */
  closeGraceMs?: number;
  /** The fewest app.exe sessions the watcher must have seen (the gate opened at least this many). */
  minSessions?: number;
}

export interface FocusVerdict {
  ok: boolean;
  failures: string[];
  /** Foreground changes a person's input explains, each printed. */
  excused: string[];
  notes: string[];
  sessions: number;
}

/** Top-level windows every GUI thread has, never shown: not the app's windows. */
const IME_CLASSES = new Set(['IME', 'MSCTFIME UI']);

/** Parses the watcher's JSON lines; a line that does not parse is an error record. */
export function parseLog(text: string): FocusRecord[] {
  const out: FocusRecord[] = [];
  for (const [i, line] of text.split(/\r?\n/).entries()) {
    if (!line.trim()) continue;
    try {
      out.push(JSON.parse(line) as FocusRecord);
    } catch (e) {
      out.push({ kind: 'error', t: -1, message: `line ${i + 1} does not parse: ${String(e)}` });
    }
  }
  return out;
}

const inside = (x: number, y: number, r: [number, number, number, number]) => x >= r[0] && x < r[2] && y >= r[1] && y < r[3];

function windowName(w: { pid: number; title: string; hwnd: string }): string {
  return `'${w.title}' (pid ${w.pid}, hwnd ${w.hwnd})`;
}

/** Judges a watcher log. */
export function judge(records: FocusRecord[], opts: JudgeOptions = {}): FocusVerdict {
  const inputMs = opts.inputWindowMs ?? 1_000;
  const graceMs = opts.closeGraceMs ?? 1_000;
  const failures: string[] = [];
  const excused: string[] = [];
  const notes: string[] = [];

  // The watcher was live.
  const start = records.find((r) => r.kind === 'start');
  const stop = records.find((r) => r.kind === 'stop');
  if (!start) failures.push('the watcher wrote no start record: it never ran');
  else {
    const h = start.hooks;
    if (!h.foreground || !h.mouse || !h.keyboard) {
      failures.push(`the watcher's hooks did not all install (foreground ${h.foreground}, mouse ${h.mouse}, keyboard ${h.keyboard})`);
    }
    notes.push(`watcher started ${start.wall}, gate root pid ${start.root}, a sample every ${start.sample_ms} ms`);
  }
  if (!stop) failures.push('the watcher wrote no stop record: it died or was killed, so the log may be cut short');
  else notes.push(`watcher stopped (${stop.reason}) after ${stop.t} ms: ${stop.fg} foreground change(s), ${stop.mouse} button-down(s), ${stop.key} key-down(s), ${stop.samples} sample(s)`);
  for (const r of records) if (r.kind === 'error') failures.push(`watcher error: ${r.message}`);

  // Foreground changes into the gate's tree.
  const mice = records.filter((r): r is Extract<FocusRecord, { kind: 'mouse' }> => r.kind === 'mouse' && !r.injected);
  const keys = records.filter((r): r is Extract<FocusRecord, { kind: 'key' }> => r.kind === 'key' && !r.injected);
  for (const r of records) {
    if (r.kind !== 'fg' || !r.gate) continue;
    const what = `${r.name} ${windowName(r)} at ${r.t} ms`;
    const click = mice.find((m) => m.t <= r.t && r.t - m.t <= inputMs && inside(m.x, m.y, r.rect));
    const key = keys.find((k) => k.t <= r.t && r.t - k.t <= inputMs);
    if (click) excused.push(`${what}: a real click inside it ${r.t - click.t} ms before`);
    else if (key) excused.push(`${what}: a real key ${r.t - key.t} ms before`);
    else failures.push(`FOCUS TAKEN: ${what} became the foreground window with no real input in the ${inputMs} ms before it (chain ${r.chain.join(' < ')})`);
  }

  // The app's windows stay visible, restored and on screen.
  const samples = records.filter((r): r is Extract<FocusRecord, { kind: 'sample' }> => r.kind === 'sample');
  const seenVisible = new Set<string>();
  const sessions = new Map<number, boolean>();
  const reported = new Set<string>();
  for (const [i, s] of samples.entries()) {
    for (const w of s.windows) {
      if (!w.title || IME_CLASSES.has(w.cls)) continue;
      const key = `${w.pid}/${w.hwnd}`;
      const shown = w.visible && !w.iconic && w.monitor;
      if (!sessions.has(w.pid)) sessions.set(w.pid, false);
      if (shown) {
        seenVisible.add(key);
        sessions.set(w.pid, true);
        continue;
      }
      if (!seenVisible.has(key) || reported.has(key)) continue;
      const state = w.iconic ? 'minimised' : !w.monitor ? 'on no monitor' : 'invisible';
      if (state === 'invisible') {
        // Closing: gone (window or process) in a later sample within the grace.
        const gone = samples
          .slice(i + 1)
          .some((later) => later.t - s.t <= graceMs && !later.windows.some((x) => x.pid === w.pid && x.hwnd === w.hwnd));
        if (gone) {
          notes.push(`${windowName(w)} was hidden at ${s.t} ms and gone within ${graceMs} ms: closing`);
          continue;
        }
      }
      reported.add(key);
      failures.push(`WINDOW ${state.toUpperCase()}: ${windowName(w)} was seen visible, then sampled ${state} at ${s.t} ms`);
    }
  }
  for (const [pid, visible] of sessions) {
    if (!visible) failures.push(`app.exe pid ${pid} had titled windows but none was ever seen visible`);
  }
  const min = opts.minSessions ?? 0;
  if (sessions.size < min) failures.push(`the watcher saw ${sessions.size} app.exe session(s) with a window, and the gate opened at least ${min}`);
  notes.push(`${sessions.size} app.exe session(s) seen, ${samples.length} sample(s)`);

  return { ok: failures.length === 0, failures, excused, notes, sessions: sessions.size };
}
