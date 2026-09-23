// What the webview renders with. A software fallback must never be silent (rebuild plan,
// risks[14]): the renderer string goes to the Console at startup and into the self-test.

export interface WebGLInfo {
  context: 'webgl2' | null;
  /** The unmasked renderer when WEBGL_debug_renderer_info is exposed, else gl.RENDERER. */
  renderer: string | null;
  vendor: string | null;
  unmasked: boolean;
  version: string | null;
  shading_language: string | null;
  max_texture_size: number | null;
  max_3d_texture_size: number | null;
  max_renderbuffer_size: number | null;
  error: string | null;
}

export function probeWebGL(): WebGLInfo {
  const info: WebGLInfo = {
    context: null,
    renderer: null,
    vendor: null,
    unmasked: false,
    version: null,
    shading_language: null,
    max_texture_size: null,
    max_3d_texture_size: null,
    max_renderbuffer_size: null,
    error: null,
  };
  try {
    const gl = document.createElement('canvas').getContext('webgl2');
    if (!gl) {
      info.error = 'getContext("webgl2") returned null';
      return info;
    }
    info.context = 'webgl2';
    const debug = gl.getExtension('WEBGL_debug_renderer_info');
    info.unmasked = debug !== null;
    info.renderer = String(gl.getParameter(debug ? debug.UNMASKED_RENDERER_WEBGL : gl.RENDERER));
    info.vendor = String(gl.getParameter(debug ? debug.UNMASKED_VENDOR_WEBGL : gl.VENDOR));
    info.version = String(gl.getParameter(gl.VERSION));
    info.shading_language = String(gl.getParameter(gl.SHADING_LANGUAGE_VERSION));
    info.max_texture_size = Number(gl.getParameter(gl.MAX_TEXTURE_SIZE));
    info.max_3d_texture_size = Number(gl.getParameter(gl.MAX_3D_TEXTURE_SIZE));
    info.max_renderbuffer_size = Number(gl.getParameter(gl.MAX_RENDERBUFFER_SIZE));
    gl.getExtension('WEBGL_lose_context')?.loseContext();
  } catch (e) {
    info.error = String(e);
  }
  return info;
}

export interface WebGPUInfo {
  navigator_gpu: boolean;
  adapter: boolean;
  vendor: string | null;
  architecture: string | null;
  description: string | null;
  error: string | null;
}

interface GpuAdapterLike {
  info?: { vendor?: string; architecture?: string; description?: string };
}
interface GpuLike {
  requestAdapter(): Promise<GpuAdapterLike | null>;
}

/** Informational only (an open question in the plan): the viewport is WebGL2. */
export async function probeWebGPU(timeoutMs = 3000): Promise<WebGPUInfo> {
  const gpu = (navigator as Navigator & { gpu?: GpuLike }).gpu;
  const info: WebGPUInfo = {
    navigator_gpu: gpu !== undefined,
    adapter: false,
    vendor: null,
    architecture: null,
    description: null,
    error: null,
  };
  if (!gpu) return info;
  try {
    const adapter = await Promise.race([
      gpu.requestAdapter(),
      new Promise<'timeout'>((r) => setTimeout(() => r('timeout'), timeoutMs)),
    ]);
    if (adapter === 'timeout') info.error = `requestAdapter did not settle in ${timeoutMs} ms`;
    else if (adapter) {
      info.adapter = true;
      info.vendor = adapter.info?.vendor ?? null;
      info.architecture = adapter.info?.architecture ?? null;
      info.description = adapter.info?.description ?? null;
    }
  } catch (e) {
    info.error = String(e);
  }
  return info;
}
