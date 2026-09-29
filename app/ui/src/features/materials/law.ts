// The reflection law of a material, as the materials grid's Law column shows and sets it (M11
// PLAN.md 1.2, row 22 item M5). A pure module: no store, no backend, only erasable TypeScript,
// tested by law.test.ts under `node --test`.
import type { Material, ReflectionLaw } from '../../bindings/schema.ts';

export interface LawOption {
  law: ReflectionLaw;
  label: string;
  /** What the solvers do with it, where that is not what the name says. */
  note: string;
}

/**
 * Every law, in the core's `ReflectionLaw::ALL` order (the solver codes 0 to 6), with the names
 * upstream's GUI gives them. The notes are the schema's own (`ReflectionLaw`'s documentation).
 */
export const LAWS: readonly LawOption[] = [
  { law: 'specular', label: 'Specular', note: '' },
  { law: 'uniform', label: 'Uniform', note: '' },
  { law: 'lambert', label: 'Lambert', note: '' },
  { law: 'w2', label: 'W2', note: '' },
  { law: 'w3', label: 'W3', note: '' },
  { law: 'w4', label: 'W4', note: '' },
  {
    law: 'semi_diffuse',
    label: 'Semi-diffuse',
    note: 'SPPS has no semi-diffuse case and reflects these surfaces specularly (spps/tools/dotreflection.h)',
  },
];

/** What a scattering of 0 means for every law: SPPS draws a diffuse reflection with probability
 * equal to the scattering coefficient, so the law then has no effect. */
export const SCATTERING_NOTE = 'The law shapes the diffuse part only: at scattering 0 it has no effect in SPPS.';

/** The label of a law, or the law's own spelling for one this build does not know. */
export function lawLabel(law: string): string {
  return LAWS.find((o) => o.law === law)?.label ?? law;
}

/** A material's law as the column shows it: one law for every band, or one per band (only a
 * `.proj` import makes those). */
export type LawState = { kind: 'all'; law: ReflectionLaw } | { kind: 'per_band'; laws: readonly ReflectionLaw[] };

export function lawState(m: Pick<Material, 'reflection_law'>): LawState {
  const l = m.reflection_law;
  return Array.isArray(l) ? { kind: 'per_band', laws: l } : { kind: 'all', law: l };
}

/** Whether `law` is `m`'s law in any band. */
export function usesLaw(m: Pick<Material, 'reflection_law'>, law: ReflectionLaw): boolean {
  const s = lawState(m);
  return s.kind === 'all' ? s.law === law : s.laws.includes(law);
}

/** The line under the grid while a material uses a law SPPS does not do as named. */
export const SEMI_DIFFUSE_NOTE =
  'Semi-diffuse: SPPS has no semi-diffuse case and reflects those surfaces specularly (spps/tools/dotreflection.h).';

/** The select's value: the law, or `per_band` for a per-band law (no single law is chosen). */
export const PER_BAND = 'per_band';
export function lawValue(state: LawState): string {
  return state.kind === 'all' ? state.law : PER_BAND;
}

/** The value a choice in the select stands for, or null for the per-band placeholder or a value
 * that is not a law. */
export function lawOf(value: string): ReflectionLaw | null {
  return LAWS.find((o) => o.law === value)?.law ?? null;
}

/**
 * Whether choosing `law` changes the material: a per-band law always does (it becomes one law for
 * every band); the same single law does not, so no empty undo step is made.
 */
export function changesLaw(m: Pick<Material, 'reflection_law'>, law: ReflectionLaw): boolean {
  const s = lawState(m);
  return s.kind === 'per_band' || s.law !== law;
}

/**
 * The column's tooltip: the law and its note, or each band's law for a per-band law, as
 * `125 Hz Lambert`, one band per line. `frequencies` are the project's bands in project order.
 */
export function lawTitle(state: LawState, frequencies: readonly number[]): string {
  if (state.kind === 'all') {
    const note = LAWS.find((o) => o.law === state.law)?.note;
    return [`Reflection law: ${lawLabel(state.law)} in every band`, note, SCATTERING_NOTE].filter(Boolean).join('\n');
  }
  const bands = state.laws.map((l, i) => {
    const f = frequencies[i];
    return `${f === undefined ? `band ${i + 1}` : `${f} Hz`} ${lawLabel(l)}`;
  });
  return ['Reflection law per band (from an I-Simpa project):', ...bands, 'Choosing a law sets it for every band.'].join('\n');
}
