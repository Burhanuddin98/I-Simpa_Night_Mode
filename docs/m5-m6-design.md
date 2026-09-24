# M5 and M6: design and interfaces

**2026-09-23.** This is the working design for M5 (process layer, meshing, mesh verification)
and M6 (run manager, end-to-end solves). The gates come from `rebuild-plan-raw-2026-09-23.json`,
milestones M5 and M6, with the amendments below. The terrain maps behind these decisions are in
`target/workflows/m5m6-map/` (not committed); every fact here carries its receipt.

## Decisions

1. **Fitting ids stay as M3 set them, and the `.mbin` builder writes TetGen's region attribute
   unchanged, as upstream does.** Reversed on 2026-09-24; until then the builder wrote the room
   as 0.
   - `FIRST_FITTING_ID` stays 2 (`config_xml/ids.rs:20`), and the zone ids stay whatever
     `SolverIds::assign` gives. TetGen 1.5.0's `-A` gives a seeded region its seed's attribute,
     and each region no seed reaches the next attribute above the largest located seed, starting
     at 1, one per region (`tetgen.cxx:22352-22436`). A room without fitting zones is therefore 1,
     as in every upstream tutorial mesh; with fittings 2 and 3 its parts are 4, 5, ...; tutorial
     3's room is three regions, 2084 to 2086, above its fittings 1930 and 2083.
   - The builder writes that attribute as `idVolume` (`mesh/build.rs`), as upstream's GUI does
     (`Objet3D_maillage.cpp:165, 868`). `mesh::verify` checks TetGen's numbering
     (`VolumeIds::tetgen`: the fittings, and the room from one above the largest) and refuses any
     other id as `unknown_volume_ids`, a room written 0 included. `run-folder`, which has no
     project, takes the room from one above the declared fittings or, with none, from the
     smallest id in the mesh: upstream's Python-binding test mesh and Night Mode's broken hall
     carry 0, which the solver also reads as the room (`coreTypes.h:445`).
   - **Why it was reversed.** SPPS gives every tetrahedron with a nonzero `idVolume` the fitting
     of that id, NULL for an id no `encombrement` declares, and hands it to the tetrahedron's scene
     faces in tetrahedron order; 0 is skipped (`coreinitialisation.cpp:151-176`). Upstream's room
     tetrahedra therefore overwrite a fitting on the scene faces they share with it, and a room
     written 0 did not. On tutorial 3 (the parity bed, decision 3), with the room written 0 all 24
     output files of each of its 3 runs differed from the original's; with TetGen's numbering none
     does. Recorded in `DECISIONS.md` (05:13 on 2026-09-24): Burhan was told at 05:03 and did not
     object.
   - **M5(e) reads "the first fitting zone, solver id 2, and the room 3"**, not "id 1" and 0.
   - A hazard upstream shares: when TetGen locates no tetrahedron for a fitting's seed, the room's
     first id can be that fitting's own. `validate_export` refuses such a run before launch
     (`fitting_id_collides_with_room_region`: room tetrahedra, outside every zone, carrying a
     declared fitting id).
2. **The mesher takes its flags from the project's `MeshSettings`.** The flags are
   `[-a<v>] -pq<r> -A -n [-Y]`, in upstream's order (`projet_maillage.cpp:167-170`).
   - Numbers are narrowed to f32 first, as upstream's float settings are (`projet.h:98-99`),
     and then printed like upstream's `Convertor::ToString` (precision 15, classic locale;
     `sppsString.cpp:91-98`). So a q of 1.1 prints as `-pq1.10000002384186`.
   - The room fixtures carry `q 2, preserve_boundary false`, and the box also has a `.var` at
     0.1 m². That gives `-pq2 -A -n`, the same flags as upstream's tutorial_2 mesh.
   - The default `MeshSettings` gives upstream's GUI default, `-pq5 -A -n -Y`.
   - **M5(b):** without `-Y`, the hall's `.1.face` has far more rows than 7,860. Measured with
     TetGen 1.6.0: 36,716 rows; with 1.5.0, upstream's older build and ours since decision 3,
     60,974. The gate instead
     checks three things: every row has a marker ≥ 0, the markers cover all 7,860 scene faces,
     and no marker's geometry mismatches.
3. **The mesher is TetGen 1.5.0, and a parity bed holds our whole pipeline to original
   I-Simpa's.**
   - **Decided by Burhan.** On 2026-09-23 at 22:53 he chose TetGen 1.5.0 ("C: TetGen 1.5.0, if
     the hall passes"), and at 23:45 made it unconditional with the order to build our own
     version, "a proper build, proper and works as a solver should". Upstream's SPPS and TCR stay
     as `929a5c8` has them; our solver inputs must equal what original I-Simpa writes, proven on
     upstream's tutorials. His words are in
     `docs/investigations/2026-09-23-upstream-meshing/DECISIONS.md`.
   - **Why 1.5.0** (`docs/investigations/2026-09-23-upstream-meshing/`, with receipts):
     - The pinned TetGen 1.6.0 never tests a facet's area bound: `check_subface` tests only the
       radius-edge ratio (`tetgen.cxx:27347-27388`). So the `.var` does nothing, and the box
       stays 6 tetrahedra with the floor receiver 2 faces of 30 m². Upstream moved to 1.6.0 in
       PR #275 (2021-04-20); a contributor asked for a revert three days later and had no answer.
       v1.4.0 (January 2026) is the first release to ship it.
     - WIAS TetGen 1.5.0 (tarball sha256 `4d114861…9cf3`, the same files as upstream's
       `4db335c`) is what upstream shipped in 1.3.3 and 1.3.4, and made its 2019 tutorial meshes.
     - On the Elmia hall with a scene receiver, 1.5.0 keeps every receiver face at or under
       0.5 m² (1.6.0: up to 6.14 m²) and loses 6.8e-4 of particles to meshing against 1.6.0's
       5.0e-4, 15 times under the 1 % run limit. Criterion 3 was restated as "within the 1 % run
       limit, and identical to original I-Simpa's meshes" (`confirm150.md`).
     - On the self-intersecting raw hall, 1.6.0 crashes on every flag set; 1.5.0 stops with
       exit 3 and names the pair.
   - **Built and gated.** `third_party/tetgen-1.5.0/` holds the tarball and its files;
     `solvers/build.ps1` and M1 hash the tarball and build it with the command lines of
     upstream's own tetgen target. M1 gates tutorials 1 and 3 byte for byte.
   - **What changed with it:**
     - **The `.var` refines.** The box meshes to upstream's 2019 mesh: 732 nodes, 2,257
       tetrahedra, 934 floor faces of at most 0.0998 m². M5(a)'s area check runs
       (`mesh_project.rs::the_var_refines_the_receiver_faces`) beside its refusal: the same box
       without its `.var` gives 60 tetrahedra and a floor face of 13.43 m².
     - **Self-intersections stop TetGen.** 1.5.0 writes no `_skipped.face`; it exits 3 at the
       first self-intersection, sometimes naming the pair (on `tg_bad`; not on the box with a
       piercing baffle, nor with two overlapping zones). The mesher reports
       `tetgen_self_intersection` with that pair and the findings of a `tetgen -d` follow-up
       (which 1.5.0 runs to exit 0, writing the intersecting triangles as a `.1.face`), mapped to
       scene faces, groups and box zones (`docs/formats/mesh-manifest.md`). A 1.6.0
       `_skipped.face`, such as the committed broken hall's, still gives `tetgen_skipped_facets`.
     - **The `.mbin` takes upstream's corner order**, `(d,c,b,a)`, and the `float` GL round trip
       (`docs/formats/mbin.md`). SPPS then locates tutorial 1's source, by a margin of 2.4e-7,
       and `run::locate` refuses before launch any source or point receiver SPPS would lose
       (`source_unlocatable`, `receiver_unlocatable`).
     - **Gate M6(a)** is judged against upstream's own mesh (Burhan, 2026-09-24 05:13, "No worse
       than upstream"): the seeded box runs OK and loses 1 particle of 10,000 at 2 kHz, and so does
       the same project, seed and config on upstream's 2019 mesh of the box, byte for byte its
       `tetramesh.mbin`. The gate text's "none lost" is replaced by that comparison, as M6(c)
       judges the hall.
   - **The parity bed**, `crates/simpa/tests/parity_tutorials.rs`, run by
     `tools/gates/parity.ps1`. For upstream's tutorials 1, 2 and 3: `simpa import-proj`, then
     `simpa mesh`, then our files against the ones original I-Simpa wrote into the `.proj`, then
     SPPS and TCR with one seed on our inputs and on the original's, every output file compared
     (`.csbin` decoded, the rest byte for byte). Measured on 2026-09-24:

     | | Inputs | Same-seed results |
     |---|---|---|
     | Tutorial 1 | `.poly`, `.var`, TetGen's `.1.*` and the `.mbin` byte-identical, `idVolume` included; `config.xml` and `.cbin` the original's but the ids, the stored receiver directions and the listed differences by design | TCR: 15 of 15 files identical. SPPS: 15 of 17, and the two `Advanced sound level.gap` differ in the last bits; with the run's own receiver directions, 17 of 17 |
     | Tutorial 2 | `.poly` and `.1.node` equal in every value; 307 and 1,036 lines differ in text at exact decimal ties (below); the other `.1.*` byte-identical | none: the `.proj` holds no run folder |
     | Tutorial 3 | `simpa import-proj` reads it (2026-09-24: fitting zones, per-band laws and transmission, source groups); not meshable by us (below). Our TetGen on upstream's own `.poly` gives its `.1.*` byte for byte, and our builder its `.mbin`; `config.xml` and `.cbin` from the `.proj` the original's, `.cbin` byte for byte, once upstream's ids are read through the map the import records | 3 runs: 24 of 24 files identical in each, on our `.proj`'s config and `.cbin`, with the fittings under our ids and the room as TetGen numbers it above them (decision 1). The refusal: the room written 0, as the builder wrote it until 2026-09-24, gives different results |

   - **What does not match**, each measured:
     1. **Tutorial 3 cannot go through our mesher.** `simpa import-proj` reads it since
        2026-09-24, and `simpa mesh` refuses the project: TetGen stops on the box standing on
        the floor (`tetgen_self_intersection`, exit 4); the scene its runs hold, imported from a
        run's `config.xml` and `.cbin`, is refused by the geometry check (`self_intersections`,
        `open_boundary`, `unresolved_topology`). Upstream's GUI took its `.poly` through
        `preprocess.exe` (the mesh settings' "preprocess", `projet_maillage.cpp:206-213`): its
        `.poly` has 57 vertices and 133 facets for the run's 100 faces. Our `.poly` of the same
        scene, taken through our build of `preprocess.exe`, has 56 and 140. Running
        `preprocess.exe` in our mesher, and what our geometry check should accept before it, is
        not built: Burhan chose to investigate tutorial 3 first (05:13 on 2026-09-24, "i think we
        should investigate tutorial 3"; `DECISIONS.md`).
     2. **Decimal ties in text.** A coordinate at an exact tie in the 17th significant digit is
        printed rounded to even by programs built with Visual Studio 2019 16.2 or later, and up
        by older ones. Our `.poly` writer and our TetGen build round to even; the 2019 files
        round up. The values are equal. Linking TetGen with `legacy_stdio_float_rounding.obj`
        (`third_party/tetgen-1.5.0/PROVENANCE.md`) and rounding ties up in our writer would give
        the 2019 text; the default follows upstream's own 1.3.4 TetGen, which prints as ours
        does. Open for Burhan.
     3. **Receiver directions: a difference from the stored runs, not from what original I-Simpa
        writes from this `.proj`.** The GUI saves a direction with `ostream`'s default 6 digits
        (`e_data_float.h:163, 249-257`: tutorial 1 stores `-0.383357`), reads that text back on
        load (`:65-66`), and recomputes a direction only when a position is edited, never while
        loading (`e_scene_recepteursp_recepteur.h:63, 105, 195-219`). Its runs were written in the
        2019 session, from the direction computed at full precision (`-0.3833573`). Reopened, the
        original writes the 6-digit value, as we do (`parity_inputs.rs::tutorial1_directions`).
     4. **Element ids.** Upstream's are GUI session ids; they reach no output but the `.csbin`
        `xmlIndex` (`docs/formats/config_xml.md`).
   - **Upstream's shipped solvers against ours** (the bed's `shipped_…` test, tutorial 1, seed 1,
     10,000 particles, 500, 1000 and 2000 Hz):
     - **v1.4.0** (`929a5c8`, the pin): SPPS 17 of 17 and TCR 15 of 15 files identical to ours.
     - **v1.3.4** (2020-12-23, the last stable release): TCR identical but for the sign of 4 NaNs
       in `Main results.gabe`. SPPS: the particle statistics and `Total energy.recp` identical (the
       same particle fates, inferred); 15 of 17 files differ. Point receivers' levels differ by at most
       1.9e-4 relative (`Sound level.recp` and `Advanced sound level.gap`), per source by 4.1e-6,
       and their intensity components by up to 3.1e-3 relative (small components, 1.8e-13
       absolute at most; ratio taken over every value, not only those above 1e-3 of the file's
       largest). Surface receivers differ far more: single time-step records by up to 98 %, and
       each face's energy summed over time by up to 84 % (2 kHz). Why the surface receivers differ
       between the two releases is not examined.
   - `mesh_settings_conflict` (`.var` together with `-Y`) stays as a project-stage error. It
     keeps parity with upstream's GUI, which clears `-Y` whenever the constraint is on
     (`e_core_core_tetconf.h:82-90`), and with 1.5.0 `-Y` would forbid the splits the `.var`
     asks for.
4. **The `.var` file** follows `var.cpp:54-64`. It has one row per face whose group belongs to
   an *enabled scene* surface receiver, keyed by the face's `.poly` marker (the `.cbin` face
   index), with one global area. It must be byte-identical to
   `tests/fixtures/upstream/tutorial1/tetgen/scene_mesh.var` for tutorial 1: CRLF, `k  marker area`,
   the area as `f32` printed at 15 significant digits, and no newline after the final `0`.
5. **Box fitting zones get 12 triangles in the `.poly` only.** Their markers are
   `scene_faces + k`, and the `.mbin` builder writes them as -1: a plain tet-to-tet transition.
   Each zone gets one region seed at the box centre, with attribute = its solver id.
   - `Surfaces` zones use the scene's own faces and the zone's `inside_point`.
   - **This departs from upstream**, which marks the box's triangles with their `.cbin` faces
     (Part 5 of its `.poly`, through `preprocess.exe`). Since 2026-09-24 our `.cbin` carries
     them as upstream's does, material-0 faces with `idEn` set after the room's
     (`Objet3D_maillage.cpp:800-813`; `config_xml::scene_mesh`, tutorial 3 byte for byte), and
     `config.xml` declares material 0 for them; the mesher's `.poly` markers still index the
     room's faces only (`config_xml::room_mesh`), so no `.mbin` marker names a box face.
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
- Options: `--json`, `--room-id <n>`, `--fittings <a,b,..>`. Without `--room-id` the room's
  first id is TetGen's, one above the largest fitting or 1 (decision 1).
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
- TetGen outputs: `scene_mesh.1.{node,ele,face,neigh,edge}`; after a self-intersection, TetGen
  1.5.0 writes none of them, and `diag/` holds the `-d` follow-up (TetGen 1.6.0 wrote
  `scene_mesh_skipped.*` instead)
- logs: `tetgen.stdout.txt` and `tetgen.stderr.txt`
- `mesh.cbin`, and `tetramesh.mbin` only on success
- `mesh.json`, the manifest, always written

TetGen runs with cwd = the folder and the relative argument `scene_mesh.poly`, because it opens
files with narrow `fopen` (a `Ł` path works this way, `m1.ps1:117`).

**Run folder** (`simpa run <project> --solver spps|tcr [--mesh <dir>] [--runs <root>]`):
- `<root>/<yyyyMMdd-HHmmss-fff>-<solver>[-n]/` is created with `create_dir`, never reused
- `mesh/` inside it, when no `--mesh` is given
- `solve/`, the solver's fresh working folder: only `config.xml`, `mesh.cbin`,
  `tetramesh.mbin` and `loudspeakers\` (upstream's folder name, `config_xml::names::DIRECTIVITY_DIR`)
  before launch (`validate/export.rs:494-523`)
- `run.json` and `solver.{stdout,stderr}.txt`, beside `solve/`, not in it

The solver runs with cwd = `solve/` and the argument `config.xml`.

**`simpa run-folder <fixture-dir> --solver spps|tcr [--solver-exe <exe>]`** copies the fixture
into a fresh run folder, replaces `__RUNDIR__` in `config.xml` with the absolute `solve\` path,
and runs it with no validator. The verdict is the same as `run`'s.

Expected files are derived from the `config.xml` actually in `solve/`, for both `run` and
`run-folder`. That file is the one source of truth.

`run-folder` skips only the *project* validator. Before launch it runs `mesh::verify` on the
folder's `.mbin` and the `.cbin` it indexes, with the config's fittings and the room from one
above the largest of them, or, with none, from the smallest `idVolume` (decision 1). A failure there is a run failure: status FAIL, reason
`mesh_invalid` plus the verifier's codes, exit 5. This is what refuses the broken-hall TCR
folder, since TCR itself exits 0 on it. A missing `.mbin` or an unreadable `.cbin` is
`mesh_invalid` alone. The config-only band check of decision 11 runs beside it:
`band_set_mismatch`, exit 5. `docs/solver-contract.md` Part B, "The run manager", is the
reference.

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
pub struct VolumeIds { pub room: i32 /* the room's first id, TetGen's numbering */, pub fittings: Vec<i32> }
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
- `unknown_volume_ids` (an id below `room` that is no fitting's), plus the volume per id

`verify_dir` adds `tetgen_skipped_facets` (count and markers), `neigh_missing` and
`tetgen_output_missing`, and accepts any basename (`model.*` as well as `scene_mesh.*`).

**`run` classifier:** a static table equal to `docs/solver-contract.md` Part B, 22 rows (ids,
streams, anchored patterns, classes PROGRESS/INFO/OK/WARN/FAIL). A test fails if the table and
the doc drift apart. The classifier holds the continuation-line state for
`scene_mesh_unreadable` and `tetra_mesh_empty`.

## Gate amendments (in force: the gates passed 2026-09-24, merged as `34d1d5f`; `docs/rebuild-plan.md` points here)

- **M5(a):** the box is meshed with its own settings, plus these checks:
  - the `.var` is byte-identical to upstream's tutorial-1 `.var`
  - more than 2 tet faces carry markers 0/1, and each such face has area ≤ 0.1 m² × (1 + 1e-4)
    (TetGen 1.5.0, decision 3; the same box without its `.var` must fail it)
- **M5(b):** as in decision 2.
- **M5(c):** `simpa mesh <file.poly>` accepts a raw `.poly`, whose facets then act as the scene.
  The survey's `tg_bad` poly gives exit 4, `tetgen_self_intersection` naming facets [8, 9, 12]
  in the pairs [8, 12] and [9, 12], mapped to scene faces [8, 9, 12], and no `.mbin` (TetGen
  1.5.0 stops and writes no `_skipped.face`). The committed broken-hall set, TetGen 1.6.0's
  output, gives `mesh-verify` exit 4 with `tetgen_skipped_facets` (535) and `neigh_missing`.
- **M5(d):** as in decision 7.
- **M5(e):** as in decision 1.
- **M5(g):** also asserts that the cancel hit a running TetGen. The manifest must show
  `tetgen.cancelled` true and `tetgen.exit_code` null. Status CANCELLED alone does not prove it:
  the pipeline also reports CANCELLED when a mesher ignores the cancel. No `.1.ele` may have
  been written. Run it on a release build, since TetGen starts 36-45 ms into the call in a
  debug build.
- **M6(a):** uses a derived fixture, `rooms/tutorial1_box_seeded.simpa`: seed 1 and 10,000
  particles, M1's reference configuration. Totals as in decision 10. **Losses are judged against
  upstream's own mesh of the box** (Burhan, 2026-09-24 05:13): per band, lost by meshing and lost
  by loops no higher than the same project, seed and config give on upstream's 2019 tutorial-1
  mesh, built with `--from-tetgen` from its 2019 TetGen output and checked byte for byte against
  its `tetramesh.mbin`. Measured: both lose 1 particle at 2 kHz. The control is the box on TetGen
  1.6.0's 6-tetrahedron mesh in upstream's corner order, which loses none; judged against that
  control, the box's own run is refused at 2 kHz (`cli_run.rs::spps_runs_the_seeded_box_ok`,
  `tools/gates/m6.ps1`).
- **Parity:** `tools/gates/parity.ps1` runs the parity bed of decision 3 and its refusals.
- **M6(c):** uses the derived fixture `rooms/elmia_loss_gate.simpa`: seed 1, bands 125-4,000 Hz
  computed. The floor mesh is upstream's tutorial_2 `.1.*`, taken from the zip at gate time and
  built by `simpa mesh <project> --from-tetgen <dir>`. The tolerance follows decision 9.
- **Every gate:** the room fixtures are `.simpa`, not `.json`, and every gate is launched as
  `powershell -File`, not `pwsh`.
