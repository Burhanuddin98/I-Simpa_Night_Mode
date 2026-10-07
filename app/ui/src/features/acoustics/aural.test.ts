// node --test suite for the listening window's model (aural.ts): the clips the window offers are
// the ones the core ships (src-tauri/src/aural.rs), each file beside its provenance with its sha256
// and a public-domain or CC0 licence; the WAV reader reads what the core writes and refuses what it
// is not; the meter shows a clip, never hides one.
import { strict as assert } from 'node:assert';
import { createHash } from 'node:crypto';
import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { AURAL_NOTE, CLIPS, CLIPS_NOTE, dbText, meter, peakDbfs, readWav, wavName } from './aural.ts';

const TAURI = path.join(path.dirname(fileURLToPath(import.meta.url)), '..', '..', '..', '..', 'src-tauri');
const SHIPPED = path.join(TAURI, 'examples', 'clips');

const ab = (b: Buffer): ArrayBuffer => b.buffer.slice(b.byteOffset, b.byteOffset + b.byteLength) as ArrayBuffer;

test('aural: the clips offered are aural.rs\'s, in its order, each shipped with its provenance', () => {
  const rs = readFileSync(path.join(TAURI, 'src', 'aural.rs'), 'utf8');
  const rows = [...rs.matchAll(/id: "([^"]+)",\s*bytes: include_bytes!\("\.\.\/examples\/clips\/([^"]+)"\)/g)].map((m) => [m[1], m[2]]);
  assert.deepEqual(
    rows,
    CLIPS.map((c) => [c.id, `${c.id}.wav`]),
  );
  const files = readdirSync(SHIPPED).sort();
  assert.deepEqual(files, CLIPS.flatMap((c) => [`${c.id}.provenance.md`, `${c.id}.wav`]).sort(), 'nothing shipped that is not offered');
  let total = 0;
  for (const c of CLIPS) {
    const wav = readFileSync(path.join(SHIPPED, `${c.id}.wav`));
    total += wav.length;
    const prov = readFileSync(path.join(SHIPPED, `${c.id}.provenance.md`), 'utf8');
    const sha = createHash('sha256').update(wav).digest('hex');
    assert.ok(prov.includes(sha), `${c.id}: the provenance does not hold the file's sha256 ${sha}`);
    const licence = prov.split('\n').find((l) => l.startsWith('- Licence:')) ?? '';
    assert.match(licence, /Public domain|CC0/, `${c.id}: ${licence}`);
    assert.match(prov, /Licence text, quoted from https?:\/\//, `${c.id}: the licence page is quoted`);
    assert.match(c.licence, /public domain|CC0/);
    const w = readWav(ab(wav));
    assert.equal(w.rate, 48000, c.id);
    assert.equal(w.channels, 1, c.id);
  }
  assert.ok(total < 5 * 1024 * 1024, `${total} bytes`);
});

/** A WAV made here, as the core lays one out: RIFF, fmt, (fact), LIST/INFO/ICMT, data. */
function makeWav(samples: number[], opts: { float: boolean; bits: number; comment?: string }): ArrayBuffer {
  const chunks: Buffer[] = [];
  const chunk = (id: string, body: Buffer) => {
    const h = Buffer.alloc(8);
    h.write(id, 0, 'latin1');
    h.writeUInt32LE(body.length, 4);
    chunks.push(h, body, body.length % 2 ? Buffer.alloc(1) : Buffer.alloc(0));
  };
  const fmt = Buffer.alloc(16);
  fmt.writeUInt16LE(opts.float ? 3 : 1, 0);
  fmt.writeUInt16LE(1, 2);
  fmt.writeUInt32LE(48000, 4);
  fmt.writeUInt32LE(48000 * (opts.bits / 8), 8);
  fmt.writeUInt16LE(opts.bits / 8, 12);
  fmt.writeUInt16LE(opts.bits, 14);
  chunk('fmt ', fmt);
  if (opts.comment) {
    const t = Buffer.from(`${opts.comment}\0`, 'utf8');
    const icmt = Buffer.alloc(8);
    icmt.write('ICMT', 0, 'latin1');
    icmt.writeUInt32LE(t.length, 4);
    chunk('LIST', Buffer.concat([Buffer.from('INFO', 'latin1'), icmt, t, t.length % 2 ? Buffer.alloc(1) : Buffer.alloc(0)]));
  }
  const data = Buffer.alloc(samples.length * (opts.bits / 8));
  samples.forEach((s, i) => {
    if (opts.float) data.writeFloatLE(s, i * 4);
    else if (opts.bits === 16) data.writeInt16LE(Math.round(s * 32767), i * 2);
    else data.writeIntLE(Math.round(s * 8388607), i * 3, 3);
  });
  chunk('data', data);
  const body = Buffer.concat(chunks);
  const head = Buffer.alloc(12);
  head.write('RIFF', 0, 'latin1');
  head.writeUInt32LE(body.length + 4, 4);
  head.write('WAVE', 8, 'latin1');
  return ab(Buffer.concat([head, body]));
}

test('aural: the reader reads float and PCM WAVs with their comment and refuses anything else', () => {
  const x = [0, 0.5, -0.25, 0.891];
  const f = readWav(makeWav(x, { float: true, bits: 32, comment: 'seed 0x4e4d' }));
  assert.equal(f.rate, 48000);
  assert.deepEqual([...f.samples], x.map((v) => Math.fround(v)));
  assert.equal(f.comment, 'seed 0x4e4d');
  const p = readWav(makeWav(x, { float: false, bits: 24 }));
  assert.equal(p.comment, null);
  p.samples.forEach((v, i) => assert.ok(Math.abs(v - x[i]) < 2 / 8388608, `${v} vs ${x[i]}`));
  const s = readWav(makeWav([-1, 0.5], { float: false, bits: 16 }));
  assert.ok(Math.abs(s.samples[0] + 32767 / 32768) < 1e-9);
  assert.throws(() => readWav(ab(Buffer.from('RIFF\0\0\0\0AVI junk'))));
  assert.throws(() => readWav(new ArrayBuffer(4)));
  assert.throws(() => readWav(makeWav(x, { float: true, bits: 64 })), /format 3 at 64/);
});

test('aural: the meter shows a clip and silence, and the words say what this is', () => {
  assert.equal(peakDbfs([0, 0]), -Infinity);
  assert.ok(Math.abs(peakDbfs([0.5, -0.891]) + 1.0) < 0.01);
  assert.deepEqual(meter([0, 0]), { db: -Infinity, fill: 0, clip: false });
  assert.equal(meter([1.2]).clip, true, 'a sample past full scale reads as a clip');
  assert.equal(meter([1.2]).fill, 1);
  assert.equal(meter([0.891]).clip, false);
  assert.equal(dbText(-1.04), '−1.0 dBFS');
  assert.equal(dbText(-Infinity), 'silent');
  assert.match(AURAL_NOTE, /synthesised from the SPPS energy echogram/i);
  assert.match(AURAL_NOTE, /not a measured or wave-based impulse response/i);
  assert.match(CLIPS_NOTE, /not anechoic/);
  for (const c of CLIPS) assert.doesNotMatch(`${c.title} ${c.licence}`, /anechoic/i, 'a bundled clip called anechoic');
  assert.equal(wavName('CR4', 2, 'MP1', null, 'ir', null), 'CR4 - run 2 - MP1 - sources summed - impulse response.wav');
  assert.equal(wavName(null, null, 'a/b', 'LS1', 'aural', 'harp'), 'Untitled - a_b - LS1 - auralization harp.wav');
});
