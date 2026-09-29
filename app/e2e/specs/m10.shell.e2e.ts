// The foundation's gate ids (PLAN.md 3): the Console wording of the model check (a, b), undo
// through real key events back to the same bytes (f), and no solver-computed acoustic number
// on screen (h, added by the plan from the milestone rules; the gate text does not name it).
import { strict as assert } from 'node:assert';
import { randomUUID } from 'node:crypto';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { ACOUSTIC_NUMBER, allConsoleLines, clickSelector, consoleLines, textOutsideInputsAndGeometry } from '../lib/dom.ts';
import { compareFiles } from '../lib/files.ts';
import { m10, waitForHooks } from '../lib/hooks.ts';
import { env, type Op, type Vec3 } from '../lib/types.ts';

const repo = (rel: string) => path.join(env('M10_REPO'), rel);
const TEACHING_ROOM = () => repo('tests/fixtures/ui/teaching_room.simpa');
const CORRECTED_HALL = () => repo('testdata/elmia_corrected.ply');
const CHECK_OK = 'Closed volume, 0 self-intersections';
/** WebDriver's Control key (the W3C key code; the specs import nothing from wdio). */
const CTRL = '';

/** Lines added to the Console by `action`, as `CLASS text`. */
async function linesDuring(action: () => Promise<unknown>): Promise<string[]> {
  const before = (await allConsoleLines()).length;
  await action();
  return (await allConsoleLines()).slice(before);
}

describe('M10 shell (foundation)', () => {
  before(async () => {
    await waitForHooks(['idle', 'openProject', 'importModel', 'saveAs', 'edit', 'undo', 'undoDepth', 'dirty']);
  });

  it('m10-a-console: the raw hall shows a FAIL line naming the open boundary and its 955 census edges', async () => {
    const added = await linesDuring(() => m10.importModel(env('M10_ELMIA_RAW'), 'm', 'z'));
    const fails = added.filter((l) => l.startsWith('FAIL '));
    // The receipt, in the gate's log.
    console.log(`m10-a-console receipt:\n${added.join('\n')}`);
    assert.ok(
      fails.some((l) => l.includes('open') && l.includes('955')),
      `no FAIL line with 'open' and '955' among:\n${added.join('\n')}`,
    );
    // The DOM's FAIL lines hold it too (the gate's reading).
    const dom = await consoleLines('FAIL');
    assert.ok(dom.some((t) => t.includes('open') && t.includes('955')));
    assert.ok(!added.some((l) => l.startsWith(`INFO ${CHECK_OK}`)), 'the raw hall must not read as closed');
    // Negative control: the teaching room adds no FAIL line.
    const room = await linesDuring(() => m10.openProject(TEACHING_ROOM()));
    assert.deepEqual(room.filter((l) => l.startsWith('FAIL ')), [], room.join('\n'));
  });

  it("m10-b-console: the corrected hall shows the INFO line 'Closed volume, 0 self-intersections'", async () => {
    const added = await linesDuring(() => m10.importModel(CORRECTED_HALL(), 'm', 'z'));
    console.log(`m10-b-console receipt:\n${added.join('\n')}`);
    assert.ok(added.some((l) => l.startsWith(`INFO ${CHECK_OK}`)), added.join('\n'));
    const dom = await consoleLines('INFO');
    assert.ok(dom.some((t) => t.startsWith(CHECK_OK)));
    assert.deepEqual(added.filter((l) => l.startsWith('FAIL ')), []);
  });

  it('m10-f: 50 edits then 50 Ctrl+Z leave the saved file byte-identical', async () => {
    await m10.openProject(TEACHING_ROOM());
    const p = JSON.parse(await m10.projectJson());
    const [m0, m1, m2, m3, m4, m5] = p.materials.map((m: { id: string }) => m.id);
    const [floor, , , , , rear] = p.surface_groups.map((g: { id: string }) => g.id);
    const s1 = p.sources[0].id;
    const [r1, r2, r3] = p.point_receivers.map((r: { id: string }) => r.id);
    const r4 = randomUUID();
    const s2 = randomUUID();
    const carpet = randomUUID();
    const variant = randomUUID();
    const receiver = (id: string, name: string, position: Vec3) => ({
      id,
      name,
      position,
      orientation: [1, 0, 0],
      background_noise: null,
      solver_id: null,
    });
    const source = (id: string, name: string, position: Vec3) => ({
      id,
      name,
      enabled: true,
      position,
      power: { global_db: 85, shape: { kind: 'pink' } },
      directivity: { kind: 'omni' },
      delay_s: 0,
      group: null,
      solver_id: null,
    });
    const material = (id: string, name: string) => ({
      id,
      name,
      color: '#555560',
      absorption: [0.1, 0.1, 0.1, 0.1, 0.1, 0.1],
      scattering: [0.1, 0.1, 0.1, 0.1, 0.1, 0.1],
      reflection_law: 'specular',
      transmission_loss_db: null,
      double_sided: true,
      solver_id: null,
    });
    const band = (m: string, quantity: string, b: number, value: number): Op => ({
      op: 'set_material_band',
      material: m,
      quantity,
      band: b,
      value,
    });
    const moveR = (id: string, position: Vec3): Op => ({ op: 'move_point_receiver', id, position });
    const rename = (kind: string, id: string, name: string): Op => ({ op: 'rename', target: { kind, id }, name });
    // Every op kind the M10 UI issues: rename, move, add and remove of receivers and sources,
    // material bands, add, remove and assign of materials, variants, and batches.
    const edits: Op[] = [
      rename('point_receiver', r1, 'Front row'),
      moveR(r1, [4, 2, 1.2]),
      moveR(r2, [7.5, 4, 1.2]),
      { op: 'move_source', id: s1, position: [2.5, 3, 1.5] },
      rename('source', s1, 'Talker'),
      band(m0, 'absorption', 0, 0.02),
      band(m0, 'absorption', 1, 0.03),
      band(m1, 'scattering', 2, 0.2),
      { op: 'add_point_receiver', index: 3, receiver: receiver(r4, 'R4', [6, 3, 1.2]) },
      moveR(r4, [6.5, 3, 1.2]),
      rename('point_receiver', r4, 'R4 moved'),
      { op: 'remove_point_receiver', id: r4 },
      { op: 'add_source', index: 1, source: source(s2, 'S2', [8, 3, 1.5]) },
      { op: 'move_source', id: s2, position: [8.5, 2.5, 1.5] },
      { op: 'remove_source', id: s2 },
      { op: 'add_material', index: 6, material: material(carpet, 'Carpet') },
      rename('material', carpet, 'Carpet on pad'),
      band(carpet, 'absorption', 3, 0.4),
      { op: 'set_group_material', group: floor, material: carpet },
      { op: 'set_group_material', group: floor, material: m0 },
      { op: 'remove_material', id: carpet },
      { op: 'add_variant', index: 0, variant: { id: variant, name: 'Treated rear wall', overrides: [] } },
      { op: 'set_active_variant', variant },
      { op: 'set_variant_override', variant, group: rear, material: m4 },
      { op: 'set_variant_override', variant, group: rear, material: m5 },
      rename('variant', variant, 'Rear curtain'),
      { op: 'set_active_variant', variant: null },
      { op: 'batch', ops: [moveR(r3, [3, 4.5, 1.2]), moveR(r2, [7, 4, 1.2])] },
      { op: 'batch', ops: [0.013, 0.015, 0.02, 0.03, 0.04, 0.05].map((v, b) => band(m2, 'absorption', b, v)) },
      ...[0.25, 0.6, 0.8, 0.7, 0.5, 0.4].map((v, b) => band(m3, 'absorption', b, v)),
      ...[0.3, 0.3, 0.4, 0.4, 0.5, 0.5].map((v, b) => band(m3, 'scattering', b, v)),
      ...([
        [4.5, 2, 1.2],
        [5, 2.5, 1.2],
        [5.5, 2.5, 1.2],
        [5, 2, 1.2],
        [4, 2.5, 1.2],
      ] as Vec3[]).map((pos) => moveR(r1, pos)),
      rename('point_receiver', r2, 'Middle'),
      rename('point_receiver', r2, 'Middle row'),
      rename('point_receiver', r2, 'R2 again'),
      rename('material', m4, 'Velour curtain'),
    ];
    assert.equal(edits.length, 50);

    for (const [i, op] of edits.entries()) {
      const out = await m10.edit(op);
      assert.ok(out.applied, `edit ${i} (${op.op}) refused: ${JSON.stringify(out.refusals)}`);
      assert.equal(await m10.undoDepth(), i + 1, `undo depth after edit ${i}`);
      if (i === 24) {
        // One refused edit in the middle: a receiver outside the room moves nothing.
        const refused = await m10.edit(moveR(r1, [20, 2, 1.2]));
        assert.equal(refused.applied, false);
        assert.equal(refused.refusals[0]?.code, 'RECEIVER_OUTSIDE');
        assert.equal(await m10.undoDepth(), i + 1, 'a refused edit does not move the undo depth');
      }
    }

    // 50 undos as real key events, with the focus off any text field.
    await clickSelector('.statusbar');
    const work = env('M10_WORK');
    for (let n = 1; n <= 50; n++) {
      await browser.keys([CTRL, 'z']);
      await m10.idle();
      assert.equal(await m10.undoDepth(), 50 - n, `undo depth after ${n} Ctrl+Z`);
      if (n === 49) {
        // Control: one edit short of the start, the saved bytes differ.
        const f49 = path.join(work, 'f49.simpa');
        await m10.saveAs(f49);
        assert.notEqual(compareFiles(f49, TEACHING_ROOM()), null, 'after 49 undos the file must still differ');
      }
    }
    const f = path.join(work, 'f.simpa');
    await m10.saveAs(f);
    const diff = compareFiles(f, TEACHING_ROOM());
    assert.equal(diff, null, diff ?? '');
    assert.equal(await m10.undoDepth(), 0);
    assert.equal(await m10.dirty(), false);
    assert.ok(readFileSync(f).length > 0);
  });

  it('m10-h: no solver-computed acoustic number is shown, on any step or dock tab', async () => {
    const steps = ['geometry', 'materials', 'sources', 'simulate', 'results'];
    const tabs = ['acoustics', 'console', 'runs'];
    const seen: string[] = [];
    for (const load of [() => m10.openProject(TEACHING_ROOM()), () => m10.importModel(CORRECTED_HALL(), 'm', 'z')]) {
      await load();
      for (const step of steps) {
        await clickSelector(`[data-step="${step}"]`);
        for (const tab of tabs) {
          await clickSelector(`[data-dock-tab="${tab}"]`);
          await m10.idle();
          if (tab === 'acoustics') {
            const acoustics = await browser.execute(
              () => document.querySelector('[data-dock-panel="acoustics"]')?.textContent ?? null,
            );
            assert.notEqual(acoustics, null, 'the Acoustics panel is shown');
            assert.ok(!/\d/.test(acoustics ?? ''), `a digit in the Acoustics panel: ${acoustics}`);
          }
          const text = await textOutsideInputsAndGeometry();
          const m = text.match(ACOUSTIC_NUMBER);
          assert.equal(m, null, `step ${step}, tab ${tab}: "${m?.[0]}" in the text outside [data-input] and [data-geometry]`);
          seen.push(`${step}/${tab}`);
        }
      }
    }
    assert.equal(seen.length, 2 * steps.length * tabs.length);
  });
});
