import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import type { CheckSummary, Material, ProjectView, SceneState, UiIssue } from '../bindings/ipc.ts';
import {
  blockerText,
  checkRows,
  coord,
  effectiveMaterial,
  effectiveMaterialId,
  exact,
  fact,
  fileLabel,
  isClosed,
  matchesFilter,
  nextVariantName,
  powerText,
  roomCentre,
  runTooltip,
  sentence,
  stepSubs,
  uniqueIssues,
  unitsText,
  variantName,
  withAxis,
  worstSeverity,
} from './sceneModel.ts';

const material = (id: string, name: string): Material => ({
  id,
  name,
  color: '#555560',
  absorption: [0.1],
  scattering: [0.1],
  reflection_law: 'specular',
  transmission_loss_db: null,
  double_sided: true,
  solver_id: null,
});

const source = (enabled: boolean) => ({
  id: `s${enabled}`,
  name: 'S',
  enabled,
  position: [1, 1, 1] as [number, number, number],
  power: { global_db: 85, shape: { kind: 'pink' as const } },
  directivity: { kind: 'omni' as const },
  delay_s: 0,
  group: null,
  solver_id: null,
});

const receiver = (id: string) => ({
  id,
  name: id,
  position: [1, 1, 1] as [number, number, number],
  orientation: [1, 0, 0] as [number, number, number],
  background_noise: null,
  solver_id: null,
});

function view(over: Partial<ProjectView> = {}): ProjectView {
  return {
    name: 'Room',
    description: '',
    bands: { kind: 'octave', frequencies_hz: [125] },
    surface_groups: [
      { id: 'g1', name: 'Floor', material: 'm1' },
      { id: 'g2', name: 'Rear wall', material: 'm2' },
    ],
    materials: [material('m1', 'Linoleum'), material('m2', 'Plaster'), material('m3', 'Curtain')],
    sources: [],
    point_receivers: [],
    surface_receivers: [],
    variants: [],
    active_variant: null,
    ...over,
  };
}

function check(over: Partial<CheckSummary> = {}, counts: Partial<CheckSummary['counts']> = {}): CheckSummary {
  return {
    verdict: 'ok',
    reasons: [],
    area_m2: 216,
    enclosed_volume_m3: 180,
    bbox_min: [0, 0, 0],
    extents_m: [10, 6, 3],
    highlight_faces: [],
    counts: {
      vertices: 8,
      faces: 12,
      edges: 18,
      open_edges: 0,
      nonmanifold_edges: 0,
      self_intersecting_pairs: 0,
      self_intersecting_faces: 0,
      degenerate_faces: 0,
      duplicate_faces: 0,
      invalid_faces: 0,
      inverted_faces: 0,
      exterior_faces: 0,
      boundary_open_edges: 0,
      cells: 1,
      components: 1,
      coincident_vertices: 0,
      ...counts,
    },
    ...over,
  };
}

function scene(v: ProjectView, c: CheckSummary | null, assigned: number): SceneState {
  return {
    info: { groups_assigned: assigned, surface_groups: v.surface_groups.length } as SceneState['info'],
    view: v,
    groups: [],
    check: c,
    issues: [],
    run_blockers: [],
    lines: [],
  };
}

test('step subs: closed, refused or no model; assigned / groups; enabled sources · receivers', () => {
  const v = view({ sources: [source(true), source(false)], point_receivers: [receiver('r1'), receiver('r2'), receiver('r3')] });
  assert.deepEqual(stepSubs(scene(v, check(), 2)), {
    geometry: 'closed',
    materials: '2 / 2',
    sources: '1 · 3',
    simulate: '',
    results: '',
  });
  assert.equal(stepSubs(scene(v, check({ verdict: 'refused' }), 0)).geometry, 'refused');
  assert.equal(stepSubs(scene(v, check({ verdict: 'refused' }), 0)).materials, '0 / 2');
  assert.equal(stepSubs(scene(view({ surface_groups: [] }), null, 0)).geometry, 'no model');
  assert.deepEqual(stepSubs(null), { geometry: 'no model', materials: '', sources: '', simulate: '', results: '' });
});

test('the effective material follows the active variant, else the base', () => {
  const variants = [{ id: 'v1', name: 'Rear curtain', overrides: [{ group: 'g2', material: 'm3' }] }];
  const base = view({ variants });
  assert.equal(effectiveMaterialId(base, 'g2'), 'm2');
  const active = view({ variants, active_variant: 'v1' });
  assert.equal(effectiveMaterialId(active, 'g2'), 'm3');
  assert.equal(effectiveMaterialId(active, 'g1'), 'm1', 'a group without an override keeps its base material');
  assert.equal(effectiveMaterial(active, 'g2')?.name, 'Curtain');
  assert.equal(effectiveMaterialId(active, 'nope'), null);
  assert.equal(variantName(active), 'Rear curtain');
  assert.equal(variantName(base), 'Baseline');
  assert.equal(variantName(null), 'Baseline');
});

test('a new variant takes the first free name', () => {
  assert.equal(nextVariantName(view()), 'Variant 1');
  const variants = [
    { id: 'a', name: 'Variant 1', overrides: [] },
    { id: 'b', name: 'Variant 3', overrides: [] },
  ];
  assert.equal(nextVariantName(view({ variants })), 'Variant 2');
});

test('the check list: an ok box passes every row, the units stated without a verdict', () => {
  const rows = checkRows(check(), 'm (chosen at import)');
  assert.deepEqual(
    rows.map((r) => [r.label, r.value, r.state]),
    [
      ['Closed volume', 'yes', 'OK'],
      ['Self-intersections', '0', 'OK'],
      ['Flipped normals', '0', 'OK'],
      ['Open edges', '0', 'OK'],
      ['Units', 'm (chosen at import)', null],
    ],
  );
});

test('the check list: the raw hall fails closed, self-intersections and open edges, and lists its other reasons', () => {
  const reason = (code: string, count: number, faces: number) => ({ code, count, faces, message: code, repairable: false });
  const raw = check(
    {
      verdict: 'refused',
      reasons: [
        reason('degenerate_faces', 1, 1),
        reason('self_intersections', 1395, 896),
        reason('open_boundary', 953, 1085),
        reason('no_enclosed_volume', 1, 0),
      ],
    },
    { open_edges: 955, self_intersecting_pairs: 1395, self_intersecting_faces: 896 },
  );
  assert.equal(isClosed(raw), false);
  const rows = checkRows(raw, 'm (chosen at import)');
  const byKey = Object.fromEntries(rows.map((r) => [r.key, r]));
  assert.equal(byKey.closed.state, 'FAIL');
  assert.equal(byKey.closed.value, 'no');
  assert.equal(byKey.self_intersections.value, '1395 pairs · 896 faces');
  assert.equal(byKey.self_intersections.state, 'FAIL');
  assert.equal(byKey.open_edges.value, '955', 'the census count, as the Console prints it');
  assert.equal(byKey.open_edges.state, 'FAIL');
  assert.equal(byKey.flipped.state, 'OK');
  assert.deepEqual(
    rows.slice(5).map((r) => [r.key, r.state]),
    [['degenerate_faces', 'FAIL']],
    'reasons not named by a row get their own row; the named ones do not repeat',
  );
});

test('closed needs no open edges and an enclosed volume', () => {
  assert.equal(isClosed(check()), true);
  assert.equal(isClosed(check({}, { open_edges: 2 })), false);
  assert.equal(
    isClosed(check({ verdict: 'refused', reasons: [{ code: 'no_enclosed_volume', count: 1, faces: 0, message: '', repairable: false }] })),
    false,
  );
});

test('units and file label follow the saved path', () => {
  assert.equal(unitsText({ path: null }), 'm (chosen at import)');
  assert.equal(unitsText({ path: 'C:\\rooms\\hall.simpa' }), 'm (project file)');
  assert.equal(fileLabel({ path: 'C:\\rooms\\hall.simpa', name: 'Hall' }), 'hall.simpa');
  assert.equal(fileLabel({ path: '/a/b/box.simpa', name: 'Box' }), 'box.simpa');
  assert.equal(fileLabel({ path: null, name: 'elmia' }), 'elmia');
});

test('Run blockers read as text, each with its code', () => {
  assert.equal(blockerText('GEOMETRY_REFUSED'), 'GEOMETRY_REFUSED: the model check refused the geometry');
  assert.equal(blockerText('SOMETHING_NEW'), 'SOMETHING_NEW');
  const tip = runTooltip(['GEOMETRY_REFUSED', 'SOLVER_NOT_FOUND', 'RUN_ACTIVE']);
  assert.ok(tip.includes('GEOMETRY_REFUSED') && tip.includes('SOLVER_NOT_FOUND') && tip.includes('RUN_ACTIVE'), tip);
  assert.ok(!tip.includes('M11_PENDING'), 'Run is wired in M11');
  assert.match(runTooltip(null), /open a project/);
  assert.match(runTooltip([]), /^Run the solver/);
});

const issue = (code: string, severity: 'error' | 'warning', path = '/x'): UiIssue => ({
  code,
  rule: code.toLowerCase(),
  severity,
  path,
  entity: null,
  field: '',
  message: code,
});

test('severity and de-duplication of issues', () => {
  assert.equal(worstSeverity([]), null);
  assert.equal(worstSeverity([issue('A', 'warning')]), 'warning');
  assert.equal(worstSeverity([issue('A', 'warning'), issue('B', 'error')]), 'error');
  const a = issue('A', 'error');
  assert.deepEqual(uniqueIssues([a], [a, issue('A', 'error', '/y')]).map((i) => i.path), ['/x', '/y']);
});

test('the scene filter: every term, any field, ignoring case', () => {
  assert.equal(matchesFilter('', 'Floor'), true);
  assert.equal(matchesFilter('  ', 'Floor'), true);
  assert.equal(matchesFilter('wall', 'Rear wall', 'Plaster'), true);
  assert.equal(matchesFilter('WALL', 'Rear wall'), true);
  assert.equal(matchesFilter('wall', 'Floor', 'Linoleum'), false);
  assert.equal(matchesFilter('rear plaster', 'Rear wall', 'Plaster'), true, 'terms may match different fields');
  assert.equal(matchesFilter('rear curtain', 'Rear wall', 'Plaster'), false);
});

test('facts drop trailing zeros; a missing value is a dash', () => {
  assert.equal(fact(180, 1), '180');
  assert.equal(fact(10389.096, 1), '10389.1');
  assert.equal(fact(4001.809, 1), '4001.8');
  assert.equal(fact(10, 2), '10');
  assert.equal(fact(0.5, 2), '0.5');
  assert.equal(fact(null, 1), '—');
  assert.equal(fact(NaN, 1), '—');
  assert.equal(fact(undefined, 1), '—');
});

test('coordinates for lists round; edit fields are exact', () => {
  assert.equal(coord(5, 1), '5.0');
  assert.equal(coord('NaN', 1), 'NaN');
  assert.equal(exact(0.1 + 0.2), '0.30000000000000004');
  assert.equal(exact(-0), '-0');
  assert.equal(exact(1.2), '1.2');
  assert.equal(exact('inf'), 'inf');
});

test('an edited axis replaces one coordinate and passes the others on as stored', () => {
  assert.deepEqual(withAxis([5, 2, 1.2], 'x', 20), [20, 2, 1.2]);
  assert.deepEqual(withAxis([5, 'NaN', 1.2], 'z', 1.5), [5, 'NaN', 1.5]);
});

test('a new point goes to the middle of the bounding box, lifted from its floor', () => {
  assert.deepEqual(roomCentre(check(), 1.2), [5, 3, 1.2]);
  assert.deepEqual(roomCentre(check({ bbox_min: [-5, 0, 2], extents_m: [10, 6, 1] }), 1.2), [0, 3, 2.5], 'never above mid-height');
  assert.equal(roomCentre(null, 1.2), null);
  assert.equal(roomCentre(check({ extents_m: [NaN, 6, 3] }), 1.2), null);
});

test('emission reads as stored', () => {
  assert.equal(powerText(85), '85 dB');
  assert.equal(powerText(-0), '-0 dB');
});

test('a validator message reads as a sentence', () => {
  assert.equal(sentence('no source is enabled: the run would emit nothing and exit 0'), 'No source is enabled: the run would emit nothing and exit 0.');
  assert.equal(sentence('Already one.'), 'Already one.');
  assert.equal(sentence('  '), '');
});
