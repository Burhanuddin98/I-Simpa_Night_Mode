// C5, auralization (docs/investigations/2026-10-07-auralization/SPEC.md, done-when 5): the
// listening window on BRAS CR4's results. The shipped CR4 example, copied into this spec's work
// folder with its plane and maps off (the echograms are the same; the maps are not listened to),
// run on the GPU from the Simulate step's Run button. Each id with its control:
//   c5-words   the Decay card's "Listen" opens a floating window titled as a synthesised impulse
//              response, saying it is synthesised from the SPPS energy echogram and not a measured
//              or wave-based impulse response; its button closes it. Control: no window before.
//   c5-ir      Play synthesises the receiver's response: 48 kHz, as many samples as the run's
//              duration at 48 kHz, its comment naming the seed. Control: another receiver gives
//              another response of the same length.
//   c5-play    Play starts (the button reads Stop, the meter reads a level, no clip) and Stop
//              stops (the button reads Play, nothing plays).
//   c5-save    Save writes a WAV whose header says RIFF/WAVE, IEEE float, mono, 48 kHz, and whose
//              data is the run's duration at 48 kHz, read here from the file itself.
//   c5-aural   Auralize with the first bundled clip produces audio longer than the clip, which plays
//              and saves at 48 kHz; a 44.1 kHz WAV of this spec's own ("Open your own WAV…")
//              does too.
//
// Its files, under <M11_WORK>\m13-aural (C:). The repository is only read.
import { strict as assert } from 'node:assert';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { clickSelector } from '../lib/dom.ts';
import { hook, m10, waitForHooks } from '../lib/hooks.ts';
import { env } from '../lib/types.ts';

const WORK = () => path.join(env('M11_WORK'), 'm13-aural');
const WIN = '[data-aural-window]';
const PANEL = '[data-dock-panel="acoustics"]';
const RATE = 48000;

interface Row {
  run: string;
  status: string;
  gpu_device?: string | null;
}
interface AuralView {
  receiver: string;
  source: string | null;
  dry: string;
  ir: { key: string; rate: number; samples: number; seconds: number; comment: string | null } | null;
  aural: { key: string; rate: number; samples: number; seconds: number } | null;
  playing: 'ir' | 'aural' | null;
  meterPeak: number;
  clipped: boolean;
  busy: string | null;
  error: string | null;
  saved: { what: string; path: string; bytes: number }[];
}

/** A WAV file's header and data size, read here from its bytes. */
function wavHeader(file: string) {
  const b = readFileSync(file);
  assert.equal(b.toString('latin1', 0, 4), 'RIFF');
  assert.equal(b.toString('latin1', 8, 12), 'WAVE');
  let o = 12;
  let fmt: { format: number; channels: number; rate: number; bits: number } | null = null;
  let data = -1;
  while (o + 8 <= b.length) {
    const id = b.toString('latin1', o, o + 4);
    const len = b.readUInt32LE(o + 4);
    if (id === 'fmt ') fmt = { format: b.readUInt16LE(o + 8), channels: b.readUInt16LE(o + 10), rate: b.readUInt32LE(o + 12), bits: b.readUInt16LE(o + 22) };
    if (id === 'data') data = len;
    o += 8 + len + (len % 2);
  }
  assert.ok(fmt, 'a fmt chunk');
  assert.ok(data >= 0, 'a data chunk');
  return { ...fmt, frames: data / ((fmt.bits / 8) * fmt.channels), bytes: b.length };
}

/** A mono 16-bit WAV at 44.1 kHz: a 0.6 s phrase of two tones with gaps. */
function ownWav(file: string) {
  const n = Math.round(0.6 * 44100);
  const data = Buffer.alloc(n * 2);
  for (let i = 0; i < n; i++) {
    const t = i / 44100;
    const on = t < 0.15 || (t > 0.3 && t < 0.45);
    const f = t < 0.2 ? 440 : 660;
    data.writeInt16LE(on ? Math.round(12000 * Math.sin(2 * Math.PI * f * t)) : 0, i * 2);
  }
  const head = Buffer.alloc(44);
  head.write('RIFF', 0, 'latin1');
  head.writeUInt32LE(36 + data.length, 4);
  head.write('WAVE', 8, 'latin1');
  head.write('fmt ', 12, 'latin1');
  head.writeUInt32LE(16, 16);
  head.writeUInt16LE(1, 20);
  head.writeUInt16LE(1, 22);
  head.writeUInt32LE(44100, 24);
  head.writeUInt32LE(88200, 28);
  head.writeUInt16LE(2, 32);
  head.writeUInt16LE(16, 34);
  head.write('data', 36, 'latin1');
  head.writeUInt32LE(data.length, 40);
  writeFileSync(file, Buffer.concat([head, data]));
}

const view = () => hook<AuralView | null>('auralView');

async function waitView(pred: (v: AuralView) => boolean, what: string, timeout = 120_000): Promise<AuralView> {
  let last: AuralView | null = null;
  await browser.waitUntil(
    async () => {
      last = await view();
      return last !== null && pred(last);
    },
    { timeout, interval: 200, timeoutMsg: `${what}: ${JSON.stringify(last)}` },
  );
  return last as unknown as AuralView;
}

describe('C5: listening to CR4: the synthesised impulse response and an anechoic clip through it', () => {
  let project = '';
  let duration = 0;
  let run: Row | undefined;

  before(async () => {
    await waitForHooks(['idle', 'openProject', 'setStep', 'dockTab', 'runsRows', 'selectRun', 'waitRun', 'gpuStatus', 'acousticsView', 'auralView', 'auralSave', 'auralOpen']);
    mkdirSync(WORK(), { recursive: true });
    const src = JSON.parse(readFileSync(path.join(env('M11_REPO'), 'app', 'src-tauri', 'examples', 'bras_cr4.simpa'), 'utf8'));
    src.solvers.spps.sound_maps_per_band = false;
    src.solvers.spps.particles_saved = 0;
    for (const s of src.surface_receivers) s.enabled = false;
    duration = src.solvers.spps.duration_s;
    project = path.join(WORK(), 'CR4-aural.simpa');
    writeFileSync(project, JSON.stringify(src, null, 1));
    await m10.openProject(project);
    const gpu = await hook<{ available: boolean }>('gpuStatus');
    assert.equal(gpu.available, true, `the GPU entry: ${JSON.stringify(gpu)}`);
    await m10.setStep('simulate');
    await clickSelector('[data-solver="spps-gpu"]');
    const before = new Set((await hook<Row[]>('runsRows')).map((r) => r.run));
    await clickSelector('[data-part="run-panel"]');
    const name = await hook<string>('waitRun', null, 'ended', 900_000);
    assert.ok(!before.has(name));
    run = (await hook<Row[]>('runsRows')).find((x) => x.run === name);
    assert.equal(run?.status, 'OK', JSON.stringify(run));
    await hook('selectRun', name);
    await m10.setStep('results');
    await hook('dockTab', 'acoustics');
    await browser.waitUntil(async () => (await hook<{ state: string; run: string } | null>('acousticsView'))?.state === 'ready', { timeout: 60_000, timeoutMsg: 'the Acoustics tab is not ready' });
  });

  it('c5-words: "Listen" opens a floating window that says what it plays; its button closes it', async () => {
    assert.equal(await $(WIN).isExisting(), false, 'a window before the click');
    await clickSelector(`${PANEL} [data-action="open-aural"]`);
    await $(WIN).waitForExist({ timeout: 10_000 });
    const w = await browser.execute((sel: string, panel: string) => {
      const el = document.querySelector<HTMLElement>(sel);
      return {
        inPanel: !!el?.closest(panel),
        title: el?.querySelector('[data-part="aural-title"]')?.textContent ?? '',
        note: el?.querySelector('[data-part="aural-note"]')?.textContent ?? '',
        text: el?.innerText ?? '',
      };
    }, WIN, PANEL);
    assert.equal(w.inPanel, false);
    assert.match(w.title, /impulse response \(synthesised\)/i);
    assert.match(w.note, /synthesised from the SPPS energy echogram/i);
    assert.match(w.note, /not a measured or wave-based impulse response/i);
    assert.match(w.text, /Auralize/);
    assert.match(w.text, /Open your own WAV/);
    await clickSelector(`${WIN} [data-action="close-aural"]`);
    await browser.waitUntil(async () => !(await $(WIN).isExisting()), { timeout: 10_000 });
    await clickSelector(`${PANEL} [data-action="open-aural"]`);
    await $(WIN).waitForExist({ timeout: 10_000 });
    console.log(`c5-words receipt: run ${run?.run} on ${run?.gpu_device}; "${w.title}"; "${w.note}"`);
  });

  let first: AuralView['ir'] = null;
  it('c5-ir: Play synthesises the response at 48 kHz, the run\'s duration long, seeded', async () => {
    const t0 = Date.now();
    await clickSelector(`${WIN} [data-action="aural-play-ir"]`);
    const v = await waitView((x) => x.ir !== null && x.busy === null, 'the response');
    const ms = Date.now() - t0;
    assert.equal(v.error, null);
    assert.equal(v.ir?.rate, RATE);
    assert.equal(v.ir?.samples, Math.round(duration * RATE), 'the run\'s duration at 48 kHz');
    assert.match(v.ir?.comment ?? '', /seed 0x4e4d415552414c31/);
    assert.match(v.ir?.comment ?? '', /not a measured or wave-based impulse response/);
    first = v.ir;
    console.log(`c5-ir receipt: ${v.receiver}, ${v.source ?? 'summed'}: ${v.ir?.samples} samples at ${v.ir?.rate} Hz (${v.ir?.seconds} s) in ${ms} ms`);
  });

  it('c5-play: Play starts with a level on the meter and no clip; Stop stops', async () => {
    const v = await waitView((x) => x.playing === 'ir' && x.meterPeak > -60, 'a level while playing', 30_000);
    const label = await $(`${WIN} [data-action="aural-play-ir"]`).getText();
    assert.equal(label, 'Stop');
    assert.equal(v.clipped, false, 'no clip');
    assert.ok(v.meterPeak <= 0, `peak ${v.meterPeak} dBFS`);
    const meterText = await $(`${WIN} [data-part="aural-meter"]`).getText();
    assert.match(meterText, /dBFS|silent/);
    await clickSelector(`${WIN} [data-action="aural-play-ir"]`);
    const s = await waitView((x) => x.playing === null, 'stopped', 10_000);
    assert.equal(await $(`${WIN} [data-action="aural-play-ir"]`).getText(), 'Play');
    console.log(`c5-play receipt: played, meter peak ${s.meterPeak.toFixed(1)} dBFS, clipped ${s.clipped}, stopped; meter "${meterText}"`);
  });

  it('c5-save: Save writes a 48 kHz float WAV the run\'s duration long', async () => {
    const file = path.join(WORK(), 'ir.wav');
    const done = await hook<{ path: string; bytes: number }>('auralSave', 'ir', file);
    await m10.idle();
    assert.equal(done.path, file);
    assert.ok(existsSync(file));
    const h = wavHeader(file);
    assert.deepEqual({ format: h.format, channels: h.channels, rate: h.rate, bits: h.bits }, { format: 3, channels: 1, rate: RATE, bits: 32 });
    assert.equal(h.frames, Math.round(duration * RATE));
    assert.equal(h.bytes, done.bytes);
    // Control: another receiver, the same length, other samples.
    await browser.execute(() => {
      const s = document.querySelector<HTMLSelectElement>('[data-aural-window] select[data-part="aural-receiver"]');
      if (!s) throw new Error('no receiver picker');
      s.value = '1';
      s.dispatchEvent(new Event('change', { bubbles: true }));
    });
    await waitView((x) => x.ir === null || x.ir.key !== first?.key, 'the other receiver', 10_000);
    const other = path.join(WORK(), 'ir-other.wav');
    await hook('auralSave', 'ir', other);
    await m10.idle();
    const h2 = wavHeader(other);
    assert.equal(h2.frames, h.frames);
    assert.notEqual(readFileSync(other).toString('base64'), readFileSync(file).toString('base64'), 'another receiver, another response');
    console.log(`c5-save receipt: ${file}: format ${h.format}, ${h.channels} channel, ${h.rate} Hz, ${h.bits} bits, ${h.frames} frames = ${duration} s x ${RATE}; ${h.bytes} bytes; control ${other} differs`);
  });

  it('c5-aural: a bundled clip and an own 44.1 kHz WAV through the room play and save at 48 kHz', async () => {
    const dry = await $(`${WIN} select[data-part="aural-dry"]`).getValue();
    assert.match(dry, /^clip:speech-/, 'the first clip offered is speech');
    await clickSelector(`${WIN} [data-action="aural-play-conv"]`);
    const v = await waitView((x) => x.aural !== null && x.playing === 'aural', 'the auralization playing');
    assert.equal(v.aural?.rate, RATE);
    assert.ok((v.aural?.seconds ?? 0) > 5, `${v.aural?.seconds} s: the clip and the room's tail`);
    await clickSelector(`${WIN} [data-action="aural-play-conv"]`);
    await waitView((x) => x.playing === null, 'stopped', 10_000);
    const file = path.join(WORK(), 'aural-speech.wav');
    await hook('auralSave', 'aural', file);
    await m10.idle();
    const h = wavHeader(file);
    assert.equal(h.rate, RATE);
    assert.equal(h.frames, v.aural?.samples);
    // An own WAV at 44.1 kHz.
    const own = path.join(WORK(), 'own-44k1.wav');
    ownWav(own);
    await hook('auralOpen', own);
    await waitView((x) => x.dry === `file:${own}`, 'the own WAV chosen', 10_000);
    await clickSelector(`${WIN} [data-action="aural-play-conv"]`);
    const o = await waitView((x) => x.aural !== null && x.playing === 'aural', 'the own WAV playing');
    assert.equal(o.aural?.rate, RATE);
    assert.ok((o.aural?.samples ?? 0) >= Math.round(0.6 * RATE), `${o.aural?.samples} samples`);
    await clickSelector(`${WIN} [data-action="aural-play-conv"]`);
    await waitView((x) => x.playing === null, 'stopped', 10_000);
    console.log(`c5-aural receipt: speech clip -> ${v.aural?.samples} samples (${v.aural?.seconds.toFixed(2)} s) at ${v.aural?.rate} Hz, saved ${file} (${h.frames} frames, ${h.rate} Hz); own 44.1 kHz WAV -> ${o.aural?.samples} samples at ${o.aural?.rate} Hz`);
  });
});
