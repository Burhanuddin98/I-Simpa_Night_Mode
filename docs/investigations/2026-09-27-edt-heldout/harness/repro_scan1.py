"""Step 7a reproduction (HARNESS-PLAN.md section 6 step 7): regenerate the critique's scan 1
(docs/investigations/2026-09-27-edt-simplify/final/run_scans.py:25-58) through the harness, not by
calling the critique's script. Uses m8b.corpus.scan1(ctx), which already reimplements that grid on
ctx.S (critique/synth.py, loaded through corpus.load_synth's sha256 gate) and ctx.F (frozen/method.py,
loaded through corpus.load_frozen's sha256 gate). No m8b/ code is changed here; this only calls it and
reports what it returns, plus an independent sha256 of frozen/method.py read directly.

Usage (from harness/, with the venv and no bytecode):
    set PYTHONDONTWRITEBYTECODE=1 & C:\\tmp\\m8b-edt\\venv\\Scripts\\python.exe repro_scan1.py
"""
import hashlib
import json
import sys

sys.path.insert(0, '.')
sys.dont_write_bytecode = True

from m8b import corpus  # noqa: E402

FINAL_MD_55 = dict(
    rows=3648,
    wrong_silent={'1': 0, '2': 0, '5': 1, '10': 0},
    refusals={'1': 0, '2': 2, '5': 40, '10': 160},
)


def main():
    target = corpus.find_target_root()
    ctx = corpus.Ctx(target, workers=1)

    # Independent receipt of the frozen method's bytes, read directly (not via load_frozen's
    # internals), so the sha256 in the report does not merely echo corpus.FROZEN_SHA256.
    frozen_bytes = corpus.FROZEN.read_bytes()
    frozen_sha256 = hashlib.sha256(frozen_bytes).hexdigest()
    assert frozen_sha256 == corpus.FROZEN_SHA256, 'frozen/method.py does not match PREREG\'s pinned hash'

    weak, summary = corpus.scan1(ctx)

    tally = summary['tally']               # {'1.0': {...}, '2.0': {...}, ...} - run_scans' own dedup/well-conditioned subset
    steps = ['1.0', '2.0', '5.0', '10.0']
    # kind in ok_good, ok_WRONG, wide_in, wide_out, refused (m8b/corpus.py:303-311, critique/common.py:18-26).
    # FINAL.md:55's "wrong-silent" is ok_WRONG; its "refuses" is the refused kind specifically.
    wrong_silent = {s: tally.get(s, {}).get('ok_WRONG', 0) for s in steps}
    refusals = {s: tally.get(s, {}).get('refused', 0) for s in steps}
    other_kinds = {s: {k: v for k, v in tally.get(s, {}).items() if k not in ('ok_good', 'ok_WRONG', 'refused')}
                   for s in steps}

    got_ws = [wrong_silent[s] for s in steps]
    got_ref = [refusals[s] for s in steps]
    exp_ws = [FINAL_MD_55['wrong_silent'][k] for k in ('1', '2', '5', '10')]
    exp_ref = [FINAL_MD_55['refusals'][k] for k in ('1', '2', '5', '10')]

    report = dict(
        n_rows=summary['n_rows'],
        tally=tally,
        wrong_silent_by_step=wrong_silent,
        refusals_by_step=refusals,
        other_kinds_by_step=other_kinds,
        expected_FINAL_md_55=FINAL_MD_55,
        rows_match_expected=summary['n_rows'] == FINAL_MD_55['rows'],
        wrong_silent_match_expected=got_ws == exp_ws,
        refusals_match_expected=got_ref == exp_ref,
        frozen_method_sha256=frozen_sha256,
        frozen_method_sha256_expected=corpus.FROZEN_SHA256,
        synth_sha256_lf_expected=corpus.SYNTH_SHA256_LF,
        target_root=str(target),
    )
    print(json.dumps(report, indent=2, default=str))


if __name__ == '__main__':
    main()
