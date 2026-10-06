import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import type { SceneState } from '../../bindings/ipc.ts';
import type { ProjectSettings } from './model.ts';
import { blockersWithSize, BYTES_PER_RECORD, BYTES_PER_VALUE, CROSSINGS_PER_PARTICLE, cubeText, mapStepRatio, REFUSE_GB, RESULTS_TOO_BIG, resultCube, WARN_GB } from './runSize.ts';

// CR4-third as it crashed at 06:37 on 2026-10-06: Plane 1 over the whole room, 33.1 x 33.3 m at 0.1 m, 10 s at
// 1 ms, 18 bands with maps per band, 2 sources with an echogram each, 150,000 particles a source.
function scene(resolution: number, bands = 18, extra: Record<string, unknown> = {}): [SceneState, ProjectSettings] {
  const spps = {
    time_step_s: 0.001,
    duration_s: 10,
    particles_per_source: 150_000,
    bands_computed: Array.from({ length: bands }, () => true),
    sound_maps_per_band: true,
    echogram_per_source: true,
    ...extra,
  };
  const sc = {
    run_blockers: [],
    solver_issues: { spps: [], tcr: [] },
    groups: [{ id: 'g1', faces: 500 }],
    view: {
      sources: [{ enabled: true }, { enabled: true }],
      surface_receivers: [
        { id: 'p', enabled: true, shape: { kind: 'cutting_plane', a: [-7.528948, 16.56255, 0.35], b: [-7.528948, -16.56255, 0.35], c: [25.82, -16.56255, 0.35], resolution_m: resolution } },
      ],
    },
  } as unknown as SceneState;
  return [sc, { bands: { kind: 'third_octave', frequencies_hz: [] }, environment: {}, solvers: { spps, tcr: {} } } as unknown as ProjectSettings];
}

test('the whole-room plane at 0.1 m is sparse: the data bound, about 1.4 GB, and the run is no longer refused', () => {
  const [sc, st] = scene(0.1);
  const c = resultCube(sc, 'spps', st);
  assert.ok(c && c.cells === 110888, `${c?.cells}`);
  assert.equal(c!.steps, 10_000);
  assert.equal(c!.ratio, 1);
  assert.equal(c!.bins, 10_000);
  // Grid 1.1e9 cell-steps a band; data 150,000 x 2 x 32 = 9.6 million records a band: the data wins, sparse.
  assert.equal(c!.records, 150_000 * 2 * CROSSINGS_PER_PARTICLE);
  assert.equal(c!.sparse, true);
  assert.equal(c!.bytes, 150_000 * 2 * CROSSINGS_PER_PARTICLE * BYTES_PER_RECORD * 18);
  assert.equal(cubeText(c!), 'about 1.4 GB');
  assert.ok(c!.gb < WARN_GB);
  assert.deepEqual(blockersWithSize(sc, 'spps', st), []);
});

test('the old solver’s grid bound for the same run was 74 GB: that is what it allocated and why it aborted', () => {
  const [sc, st] = scene(0.1);
  const c = resultCube(sc, 'spps', st)!;
  const grid = c.cells * c.bins * BYTES_PER_VALUE * c.bands;
  assert.ok(grid / 2 ** 30 > 70 && grid / 2 ** 30 < 80, `${grid / 2 ** 30}`);
  assert.ok(c.bytes < grid / 40, 'the sparse forecast is under a fortieth of the grid');
});

test('a coarse plane with many particles goes dense: the grid bound, as upstream always paid', () => {
  // The whole-room plane at 0.5 m: 4,489 cells. 300,000 x 2 x 32 = 19.2 million records a band spread over
  // 4,489 cells is 4,277 steps a cell, over an eighth of 10,000, so the cells go dense and the cost is the grid.
  const [sc, st] = scene(0.5, 18, { particles_per_source: 300_000 });
  const c = resultCube(sc, 'spps', st)!;
  assert.equal(c.cells, 4489);
  assert.equal(c.sparse, false);
  assert.equal(c.bytes, 4489 * 10_000 * BYTES_PER_VALUE * 18);
  assert.equal(cubeText(c), 'about 3.0 GB');
  assert.ok(c.gb < WARN_GB);
});

test('enough particles on a fine plane still refuse: 3 million a source fill the cells past an eighth, and the grid is 74 GB', () => {
  const [sc, st] = scene(0.1, 18, { particles_per_source: 3_000_000 });
  const c = resultCube(sc, 'spps', st)!;
  // 192 million records a band over 110,888 cells is 1,731 steps a cell: dense, so the old grid bound is back.
  assert.equal(c.sparse, false);
  assert.ok(c.gb > REFUSE_GB, `${c.gb}`);
  assert.deepEqual(blockersWithSize(sc, 'spps', st), [RESULTS_TOO_BIG]);
  // Half of that stays sparse and under the refusal: 96 million records a band, about 14 GB, a warning.
  const [sc2, st2] = scene(0.1, 18, { particles_per_source: 1_500_000 });
  const c2 = resultCube(sc2, 'spps', st2)!;
  assert.equal(c2.sparse, true);
  assert.ok(c2.gb > WARN_GB && c2.gb < REFUSE_GB, `${c2.gb}`);
  assert.deepEqual(blockersWithSize(sc2, 'spps', st2), []);
});

test('a sound-map time step of 10 ms divides the grid bound by 10 and leaves the data bound alone', () => {
  const [sc, st] = scene(0.5, 18, { particles_per_source: 300_000, map_time_step_s: 0.01 });
  const c = resultCube(sc, 'spps', st)!;
  assert.equal(c.ratio, 10);
  assert.equal(c.bins, 1_000);
  // Grid 4.49 million cell-steps a band, now under the data bound, and 1,000 bins a cell is dense: 17 MB a band.
  assert.equal(c.sparse, false);
  assert.equal(c.bytes, 4489 * 1000 * BYTES_PER_VALUE * 18);
  assert.equal(cubeText(c), 'about 308 MB');
});

test('the ratio is the solver’s: whole steps, rounded, at least 1, and 1 for an unset or shorter sound-map step', () => {
  assert.equal(mapStepRatio(0.001, null), 1);
  assert.equal(mapStepRatio(0.001, 0.0005), 1, 'shorter than the time step: the solver keeps the step');
  assert.equal(mapStepRatio(0.001, 0.001), 1);
  assert.equal(mapStepRatio(0.001, 0.01), 10);
  assert.equal(mapStepRatio(0.001, 0.05), 50);
  assert.equal(mapStepRatio(0.001, 0.0034), 3, 'rounded to whole steps');
  assert.equal(mapStepRatio(0.001, 0.0036), 4);
  assert.equal(mapStepRatio(0.003, 0.01), 3, 'and the bin count rounds up: 10 s / 3 ms = 3,334 steps in 1,112 bins');
  const [sc, st] = scene(0.5, 18, { time_step_s: 0.003, map_time_step_s: 0.01 });
  const c = resultCube(sc, 'spps', st);
  assert.ok(c && c.steps === 3334 && c.bins === 1112, `${c?.steps} ${c?.bins}`);
});

test('every computed band counts whatever "sound maps per band" says; TCR has no cube; no settings, no cube', () => {
  const [sc, st] = scene(0.5, 18, { sound_maps_per_band: false, echogram_per_source: false });
  const c = resultCube(sc, 'spps', st);
  assert.ok(c && c.bands === 18, 'the solver allocates every band it computes (coreinitialisation.cpp:258-266)');
  const [sc1, st1] = scene(0.5, 18, { sound_maps_per_band: true, echogram_per_source: true });
  assert.equal(resultCube(sc1, 'spps', st1)!.bytes, c!.bytes, 'the output switches do not change the allocation');
  assert.equal(resultCube(scene(0.1)[0], 'tcr', scene(0.1)[1]), null);
  assert.equal(resultCube(scene(0.1)[0], 'spps', null), null, 'no settings read yet: no cube, no refusal');
  assert.equal(blockersWithSize(null, 'spps', null), null);
});

test('a surface-group receiver counts its faces as cells', () => {
  const [s, st] = scene(0.5);
  (s.view.surface_receivers as unknown[]).push({ id: 'g', enabled: true, shape: { kind: 'scene', groups: ['g1'] } });
  const c = resultCube(s, 'spps', st);
  const plane = resultCube(scene(0.5)[0], 'spps', st);
  assert.ok(c && plane && c.cells === plane.cells + 500);
});
