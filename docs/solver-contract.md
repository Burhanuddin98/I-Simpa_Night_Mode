# Solver contract: pre-launch rules and the run contract

This page is the contract between our core and upstream's unchanged solvers, SPPS
(`spps.exe`) and TCR (`classicalTheory.exe`), built from `929a5c8` (M1). It has two parts:

- **Part A, the pre-launch rules.** Each has a stable reason code. `core::validate` implements
  them, and **the codes are its API.**
- **Part B, the run contract:** how a solver is launched, what it must find and what it writes,
  what its exit codes and output lines mean, and how a run is judged.

The file format itself is in `docs/formats/config_xml.md`. Receipts follow that page's
conventions: short source names, paths under `target/solvers/src-929a5c8/src/`, and the
**P1**, **P2 `<case>`** and **S `<case>`** runs. This page adds these short names:

| Short name | Path |
|---|---|
| `progressionInfo.h` | `lib_interface/input_output/progressionInfo.h` |
| `part_binary.h` | `lib_interface/input_output/particles/part_binary.h` |
| `processManager.cpp` | `isimpa/manager/processManager.cpp` (upstream's GUI) |
| `projet.cpp`, `projet_maillage.cpp` | `isimpa/data_manager/` (upstream's GUI) |
| `e_core_core_tetconf.h`, `e_core_sppscore.h` | `isimpa/data_manager/tree_core/` (upstream's GUI) |
| `e_data_row_materiau.h` | `isimpa/data_manager/e_data_row_materiau.h` (upstream's GUI) |
| `e_scene_sources_source.h` | `isimpa/data_manager/tree_scene/e_scene_sources_source.h` (upstream's GUI) |
| `tetgen.h` | `tetgen/tetgen.h` |

## Part A: pre-launch rules

### Conventions

- **Codes** are `lower_snake_case`. They are unique across this page, Part B's codes included,
  and stable: a code is never reused or renamed. A code that has to go is marked retired and
  kept.
- **Stage.**
  - `project` rules are checked on the typed project, together with its geometry and mesh,
    before export.
  - `export` rules are checked on the exact files the exporter produced (`config.xml`,
    `mesh.cbin` and `tetramesh.mbin`) immediately before launch. They guard against writer bugs
    and hand-built run folders.
- **Severity.**
  - An `error` blocks the run. The CLI exits 2 (plan, `cli`).
  - A `warning` is reported and recorded in the run manifest, and the run proceeds.
  - No rule turns a solver failure into a warning.
- **Receipts.** Each rule names the solver behaviour it prevents, with a receipt. "Silent" means
  exit 0 with no FAIL-class line (Part B).

### The rules

#### Bands

| Code | Stage | Severity | Rule | What it prevents |
|---|---|---|---|---|
| `band_set_empty` | project | error | The project has at least one band | With no bands both solvers compute nothing and exit normally. SPPS's loss check then divides 0 by 0 and stays silent (`base_core_configuration.cpp:98-118`; `sppsNantes.cpp:369-382, 437`) (inferred) |
| `band_duplicate` | project | error | No two bands share a centre frequency | Both copies are computed and their `<f> Hz` folders collide, so one overwrites the other, silently. VERIFIED P2 `dup_freq`: two `1000 Hz` statistics columns and 63 files instead of 65 |
| `band_frequency_not_integer` | project | error | Every band centre frequency is a positive whole number of hertz | `@freq` is read with `atoi`, so 31.5 Hz becomes a 31 Hz band and a folder named `31 Hz`. It then misses a directivity block keyed 31.5 (`base_core_configuration.cpp:109`; `coreString.cpp:84-87`; `directivityBalloon.cpp:49-52`) (inferred) |
| `band_set_mismatch` | project | error | Every source, material and fitting zone carries exactly the project's band set | Spectra are mapped to bands by position, never by frequency. A short source spectrum is read past its end, silently. VERIFIED S `run_oneband`: the 1000 Hz level was applied to 500 Hz. VERIFIED P2 `tcr_oneband_src`: NaN and -inf levels, exit 0. A short material spectrum is also read out of bounds (`CalculationCore.cpp:231`), and a fitting's missing bands are left uninitialised (`coreTypes.h:337`) (both inferred) |

#### Materials

| Code | Stage | Severity | Rule | What it prevents |
|---|---|---|---|---|
| `material_unassigned` | project | error | Every surface group has a material in the active variant | Where a face's material is undeclared, both solvers print `Wrong project configuration…` on stderr and exit -1 (VERIFIED P2 `mat22_miss`, `tcr_mat22_miss`). For material id 0 on the first faces there is no check at all, and SPPS crashes `0xC0000005` (VERIFIED S `run_mat0miss`; `coreinitialisation.cpp:409-434`) |
| `material_value_out_of_range` | project | error | 0 ≤ α ≤ 1, 0 ≤ diffusion ≤ 1, transmission loss R ≥ 0 dB, all finite | The solvers check none of these. Energetic mode multiplies energy by (1 - α) and by τ = 10^(-R/10) (`CalculationCore.cpp:249-285`), so α > 1 or R < 0 negates or creates energy. Random mode compares α and diffusion with a uniform draw (`CalculationCore.cpp:288-300, 318`) (inferred) |
| `material_diffusion_ignored` | project | warning | α = 1 while diffusion > 0: the diffusion value has no effect | An α = 1 hit never reflects (`CalculationCore.cpp:249-261`). Upstream's GUI forces diffusion to 0 in that case (`e_data_row_materiau.h:116-120`) |
| `material_transmission_exceeds_absorption` | project | warning, corrected | The transmission coefficient τ = 10^(-R/10) must not exceed α, and transmission needs α > 0. The exporter writes τ = α (R = -10·log10 α) and says so | Random mode transmits an absorbed particle when rand·α ≤ τ, so τ > α acts as τ = α (`CalculationCore.cpp:292`). Energetic mode keeps (1 - α) of the energy and adds a τ copy, which creates energy when τ > α (`CalculationCore.cpp:262-285`). With α = 0 no transmission happens in either mode (`CalculationCore.cpp:262, 288`). Upstream's GUI makes the same correction, with a warning (`e_data_row_materiau.h:128-143, 185-197`) |

#### Sources and receivers

| Code | Stage | Severity | Rule | What it prevents |
|---|---|---|---|---|
| `source_none` | project | error | At least one source is enabled | An empty `<sources>` runs, emits nothing and exits 0, and TCR's level becomes 10·log10(0) (`base_core_configuration.cpp:121-183`; `TC_CalculationCore.cpp:142-158`) (inferred). Upstream's GUI refuses such a run (`projet.cpp:649-653`) |
| `source_outside_volume` | project | error | Every source is strictly inside the room volume | SPPS dereferences a NULL tetrahedron and crashes `0xC0000005` with no message. TCR computes and exits 0 (VERIFIED P2 `src_out`, `tcr_src_out`; S `run_srcout`; `sppsInitialisation.cpp:13-20`). SPPS's own guard at `sppsNantes.cpp:58-65` is never reached |
| `source_near_surface` | project | error | Every source is at least a clearance d_min from every model face. The value is proposed at 1 mm and open; the solver's own test uses 10⁻⁴ m (`mathlib.h:58`) | When SPPS's on-face check fires, it prints to stderr and exits 0 without results (`sppsNantes.cpp:322-325`). When it does not fire, a point on a face belongs to either adjacent tetrahedron (`coreinitialisation.cpp:79-92`). VERIFIED P2 `src_face`: a source on the floor plane passed the check silently. A source exactly on a mesh vertex is moved 0.5 % of the way towards the tetrahedron's centre (`sppsInitialisation.cpp:13-34`) |
| `receiver_outside_volume` | project | error | Every point receiver is strictly inside the room volume | The receiver is never linked to a tetrahedron and records zero, silently. VERIFIED P2 `rcv_out`: `.recp` sum 0.0 (`coreinitialisation.cpp:178-213`; `sppsInitialisation.cpp:82`) |
| `receiver_sphere_crosses_surface` | project | warning | A point receiver's sphere of radius `rayon_recepteurp` does not cross a model face | SPPS normalises by the full sphere volume 4/3·π·r³ but collects energy only in tetrahedra it reaches inside the room, so a sphere cut by a wall reads low (`sppsInitialisation.cpp:43-93`) (inferred) |
| `receiver_radius_invalid` | project | error | The receiver radius is > 0 and finite | r = 0 gives NaN in every `.recp` value, silently (VERIFIED P2 `radius0`; `sppsInitialisation.cpp:79, 86`) |
| `direction_vector_zero` | project | error | Direction vectors are non-zero: those of unidirectional and directivity-balloon sources, and every point receiver's | Vectors are divided by their length with no check, so a zero vector gives 0/0 = NaN (`base_core_configuration.cpp:137-138, 248-249`). A receiver's orientation feeds SPPS's lateral-energy terms (`spps/reportmanager.cpp:222`) (inferred) |

#### Directivity

| Code | Stage | Severity | Rule | What it prevents |
|---|---|---|---|---|
| `directivity_file_missing` | project | error | A directivity-balloon source names a directivity file, and the file exists | Without the attribute the directivity is NULL, and SPPS crashes `0xC0000005` (VERIFIED P2 `dir_noattr`, S `run_dirempty`; `sppsNantes.cpp:84`). With the file missing, stdout gets `DirectivityBalloon : File not open` and the source emits nothing in any band, exit 0 (VERIFIED S `run_dirmiss`; `directivityParser.cpp:45-47`) |
| `directivity_file_invalid` | project | error | The file parses: every data row has exactly 38 comma-separated fields, every value is a number, φ runs from 0 to 355 in 5° steps with all 72 rows present for each frequency, and θ covers 0 to 180° | A non-numeric field makes `std::stod` throw, and both solvers abort `0xC0000409` (VERIFIED P2 `dir_nan`, `tcr_dir_nan`). Rows with any other field count are skipped silently: a file whose rows all end with a comma gives a source that emits nothing (VERIFIED P2 `dir_badrow`; `directivityParser.cpp:62-77`). A missing φ row sends the interpolation into an empty map (`directivityBalloon.cpp:88-96`) (inferred) |
| `directivity_band_missing` | project | error | The file has a `"Frequency"` block, at the exact centre frequency, for every project band | A band with no block is skipped for that source: zero particles and zero energy, and the only message needs `-v`. VERIFIED P2 `dir_partial`: totals of 0 from 10 to 20 kHz, exit 0 (`sppsNantes.cpp:84-88`; `directivityBalloon.cpp:49-52`) |

#### Time

| Code | Stage | Severity | Rule | What it prevents |
|---|---|---|---|---|
| `time_step_invalid` | project | error | Time step > 0 and duration > 0, both finite | ceil(d/0) does not fit in an int. VERIFIED P2 `no_dt`: SPPS printed `Xml Property pasdetemps doesn't exist !` and aborted `0xC0000409` after 13 s (`base_core_configuration.cpp:94`) |
| `step_count_overflow` | project | error | ceil(duration / time step) < 65,536 | A particle's step counter is a u16 (`sppsTypes.h:69`; `CalculationCore.cpp:49, 85`), and `.pbin` and `.csbin` store u16 steps (`part_binary.h:61`; `rsbin.h:114-115`). Past 65,535 steps the counter wraps (inferred), as the delay's does in the next rule |
| `source_delay_invalid` | project | error | 0 ≤ delay < duration, and ceil(delay / time step) < 65,536 | The start step is cast to u16 (`sppsNantes.cpp:91`). VERIFIED P2 `delay_wrap`: a 655.37 s delay in a 2 s run emitted from step 1 instead of never. A delay at or after the end emits nothing, silently (`sppsNantes.cpp:93`) |

#### SPPS settings

| Code | Stage | Severity | Rule | What it prevents |
|---|---|---|---|---|
| `trans_epsilon_invalid` | project | error | The particle extinction exponent is > 0 and finite. Upstream's GUI allows 0 to 10, default 5 (`e_core_sppscore.h:52`) | With 0 a particle dies as soon as its energy is at or below its starting energy, silently (`sppsNantes.cpp:75`; `CalculationCore.cpp:55-60, 305`). In random mode, upstream's default, every particle dies at its first surface hit. VERIFIED P2 `eps_short`: total energy 0 from the second step on. In energetic mode with air absorption every particle dies at the first step (VERIFIED S `run_noeps`) |
| `particle_count_invalid` | project | error | 1 ≤ particles per source per band ≤ 2³¹ - 1, and 0 ≤ particles saved ≤ particles per source | SPPS silently raises `nbparticules` below 1 to 1 (`spps/core_configuration.cpp:21-23`). A saved count above `nbparticules` fails the save ratio's [0, 1] test, so no particle is saved while the `.pbin` header is sized for them (`sppsNantes.cpp:67-72, 332`) (inferred) |

#### Environment

| Code | Stage | Severity | Rule | What it prevents |
|---|---|---|---|---|
| `atmosphere_invalid` | project | error | Temperature > -273.15 °C, pressure > 0 Pa, 0 ≤ relative humidity ≤ 100 %, and z0 > 0 whenever a sound-speed gradient (alog or blin) is set | c = 343.2·√(T/293.15) and ρ = P·M/(R·T) (`Celerite_du_son.cpp:46`; `Masse_volumique_air.cpp:51`), so T ≤ 0 K gives NaN. P = 0 gives ρ = 0 and an infinite ISO 9613-1 term (`Coef_Att_Atmos.cpp:59`). The gradient uses ln(1 + z/z0) (`spps/core_configuration.cpp:77-83`). With no environment at all, every particle is lost by meshing problems, exit 0 (VERIFIED P2 `no_atmo`) |
| `absatmo_invalid` | project | error | A user-set air absorption has an explicit unit (dB/m or 1/m energy attenuation) and is finite and ≥ 0. The exporter converts it to 1/m | The solver uses `@absatmo` verbatim, as 1/m, for every band, while its own ISO path converts dB/m by ln10/10 (`base_core_configuration.cpp:111-114`). A dB/m value is therefore 4.34 times too strong. VERIFIED P2 `tcr_absatmo`: 0.01 gave TR = 0.163·V/(4·0.01·V + A) = 0.582143 s exactly |

#### Names

| Code | Stage | Severity | Rule | What it prevents |
|---|---|---|---|---|
| `name_too_long` | project | error | Source names and point-receiver labels are at most 49 bytes in UTF-8 | A source name is copied with `strcpy` into a 50-byte GABE cell (`gabe.cpp:174-178`; `gabe.h:230-233`; `spps/reportmanager.cpp:593-595`). VERIFIED P2 `name_long`: a 120-byte name overflowed the heap and left an unterminated cell, and SPPS still exited 0. Labels have the same limit because they become path components (`output_path_too_long`) |
| `name_not_filename_safe` | project | error | Source names and point-receiver labels are valid Windows file names: not empty; no control characters; none of `< > : " / \ \| ? *`; no trailing space or dot; not a reserved device name (CON, PRN, AUX, NUL, COM1-9, LPT1-9) | Labels become SPPS folder names and TCR file names unvalidated (`baseReportManager.cpp:193-203`; `ctr/reportmanager.cpp:148`). Source names become folder names when per-source output is on (`spps/reportmanager.cpp:650`). UTF-8 is fine (VERIFIED P2 `label_utf8`). No solver passes a label to `printf`: grep of `spps/` and `ctr/`. The `%` trap is in upstream's GUI only |
| `name_duplicate` | project | error | Point-receiver labels are unique, compared case-insensitively. So are source names | SPPS resolves a clash by appending 0 to 19, silently: VERIFIED P2 `label_dup` gave `Receiver 10` (`baseReportManager.cpp:196-199`). TCR writes `<lbl>.gabe`, and the second overwrites the first: VERIFIED P2 `tcr_label_dup` gave one file for two receivers. NTFS names are case-insensitive. Duplicate source names collide in per-source folders (`spps/reportmanager.cpp:650`) (inferred) |

#### Surface receivers, cutting planes and fittings

| Code | Stage | Severity | Rule | What it prevents |
|---|---|---|---|---|
| `surface_receiver_empty` | project | error | Every surface receiver covers at least one model face | The `Global\` surface-receiver file is skipped whenever the *first* declared receiver has no faces, even if later ones have some (`baseReportManager.cpp:393-394, 432-433`) (inferred) |
| `cutting_plane_invalid` | project | error | A cutting plane's corners A, B and C are not collinear, and its resolution is > 0 and no larger than its sides | The cell count is ceil(\|BC\|/res) × ceil(\|BA\|/res), unchecked (`base_core_configuration.cpp:288-298`). res = 0 divides by zero, and a zero-length side gives zero cells and a 0/0 cell size (inferred) |
| `fitting_parameters_invalid` | project | error | In every band, 0 ≤ the fitting's α ≤ 1 and λ (the mean free path) > 0 | The free path drawn is -λ·ln(1 - u) (`CalculationCore.cpp:37`), so λ ≤ 0 means a collision at every step or never. An α outside [0, 1] negates or creates energy (`CalculationCore.cpp:137-151`) (inferred) |

#### Project integrity

| Code | Stage | Severity | Rule | What it prevents |
|---|---|---|---|---|
| `variant_reference_invalid` | project | error | The active variant overrides only surface groups that exist, and only with materials that exist | A dangling override leaves the group without a material in the exported config, which is `material_unassigned`'s failure: exit -1 or `0xC0000005` |
| `mesh_out_of_date` | project | error | The tetrahedral mesh was built from the exact scene geometry being exported: the hashes match | A `.mbin` has no magic number, no version and no link to its `.cbin` (`docs/formats/mbin.md`). Face markers index the `.cbin`'s face array with no bounds check (`coreTypes.cpp:223-225`), so a stale mesh reads the wrong faces or past the end (inferred). This is a trap in `docs/upstream-laydown.md` |

#### Export checks

| Code | Stage | Severity | Rule | What it prevents |
|---|---|---|---|---|
| `working_directory_invalid` | export | error | `workingdirectory` is the absolute path of a fresh run folder, ends in `\`, exists, and holds only the exporter's inputs | Names are appended with no separator. VERIFIED P2 `wd_nosep`: `Unable to read the scene mesh file`, exit 0. An empty value makes SPPS abort `0xC0000409` after the solve (VERIFIED P2 `wd_empty`). A reused folder is overwritten silently, and receiver folders gain suffixes (`baseReportManager.cpp:196-199`) |
| `output_path_too_long` | export | error | Every path a solver will create is shorter than 260 UTF-16 code units. The longest is working directory + result folder + band folder + label + file name | The solvers open plain wide paths without the `\\?\` prefix (`std_tools.cpp:46`; `gabe.cpp:463`), so the Windows `MAX_PATH` limit applies unless long paths are enabled for the process (inferred) |
| `config_attribute_missing` | export | error | `config.xml` contains every attribute that `docs/formats/config_xml.md`'s Writer column requires for that solver | Each missing attribute prints `Xml Property <name> doesn't exist !` and silently becomes 0 or empty (`cxml.cpp:108-118`). Upstream's own tutorial-1 TCR config trips this on `directivities_directory` (VERIFIED P1) |
| `docalc_not_literal_one` | export | error | Every computed band is written `docalc="1"`, exactly | Only the string `"1"` computes a band, and `"true"` drops it silently. VERIFIED P2 `docalc_true` (`base_core_configuration.cpp:106`) |
| `config_value_format` | export | error | Numbers are plain C-locale literals: integers as decimal integers, reals with `.` and no grouping separator. Enumerations are in range: `directivite` 0-5, `loi` 0-5, `loi_diff` 0-2, `surf_receiv_method` 0-1, `computation_method` 0-1 | `atoi` stops at the first non-digit, so `"1e3"` reads 1. `atof` runs after the first `,` becomes `.`, so `"1,000.5"` reads 1.0 (`coreString.cpp:84-105`). `loi` 6 (semi-diffuse) is declared but falls to the default branch and reflects specularly (`coreTypes.h:83-92`; `dotreflection.h:23-45`). A `loi_diff` outside 0-2 leaves the direction unchanged (`CalculationCore.cpp:166-182`). An unknown source type matches no emission branch (`sppsNantes.cpp:107-127`) (inferred) |
| `solver_id_mapping_invalid` | export | error | Solver ids are unique within materials, within surface receivers and within fittings. Every id used by a face (idMat, idRs, idEn) or by a tetrahedron (idVolume ≠ 0) is declared | Lookups return the first match, so a duplicate id silently hides the second item (`base_core_configuration.cpp:374-391`). An undeclared idMat exits -1 or crashes (`material_unassigned`). An undeclared idRs indexes a vector at -1 (`coreinitialisation.cpp:291-304, 353-360`). An undeclared fitting id drops the fitting silently (`coreinitialisation.cpp:151-176, 437-445`) (inferred) |
| `fitting_id_collides_with_room_region` | export | error | No fitting's solver id equals the TetGen region attribute of the room's own tetrahedra | Every tetrahedron whose idVolume is not 0 gets the fitting with that id (`coreinitialisation.cpp:151-176`). TetGen `-A` gave all 2,257 of tutorial 1's room tetrahedra idVolume 1 (read from `tests/fixtures/upstream/tutorial1/spps/tetramesh.mbin` on 2026-09-23), so a fitting with id 1 would fill the whole room (inferred) |

**Count: 40 rules.** 33 are `project` rules and 7 are `export` rules; 37 are errors and 3 are
warnings. Every item in `plan.components[core::validate]` maps to a rule:

| Plan item | Rule(s) |
|---|---|
| same band set everywhere, no duplicate frequencies | `band_set_mismatch`, `band_duplicate`, `band_set_empty`, `band_frequency_not_integer` |
| `docalc` as the literal `'1'` | `docalc_not_literal_one` |
| material 0 defined; every idMat, idRs and idEn declared | `material_unassigned`, `solver_id_mapping_invalid` |
| sources and receivers strictly inside and off every face | `source_outside_volume`, `source_near_surface`, `receiver_outside_volume`, `receiver_sphere_crosses_surface` |
| type-5 directivity files parsed | `directivity_file_missing`, `directivity_file_invalid`, `directivity_band_missing` |
| `trans_epsilon` present, `rayon_recepteurp` > 0, `pasdetemps` > 0 | `trans_epsilon_invalid`, `receiver_radius_invalid`, `time_step_invalid` |
| duree/pasdetemps below 65,536 steps | `step_count_overflow`, `source_delay_invalid` |
| names and labels under 50 bytes, file-name-safe, unique | `name_too_long`, `name_not_filename_safe`, `name_duplicate` |
| no zero-length direction vectors | `direction_vector_zero` |
| α = 1 forces diffusion to 0; τ ≤ α corrected with a warning | `material_diffusion_ignored`, `material_transmission_exceeds_absorption` |
| absatmo in explicit units | `absatmo_invalid` |

The laydown traps a project can violate are covered too:
- `band_set_mismatch`: "every source, material and fitting must carry every band".
- the directivity rules: "a missing or unparseable directivity file", which in fact silences the
  source rather than making it omnidirectional.
- `mesh_out_of_date`: "tie each `.mbin` to the `.cbin`".
- `name_*`: "labels become folder names unvalidated".

The other traps are handled elsewhere:
- stable ids: the schema's UUIDs, plus `solver_id_mapping_invalid`
- world units: the schema's f64 metres
- untrusted exit codes and streams: Part B

### Rules that belong to other stages

These are not `core::validate` rules, so they carry no code here:
- **Geometry (M4):** open shells, self-intersections (TetGen exit 3), and points that collapse
  when rounded to `f32`. Every coordinate reaches the solver as a 32-bit float
  (`coreString.h:41`; `docs/formats/cbin.md`).
- **Mesh verification (M5):** degenerate tetrahedra. The solvers call `exit(1)` on one
  (`coreTypes.cpp:210-216`).

## Part B: the run contract

### Launch

- **Programs:** `spps.exe` and `classicalTheory.exe` from the M1 build.
- **Arguments.** Pass exactly one argument, `config.xml`, and never `-v`.
  - SPPS reads `-v` only before the path. It appends every later argument to the path with a
    space (`sppsNantes.cpp:264-283`).
  - TCR joins all its arguments with spaces (`main_tc.cpp:41-48`).
  - With no argument, SPPS prints `The path of the XML configuration file must be specified!` and
    exits 0: `MainProcess` returns 1, and `main` discards it
    (`sppsNantes.cpp:285-289, 449-460`). TCR prints the same and exits 1
    (`main_tc.cpp:49-52, 160-173`). Both are VERIFIED S.
  - `-v` only adds verbose lines, which the classifier would then have to know.
- **Working directory:** the run folder. The argument is relative because pugixml opens it with
  the narrow `fopen` (`pugixml.cpp:6944`). An absolute path outside the ANSI code page fails
  silently with exit 0 (VERIFIED S `run_Ł`). With the cwd set to the run folder and an absolute
  `workingdirectory`, nothing is written relative to the cwd (VERIFIED S: the `cwd_*` folders
  stayed empty). P1 and P2 wrote only inside their run folders.
- **Upstream's GUI**, by contrast, passes one quoted absolute path and never sets the cwd
  (`projet.cpp:806-811`; `processManager.cpp:172`).

### The run folder

- **It must be new and empty.** Re-running into the same folder silently overwrites most files,
  and receiver folders gain suffixes: `R10` (S), and `baseReportManager.cpp:193-203`.
- **It must contain before launch:**
  - `config.xml`, written by us, UTF-8
  - the `.cbin` named by `modelName`
  - the `.mbin` named by `tetrameshFileName`, built from that `.cbin`
  - every type-5 source's directivity file at `workingdirectory + directivities_directory +
    directivity_file`. Upstream's GUI copies these into `loudspeakers\`
    (`e_scene_sources_source.h:117-133`), so the run folder is self-contained
- **The solvers create their output folders** through boost's `create_directories`
  (`std_tools.cpp:42-49`).

### Expected outputs

Paths are relative to the working directory. `<f>` is a computed band's `@freq`, and `<lbl>` a
point receiver's label. The listings of P2 `spps_base` (65 files) and `tcr_base` (87 files)
match these tables: tutorial 1 has 27 bands, 2 point receivers, 1 surface receiver and
`nbparticules_rendu` 0.

**SPPS**

| Path | When | Receipt |
|---|---|---|
| `<stats_filename>` | always | `sppsNantes.cpp:395`; `spps/reportmanager.cpp:482-517` |
| `<cumul_filename>` | always | `sppsNantes.cpp:395`; `spps/reportmanager.cpp:520-538` |
| `<receiversp_directory>\<lbl>\<receiversp_filename>` | per point receiver | `coreinitialisation.cpp:473-488`; `baseReportManager.cpp:190-208` |
| `<receiversp_directory>\<lbl>\<receiversp_filename_adv>` | per point receiver | `sppsNantes.cpp:398`; `spps/reportmanager.cpp:871` |
| `<receiversp_directory>\<lbl>\Punctual receiver intensity.gabe` | per point receiver, fixed name | `sppsNantes.cpp:402`; `spps/reportmanager.cpp:704` |
| `<receiversp_directory>\<lbl>\Sound level per source.recps` | per point receiver, fixed name | `sppsNantes.cpp:406`; `spps/reportmanager.cpp:614` |
| `<receiversp_directory>\<lbl>\<source name>\<receiversp_filename>` | `output_recp_bysource` ≠ 0 | `spps/reportmanager.cpp:620-656` |
| `Intensity animation\<f> Hz\Intensity.rpi` | per computed band, fixed name | `spps/reportmanager.cpp:719-757` |
| `<recepteurss_directory><f> Hz\<recepteurss_filename>` | surface receivers exist and `output_recs_byfreq` ≠ 0 | `sppsNantes.cpp:183-193, 219-224` |
| `<recepteurss_directory><f> Hz\<recepteurss_cut_filename>` | cutting planes exist and `output_recs_byfreq` ≠ 0 | `sppsNantes.cpp:225-226` |
| `<recepteurss_directory>Global\<recepteurss_filename>` | surface receivers exist and the first has faces | `sppsNantes.cpp:408-421`; `baseReportManager.cpp:430-433` |
| `<recepteurss_directory>Global\<recepteurss_cut_filename>` | cutting planes exist | `sppsNantes.cpp:412, 421`; `baseReportManager.cpp:211-214` |
| `<particules_directory><f>\<particules_filename>` | `nbparticules_rendu` > 0. The folder is `<f>`, without ` Hz` | `spps/reportmanager.cpp:85-86, 99-111` |
| `<particules_directory><f>\particle_surface_collision_statistics.csv`, `…\particle_receivers_collision_statistics.csv` | `nbparticules_rendu` > 0 and `save_*_intersection` ≠ 0 (default 1) | `spps/reportmanager.cpp:113-132` |

SPPS creates `<recepteurss_directory>` and its `Global\` folder even when there are no surface
receivers (`sppsNantes.cpp:408-411`). VERIFIED P2 `recs_byfreq0`: `output_recs_byfreq="0"` leaves
only the `Global\` file.

**TCR** (GUI mode, the only mode we use)

| Path | When | Receipt |
|---|---|---|
| `Main results.gabe` | always | `main_tc.cpp:136-137, 145` |
| `Punctual receivers\<lbl>.gabe` | per point receiver. The folder name is fixed; `receiversp_directory` is ignored | `main_tc.cpp:139-144`; `ctr/reportmanager.cpp:148` |
| `<direct\|sabine\|eyring prefix><recepteurss_directory><f> Hz\<recepteurss_filename>` (and `…cut_filename` for cutting planes) | surface receivers or cutting planes exist; every computed band, whatever `output_recs_byfreq` says | `TC_CalculationCore.cpp:344-402` |
| `<direct\|sabine\|eyring prefix><recepteurss_directory>Global\<recepteurss_filename>` (and the cut file) | the same | `TC_CalculationCore.cpp:406-420` |

### Exit codes

**SPPS returns 0 whether the run worked or not.** Its `main` discards `MainProcess`'s result
(`sppsNantes.cpp:449-460`).

| Exit (as u32) | Class | SPPS cases | Receipt |
|---|---|---|---|
| 0 | not evidence of success | Success, and also: no argument; unparseable config (P2 `bad_xml`); unreadable mesh; the on-face stop; a missing separator on `workingdirectory` (P2 `wd_nosep`); 100 % particle loss (S `run_lossy`); a missing directivity file or band (S `run_dirmiss`, P2 `dir_partial`, `dir_badrow`); a short spectrum (S `run_oneband`); zero `trans_epsilon` (P2 `eps_short`); a receiver outside (P2 `rcv_out`); receiver radius 0 (P2 `radius0`); a name overflow (P2 `name_long`); a wrapped delay (P2 `delay_wrap`) | `sppsNantes.cpp:449-460` |
| `0xFFFFFFFF` (-1) | FAIL | A face's material is undeclared | `coreinitialisation.cpp:429-432`; VERIFIED P2 `mat22_miss` |
| 1 | FAIL | A degenerate tetrahedron in the `.mbin` (not run) | `coreTypes.cpp:210-216` |
| `0xC0000005` | CRASH | Access violation, with no message: a source outside the mesh, an undeclared material 0 on the first faces, or a balloon source with no file attribute | VERIFIED P2 `src_out`, `dir_noattr`; S `run_mat0miss` |
| `0xC0000409` | CRASH | Abort from an uncaught C++ exception: a non-numeric directivity value, an empty `workingdirectory`, or a missing time step | VERIFIED P2 `dir_nan`, `wd_empty`, `no_dt` |

TCR returns `MainProcess`'s value (`main_tc.cpp:160-173`):

| Exit (as u32) | Class | TCR cases | Receipt |
|---|---|---|---|
| 0 | success, but check it | Success, and also silent failures: a source outside the room (P2 `tcr_src_out`), a short source spectrum (P2 `tcr_oneband_src`: NaN levels), and duplicate labels (P2 `tcr_label_dup`) | `main_tc.cpp:157` |
| 1 | FAIL | No argument; an unreadable config, `.cbin` or `.mbin` | `main_tc.cpp:49-52, 73-79`; VERIFIED P2 `tcr_bad_xml`, S `tcr_nomesh` |
| `0xFFFFFFFF` (-1) | FAIL | A face's material is undeclared | VERIFIED P2 `tcr_mat22_miss` |
| `0xC0000409` | CRASH | A non-numeric directivity value | VERIFIED P2 `tcr_dir_nan` |

**Rule:** any exit at or above `0xC0000000` is CRASH (`crash_access_violation` for
`0xC0000005`, `crash_abort` for `0xC0000409`, `crash_other` otherwise). Any other non-zero exit
is FAIL, with reason `exit_nonzero`. Exit 0 is necessary for OK, never sufficient.

### Output line classification

- **Both streams are read concurrently** and classified by content, never by stream.
- **Fatal lines arrive on stdout.** SPPS's loss warning arrives on stderr.
- **Three stderr messages end without a newline**, so a trailing partial line is flushed at EOF:
  `sppsNantes.cpp:63, 438`; `coreTypes.cpp:213`.
- **Patterns are anchored at the start of the line.** Those ending in `$` must match the whole
  line. A line that matches none of them is `unclassified_line`. In the raw Markdown, `\|` inside
  a pattern is the table escape for the regex `|`.
- **Checked against real output.** Ten P2-style runs were classified with this table: SPPS
  base, missing `trans_epsilon`, bad XML, missing material, no atmosphere, balloon source
  without a file, and missing directivity file; TCR base, bad XML and missing material. Every
  one of their stdout and stderr lines matched a row or was a continuation line, with 0
  `unclassified_line`. The rows hit were `progress`, `spps_banner`, `tcr_banner`,
  `tcr_loading`, `tcr_config_echo`, `tcr_step`, `spps_output_start`,
  `spps_end_of_calculation`, `xml_property_missing`, `scene_mesh_unreadable`,
  `directivity_not_open`, `material_missing` and `particle_loss_reported`.
- **Continuation lines belong to their event.** The line after `scene_mesh_unreadable` and the
  line before `tetra_mesh_empty` are file paths (`coreinitialisation.cpp:396`;
  `coreTypes.cpp:241`). They are classified with that event, never as `unclassified_line`.

| Pattern id / reason | Stream | Pattern | Class | Receipt |
|---|---|---|---|---|
| `progress` | stdout | `^#[0-9.eE+-]+$` (the percentage to 4 significant digits) | PROGRESS | `progressionInfo.h:154-155` |
| `spps_banner` | stdout | `^SPPS version \d+\.\d+\.\d+$` | INFO | `sppsNantes.cpp:247` |
| `tcr_banner` | stdout | `^Classical Theory of Reverberation version \d+\.\d+\.\d+$` | INFO | `TC_CalculationCore.cpp:48` |
| `tcr_loading` | stdout | `^XML configuration file (is currently loading\.\.\.\|has been loaded\.)$` | INFO | `main_tc.cpp:57, 61` |
| `tcr_config_echo` | stdout | the argument itself, `^config\.xml$` | INFO | `main_tc.cpp:58` |
| `tcr_step` | stdout | `^Step [123]/3 : .+$` | INFO | `TC_CalculationCore.cpp:54-80` |
| `ground_height` | stdout | `^(Compute of z ground inside each tetra\|Calculation of z ground inside each tetra has been done)\.$` | INFO | `coreinitialisation.cpp:139, 148` |
| `spps_output_start` | stdout | `^Output results files\.$` | INFO | `coreinitialisation.cpp:477` |
| `spps_end_of_calculation` | stdout | `^End of calculation\.$` | OK: required for an SPPS OK | `sppsNantes.cpp:389` |
| `xml_property_missing` | stdout | `^Xml Property (.+) doesn't exist !$` | FAIL | `cxml.cpp:115` |
| `scene_mesh_unreadable` | stdout | `^Unable to read the scene mesh file :$`. The next line is the path | FAIL | `coreinitialisation.cpp:396` |
| `tetra_mesh_unreadable` | stdout | `^Unable to read the tetrahedalization of the scene mesh file, calculation canceled\.$` | FAIL | `coreinitialisation.cpp:456` |
| `tetra_mesh_empty` | stdout | `^Tetrahedron file is empty, the calculation can't be done !$`. The previous line is the file name | FAIL | `coreTypes.cpp:241` |
| `config_path_missing` | stdout | `^The path of the XML configuration file must be specified!$` | FAIL | `sppsNantes.cpp:287`; `main_tc.cpp:50` |
| `directivity_not_open` | stdout | `^DirectivityBalloon : File not open$` | FAIL | `directivityParser.cpp:46` |
| `source_moved_off_vertex` | stdout | `^Source at tetrahedron vertex, move source position from \[.*\] to \[.*\]$` | WARN | `sppsInitialisation.cpp:27-29` |
| `material_missing` | stderr | `^Wrong project configuration, a face is defined but no materials are attached to it !` | FAIL | `coreinitialisation.cpp:430` |
| `degenerate_tetrahedron` | stderr | `^Error in input mesh, a tetrahedra have at least the same two vertices` (no newline) | FAIL | `coreTypes.cpp:213` |
| `source_on_surface` | stderr | `^A sound source position is intersecting with the 3D model` | FAIL | `sppsNantes.cpp:323` |
| `source_not_located` | stderr | `^Unable to find the source position!` (no newline; unreachable in practice) | FAIL | `sppsNantes.cpp:63` |
| `particle_loss_reported` | stderr | `^Warning (\d+) particles has been in error on (\d+) particles\.` (no newline) | FAIL | `sppsNantes.cpp:425-439` |
| `unclassified_line` | either | anything else | WARN | – |

Notes on the classifier:
- **No user string is ever a `printf` format.** Labels and names never reach `printf` in
  `spps/` or `ctr/` (grep). The only user text on the streams is attribute names in
  `xml_property_missing`, the continuation paths, and TCR's echo of the argument.
- **The loss warning fires only when (lost + infinite-loop) / total exceeds 5 %**
  (`sppsNantes.cpp:33, 437`). It does **not** count particles killed by the atmosphere, by
  materials or by `trans_epsilon` (P2 `eps_short`, S `run_noeps`). The statistics check below
  is therefore needed as well as the line.
- **Upstream's GUI protocol** uses `#` for progress and `!` for warnings
  (`processManager.cpp:61-71`). Neither solver prints `!`.

### Judging a run

A run is **OK** only when all four signals agree (plan, `core::run`):

1. **Exit code.** TCR: 0. SPPS: 0 and a `spps_end_of_calculation` line. Anything else takes the
   class from "Exit codes" above.
2. **Lines.** No FAIL-class line, and every WARN is recorded.
3. **Statistics** (SPPS only). The GABE at `<stats_filename>` is read with
   `docs/formats/gabe.md`'s reader.
   - Column 0 holds the seven labels in order: absorbed by the atmosphere, absorbed by the
     materials, absorbed by the fittings, lost by infinite loops, lost by meshing problems,
     remaining, total (`spps/reportmanager.cpp:489-496`).
   - There is one integer column per computed band, labelled `<f> Hz` (`spps/reportmanager.cpp:349, 498-515`).
   - The set of band columns must equal the project's bands. This catches `docalc` and duplicate
     errors (P2 `docalc_true`, `dup_freq`). Reason: `stats_band_mismatch`.
   - In every band, total ≥ `nbparticules` × the number of sources, or the reason is
     `particle_total_short`. Equality is expected, except that energetic mode also counts the
     copies it makes of transmitted particles: every `Run` call counts once
     (`CalculationCore.cpp:109, 262-285`; `sppsNantes.cpp:147-154`) (inferred). VERIFIED
     equality on tutorial 1: 10,000 per band in P1 and P2. Totals are 0 when a source emits
     nothing (P2 `dir_partial`, `dir_badrow`).
   - In every band, (lost by meshing + lost by loops) / total must not exceed the tolerance of
     open decision 7. Reason: `particle_loss_excess`.
4. **Files.** Every expected file above exists and is non-empty. Reason:
   `expected_file_missing`.
   - A `.csbin` is compared only after decoding.
   - TCR additionally: no NaN or ±inf in any value we display, or the reason is
     `nonfinite_result`. TCR's Global row is NaN by design, and P2 `tcr_oneband_src` shows NaN
     from a band error.

A stats file that cannot be read is `stats_unreadable`. A cancelled run is `cancelled`, and is
never OK.

### Threads, cancelling and partial output

- **SPPS runs one thread per computed band,** unless `random_seed` ≠ 0 makes it single-threaded
  (`spps/core_configuration.cpp:28, 44-47`; `sppsNantes.cpp:369-387`). With a seed, two runs of
  the same config give identical files apart from `.csbin` (M1 gate).
- **SPPS writes per-band surface-receiver files as each band finishes, and everything else after
  all bands** (`sppsNantes.cpp:216-226, 389-423`). A killed run leaves partial files and counts
  as failed (inferred). Its folder is never reused.
- **TCR is single-threaded** and takes about 0.1 s on tutorial 1 (P2 `tcr_base`).

### TetGen

Meshing is M5's stage. The TetGen part of the survey's run contract is kept here for one place
of reference; its file formats are in `docs/formats/tetgen.md`.

- **Command:** `tetgen.exe <switches> <name>.poly`.
  - Upstream's GUI uses `-pq<minratio, default 5> [-a<maxvol>] -A -n <appendparams, default
    -Y>`, or `-d` for its self-intersection test (`projet_maillage.cpp:163-178`;
    `e_core_core_tetconf.h:86-103`).
  - The outputs are `<name>.1.{node,ele,face,neigh,edge}`, beside the input (VERIFIED S
    `tg_ok`).
- **Exit codes** (`tetgen.h:2487-2524`). Messages go to stdout through `printf`.

  | Exit | Meaning |
  |---|---|
  | 0 | success |
  | 1 | out of memory |
  | 2 | internal error |
  | 3 | self-intersection |
  | 4 | a very small feature (use `-T`) |
  | 5 | two very close facets (use `-Y`) |
  | 10 | input error |
  | 200 | Steiner points on the boundary under `-YY` |

- **Failure signature** (VERIFIED S `tg_bad`): exit 3; partial `.1.node`, `.1.ele`, `.1.face`
  and `.1.edge`; no `.1.neigh`; and `<name>_skipped.face` and `.node`, whose markers are `.cbin`
  face indices. The line `Program stopped.` arrives on stdout, and stderr is empty.

### Corrections to the survey's run contract

- **Not every non-success SPPS exit is 0 or `0xC0000005`.** There is also `0xC0000409`, an
  abort, from a non-numeric directivity value, an empty `workingdirectory` or a missing time step
  (P2). TCR aborts the same way on the directivity value.
- **"Source on a face gives exit 0 after a stderr message"** is only the code path. A source on
  the floor plane did not trigger it (P2 `src_face`), so `source_near_surface` is a pre-launch
  rule rather than something left to the solver.
- **TCR also exits 0** for a type-5 source without a file (P2 `tcr_dir_noattr`) and for a short
  source spectrum (P2 `tcr_oneband_src`).
- **"total == nbparticules × bands"** holds only per band, per source, and outside energetic mode
  with transmission copies (inferred).
- **Counting `Xml Property` lines as FAIL** requires our writer to emit `directivities_directory`
  for TCR, since upstream's GUI does not (P1).

## Not examined

- A run killed mid-solve. Cancel behaviour is inferred from the code.
- Long-path behaviour beyond 260 characters.
- SPPS's `-v` output.
- The `.pbin` header when `nbparticules_rendu` exceeds `nbparticules`.
