# The default SPPS run length raised to 10 s: gate record (2026-10-01)

Decision row 36 (Burhan, 08:13, "keep it fixed, 10s for now then"). Commits `ccf28f4` (the change) and
`e226959` (rustfmt of its two new spots). Builder's report: `C:\tmp\m8b-edt\product-10s-report.md`.

## What changed
- `crates/simpa-core/src/schema/model.rs`: `SppsSettings::for_bands` duration 2.0 s -> 10.0 s, the one real default.
- `crates/simpa-core/src/geometry/import/proj.rs`: a legacy `.proj` with no `duree_simulation` keeps upstream's own
  2.0 s, no longer tied to our default.
- `crates/simpa-core/tests/ui_fixtures.rs`: four UI fixtures (teaching_room, box_run, box_long, hall_run) had
  inherited the default through `Project::new`; pinned to 2.0 s so their recorded results stay reproducible.
- New tests: `tests/spps_default_run_length.rs`, `tests/config_xml_default_duration.rs`.
- The M8a T30 bed sets every cell's length itself; it did not rely on the default and is unchanged.

## Gates (Grace, Windows PowerShell 5.1, worktree `m8b-edt`)
| Gate | Commit | Time | Result |
|---|---|---|---|
| `m11.ps1`, bare | `ccf28f4` | 14:34:38-14:47:37 | FAILED on one check, workspace `rustfmt --check`, both spots new code of `ccf28f4` (clean on parent `3ddaa64`); e2e 32 of 32, npm and tsc pass |
| `m10.ps1 -SolversDir C:\tmp\nm-m8a-solvers`, `SIMPA_TETGEN160` set | `ccf28f4` | 14:48:32-14:58:06 | **PASSED**, exit 0; core crates 785 passed, 0 failed, 31 ignored |
| `m9.ps1 -TargetDir C:\tmp\nm-target` | `ccf28f4` | 14:58:25-14:59:10 | **PASSED**, exit 0, 23 PASS, 0 FAIL |
| `m11.ps1`, bare | `e226959` | 15:00:23-15:14:47 | **PASSED**, exit 0; m11-focus PASS (0 foreground changes, 15 sessions); 0 files left in the repository |

The first bare M11 attempt (14:22-14:33) failed only because this worktree had no `app/node_modules`; `npm ci` in
`app/` (as `2026-09-30-b38-39/GREEN.md:113`) fixed it. `e226959` changes formatting only, so M10 and M9 at
`ccf28f4` stand for it.
