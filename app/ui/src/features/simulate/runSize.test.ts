import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import type { SceneState } from '../../bindings/ipc.ts';
import type { ProjectSettings } from './model.ts';
import { blockersWithSize, cubeText, mapStepRatio, REFUSE_GB, RESULTS_TOO_BIG, resultCube, WARN_GB } from './runSize.ts';

// CR4-third as it crashed at 06:37 on 2026-10-06: Plane 1 over the whole room, 33.1 x 33.3 m at 0.1 m, 10 s at
// 1 ms, 18 bands with maps per band, 2 sources with an echogram each.
function scene(resolution: number, bands = 18, extra: Record<string, unknown> = {}): [SceneState, ProjectSettings] {
  const spps = {
    time_step_s: 0.001,
    duration_s: 10,
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

test('the whole-room plane at 0.1 m is about 74 GB (float32, one cube for every source) and blocks the run; at 0.5 m it does not', () => {
  const [bigScene, bigSettings] = scene(0.1);
  const big = resultCube(bigScene, 'spps', bigSettings);
  assert.ok(big && big.cells === 110888, `${big?.cells}`);
  assert.equal(big!.steps, 10_000);
  assert.equal(big!.ratio, 1);
  assert.equal(big!.bins, 10_000);
  // 110,888 x 10,000 x 18 x 4 bytes = 79.84e9 bytes = 74.4 GiB.
  assert.equal(big!.bytes, 110888 * 10000 * 18 * 4);
  assert.equal(cubeText(big!), 'about 74 GB');
  assert.deepEqual(blockersWithSize(bigScene, 'spps', bigSettings), [RESULTS_TOO_BIG]);
  const [smallScene, smallSettings] = scene(0.5);
  const small = resultCube(smallScene, 'spps', smallSettings);
  assert.ok(small && small.gb < REFUSE_GB, `${small?.gb}`);
  assert.deepEqual(blockersWithSize(smallScene, 'spps', smallSettings), []);
});

test('a sound-map time step of 10 ms divides the cube by 10: the 0.1 m plane fits under the warning line', () => {
  const [sc, st] = scene(0.1, 18, { map_time_step_s: 0.01 });
  const c = resultCube(sc, 'spps', st);
  assert.ok(c);
  assert.equal(c.ratio, 10);
  assert.equal(c.bins, 1_000);
  assert.equal(c.bytes, 110888 * 1000 * 18 * 4);
  assert.equal(cubeText(c), 'about 7.4 GB');
  assert.ok(c.gb < WARN_GB);
  assert.deepEqual(blockersWithSize(sc, 'spps', st), []);
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

test('every computed band counts whatever "sound maps per band" says, sources never do; TCR has no cube', () => {
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
