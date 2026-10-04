// png.ts held to Node's own zlib: a PNG it writes is read back here by a decoder that shares no
// code with it (node:zlib inflates the stream and checks its Adler-32; zlib.crc32 checks each
// chunk), and the pixels must come back as they went in. Each rule with a case it must say NO to.
import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { crc32 as zcrc32, inflateSync } from 'node:zlib';
import { adler32, base64, crc32, encodePng, pngDataUrl } from './png.ts';

/** A PNG read back: its chunks' CRCs checked, IHDR read, IDAT inflated, filter-0 rows unpacked. */
function decode(png: Uint8Array): { width: number; height: number; rgba: Uint8Array; chunks: string[] } {
  const sig = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
  assert.deepEqual([...png.subarray(0, 8)], sig, 'the PNG signature');
  const view = new DataView(png.buffer, png.byteOffset, png.byteLength);
  let o = 8;
  let width = 0;
  let height = 0;
  const idat: Uint8Array[] = [];
  const chunks: string[] = [];
  while (o < png.length) {
    const len = view.getUint32(o);
    const type = String.fromCharCode(...png.subarray(o + 4, o + 8));
    const data = png.subarray(o + 8, o + 8 + len);
    const crc = view.getUint32(o + 8 + len);
    assert.equal(crc, zcrc32(png.subarray(o + 4, o + 8 + len)), `${type}'s CRC`);
    chunks.push(type);
    if (type === 'IHDR') {
      width = view.getUint32(o + 8);
      height = view.getUint32(o + 12);
      assert.deepEqual([...data.subarray(8)], [8, 6, 0, 0, 0], 'IHDR: 8-bit RGBA, no interlace');
    }
    if (type === 'IDAT') idat.push(data);
    o += 12 + len;
  }
  const raw = inflateSync(Buffer.concat(idat));
  const stride = width * 4;
  assert.equal(raw.length, height * (stride + 1), 'one filter byte and a row per line');
  const rgba = new Uint8Array(width * height * 4);
  for (let y = 0; y < height; y++) {
    assert.equal(raw[y * (stride + 1)], 0, `row ${y}'s filter`);
    rgba.set(raw.subarray(y * (stride + 1) + 1, (y + 1) * (stride + 1)), y * stride);
  }
  return { width, height, rgba, chunks };
}

const pattern = (w: number, h: number) => {
  const px = new Uint8Array(w * h * 4);
  for (let i = 0; i < px.length; i++) px[i] = (i * 37 + (i >> 3)) & 0xff;
  return px;
};

test('the checks equal zlib\'s CRC-32 and the RFC 1950 Adler-32 known answer', () => {
  const bytes = new TextEncoder().encode('Wikipedia');
  assert.equal(adler32(bytes), 0x11e60398, 'Adler-32 of "Wikipedia"');
  assert.equal(crc32(bytes), zcrc32(bytes));
  assert.equal(crc32(new TextEncoder().encode('123456789')), 0xcbf43926, 'CRC-32 check value');
  // Say NO: a changed byte changes both.
  const other = new TextEncoder().encode('Wikipedib');
  assert.notEqual(adler32(other), adler32(bytes));
  assert.notEqual(crc32(other), crc32(bytes));
});

test('a known 3 x 2 image round-trips through Node\'s zlib pixel for pixel', () => {
  // prettier-ignore
  const px = Uint8Array.from([
    255, 255, 255, 255,   224, 32, 46, 255,   58, 11, 16, 255,
    0, 0, 0, 255,         1, 2, 3, 4,         250, 128, 7, 0,
  ]);
  const back = decode(encodePng(px, 3, 2));
  assert.equal(back.width, 3);
  assert.equal(back.height, 2);
  assert.deepEqual(back.chunks, ['IHDR', 'IDAT', 'IEND']);
  assert.deepEqual([...back.rgba], [...px]);
  // Control: the decoder sees a change: the image with one byte flipped does not decode equal.
  const flipped = px.slice();
  flipped[5] ^= 1;
  assert.notDeepEqual([...decode(encodePng(flipped, 3, 2)).rgba], [...px]);
});

test('an image larger than one stored block (65535 bytes) round-trips', () => {
  const w = 200;
  const h = 100; // 100 rows of 801 bytes: 80100 bytes, two stored blocks
  const px = pattern(w, h);
  const back = decode(encodePng(px, w, h));
  assert.equal(back.width, w);
  assert.equal(back.height, h);
  assert.deepEqual(Buffer.from(back.rgba), Buffer.from(px));
});

test('the data URL is the PNG in base64', () => {
  const px = pattern(4, 5);
  const url = pngDataUrl(px, 4, 5);
  assert.ok(url.startsWith('data:image/png;base64,'));
  const bytes = Buffer.from(url.slice('data:image/png;base64,'.length), 'base64');
  assert.deepEqual([...decode(new Uint8Array(bytes)).rgba], [...px]);
  assert.equal(base64(Uint8Array.from([0, 1, 2, 3, 255])), Buffer.from([0, 1, 2, 3, 255]).toString('base64'));
});

test('say NO: a length that is not width x height x 4, and a size that is not a whole number above 0', () => {
  assert.throws(() => encodePng(new Uint8Array(23), 3, 2), RangeError, 'one byte short');
  assert.throws(() => encodePng(new Uint8Array(25), 3, 2), RangeError, 'one byte over');
  assert.throws(() => encodePng(new Uint8Array(24), 2, 3 + 0.5), RangeError, 'a fractional height');
  assert.throws(() => encodePng(new Uint8Array(0), 0, 0), RangeError, 'an empty image');
  assert.throws(() => encodePng(new Uint8Array(24), -3, -2), RangeError, 'a negative size');
  assert.throws(() => pngDataUrl(new Uint8Array(23), 3, 2), RangeError, 'the URL refuses what the encoder refuses');
  // Control: the right length is taken.
  assert.doesNotThrow(() => encodePng(new Uint8Array(24), 3, 2));
});
