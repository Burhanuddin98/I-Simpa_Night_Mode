# Sentinel check of FINAL.md (2026-09-27 06:01)

An independent Sonnet sentinel checked FINAL.md against its logs before its numbers went to Burhan.

- **Tables:** 48 cells checked against `final/eval_final.txt` and `eval/EVAL.md`; 0 mismatches.
- **Version:** eval_final.txt, res_*.pkl and scans_all.log were produced by the current `final/method.py` (05:47:22); it was not edited afterwards.
- **Recount from res_*.pkl** (wrong-silent / truth outside range): t1 0/0, z3 4/18, real 0/570, ISM 0/1. Matches FINAL.md.
- **Section 3/5 claims:** 11 confirmed. One figure has no receipt: "C-R3 sigma about 2.3 %". The 13 of 19 count is confirmed.
  - The z3 ball-size explanation was not checked.
  - The Z=3 ISM figures (871 to 613) come from `dev/ablate_eval.log`, not from a Z=3 rerun of the ISM set.
- **Upstream line claims:** confirmed. GetTimeRange `:143-170`; GetTimeDecay `:127-139`, used in C/D/Ts and never in Compute_TR_Param; EDT at `:936`. The +0.3 % at 2 m and +20 % at 20 m figures are in READING.md, both analytic and simulated.
- **Held-out data: NONE.** The 20 % block size was chosen on the real seed runs (V-R6 refusals, `final/dev/tail_blocks.py:44-56`), and ISM fed the Z choice. **Every corpus in the repo has touched threshold selection or the Z decision, so the graft's 0 wrong-silent is in-sample.**
- **Decorative:** upstream's usable/ok of 100 % holds by construction (ok whenever the value is finite). Read its wrong-silent count instead.

**Consequence:** before the graft is adopted, it needs fresh data it has never seen (new rooms, new seeds, ideally new SPPS runs) and an independent attacker.
