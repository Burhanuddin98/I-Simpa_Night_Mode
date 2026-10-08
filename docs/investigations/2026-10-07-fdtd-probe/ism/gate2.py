"""PREREG-5 gate 2: the ISM against the closed-form shoebox image lattice, orders <= 3.

Pass: the same multiset of image positions (to 1e-9 m), hence path lengths, and per-order counts, by the
exhaustive generator and by the ray-guided generator (which must also find every one).
"""
import itertools, sys, time

import numpy as np

from ism import Room

L = np.array([7.3, 5.1, 3.2])
S = np.array([1.9, 2.2, 1.4]); R = np.array([5.6, 3.9, 1.1])
ORDER = 3

# 12 triangles, a few split unevenly so a plane has non-trivial triangulation
V = np.array(list(itertools.product([0, L[0]], [0, L[1]], [0, L[2]])), float)


def quad(i, j, k, l):
    return [[i, j, k], [i, k, l]]


F = np.array(quad(0, 1, 3, 2) + quad(4, 6, 7, 5) + quad(0, 4, 5, 1) + quad(2, 3, 7, 6) + quad(0, 2, 6, 4) + quad(1, 5, 7, 3))
G = np.arange(len(F)) // 2
room = Room(V, F, G)
assert room.P == 6, room.P

# closed form: per axis, x = (1-2q) s + 2 n L, reflections |2n - q|
ref = []
for axes in itertools.product(*[[(q, n) for q in (0, 1) for n in range(-2, 3)] for _ in range(3)]):
    o = sum(abs(2 * n - q) for q, n in axes)
    if o <= ORDER:
        ref.append((o, np.array([(1 - 2 * q) * S[a] + 2 * n * L[a] for a, (q, n) in enumerate(axes)])))


def key(o, x):
    return (o,) + tuple(np.round(x, 9))


ref_set = sorted(key(o, x) for o, x in ref)
ok_all = True
for name, seqs in [('exhaustive', room.exhaustive(S, ORDER)),
                   ('ray-guided', None)]:
    t0 = time.time()
    if seqs is None:
        rs = room.ray_sequences(S, ORDER, 20000)
        seqs = [np.zeros((1, 0), int)] + [np.array(sorted(s for s in rs if len(s) == k), int).reshape(-1, k)
                                          for k in range(1, ORDER + 1)]
    got = []
    for seq in seqs:
        if seq.shape[1] == 0:
            if not room.blocked(S[None], R[None])[0]:
                got.append(key(0, S))
            continue
        ok, Lp, img = room.validate(S, R, seq)
        got += [key(seq.shape[1], img[i]) for i in np.where(ok)[0]]
    got = sorted(got)
    cnt = lambda s: [sum(1 for x in s if x[0] == o) for o in range(ORDER + 1)]
    same = got == ref_set
    ok_all &= same
    lens_ref = sorted(np.linalg.norm(np.array(x[1:]) - R) for x in ref_set)
    lens_got = sorted(np.linalg.norm(np.array(x[1:]) - R) for x in got)
    dl = max(abs(a - b) for a, b in zip(lens_ref, lens_got)) if len(lens_ref) == len(lens_got) else float('nan')
    print(f'{name:10s} counts per order {cnt(got)} vs closed form {cnt(ref_set)}  identical: {same}  '
          f'max |dL| {dl:.2e} m  ({time.time() - t0:.1f} s)')
print('GATE 2', 'PASS' if ok_all else 'FAIL')
sys.exit(0 if ok_all else 1)
