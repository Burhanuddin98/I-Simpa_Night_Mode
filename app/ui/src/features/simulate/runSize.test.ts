import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import type { SceneState } from '../../bindings/ipc.ts';
import type { ProjectSettings } from './model.ts';
import { blockersWithSize, cubeText, REFUSE_GB, RESULTS_TOO_BIG, resultCube } from './runSize.ts';

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

test('the whole-room plane at 0.1 m is about 149 GB and blocks the run; at 0.5 m it is a few GB and does not', () => {
  const [bigScene, bigSettings] = scene(0.1);
  const big = resultCube(bigScene, 'spps', bigSettings);
  assert.ok(big && big.cells === 110888, `${big?.cells}`);
  assert.ok(big && big.gb > 140 && big.gb < 160, `${big?.gb}`);
  assert.equal(cubeText(big!), 'about 149 GB');
  assert.deepEqual(blockersWithSize(bigScene, 'spps', bigSettings), [RESULTS_TOO_BIG]);
  const [smallScene, smallSettings] = scene(0.5);
  const small = resultCube(smallScene, 'spps', smallSettings);
  assert.ok(small && small.gb < REFUSE_GB, `${small?.gb}`);
  assert.deepEqual(blockersWithSize(smallScene, 'spps', smallSettings), []);
});

test('bands and sources count only with maps per band and an echogram per source; TCR has no cube', () => {
  const [sc, st] = scene(0.5, 18, { sound_maps_per_band: false, echogram_per_source: false });
  const c = resultCube(sc, 'spps', st);
  assert.ok(c && c.bands === 1 && c.sources === 1);
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
