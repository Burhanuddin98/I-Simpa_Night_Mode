"""W1G at POS200's 200 positions in the noise-free specular model of tutorial 1's room
(bracket-skeptic/synth.py; octaves 125 Hz-4 kHz plus 8 and 16 kHz), truth the shipped midpoint at
0.02 ms (converged), exact rebins to 1-5 ms. Usage: python eval_pos200.py [workers] -> pos200_rows.pkl"""
import glob, os, pickle, sys, time
from concurrent.futures import ProcessPoolExecutor
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import eval_synth as E  # noqa: E402
import numpy as np  # noqa: E402
S, synth, C = E.S, E.synth, E.C

def work(item):
    pos, band = item
    S.ROOMS['T1pos'] = ((6, 10, 3), (3.0, 5.0, 1.8), [0.2, 0.2, 0.2, 0.2, 0.1, 0.3], 1.0, {band: synth.m_air(band)})
    return E.work(('T1pos', band, 0.31, 'pos200', np.array(pos)))

if __name__ == '__main__':
    w = int(sys.argv[1]) if len(sys.argv) > 1 else 4
    run = C.load_solve(glob.glob(C.POS + '/runs/*/solve')[0])
    items = [(r['pos'], band) for r in run['recs'] for band in (125, 500, 1000, 2000, 4000, 8000, 16000)]
    t0 = time.time(); rows = []
    with ProcessPoolExecutor(w) as ex:
        for rr in ex.map(work, items, chunksize=4):
            rows += rr
    pickle.dump(rows, open(os.path.join(HERE, 'pos200_rows.pkl'), 'wb'))
    print(len(rows), 'rows', round(time.time() - t0), 's')
