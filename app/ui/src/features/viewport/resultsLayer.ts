// The Results step's layer of the 3D view (M12 P3): the surface map and particle playback, both
// drawn at the shared Animator's step (animator.ts), both GPU-resident.
//
//   - The map: the `.csbin`'s own triangles (its nodes and faces, as SMAP v1 carries them), each
//     vertex tagged with its face. Its values live in one float32 texture, faces x steps,
//     face-major (mapData.ts), unfiltered: the vertex stage loads the texel of (face, step)
//     (`textureLoad`) and colours the face from it. A step changes one uniform, nothing else.
//     Difference from the baseline: a second texture of the same layout, subtracted in dB.
//   - Particles: every record of the band's `.pbin` uploaded once (position, step, energy); the
//     vertex stage keeps a record only at its own step. Changing the step uploads nothing.
//   - W5 (wow list; mapView.ts): smooth colour, upstream's node mean of the linked faces' texels
//     at the step, computed in the vertex stage from a node-to-faces texture, its level
//     interpolated across the face and coloured per fragment; iso-contours on that level
//     (`fwidth`); a fixed colour range (the lo/hi uniforms); and a BVH over the map's own
//     triangles for the value probe, whose number is the CPU's read of the face's record.
//   - W2 (cumulative.ts): a cumulative map is the same layout with each texel the face's running
//     sum (upstream's float32 rule), built on the CPU and uploaded in place of the instantaneous one.
//   - The time window (window.ts): `faceValue` is the mean of a face's texels over the last `win`
//     steps up to the step, a non-finite one as 0, divided by the steps the window holds; win 1
//     is the texel itself. The draw, the difference, the node mean and the read-back all go
//     through it, so a window changes one uniform and uploads nothing.
//   - W3 (particles.ts): trails, one line segment per pair of consecutive records, both ends
//     tagged with the head's step and the particle's last step; `keptTrail()` keeps a segment
//     whole. Counted like gate (d): the trail rule drawn as points in its count pass.
//   - Test hooks (gate (c), (d)) read what the GPU holds and draws, through the same code: texels
//     read back by a pass that calls the map's own `mapTexel` (mapNodes.ts) into a float32 target,
//     and the particle count by drawing every record through the draw's own keep rule
//     (`keptRecord`), additively into a 1 x 1 float32 target.
//
// Decision 68 (b): all of it is TSL (mapNodes.ts), so it runs on WebGPURenderer's WebGPU backend and
// on its WebGL2 fallback alike. Reading back from the GPU is asynchronous on both, so the hooks that
// read pixels return promises (the e2e harness awaits every hook).
import {
  AddEquation,
  BufferAttribute,
  Vector3,
  BufferGeometry,
  CustomBlending,
  DataTexture,
  DoubleSide,
  FloatType,
  Group,
  InstancedBufferAttribute,
  LineSegments,
  Mesh,
  NearestFilter,
  NoBlending,
  OneFactor,
  OrthographicCamera,
  type Ray,
  Points,
  RedFormat,
  RenderTarget,
  RGBAFormat,
  Scene,
  Sprite,
} from 'three';
import { LineBasicNodeMaterial, MeshBasicNodeMaterial, PointsNodeMaterial, type WebGPURenderer } from 'three/webgpu';
import { T } from './tsl';
import { MeshBVH } from 'three-mesh-bvh';
import { emptyGeometry } from './nodes';
import type { Particles, SurfaceMap } from '../../resultsData';
import { denseValues, mapLayout, type MapLayout, type Range } from './mapData';
import { cumulativeValues } from './cumulative';
import { MAX_NODE_FACES, nodeFaces, type NodeFaces } from './mapView';
import { MAX_WINDOW_STEPS } from './window';
import { keptRecord, keptTrail, recordSteps, TRAIL_BYTES_PER_SEGMENT, trailRefusal, trailSegments } from './particles';
import { GpuParticles, type ParticleLook } from './rays';
import { particleAt } from './raysData';
import { NO_REVEAL, revealKeep, revealReach, SPREAD_MAX } from './spread';
import { diffDb, faceLevel, faceValue, hot, lookUniforms, mapColour, mapTexel, mapUniforms, nodeEnergy, nodeLevel, NODE_OPS, rampPlaceNode, trailAlphaNode, type MapUniforms } from './mapNodes';

const { attribute, dot, float, Fn, instancedBufferAttribute, int, modelViewProjection, screenDPR, select, uniform, uniformArray, uv, varying, vec4 } = T;

/** Particle size on screen, device px. */
const PARTICLE_PX = 4;

/** What the map shows; the hooks report it. */
export interface MapMeta {
  run: string;
  path: string;
  bandHz: number | null;
  kind: 'level' | 'diff';
  range: Range;
  baseline: string | null;
  /** W2: the texture holds each face's running sum from the first step (cumulative.ts). */
  cumulative?: boolean;
  /** The time window in steps (window.ts), 1 for none. */
  windowSteps?: number;
  /**
   * Particle steps per map time bin (patch 0001, `recepteurs_surfaciques_pas_temps`): the timeline runs in
   * particle steps and the map's texture in its own bins, so a step shows bin floor(step / stepRatio). 1 (or
   * unset) when the map was stored at the particle step.
   */
  stepRatio?: number;
}

/** W5: how the map is drawn (smooth colour, contour spacing in dB, 0 for none). */
export interface MapLook {
  smooth: boolean;
  isoDb: number;
}

export interface ParticleMeta {
  run: string;
  bandHz: number;
  particles: number;
  records: number;
}

const floatTexture = (data: Float32Array, l: MapLayout): DataTexture => {
  const t = new DataTexture(data, l.width, l.height, RedFormat, FloatType);
  // No internalFormat: WebGPU would take it as its own format name; red + float is r32float / R32F on both backends.
  t.minFilter = NearestFilter;
  t.magFilter = NearestFilter;
  t.generateMipmaps = false;
  t.flipY = false;
  t.unpackAlignment = 4;
  t.needsUpdate = true;
  return t;
};

/** The textures' stand-in while no map is loaded (a texture binding is never empty). */
const PLACEHOLDER = floatTexture(new Float32Array(1), { faces: 1, steps: 1, width: 1, height: 1 });

/** Whether `r` can add float32 values by blending (the count passes): WebGPU's `float32-blendable`, WebGL2's `EXT_float_blend`. */
function floatBlendable(r: WebGPURenderer): boolean {
  const b = r.backend as unknown as { isWebGPUBackend?: boolean; extensions?: { has(name: string): boolean } };
  return b.isWebGPUBackend ? r.hasFeature('float32-blendable') : !!b.extensions?.has('EXT_float_blend');
}

/** The GPU's 2D texture size limit. */
export function maxTextureSizeOf(r: WebGPURenderer): number {
  const b = r.backend as unknown as { isWebGPUBackend?: boolean; device?: { limits: { maxTextureDimension2D: number } }; gl?: WebGL2RenderingContext };
  if (b.isWebGPUBackend && b.device) return b.device.limits.maxTextureDimension2D;
  if (b.gl) return b.gl.getParameter(b.gl.MAX_TEXTURE_SIZE) as number;
  return 16384;
}

/** A points material that writes `value` (computed in the vertex stage, flat) to pixel `index` of an n x 1 target. */
function readMaterial(index: unknown, n: unknown, value: unknown): PointsNodeMaterial {
  const m = new PointsNodeMaterial({ blending: NoBlending, depthTest: false, depthWrite: false });
  const i = index as ReturnType<typeof float>;
  m.vertexNode = vec4(i.mul(2).add(1).div(n as number).sub(1), 0, 0, 1);
  m.fragmentNode = varying(value as ReturnType<typeof vec4>).setInterpolation('flat');
  return m;
}

/** A points material that adds 1 to the one pixel of a 1 x 1 target for every vertex `kept` lets through. */
function countMaterial(kept: unknown): PointsNodeMaterial {
  const m = new PointsNodeMaterial({ transparent: true, depthTest: false, depthWrite: false });
  m.blending = CustomBlending;
  m.blendSrc = OneFactor;
  m.blendDst = OneFactor;
  m.blendEquation = AddEquation;
  m.blendSrcAlpha = OneFactor;
  m.blendDstAlpha = OneFactor;
  m.blendEquationAlpha = AddEquation;
  m.vertexNode = select(kept as ReturnType<typeof float>, vec4(0, 0, 0, 1), vec4(2, 2, 2, 1));
  m.fragmentNode = vec4(1, 0, 0, 1);
  return m;
}

export class ResultsLayer {
  readonly group = new Group();
  private renderer: WebGPURenderer | null = null;
  private maxTexture = 16384;
  private readonly mu: MapUniforms = mapUniforms(PLACEHOLDER);
  private readonly lu = lookUniforms();
  /** The map's opacity: 1 (opaque, as measured) except while the particles' light plays (round 2: 25 %). */
  private readonly mapAlpha = uniform(1);
  /** Item 10 (spread.ts): the reveal's sources and its front, metres (NO_REVEAL: the whole map). */
  private readonly revealPos = uniformArray(Array.from({ length: SPREAD_MAX }, () => new Vector3()), 'vec3');
  private readonly revealCount = uniform(0, 'int');
  private readonly revealR = uniform(NO_REVEAL);
  private readonly map: Mesh<BufferGeometry, MeshBasicNodeMaterial>;
  /** The playback: one sprite instance per record, its size 0 unless the record is at the step. */
  private particles: Sprite;
  private readonly pStep = uniform(0);
  private readonly pLogMax = uniform(0);
  private readonly pSize = uniform(PARTICLE_PX);
  /** The count pass's points: one per record, with the record's step. */
  private particleCount: Points | null = null;
  /** W3: the band's trail segments, drawn while `trailSteps` > 0. */
  private readonly trails: LineSegments<BufferGeometry, LineBasicNodeMaterial>;
  private readonly tStep = uniform(0);
  private readonly tLength = uniform(0);
  private readonly tLogMax = uniform(0);
  private trailData: { segments: number; bytes: number } | null = null;
  /** W3's trails are on (drawn as lines while the particles are dots; the GPU looks draw their own). */
  private trailsOn = false;
  private trailSource: Particles | null = null;
  /** W3: why trails cannot be drawn for the particles shown, or null. */
  trailRefusal: string | null = 'No particles saved for this run';
  private layout: MapLayout | null = null;
  private mapTex: DataTexture | null = null;
  private baseTex: DataTexture | null = null;
  /** W5: the node-to-faces list of the shown map, its texture, and the BVH of its triangles. */
  private adj: NodeFaces | null = null;
  private adjTex: DataTexture | null = null;
  private mapBvh: MeshBVH | null = null;
  /** Why smooth colour cannot be drawn for this map, or null. */
  smoothRefusal: string | null = null;
  mapMeta: MapMeta | null = null;
  particleMeta: ParticleMeta | null = null;
  private particleBytes = 0;
  private textureBytes = 0;
  /** The read-back passes' materials, built once each. */
  private readonly probes = new Map<string, PointsNodeMaterial>();
  private readonly probeN = uniform(1);
  /** B2: the GPU particle layer (rays.ts): compute-animated glow and rays over the same particles. */
  readonly gpu = new GpuParticles();

  constructor() {
    const mat = new MeshBasicNodeMaterial({ side: DoubleSide, polygonOffset: true, polygonOffsetFactor: -1, polygonOffsetUnits: -2 });
    const face = int(attribute('aFace', 'float').add(0.5));
    const flat = varying(faceLevel(this.mu, this.lu, face)).setInterpolation('flat');
    const smooth = varying(nodeLevel(this.mu, this.lu, int(attribute('aAdj', 'float').add(0.5)), int(attribute('aAdjN', 'float').add(0.5))));
    mat.fragmentNode = mapColour(this.lu, flat, smooth, this.mapAlpha, revealKeep(this.revealPos, this.revealCount, this.revealR));
    this.map = new Mesh(emptyGeometry(), mat);
    this.particles = new Sprite(new PointsNodeMaterial());
    const trailMat = new LineBasicNodeMaterial({ transparent: true, depthWrite: false });
    const head = attribute('aHead', 'float');
    trailMat.vertexNode = select(keptTrail(NODE_OPS, attribute('aLast', 'float'), head, this.tStep, this.tLength), modelViewProjection, vec4(2, 2, 2, 1));
    trailMat.colorNode = vec4(varying(hot(rampPlaceNode(attribute('aEnergy', 'float'), this.tLogMax))), varying(trailAlphaNode(this.tStep, head, this.tLength)));
    this.trails = new LineSegments(emptyGeometry(), trailMat);
    this.map.renderOrder = 1;
    this.particles.renderOrder = 9;
    this.trails.renderOrder = 8;
    this.map.frustumCulled = false;
    this.particles.frustumCulled = false;
    this.trails.frustumCulled = false;
    this.map.visible = false;
    this.particles.visible = false;
    this.trails.visible = false;
    // The GPU looks are light: the engine draws `gpu.group` into its own half-float target (decision 69).
    this.group.add(this.map, this.particles, this.trails);
  }

  setRenderer(r: WebGPURenderer | null): void {
    this.renderer = r;
    this.gpu.setRenderer(r);
    if (r) this.maxTexture = maxTextureSizeOf(r);
  }

  /** The GPU's texture size limit (16384 before the renderer is up, the WebGL2 floor's double). */
  maxTextureSize(): number {
    return this.maxTexture;
  }

  /** Uploads `m` (and `base` for a difference); returns why it cannot, or null. */
  setMap(m: SurfaceMap, meta: MapMeta, base: SurfaceMap | null): string | null {
    if (meta.kind === 'diff' && !base) throw new Error('a difference map needs its baseline');
    const l = mapLayout(m.faceCount, m.timeStepCount, this.maxTextureSize());
    if ('error' in l) {
      this.clearMap();
      return l.error;
    }
    this.disposeTextures();
    this.layout = l;
    this.mapTex = floatTexture(meta.cumulative && !base ? cumulativeValues(m, l) : denseValues(m, l), l);
    this.baseTex = base ? floatTexture(denseValues(base, l), l) : null;
    this.textureBytes = 4 * l.width * l.height * (base ? 2 : 1);
    // W5: the node mean's faces; above MAX_NODE_FACES a node's mean is not computed, so the map
    // is drawn flat and says why. Face indices as float32 are exact below 2^24.
    this.adj = nodeFaces(m);
    this.smoothRefusal = this.adj.maxFaces > MAX_NODE_FACES ? `a node of this map links ${this.adj.maxFaces} faces, more than the ${MAX_NODE_FACES} the smooth colouring averages` : m.faceCount >= 1 << 24 ? 'the map has too many faces for smooth colouring' : null;
    const adjN = Math.max(1, this.adj.faces.length);
    const adjW = Math.min(this.maxTextureSize(), adjN);
    const adjH = Math.ceil(adjN / adjW);
    const adjData = new Float32Array(adjW * adjH);
    adjData.set(this.adj.faces);
    this.adjTex = adjH <= this.maxTextureSize() ? floatTexture(adjData, { faces: 0, steps: 0, width: adjW, height: adjH }) : null;
    if (!this.adjTex) this.smoothRefusal ??= 'the node list is larger than one texture';
    this.textureBytes += this.adjTex ? 4 * adjW * adjH : 0;
    const pos = new Float32Array(9 * m.faceCount);
    const face = new Float32Array(3 * m.faceCount);
    const adjStart = new Float32Array(3 * m.faceCount);
    const adjCount = new Float32Array(3 * m.faceCount);
    for (let f = 0; f < m.faceCount; f++) {
      for (let c = 0; c < 3; c++) {
        const v = m.indices[3 * f + c];
        pos.set(m.positions.subarray(3 * v, 3 * v + 3), 9 * f + 3 * c);
        face[3 * f + c] = f;
        adjStart[3 * f + c] = this.adj.offsets[v];
        adjCount[3 * f + c] = Math.min(this.adj.offsets[v + 1] - this.adj.offsets[v], MAX_NODE_FACES);
      }
    }
    this.map.geometry.dispose();
    const g = emptyGeometry();
    g.setAttribute('position', new BufferAttribute(pos, 3));
    g.setAttribute('aFace', new BufferAttribute(face, 1));
    g.setAttribute('aAdj', new BufferAttribute(adjStart, 1));
    g.setAttribute('aAdjN', new BufferAttribute(adjCount, 1));
    // An index in face order, so the probe's BVH (indirect: it keeps its own order) names map faces.
    const index = new Uint32Array(3 * m.faceCount);
    for (let i = 0; i < index.length; i++) index[i] = i;
    g.setIndex(new BufferAttribute(index, 1));
    this.map.geometry = g;
    this.mapBvh = m.faceCount > 0 ? new MeshBVH(g, { indirect: true }) : null;
    const u = this.mu;
    u.map.value = this.mapTex;
    u.base.value = this.baseTex ?? this.mapTex;
    u.adj.value = this.adjTex ?? this.mapTex;
    u.adjWidth.value = adjW;
    if (this.smoothRefusal) this.lu.smooth.value = 0;
    u.width.value = l.width;
    u.steps.value = l.steps;
    this.lu.diff.value = meta.kind === 'diff' && base ? 1 : 0;
    u.win.value = this.windowOf(meta);
    this.lu.lo.value = meta.range.lo;
    this.lu.hi.value = meta.range.hi;
    this.mapMeta = meta;
    this.map.visible = true;
    return null;
  }

  /** The window a map is drawn with: none on a cumulative map (window.ts), at most MAX_WINDOW_STEPS. */
  private windowOf(meta: MapMeta): number {
    const w = Math.floor(meta.windowSteps ?? 1);
    return meta.cumulative || !(w > 1) ? 1 : Math.min(w, MAX_WINDOW_STEPS);
  }

  /** The time window, steps (1: each step on its own), without reloading the map. */
  setWindow(steps: number): void {
    if (!this.mapMeta) return;
    this.mapMeta = { ...this.mapMeta, windowSteps: steps };
    this.mu.win.value = this.windowOf(this.mapMeta);
  }

  /** The window the map is drawn with now, steps. */
  windowSteps(): number {
    return this.mu.win.value as number;
  }

  /** W5: smooth colour and contours; smooth stays off where `smoothRefusal` says why. */
  setLook(look: MapLook): void {
    this.lu.smooth.value = look.smooth && !this.smoothRefusal ? 1 : 0;
    this.lu.iso.value = look.smooth && look.isoDb > 0 ? look.isoDb : 0;
  }

  /** W5: the colour scale's range (a fixed range, R47), without reloading the map. */
  setRange(range: Range): void {
    this.lu.lo.value = range.lo;
    this.lu.hi.value = range.hi;
    if (this.mapMeta) this.mapMeta = { ...this.mapMeta, range };
  }

  look(): { smooth: boolean; isoDb: number; lo: number; hi: number } {
    const u = this.lu;
    return { smooth: u.smooth.value === 1, isoDb: u.iso.value as number, lo: u.lo.value as number, hi: u.hi.value as number };
  }

  /** The map face a ray meets first (either side), and how far; null off the map or with none shown. */
  pickMap(ray: Ray): { face: number; distance: number; point: [number, number, number] } | null {
    if (!this.mapBvh || !this.map.visible || !this.group.visible) return null;
    const hit = this.mapBvh.raycastFirst(ray, DoubleSide);
    if (!hit || typeof hit.faceIndex !== 'number') return null;
    return { face: hit.faceIndex, distance: hit.distance, point: [hit.point.x, hit.point.y, hit.point.z] };
  }

  /** A map face's centroid, from the drawn positions. */
  faceCentroid(face: number): [number, number, number] | null {
    const p = this.map.geometry.getAttribute('position');
    if (!p || !(face >= 0 && 3 * face + 2 < p.count)) return null;
    const out: [number, number, number] = [0, 0, 0];
    for (let c = 0; c < 3; c++) for (let i = 0; i < 3; i++) out[i] += (p.array[9 * face + 3 * c + i] as number) / 3;
    return out;
  }

  /** A read-back pass's material, built on first use: `make` gets the sample attribute and returns the vec4 to write. */
  private probe(key: string, size: number, make: (s: ReturnType<typeof attribute>) => unknown): PointsNodeMaterial {
    let m = this.probes.get(key);
    if (!m) {
      const s = attribute('aSample', size === 4 ? 'vec4' : 'vec3');
      // Built inside a TSL function: the window and node loops are statements, which need a function body.
      m = readMaterial(size === 4 ? s.w : s.z, this.probeN, Fn(() => make(s))());
      this.probes.set(key, m);
    }
    return m;
  }

  /** W5's read-back: each node's mean energy at a step, computed by the map's own `nodeEnergy`, as float32 bits. */
  async readNodes(samples: [number, number][]): Promise<number[]> {
    const l = this.layout;
    const adj = this.adj;
    if (!l || !this.mapTex || !adj || !this.adjTex) throw new Error('no map with a node list loaded');
    const nodes = adj.offsets.length - 1;
    for (const [n, s] of samples) {
      if (!(n >= 0 && n < nodes && s >= 0 && s < l.steps)) throw new Error(`no node ${n} or step ${s} in ${nodes} nodes x ${l.steps} steps`);
      if (adj.offsets[n + 1] - adj.offsets[n] > MAX_NODE_FACES) throw new Error(`node ${n} links more than ${MAX_NODE_FACES} faces`);
    }
    const n = samples.length;
    const mat = this.probe('node', 4, (s) => vec4(nodeEnergy(this.mu, this.mu.map, int(s.x.add(0.5)), int(s.y.add(0.5)), int(s.z.add(0.5))), 0, 0, 1));
    const g = emptyGeometry();
    g.setAttribute('position', new BufferAttribute(new Float32Array(3 * n), 3));
    g.setAttribute('aSample', new BufferAttribute(new Float32Array(samples.flatMap(([v, s], i) => [adj.offsets[v], adj.offsets[v + 1] - adj.offsets[v], s, i])), 4));
    try {
      const px = await this.readPass(n, new Points(g, mat));
      const bits = new Uint32Array(px.buffer, px.byteOffset, px.length);
      return samples.map((_, i) => bits[4 * i]);
    } finally {
      g.dispose();
    }
  }

  clearMap(): void {
    this.disposeTextures();
    this.map.geometry.dispose();
    this.map.geometry = emptyGeometry();
    this.map.visible = false;
    this.mapMeta = null;
    this.layout = null;
    this.adj = null;
    this.mapBvh = null;
    this.smoothRefusal = null;
  }

  private disposeTextures(): void {
    this.mu.map.value = PLACEHOLDER;
    this.mu.base.value = PLACEHOLDER;
    this.mu.adj.value = PLACEHOLDER;
    this.mapTex?.dispose();
    this.baseTex?.dispose();
    this.adjTex?.dispose();
    this.mapTex = null;
    this.baseTex = null;
    this.adjTex = null;
    this.textureBytes = 0;
  }

  setParticles(p: Particles | null, meta: ParticleMeta | null): void {
    this.group.remove(this.particles);
    (this.particles.material as PointsNodeMaterial).dispose();
    this.particleCount?.geometry.dispose();
    (this.particleCount?.material as PointsNodeMaterial | undefined)?.dispose();
    this.particleCount = null;
    this.particleBytes = 0;
    let logMax = 0;
    const mat = new PointsNodeMaterial({ transparent: true, depthWrite: false, sizeAttenuation: false });
    mat.alphaToCoverage = false;
    this.particles = new Sprite(mat);
    if (p && meta) {
      const steps = recordSteps(p);
      for (let k = 0; k < p.recordCount; k++) if (p.energies[k] > logMax && Number.isFinite(p.energies[k])) logMax = p.energies[k];
      logMax = logMax > 0 ? 10 * Math.log10(logMax) : 0;
      // The draw: a sprite instance per record, sized 0 unless the record is at the step (keptRecord).
      const step = instancedBufferAttribute(new InstancedBufferAttribute(steps, 1));
      mat.positionNode = instancedBufferAttribute(new InstancedBufferAttribute(p.positions, 3));
      mat.sizeNode = select(keptRecord(NODE_OPS, step, this.pStep), this.pSize.div(screenDPR), float(0));
      mat.colorNode = vec4(varying(hot(rampPlaceNode(instancedBufferAttribute(new InstancedBufferAttribute(p.energies, 1)), this.pLogMax))), 1);
      const d = uv().sub(0.5);
      mat.maskNode = dot(d, d).lessThanEqual(0.25);
      this.particles.count = p.recordCount;
      // The count pass: the same records and the same keep rule, as points.
      const cg = emptyGeometry();
      cg.setAttribute('position', new BufferAttribute(p.positions, 3));
      cg.setAttribute('aStep', new BufferAttribute(steps, 1));
      this.particleCount = new Points(cg, countMaterial(keptRecord(NODE_OPS, attribute('aStep', 'float'), this.pStep)));
      this.particleCount.frustumCulled = false;
      this.particleBytes = 20 * p.recordCount;
    }
    this.pLogMax.value = logMax;
    this.particles.renderOrder = 9;
    this.particles.frustumCulled = false;
    this.group.add(this.particles);
    this.particles.visible = !!p;
    this.particleMeta = p ? meta : null;
    this.trailSource = p && meta ? p : null;
    this.trails.geometry.dispose();
    this.trails.geometry = emptyGeometry();
    this.trailData = null;
    this.trailRefusal = trailRefusal(this.trailSource);
    this.tLogMax.value = logMax;
    this.setTrails(this.trailSteps());
    this.gpu.setParticles(p && meta ? p : null, logMax);
    this.setParticleLook(this.gpu.currentLook());
  }

  /** B2: how the particles are drawn; returns the look in force ('dots' where the GPU looks are refused). */
  setParticleLook(look: ParticleLook): ParticleLook {
    const inForce = this.gpu.setLook(look);
    // The per-record playback draws only as dots; its count pass (gate (d)) stays whichever look is drawn.
    this.particles.visible = !!this.particleMeta && inForce === 'dots';
    this.trails.visible = this.trailsOn && inForce === 'dots';
    return inForce;
  }

  /** Why a particle look cannot be drawn now, or null. */
  particleLookRefusal(look: ParticleLook): string | null {
    return this.gpu.refusal(look);
  }

  /** The timeline's fractional step while it plays (the GPU looks move smoothly between records). */
  setTime(t: number): void {
    this.gpu.setTime(t);
  }

  /**
   * B2's check: the compute pass's particles at fractional step `t` against raysData.ts `particleAt` on
   * the CPU, for `indices` (the file's own particles): the largest difference of position (m) and of
   * energy (relative), and how many agree on alive or dead.
   */
  async checkGpuParticles(indices: number[], t: number): Promise<{ t: number; checked: number; aliveAgree: number; alive: number; maxPosM: number; maxEnergyRel: number }> {
    const p = this.trailSource;
    if (!p) throw new Error('no particles loaded');
    this.gpu.setTime(t);
    const gpu = await this.gpu.readState(indices);
    let agree = 0;
    let alive = 0;
    let dp = 0;
    let de = 0;
    indices.forEach((i, k) => {
      const cpu = particleAt(p, i, t);
      const g = gpu[k];
      if ((cpu === null) === (g[3] < 0)) agree++;
      if (!cpu || g[3] < 0) return;
      alive++;
      dp = Math.max(dp, Math.hypot(cpu[0] - g[0], cpu[1] - g[1], cpu[2] - g[2]));
      de = Math.max(de, Math.abs(cpu[3] - g[3]) / Math.max(Math.abs(cpu[3]), 1e-30));
    });
    return { t, checked: indices.length, aliveAgree: agree, alive, maxPosM: dp, maxEnergyRel: de };
  }

  /** The GPU particles' compute pass for this frame (before the draw). */
  computeFrame(): void {
    this.gpu.update();
  }

  /** W3: the trails' length in steps (0: off). Built on first use; refused where `trailRefusal` says why. */
  setTrails(steps: number): void {
    const on = steps > 0 && this.trailRefusal === null && this.trailSource !== null;
    this.tLength.value = steps > 0 ? steps : 0;
    if (on && !this.trailData) {
      const t = trailSegments(this.trailSource as Particles);
      const g = emptyGeometry();
      g.setAttribute('position', new BufferAttribute(t.positions, 3));
      g.setAttribute('aHead', new BufferAttribute(t.head, 1));
      g.setAttribute('aLast', new BufferAttribute(t.last, 1));
      g.setAttribute('aEnergy', new BufferAttribute(t.energy, 1));
      this.trails.geometry.dispose();
      this.trails.geometry = g;
      this.trailData = { segments: t.segments, bytes: TRAIL_BYTES_PER_SEGMENT * t.segments };
    }
    this.trailsOn = on;
    this.trails.visible = on && this.gpu.currentLook() === 'dots';
    this.gpu.setTrailSteps(on ? steps : 0);
  }

  trailSteps(): number {
    return this.tLength.value as number;
  }

  /** W3's hook: what the trails hold and draw now. */
  trailState(): { steps: number; on: boolean; segments: number; bytes: number; refusal: string | null } {
    return { steps: this.trailSteps(), on: this.trailsOn, segments: this.trailData?.segments ?? 0, bytes: this.trailData?.bytes ?? 0, refusal: this.trailRefusal };
  }

  private trailCounter: Points | null = null;

  /** W3: the segments the trail draw keeps at its step, counted on the GPU (two points a segment, the draw's keptTrail rule). */
  async countTrails(): Promise<number> {
    const r = this.renderer;
    if (!r) throw new Error('no renderer');
    if (!this.trailsOn || !this.trailData) throw new Error('no trails drawn');
    if (!floatBlendable(r)) throw new Error('float32 blending is not available: float32 additive counting cannot run');
    if (!this.trailCounter) {
      this.trailCounter = new Points(this.trails.geometry, countMaterial(keptTrail(NODE_OPS, attribute('aLast', 'float'), attribute('aHead', 'float'), this.tStep, this.tLength)));
      this.trailCounter.frustumCulled = false;
    }
    this.trailCounter.geometry = this.trails.geometry;
    const px = await this.readPass(1, this.trailCounter);
    return px[0] / 2;
  }

  /** The timeline's step, for both: the map at its bin for the step (stepRatio), clamped to its own bins (TCR has one). */
  setStep(step: number): void {
    const steps = this.layout?.steps ?? 1;
    const ratio = Math.max(1, Math.floor(this.mapMeta?.stepRatio ?? 1));
    this.lu.step.value = Math.max(0, Math.min(steps - 1, Math.floor(step / ratio)));
    this.pStep.value = step;
    this.tStep.value = step;
    this.gpu.setTime(step);
  }

  /** The map faded to `a` (1: as measured, opaque, drawn exactly as before); see-through below 1. */
  setMapFade(a: number): void {
    const m = this.map.material;
    const faded = a < 1;
    if (m.transparent !== faded) {
      m.transparent = faded;
      m.depthWrite = !faded;
      m.needsUpdate = true;
    }
    this.mapAlpha.value = a;
  }

  /** Item 10: the reveal's sources (the first SPREAD_MAX) and how far the front must go to pass every map vertex; 0 without a map. */
  setRevealSources(sources: readonly (readonly number[])[]): number {
    const n = Math.min(sources.length, SPREAD_MAX);
    for (let i = 0; i < n; i++) (this.revealPos.array[i] as Vector3).set(sources[i][0], sources[i][1], sources[i][2]);
    this.revealCount.value = n;
    const p = this.map.geometry.getAttribute('position');
    return p && this.mapMeta ? revealReach(p.array as Float32Array, sources.slice(0, n)) : 0;
  }

  /** Item 10: the reveal's front, metres from the nearest source; NO_REVEAL draws the whole map. */
  setRevealRadius(r: number): void {
    this.revealR.value = r;
  }

  revealRadius(): number {
    return this.revealR.value as number;
  }

  /** Hides the map for one draw (the m12 pixel hook); returns the restore. */
  hideMapFor(): () => void {
    const was = this.map.visible;
    this.map.visible = false;
    return () => {
      this.map.visible = was;
    };
  }

  /** Shown only on the Results step. */
  setShown(on: boolean): void {
    this.group.visible = on;
  }

  mapStep(): number | null {
    return this.mapMeta ? (this.lu.step.value as number) : null;
  }

  particleStep(): number {
    return this.pStep.value as number;
  }

  sizes() {
    const l = this.layout;
    return {
      faces: l?.faces ?? 0,
      steps: l?.steps ?? 0,
      texture: l ? { type: 'float32', format: 'R32F', filter: 'nearest', width: l.width, height: l.height, bytes: this.textureBytes } : null,
      particleBytes: this.particleBytes,
    };
  }

  /** Draws `object` alone into a fresh n x 1 float32 target and returns its RGBA floats. */
  private async readPass(n: number, object: Points): Promise<Float32Array> {
    const r = this.renderer;
    if (!r) throw new Error('no renderer');
    const target = new RenderTarget(n, 1, { type: FloatType, format: RGBAFormat, minFilter: NearestFilter, magFilter: NearestFilter, depthBuffer: false });
    const prev = r.getRenderTarget();
    const scene = new Scene();
    object.frustumCulled = false;
    scene.add(object);
    this.probeN.value = n;
    try {
      r.setScissorTest(false);
      r.setRenderTarget(target);
      r.setClearColor(0x000000, 0);
      r.clear(true, false, false);
      r.render(scene, new OrthographicCamera());
      r.setRenderTarget(prev);
      const px = (await r.readRenderTargetPixelsAsync(target, 0, 0, n, 1)) as Float32Array;
      return px.slice(0, 4 * n);
    } finally {
      scene.remove(object);
      r.setRenderTarget(prev);
      target.dispose();
    }
  }

  /**
   * The texture's texels at (face, step), as float32 bits, read back from the GPU: 'texel' the
   * texture as held, 'drawn' the face's value the draw colours (`faceValue`, the window applied),
   * 'diff' the difference in dB the draw colours (the window applied to both runs).
   */
  async readTexels(samples: [number, number][], what: 'texel' | 'drawn' | 'diff' = 'texel'): Promise<number[]> {
    const l = this.layout;
    if (!l || !this.mapTex) throw new Error('no map loaded');
    if (what === 'diff' && !this.baseTex) throw new Error('no baseline loaded');
    for (const [f, s] of samples) {
      if (!(f >= 0 && f < l.faces && s >= 0 && s < l.steps)) throw new Error(`no cell (${f}, ${s}) in ${l.faces} faces x ${l.steps} steps`);
    }
    const n = samples.length;
    const mu = this.mu;
    const mat = this.probe(what, 3, (s) => {
      const face = int(s.x.add(0.5));
      const step = int(s.y.add(0.5));
      if (what === 'diff') {
        const r = diffDb(mu, face, step);
        return vec4(r.d, select(r.ok, float(1), float(0)), 0, 1);
      }
      return vec4(what === 'drawn' ? faceValue(mu, mu.map, face, step) : mapTexel(mu, mu.map, face, step), 0, 0, 1);
    });
    const g = emptyGeometry();
    g.setAttribute('position', new BufferAttribute(new Float32Array(3 * n), 3));
    g.setAttribute('aSample', new BufferAttribute(new Float32Array(samples.flatMap(([f, s], i) => [f, s, i])), 3));
    try {
      const px = await this.readPass(n, new Points(g, mat));
      const bits = new Uint32Array(px.buffer, px.byteOffset, px.length);
      return samples.map((_, i) => {
        if (what === 'diff' && px[4 * i + 1] !== 1) throw new Error(`cell (${samples[i][0]}, ${samples[i][1]}) has no energy in one of the runs`);
        return bits[4 * i];
      });
    } finally {
      g.dispose();
    }
  }

  /**
   * The particles the playback draw lets through at its current step, counted on the GPU: every
   * record through the draw's own keep rule (`keptRecord`), each kept one adding 1 to one float32 pixel.
   */
  async countParticles(): Promise<number> {
    const r = this.renderer;
    if (!r) throw new Error('no renderer');
    if (!this.particleMeta || !this.particleCount) throw new Error('no particles loaded');
    if (!floatBlendable(r)) throw new Error('float32 blending is not available: float32 additive counting cannot run');
    const px = await this.readPass(1, this.particleCount);
    return px[0];
  }
}
