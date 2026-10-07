// The response window (Burhan 2026-10-04 11:45): the selected receiver's energy echogram per
// octave band as a time x band map in the Dockyard black-red-white map. The box
// (tests/fixtures/ui/box_run.simpa, one source) run from the app. Each id with its control:
//   resp-window   the Decay card's "Open response" opens a floating window over the app (outside
//                 the tab, on the screen) titled as an SPPS energy echogram, saying it is not a
//                 pressure impulse response; its close button removes it. Control: before the
//                 click, and after the close, there is no window
//   resp-pixels   the map is an image (no canvas: M10 PLAN rule 3; the window holds none and
//                 opening it adds none), a PNG this spec decodes itself (node:zlib)
//                 and the WebView decoded too (its natural size); it holds one pixel per bin and
//                 each is the colour this spec computes from `simpa results <run> --json` (its own
//                 copy of the map's stops): the loudest bin white, a bin 60 dB or more down
//                 black; picking another receiver redraws it from that receiver's series.
//                 Control: the drawn map against another receiver's expectation is caught
//   resp-crop     the map ends shortly after the last step with a bin above -60 dB in any band
//                 (that time plus 5 %, rounded up to the 1-2-5 tick interval), computed here from
//                 the JSON; every bin past it is 60 dB down or more; "Full run" draws every step and
//                 says so, a second click cuts it again. Control: the cut map against the full
//                 run's width is caught. With nothing to cut, "Full run" is shown pressed and
//                 "Fit energy" disabled (re-pinned 2026-10-07: 7e44f63 offers the range always)
//   resp-numbers  every number the window prints (band labels, time ticks) equals the JSON at
//                 the precision shown, every name is the JSON's, the colour bar's words are the
//                 span's, and no digit is outside them; the time bins' widths are the JSON's time
//                 step, the span chips the offered spans, the readout (pointer off the map) holds
//                 no digit, and the pickers' options, checked on their own as the tab's are, are
//                 the JSON's receivers and the printed band labels. Control: a planted wrong digit
//                 is caught, and a changed receiver label.
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
/** The level spans the window offers (Burhan 2026-10-06 18:20: "the level span 30 to 100 dB"). */
const SPAN_CHOICES = ['30 dB', '40 dB', '60 dB', '80 dB', '100 dB'];
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
  shown: number;
  crop: number | null;
  full: boolean;
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
  const cols = rows[0].length;
  return { k0, paths, rows, db, cols, bands, crop: cropOf(db, cols, dt) };
}

/** Where the map is cut, from the request: the end of the last step with a bin above -60 dB in any
 * band, plus 5 %, rounded up to a whole multiple of the 1-2-5 interval for about 6 ticks over that
 * time (at least one step); null (the full run) when a band is above the floor at the last step or
 * the rounded end reaches the run's. */
function cropOf(db: number[][], cols: number, dt: number): number | null {
  let last = -1;
  db.forEach((row) => row.forEach((d, c) => (d > -SPAN && c > last ? (last = c) : null)));
  if (last >= cols - 1) return null;
  const t = (last + 1) * dt * 1.05;
  const raw = t / 6;
  const p = 10 ** Math.floor(Math.log10(raw));
  const iv = Math.max([p, 2 * p, 5 * p, 10 * p].find((v) => v >= raw * (1 - 1e-9)) ?? 10 * p, dt);
  const n = Math.round((Math.ceil(t / iv - 1e-9) * iv) / dt);
  return n >= cols ? null : n;
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
 * window to no canvas and the WebView's decode to the same size. */
async function pixels(): Promise<{ w: number; h: number; data: number[] } | null> {
  const got = await browser.execute(() => {
    const img = document.querySelector<HTMLImageElement>('[data-response-window] img[data-part="response-map"]');
    return img
      ? {
          src: img.getAttribute('src') ?? '',
          complete: img.complete,
          natural: [img.naturalWidth, img.naturalHeight],
          inWindow: document.querySelectorAll('[data-response-window] canvas').length,
        }
      : null;
  });
  if (!got) return null;
  assert.equal(got.inWindow, 0, 'a canvas in the response window');
  const prefix = 'data:image/png;base64,';
  assert.ok(got.src.startsWith(prefix), `the map's src: ${got.src.slice(0, 40)}`);
  const px = decodePng(Buffer.from(got.src.slice(prefix.length), 'base64'));
  // The WebView decoded it too (a data URL the CSP refused would read 0 x 0).
  assert.ok(got.complete, 'the map image loaded');
  assert.deepEqual(got.natural, [px.w, px.h], 'the size the WebView decoded the map at');
  return px;
}

/** Pixels that are not the expected map's colour (within 1 per channel), with where. */
function pixelMismatches(px: { w: number; h: number; data: number[] }, want: ReturnType<typeof expected>, cols = want.crop ?? want.cols): string[] {
  const out: string[] = [];
  if (px.w !== cols || px.h !== want.bands) return [`image ${px.w} x ${px.h}, the JSON's map ${cols} x ${want.bands}`];
  for (let b = 0; b < want.bands; b++) {
    const y = want.bands - 1 - b;
    for (let c = 0; c < cols; c++) {
      const i = (y * cols + c) * 4;
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
    // The pickers (`<select>`) leave the stray text as the tab's scan has them leave it
    // (acousticsTab.ts `scan`); their options are returned and checked on their own below.
    clone?.querySelectorAll('[data-num], [data-str], [data-label], select').forEach((e) => e.remove());
    const options = (part: string) => all(`select[data-part="${part}"] option`).map((o) => ({ value: (o as HTMLOptionElement).value, text: o.textContent ?? '' }));
    return {
      receiverOptions: options('response-receiver'),
      sourceOptions: options('response-source'),
      stripOptions: options('response-strip-band'),
      bandLabels: all('[data-part="response-bands"] > *').map((e) => e.textContent ?? ''),
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
    const canvases = () => browser.execute(() => document.querySelectorAll('canvas').length);
    const before = await canvases();
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
    // M10 PLAN rule 3: the window adds no canvas (it holds none; resp-pixels checks inside it).
    const after = await canvases();
    assert.equal(after, before, `canvases in the document: ${before} before the window, ${after} with it open`);
    console.log(`resp-window receipt: run ${run?.run}; opened, closed and opened again; ${JSON.stringify(w.box)} in ${w.view.w} x ${w.view.h}; canvases ${before} before, ${after} open`);
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
    want.db.forEach((row, b) => row.slice(0, px.w).forEach((d, c) => (d <= -SPAN ? down.push({ b, c, d }) : null)));
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
    assert.equal(v1.crop, want1.crop, "R2's crop");
    await pick('receiver', '0');
    console.log(
      `resp-pixels receipt: ${want.cols} steps x ${want.bands} bands from step ${want.k0}; ${bad.length} mismatched pixels (R1), ${bad1.length} (R2); loudest bin band ${top.b} step ${top.c} drawn ${hot.join(',')}; ${down.length} bins >= 60 dB down all black (${near.length} within 1 dB of the edge); control ${other.length} pixels differ from R2's map`,
    );
  });

  it('resp-crop: the map ends after the last bin above the floor; "Full run" shows every step', async () => {
    const want = expected(json, 0);
    const shownAs = () => browser.execute(() => {
      const el = document.querySelector<HTMLElement>('[data-response-window] [data-part="response-shown"]');
      const b = document.querySelector<HTMLElement>('[data-response-window] [data-action="response-full"]');
      const fit = document.querySelector<HTMLButtonElement>('[data-response-window] [data-action="response-fit"]');
      return { shown: el?.getAttribute('data-shown') ?? '', text: el?.innerText ?? '', button: b ? b.getAttribute('aria-pressed') : null, fit: fit ? { pressed: fit.getAttribute('aria-pressed'), disabled: fit.disabled } : null };
    });
    const v = await hook<HookView>('responseView');
    assert.equal(v.crop, want.crop, 'the crop, against the JSON');
    const s0 = await shownAs();
    if (want.crop === null) {
      // The full run, with nothing to cut. Re-pinned 2026-10-07 (C3): since 7e44f63 (Burhan
      // 2026-10-06 18:20, the window "legitimately manipulated and controlled") the time range is
      // always under the user's hand, so "Full run" is always offered: it is the preset that also
      // undoes a zoom. With nothing cut it reads pressed (the full run is what is shown), and
      // "Fit energy", the crop's preset, is offered disabled. The old pin was "no toggle at all".
      assert.equal(v.shown, want.cols);
      assert.equal(v.full, true);
      assert.equal(s0.shown, 'full');
      assert.equal(s0.button, 'true', '"Full run" with nothing cut: shown pressed');
      assert.deepEqual(s0.fit, { pressed: 'false', disabled: true }, '"Fit energy" with nothing to cut');
      assert.match(s0.text, /^Shown: the full run/);
      console.log(`resp-crop receipt: run ${run?.run}: no crop (${want.cols} steps), the full run shown, "Full run" pressed, "Fit energy" disabled`);
      return;
    }
    // Every bin past the end is 60 dB down or more: nothing above the floor is cut off.
    const hidden = want.db.flatMap((row, b) => row.slice(want.crop as number).flatMap((d, c) => (d > -SPAN ? [`band ${b} step ${c + (want.crop as number)}: ${d.toFixed(1)} dB`] : [])));
    assert.deepEqual(hidden, [], 'bins above the floor past the end');
    assert.equal(v.shown, want.crop);
    assert.equal(v.full, false);
    assert.equal(s0.shown, 'crop');
    assert.equal(s0.button, 'false');
    assert.match(s0.text, /^Shown: to [\d.]+ s of the [\d.]+ s run/);
    const cut = await pixels();
    assert.ok(cut);
    assert.equal(cut.w, want.crop);
    // Control: the cut map against the full run's width is caught.
    assert.ok(pixelMismatches(cut, want, want.cols).length > 0, 'the cut map read as the full run');
    await clickSelector(`${WIN} [data-action="response-full"]`);
    await browser.waitUntil(async () => (await hook<HookView>('responseView')).full, { timeout: 10_000, timeoutMsg: '"Full run" did not take' });
    await m10.idle();
    const vf = await hook<HookView>('responseView');
    assert.equal(vf.shown, want.cols);
    const sf = await shownAs();
    assert.equal(sf.shown, 'full');
    assert.equal(sf.button, 'true');
    assert.match(sf.text, /^Shown: the full run, [\d.]+ s/);
    const whole = await pixels();
    assert.ok(whole);
    const badFull = pixelMismatches(whole, want, want.cols);
    assert.deepEqual(badFull.slice(0, 10), [], `full run: ${badFull.length} pixels`);
    await clickSelector(`${WIN} [data-action="response-full"]`);
    await browser.waitUntil(async () => !(await hook<HookView>('responseView')).full, { timeout: 10_000, timeoutMsg: 'the second click did not cut the map again' });
    await m10.idle();
    assert.equal((await hook<HookView>('responseView')).shown, want.crop);
    console.log(`resp-crop receipt: run ${run?.run}: ${want.crop} of ${want.cols} steps shown ("${s0.text}"); 0 bins above -60 dB past the end; full run ${whole.w} steps, ${badFull.length} mismatched ("${sf.text}"); cut again`);
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
    // Words that are not the report's (narrowed 2026-10-07, C3, for 7e44f63's controls): the colour
    // bar's span words; the span chips, the window's own choices; the readout, derived from the map
    // under the pointer, which is off the map here, so it holds no digit at all.
    for (const l of s.labels) {
      if (l.kind === 'span') assert.ok(SPAN_WORDS.includes(l.text), `a span word not known: ${JSON.stringify(l.text)}`);
      else if (l.kind === 'control') assert.ok(SPAN_CHOICES.includes(l.text), `a span choice not offered: ${JSON.stringify(l.text)}`);
      else if (l.kind === 'readout') assert.equal(strayDigits(l.text), null, `a digit in the readout with the pointer off the map: ${strayDigits(l.text)}`);
      else assert.fail(`a label of kind ${l.kind}: ${JSON.stringify(l.text)}`);
    }
    assert.deepEqual(s.labels.filter((l) => l.kind === 'control').map((l) => l.text), SPAN_CHOICES, 'the span chips');
    // The pickers' options, on their own: the receivers are the JSON's labels in order; the strip's
    // bands are the band labels the map prints (each a checked [data-num]) after "bands summed"; a
    // source is a name of the JSON's echograms per source.
    const labelsJson = (at(json, 'spps.point_receivers') as { label: string }[]).map((x) => x.label);
    assert.deepEqual(s.receiverOptions.map((o) => o.text), labelsJson, 'the receiver picker');
    assert.deepEqual(s.stripOptions.map((o) => o.text), ['bands summed', ...s.bandLabels], 'the strip picker');
    const perSource = new Set((at(json, 'spps.point_receivers') as { per_source?: { source: string }[] }[]).flatMap((x) => (x.per_source ?? []).map((p) => p.source)));
    for (const o of s.sourceOptions) assert.ok(o.value === '' || (perSource.has(o.text) && o.value === o.text), `a source option not the JSON's: ${JSON.stringify(o)}`);
    // Control: the receiver picker against the JSON's labels with one changed is caught.
    const wrongOption = labelsJson.map((t, i) => (i === 0 ? `${t}0` : t));
    assert.notDeepEqual(s.receiverOptions.map((o) => o.text), wrongOption, 'the control: a changed receiver label');
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
