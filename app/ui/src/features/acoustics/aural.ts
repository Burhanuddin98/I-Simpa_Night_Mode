// The listening window's model (C5, decision 75; docs/investigations/2026-10-07-auralization/):
// the words it shows, the shipped clips (dry, not anechoic), and the WAV the core returns read back into
// samples for Web Audio. A pure module, tested by aural.test.ts under `node --test`.
//
// What is heard, honestly: the impulse response is synthesised from the SPPS energy echogram
// (`simpa_core::auralize`): the room's decay and spectrum per band with a random fine structure.
// It is not a measured or wave-based impulse response, and the window says so (the response
// window's wording is the precedent, response.ts).

/** The window's title and its one line on what is heard. */
export const AURAL_TITLE = 'Listen: impulse response (synthesised)';
export const AURAL_NOTE =
  'Synthesised from the SPPS energy echogram: the room’s decay and spectrum per band, with a random fine structure. Not a measured or wave-based impulse response.';

/** A shipped recording, dry and close-miked (not anechoic): `id` as `src-tauri/src/aural.rs` names it (aural.test.ts holds the
 * two lists to each other and to the files and their provenance). */
export interface Clip {
  id: string;
  title: string;
  /** Its licence, in a few words (the provenance file beside it quotes the licence page). */
  licence: string;
}

/** What the bundled clips are, said where they are offered. */
export const CLIPS_NOTE = 'The bundled clips are dry recordings, close-miked: not anechoic, so the room they were recorded in adds a little. Your own file is best dry or anechoic.';

export const CLIPS: readonly Clip[] = [
  { id: 'speech-lv-hislastbow', title: 'Speech (English, male reader)', licence: 'public domain, LibriVox' },
  { id: 'tenorsax-vcsl-c3', title: 'Tenor saxophone, one note', licence: 'CC0, VCSL' },
  { id: 'harp-vcsl-c5', title: 'Concert harp, one note', licence: 'CC0, VCSL' },
];

/** A WAV as the core writes it (`simpa_core::auralize::wav`): mono IEEE float 32-bit, or 16/24-bit
 * PCM, read back here for playback. */
export interface WavData {
  rate: number;
  channels: number;
  bits: number;
  float: boolean;
  /** The first channel's samples. */
  samples: Float32Array<ArrayBuffer>;
  /** `INFO/ICMT`, when present. */
  comment: string | null;
}

/** Reads a RIFF/WAVE file's format, data and comment; throws on anything else. */
export function readWav(buf: ArrayBuffer): WavData {
  const v = new DataView(buf);
  const tag = (o: number) => String.fromCharCode(v.getUint8(o), v.getUint8(o + 1), v.getUint8(o + 2), v.getUint8(o + 3));
  if (buf.byteLength < 12 || tag(0) !== 'RIFF' || tag(8) !== 'WAVE') throw new Error('not a RIFF/WAVE file');
  let o = 12;
  let fmt: { format: number; channels: number; rate: number; bits: number } | null = null;
  let data: { start: number; len: number } | null = null;
  let comment: string | null = null;
  while (o + 8 <= buf.byteLength) {
    const id = tag(o);
    const len = v.getUint32(o + 4, true);
    const start = o + 8;
    const end = Math.min(start + len, buf.byteLength);
    if (id === 'fmt ') {
      let format = v.getUint16(start, true);
      if (format === 0xfffe) format = v.getUint16(start + 24, true);
      fmt = { format, channels: v.getUint16(start + 2, true), rate: v.getUint32(start + 4, true), bits: v.getUint16(start + 14, true) };
    } else if (id === 'data') {
      data = { start, len: end - start };
    } else if (id === 'LIST' && end - start >= 4 && tag(start) === 'INFO') {
      let p = start + 4;
      while (p + 8 <= end) {
        const l = v.getUint32(p + 4, true);
        if (tag(p) === 'ICMT') comment = new TextDecoder().decode(new Uint8Array(buf, p + 8, Math.min(l, end - p - 8))).replace(/\0+$/, '');
        p += 8 + l + (l % 2);
      }
    }
    o = start + len + (len % 2);
  }
  if (!fmt || !data) throw new Error('no fmt or no data chunk');
  const float = fmt.format === 3;
  if (!((float && fmt.bits === 32) || (fmt.format === 1 && (fmt.bits === 16 || fmt.bits === 24)))) throw new Error(`format ${fmt.format} at ${fmt.bits} bits`);
  const width = fmt.bits / 8;
  const frames = Math.floor(data.len / (width * fmt.channels));
  const samples = new Float32Array(frames);
  for (let i = 0; i < frames; i++) {
    const at = data.start + i * width * fmt.channels;
    samples[i] = float ? v.getFloat32(at, true) : fmt.bits === 16 ? v.getInt16(at, true) / 32768 : ((v.getUint8(at) | (v.getUint8(at + 1) << 8) | (v.getInt8(at + 2) << 16)) as number) / 8388608;
  }
  return { rate: fmt.rate, channels: fmt.channels, bits: fmt.bits, float, samples, comment };
}

/** The largest magnitude of `x`, dBFS (−Infinity for silence). */
export function peakDbfs(x: ArrayLike<number>): number {
  let m = 0;
  for (let i = 0; i < x.length; i++) m = Math.max(m, Math.abs(x[i]));
  return m > 0 ? 20 * Math.log10(m) : -Infinity;
}

/** The meter's reading of a block of samples: its peak in dBFS, clamped to [−60, 0] for the bar's
 * width (0 to 1), and whether it reached full scale (a clip, which the window shows, never hides). */
export function meter(block: ArrayLike<number>): { db: number; fill: number; clip: boolean } {
  const db = peakDbfs(block);
  const shown = Math.max(-60, Math.min(0, db));
  return { db, fill: (shown + 60) / 60, clip: db >= 0 };
}

/** A level as the meter prints it: `−12.3 dBFS`, or `silent`. */
export function dbText(db: number): string {
  return db === -Infinity ? 'silent' : `${db.toFixed(1).replace('-', '−')} dBFS`;
}

/** The name a saved WAV is offered under. */
export function wavName(project: string | null, run: number | null, receiver: string, source: string | null, what: 'ir' | 'aural', clip: string | null): string {
  const parts = [project ?? 'Untitled', run !== null ? `run ${run}` : null, receiver, source ?? 'sources summed', what === 'ir' ? 'impulse response' : `auralization${clip ? ` ${clip}` : ''}`];
  return `${parts.filter((p) => p !== null).join(' - ').replace(/[\\/:*?"<>|]/g, '_')}.wav`;
}
