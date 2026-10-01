"""The attack kit (HARNESS-PLAN.md P25, P26; PREREG.md:42, :60): the sandbox, the class schema, the
draws, the judges' prompt and the panel rule.

STUB IN PART (HARNESS-PLAN.md section 6, step 3): every function returns None until the attack-kit
step, except parse_vote and panel, built in step 6 with the scorer because T20 calls them and
score.h6 rests on them. Tests T17 and T20 (the panel, through score.h6) hold the contract below.

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

Where parse_vote's contract is silent, it reads "anything else" strictly, since a vote that does not
parse counts as plausible and so can only fail H6, never pass it (P26): the text must be one JSON
object with exactly the keys 'verdict' and 'reasons', no key twice; the verdict exactly 'plausible'
or 'implausible'; the reasons a string that is not blank ("with reasons", P26). A raw vote that is
not a str, bytes included, is None. panel takes a list or tuple of raw votes; anything else, None
included, is 'plausible'. Three copies of one text count as three votes: the panel cannot tell
judges apart, and the harness gives it one vote per judge.
"""
import json
from pathlib import Path

SANDBOX = Path(r'C:\tmp\m8b-edt\attack')
SANDBOX_FILES = ('INTERFACE.md', 'PHYSICS.md', 'generators.py', 'method.py')
LIMITS = {'t60_s': (0.1, 10.0), 'drr_db': (-40.0, 30.0)}     # P25, PHYSICS.md's hard limits
MAX_CLASSES = 30
DRAWS = 20
REPRODUCIBLE = 5                    # P26: 5 or more of 20 draws wrong-silent
VERDICTS = ('plausible', 'implausible')
PANEL_SIZE = 3                      # P26: three judges, implausible only 3 of 3


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


def _object_once(pairs):
    keys = [k for k, _ in pairs]
    if len(set(keys)) != len(keys):
        raise ValueError('a key appears twice: %s' % keys)
    return dict(pairs)


def parse_vote(raw):
    if not isinstance(raw, str):
        return None
    try:
        obj = json.loads(raw, object_pairs_hook=_object_once)
    except (ValueError, RecursionError):        # not JSON, or a key twice
        return None
    if not isinstance(obj, dict) or set(obj) != {'verdict', 'reasons'}:
        return None
    verdict, reasons = obj['verdict'], obj['reasons']
    if not isinstance(verdict, str) or verdict not in VERDICTS:
        return None
    if not isinstance(reasons, str) or not reasons.strip():
        return None
    return verdict


def panel(raw_votes):
    if not isinstance(raw_votes, (list, tuple)) or len(raw_votes) != PANEL_SIZE:
        return 'plausible'
    if all(parse_vote(v) == 'implausible' for v in raw_votes):
        return 'implausible'
    return 'plausible'
