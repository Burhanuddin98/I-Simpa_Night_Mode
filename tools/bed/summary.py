"""beds/summary.json: each parameter's bed status, derived from the bed artifacts.

    python tools/bed/summary.py [--data B:/data] [--out beds/summary.json]

M12 PLAN.md, P1 item 1. The Results screen renders a parameter only when its status here is PASS
(gate (b)). This script reads the artifacts the result documents name as final, hashes each
(sha256), reads the commit each bed ran at from the artifact itself, and holds each parameter to
decision 39's product grade, check by check, as the artifacts state it:

- exact inputs (set A, STI set A): no answered value beyond the just-noticeable difference (for
  STI, beyond its 0.03 tolerance);
- default runs (sets B and C, STI B7 and C7): no wrong-silent value, and the scorer's own pass
  flag (coverage at least 90 %, every room answered at least 80 %: PREREG criteria 2 and 3);
- T30 also its own bed, M8a: the gate passed, no failures, not exploratory;
- EDT its held-out round 2 in Random mode, as VERDICT-2.md reads it: H1-H4 and H5 on the three
  real fresh sets from the scored summary, which left the attack set empty (SENTINEL-EDT.md), and
  H6 from the attacker run on the same frozen method (`attack/h6.json`);
- G and dB(A) set C's end-to-end summary (ADDENDUM-6), read as the other set summaries are.

A parameter is PASS when every check on every artifact holds. A parameter no final artifact
scores, an artifact that is missing or does not read, or a check that does not hold, makes it
FAIL with the reason named: nothing here is a judgement, and no status is written by hand.

One layer sits above the rule, and only where the decision log puts it: a ruling (RULINGS) that
names its exception exactly. It turns a FAIL into PASS only while the exception it names is the
only thing failing; the entry then says `by: "decision N"` and keeps the rule's own status and
reasons beside it (`rule`). Otherwise the rule's FAIL stands and the ruling says why it does not
apply. Every other entry says `by: "the rule"`.

Never edit beds/summary.json by hand: crates/simpa-core/tests/bed_summary.rs re-runs this script
and fails unless the committed file is its output byte for byte. Paths are recorded as they are on
Grace (`B:/data/...`, or relative to the repository) whatever `--data` says, so the output does
not depend on where the evidence was read from. A file in the repository is hashed with CRLF read
as LF, the bytes git stores (`core.autocrlf` rewrites the working copy); M8a's own
`report_sha256` is computed the same way.

Standard library only.
"""

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
DATA_PREFIX = "B:/data/"

RESULT = "docs/investigations/2026-10-02-bed/RESULT.md"
EDT_RESULT = "docs/investigations/2026-09-27-edt-heldout/VERDICT-2.md"
GDBA_RESULT = "docs/investigations/2026-10-02-bed/RESULT-GDBA.md"
EDT_SENTINEL = "docs/investigations/2026-10-03-m12/SENTINEL-EDT.md"
M8A_RESULT = "docs/investigations/2026-09-29-m8a/SPEC.md"
M12C_RESULT = "docs/investigations/2026-10-09-parity-refresh/README.md"

RULE = (
    "Decision 39's product grade, read from the artifacts: PASS when every check on every "
    "artifact named final holds. Exact inputs: no answered value beyond the just-noticeable "
    "difference (STI: beyond 0.03). Default runs: no wrong-silent value and the scorer's own "
    "pass flag (coverage >= 90 %, every room answered >= 80 %). T30 also M8a's gate; EDT its "
    "held-out round 2 in Random mode, H1-H5 from the scored summary on its real sets joined with "
    "H6 from the attacker run, as VERDICT-2.md reads it; G and dB(A) set C's end-to-end summary "
    "(ADDENDUM-6). A parameter no final artifact scores, a missing artifact or a check that does "
    "not hold: FAIL, with the reason. A decision-log ruling that names its exception exactly "
    "makes a FAIL PASS only while that exception is the only thing failing, and the entry says "
    "so (`by`, with the rule's status and reasons kept under `rule`)."
)

# ---- the artifacts ------------------------------------------------------------------------------

# key: (path as recorded, where it is read from, the section of its result document that cites it)
ARTIFACTS = {
    "m8a": (
        "beds/m8a-20260929T093134Z/summary.json",
        "repo",
        f"{M8A_RESULT} (the T30 bed); its summary, gate checks A-E",
    ),
    "m8a-report": (
        "beds/m8a-20260929T093134Z/report.json",
        "repo",
        f"{M8A_RESULT} section 6: report.json, hashed by the summary's report_sha256",
    ),
    "A": (
        "B:/data/m8b-bed/A/summary.json",
        "data",
        f"{RESULT}, Set A: exact inputs (the table, before build F)",
    ),
    "A-buildF-s1": (
        "B:/data/m8b-bed/A-buildF/s1/summary.json",
        "data",
        f"{RESULT}, Set A: exact inputs (S1 after build F)",
    ),
    "A-buildF-s2": (
        "B:/data/m8b-bed/A-buildF/s2-shim/rescore.json",
        "data",
        f"{RESULT}, Set A: exact inputs (S2 after build F: ok rows beyond 1/10 limen)",
    ),
    "B-F": (
        "B:/data/m8b-bed/B-score-F/summary.json",
        "data",
        f"{RESULT}, Set B: seeds 4101-4103 under build F",
    ),
    "B-F-fresh": (
        "B:/data/m8b-bed/B-score-F-fresh/summary.json",
        "data",
        f"{RESULT}, Set B: fresh seeds 4201-4203 (the table)",
    ),
    "C-F": (
        "B:/data/m8b-bed/C-score-F/summary.json",
        "data",
        f"{RESULT}, Set C: specular boxes against the exact image-source answer",
    ),
    "sti-a-s1": (
        "B:/data/m8b-bed/STI/a-s1-buildH/summary.json",
        "data",
        f"{RESULT}, STI: set A S1 after build H",
    ),
    "sti-a-s2": (
        "B:/data/m8b-bed/STI/a-s2/summary.json",
        "data",
        f"{RESULT}, STI: set A S2",
    ),
    "sti-b7": (
        "B:/data/m8b-bed/STI/b7/summary.json",
        "data",
        f"{RESULT}, STI: B7 seven rooms, seeds 4301-4303",
    ),
    "sti-c7-H": (
        "B:/data/m8b-bed/STI/c7-fresh-H/summary.json",
        "data",
        f"{RESULT}, STI: fresh C7 draw after build H, frozen ADDENDUM-3 scoring",
    ),
    "sti-c7-H-add5": (
        "B:/data/m8b-bed/STI/c7-fresh-H-add5/summary.json",
        "data",
        f"{RESULT}, STI: fresh C7 draw after build H, ADDENDUM-5 scoring",
    ),
    "edt-r2": (
        "B:/data/m8b-edt/round2/results/score/summary.json",
        "data",
        "docs/investigations/2026-09-27-edt-heldout/RESULTS-2.md (the scored run, H1-H5) and "
        "VERDICT-2.md, Criteria",
    ),
    "edt-h6": (
        "B:/data/m8b-edt/round2/results/attack/h6.json",
        "data",
        "docs/investigations/2026-09-27-edt-heldout/VERDICT-2.md, Criteria row H6 (attack/H6.md; "
        "the runner: ADDENDUM-B2.md)",
    ),
    "C-GdBA": (
        "B:/data/m8b-bed/C-score-GdBA/summary.json",
        "data",
        f"{GDBA_RESULT}, Numbers (ADDENDUM-6: G and dB(A) end to end on set C)",
    ),
    "m12c-r15": (
        "beds/m12c-r15.json",
        "repo",
        f"{M12C_RESULT} section 4, R15: simpa bed-extra r15 (crates/simpa-core/src/results/extra_bed.rs)",
    ),
}

# Where a set's commit is read when the summary does not carry it: the scorer's log, first line.
RUN_LOG = {
    "B-F": "B:/data/m8b-bed/B-score-F/run.log",
    "B-F-fresh": "B:/data/m8b-bed/B-score-F-fresh/run.log",
    "C-F": "B:/data/m8b-bed/C-score-F/run.log",
}


class Evidence:
    """Reads and hashes artifacts once; remembers what could not be read."""

    def __init__(self, data: Path):
        self.data = data
        self.cache = {}

    def local(self, recorded: str, where: str) -> Path:
        if where == "repo":
            return REPO / recorded
        return self.data / recorded[len(DATA_PREFIX):]

    def raw(self, recorded: str, where: str):
        """(bytes as hashed, None) or (None, why)."""
        key = ("raw", recorded)
        if key not in self.cache:
            p = self.local(recorded, where)
            try:
                b = p.read_bytes()
            except OSError as e:
                self.cache[key] = (None, f"{recorded}: missing or unreadable ({e.strerror or e})")
            else:
                if where == "repo":
                    b = b.replace(b"\r\n", b"\n")
                self.cache[key] = (b, None)
        return self.cache[key]

    def sha256(self, recorded: str, where: str):
        b, _ = self.raw(recorded, where)
        return None if b is None else hashlib.sha256(b).hexdigest()

    def json(self, key: str):
        """(value, None) or (None, why)."""
        recorded, where, _ = ARTIFACTS[key]
        b, why = self.raw(recorded, where)
        if b is None:
            return None, why
        try:
            return json.loads(b.decode("utf-8")), None
        except (UnicodeDecodeError, json.JSONDecodeError) as e:
            return None, f"{recorded}: does not read as JSON ({e})"


def uncommitted_of(text):
    """The `src_uncommitted` diffstat: (bool, its last line) or (None, None) when not recorded."""
    if text is None:
        return None, None
    text = text.strip()
    if not text:
        return False, None
    return True, text.splitlines()[-1].strip()


def commit_of(key: str, ev: Evidence, value):
    """(commit, uncommitted, its summary line, where it was read)."""
    recorded = ARTIFACTS[key][0]
    if key == "m8a":
        return value.get("git_commit"), None, None, recorded
    if key == "C-GdBA":
        return value.get("head"), None, None, recorded
    if key == "A":
        h = value.get("hashes", {}).get("s1", {})
        u, line = uncommitted_of(h.get("src_uncommitted"))
        return h.get("head"), u, line, recorded
    if key in RUN_LOG:
        log = RUN_LOG[key]
        b, _ = ev.raw(log, "data")
        if b is None:
            return None, None, None, None
        first = b.decode("utf-8", "replace").splitlines()[0] if b else ""
        m = re.search(r"hashes (\{.*\})\s*$", first)
        if not m:
            return None, None, None, log
        h = json.loads(m.group(1))
        u, line = uncommitted_of(h.get("src_uncommitted"))
        return h.get("head"), u, line, log
    h = value.get("hashes") if isinstance(value, dict) else None
    if isinstance(h, dict) and "head" in h:
        u, line = uncommitted_of(h.get("src_uncommitted"))
        return h.get("head"), u, line, recorded
    return None, None, None, None


# ---- the checks ---------------------------------------------------------------------------------


def dig(value, *path):
    for p in path:
        if isinstance(value, dict) and p in value:
            value = value[p]
        else:
            return None, ".".join(path)
    return value, ".".join(path)


def check(value, path, want, op="=="):
    """A check as recorded: what was read, where, and whether it holds."""
    got, where = dig(value, *path)
    if op == "==":
        holds = got == want and type(got) is type(want)
        expect = f"== {json.dumps(want)}"
    else:  # pragma: no cover - only == is used
        raise ValueError(op)
    return {"read": where, "value": got, "expect": expect, "holds": holds}


def m8b_checks(key, metric, value):
    if key in ("A", "A-buildF-s1"):
        return [check(value, ("score", metric, "all", "beyond_limen"), 0)]
    if key == "A-buildF-s2":
        return [check(value, (metric, "beyond_limen_after"), 0)]
    # sets B and C: the default runs
    return [
        check(value, ("score", metric, "wrong_silent"), 0),
        check(value, ("score", metric, "pass_"), True),
    ]


def sti_checks(key, value):
    if key in ("sti-a-s1", "sti-a-s2"):
        return [
            check(value, ("score", "all", "beyond_0_03"), 0),
            check(value, ("pass_",), True),
        ]
    return [
        check(value, ("score", "all", "wrong_silent"), 0),
        check(value, ("pass_",), True),
    ]


def extra_checks(value):
    """An M12c closed-form bed (`simpa bed-extra`): every case held, controls included."""
    return [check(value, ("pass",), True), check(value, ("failures",), 0)]


def m8a_checks(value, ev):
    report_sha = ev.sha256(*ARTIFACTS["m8a-report"][:2])
    return [
        check(value, ("pass",), True),
        check(value, ("failures",), 0),
        check(value, ("exploratory",), False),
        check(value, ("report_sha256",), report_sha if report_sha else "report.json unreadable"),
    ]


EDT_RANDOM = ("criteria", "frozen", "random")
EDT_REAL_SETS = ("spps", "ism", "synth")


def edt_checks(value):
    """The scored round-2 summary's half of VERDICT-2.md's join: H1-H4, and H5 on the three real
    fresh sets (7 v 1,471; 0 v 2,793; 7 v 3,034). Its attack set is empty (SENTINEL-EDT.md), so
    its H5 entry for that set and its H6 judge nothing; the join holds only while that is so."""
    out = [check(value, EDT_RANDOM + (h, "pass"), True) for h in ("H1", "H2", "H3", "H4")]
    out += [check(value, EDT_RANDOM + ("H5", "per_set", s, "pass"), True) for s in EDT_REAL_SETS]
    out += [
        check(value, ("inputs", "per_set", "attack"), 0),
        check(value, EDT_RANDOM + ("H5", "per_set", "attack", "n_paired"), 0),
        check(value, EDT_RANDOM + ("H6", "n_classes"), 0),
    ]
    return out


def edt_h6_checks(value, ev):
    """The attacker run's half: H6 passed on VERDICT-2.md's 11 classes, none failing, scored with
    the same frozen method as the summary (its sha256)."""
    r2, _ = ev.json("edt-r2")
    method, _ = dig(r2, "method", "sha256")
    return [
        check(value, ("pass",), True),
        check(value, ("n_failing",), 0),
        check(value, ("n_classes",), 11),
        check(
            value,
            ("method_sha256",),
            method if isinstance(method, str) else "the scored summary's method.sha256, unreadable",
        ),
    ]


# ---- the parameters -----------------------------------------------------------------------------

M8B_SETS = ["A", "A-buildF-s1", "A-buildF-s2", "B-F", "B-F-fresh", "C-F"]
SET_B_NOTE = (
    "Set B is a convergence check: its truths are SPPS itself at 1,000,000 particles, not an "
    "independent physics reference (backlog 74)."
)
NO_MEASURED_ROOM = "No comparison with a measured room (V2-9)."

# name in the report, the metric's key in the bed artifacts, the bed, the artifacts, notes
PARAMETERS = [
    (
        "spl_db",
        "spl",
        "M8b, the one test bed (decision 39)",
        M8B_SETS,
        [
            "On set A SPL checks a sum against itself (ADDENDUM-1 item 6); set C is its real test.",
            SET_B_NOTE,
            NO_MEASURED_ROOM,
        ],
    ),
    (
        "edt_s",
        "edt",
        "M8b, the EDT held-out test, round 2 (Random mode decides, PREREG-2 P11)",
        ["edt-r2", "edt-h6"],
        [
            "Marks (decision 37 (1)-(2), both shown in the GUI): EDT not validated for receiver "
            "radius above 1 m; Energetic-mode EDT not validated.",
            "Read as VERDICT-2.md reads it: H1-H5 from the scored summary on its three real fresh "
            "sets, H6 from the attacker run (attack/h6.json, same frozen method). The scored "
            "summary's own verdict.pass is false (H5, H6) because it ran with an empty attack set "
            f"({EDT_SENTINEL}); it is recorded, not used.",
            "H6 is weak evidence beyond H1-H5: one attacker, 11 synthetic classes (VERDICT-2.md, "
            "Caveats).",
            "Ball against ISO point receiver (decision 37 (6)): "
            "docs/investigations/2026-10-02-edt-ball-vs-point/RESULT.md, PARTIAL as "
            "pre-registered, no row beyond the JND.",
        ],
    ),
    ("t20_s", "t20", "M8b, the one test bed (decision 39)", M8B_SETS, [SET_B_NOTE, NO_MEASURED_ROOM]),
    (
        "t30_s",
        "t30",
        "M8a, the T30 bed; and M8b, the one test bed, which scores T30 beside the others",
        ["m8a"] + M8B_SETS,
        [
            "The T30 stand-in range is not calibrated (backlog 65); G3's (coupled room) T30 range "
            "is +-87 %.",
            SET_B_NOTE,
            NO_MEASURED_ROOM,
        ],
    ),
    (
        "c50_db",
        "c50",
        "M8b, the one test bed (decision 39)",
        M8B_SETS,
        [
            "Shown `wide` where the bin straddling 50 ms can move it past 1/10 of the JND (build "
            "F, 917345c).",
            "Backlog 69: `ok` beyond 1/10 of the JND on short double-slope closed forms at 1 ms.",
            SET_B_NOTE,
            NO_MEASURED_ROOM,
        ],
    ),
    (
        "c80_db",
        "c80",
        "M8b, the one test bed (decision 39)",
        M8B_SETS,
        [
            "Shown `wide` where the bin straddling 80 ms can move it past 1/10 of the JND (build "
            "F, 917345c).",
            "Backlog 69: `ok` beyond 1/10 of the JND on short double-slope closed forms at 1 ms.",
            SET_B_NOTE,
            NO_MEASURED_ROOM,
        ],
    ),
    (
        "d50",
        "d50",
        "M8b, the one test bed (decision 39)",
        M8B_SETS,
        [
            "Shown `wide` where the bin straddling 50 ms can move it past 1/10 of the JND (build "
            "F, 917345c).",
            "Backlog 69: `ok` beyond 1/10 of the JND on short double-slope closed forms at 1 ms.",
            SET_B_NOTE,
            NO_MEASURED_ROOM,
        ],
    ),
    (
        "ts_s",
        "ts",
        "M8b, the one test bed (decision 39)",
        M8B_SETS,
        ["Ts has no straddle bracket (backlog 64).", SET_B_NOTE, NO_MEASURED_ROOM],
    ),
    (
        "sti",
        "sti",
        "M8b, the one test bed, STI (ADDENDUM-2 to -5)",
        ["sti-a-s1", "sti-a-s2", "sti-b7", "sti-c7-H", "sti-c7-H-add5"],
        [
            "No noise range: STI's Monte Carlo noise is not modelled (`mc_sd` null, backlog 71); "
            "shown with a visible 'noise range not computed' note (MQ3).",
            "Tolerance 0.03 STI, not a JND (IEC 60268-16 ed. 4: repeatability 0.02).",
            "B7 was scored at 5d0d281 (build G); RESULT.md says build H's re-read moved no "
            "answered value, and no artifact of that re-read is named.",
            "Several sources at one receiver are refused; directional sources are treated as omni.",
            NO_MEASURED_ROOM,
        ],
    ),
    (
        "g_db",
        "g_db",
        "M8b set C, G and dB(A) end to end (ADDENDUM-6)",
        ["C-GdBA"],
        [
            "G = SPL minus the same source's free-field level at 10 m (ISO 3382-1 A.2.1); the "
            "product's constant agrees with the standard's to 0.0004 dB (RESULT-GDBA.md).",
            "The reference SPL carries the product's own W rho c, so G's error on set C is SPL's "
            "plus the constant: not a second test of SPL (RESULT-GDBA.md).",
            "Set C only (two specular boxes); set B not scored for G.",
            NO_MEASURED_ROOM,
        ],
    ),
    (
        "dba",
        "dba",
        "M8b set C, G and dB(A) end to end (ADDENDUM-6)",
        ["C-GdBA"],
        [
            "dB(A) = band SPL plus IEC 61672-1 octave weights over the bands the run computes; "
            "IEC 61672-1 itself not in hand, the weights checked against its Annex E closed form "
            "(RESULT-GDBA.md).",
            "Set C only (two specular boxes, 125 Hz - 4 kHz); set B not scored for dB(A).",
            NO_MEASURED_ROOM,
        ],
    ),
]

M12C_NOISE = (
    "Its Monte-Carlo noise is judged with a calibration it carries, not one measured for it: of its "
    "measured neighbours, the largest calibrated factor, on the narrowest domain "
    "(params::noise::extra_calibration)."
)
M12C_CLOSED_FORM = (
    "The bed is closed-form series through the report's own path (results::report::parameters_with); "
    "no SPPS-scale bed (M8b sets B and C) scores it."
)
PARAMETERS += [
    (
        "t15_s",
        "t15",
        "M12c R15, closed-form decays through the report's path (simpa bed-extra r15)",
        ["m12c-r15"],
        [
            "T15 is -5 to -20 dB (upstream's TR15), fitted by T20's and T30's regression over its own range.",
            M12C_NOISE + " For T15: EDT's, T20's and T30's.",
            M12C_CLOSED_FORM,
            NO_MEASURED_ROOM,
        ],
    ),
    (
        "decay_custom",
        "decay_custom",
        "M12c R15, closed-form decays through the report's path (simpa bed-extra r15)",
        ["m12c-r15"],
        [
            "Decay ranges a user chooses from -5 dB down 10 to 60 dB (upstream's TR list), fitted by "
            "T20's and T30's regression; a range the series does not reach is refused.",
            M12C_NOISE + " For a decay range: EDT's, T20's and T30's.",
            M12C_CLOSED_FORM,
            NO_MEASURED_ROOM,
        ],
    ),
]

# ---- the rulings --------------------------------------------------------------------------------

# A ruling from the decision log, per parameter, with the exception it names, exactly. It applies
# only while that exception is the only thing failing (ruling_applies).
RULINGS = {
    "t30_s": {
        "decision": 46,
        "date": "2026-10-03 12:52",
        "text": "docs/decision-log.md, row 46",
        "rules": "T30 is shown on the Results screen although its bed status by the strict rule "
        "is FAIL: one wrong-silent row in 833 on M8b set B's fresh draw. When backlog 65 closes, "
        "T30 must pass by the rule.",
        "artifact": "B-F-fresh",
        "metric": "t30",
        "row": {"room": "G6", "receiver": "R007", "band_hz": 1000, "seed": 4201},
    },
}

# What one wrong-silent row fails on its default-run artifact: the count, and the scorer's flag.
RULED_CHECKS = ("wrong_silent", "pass_")


def ruling_applies(ruling, failed, values):
    """Whether `ruling` covers every check that failed: (True, the named row) or (False, why not).

    `failed` is [(artifact key, check)] for every check that did not hold; `values` is each
    artifact read, by key (None when it did not read)."""
    why = []
    key, metric, want = ruling["artifact"], ruling["metric"], ruling["row"]
    path = ARTIFACTS[key][0]
    for k, v in values.items():
        if v is None:
            why.append(f"{ARTIFACTS[k][0]} did not read")
    covered = tuple(f"score.{metric}.{x}" for x in RULED_CHECKS)
    for k, c in failed:
        if k != key or c["read"] not in covered:
            why.append(f"{ARTIFACTS[k][0]}: {c['read']} is {json.dumps(c['value'])}, not the ruling's")
    t, _ = dig(values.get(key), "score", metric)
    t = t if isinstance(t, dict) else {}
    rows = t.get("wrong_silent_rows")
    rows = rows if isinstance(rows, list) else []
    ident = [want["room"], want["receiver"], want["band_hz"], want["seed"]]
    named = [r for r in rows if isinstance(r, list) and r[:4] == ident]
    for r in rows:
        if r not in named:
            shown = r[:4] if isinstance(r, list) else r
            why.append(f"{path}: wrong-silent row {json.dumps(shown)} is not the one the ruling names")
    if t.get("wrong_silent") != 1 or len(named) != 1:
        why.append(
            f"{path}: score.{metric}.wrong_silent is {json.dumps(t.get('wrong_silent'))}, "
            f"{len(named)} of them the named row; the ruling covers exactly one"
        )
    # The scorer's flag fails only for the row: coverage and answered share hold (PREREG 2, 3).
    cov = t.get("coverage")
    if not (isinstance(cov, (int, float)) and cov >= 0.9):
        why.append(f"{path}: score.{metric}.coverage is {json.dumps(cov)}, below 0.9")
    rooms = t.get("by_room")
    if not isinstance(rooms, dict) or any(
        not isinstance(r, dict) or r.get("flag_below_80") is not False for r in rooms.values()
    ):
        why.append(f"{path}: score.{metric}.by_room has a room not shown answered >= 80 %")
    if why:
        return False, why
    return True, named[0]


def ruling_note(ruling, row, values):
    """The note the report carries, so the Results screen says PASS by ruling too."""
    t, _ = dig(values[ruling["artifact"]], "score", ruling["metric"])
    value, truth, lo, hi = row[4:8]
    side = f"{lo - truth:.4f} s below" if truth < lo else f"{truth - hi:.4f} s above"
    return (
        f"PASS by decision {ruling['decision']} ({ruling['date']}), not by the rule, which reads "
        f"FAIL: one wrong-silent row in {t.get('answered')} answered on "
        f"{ARTIFACTS[ruling['artifact']][0]} ({row[0]} {row[1]} {row[2]} Hz, seed {row[3]}: "
        f"{value:.3f} s shown, range {lo:.3f}-{hi:.3f} s, true {truth:.3f} s, {side} the range). "
        "When backlog 65 closes, T30 must pass by the rule."
    )


def derived_notes(name, metric, ev):
    """Notes read from the artifacts: set A's share within 1/10 of the JND, the research bar."""
    out = []
    a, _ = ev.json("A")
    if metric in ("t20", "t30", "ts", "c50", "c80", "d50", "spl") and a is not None:
        share, _ = dig(a, "score", metric, "all", "within_tenth_share")
        bar, _ = dig(a, "pass_", metric)
        if isinstance(share, float):
            out.append(
                f"Exact inputs (set A, before build F): {share * 100:.1f} % of answered rows "
                f"within 1/10 of the JND; the bed's research bar is 99 % "
                f"({'met' if bar is True else 'not met' if bar is False else 'not judged by the scorer'}"
                "; research grade is v1.1, decision 39; MQ1)."
            )
    return out


def artifact_entry(key, ev, value):
    recorded, where, section = ARTIFACTS[key]
    commit, uncommitted, line, source = commit_of(key, ev, value) if value is not None else (None, None, None, None)
    entry = {
        "path": recorded,
        "sha256": ev.sha256(recorded, where),
        "section": section,
        "git_commit": commit,
        "uncommitted_changes": uncommitted,
    }
    if line:
        entry["uncommitted_summary"] = line
    if source and source != recorded:
        entry["git_commit_from"] = {"path": source, "sha256": ev.sha256(source, "data")}
    if commit is None:
        entry["git_commit_note"] = "not recorded by the artifact"
    mode = value.get("mode") if isinstance(value, dict) else None
    if isinstance(mode, str):
        entry["mode"] = mode
    return entry


def parameter(name, metric, bed, keys, notes, ev):
    artifacts, reasons, failed, values = [], [], [], {}
    if not keys:
        reasons.append(
            f"no final bed artifact scores {name}: its status cannot be derived from the evidence"
        )
    for key in keys:
        value, why = ev.json(key)
        values[key] = value
        entry = artifact_entry(key, ev, value)
        if value is None:
            entry["checks"] = []
            reasons.append(why)
        else:
            if key == "m8a":
                checks = m8a_checks(value, ev)
                entry["also"] = artifact_entry("m8a-report", ev, None)
            elif key == "edt-r2":
                checks = edt_checks(value)
                entry["verdict_as_scored"] = {
                    "pass": dig(value, "verdict", "pass")[0],
                    "failing": dig(value, "verdict", "failing")[0],
                    "used": False,
                    "why": "scored with an empty attack set, so its H5 attack entry and its H6 "
                    f"judge nothing ({EDT_SENTINEL}); H6 is read from attack/h6.json",
                }
            elif key == "edt-h6":
                checks = edt_h6_checks(value, ev)
            elif key.startswith("m12c-"):
                checks = extra_checks(value)
            elif metric == "sti":
                checks = sti_checks(key, value)
            else:
                checks = m8b_checks(key, metric, value)
            entry["checks"] = checks
            for c in checks:
                if not c["holds"]:
                    failed.append((key, c))
                    reasons.append(
                        f"{entry['path']}: {c['read']} is {json.dumps(c['value'])}, not {c['expect'][3:]}"
                    )
        artifacts.append(entry)
    result_doc = {"edt": EDT_RESULT, "g_db": GDBA_RESULT, "dba": GDBA_RESULT}.get(metric, RESULT)
    if all(k.startswith("m12c-") for k in keys):
        result_doc = M12C_RESULT
    sections = sorted({ARTIFACTS[k][2] for k in keys})
    out = {
        "status": "FAIL" if reasons else "PASS",
        "by": "the rule",
        "bed": bed,
        "result": {"document": result_doc, "sections": sections},
        "artifacts": artifacts,
        "reasons": reasons,
        "notes": derived_notes(name, metric, ev) + notes,
    }
    ruling = RULINGS.get(name)
    if ruling is not None and reasons:
        record = {k: ruling[k] for k in ("decision", "date", "text", "rules", "row")}
        record["artifact"] = ARTIFACTS[ruling["artifact"]][0]
        applies, got = ruling_applies(ruling, failed, values)
        record["applies"] = applies
        if applies:
            out["status"] = "PASS"
            out["by"] = f"decision {ruling['decision']}"
            out["rule"] = {"status": "FAIL", "reasons": reasons}
            out["reasons"] = []
            out["notes"] = [ruling_note(ruling, got, values)] + out["notes"]
        else:
            record["why_not"] = got
        out["ruling"] = record
        # The status, what it rests on, and the rule's own reading beside it, at the top.
        first = ("status", "by", "rule", "ruling")
        out = {k: out[k] for k in first if k in out} | {k: v for k, v in out.items() if k not in first}
    return out


def summary(data: Path):
    ev = Evidence(data)
    params = {}
    for name, metric, bed, keys, notes in PARAMETERS:
        params[name] = parameter(name, metric, bed, keys, notes, ev)
    return {
        "generated_by": "tools/bed/summary.py",
        "do_not_edit": "Generated. crates/simpa-core/tests/bed_summary.rs fails unless this file is "
        "the script's output byte for byte.",
        "plan": "docs/investigations/2026-10-03-m12/PLAN.md, P1 item 1",
        "rule": RULE,
        "parameters": params,
    }


def main(argv):
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--data", default="B:/data", help="the evidence root (default B:/data)")
    ap.add_argument("--out", default=str(REPO / "beds" / "summary.json"))
    a = ap.parse_args(argv)
    text = json.dumps(summary(Path(a.data)), indent=2, ensure_ascii=True) + "\n"
    Path(a.out).parent.mkdir(parents=True, exist_ok=True)
    with open(a.out, "w", encoding="ascii", newline="\n") as f:
        f.write(text)
    s = json.loads(text)
    for name, p in s["parameters"].items():
        by = "" if p["by"] == "the rule" else f" by {p['by']} (the rule: {p['rule']['status']})"
        print(f"{name:7} {p['status']}{by}" + (f"  ({p['reasons'][0]})" if p["reasons"] else ""))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
