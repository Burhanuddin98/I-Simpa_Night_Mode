// Run folders read from outside the app (docs/investigations/2026-09-29-m11/PLAN.md 4.3): the
// core's run.json, the logs beside it, and the Runs tab's numbers recomputed from them with the
// integer rules of PLAN.md 2.3 (T14), so a check never depends on how Rust or JavaScript rounds
// a float at an exact half. Pure but for the file reads; `runs.test.ts` holds the formulas to
// hand-worked values and to the Rust unit test's cases (app/src-tauri/src/runs.rs).
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import path from 'node:path';

/** The fields of run.json the gate reads (crates/simpa-core/src/run/manifest.rs). */
export interface Manifest {
  stage: string;
  exit_class: number;
  source: { kind: string; path?: string; variant?: string | null };
  solver: string;
  exe: { path: string; sha256: string };
  solvers?: { name: string; path: string; matches: boolean }[] | null;
  outcome: { exit_code: number | null; cancelled: boolean; elapsed_ms: number } | null;
  lines: { progress: number; info: number; ok: number; warn: number; fail: number; unclassified: number };
  files: { total: number; expected: number; present: number };
  particles: { bands: BandStats[] } | null;
  loss_limit: number | string;
  verdict: { status: string; reasons: { code: string; detail: string }[]; warnings: { code: string; detail: string }[] };
}

export interface BandStats {
  freq_hz: number;
  lost_by_infinite_loops: number;
  lost_by_meshing_problems: number;
  total: number;
}

export const MANIFEST_FILE = 'run.json';

/** `yyyyMMdd-HHmmss-fff-(spps|tcr)` and an optional `-<n>` (runs.rs `is_run_name`). */
export const RUN_NAME = /^\d{8}-\d{6}-\d{3}-(spps|tcr)(-\d+)?$/;

/** The run folders under `root`, in start order (stamp, then suffix); none when `root` is absent. */
export function runFolders(root: string): string[] {
  if (!existsSync(root)) return [];
  const order = (n: string): [string, number] => {
    const m = /-(\d+)$/.exec(n.slice(20));
    return [n.slice(0, 19), m ? Number(m[1]) : 1];
  };
  return readdirSync(root)
    .filter((n) => RUN_NAME.test(n) && statSync(path.join(root, n)).isDirectory())
    .sort((a, b) => {
      const [sa, na] = order(a);
      const [sb, nb] = order(b);
      return sa < sb ? -1 : sa > sb ? 1 : na - nb;
    });
}

/** `<runDir>/run.json`, parsed; `null` when there is none. */
export function readManifest(runDir: string): Manifest | null {
  const p = path.join(runDir, MANIFEST_FILE);
  if (!existsSync(p)) return null;
  return JSON.parse(readFileSync(p, 'utf8')) as Manifest;
}

// ---- the numbers, as runs.rs formats them ---------------------------------------------------

/** A band's loss in hundredths of a percent, rounded half up in integers:
 * `(lost·10000·2 + total) / (2·total)`; a band with no particles has lost none. */
export function lossHundredths(lost: bigint, total: bigint): bigint {
  if (total === 0n) return 0n;
  return (lost * 20_000n + total) / (2n * total);
}

/** `lossHundredths` printed as `x.yz`. */
export function lossPct(lost: bigint, total: bigint): string {
  const h = lossHundredths(lost, total);
  return `${h / 100n}.${(h % 100n).toString().padStart(2, '0')}`;
}

export interface BandLoss {
  freq_hz: number;
  lost: bigint;
  total: bigint;
  hundredths: bigint;
  pct: string;
}

/** Each band's loss (lost by meshing problems plus lost by infinite loops, over its total). */
export function bandLosses(m: Manifest): BandLoss[] {
  return (m.particles?.bands ?? []).map((b) => {
    const lost = BigInt(b.lost_by_infinite_loops) + BigInt(b.lost_by_meshing_problems);
    const total = BigInt(b.total);
    return { freq_hz: b.freq_hz, lost, total, hundredths: lossHundredths(lost, total), pct: lossPct(lost, total) };
  });
}

/** The largest band's loss, the first such band on a tie; `null` with no statistics table. */
export function worstLoss(m: Manifest): BandLoss | null {
  let worst: BandLoss | null = null;
  for (const b of bandLosses(m)) if (worst === null || b.hundredths > worst.hundredths) worst = b;
  return worst;
}

/** The loss limit (a fraction) in percent: `limit × 100` as its shortest decimal (Rust's `{}`
 * of an f64: `1` for 1.0, no exponent in the range a limit takes). JSON has no NaN or infinity,
 * so run.json writes them as strings. */
export function limitPct(limit: number | string): string {
  if (limit === 'NaN') return 'NaN';
  if (limit === 'inf') return 'inf';
  if (limit === '-inf') return '-inf';
  // The point of the limit's own shortest decimal moved two places, in the text: `0.07 * 100` is
  // 7.000000000000001 in doubles, and the product would pass a UI printing that (M11 review 2,
  // app 5). Written apart from runs.rs `limit_pct`, from the decimal string, not the double.
  const v = limit as number;
  const text = String(Math.abs(v));
  if (/e/i.test(text)) throw new Error(`limitPct: ${v} prints in exponent form in JavaScript, not in Rust`);
  const [int, frac = ''] = text.split('.');
  const digits = int + frac.padEnd(2, '0');
  const at = int.length + 2;
  const whole = digits.slice(0, at).replace(/^0+/, '') || '0';
  const part = digits.slice(at).replace(/0+$/, '');
  return `${v < 0 ? '-' : ''}${whole}${part ? `.${part}` : ''}`;
}

/** A wall time in ms as seconds with one decimal: `floor(ms / 100 + 0.5) / 10`, the same IEEE
 * operations as runs.rs `elapsed_s`, printed from the integer number of tenths. */
export function elapsedS(ms: number): string {
  const tenths = Math.floor(ms / 100 + 0.5);
  const t = Number.isFinite(tenths) && tenths > 0 ? BigInt(tenths) : 0n;
  return `${t / 10n}.${t % 10n}`;
}

// ---- the logs ----------------------------------------------------------------------------------

/** A log file's lines as the core streamed them: split on `\n` with a `\r` before it dropped, the
 * bytes decoded as lossy UTF-8. A final empty piece (the file ended in a newline) is no line. */
export function logLines(file: string): string[] {
  if (!existsSync(file)) return [];
  const lines = readFileSync(file, 'utf8').split('\n').map((l) => (l.endsWith('\r') ? l.slice(0, -1) : l));
  if (lines.length > 0 && lines[lines.length - 1] === '') lines.pop();
  return lines;
}

/** Every `*.stdout.txt` and `*.stderr.txt` under `dir`, recursively (the mesher's TetGen, its
 * `diag/` follow-up and preprocess.exe). */
function streamLogs(dir: string): string[] {
  if (!existsSync(dir)) return [];
  const out: string[] = [];
  for (const e of readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) out.push(...streamLogs(p));
    else if (/\.(stdout|stderr)\.txt$/.test(e.name)) out.push(p);
  }
  return out.sort();
}

export type LogSource = 'solver' | 'mesh';

/** The files a verbatim line of `source` may come from: the solver's two streams, or every
 * program the mesher ran (its TetGen lines are streamed as source `mesh`). */
export function logFiles(runDir: string, source: LogSource): string[] {
  if (source === 'solver') return ['solver.stdout.txt', 'solver.stderr.txt'].map((f) => path.join(runDir, f));
  return streamLogs(path.join(runDir, 'mesh'));
}

/** Every line `source` wrote in the run. */
export function sourceLines(runDir: string, source: LogSource): string[] {
  return logFiles(runDir, source).flatMap(logLines);
}

/** The percentages of SPPS's progress lines (`#25.22` is `25.22`), the contract's `progress` row. */
export function progressValues(runDir: string): string[] {
  return logLines(path.join(runDir, 'solver.stdout.txt'))
    .filter((l) => /^#[0-9.eE+-]+$/.test(l))
    .map((l) => l.slice(1));
}

/** Whether SPPS printed its end of calculation (the contract's `spps_end_of_calculation`). */
export function endedCalculation(runDir: string): boolean {
  return logLines(path.join(runDir, 'solver.stdout.txt')).some((l) => /^End of calculation\.$/.test(l));
}

// ---- decimals ------------------------------------------------------------------------------------

/** A decimal numeral (`25.22`, `1.667e-05`, `100`) as an exact rational `n / 10^k`; null when it
 * is not one. */
export function parseDecimal(text: string): { n: bigint; k: number } | null {
  const m = /^([+-]?)(\d*)(?:\.(\d*))?(?:[eE]([+-]?\d+))?$/.exec(text.trim());
  if (!m || (m[2] === '' && (m[3] ?? '') === '')) return null;
  const digits = `${m[2]}${m[3] ?? ''}`.replace(/^0+(?=\d)/, '');
  let k = (m[3] ?? '').length - Number(m[4] ?? '0');
  let n = BigInt(digits === '' ? '0' : digits);
  if (k < 0) {
    n *= 10n ** BigInt(-k);
    k = 0;
  }
  return { n: m[1] === '-' ? -n : n, k };
}

/** Whether decimal numeral `a` is strictly above `b`, compared exactly (`1.667e-05`, `25.22`);
 * null when either is not a numeral. */
export function decimalAbove(a: string, b: string): boolean | null {
  const x = parseDecimal(a);
  const y = parseDecimal(b);
  if (!x || !y) return null;
  return x.n * 10n ** BigInt(y.k) > y.n * 10n ** BigInt(x.k);
}

/** A non-negative decimal rounded half up to `d` decimals, as the integer `round(x · 10^d)`. */
export function roundedTo(text: string, d: number): bigint | null {
  const x = parseDecimal(text);
  if (!x || x.n < 0n) return null;
  if (x.k <= d) return x.n * 10n ** BigInt(d - x.k);
  const div = 10n ** BigInt(x.k - d);
  return (x.n * 2n + div) / (2n * div);
}
