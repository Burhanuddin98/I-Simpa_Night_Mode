# SENTINEL-EDT: why summary.json says fail and VERDICT-2 says pass (read-only, 2026-10-03)

## (1) Cause
The scorer was run at 2026-10-02 05:20:16 with an EMPTY Attack set. Not pooled-vs-per-radius, not an addendum changing a rule.
- `B:/data/m8b-edt/round2/results/score/summary.json`: `inputs.per_set.attack = 0`, `inputs.attack_classes = 0`, `tables.attack.frozen.n = 0`.
  `verdict.pass=false`, `verdict.failing=["H5","H6"]`; Energetic `["H3","H5","H6"]`.
- H5 is "strictly lower than upstream on every fresh set" (`score/REPORT.md:11`, `harness2/m8b/score.py:490` `h5`). Attack row reads 0 vs 0 on 0 rows, so it cannot be strictly lower: fail by P27 ("a set that answers nothing cannot pass", `score/REPORT.md:3`).
- H6 has no classes to judge, so fails (`score/REPORT.md:12`, `:73`).
- The three real fresh sets in the same file do pass H5: `tables.{spps-random,ism,synth}.upstream.n_wrong_silent` = 1471 / 2793 / 3034 vs ours 7 / 0 / 7.
- RESULTS-2.md:3 says "H6 is not part of this run". RESULTS-2.md:14-15 and SENTINEL-2.md:35 both state the REPORT "fails H5, H6" is an artefact of the empty attack set.
- The H6 runner was written after scoring (`ADDENDUM-B2.md:1-18`), output written 05:42:06, 22 minutes after summary.json. summary.json was never regenerated with it.

## (2) Where the PASS lives
- H1-H5 numbers (7 v 1,471; 0 v 2,793; 7 v 3,034): same file, `B:/data/m8b-edt/round2/results/score/summary.json` `tables.*` and `criteria.frozen.random.H1/H2/H3` (all `pass:true`), also `docs/investigations/2026-09-27-edt-heldout/RESULTS-2.md:12-34`. The file's own H5 / overall verdict is the thing that reads false.
- H6 (0 of 220 draws wrong-silent): `B:/data/m8b-edt/round2/results/attack/h6.json` (`pass:true`, 11 classes x 20 draws = 220, sum `n_wrong_silent` = 0, all `reproducible:false`), `attack/H6.md`, `attack/rows.csv`.
- No single artifact holds the combined PASS. VERDICT-2.md is a hand join of summary.json (H1-H5) and attack/h6.json (H6). The "H5 pass" counts Attack as not applicable; no artifact scores H5 with Attack excluded.
- Not checked: the "max error of an ok row 2.4 %" figure (not read from rows.csv).

## (3) Decision-log row 37 (2026-10-02 08:42, docs/decision-log.md:49)
Rules on it: (1) "EDT's round-2 verdict stands as scored, radii pooled"; large spheres (> 1 m) flagged not validated via `edt_validated`; no round 3. (2) Energetic stays not validated. It does NOT mention the empty-Attack H5/H6 fail in summary.json or the join of two artifacts. The "stands as scored" wording is about the radii-pooled ruling (VERDICT-2.md ruling 1), not about re-scoring.

## m12 generator
`.claude/worktrees/m12/tools/bed/summary.py:123-126` reads summary.json only (`edt-r2`); `:310` comment itself notes VERDICT-2 reads PASS with H6 from a separate run. It does not read attack/h6.json, so it marks FAIL from the empty-attack artefact.
