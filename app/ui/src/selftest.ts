// `app.exe --selftest <out.json>`: the UI measures itself and reports as JSON text; the backend
// adds `meta` (machine, versions, time) and writes the file. Gate M9 (tools/gates/m9.ps1) reads
// it. Nothing here is shown in the UI except one Console line at the start and one at the end.
import { Channel } from '@tauri-apps/api/core';
import { asCmdError, backend, type CmdError, type RunEventBatch } from './backend';
import type { Prepared } from './bindings/ipc';
import { checksum, knownAnswers } from './checksum';
import checksumKat from './checksum-kat.json';
import type { Op } from './bindings/schema';
import { probeWebGPU, type WebGLInfo } from './gpu';
import { STEP_NAMES } from './steps';
import { log, statusStore } from './store';

// The corrected Elmia solve's surface-receiver files (build-clean/sim_output/spps/
// Surface_receiver/<band>/rs_cut.csbin, measured 2026-09-23): Global, then 125 Hz to 4 kHz.
const CSBIN_GLOBAL = 16_470_804;
const CSBIN_BANDS = [13_740_628, 13_741_956, 13_747_476, 13_719_716, 13_543_092, 13_476_196];
const CSBIN_ALL = [CSBIN_GLOBAL, ...CSBIN_BANDS];
/** Above this the virtual-host fallback must be built and shown faster (plan, M9 c). */
export const SINGLE_16MB_LIMIT_MS = 500;
/** The run-event batching period (events.rs `BATCH_PERIOD`) the channel probe must measure. */
export const BATCH_PERIOD_MS = 50;

// Shortest decimals that serde_json::from_str misreads in this build (bridge.rs tests).
const MISREAD = ['1.5990461000457081', '484.53035250855277', '980.0819440890737'];
const MISREAD_BITS = ['0x3ff995b15d07e1f0', '0x407e487c52e9795f', '0x408ea0a7d24d7560'];

const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));
const median = (xs: number[]) => {
  const s = [...xs].sort((a, b) => a - b);
  const m = s.length >> 1;
  return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2;
};
const quantile = (xs: number[], q: number) => {
  const s = [...xs].sort((a, b) => a - b);
  return s[Math.min(s.length - 1, Math.ceil(q * s.length) - 1)];
};
const round3 = (x: number) => Math.round(x * 1000) / 1000;

function bitsOf(x: number): string {
  const v = new DataView(new ArrayBuffer(8));
  v.setFloat64(0, x);
  return '0x' + v.getBigUint64(0).toString(16).padStart(16, '0');
}

interface Transfer {
  bytes: number;
  ms: number;
  verified: boolean;
  /** What `invoke` handed back: `ArrayBuffer` on the normal path. */
  received: string;
}

/** The bytes of whatever `invoke` returned, and what it was. */
function asBytes(value: unknown): { buf: ArrayBuffer | null; received: string } {
  const received = Object.prototype.toString.call(value).slice(8, -1);
  if (value instanceof ArrayBuffer) return { buf: value, received };
  if (ArrayBuffer.isView(value)) {
    const v = value as ArrayBufferView;
    return { buf: new Uint8Array(v.buffer, v.byteOffset, v.byteLength).slice().buffer, received };
  }
  if (Array.isArray(value) && value.every((x) => Number.isInteger(x) && x >= 0 && x < 256)) {
    return { buf: Uint8Array.from(value as number[]).buffer, received: `${received} of numbers` };
  }
  return { buf: null, received };
}

async function transfer(bytes: number, seed: number): Promise<Transfer> {
  const p = await backend.benchPrepare(bytes, seed);
  const t0 = performance.now();
  const value: unknown = await backend.benchTake(p.token);
  const ms = performance.now() - t0;
  const { buf, received } = asBytes(value);
  const verified = received === 'ArrayBuffer' && buf !== null && buf.byteLength === bytes && checksum(buf) === p.checksum;
  const intact = buf !== null && buf.byteLength === bytes && checksum(buf) === p.checksum;
  return { bytes, ms, verified, received: intact ? received : `${received} (bytes differ)` };
}

/** Prepares every file first, then times taking them all, one after another or all at once. */
async function transferSet(concurrent: boolean, seed: number) {
  const prepared: Prepared[] = [];
  for (const [i, bytes] of CSBIN_ALL.entries()) prepared.push(await backend.benchPrepare(bytes, seed + i));
  const t0 = performance.now();
  const bufs: ArrayBuffer[] = [];
  if (concurrent) bufs.push(...(await Promise.all(prepared.map((p) => backend.benchTake(p.token)))));
  else for (const p of prepared) bufs.push(await backend.benchTake(p.token));
  const ms = performance.now() - t0;
  const verified = bufs.every((b, i) => b.byteLength === prepared[i].bytes && checksum(b) === prepared[i].checksum);
  return { ms, verified };
}

async function benchmarkIpc() {
  const pings: number[] = [];
  for (let i = 0; i < 100; i++) {
    const t0 = performance.now();
    await backend.ping();
    pings.push(performance.now() - t0);
  }
  const cold = await transfer(CSBIN_GLOBAL, 1);
  const single: Transfer[] = [];
  for (let i = 0; i < 7; i++) single.push(await transfer(CSBIN_GLOBAL, 100 + i));
  const sequential = [];
  for (let i = 0; i < 3; i++) sequential.push(await transferSet(false, 1000 * (i + 1)));
  const concurrent = [];
  for (let i = 0; i < 3; i++) concurrent.push(await transferSet(true, 5000 * (i + 1)));
  await backend.benchClear();
  const setBytes = CSBIN_ALL.reduce((a, b) => a + b, 0);
  const singleMs = median(single.map((t) => t.ms));
  const setMs = median(sequential.map((t) => t.ms));
  return {
    path: 'tauri::ipc::Response (raw ArrayBuffer)',
    single_16MB_ms: round3(singleMs),
    all_bands_98MB_ms: round3(setMs),
    all_bands_98MB_concurrent_ms: round3(median(concurrent.map((t) => t.ms))),
    single_bytes: CSBIN_GLOBAL,
    all_bands_bytes: setBytes,
    all_bands_files: CSBIN_ALL.length,
    single_MBps: round3(CSBIN_GLOBAL / 1e6 / (singleMs / 1e3)),
    all_bands_MBps: round3(setBytes / 1e6 / (setMs / 1e3)),
    single_cold_ms: round3(cold.ms),
    single_samples_ms: single.map((t) => round3(t.ms)),
    all_bands_samples_ms: sequential.map((t) => round3(t.ms)),
    all_bands_concurrent_samples_ms: concurrent.map((t) => round3(t.ms)),
    ping_median_ms: round3(median(pings)),
    ping_p99_ms: round3(quantile(pings, 0.99)),
    verified: cold.verified && single.every((t) => t.verified) && [...sequential, ...concurrent].every((t) => t.verified),
    limit_single_16MB_ms: SINGLE_16MB_LIMIT_MS,
  };
}

async function probePanic() {
  let error: CmdError | null = null;
  let resolved = false;
  try {
    await backend.panicProbe();
    resolved = true;
  } catch (e) {
    error = asCmdError(e);
  }
  const pingAfter = await backend.ping().catch((e) => `error: ${asCmdError(e).message}`);
  return { resolved, error, ping_after: pingAfter };
}

/**
 * Without the guard, Tauri drops the reply of a panicked async command. The page's fetch over the
 * IPC custom protocol then fails, and Tauri's IPC script marks the protocol failed for the rest of
 * the page's life and re-sends the call over postMessage: the command body runs a second time and
 * the promise never settles. Run LAST, because every later call takes the postMessage path; the
 * transfer measured afterwards shows what that path costs.
 */
async function probeUnguardedPanic() {
  const runsBefore = await backend.unguardedPanicRuns();
  const outcome = await Promise.race([
    backend.panicProbeUnguarded().then(
      () => ({ outcome: 'resolved', error: null as CmdError | null }),
      (e) => ({ outcome: 'rejected', error: asCmdError(e) }),
    ),
    sleep(2000).then(() => ({ outcome: 'still pending after 2000 ms', error: null as CmdError | null })),
  ]);
  const pingAfter = await backend.ping().catch((e) => `error: ${asCmdError(e).message}`);
  const runs = (await backend.unguardedPanicRuns()) - runsBefore;
  const after = await transfer(CSBIN_GLOBAL, 9001);
  await backend.benchClear();
  return {
    ...outcome,
    body_runs: runs,
    ping_after: pingAfter,
    single_16MB_after_ms: round3(after.ms),
    single_16MB_after_received: after.received,
  };
}

async function probeChannel(lines = 2000, spacingUs = 500) {
  const arrivals: number[] = [];
  const batchNumbers: number[] = [];
  const seqs: number[] = [];
  let sawLast = false;
  let resolveLast: () => void = () => {};
  const lastSeen = new Promise<void>((r) => (resolveLast = r));
  const channel = new Channel<RunEventBatch>((batch) => {
    arrivals.push(performance.now());
    batchNumbers.push(batch.batch);
    for (const e of batch.events) seqs.push(e.seq);
    if (batch.last) {
      sawLast = true;
      resolveLast();
    }
  });
  const report = await backend.runEventsProbe(channel, lines, spacingUs);
  await Promise.race([lastSeen, sleep(5000)]);
  const gaps = arrivals.slice(1).map((t, i) => t - arrivals[i]);
  const inOrder = seqs.length === lines && seqs.every((s, i) => s === i);
  const batchesContiguous = batchNumbers.every((b, i) => b === i);
  return {
    lines,
    spacing_us: spacingUs,
    received: seqs.length,
    batches: batchNumbers.length,
    in_order: inOrder,
    batches_contiguous: batchesContiguous,
    saw_last: sawLast,
    median_gap_ms: gaps.length ? round3(median(gaps)) : null,
    backend: report,
  };
}

async function probeBridge() {
  const probe = await backend.exactFloatProbe(`[${MISREAD.join(', ')}]`);
  const jsBits = MISREAD.map((t) => bitsOf(Number(t)));
  await backend.projectNew('Self-test');
  const eye = MISREAD.map(Number) as [number, number, number];
  const op: Op = { op: 'set_camera', camera: { eye, target: [0, 0, 0], vertical_fov_deg: 45 } };
  const applied = await backend.projectApply(op);
  const project = JSON.parse(await backend.projectJson());
  const backBits = (project.view.camera?.eye ?? []).map((x: number) => bitsOf(x));
  const undone = await backend.projectUndo();
  const afterUndo = JSON.parse(await backend.projectJson());
  return {
    values: MISREAD,
    js_bits: jsBits,
    exact_bits: probe.exact_bits,
    serde_json_bits: probe.serde_json_bits,
    serde_json_disagreements: probe.disagreements,
    op_roundtrip_bits: backBits,
    applied_can_undo: applied.can_undo,
    undo_can_redo: undone.can_redo,
    camera_after_undo: afterUndo.view.camera,
  };
}

async function probeFonts() {
  const faces: { family: string; weight: string; status: string }[] = [];
  for (const face of document.fonts) {
    try {
      await face.load();
    } catch {
      /* status stays 'error' */
    }
    faces.push({ family: face.family.replace(/["']/g, ''), weight: face.weight, status: face.status });
  }
  return { faces, loaded: faces.filter((f) => f.status === 'loaded').length };
}

async function probeCsp() {
  const violations: string[] = [];
  const onViolation = (e: SecurityPolicyViolationEvent) => violations.push(e.effectiveDirective);
  document.addEventListener('securitypolicyviolation', onViolation);
  const w = window as Window & { __simpaCspProbe?: boolean };
  const s = document.createElement('script');
  s.textContent = 'window.__simpaCspProbe = true;';
  document.head.appendChild(s);
  s.remove();
  let evalBlocked = false;
  try {
    new Function('return 1')();
  } catch {
    evalBlocked = true;
  }
  await sleep(100);
  document.removeEventListener('securitypolicyviolation', onViolation);
  return {
    inline_script_ran: w.__simpaCspProbe === true,
    eval_blocked: evalBlocked,
    violations,
    prototype_frozen: Object.isFrozen(Object.prototype),
  };
}

/** Numbers with an acoustic unit anywhere in the visible text (the physics-before-pixels rule). */
const ACOUSTIC = /\d(?:[\d.,]*\d)?\s*(?:dB|Hz|kHz|ms|s|%|m²|m³|m|°C)(?![\p{L}\p{N}])/gu;

function probeDom() {
  const steps = [...document.querySelectorAll<HTMLElement>('[data-step]')];
  const names = steps.map((el) => el.querySelector('[data-part="name"]')?.textContent ?? '');
  const visible = steps.map((el) => {
    const r = el.getBoundingClientRect();
    return r.width > 0 && r.height > 0 && getComputedStyle(el).visibility !== 'hidden';
  });
  const text = document.body.innerText;
  const matches = text.match(ACOUSTIC) ?? [];
  return {
    steps: names,
    steps_visible: visible,
    current_step: document.querySelector('[data-step][aria-current="step"]')?.getAttribute('data-step') ?? null,
    acoustic_number_matches: matches.length,
    acoustic_number_samples: matches.slice(0, 10),
    menu: [...document.querySelectorAll('[data-menu]')].map((el) => el.textContent),
    dock_tabs: [...document.querySelectorAll('[data-dock-tab]')].map((el) => el.getAttribute('data-dock-tab')),
    viewport_px: (() => {
      const r = document.querySelector('[data-part="viewport"]')?.getBoundingClientRect();
      return r ? [Math.round(r.width), Math.round(r.height)] : null;
    })(),
  };
}

export async function runSelftest(webgl: WebGLInfo): Promise<void> {
  statusStore.set({ kind: 'busy', text: 'Self-test running' });
  log('INFO', 'Self-test started (--selftest)');
  const t0 = performance.now();
  const report: Record<string, unknown> = { ok: false, format: 'simpa-app-selftest/1', webgl };
  const checks: Record<string, boolean> = {};
  try {
    await document.fonts.ready;
    report.fonts = await probeFonts();
    report.csp = await probeCsp();
    report.webgpu = await probeWebGPU();
    const ipc = await benchmarkIpc();
    report.ipc = ipc;
    const panic = await probePanic();
    report.panic_probe = panic;
    const channel = await probeChannel();
    report.channel = channel;
    const bridge = await probeBridge();
    report.bridge = bridge;
    const dom = probeDom();
    report.dom = dom;
    const unguarded = await probeUnguardedPanic();
    report.panic_probe_unguarded = unguarded;
    const kat = knownAnswers(checksumKat.cases);
    report.checksum_known_answers = kat;
    const fonts = report.fonts as Awaited<ReturnType<typeof probeFonts>>;
    const csp = report.csp as Awaited<ReturnType<typeof probeCsp>>;

    checks.webgl2_context = webgl.context === 'webgl2' && !!webgl.renderer;
    checks.webgl_max_texture_size = (webgl.max_texture_size ?? 0) >= 4096;
    checks.ipc_transfers_verified = ipc.verified;
    checks.ipc_numbers_present =
      Number.isFinite(ipc.single_16MB_ms) && ipc.single_16MB_ms > 0 && Number.isFinite(ipc.all_bands_98MB_ms) && ipc.all_bands_98MB_ms > 0;
    checks.ipc_single_16MB_within_limit = ipc.single_16MB_ms <= SINGLE_16MB_LIMIT_MS;
    checks.panic_probe_returns_error = !panic.resolved && panic.error?.code === 'PANIC';
    checks.ping_after_panic = panic.ping_after === 'pong';
    checks.alive_after_unguarded_panic = unguarded.ping_after === 'pong';
    checks.channel_complete_in_order = channel.in_order && channel.batches_contiguous && channel.saw_last;
    // About one batch per 50 ms, measured, not just reported: the backend's period must be 50 ms,
    // the arrival gaps in the webview must sit near it, and the batch count must match the run's
    // length. A 10 ms batcher (about 100 batches, 10 ms gaps) or a 100 ms one fails all three.
    const expectedBatches = channel.backend.elapsed_ms / BATCH_PERIOD_MS;
    checks.channel_batch_period_50ms = channel.backend.batch_period_ms === BATCH_PERIOD_MS;
    checks.channel_gap_near_period =
      channel.median_gap_ms !== null && channel.median_gap_ms >= 0.8 * BATCH_PERIOD_MS && channel.median_gap_ms <= 1.5 * BATCH_PERIOD_MS;
    checks.channel_batched = channel.batches >= 0.7 * expectedBatches && channel.batches <= 1.3 * expectedBatches + 2;
    checks.checksum_known_answers = kat.length >= 6 && kat.every((r) => r.ok);
    checks.bridge_exact_floats = JSON.stringify(bridge.exact_bits) === JSON.stringify(MISREAD_BITS) && JSON.stringify(bridge.js_bits) === JSON.stringify(MISREAD_BITS);
    checks.bridge_op_roundtrip_exact = JSON.stringify(bridge.op_roundtrip_bits) === JSON.stringify(MISREAD_BITS);
    checks.bridge_undo = bridge.applied_can_undo && bridge.undo_can_redo && bridge.camera_after_undo === null;
    checks.step_bar_five_steps = JSON.stringify(dom.steps) === JSON.stringify(STEP_NAMES) && dom.steps_visible.every(Boolean);
    checks.no_acoustic_numbers = dom.acoustic_number_matches === 0;
    checks.fonts_bundled_and_loaded = fonts.faces.length === 7 && fonts.loaded === 7;
    checks.csp_blocks_inline_script = !csp.inline_script_ran;
    checks.csp_blocks_eval = csp.eval_blocked;
    checks.prototype_frozen = csp.prototype_frozen;
  } catch (e) {
    report.error = asCmdError(e);
  }
  report.checks = checks;
  const failed = Object.entries(checks).filter(([, ok]) => !ok).map(([k]) => k);
  report.failed = failed;
  report.ok = !report.error && failed.length === 0 && Object.keys(checks).length > 0;
  report.duration_ms = round3(performance.now() - t0);
  const passed = Object.keys(checks).length - failed.length;
  log(report.ok ? 'OK' : 'FAIL', `Self-test ${report.ok ? 'passed' : 'failed'}: ${passed} of ${Object.keys(checks).length} checks`);
  statusStore.set(report.ok ? { kind: 'ready', text: 'Self-test passed' } : { kind: 'bad', text: 'Self-test failed' });
  await backend.selftestReport(report);
}
