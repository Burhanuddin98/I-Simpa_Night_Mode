// The Simulate step's settings editor (docs/investigations/2026-10-03-pq3/PLAN.md, PQ3): the
// chosen solver's settings as fields, each edit one op through the checked apply (marked dirty,
// one undo step), and every refusal shown inline under its field, as the Sources panel shows
// them (`fieldKey`, `refusalStore`).
//
// SPPS: particles per source and band (C7), particles saved for playback with the particle
// file's size (C8), duration (C10), time step in ms with the step count (C11), the receiver
// radius, the particle extinction and "Preserve walls when meshing (-Y)" (backlog 80: every value
// the run-quality advisor's Apply sets has its field), the method (C12),
// sound maps per band (C21), echogram per source (C22), the bands it computes (C25) and the band
// presets (C26), and the air (C27). TCR: its method as drawn, its bands and the air.
//
// Numbers. The values typed are inputs: an <input>'s value is not page text, so no exemption is
// needed for them (m11-h, GATE.md F2), and nothing here is marked `[data-input]`. The readouts
// beside the fields (a step count, a file size) are the UI's own arithmetic on the inputs, never
// a solver's number. A field shows the stored value exactly; a time step is shown and read in
// milliseconds by moving the decimal point in the text (settings.ts).
import { useState } from 'react';
import * as actions from '../../actions';
import type { SceneState, UiIssue } from '../../bindings/ipc';
import type { ComputationMethod, Op } from '../../bindings/schema';
import { CommitInput, Issues } from '../../chrome/SourcesPanel';
import { fieldKey } from '../../issues';
import { parseStrictDecimal } from '../../numbers';
import { setBandComputed, setEnvironment, setSolverSettings } from '../../ops';
import { refusalStore, type SolverName, useStore } from '../../store';
import { EDT_MARKS } from '../acoustics/model';
import { bandsText, hzText, projectSettings, type ProjectSettings, settingsRows } from './model';
import {
  BAND_PRESETS,
  bandPresetOf,
  NOT_A_COUNT,
  parseCount,
  pbinBytes,
  realInputText,
  secondsFromMs,
  sizeText,
  stepCount,
  stepCountText,
  timeStepInputText,
  withAir,
  withMeshing,
  withSpps,
} from './settings';

/** What a field accepts: a value, or why the text is not one. */
type Read = { ok: true; value: number } | { ok: false; code: string };

/** The key a settings field's refusals are filed under: `settings:<group>:<field>`. */
const keyOf = (group: string, field: string) => fieldKey('settings', group, field);

/** The project's issues on these JSON pointers (exactly, or below them). */
function issuesAt(issues: readonly UiIssue[], paths: readonly string[]): UiIssue[] {
  return issues.filter((i) => paths.some((p) => i.path === p || i.path.startsWith(`${p}/`)));
}

function parseIssue(code: string, field: string, text: string): UiIssue {
  const message =
    code === NOT_A_COUNT
      ? `"${text}" is not a whole number from 0 to 2,147,483,647: write digits only, like 150000`
      : `"${text}" is not a number: write digits with a decimal point, like 4.5`;
  return { code, rule: '', severity: 'error', path: `settings:${field}`, entity: null, field, message };
}

/**
 * One edit of the settings, built from the project as it is now (read fresh, so an edit never
 * sends settings older than the last one). `build` returns null when nothing changes: no op, no
 * empty undo step. True when the project took it.
 */
async function edit(key: string, build: (s: ProjectSettings) => Op | null): Promise<boolean> {
  const now = projectSettings(await actions.projectJson());
  if (!now) return false;
  const op = build(now);
  if (!op) return true;
  const out = await actions.apply(op, key);
  return out.applied;
}

/** A numeric field: committed on Enter or blur, refused inline, Esc restores. */
function NumberField(props: {
  group: string;
  field: string;
  label: string;
  unit?: string;
  value: string;
  read: (text: string) => Read;
  /** The op for a value read, or null when it is the value stored. */
  op: (s: ProjectSettings, value: number) => Op | null;
  current: readonly UiIssue[];
  onDraft?: (text: string | null) => void;
}) {
  const { group, field, label, unit, value, read, op, current, onDraft } = props;
  const refusals = useStore(refusalStore);
  const [local, setLocal] = useState<UiIssue | null>(null);
  const [hidden, setHidden] = useState(false);
  const key = keyOf(group, field);
  const refused = hidden ? [] : (refusals.get(key) ?? []);
  const commit = async (text: string) => {
    const r = read(text);
    if (!r.ok) {
      setLocal(parseIssue(r.code, field, text));
      return false;
    }
    setLocal(null);
    const taken = await edit(key, (s) => op(s, r.value));
    setHidden(taken);
    return taken;
  };
  return (
    <div className="sim-field" data-field-row={field}>
      <label className="sim-field-line">
        <span className="k">{label}</span>
        <span className="sim-input-wrap">
          <CommitInput
            field={field}
            label={label}
            value={value}
            invalid={local !== null || refused.length > 0}
            commit={commit}
            onRevert={() => {
              setLocal(null);
              setHidden(true);
            }}
            onDraft={onDraft}
            className="sim-input mono"
          />
          {unit && <span className="sim-unit">{unit}</span>}
        </span>
      </label>
      <Issues refused={refused} current={[...(local ? [local] : []), ...current]} />
    </div>
  );
}

function Toggle(props: { field: string; label: string; checked: boolean; onChange: (on: boolean) => void }) {
  return (
    <label className="sim-toggle">
      <input type="checkbox" data-field={props.field} checked={props.checked} onChange={(e) => props.onChange(e.target.checked)} />
      <span>{props.label}</span>
    </label>
  );
}

const METHODS: readonly { key: ComputationMethod; label: string }[] = [
  { key: 'energetic', label: 'Energetic' },
  { key: 'random', label: 'Random' },
];

/** The bands one solver computes (C25), and the band presets (C26). */
function BandsEditor({ scene, s, solver }: { scene: SceneState; s: ProjectSettings; solver: SolverName }) {
  const refusals = useStore(refusalStore);
  const flags = s.solvers[solver].bands_computed;
  const current = bandPresetOf(s.bands);
  const [pick, setPick] = useState<string>(current?.key ?? '');
  const [confirm, setConfirm] = useState(false);
  const key = keyOf(solver, 'bands');
  const presetKey = keyOf('project', 'bands');
  const chosen = BAND_PRESETS.find((p) => p.key === pick) ?? null;
  const toggle = (band: number, on: boolean) =>
    actions.fire(edit(key, (now) => (now.solvers[solver].bands_computed[band] === on ? null : setBandComputed(solver, band, on))));
  const apply = async () => {
    if (!chosen) return;
    setConfirm(false);
    await actions.reband(chosen.kind, chosen.lowest_hz, chosen.highest_hz, presetKey);
  };
  return (
    <>
      <div className="sim-field-line">
        <span className="k">Bands</span>
        <span className="v mono" data-part="bands-summary">
          {bandsText(s.bands, flags)}
        </span>
      </div>
      <div className="sim-bands" role="group" aria-label={`Bands ${solver === 'tcr' ? 'TCR' : 'SPPS'} computes`}>
        {s.bands.frequencies_hz.map((f, i) => (
          <label key={f} className="sim-band" data-band={i} data-hz={f}>
            <input type="checkbox" checked={flags[i] ?? false} onChange={(e) => toggle(i, e.target.checked)} />
            <span className="mono">{hzText(f)}</span>
          </label>
        ))}
      </div>
      <Issues refused={refusals.get(key) ?? []} current={scene.solver_issues[solver]} />
      <div className="sim-preset">
        <select
          data-field="band-preset"
          aria-label="Band preset"
          value={pick}
          onChange={(e) => {
            setPick(e.target.value);
            setConfirm(false);
          }}
        >
          {!current && <option value="">Custom bands (as they are)</option>}
          {BAND_PRESETS.map((p) => (
            <option key={p.key} value={p.key}>
              {p.label}
            </option>
          ))}
        </select>
        <button
          className="small-button"
          data-part="reband"
          disabled={!chosen || chosen.key === current?.key}
          onClick={() => setConfirm(true)}
        >
          Change bands…
        </button>
      </div>
      {confirm && chosen && (
        <div className="sim-confirm" data-part="reband-confirm" role="alertdialog" aria-label="Change the bands">
          <div>
            {chosen.label}: each new band takes every per-band value (materials, sources, fittings, the bands computed) from the
            nearest current band. One undo restores the current bands.
          </div>
          <div className="sim-confirm-buttons">
            <button className="small-button" data-part="reband-apply" onClick={() => actions.fire(apply())}>
              Change bands
            </button>
            <button className="small-button" data-part="reband-cancel" onClick={() => setConfirm(false)}>
              Cancel
            </button>
          </div>
        </div>
      )}
      <Issues refused={refusals.get(presetKey) ?? []} current={[]} />
    </>
  );
}

type AirKey = 'temperature_c' | 'relative_humidity_percent' | 'pressure_pa';

function airPatch(key: AirKey, v: number) {
  if (key === 'temperature_c') return { temperature_c: v };
  return key === 'pressure_pa' ? { pressure_pa: v } : { relative_humidity_percent: v };
}

/** The air (C27): temperature, humidity and pressure, which both solvers use. */
function AirEditor({ scene, s }: { scene: SceneState; s: ProjectSettings }) {
  const env = s.environment;
  const fields = [
    { field: 'temperature', label: 'Temperature', unit: '°C', key: 'temperature_c', value: env.temperature_c },
    { field: 'humidity', label: 'Relative humidity', unit: '%', key: 'relative_humidity_percent', value: env.relative_humidity_percent },
    { field: 'pressure', label: 'Pressure', unit: 'Pa', key: 'pressure_pa', value: env.pressure_pa },
  ] as const;
  const range = issuesAt(scene.issues, ['/environment']).filter((i) => i.path === '/environment');
  return (
    <>
      {fields.map((f) => (
        <NumberField
          key={f.field}
          group="air"
          field={f.field}
          label={f.label}
          unit={f.unit}
          value={realInputText(f.value)}
          read={parseStrictDecimal}
          op={(now, v) => (Object.is(now.environment[f.key], v) ? null : setEnvironment(withAir(now.environment, airPatch(f.key, v))))}
          current={issuesAt(scene.issues, [`/environment/${f.key}`])}
        />
      ))}
      <Issues refused={[]} current={range} />
    </>
  );
}

export function SettingsEditor({ scene, settings, solver }: { scene: SceneState | null; settings: ProjectSettings | null; solver: SolverName }) {
  const [savedDraft, setSavedDraft] = useState<string | null>(null);
  const [stepDraft, setStepDraft] = useState<string | null>(null);
  const refusals = useStore(refusalStore);

  if (!scene || !settings) {
    return (
      <div className="sim-settings" data-part="settings">
        {settingsRows(null, solver).map((r) => (
          <div key={r.key} className="sim-setting" data-setting={r.key}>
            <span className="k">{r.label}</span>
            <span className="v mono">{r.value}</span>
          </div>
        ))}
      </div>
    );
  }
  const s = settings;
  const issues = scene.issues;

  if (solver === 'tcr') {
    return (
      <div className="sim-settings" data-part="settings">
        <div className="sim-setting" data-setting="method">
          <span className="k">Method</span>
          <span className="v mono">Sabine · Eyring</span>
        </div>
        <div className="sim-setting sim-block" data-setting="bands">
          <BandsEditor scene={scene} s={s} solver="tcr" />
        </div>
        <div className="sim-setting sim-block" data-setting="air">
          <AirEditor scene={scene} s={s} />
        </div>
      </div>
    );
  }

  const spps = s.solvers.spps;
  const sppsOp = (patch: (v: number) => Partial<typeof spps>, same: (now: typeof spps, v: number) => boolean) => (now: ProjectSettings, v: number) =>
    same(now.solvers.spps, v) ? null : setSolverSettings(withSpps(now.solvers, patch(v)));
  // The step count and the particle file's size follow what is typed, before it is committed.
  const typedStep = stepDraft === null ? null : secondsFromMs(stepDraft);
  const step = typedStep?.ok ? typedStep.value : spps.time_step_s;
  const steps = stepCount(spps.duration_s, step);
  const typedSaved = savedDraft === null ? null : parseCount(savedDraft);
  const saved = typedSaved?.ok ? typedSaved.value : spps.particles_saved;
  const sources = scene.view.sources.filter((x) => x.enabled).length;
  const bandsOn = spps.bands_computed.filter(Boolean).length;
  let pbin = 'none saved';
  if (saved > 0) {
    if (steps === null) pbin = '—';
    else {
      const perBand = pbinBytes(saved, steps, sources);
      pbin = `particle file about ${sizeText(perBand)} per band, ${sizeText(perBand * BigInt(bandsOn))} for ${bandsOn} bands`;
    }
  }
  const toggle = (field: 'sound_maps_per_band' | 'echogram_per_source') => (on: boolean) =>
    actions.fire(
      edit(keyOf('spps', field), (now) =>
        now.solvers.spps[field] === on
          ? null
          : setSolverSettings(withSpps(now.solvers, field === 'sound_maps_per_band' ? { sound_maps_per_band: on } : { echogram_per_source: on })),
      ),
    );
  const setPreserve = (on: boolean) =>
    actions.fire(
      edit(keyOf('meshing', 'preserve_boundary'), (now) =>
        now.solvers.meshing.preserve_boundary === on ? null : setSolverSettings(withMeshing(now.solvers, { preserve_boundary: on })),
      ),
    );
  const setMethod = (m: ComputationMethod) =>
    actions.fire(edit(keyOf('spps', 'method'), (now) => (now.solvers.spps.method === m ? null : setSolverSettings(withSpps(now.solvers, { method: m })))));

  return (
    <div className="sim-settings" data-part="settings">
      <div className="sim-setting sim-block" data-setting="particles">
        <NumberField
          group="spps"
          field="particles"
          label="Particles per source and band"
          value={String(spps.particles_per_source)}
          read={parseCount}
          op={sppsOp((v) => ({ particles_per_source: v }), (now, v) => now.particles_per_source === v)}
          current={issuesAt(issues, ['/solvers/spps/particles_per_source'])}
        />
      </div>
      <div className="sim-setting sim-block" data-setting="particles_saved">
        <NumberField
          group="spps"
          field="particles_saved"
          label="Particles saved for playback"
          value={String(spps.particles_saved)}
          read={parseCount}
          op={sppsOp((v) => ({ particles_saved: v }), (now, v) => now.particles_saved === v)}
          current={issuesAt(issues, ['/solvers/spps/particles_saved'])}
          onDraft={setSavedDraft}
        />
        <div className="sim-hint" data-part="pbin-size">
          {pbin}
        </div>
      </div>
      <div className="sim-setting sim-block" data-setting="duration">
        <NumberField
          group="spps"
          field="duration"
          label="Duration"
          unit="s"
          value={realInputText(spps.duration_s)}
          read={parseStrictDecimal}
          op={sppsOp((v) => ({ duration_s: v }), (now, v) => Object.is(now.duration_s, v))}
          current={issuesAt(issues, ['/solvers/spps/duration_s'])}
        />
      </div>
      <div className="sim-setting sim-block" data-setting="time_step">
        <NumberField
          group="spps"
          field="time_step"
          label="Time step"
          unit="ms"
          value={timeStepInputText(spps.time_step_s)}
          read={secondsFromMs}
          op={sppsOp((v) => ({ time_step_s: v }), (now, v) => Object.is(now.time_step_s, v))}
          current={issuesAt(issues, ['/solvers/spps/time_step_s'])}
          onDraft={setStepDraft}
        />
        <div className="sim-hint mono" data-part="steps">
          {stepCountText(spps.duration_s, step)}
        </div>
      </div>
      <div className="sim-setting sim-block" data-setting="receiver_radius">
        <NumberField
          group="spps"
          field="receiver_radius"
          label="Receiver radius"
          unit="m"
          value={realInputText(spps.receiver_radius_m)}
          read={parseStrictDecimal}
          op={sppsOp((v) => ({ receiver_radius_m: v }), (now, v) => Object.is(now.receiver_radius_m, v))}
          current={issuesAt(issues, ['/solvers/spps/receiver_radius_m'])}
        />
      </div>
      <div className="sim-setting sim-block" data-setting="extinction">
        <NumberField
          group="spps"
          field="extinction"
          label="Particle extinction"
          value={realInputText(spps.extinction_exponent)}
          read={parseStrictDecimal}
          op={sppsOp((v) => ({ extinction_exponent: v }), (now, v) => Object.is(now.extinction_exponent, v))}
          current={issuesAt(issues, ['/solvers/spps/extinction_exponent'])}
        />
        <div className="sim-hint">A particle is dropped once its energy falls this many tens of decibels.</div>
      </div>
      <div className="sim-setting sim-block" data-setting="preserve_boundary">
        <Toggle field="preserve_boundary" label="Preserve walls when meshing (-Y)" checked={s.solvers.meshing.preserve_boundary} onChange={setPreserve} />
        <Issues
          refused={refusals.get(keyOf('meshing', 'preserve_boundary')) ?? []}
          current={issuesAt(issues, ['/solvers/meshing/preserve_boundary'])}
        />
      </div>
      <div className="sim-setting sim-block" data-setting="method">
        <div className="sim-field-line">
          <span className="k">Method</span>
          <span className="sim-methods" role="radiogroup" aria-label="Method">
            {METHODS.map((m) => (
              <button
                key={m.key}
                className="sim-method"
                role="radio"
                aria-checked={spps.method === m.key}
                data-method={m.key}
                onClick={() => setMethod(m.key)}
              >
                {m.label}
              </button>
            ))}
          </span>
        </div>
        <div className="sim-hint">Random for drafts, Energetic for final results. {EDT_MARKS[1]}.</div>
        <Issues refused={refusals.get(keyOf('spps', 'method')) ?? []} current={[]} />
      </div>
      <div className="sim-setting sim-block" data-setting="sound_maps">
        <Toggle field="sound_maps_per_band" label="Sound maps per band" checked={spps.sound_maps_per_band} onChange={toggle('sound_maps_per_band')} />
        <Issues refused={refusals.get(keyOf('spps', 'sound_maps_per_band')) ?? []} current={[]} />
      </div>
      <div className="sim-setting sim-block" data-setting="echogram_per_source">
        <Toggle field="echogram_per_source" label="Echogram per source" checked={spps.echogram_per_source} onChange={toggle('echogram_per_source')} />
        <Issues refused={refusals.get(keyOf('spps', 'echogram_per_source')) ?? []} current={[]} />
      </div>
      <div className="sim-setting sim-block" data-setting="bands">
        <BandsEditor scene={scene} s={s} solver="spps" />
      </div>
      <div className="sim-setting sim-block" data-setting="air">
        <AirEditor scene={scene} s={s} />
      </div>
    </div>
  );
}
