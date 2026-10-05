// The 3D view's engine (PLAN.md 6.1): the one WebGLRenderer and its canvas, created on the
// first mount and kept for the app's lifetime (PLAN.md 2.4, rule 3), rendering on demand.
//
//   - The mesh comes from `meshStore`: f64 positions sent to the GPU as f32, indexed in project
//     face order. Faces are drawn BackSide, so the near walls drop away (the inside view), and
//     every raycast honours BackSide.
//   - Picking is three-mesh-bvh built with `indirect: true` (finding F6), so a hit's faceIndex
//     is the project's face index. A click selects a face, a double-click its flat surface
//     (floodfill.ts, upstream's rule).
//   - Overlays: the model check's faces in the warn colour, the selection in the design's red
//     wash and outline. Markers: sources as a red glow, receivers as white rings (DataTexture
//     sprites), stems to the floor, DOM labels.
//   - The plan inset is a second pass in the same renderer: an orthographic top camera and a
//     scissor rectangle under the DOM frame. The Plan tab uses that camera as the main view.
//
// No acoustic number is computed or shown here; the plan label is a geometry fact. The Results
// step's layer (resultsLayer.ts: the surface map and particle playback, M12 P3) is drawn in this
// scene, only on that step, at the shared Animator's step (animator.ts).
import {
  AmbientLight,
  BackSide,
  FrontSide,
  BufferAttribute,
  BufferGeometry,
  Color,
  DataTexture,
  DirectionalLight,
  DoubleSide,
  EdgesGeometry,
  Float32BufferAttribute,
  LinearFilter,
  LineBasicMaterial,
  LineSegments,
  Mesh,
  MeshBasicMaterial,
  MeshLambertMaterial,
  MeshMatcapMaterial,
  OrthographicCamera,
  PerspectiveCamera,
  Points,
  PointsMaterial,
  Ray,
  Raycaster,
  RGBAFormat,
  Scene,
  SRGBColorSpace,
  Vector2,
  Vector3,
  WebGLRenderer,
  type Camera,
} from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import { LineMaterial } from 'three/addons/lines/LineMaterial.js';
import { LineSegments2 } from 'three/addons/lines/LineSegments2.js';
import { LineSegmentsGeometry } from 'three/addons/lines/LineSegmentsGeometry.js';
import type { MeshBVH } from 'three-mesh-bvh';
import * as actions from '../../actions';
import type { SceneMesh } from '../../mesh';
import type { Particles, SurfaceMap } from '../../resultsData';
import { effectiveMaterial } from '../../chrome/sceneModel';
import { log, meshStore, sceneStore, selectionStore, stepStore, Store, toolStore, viewportStore, type Selection } from '../../store';
import { registerHook } from '../../testhooks';
import { buildTopology, coplanarFaces, faceNormalOf, type FaceTopology } from './floodfill';
import { flipRows } from './snapshot';
import { animatorStore } from './animator';
import { firstFace, modelGeometry, pickingBvh } from './pick';
import { ResultsLayer, type MapMeta, type ParticleMeta } from './resultsLayer';
import { faceBounds, faceCentroid, fitOrtho, gizmoAxes, isFloorLike, placementPoint, planDimensions, rayOnFacePlane, type Box, type Vec } from './geometry';
import { BG, glowPixels, RED, ringPixels, WHITE } from './sprites';
import { planeCells } from '../../chrome/planes';

export type ViewMode = 'perspective' | 'plan';

/** What the React overlays show; written by the engine only. */
export interface ViewportUi {
  view: ViewMode;
  /** A mesh with at least one face is loaded. */
  hasModel: boolean;
  /** Faces uploaded to the check-highlight overlay. */
  highlightCount: number;
  /** "L × W m" of the model's plan, a geometry fact. */
  planLabel: string | null;
  /** A transient message from a placement click. */
  notice: string | null;
  /** Why the 3D view cannot draw (no WebGL). */
  error: string | null;
}

/**
 * W5 (R51): the map face under the pointer on the Results step, where nothing of the model hides
 * it, with the pointer's client point; null off the map. The overlay reads the face's value.
 */
/** The particle step of each frame drawn on the Results step, newest last (the playback e2e reads it). */
const drawn: { t: number; step: number }[] = [];
const DRAWN_MAX = 4096;
function noteDrawn(step: number): void {
  drawn.push({ t: performance.now(), step });
  if (drawn.length > DRAWN_MAX) drawn.splice(0, drawn.length - DRAWN_MAX);
}
/** The frames drawn since the last call (and clears them): the time each was drawn, ms, and its step. */
export function drawnSteps(): { t: number; step: number }[] {
  return drawn.splice(0, drawn.length);
}

export const mapPointerStore = new Store<{ face: number; x: number; y: number } | null>(null);

export const viewportUi = new Store<ViewportUi>({
  view: 'perspective',
  hasModel: false,
  highlightCount: 0,
  planLabel: null,
  notice: null,
  error: null,
});

/**
 * How the 3D view draws the room (the View style menu, Burhan 2026-10-06): surfaces in their
 * material colours (the default), grey, see-through (the near walls as glass at `glass` %), or the
 * old wireframe look; edges at every triangle or only where faces meet at 20 degrees or more.
 * Remembered per viewer in localStorage, which can be absent: then the defaults.
 */
export type SurfaceStyle = 'colour' | 'grey' | 'glass' | 'wire';
export type EdgeStyle = 'all' | 'feature';
export interface ViewStyle {
  surfaces: SurfaceStyle;
  edges: EdgeStyle;
  /** The near walls' opacity in see-through, percent, 0 to 60. */
  glass: number;
}
const STYLE_KEY = 'nm.viewStyle';
const DEFAULT_STYLE: ViewStyle = { surfaces: 'colour', edges: 'all', glass: 15 };
function loadStyle(): ViewStyle {
  try {
    const v = JSON.parse(localStorage.getItem(STYLE_KEY) ?? 'null') as Partial<ViewStyle> | null;
    if (!v) return DEFAULT_STYLE;
    return {
      surfaces: (['colour', 'grey', 'glass', 'wire'] as const).includes(v.surfaces as SurfaceStyle) ? (v.surfaces as SurfaceStyle) : DEFAULT_STYLE.surfaces,
      edges: v.edges === 'feature' ? 'feature' : 'all',
      glass: typeof v.glass === 'number' ? Math.min(60, Math.max(0, v.glass)) : DEFAULT_STYLE.glass,
    };
  } catch {
    return DEFAULT_STYLE;
  }
}
export const viewStyle = new Store<ViewStyle>(loadStyle());
viewStyle.subscribe(() => {
  try {
    localStorage.setItem(STYLE_KEY, JSON.stringify(viewStyle.get()));
  } catch {
    // No storage (a private profile): the choice lasts this session only.
  }
});

export interface ViewportDom {
  /** The viewport root (`data-part="viewport"`). */
  root: HTMLElement;
  /** The element the canvas goes into. */
  host: HTMLElement;
  /** The plan inset's drawing box: the scissor rectangle is its client rectangle. */
  inset: HTMLElement;
  /** The container for the markers' DOM labels. */
  labels: HTMLElement;
  /** The axis gizmo. */
  gizmo: SVGSVGElement;
}

// Colours: the design's (concept-b-approved.dc.html) and theme.css's tokens.
const PANEL = new Color(0x0f0f11);
const LINE = 0xededef;
const WARN = 0xf2a93b;
const SELECT = 0xe0202e;

const SOURCE_PX = 64;
const RECEIVER_PX = 14;
const HALO_PX = 28;
/** Markers in the inset are drawn at this fraction of their size. */
const INSET_MARKER_SCALE = 0.5;
/** A click within this many pixels of a marker picks it. */
const MARKER_PICK_PX = { source: 11, receiver: 9 } as const;
/** A pointer that travels further than this between down and up was a drag, not a click. */
const CLICK_SLOP_PX = 4;
/** A plane's drawn grid has at most this many lines each way (a finer plane is drawn coarser, its outline exact). */
const PLANE_GRID_MAX = 120;

type MarkerKind = 'source' | 'receiver';
interface Marker {
  kind: MarkerKind;
  id: string;
  name: string;
  p: Vector3;
  label: HTMLDivElement;
}

type Pick =
  | { kind: 'none' }
  | { kind: 'marker'; marker: Marker }
  | { kind: 'face'; face: number; origin: Vec; dir: Vec };

const finite = (v: number | string): v is number => typeof v === 'number' && Number.isFinite(v);

function dataTexture(pixels: Uint8Array, size: number): DataTexture {
  const t = new DataTexture(pixels, size, size, RGBAFormat);
  t.colorSpace = SRGBColorSpace;
  t.magFilter = LinearFilter;
  t.minFilter = LinearFilter;
  t.generateMipmaps = false;
  t.needsUpdate = true;
  return t;
}

// The surfaces' shading (decision 59): a camera-fixed matcap, so each face is lit by its angle to
// the eye and walls, floor and ceiling separate at a glance. Mid greys on black, a key light upper
// left, no red (red stays for actions and selection). Built from pixels, not a canvas (rule 3).
// Chosen by Burhan on the proof page, docs/investigations/2026-10-06-ui-3d/.
function matcapTexture(): DataTexture {
  const n = 128;
  const stops: [number, number[]][] = [
    [0, [0x76, 0x76, 0x80]],
    [0.45, [0x4a, 0x4a, 0x52]],
    [0.85, [0x26, 0x26, 0x2b]],
    [1, [0x1a, 0x1a, 0x1e]],
  ];
  const at = (t: number): number[] => {
    for (let i = 1; i < stops.length; i++) {
      const [t1, c1] = stops[i];
      const [t0, c0] = stops[i - 1];
      if (t <= t1) return c0.map((c, k) => c + ((c1[k] - c) * (t - t0)) / (t1 - t0));
    }
    return stops[stops.length - 1][1];
  };
  const pixels = new Uint8Array(n * n * 4);
  for (let y = 0; y < n; y++) {
    for (let x = 0; x < n; x++) {
      const u = (x + 0.5) / n, v = (y + 0.5) / n;
      const inside = Math.hypot(u - 0.5, v - 0.5) <= 0.5;
      // DataTexture rows run bottom-up: the key sits upper left on screen at (0.36, 0.68).
      const c = inside ? at(Math.min(1, Math.hypot(u - 0.36, v - 0.68) / 0.62)) : [0, 0, 0];
      pixels.set([c[0], c[1], c[2], 255], (y * n + x) * 4);
    }
  }
  return dataTexture(pixels, n);
}

function markerMaterial(pixels: Uint8Array, texSize: number, px: number): PointsMaterial {
  return new PointsMaterial({
    size: px,
    sizeAttenuation: false,
    map: dataTexture(pixels, texSize),
    transparent: true,
    depthWrite: false,
    alphaTest: 0.01,
  });
}

class ViewportEngine {
  private renderer: WebGLRenderer | null = null;
  private canvas: HTMLCanvasElement | null = null;
  private contextLost = false;
  private readonly scene = new Scene();
  private readonly persp = new PerspectiveCamera(45, 1, 0.01, 1000);
  private readonly plan = new OrthographicCamera(-1, 1, 1, -1, 0.01, 1000);
  private controls: OrbitControls | null = null;
  private readonly raycaster = new Raycaster();
  private dom: ViewportDom | null = null;
  private detachers: (() => void)[] = [];
  private frameRequest = 0;
  private noticeTimer = 0;
  private view: ViewMode = 'perspective';

  // The model as drawn.
  private mesh: SceneMesh | null = null;
  private positions32: Float32Array | null = null;
  private bvh: MeshBVH | null = null;
  private topo: FaceTopology | null = null;
  private bounds: Box | null = null;
  private builtRev: number | null = null;
  private highlightCount = 0;
  private markers: Marker[] = [];

  // Scene objects, created once; their geometries are swapped.
  private readonly faces: Mesh;
  private readonly tint: Mesh;
  private readonly ghost: Mesh;
  private readonly tintColour: MeshMatcapMaterial;
  private readonly tintGrey: MeshMatcapMaterial;
  private readonly tintWire: MeshLambertMaterial;
  private triangleEdges: BufferGeometry = new BufferGeometry();
  private featureEdges: BufferGeometry = new BufferGeometry();
  private readonly edges: LineSegments;
  private readonly highlight: Mesh;
  private readonly selectionWash: Mesh;
  private readonly selectionEdges: LineSegments2;
  private readonly sourcePoints: Points;
  private readonly receiverPoints: Points;
  private readonly halo: Points;
  private readonly sourceStems: LineSegments;
  private readonly receiverStems: LineSegments;
  /** W1: each cutting plane's outline, and its cell grid off the Results step (upstream's DrawPlan). */
  private readonly planeOutline: LineSegments2;
  private readonly planeGrid: LineSegments2;
  private planeSummary: { name: string; corners: Vec[]; u: number; v: number; gridLines: number }[] = [];

  // The pointer between down and up.
  private down = { x: 0, y: 0, moved: false };

  /** The Results step's surface map and particles (M12 P3). */
  readonly results = new ResultsLayer();

  constructor() {
    this.persp.up.set(0, 0, 1);
    this.plan.up.set(0, 1, 0);
    this.scene.add(new AmbientLight(0xffffff, 1.6));
    const sun = new DirectionalLight(0xffffff, 2.6);
    sun.position.set(0.35, 0.55, 1);
    this.scene.add(sun);

    // Faces are pushed back a little so lines on them and the overlays win the depth test.
    // `faces` is the picked mesh (indexed, project face order) and writes only depth; `tint`, a
    // non-indexed copy, draws it with each face in its surface group's material colour, muted, under
    // the matcap's shading (Burhan 2026-10-06: "the surfaces still look kinda greyscale"). A copy,
    // because groups share vertices: per-vertex colour on the indexed mesh would bleed across seams.
    const matcap = matcapTexture();
    this.faces = new Mesh(
      new BufferGeometry(),
      new MeshMatcapMaterial({ matcap, side: BackSide, flatShading: true, polygonOffset: true, polygonOffsetFactor: 1, polygonOffsetUnits: 1, colorWrite: false }),
    );
    const flat = { side: BackSide, flatShading: true, polygonOffset: true, polygonOffsetFactor: 1, polygonOffsetUnits: 1 } as const;
    this.tintColour = new MeshMatcapMaterial({ matcap, vertexColors: true, ...flat });
    this.tintGrey = new MeshMatcapMaterial({ matcap, ...flat });
    // The old look, kept as Wireframe in the menu: near-black Lambert faces under every edge.
    this.tintWire = new MeshLambertMaterial({ color: 0x19191d, ...flat });
    this.tint = new Mesh(new BufferGeometry(), this.tintColour);
    // See-through: the near walls drawn again as glass on top, writing no depth, so nothing behind
    // them is hidden (the proof page's layer 2, decision 59).
    this.ghost = new Mesh(
      new BufferGeometry(),
      new MeshMatcapMaterial({ matcap, vertexColors: true, side: FrontSide, flatShading: true, transparent: true, opacity: 0.15, depthWrite: false }),
    );
    this.ghost.visible = false;
    this.edges = new LineSegments(new BufferGeometry(), new LineBasicMaterial({ color: LINE, transparent: true, opacity: 0.3, depthWrite: false }));
    this.highlight = new Mesh(
      new BufferGeometry(),
      new MeshBasicMaterial({ color: WARN, transparent: true, opacity: 0.55, side: DoubleSide, depthWrite: false }),
    );
    this.selectionWash = new Mesh(
      new BufferGeometry(),
      new MeshBasicMaterial({ color: SELECT, transparent: true, opacity: 0.3, side: BackSide, depthWrite: false }),
    );
    // The red outlines are fat lines (WebGL draws 1 px whatever is asked): the selection 2 px, a sound-level
    // plane's outline 3.5 px and its grid 1.5 px (Burhan 2026-10-06: "the red lines need to be a bit thicker ... like the sound-level plane").
    this.selectionEdges = new LineSegments2(new LineSegmentsGeometry(), new LineMaterial({ color: SELECT, linewidth: 2, transparent: true, opacity: 1 }));
    this.sourcePoints = new Points(new BufferGeometry(), markerMaterial(glowPixels(64), 64, SOURCE_PX));
    this.receiverPoints = new Points(new BufferGeometry(), markerMaterial(ringPixels(32, 0.55, 0.85, WHITE, BG), 32, RECEIVER_PX));
    this.halo = new Points(new BufferGeometry(), markerMaterial(ringPixels(64, 0.78, 0.92, RED, null), 64, HALO_PX));
    this.sourceStems = new LineSegments(new BufferGeometry(), new LineBasicMaterial({ color: SELECT, transparent: true, opacity: 0.9 }));
    this.receiverStems = new LineSegments(new BufferGeometry(), new LineBasicMaterial({ color: LINE, transparent: true, opacity: 0.5 }));
    this.planeOutline = new LineSegments2(new LineSegmentsGeometry(), new LineMaterial({ color: SELECT, linewidth: 3.5, transparent: true, opacity: 0.95, depthWrite: false }));
    this.planeGrid = new LineSegments2(new LineSegmentsGeometry(), new LineMaterial({ color: SELECT, linewidth: 1.5, transparent: true, opacity: 0.45, depthWrite: false }));
    const order: [{ renderOrder: number }, number][] = [
      [this.faces, 0],
      [this.tint, 0.5],
      [this.ghost, 2.5],
      [this.highlight, 1],
      [this.selectionWash, 2],
      [this.edges, 3],
      [this.selectionEdges, 4],
      [this.sourceStems, 5],
      [this.receiverStems, 5],
      [this.planeGrid, 4],
      [this.planeOutline, 5],
      [this.receiverPoints, 6],
      [this.sourcePoints, 7],
      [this.halo, 8],
    ];
    for (const [o, n] of order) o.renderOrder = n;
    this.scene.add(
      this.faces,
      this.tint,
      this.ghost,
      this.highlight,
      this.selectionWash,
      this.edges,
      this.selectionEdges,
      this.sourceStems,
      this.receiverStems,
      this.planeGrid,
      this.planeOutline,
      this.receiverPoints,
      this.sourcePoints,
      this.halo,
      this.results.group,
    );
    this.results.setShown(stepStore.get() === 'results');
    this.defaultCamera();
  }

  // ---- lifetime -------------------------------------------------------------------------

  /** Mounts the view into `dom`. The renderer and canvas are made on the first call only. */
  attach(dom: ViewportDom): () => void {
    this.dom = dom;
    if (!this.renderer && !viewportUi.get().error) this.createRenderer();
    if (this.canvas) {
      dom.host.appendChild(this.canvas);
      this.controls?.connect(this.canvas);
      this.applyTool();
    }
    const resize = new ResizeObserver(() => this.invalidate());
    resize.observe(dom.root);
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== 'Escape' || e.ctrlKey || e.altKey || e.metaKey) return;
      const t = e.target;
      if (t instanceof HTMLElement && (t.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(t.tagName))) return;
      if (toolStore.get().startsWith('place-')) toolStore.set('select');
    };
    window.addEventListener('keydown', onKey);
    this.detachers = [
      meshStore.subscribe(() => this.setMesh(meshStore.get())),
      sceneStore.subscribe(() => this.onScene()),
      selectionStore.subscribe(() => this.onSelection()),
      viewStyle.subscribe(() => this.applyStyle()),
      toolStore.subscribe(() => this.applyTool()),
      stepStore.subscribe(() => {
        this.results.setShown(stepStore.get() === 'results');
        if (stepStore.get() !== 'results') mapPointerStore.set(null);
        this.updatePlanes();
        this.invalidate();
      }),
      animatorStore.subscribe(() => {
        this.results.setStep(animatorStore.get().step);
        this.invalidate();
      }),
      () => resize.disconnect(),
      () => window.removeEventListener('keydown', onKey),
      registerHook('highlightedFaceCount', () => this.highlightCount),
      registerHook('selection', () => this.selectionSummary()),
      registerHook('facesOfGroup', (name: string) => this.facesOfGroup(name)),
      registerHook('aimAtFace', (face: number) => this.aimAtFace(face)),
      registerHook('frame', () => {
        this.frame();
        this.renderNow();
        return true;
      }),
      // Beyond PLAN.md 2.5, for the package's own checks: a face's client point under the
      // current camera (no camera move), and the camera itself.
      registerHook('faceClientPoint', (face: number) => this.faceClientPoint(face)),
      registerHook('cameraState', () => this.cameraState()),
      // The recorded tour (app/e2e/specs/tour.e2e.ts): the perspective camera flown to a position and target over `ms`.
      registerHook('flyCamera', (position: Vec, target: Vec, ms: number) => this.flyCamera(position, target, ms)),
      // Gate (a): what the check-highlight overlay puts on screen, not what was uploaded to it.
      registerHook('highlightPixels', () => this.highlightPixels()),
      // W1: the cutting planes the view draws (outline corners, cells, grid lines drawn).
      registerHook('planeOutlines', () => this.planeSummary),
    ];
    // A remount (React StrictMode in dev) rebuilds the DOM-side state from the stores.
    this.setMesh(meshStore.get(), true);
    viewportStore.set({ live: this.renderer !== null, drawnRev: null });
    this.invalidate();
    return () => this.detach();
  }

  private detach(): void {
    for (const d of this.detachers) d();
    this.detachers = [];
    this.controls?.disconnect();
    this.canvas?.remove();
    if (this.frameRequest) cancelAnimationFrame(this.frameRequest);
    this.frameRequest = 0;
    if (this.dom) this.dom.labels.replaceChildren();
    this.markers = [];
    this.dom = null;
    viewportStore.set({ live: false, drawnRev: null });
  }

  private createRenderer(): void {
    const canvas = document.createElement('canvas');
    canvas.className = 'viewport-canvas';
    let renderer: WebGLRenderer;
    try {
      renderer = new WebGLRenderer({ canvas, antialias: true, alpha: true, powerPreference: 'high-performance' });
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      log('FAIL', `The 3D view could not start WebGL: ${message}`);
      this.setUi({ error: message });
      return;
    }
    renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
    renderer.outputColorSpace = SRGBColorSpace;
    renderer.autoClear = false;
    this.renderer = renderer;
    this.canvas = canvas;
    this.results.setRenderer(renderer);

    canvas.addEventListener('webglcontextlost', (e) => {
      e.preventDefault();
      this.contextLost = true;
      log('WARN', 'The 3D view lost its WebGL context; it draws again when the context is restored');
      this.invalidate();
    });
    canvas.addEventListener('webglcontextrestored', () => {
      this.contextLost = false;
      log('INFO', 'The 3D view has its WebGL context back');
      this.invalidate();
    });
    canvas.addEventListener('pointerdown', (e) => {
      this.down = { x: e.clientX, y: e.clientY, moved: false };
    });
    canvas.addEventListener('pointermove', (e) => {
      if (e.buttons !== 0 && Math.hypot(e.clientX - this.down.x, e.clientY - this.down.y) > CLICK_SLOP_PX) this.down.moved = true;
      this.probeAt(e.buttons === 0 ? e.clientX : null, e.clientY);
    });
    canvas.addEventListener('pointerleave', () => this.probeAt(null, 0));
    canvas.addEventListener('click', (e) => {
      if (!this.down.moved) this.onClick(e.clientX, e.clientY);
    });
    canvas.addEventListener('dblclick', (e) => {
      if (!this.down.moved) this.onDoubleClick(e.clientX, e.clientY);
    });

    const controls = new OrbitControls(this.persp, canvas);
    controls.enableDamping = false;
    controls.screenSpacePanning = true;
    controls.addEventListener('change', () => this.invalidate());
    this.controls = controls;
    this.frame();
  }

  // ---- drawing --------------------------------------------------------------------------

  invalidate(): void {
    if (this.frameRequest || !this.dom) return;
    this.frameRequest = requestAnimationFrame(() => {
      this.frameRequest = 0;
      this.render();
    });
  }

  private renderNow(): void {
    if (this.frameRequest) cancelAnimationFrame(this.frameRequest);
    this.frameRequest = 0;
    this.render();
  }

  /** Keeps the drawing buffer and the perspective aspect at the canvas's CSS size. */
  private syncSize(): { w: number; h: number } | null {
    const r = this.renderer;
    const c = this.canvas;
    if (!r || !c) return null;
    const w = c.clientWidth;
    const h = c.clientHeight;
    if (w <= 0 || h <= 0) return null;
    const size = r.getSize(new Vector2());
    if (size.x !== w || size.y !== h) {
      r.setSize(w, h, false);
      this.persp.aspect = w / h;
      this.persp.updateProjectionMatrix();
    }
    // Fat lines measure their width against the canvas: told on every check, not only on a resize,
    // or a canvas sized before they existed leaves them at the default 1 x 1 and they draw wrong.
    for (const m of [this.selectionEdges.material, this.planeOutline.material, this.planeGrid.material]) m.resolution.set(w, h);
    return { w, h };
  }

  private render(): void {
    const r = this.renderer;
    const dom = this.dom;
    const size = this.syncSize();
    if (r && dom && size && !this.contextLost) {
      const { w, h } = size;
      r.setScissorTest(false);
      r.setViewport(0, 0, w, h);
      r.setClearColor(0x000000, 0);
      r.clear();
      const main = this.mainCamera(w / h);
      this.setMarkerScale(1);
      r.render(this.scene, main);
      if (this.results.particleMeta && stepStore.get() === 'results') noteDrawn(this.results.particleStep());
      if (this.view === 'perspective' && this.bounds) this.renderInset(r, dom.inset);
      this.placeLabels(main, w, h);
      this.placeGizmo(main);
    }
    const vp = viewportStore.get();
    const live = this.renderer !== null;
    if (vp.live !== live || vp.drawnRev !== this.builtRev) viewportStore.set({ live, drawnRev: this.builtRev });
  }

  /** The plan inset: the same scene through the top camera, scissored to the DOM box. */
  private renderInset(r: WebGLRenderer, inset: HTMLElement): void {
    const c = this.canvas?.getBoundingClientRect();
    const b = inset.getBoundingClientRect();
    if (!c || b.width < 2 || b.height < 2) return;
    const x = b.left - c.left;
    const y = c.bottom - b.bottom;
    r.setScissorTest(true);
    r.setScissor(x, y, b.width, b.height);
    r.setViewport(x, y, b.width, b.height);
    // The inset is a floating panel too (decision-log row 50): its box is see-through.
    r.setClearColor(PANEL, 0.35);
    r.clear();
    this.fitPlan(b.width / b.height);
    this.setMarkerScale(INSET_MARKER_SCALE);
    r.render(this.scene, this.plan);
    r.setScissorTest(false);
  }

  private setMarkerScale(k: number): void {
    (this.sourcePoints.material as PointsMaterial).size = SOURCE_PX * k;
    (this.receiverPoints.material as PointsMaterial).size = RECEIVER_PX * k;
    (this.halo.material as PointsMaterial).size = HALO_PX * k;
  }

  private mainCamera(aspect: number): Camera {
    if (this.view === 'plan') {
      this.fitPlan(aspect);
      return this.plan;
    }
    this.persp.updateMatrixWorld();
    return this.persp;
  }

  /** The top camera, fitted to the model's plan at `aspect`. */
  private fitPlan(aspect: number): void {
    const b = this.bounds ?? { min: [-5, -5, 0], max: [5, 5, 3] };
    const cx = (b.min[0] + b.max[0]) / 2;
    const cy = (b.min[1] + b.max[1]) / 2;
    const ez = b.max[2] - b.min[2];
    const { halfW, halfH } = fitOrtho(b.max[0] - b.min[0], b.max[1] - b.min[1], aspect);
    const p = this.plan;
    p.left = -halfW;
    p.right = halfW;
    p.top = halfH;
    p.bottom = -halfH;
    p.zoom = 1;
    p.near = 0.01;
    p.far = ez + 20;
    p.position.set(cx, cy, b.max[2] + 10);
    p.lookAt(cx, cy, b.min[2]);
    p.updateProjectionMatrix();
    p.updateMatrixWorld();
  }

  private placeLabels(camera: Camera, w: number, h: number): void {
    const v = new Vector3();
    for (const m of this.markers) {
      v.copy(m.p).project(camera);
      const visible = v.z > -1 && v.z < 1 && Math.abs(v.x) <= 1.02 && Math.abs(v.y) <= 1.02;
      m.label.style.display = visible ? '' : 'none';
      if (visible) {
        const x = ((v.x + 1) / 2) * w;
        const y = ((1 - v.y) / 2) * h;
        const dx = m.kind === 'source' ? 12 : 10;
        m.label.style.transform = `translate(${Math.round(x + dx)}px, ${Math.round(y - 8)}px)`;
      }
    }
  }

  private placeGizmo(camera: Camera): void {
    const svg = this.dom?.gizmo;
    if (!svg) return;
    for (const a of gizmoAxes(camera.matrixWorldInverse.elements, 17)) {
      const path = svg.querySelector(`[data-axis="${a.axis}"]`);
      const text = svg.querySelector(`[data-axis-label="${a.axis}"]`);
      const opacity = a.depth < -0.25 ? '0.45' : '1';
      path?.setAttribute('d', `M32 32l${a.dx.toFixed(2)} ${a.dy.toFixed(2)}`);
      path?.setAttribute('opacity', opacity);
      const len = Math.hypot(a.dx, a.dy);
      const k = len > 1e-3 ? (len + 7) / len : 0;
      text?.setAttribute('x', (32 + a.dx * k - 2.7).toFixed(2));
      text?.setAttribute('y', (32 + a.dy * k + 3.2).toFixed(2));
      text?.setAttribute('opacity', opacity);
    }
  }

  // ---- the model ------------------------------------------------------------------------

  /** Builds the drawn model from `mesh`; `force` rebuilds the same mesh (a remount). */
  private setMesh(mesh: SceneMesh | null, force = false): void {
    if (mesh === this.mesh && !force) return;
    const previousRev = this.mesh?.geometryRev ?? null;
    this.mesh = mesh;
    this.disposeModel();
    if (mesh && mesh.faceCount > 0) {
      const geometry = modelGeometry(mesh.positions, mesh.indices);
      this.positions32 = geometry.getAttribute('position').array as Float32Array;
      // indirect: the BVH keeps its own order and leaves the index in project face order (F6).
      this.bvh = pickingBvh(geometry);
      // Flat shading takes its normals from the screen; these only keep the attribute bound.
      geometry.computeVertexNormals();
      this.topo = buildTopology(mesh.positions, mesh.indices);
      this.bounds = faceBounds(mesh.positions, mesh.indices);
      this.faces.geometry = geometry;
      this.tint.geometry = geometry.toNonIndexed();
      this.ghost.geometry = this.tint.geometry;
      this.recolor();
      this.triangleEdges = new EdgesGeometry(geometry, 1);
      this.featureEdges = new EdgesGeometry(geometry, 20);
      this.applyStyle();
    }
    this.builtRev = mesh?.geometryRev ?? null;
    // A face selection names faces of the geometry it was made on.
    if (previousRev !== this.builtRev && selectionStore.get().kind === 'faces') selectionStore.set({ kind: 'none' });
    this.frame();
    this.onScene();
  }

  private disposeModel(): void {
    for (const o of [this.faces, this.tint, this.highlight, this.selectionWash]) {
      o.geometry.dispose();
      o.geometry = new BufferGeometry();
    }
    this.ghost.geometry = new BufferGeometry();
    this.triangleEdges.dispose();
    this.featureEdges.dispose();
    this.triangleEdges = new BufferGeometry();
    this.featureEdges = new BufferGeometry();
    this.edges.geometry = this.triangleEdges;
    this.selectionEdges.geometry.dispose();
    this.selectionEdges.geometry = new LineSegmentsGeometry();
    this.bvh = null;
    this.positions32 = null;
    this.topo = null;
    this.bounds = null;
  }

  /** An overlay geometry of `faces` over the model's positions (its own GPU buffer). */
  private overlay(faces: ArrayLike<number>): BufferGeometry {
    const g = new BufferGeometry();
    const mesh = this.mesh;
    if (!mesh || !this.positions32 || faces.length === 0) return g;
    const index = new Uint32Array(faces.length * 3);
    let n = 0;
    for (let i = 0; i < faces.length; i++) {
      const f = faces[i];
      if (!(f >= 0 && f < mesh.faceCount)) continue;
      index[3 * n] = mesh.indices[3 * f];
      index[3 * n + 1] = mesh.indices[3 * f + 1];
      index[3 * n + 2] = mesh.indices[3 * f + 2];
      n++;
    }
    g.setAttribute('position', new BufferAttribute(this.positions32, 3));
    g.setIndex(new BufferAttribute(index.subarray(0, 3 * n), 1));
    return g;
  }

  /** Applies `viewStyle`: the surface material, the glass layer and the edge set. */
  private applyStyle(): void {
    const st = viewStyle.get();
    this.tint.material = st.surfaces === 'wire' ? this.tintWire : st.surfaces === 'grey' ? this.tintGrey : this.tintColour;
    this.ghost.visible = st.surfaces === 'glass';
    (this.ghost.material as MeshMatcapMaterial).opacity = st.glass / 100;
    this.edges.geometry = st.edges === 'feature' ? this.featureEdges : this.triangleEdges;
    this.invalidate();
  }

  /** Each face in its surface group's effective material colour (variants included), muted toward white. */
  private recolor(): void {
    const mesh = this.mesh;
    const view = sceneStore.get()?.view;
    const g = this.tint.geometry;
    const pos = g.getAttribute('position');
    if (!mesh || !view || !pos) return;
    const byGroup = new Map<number, Color>();
    const colorOf = (gi: number): Color => {
      let c = byGroup.get(gi);
      if (!c) {
        const group = view.surface_groups[gi];
        const m = group ? effectiveMaterial(view, group.id) : null;
        const base = new Color(m?.color ?? '#9a9aa0');
        // The hue at full brightness, then 40 % of the way from white: the matcap keeps the shading.
        const peak = Math.max(base.r, base.g, base.b, 1e-6);
        c = new Color(1, 1, 1).lerp(new Color(base.r / peak, base.g / peak, base.b / peak), 0.4);
        byGroup.set(gi, c);
      }
      return c;
    };
    const colors = new Float32Array(pos.count * 3);
    const faces = Math.min(mesh.faceCount, pos.count / 3);
    for (let f = 0; f < faces; f++) {
      const c = colorOf(mesh.groups[f]);
      for (let k = 0; k < 3; k++) colors.set([c.r, c.g, c.b], (3 * f + k) * 3);
    }
    g.setAttribute('color', new Float32BufferAttribute(colors, 3));
  }

  private onScene(): void {
    this.recolor();
    this.updateHighlight();
    this.updateSelection();
    this.updateMarkers();
    this.updatePlanes();
    this.setUi({
      hasModel: !!this.mesh && this.mesh.faceCount > 0,
      highlightCount: this.highlightCount,
      planLabel: this.bounds ? planDimensions(this.bounds) : null,
    });
    this.invalidate();
  }

  private onSelection(): void {
    this.updateSelection();
    this.updateMarkers();
    this.invalidate();
  }

  /** The model check's faces, only when the check and the drawn mesh are the same geometry. */
  private updateHighlight(): void {
    const state = sceneStore.get();
    const mesh = this.mesh;
    let faces: number[] = [];
    if (state?.check && mesh && state.info.geometry_rev === mesh.geometryRev) {
      faces = state.check.highlight_faces.filter((f) => f >= 0 && f < mesh.faceCount);
    }
    this.highlight.geometry.dispose();
    this.highlight.geometry = this.overlay(faces);
    this.highlightCount = (this.highlight.geometry.index?.count ?? 0) / 3;
  }

  private updateSelection(): void {
    const faces = this.selectedFaces();
    this.selectionWash.geometry.dispose();
    this.selectionEdges.geometry.dispose();
    const g = this.overlay(faces);
    this.selectionWash.geometry = g;
    const fat = new LineSegmentsGeometry();
    if (faces.length > 0) {
      const e = new EdgesGeometry(g, 1);
      fat.setPositions(e.getAttribute('position').array as Float32Array);
      e.dispose();
    }
    this.selectionEdges.geometry = fat;
  }

  /** The faces the selection covers: picked faces, or every face of a selected group. */
  private selectedFaces(): number[] {
    const s = selectionStore.get();
    if (s.kind === 'faces') return s.faces;
    if (s.kind === 'group') {
      const groups = sceneStore.get()?.view.surface_groups ?? [];
      const gi = groups.findIndex((g) => g.id === s.id);
      return gi < 0 ? [] : this.facesOfGroupIndices(new Set([gi]));
    }
    return [];
  }

  private facesOfGroupIndices(indices: Set<number>): number[] {
    const mesh = this.mesh;
    if (!mesh) return [];
    const out: number[] = [];
    for (let f = 0; f < mesh.faceCount; f++) if (indices.has(mesh.groups[f])) out.push(f);
    return out;
  }

  private facesOfGroup(name: string): number[] {
    const groups = sceneStore.get()?.view.surface_groups ?? [];
    const indices = new Set<number>();
    groups.forEach((g, i) => {
      if (g.name === name) indices.add(i);
    });
    return this.facesOfGroupIndices(indices);
  }

  /** The group names of `faces`, sorted and unique. */
  private groupNames(faces: readonly number[]): string[] {
    const mesh = this.mesh;
    const groups = sceneStore.get()?.view.surface_groups ?? [];
    const names = new Set<string>();
    if (mesh) for (const f of faces) {
      const g = groups[mesh.groups[f]];
      if (g) names.add(g.name);
    }
    return [...names].sort();
  }

  private facesSelection(faces: number[]): Selection {
    const sorted = [...faces].sort((a, b) => a - b);
    return { kind: 'faces', faces: sorted, groups: this.groupNames(sorted) };
  }

  private selectionSummary(): { faces: number[]; groups: string[] } {
    const s = selectionStore.get();
    if (s.kind === 'faces') return { faces: [...s.faces].sort((a, b) => a - b), groups: [...new Set(s.groups)].sort() };
    if (s.kind === 'group') {
      const faces = this.selectedFaces();
      const name = sceneStore.get()?.view.surface_groups.find((g) => g.id === s.id)?.name;
      return { faces, groups: name === undefined ? [] : [name] };
    }
    return { faces: [], groups: [] };
  }

  // ---- markers --------------------------------------------------------------------------

  private updateMarkers(): void {
    const view = sceneStore.get()?.view;
    const labels = this.dom?.labels;
    const sel = selectionStore.get();
    const list: Marker[] = [];
    const make = (kind: MarkerKind, id: string, name: string, pos: readonly (number | string)[], off: boolean) => {
      const [x, y, z] = pos;
      if (!finite(x) || !finite(y) || !finite(z)) return;
      const label = document.createElement('div');
      const selected = (sel.kind === 'source' || sel.kind === 'receiver') && sel.kind === kind && sel.id === id;
      label.className = `vp-label ${kind}${selected ? ' selected' : ''}${off ? ' off' : ''}`;
      label.textContent = name;
      list.push({ kind, id, name, p: new Vector3(x, y, z), label });
    };
    for (const s of view?.sources ?? []) make('source', s.id, s.name, s.position, !s.enabled);
    for (const r of view?.point_receivers ?? []) make('receiver', r.id, r.name, r.position, false);
    this.markers = list;
    labels?.replaceChildren(...list.map((m) => m.label));

    const points = (kind: MarkerKind) => {
      const ms = list.filter((m) => m.kind === kind);
      const g = new BufferGeometry();
      g.setAttribute('position', new Float32BufferAttribute(ms.flatMap((m) => [m.p.x, m.p.y, m.p.z]), 3));
      return { g, ms };
    };
    const stems = (ms: Marker[]) => {
      const g = new BufferGeometry();
      if (this.bvh && this.bounds) {
        const flat: number[] = [];
        for (const m of ms) {
          const hit = this.bvh.raycastFirst(new Ray(m.p.clone(), new Vector3(0, 0, -1)), DoubleSide);
          const floorZ = hit ? hit.point.z : this.bounds.min[2];
          flat.push(m.p.x, m.p.y, m.p.z, m.p.x, m.p.y, floorZ);
        }
        g.setAttribute('position', new Float32BufferAttribute(flat, 3));
      }
      return g;
    };
    const src = points('source');
    const rcv = points('receiver');
    for (const o of [this.sourcePoints, this.receiverPoints, this.halo, this.sourceStems, this.receiverStems]) o.geometry.dispose();
    this.sourcePoints.geometry = src.g;
    this.receiverPoints.geometry = rcv.g;
    this.sourceStems.geometry = stems(src.ms);
    this.receiverStems.geometry = stems(rcv.ms);
    const picked = list.find((m) => (sel.kind === 'source' || sel.kind === 'receiver') && m.kind === sel.kind && m.id === sel.id);
    const halo = new BufferGeometry();
    if (picked) halo.setAttribute('position', new Float32BufferAttribute([picked.p.x, picked.p.y, picked.p.z], 3));
    this.halo.geometry = halo;
  }

  /**
   * W1: every enabled cutting plane as its parallelogram A, B, C, A + C - B, and, off the Results
   * step (where its map is the picture), the solver's cell grid: ceil(|BC| / r) cells along BC and
   * ceil(|BA| / r) along BA (planes.ts), at most `PLANE_GRID_MAX` lines each way.
   */
  private updatePlanes(): void {
    const receivers = sceneStore.get()?.view.surface_receivers ?? [];
    const outline: number[] = [];
    const grid: number[] = [];
    const summary: typeof this.planeSummary = [];
    const onResults = stepStore.get() === 'results';
    for (const r of receivers) {
      if (!r.enabled || r.shape.kind !== 'cutting_plane') continue;
      const { a, b, c, resolution_m } = r.shape;
      if (![...a, ...b, ...c].every(finite)) continue;
      const A = new Vector3(...(a as Vec));
      const B = new Vector3(...(b as Vec));
      const C = new Vector3(...(c as Vec));
      const D = A.clone().add(C).sub(B);
      const seg = (out: number[], p: Vector3, q: Vector3) => out.push(p.x, p.y, p.z, q.x, q.y, q.z);
      seg(outline, B, C);
      seg(outline, C, D);
      seg(outline, D, A);
      seg(outline, A, B);
      const cells = planeCells(a, b, c, resolution_m);
      let lines = 0;
      if (cells && !onResults) {
        const bc = C.clone().sub(B);
        const ba = A.clone().sub(B);
        const nu = Math.min(cells.u, PLANE_GRID_MAX);
        const nv = Math.min(cells.v, PLANE_GRID_MAX);
        for (let i = 1; i < nu; i++) {
          const t = bc.clone().multiplyScalar(i / nu);
          seg(grid, B.clone().add(t), A.clone().add(t));
          lines++;
        }
        for (let j = 1; j < nv; j++) {
          const t = ba.clone().multiplyScalar(j / nv);
          seg(grid, B.clone().add(t), C.clone().add(t));
          lines++;
        }
      }
      summary.push({ name: r.name, corners: [A, B, C, D].map((p) => [p.x, p.y, p.z] as Vec), u: cells?.u ?? 0, v: cells?.v ?? 0, gridLines: lines });
    }
    for (const o of [this.planeOutline, this.planeGrid]) o.geometry.dispose();
    const g1 = new LineSegmentsGeometry();
    if (outline.length > 0) g1.setPositions(outline);
    const g2 = new LineSegmentsGeometry();
    if (grid.length > 0) g2.setPositions(grid);
    this.planeOutline.geometry = g1;
    this.planeGrid.geometry = g2;
    this.planeSummary = summary;
  }

  // ---- cameras --------------------------------------------------------------------------

  private defaultCamera(): void {
    this.persp.position.set(8, -12, 8);
    this.persp.near = 0.05;
    this.persp.far = 500;
    this.persp.lookAt(0, 0, 1.5);
    this.persp.updateProjectionMatrix();
    this.controls?.target.set(0, 0, 1.5);
    this.controls?.update();
  }

  /** Frames the model in the perspective camera, from the front left and above. */
  frame(): void {
    this.syncSize();
    const b = this.bounds;
    if (!b) {
      this.defaultCamera();
      this.invalidate();
      return;
    }
    const c = new Vector3((b.min[0] + b.max[0]) / 2, (b.min[1] + b.max[1]) / 2, (b.min[2] + b.max[2]) / 2);
    const radius = Math.max(0.5 * Math.hypot(b.max[0] - b.min[0], b.max[1] - b.min[1], b.max[2] - b.min[2]), 0.05);
    const aspect = this.persp.aspect > 0 ? this.persp.aspect : 1;
    const fovV = (this.persp.fov * Math.PI) / 180;
    const fovH = 2 * Math.atan(Math.tan(fovV / 2) * aspect);
    const dist = (radius / Math.sin(Math.min(fovV, fovH) / 2)) * 1.05;
    const dir = new Vector3(-0.5, -0.8, 0.62).normalize();
    this.persp.near = Math.max(radius * 0.002, 0.002);
    this.persp.far = Math.max(dist * 6, radius * 20);
    this.persp.position.copy(c).addScaledVector(dir, dist);
    this.persp.updateProjectionMatrix();
    if (this.controls) {
      this.controls.target.copy(c);
      this.controls.minDistance = 0;
      this.controls.maxDistance = dist * 8;
      this.controls.update();
    } else {
      this.persp.lookAt(c);
    }
    this.persp.updateMatrixWorld();
    this.invalidate();
  }

  setView(view: ViewMode): void {
    if (view === this.view) return;
    this.view = view;
    this.applyTool();
    this.setUi({ view });
    this.invalidate();
  }

  /** The perspective camera eased from where it is to `position` looking at `target` over `ms`; resolves when there. */
  private flyCamera(position: Vec, target: Vec, ms: number): Promise<boolean> {
    const controls = this.controls;
    if (!controls || this.view !== 'perspective') return Promise.resolve(false);
    const p0 = this.persp.position.clone();
    const t0 = controls.target.clone();
    const p1 = new Vector3(...position);
    const t1 = new Vector3(...target);
    const start = performance.now();
    return new Promise((resolve) => {
      const step = (now: number) => {
        const f = ms > 0 ? Math.min(1, (now - start) / ms) : 1;
        const e = f < 0.5 ? 2 * f * f : 1 - 2 * (1 - f) * (1 - f);
        this.persp.position.lerpVectors(p0, p1, e);
        controls.target.lerpVectors(t0, t1, e);
        controls.update();
        this.invalidate();
        if (f < 1) requestAnimationFrame(step);
        else resolve(true);
      };
      requestAnimationFrame(step);
    });
  }

  private cameraState() {
    const main = this.view === 'plan' ? this.plan : this.persp;
    const size = this.syncSize();
    this.mainCamera(size ? size.w / size.h : 1);
    const dir = main.getWorldDirection(new Vector3());
    const arr = (v: Vector3): Vec => [v.x, v.y, v.z];
    return {
      view: this.view,
      projection: main === this.plan ? 'orthographic' : 'perspective',
      position: arr(main.position),
      direction: arr(dir),
      target: this.controls ? arr(this.controls.target) : null,
    };
  }

  // ---- picking --------------------------------------------------------------------------

  /** The pick ray through a client point, from the camera drawn in the main view. */
  private rayAt(clientX: number, clientY: number): Ray | null {
    const c = this.canvas;
    if (!c || !this.syncSize()) return null;
    const r = c.getBoundingClientRect();
    const ndc = new Vector2(((clientX - r.left) / r.width) * 2 - 1, -((clientY - r.top) / r.height) * 2 + 1);
    const camera = this.mainCamera(r.width / r.height);
    this.raycaster.setFromCamera(ndc, camera);
    return this.raycaster.ray.clone();
  }

  /** The first face the BVH hits at a client point, BackSide as drawn. */
  private pickFace(clientX: number, clientY: number): { face: number; distance: number; origin: Vec; dir: Vec } | null {
    if (!this.bvh) return null;
    const ray = this.rayAt(clientX, clientY);
    if (!ray) return null;
    const hit = firstFace(this.bvh, ray);
    if (!hit) return null;
    return {
      face: hit.face,
      distance: hit.distance,
      origin: [ray.origin.x, ray.origin.y, ray.origin.z],
      dir: [ray.direction.x, ray.direction.y, ray.direction.z],
    };
  }

  /** The client point of a world point under the main camera, or null behind it. */
  private clientOf(p: Vector3): { x: number; y: number } | null {
    const c = this.canvas;
    const size = this.syncSize();
    if (!c || !size) return null;
    const r = c.getBoundingClientRect();
    const v = p.clone().project(this.mainCamera(r.width / r.height));
    if (!(v.z > -1 && v.z < 1)) return null;
    return { x: r.left + ((v.x + 1) / 2) * r.width, y: r.top + ((1 - v.y) / 2) * r.height };
  }

  /** A marker within its pick radius of the point, nearest first, unless a face hides it. */
  private pickMarker(clientX: number, clientY: number): Marker | null {
    let best: Marker | null = null;
    let bestD = Infinity;
    for (const m of this.markers) {
      const q = this.clientOf(m.p);
      if (!q) continue;
      const d = Math.hypot(q.x - clientX, q.y - clientY);
      if (d > MARKER_PICK_PX[m.kind] || d >= bestD) continue;
      const ray = this.rayAt(q.x, q.y);
      if (!ray) continue;
      const along = m.p.clone().sub(ray.origin).dot(ray.direction);
      const hit = this.bvh ? firstFace(this.bvh, ray) : null;
      if (hit && hit.distance < along - 1e-3) continue;
      best = m;
      bestD = d;
    }
    return best;
  }

  /** What a click at a client point takes: a marker, else a face, else nothing. */
  private pickAt(clientX: number, clientY: number): Pick {
    const marker = this.pickMarker(clientX, clientY);
    if (marker) return { kind: 'marker', marker };
    const hit = this.pickFace(clientX, clientY);
    return hit ? { kind: 'face', face: hit.face, origin: hit.origin, dir: hit.dir } : { kind: 'none' };
  }

  /** W5: the map face at a client point, unless a drawn model face is in front of it. */
  private mapFaceAt(clientX: number, clientY: number): number | null {
    if (stepStore.get() !== 'results') return null;
    const ray = this.rayAt(clientX, clientY);
    if (!ray) return null;
    const hit = this.results.pickMap(ray);
    if (!hit) return null;
    const wall = this.bvh ? firstFace(this.bvh, ray) : null;
    // A surface receiver's map lies on the model's own faces: the same distance is not in front.
    if (wall && wall.distance < hit.distance - (1e-3 + 1e-4 * hit.distance)) return null;
    return hit.face;
  }

  private probeAt(clientX: number | null, clientY: number): void {
    const face = clientX === null ? null : this.mapFaceAt(clientX, clientY);
    const cur = mapPointerStore.get();
    if (face === null || clientX === null) {
      if (cur !== null) mapPointerStore.set(null);
      return;
    }
    if (cur?.face === face && cur.x === clientX && cur.y === clientY) return;
    mapPointerStore.set({ face, x: clientX, y: clientY });
  }

  /**
   * W5's hook: a client point over map face `face`'s centroid where the probe takes exactly that
   * face (on the canvas, nothing of the model in front); null when there is none.
   */
  mapFacePoint(face: number): { x: number; y: number } | null {
    const c = this.canvas;
    const centre = this.results.faceCentroid(face);
    if (!c || !centre) return null;
    const q = this.clientOf(new Vector3(...centre));
    if (!q) return null;
    const p = { x: Math.round(q.x), y: Math.round(q.y) };
    const r = c.getBoundingClientRect();
    if (p.x < r.left + 1 || p.x > r.right - 1 || p.y < r.top + 1 || p.y > r.bottom - 1) return null;
    if (document.elementFromPoint(p.x, p.y) !== c) return null;
    return this.mapFaceAt(p.x, p.y) === face ? p : null;
  }

  /** W5's hook: a client point on the canvas where the probe takes no map face; null if none on a 24 px grid. */
  offMapPoint(): { x: number; y: number } | null {
    const c = this.canvas;
    if (!c) return null;
    const r = c.getBoundingClientRect();
    for (let y = r.top + 12; y < r.bottom - 12; y += 24) {
      for (let x = r.left + 12; x < r.right - 12; x += 24) {
        const p = { x: Math.round(x), y: Math.round(y) };
        if (document.elementFromPoint(p.x, p.y) === c && this.mapFaceAt(p.x, p.y) === null) return p;
      }
    }
    return null;
  }

  /** W5's hook: an FNV-1a hash of the frame as drawn now, to tell two looks of the map apart. */
  framePixels(): { pixels: number; hash: number } | null {
    const r = this.renderer;
    if (!r || !this.dom || this.contextLost || !this.syncSize()) return null;
    const gl = r.getContext();
    this.renderNow();
    const px = new Uint8Array(gl.drawingBufferWidth * gl.drawingBufferHeight * 4);
    gl.readPixels(0, 0, gl.drawingBufferWidth, gl.drawingBufferHeight, gl.RGBA, gl.UNSIGNED_BYTE, px);
    let h = 0x811c9dc5;
    for (let i = 0; i < px.length; i++) h = Math.imul(h ^ px[i], 0x01000193) >>> 0;
    return { pixels: px.length / 4, hash: h };
  }

  /** W9: the frame as drawn now, top row first, the GPU's premultiplied RGBA; null without a view. */
  frameRgba(): { width: number; height: number; rgba: Uint8ClampedArray } | null {
    const r = this.renderer;
    if (!r || !this.dom || this.contextLost || !this.syncSize()) return null;
    const gl = r.getContext();
    this.renderNow();
    const w = gl.drawingBufferWidth;
    const h = gl.drawingBufferHeight;
    const px = new Uint8Array(w * h * 4);
    gl.readPixels(0, 0, w, h, gl.RGBA, gl.UNSIGNED_BYTE, px);
    return { width: w, height: h, rgba: flipRows(px, w, h) };
  }

  private onClick(x: number, y: number): void {
    const tool = toolStore.get();
    if (tool === 'orbit') return;
    if (tool === 'place-receiver' || tool === 'place-source') {
      this.place(tool === 'place-receiver' ? 'receiver' : 'source', x, y);
      return;
    }
    const pick = this.pickAt(x, y);
    if (pick.kind === 'marker') selectionStore.set({ kind: pick.marker.kind, id: pick.marker.id });
    else if (pick.kind === 'face') selectionStore.set(this.facesSelection([pick.face]));
    else selectionStore.set({ kind: 'none' });
  }

  /** Double-click: the clicked face's whole flat surface (upstream's coplanar fill). */
  private onDoubleClick(x: number, y: number): void {
    if (toolStore.get() !== 'select' || !this.topo) return;
    const pick = this.pickAt(x, y);
    if (pick.kind !== 'face') return;
    selectionStore.set(this.facesSelection(coplanarFaces(this.topo, pick.face)));
  }

  private place(kind: MarkerKind, x: number, y: number): void {
    const mesh = this.mesh;
    const hit = this.pickFace(x, y);
    if (!mesh || !hit) {
      this.notify(`Click a floor inside the room to place a ${kind}.`);
      return;
    }
    const normal = this.topo ? (Array.from(this.topo.normals.subarray(3 * hit.face, 3 * hit.face + 3)) as Vec) : faceNormalOf(mesh.positions, mesh.indices, hit.face);
    if (!isFloorLike(normal)) {
      this.notify(`Not a floor. Click a surface that faces up to place a ${kind}.`);
      return;
    }
    const floor = rayOnFacePlane(mesh.positions, mesh.indices, hit.face, hit.origin, hit.dir);
    if (!floor) return;
    const point = placementPoint(floor, actions.PLACE_HEIGHT_M[kind]);
    actions.fire(
      actions.placeAt(kind, point).then((outcome) => {
        if (!outcome.applied) return;
        const view = outcome.state.view;
        const list = kind === 'receiver' ? view.point_receivers : view.sources;
        const added = list[list.length - 1];
        if (added) selectionStore.set({ kind, id: added.id });
      }),
    );
  }

  private notify(text: string): void {
    window.clearTimeout(this.noticeTimer);
    this.setUi({ notice: text });
    this.noticeTimer = window.setTimeout(() => this.setUi({ notice: null }), 2600);
  }

  private applyTool(): void {
    const tool = toolStore.get();
    if (this.controls) this.controls.enabled = this.view === 'perspective';
    if (this.canvas) this.canvas.style.cursor = tool.startsWith('place-') ? 'crosshair' : tool === 'orbit' ? 'grab' : 'default';
    if (!tool.startsWith('place-')) this.setUi({ notice: null });
  }

  // ---- the test hooks -------------------------------------------------------------------

  /**
   * The rounded client point of face `f`'s centroid under the current camera, or null unless a
   * click there would take exactly that face: inside the canvas, on the canvas (no overlay on
   * top), no marker in the way, and the BVH's first hit is `f`.
   */
  private faceClientPoint(f: number): { x: number; y: number } | null {
    const mesh = this.mesh;
    const c = this.canvas;
    if (!mesh || !c || !(f >= 0 && f < mesh.faceCount)) return null;
    const q = this.clientOf(new Vector3(...faceCentroid(mesh.positions, mesh.indices, f)));
    if (!q) return null;
    const p = { x: Math.round(q.x), y: Math.round(q.y) };
    const r = c.getBoundingClientRect();
    if (p.x < r.left + 1 || p.x > r.right - 1 || p.y < r.top + 1 || p.y > r.bottom - 1) return null;
    if (document.elementFromPoint(p.x, p.y) !== c) return null;
    const pick = this.pickAt(p.x, p.y);
    return pick.kind === 'face' && pick.face === f ? p : null;
  }

  /**
   * Puts the perspective camera on face `f`'s visible (interior) side, aimed at its centroid,
   * and returns the centroid's client point; null unless the BVH's first hit there is `f`.
   * Tries a few distances and tilts off the normal, nearest last.
   */
  private aimAtFace(f: number): { x: number; y: number } | null {
    const mesh = this.mesh;
    if (!mesh || !this.controls || !(f >= 0 && f < mesh.faceCount)) return null;
    this.setView('perspective');
    const n = new Vector3(...faceNormalOf(mesh.positions, mesh.indices, f));
    if (n.lengthSq() === 0) return null;
    const centre = new Vector3(...faceCentroid(mesh.positions, mesh.indices, f));
    const b = this.bounds;
    const radius = b ? Math.max(0.5 * Math.hypot(b.max[0] - b.min[0], b.max[1] - b.min[1], b.max[2] - b.min[2]), 0.05) : 5;
    const side = new Vector3().crossVectors(n, Math.abs(n.z) < 0.9 ? new Vector3(0, 0, 1) : new Vector3(1, 0, 0)).normalize();
    for (const tilt of [12, 28, 45]) {
      const t = (tilt * Math.PI) / 180;
      const toCamera = n.clone().multiplyScalar(-Math.cos(t)).addScaledVector(side, Math.sin(t)).normalize();
      for (const k of [1.6, 0.9, 0.45, 0.2, 0.08]) {
        const d = Math.max(radius * k, 0.3);
        this.persp.position.copy(centre).addScaledVector(toCamera, d);
        this.controls.target.copy(centre);
        this.controls.update();
        this.persp.updateMatrixWorld();
        const p = this.faceClientPoint(f);
        if (p) {
          this.renderNow();
          return p;
        }
      }
    }
    this.renderNow();
    return null;
  }

  /**
   * What the check-highlight overlay changes on screen. The view is drawn as the app draws it
   * and read back, drawn again with the overlay hidden and read back, then drawn as before; all
   * in one task, so the hidden frame is never shown. `changed` counts the pixels that differ.
   * `warn` counts those the overlay moved towards the warn colour: the change is a multiple
   * between 0.2 and 1.05 of (warn − the pixel without it), within 16 levels. An overlay that is
   * hidden, transparent, recoloured, rejected by the depth test or not in the scene counts 0.
   * Null without a live renderer.
   */
  /**
   * M12 P3: the pixels the surface map changes on screen, drawn as the app draws it and again
   * with the map hidden, in one task (as `highlightPixels`). A map whose shader does not compile,
   * or that draws nothing, changes 0.
   */
  mapPixels(): { pixels: number; changed: number } | null {
    const r = this.renderer;
    if (!r || !this.dom || this.contextLost || !this.syncSize()) return null;
    const gl = r.getContext();
    const read = () => {
      const px = new Uint8Array(gl.drawingBufferWidth * gl.drawingBufferHeight * 4);
      gl.readPixels(0, 0, gl.drawingBufferWidth, gl.drawingBufferHeight, gl.RGBA, gl.UNSIGNED_BYTE, px);
      return px;
    };
    this.renderNow();
    const drawn = read();
    const restore = this.results.hideMapFor();
    try {
      this.renderNow();
    } finally {
      restore();
    }
    const bare = read();
    this.renderNow();
    let changed = 0;
    for (let i = 0; i < drawn.length; i += 4) {
      if (Math.max(Math.abs(drawn[i] - bare[i]), Math.abs(drawn[i + 1] - bare[i + 1]), Math.abs(drawn[i + 2] - bare[i + 2])) > 2) changed++;
    }
    return { pixels: drawn.length / 4, changed };
  }

  private highlightPixels(): { pixels: number; changed: number; warn: number } | null {
    const r = this.renderer;
    if (!r || !this.dom || this.contextLost || !this.syncSize()) return null;
    const gl = r.getContext();
    const read = () => {
      const px = new Uint8Array(gl.drawingBufferWidth * gl.drawingBufferHeight * 4);
      gl.readPixels(0, 0, gl.drawingBufferWidth, gl.drawingBufferHeight, gl.RGBA, gl.UNSIGNED_BYTE, px);
      return px;
    };
    this.renderNow();
    const drawn = read();
    const was = this.highlight.visible;
    this.highlight.visible = false;
    try {
      this.renderNow();
    } finally {
      this.highlight.visible = was;
    }
    const bare = read();
    this.renderNow();
    if (drawn.length !== bare.length) return null;

    const warn = [(WARN >> 16) & 0xff, (WARN >> 8) & 0xff, WARN & 0xff];
    let changed = 0;
    let toward = 0;
    for (let i = 0; i < drawn.length; i += 4) {
      const d0 = drawn[i] - bare[i];
      const d1 = drawn[i + 1] - bare[i + 1];
      const d2 = drawn[i + 2] - bare[i + 2];
      if (Math.max(Math.abs(d0), Math.abs(d1), Math.abs(d2)) <= 2) continue;
      changed++;
      const t0 = warn[0] - bare[i];
      const t1 = warn[1] - bare[i + 1];
      const t2 = warn[2] - bare[i + 2];
      const tt = t0 * t0 + t1 * t1 + t2 * t2;
      // Under a pixel already close to the warn colour a change cannot be told apart.
      if (tt < 40 * 40) continue;
      const a = (d0 * t0 + d1 * t1 + d2 * t2) / tt;
      const e0 = d0 - a * t0;
      const e1 = d1 - a * t1;
      const e2 = d2 - a * t2;
      if (a >= 0.2 && a <= 1.05 && e0 * e0 + e1 * e1 + e2 * e2 <= 16 * 16) toward++;
    }
    return { pixels: drawn.length / 4, changed, warn: toward };
  }

  private setUi(patch: Partial<ViewportUi>): void {
    const cur = viewportUi.get();
    if ((Object.keys(patch) as (keyof ViewportUi)[]).every((k) => cur[k] === patch[k])) return;
    viewportUi.set({ ...cur, ...patch });
  }
}

const engine = new ViewportEngine();

/** Mounts the one 3D view into the viewport's DOM; returns the unmount. */
export function attachViewport(dom: ViewportDom): () => void {
  return engine.attach(dom);
}

/** The main view: perspective, or the plan's top camera. View › Perspective / Plan may call it. */
export function setView(view: ViewMode): void {
  engine.setView(view);
}

/** The Results step's map: `m` drawn with `meta` (a difference when `base` is given); null clears it. Returns why it cannot be drawn, or null. */
export function showMap(m: SurfaceMap | null, meta?: MapMeta, base: SurfaceMap | null = null): string | null {
  const err = m && meta ? engine.results.setMap(m, meta, base) : (engine.results.clearMap(), null);
  engine.results.setStep(animatorStore.get().step);
  engine.invalidate();
  return err;
}

/** The Results step's particles; null clears them. */
export function showParticles(p: Particles | null, meta?: ParticleMeta): void {
  engine.results.setParticles(p, meta ?? null);
  engine.results.setStep(animatorStore.get().step);
  engine.invalidate();
}

/** What the surface map changes on screen (the m12 pixel hook). */
export function mapPixels(): { pixels: number; changed: number } | null {
  return engine.mapPixels();
}

/** W5's hook: a client point where the probe takes map face `face`, or null. */
export function mapFacePoint(face: number): { x: number; y: number } | null {
  return engine.mapFacePoint(face);
}

export const offMapPoint = () => engine.offMapPoint();
export const framePixels = () => engine.framePixels();
/** W9: the frame as drawn, top row first (export/snapshot.ts composites and encodes it). */
export const frameRgba = () => engine.frameRgba();

/** The layer itself, for the M12 test hooks (ResultsOverlay.tsx). */
export const resultsLayer = (): ResultsLayer => engine.results;

/** Draws now (a test hook's read must see the current step). */
export function renderNow(): void {
  engine.invalidate();
}

/** Frames the model in the perspective view (View › Frame model may call it). */
export function frameModel(): void {
  engine.frame();
}
