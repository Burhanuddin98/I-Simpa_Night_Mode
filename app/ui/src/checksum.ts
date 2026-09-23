// The IPC benchmark's checksum: the same function as `checksum` in app/src-tauri/src/bench.rs.
// Both are held to the known answers in checksum-kat.json: bench.rs by `cargo test`, this file by
// `npm test` (ui/scripts/checksum-kat.test.mjs runs it under node) and by the app's self-test,
// which runs it inside the webview on the bundled code. No imports, so node can load it as is.

/**
 * The wrapping (mod 2^32) sum of `w * (2k + 1)` over each little-endian u32 word `w` at word index
 * `k`, then of `b * (2k + 1)` over each trailing byte `b`, `k` continuing the count. The odd,
 * position-dependent weights make a reordered or shifted transfer fail it.
 */
export function checksum(buf: ArrayBuffer): number {
  const nWords = buf.byteLength >>> 2;
  const words = new Uint32Array(buf, 0, nWords);
  let sum = 0;
  for (let k = 0; k < nWords; k++) sum = (sum + Math.imul(words[k], 2 * k + 1)) >>> 0;
  const tail = new Uint8Array(buf, nWords * 4);
  for (let j = 0; j < tail.length; j++) sum = (sum + Math.imul(tail[j], 2 * (nWords + j) + 1)) >>> 0;
  return sum;
}

/** One case of checksum-kat.json. */
export interface KnownAnswer {
  name: string;
  hex: string;
  checksum: number;
}

export interface KnownAnswerResult {
  name: string;
  bytes: number;
  want: number;
  got: number;
  ok: boolean;
}

function fromHex(hex: string): ArrayBuffer {
  if (hex.length % 2 !== 0 || /[^0-9a-f]/i.test(hex)) throw new Error(`not a hex byte string: ${hex.slice(0, 16)}`);
  const out = new Uint8Array(hex.length / 2);
  for (let i = 0; i < out.length; i++) out[i] = parseInt(hex.slice(2 * i, 2 * i + 2), 16);
  return out.buffer;
}

/** Runs every case; the caller decides what a failure means. */
export function knownAnswers(cases: readonly KnownAnswer[]): KnownAnswerResult[] {
  return cases.map((c) => {
    const buf = fromHex(c.hex);
    const got = checksum(buf);
    return { name: c.name, bytes: buf.byteLength, want: c.checksum, got, ok: got === c.checksum };
  });
}
