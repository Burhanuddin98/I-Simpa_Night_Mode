# ADDENDUM-B2: the H6 attack runner, written after B1 and after H1-H5 were scored

Dated 2026-10-02, branch `m8b-edt`. Same form as ADDENDUM-B1.md.

## What this is, and when it was written

The code in `attack2_run/` was written **after ADDENDUM-B1 was committed and after H1-H5 were scored**, and **before any attack
class was run**. `main()` has not been run on c01-c11; only its unit tests ran, on planted and synthetic inputs.

## Why

The gap: harness2 holds the attack kit (`attack.validate_class`, `draws`, `panel`, `score.h6`) but nothing that turns an attack draw
into a scored input row, so round 1 never reached H6 and round 2 could not either. `attack2_run/run_h6.py` is that piece:
`draw_to_row` (the row Synth-fresh-2 builds, through synth_fresh's own `make_spec`, `histogram` and `truth`; the delayed source
emits at the next whole step and `t_arrival` = emission + d/c is the product's arrival_s), the corpus's compound-Poisson noise at
the drawn particle count, the judge-file vote parser, and `main()`: 20 draws per class, frozen2 analyse (through `method.load`,
hash pin), wrong-silent count, `score.h6`, and the files `rows.csv`, `h6.json`, `H6.md` under
`B:\data\m8b-edt\round2\results\attack\` (never overwritten).

Choices a reader should know (details in `run_h6.py`'s docstring):
- Parameters a class does not name (ratio, late_share_db, gap_ms) take single-slope defaults: ratio 1.0, late_share_db -300, gap 0.
  c01-c11 name only t60_s, drr_db, d_m and delay_ms.
- Noise seed: `SeedSequence([int(class_sha256, 16), ATTACK_SEED, draw_index, 0x6e6f697365])`; weights as the corpus's scan 3.
- An ISM class raises NotImplementedError (none exists among c01-c11).
- The attacker's note on c01 ("assumes the harness passes nominal t_arrival = d/c, not the delayed one") is not honoured: the
  row carries the product's delayed arrival_s, as Synth-fresh-2 does.
- A draw whose truth is NaN (DRR above 9.54 dB) is not counted wrong-silent; none of c01-c11 reaches that DRR.

## It changes no B1-hashed file

Re-run after the code was written and committed (cwd `harness2/`):
- `python -m m8b.freeze2 --check ../ADDENDUM-B1.md` gave **freeze check: every hash reproduced**
- `python -m m8b.corpus2 --check` gave **pin check: all reproduced**
- `run_h6.verify_frozen()` (the method.load pin and the freeze check together) passes; frozen2/method.py sha256
  `029d90ac5e8f6a634a51a7ffce136cbd4c0b7ea3d3ad8a18b78fd8240ff224a0`.

Nothing under `harness2/`, `frozen2/`, `harness/` or `frozen/` was edited; `run_h6.py` imports them read-only and `main()` runs
`verify_frozen()` before anything else.

## Tests

`attack2_run/test_run_h6.py`, 8 tests, recorded failing then passing in `attack2_run/RED-GREEN.md`.

## Hashes of attack2_run/ (sha256, line ends made LF as freeze2 does; this file cannot hash itself)

| file | sha256 |
|---|---|
| attack2_run/RED-GREEN.md | a70bf905a1e68f898d63a87847c92a842bf1771c14175631e5a9e61a64e3b3ef |
| attack2_run/run_h6.py | 708c84744fca4a277050c940e881dd35d6f811f416fe00b6ffa5f3eaef22d4a5 |
| attack2_run/test_run_h6.py | d9a79b0483fbf0669855239ec89c04372dd562c0c79caac581d689e16afe59b6 |
