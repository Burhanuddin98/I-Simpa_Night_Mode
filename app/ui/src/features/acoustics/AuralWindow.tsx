// The listening window (C5, decision 75; docs/investigations/2026-10-07-auralization/SPEC.md):
// opened from the Acoustics tab's Decay card beside "Open response", floating on the glass as the
// response window does, rendered into <body>, so the tab's own scans never see it.
//
// "Impulse response (synthesised)": the chosen receiver's response, per source or the sources
// summed, synthesised by the core from the SPPS energy echogram (`run_auralize`), played through
// Web Audio and saved as a WAV. "Auralize": a bundled clip (aural.ts `CLIPS`: dry, close-miked,
// not anechoic, which the window says) or the user's own WAV convolved with it. Every buffer comes peak-normalised to −1 dBFS by the core; the
// meter reads the output as it plays and shows a clip if one ever reaches full scale, never hides
// it. The words say what this is (`AURAL_NOTE`), as the response window's do for its map.
import { useCallback, useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import * as actions from '../../actions';
import { AURAL_NOTE, AURAL_TITLE, CLIPS, CLIPS_NOTE, clipText, dbText, meter, readWav, type WavData, wavName } from './aural';
import type { SourceSel } from './model';

type What = 'ir' | 'aural';

interface Loaded {
  key: string;
  bytes: ArrayBuffer;
  wav: WavData;
}

/** What the window holds, for the `auralView` test hook; null while it is closed. */
export interface AuralHookView {
  receiver: string;
  source: string | null;
  dry: string;
  ir: { key: string; rate: number; samples: number; seconds: number; comment: string | null } | null;
  aural: { key: string; rate: number; samples: number; seconds: number } | null;
  playing: What | null;
  /** The highest meter reading since the window opened, dBFS, and whether any reached full scale. */
  meterPeak: number;
  clipped: boolean;
  busy: What | null;
  error: string | null;
  saved: { what: What; path: string; bytes: number }[];
}
let hookView: AuralHookView | null = null;
export const auralHookView = (): AuralHookView | null => hookView;
/** The e2e's save-as (no native dialog): saves `what` to `path`, as the Save button does. */
let hookSave: ((what: What, path: string) => Promise<{ path: string; bytes: number } | null>) | null = null;
export const auralHookSave = (what: What, path: string) => (hookSave ? hookSave(what, path) : Promise.reject(new Error('the listening window is closed')));
/** The e2e's "Open your own WAV…" (no native dialog). */
let hookOpen: ((path: string) => void) | null = null;
export const auralHookOpen = (path: string) => {
  if (!hookOpen) throw new Error('the listening window is closed');
  hookOpen(path);
};

export function AuralWindow({
  run,
  runNumber,
  project,
  receiver,
  source,
  receivers,
  sources,
  onReceiver,
  onSource,
  onClose,
}: {
  run: string;
  runNumber: number | null;
  project: string | null;
  receiver: number;
  source: SourceSel;
  receivers: string[];
  sources: string[];
  onReceiver: (r: number) => void;
  onSource: (s: SourceSel) => void;
  onClose: () => void;
}) {
  const label = receivers[receiver] ?? '';
  const [dry, setDry] = useState<string>(`clip:${CLIPS[0]?.id ?? ''}`);
  const [ir, setIr] = useState<Loaded | null>(null);
  const [wet, setWet] = useState<Loaded | null>(null);
  const [busy, setBusy] = useState<What | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [playing, setPlaying] = useState<What | null>(null);
  const [level, setLevel] = useState<{ db: number; fill: number; clip: boolean }>({ db: -Infinity, fill: 0, clip: false });
  const [hold, setHold] = useState<{ peak: number; clipped: boolean }>({ peak: -Infinity, clipped: false });
  const [saved, setSaved] = useState<{ what: What; path: string; bytes: number }[]>([]);
  const ctx = useRef<AudioContext | null>(null);
  const node = useRef<AudioBufferSourceNode | null>(null);
  const analyser = useRef<AnalyserNode | null>(null);
  const raf = useRef<number | null>(null);

  const irKey = `${run}|${label}|${source ?? ''}`;
  const wetKey = `${irKey}|${dry}`;
  const dryArg = (d: string) => (d.startsWith('clip:') ? { clip: d.slice(5) } : { path: d.slice(5) });

  const fetchWav = useCallback(
    async (what: What): Promise<Loaded | null> => {
      const key = what === 'ir' ? irKey : wetKey;
      const have = what === 'ir' ? ir : wet;
      if (have?.key === key) return have;
      setBusy(what);
      setError(null);
      try {
        const bytes = await actions.runAuralize(run, label, source, what === 'ir' ? {} : dryArg(dry));
        const loaded = { key, bytes, wav: readWav(bytes) };
        (what === 'ir' ? setIr : setWet)(loaded);
        return loaded;
      } catch (e) {
        const c = actions.asCmdError(e);
        setError(`${c.code}: ${c.message}`);
        return null;
      } finally {
        setBusy(null);
      }
    },
    [irKey, wetKey, ir, wet, run, label, source, dry],
  );

  const stop = useCallback(() => {
    try {
      node.current?.stop();
    } catch {
      // Already ended.
    }
    node.current = null;
    if (raf.current !== null) cancelAnimationFrame(raf.current);
    raf.current = null;
    setPlaying(null);
    setLevel({ db: -Infinity, fill: 0, clip: false });
  }, []);

  const play = useCallback(
    async (what: What) => {
      stop();
      const loaded = await fetchWav(what);
      if (!loaded) return;
      const ac = ctx.current ?? new AudioContext();
      ctx.current = ac;
      if (ac.state === 'suspended') await ac.resume();
      const buf = ac.createBuffer(1, loaded.wav.samples.length, loaded.wav.rate);
      buf.copyToChannel(loaded.wav.samples, 0);
      const src = ac.createBufferSource();
      src.buffer = buf;
      const an = analyser.current ?? ac.createAnalyser();
      an.fftSize = 2048;
      if (!analyser.current) an.connect(ac.destination);
      analyser.current = an;
      src.connect(an);
      src.onended = () => {
        if (node.current === src) stop();
      };
      node.current = src;
      setPlaying(what);
      src.start();
      const block = new Float32Array(an.fftSize);
      const tick = () => {
        an.getFloatTimeDomainData(block);
        const m = meter(block);
        setLevel(m);
        setHold((h) => ({ peak: Math.max(h.peak, m.db), clipped: h.clipped || m.clip }));
        raf.current = requestAnimationFrame(tick);
      };
      raf.current = requestAnimationFrame(tick);
    },
    [fetchWav, stop],
  );

  const save = useCallback(
    async (what: What, path?: string) => {
      const loaded = await fetchWav(what);
      if (!loaded) return null;
      const clip = dry.startsWith('clip:') ? dry.slice(5) : 'own WAV';
      const target = await actions.exportPath('wav', wavName(project, runNumber, label, source, what, what === 'aural' ? clip : null), path);
      if (target === null) return null;
      const bytes = await actions.exportWrite('wav', target, new Uint8Array(loaded.bytes), what === 'ir' ? `the impulse response of ${label}` : `the auralization at ${label}`, run);
      setSaved((s) => [...s, { what, path: target, bytes }]);
      return { path: target, bytes };
    },
    [fetchWav, dry, project, runNumber, label, source, run],
  );

  const openOwn = useCallback(async (path?: string) => {
    const p = path ?? (await actions.openWavDialog());
    if (p) setDry(`file:${p}`);
  }, []);

  // A new receiver, source or clip stops what plays; the response is fetched again when asked.
  useEffect(() => stop, [irKey, dry, stop]);
  useEffect(
    () => () => {
      stop();
      void ctx.current?.close();
      hookView = null;
      hookSave = null;
      hookOpen = null;
    },
    [stop],
  );
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && !(e.target as HTMLElement | null)?.closest('input, select, textarea')) onClose();
    };
    window.addEventListener('keydown', key);
    return () => window.removeEventListener('keydown', key);
  }, [onClose]);

  hookSave = (what, path) => save(what, path);
  hookOpen = (path) => void openOwn(path);
  const info = (l: Loaded | null) => (l ? { key: l.key, rate: l.wav.rate, samples: l.wav.samples.length, seconds: l.wav.samples.length / l.wav.rate } : null);
  hookView = {
    receiver: label,
    source,
    dry,
    ir: ir && ir.key === irKey ? { ...info(ir)!, comment: ir.wav.comment } : null,
    aural: wet && wet.key === wetKey ? info(wet) : null,
    playing,
    meterPeak: hold.peak,
    clipped: hold.clipped,
    busy,
    error,
    saved,
  };

  const ownName = dry.startsWith('file:') ? dry.slice(5).split(/[\\/]/).pop() : null;
  const irNow = ir && ir.key === irKey ? ir : null;
  const wetNow = wet && wet.key === wetKey ? wet : null;
  const seconds = (l: Loaded | null) => (l ? `${(l.wav.samples.length / l.wav.rate).toFixed(2)} s at ${l.wav.rate / 1000} kHz, peak −1 dBFS` : 'not synthesised yet');

  return createPortal(
    <div className="aw rw glass" role="dialog" aria-label={AURAL_TITLE} data-aural-window data-receiver={receiver} data-source={source ?? ''}>
      <div className="rw-head">
        <span className="rw-title" data-part="aural-title">
          {AURAL_TITLE}
        </span>
        <span className="rw-pick">
          <label>
            Receiver
            <select data-part="aural-receiver" value={receiver} onChange={(e) => onReceiver(Number(e.target.value))}>
              {receivers.map((name, i) => (
                <option key={i} value={i}>
                  {name}
                </option>
              ))}
            </select>
          </label>
          {sources.length ? (
            <label>
              Source
              <select data-part="aural-source" value={source ?? ''} onChange={(e) => onSource(e.target.value === '' ? null : e.target.value)}>
                <option value="">all sources summed</option>
                {sources.map((s) => (
                  <option key={s} value={s}>
                    {s}
                  </option>
                ))}
              </select>
            </label>
          ) : null}
        </span>
        <button type="button" className="rw-close" data-action="close-aural" aria-label="Close" title="Close (Esc)" onClick={onClose}>
          ×
        </button>
      </div>
      <div className="rw-note" data-part="aural-note">
        {AURAL_NOTE}
      </div>

      <section className="aw-sec" data-part="aural-ir">
        <div className="aw-sec-title">Impulse response (synthesised)</div>
        <div className="aw-row">
          <button type="button" className="small-button" data-action="aural-play-ir" disabled={busy !== null} onClick={() => void (playing === 'ir' ? stop() : play('ir'))}>
            {playing === 'ir' ? 'Stop' : busy === 'ir' ? 'Synthesising…' : 'Play'}
          </button>
          <button type="button" className="small-button" data-action="aural-save-ir" disabled={busy !== null} onClick={() => void save('ir')}>
            Save WAV…
          </button>
          <span className="aw-info" data-part="aural-ir-info">
            {seconds(irNow)}
          </span>
        </div>
      </section>

      <section className="aw-sec" data-part="aural-conv">
        <div className="aw-sec-title">Auralize</div>
        <div className="aw-row">
          <select data-part="aural-dry" value={dry.startsWith('clip:') ? dry : 'file'} onChange={(e) => e.target.value !== 'file' && setDry(e.target.value)} title="A dry recording to hear through the room (the bundled clips are close-miked, not anechoic)">
            {CLIPS.map((c) => (
              <option key={c.id} value={`clip:${c.id}`}>
                {clipText(c)}
              </option>
            ))}
            {ownName ? <option value="file">{ownName}</option> : null}
          </select>
          <button type="button" className="small-button" data-action="aural-open-wav" title="A dry or anechoic recording of your own (WAV)" onClick={() => void openOwn()}>
            Open your own WAV…
          </button>
        </div>
        <div className="aw-clips-note" data-part="aural-clips-note">
          {CLIPS_NOTE}
        </div>
        <div className="aw-row">
          <button type="button" className="small-button" data-action="aural-play-conv" disabled={busy !== null} onClick={() => void (playing === 'aural' ? stop() : play('aural'))}>
            {playing === 'aural' ? 'Stop' : busy === 'aural' ? 'Convolving…' : 'Play'}
          </button>
          <button type="button" className="small-button" data-action="aural-save-conv" disabled={busy !== null} onClick={() => void save('aural')}>
            Save WAV…
          </button>
          <span className="aw-info" data-part="aural-conv-info">
            {seconds(wetNow)}
          </span>
        </div>
      </section>

      <div className="aw-meter" data-part="aural-meter" data-clip={level.clip || hold.clipped ? 'yes' : 'no'}>
        <span className="aw-meter-label">Level</span>
        <span className="aw-meter-bar">
          <span className="aw-meter-fill" style={{ width: `${(level.fill * 100).toFixed(1)}%` }} />
        </span>
        <span className="aw-meter-text">{playing ? dbText(level.db) : 'stopped'}</span>
        {hold.clipped ? <span className="aw-clip">CLIPPED</span> : null}
      </div>
      {error ? (
        <div className="aw-error" data-part="aural-error">
          {error}
        </div>
      ) : null}
    </div>,
    document.body,
  );
}
