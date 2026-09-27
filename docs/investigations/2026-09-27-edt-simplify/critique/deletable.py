"""Which parts change no outcome? Counts over this critique's own scans + the eval's pickles."""
import sys, json, re, collections, pickle, math
sys.dont_write_bytecode = True
SP = 'C:/Users/Burhan/AppData/Local/Temp/claude/b--repos-I-Simpa-Night-Mode/97f2c13c-beea-4d4b-b842-603feb756e57/scratchpad/edtsimp'
rows = []
for f in ('scan_i12_R0.31.json', 'scan_i4.json', 'scan_i4b.json'):
    for r in json.load(open(f)):
        rows.append(r['res'])
for s in ('t1', 'z3', 'real', 'ism'):
    d = pickle.load(open(f'{SP}/results_{s}.pkl', 'rb'))['results']
    by = collections.defaultdict(dict)
    for r in d:
        if r['method']:
            by[r['id']][r['method']] = (r['edt'], r['edt_lo'], r['edt_hi'], r['status'], str(r['reason']))
    rows += list(by.values())
print('rows considered:', len(rows))
mr = collections.Counter(r['minimal'][4] for r in rows if 'minimal' in r)
print('minimal reasons:', dict(mr))
lb_r2_only = 0; lb_ok = 0; lb_n = 0
for r in rows:
    if 'leanband' not in r: continue
    reason = r['leanband'][4]; lb_n += 1
    m = re.search(r'r2=([-\d.]+);n=(\d+);halfwidth_pct=([\d.]+)', reason)
    if m:
        r2, hw = float(m.group(1)), float(m.group(3))
        if hw <= 5.0 and r2 < 0.5:
            lb_r2_only += 1
print('leanband rows where r2>=0.5 is the ONLY thing standing between the row and ok:', lb_r2_only, 'of', lb_n)
lr = collections.Counter(r['leanband'][4].split(';')[0] if r['leanband'][3] == 'refused' else r['leanband'][3] for r in rows if 'leanband' in r)
print('leanband statuses/refusals:', dict(lr))
pr = collections.Counter((r['practice'][3] if r['practice'][3] != 'refused' else 'refused:' + r['practice'][4].split(' ')[0]) for r in rows if 'practice' in r)
print('practice statuses/refusals:', dict(pr))
