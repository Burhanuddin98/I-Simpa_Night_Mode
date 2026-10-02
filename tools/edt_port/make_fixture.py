"""Draw the parity fixture for the Rust port of EDT v2.1 (docs/investigations/2026-09-27-edt-heldout/frozen2).

  python tools/edt_port/make_fixture.py [--results B:/data/m8b-edt/round2/results] [--out testdata/edt_parity]

Writes manifest.jsonl (one JSON object per row) and bins.bin (little-endian f64 blobs, dense or sparse)
and records frozen2's OWN output for every row. frozen2/method.py is loaded only after its sha256 matches
frozen2.sha256 and the value PREREG-2 pinned; its outputs on the round-2 rows must equal rows.csv.gz
(status, reason, edt, edt_lo, edt_hi, exactly) or the script stops.

Sample: round-2 rows stratified by (set, SPPS mode, step, status, refusal reason), at most PER_STRATUM each
(seeded), then crafted rows for refusal reasons round 2 never produced, a missing arrival, and unknown noise
(a seeded fuzz keeps the first row of each outcome signature). Numpy only.
"""
import argparse, csv, gzip, hashlib, importlib.util, json, pickle, random, struct
from pathlib import Path
import numpy as np

ROOT = Path(__file__).resolve().parents[2]
D = ROOT / 'docs/investigations/2026-09-27-edt-heldout'
PINNED = '029d90ac5e8f6a634a51a7ffce136cbd4c0b7ea3d3ad8a18b78fd8240ff224a0'
PER_STRATUM = 10
SEED = 20261002


def load_frozen2():
    raw = (D / 'frozen2/method.py').read_bytes()
    got = hashlib.sha256(raw).hexdigest()
    recorded = (D / 'frozen2.sha256').read_text().split()[0]
    assert got == recorded == PINNED, f'frozen2/method.py hash {got} != {recorded} / {PINNED}'
    spec = importlib.util.spec_from_file_location('frozen2_method', D / 'frozen2/method.py')
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m, got


def run(m, row):
    return m.analyse(row['bins'], row['dt'], row['t_arrival'], row['meta'])


def crafted(m):
    """Rows for what round 2 never produced; deterministic."""
    rng = np.random.default_rng(SEED)
    out = []

    def add(tag, bins, dt, t_arrival, half_width=None):
        out.append(dict(set='crafted', id='crafted|' + tag, mode=None, step_ms=dt * 1e3,
                        bins=np.asarray(bins, float), dt=float(dt), t_arrival=t_arrival,
                        meta={} if half_width is None else {'half_width': half_width}))

    add('zeros', np.zeros(200), 1e-3, 0.01)
    add('too_short', [1.0, 2.0, 1.0], 1e-3, 0.0)
    add('bad_dt', np.ones(50), 0.0, 0.0)
    add('negative_only', -np.ones(50), 1e-3, 0.0)
    e = np.zeros(400)
    e[2] = 5.0
    add('energy_before_arrival', e, 1e-3, 0.5)
    sigs = set()
    for i in range(6000):
        dt = float(rng.choice([1e-3, 2e-3, 5e-3, 1e-3]))
        n = int(rng.integers(5, 1200))
        t60 = float(rng.uniform(0.02, 4.0))
        a = float(rng.choice([0.0, 0.002, 0.01, 0.05, 0.3, 1.0]))
        k = np.arange(n) * dt
        env = 10 ** (-6 * (k - a).clip(0) / t60)
        mean = float(rng.choice([0.0, 2.0, 40.0, 5000.0]))
        if mean == 0.0:
            bins = env * (1 + (rng.random(n) < 0.05) * rng.uniform(0, 30))
        else:
            bins = rng.poisson(env * mean) * rng.uniform(0.5, 2.0)
        if rng.random() < 0.15:
            bins = bins * (np.arange(n) > rng.integers(0, n))
        ta = None if rng.random() < 0.3 else float(a + rng.uniform(-0.01, 0.01))
        hw = None if rng.random() < 0.5 else float(rng.choice([0.0003, 0.0009, 0.002, 0.01, 0.05]))
        r = dict(bins=bins, dt=dt, t_arrival=ta, meta={} if hw is None else {'half_width': hw})
        try:
            o = run(m, r)
        except Exception:   # a Python error is not a fixture row
            continue
        if o['status'] == 'refused':
            sig = ('refused', o['reason'], ta is None)
        else:
            sig = (o['status'], 'noise=unknown' in o['reason'], ta is None)
        if sig in sigs:
            continue
        sigs.add(sig)
        add('fuzz%04d|%s|%s' % (i, o['status'], o['reason'] if o['status'] == 'refused' else 'x'), bins, dt, ta, hw)
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--results', default=r'B:\data\m8b-edt\round2\results')
    ap.add_argument('--out', default=str(ROOT / 'testdata/edt_parity'))
    a = ap.parse_args()
    m, sha = load_frozen2()
    res = Path(a.results)
    csv_rows = {r['id']: r for r in csv.DictReader(gzip.open(res / 'rows.csv.gz', 'rt'))}
    inputs = []
    for s in ('spps', 'ism', 'synth'):
        inputs += pickle.load(gzip.open(res / f'inputs_{s}.pkl.gz'))
    assert len(inputs) == len(csv_rows), (len(inputs), len(csv_rows))
    strata = {}
    for r in inputs:
        c = csv_rows[r['id']]
        key = (r['set'], r.get('mode'), round(r['step_ms'], 3), c['frozen_status'],
               c['frozen_reason'] if c['frozen_status'] == 'refused' else '')
        strata.setdefault(key, []).append(r)
    rnd = random.Random(SEED)
    chosen = []
    for key in sorted(strata, key=str):
        rows = sorted(strata[key], key=lambda r: r['id'])
        chosen += rnd.sample(rows, min(PER_STRATUM, len(rows)))
    # frozen2 run now vs rows.csv: every chosen row, exact
    for r in chosen:
        o, c = run(m, r), csv_rows[r['id']]
        assert o['status'] == c['frozen_status'], r['id']
        assert o['reason'] == c['frozen_reason'], r['id']
        if o['status'] != 'refused':
            for k, ck in (('edt', 'frozen_edt'), ('edt_lo', 'frozen_edt_lo'), ('edt_hi', 'frozen_edt_hi')):
                assert o[k] == float(c[ck]), (r['id'], k, o[k], c[ck])
    n_round2 = len(chosen)
    chosen += crafted(m)
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    blob = bytearray()
    lines = []
    for r in chosen:
        b = np.asarray(r['bins'], float).ravel()
        nz = np.nonzero(b)[0]
        if len(nz) * 12 < len(b) * 8:
            enc = 'sparse'
            data = b''.join(struct.pack('<Id', int(i), float(b[i])) for i in nz)
        else:
            enc, data = 'dense', b.astype('<f8').tobytes()
        o = run(m, r)
        lines.append(json.dumps(dict(
            id=r['id'], set=r['set'], mode=r.get('mode'), step_ms=r['step_ms'], dt=r['dt'],
            t_arrival=r['t_arrival'], half_width=(r['meta'] or {}).get('half_width'),
            n=len(b), enc=enc, offset=len(blob), bytes=len(data),
            expect=dict(status=o['status'], reason=o['reason'], edt=o['edt'], edt_lo=o['edt_lo'], edt_hi=o['edt_hi']))))
        blob += data
    (out / 'manifest.jsonl').write_text('\n'.join(lines) + '\n', newline='\n')
    (out / 'bins.bin').write_bytes(bytes(blob))
    parsed = [json.loads(l) for l in lines]
    reasons = sorted({p['expect']['reason'] for p in parsed if p['expect']['status'] == 'refused'})
    print(f'frozen2 sha256 {sha}; {len(lines)} rows ({n_round2} round 2, {len(lines) - n_round2} crafted); '
          f'bins.bin {len(blob) / 1e6:.2f} MB, manifest {sum(map(len, lines)) / 1e6:.2f} MB; refusal reasons: {reasons}')


if __name__ == '__main__':
    main()
