// The listening window (C5, decisions 75 and 84; docs/investigations/2026-10-07-auralization/SPEC.md):
// opened from the Acoustics tab's Decay card beside "Open response", floating on the glass as the
// response window does, rendered into <body>, so the tab's own scans never see it.
//
// It leads with sound through the room (decision 84: Burhan, 20:45, "just playing the IR itself
// doesnt help at all"): a clip (aural.ts `CLIPS`: the claps first, then a plucked melody, a drum
// groove, all generated and anechoic, and a recorded speech clip, close-miked, which the window says)
// or the user's own WAV, heard Dry or through the Room, convolved by the core with the chosen
// receiver's response. Dry and Room come from the core on one loudness reference (`LEVEL_NOTE`), and
// the switch changes buffers at the same moment of the clip, so what changes is the room. The
// impulse response itself is below, as the detail. The meter reads the output as it plays and shows
// a clip if one ever reaches full scale, never hides it. The words say what this is (`AURAL_NOTE`),
// as the response window's do for its map. It is moved by its title bar and resized by its corner, on
// the response window's frame (`useFrame`; Burhan 2026-10-10 04:11).
import { useCallback, useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import * as actions from '../../actions';
import { AURAL_NOTE, AURAL_TITLE, CLIPS, CLIPS_NOTE, clipText, dbText, energy, type Hear, LEVEL_NOTE, meter, peakDbfs, readWav, type WavData, wavName } from './aural';
import type { SourceSel } from './model';
import { useFrame } from './ResponseWindow';

type What = 'ir' | Hear;

interface Loaded {
  key: string;
  bytes: ArrayBuffer;
  wav: WavData;
}

interface Info {
  key: string;
  rate: number;
  samples: number;
  seconds: number;
  /** The sum of the samples' squares (Dry and Room carry the same). */
  energy: number;
  peakDbfs: number;
}

/** What the window holds, for the `auralView` test hook; null while it is closed. */
export interface AuralHookView {
  receiver: string;
  source: string | null;
  dry: string;
  /** The Dry / Room switch. */
  hear: Hear;
  ir: (Info & { comment: string | null }) | null;
  /** The clip alone, at the level it shares with its room version. */
  dryClip: (Info & { comment: string | null }) | null;
  /** The clip through the room. */
  room: (Info & { comment: string | null }) | null;
  playing: What | null;
  /** Seconds into what plays (null when nothing does). */
  at: number | null;
  /** The highest meter reading since the window opened, dBFS, and whether any reached full scale. */
  meterPeak: number;
  clipped: boolean;
  busy: What | null;
  error: string | null;
  saved: { what: What; path: string; bytes: number }[];
}
let hookView: AuralHookView | null = null;
export const auralHookView = (): AuralHookView | null => hookView;
/** The e2e's save-as (no native dialog): saves `what` to `path`, as the Save buttons do. */
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
  const [hear, setHear] = useState<Hear>('room');
  const [ir, setIr] = useState<Loaded | null>(null);
  const [dryBuf, setDryBuf] = useState<Loaded | null>(null);
  const [roomBuf, setRoomBuf] = useState<Loaded | null>(null);
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
  /** The context time at which what plays would have started from its beginning. */
  const startedAt = useRef(0);

  const irKey = `${run}|${label}|${source ?? ''}`;
  const keyOf = (what: What) => (what === 'ir' ? irKey : `${irKey}|${dry}|${what}`);
  const dryArg = (d: string) => (d.startsWith('clip:') ? { clip: d.slice(5) } : { path: d.slice(5) });
  const held = (what: What) => (what === 'ir' ? ir : what === 'dry' ? dryBuf : roomBuf);
  const setHeld = (what: What) => (what === 'ir' ? setIr : what === 'dry' ? setDryBuf : setRoomBuf);

  const fetchWav = useCallback(
    async (what: What): Promise<Loaded | null> => {
      const key = keyOf(what);
      const have = held(what);
      if (have?.key === key) return have;
      setBusy(what);
      setError(null);
      try {
        const bytes = await actions.runAuralize(run, label, source, what === 'ir' ? {} : { ...dryArg(dry), room: what === 'room' });
        const loaded = { key, bytes, wav: readWav(bytes) };
        setHeld(what)(loaded);
        return loaded;
      } catch (e) {
        const c = actions.asCmdError(e);
        setError(`${c.code}: ${c.message}`);
        return null;
      } finally {
        setBusy(null);
      }
    },
    [irKey, dry, ir, dryBuf, roomBuf, run, label, source],
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

  /** Plays `loaded` from `offset` seconds, replacing what plays. */
  const start = useCallback(
    async (what: What, loaded: Loaded, offset: number) => {
      stop();
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
      const from = Math.min(Math.max(0, offset), buf.duration);
      startedAt.current = ac.currentTime - from;
      src.start(0, from);
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
    [stop],
  );

  const position = () => (ctx.current && node.current ? ctx.current.currentTime - startedAt.current : null);

  /** The response alone, or the clip as the switch says; Dry and Room are both made first, so the
   * switch is instant while it plays. */
  const play = useCallback(
    async (what: What) => {
      stop();
      if (what === 'ir') {
        const l = await fetchWav('ir');
        if (l) await start('ir', l, 0);
        return;
      }
      const other: Hear = what === 'room' ? 'dry' : 'room';
      const l = await fetchWav(what);
      if (!l || !(await fetchWav(other))) return;
      await start(what, l, 0);
    },
    [fetchWav, start, stop],
  );

  /** The switch: while the clip plays, the other version takes over at the same moment. */
  const switchTo = useCallback(
    async (to: Hear) => {
      setHear(to);
      if (playing !== 'dry' && playing !== 'room') return;
      if (playing === to) return;
      const at = position() ?? 0;
      const l = await fetchWav(to);
      if (l) await start(to, l, at);
    },
    [playing, fetchWav, start],
  );

  const save = useCallback(
    async (what: What, path?: string) => {
      const loaded = await fetchWav(what);
      if (!loaded) return null;
      const clip = dry.startsWith('clip:') ? dry.slice(5) : 'own WAV';
      const target = await actions.exportPath('wav', wavName(project, runNumber, label, source, what, what === 'ir' ? null : clip), path);
      if (target === null) return null;
      const said = what === 'ir' ? `the impulse response of ${label}` : what === 'room' ? `${clip} through the room at ${label}` : `${clip} dry, at the level of its room version at ${label}`;
      const bytes = await actions.exportWrite('wav', target, new Uint8Array(loaded.bytes), said, run);
      setSaved((s) => [...s, { what, path: target, bytes }]);
      return { path: target, bytes };
    },
    [fetchWav, dry, project, runNumber, label, source, run],
  );

  const openOwn = useCallback(async (path?: string) => {
    const p = path ?? (await actions.openWavDialog());
    if (p) setDry(`file:${p}`);
  }, []);

  // A new receiver, source or clip stops what plays; the sound is fetched again when asked.
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

  const now = (what: What) => {
    const l = held(what);
    return l && l.key === keyOf(what) ? l : null;
  };
  const info = (l: Loaded | null) =>
    l ? { key: l.key, rate: l.wav.rate, samples: l.wav.samples.length, seconds: l.wav.samples.length / l.wav.rate, energy: energy(l.wav.samples), peakDbfs: peakDbfs(l.wav.samples), comment: l.wav.comment } : null;
  hookSave = (what, path) => save(what, path);
  hookOpen = (path) => void openOwn(path);
  hookView = {
    receiver: label,
    source,
    dry,
    hear,
    ir: info(now('ir')),
    dryClip: info(now('dry')),
    room: info(now('room')),
    playing,
    at: playing ? position() : null,
    meterPeak: hold.peak,
    clipped: hold.clipped,
    busy,
    error,
    saved,
  };

  const ownName = dry.startsWith('file:') ? dry.slice(5).split(/[\\/]/).pop() : null;
  const describe = (l: Loaded | null, none: string) => (l ? `${(l.wav.samples.length / l.wav.rate).toFixed(2)} s at ${l.wav.rate / 1000} kHz, peak ${dbText(peakDbfs(l.wav.samples))}` : none);
  const clipPlaying = playing === 'dry' || playing === 'room';
  const frame = useFrame();

  return createPortal(
    <div
      ref={frame.ref}
      className={`aw rw glass${frame.pos ? ' rw-moved' : ''}`}
      style={frame.pos ? { left: frame.pos.x, top: frame.pos.y } : undefined}
      role="dialog"
      aria-label={AURAL_TITLE}
      data-aural-window
      data-receiver={receiver}
      data-source={source ?? ''}
    >
      <div className="rw-head" onPointerDown={frame.onHeadDown} onPointerMove={frame.onHeadMove} onPointerUp={frame.onHeadUp} onPointerCancel={frame.onHeadUp}>
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

      <section className="aw-sec aw-lead" data-part="aural-conv">
        <div className="aw-row">
          <select data-part="aural-dry" value={dry.startsWith('clip:') ? dry : 'file'} onChange={(e) => e.target.value !== 'file' && setDry(e.target.value)} title="What to hear through the room">
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
        <div className="aw-row">
          <div className="segmented aw-hear" role="radiogroup" aria-label="Hear" data-part="aural-hear">
            {(['dry', 'room'] as const).map((h) => (
              <button
                key={h}
                type="button"
                role="radio"
                aria-checked={hear === h}
                aria-selected={hear === h}
                data-hear={h}
                data-action={`aural-hear-${h}`}
                title={h === 'dry' ? 'The clip as it is, without the room' : `The clip through the room, at ${label}`}
                onClick={() => void switchTo(h)}
              >
                {h === 'dry' ? 'Dry' : 'Room'}
              </button>
            ))}
          </div>
          <button type="button" className="small-button aw-play" data-action="aural-play-conv" disabled={busy !== null} onClick={() => void (clipPlaying ? stop() : play(hear))}>
            {clipPlaying ? 'Stop' : busy === 'dry' || busy === 'room' ? 'Convolving…' : 'Play'}
          </button>
          <button type="button" className="small-button" data-action="aural-save-conv" disabled={busy !== null} title={hear === 'dry' ? 'Save the clip dry, at the level of its room version' : 'Save the clip through the room'} onClick={() => void save(hear)}>
            Save WAV…
          </button>
          <span className="aw-info" data-part="aural-conv-info">
            {describe(now(hear), 'not convolved yet')}
          </span>
        </div>
        <div className="aw-clips-note" data-part="aural-level-note">
          {LEVEL_NOTE}
        </div>
        <div className="aw-clips-note" data-part="aural-clips-note">
          {CLIPS_NOTE}
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

      <section className="aw-sec" data-part="aural-ir">
        <div className="aw-sec-title">The detail: the impulse response (synthesised)</div>
        <div className="aw-row">
          <button type="button" className="small-button" data-action="aural-play-ir" disabled={busy !== null} onClick={() => void (playing === 'ir' ? stop() : play('ir'))}>
            {playing === 'ir' ? 'Stop' : busy === 'ir' ? 'Synthesising…' : 'Play'}
          </button>
          <button type="button" className="small-button" data-action="aural-save-ir" disabled={busy !== null} onClick={() => void save('ir')}>
            Save WAV…
          </button>
          <span className="aw-info" data-part="aural-ir-info">
            {describe(now('ir'), 'not synthesised yet')}
          </span>
        </div>
      </section>
      {error ? (
        <div className="aw-error" data-part="aural-error">
          {error}
        </div>
      ) : null}
    </div>,
    document.body,
  );
}
