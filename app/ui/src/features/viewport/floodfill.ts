// Double-click flood-fill: every face reachable from a seed face through shared vertex
// positions whose normal is parallel to the seed's. A pure module (PLAN.md 2.4, rule 8), tested
// by floodfill.test.ts under `node --test`.
//
// The rule is upstream's `CObjet3D::GetAllCoplanarFaces` and `AppendCoplanarFace`
// (src/isimpa/3dengine/Core/Objet3D.cpp:1397-1444 at the pinned tag):
//   - the neighbours of a face are the faces touching one of its three vertex *positions*
//     (`IntersectionSeeker::GetSommetFaceNeighboors` looks vertices up by position, so two
//     vertex indices at the same point are one vertex);
//   - a neighbour is taken when |n_src . n_face| > 0.998, always against the *seed's* normal
//     (a slow curve stops once it has turned about 3.6 degrees away from the seed);
//   - the fill is not limited to the seed's surface group.
// Upstream recurses once per accepted face; on a large flat floor that is one stack frame per
// face. This one keeps an explicit stack and a visited byte per face, and gives the same set.

/** Upstream's `coplanarEpsilon`: the fill takes |n_src . n_face| strictly above it. */
export const COPLANAR_EPS = 0.998;

export interface FaceTopology {
  faceCount: number;
  /** Unit outward normal per face (x, y, z), computed in f64; zero for a degenerate face. */
  normals: Float64Array;
  /** Per vertex index, the id of its position (vertex indices at one point share an id). */
  positionId: Uint32Array;
  /** CSR: the faces touching position p are `faces[offsets[p] .. offsets[p + 1])`. */
  offsets: Uint32Array;
  faces: Uint32Array;
  /** The mesh's vertex indices, three per face, as given. */
  indices: Uint32Array;
}

/**
 * Builds the face normals and the position-to-face adjacency of a triangle mesh.
 * `positions` holds x, y, z per vertex; `indices` holds a, b, c per face.
 */
export function buildTopology(positions: Float64Array, indices: Uint32Array): FaceTopology {
  const vertexCount = Math.floor(positions.length / 3);
  const faceCount = Math.floor(indices.length / 3);

  // Exact position identity. String keys are exact for doubles (shortest round-trip form);
  // -0 and 0 print alike, which is right: they are the same point.
  const positionId = new Uint32Array(vertexCount);
  const ids = new Map<string, number>();
  for (let v = 0; v < vertexCount; v++) {
    const key = `${positions[3 * v]},${positions[3 * v + 1]},${positions[3 * v + 2]}`;
    let id = ids.get(key);
    if (id === undefined) {
      id = ids.size;
      ids.set(key, id);
    }
    positionId[v] = id;
  }
  const positionCount = ids.size;

  // CSR of the faces around each position. A face that names one position twice (degenerate)
  // is listed once for it.
  const counts = new Uint32Array(positionCount + 1);
  const cornerIds = (f: number): number[] => {
    const out: number[] = [];
    for (let k = 0; k < 3; k++) {
      const v = indices[3 * f + k];
      const p = v < vertexCount ? positionId[v] : -1;
      if (p >= 0 && !out.includes(p)) out.push(p);
    }
    return out;
  };
  for (let f = 0; f < faceCount; f++) for (const p of cornerIds(f)) counts[p + 1]++;
  const offsets = new Uint32Array(positionCount + 1);
  for (let p = 0; p < positionCount; p++) offsets[p + 1] = offsets[p] + counts[p + 1];
  const fill = offsets.slice(0, positionCount);
  const faces = new Uint32Array(offsets[positionCount]);
  for (let f = 0; f < faceCount; f++) for (const p of cornerIds(f)) faces[fill[p]++] = f;

  const normals = new Float64Array(3 * faceCount);
  for (let f = 0; f < faceCount; f++) {
    const n = faceNormalOf(positions, indices, f);
    normals[3 * f] = n[0];
    normals[3 * f + 1] = n[1];
    normals[3 * f + 2] = n[2];
  }
  return { faceCount, normals, positionId, offsets, faces, indices };
}

/** The unit normal of face `f` by the right-hand rule on (a, b, c); zero when degenerate. */
export function faceNormalOf(positions: Float64Array, indices: Uint32Array, f: number): [number, number, number] {
  const a = 3 * indices[3 * f];
  const b = 3 * indices[3 * f + 1];
  const c = 3 * indices[3 * f + 2];
  const ux = positions[b] - positions[a];
  const uy = positions[b + 1] - positions[a + 1];
  const uz = positions[b + 2] - positions[a + 2];
  const vx = positions[c] - positions[a];
  const vy = positions[c + 1] - positions[a + 1];
  const vz = positions[c + 2] - positions[a + 2];
  const nx = uy * vz - uz * vy;
  const ny = uz * vx - ux * vz;
  const nz = ux * vy - uy * vx;
  const len = Math.hypot(nx, ny, nz);
  if (!(len > 0) || !Number.isFinite(len)) return [0, 0, 0];
  // `+ 0` turns a -0 component into 0.
  return [nx / len + 0, ny / len + 0, nz / len + 0];
}

/**
 * The faces upstream's double-click takes from `seed`: the seed, and every face reachable
 * through shared vertex positions whose normal satisfies |n_seed . n| > `eps`. Ascending.
 */
export function coplanarFaces(topo: FaceTopology, seed: number, eps = COPLANAR_EPS): number[] {
  const { faceCount, normals, positionId, offsets, faces, indices } = topo;
  if (!(seed >= 0 && seed < faceCount) || !Number.isInteger(seed)) return [];
  const sx = normals[3 * seed];
  const sy = normals[3 * seed + 1];
  const sz = normals[3 * seed + 2];
  // 0 unseen, 1 taken, 2 refused. Upstream re-tests a refused face each time it is met; the
  // test is against the seed's normal only, so its answer cannot change and once is enough.
  const state = new Uint8Array(faceCount);
  const stack: number[] = [seed];
  state[seed] = 1;
  const out: number[] = [];
  while (stack.length > 0) {
    const f = stack.pop() as number;
    out.push(f);
    for (let k = 0; k < 3; k++) {
      const v = indices[3 * f + k];
      if (v >= positionId.length) continue;
      const p = positionId[v];
      for (let j = offsets[p]; j < offsets[p + 1]; j++) {
        const g = faces[j];
        if (state[g] !== 0) continue;
        const dot = sx * normals[3 * g] + sy * normals[3 * g + 1] + sz * normals[3 * g + 2];
        if (Math.abs(dot) > eps) {
          state[g] = 1;
          stack.push(g);
        } else {
          state[g] = 2;
        }
      }
    }
  }
  return out.sort((a, b) => a - b);
}
