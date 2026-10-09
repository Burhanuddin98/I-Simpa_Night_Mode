// R72: the chart axes typed by hand (chartAxes.ts).
import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { handAxes, MAX_TICKS, stepTicks } from './chartAxes.ts';

test('R72: both ends or neither, from below to, a decimal comma read as a point', () => {
  assert.deepEqual(handAxes({}), { axes: {}, error: null });
  assert.deepEqual(handAxes({ yFrom: '0', yTo: '2,5' }).axes, { y: [0, 2.5] });
  assert.deepEqual(handAxes({ xFrom: '−0.1', xTo: '1' }).axes, { x: [-0.1, 1] });
  assert.match(handAxes({ yFrom: '1' }).error ?? '', /both ends/);
  assert.match(handAxes({ yFrom: '2', yTo: '1' }).error ?? '', /below/);
  assert.match(handAxes({ yFrom: '1', yTo: '1' }).error ?? '', /below/);
  assert.match(handAxes({ yFrom: '1e3', yTo: '2' }).error ?? '', /number/);
  assert.match(handAxes({ xFrom: 'a', xTo: '2' }).error ?? '', /number/);
});

test('R72: a tick spacing above 0, and never more than MAX_TICKS ticks over the axis', () => {
  assert.deepEqual(handAxes({ yFrom: '0', yTo: '2', yStep: '0.5' }).axes, { y: [0, 2], yStep: 0.5 });
  assert.match(handAxes({ yStep: '0' }).error ?? '', /above 0/);
  assert.match(handAxes({ yFrom: '0', yTo: '100', yStep: '1' }).error ?? '', /at most/);
  // Over the chart's own range when no range is typed.
  assert.match(handAxes({ yStep: '0.01' }, [0, 2]).error ?? '', /at most/);
  assert.equal(handAxes({ yStep: '0.1' }, [0, 2]).error, null);
});

test('R72: ticks at whole multiples of the spacing, inside the range, read at the spacing', () => {
  assert.deepEqual(stepTicks(0, 1, 0.25), [0, 0.25, 0.5, 0.75, 1]);
  assert.deepEqual(stepTicks(0.05, 0.95, 0.1), [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9]);
  assert.deepEqual(stepTicks(-62, -38, 10), [-60, -50, -40]);
  assert.ok(stepTicks(0, 1e6, 1).length <= MAX_TICKS + 1);
  assert.deepEqual(stepTicks(1, 0, 1), []);
  assert.deepEqual(stepTicks(0, 1, 0), []);
});
