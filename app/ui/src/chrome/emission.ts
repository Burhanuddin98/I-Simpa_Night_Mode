// C1: a source's emission, edited (docs/investigations/2026-10-07-blank-geometry/SPEC.md, order
// of work 4). The sound power level overall or per band on the project's bands, the spectrum
// picked from upstream's reference list (the core's `spectrum_library`) or typed band by band,
// and a directivity the solvers take. A pure module: no store, no backend, only erasable
// TypeScript, tested by emission.test.ts under `node --test`.
//
// The project stores a power as a global level and a shape (`Spectrum`): the band levels are
// `L_i = global + r_i - 10 log10(sum_j 10^(r_j / 10))`, what config.xml writes as `bfreq@db`
// (`Spectrum::band_levels_db` in the core). A level typed in one band makes the shape `custom`,
// with the band levels as its relative levels and their energy sum as the global level, so the
// other bands keep their levels and the typed one is written as typed.
import type { Directivity, Source, SpectrumShape } from '../bindings/ipc.ts';

export type F64 = number | string;

/** A spectrum of the library, as the core gives it for the project's bands. */
export interface LibrarySpectrumLike {
  reference_id: number;
  name: string;
  shape: SpectrumShape;
}

const num = (v: F64): number => (typeof v === 'number' ? v : Number(v));

/** The third-octave band number of a nominal frequency (1 kHz is 0, 1.25 kHz is 1): exact for
 * every nominal band, which lies within 3 % of its base-10 centre. */
export function bandNumber(hz: number): number {
  return Math.round(10 * Math.log10(hz / 1000));
}

/** The shape's relative levels on the bands, in dB; null when a custom shape has another length. */
export function relativeDb(shape: SpectrumShape, frequenciesHz: readonly number[]): number[] | null {
  switch (shape.kind) {
    case 'pink':
      return frequenciesHz.map(() => 0);
    case 'white':
      return frequenciesHz.map(bandNumber);
    case 'custom':
      return shape.relative_db.length === frequenciesHz.length ? shape.relative_db.map(num) : null;
  }
}

/** Each band's sound power level, in dB re 1 pW, summing energetically to the global level. */
export function bandLevels(power: Source['power'], frequenciesHz: readonly number[]): number[] | null {
  const r = relativeDb(power.shape, frequenciesHz);
  if (!r || r.length === 0) return null;
  const total = r.reduce((a, x) => a + 10 ** (x / 10), 0);
  const offset = num(power.global_db) - 10 * Math.log10(total);
  return r.map((x) => x + offset);
}

/** The energy sum of levels in dB. */
export function energySum(levels: readonly number[]): number {
  return 10 * Math.log10(levels.reduce((a, l) => a + 10 ** (l / 10), 0));
}

/** The power with band `band` at `level` dB and every other band kept: a custom shape. */
export function withBandLevel(power: Source['power'], frequenciesHz: readonly number[], band: number, level: number): Source['power'] | null {
  const levels = bandLevels(power, frequenciesHz);
  if (!levels || band < 0 || band >= levels.length) return null;
  levels[band] = level;
  return { global_db: energySum(levels), shape: { kind: 'custom', relative_db: levels } };
}

/** What the spectrum select shows for a shape: `pink`, `white`, `ref:<id>` for a library
 * spectrum the shape equals, else `custom` (typed per band). */
export function spectrumKey(shape: SpectrumShape, library: readonly LibrarySpectrumLike[]): string {
  if (shape.kind !== 'custom') return shape.kind;
  const same = library.find(
    (l) =>
      l.shape.kind === 'custom' &&
      l.shape.relative_db.length === shape.relative_db.length &&
      l.shape.relative_db.every((v, i) => Object.is(num(v), num(shape.relative_db[i]))),
  );
  return same ? `ref:${same.reference_id}` : 'custom';
}

/** The select's options: upstream's list as the core gives it, then "Typed per band" (shown only
 * while it is). */
export function spectrumOptions(library: readonly LibrarySpectrumLike[], current: string): { value: string; label: string }[] {
  const out = library.map((l) => ({
    value: l.shape.kind === 'custom' ? `ref:${l.reference_id}` : l.shape.kind,
    label: l.name,
  }));
  if (current === 'custom') out.push({ value: 'custom', label: 'Typed per band' });
  return out;
}

/** The shape a select value stands for, or null (`custom` keeps the current shape). */
export function shapeFor(value: string, library: readonly LibrarySpectrumLike[]): SpectrumShape | null {
  if (value === 'pink') return { kind: 'pink' };
  if (value === 'white') return { kind: 'white' };
  if (value.startsWith('ref:')) {
    const id = Number(value.slice(4));
    return library.find((l) => l.reference_id === id)?.shape ?? null;
  }
  return null;
}

/** The directivities a source can be given here: the solvers' codes 0 to 4. A measured balloon
 * (code 5) comes with an imported project and is kept as it is; it is not offered. */
export const DIRECTIVITIES: readonly { kind: Exclude<Directivity['kind'], 'balloon'>; label: string; title: string }[] = [
  { kind: 'omni', label: 'Omni', title: 'The same in every direction (code 0)' },
  { kind: 'unidirectional', label: 'Unidirectional', title: 'Emits along one direction (code 1)' },
  { kind: 'plane_xy', label: 'Plane XY', title: 'Emits in the XY plane (code 2)' },
  { kind: 'plane_yz', label: 'Plane YZ', title: 'Emits in the YZ plane (code 3)' },
  { kind: 'plane_xz', label: 'Plane XZ', title: 'Emits in the XZ plane (code 4)' },
];

/** `current` changed to `kind`: a unidirectional source keeps its direction, or starts along +x. */
export function withDirectivity(current: Directivity, kind: string): Directivity | null {
  switch (kind) {
    case 'omni':
    case 'plane_xy':
    case 'plane_yz':
    case 'plane_xz':
      return { kind };
    case 'unidirectional':
      return { kind: 'unidirectional', direction: 'direction' in current ? current.direction : [1, 0, 0] };
    default:
      return null;
  }
}
