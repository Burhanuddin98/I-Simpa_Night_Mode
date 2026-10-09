// C1 (docs/investigations/2026-10-07-blank-geometry/SPEC.md, order of work 4): the selected
// source's emission, edited in the Sources step. The sound power level overall
// (`[data-field="power.global_db"]`) or per band (`[data-field="power.band.<i>"]`), on the project's
// bands; the spectrum from upstream's reference list or typed per band (`[data-field="spectrum"]`);
// the directivity among the solvers' codes 0 to 4 (`[data-field="directivity"]`), with a direction
// for a unidirectional source (`[data-field="direction.x|y|z"]`). A measured balloon, which only an
// imported project brings, is kept and shown, not offered. Every change is one `replace_source`
// through the checked apply, one undo step; what the validator refuses (a zero direction, say)
// is shown under the field with the validator's words, and the project is unchanged.
//
// Parity M17 and M48: the spectrum list holds the project's own library after upstream's
// reference spectra. "Save to library" makes an entry of the source's band levels and links the
// source to it, in one step; a linked source shows its entry (`[data-part="spectrum-entry"]`):
// the entry's name and levels are edited there (`replace_spectrum`), and every source linked to
// it follows, as upstream's sources follow their user spectrum. Unlink keeps the levels; Delete
// removes an entry no other source uses.
import { useEffect, useState } from 'react';
import * as actions from '../actions';
import type { SceneState, Source, UiIssue, UserSpectrum } from '../bindings/ipc';
import { fieldKey, issuesForField } from '../issues';
import { NOT_A_NUMBER, parseStrictDecimal } from '../numbers';
import { addSpectrum, batch, nextName, removeSpectrum, replaceSource, replaceSpectrum } from '../ops';
import { log, refusalStore, sceneStore, spectrumLibraryStore, useStore } from '../store';
import {
  bandLevels,
  DIRECTIVITIES,
  entryFrom,
  entryUsers,
  entryWithLevel,
  linkedTo,
  powerFor,
  powerKey,
  powerOptions,
  unlinked,
  withBandLevel,
  withDirectivity,
} from './emission';
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
/** The project's spectrum library as it is now. */
const entries = () => sceneStore.get()?.view.spectra ?? [];

/** A refusal made here, in words, filed under `field` of source `id`. */
function said(id: string, field: string, code: string, message: string): UiIssue {
  return { code, rule: '', severity: 'error', path: `source:${id}:${field}`, entity: { kind: 'source', id }, field, message };
}

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
    actions.fire(
      replace('power', (s) => {
        const power = powerFor(value, s.power, library, entries());
        return power ? { ...s, power } : null;
      }),
    );
  };
  /** M17: the source's band levels as a new library entry, the source linked to it: one step. */
  const saveToLibrary = () => {
    const s = now(id);
    if (!s) return;
    const list = entries();
    const made = entryFrom(crypto.randomUUID(), nextName('Spectrum ', list.map((e) => e.name)), s.power, freqs);
    if (!made) return;
    const entry: UserSpectrum = { id: made.id, name: made.name, levels_db: [...made.levels_db] };
    const ops = [addSpectrum(list.length, entry), replaceSource({ ...s, power: linkedTo(s.power, entry) })];
    actions.fire(
      actions.apply(batch(ops), keyOf('power')).then((out) => {
        if (out.applied) log('OK', `Saved the spectrum of ${s.name} to the library as ${entry.name}; ${s.name} is linked to it`);
      }),
    );
  };
  /** M17/M48: the linked entry replaced (its name or a band), every linked source following. */
  const replaceEntry = async (change: (e: UserSpectrum) => UserSpectrum | null): Promise<boolean> => {
    const libId = now(id)?.power.library;
    const e = entries().find((x) => x.id === libId);
    if (!e) return false;
    const next = change(e);
    if (!next) return true;
    const out = await actions.apply(replaceSpectrum(next), keyOf('power'));
    return out.applied;
  };
  const commitEntryName = async (text: string) => {
    const name = text.trim();
    if (!name) {
      setLocalIssue('spectrum.name', said(id, 'spectrum.name', 'NAME_EMPTY', 'A library spectrum needs a name.'));
      return false;
    }
    setLocalIssue('spectrum.name', null);
    return replaceEntry((e) => (e.name === name ? null : { ...e, name }));
  };
  const commitEntryBand = (band: number) => async (text: string) => {
    const v = number(`entry.band.${band}`, text);
    if (v === null) return false;
    return replaceEntry((e) => {
      const next = entryWithLevel(e, band, v);
      return next ? { ...e, levels_db: [...next.levels_db] } : null;
    });
  };
  const unlink = () => actions.fire(replace('power', (s) => (s.power.library ? { ...s, power: unlinked(s.power) } : null)));
  /** M17: the linked entry deleted, this source unlinked first, in one step; refused, in words,
   * while another source or a receiver uses it (the core refuses it too, `in_use`). */
  const deleteEntry = () => {
    const s = now(id);
    const view = sceneStore.get()?.view;
    const e = entries().find((x) => x.id === s?.power.library);
    if (!s || !view || !e) return;
    const others = entryUsers(e.id, view).filter((n) => n !== s.name);
    if (others.length > 0) {
      const them = others.length === 1 ? 'it' : 'them';
      setLocalIssue('spectrum.delete', said(id, 'spectrum.delete', 'SPECTRUM_IN_USE', `${e.name} is used by ${others.join(', ')} too: unlink ${them} first.`));
      return;
    }
    setLocalIssue('spectrum.delete', null);
    actions.fire(
      actions.apply(batch([replaceSource({ ...s, power: unlinked(s.power) }), removeSpectrum(e.id)]), keyOf('power')).then((out) => {
        if (out.applied) log('OK', `Deleted ${e.name} from the library; ${s.name} keeps its levels`);
      }),
    );
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
  const user = scene.view.spectra;
  const key = powerKey(source.power, library);
  const options = powerOptions(library, user, key);
  const entry = source.power.library ? user.find((e) => e.id === source.power.library) : undefined;
  const users = entry ? entryUsers(entry.id, scene.view) : [];
  const powerRefused = refusals.get(keyOf('power')) ?? [];
  const dirRefused = refusals.get(keyOf('directivity')) ?? [];
  const powerIssues = issuesForField(scene.issues, 'source', id, 'power');
  const dirIssues = issuesForField(scene.issues, 'source', id, 'directivity');
  const localPower = Object.entries(local)
    .filter(([k]) => k.startsWith('power.'))
    .map(([, v]) => v);
  const localEntry = Object.entries(local)
    .filter(([k]) => k.startsWith('spectrum.') || k.startsWith('entry.'))
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
          <optgroup label="I-Simpa">
            {options
              .filter((o) => o.group === 'reference')
              .map((o) => (
                <option key={o.value} value={o.value}>
                  {o.label}
                </option>
              ))}
          </optgroup>
          {user.length > 0 && (
            <optgroup label="Project library">
              {options
                .filter((o) => o.group === 'library')
                .map((o) => (
                  <option key={o.value} value={o.value}>
                    {o.label}
                  </option>
                ))}
            </optgroup>
          )}
          {key === 'custom' && (
            <option value="custom" disabled>
              Typed per band
            </option>
          )}
        </select>
      </label>
      {entry ? (
        <div className="spectrum-entry" data-part="spectrum-entry" data-entry-id={entry.id}>
          <label className="field-row">
            <span className="label">Library</span>
            <CommitInput
              field="spectrum.name"
              label="Library spectrum name"
              value={entry.name}
              invalid={!!local['spectrum.name']}
              commit={commitEntryName}
              onRevert={() => setLocalIssue('spectrum.name', null)}
            />
          </label>
          <div className="entry-users" data-part="spectrum-users">
            Linked: {users.join(', ')}. A level changed here changes {users.length === 1 ? 'it' : 'every one'}.
          </div>
          <div
            className="band-levels"
            data-part="entry-levels"
            title="The library spectrum's level in each band, dB. A linked source keeps its own sound power and takes the shape of these levels."
          >
            {entry.levels_db.map((l, i) => (
              <label key={freqs[i] ?? i} className="band-level">
                <span className="k">{bandLabel(freqs[i])}</span>
                <CommitInput
                  field={`entry.band.${i}`}
                  label={`Library spectrum level at ${freqs[i]} Hz, dB`}
                  className="mono"
                  value={String(Math.round(Number(l) * 10) / 10)}
                  invalid={!!local[`entry.band.${i}`]}
                  commit={commitEntryBand(i)}
                  onRevert={() => setLocalIssue(`entry.band.${i}`, null)}
                />
              </label>
            ))}
          </div>
          <div className="entry-actions">
            <button type="button" className="small-button" data-action="spectrum-unlink" onClick={unlink} title="Keep these levels as the source's own, typed per band">
              Unlink
            </button>
            <button type="button" className="small-button" data-action="spectrum-delete" onClick={deleteEntry} title="Delete this spectrum from the project's library">
              Delete from library
            </button>
          </div>
        </div>
      ) : (
        <div className="entry-actions">
          <button
            type="button"
            className="small-button"
            data-action="spectrum-save"
            onClick={saveToLibrary}
            disabled={!levels}
            title="Keep this spectrum in the project's library, for other sources to use; this source stays linked to it"
          >
            Save to library
          </button>
        </div>
      )}
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
      <Issues refused={[...localPower, ...localEntry, ...powerRefused]} current={powerIssues} />

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
