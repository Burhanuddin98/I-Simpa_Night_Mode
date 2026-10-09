// Parity G43: which faces the 3D view draws, as upstream's View > Faces offers it
// (i_simpa_main.cpp:238-242; projet.cpp:2664-2692; Objet3D.cpp:585-594):
//   - Inside (upstream's default): the front faces culled, so the walls nearest the camera drop away
//     and the room shows from within. Here faces are drawn BackSide for that (engine.ts).
//   - Outside: no culling, every face drawn, so the room is a closed, opaque shell seen from outside.
//   - None: no faces at all, only the edges (and the markers, the selection and the overlays).
// A plan is the room seen from above into it, so the plan view and the plan inset always draw the
// inside: from outside they would show only the roof. A pure module: no three.js, no DOM, tested by
// faces.test.ts under `node --test`.

/** What View style > Faces is set to. */
export type FaceShow = 'inside' | 'outside' | 'none';

export const FACE_SHOWS: { key: FaceShow; label: string; hint: string }[] = [
  { key: 'inside', label: 'Inside', hint: 'The walls nearest you drop away, so you see into the room (upstream’s default)' },
  { key: 'outside', label: 'Outside', hint: 'Every face drawn: the room as a closed shell, seen from outside' },
  { key: 'none', label: 'None', hint: 'No faces, only the edges' },
];

/** A stored value as a FaceShow; anything else is the default, Inside. */
export function faceShowOf(v: unknown): FaceShow {
  return v === 'outside' || v === 'none' ? v : 'inside';
}

/** How one pass draws and picks the faces. */
export interface FacePlan {
  /** The faces are drawn at all (their colour and their depth). */
  drawn: boolean;
  /** Both sides of each face, not only the back (the side facing into the room). */
  bothSides: boolean;
  /** See-through's glass layer over the near walls: only in the inside view, where they dropped away. */
  glass: boolean;
  /** Which side a pick ray meets: the drawn one; with no faces drawn, the nearest face either way. */
  pickSide: 'back' | 'double';
  /** A drawn face can hide a marker or a map face behind it from a pick. */
  occludes: boolean;
}

/** The plan for one pass: `show` as set, `seeThrough` when the surface style is See-through, `plan`
 * for the plan camera (the Plan view or the inset), which always draws the inside. */
export function facePlan(show: FaceShow, seeThrough: boolean, plan: boolean): FacePlan {
  const s = plan ? 'inside' : show;
  if (s === 'outside') return { drawn: true, bothSides: true, glass: false, pickSide: 'double', occludes: true };
  if (s === 'none') return { drawn: false, bothSides: false, glass: false, pickSide: 'double', occludes: false };
  return { drawn: true, bothSides: false, glass: seeThrough, pickSide: 'back', occludes: true };
}
