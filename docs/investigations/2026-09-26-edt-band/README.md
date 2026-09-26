# EDT band: a model-free early-parameter band (EDT, Ts, C50, C80, D50)

Workflow `wf_83f875af-399` (2026-09-26, 01:20-07:22; resumed after a machine crash at about 06:46).
Burhan's order (2026-09-25 23:33): "IT NEEDS TO BE BETTER, FAR BETTER THAN THE I SIMPA EDT METHOD".

**The method.** Each quantity comes with an interval that contains its continuous-time (ISO) value for
every energy arrangement inside the bins that matches what SPPS recorded. It assumes no curve shape,
models SPPS's per-step air exactly, and anchors t = 0 at the known direct arrival. EDT uses a certified
outer bound; Ts, C and D are exact.

**Results:**

- **Adversary:** holds, with 0 counterexamples.
- **Fix round:** all attacks re-run with 0 violations.
- **Evaluation**, on 67,468 noise-free rows at steps from 0.1 to 5 ms:
  - EDT: 24,866 accepted at 2.5 %, 0 wrong. Upstream is wrong beyond 2.5 % on 41,865 (62 %); the shipped check
    has 2,583 wrong accepts; W1G has 39.
  - EDT and Ts contain the truth on every set, including 26,808 real SPPS rows.
  - C/D have 23 misses of at most 0.0015 L, from SPPS's f32 air factor. The fix is known and is not yet in `band_early.py`.

**Critic: not ready for the Rust build.** Five gates remain:

1. The f32 air-rate fix, put into the code.
2. The production inputs (tail bound and eps).
3. Noise and band evaluated together.
4. A Z4 golden parity corpus.
5. A measured Rust benchmark.

The decisions it lists are in `workflow-reports.json` (critic) and in `docs/scope.md`.

**Files:** `SPEC.md` (method and proof), `band_early.py` and `test_band_early.py` (the reference implementation),
`EVAL.md`, `eval_tables.txt` and `eval_summary.txt` (the evaluation), `critic_check.json`, `workflow-reports.json` (all five
agents' reports), and `fig/` (band against W1G and upstream on the Z3 cases). The large per-row JSONs (about 290 MB) stay in
`target/agents/edt-band/`.

Per the project CLAUDE.md, none of the upstream comparisons are claims for the README, the UI or marketing until arc item 4 has run.
