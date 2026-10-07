# A6: spps-gpu's child queue (spec, 2026-10-07 10:30)

A4 (A4-REPORT.md on `a4`, b479eb7) found that spps-gpu DIVERGES from SPPS on transmission in energetic mode:
each GPU thread holds at most 16 waiting children (`QCAP`, `walk.h:371`; `pushChild` drops the child at
`:528`, counting its energy in `childOverflow`, `main.cu:87`), where SPPS's queue is an unbounded `std::list`
(`CalculationCore.h:33`, drained at `sppsNantes.cpp:148-155`). REALISED on A4's box (6 x 10 x 3 m, an 18 m²
panel, alpha 0.3, TL 10 dB, eps 7): the beam case needs a queue 1,192 deep, spps-gpu drops 383 of 2,541
children per particle and its receiver totals fall 2.45 to 3.26 % short; the omni case at 500 Hz runs 8.49 M
particles where SPPS runs 14.46 M and T30 reads 0.8 % short, every GPU value below every SPPS value. The
run's verdict is OK throughout: the overflow line on stderr is an `unclassified_line`. A wrong number reaches
the user with a green light. v1 takes exactly this kind of thing (CLAUDE.md, "Picking this up").

Burhan's order for the day, 08:57: "okay proceed with da work".

## The fix (log as decision 72 in `docs/decision-log.md`)

1. **A child that does not fit the thread's queue becomes a new family in the global pool** the kernel
   already draws families from (`main.cu:565-568`, `dNextFam`), instead of being dropped. The pool is a
   device array of start particles with an atomic tail; capacity set at launch from the particle count and
   the memory available (state the rule and the bytes per entry; at 2 x 300 k particles and at CR4-27's
   count, on 6 GB and 12 GB). Keep the local queue as the fast path. The per-child RNG must not depend on
   which thread walks the child: check how a child's Philox key/counter is derived (`walk.h`, the transmit
   branch at `:574-579`); if it depends on the parent thread's state, derive it from the parent's key and
   the child's ordinal so the walk is the same wherever it runs.
2. **If the pool itself fills, the run fails loudly.** The solver prints one line naming the count and the
   energy dropped and exits non-zero (the refusal path A2 uses for fittings, exit 2, with a reason the app
   can show: `child_pool_overflow`), and writes no results files that could be read as a verdict OK. Never
   silent, never OK.
3. **The run manager classes an overflow as a failure.** `crates/simpa-core/src/run/classify.rs` gets a rule
   for the solver's overflow line (today an `unclassified_line`), so even an older spps-gpu build that only
   counts cannot pass as OK; `run/verdict.rs` names it in the reasons; a Rust test on a stderr fixture.
4. **Zero behaviour change where no child is queued:** random mode, alpha 1, and every A2 case must stay
   bit-identical with the CPU walk and within noise of SPPS. Run A2's `cpu_gpu_all.py` and `vs_spps.py` at
   the end.

## Bed (the done-when)

Re-run A4's transmission cases from `solvers/spps-gpu/bed/a4_cases.py` and its drivers (on `a4`; cherry-pick
or merge `a4` b479eb7 into your branch first so the bed is in your tree) against the new build:
- Beam, TL 10 dB and 20 dB: spps-gpu against SPPS bit for bit where A4 found them bit-identical, and the
  receiver totals against the exact model to the 1.7e-5 SPPS reaches (A4-REPORT.md's table).
- Omni, 500 Hz, 200 k particles: particles run equal to SPPS's 14.46 M (same seed convention as A4), T30
  within noise of SPPS with no sign bias (all 8 GPU values no longer below all 8 SPPS values).
- A4's one-sided and alpha = 1 and random-mode cases unchanged (bit-identical where they were).
- The loud failure: a case built to overflow the pool (a tiny pool through a flag or env var is fine for
  the test) ends with exit 2 and `child_pool_overflow` in the app's run reasons (drive the app headless as
  A5's `tools/devtools/bed-a5.py` does, or a `simpa run` is enough if the app path is the same code).
- Cost: the CR4 1 kHz 2 x 300 k 10 s case before and after (REALISED, three runs each); the pool must not
  slow a run that never spills.
Done when every row above has its receipt and the report leads with PROVEN per row, or says which is not.

## Constraints

- Worktree `B:\repos\I-Simpa_Night_Mode\.claude\worktrees\a6` (branch `a6`, from `gpu`). Outputs only to
  `B:\repos\I-Simpa_Night_Mode\.out\a6\`, `C:\tmp\nm-a6\` (build: `solvers/spps-gpu/build.cmd C:\tmp\nm-a6\bin`
  through cmd.exe; build `fat` for the final exe) and `C:\tmp\nm-target-a6` (cargo). Solvers folder for app
  runs: copy `C:\tmp\nm-solvers-a5\*` to `C:\tmp\nm-solvers-a6\`, put your spps-gpu.exe there and pin its
  hashes in `solvers/manifest.json` on `a6` (`solvers/pe-fingerprint.ps1` for the code sha, as A5 did); the
  merge re-pin is the main thread's. Read-only for you: `.out\a4`, `.out\a5`, `C:\tmp\nm-solvers-a5`.
- Another builder (B3) is editing `main.cu`'s output path and `output.cpp` on branch `b3` at the same time.
  Keep your `main.cu` changes to the pool, the launch, the overflow exit and the stderr line; do not
  reformat or move code you do not need to touch, so the merge stays small.
- Before any `cargo test`, check C: free space; stop and report if under 20 GB. Run the targets you touch
  (`simpa-core` classify and verdict), not the whole workspace.
- Do not touch `C:\tmp\nm-target`, other worktrees, or any app.exe you did not start. Do NOT run the e2e gates.
- Read the solver's stderr on every run. Every number carries its configuration and the label REALISED.
- Commit on `a6` as you go, messages on the why, no Claude attribution, no Co-Authored-By. Do not push.

## Report

`docs/investigations/2026-10-06-gpu/A6-REPORT.md` on `a6`, committed: the bed table first, then the pool's
design as built (capacity rule, bytes, the RNG derivation and its receipt), what was pinned, test counts, what
is untested (A4's list minus what this covers), and anything found on the way.
