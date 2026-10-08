"""PREREG-5 gate 1: the untouched copy's `simpa results` equals the original run's, value by value."""
import json, sys
a = json.load(open(sys.argv[1])); b = json.load(open(sys.argv[2]))
K = ['edt_s', 'c80_db', 't30_s', 'c50_db', 'spl_db']
n = bad = 0; mx = {k: 0.0 for k in K}
for ra, rb in zip(a['spps']['point_receivers'], b['spps']['point_receivers']):
    for pa, pb in zip(ra['per_source'], rb['per_source']):
        for ba, bb in zip(pa['bands'], pb['bands']):
            for k in K:
                va = (ba['parameters'].get(k) or {}).get('value'); vb = (bb['parameters'].get(k) or {}).get('value')
                n += 1
                if va is None or vb is None:
                    bad += (va is None) != (vb is None); continue
                mx[k] = max(mx[k], abs(va - vb))
ok = bad == 0 and all(v == 0 for v in mx.values())
print(f'GATE 1 {"PASS" if ok else "FAIL"}: {n} values, None mismatches {bad}, max |diff| {mx}')
