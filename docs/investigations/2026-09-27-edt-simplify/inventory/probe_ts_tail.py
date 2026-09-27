import json
from collections import Counter

d = json.load(open('B:/repos/I-Simpa_Night_Mode/target/agents/edt-band/gate4b_real_check.json'))
rows = d['rows']
print('total rows', len(rows))
print('ts_refused counts:', Counter(r.get('ts_refused') for r in rows))
print('edt_refused counts:', Counter(r.get('edt_refused') for r in rows))
print('num rows with ts_tail_widening key present:', sum(1 for r in rows if 'ts_tail_widening' in r))
print('num rows with ts_tail_widening not None:', sum(1 for r in rows if r.get('ts_tail_widening') is not None))
vals = [r.get('ts_tail_widening') for r in rows if r.get('ts_tail_widening') is not None]
print('num nonzero ts_tail_widening:', sum(1 for v in vals if v != 0.0))
print('max ts_tail_widening:', max(vals) if vals else None)
print('num rows with ts_inside not None:', sum(1 for r in rows if r.get('ts_inside') is not None))
print('num rows with ts_band not None:', sum(1 for r in rows if r.get('ts_band') is not None))
print('num rows with ts_band None but ts_refused None:', sum(1 for r in rows if r.get('ts_band') is None and r.get('ts_refused') is None))

# sample a row with ts_band not None
for r in rows:
    if r.get('ts_band') is not None:
        print('sample row with ts_band:', {k: r[k] for k in ('key','ts_refused','ts_tail_widening','ts_tau','ts_band','ts_inside','tail_max','f_end','kill_frac')})
        break

# sample a row with ts_band None
for r in rows:
    if r.get('ts_band') is None:
        print('sample row with ts_band None:', {k: r.get(k) for k in ('key','ts_refused','ts_tail_widening','ts_tau','ts_band','ts_inside','tail_max','f_end','kill_frac')})
        break
