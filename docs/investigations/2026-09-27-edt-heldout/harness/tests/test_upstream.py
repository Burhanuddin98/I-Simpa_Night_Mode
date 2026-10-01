"""T4-T5 (HARNESS-PLAN.md section 4): upstream's EDT through the validated port (P32, PREREG.md:12)."""
import json
import math

import numpy as np
from conftest import HARNESS, sha256_bytes

from m8b import upstream

FIX = HARNESS / 'fixtures' / 'upstream-t1-2019'
FIXTURE_SHA256 = {                  # as committed; fixtures/upstream-t1-2019/PROVENANCE.json says where from
    'Sound level.recp': '521c81891d5ede31e34beffe5f7f2915875bb8e888160e76a8ccbdf7f2a37afc',
    'Acoustic parameters.gabe': 'c960c782ef0564dda11d6903b82915e9d0e95282b11068e1195719ae0fcb2b22',
    'Schroeder curves.gabe': '0068370b838dcbba15a33ec1d3d8bd104b0c096d680a5267e26ccfbba105adcb',
    'oracle.json': '35a767ed33154d8e0df2796c57df03717cbda880b8f9f81457b0085195f002bc',     # LF; CRLF original 713832b2...
}
PORT_SHA256 = '57f391e6cb09ded8a408462fbabc7aacd93743a3fa46cc28deeef3f7a319f282'    # P32


def f32_bits(x):
    return int(np.float32(x).view(np.int32))


def test_t04_upstream_copy_hash_and_oracle_bit_for_bit():
    """T4: the copy is the port (sha256 57f391e6... in its source's CRLF form) and reproduces
    oracle.json's EDT for every band, the Global row and the Average, bit for bit; against upstream's
    own 'Acoustic parameters.gabe' it is bit-identical exactly where oracle.json says so (24 of 29)."""
    for name, want in FIXTURE_SHA256.items():
        data = (FIX / name).read_bytes()
        if name.endswith('.json'):
            data = data.replace(b'\r\n', b'\n')
        assert sha256_bytes(data) == want, 'fixture %s changed' % name

    assert upstream.UPSTREAM_SHA256 == PORT_SHA256
    assert upstream.COPY.is_file(), 'no copy of the port at %s' % upstream.COPY
    lf = upstream.COPY.read_bytes().replace(b'\r\n', b'\n')
    assert sha256_bytes(lf.replace(b'\n', b'\r\n')) == PORT_SHA256, 'the copy is not the port, line ends aside'

    oracle = json.loads((FIX / 'oracle.json').read_text(encoding='utf-8'))['params']['EDT (s)']
    rows = upstream.edt_table(FIX / 'Sound level.recp')
    assert rows is not None, 'upstream.edt_table returned nothing'
    assert [label for label, _ in rows] == [r['row'] for r in oracle['rows']]
    assert len(rows) == 29
    for (label, value), r in zip(rows, oracle['rows']):
        assert f32_bits(value) == f32_bits(r['port']), '%s: %r, oracle %r' % (label, value, r['port'])

    port = upstream.port()
    assert port is not None
    ap = port.read_gabe(str(FIX / 'Acoustic parameters.gabe'))
    col = [c for c in ap['cols'] if c['label'] == 'EDT (s)'][0]['data']
    same = [f32_bits(v) == f32_bits(f) for (_, v), f in zip(rows, col)]
    assert same == [r['bit_identical'] for r in oracle['rows']]
    assert sum(same) == oracle['bit_identical'] == 24
    rel = max(abs(float(v) / float(f) - 1) for (_, v), f in zip(rows, col)       # uphunt_oracle.py:58
              if math.isfinite(float(v)) and math.isfinite(float(f)) and float(f) != 0)
    assert rel == oracle['max_rel_diff']


def test_t05_upstream_wrapper_point_range_and_nan_refusal():
    """T5: a finite positive value is ok with a point range (edt_lo == edt_hi == edt); NaN is refused."""
    dt = 1e-3
    t_arr = 0.02
    k = 6 * math.log(10) / 1.2
    t = np.arange(1500) * dt
    bins = np.where(t >= t_arr, np.exp(-k * (t - t_arr)) * dt, 0.0)
    bins[int(t_arr / dt)] += 0.05                                # a direct sound
    r = upstream.analyse(bins, dt, t_arr)
    assert r is not None, 'upstream.analyse returned nothing'
    assert r['status'] == 'ok'
    assert math.isfinite(r['edt']) and r['edt'] > 0
    assert r['edt_lo'] == r['edt_hi'] == r['edt']
    v, _ = upstream.port().edt(np.asarray(bins, np.float32), float(dt), kind='point')
    assert r['edt'] == float(v)

    zeros = np.zeros(1000)
    v0, _ = upstream.port().edt(zeros.astype(np.float32), float(dt), kind='point')
    assert math.isnan(v0), 'the port gives NaN on a series with no energy'
    z = upstream.analyse(zeros, dt, t_arr)
    assert z['status'] == 'refused' and z['reason'] == 'nan_or_nonpositive'
    assert z['edt'] is None and z['edt_lo'] is None and z['edt_hi'] is None
