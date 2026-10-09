import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { DEFAULT_MARKER_COLOR, hasOwnColor, markerColor, nameShown, rgb01, withDisplay } from './markerDisplay.ts';

const source = { id: 's', display: null as null | { color?: string | null; show_name?: boolean | null } };

test('G50: an element without a display takes the kind\'s colour and upstream\'s Show name', () => {
  assert.equal(markerColor(source, 'source'), DEFAULT_MARKER_COLOR.source);
  assert.equal(markerColor({}, 'receiver'), '#ededef');
  assert.equal(nameShown(source, 'source'), true);
  assert.equal(nameShown({}, 'receiver'), true);
  assert.equal(nameShown({}, 'zone'), false);
  assert.equal(hasOwnColor(source), false);
});

test('G50: a colour and a hidden name are kept; set back to the defaults, display is null again', () => {
  const a = withDisplay(source, 'source', { color: '#3FA7D6' });
  assert.deepEqual(a.display, { color: '#3fa7d6' });
  assert.equal(markerColor(a, 'source'), '#3fa7d6');
  const b = withDisplay(a, 'source', { show_name: false });
  assert.deepEqual(b.display, { color: '#3fa7d6', show_name: false });
  assert.equal(nameShown(b, 'source'), false);
  const c = withDisplay(b, 'source', { color: null });
  assert.deepEqual(c.display, { show_name: false });
  const d = withDisplay(c, 'source', { show_name: true });
  assert.equal(d.display, null);
  assert.equal(d.id, 's');
});

test('G50: a zone\'s name shown is its own setting, hidden its default', () => {
  const z = withDisplay({ display: null }, 'zone', { show_name: true });
  assert.deepEqual(z.display, { show_name: true });
  assert.equal(withDisplay(z, 'zone', { show_name: false }).display, null);
});

test('rgb01 reads #rrggbb', () => {
  assert.deepEqual(rgb01('#ff8000'), [1, 128 / 255, 0]);
});
