"""Re-run the evaluator's own ISM sample (loaders.load_ism, 160 receivers x 6 bands x 1/2 ms) through
LeanBand as submitted and through fix_probe.probe_edt, against the evaluator's own truth.
Read-only on every input, single process."""
import sys, collections, math, warnings
sys.dont_write_bytecode = True
warnings.filterwarnings('ignore')
SP = 'C:/Users/Burhan/AppData/Local/Temp/claude/b--repos-I-Simpa-Night-Mode/97f2c13c-beea-4d4b-b842-603feb756e57/scratchpad/edtsimp'
sys.path.insert(0, SP)
import wrappers  # noqa: F401  (as run_eval.py does: sets up the import paths the ISM loader needs)
import loaders as L

rows = L.load_ism(n_tasks=160, steps_ms=(1.0, 2.0))   # before importing this folder's common.py:
sys.modules.pop('common', None)                        # the ISM loader imports bracket/common.py
sys.path.insert(0, 'B:/repos/I-Simpa_Night_Mode/target/agents/edt-simplify/critique')
from common import CANDS, score  # noqa: E402
from fix_probe import probe_edt  # noqa: E402
tal = {'leanband': collections.Counter(), 'probe': collections.Counter()}
bad = {'leanband': [], 'probe': []}
for r in rows:
    if r.get('bins') is None or r['truth_edt'] is None or not math.isfinite(r['truth_edt']):
        continue
    for nm, res in (('leanband', CANDS['leanband'].analyse(r['bins'], r['dt'], r['t_arrival'], {})),
                    ('probe', probe_edt(r['bins'], r['dt'], r['t_arrival']))):
        kind, e = score(res, r['truth_edt'])
        tal[nm][kind] += 1
        if kind == 'ok_WRONG':
            bad[nm].append((e, r['id']))
for nm in tal:
    print(nm, dict(tal[nm]))
    for e, i in sorted(bad[nm], key=lambda x: -abs(x[0]))[:6]:
        print('   %+.1f%%  %s' % (100 * e, i))
