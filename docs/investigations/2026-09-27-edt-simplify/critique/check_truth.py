import sys, math; sys.dont_write_bytecode = True
sys.path.insert(0, 'B:/repos/I-Simpa_Night_Mode/target/agents/edt-band')
import numpy as np, synth as S, test_band_early as TB
# compare my numeric truth with the band's closed-form expo_truth (single slope, gap)
for T60, gap, Edf in ((0.5, 0.0, 0.5), (0.5, 2.5e-3, 0.5), (1.0, 5e-3, 3.0), (2.0, 0.0, 0.1)):
    k = 6 * math.log(10) / T60; ta = 0.01234; tr = ta + gap
    A = 1.0; Sr = A / k * math.exp(-k * tr)
    Ed = Edf * Sr
    ref = TB.expo_truth(ta, Ed, A, k, tr)['edt']
    # my param: density A' exp(-k (t - t_r)), A' = A exp(-k tr)
    mine = S.truth_edt(ta, Ed, gap, [A * math.exp(-k * tr)], [k])
    print(T60, gap, Edf, ref, mine, mine / ref - 1)
