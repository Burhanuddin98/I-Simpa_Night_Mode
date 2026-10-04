// A PNG encoder for the response window's map (M10 PLAN rule 3, "one canvas": the 3D view's is
// the only one, so the map is an image, not a drawing surface). A pure module, tested by
// png.test.ts under `node --test` against Node's own zlib.
//
// What it writes: 8-bit RGBA (colour type 6), no interlace, every row filter 0 (none), the zlib
// stream in stored (uncompressed) deflate blocks. The map is a few thousand pixels, so size is
// not the point; being simple enough to read whole is.

const SIGNATURE = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
/** The most a stored deflate block holds. */
const STORED_MAX = 65535;

let crcTable: Uint32Array | null = null;
/** CRC-32 (ISO 3309, the PNG chunk check) of `bytes`. */
export function crc32(bytes: Uint8Array): number {
  if (!crcTable) {
    crcTable = new Uint32Array(256);
    for (let n = 0; n < 256; n++) {
      let c = n;
      for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
      crcTable[n] = c >>> 0;
    }
  }
  let c = 0xffffffff;
  for (let i = 0; i < bytes.length; i++) c = crcTable[(c ^ bytes[i]) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

/** Adler-32 (RFC 1950, the zlib stream check) of `bytes`. */
export function adler32(bytes: Uint8Array): number {
  let a = 1;
  let b = 0;
  for (let i = 0; i < bytes.length; i++) {
    a = (a + bytes[i]) % 65521;
    b = (b + a) % 65521;
  }
  return ((b << 16) | a) >>> 0;
}

const u32 = (out: number[], v: number) => out.push((v >>> 24) & 0xff, (v >>> 16) & 0xff, (v >>> 8) & 0xff, v & 0xff);

function chunk(out: number[], type: string, data: ArrayLike<number>) {
  const body = new Uint8Array(4 + data.length);
  for (let i = 0; i < 4; i++) body[i] = type.charCodeAt(i);
  body.set(data as ArrayLike<number>, 4);
  u32(out, data.length);
  for (let i = 0; i < body.length; i++) out.push(body[i]);
  u32(out, crc32(body));
}

/** `bytes` as a zlib stream of stored deflate blocks. */
export function zlibStored(bytes: Uint8Array): Uint8Array {
  const blocks = Math.max(1, Math.ceil(bytes.length / STORED_MAX));
  const out = new Uint8Array(2 + blocks * 5 + bytes.length + 4);
  // CMF 0x78 (deflate, 32 K window), FLG 0x01 (no dictionary, fastest; 0x7801 % 31 == 0).
  out[0] = 0x78;
  out[1] = 0x01;
  let o = 2;
  for (let k = 0; k < blocks; k++) {
    const start = k * STORED_MAX;
    const len = Math.min(STORED_MAX, bytes.length - start);
    out[o++] = k === blocks - 1 ? 1 : 0; // BFINAL, BTYPE 00 (stored)
    out[o++] = len & 0xff;
    out[o++] = len >>> 8;
    out[o++] = ~len & 0xff;
    out[o++] = (~len >>> 8) & 0xff;
    out.set(bytes.subarray(start, start + len), o);
    o += len;
  }
  const a = adler32(bytes);
  out[o++] = a >>> 24;
  out[o++] = (a >>> 16) & 0xff;
  out[o++] = (a >>> 8) & 0xff;
  out[o++] = a & 0xff;
  return out;
}

/** `rgba` (row-major, top row first, 4 bytes a pixel) as a PNG file. Refuses a size that is not
 * a whole number above 0, and an array whose length is not `width * height * 4`. */
export function encodePng(rgba: ArrayLike<number>, width: number, height: number): Uint8Array {
  for (const [name, v] of [
    ['width', width],
    ['height', height],
  ] as const) {
    if (!Number.isInteger(v) || v < 1 || v > 0x7fffffff) throw new RangeError(`encodePng: ${name} ${v} is not a whole number above 0`);
  }
  if (rgba.length !== width * height * 4) throw new RangeError(`encodePng: ${rgba.length} bytes for ${width} x ${height} RGBA (${width * height * 4} expected)`);
  const stride = width * 4;
  const raw = new Uint8Array(height * (stride + 1));
  for (let y = 0; y < height; y++) {
    const o = y * (stride + 1);
    raw[o] = 0; // filter: none
    for (let i = 0; i < stride; i++) raw[o + 1 + i] = rgba[y * stride + i];
  }
  const out: number[] = [...SIGNATURE];
  const ihdr: number[] = [];
  u32(ihdr, width);
  u32(ihdr, height);
  ihdr.push(8, 6, 0, 0, 0); // bit depth 8, RGBA, deflate, filter method 0, no interlace
  chunk(out, 'IHDR', ihdr);
  chunk(out, 'IDAT', zlibStored(raw));
  chunk(out, 'IEND', []);
  return Uint8Array.from(out);
}

/** `bytes` in base64 (RFC 4648, with padding). */
export function base64(bytes: Uint8Array): string {
  let s = '';
  for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(s);
}

/** `rgba` as a `data:image/png;base64,` URL; refuses what `encodePng` refuses. */
export const pngDataUrl = (rgba: ArrayLike<number>, width: number, height: number): string => `data:image/png;base64,${base64(encodePng(rgba, width, height))}`;
