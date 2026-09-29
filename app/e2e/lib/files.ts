// Byte comparisons for the saved-file gates ((c) and (f)): equal, or the first offset at which
// they differ, with a little context.
import { readFileSync } from 'node:fs';

/** -1 when equal, else the first differing offset (the shorter length when one is a prefix). */
export function firstDifference(a: Uint8Array, b: Uint8Array): number {
  const n = Math.min(a.length, b.length);
  for (let i = 0; i < n; i++) if (a[i] !== b[i]) return i;
  return a.length === b.length ? -1 : n;
}

function around(bytes: Uint8Array, at: number): string {
  return JSON.stringify(Buffer.from(bytes.subarray(Math.max(0, at - 40), at + 40)).toString('utf8'));
}

/** Compares two files byte for byte. `null` when equal, else a description of the difference. */
export function compareFiles(actualPath: string, expectedPath: string): string | null {
  const a = readFileSync(actualPath);
  const e = readFileSync(expectedPath);
  const at = firstDifference(a, e);
  if (at < 0) return null;
  return (
    `${actualPath} (${a.length} B) differs from ${expectedPath} (${e.length} B) at byte ${at}:\n` +
    `  actual   ${around(a, at)}\n  expected ${around(e, at)}`
  );
}
