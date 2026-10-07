// Wow list W9 (parity R56, R63; docs/investigations/2026-10-04-wow-w2w3w9/PLAN.md): export, and the
// bottom dock's layout (W5's overlap fixed). Each id with its control:
//   w9-csv     File › Export parameters as CSV (the hook passes the path the dialog would): every
//              number in the file equals `simpa results <run> --json` at the row's path, read by
//              this spec's own CSV reader; a refused row carries the report's code and no number;
//              every row's parameter is PASS in the report's bed. Control: one digit changed in a
//              copy of the text is caught by the same comparison
//   w9-json    the same for the JSON export, and its wording is the tab's (MQ2)
//   w9-png     Export view as PNG: the file decodes (this spec's own PNG reader), is the frame's
//              size plus the legend strip, and its pixels at sampled points equal the frame the
//              GPU holds over the window's background (the page's --bg, which <body> paints), the
//              frame settled and unchanged between the two read-backs; the strip's texts are the on-screen
//              legend's. Controls: some sampled pixels are not the background (the map is in the
//              frame), and the strip differs from the frame. The PNG is written to $M11_SCREENS
//              as w9-export.png
//   w9-refuse  the core refuses a `.exe` path and a relative path for an export, and writes
//              nothing; off the Results step the parameter items are disabled and say why
//   w9-layout  with the map's legend, the timeline, the "no particles" notice and the probe all
//              shown, and the map panel at its tallest the box offers (cumulative and fixed range
//              on), no two of them, the map panel, the plan inset, the tools and the gizmo overlap.
//              Screenshot w9-layout.png
//
// The box: tests/fixtures/rooms/outputs_box.simpa, copied into this spec's own folder on C:
// (M11_P), run once from the app with particles saved 0. The exports land in that folder.
import { strict as assert } from 'node:assert';
import { copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { inflateSync } from 'node:zlib';
import { cliReport } from '../lib/acousticsTab.ts';
import { clickSelector, RESULTS_STEP_CURRENT } from '../lib/dom.ts';
import { hook, m10, waitForHooks } from '../lib/hooks.ts';
import { env } from '../lib/types.ts';

const HOOKS = ['idle', 'openProject', 'edit', 'projectJson', 'runStart', 'runState', 'runsRows', 'selectRun', 'setStep', 'm12Map', 'm12SetStep', 'wowExport', 'wowLastExport', 'wowFrameSamples', 'wowFramePixels', 'wowCardRects', 'wowMapFacePoint'];
const MQ2 =
  'Computed to ISO 3382-1 / IEC 60268-16 and checked against exact solutions to within the just-noticeable difference. Simulated with I-Simpa’s solvers; not compared with measured rooms.';

interface Done {
  kind: string;
  path: string;
  bytes: number;
  rows?: number;
  width?: number;
  height?: number;
  strip?: { heading: string; title: string; lo: string; mid: string; hi: string; note: string | null } | null;
}
type Result = { done: Done | null; error: { code: string; message: string } | null };

const shown = (s: string | null) => (s ?? '').replace(/\s+/g, ' ').trim();

/** The value at a dot path. */
function at(root: unknown, p: string): unknown {
  let v: unknown = root;
  for (const k of p.split('.')) {
    if (v === null || typeof v !== 'object') return undefined;
    v = Array.isArray(v) ? v[Number(k)] : (v as Record<string, unknown>)[k];
  }
  return v;
}

/** RFC 4180, this spec's own reader. */
function readCsv(s: string): Record<string, string>[] {
  const rows: string[][] = [];
  let row: string[] = [];
  let f = '';
  let q = false;
  for (let i = 0; i < s.length; i++) {
    const c = s[i];
    if (q) {
      if (c === '"' && s[i + 1] === '"') (f += '"'), i++;
      else if (c === '"') q = false;
      else f += c;
    } else if (c === '"') q = true;
    else if (c === ',') row.push(f), (f = '');
    else if (c === '\r' && s[i + 1] === '\n') row.push(f), rows.push(row), (row = []), (f = ''), i++;
    else f += c;
  }
  const [head, ...body] = rows;
  return body.map((r) => Object.fromEntries(head.map((h, i) => [h, r[i]])));
}

interface Row {
  parameter: string;
  status: string;
  value: number | string | null;
  lo: number | string | null;
  hi: number | string | null;
  refusal: string | null;
  path: string;
}

/** Every number of the rows against the report at its path; the mismatches. */
function compare(rows: Row[], json: Record<string, unknown>): { mismatches: string[]; numbers: number; refused: number } {
  const out: string[] = [];
  let numbers = 0;
  let refused = 0;
  const bed = (json.bed as { parameters: Record<string, { status: string }> }).parameters;
  for (const r of rows) {
    if (bed[r.parameter]?.status !== 'PASS') out.push(`${r.path}: ${r.parameter} is not PASS in the bed`);
    for (const k of ['value', 'lo', 'hi'] as const) {
      const t = r[k];
      if (t === null || t === '') continue;
      const want = at(json, `${r.path}.${k}`);
      numbers++;
      if (typeof want !== 'number' || Number(t) !== want) out.push(`${r.path}.${k}: file ${t}, report ${String(want)}`);
    }
    if (r.status === 'refused') {
      refused++;
      if (r.refusal !== at(json, `${r.path}.not_evaluable.code`)) out.push(`${r.path}: refusal ${r.refusal}`);
      if (![r.value, r.lo, r.hi].every((x) => x === null || x === '')) out.push(`${r.path}: a refused row with a number`);
    }
  }
  return { mismatches: out, numbers, refused };
}

/** A PNG's RGBA (8-bit, colour type 2 or 6, no interlace), this spec's own reader. */
function readPng(file: string): { width: number; height: number; rgba: Uint8Array } {
  const b = readFileSync(file);
  assert.deepEqual([...b.subarray(0, 8)], [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a], 'PNG signature');
  let o = 8;
  let width = 0;
  let height = 0;
  let type = 0;
  const idat: Buffer[] = [];
  while (o < b.length) {
    const len = b.readUInt32BE(o);
    const kind = b.toString('latin1', o + 4, o + 8);
    const data = b.subarray(o + 8, o + 8 + len);
    if (kind === 'IHDR') {
      width = data.readUInt32BE(0);
      height = data.readUInt32BE(4);
      assert.equal(data[8], 8, 'bit depth 8');
      type = data[9];
      assert.ok(type === 2 || type === 6, `colour type ${type}`);
      assert.equal(data[12], 0, 'not interlaced');
    } else if (kind === 'IDAT') idat.push(data);
    o += 12 + len;
  }
  const raw = inflateSync(Buffer.concat(idat));
  const bpp = type === 6 ? 4 : 3;
  const stride = width * bpp;
  const px = new Uint8Array(height * stride);
  for (let y = 0; y < height; y++) {
    const filter = raw[y * (stride + 1)];
    const line = raw.subarray(y * (stride + 1) + 1, (y + 1) * (stride + 1));
    for (let x = 0; x < stride; x++) {
      const a = x >= bpp ? px[y * stride + x - bpp] : 0;
      const up = y > 0 ? px[(y - 1) * stride + x] : 0;
      const ul = y > 0 && x >= bpp ? px[(y - 1) * stride + x - bpp] : 0;
      let v = line[x];
      if (filter === 1) v += a;
      else if (filter === 2) v += up;
      else if (filter === 3) v += (a + up) >> 1;
      else if (filter === 4) {
        const p = a + up - ul;
        const pa = Math.abs(p - a);
        const pb = Math.abs(p - up);
        const pc = Math.abs(p - ul);
        v += pa <= pb && pa <= pc ? a : pb <= pc ? up : ul;
      }
      px[y * stride + x] = v & 255;
    }
  }
  const rgba = new Uint8Array(width * height * 4);
  for (let i = 0; i < width * height; i++) {
    rgba.set(px.subarray(i * bpp, i * bpp + 3), 4 * i);
    rgba[4 * i + 3] = bpp === 4 ? px[i * bpp + 3] : 255;
  }
  return { width, height, rgba };
}

async function hover(p: { x: number; y: number }): Promise<void> {
  await browser.action('pointer', { parameters: { pointerType: 'mouse' } }).move({ x: p.x, y: p.y, origin: 'viewport', duration: 0 }).perform();
}

describe('W9: export, and the bottom dock', () => {
  let dir = '';
  let box = '';
  let run = '';
  let json: Record<string, unknown> = {};

  before(async () => {
    await waitForHooks(HOOKS);
    dir = path.join(env('M11_P'), 'm12-export');
    mkdirSync(dir, { recursive: true });
    mkdirSync(env('M11_SCREENS'), { recursive: true });
    box = path.join(dir, 'outputs_box.simpa');
    copyFileSync(path.join(env('M11_REPO'), 'tests', 'fixtures', 'rooms', 'outputs_box.simpa'), box);
    await m10.openProject(box);
    const p = JSON.parse(await m10.projectJson()) as { solvers: { spps: Record<string, unknown> } };
    p.solvers.spps.particles_saved = 0;
    assert.equal((await m10.edit({ op: 'set_solver_settings', settings: p.solvers })).applied, true);
    const before = new Set((await hook<{ run: string }[]>('runsRows')).map((r) => r.run));
    await hook('runStart', 'spps');
    let fresh: { run: string; status: string } | undefined;
    await browser.waitUntil(
      async () => {
        if ((await hook<unknown>('runState')) !== null) return false;
        fresh = (await hook<{ run: string; status: string }[]>('runsRows')).find((r) => !before.has(r.run) && r.status !== 'RUNNING');
        return fresh !== undefined;
      },
      { timeout: 300_000, interval: 250, timeoutMsg: 'the run did not end within 300 s' },
    );
    await m10.idle();
    const f = fresh as unknown as { run: string; status: string };
    assert.equal(f.status, 'OK');
    run = f.run;
    json = cliReport(box, run);
    await hook('selectRun', run);
    await m10.setStep('results');
    await $(RESULTS_STEP_CURRENT).waitForExist({ timeout: 30_000 });
    console.log(`receipt w9: run ${run} under ${dir}`);
  });

  it('w9-csv: every number in the exported CSV equals simpa results --json at its path', async () => {
    const target = path.join(dir, 'parameters.csv');
    const r = await hook<Result>('wowExport', 'csv', target);
    assert.equal(r.error, null, JSON.stringify(r.error));
    const text = readFileSync(target, 'utf8');
    assert.equal(r.done?.bytes, Buffer.byteLength(text, 'utf8'));
    const rows = readCsv(text) as unknown as Row[];
    assert.equal(rows.length, r.done?.rows);
    const c = compare(rows, json);
    console.log(`receipt w9-csv: ${rows.length} rows, ${c.numbers} numbers, ${c.refused} refused, mismatches ${c.mismatches.length} (${target})`);
    assert.deepEqual(c.mismatches, []);
    // The box's 2,000 particles leave most parameters refused (range not reached): both kinds are checked.
    assert.ok(c.numbers >= 3 && c.refused >= 1, `${c.numbers} numbers, ${c.refused} refused`);
    const labels = (json.spps as { point_receivers: { label: string }[] }).point_receivers.map((x) => x.label);
    assert.deepEqual([...new Set(rows.map((x) => (x as unknown as Record<string, string>).receiver))], labels);
    // Control: one digit changed is caught by the same comparison.
    const victim = rows.find((x) => typeof x.value === 'string' && /\d/.test(x.value)) as Row;
    const planted = rows.map((x) => (x === victim ? { ...x, value: String(x.value).replace(/(\d)(?!.*\d)/, (d) => String((Number(d) + 1) % 10)) } : x));
    assert.equal(compare(planted, json).mismatches.length, 1, 'the control: a changed digit is caught');
  });

  it('w9-json: every number in the exported JSON equals the report at its path; the words are the tab\'s', async () => {
    const target = path.join(dir, 'parameters.json');
    const r = await hook<Result>('wowExport', 'json', target);
    assert.equal(r.error, null, JSON.stringify(r.error));
    const j = JSON.parse(readFileSync(target, 'utf8')) as { run: string; bands_hz: number[]; wording: string; rows: Row[] };
    assert.equal(j.run, run);
    assert.deepEqual(j.bands_hz, json.bands_hz);
    assert.equal(j.wording, MQ2);
    const c = compare(j.rows, json);
    console.log(`receipt w9-json: ${j.rows.length} rows, ${c.numbers} numbers, mismatches ${c.mismatches.length}`);
    assert.deepEqual(c.mismatches, []);
    assert.ok(c.numbers >= 3 && c.refused >= 1, `${c.numbers} numbers, ${c.refused} refused`);
    const victim = j.rows.find((x) => typeof x.value === 'number') as Row;
    assert.equal(compare(j.rows.map((x) => (x === victim ? { ...x, value: (x.value as number) * (1 + 1e-12) } : x)), json).mismatches.length, 1, 'the control');
  });

  it('w9-png: the PNG is the frame as drawn over the background, with the legend\'s own texts in a strip', async () => {
    await mapOf(run);
    await hook('m12SetStep', 20);
    // The frame settles first: after a step change the view eases in for a moment (two read-backs
    // 9,045 of 36,000 pixels apart right after the step, 0 three seconds later; C3 2026-10-07), and
    // the export and the samples below are two read-backs, so both must be of the same frame.
    const frameHash = async () => (await hook<{ hash: number } | null>('wowFramePixels'))?.hash ?? null;
    let settled: number | null = null;
    await browser.waitUntil(
      async () => {
        const h = await frameHash();
        const same = h !== null && h === settled;
        settled = h;
        return same;
      },
      { timeout: 20_000, interval: 400, timeoutMsg: 'the frame did not settle' },
    );
    const target = path.join(env('M11_SCREENS'), 'w9-export.png');
    const r = await hook<Result>('wowExport', 'png', target);
    assert.equal(r.error, null, JSON.stringify(r.error));
    const d = r.done as Done;
    const png = readPng(target);
    assert.equal(png.width, d.width);
    assert.equal(png.height, (d.height as number) + 64, 'the frame and the 64 px strip');
    const w = d.width as number;
    const h = d.height as number;
    const points: [number, number][] = [];
    for (let y = 5; y < h; y += Math.max(1, Math.floor(h / 14))) for (let x = 5; x < w; x += Math.max(1, Math.floor(w / 14))) points.push([x, y]);
    const s = await hook<{ width: number; height: number; rgba: number[][] }>('wowFrameSamples', points);
    assert.deepEqual([s.width, s.height], [w, h]);
    assert.equal(await frameHash(), settled, 'the frame changed between the export and the samples');
    // The window's background, as the page paints it (the theme's --bg on <body>), read from the
    // page: re-pinned 2026-10-07 (C3) from a hard-coded [9, 9, 11], the --bg #09090b that ff820a9
    // (decision 64, 10-06 05:27) made #0c0a0a; 170 of 210 samples differed by that alone (at most
    // 3 levels), not by the export.
    const bgCss = await browser.execute(() => ({ token: getComputedStyle(document.documentElement).getPropertyValue('--bg').trim(), body: getComputedStyle(document.body).backgroundColor }));
    const m = /^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(bgCss.token);
    assert.ok(m, `--bg: ${bgCss.token}`);
    const bg = [1, 2, 3].map((k) => parseInt(m[k], 16));
    assert.equal(bgCss.body, `rgb(${bg.join(', ')})`, 'the window background is --bg');
    let bad = 0;
    let drawn = 0;
    points.forEach(([x, y], i) => {
      const [r0, g0, b0, a] = s.rgba[i];
      const want = [r0, g0, b0].map((c, k) => Math.min(255, c + Math.round((bg[k] * (255 - a)) / 255)));
      const got = [...png.rgba.subarray(4 * (y * w + x), 4 * (y * w + x) + 3)];
      if (got.some((v, k) => v !== want[k])) bad++;
      if (want.some((v, k) => v !== bg[k])) drawn++;
    });
    console.log(`receipt w9-png: ${png.width} x ${png.height} (${d.bytes} B), ${points.length} sampled pixels, ${bad} differ from the frame, ${drawn} not the background; strip ${JSON.stringify(d.strip)}`);
    assert.equal(bad, 0);
    assert.ok(drawn > 0, 'the control: the frame is not empty');
    const legend = {
      title: shown(await $('[data-part="map-legend"] .vp-legend-title').getText()),
      lo: shown(await $('[data-part="map-legend"] [data-legend="lo"]').getText()),
      mid: shown(await $('[data-part="map-legend"] [data-legend="mid"]').getText()),
      hi: shown(await $('[data-part="map-legend"] [data-legend="hi"]').getText()),
    };
    assert.deepEqual({ title: d.strip?.title, lo: d.strip?.lo, mid: d.strip?.mid, hi: d.strip?.hi }, legend);
    assert.ok(d.strip?.heading.endsWith(shown(await $('[data-part="anim-time"]').getText())), d.strip?.heading);
    // Control: the strip is not a copy of the frame's last rows.
    const stripRow = png.rgba.subarray(4 * (h + 30) * w, 4 * (h + 31) * w);
    const frameRow = png.rgba.subarray(4 * (h - 1) * w, 4 * h * w);
    assert.notDeepEqual([...stripRow], [...frameRow]);
  });

  it('w9-refuse: the core writes no .exe and no relative path; off the Results step the parameters are not offered', async () => {
    const before = readdirSync(dir).sort();
    const exe = path.join(dir, 'view.exe');
    const a = await hook<Result>('wowExport', 'png', exe);
    console.log(`receipt w9-refuse: ${exe}: ${JSON.stringify(a.error)}`);
    assert.equal(a.error?.code, 'EXPORT_PATH');
    assert.match(a.error?.message ?? '', /does not end in \.png/);
    assert.equal(existsSync(exe), false);
    const b = await hook<Result>('wowExport', 'csv', 'parameters.csv');
    assert.equal(b.error?.code, 'EXPORT_PATH');
    assert.deepEqual(readdirSync(dir).sort(), before, 'nothing written');
    await m10.setStep('geometry');
    await clickSelector('[data-menu="File"]');
    const item = await $('[data-menu-item="export-csv"]');
    await item.waitForExist({ timeout: 5_000 });
    assert.equal(await item.getAttribute('disabled'), 'true');
    assert.equal(await item.getAttribute('title'), 'Parameters are exported from the Results step');
    assert.equal(await $('[data-menu-item="export-view"]').getAttribute('disabled'), null, 'the view is exported from any step');
    await browser.keys('Escape');
    const c = await hook<Result>('wowExport', 'csv', path.join(dir, 'off-step.csv'));
    assert.equal(c.error?.code, 'EXPORT_NOTHING');
    assert.equal(existsSync(path.join(dir, 'off-step.csv')), false);
    await m10.setStep('results');
    await $(RESULTS_STEP_CURRENT).waitForExist({ timeout: 30_000 });
  });

  it('w9-layout: the legend, the timeline, the notice and the probe in the dock; no two cards overlap', async () => {
    await mapOf(run);
    await hook('m12SetStep', 20);
    await $('[data-part="particles-none"]').waitForExist({ timeout: 30_000 });
    await clickSelector('[data-part="map-cumulative"]');
    await clickSelector('[data-part="map-smooth"]');
    await clickSelector('[data-part="map-fixed"]');
    await browser.waitUntil(async () => (await hook<{ cumulative: boolean } | null>('m12Map'))?.cumulative === true, { timeout: 30_000, timeoutMsg: 'cumulative did not come on' });
    await $('[data-part="map-range"]').waitForExist({ timeout: 10_000 });
    await $('[data-part="cumulative-note"]').waitForExist({ timeout: 10_000 });
    const m = await hook<{ faces: number }>('m12Map');
    let hovered = false;
    for (let face = 0; face < m.faces && !hovered; face++) {
      const p = await hook<{ x: number; y: number } | null>('wowMapFacePoint', face);
      if (!p) continue;
      await hover(p);
      hovered = await $('[data-part="map-probe"]')
        .waitForDisplayed({ timeout: 3_000 })
        .then(() => true)
        .catch(() => false);
    }
    assert.ok(hovered, 'the probe is shown');
    const rects = await hook<Record<string, { left: number; top: number; right: number; bottom: number }>>('wowCardRects');
    for (const k of ['map-panel', 'plan-inset', 'legend', 'timeline', 'particles-none', 'probe']) assert.ok(rects[k], `${k} is shown`);
    const names = Object.keys(rects);
    const overlaps: string[] = [];
    for (let i = 0; i < names.length; i++) {
      for (let j = i + 1; j < names.length; j++) {
        const a = rects[names[i]];
        const b = rects[names[j]];
        if (a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom) overlaps.push(`${names[i]} and ${names[j]}`);
      }
    }
    console.log(`receipt w9-layout: ${names.join(', ')}; overlaps: ${overlaps.join('; ') || 'none'}`);
    await browser.saveScreenshot(path.join(env('M11_SCREENS'), 'w9-layout.png'));
    assert.deepEqual(overlaps, []);
  });

  async function mapOf(r: string): Promise<void> {
    await browser.waitUntil(async () => {
      const m = await hook<{ run: string; error: string | null } | null>('m12Map');
      return m !== null && m.run === r && m.error === null;
    }, { timeout: 60_000, interval: 250, timeoutMsg: 'no map shown' });
  }
});
