// PQ3's settings editor, its pure half (docs/investigations/2026-10-03-pq3/PLAN.md, order of
// work 3): parsing, the step count, the .pbin size, the band presets and the ops it sends.
import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import type { BandSet, Environment, SolverSettings } from '../../bindings/schema.ts';
import { opText, setBandComputed, setEnvironment, setSolverSettings } from '../../ops.ts';
import {
  BAND_PRESETS,
  bandPresetOf,
  moveDecimalPoint,
  parseCount,
  pbinBytes,
  secondsFromMs,
  sizeText,
  stepCount,
  stepCountText,
  timeStepInputText,
  withAir,
  withSpps,
} from './settings.ts';

const solvers = (): SolverSettings => ({
  spps: {
    particles_per_source: 150000,
    particles_saved: 0,
    duration_s: 10,
    time_step_s: 0.001,
    random_seed: 0,
    method: 'energetic',
    air_absorption: true,
    fittings: true,
    direct_field_only: false,
    transmission: true,
    extinction_exponent: 7,
    receiver_radius_m: 0.31,
    sound_map: 'intensity',
    sound_maps_per_band: true,
    echogram_per_source: true,
    save_surface_intersections: true,
    save_receiver_intersections: true,
    bands_computed: [true, true, true, true, true, true, true],
  },
  tcr: { air_absorption: true, bands_computed: [true, true, true, true, true, true, true] },
  meshing: { min_radius_edge_ratio: 2, max_volume_m3: null, surface_receiver_max_area_m2: null, preserve_boundary: false, preprocess: true },
});

test('parseCount takes whole numbers 0 to 2,147,483,647, the true number, never a guess', () => {
  assert.deepEqual(parseCount('150000'), { ok: true, value: 150000 });
  assert.deepEqual(parseCount(' 0 '), { ok: true, value: 0 });
  assert.deepEqual(parseCount('+12'), { ok: true, value: 12 });
  assert.deepEqual(parseCount('2147483647'), { ok: true, value: 2147483647 });
  for (const bad of ['', 'auto', '1.5', '1e5', '-1', '150,000', '0x10', '2147483648', '99999999999999999999']) {
    assert.equal(parseCount(bad).ok, false, bad);
  }
});

test('moveDecimalPoint shifts by string, never through a float', () => {
  assert.equal(moveDecimalPoint('0.0013', 3), '1.3');
  assert.equal(moveDecimalPoint('0.001', 3), '1');
  assert.equal(moveDecimalPoint('0.01', 3), '10');
  assert.equal(moveDecimalPoint('2', 3), '2000');
  assert.equal(moveDecimalPoint('1e-7', 3), '0.0001');
  assert.equal(moveDecimalPoint('1.5e-7', 3), '0.00015');
  assert.equal(moveDecimalPoint('1.3', -3), '0.0013');
  assert.equal(moveDecimalPoint('-0.5', 1), '-5');
  assert.equal(moveDecimalPoint('0', 3), '0');
});

test('a time step typed in ms is the seconds value its decimal names, bit for bit', () => {
  for (const [ms, s] of [
    ['1', 0.001],
    ['1.3', 0.0013],
    ['0.5', 0.0005],
    ['10', 0.01],
    ['3.5', 0.0035],
    ['2.5e-1', 0.00025],
  ] as const) {
    const got = secondsFromMs(ms);
    assert.ok(got.ok, ms);
    if (got.ok) assert.ok(Object.is(got.value, s), `${ms} ms -> ${got.value}, want ${s}`);
  }
  // The division this avoids: 2.1 ms / 1000 is not the double nearest 0.0021.
  assert.notEqual(2.1 / 1000, 0.0021);
  const two = secondsFromMs('2.1');
  assert.ok(two.ok && Object.is(two.value, 0.0021));
  assert.equal(secondsFromMs('abc').ok, false);
  assert.equal(secondsFromMs('NaN').ok, false);
  // Shown back as typed: stored seconds to ms by string.
  assert.equal(timeStepInputText(0.0013), '1.3');
  assert.equal(timeStepInputText(0.001), '1');
  assert.equal(timeStepInputText('NaN'), 'NaN');
  for (const s of [0.001, 0.0013, 0.0035, 0.01, 1e-7, 0.000123456789]) {
    const back = secondsFromMs(timeStepInputText(s));
    assert.ok(back.ok && Object.is(back.value, s), `${s} round-trips through the field`);
  }
});

test("the step count is the solver's: ceil of duration over step, in f64 and in f32, the larger", () => {
  assert.equal(stepCount(10, 0.001), 10000);
  assert.equal(stepCount(2, 0.01), 200);
  assert.equal(stepCount(1.7, 0.0013), 1308);
  assert.equal(stepCount(0, 0.001), null);
  assert.equal(stepCount(10, 0), null);
  assert.equal(stepCount('NaN', 0.001), null);
  assert.equal(stepCountText(10, 0.001), '10,000 steps');
  assert.equal(stepCountText(70, 0.001), '70,000 steps, above the 65,535 the solver can count');
  assert.equal(stepCountText(10, 0), '—');
});

test("the .pbin size is upstream's formula: saved x steps x 16 B x active sources, per band", () => {
  assert.equal(pbinBytes(1000, 10000, 1), 160_000_000n);
  assert.equal(pbinBytes(0, 10000, 3), 0n);
  assert.equal(pbinBytes(2147483647, 65535, 50), 2147483647n * 65535n * 16n * 50n);
  assert.equal(sizeText(160_000_000n), '160 MB');
  assert.equal(sizeText(1_120_000_000n), '1.1 GB');
  assert.equal(sizeText(999n), '999 B');
  assert.equal(sizeText(1_250n), '1.3 kB');
  assert.equal(sizeText(0n), '0 B');
});

test("the band presets are upstream's four and ours, and the current set is named when it is one", () => {
  assert.deepEqual(
    BAND_PRESETS.map((p) => [p.kind, p.lowest_hz, p.highest_hz]),
    [
      ['octave', 125, 8000],
      ['octave', 63, 16000],
      ['octave', 125, 4000],
      ['third_octave', 50, 20000],
      ['third_octave', 100, 5000],
    ],
  );
  const oct: BandSet = { kind: 'octave', frequencies_hz: [125, 250, 500, 1000, 2000, 4000, 8000] };
  assert.equal(bandPresetOf(oct)?.key, 'octave-125-8000');
  assert.equal(bandPresetOf({ kind: 'octave', frequencies_hz: [125, 250, 500, 1000, 2000, 4000] })?.key, 'octave-125-4000');
  assert.equal(bandPresetOf({ kind: 'octave', frequencies_hz: [125, 500] }), null, 'not a contiguous preset');
  assert.equal(bandPresetOf({ kind: 'third_octave', frequencies_hz: [100, 125] }), null);
});

test('the settings ops: the whole solver settings with one field changed, and the others as stored', () => {
  const s = solvers();
  const next = withSpps(s, { particles_per_source: 1000 });
  assert.equal(next.spps.particles_per_source, 1000);
  assert.deepEqual({ ...next.spps, particles_per_source: 150000 }, s.spps);
  assert.deepEqual(next.tcr, s.tcr);
  assert.equal(s.spps.particles_per_source, 150000, 'the input is not changed');
  const text = opText(setSolverSettings(next));
  assert.ok(text.startsWith('{"op":"set_solver_settings","settings":{"spps":{"particles_per_source":1000,'), text);
  assert.equal(opText(setBandComputed('tcr', 3, false)), '{"op":"set_band_computed","solver":"tcr","band":3,"computed":false}');
  const env: Environment = {
    temperature_c: 20,
    relative_humidity_percent: 50,
    pressure_pa: 101325,
    air_absorption: { kind: 'iso9613' },
    ground_roughness_m: 0.02,
    celerity_gradient_log: 0,
    celerity_gradient_lin: 0,
  };
  const air = withAir(env, { relative_humidity_percent: 61.5 });
  assert.equal(air.relative_humidity_percent, 61.5);
  assert.equal(air.temperature_c, 20);
  assert.equal(
    opText(setEnvironment(air)),
    '{"op":"set_environment","environment":{"temperature_c":20,"relative_humidity_percent":61.5,"pressure_pa":101325,"air_absorption":{"kind":"iso9613"},"ground_roughness_m":0.02,"celerity_gradient_log":0,"celerity_gradient_lin":0}}',
  );
});

// Backlog 80 (T6): every value an advisor Apply sets has a field, -Y among them.
import { withMeshing } from './settings.ts';

test('withMeshing replaces the meshing fields only, and travels as one set_solver_settings', () => {
  const s = solvers();
  const next = withMeshing(s, { preserve_boundary: true });
  assert.equal(next.meshing.preserve_boundary, true);
  assert.deepEqual({ ...next, meshing: s.meshing }, s);
  assert.deepEqual({ ...next.meshing, preserve_boundary: false }, s.meshing);
  assert.equal(s.meshing.preserve_boundary, false, 'the input is not changed');
  const text = opText(setSolverSettings(next));
  assert.match(text, /"preserve_boundary":true/);
  // The receiver radius and the extinction go through withSpps, as the other SPPS fields.
  const r = withSpps(s, { receiver_radius_m: 0.6, extinction_exponent: 7 });
  assert.deepEqual([r.spps.receiver_radius_m, r.spps.extinction_exponent], [0.6, 7]);
});
