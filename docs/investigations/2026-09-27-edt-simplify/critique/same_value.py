"""Is LeanBand's point value cand_minimal's point value? Over every eval row where both report one;
plus, from real_seeds-style runs, which cells LeanBand's real-seed wrong-silent rows sit in."""
import pickle, collections, sys
sys.dont_write_bytecode = True
SP = 'C:/Users/Burhan/AppData/Local/Temp/claude/b--repos-I-Simpa-Night-Mode/97f2c13c-beea-4d4b-b842-603feb756e57/scratchpad/edtsimp'
mx = 0.0
n = 0
for s in ('t1', 'z3', 'real', 'ism'):
    d = pickle.load(open(SP + '/results_' + s + '.pkl', 'rb'))['results']
    by = collections.defaultdict(dict)
    for r in d:
        if r['method'] in ('minimal', 'leanband'):
            by[r['id']][r['method']] = r['edt']
    for v in by.values():
        if v.get('minimal') is not None and v.get('leanband') is not None:
            n += 1
            mx = max(mx, abs(v['minimal'] / v['leanband'] - 1))
print('eval rows where both report an EDT:', n, ' max relative difference:', mx)
