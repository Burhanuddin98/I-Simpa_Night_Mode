"""corpus_rooms_2.json and preview_pin.json (HARNESS-PLAN-2.md section 2; section 9 M3): the corpus round 2 keeps
clear of, and the pin of what round 2 drew before B1.

corpus_rooms_2.json is built here, deterministically, never by hand: round 1's 119 rooms (corpus_rooms.json, whose
sha256 is asserted first), then everything round 1 made or solved, then the G rooms, so that ISM-fresh-2's draw
keeps clear of all of them and a G room's own clearance is computed over the entries of other sets:

| set          | n  | what                                                                                          |
| (round 1's)  |119 | z3, noise_cal, ism, m8a, in corpus_rooms.json's own order                                      |
| run1_spps    |  7 | F1-F7 (rooms.rooms()); F3 and F4 are the list's first non-box rooms                            |
| run1_probe   |  2 | P0, P0b: never scored, but their histograms exist; they can only remove candidates             |
| run1_ism     | 12 | ism_fresh._draw(2026100102, corpus_rooms.json): run 1's four relatives and eight drawn rooms,  |
|              |    | each equal to the room run 1 scored (counts.json), checked here                                |
| round2_spps  |  7 | G1-G7 (rooms2.rooms()), added last                                                             |

147 rooms, 143 boxes and 4 non-box. Round 1's Synth-fresh seed, SPPS, truth and probe seeds and the sha256 of its
4,000 specs go in as lists (`seeds_used`, `synth_specs_sha256`): they are not rooms.

preview_pin.json (M3) holds the sha256 of the canonical JSON of what round 2 drew before B1 (ISM-fresh-2's draw,
Synth-fresh-2's 4,000 specs, the G rooms' geometry and receivers, corpus_rooms_2.json). They are geometry and
parameters, made without an echogram, a histogram or a method call, as the plan's own preview was. B1 records the
pin; the scorer's preflight draws again and stops if any of them differs (check_pin).

    python -m m8b.corpus2 [--write] [--pin] [--check]
"""
import hashlib
import json
import sys
from pathlib import Path

from . import corpus, ism_fresh, rooms, rooms2, round2, synth_fresh
from .corpus import sha256_bytes

LIST1 = corpus.HARNESS / 'corpus_rooms.json'
LIST2 = corpus.HARNESS / 'corpus_rooms_2.json'
LIST1_SHA256 = 'dc3d0b2c0b8be0f69c7927f0c749a88dad76cc791edae2474f09cb1fd44da882'      # asserted before the build
COUNTS_RUN1 = Path(r'B:\data\m8b-edt\results\run1\counts.json')
SCHEMA = 'm8b.corpus_rooms/2'
SCHEMA_PIN = 'm8b.preview_pin/1'
RUN1_ISM_SEED, RUN1_SYNTH_SEED = 2026100102, 2026100101

# round 1's seeds, typed as round 1 listed them (HARNESS-PLAN.md P12, P21, P24)
SEEDS_USED = dict(
    spps_tested=[1101, 1102, 1103, 1201, 1202, 1203, 1501, 1502, 1503, 2101, 2102, 2103, 2201, 2202, 2203, 2501, 2502, 2503],
    spps_truth=[9001, 9002, 9003, 9004], probes=[9998, 9999], ism_fresh=RUN1_ISM_SEED, synth_fresh=RUN1_SYNTH_SEED,
    dev=[1, 20261001])


def canon(x):
    return json.dumps(x, sort_keys=True, separators=(',', ':'), allow_nan=False)


def canon_sha256(x):
    return sha256_bytes(canon(x).encode('utf-8'))


def _entry_of_room(set_, name, room):
    """A corpus entry for a room of rooms.rooms() / rooms2.rooms()'s form (box or union of boxes)."""
    return dict(set=set_, id='%s:%s' % (set_, name), kind=room['kind'], dims_sorted_m=rooms.sorted_dims(room),
                volume_m3=room['volume_m3'], surface_m2=room['surface_m2'], bbox_m=room['bbox_m'], boxes=room['boxes'],
                source_m=room['source_m'], receivers_m=[r['position_m'] for r in room['receivers']],
                design_t60_s={str(f): v for f, v in room['design_t60_s'].items()}, role=room['role'])


def _entry_of_ism(room):
    dims = [float(x) for x in room['dims_m']]
    return dict(set='run1_ism', id='run1_ism:' + room['id'], kind='box', draw_kind=room['kind'], parent=room['parent'],
                dims_m=dims, dims_sorted_m=sorted(dims, reverse=True), volume_m3=dims[0] * dims[1] * dims[2],
                surface_m2=sum(corpus.box_areas(dims)), bbox_m=[[0.0, 0.0, 0.0], dims], source_m=room['source_m'],
                alpha_walls=room['alpha_walls'], design_t60_s={str(b): v for b, v in room['design_t60_s'].items()},
                receivers=[{'position_m': r['position_m'], 'R_m': r['R_m'], 'class': r['class']} for r in room['receivers']])


def run1_ism_draw():
    """Run 1's ISM-fresh draw, reproduced against run 1's own list (no guard: it is a record, not data)."""
    return ism_fresh._draw(RUN1_ISM_SEED, LIST1)


def build(counts_path=None):
    """The corpus_rooms_2.json object. Raises when corpus_rooms.json is not run 1's, or when the reproduced run-1
    ISM draw is not the one run 1 scored (counts.json)."""
    data1 = LIST1.read_bytes()
    if sha256_bytes(data1) != LIST1_SHA256:
        raise ValueError('%s has sha256 %s, not run 1\'s %s' % (LIST1, sha256_bytes(data1), LIST1_SHA256))
    c1 = json.loads(data1)
    counts_path = Path(counts_path or COUNTS_RUN1)
    counts_bytes = counts_path.read_bytes()
    counts = json.loads(counts_bytes)
    D = run1_ism_draw()
    if (len(D['rooms']) != len(counts['ism_rooms']) or D['rejections'] != counts['ism_rejections']
            or counts['ism_seed'] != RUN1_ISM_SEED):
        raise ValueError('the reproduced run-1 ISM draw is not the one in %s' % counts_path)
    for mine, theirs in zip(D['rooms'], counts['ism_rooms']):
        got = dict(id=mine['id'], kind=mine['kind'], parent=mine['parent'], dims_m=mine['dims_m'],
                   alpha_walls=mine['alpha_walls'], design_t60_s={str(b): v for b, v in mine['design_t60_s'].items()})
        if got != theirs:
            raise ValueError('run-1 ISM room %s differs from %s' % (mine['id'], counts_path))
    if sorted(SEEDS_USED['spps_tested'] + SEEDS_USED['spps_truth'] + [RUN1_ISM_SEED, RUN1_SYNTH_SEED]) != sorted(round2.ROUND1_SEEDS):
        raise ValueError('round 1\'s seed list here is not round2.ROUND1_SEEDS')
    specs1 = synth_fresh._draw(RUN1_SYNTH_SEED)
    F = rooms.rooms()
    entries = list(c1['rooms'])
    entries += [_entry_of_room('run1_spps', n, F[n]) for n in rooms.NAMES]
    entries += [_entry_of_room('run1_probe', n, F[n]) for n in rooms.PROBES]
    entries += [_entry_of_ism(r) for r in D['rooms']]
    G = rooms2.rooms()
    entries += [_entry_of_room('round2_spps', n, G[n]) for n in rooms2.NAMES]
    sets = {}
    for e in entries:
        sets[e['set']] = sets.get(e['set'], 0) + 1
    kinds = {}
    for e in entries:
        kinds[e['kind']] = kinds.get(e['kind'], 0) + 1
    return dict(
        schema=SCHEMA,
        plan='HARNESS-PLAN-2.md section 2 and PREREG-2.md "The corpus to keep clear of" (corpus2.py)',
        rule_p3=c1['rule_p3'],
        corpus_rooms_sha256=LIST1_SHA256,
        rooms=entries,
        no_geometry=c1['no_geometry'],
        seeds_used=SEEDS_USED,
        synth_specs_n=len(specs1),
        synth_specs_sha256=canon_sha256(specs1),
        run1_ism_seed=RUN1_ISM_SEED,
        run1_ism_rejections=D['rejections'],
        run1_ism_draw_sha256=canon_sha256(D),
        run1_counts_json=dict(path='results/run1/counts.json', sha256=sha256_bytes(counts_bytes)),
        summary=dict(n=len(entries), per_set=sets, per_kind=kinds))


def build_text(counts_path=None):
    return json.dumps(build(counts_path), indent=1, sort_keys=False, ensure_ascii=False, allow_nan=False) + '\n'


def write(path=None, counts_path=None):
    text = build_text(counts_path)
    Path(path or LIST2).write_text(text, encoding='utf-8', newline='\n')
    return sha256_bytes(text.encode('utf-8'))


# ---- the pin (section 9 M3) -------------------------------------------------------------------------------------
def preview_pin():
    """What round 2 drew before B1, as hashes: geometry and parameters only (no echogram, no histogram, no method)."""
    list2 = LIST2.read_bytes()
    ism = ism_fresh.preview_draw(round2.ISM_SEED)
    synth = synth_fresh.preview_draw(round2.SYNTH_SEED)
    return dict(
        schema=SCHEMA_PIN,
        note=('Made before B1 and before any round-2 solver run, histogram, echogram or method call: the ISM-fresh-2 draw '
              '(rooms, relatives, receivers, image counts, rejections), the Synth-fresh-2 specs (parameters) and the G '
              'rooms (geometry, materials, source, receivers, design T60s). B1 records this file; the scorer draws again '
              'and refuses to run if any hash differs (corpus2.check_pin).'),
        seeds=dict(ism_fresh_2=round2.ISM_SEED, synth_fresh_2=round2.SYNTH_SEED, attack=round2.ATTACK_SEED),
        corpus_rooms_2_sha256=sha256_bytes(list2),
        ism_fresh_2=dict(seed=round2.ISM_SEED, n_rooms=len(ism['rooms']), rejections=ism['rejections'], sha256=canon_sha256(ism)),
        synth_fresh_2=dict(seed=round2.SYNTH_SEED, n_specs=len(synth), sha256=canon_sha256(synth)),
        g_rooms=dict(names=list(rooms2.NAMES), sha256=rooms2.geometry_sha256()))


def pin_text():
    return json.dumps(preview_pin(), indent=1, sort_keys=False, ensure_ascii=False, allow_nan=False) + '\n'


def write_pin(path=None):
    text = pin_text()
    Path(path or round2.PREVIEW_PIN).write_text(text, encoding='utf-8', newline='\n')
    return sha256_bytes(text.encode('utf-8'))


def check_pin(path=None):
    """The names of the pinned items a fresh draw does not reproduce ([] when all do)."""
    pinned = json.loads(Path(path or round2.PREVIEW_PIN).read_text(encoding='utf-8'))
    fresh = json.loads(pin_text())
    bad = []
    for k in ('corpus_rooms_2_sha256',):
        if pinned.get(k) != fresh[k]:
            bad.append(k)
    for k in ('ism_fresh_2', 'synth_fresh_2', 'g_rooms'):
        if pinned.get(k) != fresh[k]:
            bad.append(k)
    return bad


def main(argv=None):
    import argparse
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('--write', action='store_true', help='write corpus_rooms_2.json')
    ap.add_argument('--pin', action='store_true', help='write preview_pin.json (after corpus_rooms_2.json)')
    ap.add_argument('--check', action='store_true', help='draw again and compare with preview_pin.json')
    a = ap.parse_args(argv)
    if a.write:
        print('corpus_rooms_2.json', write())
    if a.pin:
        print('preview_pin.json', write_pin())
    if a.check:
        bad = check_pin()
        print('pin check:', 'all reproduced' if not bad else 'DIFFERS: %s' % bad)
        return 1 if bad else 0
    return 0


if __name__ == '__main__':
    sys.dont_write_bytecode = True
    sys.exit(main())
