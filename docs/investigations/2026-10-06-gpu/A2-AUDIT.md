# sentinel A2 audit (2026-10-06)

CLAIM1 HOLDS WITH ONE NUMBER WRONG.
- C:\tmp\nm-spps-gpu\runs\a-final\summary.jsonl: 17 cases walk_identical=true, walk_mismatches sum 0, worst double_rel 4.557e-14 (cr4-1k). Matches 4.6e-14.
- CR4 cr4-1k-compare.json: 600,000 particles, steps 1,360,935,330, all walk mismatch fields 0.
- Both arms ran: cr4-1k-cpu/spps-gpu.json backend=cpu "OpenMP, 28 threads" trace 6.16 s; cr4-1k-gpu/spps-gpu.json backend=gpu RTX 5070 gpu_slots 49152 kernel 1.15 s. The two .walk dumps are byte-identical (md5 3785b2f1...), 19,200,008 bytes each.
- WRONG: report says "0 of 9.0 M primary particles", 1.53 billion steps. summary.jsonl band rows sum to 7,400,000 particles (the report's own table column also sums to 7.4 M) and 1,522,101,344 steps (1.52 B).

CLAIM2 HOLDS.
- .out\spps-gpu\bed\analysis.md: CR4 gpu s101 vs spps s101 = 54 of 55, worst 1.14; gpu s202 55/55 (0.79); control spps s101 vs s202 54/55 (1.11).
- 1.14 check by hand: MP3/LS2 EDT 2.416+-0.057 vs 2.330+-0.049, d=0.086, hypot=0.0752, ratio 1.14.
- Ranges: analyse.py uses simpa results.json lo/hi half-range, hypot of the two; no multiplier. Fallback 2.5*mc_sd, else 0 (stricter, not wider).
- Upstream arm: runs\cr4-1k\20261006-212046-613-spps (s101) and ...212931-544-spps (s202) run.json exe.path C:\tmp\nm-solvers-timebin\bin\spps.exe, sha 973059a5..., matches=True, manifest embedded.
- GPU arm used exe sha 1dd49cb5... (runs 214149/214211), identical to current bin\spps-gpu.exe. Earlier gpu CR4 runs (212833, 212856, 213138) used a different exe (357adc5a..., 4cc: 5.4 s and 8.8 s elapsed) and are superseded by last-wins in load_runs.
- Note: every run folder is named "-spps" regardless of arm; arm is in the log, not the folder name.
- Note: 20 of 55 values per pair are both_ne (summed receivers, several_sources): 55 is the evaluable count only.

CLAIM3 HOLDS.
- bed\timings-cr4.jsonl: spps s202 108.886 s, spps s101 109.404 s, gpu s101 4.023 s, gpu s202 4.030 s, cpu 8.932 s. 108.886/4.030 = 27.0.
- Configs: diff of spps s202 solve/config.xml vs gpu s202 solve/config.xml differs only in workingdirectory. nbparticules=300000, duree_simulation=10, pasdetemps=0.001, particles [[1000,600000]] in both.
- Caveat: SPPS s101's 109.4 s was while a GPU build ran beside it (report says so).

SAFETY CLEAN.
- 205 config*.xml under .out\spps-gpu and C:\tmp\nm-spps-gpu: 49 are bed templates with workingdirectory="__RUNDIR__" (rewritten per run; every run's solve/config.xml read back shows its own run folder), all others under .out\spps-gpu\bed\runs\ or C:\tmp\nm-spps-gpu\runs\. None outside. (An earlier sweep that included all of .out also saw 3 under .out\ui\cr4-27 which is not A2's tree.)
- find tests\fixtures .out\ui -newermt "2026-10-06 20:46" returned nothing; git status --short empty.

UNEXERCISED
- Rule 5 XZ plane: report says "XZ in the identity set" but no config in a-final uses it (directivite values: 0, 1 uni, 2 XY, 3 YZ only); no v-srcxz case.
- Rule 11 transmission children: admitted in the report, no case.
- Rule 10 one-sided pass-through: admitted ("not truly exercised").
- v-direct: simpa results refuses the report for both arms, so there is no parameter comparison; only identity and raw .recp totals.
- Rule 18 normal-flip persistence: not reproducible, admitted.
