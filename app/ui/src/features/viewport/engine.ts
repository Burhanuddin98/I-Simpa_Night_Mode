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
import type { MeshBVH } from 'three-mesh-bvh';
import * as actions from '../../actions';
import type { SceneMesh } from '../../mesh';
import type { Particles, SurfaceMap } from '../../resultsData';
import { log, meshStore, sceneStore, selectionStore, stepStore, Store, toolStore, viewportStore, type Selection } from '../../store';
import { registerHook } from '../../testhooks';
import { buildTopology, coplanarFaces, faceNormalOf, type FaceTopology } from './floodfill';
import { animatorStore } from './animator';
import { firstFace, modelGeometry, pickingBvh } from './pick';
import { ResultsLayer, type MapMeta, type ParticleMeta } from './resultsLayer';
import { faceBounds, faceCentroid, fitOrtho, gizmoAxes, isFloorLike, placementPoint, planDimensions, rayOnFacePlane, type Box, type Vec } from './geometry';
import { BG, glowPixels, RED, ringPixels, WHITE } from './sprites';

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

export const viewportUi = new Store<ViewportUi>({
  view: 'perspective',
  hasModel: false,
  highlightCount: 0,
  planLabel: null,
  notice: null,
  error: null,
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
const FACE = 0x19191d;
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
  private readonly edges: LineSegments;
  private readonly highlight: Mesh;
  private readonly selectionWash: Mesh;
  private readonly selectionEdges: LineSegments;
  private readonly sourcePoints: Points;
  private readonly receiverPoints: Points;
  private readonly halo: Points;
  private readonly sourceStems: LineSegments;
  private readonly receiverStems: LineSegments;

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
    this.faces = new Mesh(
      new BufferGeometry(),
      new MeshLambertMaterial({ color: FACE, side: BackSide, flatShading: true, polygonOffset: true, polygonOffsetFactor: 1, polygonOffsetUnits: 1 }),
    );
    this.edges = new LineSegments(new BufferGeometry(), new LineBasicMaterial({ color: LINE, transparent: true, opacity: 0.3, depthWrite: false }));
    this.highlight = new Mesh(
      new BufferGeometry(),
      new MeshBasicMaterial({ color: WARN, transparent: true, opacity: 0.55, side: DoubleSide, depthWrite: false }),
    );
    this.selectionWash = new Mesh(
      new BufferGeometry(),
      new MeshBasicMaterial({ color: SELECT, transparent: true, opacity: 0.3, side: BackSide, depthWrite: false }),
    );
    this.selectionEdges = new LineSegments(new BufferGeometry(), new LineBasicMaterial({ color: SELECT, transparent: true, opacity: 1 }));
    this.sourcePoints = new Points(new BufferGeometry(), markerMaterial(glowPixels(64), 64, SOURCE_PX));
    this.receiverPoints = new Points(new BufferGeometry(), markerMaterial(ringPixels(32, 0.55, 0.85, WHITE, BG), 32, RECEIVER_PX));
    this.halo = new Points(new BufferGeometry(), markerMaterial(ringPixels(64, 0.78, 0.92, RED, null), 64, HALO_PX));
    this.sourceStems = new LineSegments(new BufferGeometry(), new LineBasicMaterial({ color: SELECT, transparent: true, opacity: 0.9 }));
    this.receiverStems = new LineSegments(new BufferGeometry(), new LineBasicMaterial({ color: LINE, transparent: true, opacity: 0.5 }));
    const order: [{ renderOrder: number }, number][] = [
      [this.faces, 0],
      [this.highlight, 1],
      [this.selectionWash, 2],
      [this.edges, 3],
      [this.selectionEdges, 4],
      [this.sourceStems, 5],
      [this.receiverStems, 5],
      [this.receiverPoints, 6],
      [this.sourcePoints, 7],
      [this.halo, 8],
    ];
    for (const [o, n] of order) o.renderOrder = n;
    this.scene.add(
      this.faces,
      this.highlight,
      this.selectionWash,
      this.edges,
      this.selectionEdges,
      this.sourceStems,
      this.receiverStems,
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
      toolStore.subscribe(() => this.applyTool()),
      stepStore.subscribe(() => {
        this.results.setShown(stepStore.get() === 'results');
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
      // Gate (a): what the check-highlight overlay puts on screen, not what was uploaded to it.
      registerHook('highlightPixels', () => this.highlightPixels()),
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
    });
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
      const main = this.mainCamera(w, h);
      this.setMarkerScale(1);
      r.render(this.scene, main);
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
    r.setClearColor(PANEL, 0.55);
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

  /** The region the floating panels leave clear (`dom.root`, decision-log row 50), in the
   * canvas's pixels; the whole canvas when it cannot be read. */
  private clearRegion(w: number, h: number): { x: number; y: number; w: number; h: number } {
    const c = this.canvas?.getBoundingClientRect();
    const s = this.dom?.root.getBoundingClientRect();
    if (!c || !s || s.width < 2 || s.height < 2) return { x: 0, y: 0, w, h };
    return { x: s.left - c.left, y: s.top - c.top, w: s.width, h: s.height };
  }

  /** The main camera for a `w` x `h` canvas, its axis through the centre of the clear region. */
  private mainCamera(w: number, h: number): Camera {
    const s = this.clearRegion(w, h);
    if (this.view === 'plan') {
      this.fitPlan(s.w / s.h, { full: [w, h], region: s });
      return this.plan;
    }
    const ox = w / 2 - (s.x + s.w / 2);
    const oy = h / 2 - (s.y + s.h / 2);
    const v = this.persp.view;
    if (!v || v.fullWidth !== w || v.fullHeight !== h || v.offsetX !== ox || v.offsetY !== oy) this.persp.setViewOffset(w, h, ox, oy, w, h);
    this.persp.updateMatrixWorld();
    return this.persp;
  }

  /** The top camera, fitted to the model's plan at `aspect`; with `on`, fitted to the clear
   * region of a full canvas and centred on it (the Plan tab), else to its whole box (the inset). */
  private fitPlan(aspect: number, on?: { full: [number, number]; region: { x: number; y: number; w: number; h: number } }): void {
    const b = this.bounds ?? { min: [-5, -5, 0], max: [5, 5, 3] };
    const cx = (b.min[0] + b.max[0]) / 2;
    const cy = (b.min[1] + b.max[1]) / 2;
    const ez = b.max[2] - b.min[2];
    const fit = fitOrtho(b.max[0] - b.min[0], b.max[1] - b.min[1], aspect);
    const p = this.plan;
    let halfW = fit.halfW;
    let halfH = fit.halfH;
    if (on) {
      const [w, h] = on.full;
      const s = on.region;
      halfW *= w / s.w;
      halfH *= h / s.h;
      p.setViewOffset(w, h, w / 2 - (s.x + s.w / 2), h / 2 - (s.y + s.h / 2), w, h);
    } else {
      p.clearViewOffset();
    }
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
      this.edges.geometry = new EdgesGeometry(geometry, 1);
    }
    this.builtRev = mesh?.geometryRev ?? null;
    // A face selection names faces of the geometry it was made on.
    if (previousRev !== this.builtRev && selectionStore.get().kind === 'faces') selectionStore.set({ kind: 'none' });
    this.frame();
    this.onScene();
  }

  private disposeModel(): void {
    for (const o of [this.faces, this.edges, this.highlight, this.selectionWash, this.selectionEdges]) {
      o.geometry.dispose();
      o.geometry = new BufferGeometry();
    }
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

  private onScene(): void {
    this.updateHighlight();
    this.updateSelection();
    this.updateMarkers();
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
    this.selectionEdges.geometry = faces.length > 0 ? new EdgesGeometry(g, 1) : new BufferGeometry();
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
    // Fitted to the clear region (decision-log row 50): its share of the view's height and width.
    const cw = this.canvas?.clientWidth ?? 0;
    const ch = this.canvas?.clientHeight ?? 0;
    const s = cw > 0 && ch > 0 ? this.clearRegion(cw, ch) : null;
    const t = Math.tan((this.persp.fov * Math.PI) / 360);
    const aspect = this.persp.aspect > 0 ? this.persp.aspect : 1;
    const fovV = 2 * Math.atan(t * (s ? s.h / ch : 1));
    const fovH = 2 * Math.atan(t * aspect * (s ? s.w / cw : 1));
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

  private cameraState() {
    const main = this.view === 'plan' ? this.plan : this.persp;
    const size = this.syncSize();
    if (size) this.mainCamera(size.w, size.h);
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
    const camera = this.mainCamera(r.width, r.height);
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
    const v = p.clone().project(this.mainCamera(r.width, r.height));
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
