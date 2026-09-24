# Rebuild plan: a new GUI and core over I-Simpa's solvers

**Draft 1, 2026-09-23.** Produced by a 7-agent planning workflow: surveys of the toolchain,
Night Mode's code, upstream's solver contract and Tauri 2, followed by architecture options, a
plan and an adversarial critic.

**The critic rejected the first plan: 0 blockers, 9 major flaws and 9 minor ones.** Every major
flaw has a fix applied below. This draft has **not** been re-critiqued. The full plan, every
gate and every receipt are in `rebuild-plan-raw-2026-09-23.json`.

## Decided

- **A new GUI and core over upstream's unchanged solvers:** `spps.exe`, `classicalTheory.exe` and
  `preprocess.exe`, built from the pinned tag `929a5c8`, and TetGen 1.5.0 (amended 2026-09-23,
  `docs/m5-m6-design.md`, decision 3).
- **A Tauri 2 desktop app with a web UI,** following the approved Concept B (`docs/design/`).
- **The core with no GUI comes first.**
- **A new repo.** Night Mode is a quarry, not the base.
- **GPL-3 for now.**

## Architecture: a Rust core (option B, 8.5/10)

One Cargo workspace holds three parts:
- `core`, a library
- `cli`, the command-line interface to it
- `app`, the Tauri shell, which links `core` directly

The solvers stay separate executables run as child processes. Upstream's own format library
(`lib_interface`) is **never shipped**. It is built only as a test **oracle**, a reference
implementation our file writers and readers are checked against.

Every stage returns a typed result with a reason code, and **no stage turns a failure into a
warning.** A run counts as OK only when four signals agree: the exit code, the classified output
lines, the per-band particle statistics and the check that every expected file exists. This is
necessary: the survey verified that SPPS exits 0 on an unreadable mesh, on 100 % particle loss
and when a setting is missing. It crashes outright when a source is outside the mesh.

| Option | Score | Why not chosen |
|---|---|---|
| **B. Rust core, formats rewritten from a written spec, upstream library as test oracle only** | **8.5** | Chosen |
| E. B with the core as a separate daemon process | 7 | A second IPC contract and supervision to guard against a crash risk B does not have |
| A. C++ core linking upstream's library | 4.5 | Linking GPL code closes the commercial option for good |
| D. Python core on upstream's Python bindings (`libsimpa`) | 3 | `libsimpa` is not built on Grace, covers formats only, and freezing Python into the app is heavy |
| C. Rust core calling upstream's library in-process | 2 | Breaks the rule "never link it into the GUI process": its format code throws and aborts |

**The frontend** is React 19, Vite, three.js (with `three-mesh-bvh` for picking) and uPlot. It
talks to the core through Tauri's `ipc::Channel` for run events and `ipc::Response` for raw
data.

## Answered 2026-09-23, 04:08

> i simpa nioght mode whatever fuckin repo we have, we replace all the work there with this in github, rust goes in B, install rust, it is non fucking commercial

1. **Repo: this one,** `Burhanuddin98/I-Simpa_Night_Mode`. The rebuild replaces Night Mode's code.
   Work happens on branch `rebuild`. Removing the old code on `main` and pushing are queued in
   `session-logs/TO-DELETE-2026-09-23.md` for after 07:00, on Michael's word.
2. **Location: B:, as far as B: allows.**
   - **On B:**
     - the repo
     - the cargo build output (`target/`); it builds, and M0 passed there
   - **On C:**
     - the Rust toolchain itself (`%USERPROFILE%\.rustup`, about 1 GB)
     - the launchers and package cache (`%USERPROFILE%\.cargo`)
   - **Why not all of it on B:?** A verified 2026-09-23 failure. B: is exFAT, so rustup's
     launcher links fail with *"could not create link … Incorrect function (os error 1)"*, and
     the toolchain install on B: failed as *"not installable"*. The identical install on C:
     succeeded on the first try.
3. **Rust is installed:** rustc 1.98.1, pinned in `rust-toolchain.toml`, with clippy and rustfmt.
4. **Non-commercial.** GPL-3 only. The clean-room discipline, the licence-deny gates and the
   commercial-path notes below **no longer apply.** Night Mode's code and upstream's
   `lib_interface` may be read and ported directly. `lib_interface` stays a test oracle, because
   option B still wins without the commercial criterion: on CI, traps, crash isolation and
   Mac/Linux.

Michael should still ratify the Rust core, since he will review it.

## Milestones (reordered per the critic)

Every milestone has a script gate (`tools/gates/mN.ps1`) that exits 0 or not. The full gates
are in the raw JSON.

| # | Milestone | Depends on |
|---|---|---|
| M0 | Repo, toolchain, gate harness | – |
| M1 | Reproducible solver build. SPPS, TCR and preprocess from `929a5c8`, byte-identical to today's reference binaries apart from the link timestamps. TetGen is 1.5.0 from `third_party/tetgen-1.5.0` (= upstream `4db335c`, amended 2026-09-23), extracted unmodified from the committed WIAS tarball, built with upstream's tetgen command lines, and checked by its code sha256 (the sha256 with the link timestamps zeroed, `solvers/pe-fingerprint.ps1`) against `solvers/manifest.json` after a from-scratch build, and by the 2019 meshes of tutorials 1 and 3, byte for byte apart from the trailer line | M0 |
| M2 | Written format specs (`docs/formats/`), Rust format writers and readers, oracle diffs over 1,000+ generated models | M1 |
| M3 | Typed project schema **including variants**, `config.xml` writer, pre-launch validator | M2 |
| M4 | Geometry import, check and safe repair. Raw Elmia **refused with a reason**; corrected Elmia accepted | M3 |
| M5 | Process layer (Windows Job Object), meshing **with surface-receiver refinement**, mesh verification | M1, M2, M4 |
| M6 | Run manager and end-to-end solves: the box and corrected Elmia with a clean verdict; each negative fixture refused with its reason. **This is Night Mode's release blocker, fixed** | M3, M5 |
| M9 | Tauri shell and **IPC benchmark on Grace**, moved ahead of M7 so the data shapes are fixed after the benchmark | M3 |
| M10 | Concept B: Geometry, Materials, Sources & receivers | M9, M4 |
| M7 | Result readers and our own acoustic parameters, checked on synthetic decays | M6, M9 |
| M11 | Concept B: Simulate, Console, Runs | M10, M6 |
| M8 | Physics test bed: T30, SPL level, EDT, C80 and D50 against analytic references | M7 |
| M12 | Concept B: Results. Only numbers with a passing bed are shown | M11, M7, M8 |
| M13 | Windows installer (one NSIS build) | M12, M1 |

**M5 and M6 are gated as amended** in `docs/m5-m6-design.md`, section "Gate amendments": receiver
refinement checked by face area, raw `.poly` input, a cancel that must hit a running TetGen, M6(a)
judged against upstream's own mesh of the box (Burhan, 2026-09-24 05:13), M6(c) against
upstream's tutorial_2 mesh of the hall, and a separate parity gate (`tools/gates/parity.ps1`).

**What the reorder buys:** only M12, the screen that shows acoustic numbers, waits for the
physics bed. The shell, the IPC and WebGL risks, and the first three screens now run alongside
M5-M8 instead of after them.

## The critic's major flaws and what changed

1. **The TCR solver hard-codes 0.163 in the Sabine formula; the textbook value is 0.161.** That
   is a 1.24 % gap, so an exact-match gate would fail on a correct solver. TCR is now checked
   against its own constant, and the gap is recorded as an upstream convention. The gate "no NaN
   in any output" becomes "no NaN in any value we display", because TCR's Global row is NaN by
   design.
2. **Eyring is only exact in a diffuse field.** The bed now uses a near-cubic room and a
   low-absorption range where it holds, and reports the Kuttruff correction alongside. A bed
   failure blocks only M12, and it is treated as a finding to investigate. Tolerance stays an
   open decision (proposed ±5 %).
3. **The GUI had been queued behind the bed.** Reordered, as above.
4. **CI could not run gates that need Grace.** Gates are split in two:
   - **CI-portable:** committed fixtures, no `B:\` paths, no GPU.
   - **Grace-local:** reference binaries, GPU, and the full bed, run on a schedule.
5. **The geometry check refused legitimate models.** Partitions, hanging reflectors and fittings
   resting on a wall create edges that don't meet the "used exactly twice" rule. The check now
   classifies these internal facets instead of refusing them. Open edges on the outer shell are
   still refused. SPPS needs such facets to model transmission.
6. **Surface-receiver meshes were not refined.** M5 now writes the facet-area constraint upstream
   uses, so result maps are not drawn as flat patches.
7. **Parts of the approved design were missing.**
   - **Variants enter the schema at M3**, since adding them later would break the schema, undo
     and run records.
   - **The map selector and the difference-from-baseline map** are in M12.
   - **Deferred to after M12, each with its own gate:** the material library with Odeon and CATT
     import, receiver grids, source groups and intensity vectors.
8. **The bed covered only T30,** so the Results screen would have hidden everything else. M8 now
   also covers:
   - **SPL:** direct-field calibration plus the diffuse field.
   - **EDT.**
   - **C80 and D50,** against Barron's revised theory.

   STI comes later.
9. **Nothing enforced keeping the commercial option open.** `cargo-deny` now denies GPL, AGPL and
   LGPL crates in `core`, and a licence check runs on the npm packages. The clean-room question
   is item 4 above.

**Lighter, per the critic:**
- The full bed runs nightly on Grace, not in CI.
- GUI gates use the app's own self-test hooks instead of a WebdriverIO stack.
- One installer build, not two.
- The C runtime choice moves to M13.

## Taken from Night Mode

Night Mode's code comments say it mirrors upstream GPL code, so it is read **as a spec, not
copied.**

**Carried over as specs:**
- the file layouts of `.cbin`, `.mbin`, `.gabe`, `.csbin` and `.pbin`
- the PLY, STL and OBJ loaders, with their fixes
- the box generator
- the reader in `tools/extract_upstream_scene.py`
- the valid SPPS run and the broken TCR and mesh outputs, as test fixtures

**Dropped:**
- Night Mode's `preprocess.exe` call (the rebuild calls upstream's `preprocess.exe` only when a
  project's mesh settings ask for its scene correction, as upstream's GUI does:
  `docs/m5-m6-design.md`, decision 12)
- the `.proj` loader: wrong record size and a zip-slip hole
- all of Night Mode's acoustic-parameter code, which has a +26 dB offset, NaN reverberation times and a 3 dB double count
- the ImGui GUI

⚠️ `testdata/elmia_corrected.ply` has **2,438 of 7,860 faces in the wrong material group**. The
geometry is fine but the grouping is wrong. It is regenerated from `tutorial_2.proj`.

## Found while building, with receipts

- **M0 passed on 2026-09-23 with 9 of 9 checks** (`tools/gates/m0.ps1`). **M1 passed with 24 of 24**
  (`tools/gates/m1.ps1`) on a from-scratch compile with TetGen 1.6.0; amended for TetGen 1.5.0
  (decision 3), it passed with **62 of 62** on 2026-09-24 at `46b265b`, and with **67 of 67** at
  the tutorial-3 merge (`8f508eb`), which holds each executable to `solvers/manifest.json` by its
  code sha256, so a rebuild of the same code passes.
- **What M1 proves.** Our solver build from `929a5c8` produces the same results as the
  2026-09-08 reference build, compared on a seeded tutorial-1 fixture:
  - SPPS: every file except `.csbin` is byte-identical, including the statistics table and every `.recp`
  - TCR: every file except `.csbin` is byte-identical
  - TetGen 1.5.0 (from WIAS's tarball, sha256 `4d114861…`): on tutorials 1 and 3, the `.node`,
    `.ele`, `.face`, `.neigh` and `.edge` equal the files upstream's GUI wrote in 2019, byte for
    byte but the trailer line; upstream's 1.6.0 is refused. Tutorial 2 differs in 1,036 `.node`
    lines printed at exact decimal ties, every value equal (`third_party/tetgen-1.5.0/PROVENANCE.md`)
- **M5, M6 and the parity gate passed on 2026-09-24**, on a from-scratch solver build at `bd6e83f`
  (merged into `rebuild` as `34d1d5f`; sentinel pass, two minor notes):
  - **M1:** 62 of 62. **M5:** 37 of 37. **M6:** 37 of 37, none blocked. **Parity** (`tools/gates/parity.ps1`): 12 of 12.
  - `cargo test -p simpa-core -p simpa`: 596 passed, 0 failed, 7 ignored, each with its reason. clippy and fmt clean.
  - **Our meshes are upstream's, byte for byte:** tutorials 1 and 3 against the 2019 TetGen files,
    the tutorial-1 `.mbin` against 2019 (sha256 `8a6b3943…`), and the corrected hall against
    tutorial 2's own TetGen set (161,543 tets, `.mbin` sha256 `ec8aa038…`).
  - **So particle loss equals upstream's in every band**, per 300,000 particles on the hall at seed 1:
    215, 222, 181, 169, 159 and 184 lost to meshing at 125 Hz–4 kHz, the same counts as upstream's
    own mesh. That is Night Mode's release blocker closed at parity with upstream, not below it.
  - **Same seed:** tutorial 1, TCR 15 of 15 files and SPPS 17 of 17 with the run's own receiver
    directions; tutorial 3, 24 of 24 files in each of 3 runs, then from upstream's own `.poly`
    (from its `.proj` since the tutorial-3 merge, below). Upstream's shipped 1.4.0 SPPS and TCR are
    bit-identical to ours on tutorial 1.
  - Tutorial 3 from its `.proj` was not yet done then (below). The M6(c) loss bound
    (floor + 4√floor) and the 1 % run limit stay PROPOSED, open decision 7.
- **Tutorial 3 end to end, merged into `rebuild` on 2026-09-24 (`8f508eb`).** `simpa import-proj`
  reads its fitting zones, source groups and per-band materials; `simpa mesh --parity` takes it
  through our `.poly`, upstream's `preprocess.exe`, our geometry check, TetGen 1.5.0 and our
  builder to upstream's stored `.poly` (4,889 bytes, `74b8f831…`) and `.mbin` (338,528 bytes,
  `bc2f0904…`), byte for byte; each of its 3 stored runs gives 24 of 24 files identical on our
  inputs (`docs/m5-m6-design.md`, decision 12). Gates at the merge: M1 67 of 67, M5 40 of 40,
  M6 37 of 37, parity 16 of 16.
- **Burhan's decisions of 2026-09-24 14:11** (`session-logs/HANDOFF-2026-09-23.md`):
  1. Imported `.proj` projects keep upstream's element ids; projects made here keep ours
     (decision 13).
  2. The default mesher restores the true facet markers after `preprocess.exe`; the named parity
     mode copies upstream's, and a run on that mesh is never OK (decision 12).
  3. Source names need be unique only within their source group; a receiver exactly on a wall
     stays refused, with a message naming the wall.
  4. The truncation limits stay strict, 1/10 of the difference limens (0.5 % on T, 0.1 dB on C),
     to revisit after M8.
- **The follow-ups on branch `t3-followups`:**
  - Piece A (decisions 13 to 15: upstream's ids pinned on import, what `preprocess.exe` gives up
    on meshed as upstream's GUI meshes it, every folder's regions held to its own geometry, time
    limits on the mesher's programs): 636 tests passed, M4 12 of 12, M5 43 of 43, M6 37 of 37,
    parity 17 of 17.
  - Piece B: tutorial 3's loss to loops on the parity mesh (20.1 %) against the default mesh's
    (under 0.0001 %), the run manager's refusal and the run verdict, as committed tests the
    parity gate runs; each region check refused on its own; the shipped 1.3.4 and 1.4.0 SPPS on
    tutorial 3; receiver levels of the two meshes; `spps_oneband`'s non-reproducibility named in
    its fixture generator; the gate readings that could not fail given their own refusals.
    637 tests passed, 0 failed, 10 ignored, each with its reason; parity 23 of 23 (5.2 min);
    `tools/fixture-gen/test_fixture_gen.py` 57 of 57.
- **Upstream findings**, measured, internal only and not for publication before arc item 4:
  `docs/upstream-findings.md`.
- **`.csbin` output is nondeterministic by construction.** The surface-receiver format dumps whole
  C structs, padding included. Two runs of the *same* executable differ in the last 2 bytes of
  every 8-byte record. `.csbin` must only ever be compared after decoding, never byte for byte.
  This is binding on M2's readers and on any gate.
- **Upstream's compile produces 1,415 warning lines.** The log is
  `target/solvers/build-build-fresh-*.log`, and MSBuild may print a warning more than once.
  - C4244 (lossy numeric conversion) ×1024
  - C4996 (deprecated function) ×190
  - C4267 (`size_t` truncation) ×134
  - C4477 (printf format mismatch) ×28
  - C4018 (signed/unsigned comparison) ×22
  - C4101 (unused local variable) ×17

  The solvers stay unchanged by decision. C4267 and C4477 are the classes worth knowing about
  when a format reader disagrees with the oracle.

## Verified on Grace by the surveys

- **Missing:** Rust and the Tauri CLI.
- **Present:** Node v24.15.0 and npm 11.12.1, CMake 4.3.2, MSVC 14.44, the Windows SDK, and the WebView2 runtime 153.0.4234.48.
- **Ninja is broken.** The WinGet `ninja` returns "Access is denied", so the plan uses the Visual Studio generator.
- **Today's solver binaries match upstream.** `spps.exe`, `classicalTheory.exe` and
  `preprocess.exe` equal the ones built from `929a5c8` on 2026-09-08 byte for byte outside the
  link timestamps (6 bytes each, M1). `tetgen.exe` is 1.5.0 now, not that build's 1.6.0. Upstream's
  shipped 1.4.0 `spps.exe` and `classicalTheory.exe` give our results bit for bit on tutorial 1
  (the parity bed, `tools/gates/parity.ps1`).
