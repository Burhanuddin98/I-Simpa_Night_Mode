"""Round 2's constants, in one place (HARNESS-PLAN-2.md sections 1, 3, 4 and 7, with section 9).

harness2/ is round 1's harness edited in place for round 2 only (section 9 M5): there is no `round_` argument and
no table of two rounds. What a module needs of round 2 that is a plain value lives here, so that a reviewer can
read the seeds, the folders and the H4 rule on one page. Nothing here opens a file or runs anything.
"""
from pathlib import Path

from . import corpus

INVESTIGATION = corpus.INVESTIGATION
SCRATCH = Path(r'C:\tmp\m8b-edt')

# ---- the freeze (P1) ------------------------------------------------------------------------------------------
ADDENDUM = 'docs/investigations/2026-09-27-edt-heldout/ADDENDUM-B1.md'      # relative to the repo
PREVIEW_PIN = INVESTIGATION / 'preview_pin.json'                               # section 9 M3

# ---- seeds (section 3) ----------------------------------------------------------------------------------------
TESTED_SEEDS = {(1.0, 150000): (3101, 3102, 3103), (2.0, 150000): (3201, 3202, 3203), (5.0, 150000): (3501, 3502, 3503),
                (1.0, 50000): (4101, 4102, 4103), (2.0, 50000): (4201, 4202, 4203), (5.0, 50000): (4501, 4502, 4503)}
TRUTH_SEEDS = (9101, 9102, 9103, 9104)
ISM_SEED = 2026100201
SYNTH_SEED = 2026100202
ATTACK_SEED = 2026100203
RESERVED_SEEDS = (9998, 9999)                    # the probes', unchanged: still refused
# round 1's held-out seeds (HARNESS-PLAN.md P12, P21, P24): never used again, in any guard
ROUND1_SEEDS = (1101, 1102, 1103, 1201, 1202, 1203, 1501, 1502, 1503, 2101, 2102, 2103, 2201, 2202, 2203, 2501, 2502, 2503,
                9001, 9002, 9003, 9004, 2026100101, 2026100102)

# ---- paths (P36; the build brief) ------------------------------------------------------------------------------
ROOMS_ROOT = SCRATCH / 'round2-rooms'            # room projects: rooms.py refuses B:
DATA_ROOT = Path(r'B:\data\m8b-edt\round2\heldout')              # the 172 run folders
RESULTS_ROOT = Path(r'B:\data\m8b-edt\round2\results')           # scoring output
SENTINEL = RESULTS_ROOT / 'SENTINEL.md'          # written by the sentinel seat; the attack waits for it
ATTACK_SANDBOX = SCRATCH / 'attack2'
LAUNCH_LOG = SCRATCH / 'round2-launch.log'
PROGRESS_LOG = SCRATCH / 'round2-progress.log'
# round 1's folders: refused as round-2 roots, in the driver and the scorer
ROUND1_ROOTS = (Path(r'B:\data\m8b-edt\heldout'), Path(r'B:\data\m8b-edt\results'), SCRATCH / 'heldout',
                SCRATCH / 'heldout-rooms')

# ---- parallelism (P35) ------------------------------------------------------------------------------------------
MAX_WORKERS = 4                                  # SPPS queue; Burhan, after a machine reset under 8
ISM_WORKERS = 2                                  # 7.6 GB per large echogram

# ---- H4 (PREREG-2.md, P30') -------------------------------------------------------------------------------------
H4_R_MAX_M = 0.5
H4_REFUSAL = 'receiver_too_large'
