import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import type { CheckSummary, Material, ProjectView, SceneState, UiIssue } from '../bindings/ipc.ts';
import {
  directionTo,
  repairSummary,
  unrepairable,
  environmentText,
  fittingZonesOf,
  volumeRow,
  blockerText,
  displayName,
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
  receiverFolder,
  regroupFaces,
  roomCentre,
  sourceSpot,
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
    air_volume_m3: 180,
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
    solver_issues: { spps: [], tcr: [] },
    advice: [],
    advice_conflicts: [],
    lines: [],
  };
}

test('step subs: closed, refused or no model; assigned / groups; enabled sources · receivers', () => {
  const v = view({ sources: [source(true), source(false)], point_receivers: [receiver('r1'), receiver('r2'), receiver('r3')] });
  assert.deepEqual(stepSubs(scene(v, check(), 2)), {
    geometry: 'room watertight',
    materials: '2 of 2 set',
    sources: '1 source · 3 receivers',
    simulate: '',
    results: '',
  });
  assert.equal(stepSubs(scene(v, check({ verdict: 'refused' }), 0)).geometry, 'not watertight');
  assert.equal(stepSubs(scene(v, check({ verdict: 'refused' }), 0)).materials, '0 of 2 set');
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
      ['Watertight', 'yes', 'OK'],
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
  // C1: + Source sits off the centre: the 6 x 10 x 3 m box's centroid fails SPPS on loops.
  const box = check({ bbox_min: [0, 0, 0], extents_m: [6, 10, 3] });
  const s = sourceSpot(box, 1.5)!;
  assert.notDeepEqual(s, roomCentre(box, 1.5));
  assert.ok(Math.abs(s[0] - 2.292) < 1e-12 && Math.abs(s[1] - 4.14) < 1e-12 && s[2] === 1.5, String(s));
  assert.notEqual(s[0] / 6, s[1] / 10, 'not on the plane x/Lx = y/Ly a box mesh has');
  assert.equal(sourceSpot(null, 1.5), null);
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

test('New group from selection takes the picked faces, each once, ascending; nothing else', () => {
  assert.deepEqual(regroupFaces({ kind: 'faces', faces: [3, 2], groups: ['Walls'] }), [2, 3]);
  assert.deepEqual(regroupFaces({ kind: 'faces', faces: [5, 2, 5], groups: ['Walls'] }), [2, 5]);
  assert.equal(regroupFaces({ kind: 'faces', faces: [], groups: [] }), null);
  assert.equal(regroupFaces({ kind: 'group', id: 'g1' }), null, 'a whole group is already a group');
  assert.equal(regroupFaces({ kind: 'none' }), null);
  assert.equal(regroupFaces({ kind: 'receiver', id: 'r1' }), null);
});

test("a point receiver's folder reads as its group path, and the filter finds it", () => {
  assert.equal(receiverFolder({ group: 'Stalls / Front' }), 'Stalls / Front');
  assert.equal(receiverFolder({ group: null }), '');
  assert.equal(receiverFolder({}), '', 'a project saved before receiver groups');
  assert.ok(matchesFilter('front', 'Receiver 1', receiverFolder({ group: 'Stalls / Front' })));
});

test('surface and material names read as words: the import prefix dropped, sentence case, the stored name untouched', () => {
  assert.equal(displayName('mat_CR4_whitePanels'), 'White panels');
  assert.equal(displayName('mat_CR1_tablesEquipment'), 'Tables equipment');
  assert.equal(displayName('mat_CR3_structuredPlaster'), 'Structured plaster');
  assert.equal(displayName('door_room1'), 'Door room 1');
  assert.equal(displayName('ext_walls'), 'Ext walls');
  assert.equal(displayName('audience'), 'Audience');
  // A name that is all prefix keeps its stored form rather than showing nothing.
  assert.equal(displayName('mat_CR2_'), 'mat_CR2_');
});

test('M32: the direction toward a point is of length one; none toward the same point or a non-number', () => {
  assert.deepEqual(directionTo([1, 2, 3], [1, 2, 8]), [0, 0, 1]);
  assert.deepEqual(directionTo([0, 0, 0], [3, -4, 0]), [0.6, -0.8, 0]);
  assert.equal(directionTo([1, 1, 1], [1, 1, 1]), null);
  assert.equal(directionTo([1, 1, 1], ['NaN', 1, 1]), null);
});

test('G16: fitting zones are read from the project file, box or scene, on or off; none without a list', () => {
  const json = JSON.stringify({
    fitting_zones: [
      { id: 'a', name: 'Stalls', enabled: true, shape: { kind: 'box', min: [0, 0, 0], max: [1, 1, 1], destination: null } },
      { id: 'b', name: 'Stage', enabled: false, shape: { kind: 'scene', groups: [] } },
    ],
  });
  assert.deepEqual(fittingZonesOf(json), [
    { id: 'a', name: 'Stalls', enabled: true, kind: 'box' },
    { id: 'b', name: 'Stage', enabled: false, kind: 'scene' },
  ]);
  assert.deepEqual(fittingZonesOf('{"fitting_zones":[]}'), []);
  assert.equal(fittingZonesOf('{}'), null);
  assert.equal(fittingZonesOf('not json'), null);
});

test('G16: the volume row is the Geometry panel number, and none for a refused model', () => {
  const ok = { verdict: 'ok', air_volume_m3: 180.04 } as unknown as CheckSummary;
  assert.equal(volumeRow(ok).text, 'Room air · 180 m³');
  const refused = { verdict: 'refused', air_volume_m3: null } as unknown as CheckSummary;
  assert.equal(volumeRow(refused).text, 'None: the model is refused');
  assert.ok(!/\d/.test(volumeRow(refused).text), 'no digit for a refused model');
  assert.equal(volumeRow(null).text, 'No model');
});

test('G16: the environment row spells the air as stored', () => {
  assert.equal(environmentText({ temperature_c: 20, relative_humidity_percent: 50, pressure_pa: 101325 }), '20 °C · 50 % · 101325 Pa');
  assert.equal(environmentText(null), '—');
});

test('G8: a repair in words names each fix, the new file and the untouched original', () => {
  const base = { welded_vertices: 1, degenerate_faces: 0, duplicate_faces: 2, flipped_faces: 3, oriented: true };
  assert.equal(
    repairSummary({ ...base, changed: true, file: 'C:\\models\\hall_repaired.obj', original: 'C:\\models\\hall.ply' }),
    'Repaired: 1 vertex welded, 0 faces of zero area removed, 2 repeated faces removed, 3 faces turned out. Written to hall_repaired.obj beside hall.ply, which is unchanged.',
  );
  assert.ok(repairSummary({ ...base, changed: true, file: 'x_repaired.obj', original: 'x.ply', oriented: false }).endsWith('none was turned.'));
  assert.ok(repairSummary({ ...base, changed: false }).startsWith('Repair found nothing to change'));
  const check = { reasons: [{ code: 'self_intersections', message: 'Faces intersect', repairable: false, count: 1, faces: 2 }, { code: 'duplicate_faces', message: 'Repeated', repairable: true, count: 1, faces: 1 }] } as unknown as CheckSummary;
  assert.deepEqual(unrepairable(check), ['Faces intersect']);
  assert.deepEqual(unrepairable(null), []);
});
