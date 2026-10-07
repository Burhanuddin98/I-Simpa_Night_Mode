// The 3D view's engine (PLAN.md 6.1): the one renderer and its canvas, created on the first mount
// and kept for the app's lifetime (PLAN.md 2.4, rule 3), rendering on demand.
//
//   - Decision 68 (b): the renderer is three.js's WebGPURenderer, one GPU context: WebGPU where the
//     WebView has it, WebGL2 where it does not (three falls back by itself; `nm.forceWebGL` in
//     localStorage forces the fallback for testing). Its start is asynchronous (`init()`); the view
//     draws from the first frame after. Every material is a node material (TSL), so both backends
//     draw the same picture; colour is converted to sRGB inside each material as WebGLRenderer did
//     (nodes.ts), so blending and every pixel stay as they were.
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
  AddEquation,
  AlwaysDepth,
  AmbientLight,
  CustomBlending,
  HalfFloatType,
  OneFactor,
  RenderTarget,
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
  InstancedBufferAttribute,
  LinearFilter,
  LinearSRGBColorSpace,
  LineSegments,
  Mesh,
  NoBlending,
  NoToneMapping,
  OrthographicCamera,
  PerspectiveCamera,
  PlaneGeometry,
  Ray,
  REVISION,
  Raycaster,
  RGBAFormat,
  Scene,
  Sprite,
  SRGBColorSpace,
  Vector2,
  Vector3,
  type Camera,
} from 'three';
import {
  LineBasicNodeMaterial,
  MeshBasicNodeMaterial,
  MeshLambertNodeMaterial,
  MeshMatcapNodeMaterial,
  PointsNodeMaterial,
  QuadMesh,
  WebGPURenderer,
} from 'three/webgpu';
import { T } from './tsl';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import { LineSegments2 } from 'three/addons/lines/webgpu/LineSegments2.js';
import type { MeshBVH } from 'three-mesh-bvh';
import * as actions from '../../actions';
import type { SceneMesh } from '../../mesh';
import type { Particles, SurfaceMap } from '../../resultsData';
import { effectiveMaterial } from '../../chrome/sceneModel';
import { log, meshStore, sceneStore, selectionStore, stepStore, Store, toolStore, viewportStore, type Selection } from '../../store';
import { registerHook } from '../../testhooks';
import { buildTopology, coplanarFaces, faceNormalOf, type FaceTopology } from './floodfill';
import { bgraToRgba, flipRows, unpadRows } from './snapshot';
import { animatorStore } from './animator';
import { firstFace, modelGeometry, pickingBvh } from './pick';
import { ResultsLayer, type MapMeta, type ParticleMeta } from './resultsLayer';
import { faceBounds, faceCentroid, fitOrtho, gizmoAxes, isFloorLike, placementPoint, planDimensions, rayOnFacePlane, type Box, type Vec } from './geometry';
import { BG, glowPixels, RED, ringPixels, WHITE } from './sprites';
import { GLOW_MAX, GLOW_RGB, glowLevel, glowRadius, glowTerm, pulsePhase, spriteScale, STILL_PHASE } from './glow';
import { aoFactor, aoReach, bakeAo } from './ao';
import { FADE_BG, FADE_MAX, fadeAmount, fadeRange } from './fade';
import { glassOpacity } from './glass';
import { BUILD_MS, buildHeight, buildKeep, buildLine, NO_CUT, swingAngle } from './build';
import { groundColour, groundLayout } from './ground';
import { emptyFat, emptyGeometry, FatLineMaterial, inSrgb, rawColor } from './nodes';
import { bloom } from 'three/addons/tsl/display/BloomNode.js';
import type { ParticleLook } from './rays';
import { planeCells, roomBox } from '../../chrome/planes';
import { dimensionLines } from './dims';

const { acesFilmicToneMapping, clamp, Fn, max, screenUV, texture, float, instancedBufferAttribute, materialOpacity, mix, output, positionGeometry, sRGBTransferOETF, uniform, uniformArray, vec3, vec4 } = T;

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
  /** Why the 3D view cannot draw (no GPU renderer could start). */
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

/** Round 2: the presentation view (no panels) and its turntable camera. */
export const presentStore = new Store<{ on: boolean; turntable: boolean }>({ on: false, turntable: false });
/** The benchmark's replication in force (the card says so in words), or null: the real saved particles. */
export const replicaStore = new Store<{ saved: number; copies: number } | null>(null);
/** The turntable's speed, degrees a second. */
const TURN_DEG_S = 6;

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
  /** Corner shading (ao.ts) on. */
  corners: boolean;
  /** Distance fade (fade.ts) on. */
  fade: boolean;
  /** The floor grid and shadow under the room (ground.ts) on. */
  ground: boolean;
  /** The dimensions overlay (dims.ts) on: length, width and height beside the room. */
  dims: boolean;
}
const STYLE_KEY = 'nm.viewStyle';
const DEFAULT_STYLE: ViewStyle = { surfaces: 'colour', edges: 'all', glass: 15, corners: true, fade: true, ground: true, dims: false };
function loadStyle(): ViewStyle {
  try {
    const v = JSON.parse(localStorage.getItem(STYLE_KEY) ?? 'null') as Partial<ViewStyle> | null;
    if (!v) return DEFAULT_STYLE;
    return {
      surfaces: (['colour', 'grey', 'glass', 'wire'] as const).includes(v.surfaces as SurfaceStyle) ? (v.surfaces as SurfaceStyle) : DEFAULT_STYLE.surfaces,
      edges: v.edges === 'feature' ? 'feature' : 'all',
      glass: typeof v.glass === 'number' ? Math.min(60, Math.max(0, v.glass)) : DEFAULT_STYLE.glass,
      corners: v.corners !== false,
      fade: v.fade !== false,
      ground: v.ground !== false,
      dims: v.dims === true,
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
const PANEL = 0x131010;
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

/**
 * Markers drawn as sprites of a fixed size on screen, one instance a marker (WebGPU draws points one pixel
 * wide, so WebGL's sized points are instanced quads here): the DataTexture sprite in sRGB, as PointsMaterial
 * drew it. The positions live in one instanced buffer, grown when a project has more markers.
 */
class MarkerSprites {
  readonly sprite: Sprite;
  readonly material: PointsNodeMaterial;
  private positions = new InstancedBufferAttribute(new Float32Array(3 * 16), 3);

  constructor(pixels: Uint8Array, texSize: number, px: number) {
    this.material = inSrgb(new PointsNodeMaterial({ size: px, sizeAttenuation: false, map: dataTexture(pixels, texSize), transparent: true, depthWrite: false, alphaTest: 0.01 }));
    this.material.alphaToCoverage = false;
    this.material.positionNode = instancedBufferAttribute(this.positions);
    this.sprite = new Sprite(this.material);
    this.sprite.frustumCulled = false;
    this.sprite.count = 0;
    this.sprite.visible = false;
  }

  /** The markers at `xyz` (three numbers each). */
  set(xyz: number[]): void {
    const n = xyz.length / 3;
    if (n > this.positions.count) {
      this.positions = new InstancedBufferAttribute(new Float32Array(3 * Math.max(n, 2 * this.positions.count)), 3);
      this.material.positionNode = instancedBufferAttribute(this.positions);
      this.material.needsUpdate = true;
    }
    (this.positions.array as Float32Array).set(xyz);
    this.positions.needsUpdate = true;
    this.sprite.count = n;
    this.sprite.visible = n > 0;
  }
}

/** The surface and edge materials' shared uniforms: corner shading (ao.ts), the build-up (build.ts), the distance fade (fade.ts) and the sources' glow (glow.ts). */
function sharedUniforms() {
  return {
    nmAoMix: uniform(1),
    /** Decision 69: the room dimmed (0 to 1) while the particles' light plays, so the light carries the image. */
    nmDim: uniform(0),
    nmBuildZ: uniform(NO_CUT),
    nmBuildBand: uniform(0.3),
    nmFadeNear: uniform(0),
    nmFadeFar: uniform(1),
    nmFadeMax: uniform(0),
    nmFadeColor: uniform(new Color(FADE_BG)),
    glowPos: uniformArray(Array.from({ length: GLOW_MAX }, () => new Vector3()), 'vec3'),
    glowCount: uniform(0, 'int'),
    glowRadius: uniform(1.5),
    glowLevel: uniform(0),
    glowColor: uniform(new Vector3(...GLOW_RGB)),
  };
}
type Shared = ReturnType<typeof sharedUniforms>;

// The few WebGPU objects the frame read-back touches (the app's TypeScript has no WebGPU types).
const GPU_MAP_READ = 0x0001;
const GPU_COPY_DST = 0x0008;
const GPU_MAP_MODE_READ = 0x0001;
interface GpuBuffer {
  mapAsync(mode: number): Promise<void>;
  getMappedRange(): ArrayBuffer;
  destroy(): void;
}
interface GpuTexture {
  width: number;
  height: number;
  format: string;
}
interface GpuDevice {
  createBuffer(d: { size: number; usage: number }): GpuBuffer;
  createCommandEncoder(): { copyTextureToBuffer(src: { texture: GpuTexture }, dst: { buffer: GpuBuffer; bytesPerRow: number }, size: number[]): void; finish(): unknown };
  queue: { submit(buffers: unknown[]): void };
}
interface GpuCanvas {
  getCurrentTexture(): GpuTexture;
}

/** The WebGL2 fallback forced, for testing it on a machine with WebGPU: `localStorage['nm.forceWebGL'] = '1'`, then reload. */
function forceWebGL(): boolean {
  try {
    return localStorage.getItem('nm.forceWebGL') === '1';
  } catch {
    return false;
  }
}

/** GPU timestamps for the benchmark hook: `localStorage['nm.gpuTiming'] = '1'`, then reload (off by default: it queries every pass). */
function gpuTiming(): boolean {
  try {
    return localStorage.getItem('nm.gpuTiming') === '1';
  } catch {
    return false;
  }
}

/** How strongly the particles' light blooms (decision 69: "so it can be toned down for measurement"). */
export type Glow = 'off' | 'soft' | 'full';
export const GLOW_STRENGTH: Record<Glow, number> = { off: 0, soft: 0.6, full: 1.2 };

/**
 * The adapter's own limits for what the view needs beyond WebGPU's defaults (a map texture wider than
 * 8192, large particle buffers, big compute workgroups); undefined without WebGPU (the fallback).
 */
async function gpuLimits(): Promise<Record<string, number> | undefined> {
  const gpu = (navigator as unknown as { gpu?: { requestAdapter(o: object): Promise<{ limits: Record<string, number> } | null> } }).gpu;
  if (!gpu || forceWebGL()) return undefined;
  const adapter = await gpu.requestAdapter({ powerPreference: 'high-performance' });
  if (!adapter) return undefined;
  const want = ['maxTextureDimension2D', 'maxBufferSize', 'maxStorageBufferBindingSize', 'maxComputeWorkgroupSizeX', 'maxComputeInvocationsPerWorkgroup', 'maxComputeWorkgroupStorageSize'];
  return Object.fromEntries(want.map((k) => [k, adapter.limits[k]]));
}

/** The glow's pulse holds still under prefers-reduced-motion, and under WebDriver, where the gates compare frames. */
function stillMotion(): boolean {
  try {
    return navigator.webdriver || matchMedia('(prefers-reduced-motion: reduce)').matches;
  } catch {
    return false;
  }
}

/**
 * `m` with the corner shading (ao.ts), the sources' glow (glow.ts), the build-up's line (build.ts) and
 * the distance fade (fade.ts) in its colour, in that order, then converted to sRGB as WebGLRenderer
 * did; fragments above the build-up's cut are discarded.
 */
function shaded<M extends { maskNode: unknown; outputNode: unknown; opacityNode: unknown }>(m: M, u: Shared): M {
  m.maskNode = buildKeep(u.nmBuildZ);
  // Ghosted while the light plays (engine.setGhosted makes the surfaces see-through): 10 % at full dim.
  m.opacityNode = materialOpacity.mul(float(1).sub(u.nmDim.mul(0.9)));
  const lit = output.rgb
    .mul(aoFactor(u.nmAoMix))
    .add(glowTerm({ pos: u.glowPos, count: u.glowCount, radius: u.glowRadius, level: u.glowLevel, color: u.glowColor }))
    .add(buildLine(u.nmBuildZ, u.nmBuildBand))
    .mul(float(1).sub(u.nmDim.mul(0.75)));
  const faded = mix(lit, u.nmFadeColor, fadeAmount({ near: u.nmFadeNear, far: u.nmFadeFar, max: u.nmFadeMax }));
  m.outputNode = vec4(sRGBTransferOETF(faded), output.a);
  return m;
}

/**
 * The floating panels' boxes, in client pixels (the canvas fills the window, so these are canvas
 * pixels too). The panels are see-through glass: a label under one would read as the panel's own
 * text, so labels there are hidden (the UI study's increment 1, "marker labels ... with a mask").
 */
const PANELS = '.menubar, .stepbar, .statusbar, .scene, .props, .dock, .plan-inset, .viewport .float-panel';
function panelRects(): DOMRect[] {
  return Array.from(document.querySelectorAll(PANELS), (e) => e.getBoundingClientRect()).filter((r) => r.width > 0 && r.height > 0);
}
/** A label's size, measured once per text (0 x 0 until it has been shown once: then its anchor point alone is tested). */
const labelSizes = new WeakMap<HTMLElement, { w: number; h: number; text: string }>();
function labelSize(el: HTMLElement): { w: number; h: number } {
  const known = labelSizes.get(el);
  if (known && known.text === el.textContent) return known;
  const m = { w: el.offsetWidth, h: el.offsetHeight, text: el.textContent ?? '' };
  if (m.w > 0) labelSizes.set(el, m);
  return m;
}
/** Whether the box at `left`, `top`, `w` by `h` touches any of `rects`. */
function underPanel(left: number, top: number, w: number, h: number, rects: DOMRect[]): boolean {
  return rects.some((r) => left <= r.right && left + w >= r.left && top <= r.bottom && top + h >= r.top);
}

/** The edges with the distance fade taken from their opacity (fade.ts), cut by the build-up, in sRGB. */
function fadingLines(m: LineBasicNodeMaterial, u: Shared): LineBasicNodeMaterial {
  m.maskNode = buildKeep(u.nmBuildZ);
  m.opacityNode = materialOpacity.mul(float(1).sub(fadeAmount({ near: u.nmFadeNear, far: u.nmFadeFar, max: u.nmFadeMax }))).mul(float(1).sub(u.nmDim.mul(0.6)));
  return inSrgb(m);
}

/**
 * The surfaces' geometry before a model: empty, but with every attribute their materials read (the face
 * colours, the corner shading). A node material is built for the attributes its first geometry has, and a
 * build without `nmAo` reads it as 0: every surface black (seen on the WebGL2 fallback).
 */
function emptyTint(): BufferGeometry {
  const g = emptyGeometry();
  g.setAttribute('color', new Float32BufferAttribute(new Float32Array(0), 3));
  g.setAttribute('nmAo', new Float32BufferAttribute(new Float32Array(0), 1));
  return g;
}

/** Faces baked per frame: about 15 ms of rays on Grace, so the view stays live while a hall bakes. */
const AO_SLICE = 220;

class ViewportEngine {
  private renderer: WebGPURenderer | null = null;
  private canvas: HTMLCanvasElement | null = null;
  private contextLost = false;
  /** Which backend the renderer runs on, once it has started. */
  private backend: 'webgpu' | 'webgl2' | null = null;
  /**
   * Decision 69's light: the GPU particle looks drawn additively into a half-float target with the room's
   * depth (`fxDepth`, the picked faces' geometry, depth only), bloomed, tone mapped (ACES, black stays
   * black) and added onto the frame. Only while a GPU look is on; otherwise nothing of it runs.
   */
  private readonly fxScene = new Scene();
  private readonly fxDepth: Mesh;
  private fxTarget: RenderTarget | null = null;
  private readonly fxInput = texture(new DataTexture(new Uint8Array(4), 1, 1));
  private readonly fxStrength = uniform(GLOW_STRENGTH.soft);
  private fxGlow: Glow = 'soft';
  /** The composite: a full-view quad in a scene of its own, drawn like the inset's background (a QuadMesh pass on the canvas lost the frame). */
  private fxBloomed: Scene | null = null;
  private fxPlain: Scene | null = null;
  /** The bloom, drawn into its own target first (its node renders passes of its own, which must not run inside the frame's). */
  private fxBloomPass: QuadMesh | null = null;
  private fxBloomTarget: RenderTarget | null = null;
  private readonly fxBloomInput = texture(new DataTexture(new Uint8Array(4), 1, 1));
  private dimHold = false;
  private dimAt = 0;
  private ghosted = false;
  private mapFullWhilePlaying = false;
  private turnFrame = 0;
  /** The inset's background: a quad that replaces colour and depth inside the scissor (WebGPU clears whole attachments only). */
  private readonly insetClear = new Scene();
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
  private readonly tintColour: MeshMatcapNodeMaterial;
  private readonly tintGrey: MeshMatcapNodeMaterial;
  private readonly tintWire: MeshLambertNodeMaterial;
  private triangleEdges: BufferGeometry = emptyGeometry();
  private featureEdges: BufferGeometry = emptyGeometry();
  private readonly edges: LineSegments;
  private readonly highlight: Mesh;
  private readonly selectionWash: Mesh;
  private readonly selectionEdges: LineSegments2;
  private readonly sourcePoints: MarkerSprites;
  private readonly receiverPoints: MarkerSprites;
  private readonly halo: MarkerSprites;
  private readonly sourceStems: LineSegments;
  private readonly receiverStems: LineSegments;
  /** W1: each cutting plane's outline, and its cell grid off the Results step (upstream's DrawPlan). */
  private readonly planeOutline: LineSegments2;
  private readonly planeGrid: LineSegments2;
  private planeSummary: { name: string; corners: Vec[]; u: number; v: number; gridLines: number }[] = [];
  /**
   * The surface and edge materials' shared uniforms: the corner shading's switch (ao.ts), the
   * distance fade (fade.ts, set before each render) and the sources' glow (glow.ts).
   */
  private readonly shared: Shared = sharedUniforms();
  /** The ground's uniforms (ground.ts). */
  private readonly groundU = {
    centre: uniform(new Vector2()),
    half: uniform(1),
    step: uniform(1),
    footMin: uniform(new Vector2()),
    footMax: uniform(new Vector2()),
    soft: uniform(1),
    line: uniform(new Color(LINE)),
    lineA: uniform(0.14),
    shadowA: uniform(0.55),
  };
  private glowFrame = 0;
  private glowDrawn = 0;
  private glowPhase = STILL_PHASE;
  /** The corner-shading bake in progress (its next slice's frame), cancelled when the model changes. */
  private aoFrame = 0;
  /** The floor grid and shadow under the room (ground.ts), and the height it sits at. */
  private readonly ground: Mesh;
  private groundZ = 0;
  /** The build-up (build.ts): the model it last played for, and its frame while running. */
  private builtKey = '';
  private buildFrame = 0;
  /** The dimensions overlay (dims.ts): its lines, its three labels (made on first use) and where they go. */
  private readonly dimLines: LineSegments;
  private dimLabels: HTMLDivElement[] = [];
  private dimMids: Vec[] = [];

  // The pointer between down and up.
  private down = { x: 0, y: 0, moved: false };

  /** The Results step's surface map and particles (M12 P3). */
  readonly results = new ResultsLayer();
  /**
   * B3 (decision 71): the running GPU solve's saved particles as they arrive (liveView.ts), drawn
   * in the looks the Results step draws them in, on the Simulate step only while a run is live. Its
   * own layer, so the Results step's particles are never touched by a run.
   */
  readonly live = new ResultsLayer();
  private liveOn = false;

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
      emptyGeometry(),
      new MeshMatcapNodeMaterial({ matcap, side: BackSide, flatShading: true, polygonOffset: true, polygonOffsetFactor: 1, polygonOffsetUnits: 1, colorWrite: false }),
    );
    const flat = { side: BackSide, flatShading: true, polygonOffset: true, polygonOffsetFactor: 1, polygonOffsetUnits: 1 } as const;
    this.tintColour = new MeshMatcapNodeMaterial({ matcap, vertexColors: true, ...flat });
    this.tintGrey = new MeshMatcapNodeMaterial({ matcap, ...flat });
    // The old look, kept as Wireframe in the menu: near-black Lambert faces under every edge.
    this.tintWire = new MeshLambertNodeMaterial({ color: 0x19191d, ...flat });
    this.tint = new Mesh(emptyTint(), this.tintColour);
    // See-through: the near walls drawn again as glass on top, writing no depth, so nothing behind
    // them is hidden (the proof page's layer 2, decision 59).
    const ghostMaterial = new MeshMatcapNodeMaterial({ matcap, vertexColors: true, side: FrontSide, flatShading: true, transparent: true, opacity: 0.15, depthWrite: false });
    this.ghost = new Mesh(this.tint.geometry, ghostMaterial);
    this.ghost.visible = false;
    for (const m of [this.tintColour, this.tintGrey, this.tintWire, ghostMaterial]) shaded(m, this.shared);
    // The glass case (glass.ts): the see-through layer's opacity by view angle.
    ghostMaterial.opacityNode = glassOpacity(materialOpacity);
    this.edges = new LineSegments(emptyGeometry(), fadingLines(new LineBasicNodeMaterial({ color: LINE, transparent: true, opacity: 0.3, depthWrite: false }), this.shared));
    this.highlight = new Mesh(
      emptyGeometry(),
      inSrgb(new MeshBasicNodeMaterial({ color: WARN, transparent: true, opacity: 0.55, side: DoubleSide, depthWrite: false })),
    );
    this.selectionWash = new Mesh(
      emptyGeometry(),
      inSrgb(new MeshBasicNodeMaterial({ color: SELECT, transparent: true, opacity: 0.3, side: BackSide, depthWrite: false })),
    );
    // The red outlines are fat lines (a GPU draws 1 px lines whatever is asked): the selection 2 px, a sound-level
    // plane's outline 3.5 px and its grid 1.5 px (Burhan 2026-10-06: "the red lines need to be a bit thicker ... like the sound-level plane").
    this.selectionEdges = new LineSegments2(emptyFat(), new FatLineMaterial({ color: SELECT, linewidth: 2, opacity: 1 }));
    this.sourcePoints = new MarkerSprites(glowPixels(64), 64, SOURCE_PX);
    this.receiverPoints = new MarkerSprites(ringPixels(32, 0.55, 0.85, WHITE, BG), 32, RECEIVER_PX);
    this.halo = new MarkerSprites(ringPixels(64, 0.78, 0.92, RED, null), 64, HALO_PX);
    this.sourceStems = new LineSegments(emptyGeometry(), inSrgb(new LineBasicNodeMaterial({ color: SELECT, transparent: true, opacity: 0.9 })));
    this.receiverStems = new LineSegments(emptyGeometry(), inSrgb(new LineBasicNodeMaterial({ color: LINE, transparent: true, opacity: 0.5 })));
    this.planeOutline = new LineSegments2(emptyFat(), new FatLineMaterial({ color: SELECT, linewidth: 3.5, opacity: 0.95, depthWrite: false }));
    this.planeGrid = new LineSegments2(emptyFat(), new FatLineMaterial({ color: SELECT, linewidth: 1.5, opacity: 0.45, depthWrite: false }));
    // The ground (ground.ts) writes its colour raw, as its WebGL ShaderMaterial did.
    const groundMaterial = new MeshBasicNodeMaterial({ transparent: true, depthWrite: false, side: DoubleSide });
    groundMaterial.fragmentNode = groundColour(this.groundU);
    this.ground = new Mesh(new PlaneGeometry(1, 1), groundMaterial);
    // The plan inset's background (decision-log row 50: a floating panel, see-through), as WebGL's
    // scissored clear wrote it: the panel colour at 35 %, premultiplied, and the far depth.
    const panel = rawColor(PANEL);
    const clear = new MeshBasicNodeMaterial({ blending: NoBlending, depthTest: true, depthWrite: true, depthFunc: AlwaysDepth });
    clear.vertexNode = vec4(positionGeometry.xy, 0.5, 1);
    clear.fragmentNode = vec4(vec3(panel.r, panel.g, panel.b).mul(0.35), 0.35);
    clear.depthNode = float(1);
    const clearQuad = new Mesh(new PlaneGeometry(2, 2), clear);
    clearQuad.frustumCulled = false;
    this.insetClear.add(clearQuad);
    this.fxDepth = new Mesh(emptyGeometry(), new MeshBasicNodeMaterial({ side: BackSide, colorWrite: false, polygonOffset: true, polygonOffsetFactor: 1, polygonOffsetUnits: 1 }));
    this.fxScene.add(this.fxDepth, this.results.gpu.group, this.live.gpu.group);
    this.ground.visible = false;
    // Drawn over everything, like a drawing's dimension lines, so a wall never hides one.
    this.dimLines = new LineSegments(emptyGeometry(), inSrgb(new LineBasicNodeMaterial({ color: LINE, transparent: true, opacity: 0.85, depthTest: false, depthWrite: false })));
    this.dimLines.frustumCulled = false;
    this.dimLines.visible = false;
    const order: [{ renderOrder: number }, number][] = [
      [this.ground, 0.2],
      [this.dimLines, 9],
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
      [this.receiverPoints.sprite, 6],
      [this.sourcePoints.sprite, 7],
      [this.halo.sprite, 8],
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
      this.receiverPoints.sprite,
      this.sourcePoints.sprite,
      this.halo.sprite,
      this.ground,
      this.dimLines,
      this.results.group,
      this.live.group,
    );
    this.results.setShown(stepStore.get() === 'results');
    this.showLayers();
    this.defaultCamera();
  }

  // ---- lifetime -------------------------------------------------------------------------

  /** Mounts the view into `dom`. The renderer and canvas are made on the first call only. */
  attach(dom: ViewportDom): () => void {
    this.dom = dom;
    if (!this.canvas && !viewportUi.get().error) this.createRenderer();
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
        this.showLayers();
        if (stepStore.get() !== 'results') mapPointerStore.set(null);
        this.updatePlanes();
        this.updateGlow();
        this.invalidate();
      }),
      animatorStore.subscribe(() => {
        const a = animatorStore.get();
        this.results.setStep(a.step);
        // B2: the GPU looks move between records at the fraction of the step the timeline has carried.
        if (a.playing) this.results.setTime(a.step + a.carry);
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
      // Decision 68 (b): which backend draws the view, once it has started (null while it starts).
      registerHook('gpuBackend', () => this.backend),
      // The device the view runs on: its limits and features as granted (the WebGL2 fallback: its texture limit).
      registerHook('gpuInfo', () => this.gpuInfo()),
      // B2: the GPU particles: their look, glow and count; their computed state; the dimming held; the benchmark.
      registerHook('gpuParticles', () => ({ look: this.results.gpu.currentLook(), glow: this.fxGlow, particles: this.results.gpu.particles(), active: this.results.gpu.active(), dim: this.shared.nmDim.value, refusal: { glow: this.results.particleLookRefusal('glow'), rays: this.results.particleLookRefusal('rays') } })),
      registerHook('gpuParticleState', (indices: number[]) => this.results.gpu.readState(indices)),
      registerHook('gpuParticleCheck', (indices: number[], t: number) => this.results.checkGpuParticles(indices, t)),
      registerHook('gpuDimHold', (on: boolean) => {
        this.dimHold = on;
        this.invalidate();
        return on;
      }),
      registerHook('present', (on: boolean, turntable = false) => {
        this.setPresent(on);
        if (on) this.setTurntable(turntable);
        return presentStore.get();
      }),
      registerHook('gpuGlow', (g: Glow) => {
        this.setGlow(g);
        return g;
      }),
      registerHook('gpuBench', (n: number | null, frames: number, look: ParticleLook, glow: Glow) => this.bench(n, frames, look, glow)),
      // The benchmark's particle count held (null: the file's own), to look at what it draws.
      registerHook('gpuBenchSet', (n: number | null) => {
        const c = this.results.gpu.bench(n);
        replicaStore.set(this.results.gpu.replicas());
        this.results.setParticleLook(this.results.gpu.currentLook());
        this.invalidate();
        return c;
      }),
    ];
    // A remount (React StrictMode in dev) rebuilds the DOM-side state from the stores.
    this.setMesh(meshStore.get(), true);
    viewportStore.set({ live: this.canvas !== null, drawnRev: null });
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
    if (this.glowFrame) cancelAnimationFrame(this.glowFrame);
    this.glowFrame = 0;
    if (this.buildFrame) cancelAnimationFrame(this.buildFrame);
    this.buildFrame = 0;
    this.shared.nmBuildZ.value = NO_CUT;
    if (this.dom) this.dom.labels.replaceChildren();
    this.markers = [];
    this.dom = null;
    viewportStore.set({ live: false, drawnRev: null });
  }

  /**
   * Makes the canvas, its input handlers and the orbit controls now, and starts the renderer, which
   * draws from the first frame after its asynchronous start. A canvas with a renderer still starting
   * is live: the e2e's idle() then waits for the first drawn frame of the model.
   */
  private createRenderer(): void {
    const canvas = document.createElement('canvas');
    canvas.className = 'viewport-canvas';
    this.canvas = canvas;
    void this.startRenderer(canvas);

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

  private gpuInfo(): { backend: string | null; maxTexture: number; features: string[]; limits: Record<string, number> } | null {
    const r = this.renderer;
    if (!r) return null;
    const dev = (r.backend as unknown as { device?: { features: Set<string>; limits: Record<string, number> } }).device;
    const limits: Record<string, number> = {};
    if (dev) for (const k of ['maxTextureDimension2D', 'maxBufferSize', 'maxStorageBufferBindingSize', 'maxComputeWorkgroupSizeX', 'maxComputeInvocationsPerWorkgroup']) limits[k] = dev.limits[k];
    return { backend: this.backend, maxTexture: this.results.maxTextureSize(), features: dev ? [...dev.features].sort() : [], limits };
  }

  /** Starts WebGPURenderer on `canvas`: WebGPU, or WebGL2 where the WebView has no WebGPU (or `nm.forceWebGL` is set). */
  private async startRenderer(canvas: HTMLCanvasElement): Promise<void> {
    let renderer: WebGPURenderer;
    try {
      renderer = new WebGPURenderer({ canvas, antialias: true, alpha: true, powerPreference: 'high-performance', forceWebGL: forceWebGL(), requiredLimits: await gpuLimits(), trackTimestamp: gpuTiming() });
      await renderer.init();
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      log('FAIL', `The 3D view could not start its GPU renderer: ${message}`);
      this.canvas = null;
      canvas.remove();
      this.setUi({ error: message });
      viewportStore.set({ live: false, drawnRev: null });
      return;
    }
    renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
    // Linear output: no framebuffer pass, each material writes sRGB itself (nodes.ts).
    renderer.outputColorSpace = LinearSRGBColorSpace;
    renderer.toneMapping = NoToneMapping;
    renderer.autoClear = false;
    const webgpu = (renderer.backend as unknown as { isWebGPUBackend?: boolean }).isWebGPUBackend === true;
    this.backend = webgpu ? 'webgpu' : 'webgl2';
    renderer.onDeviceLost = (info) => {
      this.contextLost = true;
      log('WARN', `The 3D view lost its GPU device (${info.reason ?? 'unknown'}): ${info.message}`);
    };
    this.renderer = renderer;
    this.results.setRenderer(renderer);
    this.live.setRenderer(renderer);
    log('INFO', `3D view: ${webgpu ? 'WebGPU' : 'WebGL2 (no WebGPU in this WebView)'}, three r${REVISION}`);
    this.frame();
    this.invalidate();
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
    // The fat lines (FatLineMaterial) take their width from the viewport the renderer draws to.
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
      this.setFade(main === this.persp);
      // The ground only from above, in the perspective view; never in plan, where it would be a second grid.
      this.ground.visible = main === this.persp && !!this.bounds && viewStyle.get().ground && this.persp.position.z > this.groundZ;
      this.updateDims(main === this.persp);
      const resultsFx = stepStore.get() === 'results' && this.results.gpu.active();
      const liveFx = this.liveShown() && this.live.gpu.active();
      const fx = resultsFx || liveFx;
      this.easeDim(fx, liveFx);
      if (resultsFx) this.results.computeFrame();
      if (liveFx) this.live.computeFrame();
      r.render(this.scene, main);
      if (fx) {
        this.fxParticles(r, main);
        this.fxComposite(r, w, h);
      }
      if (this.results.particleMeta && stepStore.get() === 'results') noteDrawn(this.results.particleStep());
      if (this.view === 'perspective' && this.bounds) this.renderInset(r, dom.inset);
      const covers = panelRects();
      this.placeLabels(main, w, h, covers);
      this.placeDims(main, w, h, covers);
      this.placeGizmo(main);
    }
    // While the renderer starts the view is live but has drawn nothing: idle() waits for the first frame.
    const vp = viewportStore.get();
    const live = this.canvas !== null;
    const drawnRev = this.renderer ? this.builtRev : vp.drawnRev;
    if (vp.live !== live || vp.drawnRev !== drawnRev) viewportStore.set({ live, drawnRev });
  }

  /** The room dimmed while the light plays (or is held for a still, or the live solve plays), eased over about a third of a second. */
  private easeDim(fx: boolean, live = false): void {
    const now = performance.now();
    const dt = this.dimAt ? Math.min(100, now - this.dimAt) : 16;
    this.dimAt = now;
    const target = fx && (animatorStore.get().playing || this.dimHold || live) ? 1 : 0;
    const u = this.shared.nmDim;
    const cur = u.value as number;
    const next = stillMotion() ? target : cur + (target - cur) * Math.min(1, dt / 120);
    u.value = Math.abs(next - target) < 0.004 ? target : next;
    if (u.value !== target) this.invalidate();
    this.setGhosted((u.value as number) > 0);
    this.results.setMapFade(this.mapFullWhilePlaying ? 1 : 1 - 0.75 * (u.value as number));
  }

  /**
   * Round 2, item 4: while the light plays the room is a ghost, its surfaces see-through (10 %, depth-faded)
   * and its edges thin dim lines, so no slab hides a particle; the depth that hid the light behind the walls is
   * off. Restored, exactly as drawn before, when the dimming has eased back to 0.
   */
  private setGhosted(on: boolean): void {
    if (on === this.ghosted) return;
    this.ghosted = on;
    for (const m of [this.tintColour, this.tintGrey, this.tintWire]) {
      m.transparent = on;
      m.depthWrite = !on;
      m.needsUpdate = true;
    }
    this.faces.visible = !on;
    this.fxDepth.visible = !on;
  }

  /** B3: the live layer is drawn on the Simulate step while it holds particles; the Results step's otherwise. */
  private liveShown(): boolean {
    return this.liveOn && stepStore.get() === 'simulate';
  }

  /** Which particle layer is drawn: each GPU group only with its own step, and the shown one owns the shared ramp. */
  private showLayers(): void {
    const live = this.liveShown();
    this.live.setShown(live);
    this.live.gpu.group.visible = live;
    this.results.gpu.group.visible = stepStore.get() === 'results';
    (live ? this.live : this.results).gpu.claimRamp();
  }

  /** B3: the live particles (null clears the layer), in `look` with `trails`-step trails; returns the look in force. */
  setLive(p: Particles | null, look: ParticleLook, trails: number): ParticleLook {
    this.live.setParticles(p, p ? { run: 'live', bandHz: p.bandHz, particles: p.particleCount, records: p.recordCount } : null);
    this.live.setTrails(p ? trails : 0);
    const inForce = this.live.setParticleLook(look);
    this.liveOn = !!p;
    this.showLayers();
    this.invalidate();
    return inForce;
  }

  /** B3: the live clock, fractional steps. */
  setLiveTime(t: number): void {
    this.live.setStep(Math.floor(t));
    this.live.setTime(t);
    if (this.liveShown()) this.invalidate();
  }

  /** B3's hook: what the live layer holds and draws. */
  liveState(): { on: boolean; shown: boolean; look: ParticleLook; particles: number; records: number; active: boolean; step: number; dim: number } {
    const m = this.live.particleMeta;
    return { on: this.liveOn, shown: this.liveShown(), look: this.live.gpu.currentLook(), particles: m?.particles ?? 0, records: m?.records ?? 0, active: this.live.gpu.active(), step: this.live.particleStep(), dim: this.shared.nmDim.value as number };
  }

  /** Round 2, item 5: keep the map at full strength while the light plays (else it fades to 25 %). */
  setMapWhilePlaying(full: boolean): void {
    this.mapFullWhilePlaying = full;
    this.invalidate();
  }

  /**
   * Round 2, item 6: the presentation view (H, or the tools' button): every panel hidden, the canvas alone. The
   * turntable (O while presenting, or its button) circles the camera slowly about the view's target.
   */
  setPresent(on: boolean): void {
    presentStore.set({ ...presentStore.get(), on });
    try {
      document.documentElement.classList.toggle('present', on);
    } catch {
      // No document (tests).
    }
    if (!on) this.setTurntable(false);
    this.invalidate();
  }

  setTurntable(on: boolean): void {
    presentStore.set({ ...presentStore.get(), turntable: on });
    if (this.turnFrame) cancelAnimationFrame(this.turnFrame);
    this.turnFrame = 0;
    const controls = this.controls;
    if (!on || !controls || stillMotion()) return;
    let last = 0;
    const stop = () => this.setTurntable(false);
    controls.addEventListener('start', stop);
    const tick = (now: number) => {
      if (!presentStore.get().turntable) {
        controls.removeEventListener('start', stop);
        return;
      }
      const dt = last ? Math.min(50, now - last) : 16;
      last = now;
      const off = this.persp.position.clone().sub(controls.target).applyAxisAngle(new Vector3(0, 0, 1), (TURN_DEG_S * Math.PI * dt) / 180000);
      this.persp.position.copy(controls.target).add(off);
      controls.update();
      this.invalidate();
      this.turnFrame = requestAnimationFrame(tick);
    };
    this.turnFrame = requestAnimationFrame(tick);
  }

  /** The light pass: the GPU looks, additive, into the half-float target at the drawing buffer's size, against the room's depth. */
  private fxParticles(r: WebGPURenderer, camera: Camera): void {
    const size = r.getDrawingBufferSize(new Vector2());
    if (!this.fxTarget) {
      this.fxTarget = new RenderTarget(size.x, size.y, { type: HalfFloatType, samples: 4, depthBuffer: true });
      this.fxInput.value = this.fxTarget.texture;
    } else if (this.fxTarget.width !== size.x || this.fxTarget.height !== size.y) {
      this.fxTarget.setSize(size.x, size.y);
    }
    const prev = r.getRenderTarget();
    r.setRenderTarget(this.fxTarget);
    r.setClearColor(0x000000, 0);
    r.clear();
    r.render(this.fxScene, camera);
    r.setRenderTarget(prev);
  }

  /** The light onto the frame: plus its bloom (unless the glow is off), ACES tone mapped, to sRGB, added. */
  private fxComposite(r: WebGPURenderer, w: number, h: number): void {
    if (this.fxGlow !== 'off' && this.fxTarget) {
      if (!this.fxBloomPass) {
        const bm = new MeshBasicNodeMaterial({ depthTest: false, depthWrite: false });
        // Only the hottest light blooms (threshold on luminance), in a tight radius: sparks, not smudges.
        bm.fragmentNode = bloom(this.fxInput, this.fxStrength, 0.12, 0.85);
        this.fxBloomPass = new QuadMesh(bm);
      }
      if (!this.fxBloomTarget) {
        this.fxBloomTarget = new RenderTarget(this.fxTarget.width, this.fxTarget.height, { type: HalfFloatType, depthBuffer: false });
        this.fxBloomInput.value = this.fxBloomTarget.texture;
      } else if (this.fxBloomTarget.width !== this.fxTarget.width || this.fxBloomTarget.height !== this.fxTarget.height) {
        this.fxBloomTarget.setSize(this.fxTarget.width, this.fxTarget.height);
      }
      const prev = r.getRenderTarget();
      r.setRenderTarget(this.fxBloomTarget);
      this.fxBloomPass.render(r);
      r.setRenderTarget(prev);
    }
    const quad = this.fxGlow === 'off' ? (this.fxPlain ??= this.fxQuad(false)) : (this.fxBloomed ??= this.fxQuad(true));
    r.setScissorTest(false);
    r.setViewport(0, 0, w, h);
    r.render(quad, this.persp);
  }

  private fxQuad(withBloom: boolean): Scene {
    const m = new MeshBasicNodeMaterial({ transparent: true, depthTest: false, depthWrite: false });
    m.blending = CustomBlending;
    m.blendSrc = OneFactor;
    m.blendDst = OneFactor;
    m.blendEquation = AddEquation;
    m.blendSrcAlpha = OneFactor;
    m.blendDstAlpha = OneFactor;
    m.blendEquationAlpha = AddEquation;
    m.fragmentNode = Fn(() => {
      const light = withBloom ? texture(this.fxInput, screenUV).rgb.add(texture(this.fxBloomInput, screenUV).rgb) : texture(this.fxInput, screenUV).rgb;
      const c = sRGBTransferOETF(clamp(acesFilmicToneMapping(light, float(1)), 0, 1)).toVar();
      return vec4(c, clamp(max(c.r, max(c.g, c.b)), 0, 1));
    })();
    m.vertexNode = vec4(positionGeometry.xy, 0.5, 1);
    const quad = new Mesh(new PlaneGeometry(2, 2), m);
    quad.frustumCulled = false;
    const scene = new Scene();
    scene.add(quad);
    return scene;
  }

  /** Decision 69's glow control: off (no bloom pass), soft or full. */
  setGlow(g: Glow): void {
    this.fxGlow = g;
    this.fxStrength.value = GLOW_STRENGTH[g];
    this.invalidate();
  }

  glow(): Glow {
    return this.fxGlow;
  }

  /**
   * B2's measurement: `frames` frames of `n` particles (null: the file's own) in `look` and `glow`, the
   * timeline advancing a quarter step a frame. Wall: the interval between animation frames and the CPU time
   * of the draw. GPU (with `nm.gpuTiming`): timestamp queries per stage, each stage resolved before the next.
   */
  private async bench(n: number | null, frames: number, look: ParticleLook, glow: Glow): Promise<Record<string, unknown>> {
    const r = this.renderer;
    if (!r || !this.dom) throw new Error('no renderer');
    if (stepStore.get() !== 'results') throw new Error('open the Results step first');
    const gpu = this.results.gpu;
    const count = gpu.bench(n);
    replicaStore.set(gpu.replicas());
    const inForce = this.results.setParticleLook(look);
    this.setGlow(glow);
    const t0 = animatorStore.get().step;
    const stats = (xs: number[]) => {
      const s = [...xs].sort((a, b) => a - b);
      const q = (p: number) => s[Math.min(s.length - 1, Math.floor(p * s.length))];
      return { mean: +(s.reduce((a, b) => a + b, 0) / s.length).toFixed(3), p50: +q(0.5).toFixed(3), p95: +q(0.95).toFixed(3), max: +s[s.length - 1].toFixed(3) };
    };
    const wall: number[] = [];
    const cpu: number[] = [];
    await new Promise<void>((done) => {
      let i = 0;
      let last = 0;
      const tick = (now: number) => {
        if (last) wall.push(now - last);
        last = now;
        this.results.setTime(t0 + 0.25 * i);
        const c0 = performance.now();
        this.renderNow();
        cpu.push(performance.now() - c0);
        if (++i <= frames) requestAnimationFrame(tick);
        else done();
      };
      requestAnimationFrame(tick);
    });
    const out: Record<string, unknown> = { particles: count, look: inForce, glow, frames, backend: this.backend, canvas: r.getDrawingBufferSize(new Vector2()).toArray(), wallMs: stats(wall), fps: +(1000 / stats(wall).mean).toFixed(1), drawCpuMs: stats(cpu) };
    // Unthrottled (no display refresh): each frame drawn and waited for on the GPU queue, and the same frames back to back.
    const dev = (r.backend as unknown as { device?: { queue: { onSubmittedWorkDone(): Promise<void> } } }).device;
    if (dev) {
      const synced: number[] = [];
      for (let i = 0; i < frames; i++) {
        const a = performance.now();
        this.results.setTime(t0 + 0.25 * i);
        this.renderNow();
        await dev.queue.onSubmittedWorkDone();
        synced.push(performance.now() - a);
      }
      const b0 = performance.now();
      for (let i = 0; i < frames; i++) {
        this.results.setTime(t0 + 0.25 * i);
        this.renderNow();
      }
      await dev.queue.onSubmittedWorkDone();
      out.frameSyncedMs = stats(synced);
      out.frameBatchedMs = +((performance.now() - b0) / frames).toFixed(3);
    }
    if (gpuTiming() && this.backend === 'webgpu') {
      const comp: number[] = [];
      const main: number[] = [];
      const part: number[] = [];
      const post: number[] = [];
      const size = this.syncSize();
      const cam = this.mainCamera(size ? size.w / size.h : 1);
      await r.resolveTimestampsAsync('render');
      await r.resolveTimestampsAsync('compute');
      for (let i = 0; i < frames; i++) {
        this.results.setTime(t0 + 0.25 * i);
        gpu.invalidate();
        gpu.update();
        comp.push((await r.resolveTimestampsAsync('compute')) ?? NaN);
        r.setViewport(0, 0, size?.w ?? 1, size?.h ?? 1);
        r.setClearColor(0x000000, 0);
        r.clear();
        r.render(this.scene, cam);
        main.push((await r.resolveTimestampsAsync('render')) ?? NaN);
        this.fxParticles(r, cam);
        part.push((await r.resolveTimestampsAsync('render')) ?? NaN);
        this.fxComposite(r, size?.w ?? 1, size?.h ?? 1);
        post.push((await r.resolveTimestampsAsync('render')) ?? NaN);
      }
      out.gpuMs = { compute: stats(comp), scene: stats(main), particles: stats(part), bloomAndComposite: stats(post) };
    }
    gpu.bench(null);
    replicaStore.set(null);
    this.results.setTime(t0);
    this.invalidate();
    return out;
  }

  /** The distance fade (fade.ts) across the model's bounding sphere as the perspective camera sees it; none in plan. */
  private setFade(perspective: boolean): void {
    const u = this.shared;
    const b = this.bounds;
    if (b) {
      const c = new Vector3((b.min[0] + b.max[0]) / 2, (b.min[1] + b.max[1]) / 2, (b.min[2] + b.max[2]) / 2);
      const range = fadeRange(this.persp.position.distanceTo(c), Math.hypot(b.max[0] - b.min[0], b.max[1] - b.min[1], b.max[2] - b.min[2]) / 2);
      this.results.gpu.setDepthRange(range.near, range.far);
      this.live.gpu.setDepthRange(range.near, range.far);
    }
    if (!perspective || !b || !viewStyle.get().fade) {
      u.nmFadeMax.value = 0;
      return;
    }
    const centre = new Vector3((b.min[0] + b.max[0]) / 2, (b.min[1] + b.max[1]) / 2, (b.min[2] + b.max[2]) / 2);
    const radius = Math.hypot(b.max[0] - b.min[0], b.max[1] - b.min[1], b.max[2] - b.min[2]) / 2;
    const { near, far } = fadeRange(this.persp.position.distanceTo(centre), radius);
    u.nmFadeNear.value = near;
    u.nmFadeFar.value = far;
    u.nmFadeMax.value = FADE_MAX;
  }

  /** The plan inset: the same scene through the top camera, scissored to the DOM box. */
  private renderInset(r: WebGPURenderer, inset: HTMLElement): void {
    const c = this.canvas?.getBoundingClientRect();
    const b = inset.getBoundingClientRect();
    if (!c || b.width < 2 || b.height < 2) return;
    // WebGPURenderer's viewport and scissor run from the top left (WebGLRenderer's from the bottom left).
    const x = b.left - c.left;
    const y = b.top - c.top;
    r.setScissorTest(true);
    r.setScissor(x, y, b.width, b.height);
    r.setViewport(x, y, b.width, b.height);
    // The inset is a floating panel too (decision-log row 50): its box is see-through. A clear would
    // take the whole canvas on WebGPU; the quad replaces colour and depth inside the scissor only.
    r.render(this.insetClear, this.plan);
    this.setFade(false);
    this.ground.visible = false;
    this.dimLines.visible = false;
    this.fitPlan(b.width / b.height);
    this.setMarkerScale(INSET_MARKER_SCALE);
    r.render(this.scene, this.plan);
    r.setScissorTest(false);
  }

  private setMarkerScale(k: number): void {
    this.sourcePoints.material.size = SOURCE_PX * k * spriteScale(this.glowPhase);
    this.receiverPoints.material.size = RECEIVER_PX * k;
    this.halo.material.size = HALO_PX * k;
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

  private placeLabels(camera: Camera, w: number, h: number, covers: DOMRect[]): void {
    const v = new Vector3();
    for (const m of this.markers) {
      v.copy(m.p).project(camera);
      const x = ((v.x + 1) / 2) * w;
      const y = ((1 - v.y) / 2) * h;
      const left = x + (m.kind === 'source' ? 12 : 10);
      const top = y - 8;
      const size = labelSize(m.label);
      const visible = v.z > -1 && v.z < 1 && Math.abs(v.x) <= 1.02 && Math.abs(v.y) <= 1.02 && !underPanel(left, top, size.w, size.h, covers);
      m.label.style.display = visible ? '' : 'none';
      if (visible) m.label.style.transform = `translate(${Math.round(left)}px, ${Math.round(top)}px)`;
    }
  }

  /**
   * The dimensions overlay (dims.ts) for the perspective view: the model check's box, the lines on
   * the sides facing the camera. Hidden in plan, without a model, or when switched off.
   */
  private updateDims(perspective: boolean): void {
    const box = roomBox(sceneStore.get()?.check);
    const on = perspective && !!box && !!this.mesh && viewStyle.get().dims;
    this.dimLines.visible = on;
    if (!on || !box) {
      this.dimMids = [];
      for (const l of this.dimLabels) l.style.display = 'none';
      return;
    }
    const p = this.persp.position;
    const { lines, marks, segments } = dimensionLines(box, [p.x, p.y, p.z]);
    const g = this.dimLines.geometry;
    const pos = g.getAttribute('position');
    if (pos && pos.count * 3 === segments.length) {
      (pos.array as Float32Array).set(segments);
      pos.needsUpdate = true;
    } else {
      g.setAttribute('position', new Float32BufferAttribute(segments, 3));
    }
    // The three sizes, then the rulers' numbers; their count changes only with the model.
    const all = [
      ...lines.map((l) => ({ cls: 'vp-label dim', key: l.key, text: l.text, p: l.mid })),
      ...marks.map((m) => ({ cls: 'vp-label dim-tick', key: '', text: m.text, p: m.p })),
    ];
    if (this.dimLabels.length !== all.length && this.dom) {
      for (const d of this.dimLabels) d.remove();
      this.dimLabels = all.map(() => document.createElement('div'));
      this.dom.labels.append(...this.dimLabels);
    }
    all.forEach((l, i) => {
      const d = this.dimLabels[i];
      if (!d) return;
      d.className = l.cls;
      if (l.key) d.dataset.dim = l.key;
      d.textContent = l.text;
    });
    this.dimMids = all.map((l) => l.p);
  }

  private placeDims(camera: Camera, w: number, h: number, covers: DOMRect[]): void {
    const v = new Vector3();
    this.dimLabels.forEach((d, i) => {
      const m = this.dimMids[i];
      if (!m) return void (d.style.display = 'none');
      v.set(m[0], m[1], m[2]).project(camera);
      const x = ((v.x + 1) / 2) * w;
      const y = ((1 - v.y) / 2) * h;
      const size = labelSize(d);
      const visible = v.z > -1 && v.z < 1 && Math.abs(v.x) <= 1.02 && Math.abs(v.y) <= 1.02 && !underPanel(x - size.w / 2, y - size.h / 2, size.w, size.h, covers);
      d.style.display = visible ? '' : 'none';
      if (visible) d.style.transform = `translate(${Math.round(x)}px, ${Math.round(y)}px) translate(-50%, -50%)`;
    });
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
      if (this.bounds) this.layoutGround(this.bounds);
      this.faces.geometry = geometry;
      this.fxDepth.geometry = geometry;
      this.tint.geometry = geometry.toNonIndexed();
      this.ghost.geometry = this.tint.geometry;
      this.bakeCorners(mesh);
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
    // A new model (not an edit of this one, which keeps its faces and box) builds itself up.
    const key = mesh && this.bounds ? `${mesh.faceCount}:${this.bounds.min.join(',')}:${this.bounds.max.join(',')}` : '';
    if (key && key !== this.builtKey) {
      this.builtKey = key;
      this.startBuild();
    }
  }

  /**
   * The build-up (build.ts): the cut rises floor to ceiling while the camera swings into the framing
   * `frame()` just set. Any drag on the view stops the swing; the cut always finishes.
   */
  private startBuild(): void {
    const b = this.bounds;
    const controls = this.controls;
    if (this.buildFrame) cancelAnimationFrame(this.buildFrame);
    this.buildFrame = 0;
    this.shared.nmBuildZ.value = NO_CUT;
    if (!b || !this.dom || stillMotion() || this.view !== 'perspective') return;
    const target = controls ? controls.target.clone() : new Vector3();
    const offset = this.persp.position.clone().sub(target);
    let swinging = !!controls;
    const stop = () => (swinging = false);
    controls?.addEventListener('start', stop);
    this.shared.nmBuildBand.value = 0.02 * (b.max[2] - b.min[2]) + 0.05;
    const t0 = performance.now();
    const tick = (now: number) => {
      const f = Math.min(1, (now - t0) / BUILD_MS);
      this.shared.nmBuildZ.value = buildHeight(f, b.min[2], b.max[2]);
      if (swinging && controls) {
        this.persp.position.copy(target).add(offset.clone().applyAxisAngle(new Vector3(0, 0, 1), swingAngle(f)));
        controls.update();
      }
      this.invalidate();
      if (f < 1) {
        this.buildFrame = requestAnimationFrame(tick);
      } else {
        this.buildFrame = 0;
        controls?.removeEventListener('start', stop);
      }
    };
    this.buildFrame = requestAnimationFrame(tick);
  }

  /** Places the ground plane (ground.ts) under `box` and sets its grid and shadow. */
  private layoutGround(box: Box): void {
    const g = groundLayout(box);
    this.groundZ = g.z;
    this.ground.position.set(g.centre[0], g.centre[1], g.z);
    this.ground.scale.set(2 * g.half, 2 * g.half, 1);
    const u = this.groundU;
    u.centre.value.set(g.centre[0], g.centre[1]);
    u.half.value = g.half;
    u.step.value = g.step;
    u.footMin.value.set(box.min[0], box.min[1]);
    u.footMax.value.set(box.max[0], box.max[1]);
    u.soft.value = g.soft;
  }

  /**
   * Corner shading (ao.ts): the faces start unshaded (1) so the room draws at once, and the baked
   * values replace them when the last slice is done, in one upload, so no frame is half shaded.
   */
  private bakeCorners(mesh: SceneMesh): void {
    const g = this.tint.geometry;
    const pos = g.getAttribute('position');
    const bvh = this.bvh;
    g.setAttribute('nmAo', new BufferAttribute(new Float32Array(pos.count).fill(1), 1));
    if (!bvh) return;
    const positions = pos.array as Float32Array;
    const faces = Math.floor(pos.count / 3);
    const reach = aoReach(this.bounds);
    const out = new Float32Array(pos.count).fill(1);
    const done = new Map<string, number>();
    const ray = new Ray();
    const hit = (o: Vec, d: Vec, far: number) => {
      ray.origin.set(o[0], o[1], o[2]);
      ray.direction.set(d[0], d[1], d[2]);
      return !!bvh.raycastFirst(ray, DoubleSide, 0, far);
    };
    let from = 0;
    const slice = () => {
      if (this.tint.geometry !== g) return;
      bakeAo(positions, mesh.indices, hit, reach, from, from + AO_SLICE, out, done);
      from += AO_SLICE;
      if (from < faces) {
        this.aoFrame = requestAnimationFrame(slice);
        return;
      }
      this.aoFrame = 0;
      g.setAttribute('nmAo', new BufferAttribute(out, 1));
      this.invalidate();
    };
    this.aoFrame = requestAnimationFrame(slice);
  }

  private disposeModel(): void {
    if (this.aoFrame) cancelAnimationFrame(this.aoFrame);
    this.aoFrame = 0;
    for (const o of [this.faces, this.tint, this.highlight, this.selectionWash]) {
      o.geometry.dispose();
      o.geometry = emptyGeometry();
    }
    this.tint.geometry = emptyTint();
    this.ghost.geometry = this.tint.geometry;
    this.fxDepth.geometry = emptyGeometry();
    this.triangleEdges.dispose();
    this.featureEdges.dispose();
    this.triangleEdges = emptyGeometry();
    this.featureEdges = emptyGeometry();
    this.edges.geometry = this.triangleEdges;
    this.selectionEdges.geometry.dispose();
    this.selectionEdges.geometry = emptyFat();
    this.bvh = null;
    this.positions32 = null;
    this.topo = null;
    this.bounds = null;
  }

  /** An overlay geometry of `faces` over the model's positions (its own GPU buffer). */
  private overlay(faces: ArrayLike<number>): BufferGeometry {
    const g = emptyGeometry();
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
    (this.ghost.material as MeshMatcapNodeMaterial).opacity = st.glass / 100;
    this.edges.geometry = st.edges === 'feature' ? this.featureEdges : this.triangleEdges;
    this.shared.nmAoMix.value = st.corners ? 1 : 0;
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
    const fat = emptyFat();
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
    labels?.replaceChildren(...list.map((m) => m.label), ...this.dimLabels);

    const points = (kind: MarkerKind) => {
      const ms = list.filter((m) => m.kind === kind);
      return { xyz: ms.flatMap((m) => [m.p.x, m.p.y, m.p.z]), ms };
    };
    const stems = (ms: Marker[]) => {
      const g = emptyGeometry();
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
    for (const o of [this.sourceStems, this.receiverStems]) o.geometry.dispose();
    this.sourcePoints.set(src.xyz);
    this.receiverPoints.set(rcv.xyz);
    this.sourceStems.geometry = stems(src.ms);
    this.receiverStems.geometry = stems(rcv.ms);
    const picked = list.find((m) => (sel.kind === 'source' || sel.kind === 'receiver') && m.kind === sel.kind && m.id === sel.id);
    this.halo.set(picked ? [picked.p.x, picked.p.y, picked.p.z] : []);

    const lit = (view?.sources ?? []).filter((s) => s.enabled && s.position.every(finite)).slice(0, GLOW_MAX);
    lit.forEach((s, i) => (this.shared.glowPos.array[i] as Vector3).set(...(s.position as [number, number, number])));
    this.shared.glowCount.value = lit.length;
    this.updateGlow();
  }

  /**
   * The glow's strength and its pulse: off on the Results step (a red pool there would read as a level
   * on the map) and with no enabled source; the pulse runs at about 30 frames a second while the view
   * is mounted, unless motion is to hold still.
   */
  private updateGlow(): void {
    const u = this.shared;
    u.glowRadius.value = glowRadius(this.bounds);
    const on = u.glowCount.value > 0 && stepStore.get() !== 'results';
    const moving = on && !!this.dom && !stillMotion();
    if (!moving) {
      if (this.glowFrame) cancelAnimationFrame(this.glowFrame);
      this.glowFrame = 0;
      this.glowPhase = STILL_PHASE;
    } else if (!this.glowFrame) {
      this.glowFrame = requestAnimationFrame(this.glowTick);
    }
    u.glowLevel.value = on ? glowLevel(this.glowPhase) : 0;
  }

  private readonly glowTick = (now: number): void => {
    this.glowFrame = requestAnimationFrame(this.glowTick);
    if (now - this.glowDrawn < 33) return;
    this.glowDrawn = now;
    this.glowPhase = pulsePhase(now);
    this.shared.glowLevel.value = glowLevel(this.glowPhase);
    this.invalidate();
  };

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
    const g1 = emptyFat();
    if (outline.length > 0) g1.setPositions(outline);
    const g2 = emptyFat();
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

  /**
   * Draws the view now and copies the canvas as drawn; the promise gives its pixels, top row first,
   * premultiplied RGBA as the GPU holds them. Several grabs in one task (a frame, then the same frame
   * with an overlay hidden) each read their own frame: the copy is queued behind the draw it follows.
   * Null without a live renderer.
   */
  private grabFrame(): Promise<{ width: number; height: number; rgba: Uint8Array }> | null {
    const r = this.renderer;
    const c = this.canvas;
    if (!r || !c || !this.dom || this.contextLost || !this.syncSize()) return null;
    this.renderNow();
    const b = r.backend as unknown as { isWebGPUBackend?: boolean; device?: GpuDevice; gl?: WebGL2RenderingContext };
    if (b.isWebGPUBackend && b.device) {
      const device = b.device;
      const ctx = c.getContext('webgpu') as unknown as GpuCanvas;
      const tex = ctx.getCurrentTexture();
      const w = tex.width;
      const h = tex.height;
      const bytesPerRow = Math.ceil((4 * w) / 256) * 256;
      const buffer = device.createBuffer({ size: bytesPerRow * h, usage: GPU_MAP_READ | GPU_COPY_DST });
      const enc = device.createCommandEncoder();
      enc.copyTextureToBuffer({ texture: tex }, { buffer, bytesPerRow }, [w, h]);
      device.queue.submit([enc.finish()]);
      const bgra = tex.format.startsWith('bgra');
      return buffer.mapAsync(GPU_MAP_MODE_READ).then(() => {
        const px = unpadRows(new Uint8Array(buffer.getMappedRange()), w, h, bytesPerRow);
        buffer.destroy();
        return { width: w, height: h, rgba: bgra ? bgraToRgba(px) : px };
      });
    }
    const gl = b.gl;
    if (!gl) return null;
    const w = gl.drawingBufferWidth;
    const h = gl.drawingBufferHeight;
    const px = new Uint8Array(w * h * 4);
    const prev = gl.getParameter(gl.READ_FRAMEBUFFER_BINDING) as WebGLFramebuffer | null;
    gl.bindFramebuffer(gl.READ_FRAMEBUFFER, null);
    gl.readPixels(0, 0, w, h, gl.RGBA, gl.UNSIGNED_BYTE, px);
    gl.bindFramebuffer(gl.READ_FRAMEBUFFER, prev);
    return Promise.resolve({ width: w, height: h, rgba: new Uint8Array(flipRows(px, w, h)) });
  }

  /** W5's hook: an FNV-1a hash of the frame as drawn now (top row first), to tell two looks of the map apart. */
  async framePixels(): Promise<{ pixels: number; hash: number } | null> {
    const f = await this.grabFrame();
    if (!f) return null;
    let h = 0x811c9dc5;
    for (let i = 0; i < f.rgba.length; i++) h = Math.imul(h ^ f.rgba[i], 0x01000193) >>> 0;
    return { pixels: f.rgba.length / 4, hash: h };
  }

  /** W9: the frame as drawn now, top row first, the GPU's premultiplied RGBA; null without a view. */
  async frameRgba(): Promise<{ width: number; height: number; rgba: Uint8ClampedArray } | null> {
    const f = await this.grabFrame();
    return f ? { width: f.width, height: f.height, rgba: new Uint8ClampedArray(f.rgba.buffer, f.rgba.byteOffset, f.rgba.length) } : null;
  }

  /** The frame as drawn, and as drawn with `hide` applied (restored at once), both grabbed in one task. */
  private async framePair(hide: () => () => void): Promise<{ drawn: Uint8Array; bare: Uint8Array } | null> {
    const a = this.grabFrame();
    if (!a) return null;
    const restore = hide();
    let b: ReturnType<ViewportEngine['grabFrame']>;
    try {
      b = this.grabFrame();
    } finally {
      restore();
    }
    this.renderNow();
    if (!b) return null;
    const [drawn, bare] = await Promise.all([a, b]);
    return { drawn: drawn.rgba, bare: bare.rgba };
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
  async mapPixels(): Promise<{ pixels: number; changed: number } | null> {
    const pair = await this.framePair(() => this.results.hideMapFor());
    if (!pair) return null;
    const { drawn, bare } = pair;
    let changed = 0;
    for (let i = 0; i < drawn.length; i += 4) {
      if (Math.max(Math.abs(drawn[i] - bare[i]), Math.abs(drawn[i + 1] - bare[i + 1]), Math.abs(drawn[i + 2] - bare[i + 2])) > 2) changed++;
    }
    return { pixels: drawn.length / 4, changed };
  }

  private async highlightPixels(): Promise<{ pixels: number; changed: number; warn: number } | null> {
    const pair = await this.framePair(() => {
      const was = this.highlight.visible;
      this.highlight.visible = false;
      return () => {
        this.highlight.visible = was;
      };
    });
    if (!pair) return null;
    const { drawn, bare } = pair;
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
export function mapPixels(): Promise<{ pixels: number; changed: number } | null> {
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

/** B3: the running solve's saved particles on the Simulate step (null clears); returns the look in force. */
export function showLive(p: Particles | null, look: ParticleLook, trails: number): ParticleLook {
  return engine.setLive(p, look, trails);
}

/** B3: the live clock, fractional steps. */
export function setLiveTime(t: number): void {
  engine.setLiveTime(t);
}

/** B3's hook. */
export const liveState = () => engine.liveState();

/** The layer itself, for the M12 test hooks (ResultsOverlay.tsx). */
export const resultsLayer = (): ResultsLayer => engine.results;

/** Draws now (a test hook's read must see the current step). */
export function renderNow(): void {
  engine.invalidate();
}

/** Decision 69: the particles' light, bloom off, soft or full. */
export function setGlow(g: Glow): void {
  engine.setGlow(g);
}

/** Round 2: the map at full strength while the light plays (else faded to 25 %). */
export function setMapWhilePlaying(full: boolean): void {
  engine.setMapWhilePlaying(full);
}

/** Round 2: the presentation view and its turntable. */
export function setPresent(on: boolean): void {
  engine.setPresent(on);
}
export function setTurntable(on: boolean): void {
  engine.setTurntable(on);
}

/** Frames the model in the perspective view (View › Frame model may call it). */
export function frameModel(): void {
  engine.frame();
}
