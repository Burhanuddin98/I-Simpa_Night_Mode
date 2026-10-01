# ADDENDUM-B1: round 2 of the M8b EDT held-out test, fixed before any round-2 held-out row exists

Committed 2026-10-02 on branch `m8b-edt`, on top of `9e80d0c` (tree `15f230d81f8d`). Same form as `ADDENDUM-A1.md`. From this
commit on, nothing in the table below changes: every file under `harness2/` and `frozen2/`, and the named documents, are
hashed here, and `PREREG-2.md` (with amendment 1: Z = 2.5), `HARNESS-PLAN-2.md` (sections 1-5 and 9, the Crucible
resolutions) and `preview_pin.json` carry the fixed parameters; they are not restated. A failed method is fixed and tested
again on a different fresh set. Round 1's `harness/`, `frozen/` and `frozen2/method.py` were never touched by round 2's build.

## The go-ahead

Burhan, verbatim: 2026-10-01 10:59 "exactly i did not want to be asked, keep building and building and finish the work";
2026-10-02 00:15 "proceed"; 00:24 "Z = 2.5".

## State at the freeze (checked when this file was written)

- No round-2 held-out row exists: `B:\data\m8b-edt\round2` and `C:\tmp\m8b-edt\round2-rooms` do not exist (nothing in either),
  and `freeze2.facts()` reports `round2_rows_exist: False` over the data, rooms, results, launch-log and progress-log paths.
- The harness: harness2 suite **105 of 105 pass** (`python -m pytest` in `harness2/`, 3 min 6 s, at `9e80d0c`).
  The audit's closing fixes are in that commit: the P6 truth-feature report (`m8b/features.py`), R_m required on every
  set, and the corrected simpa.exe hash in `harness2/SETUP.md`.
- Preflight, all passing and none of it running a solver or reading round-2 data: `python -m m8b.corpus2 --check` ("pin check:
  all reproduced": a fresh draw of ISM-fresh-2, Synth-fresh-2 and the G rooms equals `preview_pin.json`);
  `python -m m8b.run_heldout --dry-run` (the 172-run plan lists, first `truth-G1-9101`, last `tested-G7-energetic-5.0ms-150k-3503`);
  `driver.check_solvers` on `C:\tmp\nm-m8a-solvers` and on `C:\tmp\nm-target\target\solvers\bin`: all four executables match the
  manifest's code hashes in both; `python -m m8b.freeze2 --check` on the table below: every hash reproduced.
  The runner's and the scorer's own B1 gate (`driver.b1_committed`) opens only once this file is committed.
- Executables: `simpa.exe` sha256 `6a1656ed92efe2b4fa7229a77814a6e56f9b6e56b69166e01655a0c53e2c19f5` (8,204,800 bytes,
  `C:\tmp\nm-target\release\simpa.exe`); `spps.exe` sha256 `1d9900db6cc260039c53963754d4df8de4370c7c6a65cfe998a66cb5720822e8`
  (`C:\tmp\nm-m8a-solvers\spps.exe`), code sha256 `550485c6952925013b421207a726e0f0f0faf45d17652955445c562b6601963d`
  (`solvers/manifest.json`, itself sha256 `8e45168f58cd84b6193bc0c6ed8182b814bd05e24b3335d134c0487e4129c39b`).
- `preview_pin.json` sha256 `69a47adc92f094b7e1b3798d0eb2f288fcfa18d96e914ac44cf6505e1d343b39`.
- Python 3.13.13, numpy 2.5.2.
- Note on the table: `freeze2.NAMED` also hashes `PREREG.md`, `HARNESS-PLAN.md`, `ADDENDUM-A1.md`, `expected_dry_2.json`,
  `frozen2.sha256`, `.gitattributes` and `frozen/method.py` (round 1's, raw CRLF bytes); a superset of the brief's list, kept as
  the tool writes it. `ADDENDUM-B1.md` cannot hash itself and is not in it.

## Hash table (m8b.freeze2, text files by LF bytes as committed)

| file | sha256 |
|---|---|
| .gitattributes | 9447565221d14b147374e3d48f3ecada87760cfe1a7b84c2428c5273f3a57130 |
| ADDENDUM-A1.md | 0066305d6cc1f977b399b579b9be8143c8cfb2417281e4416088fb7acd4d4331 |
| HARNESS-PLAN-2.md | a7a51b29d73693a88df7b043c58fca9bfd4fb2808077ea8a6876e15d54038f84 |
| HARNESS-PLAN.md | b819cc9f91bcb3770a51d2146f0738575bf1ea89200d3e903803432a8149b9b9 |
| PREREG-2.md | 4f1dd30e644c98f7913476fc73a76993bd4050b8bcfbb30ee7d507690ddbcd1c |
| PREREG.md | 9c90ca0c35171e82adcbdeb66038e7793d13de4d6bdedd828a27ad4af20f785c |
| expected_dry_2.json | 19477e944e0fff53aa29a2725e499fc147539aa0c26e56d01da6dc089ae687dd |
| frozen/method.py | 462c37cf159d4d9bd8abadd8261f975fa47ace66f0d4cb4b1e6f4234c77fdf6e |
| frozen2.sha256 | 57fde6341595994a4e04a1727879db2f56d9951c7bb5ff1d5f46f88c65e16367 |
| frozen2/method.py | 029d90ac5e8f6a634a51a7ffce136cbd4c0b7ea3d3ad8a18b78fd8240ff224a0 |
| harness2/.gitattributes | 47891b3a941fc356829e95fad15f68001104a52a97f39de6d91ae94a274e6335 |
| harness2/SETUP.md | 43d87bb36654cdd5cd65d6d2c1d218e1ce66dc7d6a79b3cdb37ac9acec08c70b |
| harness2/corpus_rooms.json | dc3d0b2c0b8be0f69c7927f0c749a88dad76cc791edae2474f09cb1fd44da882 |
| harness2/corpus_rooms_2.json | 5b9b0fba148f7ce3f013eb84b3755ac2b7acd78b657d63c935d3fcf2e8b7fdcc |
| harness2/dry/build_d2.py | 7c018cd5d5029fcf6d81b9049cdc67e9a94419e0eb856c0daff15c8034fbefbb |
| harness2/dry/run_d4.py | c17a0afbb844ae947a8c5f761b5064db181f40a1a51fc07b0c7f2ef732c43288 |
| harness2/dry/run_dry.py | 942f754d1ecaa17353e7f73dc61e2a80378c8fc405cbca4ed1db7f3e7054dc9c |
| harness2/dry/run_dry2.py | b4d598580150195eabe18280a557eac27ba01226af2b029fe4f05f73111e4eb8 |
| harness2/dry/smoke_run1.py | df84c5755ac497fe58af69d235ebae85da3f1265b047fe61208ffc2fab9d4d9e |
| harness2/fixtures/upstream-t1-2019/Acoustic parameters.gabe | c960c782ef0564dda11d6903b82915e9d0e95282b11068e1195719ae0fcb2b22 |
| harness2/fixtures/upstream-t1-2019/PROVENANCE.json | 145dbdc249a9d7f008705e3c5750550dbf93b0497b8810df2240307db0fb6e1b |
| harness2/fixtures/upstream-t1-2019/Schroeder curves.gabe | 0068370b838dcbba15a33ec1d3d8bd104b0c096d680a5267e26ccfbba105adcb |
| harness2/fixtures/upstream-t1-2019/Sound level.recp | 521c81891d5ede31e34beffe5f7f2915875bb8e888160e76a8ccbdf7f2a37afc |
| harness2/fixtures/upstream-t1-2019/oracle.json | 35a767ed33154d8e0df2796c57df03717cbda880b8f9f81457b0085195f002bc |
| harness2/m8b/__init__.py | 18f440ec399cf17cdf8f3bfd8a7781ba5231eb201f13396b208d3f5fcf92432f |
| harness2/m8b/_ism.py | a7c9d41ec61dd119a14583faaa98d4a88c98e897a0cf4a8d69c6d151d2cef747 |
| harness2/m8b/_mirror.py | 9824f4bb1d0e4f24554f31efc7749a20ba243bf8ef971e4f88d82ad5b4ebad79 |
| harness2/m8b/_truth_ideal.py | 2a37e532855df11e61bfc402ef2637c3e3acc2c372f29aa22e5e8991c72c6f3e |
| harness2/m8b/_upstream_edt.py | 57f391e6cb09ded8a408462fbabc7aacd93743a3fa46cc28deeef3f7a319f282 |
| harness2/m8b/_z3echo.py | 5fff8405ed0b485d87a8c9fc5097d343b7c56c29232da0f44ebd698a51e16c4e |
| harness2/m8b/attack.py | e00def770d6da158e715e15e937145457f71c607a5832e52f8f69ac758308f9c |
| harness2/m8b/corpus.py | af13bd7587ec9c79c05da02bef313259008ff057910670b0c6b0eb4aace812b9 |
| harness2/m8b/corpus2.py | 3f697cfa59ee4452e25a802d79a93cbf4204fa0d58b11119e2aa6e2c440f12bb |
| harness2/m8b/driver.py | b1ef42cc9029396e76c3f03d6be47b069997bea52d6d8a15c6bed5a6858fd4d3 |
| harness2/m8b/features.py | bd8da506798dd8df424fe971dc43b4f9b93561a102f695adbf66619a2cd5e7b7 |
| harness2/m8b/freeze2.py | 4f76a8092bc399b20d29399e78fc3d79df29d11cb2fa5550d52d59487ac834ae |
| harness2/m8b/ism_fresh.py | 3d04f6ff1d7236276c57e50aca8bbdd4e6964b3ad7373582293cf1a5dd530d3e |
| harness2/m8b/method.py | fa3cb823e859c97d970e3f840ae350c58cd28d44f82e6a6a1a44e3405cb0aa4a |
| harness2/m8b/provenance.json | a72ec594de85e7988968e941739639e151314719220a92c842e109fe69f0a69b |
| harness2/m8b/rooms.py | 4e99e457a3b59f886a87b0c1ab7661ab0cdd10631f6d6f7c972c25fc46d5b1fc |
| harness2/m8b/rooms2.py | 6a36b895c0fcff85215a607b60e2f810fc3de60dfe52f6aaa22c647267a5b28c |
| harness2/m8b/round2.py | 712f2784a20d715958659a683c7b329cdb2e188879b1955d4c3f4c8ca041f8a8 |
| harness2/m8b/run_heldout.py | 0f6d6da7ba6759090a021bed7c6147fa1696b4a476a83cf44ab5b270c386dfe3 |
| harness2/m8b/score.py | 3eb86dd2ab1daf9d689b1b0beff641cd1e3fa2459b02db3ae839a0ff8aef8e11 |
| harness2/m8b/score_heldout.py | 48095fab11b07b8532f80d36460ed6a4667635956276be744e0c738fe605e5fb |
| harness2/m8b/spps_rows.py | ea8ebb0194c9cf2ee2c7df34e31753037db64dec0aea6204a04a80829a438c68 |
| harness2/m8b/synth_fresh.py | d721aff12e16d93918755ef2c437d4cfb6dddd6a8f8e7c5fa833baf79e7aba52 |
| harness2/m8b/truth.py | 412cb9ece0cf36b3d5f2ca143235898d978c5d155d2b6c5f72244efacc47af90 |
| harness2/m8b/upstream.py | 76ea8c3209d608a83ab36c33618debdca79dff3a015f516ca2dd0539ff155781 |
| harness2/pyproject.toml | 589b8d5e2c77684efca343e78c41062e54c16d9135657c4eed196e959108cc38 |
| harness2/repro_scan1.py | bf8ee74c043d91d3741af579b282a6382b3cf68c7e22a14b5bc07ddd60aafe9e |
| harness2/requirements.txt | c41daf650ffd980c3b2bf7c90554955e906b2a28d54d59d0b8fa28540db31f87 |
| harness2/tests/conftest.py | 12c56b3ef11d8fff87ebd850a18fea3d0c2fe34f471a054df6d7278a598ed1ab |
| harness2/tests/test_attack.py | 26ddfc2116eda2b7f98eb5204735260c0232fdde6f0dca378cf8f74e3bd89437 |
| harness2/tests/test_driver.py | 887fbb56ea7e1dc756dfbb3d3a6ce927385b6cc237f2a5a7f42c7f58746f9e37 |
| harness2/tests/test_driver_guards.py | a0706139e08f959f1ddcc25889ec49a007fe060d59ddd13be41719b16d182657 |
| harness2/tests/test_fresh_sets.py | d24aa543f7beafc2e743bc1b35351e72c57f60c2df921638f6f8f8bb02f6bab5 |
| harness2/tests/test_gap_all_rooms.py | 02ac7b225112e7ecfc73f87773774be3148144950e0bd850c9e58913b03fa2fd |
| harness2/tests/test_method.py | d2747c1291091975fdd8ad8f162e14ebe74c7dbf3bda797ce5e2202b751a344a |
| harness2/tests/test_physics_md.py | d203f3869fad429b51ac5753b66250ba4c211a86af575ef4c9912a0bf367ddae |
| harness2/tests/test_planted_faults.py | 16662529dec2bbfd79b6c4db833f9cfd1a1fdaa34e28fce71e1dc227871ece93 |
| harness2/tests/test_r2_attack.py | dcefab4fe9342ceb2ae6a52df0d638db924f3b8c258d79822cb07aa94cd038c6 |
| harness2/tests/test_r2_corpus_rooms.py | fd6b6774e0d3578cb2565cee52c0badaa3b4518d1cf97f47e29521fe897f7231 |
| harness2/tests/test_r2_dry.py | e3d63d218c0fcc0738f02ba1d8c020f6972c1eb8a7cc60571d1c288dda3ee83f |
| harness2/tests/test_r2_features.py | 14d44e4dbed1639f02ff1a4de2e5ead8c74837052cdd0525f2d9e7b21247a9f5 |
| harness2/tests/test_r2_freeze.py | b13b8115b41d179f5a320c260b5ec94d47e860255b8261a9d779e02f47f27c4b |
| harness2/tests/test_r2_fresh_sets.py | 9b25c58c8f33cc6293f336f85a3a9ff8bb6d5d395e68f69e5eab2347325b3bff |
| harness2/tests/test_r2_logging.py | 365cfb20e62de1dcb61cf280da02539ede2731787caa1a4cb02fed2985d681cf |
| harness2/tests/test_r2_plan_guards.py | 768e9cec57afd8aacac4fd0850b757f093b663d11ac8a4c8192de6b3304a9b84 |
| harness2/tests/test_r2_run_order.py | 3c7022a571c2631bc85f67fd1872147d731fa2fc510bcfb98bc6d31e812ce5cc |
| harness2/tests/test_r2_scorer.py | 78e19f588eeef413cf8e4d19b2c9a3d30920d68cd4ad5527b803eed6aa5018e9 |
| harness2/tests/test_rooms.py | 3eee5020c025870f59d25094aff99a205593007e51dc640756b47c8c2941764b |
| harness2/tests/test_run_heldout.py | 495dc86805be5fcce4ae7c84f0330f58b72de93682c91a568ed6ac60d29fcac0 |
| harness2/tests/test_score.py | 1025478f70e4f6906c80e64526394e6ca5d1a52dd4767afc5bf7349b710298be |
| harness2/tests/test_step8_2.py | ae9edeb8e7564a412b9cf312803179517aa99bc5ad233e3e3516aafdcb4b82c9 |
| harness2/tests/test_truth.py | 1addea537e15f8afd47ecbe3205503a97b8fddf99dcb75324a6bd61984e4bf0d |
| harness2/tests/test_upstream.py | ff451c9ceb3a7ce8e5d72b146ddcdecb45e856ecf224ab519f89ebe4228f2e0e |
| harness2/tests/test_weak_spots.py | cdc5b6b369c290902960fd3b9d1ceeead084320909613a2ac5da7d7e394f6845 |
| harness2/tests/test_wiring.py | 689907462ed4769ec055ea375a5d6ea02628923370a22c0a19859892b48962eb |
| harness2/weak_spots.json | c75d53212f4114e785369668e50d81143bc09882c06d6cab2a9c792b730da9b8 |
| preview_pin.json | 69a47adc92f094b7e1b3798d0eb2f288fcfa18d96e914ac44cf6505e1d343b39 |

Facts at the freeze:

- python: 3.13.13
- numpy: 2.5.2
- git_commit: 9e80d0cbe6f851a37fe9a8e59a4ec442b89c22e8
- git_tree: 15f230d81f8dde01c7f8097357e2f92e5e1d1671
- solver_manifest_sha256: 8e45168f58cd84b6193bc0c6ed8182b814bd05e24b3335d134c0487e4129c39b
- spps_code_sha256: 550485c6952925013b421207a726e0f0f0faf45d17652955445c562b6601963d
- simpa_exe_sha256: 6a1656ed92efe2b4fa7229a77814a6e56f9b6e56b69166e01655a0c53e2c19f5
- round2_rows_exist: False
