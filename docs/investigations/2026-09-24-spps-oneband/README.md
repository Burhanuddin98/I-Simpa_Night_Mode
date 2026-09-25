# `spps_oneband`: SPPS reads a short spectrum past its end (2026-09-24)

The receipt behind `docs/upstream-findings.md`, 4, and the `UNREPRODUCIBLE` tolerance in
`tools/fixture-gen/mkexpected.py`. Copied here on 2026-09-25 from
`target/agents/fu-receipts-scratch/`, which the tutorial-3 follow-ups' report listed as deletable
(the follow-ups' critic).

| File | What |
|---|---|
| `oneband.py` | runs the fixture `tests/fixtures/runs/spps_oneband` N times with one `spps.exe` and seed 1: each in a fresh folder, then N times in one reused folder, then N times with the source's spectrum complete (the control); per run the 1000 Hz particle statistics and the band's `Total energy.recp` summed |
| `oneband_same.py` | runs it N more times (200) in the one reused folder |
| `oneband-30.txt` | `oneband.py` at N = 30, then (the second block, all zero) the N = 0 pass `oneband_same.py` makes to import it |
| `oneband-same-200.txt` | `oneband_same.py` at N = 200 |
| `oneband-bin.json` | that N = 0 pass's empty result file, kept as it was written |

**The count** (corrected 2026-09-25): 260 runs without the complete spectrum, 7 of them with a
positive power read for the 1000 Hz band: 1 of 30 in fresh folders, 0 of 30 in the reused folder
(`oneband-30.txt`, "same"), 6 of 200 more in that folder (`oneband-same-200.txt`). The first
write-up counted 7 of 230, leaving out the 30-run batch in the reused folder. The control gave the
second outcome's statistics in 30 of 30.
