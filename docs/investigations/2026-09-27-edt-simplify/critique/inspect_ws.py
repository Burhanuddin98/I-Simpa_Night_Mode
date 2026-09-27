"""Read-only: list wrong-silent rows per candidate per set from the evaluator's pickles."""
import pickle, sys, collections, math
sys.dont_write_bytecode = True
SP = 'C:/Users/Burhan/AppData/Local/Temp/claude/b--repos-I-Simpa-Night-Mode/97f2c13c-beea-4d4b-b842-603feb756e57/scratchpad/edtsimp'
for s in ('t1', 'z3', 'real', 'ism'):
    d = pickle.load(open(f'{SP}/results_{s}.pkl', 'rb'))['results']
    for m in ('minimal', 'leanband', 'practice'):
        ws = []
        for r in d:
            if r['method'] != m or r['status'] != 'ok' or r['edt'] is None: continue
            t = r['truth_edt']
            if t is None or not math.isfinite(t) or t <= 0: continue
            e = r['edt'] / t - 1
            if abs(e) > 0.05:
                ws.append((e, r['id'], r['edt'], t, r['edt_lo'], r['edt_hi'], r['reason']))
        ws.sort(key=lambda x: -abs(x[0]))
        sign = collections.Counter('neg' if w[0] < 0 else 'pos' for w in ws)
        print(f'== {s} {m}: {len(ws)} wrong-silent  {dict(sign)}')
        for w in ws[:8]:
            print('   err=%+.1f%% id=%s edt=%.4f truth=%.4f lo=%.4f hi=%.4f reason=%s' % (100*w[0], w[1], w[2], w[3], w[4], w[5], w[6]))
