// The 3D view's shared node-material pieces (decision 68 (b): the viewport on three.js's WebGPURenderer,
// WebGL2 its fallback). Everything the WebGLRenderer did by string-patching GLSL is TSL here, so the same
// code runs on both backends.
//
// Colour, as WebGLRenderer drew it. WebGLRenderer converted each built-in material's colour to sRGB in its
// own fragment shader, so blending ran on sRGB values; a ShaderMaterial (the map, the ground, the
// particles) wrote its value raw. WebGPURenderer instead renders linear into a half-float framebuffer and
// converts in a separate pass, so a 30 % red wash over grey would blend in linear light and look
// different. The engine keeps the old picture: the renderer's output colour space is linear (no
// framebuffer pass, the canvas takes what the shaders write) and every material WebGLRenderer would
// have converted converts its own output with `srgbOutput`; the raw ones stay raw.
import { BufferGeometry, Color, Float32BufferAttribute, LinearSRGBColorSpace, NormalBlending, SRGBColorSpace, type Material } from 'three';
import { LineSegmentsGeometry } from 'three/addons/lines/LineSegmentsGeometry.js';
import { Line2NodeMaterial, type NodeBuilder } from 'three/webgpu';
import { T } from './tsl';

const { bitAnd, floatBitsToUint, output, sRGBTransferOETF, uint, vec4 } = T;

/** `output` (the material's colour as built) with WebGLRenderer's `colorspace_fragment` applied: rgb to sRGB. */
export function srgbOutput(c: any = output): any {
  return vec4(sRGBTransferOETF(c.rgb), c.a);
}

/** `m` with its output converted to sRGB inline, as WebGLRenderer drew a built-in material. */
export function inSrgb<M extends Material & { outputNode: unknown }>(m: M): M {
  m.outputNode = srgbOutput();
  return m;
}

/**
 * A colour whose stored components are `hex`'s own sRGB numbers. The renderer writes them as they are
 * (linear output, see the head of this file), so a clear colour or a raw shader colour made with this
 * lands on the canvas as `hex`, as WebGLRenderer's sRGB-converted clear colour did.
 */
export function rawColor(hex: number | string): Color {
  const c = new Color(hex).getRGB({ r: 0, g: 0, b: 0 }, SRGBColorSpace);
  return new Color().setRGB(c.r, c.g, c.b, LinearSRGBColorSpace);
}

/**
 * Whether a float is finite: its exponent bits are not all ones. GLSL's isinf/isnan have no WGSL
 * equivalent, and a WGSL compiler may fold `e != e` to false; the bit test is exact on both backends.
 */
export function finite(e: any): any {
  return bitAnd(floatBitsToUint(e), uint(0x7f800000)).notEqual(uint(0x7f800000));
}

/**
 * The fat lines' material (the selection outline, a plane's outline and grid), as WebGL's LineMaterial
 * drew them: see-through by ordinary alpha blending, a hard edge (no alpha to coverage), in sRGB.
 * three's Line2NodeMaterial draws a transparent line by mixing in a copy of the opaque frame, under
 * NoBlending; that path is bypassed here, so a 45 % grid over the map blends as it did.
 */
export class FatLineMaterial extends Line2NodeMaterial {
  constructor(params: { color: number; linewidth: number; opacity: number; depthWrite?: boolean }) {
    super({ color: params.color, linewidth: params.linewidth, transparent: true, opacity: params.opacity, depthWrite: params.depthWrite ?? true });
    this.blending = NormalBlending;
    this.alphaToCoverage = false;
    inSrgb(this);
  }

  override setupDiffuseColor(builder: NodeBuilder): void {
    // Line2NodeMaterial mixes in the opaque frame when `transparent`; blending does that job here. The
    // builder must not take the line for opaque meanwhile, or the opacity is dropped (alpha forced to 1).
    const t = this.transparent;
    const b = builder as unknown as { isOpaque(): boolean };
    const isOpaque = b.isOpaque;
    this.transparent = false;
    b.isOpaque = () => false;
    try {
      super.setupDiffuseColor(builder);
    } finally {
      this.transparent = t;
      b.isOpaque = isOpaque;
    }
  }
}

/**
 * A geometry with nothing to draw. WebGPURenderer, unlike WebGLRenderer, warns on a geometry without a
 * position and fails the frame on a fat-line geometry whose instance count is still Infinity; this one
 * has an empty position buffer and no instances.
 */
export function emptyGeometry(): BufferGeometry {
  const g = new BufferGeometry();
  g.setAttribute('position', new Float32BufferAttribute(new Float32Array(0), 3));
  return g;
}

/** An empty fat-line geometry (see `emptyGeometry`); `setPositions` sets its count when lines arrive. */
export function emptyFat(): LineSegmentsGeometry {
  // One zero-length segment holds the attributes the line shader reads (a pipeline is built before
  // the draw is skipped); no instance of it is drawn.
  const g = new LineSegmentsGeometry().setPositions([0, 0, 0, 0, 0, 0]);
  g.instanceCount = 0;
  return g;
}
