// C1 (docs/investigations/2026-10-07-blank-geometry/SPEC.md, order of work 4): the selected
// source's emission, edited in the Sources step. The sound power level overall
// (`[data-field="power.global_db"]`) or per band (`[data-field="power.band.<i>"]`), on the project's
// bands; the spectrum from upstream's reference list or typed per band (`[data-field="spectrum"]`);
// the directivity among the solvers' codes 0 to 4 (`[data-field="directivity"]`), with a direction
// for a unidirectional source (`[data-field="direction.x|y|z"]`). A measured balloon, which only an
// imported project brings, is kept and shown, not offered. Every change is one `replace_source`
// through the checked apply, one undo step; what the validator refuses (a zero direction, say)
// is shown under the field with the validator's words, and the project is unchanged.
import { useEffect, useState } from 'react';
import * as actions from '../actions';
import type { SceneState, Source, UiIssue } from '../bindings/ipc';
import { fieldKey, issuesForField } from '../issues';
import { NOT_A_NUMBER, parseStrictDecimal } from '../numbers';
import { replaceSource } from '../ops';
import { refusalStore, sceneStore, spectrumLibraryStore, useStore } from '../store';
import { bandLevels, DIRECTIVITIES, shapeFor, spectrumKey, spectrumOptions, withBandLevel, withDirectivity } from './emission';
import { bandLabel } from '../features/materials/bands';
import { AXES, exact } from './sceneModel';
import { CommitInput, Issues } from './SourcesPanel';

function notANumber(text: string, id: string, field: string): UiIssue {
  return {
    code: NOT_A_NUMBER,
    rule: '',
    severity: 'error',
    path: `source:${id}:${field}`,
    entity: { kind: 'source', id },
    field,
    message: `"${text}" is not a number: write digits with a decimal point, like 94.5`,
  };
}

/** The source as it is now (a commit reads the latest state, not the render's). */
const now = (id: string) => sceneStore.get()?.view.sources.find((s) => s.id === id);

export function EmissionEditor({ scene, source }: { scene: SceneState; source: Source }) {
  const refusals = useStore(refusalStore);
  const library = useStore(spectrumLibraryStore).list;
  const [local, setLocal] = useState<Readonly<Record<string, UiIssue>>>({});
  const id = source.id;
  const freqs = scene.view.bands.frequencies_hz;

  useEffect(() => {
    actions.loadSpectrumLibrary().catch(() => {});
  }, [freqs.join(',')]);

  const keyOf = (field: string) => fieldKey('source', id, field);
  const setLocalIssue = (field: string, issue: UiIssue | null) =>
    setLocal((l) => {
      const n = { ...l };
      if (issue) n[field] = issue;
      else delete n[field];
      return n;
    });
  /** One `replace_source` of the latest source, changed by `change`; filed under `field`. */
  const replace = async (field: string, change: (s: Source) => Source | null): Promise<boolean> => {
    const s = now(id);
    if (!s) return false;
    const next = change(s);
    if (!next) return true;
    const out = await actions.apply(replaceSource(next), keyOf(field));
    return out.applied;
  };
  const number = (field: string, text: string): number | null => {
    const parsed = parseStrictDecimal(text);
    if (!parsed.ok || typeof parsed.value !== 'number') {
      setLocalIssue(field, notANumber(text, id, field));
      return null;
    }
    setLocalIssue(field, null);
    return parsed.value;
  };

  const commitGlobal = async (text: string) => {
    const v = number('power.global_db', text);
    if (v === null) return false;
    return replace('power', (s) => (Object.is(s.power.global_db, v) ? null : { ...s, power: { ...s.power, global_db: v } }));
  };
  const commitBand = (band: number) => async (text: string) => {
    const v = number(`power.band.${band}`, text);
    if (v === null) return false;
    return replace('power', (s) => {
      const power = withBandLevel(s.power, freqs, band, v);
      return power ? { ...s, power } : null;
    });
  };
  const chooseSpectrum = (value: string) => {
    const shape = shapeFor(value, library);
    if (!shape) return;
    actions.fire(replace('power', (s) => ({ ...s, power: { ...s.power, shape } })));
  };
  const chooseDirectivity = (kind: string) => {
    actions.fire(
      replace('directivity', (s) => {
        const d = withDirectivity(s.directivity, kind);
        return d && JSON.stringify(d) !== JSON.stringify(s.directivity) ? { ...s, directivity: d } : null;
      }),
    );
  };
  const commitDirection = (axis: number) => async (text: string) => {
    const field = `direction.${AXES[axis]}`;
    const v = number(field, text);
    if (v === null) return false;
    return replace('directivity', (s) => {
      if (s.directivity.kind !== 'unidirectional') return null;
      const direction = [...s.directivity.direction] as typeof s.directivity.direction;
      if (Object.is(direction[axis], v)) return null;
      direction[axis] = v;
      return { ...s, directivity: { ...s.directivity, direction } };
    });
  };

  const levels = bandLevels(source.power, freqs);
  const key = spectrumKey(source.power.shape, library);
  const options = spectrumOptions(library, key);
  const powerRefused = refusals.get(keyOf('power')) ?? [];
  const dirRefused = refusals.get(keyOf('directivity')) ?? [];
  const powerIssues = issuesForField(scene.issues, 'source', id, 'power');
  const dirIssues = issuesForField(scene.issues, 'source', id, 'directivity');
  const localPower = Object.entries(local)
    .filter(([k]) => k.startsWith('power.'))
    .map(([, v]) => v);
  const localDir = Object.entries(local)
    .filter(([k]) => k.startsWith('direction.'))
    .map(([, v]) => v);
  const d = source.directivity;

  return (
    <div className="props-section emission" data-input data-part="emission">
      <div className="label section-label">Emission</div>
      <label className="field-row">
        <span className="label">Sound power</span>
        <span className="axis-field">
          <CommitInput
            field="power.global_db"
            label="Sound power level, dB re 1 pW, over every band"
            className="mono"
            value={exact(source.power.global_db)}
            invalid={powerRefused.length > 0 || !!local['power.global_db']}
            commit={commitGlobal}
            onRevert={() => setLocalIssue('power.global_db', null)}
          />
          <span className="unit">dB</span>
        </span>
      </label>
      <label className="field-row">
        <span className="label">Spectrum</span>
        <select
          data-field="spectrum"
          aria-label="Spectrum"
          value={options.some((o) => o.value === key) ? key : 'custom'}
          onChange={(e) => chooseSpectrum(e.target.value)}
        >
          {options.map((o) => (
            <option key={o.value} value={o.value} disabled={o.value === 'custom'}>
              {o.label}
            </option>
          ))}
        </select>
      </label>
      {levels && (
        <div className="band-levels" data-part="band-levels" title="Each band's sound power level, dB re 1 pW. Typing one keeps the others and makes the spectrum 'Typed per band'.">
          {levels.map((l, i) => (
            <label key={freqs[i]} className="band-level">
              <span className="k">{bandLabel(freqs[i])}</span>
              <CommitInput
                field={`power.band.${i}`}
                label={`Sound power level at ${freqs[i]} Hz, dB`}
                className="mono"
                value={String(Math.round(l * 10) / 10)}
                invalid={!!local[`power.band.${i}`]}
                commit={commitBand(i)}
                onRevert={() => setLocalIssue(`power.band.${i}`, null)}
              />
            </label>
          ))}
        </div>
      )}
      <Issues refused={[...localPower, ...powerRefused]} current={powerIssues} />

      <label className="field-row">
        <span className="label">Directivity</span>
        <select data-field="directivity" aria-label="Directivity" value={d.kind} onChange={(e) => chooseDirectivity(e.target.value)}>
          {DIRECTIVITIES.map((o) => (
            <option key={o.kind} value={o.kind} title={o.title}>
              {o.label}
            </option>
          ))}
          {d.kind === 'balloon' && (
            <option value="balloon" disabled>
              Measured balloon ({d.file})
            </option>
          )}
        </select>
      </label>
      {d.kind === 'unidirectional' && (
        <div className="fact-grid">
          {AXES.map((axis, i) => (
            <label key={axis} className={`fact-cell axis-${axis}`}>
              <span className="k">{axis.toUpperCase()}</span>
              <CommitInput
                field={`direction.${axis}`}
                label={`Direction ${axis.toUpperCase()}`}
                className="mono"
                value={exact(d.direction[i])}
                invalid={dirRefused.length > 0 || !!local[`direction.${axis}`]}
                commit={commitDirection(i)}
                onRevert={() => setLocalIssue(`direction.${axis}`, null)}
              />
            </label>
          ))}
        </div>
      )}
      <Issues refused={[...localDir, ...dirRefused]} current={dirIssues} />
    </div>
  );
}
