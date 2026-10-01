"""The attack kit (HARNESS-PLAN.md P25, P26; PREREG.md:42, :60): the sandbox, the class schema, the
draws, the judges' prompt and the panel rule.

STUB (HARNESS-PLAN.md section 6, step 3): every function returns None. Tests T17 and T20 (the
panel, through score.h6) hold the contract below.

Contract:
- A class is JSON (P26): {"generator": "synth" | "ism", "params": {name: [lo, hi], ...},
  "noise": {"kind": "none"} or {"kind": "compound_poisson", "particles_per_source": [lo, hi]},
  "step_ms": x, "run_over_t60": [lo, hi], "R_m": [lo, hi]}. Synth parameter names are
  synth_fresh's spec names ('t60_s', 'drr_db', 'd_m', 'gap_ms', 'delay_ms', 'ratio',
  'late_share_db').
- validate_class(cls) -> (True, None), or (False, reason) when it is malformed or out of bounds;
  the reason names the offending key. Bounds are PHYSICS.md's hard limits (LIMITS: T60 0.1-10 s,
  DRR -40 to +30 dB) and what the product accepts.
- class_sha256(cls) -> str: the sha256 of the class's canonical JSON (keys sorted), so the same
  class written in another key order has the same hash.
- draws(cls, n=DRAWS) -> [instance]: n instances seeded by class_sha256(cls) alone, each inside
  the class's box: a dict with a value for every name in cls['params'], and 'R_m', 'run_over_t60'
  and 'step_ms'. The harness computes their truth itself (P26).
- physics_md() -> str: PHYSICS.md (SPPS histogram physics and the hard limits).
- build_sandbox(dest) -> [path]: writes exactly SANDBOX_FILES into dest, no folder: method.py
  (frozen/method.py, byte for byte), INTERFACE.md, PHYSICS.md and generators.py (the synth, ISM
  and compound-Poisson generators with the product's source delay, importing nothing from the
  harness or target/).
- judge_prompt(class_files) -> str: for {file name: JSON text}, the judge's instructions, every
  class file verbatim and PHYSICS.md verbatim, and no result field; ValueError when a class file
  holds one.
- parse_vote(raw) -> 'plausible' | 'implausible' | None: a judge's answer for one class is JSON
  text {"verdict": "plausible" | "implausible", "reasons": "..."}; anything else, or None, is None.
- panel(raw_votes) -> 'implausible' only when there are exactly three votes and all three parse as
  'implausible'; 'plausible' otherwise (a split, a missing vote or one that does not parse).
"""
from pathlib import Path

SANDBOX = Path(r'C:\tmp\m8b-edt\attack')
SANDBOX_FILES = ('INTERFACE.md', 'PHYSICS.md', 'generators.py', 'method.py')
LIMITS = {'t60_s': (0.1, 10.0), 'drr_db': (-40.0, 30.0)}     # P25, PHYSICS.md's hard limits
MAX_CLASSES = 30
DRAWS = 20
REPRODUCIBLE = 5                    # P26: 5 or more of 20 draws wrong-silent


def validate_class(cls):
    return None


def class_sha256(cls):
    return None


def draws(cls, n=DRAWS):
    return None


def physics_md():
    return None


def build_sandbox(dest):
    return None


def judge_prompt(class_files):
    return None


def parse_vote(raw):
    return None


def panel(raw_votes):
    return None
