// Faces the 3D view leaves out, as a view state, never an edit of the model (Burhan's 10-05 UI list):
//   - Item 7, Roof off: the faces that close the room from above, so the inside of a closed room can be
//     seen from above (and from outside, View style > Faces > Outside). A roof face is one whose inside
//     faces down (the outward normal, faces pointing out of the room, points up within ROOF_MAX_DEG of
//     vertical) with nothing of the model above it: a ray straight up from its centroid escapes. Peeled
//     in up to ROOF_PASSES passes, so a ceiling under a roof goes too once the roof is off, while a
//     balcony's underside, which its own floor covers, stays. No height rule: a lower annex's roof is a
//     roof as well.
//   - Item 8, Isolate: only the picked faces or surface groups drawn, the rest left out, until shown again.
// Picks follow what is drawn (engine.ts): a left-out face neither hides nor takes a click. A pure
// module: no three.js, no DOM, tested by hide.test.ts under `node --test`.

/** How far from vertical a roof face's outward normal may lean, degrees: 78, so a pitched roof counts, a wall does not. */
export const ROOF_MAX_DEG = 78;
export const ROOF_MIN_UP = Math.cos((ROOF_MAX_DEG * Math.PI) / 180);
/** Peeling passes: the roof, then what it covered, at most this many deep. */
export const ROOF_PASSES = 4;

/**
 * The roof's faces, sorted. `upZ(f)` is face f's outward normal's z (unit normal); `coveredAbove(f, out)`
 * says whether a ray straight up from face f's centroid meets a face of the model not in `out` (1 = left
 * out) before it escapes.
 */
export function roofFaces(faceCount: number, upZ: (f: number) => number, coveredAbove: (f: number, out: Uint8Array) => boolean): number[] {
  const out = new Uint8Array(faceCount);
  const candidates: number[] = [];
  for (let f = 0; f < faceCount; f++) if (upZ(f) >= ROOF_MIN_UP) candidates.push(f);
  for (let pass = 0; pass < ROOF_PASSES; pass++) {
    const peeled = candidates.filter((f) => !out[f] && !coveredAbove(f, out));
    if (peeled.length === 0) break;
    for (const f of peeled) out[f] = 1;
  }
  const faces: number[] = [];
  for (let f = 0; f < faceCount; f++) if (out[f]) faces.push(f);
  return faces;
}

/**
 * The faces left out (1) of `faceCount`: those in `off` (the roof), and with `keep` (isolate) every face
 * not in it; null when none is left out, which is how the view draws everything.
 */
export function leftOut(faceCount: number, off: readonly number[] | null, keep: readonly number[] | null): Uint8Array | null {
  if ((!off || off.length === 0) && !keep) return null;
  const out = new Uint8Array(faceCount);
  if (keep) {
    out.fill(1);
    for (const f of keep) if (f >= 0 && f < faceCount) out[f] = 0;
  }
  if (off) for (const f of off) if (f >= 0 && f < faceCount) out[f] = 1;
  return out;
}

/** The triangle index of the faces with `out[f] === want` (0: drawn, 1: left out), over `indices` (three a face). */
export function indexOf(indices: Uint32Array, out: Uint8Array, want: 0 | 1): Uint32Array {
  let n = 0;
  for (let f = 0; f < out.length; f++) if (out[f] === want) n++;
  const idx = new Uint32Array(3 * n);
  let k = 0;
  for (let f = 0; f < out.length; f++) {
    if (out[f] !== want) continue;
    idx[k++] = indices[3 * f];
    idx[k++] = indices[3 * f + 1];
    idx[k++] = indices[3 * f + 2];
  }
  return idx;
}

/** Per vertex of a non-indexed copy (three vertices a face, in face order): 1 drawn, 0 left out. */
export function shownPerVertex(faceCount: number, out: Uint8Array | null): Float32Array {
  const v = new Float32Array(3 * faceCount).fill(1);
  if (out) for (let f = 0; f < faceCount; f++) if (out[f]) v.fill(0, 3 * f, 3 * f + 3);
  return v;
}

/** The surface groups' names `faces` fall in (face f in group `groupOf[f]`), in the scene's order, each once. */
export function groupNamesOf(faces: readonly number[], groupOf: ArrayLike<number>, names: readonly string[]): string[] {
  const seen = new Set<number>();
  for (const f of faces) seen.add(groupOf[f]);
  return names.filter((_, i) => seen.has(i));
}

/** Why Isolate cannot start with this selection, or null when it can: it isolates faces or surface groups. */
export function isolateRefusal(kind: string, faces: number): string | null {
  if (kind === 'source' || kind === 'receiver') return 'Isolate shows surfaces: pick faces or a surface group, not a source or receiver';
  if (faces === 0) return 'Pick faces in the view, or a surface group in the scene list, to isolate them';
  return null;
}

/** "Ceiling, Roof" or "Ceiling, Roof and 3 more": the chip's list of groups. */
export function groupList(names: readonly string[], most = 3): string {
  if (names.length <= most) return names.join(', ');
  return `${names.slice(0, most).join(', ')} and ${names.length - most} more`;
}
