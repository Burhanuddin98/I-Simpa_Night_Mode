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
  Points,
  RedFormat,
  RGBAFormat,
  Scene,
  ShaderMaterial,
  WebGLRenderTarget,
  type WebGLRenderer,
} from 'three';
import type { Particles, SurfaceMap } from '../../resultsData';
import { COOL, denseValues, HOT, mapLayout, rampFloats, type MapLayout, type Range } from './mapData';
import { recordSteps } from './particles';

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
      uWidth: { value: 1 },
      uSteps: { value: 1 },
      uStep: { value: 0 },
      uDiff: { value: 0 },
      uLo: { value: 0 },
      uHi: { value: 1 },
      uHot: { value: vec3s(HOT) },
      uCool: { value: vec3s(COOL) },
    },
    vertexShader: /* glsl */ `
      ${MAP_GLSL}
      ${RAMP_GLSL}
      uniform int uStep;
      uniform int uDiff;
      uniform float uLo;
      uniform float uHi;
      attribute float aFace;
      flat varying vec4 vColor;
      void main() {
        int face = int(aFace + 0.5);
        vec4 c = vec4(0.0);
        if (uDiff == 1) {
          bool ok;
          float d = diffDb(face, uStep, ok);
          if (ok) {
            float t = d / max(uHi, 1e-6);
            c = vec4(t < 0.0 ? cool(-t) : hot(t), 1.0);
          }
        } else {
          float e = mapTexel(uMap, face, uStep);
          if (e > 0.0 && !isinf(e) && !isnan(e)) c = vec4(hot((levelDb(e) - uLo) / max(uHi - uLo, 1e-6)), 1.0);
        }
        vColor = c;
        gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
      }
    `,
    fragmentShader: /* glsl */ `
      flat varying vec4 vColor;
      void main() {
        if (vColor.a < 0.5) discard;
        gl_FragColor = vec4(vColor.rgb, 1.0);
      }
    `,
    side: DoubleSide,
    polygonOffset: true,
    polygonOffsetFactor: -1,
    polygonOffsetUnits: -2,
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
    vertexShader: /* glsl */ `
      ${RAMP_GLSL}
      uniform float uStep;
      uniform float uCount;
      uniform float uSize;
      uniform float uLogMax;
      attribute float aStep;
      attribute float aEnergy;
      varying vec3 vColor;
      void main() {
        // A record is drawn only at its own step: the particles alive now.
        if (abs(aStep - uStep) > 0.5) {
          gl_Position = vec4(2.0, 2.0, 2.0, 1.0);
          gl_PointSize = 0.0;
          vColor = vec3(0.0);
          return;
        }
        if (uCount > 0.5) {
          // Count mode (the m12-d hook): every drawn particle lands on the one pixel.
          gl_Position = vec4(0.0, 0.0, 0.0, 1.0);
          gl_PointSize = 1.0;
          vColor = vec3(1.0);
          return;
        }
        float db = aEnergy > 0.0 ? 10.0 * log2(aEnergy) * 0.30102999566398120 : -1e9;
        vColor = hot(0.55 + 0.45 * clamp((db - uLogMax + 40.0) / 40.0, 0.0, 1.0));
        gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
        gl_PointSize = uSize;
      }
    `,
    fragmentShader: /* glsl */ `
      uniform float uCount;
      varying vec3 vColor;
      void main() {
        if (uCount > 0.5) { gl_FragColor = vec4(1.0, 0.0, 0.0, 1.0); return; }
        vec2 d = gl_PointCoord - 0.5;
        if (dot(d, d) > 0.25) discard;
        gl_FragColor = vec4(vColor, 1.0);
      }
    `,
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
    const pos = new Float32Array(9 * m.faceCount);
    const face = new Float32Array(3 * m.faceCount);
    for (let f = 0; f < m.faceCount; f++) {
      for (let c = 0; c < 3; c++) {
        const v = m.indices[3 * f + c];
        pos.set(m.positions.subarray(3 * v, 3 * v + 3), 9 * f + 3 * c);
        face[3 * f + c] = f;
      }
    }
    this.map.geometry.dispose();
    const g = new BufferGeometry();
    g.setAttribute('position', new BufferAttribute(pos, 3));
    g.setAttribute('aFace', new BufferAttribute(face, 1));
    this.map.geometry = g;
    const u = this.map.material.uniforms;
    u.uMap.value = this.mapTex;
    u.uBase.value = this.baseTex ?? this.mapTex;
    u.uWidth.value = l.width;
    u.uSteps.value = l.steps;
    u.uDiff.value = meta.kind === 'diff' && base ? 1 : 0;
    u.uLo.value = meta.range.lo;
    u.uHi.value = meta.range.hi;
    this.mapMeta = meta;
    this.map.visible = true;
    return null;
  }

  clearMap(): void {
    this.disposeTextures();
    this.map.geometry.dispose();
    this.map.geometry = new BufferGeometry();
    this.map.visible = false;
    this.mapMeta = null;
    this.layout = null;
  }

  private disposeTextures(): void {
    this.mapTex?.dispose();
    this.baseTex?.dispose();
    this.mapTex = null;
    this.baseTex = null;
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
