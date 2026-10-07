# A4: the GPU solver's untested rules (spec, 2026-10-07 10:04)

A2 shipped `spps-gpu` with a bed that proved the walk on CR4 (A2-REPORT.md, A2-AUDIT.md) and left two rules
of the SPPS loop untested because CR4 exercises neither: **transmission** (a particle meeting a face with a
transmission loss spawns a child through it in energetic mode, or passes with the transmitted probability in
particle mode) and **one-sided pass-through faces** (a face that lets a particle through from one side and
reflects it from the other). PLAN.md's A4 row and A2-REPORT.md's "Untested" section name them. Until they are
bedded, a user running a project with either on the GPU gets a number nobody has checked.

Burhan's order for the day, 2026-10-07 08:57: "okay proceed with da work" (the NEXT list of
`session-logs/HANDOFF-2026-10-06.md` on `glass`, item 5).

## What to build

Two bed cases, each with three arms, the A2 precedent (`solvers/spps-gpu/bed/`: `stage.py` builds a case
from an example project, `vs_spps.py` compares, `cpu_gpu.py` checks the CPU and GPU builds of the walk):

1. **Transmission.** A box split by an internal panel with a transmission loss (an upstream material with
   `transmission` set, through the project's materials; read `docs/formats/` and `SPPS-LOOP.md` for how
   SPPS reads it and spawns the child). Point receivers on both sides, one source on one side. Arms:
   (a) spps-gpu `--cpu` vs spps-gpu GPU: the walk's results identical bit for bit, as A2's 17 of 17;
   (b) spps-gpu vs the verified SPPS (`C:\tmp\nm-solvers-timebin\bin\spps.exe`) at the same seed count:
   EDT, T20, C80 and the receivers' total energy, within the noise of two SPPS seeds (A2's method);
   (c) the physics: the energy ratio across the panel against the transmission loss (energetic mode:
   the sum of the far side's receiver energy over the near side's, at steady state, against 10^(-TL/10)
   corrected for the panel's absorption; state the formula with its source). Run in both energetic and
   particle modes if SPPS supports transmission in both; say so if not.
2. **One-sided faces.** The same box with a one-sided internal face (upstream's attribute: find it in
   `SPPS-LOOP.md` and the config.xml writer, `crates/simpa-core/src/validate/export.rs`); a source on the
   passing side and one on the reflecting side, receivers on both. Arms (a) and (b) as above; (c) the
   receiver on the far side of the reflecting source sees energy only through the passing direction.

A refusal is a finding too: if spps-gpu refuses either case with exit 2, record it with its reason, and
report which rule is missing rather than adding it. **Do not patch SPPS or spps-gpu in this step**: a
divergence is reported with the rule at fault and its receipt (the two arms' numbers, the loop line in
`SPPS-LOOP.md`), as A2-AUDIT.md did for its corrections. A fix is its own step with its own audit.

## Done when

Both cases have all three arms run with receipts, and A4-REPORT.md says for each rule one of: PROVEN within
noise (numbers, configuration), DIVERGES (rule, line, numbers), or REFUSED (reason). Nothing else counts.

## Constraints

- Worktree `B:\repos\I-Simpa_Night_Mode\.claude\worktrees\a4` (branch `a4`, from `gpu` ba5dda2). Outputs
  only to `B:\repos\I-Simpa_Night_Mode\.out\a4\` and `C:\tmp\nm-a4\`. Solvers: `C:\tmp\nm-solvers-a5\`
  (spps.exe is the verified build, spps-gpu.exe is the manifest's pin 2401ae86…); launch with
  `SIMPA_SOLVERS_DIR=C:\tmp\nm-solvers-a5`. Do not touch `C:\tmp\nm-target`, other worktrees, or any app.exe.
- Build nothing of the app. If you need `simpa.exe` (the CLI), build it with
  `CARGO_TARGET_DIR=C:/tmp/nm-target-a4 cargo build --release -p simpa --bin simpa` and check C: free space
  first; stop and report if under 20 GB. No `cargo test`.
- Any script that copies a solve folder rewrites the ROOT element's `workingdirectory` in config.xml and
  refuses to start unless it names the copy (the 10-06 19:30 incident).
- Read the solver's stderr on every run. Every number in the report carries its configuration (particles,
  seeds, bands, step, duration, receiver radius) and a label (REALISED).
- Commit on `a4` as you go, messages on the why, no Claude attribution, no Co-Authored-By line. Do not push.
- Do not open windows on Burhan's screen: this step is solvers and scripts only.

## Report

`docs/investigations/2026-10-06-gpu/A4-REPORT.md` on `a4`, committed: lead with the verdict per rule, then
each arm's receipt, the case definitions (so they can be re-run), what the noise band was and how it was
measured, and what remains untested after this (particle mode? diffusion? anything you saw on the way).
