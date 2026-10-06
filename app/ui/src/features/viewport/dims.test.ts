import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { dimensionLines, dimText, majorStep } from './dims.ts';
import type { Vec } from './geometry.ts';

// Elmia's extents from its model check (41.45 x 29.71 x 16.10 m).
const BOX = { min: [0, 0, 0] as Vec, max: [41.45, 29.71, 16.1] as Vec };

test('the labels read as the Room model panel writes its Dimensions', () => {
  const { lines } = dimensionLines(BOX, [-50, -50, 30]);
  assert.deepEqual(
    lines.map((l) => l.text),
    ['Length 41.45 m', 'Width 29.71 m', 'Height 16.10 m'],
  );
  assert.equal(dimText('height', 3), 'Height 3.00 m');
});

test('the lines sit outside the box on the sides facing the camera', () => {
  const near = dimensionLines(BOX, [-50, -50, 30]).lines;
  const far = dimensionLines(BOX, [100, 100, 30]).lines;
  assert.ok(near[0].mid[1] < 0 && far[0].mid[1] > 29.71, 'length on the near y side');
  assert.ok(near[1].mid[0] < 0 && far[1].mid[0] > 41.45, 'width on the near x side');
  // Height beside the far end of the width line, outside the box in x.
  assert.ok(near[2].mid[0] < 0 && near[2].mid[1] === 29.71, `${near[2].mid}`);
});

test('a ruler on each line: a tick every metre from the room edge, a number every 5 m', () => {
  const { marks, segments } = dimensionLines(BOX, [-50, -50, 30]);
  // 42 ticks along 41.45 m (0 to 41), 30 along 29.71 m, 17 up 16.10 m, after the 9 line segments.
  assert.equal(segments.length, (9 + 42 + 30 + 17) * 6);
  assert.deepEqual(
    marks.map((m) => m.text),
    ['0', '5', '10', '15', '20', '25', '30', '35', '40', '0', '5', '10', '15', '20', '25', '0', '5', '10', '15'],
  );
  // Tick k on the length line sits k metres from the room's edge.
  const tick7 = segments.slice((9 + 7) * 6, (9 + 7) * 6 + 3);
  assert.ok(Math.abs(tick7[0] - 7) < 1e-9, `${tick7}`);
  assert.equal(majorStep(100), 10);
  assert.equal(majorStep(400), 25);
});

test('each measuring line spans exactly the box, with an extension line to each end', () => {
  const { segments } = dimensionLines(BOX, [-50, -50, 30]);
  const seg = (i: number) => segments.slice(6 * i, 6 * i + 6);
  const len = (s: number[]) => Math.hypot(s[3] - s[0], s[4] - s[1], s[5] - s[2]);
  assert.ok(Math.abs(len(seg(0)) - 41.45) < 1e-9);
  assert.ok(Math.abs(len(seg(3)) - 29.71) < 1e-9);
  assert.ok(Math.abs(len(seg(6)) - 16.1) < 1e-9);
});
