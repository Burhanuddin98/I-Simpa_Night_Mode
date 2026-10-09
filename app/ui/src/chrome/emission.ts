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
import { aWeightDb } from './aweight.ts';

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

// ---- Parity M17 and M48: the project's spectrum library ---------------------------------------
//
// Upstream keeps user spectra in the project (`tree_scene/e_scene_bdd_spectrums_user.h`) and a
// source names one by `idspectre`, staying linked to it: editing the user spectrum changes every
// source that uses it (`generic_element/e_property_freq.cpp:151-175`). Here an entry is a name and
// a level per band; a source linked to it (`power.library`) has the entry's levels as its shape and
// its own global level, and the core moves every linked shape when the entry is replaced
// (`replace_spectrum`).

/** An entry of the project's spectrum library, as the schema stores it (`UserSpectrum`). */
export interface UserSpectrumLike {
  id: string;
  name: string;
  levels_db: readonly F64[];
}

type Power = Source['power'];

/** The select's value for a power: `lib:<id>` while it is linked to a library entry, else
 * {@link spectrumKey} of its shape. */
export function powerKey(power: Power, reference: readonly LibrarySpectrumLike[]): string {
  return power.library ? `lib:${power.library}` : spectrumKey(power.shape, reference);
}

/** The select's options: upstream's reference list, then the project's library (`lib:<id>`),
 * then "Typed per band" (shown only while it is). */
export function powerOptions(
  reference: readonly LibrarySpectrumLike[],
  user: readonly UserSpectrumLike[],
  current: string,
): { value: string; label: string; group: 'reference' | 'library' | 'typed' }[] {
  const out: { value: string; label: string; group: 'reference' | 'library' | 'typed' }[] = spectrumOptions(reference, 'none').map((o) => ({
    ...o,
    group: 'reference',
  }));
  for (const u of user) out.push({ value: `lib:${u.id}`, label: u.name, group: 'library' });
  if (current === 'custom') out.push({ value: 'custom', label: 'Typed per band', group: 'typed' });
  return out;
}

/** `power` without a library link: the shape and global level kept. */
export function unlinked(power: Power): Power {
  const { library: _, ...rest } = power;
  return rest;
}

/** `power` linked to `entry`: the entry's levels as its shape, its own global level kept. */
export function linkedTo(power: Power, entry: UserSpectrumLike): Power {
  return { ...power, shape: { kind: 'custom', relative_db: [...entry.levels_db] }, library: entry.id };
}

/** The power a choice in the select gives, or null when it is no change or not a choice: a library
 * entry links it; a reference spectrum sets its shape and drops any link. */
export function powerFor(
  value: string,
  power: Power,
  reference: readonly LibrarySpectrumLike[],
  user: readonly UserSpectrumLike[],
): Power | null {
  if (value === powerKey(power, reference)) return null;
  if (value.startsWith('lib:')) {
    const entry = user.find((u) => u.id === value.slice(4));
    return entry ? linkedTo(power, entry) : null;
  }
  const shape = shapeFor(value, reference);
  return shape ? { ...unlinked(power), shape } : null;
}

/** A new library entry holding `power`'s band levels on the bands, named `name`; null when the
 * shape does not fit the bands. */
export function entryFrom(id: string, name: string, power: Power, frequenciesHz: readonly number[]): UserSpectrumLike | null {
  const levels = bandLevels(power, frequenciesHz);
  return levels ? { id, name, levels_db: levels } : null;
}

/** `entry` with band `band` at `level` dB, every other band kept; null when nothing changes. */
export function entryWithLevel(entry: UserSpectrumLike, band: number, level: number): UserSpectrumLike | null {
  if (band < 0 || band >= entry.levels_db.length || Object.is(num(entry.levels_db[band]), level)) return null;
  const levels_db = [...entry.levels_db];
  levels_db[band] = level;
  return { ...entry, levels_db };
}

/** What is linked to entry `id`, named as the Console and the editor name them. */
export function entryUsers(
  id: string,
  view: { sources: readonly Pick<Source, 'name' | 'power'>[]; point_receivers: readonly { name: string; background_noise: Power | null }[] },
): string[] {
  return [
    ...view.sources.filter((s) => s.power.library === id).map((s) => s.name),
    ...view.point_receivers.filter((r) => r.background_noise?.library === id).map((r) => `${r.name} (background noise)`),
  ];
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

// ---- Parity M18 and M46: the spectrum editor's columns and its Global row ----------------------
//
// Upstream's source spectrum is a table (`generic_element/e_property_freq.cpp`,
// `e_data_row_ext_bandefreq.h`): per band Lw (the spectrum's own level), an attenuation, dB = Lw -
// attenuation (what config.xml writes) and dB(A) = dB + the band's A-weighting
// (`e_data_row_bandefreq.h:128-133`); and a Global row whose dB, dB(A) and Lw are each the
// energetic sum of the bands (`CalcNiveauSonoreGlobal`, `e_property_freq.cpp:256-272`). Typing a
// global moves every band by the same amount (`SetGlobalLevel`, `:331-358`). Here the Global row's
// attenuation is Lw - dB, the overall attenuation (upstream's own reading of it when it upgrades an
// old project, `e_property_freq.cpp:103`); upstream's energetic sum of the band attenuations reads
// 8.5 dB for seven bands of 0 dB, which tells a user nothing.

/** One row of the table, in dB. `dba` is null when a band has no A-weighting in the table. */
export interface LevelRow {
  lw: number;
  att: number;
  db: number;
  dba: number | null;
}

/** Each band's attenuation, in dB: the stored ones, or 0 in every band. */
export function attenuations(power: Power, n: number): number[] {
  const a = power.attenuation_db;
  return a && a.length === n ? a.map(num) : Array.from({ length: n }, () => 0);
}

/** The table of a power on the bands: a row per band and the Global row; null when the shape does
 * not fit the bands. */
export function spectrumTable(power: Power, frequenciesHz: readonly number[]): { bands: LevelRow[]; global: LevelRow } | null {
  const lw = bandLevels(power, frequenciesHz);
  if (!lw) return null;
  const att = attenuations(power, lw.length);
  const db = lw.map((l, i) => l - att[i]);
  const w = frequenciesHz.map(aWeightDb);
  const dba = w.every((x) => x !== null) ? db.map((d, i) => d + (w[i] as number)) : null;
  const bands = lw.map((l, i) => ({ lw: l, att: att[i], db: db[i], dba: dba ? dba[i] : null }));
  const gLw = energySum(lw);
  const gDb = energySum(db);
  return { bands, global: { lw: gLw, att: gLw - gDb, db: gDb, dba: dba ? energySum(dba) : null } };
}

/** `power` with these attenuations, stored as none when every band is 0, so a project whose
 * attenuations are set back to 0 is the project it was. */
function withAttenuation(power: Power, att: readonly number[]): Power {
  const { attenuation_db: _, ...rest } = power;
  return att.every((a) => a === 0) ? rest : { ...rest, attenuation_db: [...att] };
}

/** The power with its Global `column` typed as `value` dB: every band moves by the same amount, as
 * upstream's `SetGlobalLevel` does. dB, dB(A) and Lw move the spectrum's own level (a linked source
 * stays linked); the attenuation moves every band's attenuation. Null when nothing changes. */
export function withGlobal(power: Power, frequenciesHz: readonly number[], column: 'db' | 'dba' | 'att' | 'lw', value: number): Power | null {
  const t = spectrumTable(power, frequenciesHz);
  if (!t) return null;
  if (column === 'lw') return Object.is(num(power.global_db), value) ? null : { ...power, global_db: value };
  const current = t.global[column];
  if (current === null) return null;
  const delta = value - current;
  if (delta === 0) return null;
  if (column === 'att') return withAttenuation(power, t.bands.map((b) => b.att + delta));
  return { ...power, global_db: num(power.global_db) + delta };
}

/** The power with band `band`'s attenuation at `value` dB, every other band kept; a linked source
 * stays linked (upstream attenuates a user spectrum band by band, `e_data_row_ext_bandefreq.h`).
 * Null when nothing changes. */
export function withBandAttenuation(power: Power, n: number, band: number, value: number): Power | null {
  const att = attenuations(power, n);
  if (band < 0 || band >= n || Object.is(att[band], value)) return null;
  att[band] = value;
  return withAttenuation(power, att);
}

/** The power with band `band`'s dB (what the solver gets) typed as `value`: its Lw becomes `value`
 * plus its attenuation, the other bands kept, typed per band (a link to the library ends). */
export function withBandDb(power: Power, frequenciesHz: readonly number[], band: number, value: number): Power | null {
  const att = attenuations(power, frequenciesHz.length);
  if (band < 0 || band >= att.length) return null;
  const next = withBandLevel(power, frequenciesHz, band, value + att[band]);
  return next ? withAttenuation(next, att) : null;
}

/** The power with band `band`'s dB(A) typed as `value`: its dB is `value` minus the band's
 * A-weighting. Null for a band with no A-weighting. */
export function withBandDba(power: Power, frequenciesHz: readonly number[], band: number, value: number): Power | null {
  const w = aWeightDb(frequenciesHz[band]);
  return w === null ? null : withBandDb(power, frequenciesHz, band, value - w);
}

/** A library entry's table: dB as stored, dB(A) per band, and the Global row (energetic sums). */
export function entryTable(
  entry: UserSpectrumLike,
  frequenciesHz: readonly number[],
): { bands: { db: number; dba: number | null }[]; global: { db: number; dba: number | null } } {
  const db = entry.levels_db.map(num);
  const w = frequenciesHz.map(aWeightDb);
  const dba = w.length === db.length && w.every((x) => x !== null) ? db.map((d, i) => d + (w[i] as number)) : null;
  return {
    bands: db.map((d, i) => ({ db: d, dba: dba ? dba[i] : null })),
    global: { db: energySum(db), dba: dba ? energySum(dba) : null },
  };
}

/** A library entry with its Global dB or dB(A) typed as `value`: every band moves by the same amount. */
export function entryWithGlobal(entry: UserSpectrumLike, frequenciesHz: readonly number[], column: 'db' | 'dba', value: number): UserSpectrumLike | null {
  const current = entryTable(entry, frequenciesHz).global[column];
  if (current === null || current === value) return null;
  const delta = value - current;
  return { ...entry, levels_db: entry.levels_db.map((l) => num(l) + delta) };
}

/** A library entry with band `band`'s dB(A) typed as `value`. */
export function entryWithBandDba(entry: UserSpectrumLike, frequenciesHz: readonly number[], band: number, value: number): UserSpectrumLike | null {
  const w = aWeightDb(frequenciesHz[band]);
  return w === null ? null : entryWithLevel(entry, band, value - w);
}
