import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { fitDepthDb, fromOklab, oklab, WARM, warmAt, warmLabels, warmLut, warmPlace, WARM_DEPTH_DB, WARM_FIT_MIN_DB, WARM_LUT_SIZE } from './warmRamp.ts';

const hex = (c: string): [number, number, number] => [1, 3, 5].map((i) => parseInt(c.slice(i, i + 2), 16) / 255) as [number, number, number];

test('decision 69: the warm stops are ember to white-hot, each lighter than the last (OKLab L)', () => {
  assert.deepEqual(WARM, ['#1A0505', '#5A0A12', '#E0202E', '#FF4A1C', '#FF8A1F', '#FFC23A', '#FFE680', '#FFF8EA']);
  const L = WARM.map((c) => oklab(hex(c))[0]);
  for (let i = 1; i < L.length; i++) assert.ok(L[i] > L[i - 1], `${WARM[i]} (L ${L[i].toFixed(3)}) is lighter than ${WARM[i - 1]} (L ${L[i - 1].toFixed(3)})`);
});

test('the ramp passes through every stop and its lightness never falls between them', () => {
  for (let i = 0; i < WARM.length; i++) {
    const c = warmAt(i / (WARM.length - 1));
    hex(WARM[i]).forEach((v, k) => assert.ok(Math.abs(c[k] - v) < 2 / 255, `${WARM[i]} channel ${k}: ${c[k]} vs ${v}`));
  }
  let last = -1;
  for (let i = 0; i <= 600; i++) {
    const L = oklab(warmAt(i / 600))[0];
    assert.ok(L >= last - 1e-9, `lightness falls at t = ${i / 600}`);
    last = L;
  }
});

test('OKLab round trip, the lookup texture and the place of an energy on the ramp', () => {
  for (const c of WARM) fromOklab(oklab(hex(c))).forEach((v, k) => assert.ok(Math.abs(v - hex(c)[k]) < 1e-6));
  const lut = warmLut();
  assert.equal(lut.length, 4 * WARM_LUT_SIZE);
  assert.deepEqual([...lut.subarray(0, 4)], [0x1a, 0x05, 0x05, 255]);
  assert.deepEqual([...lut.subarray(lut.length - 4)], [0xff, 0xf8, 0xea, 255]);
  assert.equal(warmPlace(1, 0), 1);
  assert.ok(Math.abs(warmPlace(1e-3, 0) - 0.5) < 1e-12, `${WARM_DEPTH_DB / 2} dB down is half way`);
  assert.equal(warmPlace(1e-7, 0), 0);
  assert.equal(warmPlace(0, 0), 0);
});

test('the ramp fitted to a band: loudest to the quietest 2 %, whole 3 dB, between the floor and 60 dB; the legend says it', () => {
  // 1000 records spread evenly over 0 to -30 dB re the loudest (logMax 0): the 2 % point is at about -29.4 dB.
  const e = Float32Array.from({ length: 1000 }, (_, i) => 10 ** (-(30 * i) / 999 / 10));
  assert.equal(fitDepthDb(e, 0), 30);
  assert.equal(fitDepthDb(Float32Array.from([1, 1, 1]), 0), WARM_FIT_MIN_DB, 'no spread: the floor');
  assert.equal(fitDepthDb(Float32Array.from({ length: 100 }, (_, i) => 10 ** (-i)), 0), WARM_DEPTH_DB, 'wider than 60 dB: 60');
  assert.equal(fitDepthDb(new Float32Array(0), 0), WARM_DEPTH_DB);
  assert.deepEqual(warmLabels(30), { lo: '−30', mid: '−15', hi: '0 dB' });
  assert.deepEqual(warmLabels(21), { lo: '−21', mid: '−10.5', hi: '0 dB' });
});
