// B3 (decision 71): the running GPU solve on screen. Pure, tested by live.test.ts under `node --test`.
//
// While SPPS runs on the GPU the backend tails the solver's stream and sends each band's newly saved
// particles as a LIVE v1 batch (app/src-tauri/src/live.rs): a 32-byte envelope, then the PART v1
// layout the Results step already decodes. They are the run's *saved sample* (the project's
// "particles saved per source"), the same particles the Results step replays after the run, not
// every particle traced.
//
// The live layer plays them in simulation time on its own clock: the clock starts when the first
// batch arrives, at that batch's earliest first step, and runs at LIVE_SPEED of real time. A
// trajectory that arrives once the clock is past its first step starts from its first record at the
// clock (its steps are shifted), so the room keeps filling instead of jumping. Older trajectories are
// let go once the layer holds LIVE_MAX_RECORDS records, oldest arrival first.
import { decodeParticles, type Particles } from '../../resultsData.ts';

export const LIVE_MAGIC = 0x4556494c;
export const LIVE_HEADER = 32;
/** The live clock as a fraction of real time: 50 ms of sound per second (the timeline's default is 10). */
export const LIVE_SPEED = 0.05;
/** Records the live layer holds at most (about 16 MB of GPU tables). */
export const LIVE_MAX_RECORDS = 1_000_000;

export interface LiveBatch {
  /** Particles arrived in the run so far, this batch included. */
  soFar: number;
  /** Particles the run saves in all (per band x bands). */
  total: number;
  /** The batch's band among the computed bands, 0-based. */
  bandIndex: number;
  bands: number;
  particles: Particles;
}

/** Throws on a buffer that is not a LIVE v1 batch holding a PART v1. */
export function decodeLiveBatch(buf: ArrayBuffer): LiveBatch {
  if (buf.byteLength < LIVE_HEADER) throw new Error(`run live: ${buf.byteLength} bytes, shorter than the envelope`);
  const v = new DataView(buf);
  const magic = v.getUint32(0, true);
  if (magic !== LIVE_MAGIC) throw new Error(`run live: magic 0x${magic.toString(16)}, not "LIVE"`);
  if (v.getUint32(4, true) !== 1) throw new Error(`run live: version ${v.getUint32(4, true)}, this view reads 1`);
  return {
    soFar: v.getUint32(8, true),
    total: v.getUint32(12, true),
    bandIndex: v.getUint32(16, true),
    bands: v.getUint32(20, true),
    particles: decodeParticles(buf.slice(LIVE_HEADER)),
  };
}

/** One arrived trajectory, its first step as the live layer plays it. */
interface Track {
  firstStep: number;
  positions: Float32Array;
  energies: Float32Array;
}

export interface LiveSummary {
  soFar: number;
  total: number;
  bandHz: number;
  bandIndex: number;
  bands: number;
  /** Trajectories and records held now. */
  held: number;
  records: number;
  /** Trajectories let go to stay under LIVE_MAX_RECORDS. */
  dropped: number;
}

/** The caption the Simulate step shows over the view while the run is live. */
export function liveCaption(s: Pick<LiveSummary, 'soFar' | 'total' | 'bandHz'> | null): string {
  if (!s) return 'live: waiting for the first saved particles';
  return `live: the saved sample of particles, ${s.soFar.toLocaleString('en-US')} of ${s.total.toLocaleString('en-US')} so far, band ${s.bandHz} Hz`;
}

export class LiveSet {
  private tracks: Track[] = [];
  private head = 0;
  private recordsHeld = 0;
  private t0: number | null = null;
  private start = 0;
  private dtMs = 1;
  private droppedCount = 0;
  private last: LiveBatch | null = null;
  private version = 0;

  private readonly speed: number;
  private readonly maxRecords: number;

  constructor(speed = LIVE_SPEED, maxRecords = LIVE_MAX_RECORDS) {
    this.speed = speed;
    this.maxRecords = maxRecords;
  }

  /** The clock in steps at `nowMs` (fractional), or null before the first batch. */
  clock(nowMs: number): number | null {
    if (this.t0 === null) return null;
    return this.start + (Math.max(0, nowMs - this.t0) / 1000) * ((this.speed * 1000) / this.dtMs);
  }

  /** Adds a batch that arrived at `nowMs`: each trajectory starts at its own first step, or at the clock when that has passed. */
  add(b: LiveBatch, nowMs: number): void {
    const p = b.particles;
    if (this.t0 === null && p.particleCount > 0) {
      this.t0 = nowMs;
      // the file's float32 step (0.001 is 0.0010000000475): to the nanosecond, so whole steps land whole
      this.dtMs = p.timeStepS > 0 ? Math.round(p.timeStepS * 1e9) / 1e6 : 1;
      let first = Infinity;
      for (let i = 0; i < p.particleCount; i++) first = Math.min(first, p.firstStep[i]);
      this.start = first;
    }
    const now = Math.floor(this.clock(nowMs) ?? 0);
    for (let i = 0; i < p.particleCount; i++) {
      const a = p.offsets[i];
      const z = p.offsets[i + 1];
      if (z <= a) continue;
      this.tracks.push({
        firstStep: Math.max(p.firstStep[i], now),
        positions: p.positions.slice(3 * a, 3 * z),
        energies: p.energies.slice(a, z),
      });
      this.recordsHeld += z - a;
    }
    while (this.recordsHeld > this.maxRecords && this.tracks.length - this.head > 1) {
      this.recordsHeld -= this.tracks[this.head].energies.length;
      this.head++;
      this.droppedCount++;
    }
    if (this.head > 1024 && this.head * 2 > this.tracks.length) {
      this.tracks = this.tracks.slice(this.head);
      this.head = 0;
    }
    this.last = b;
    this.version++;
  }

  /** Changes with every batch (the view rebuilds its tables only when it moved). */
  revision(): number {
    return this.version;
  }

  summary(): LiveSummary | null {
    const b = this.last;
    if (!b) return null;
    return {
      soFar: b.soFar,
      total: b.total,
      bandHz: b.particles.bandHz,
      bandIndex: b.bandIndex,
      bands: b.bands,
      held: this.tracks.length - this.head,
      records: this.recordsHeld,
      dropped: this.droppedCount,
    };
  }

  /** The held trajectories as one PART-shaped set, their first steps as played. */
  particles(): Particles | null {
    const b = this.last;
    const n = this.tracks.length - this.head;
    if (!b || n === 0) return null;
    const firstStep = new Uint32Array(n);
    const offsets = new Uint32Array(n + 1);
    const positions = new Float32Array(3 * this.recordsHeld);
    const energies = new Float32Array(this.recordsHeld);
    let at = 0;
    let maxStep = 0;
    for (let i = 0; i < n; i++) {
      const t = this.tracks[this.head + i];
      firstStep[i] = t.firstStep;
      positions.set(t.positions, 3 * at);
      energies.set(t.energies, at);
      at += t.energies.length;
      offsets[i + 1] = at;
      maxStep = Math.max(maxStep, t.firstStep + t.energies.length);
    }
    return {
      particleCount: n,
      recordCount: at,
      maxSteps: Math.max(b.particles.maxSteps, maxStep),
      timeStepS: b.particles.timeStepS,
      bandHz: b.particles.bandHz,
      firstStep,
      offsets,
      positions,
      energies,
    };
  }
}

/**
 * One GPU run's live layer, from its start to its end, apart from the view (liveView.ts drives the
 * view from it). Batches of this run are kept from start to end whether the Simulate step is shown or
 * not: leaving the step costs no frames, only the table rebuilds, which wait until the step is shown
 * again and then draw everything that arrived (the clock is wall time, so it shows where the run is
 * now). Another run's batches, and anything after this run's end, are dropped.
 */
export class LiveRun {
  readonly runId: number;
  readonly set: LiveSet;
  private over = false;
  private builtRev = -1;

  constructor(runId: number, set: LiveSet = new LiveSet()) {
    this.runId = runId;
    this.set = set;
  }

  /** Keeps `b` if it is this run's and the run is live; returns whether it was kept. */
  accept(runId: number, b: LiveBatch, nowMs: number): boolean {
    if (runId !== this.runId || this.over) return false;
    this.set.add(b, nowMs);
    return true;
  }

  /** The end of run `runId` (ended, failed or the stream's last batch): true when it ends this run. */
  end(runId: number): boolean {
    if (runId !== this.runId || this.over) return false;
    this.over = true;
    return true;
  }

  ended(): boolean {
    return this.over;
  }

  /** Whether the view should rebuild its tables now: live, shown, and something arrived since the last build. */
  wantsBuild(shown: boolean): boolean {
    return !this.over && shown && this.set.revision() !== this.builtRev;
  }

  /** The view built the tables from the set as it is now. */
  built(): void {
    this.builtRev = this.set.revision();
  }

  /** The next build is wanted even with nothing new (the look changed). */
  invalidate(): void {
    this.builtRev = -1;
  }
}
