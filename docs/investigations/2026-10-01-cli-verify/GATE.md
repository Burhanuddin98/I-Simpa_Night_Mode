# `simpa run` and `run-folder` verify their solver build by default: gate record (2026-10-01)

Backlog 54's CLI half (decision row 33, 2026-10-01 00:53: "`simpa run` checks its solver build by
default, as the app does; the bed half is left open"). Commits `0694037` (the change), `5c5a3cf`
(backlog and solver-contract docs), `4fcac44` and `19384d8` (the gate's own CLI fixtures, broken
by the change, fixed and re-gated below).

## What changed

- `crates/simpa/src/mesh_run.rs`: `run_options()` (shared by `run` and `run-folder`) now passes
  `verify: Some(solver_manifest())` by default, the embedded `solvers/manifest.json`, the same one
  the app checks against. No opt-out flag: the app has none. `solver_manifest()` reads
  `$SIMPA_SOLVER_MANIFEST` first when set, a test-only lever beside `$SIMPA_SOLVERS_DIR` and
  `$SIMPA_TETGEN160`; unset, every real invocation checks the real manifest.
- New tests `t54_1`-`t54_3` (`crates/simpa/tests/cli_run.rs`, failing-first: confirmed RED against
  `verify: None` before the change, GREEN after): a default run with the verified build records
  matching checks and `simpa results` reads it verified, in text and `--json`; a tampered
  `--solver-exe` (the middle byte of its `.text` section flipped, `bed::pe::flip_text_byte`) is
  refused before launch, `solver_unverified`, exit class 2, with the mismatch still recorded in
  `run.json`'s `solvers`; `run-folder` verifies too.
- `docs/v1.1-backlog.md` row 54 and `docs/solver-contract.md`'s `solver_unverified` row updated:
  the CLI now asks by default too; the bed half (row 54's second clause) stays open.
- Fallout the change broke, fixed in the same arc: three CLI tests stood on executables that were
  never meant to be the verified build (`cli_run.rs`'s `exported()`/`t39_*` used the stub solver;
  `a_run_whose_preprocess_gives_up_records_the_warning` used a `.bat` for `--preprocess`;
  `run_folder_fixtures.rs`'s `stub_*` cases use the stub solver too) and the M11 gate's own forced
  mesh-failure run (`tools/gates/m11.ps1`, gate (e)) used a `.bat` for `--tetgen`. Where the
  scenario did not depend on a mismatch, the tests now run the real verified build. Where it needs
  a controlled stand-in, `solver_manifest()`'s `SIMPA_SOLVER_MANIFEST` override lets a test
  register a cargo-built stand-in's own code sha256; `build.rs` links the three stand-ins
  (`simpa-stub-preprocess`, `simpa-stub-solver`, the new `simpa-stub-tetgen-skips`) with
  `/DEBUG:NONE`, since a normal MSVC link embeds a CodeView debug entry the fingerprint (POGO
  only, by its own stated measurement) refuses to hash at all. `app/e2e/lib/plant-loss.ts` now
  clears the planted-loss run's `solvers` key, after confirming it matched, so m11-b38 still
  demonstrates `SOLVER_BUILD_UNRECORDED` on purpose rather than by the CLI's old default.

## Gates (Grace, Windows PowerShell 5.1, worktree `m8b-edt`)

| Gate | Commit | Time | Result |
|---|---|---|---|
| `m11.ps1`, bare | `0694037` | 15:39:58-15:53:14 | FAILED, 4 e2e checks: `tetgen_skips.bat` (gate (e)'s stand-in tetgen.exe) refused outright as not a PE file, so the forced mesh failure never reached stage `mesh` (m11-e-row, m11-e-results, m11-dock-meshfail); the planted-loss run now recorded matching solver checks, so m11-b38's "a command-line run records no solver check" no longer held |
| `m11.ps1`, bare | `4fcac44` | 16:02:09-16:14:40 | FAILED, 1 check: `app/e2e/tsconfig.json` (stricter than `npm run typecheck`, which only covers `ui/`) refused a cast of `Manifest` to `Record<string, unknown>` in the new `plant-loss.test.ts` coverage; e2e itself was 32 of 32, 0 failures |
| `m11.ps1`, bare | `19384d8` | 16:15:39-16:28:05 | **PASSED**, exit 0; core crates (`simpa-core` + `simpa`) 788 passed, 0 failed, 31 ignored; e2e 32 of 32 required, 0 failures, 0 skipped; m11-focus PASS (0 foreground changes, 15 sessions); 0 files left on B: |
| `m10.ps1 -SolversDir C:\tmp\nm-m8a-solvers`, `SIMPA_TETGEN160` set | `19384d8` | 16:28:42-16:38:16 | **PASSED**, exit 0; core crates 788 passed, 0 failed, 31 ignored; e2e 27 of 27, 13 required, 0 failures |
| `m9.ps1 -TargetDir C:\tmp\nm-target` | `19384d8` | 16:38:35-16:39:20 | **PASSED**, exit 0; every lettered check (a)-(h) and the two lint checks PASS |

The first two `m11.ps1` attempts failed on this step's own code: both regressions came from
`0694037` turning default verification on, and both were in test or gate infrastructure that
assumed `verify: None`, never in `run_project` or `results::solver_build` themselves (backlog 38's
existing core, unchanged by this step). `4fcac44` fixed the first three; its own new test coverage
then tripped the e2e tsconfig's stricter checking (`npm run typecheck` only covers `ui/`, not
`app/e2e/`), fixed by `19384d8`. M10 and M9 were run only once, at `19384d8`, after M11 passed.

## Before and after

`0694037` is the only commit that touches `run_options()`'s `verify` field; `4fcac44` and
`19384d8` touch only tests, the gate script and `app/e2e/`. No change to
`crates/simpa-core/src/run/manager.rs`, `crates/simpa-core/src/results.rs` or `bed/run.rs`
(backlog 54's second half, deliberately left open).
