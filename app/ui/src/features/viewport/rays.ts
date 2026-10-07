// Ray tracing on screen (decision 68 (b), PLAN.md B2), drawn as light (decision 69, Burhan 20:13: "UBER
// BEAUTIFUL WITH THE MANY SHADES OF RED, ORANGE, YELLOW").
//
//   - Storage buffers hold the band's records (x, y, z, energy) and per particle its first record, record
//     count and first step (raysData.ts). Uploaded once a band; per frame one uniform moves, the timeline's
//     fractional step.
//   - A compute pass places every particle at that step: the records either side, interpolated (raysData.ts
//     `particleAt`, the same rule), into a state buffer, w = energy or -1 where it is dead.
//   - Colour is the warm ramp (warmRamp.ts) by the particle's level re the band's loudest particle, as light:
//     brightness rises steeply with level, so a loud particle runs past white into the bloom and a quiet one
//     is a dim ember. Everything here is drawn additively into the engine's half-float target, bloomed and
//     tone mapped there (engine.ts), never into the measured picture.
//   - 'glow': soft round sprites, a gaussian core and halo, sized by level and attenuated with depth, with a
//     faint per-particle shimmer from a hashed phase; trails over the last steps of each path (the card's Trails)
//     that cool along their length, head colour to crimson to black, as screen-space ribbons.
//   - 'rays': each particle's whole path up to the step, every pair of consecutive records a 2.5 px ribbon in
//     that segment's colour on a ramp fitted to the step (the loudest particle down to 6 dB under the field's
//     level then), so every path cools through the whole ramp, white-hot at the source to crimson at its
//     newest bounces; a segment burns at full light for its first steps, then keeps an ember glow; each bounce
//     (raysData.ts `bounceRecords`) flashes a spark for SPARK_STEPS steps, expanding and fading. Heads glow.
//   - The benchmark (`bench`) replays the saved paths as many times as asked, copy c offset by a fixed jitter
//     (raysData.ts `copyJitter`): a visual density for measuring the GPU, not more particles. Off unless a test
//     hook asks; while it is on the timeline card says so in words.
//
// The WebGL2 fallback has no storage buffers in the vertex stage: there 'glow' is the same sprites drawn
// once per saved record, kept at its own step (particles.ts `keptRecord`), without trails, and 'rays' is
// refused with the reason.
import { AdditiveBlending, DataTexture, DoubleSide, Group, InstancedBufferAttribute, InstancedBufferGeometry, LinearFilter, Mesh, PlaneGeometry, RGBAFormat, Sprite, SRGBColorSpace } from 'three';
import { MeshBasicNodeMaterial, PointsNodeMaterial, StorageBufferAttribute, type WebGPURenderer } from 'three/webgpu';
import type { Particles } from '../../resultsData';
import { keptRecord, recordSteps } from './particles';
import { NODE_OPS } from './mapNodes';
import { bounceRecords, particleTable, raySpanDb, recordOwners, recordTable, stepLevels } from './raysData';
import { T } from './tsl';
import { fitDepthDb, WARM_DEPTH_DB, warmLut } from './warmRamp';

const {
  bitOr,
  bitXor,
  cameraProjectionMatrix,
  cameraViewMatrix,
  clamp,
  dot,
  exp,
  float,
  floor,
  Fn,
  hash,
  If,
  instanceIndex,
  instancedBufferAttribute,
  log2,
  max,
  min,
  mix,
  normalize,
  positionGeometry,
  pow,
  select,
  shiftRight,
  sin,
  storage,
  texture,
  uint,
  uniform,
  uv,
  varying,
  vec2,
  vec3,
  vec4,
  viewportSize,
} = T;

export type ParticleLook = 'dots' | 'glow' | 'rays';
export const PARTICLE_LOOKS: readonly ParticleLook[] = ['dots', 'glow', 'rays'];
/** Why rays cannot be drawn on the WebGL2 fallback. */
export const RAYS_REFUSAL_WEBGL = 'Rays need WebGPU (storage buffers and compute); this view runs on WebGL2';

/** The glow trail's segments; its length follows the timeline card's Trails choice (0: particles only). The spark's life in steps. */
const TRAIL_SEGMENTS = 6;
export const SPARK_STEPS = 6;
/** Rays: the steps over which a segment cools from full light to its ember glow. */
const RAY_FRESH = 30;

type N = ReturnType<typeof float>;

/** The ramp's lookup texture (sRGB, sampled linear: the light adds in linear and is tone mapped once). */
const LUT = (() => {
  const t = new DataTexture(warmLut(), 256, 1, RGBAFormat);
  t.colorSpace = SRGBColorSpace;
  t.magFilter = LinearFilter;
  t.minFilter = LinearFilter;
  t.generateMipmaps = false;
  t.needsUpdate = true;
  return t;
})();

/** The ramp's span for the band shown, dB below the loudest (warmRamp.ts `fitDepthDb`); the legend shows it. */
const RAMP_DEPTH = uniform(WARM_DEPTH_DB);
/** The view's depth range across the model (engine.ts sets it with the distance fade): far light fades a little. */
const DEPTH_NEAR = uniform(0);
const DEPTH_FAR = uniform(1);

/** The rays' span: from the loudest particle down to 6 dB under the field's level at the step (raysData.ts `raySpanDb`). */
const RAY_DEPTH = uniform(WARM_DEPTH_DB);

/** warmRamp.ts `warmPlace` on the GPU: 0 to 1 over `span` dB re the loudest (`logMax`, dB). */
function placeOf(energy: N, logMax: N, span: N = RAMP_DEPTH): N {
  const db = log2(max(energy, 1e-30)).mul(10 * 0.3010299956639812);
  return select(energy.greaterThan(0), clamp(db.sub(logMax).add(span).div(span), 0, 1), float(0));
}

/** Light at view depth `w`: full at the model's near side, 45 % at its far side. */
const depthFade = (w: N): N => float(1).sub(T.smoothstep(DEPTH_NEAR, DEPTH_FAR, w).mul(0.55));

/** The ramp's colour at `x`, linear. */
const warm = (x: N): N => texture(LUT, vec2(x.mul(255 / 256).add(0.5 / 256), 0.5)).rgb;

/** Light by level: an ember at the floor, past white (into the bloom) at the top. */
const brightness = (x: N): N => float(0.12).add(pow(x, 2.2).mul(2.2));

/** A spark of light: a hot core about a third of the sprite across and a tight halo, zero at the rim. */
function sprite(): N {
  const q = uv().sub(0.5).mul(2);
  const r2 = dot(q, q);
  return exp(r2.mul(-22)).add(exp(r2.mul(-6)).mul(0.22)).mul(clamp(float(1).sub(r2), 0, 1));
}

/** A spark's flash: a soft ring that opens out, for the bounce sparks. */
function flash(): N {
  const q = uv().sub(0.5).mul(2);
  const r2 = dot(q, q);
  return exp(r2.mul(-10)).add(exp(r2.sub(0.45).mul(r2.sub(0.45)).mul(-60)).mul(0.6)).mul(clamp(float(1).sub(r2), 0, 1));
}

/** The u32 hash of raysData.ts `copyJitter` on the GPU: a copy's fixed offset, 0 for copy 0. */
function jitter(copy: N, scale: N): N {
  const h = (x: N): N => {
    let z = x.add(uint(0x6d2b79f5)).toVar();
    z = bitXor(z, shiftRight(z, uint(15))).mul(bitOr(z, uint(1))).toVar();
    z = bitXor(z, z.add(bitXor(z, shiftRight(z, uint(7))).mul(bitOr(z, uint(61))))).toVar();
    return float(bitXor(z, shiftRight(z, uint(14)))).div(4294967296);
  };
  const c3 = copy.mul(uint(3));
  const j = vec3(h(c3), h(c3.add(uint(1))), h(c3.add(uint(2)))).sub(0.5).mul(scale);
  return select(copy.equal(uint(0)), vec3(0, 0, 0), j);
}

/** A screen-space ribbon from `a` to `b` (world), `px` wide: the clip position of this vertex of the unit quad. */
function ribbon(a: N, b: N, px: N, ok: N): N {
  const vp = cameraProjectionMatrix.mul(cameraViewMatrix);
  const ca = vp.mul(vec4(a, 1)).toVar();
  const cb = vp.mul(vec4(b, 1)).toVar();
  const sa = ca.xy.div(ca.w);
  const sb = cb.xy.div(cb.w);
  const d = sb.sub(sa).mul(viewportSize).toVar();
  const dir = select(dot(d, d).greaterThan(1e-8), normalize(d), vec2(1, 0));
  const side = vec2(dir.y.negate(), dir.x);
  const along = positionGeometry.x.add(0.5);
  const c = mix(ca, cb, along).toVar();
  const off = side.mul(positionGeometry.y).mul(px).mul(2).div(viewportSize).mul(c.w);
  const keep = ok.and(ca.w.greaterThan(0)).and(cb.w.greaterThan(0));
  return select(keep, vec4(c.xy.add(off), c.z, c.w), vec4(2, 2, 2, 1));
}

/** An instanced unit quad, `count` instances (ribbons). */
function quads(count: number): InstancedBufferGeometry {
  const quad = new PlaneGeometry(1, 1);
  const g = new InstancedBufferGeometry();
  g.index = quad.index;
  g.setAttribute('position', quad.getAttribute('position'));
  g.setAttribute('uv', quad.getAttribute('uv'));
  g.instanceCount = count;
  return g;
}

const additive = () => ({ transparent: true, depthWrite: false, blending: AdditiveBlending });

/** The heads' sprite material: at `pos`, its level `x` (0 to 1) and `alive`, shimmering by `phase`. */
function headMaterial(pos: N, x: N, alive: N, phase: N, time: N, size: N, gain: N): PointsNodeMaterial {
  const m = new PointsNodeMaterial({ ...additive(), sizeAttenuation: false });
  m.alphaToCoverage = false;
  m.positionNode = pos;
  // Attenuated with depth, as a point of light would be, but held between 4 and 10 px (the hot core a third of
  // that): sharp sparks, never a blur that fills the view as a particle passes the camera.
  const depth = max(cameraViewMatrix.mul(vec4(pos, 1)).z.negate(), 0.05);
  m.sizeNode = select(alive, clamp(size.mul(viewportSize.y).div(depth), 4, 10).mul(mix(0.7, 1.15, x)), float(0));
  const shimmer = float(0.86).add(sin(time.mul(2.3).add(phase.mul(6.2831853))).mul(0.14));
  m.colorNode = vec4(varying(warm(x).mul(brightness(x)).mul(shimmer).mul(gain).mul(depthFade(depth))), sprite());
  return m;
}

export class GpuParticles {
  /** Drawn into the engine's light target (engine.ts), never into the measured frame. */
  readonly group = new Group();
  private renderer: WebGPURenderer | null = null;
  private source: Particles | null = null;
  private look: ParticleLook = 'dots';
  /** The timeline's step, fractional while it plays. */
  private readonly time = uniform(0);
  private readonly logMax = uniform(0);
  /** The heads' size before depth attenuation (PointsNodeMaterial's world-scaled points). */
  private readonly headSize = uniform(0.6);
  /** The glow trails' length in steps (the card's Trails choice); 0 draws none. */
  private readonly trailLen = uniform(0);
  /** Saved particles in the source (copies cycle through them) and the jitter between copies, metres. */
  private readonly sourceCount = uniform(1, 'uint');
  private readonly jitterScale = uniform(0);
  /** Each particle's share of the light: 1 for the file's own, saved / drawn for the benchmark's copies (the room's light stays the same, finer grained). */
  private readonly gain = uniform(1);
  private count = 0;
  private rampDepth = WARM_DEPTH_DB;
  private recs: StorageBufferAttribute | null = null;
  private meta: StorageBufferAttribute | null = null;
  private owners: StorageBufferAttribute | null = null;
  private tablesOf: Particles | null = null;
  /** Per step, the field's mean level (raysData.ts `stepLevels`), for the rays' span. */
  private levels: Float32Array | null = null;
  private state: StorageBufferAttribute | null = null;
  private kernel: unknown = null;
  private heads: Sprite | null = null;
  private trails: Mesh | null = null;
  private rays: Mesh | null = null;
  private sparks: Sprite | null = null;
  /** The WebGL2 fallback's sprites, one per saved record. */
  private recordHeads: Sprite | null = null;
  /** The last time the compute pass ran for, so a still frame computes nothing. */
  private computedAt = NaN;

  setRenderer(r: WebGPURenderer | null): void {
    this.renderer = r;
    if (this.source) this.build(this.source.particleCount, 0);
  }

  /** WebGPU is running (storage buffers in the vertex stage and compute). */
  webgpu(): boolean {
    return (this.renderer?.backend as unknown as { isWebGPUBackend?: boolean } | undefined)?.isWebGPUBackend === true;
  }

  /** Why a look cannot be drawn now, or null. */
  refusal(look: ParticleLook): string | null {
    if (look === 'dots') return null;
    if (!this.source) return 'No particles saved for this run';
    if (!this.renderer) return 'The 3D view has not started';
    return look === 'rays' && !this.webgpu() ? RAYS_REFUSAL_WEBGL : null;
  }

  /** The band's particles (null clears), and the band's loudest record in dB (the ramp's top). */
  setParticles(p: Particles | null, logMax: number): void {
    this.source = p;
    this.logMax.value = logMax;
    this.levels = p ? stepLevels(p, Math.max(1, p.maxSteps)) : null;
    this.rampDepth = p ? fitDepthDb(p.energies, logMax) : WARM_DEPTH_DB;
    RAMP_DEPTH.value = this.rampDepth;
    this.build(p ? p.particleCount : 0, 0);
  }

  /** B3: the ramp's span is shared by every layer's materials; the layer shown takes it back for its band. */
  claimRamp(): void {
    RAMP_DEPTH.value = this.rampDepth;
  }

  /** The look; one the GPU cannot draw falls back to dots (returns the look in force). */
  setLook(look: ParticleLook): ParticleLook {
    this.look = this.refusal(look) === null ? look : 'dots';
    this.apply();
    return this.look;
  }

  currentLook(): ParticleLook {
    return this.look;
  }

  /** Whether the light layer draws now (a look other than dots, with particles). */
  active(): boolean {
    return this.look !== 'dots' && this.count > 0;
  }

  /** The glow trails' length in steps, 0 for none (the card's Trails choice, shared with the dots' trails). */
  setTrailSteps(n: number): void {
    this.trailLen.value = Math.max(0, n);
    this.apply();
  }

  /** The ramp's span for the band shown, dB below its loudest particle (the legend's floor). */
  depthDb(): number {
    return RAMP_DEPTH.value as number;
  }

  /** The view's depth across the model (the distance fade's range): far light fades a little. */
  setDepthRange(near: number, far: number): void {
    DEPTH_NEAR.value = near;
    DEPTH_FAR.value = Math.max(far, near + 1e-3);
  }

  /** The rays' ramp span at step `t`, dB below the loudest (the legend's floor in Rays). */
  raySpanAt(t: number): number {
    return this.levels ? raySpanDb(this.levels, t, this.logMax.value as number) : WARM_DEPTH_DB;
  }

  /** The timeline's step, fractional while it plays. */
  setTime(t: number): void {
    this.time.value = t;
    RAY_DEPTH.value = this.raySpanAt(t);
  }

  /** The benchmark: `n` particles (the saved ones replayed, a fixed offset per copy), or back to the file's own with null. */
  bench(n: number | null): number {
    if (!this.source) throw new Error('no particles loaded');
    if (!this.webgpu()) throw new Error(RAYS_REFUSAL_WEBGL);
    this.build(n ?? this.source.particleCount, n === null ? 0 : 0.6);
    this.apply();
    return this.count;
  }

  particles(): number {
    return this.count;
  }

  /** The benchmark's replication in force, or null: the saved particles and how many times each path is drawn. */
  replicas(): { saved: number; copies: number } | null {
    const p = this.source;
    return p && this.count > p.particleCount ? { saved: p.particleCount, copies: Math.round(this.count / p.particleCount) } : null;
  }

  /** Runs the compute pass if the look needs it and the time moved; call before each render. */
  update(): void {
    const r = this.renderer;
    if (!r || !this.kernel || this.look === 'dots') return;
    if (this.computedAt === this.time.value) return;
    r.compute(this.kernel as Parameters<WebGPURenderer['compute']>[0]);
    this.computedAt = this.time.value as number;
  }

  /** Forces the next `update` to compute (a benchmark frame). */
  invalidate(): void {
    this.computedAt = NaN;
  }

  /** The state buffer read back: per particle x, y, z and energy (-1: not alive), for a check against `particleAt`. */
  async readState(indices: number[]): Promise<number[][]> {
    const r = this.renderer;
    if (!r || !this.state) throw new Error('no GPU particles (WebGPU and particles needed)');
    this.invalidate();
    const was = this.look;
    if (this.look === 'dots') this.look = 'glow';
    this.update();
    this.look = was;
    const buf = new Float32Array(await r.getArrayBufferAsync(this.state));
    return indices.map((i) => Array.from(buf.subarray(4 * i, 4 * i + 4)));
  }

  private apply(): void {
    const glow = this.look === 'glow';
    const rays = this.look === 'rays';
    if (this.heads) this.heads.visible = glow || rays;
    if (this.trails) this.trails.visible = glow && (this.trailLen.value as number) > 0;
    if (this.rays) this.rays.visible = rays;
    if (this.sparks) this.sparks.visible = rays;
    if (this.recordHeads) this.recordHeads.visible = glow || rays;
    this.invalidate();
  }

  private dispose(): void {
    for (const o of [this.heads, this.trails, this.rays, this.sparks, this.recordHeads]) {
      if (!o) continue;
      this.group.remove(o);
      // A Sprite's geometry is three's one shared quad (every marker draws with it): never disposed.
      if (!(o as Sprite).isSprite) o.geometry.dispose();
      (o.material as MeshBasicNodeMaterial).dispose();
    }
    this.heads = this.trails = this.rays = this.sparks = this.recordHeads = null;
    this.kernel = null;
    this.count = 0;
  }

  /** Builds the buffers, the compute pass and the draws for `count` particles. */
  private build(count: number, jitterScale: number): void {
    this.dispose();
    const p = this.source;
    if (!p || count === 0 || !this.renderer) return;
    this.count = count;
    if (!this.webgpu()) {
      this.buildFallback(p);
      this.apply();
      return;
    }
    if (this.tablesOf !== p) {
      this.recs = new StorageBufferAttribute(recordTable(p), 4);
      this.meta = new StorageBufferAttribute(particleTable(p), 4);
      this.owners = new StorageBufferAttribute(recordOwners(p), 1);
      this.tablesOf = p;
    }
    this.sourceCount.value = p.particleCount;
    this.jitterScale.value = jitterScale;
    this.gain.value = Math.min(1, p.particleCount / count);
    this.state = new StorageBufferAttribute(new Float32Array(4 * count), 4);
    const recs = storage(this.recs, 'vec4', p.recordCount).toReadOnly();
    const meta = storage(this.meta, 'uvec4', p.particleCount).toReadOnly();
    const owners = storage(this.owners, 'uint', p.recordCount).toReadOnly();
    const stateW = storage(this.state, 'vec4', count);
    const stateR = storage(this.state, 'vec4', count).toReadOnly();

    /** Particle `i` (a copy index) at fractional step `t`: vec4(x, y, z, energy), w -1 where not alive. */
    const at = (i: N, t: N): N => {
      const src = i.mod(this.sourceCount).toVar();
      const m = meta.element(src).toVar();
      const n = float(m.y);
      const k = t.sub(float(m.z)).toVar();
      const alive = m.y.greaterThan(uint(0)).and(k.greaterThanEqual(0)).and(k.lessThanEqual(n.sub(1)));
      const k0 = min(floor(max(k, 0)), max(n.sub(1), 0)).toVar();
      const k1 = min(k0.add(1), max(n.sub(1), 0));
      const r0 = recs.element(m.x.add(uint(k0)));
      const r1 = recs.element(m.x.add(uint(k1)));
      const v = mix(r0, r1, k.sub(k0)).toVar();
      const pos = v.xyz.add(jitter(i.div(this.sourceCount), this.jitterScale));
      return select(alive, vec4(pos, v.w), vec4(0, 0, 0, -1));
    };

    this.kernel = Fn(() => {
      If(instanceIndex.lessThan(uint(count)), () => {
        stateW.element(instanceIndex).assign(at(instanceIndex, this.time));
      });
    })().compute(count, [256]);

    // Heads.
    const head = stateR.element(instanceIndex);
    const hx = placeOf(max(head.w, 0), this.logMax);
    this.heads = new Sprite(headMaterial(head.xyz, hx, head.w.greaterThanEqual(0), hash(instanceIndex), this.time, this.headSize, this.gain));
    this.heads.count = count;

    // Trails: ribbons over each particle's last `trailLen` steps, cooling from the head's colour to black.
    const tm = new MeshBasicNodeMaterial({ ...additive(), side: DoubleSide });
    const seg = instanceIndex.mod(uint(TRAIL_SEGMENTS));
    const pi = instanceIndex.div(uint(TRAIL_SEGMENTS));
    const dt = this.trailLen.div(TRAIL_SEGMENTS);
    const ta = this.time.sub(float(seg).mul(dt));
    const pa = at(pi, ta);
    const pb = at(pi, ta.sub(dt));
    const px = placeOf(max(stateR.element(pi).w, max(pa.w, 0)), this.logMax);
    tm.vertexNode = ribbon(pa.xyz, pb.xyz, float(1.4).add(px.mul(1.6)), pa.w.greaterThanEqual(0).and(pb.w.greaterThanEqual(0)));
    const u = float(seg).add(positionGeometry.x.add(0.5)).div(TRAIL_SEGMENTS);
    const cool = px.mul(pow(float(1).sub(u), 0.8));
    const tw = cameraProjectionMatrix.mul(cameraViewMatrix).mul(vec4(pa.xyz, 1)).w;
    tm.colorNode = vec4(varying(warm(cool).mul(brightness(px).mul(0.5)).mul(pow(float(1).sub(u), 1.6)).mul(this.gain).mul(depthFade(tw))), 1);
    this.trails = new Mesh(quads(count * TRAIL_SEGMENTS), tm);

    // Rays: every pair of consecutive records of the saved particles, up to the step.
    const rm = new MeshBasicNodeMaterial({ ...additive(), side: DoubleSide });
    const k = instanceIndex;
    const owner = owners.element(k);
    const same = owner.equal(owners.element(k.add(uint(1))));
    const om = meta.element(owner);
    const stepOfNext = float(om.z.add(k.add(uint(1)).sub(om.x)));
    const ra = recs.element(k);
    const rb = recs.element(k.add(uint(1)));
    const rx = placeOf(ra.w, this.logMax, RAY_DEPTH);
    // Each segment in its particle's colour on that segment: a path cools down the whole ramp, bounce by
    // bounce, white-hot at the source to crimson and ember at its late reflections. 2.5 px, additive; only the
    // hottest segments cross the bloom's threshold. Older segments of a path dim a little, so the front reads.
    rm.vertexNode = ribbon(ra.xyz, rb.xyz, float(2.5), same.and(stepOfNext.lessThanEqual(this.time)));
    const rw = cameraProjectionMatrix.mul(cameraViewMatrix).mul(vec4(ra.xyz, 1)).w;
    // A path's last RAY_FRESH steps burn at full light; older segments keep their colour as an ember glow.
    const ageR = this.time.sub(stepOfNext).max(0);
    const fresh = mix(float(0.12), float(1), exp(ageR.div(RAY_FRESH).negate()));
    rm.colorNode = vec4(varying(warm(rx).mul(mix(float(0.07), float(0.3), pow(rx, 1.5))).mul(fresh).mul(depthFade(rw))), 1);
    this.rays = new Mesh(quads(Math.max(0, p.recordCount - 1)), rm);

    // Sparks at the bounces: a flash for SPARK_STEPS steps, expanding and fading, in the particle's colour then.
    const bounces = bounceRecords(p);
    const bx = new Float32Array(3 * bounces.length);
    const bs = new Float32Array(bounces.length);
    const be = new Float32Array(bounces.length);
    const steps = recordSteps(p);
    bounces.forEach((r, i) => {
      bx.set(p.positions.subarray(3 * r, 3 * r + 3), 3 * i);
      bs[i] = steps[r];
      be[i] = p.energies[r];
    });
    const sm = new PointsNodeMaterial({ ...additive(), sizeAttenuation: false });
    sm.alphaToCoverage = false;
    sm.positionNode = instancedBufferAttribute(new InstancedBufferAttribute(bx, 3));
    const age = this.time.sub(instancedBufferAttribute(new InstancedBufferAttribute(bs, 1))).div(SPARK_STEPS);
    const sx = placeOf(instancedBufferAttribute(new InstancedBufferAttribute(be, 1)), this.logMax, RAY_DEPTH);
    const live = age.greaterThanEqual(0).and(age.lessThan(1));
    sm.sizeNode = select(live, mix(6, 30, pow(clamp(age, 0, 1), 0.5)), float(0));
    const fadeS = pow(float(1).sub(clamp(age, 0, 1)), 1.6);
    sm.colorNode = vec4(varying(warm(max(sx, 0.35)).mul(brightness(sx).mul(1.3).add(1.2)).mul(fadeS)), flash());
    this.sparks = new Sprite(sm);
    this.sparks.count = Math.max(1, bounces.length);

    for (const o of [this.rays, this.trails, this.sparks, this.heads]) {
      o.frustumCulled = false;
      this.group.add(o);
    }
    this.apply();
  }

  /** WebGL2: the glow's sprites once per saved record, drawn at their own step; no trails, rays or sparks. */
  private buildFallback(p: Particles): void {
    const step = instancedBufferAttribute(new InstancedBufferAttribute(recordSteps(p), 1));
    const energy = instancedBufferAttribute(new InstancedBufferAttribute(p.energies, 1));
    const pos = instancedBufferAttribute(new InstancedBufferAttribute(p.positions, 3));
    const x = placeOf(energy, this.logMax);
    this.recordHeads = new Sprite(headMaterial(pos, x, keptRecord(NODE_OPS, step, floor(this.time.add(0.5))), hash(step), this.time, this.headSize, this.gain));
    this.recordHeads.count = p.recordCount;
    this.recordHeads.frustumCulled = false;
    this.group.add(this.recordHeads);
  }
}
