# IPC between the core and the UI

**Decided 2026-09-23 (M9), on measurements from Grace.** Datasets cross as raw
`tauri::ipc::Response` bytes. Run events cross on one `tauri::ipc::Channel` per run, batched at
50 ms. The WebView2 virtual-host fallback is **not built**: one 16.5 MB transfer takes about
95 ms against the plan's 500 ms threshold.

## The record

`tools/gates/m9.ps1` reads this table (check c). The run is the gate's
`target/gates/m9/20260923-172234/selftest.json`, a passing run: the self-test reported `ok: true`
with 22 of 22 checks, and the whole gate passed.

| Key | Value | Notes |
|---|---|---|
| `date` | 2026-09-23 | 17:23 CEST, written 15:23:32Z |
| `machine` | GRACE | i7-14700KF, RTX 5070 12 GB (the only adapter), 32 GB DDR5 at JEDEC 4800, Windows 11 Pro |
| `single_16MB_ms` | 94.0 | Median of 7 (92.2 to 97.5). One 16,470,804 B buffer (the size of Elmia's Global `rs_cut.csbin`) = 175 MB/s |
| `all_bands_98MB_ms` | 622.7 | Median of 3 (603.0 to 631.9). 7 buffers, 98,439,868 B (Global plus 6 bands), taken one after another = 158 MB/s |
| `all_bands_98MB_concurrent_ms` | 359.1 | Median of 3. The same 7, taken with `Promise.all` |
| `ping_median_ms` | 0.6 | Median of 100 `ping` round trips; p99 1.1 ms |
| `threshold_single_16MB_ms` | 500 | The plan's proposed limit (M9 c); 5.3x headroom |

The stack: release build, Tauri 2.11.6, tauri-cli 2.11.5, WebView2 Runtime 153.0.4234.48, app
0.1.0 at commit `59d1ff2` plus the uncommitted M9 work.

**Every run so far.** They vary by about 13 %. Other agents were compiling on the same machine
during all of them, so the load is not controlled. "Self-test" is the report's `ok`.

| Run (local time) | `single_16MB_ms` | `all_bands_98MB_ms` | concurrent | Self-test |
|---|---|---|---|---|
| 15:13, first build (standalone crate) | 94.1 | 575.4 | 373.9 | not kept |
| 16:50, gate run 1 | 93.1 | 575.4 | 343.7 | ok |
| 16:52, gate run 2 | 105.4 | 661.5 | 378.4 | **failed** (see below) |
| 16:56, gate run 3 | 94.2 | 589.9 | 340.7 | ok |
| 17:00, the verifier's run | 94.2 | 583.0 | 393.9 | ok |
| 17:18, gate run 4 (after the verifier's fixes) | 95.5 | 639.1 | 390.0 | ok |
| 17:23, gate run 5 (**the record**) | 94.0 | 622.7 | 359.1 | ok |

Until 17:23 the record was gate run 2. Its self-test failed one check, `alive_after_unguarded_panic`,
in a form since removed: after the unguarded panic it asked for a verified `ArrayBuffer`, which the
postMessage path no longer delivers (see *Panics*). That check ran after the IPC benchmark, so the
run's transfer numbers were valid, but the record now names a run that passed.

The survey's anecdotal figure was about 50 MB/s (10 MB in about 200 ms, December 2024). Grace
measures about 3x that.

## How it is measured

`app.exe --selftest <out.json>` opens the window. The UI measures itself and hands its report to
the backend, which adds `meta` (machine, versions, time) and writes the file. The app then exits:
0 when every check passed, 1 when any failed, and 3 when the UI never reported within 180 s. A
watchdog writes that failure file, so the gate never hangs and never reads a stale report.

- **Only the transfer is timed.** `bench_prepare` builds a seeded splitmix64 buffer in the
  backend and returns its checksum. The timer covers `invoke('bench_take')` only, until the
  `ArrayBuffer` arrives in JS.
- **Every transfer is verified.** JS recomputes a position-weighted checksum over each buffer, so
  a truncated, reordered or shifted transfer fails the check. The Rust version (`bench.rs`) and
  the TypeScript version (`ui/src/checksum.ts`) are held to the same known answers,
  `ui/src/checksum-kat.json`, whose values come from a third implementation (Python). Rust checks
  them in `cargo test`, TypeScript in `npm test` (node) and again in the self-test, on the bundled
  code inside the webview. Each live transfer then compares the two on real buffers.
- **The sizes are the real files,** from the corrected Elmia solve:
  `build-clean/sim_output/spps/Surface_receiver/<band>/rs_cut.csbin`. The content is synthetic.
- **The benchmark runs before anything that could change the IPC path** (see *Panics* below).

## What it means for the data path

- **Load on select, never per frame.** One band's surface map costs about 0.1 s and all seven
  about 0.4-0.7 s. The core decodes each solver file once into GPU-ready typed arrays, and the
  GPU animates them with a time uniform (plan, survey finding 4).
- **Large transfers stall the window.** Wry hands the custom-protocol response to the WebView2
  UI thread (survey finding 4, not re-measured here). A 16.5 MB transfer holds that thread for
  about 100 ms. That is acceptable for a click, but not in a loop. M11's gate (b), ping p99 below
  100 ms during a solve, will show whether a transfer landing mid-run is a problem.
- **Concurrent requests are faster.** Taking the seven buffers together took 359 ms against
  623 ms one after another. When a result set loads, request its datasets together.
- **When to build the fallback.** Build it when a dataset grows past about 5x today's, for
  example 10k rendered particles at about 64 MB per band, or when `single_16MB_ms` exceeds 500 on
  the target machine. The fallback is WebView2 `SetVirtualHostNameToFolderMapping` over
  `app_cache_dir/<run-id>`, in `app/src-tauri/src/webview2.rs` and nowhere else. It must then be
  shown faster by the same self-test (gate c).

## Run events

One `Channel<RunEventBatch>` per run. A batcher thread sends each batch at most 50 ms after the
batch's first event arrived. Events are numbered (`seq`) and batches are numbered (`batch`), and
the last batch carries `last: true`.

**Measured (the record's run).** 2,000 synthetic lines at 0.5 ms spacing (1,000.2 ms) arrived as
20 batches with a median gap of 50.8 ms between arrivals in the webview. All 2,000 arrived in
order, the batches were contiguous and the last one was marked. There were 0 send failures.

**Gated.** The self-test and the M9 gate both require the backend's period to be 50 ms, the
median arrival gap to lie between 40 and 75 ms, and the batch count to lie between 0.7 and 1.3
times the run's length over 50 ms, plus 2. A 10 ms batcher (about 100 batches, 10 ms gaps) or a
100 ms one fails all three.

## Schema values cross as JSON text

No command argument is typed as a schema struct. The UI sends a project or an `Op` as JSON
**text**. The backend reads it with `simpa_core::schema::from_json` / `Op::from_json`, the core's
exact reader.

A typed argument would be read with `serde_json::from_str`, and in this build that misreads
floats. The self-test sends three shortest-round-trip decimals:

| Decimal | Exact reader, and JS `Number()` | `serde_json::from_str` |
|---|---|---|
| `1.5990461000457081` | `0x3ff995b15d07e1f0` | `0x3ff995b15d07e1ef` |
| `484.53035250855277` | `0x407e487c52e9795f` | `0x407e487c52e97960` |
| `980.0819440890737` | `0x408ea0a7d24d7560` | `0x408ea0a7d24d755f` |

A `set_camera` op carrying those values survives the trip UI → text → core → `.simpa` → UI with
the exact reader's bits, and undo restores the camera exactly.

## Panics

**Every command body runs through `guard::blocking`.** It runs the body on the blocking pool
under `catch_unwind` and returns a panic as `CmdError { code: "PANIC" }`. The release profile
keeps `panic = "unwind"`, which the guard depends on.

**Measured (gate d):** `panic_probe` rejects with `PANIC` and the message, and the next `ping`
returns `pong`.

**Without the guard it is worse than a lost reply.** The self-test measures this with
`panic_probe_unguarded`, which it runs last:

1. Tauri drops the reply of a panicked async command, so the page's `fetch` over the IPC custom
   protocol fails.
2. Tauri's IPC script (`tauri-2.11.6/scripts/ipc-protocol.js`) then sets
   `customProtocolIpcFailed = true` **for the rest of the page's life** and re-sends the same
   call over `postMessage`.
3. **The command body runs a second time.** Measured: `body_runs = 2`, and stderr shows the
   panic twice.
4. The second run panics too, and the promise never settles. Measured: still pending after
   2,000 ms.
5. Every later call takes the `postMessage` path. A 16.5 MB transfer afterwards took
   **1,381 to 1,593 ms in five runs, against about 95 ms before**, 15x slower.
6. **The bytes then arrive as a plain JS `Array` of 16.5 million numbers, not an `ArrayBuffer`.**
   The values are intact (the checksum matches), but code that expects an `ArrayBuffer` would
   misread them silently. `new Float32Array(value)` would turn each byte into a float, for
   example.

**What follows from it:**
- A panic that escapes a command can run a side effect twice, such as starting a solver.
- One escaped panic also slows every dataset load until the window reloads, and changes the type
  that dataset loads receive. **Every dataset loader (M7, M12) must check `instanceof
  ArrayBuffer` and refuse anything else**, rather than convert it.
- So the guard is not optional. The M9 gate fails any command whose body does not go through
  `guard::blocking`, apart from the probe itself.
- Commands with side effects (M6's run start) should also be idempotent per request id.

## WebGL and WebGPU on Grace

- **WebGL2:** `ANGLE (NVIDIA, NVIDIA GeForce RTX 5070 (0x00002F04) Direct3D11 vs_5_0 ps_5_0, D3D11)`.
  - The renderer string is unmasked through `WEBGL_debug_renderer_info`.
  - `MAX_TEXTURE_SIZE` is 16,384, `MAX_3D_TEXTURE_SIZE` 2,048 and `MAX_RENDERBUFFER_SIZE` 16,384.
  - The renderer also goes to the Console at startup, so a software fallback is never silent.
- **WebGPU:** `navigator.gpu` gives an adapter (vendor `nvidia`, architecture `blackwell`). This
  is informational only. The viewport stays on WebGL2 (survey finding 6).
- **Particle textures:** this bounds the layout the plan asked about. At 16,384 texels per row:
  - 500 particles × 400 steps is 200,000 texels, so 13 rows.
  - 10,000 particles × 400 steps is 4,000,000 texels, so 245 rows.

  Both are far inside the 16,384-row limit.
