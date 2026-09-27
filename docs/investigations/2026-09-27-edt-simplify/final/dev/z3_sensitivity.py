"""Z = 3 instead of 2 in final/method.py: re-run the noise scan and the real seeds (run_scans.py i5/seeds)."""
import sys, runpy
sys.dont_write_bytecode = True
sys.path.insert(0, 'B:/repos/I-Simpa_Night_Mode/target/agents/edt-simplify/final')
import method
method.Z = 3.0
for w in ('i5', 'seeds'):
    sys.argv = ['run_scans.py', w + '_Z3dummy']
    sys.argv = ['run_scans.py', w]
    runpy.run_path('B:/repos/I-Simpa_Night_Mode/target/agents/edt-simplify/final/run_scans.py', run_name='__main__')
