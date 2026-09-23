# M5 and M6: design and interfaces

**2026-09-23.** This is the working design for M5 (process layer, meshing, mesh verification)
and M6 (run manager, end-to-end solves). The gates come from `rebuild-plan-raw-2026-09-23.json`,
milestones M5 and M6, with the amendments below. The terrain maps behind these decisions are in
`target/workflows/m5m6-map/` (not committed); every fact here carries its receipt.

## Decisions

1. **Fitting ids stay as M3 set them. The `.mbin` builder writes the room as 0.**
   - `FIRST_FITTING_ID` stays 2 (`config_xml/ids.rs:20`), and the zone ids stay whatever
     `SolverIds::assign` gives. TetGen's `-A` gives every unseeded region the next attribute
     above the largest seed, starting at 1 (`tetgen.cxx:24223-24306`).
   - The builder maps each tet's attribute: a seeded fitting id stays itself, and anything else
     becomes 0. That is the documented meaning of 0, "main volume" (`coreTypes.h:445`).
   - Upstream's own meshes carry the room as 1, which the solver looks up and gets NULL
     (`coreinitialisation.cpp:158-173`).
   - **M5(e) reads "the first fitting zone, solver id 2"**, not "id 1".
2. **The mesher takes its flags from the project's `MeshSettings`.** The flags are
   `[-a<v>] -pq<r> -A -n [-Y]`, in upstream's order (`projet_maillage.cpp:167-170`).
   - Numbers are narrowed to f32 first, as upstream's float settings are (`projet.h:98-99`),
     and then printed like upstream's `Convertor::ToString` (precision 15, classic locale;
     `sppsString.cpp:91-98`). So a q of 1.1 prints as `-pq1.10000002384186`.
   - The room fixtures carry `q 2, preserve_boundary false`, and the box also has a `.var` at
     0.1 m². That gives `-pq2 -A -n`, the same flags as upstream's tutorial_2 mesh.
   - The default `MeshSettings` gives upstream's GUI default, `-pq5 -A -n -Y`.
   - **M5(b):** without `-Y`, the hall's `.1.face` has far more rows than 7,860. Measured with
     the pinned TetGen: 36,716 rows; upstream's older build gave 60,974. The gate instead
     checks three things: every row has a marker ≥ 0, the markers cover all 7,860 scene faces,
     and no marker's geometry mismatches.
3. **Surface-receiver refinement does not work with the pinned TetGen. This is an open
   architecture decision.**
   - **Why.** TetGen 1.6.0 at `929a5c8` never reads a facet's area bound when it splits a
     subface. `check_subface` tests only the radius-edge ratio (`tetgen.cxx:27347-27388`), and
     every other `areabound` read is commented out or merely copies the value.
   - **Measured:** the tutorial box meshes to the same 6 tetrahedra with and without its
     `.var`, and with or without `-Y`. The floor receiver comes out as 2 faces of 30 m².
     Upstream's 2019 tutorial mesh, made by an older TetGen, has 934 floor faces of at most
     0.0998 m².
   - **Consequence:** current upstream builds almost certainly draw unrefined surface-receiver
     maps too. That is not to be claimed publicly before a bed shows it.
   - **The two ways out, for Burhan and Michael:**
     - patch TetGen, which breaks "solvers unchanged" and M1's byte identity
     - pre-split the receiver faces in our `.poly` and keep TetGen stock
   - Until one is chosen, M5(a)'s area check is an ignored test, beside a test that pins
     today's behaviour.
   - `mesh_settings_conflict` (`.var` together with `-Y`) stays as a project-stage error. It
     keeps parity with upstream's GUI, which clears `-Y` whenever the constraint is on
     (`e_core_core_tetconf.h:82-90`), but it prevents no measured failure with this build.
4. **The `.var` file** follows `var.cpp:54-64`. It has one row per face whose group belongs to
   an *enabled scene* surface receiver, keyed by the face's `.poly` marker (the `.cbin` face
   index), with one global area. It must be byte-identical to
   `tests/fixtures/upstream/tutorial1/tetgen/scene_mesh.var` for tutorial 1: CRLF, `k  marker area`,
   the area as `f32` printed at 15 significant digits, and no newline after the final `0`.
5. **Box fitting zones get 12 triangles in the `.poly` only.** Their markers are
   `scene_faces + k`, and the `.mbin` builder writes them as -1: a plain tet-to-tet transition.
   Each zone gets one region seed at the box centre, with attribute = its solver id.
   - `Surfaces` zones use the scene's own faces and the zone's `inside_point`.
   - **This departs from upstream**, which appends the box triangles to the `.cbin` as
     material-0 faces with `idEn` set (`Objet3D_maillage.cpp:800-813`).
   - **Physical equivalence is unproven.** No gate runs a fitting zone through a solver before M8.
6. **Internal facets are marked on both sides**, as upstream does (`Objet3D_maillage.cpp:369-381`).
   The `.mbin` invariant is therefore:
   - a face with neighbour -2 has a marker ≥ 0
   - a face with a marker ≥ 0 and a neighbour ≥ 0 has a twin carrying the same marker
   - neighbours are mutual
   - every tet has `(A−D)·((B−D)×(C−D)) < 0`

   `docs/formats/mbin.md:73-75` is rewritten to match, since its "marker ≥ 0 iff neighbour -2"
   holds only without internal facets.
7. **Staleness.** The key is `validate::mesh_input_hash` (FNV-1a-128, `validate.rs:405`), the
   existing stamp. The mesh manifest records it.
   - `run` with an existing mesh refuses with `mesh_out_of_date` when the stamp differs, and
     with `mesh_missing` when the folder has no `tetramesh.mbin` or no OK manifest.
   - sha256 values (the `sha2` crate) are provenance: `.cbin`, `.mbin` and `tetgen.exe`.
     `run` checks the `.mbin`'s sha256 against the manifest to catch a partial or foreign file.
     The `.cbin` sha256 is recorded only, because `run` re-exports the `.cbin` with the
     current materials.
   - Stale outputs are deleted before every mesh: `.poly`, `.var`, `.1.*`, `_skipped.*`,
     `.mbin`, `.cbin`, the manifest and logs. TetGen auto-loads a `<name>.var`
     (`tetgen.cxx:2443-2449`), so a leftover one is silently applied.
   - **M5(d) splits in two:**
     - (d1) Mesh the box, then move a vertex. `run --mesh <dir>` exits 4 with
       `mesh_out_of_date`, and no solver starts.
     - (d2) A re-mesh cancelled at 1 ms leaves no `tetramesh.mbin`, and `run --mesh <dir>`
       exits 4 with `mesh_missing`.
8. **Reason codes are snake_case,** like the validator's and the contract's. The gate's
   uppercase names were shorthand: `MESH_STALE` is `mesh_out_of_date`, and
   `TETGEN_SKIPPED_FACETS` is `tetgen_skipped_facets`. The Tauri shell uppercases at its own
   boundary.
9. **Open decision 7, the particle-loss limit, is still open.** Both limits below are proposed
   and are recorded in every manifest.
   - **Run verdict:** `particle_loss_excess` when a band's (lost by meshing + lost by loops) /
     total exceeds `--loss-limit`, default 0.01. That is the mockup's "limit 1 %".
   - **M6(c) gate:** per band, ours ≤ floor + 4·√max(floor, 1), a 4σ Poisson bound on the
     difference of two counts.
10. **Particle totals follow the contract, not the gate text.** Per band, total ≥ nbparticules
    × sources, and equal except for energetic-mode transmission copies (`solver-contract.md:354-359`).
    The gate's "× bands" is wrong: each band is its own column.
11. **Three calls about what `run-folder` and the run verdict refuse:**
    - **A source spectrum shorter than the computed band set is `band_set_mismatch`,** found
      before launch by a check on the config only. The validator's code already names exactly
      this case (`solver-contract.md:55`, VERIFIED `run_oneband`). No output signal can catch it:
      the run exits 0 with every file present.
    - **A TCR point receiver with `-inf` in its `Direct` column fails with `nonfinite_result`.**
      That is how a source outside the room shows. A receiver truly hidden from every source
      looks identical and also fails. This is open for Burhan and Michael, with a pre-launch
      source-inside check as the alternative.
    - **Core `run::LineClass` has PROGRESS; the Tauri shell's `LineClass` does not.** The shell
      is not changed in M5/M6, so the M9 bindings gate stays green. M11 maps PROGRESS to the
      progress bar at its own boundary.

## Exit codes (CLI)

These are the plan's codes (raw json line 3); M5 and M6 implement 4, 5 and 130.

- **0:** OK
- **2:** usage or validation error
- **3:** geometry refused
- **4:** meshing failed or mesh unusable
- **5:** solver run FAIL or CRASH
- **6:** result verification (M7)
- **130:** cancelled

## CLI surface (integration)

**`simpa mesh <project.simpa | file.poly> --out <dir>`**
- Options: `--json`, `--tetgen <exe>`, `--from-tetgen <dir>`, `--cancel-after-ms <n>`.
- A project must pass `geometry::check` first (exit 3) and the mesh-relevant validator rules
  (exit 2).
- stdout carries the mesh manifest (`--json`) or a summary; TetGen's lines go to stderr.
- Exits: 0, 2, 3, 4 or 130.

**`simpa mesh-verify <dir>`**
- Options: `--json`, `--room-id <n>`, `--fittings <a,b,..>`.
- stdout carries the `DirReport`.
- Exits: 0 on a pass, 4 on a failure.

**`simpa run <project> --solver spps|tcr`**
- Options:
  - `--variant <v>`, `--mesh <dir>`, `--runs <root>`
  - `--loss-limit <f>` (default 0.01)
  - `--cancel-after-ms <n>`, `--cancel-after-progress <p>`
  - `--solver-exe <exe>`, `--tetgen <exe>`, `--json`
- Classified lines stream to stderr as `CLASS  text`.
- stdout carries the run manifest (`--json`) or a one-line verdict. Both name the run folder.
- Exits: 0, 2, 3, 4, 5 or 130.

**`simpa run-folder <dir> --solver spps|tcr`**
- Options: `--runs <root>`, `--solver-exe <exe>`, `--loss-limit <f>`, `--cancel-after-ms <n>`,
  `--cancel-after-progress <p>` and `--json`.
- Exits: 0, 5 or 130.

**Finding the executables.** The first of these that exists is used, and its path and sha256
go into the manifest:
1. `--solver-exe` or `--tetgen`
2. `$SIMPA_SOLVERS_DIR`
3. `<simpa.exe dir>\solvers\`
4. `<simpa.exe dir>\`
5. the nearest `target\solvers\bin\` above `simpa.exe`, for the dev tree

## Layout

```
crates/simpa-core/src/
  process.rs            child processes: Job Object, tagged lines, cancel      (piece: process)
  mesh.rs, mesh/        Mesher trait, TetGen backend, .poly/.var, .mbin build,  (piece: mesher)
                        manifest, stale-file deletion
  mesh/verify.rs        .mbin and TetGen-output invariants, mesh-verify report  (piece: verify)
  run.rs, run/          classifier table, SPPS stats, expected files, verdict,  (piece: classify)
                        run manifest; orchestration added at integration
tests/fixtures/meshes/  committed TetGen sets (broken hall, bad poly)            (piece: fixtures)
tests/fixtures/runs/    negative run folders with expected.json                 (piece: fixtures)
tools/fixture-gen/      the generators that made them                           (piece: fixtures)
```

**Mesh folder** (`simpa mesh <project> --out <dir>`):
- inputs: `scene_mesh.poly` and `scene_mesh.var`
- TetGen outputs: `scene_mesh.1.{node,ele,face,neigh,edge}`, and `scene_mesh_skipped.*` on
  failure
- logs: `tetgen.stdout.txt` and `tetgen.stderr.txt`
- `mesh.cbin`, and `tetramesh.mbin` only on success
- `mesh.json`, the manifest, always written

TetGen runs with cwd = the folder and the relative argument `scene_mesh.poly`, because it opens
files with narrow `fopen` (a `Ł` path works this way, `m1.ps1:117`).

**Run folder** (`simpa run <project> --solver spps|tcr [--mesh <dir>] [--runs <root>]`):
- `<root>/<yyyyMMdd-HHmmss-fff>-<solver>[-n]/` is created with `create_dir`, never reused
- `mesh/` inside it, when no `--mesh` is given
- `solve/`, the solver's fresh working folder: only `config.xml`, `mesh.cbin`,
  `tetramesh.mbin` and `directivities\` before launch (`validate/export.rs:494-523`)
- `run.json` and `solver.{stdout,stderr}.txt`, beside `solve/`, not in it

The solver runs with cwd = `solve/` and the argument `config.xml`.

**`simpa run-folder <fixture-dir> --solver spps|tcr [--solver-exe <exe>]`** copies the fixture
into a fresh run folder, replaces `__RUNDIR__` in `config.xml` with the absolute `solve\` path,
and runs it with no validator. The verdict is the same as `run`'s.

Expected files are derived from the `config.xml` actually in `solve/`, for both `run` and
`run-folder`. That file is the one source of truth.

`run-folder` skips only the *project* validator. Before launch it runs `mesh::verify` on the
folder's `.mbin` and the `.cbin` it indexes, with the room id taken as the most common
`idVolume`. A failure there is a run failure: status FAIL, reason `mesh_invalid` plus the
verifier's codes, exit 5. This is what refuses the broken-hall TCR folder, since TCR itself
exits 0 on it.

**Stub solver.** `simpa-stub-solver.exe config.xml`, a test binary in `crates/simpa`, reads
`stub.json` from its working folder:

```json
{
  "exit_code": 0,
  "lines": [{"stream": "stdout|stderr", "text": "...", "newline": true, "delay_ms": 0}],
  "files": {"relative/path": "contents"}
}
```

It prints the lines in order and creates the files. It covers the classifier rows that no real
run reaches (`config_path_missing`, `degenerate_tetrahedron`, `source_not_located`), plus the
final stderr line without a newline. `run-folder --solver-exe <path>` points at it.

## Interfaces between pieces

**`process`**, used by the mesher and the run manager:
```rust
pub enum Stream { Stdout, Stderr }
pub struct Line { pub stream: Stream, pub t_ms: f64, pub text: String, pub terminated: bool }
pub struct Spec { pub program: PathBuf, pub args: Vec<OsString>, pub cwd: PathBuf }
pub struct CancelToken  // clone + cancel() + is_cancelled()
pub struct Outcome { pub exit_code: Option<u32>, pub cancelled: bool, pub elapsed_ms: f64 }
pub fn run(spec: &Spec, cancel: &CancelToken, on_line: &mut dyn FnMut(&Line)) -> io::Result<Outcome>
```
- `on_line` runs on the caller's thread, in arrival order, and may call `cancel()`.
- Exit codes are raw `u32`, so `0xC0000005` survives.
- The child runs in a Job Object with `KILL_ON_JOB_CLOSE` and `CREATE_NO_WINDOW`, and cancel is
  `TerminateJobObject`.
- A trailing partial line is flushed with `terminated: false`.

**`mesh::verify`**, used by the mesher before writing, by `mesh-verify` and by `run`. The types
are fixed in `crates/simpa-core/src/mesh/verify.rs` (scaffold):
```rust
pub struct VolumeIds { pub room: i32 /* 0 ours, 1 upstream's */, pub fittings: Vec<i32> }
pub fn verify_mesh(mesh: &mbin::Mesh, scene: &cbin::Model, ids: &VolumeIds) -> VerifyReport
pub fn verify_dir(dir: &Path, ids: &VolumeIds) -> Result<DirReport, FormatError>
```
Counts in the report:
- `index_errors`, `degenerate_tets` and `inverted_tets`
- `unmarked_boundary_faces` and `marker_out_of_range`
- `nonmutual_neighbors` and `asymmetric_internal_faces`
- `marker_geometry_mismatches`: a marked face must lie on its scene face, within a tolerance
  scaled to f32 and the model size
- `uncovered_scene_faces`, as a count plus the first 20
- `unknown_volume_ids`, plus the volume per id

`verify_dir` adds `tetgen_skipped_facets` (count and markers), `neigh_missing` and
`tetgen_output_missing`, and accepts any basename (`model.*` as well as `scene_mesh.*`).

**`run` classifier:** a static table equal to `docs/solver-contract.md` Part B, 22 rows (ids,
streams, anchored patterns, classes PROGRESS/INFO/OK/WARN/FAIL). A test fails if the table and
the doc drift apart. The classifier holds the continuation-line state for
`scene_mesh_unreadable` and `tetra_mesh_empty`.

## Gate amendments (to be written into the plan when the gates pass)

- **M5(a):** the box is meshed with its own settings, plus these checks:
  - the `.var` is byte-identical to upstream's tutorial-1 `.var`
  - more than 2 tet faces carry markers 0/1, and each such face has area ≤ 0.1 m² × (1 + 1e-4)
    **(blocked by decision 3)**
- **M5(b):** as in decision 2.
- **M5(c):** `simpa mesh <file.poly>` accepts a raw `.poly`, whose facets then act as the scene.
  The survey's `tg_bad` poly gives exit 4, `tetgen_skipped_facets` with markers [8, 9, 12],
  and no `.mbin`. The committed broken-hall set gives `mesh-verify` exit 4 with
  `tetgen_skipped_facets` (535) and `neigh_missing`.
- **M5(d):** as in decision 7.
- **M5(e):** as in decision 1.
- **M5(g):** also asserts that the cancel hit a running TetGen. The manifest must show
  `tetgen.cancelled` true and `tetgen.exit_code` null. Status CANCELLED alone does not prove it:
  the pipeline also reports CANCELLED when a mesher ignores the cancel. No `.1.ele` may have
  been written. Run it on a release build, since TetGen starts 36-45 ms into the call in a
  debug build.
- **M6(a):** uses a derived fixture, `rooms/tutorial1_box_seeded.simpa`: seed 1 and 10,000
  particles, M1's reference configuration. Totals as in decision 10.
- **M6(c):** uses the derived fixture `rooms/elmia_loss_gate.simpa`: seed 1, bands 125-4,000 Hz
  computed. The floor mesh is upstream's tutorial_2 `.1.*`, taken from the zip at gate time and
  built by `simpa mesh <project> --from-tetgen <dir>`. The tolerance follows decision 9.
- **Every gate:** the room fixtures are `.simpa`, not `.json`, and every gate is launched as
  `powershell -File`, not `pwsh`.
