// The classes the viewport package will build on, referenced by the foundation's stub so the
// release bundle already holds them: M9 gate (f) then reads three.js and three-mesh-bvh in
// `dist` before any package starts (PLAN.md 4.5). Nothing here is instantiated; the one
// WebGPURenderer (decision 68 (b): WebGPU, or WebGL2 where the WebView lacks it), and so the one canvas,
// is the viewport package's to create.
import {
  BufferGeometry,
  DataTexture,
  EdgesGeometry,
  LineSegments,
  Mesh,
  MeshBasicMaterial,
  OrthographicCamera,
  PerspectiveCamera,
  REVISION,
  Raycaster,
  Scene,
} from 'three';
import { WebGPURenderer } from 'three/webgpu';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import { acceleratedRaycast, computeBoundsTree, MeshBVH } from 'three-mesh-bvh';

const CLASSES = [
  WebGPURenderer,
  Scene,
  PerspectiveCamera,
  OrthographicCamera,
  BufferGeometry,
  Mesh,
  MeshBasicMaterial,
  EdgesGeometry,
  LineSegments,
  DataTexture,
  Raycaster,
  OrbitControls,
  MeshBVH,
  acceleratedRaycast,
  computeBoundsTree,
];

/** A description of the bundled viewport libraries (no digit next to a unit). */
export const VIEWPORT_LIBRARIES = `three r${REVISION}; ${CLASSES.map((c) => c.name).join(', ')}`;
