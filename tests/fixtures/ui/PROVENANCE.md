# Provenance: M10 UI fixtures

The three data files here are written by one recipe, `crates/simpa-core/tests/ui_fixtures.rs`,
built from core types only (nothing in it imports app or UI code). Regenerate with
`SIMPA_WRITE_FIXTURES=1 cargo test -p simpa-core --test ui_fixtures`; without the variable the
same test fails when a committed file differs from its recipe by one byte. It runs with the core
crates' tests and in `tools/gates/m10.ps1`. Plan: `docs/investigations/2026-09-29-m10/PLAN.md`,
section 5.

| file | sha256 |
|---|---|
| `teaching_room.simpa` | `5ba9b3f53a44a1a3f935b07c2ff5040a41d4d98b6acb6b8382633bc54b3eeb7d` |
| `materials_6x6.tsv` | `5cc56d5bbe83ea96d7868dda030a40282cd8cfe3a8484e16d5c121eb76741818` |
| `materials_6x6.expected.json` | `1c037138e4c07263fd920577697db1f9a3db2f181465af552b9de969593f8820` |

`tests/fixtures/** -text` keeps the bytes: the TSV's CRLF endings are part of the test.

## `teaching_room.simpa`

The room of the approved design (`docs/design/concept-b-approved.dc.html`):

- **Geometry.** A 10 x 6 x 3 m box, x 0-10, y 0-6, z 0-3: 8 vertices and 12 triangles with
  outward normals, two per group in group order. The Front wall is at x = 0, the Rear wall at
  x = 10, the Right wall at y = 0 and the Left wall at y = 6. Octave bands, 125 Hz to 4 kHz.
- **Groups.** Floor, Ceiling, Left wall, Right wall, Front wall, Rear wall.
- **Materials.** Six, every absorption and scattering value 0.10, specular, double-sided, no
  transmission. The swatches are the design's; the glass swatch is new.

  | material | used by |
  |---|---|
  | Linoleum on concrete | Floor |
  | Mineral-fibre tile | Ceiling |
  | Painted plaster | Left wall, Right wall, Front wall |
  | Slotted wood panel | Rear wall |
  | Heavy velour curtain | (unused) |
  | Glass window | (unused) |

- **Source.** S1 at (2, 3, 1.5): omni, pink, a global level of 85 (dB re 1 pW).
- **Receivers.** R1 at (5, 2, 1.2), R2 at (8, 4, 1.2), R3 at (3.5, 4.5, 1.2), each oriented
  along +x.
- **Ids** are fixed with `Uuid::from_u128`: the project 0x1, groups 0x100 + i, materials
  0x200 + i, the source 0x300, receivers 0x400 + i.

The recipe asserts: the M4 check is ok, with an enclosed volume of 180 m3 and an area of 216 m2;
`validate()` returns no issue at all, 0 errors and 0 warnings, so none of Run's blockers is the
project's (in M10 they were exactly `M11_PENDING`); no material is upstream's `Default` placeholder. The app's own test
(`app/src-tauri/src/scene.rs`) holds the placeholder rule to 6 of 6 groups assigned.

## `materials_6x6.tsv`

Six rows, the six materials in project order, by six octave bands of absorption, as a
spreadsheet copies a block: CRLF line endings with a trailing CRLF, no header row, no name
column.

- Rows 1 to 5 are the design's textbook values (`concept-b-approved.dc.html`, the `MATERIALS`
  table: lino, tile, plaster, panel, curtain).
- Row 6 is the "ordinary window glass" row common absorption-coefficient tables give: 0.35,
  0.25, 0.18, 0.12, 0.07, 0.04. It was not checked against a primary source here; the fixture
  needs plausible values, not authoritative ones.
- **One cell is not a textbook value.** The wood panel at 4 kHz (design value 0.40) holds
  `0.39509836780726515`: the first decimal a seeded xorshift64 finds in [0.395, 0.405) that is
  its double's shortest spelling, has 17 significant digits, and that `serde_json::from_str`
  reads to a different double in this build. It is also the text JavaScript's
  `String(Number(cell))` gives, so it is what a paste sends over IPC. A lossy reader anywhere
  between the paste and the save therefore changes the saved bytes. (At 0.35 no such decimal
  exists, and above 0.5 no double needs 17 digits: the recipe's comment says why.)

The recipe asserts that every cell matches the UI's `parseStrictDecimal` grammar
`^[+-]?(\d+(\.\d*)?|\.\d+)([eE][+-]?\d+)?$`, lies in [0, 1], and that the odd cell is misread.

## `materials_6x6.expected.json`

The honest part. The recipe reads the **committed** `.tsv` bytes with its own reader (split on
CRLF, then on tabs, each cell by `str::parse::<f64>`, correctly rounded as JavaScript's
`Number()` is), loads the **committed** `teaching_room.simpa` with `schema::load`, applies one
`SetMaterialBand { material: materials[row], quantity: absorption, band, value }` per cell
through `History::apply`, and writes `schema::to_json`. So the file is the core's output for that
input. The UI must reach the same bytes by its own path (its TSV parser, `opText`,
`Op::from_json`, `History`, `schema::save`); the file is never typed to match what the UI
produces.

The recipe also asserts that the start file differs from the expected file, so gate (c)'s byte
comparison can fail, and that the odd cell reaches the file with its exact bits.

Pinned so the two paths agree: the grid's default row order is project order; a paste lands at
the focused cell, all or nothing; one paste is one `batch` op and one undo step.

# Provenance: M11 run fixtures

Written by the same recipe, `crates/simpa-core/tests/ui_fixtures.rs` (the step
`the_run_fixtures_match_their_recipes`), from core types only. Plan:
`docs/investigations/2026-09-29-m11/PLAN.md`, section 5. Every measurement below was made on
2026-09-29 (Grace) with the verified solver build at `C:\tmp\nm-m8a-solvers` (its code sha256s
are `solvers/manifest.json`'s) through `simpa run`, from copies of the files on C:, never from
the repository.

| file | sha256 |
|---|---|
| `box_run.simpa` | `3d38961c3150189a81ff9c7c481da2db7bc3d694cb4f561c7220e554c4679e75` |
| `box_long.simpa` | `474c33e35fb80a2ec35cf7beac7820b88cb06ac1505004ccbf3ecdbe8beb4047` |
| `hall_run.simpa` | `d5fb8fa39a4d4e1d32429ecbf7473f2ef319238675408f1db1c34e13a93f8fea` |
| `tetgen_skips.bat` | `96a8a0342023d018074ee9811363ec82565419cfdcc40c525f001206adaea1d1` |

The recipe asserts for each project: the M4 check is ok, `validate()` gives no error, and no
material is upstream's placeholder (`validate::is_placeholder_material`), so none of Run's
blockers is the project's.

## `box_run.simpa`: "the box" of gate (a)

`teaching_room.simpa` with SPPS `random_seed` 5, nothing else changed (the recipe proves it by
setting the seed back and comparing). A seed makes SPPS single-threaded and the run
reproducible. The seed is the first of 1, 2, 3, ... whose run at 150,000 particles lost no
particle (lost by meshing problems plus lost by infinite loops) in any of the 6 bands:

| seed | status | solve (ms) | particles lost per band, 125 / 250 / 500 / 1k / 2k / 4k Hz |
|---|---|---|---|
| 1 | OK | 1497 | 0 / 0 / 0 / 0 / 0 / 1 |
| 2 | OK | 1496 | 0 / 0 / 0 / 0 / 0 / 1 |
| 3 | OK | 1517 | 0 / 1 / 0 / 0 / 0 / 1 |
| 4 | OK | 1509 | 0 / 0 / 0 / 0 / 1 / 0 |
| **5** | OK | 1508 | **0 / 0 / 0 / 0 / 0 / 0** |
| 6 | OK | 1503 | 0 / 0 / 0 / 0 / 0 / 0 |
| 7 | OK | 1479 | 0 / 0 / 0 / 0 / 0 / 1 |
| 8 | OK | 1477 | 0 / 1 / 0 / 0 / 0 / 0 |

Seeds 2 to 8 ran in one batch; seed 5 is the first with no loss. The committed file, run again:
OK, 1,405 ms, 0 lost in every band; lines progress 9,999, info 2, ok 1, warn 0, fail 0. The run
takes 1.5 s, well under 60 s, so the particle count stays 150,000. The gate, not the recipe,
checks the loss: if a future solver change loses a particle, gate (a) fails with the numbers,
which is right.

## `box_long.simpa`: the run gate (d) closes and kills

`box_run.simpa` with 7,000,000 particles per source, nothing else changed. Measured: OK, the solve
took 64,269 ms (at least 60 s, as the plan asks), with one other SPPS run on the machine.

## `hall_run.simpa`: the run gates (b) and (c) measure and cancel

`testdata/elmia_corrected.ply` imported by the core (`import_file`, metres, z up): 7,860 faces in
10 groups, 0 open edges. Every group is on one material, the library's "20% absorbing" (upstream
reference id 22) as `geometry::import::library_material` makes it, the function a `.proj` import
uses too: absorption 0.2 in each band (the `f32`-widened value), scattering 0, specular,
double-sided, nothing pinned. The import's placeholder material is gone. Source S01 and
receivers R01 and R02 are copied from `tests/fixtures/rooms/elmia_corrected.simpa` (its first
source and first two receivers), with their pinned solver ids removed and fixed ids given. The
import's 6 octave bands, 125 Hz to 4 kHz. SPPS energetic, 1,000,000 particles, seed 1. The PLY's
grouping is wrong (CLAUDE.md); that does not matter here, since the run tests the app's
responsiveness and cancel, not the physics.

Measured under `--cancel-after-ms 75000`: 75 s after the solver's launch it was still solving, at
`#25.22` (2,522 progress lines), and was cancelled (exit 130). So it is still solving 60 s after
its first `#` line, as gates (b) and (c) need; a full run would take about five minutes, and the
gate always cancels it.

## `tetgen_skips.bat`: the stand-in TetGen of gate (e)

A batch file with CRLF line endings that writes `scene_mesh_skipped.face` into its working folder
(`2 1`, `1 1 2 3 8`, `2 1 3 4 9`: the rows the mesher's own test fake writes) and exits 3, as
TetGen 1.6.0 does when it skips self-intersecting facets. **It is a fake, named as one.** The
verified TetGen 1.5.0 never writes `_skipped.face` (it stops at the first self-intersection), and
the geometry check refuses self-intersecting input before TetGen runs, so no honest input makes a
real run end `tetgen_skipped_facets`; gate (e) needs a Runs row that does.

The gate makes that run at gate time, with the core's real run manager:
`simpa run <work>\p\meshfail\box_run.simpa --solver spps --tetgen tests\fixtures\ui\tetgen_skips.bat
--runs <work>\p\meshfail\runs --json`. Measured: exit 4; `run.json` stage `mesh`, status FAIL,
reasons `tetgen_exit_nonzero`, `tetgen_skipped_facets`, `tetgen_output_missing`, `neigh_missing`.
The app itself would refuse the stand-in (it verifies every executable before a run, C7), which
is why the run is the CLI's.
