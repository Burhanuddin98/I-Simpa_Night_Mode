// The planted particle loss (M11 review F1). Every real run the gate makes loses 0 of 150,000
// particles in every band, so a Runs row, a Simulate panel or a diagnostic span that printed a
// constant "0.00" passed every check that compares the screen with run.json (the review's
// mutation M01: 13 of 13 passed). This is the regression arc item 12 paid for: about 99.997 %
// of the particles lost while every gate reported success.
//
// tools/gates/m11.ps1 makes one real run of the box (`simpa run`, the verified solvers) in its
// own project folder on C:, `p\loss`, then rewrites that run's `particles` table here with the
// losses below, moving each lost particle out of `absorbed_by_materials` so every band still
// sums to its total. Nothing else in run.json changes. The worst band is 500 Hz, not the first,
// at 0.82 %: under the 1 % limit, so the core's OK verdict stays true of the table it now holds.
// The Results step's own check reads the solver's output files, not this table, and is not
// touched.
//
// Run: node app/e2e/lib/plant-loss.ts <run folder>   (prints one JSON line: what was planted)
import { readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { bandLosses, MANIFEST_FILE, worstLoss, type Manifest } from './runs.ts';

/** The losses planted, per band. The others keep the run's own (zero) loss. */
export const PLANTED_BANDS: readonly { freq_hz: number; lost_by_infinite_loops: number; lost_by_meshing_problems: number }[] = [
  // 150 of 150,000: 0.10 %.
  { freq_hz: 125, lost_by_infinite_loops: 0, lost_by_meshing_problems: 150 },
  // 1,234 of 150,000: 0.8227 %, shown 0.82 %: the worst band.
  { freq_hz: 500, lost_by_infinite_loops: 400, lost_by_meshing_problems: 834 },
  // 7 of 150,000: 0.0047 %, shown 0.00 %: a loss that rounds to nothing is still listed as lost.
  { freq_hz: 1000, lost_by_infinite_loops: 7, lost_by_meshing_problems: 0 },
  // 1,000 of 150,000: 0.6667 %, shown 0.67 % (rounded half up).
  { freq_hz: 2000, lost_by_infinite_loops: 0, lost_by_meshing_problems: 1000 },
];

/** What the screen must show for the planted run, worked by hand from `PLANTED_BANDS`. */
export const PLANTED = {
  worst_pct: '0.82',
  worst_band_hz: 500,
  /** Per band, `lost of total` and the percentage, as the opened Runs row prints them. */
  bands: { 125: ['150', '0.10'], 250: ['0', '0.00'], 500: ['1234', '0.82'], 1000: ['7', '0.00'], 2000: ['1000', '0.67'], 4000: ['0', '0.00'] } as Record<number, [string, string]>,
  total: 150_000,
  limit_pct: '1',
} as const;

interface Band {
  freq_hz: number;
  absorbed_by_materials: number;
  lost_by_infinite_loops: number;
  lost_by_meshing_problems: number;
  total: number;
}

/**
 * run.json's text with the planted losses. Refuses (throws) unless the run is the one the plan
 * was worked for: an OK SPPS run at the 1 % limit, 150,000 particles in each of the six octave
 * bands 125 Hz to 4 kHz, no loss of its own, and enough particles absorbed by the materials to
 * move.
 */
export function plantLoss(text: string): string {
  const m = JSON.parse(text) as Manifest & { particles: { bands: Band[] } | null };
  const why = (s: string) => new Error(`plantLoss: ${s}`);
  if (m.solver !== 'spps' || m.verdict.status !== 'OK') throw why(`the run is ${m.solver} ${m.verdict.status}, not an OK SPPS run`);
  if (m.loss_limit !== 0.01) throw why(`the loss limit is ${m.loss_limit}, not 0.01`);
  const bands = m.particles?.bands ?? [];
  const freqs = bands.map((b) => b.freq_hz).join(',');
  if (freqs !== '125,250,500,1000,2000,4000') throw why(`the bands are ${freqs}`);
  for (const b of bands) {
    if (b.total !== PLANTED.total) throw why(`${b.freq_hz} Hz has ${b.total} particles, not ${PLANTED.total}`);
    if (b.lost_by_infinite_loops !== 0 || b.lost_by_meshing_problems !== 0) throw why(`${b.freq_hz} Hz already lost particles`);
    const p = PLANTED_BANDS.find((x) => x.freq_hz === b.freq_hz);
    if (!p) continue;
    const moved = p.lost_by_infinite_loops + p.lost_by_meshing_problems;
    if (b.absorbed_by_materials < moved) throw why(`${b.freq_hz} Hz has only ${b.absorbed_by_materials} particles absorbed by the materials`);
    b.absorbed_by_materials -= moved;
    b.lost_by_infinite_loops = p.lost_by_infinite_loops;
    b.lost_by_meshing_problems = p.lost_by_meshing_problems;
  }
  // The plan and its hand-worked display agree (the formulas are runs.ts's, which runs.test.ts
  // holds to the app's Rust unit test).
  const w = worstLoss(m);
  if (!w || w.pct !== PLANTED.worst_pct || w.freq_hz !== PLANTED.worst_band_hz) throw why(`the worst band is ${w?.freq_hz} Hz at ${w?.pct} %`);
  for (const b of bandLosses(m)) {
    const want = PLANTED.bands[b.freq_hz];
    if (!want || String(b.lost) !== want[0] || b.pct !== want[1]) throw why(`${b.freq_hz} Hz is ${b.lost} lost, ${b.pct} %`);
  }
  return `${JSON.stringify(m, null, 2)}\n`;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const dir = process.argv[2];
  if (!dir) {
    console.error('usage: node plant-loss.ts <run folder>');
    process.exit(2);
  }
  const file = path.join(dir, MANIFEST_FILE);
  writeFileSync(file, plantLoss(readFileSync(file, 'utf8')));
  console.log(JSON.stringify({ planted: file, worst_pct: PLANTED.worst_pct, worst_band_hz: PLANTED.worst_band_hz }));
}
