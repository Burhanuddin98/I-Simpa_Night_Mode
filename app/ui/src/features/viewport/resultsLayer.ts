// The Results step's layer of the 3D view (M12 P3): the surface map and particle playback, both
// drawn at the shared Animator's step (animator.ts), both GPU-resident.
//
//   - The map: the `.csbin`'s own triangles (its nodes and faces, as SMAP v1 carries them), each
//     vertex tagged with its face. Its values live in one float32 texture, faces x steps,
//     face-major (mapData.ts), unfiltered: the vertex shader fetches the texel of (face, step)
//     with `texelFetch` and colours the face from it. A step changes one uniform, nothing else.
//     Difference from the baseline: a second texture of the same layout, subtracted in dB.
//   - Particles: every record of the band's `.pbin` uploaded once (position, step, energy); the
//     vertex shader keeps a record only at its own step. Changing the step uploads nothing.
//   - W5 (wow list; mapView.ts): smooth colour, upstream's node mean of the linked faces' texels
//     at the step, computed in the vertex shader from a node-to-faces texture, its level
//     interpolated across the face and coloured per fragment; iso-contours on that level
//     (`fwidth`); a fixed colour range (the uLo/uHi uniforms); and a BVH over the map's own
//     triangles for the value probe, whose number is the CPU's read of the face's record.
//   - Test hooks (gate (c), (d)) read what the GPU holds and draws, through the same GLSL: texels
//     read back by a pass that calls the map shader's own `mapTexel` into a float32 target, and
//     the particle count by drawing the particle material itself, in its count mode, additively
//     into a 1 x 1 float32 target.
import {
  BufferAttribute,
  BufferGeometry,
  CustomBlending,
  DataTexture,
  DoubleSide,
  FloatType,
  Group,
  Mesh,
  NearestFilter,
  NoBlending,
  OneFactor,
  AddEquation,
  NormalBlending,
  OrthographicCamera,
  type Ray,
  Points,
  RedFormat,
  RGBAFormat,
  Scene,
  ShaderMaterial,
  WebGLRenderTarget,
  type WebGLRenderer,
} from 'three';
import { MeshBVH } from 'three-mesh-bvh';
import type { Particles, SurfaceMap } from '../../resultsData';
import { COOL, denseValues, HOT, mapLayout, rampFloats, type MapLayout, type Range } from './mapData';
import { MAX_NODE_FACES, nodeFaces, type NodeFaces } from './mapView';
import { PARTICLE_FRAGMENT_GLSL, PARTICLE_VERTEX_GLSL, recordSteps } from './particles';

/** The GLSL that reads the map: the draw and the read-back hook share it. */
const MAP_GLSL = /* glsl */ `
uniform highp sampler2D uMap;
uniform highp sampler2D uBase;
uniform int uWidth;
uniform int uSteps;
float mapTexel(highp sampler2D t, int face, int step) {
  int i = face * uSteps + step;
  return texelFetch(t, ivec2(i % uWidth, i / uWidth), 0).r;
}
// Upstream's level, dB re 1e-12; the caller checks e > 0.
float levelDb(float e) { return 10.0 * log2(e) * 0.30102999566398120 + 120.0; }
// This run's level minus the baseline's; 'ok' false where either has no energy.
float diffDb(int face, int step, out bool ok) {
  float a = mapTexel(uMap, face, step);
  float b = mapTexel(uBase, face, step);
  ok = a > 0.0 && b > 0.0 && !isinf(a) && !isinf(b) && !isnan(a) && !isnan(b);
  return ok ? levelDb(a) - levelDb(b) : 0.0;
}
`;

/** W5: a node's energy, upstream's mean of its linked faces' texels (mapView.ts `nodeEnergy`). */
const NODE_GLSL = /* glsl */ `
uniform highp sampler2D uAdj;
uniform int uAdjWidth;
float nodeEnergy(highp sampler2D t, int start, int n, int step) {
  float s = 0.0;
  for (int k = 0; k < ${MAX_NODE_FACES}; k++) {
    if (k >= n) break;
    int j = start + k;
    int face = int(texelFetch(uAdj, ivec2(j % uAdjWidth, j / uAdjWidth), 0).r + 0.5);
    float e = mapTexel(t, face, step);
    if (!isinf(e) && !isnan(e)) s += e;
  }
  return n > 0 ? s / float(n) : 0.0;
}
`;

const RAMP_GLSL = /* glsl */ `
uniform vec3 uHot[9];
uniform vec3 uCool[8];
vec3 hot(float t) {
  float u = clamp(t, 0.0, 1.0) * 8.0;
  int k = min(7, int(floor(u)));
  return mix(uHot[k], uHot[k + 1], u - float(k));
}
vec3 cool(float t) {
  float u = clamp(t, 0.0, 1.0) * 7.0;
  int k = min(6, int(floor(u)));
  return mix(uCool[k], uCool[k + 1], u - float(k));
}
`;

/** A `vec3[]` uniform's value: three's array setters take it flat. */
const vec3s = (stops: readonly string[]) => new Float32Array(rampFloats(stops));

function mapMaterial(): ShaderMaterial {
  return new ShaderMaterial({
    uniforms: {
      uMap: { value: null },
      uBase: { value: null },
      uAdj: { value: null },
      uAdjWidth: { value: 1 },
      uWidth: { value: 1 },
      uSteps: { value: 1 },
      uStep: { value: 0 },
      uDiff: { value: 0 },
      uSmooth: { value: 0 },
      uIso: { value: 0 },
      uLo: { value: 0 },
      uHi: { value: 1 },
      uHot: { value: vec3s(HOT) },
      uCool: { value: vec3s(COOL) },
    },
    vertexShader: /* glsl */ `
      ${MAP_GLSL}
      ${NODE_GLSL}
      uniform int uStep;
      uniform int uDiff;
      uniform int uSmooth;
      attribute float aFace;
      attribute float aAdj;
      attribute float aAdjN;
      flat varying float vFaceOk;
      flat varying float vFaceLevel;
      varying float vLevel;
      varying float vOk;
      void main() {
        int face = int(aFace + 0.5);
        vFaceOk = 0.0;
        vFaceLevel = 0.0;
        if (uDiff == 1) {
          bool ok;
          float d = diffDb(face, uStep, ok);
          if (ok) { vFaceOk = 1.0; vFaceLevel = d; }
        } else {
          float e = mapTexel(uMap, face, uStep);
          if (e > 0.0 && !isinf(e) && !isnan(e)) { vFaceOk = 1.0; vFaceLevel = levelDb(e); }
        }
        vLevel = 0.0;
        vOk = 0.0;
        if (uSmooth == 1) {
          int start = int(aAdj + 0.5);
          int n = int(aAdjN + 0.5);
          float a = nodeEnergy(uMap, start, n, uStep);
          if (uDiff == 1) {
            float b = nodeEnergy(uBase, start, n, uStep);
            if (a > 0.0 && b > 0.0) { vOk = 1.0; vLevel = levelDb(a) - levelDb(b); }
          } else if (a > 0.0) { vOk = 1.0; vLevel = levelDb(a); }
        }
        gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
      }
    `,
    fragmentShader: /* glsl */ `
      ${RAMP_GLSL}
      uniform int uDiff;
      uniform int uSmooth;
      uniform float uIso;
      uniform float uLo;
      uniform float uHi;
      flat varying float vFaceOk;
      flat varying float vFaceLevel;
      varying float vLevel;
      varying float vOk;
      vec3 colourOf(float l) {
        if (uDiff == 1) {
          float t = l / max(uHi, 1e-6);
          return t < 0.0 ? cool(-t) : hot(t);
        }
        return hot((l - uLo) / max(uHi - uLo, 1e-6));
      }
      void main() {
        if (vFaceOk < 0.5) discard;
        // Smooth where every corner of the face has energy, else the face's own flat colour.
        bool smoothHere = uSmooth == 1 && vOk > 0.999;
        float l = smoothHere ? vLevel : vFaceLevel;
        vec3 c = colourOf(l);
        if (smoothHere && uIso > 0.0) {
          float f = l / uIso;
          float w = max(fwidth(f), 1e-6);
          float d = abs(fract(f + 0.5) - 0.5);
          float line = 1.0 - smoothstep(0.5 * w, 1.5 * w, d);
          c = mix(c, vec3(0.93, 0.93, 0.94), 0.85 * line);
        }
        gl_FragColor = vec4(c, 1.0);
      }
    `,
    side: DoubleSide,
    polygonOffset: true,
    polygonOffsetFactor: -1,
    polygonOffsetUnits: -2,
  });
}

/** W5's read-back: point i writes the node mean (nodeEnergy) of sample i (start, count, step) to pixel i. */
function nodeProbeMaterial(): ShaderMaterial {
  return new ShaderMaterial({
    uniforms: { uMap: { value: null }, uBase: { value: null }, uAdj: { value: null }, uAdjWidth: { value: 1 }, uWidth: { value: 1 }, uSteps: { value: 1 }, uN: { value: 1 } },
    vertexShader: /* glsl */ `
      ${MAP_GLSL}
      ${NODE_GLSL}
      uniform float uN;
      attribute vec4 aSample; // start, count, step, index
      flat varying vec4 vValue;
      void main() {
        vValue = vec4(nodeEnergy(uMap, int(aSample.x + 0.5), int(aSample.y + 0.5), int(aSample.z + 0.5)), 0.0, 0.0, 1.0);
        gl_Position = vec4((2.0 * aSample.w + 1.0) / uN - 1.0, 0.0, 0.0, 1.0);
        gl_PointSize = 1.0;
      }
    `,
    fragmentShader: /* glsl */ `
      flat varying vec4 vValue;
      void main() { gl_FragColor = vValue; }
    `,
    blending: NoBlending,
    depthTest: false,
    depthWrite: false,
  });
}

/** The read-back pass: point i of the draw writes mapTexel (or diffDb) of sample i to pixel i. */
function probeMaterial(): ShaderMaterial {
  return new ShaderMaterial({
    uniforms: { uMap: { value: null }, uBase: { value: null }, uWidth: { value: 1 }, uSteps: { value: 1 }, uWhat: { value: 0 }, uN: { value: 1 } },
    vertexShader: /* glsl */ `
      ${MAP_GLSL}
      uniform int uWhat;
      uniform float uN;
      attribute vec3 aSample; // face, step, index
      flat varying vec4 vValue;
      void main() {
        int face = int(aSample.x + 0.5);
        int step = int(aSample.y + 0.5);
        if (uWhat == 1) {
          bool ok;
          float d = diffDb(face, step, ok);
          vValue = vec4(d, ok ? 1.0 : 0.0, 0.0, 1.0);
        } else {
          vValue = vec4(mapTexel(uMap, face, step), 0.0, 0.0, 1.0);
        }
        gl_Position = vec4((2.0 * aSample.z + 1.0) / uN - 1.0, 0.0, 0.0, 1.0);
        gl_PointSize = 1.0;
      }
    `,
    fragmentShader: /* glsl */ `
      flat varying vec4 vValue;
      void main() { gl_FragColor = vValue; }
    `,
    blending: NoBlending,
    depthTest: false,
    depthWrite: false,
  });
}

/** Particle size on screen, px. */
const PARTICLE_PX = 4;

function particleMaterial(): ShaderMaterial {
  return new ShaderMaterial({
    uniforms: { uStep: { value: 0 }, uCount: { value: 0 }, uSize: { value: PARTICLE_PX }, uLogMax: { value: 0 }, uHot: { value: vec3s(HOT) }, uCool: { value: vec3s(COOL) } },
    vertexShader: `${RAMP_GLSL}
${PARTICLE_VERTEX_GLSL}`,
    fragmentShader: PARTICLE_FRAGMENT_GLSL,
    transparent: true,
    depthWrite: false,
  });
}

/** What the map shows; the hooks report it. */
export interface MapMeta {
  run: string;
  path: string;
  bandHz: number | null;
  kind: 'level' | 'diff';
  range: Range;
  baseline: string | null;
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
  t.internalFormat = 'R32F';
  t.minFilter = NearestFilter;
  t.magFilter = NearestFilter;
  t.generateMipmaps = false;
  t.flipY = false;
  t.unpackAlignment = 4;
  t.needsUpdate = true;
  return t;
};

export class ResultsLayer {
  readonly group = new Group();
  private renderer: WebGLRenderer | null = null;
  private readonly map = new Mesh(new BufferGeometry(), mapMaterial());
  private readonly particles = new Points(new BufferGeometry(), particleMaterial());
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

  constructor() {
    this.map.renderOrder = 1;
    this.particles.renderOrder = 9;
    this.map.frustumCulled = false;
    this.particles.frustumCulled = false;
    this.map.visible = false;
    this.particles.visible = false;
    this.group.add(this.map, this.particles);
  }

  setRenderer(r: WebGLRenderer | null): void {
    this.renderer = r;
  }

  /** The GPU's texture size limit (16384 without a renderer, the WebGL2 floor's double). */
  maxTextureSize(): number {
    return this.renderer?.capabilities.maxTextureSize ?? 16384;
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
    this.mapTex = floatTexture(denseValues(m, l), l);
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
    const g = new BufferGeometry();
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
    const u = this.map.material.uniforms;
    u.uMap.value = this.mapTex;
    u.uBase.value = this.baseTex ?? this.mapTex;
    u.uAdj.value = this.adjTex ?? this.mapTex;
    u.uAdjWidth.value = adjW;
    if (this.smoothRefusal) u.uSmooth.value = 0;
    u.uWidth.value = l.width;
    u.uSteps.value = l.steps;
    u.uDiff.value = meta.kind === 'diff' && base ? 1 : 0;
    u.uLo.value = meta.range.lo;
    u.uHi.value = meta.range.hi;
    this.mapMeta = meta;
    this.map.visible = true;
    return null;
  }

  /** W5: smooth colour and contours; smooth stays off where `smoothRefusal` says why. */
  setLook(look: MapLook): void {
    const u = this.map.material.uniforms;
    u.uSmooth.value = look.smooth && !this.smoothRefusal ? 1 : 0;
    u.uIso.value = look.smooth && look.isoDb > 0 ? look.isoDb : 0;
  }

  /** W5: the colour scale's range (a fixed range, R47), without reloading the map. */
  setRange(range: Range): void {
    const u = this.map.material.uniforms;
    u.uLo.value = range.lo;
    u.uHi.value = range.hi;
    if (this.mapMeta) this.mapMeta = { ...this.mapMeta, range };
  }

  look(): { smooth: boolean; isoDb: number; lo: number; hi: number } {
    const u = this.map.material.uniforms;
    return { smooth: u.uSmooth.value === 1, isoDb: u.uIso.value as number, lo: u.uLo.value as number, hi: u.uHi.value as number };
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

  /** W5's read-back: each node's mean energy at a step, computed by the map shader's `nodeEnergy`, as float32 bits. */
  readNodes(samples: [number, number][]): number[] {
    const l = this.layout;
    const adj = this.adj;
    if (!l || !this.mapTex || !adj || !this.adjTex) throw new Error('no map with a node list loaded');
    const nodes = adj.offsets.length - 1;
    for (const [n, s] of samples) {
      if (!(n >= 0 && n < nodes && s >= 0 && s < l.steps)) throw new Error(`no node ${n} or step ${s} in ${nodes} nodes x ${l.steps} steps`);
      if (adj.offsets[n + 1] - adj.offsets[n] > MAX_NODE_FACES) throw new Error(`node ${n} links more than ${MAX_NODE_FACES} faces`);
    }
    const n = samples.length;
    const mat = nodeProbeMaterial();
    const g = new BufferGeometry();
    g.setAttribute('position', new BufferAttribute(new Float32Array(3 * n), 3));
    g.setAttribute('aSample', new BufferAttribute(new Float32Array(samples.flatMap(([v, s], i) => [adj.offsets[v], adj.offsets[v + 1] - adj.offsets[v], s, i])), 4));
    const pts = new Points(g, mat);
    pts.frustumCulled = false;
    const scene = new Scene();
    scene.add(pts);
    const u = mat.uniforms;
    u.uMap.value = this.mapTex;
    u.uBase.value = this.mapTex;
    u.uAdj.value = this.adjTex;
    u.uAdjWidth.value = this.map.material.uniforms.uAdjWidth.value;
    u.uWidth.value = l.width;
    u.uSteps.value = l.steps;
    u.uN.value = n;
    try {
      const px = this.readPass(n, (r) => r.render(scene, new OrthographicCamera()));
      const bits = new Uint32Array(px.buffer);
      return samples.map((_, i) => bits[4 * i]);
    } finally {
      g.dispose();
      mat.dispose();
    }
  }

  clearMap(): void {
    this.disposeTextures();
    this.map.geometry.dispose();
    this.map.geometry = new BufferGeometry();
    this.map.visible = false;
    this.mapMeta = null;
    this.layout = null;
    this.adj = null;
    this.mapBvh = null;
    this.smoothRefusal = null;
  }

  private disposeTextures(): void {
    this.mapTex?.dispose();
    this.baseTex?.dispose();
    this.adjTex?.dispose();
    this.mapTex = null;
    this.baseTex = null;
    this.adjTex = null;
    this.textureBytes = 0;
  }

  setParticles(p: Particles | null, meta: ParticleMeta | null): void {
    this.particles.geometry.dispose();
    const g = new BufferGeometry();
    this.particleBytes = 0;
    if (p && meta) {
      g.setAttribute('position', new BufferAttribute(p.positions, 3));
      g.setAttribute('aStep', new BufferAttribute(recordSteps(p), 1));
      g.setAttribute('aEnergy', new BufferAttribute(p.energies, 1));
      this.particleBytes = 20 * p.recordCount;
      let max = 0;
      for (let k = 0; k < p.recordCount; k++) if (p.energies[k] > max && Number.isFinite(p.energies[k])) max = p.energies[k];
      this.particles.material.uniforms.uLogMax.value = max > 0 ? 10 * Math.log10(max) : 0;
    }
    this.particles.geometry = g;
    this.particles.visible = !!p;
    this.particleMeta = p ? meta : null;
  }

  /** The timeline's step, for both: the map clamped to its own steps (TCR has one). */
  setStep(step: number): void {
    const steps = this.layout?.steps ?? 1;
    this.map.material.uniforms.uStep.value = Math.max(0, Math.min(steps - 1, step));
    this.particles.material.uniforms.uStep.value = step;
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
    return this.mapMeta ? (this.map.material.uniforms.uStep.value as number) : null;
  }

  particleStep(): number {
    return this.particles.material.uniforms.uStep.value as number;
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

  /** Runs `draw` into a fresh n x 1 float32 target and returns its RGBA floats. */
  private readPass(n: number, draw: (r: WebGLRenderer, target: WebGLRenderTarget) => void): Float32Array {
    const r = this.renderer;
    if (!r) throw new Error('no WebGL renderer');
    const target = new WebGLRenderTarget(n, 1, { type: FloatType, format: RGBAFormat, minFilter: NearestFilter, magFilter: NearestFilter, depthBuffer: false });
    const out = new Float32Array(4 * n);
    const prev = r.getRenderTarget();
    try {
      r.setScissorTest(false);
      r.setRenderTarget(target);
      r.setClearColor(0x000000, 0);
      r.clear(true, false, false);
      draw(r, target);
      r.readRenderTargetPixels(target, 0, 0, n, 1, out);
    } finally {
      r.setRenderTarget(prev);
      target.dispose();
    }
    return out;
  }

  /** The texture's texels at (face, step), as float32 bits, read back from the GPU. */
  readTexels(samples: [number, number][], what: 'texel' | 'diff' = 'texel'): number[] {
    const l = this.layout;
    if (!l || !this.mapTex) throw new Error('no map loaded');
    if (what === 'diff' && !this.baseTex) throw new Error('no baseline loaded');
    for (const [f, s] of samples) {
      if (!(f >= 0 && f < l.faces && s >= 0 && s < l.steps)) throw new Error(`no cell (${f}, ${s}) in ${l.faces} faces x ${l.steps} steps`);
    }
    const n = samples.length;
    const mat = probeMaterial();
    const g = new BufferGeometry();
    g.setAttribute('position', new BufferAttribute(new Float32Array(3 * n), 3));
    g.setAttribute('aSample', new BufferAttribute(new Float32Array(samples.flatMap(([f, s], i) => [f, s, i])), 3));
    const pts = new Points(g, mat);
    pts.frustumCulled = false;
    const scene = new Scene();
    scene.add(pts);
    const u = mat.uniforms;
    u.uMap.value = this.mapTex;
    u.uBase.value = this.baseTex ?? this.mapTex;
    u.uWidth.value = l.width;
    u.uSteps.value = l.steps;
    u.uWhat.value = what === 'diff' ? 1 : 0;
    u.uN.value = n;
    try {
      const px = this.readPass(n, (r) => r.render(scene, new OrthographicCamera()));
      const bits = new Uint32Array(px.buffer);
      return samples.map((_, i) => {
        if (what === 'diff' && px[4 * i + 1] !== 1) throw new Error(`cell (${samples[i][0]}, ${samples[i][1]}) has no energy in one of the runs`);
        return bits[4 * i];
      });
    } finally {
      g.dispose();
      mat.dispose();
    }
  }

  /**
   * The particles the playback draw lets through at its current step, counted on the GPU: the
   * particle material itself in count mode, each drawn particle adding 1 to one float32 pixel.
   */
  countParticles(): number {
    const r = this.renderer;
    if (!r) throw new Error('no WebGL renderer');
    if (!this.particleMeta) throw new Error('no particles loaded');
    if (!r.extensions.has('EXT_float_blend')) throw new Error('EXT_float_blend is not available: float32 additive counting cannot run');
    const mat = this.particles.material;
    const u = mat.uniforms;
    const saved = { blending: mat.blending, src: mat.blendSrc, dst: mat.blendDst, eq: mat.blendEquation, depthTest: mat.depthTest, transparent: mat.transparent };
    const pts = new Points(this.particles.geometry, mat);
    pts.frustumCulled = false;
    const scene = new Scene();
    scene.add(pts);
    u.uCount.value = 1;
    mat.blending = CustomBlending;
    mat.blendSrc = OneFactor;
    mat.blendDst = OneFactor;
    mat.blendEquation = AddEquation;
    mat.depthTest = false;
    try {
      const px = this.readPass(1, (rr) => rr.render(scene, new OrthographicCamera()));
      return px[0];
    } finally {
      u.uCount.value = 0;
      mat.blending = saved.blending ?? NormalBlending;
      mat.blendSrc = saved.src;
      mat.blendDst = saved.dst;
      mat.blendEquation = saved.eq;
      mat.depthTest = saved.depthTest;
      mat.transparent = saved.transparent;
    }
  }
}
