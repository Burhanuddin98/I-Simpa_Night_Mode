# Decisions taken tonight, to fold into docs/m5-m6-design.md and the handoff (main tree busy with gates)

## Decision 3, receiver refinement: 22:53, Burhan

Burhan chose, verbatim (AskUserQuestion answer): "C: TetGen 1.5.0, if the hall passes (Recommended)"

- **What it means.** Mesh with TetGen 1.5.0 (WIAS 1.5.0 = upstream commit 4db335c/a0d4d43 =
  what upstream shipped in 1.3.3 and 1.3.4) instead of the pinned 1.6.0. Only if the hall test
  (workflow wf_907ac414-fd6, target/investigate/confirm150/) passes all three criteria:
  1. exit 0 with no self-intersection stop
  2. every receiver face within its limit
  3. SPPS lost_by_meshing no higher than on the 1.6 mesh
- **Fallback if it fails:** B, pre-splitting receiver faces in our .poly.
- **Evidence:** target/investigate/report-*.md, the synthesis and the challenge (all 5 checked
  claims hold).
- **Needed regardless:** the .mbin builder writes corners (d,c,b,a) with the float32
  GL-coordinate round trip, as upstream does. The pipeline lens reproduced upstream's 2019
  tetramesh.mbin byte for byte this way (sha 8a6b3943...).
- **Keep the source-location check** (wf_d688f181-cb7): the survival margin is 2.4e-7.
- **Build impact:**
  - solvers/build.ps1 must build TetGen from 1.5.0 sources (provenance: WIAS tarball sha256
    4d114861...9cf3), and solvers/manifest.json must record it
  - M1's byte-identity gate must be amended for tetgen.exe
  - the M5(c) skipped-facet path must be re-examined, since 1.5.0 writes no _skipped.face

## Our own build, 23:45, Burhan

**His words, verbatim:**

> we do okay i dont mind building our own version as long as it is a proper build, proper and works as a solver should, the developers are not nearly as good as you I am sure

(Asked first, 23:43: "which is the latest i simpa version that the devs have worked on". Answer:
v1.4.0 Preview, 929a5c8, 14 Jan 2026, the pinned tag. There are two unmerged branches after it,
the last on 18 Jan 2026, and no code commits since. The last stable release is v1.3.4,
2020-12-23.)

**My reading.** We build our own version:
- **Solvers:** upstream's SPPS and TCR from 929a5c8, unchanged, byte-identical builds.
- **Mesher:** TetGen 1.5.0 (WIAS 1.5.0 = upstream 4db335c = what shipped with 1.3.x). This
  replaces decision 3's conditional C: it is adopted, and criterion 3 is restated as "within the
  1% run limit, and identical to original I-Simpa's meshes".
- **Conversion:** .mbin, .cbin and config.xml match what original I-Simpa writes, proven byte
  for byte on the three upstream tutorials: corner order (d,c,b,a) and the float32 GL round trip.
- **Crash and silent-failure paths in upstream's solvers are refused before launch**
  (source_unlocatable, receiver_unlocatable). Solver code is not patched.
- **Proof, in two beds:**
  1. a parity bed: our inputs equal the original's, and a same-seed run gives identical outputs
  2. then M8's physics bed against analytic references
- **Also:** compare the 1.3.4 and 1.4.0 solver binaries on identical inputs, and report any
  difference.
