// The response window (Burhan 2026-10-04 11:45): the selected receiver's energy echogram per
// octave band as a time x band map in the Dockyard black-red-white map. The box
// (tests/fixtures/ui/box_run.simpa, one source) run from the app. Each id with its control:
//   resp-window   the Decay card's "Open response" opens a floating window over the app (outside
//                 the tab, on the screen) titled as an SPPS energy echogram, saying it is not a
//                 pressure impulse response; its close button removes it. Control: before the
//                 click, and after the close, there is no window
//   resp-pixels   the map is an image (no canvas: M10 PLAN rule 3, the 3D view's is the only one
//                 in the document, the window open), a PNG this spec decodes itself (node:zlib)
//                 and the WebView decoded too (its natural size); it holds one pixel per bin and
//                 each is the colour this spec computes from `simpa results <run> --json` (its own
//                 copy of the map's stops): the loudest bin white, a bin 60 dB or more down
//                 black; picking another receiver redraws it from that receiver's series.
//                 Control: the drawn map against another receiver's expectation is caught
//   resp-numbers  every number the window prints (band labels, time ticks) equals the JSON at
//                 the precision shown, every name is the JSON's, the colour bar's words are the
//                 span's, and no digit is outside them. Control: a planted wrong digit is caught.
//                 A screenshot of the open window goes to $M11_SCREENS (ir-window.png)
//
// Its files, under <M11_WORK>\m12-response (C:). The repository is only read.
import { strict as assert } from 'node:assert';
import { mkdirSync } from 'node:fs';
import { inflateSync } from 'node:zlib';
import path from 'node:path';
import { at, numberMismatch, strayDigits, stringMismatch, type NumEl } from '../lib/acoustics.ts';
import { cliReport, ownBox, PANEL, pick, type Row, runSpps, showRun } from '../lib/acousticsTab.ts';
import { clickSelector } from '../lib/dom.ts';
import { hook, m10, waitForHooks } from '../lib/hooks.ts';
import { env } from '../lib/types.ts';

const WORK = () => path.join(env('M11_WORK'), 'm12-response');
const WIN = '[data-response-window]';
const TITLE = 'Energy response per band (SPPS echogram)';
const SPAN = 60;
/** The colour bar's words, the span's ends and thirds. */
const SPAN_WORDS = ['0 dB', '−20', '−40', '−60 dB'];
/** The map's stops as the request gives them: black, deep red #3a0b10, red #e0202e, white. */
const STOPS: [number, number[]][] = [
  [0, [0, 0, 0]],
  [1 / 3, [0x3a, 0x0b, 0x10]],
  [2 / 3, [0xe0, 0x20, 0x2e]],
  [1, [255, 255, 255]],
];

type Json = Record<string, unknown>;
interface HookView {
  receiver: number;
  source: string | null;
  k0: number;
  cols: number;
  paths: string[];
  max: { band: number; col: number };
}

/** The colour of a level `db` re the maximum, written from the request, not from the UI. */
function colour(db: number): number[] {
  const t = (Math.max(Math.min(db, 0), -SPAN) + SPAN) / SPAN;
  for (let i = 1; i < STOPS.length; i++) {
    const [ta, a] = STOPS[i - 1];
    const [tb, b] = STOPS[i];
    if (t <= tb) return a.map((v, k) => Math.round(v + ((t - ta) / (tb - ta)) * (b[k] - v)));
  }
  return [255, 255, 255];
}

/** Receiver `r`'s map as the JSON gives it: per band its series from the source's emission, each
 * bin in dB re the largest. */
function expected(json: Json, r: number) {
  const bands = (at(json, `spps.point_receivers.${r}.bands`) as unknown[]).length;
  const dt = at(json, 'spps.time_step_s') as number;
  const sources = at(json, 'spps.sources') as { emission_s: number }[];
  const k0 = Math.round(Math.min(...sources.map((s) => s.emission_s)) / dt);
  const paths = Array.from({ length: bands }, (_, b) => `spps.point_receivers.${r}.bands.${b}.energy_pa2`);
  const rows = paths.map((p) => (at(json, p) as number[]).slice(k0));
  const max = Math.max(...rows.flat());
  const db = rows.map((row) => row.map((e) => (e > 0 ? 10 * Math.log10(e / max) : -Infinity)));
  return { k0, paths, rows, db, cols: rows[0].length, bands };
}

/** A PNG decoded here, sharing no code with the app's encoder: 8-bit RGBA, no interlace, rows
 * unfiltered as the app writes them (any other filter is refused, not guessed). */
function decodePng(bytes: Buffer): { w: number; h: number; data: number[] } {
  assert.equal(bytes.subarray(0, 8).toString('hex'), '89504e470d0a1a0a', 'the PNG signature');
  let o = 8;
  let w = 0;
  let h = 0;
  const idat: Buffer[] = [];
  while (o < bytes.length) {
    const len = bytes.readUInt32BE(o);
    const type = bytes.toString('latin1', o + 4, o + 8);
    if (type === 'IHDR') {
      w = bytes.readUInt32BE(o + 8);
      h = bytes.readUInt32BE(o + 12);
      assert.deepEqual([...bytes.subarray(o + 16, o + 21)], [8, 6, 0, 0, 0], 'IHDR: 8-bit RGBA, no interlace');
    }
    if (type === 'IDAT') idat.push(bytes.subarray(o + 8, o + 8 + len));
    o += 12 + len;
  }
  const raw = inflateSync(Buffer.concat(idat));
  assert.equal(raw.length, h * (w * 4 + 1), 'a filter byte and a row per line');
  const data: number[] = [];
  for (let y = 0; y < h; y++) {
    const r = y * (w * 4 + 1);
    assert.equal(raw[r], 0, `row ${y}: filter ${raw[r]}`);
    for (const v of raw.subarray(r + 1, r + 1 + w * 4)) data.push(v);
  }
  return { w, h, data };
}

/** The map image's pixels, as RGBA, with its size, decoded from its data URL; it also holds the
 * document to one canvas (none in the window) and the WebView's decode to the same size. */
async function pixels(): Promise<{ w: number; h: number; data: number[] } | null> {
  const got = await browser.execute(() => {
    const img = document.querySelector<HTMLImageElement>('[data-response-window] img[data-part="response-map"]');
    return img
      ? {
          src: img.getAttribute('src') ?? '',
          complete: img.complete,
          natural: [img.naturalWidth, img.naturalHeight],
          canvases: document.querySelectorAll('canvas').length,
          inWindow: document.querySelectorAll('[data-response-window] canvas').length,
        }
      : null;
  });
  if (!got) return null;
  assert.equal(got.inWindow, 0, 'a canvas in the response window');
  assert.equal(got.canvases, 1, "the document holds one canvas, the 3D view's (M10 rule 3)");
  const prefix = 'data:image/png;base64,';
  assert.ok(got.src.startsWith(prefix), `the map's src: ${got.src.slice(0, 40)}`);
  const px = decodePng(Buffer.from(got.src.slice(prefix.length), 'base64'));
  // The WebView decoded it too (a data URL the CSP refused would read 0 x 0).
  assert.ok(got.complete, 'the map image loaded');
  assert.deepEqual(got.natural, [px.w, px.h], 'the size the WebView decoded the map at');
  return px;
}

/** Pixels that are not the expected map's colour (within 1 per channel), with where. */
function pixelMismatches(px: { w: number; h: number; data: number[] }, want: ReturnType<typeof expected>): string[] {
  const out: string[] = [];
  if (px.w !== want.cols || px.h !== want.bands) return [`image ${px.w} x ${px.h}, the JSON's map ${want.cols} x ${want.bands}`];
  for (let b = 0; b < want.bands; b++) {
    const y = want.bands - 1 - b;
    for (let c = 0; c < want.cols; c++) {
      const i = (y * want.cols + c) * 4;
      const got = px.data.slice(i, i + 4);
      const rgb = colour(want.db[b][c]);
      if (got[3] !== 255 || rgb.some((v, k) => Math.abs(v - got[k]) > 1)) out.push(`band ${b} step ${c}: drawn ${got.join(',')}, want ${rgb.join(',')} (${want.db[b][c].toFixed(1)} dB)`);
    }
  }
  return out;
}

const pixelAt = (px: { w: number; data: number[] }, bands: number, b: number, c: number) => px.data.slice(((bands - 1 - b) * px.w + c) * 4, ((bands - 1 - b) * px.w + c) * 4 + 3);

/** The window as shown: its marked numbers, strings and words, and its text with them removed. */
const scanWindow = () =>
  browser.execute((sel: string) => {
    const w = document.querySelector<HTMLElement>(sel);
    const all = (s: string) => (w ? [...w.querySelectorAll<HTMLElement>(s)] : []);
    const clone = w ? (w.cloneNode(true) as HTMLElement) : null;
    clone?.querySelectorAll('[data-num], [data-str], [data-label]').forEach((e) => e.remove());
    return {
      nums: all('[data-num]').map((e) => ({ path: e.getAttribute('data-json') ?? '', text: e.textContent ?? '', digits: e.getAttribute('data-digits'), scale: e.getAttribute('data-scale') })),
      bandPaths: all('[data-part="response-bands"] [data-num]').map((e) => e.getAttribute('data-json') ?? ''),
      tickPaths: all('[data-part="response-ticks"] [data-num]').map((e) => ({ path: e.getAttribute('data-json') ?? '', scale: e.getAttribute('data-scale') ?? '' })),
      strs: all('[data-str]').map((e) => ({ path: e.getAttribute('data-json') ?? '', text: e.textContent ?? '' })),
      labels: all('[data-label]').map((e) => ({ kind: e.getAttribute('data-label') ?? '', text: e.textContent ?? '' })),
      stray: clone?.textContent ?? '',
      text: w?.innerText ?? '',
    };
  }, WIN);

const open = async () => {
  await clickSelector(`${PANEL} [data-action="open-response"]`);
  await $(WIN).waitForExist({ timeout: 10_000 });
  await m10.idle();
};

describe('The response window: the energy echogram per band', () => {
  let run: Row | undefined;
  let json: Json = {};

  before(async () => {
    await waitForHooks(['idle', 'openProject', 'setStep', 'dockTab', 'runStart', 'runState', 'runsRows', 'selectRun', 'setSolver', 'acousticsView', 'responseView']);
    const box = ownBox(WORK());
    await m10.openProject(box);
    run = await runSpps();
    json = cliReport(box, run.run);
    await showRun(run.run);
  });

  it('resp-window: "Open response" opens a floating window, titled an SPPS energy echogram; its button closes it', async () => {
    // Control: no window before the click.
    assert.equal(await $(WIN).isExisting(), false, 'a window before the click');
    assert.equal(await hook<HookView | null>('responseView'), null, 'the hook before the click');
    await open();
    const w = await browser.execute((sel: string, panel: string) => {
      const el = document.querySelector<HTMLElement>(sel);
      const r = el?.getBoundingClientRect();
      return {
        inPanel: !!el?.closest(panel),
        title: el?.querySelector('[data-part="response-title"]')?.textContent ?? '',
        note: el?.querySelector('[data-part="response-note"]')?.textContent ?? '',
        glass: el?.classList.contains('glass') ?? false,
        box: r ? { l: r.left, t: r.top, r: r.right, b: r.bottom } : null,
        view: { w: innerWidth, h: innerHeight },
      };
    }, WIN, PANEL);
    assert.equal(w.title, TITLE);
    assert.match(w.note, /not a pressure impulse response/);
    assert.equal(w.inPanel, false, 'the window floats outside the tab');
    assert.ok(w.glass, 'on the glass of decision-log row 50');
    assert.ok(w.box && w.box.l >= 0 && w.box.t >= 0 && w.box.r <= w.view.w && w.box.b <= w.view.h, `on screen: ${JSON.stringify(w)}`);
    await clickSelector(`${WIN} [data-action="close-response"]`);
    await browser.waitUntil(async () => !(await $(WIN).isExisting()), { timeout: 10_000, timeoutMsg: 'the window did not close' });
    assert.equal(await hook<HookView | null>('responseView'), null, 'the hook after the close');
    await open();
    console.log(`resp-window receipt: run ${run?.run}; opened, closed and opened again; ${JSON.stringify(w.box)} in ${w.view.w} x ${w.view.h}`);
  });

  it('resp-pixels: each bin of the map is the colour of its level in the JSON; another receiver redraws it', async () => {
    const v = await hook<HookView>('responseView');
    assert.equal(v.receiver, 0);
    assert.equal(v.source, null, 'one source: the receiver series');
    const want = expected(json, 0);
    assert.deepEqual(v.paths, want.paths, 'the series drawn');
    assert.equal(v.k0, want.k0);
    const px = await pixels();
    assert.ok(px, 'the map image');
    const bad = pixelMismatches(px, want);
    assert.deepEqual(bad.slice(0, 10), [], `${bad.length} of ${want.cols * want.bands} pixels`);
    // The loudest bin is white, not black; a bin 60 dB or more down is black.
    const flat = want.db.flatMap((row, b) => row.map((d, c) => ({ b, c, d })));
    const top = flat.find((x) => x.d === 0);
    assert.ok(top, 'a loudest bin');
    assert.deepEqual({ band: top.b, col: top.c }, v.max, 'the maximum the window found');
    const hot = pixelAt(px, want.bands, top.b, top.c);
    assert.deepEqual(hot, [255, 255, 255], 'the loudest bin');
    const down: { b: number; c: number; d: number }[] = [];
    want.db.forEach((row, b) => row.forEach((d, c) => (d <= -SPAN ? down.push({ b, c, d }) : null)));
    assert.ok(down.length > 0, 'bins 60 dB down');
    for (const x of down) assert.ok(pixelAt(px, want.bands, x.b, x.c).every((ch) => ch <= 2), `band ${x.b} step ${x.c} at ${x.d.toFixed(1)} dB is not black`);
    const near = down.filter((x) => x.d > -SPAN - 1);
    // Control: the drawn map against receiver R2's series is caught.
    const other = pixelMismatches(px, expected(json, 1));
    assert.ok(other.length > 0, "the map against another receiver's series");
    // Another receiver, picked as a user does: the window redraws from its series.
    await pick('receiver', '1');
    const v1 = await hook<HookView>('responseView');
    assert.equal(v1.receiver, 1);
    const want1 = expected(json, 1);
    assert.deepEqual(v1.paths, want1.paths);
    const px1 = await pixels();
    assert.ok(px1);
    const bad1 = pixelMismatches(px1, want1);
    assert.deepEqual(bad1.slice(0, 10), [], `R2: ${bad1.length} pixels`);
    await pick('receiver', '0');
    console.log(
      `resp-pixels receipt: ${want.cols} steps x ${want.bands} bands from step ${want.k0}; ${bad.length} mismatched pixels (R1), ${bad1.length} (R2); loudest bin band ${top.b} step ${top.c} drawn ${hot.join(',')}; ${down.length} bins >= 60 dB down all black (${near.length} within 1 dB of the edge); control ${other.length} pixels differ from R2's map`,
    );
  });

  it('resp-numbers: every number the window prints is the JSON at the precision shown', async () => {
    const s = await scanWindow();
    const mismatches: string[] = [];
    for (const n of s.nums) {
      const m = numberMismatch(n as NumEl, json);
      if (m) mismatches.push(m);
    }
    for (const t of s.strs) {
      const m = stringMismatch(t, json);
      if (m) mismatches.push(m);
    }
    assert.deepEqual(mismatches, [], `${mismatches.length} of ${s.nums.length} numbers and ${s.strs.length} strings`);
    assert.equal(strayDigits(s.stray), null, `a digit outside the marked numbers: ${strayDigits(s.stray)}`);
    for (const l of s.labels) {
      assert.equal(l.kind, 'span', `a label of kind ${l.kind}`);
      assert.ok(SPAN_WORDS.includes(l.text), `a span word not known: ${JSON.stringify(l.text)}`);
    }
    assert.ok(!/validated/i.test(s.text), 'no "validated"');
    // The band labels are the receiver's bands, lowest first; the ticks the step times a whole number.
    const bands = (at(json, 'spps.point_receivers.0.bands') as unknown[]).length;
    assert.deepEqual(s.bandPaths, Array.from({ length: bands }, (_, b) => `spps.point_receivers.0.bands.${b}.freq_hz`));
    const cols = expected(json, 0).cols;
    assert.ok(s.tickPaths.length >= 2, 'time ticks');
    for (const t of s.tickPaths) {
      assert.equal(t.path, 'spps.time_step_s');
      assert.ok(Number.isInteger(Number(t.scale)) && Number(t.scale) >= 1 && Number(t.scale) <= cols, `tick scale ${t.scale}`);
    }
    assert.ok(s.strs.some((t) => t.path === 'spps.point_receivers.0.label'), 'the receiver named');
    // Control: a planted wrong digit is caught.
    const first = s.nums.find((n) => n.digits !== '0') ?? s.nums[0];
    const last = first.text.slice(-1);
    const planted = { ...first, text: first.text.slice(0, -1) + (last === '9' ? '8' : String(Number(last) + 1)) };
    assert.notEqual(numberMismatch(planted as NumEl, json), null, 'a planted wrong digit');
    // The picture: the window open over the app.
    const dir = env('M11_SCREENS');
    mkdirSync(dir, { recursive: true });
    await browser.action('pointer', { parameters: { pointerType: 'mouse' } }).move({ x: 2, y: 2, origin: 'viewport', duration: 0 }).perform();
    await m10.idle();
    await browser.execute(() => new Promise<void>((r) => requestAnimationFrame(() => requestAnimationFrame(() => r()))));
    const file = path.join(dir, 'ir-window.png');
    await browser.saveScreenshot(file);
    console.log(`resp-numbers receipt: ${s.nums.length} numbers and ${s.strs.length} strings equal the JSON; ${s.labels.length} span words; band paths ${s.bandPaths.length}, ticks ${s.tickPaths.map((t) => t.scale).join(',')}; screenshot ${file}`);
  });
});
