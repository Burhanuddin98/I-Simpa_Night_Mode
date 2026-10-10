// The RT chart's target (rtPlot.ts): drawn, with a visible line and shading, whenever there is one; and with
// none (CR4's 8657 m³ under DIN 18041 A3, whose formula stops at 5000 m³) no target series and a key that
// says why, never "target, shaded a fifth either side" over a chart without a line.
import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import type { DinView } from './model.ts';
import { rtKey, rtPlot, TARGET_BAND_FILL, TARGET_WORDS, type RtLine } from './rtPlot.ts';

const lines: RtLine[] = [
  { param: 't20_s', label: 'T20', values: [1.2, 1.1, null], hi: [1.3, 1.2, null] },
  { param: 't30_s', label: 'T30', values: [1.25, 1.15, 1.0], hi: [1.4, 1.3, 1.1] },
];
const ink = (p: string) => (p === 't20_s' ? '#3d8fd1' : '#e0202e');
const visible = (c: unknown) => typeof c === 'string' && c !== '' && c !== 'transparent' && !/rgba\([^)]*,\s*0(\.0*)?\s*\)$/.test(c);
const alpha = (c: string) => Number(/rgba\([^)]*,\s*([\d.]+)\s*\)/.exec(c)?.[1] ?? 1);

function dinA3(target: number | null): DinView {
  const s = (path: string, text: string) => ({ path, text });
  return {
    group: s('room.din18041.2.group', 'A3'),
    use: s('room.din18041.2.use', 'teaching, communication'),
    target: target === null ? null : { path: 'room.din18041.2.target_s.value', digits: 2, text: target.toFixed(2) },
    refusal: target === null ? { code: s('room.din18041.2.target_s.not_evaluable.code', 'params_din_out_of_range'), why: null } : null,
    range: target === null ? s('room.din18041.2.target_s.not_evaluable.error.detail', 'A3 gives targets from 30 to 5000 m³') : null,
    volume: { path: 'room.volume_m3', digits: 0, text: '8657' },
    note: null,
  };
}

test('RT chart: a target is drawn as a visible dashed line with its fifth either side shaded', () => {
  const t = 0.32 * Math.log10(4000) - 0.17;
  const p = rtPlot(3, lines, t, 1, ink, '#a1a1aa');
  const n = lines.length;
  // x, the two lines, then the target's three: the line, its lower and its upper edge.
  assert.equal(p.data.length, 1 + n + 3);
  assert.deepEqual(p.targetSeries, [n + 1, n + 2, n + 3]);
  assert.equal(p.series.length, p.data.length);
  assert.deepEqual(p.data[n + 1], [t, t, t]);
  assert.deepEqual(p.data[n + 2], [t * 0.8, t * 0.8, t * 0.8]);
  assert.deepEqual(p.data[n + 3], [t * 1.2, t * 1.2, t * 1.2]);
  const line = p.series[n + 1];
  assert.ok(visible(line.stroke), `the target line's stroke ${String(line.stroke)}`);
  assert.ok((line.width ?? 0) >= 1, 'the target line has a width');
  assert.ok(Array.isArray(line.dash) && line.dash.length === 2, 'the target line is dashed');
  // The shading: one band from the upper edge down to the lower, with a fill that shows.
  assert.equal(p.bands.length, 1);
  assert.deepEqual(p.bands[0].series, [n + 3, n + 2]);
  assert.equal(p.bands[0].fill, TARGET_BAND_FILL);
  assert.ok(visible(TARGET_BAND_FILL) && alpha(TARGET_BAND_FILL) > 0.05, `the shading ${TARGET_BAND_FILL}`);
  // The parameters keep their own series, in order, before the target's.
  assert.deepEqual(p.series.slice(1, n + 1).map((s) => s.label), ['T20', 'T30']);
  assert.equal(rtKey(t, 'A3', dinA3(t)), TARGET_WORDS);
  // At the large window's scale the line is wider and its dashes longer.
  const big = rtPlot(3, lines, t, 1.5, ink, '#a1a1aa');
  assert.ok((big.series[n + 1].width ?? 0) > (line.width ?? 0));
});

test('RT chart: with no target there are no target series, and the key says why instead of promising one', () => {
  for (const t of [null, Number.NaN, 0]) {
    const p = rtPlot(3, lines, t, 1, ink, '#a1a1aa');
    assert.equal(p.data.length, 1 + lines.length, String(t));
    assert.equal(p.series.length, 1 + lines.length);
    assert.deepEqual(p.targetSeries, []);
    assert.deepEqual(p.bands, []);
  }
  const key = rtKey(null, 'A3', dinA3(null));
  assert.notEqual(key, TARGET_WORDS);
  assert.ok(!/shaded/.test(key), key);
  assert.equal(key, 'no DIN 18041 A3 target, so none is drawn: A3 gives targets from 30 to 5000 m³, this room is 8657 m³');
  assert.equal(rtKey(null, 'A3', null), 'no target, so none is drawn: the room was not read');
});
