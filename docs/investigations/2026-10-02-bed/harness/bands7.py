"""Seven-band projects for STI (../ADDENDUM-2-STI.md): a 6-octave bed project (125 Hz-4 kHz) extended to the seven
octaves 125 Hz-8 kHz that IEC 60268-16 needs, consistently in every per-band array.

The bed's rooms use frequency-flat materials, so 8 kHz takes the 4 kHz absorption and scattering (asserted flat).
Sources and receivers carry a spectrum *shape* (`white`), not per-band arrays; `check` refuses any other shape, so a
per-band array cannot be missed silently. `check(p6, p7)` walks both projects and accepts only lists that grew by
one trailing element whose first six are unchanged; anything else is reported.
"""
import copy
import json
import subprocess

SIX = [125, 250, 500, 1000, 2000, 4000]
SEVEN = SIX + [8000]


def to_seven(p6):
    p = copy.deepcopy(p6)
    assert p['bands']['kind'] == 'octave' and p['bands']['frequencies_hz'] == SIX, p['bands']
    p['bands']['frequencies_hz'] = list(SEVEN)
    for m in p['materials']:
        for k in ('absorption', 'scattering'):
            v = m[k]
            assert len(v) == 6 and len(set(v)) == 1, (m['name'], k, v)     # frequency-flat
            m[k] = v + [v[-1]]
    for s in ('spps', 'tcr'):
        bc = p['solvers'][s]['bands_computed']
        assert bc == [True] * 6, (s, bc)
        p['solvers'][s]['bands_computed'] = bc + [True]
    for x in p['sources']:
        assert x['power']['shape']['kind'] == 'white', x['power']
    for r in p['point_receivers']:
        assert r['background_noise']['shape']['kind'] == 'white', r['background_noise']
    return p


def check(p6, p7, path='$'):
    """Differences between p6 and p7 other than one appended band: [] when p7 is p6 plus the 8 kHz band only."""
    out = []
    if isinstance(p6, dict) and isinstance(p7, dict):
        for k in sorted(set(p6) | set(p7)):
            if k not in p6 or k not in p7:
                out.append('%s.%s present in one only' % (path, k))
            else:
                out += check(p6[k], p7[k], '%s.%s' % (path, k))
    elif isinstance(p6, list) and isinstance(p7, list):
        if len(p7) == len(p6):
            for i, (a, b) in enumerate(zip(p6, p7)):
                out += check(a, b, '%s[%d]' % (path, i))
        elif len(p6) == 6 and len(p7) == 7 and p7[:6] == p6:
            pass
        else:
            out.append('%s: %r -> %r' % (path, p6, p7))
    elif p6 != p7:
        out.append('%s: %r -> %r' % (path, p6, p7))
    return out


def validate(simpa, proj_path):
    """`simpa validate --json`: (exit code, parsed output or raw text)."""
    r = subprocess.run([simpa, 'validate', str(proj_path), '--json'], capture_output=True, text=True)
    try:
        out = json.loads(r.stdout)
    except ValueError:
        out = r.stdout + r.stderr
    return r.returncode, out
