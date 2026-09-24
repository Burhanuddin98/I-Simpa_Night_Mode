# `config.xml`: the solver configuration, as the solvers read it

Each solver takes one argument, the path of a `config.xml`. SPPS (`spps.exe`) and TCR
(`classicalTheory.exe`) both load it through `lib_interface`'s
`Base_Core_Configuration::LoadCfgFile`. Each then reads a few attributes of its own. This page
describes **what the solvers read and what they do with it**. It does not describe what
upstream's GUI writes, although "Ignored by the solvers" lists what the GUI writes that no solver
reads, and "Parity with upstream's GUI" compares what the GUI wrote for its own tutorial runs
with what our writer writes. The Rust writer is `crates/simpa-core/src/config_xml.rs` (M3). The
pre-launch rules that this page motivates, and the launch contract, are in
`docs/solver-contract.md`.

## Receipts

**Source.** Receipts are paths under `target/solvers/src-929a5c8/src/`. Short names:

| Short name | Path |
|---|---|
| `cxml.cpp` | `lib_interface/input_output/cxml.cpp` |
| `coreString.cpp`, `coreString.h` | `lib_interface/coreString.{cpp,h}` |
| `base_core_configuration.cpp` | `lib_interface/data_manager/base_core_configuration.cpp` |
| `coreinitialisation.cpp` | `lib_interface/coreinitialisation.cpp` |
| `coreTypes.h`, `coreTypes.cpp` | `lib_interface/coreTypes.{h,cpp}` |
| `baseReportManager.cpp` | `lib_interface/input_output/baseReportManager.cpp` |
| `directivityParser.cpp`, `directivityBalloon.cpp` | `lib_interface/input_output/directivity/` |
| `std_tools.cpp` | `lib_interface/std_tools.cpp` |
| `mathlib.h` | `lib_interface/Core/mathlib.h` |
| `gabe.cpp`, `gabe.h` | `lib_interface/input_output/gabe/` |
| `bin.cpp` | `lib_interface/input_output/bin.cpp` (the `.cbin` codec) |
| `mbin.cpp` | `lib_interface/input_output/importExportMaillage/mbin.cpp` |
| `rsbin.h` | `lib_interface/input_output/exportRecepteurSurf/rsbin.h` |
| `pugixml.cpp` | `lib_interface/input_output/pugixml/src/pugixml.cpp` (pugixml 1.8) |
| `Celerite_du_son.cpp`, `Masse_volumique_air.cpp`, `Coef_Att_Atmos.cpp` | `lib_interface/data_manager/data_calculation/` |
| `spps/core_configuration.cpp` | `spps/data_manager/core_configuration.cpp` |
| `sppsNantes.cpp`, `sppsInitialisation.cpp`, `CalculationCore.cpp`, `sppsTypes.h` | `spps/` |
| `spps/reportmanager.cpp` | `spps/input_output/reportmanager.cpp` |
| `dotreflection.h` | `spps/tools/dotreflection.h` |
| `ctr/core_configuration.cpp` | `ctr/data_manager/core_configuration.cpp` |
| `main_tc.cpp`, `TC_CalculationCore.cpp` | `ctr/` |
| `ctr/reportmanager.cpp` | `ctr/input_output/reportmanager.cpp` |
| GUI files, e.g. `e_core_sppscore.h` | `isimpa/data_manager/` and its `tree_core/`, `tree_scene/` subfolders |

**Runs.**
- **P1** and **P2 `<case>`** were run on 2026-09-23 against our solver build in
  `target/solvers/bin` (M1; its output is byte-identical to the reference build). The cwd was a
  fresh run folder and the argument was `config.xml`. Each case is one edit to tutorial 1's
  GUI-written configs (`tests/fixtures/upstream/tutorial1/{spps,tcr}/config.xml`: 27 bands,
  10,000 particles, seed 1). The scripts `p1_ignored.py` and `p2_behaviour.py` live in the
  session's scratch folder, not in the repo. M6's negative run fixtures are meant to make these
  cases permanent.
- **S `<case>`** is a run by the contract survey, recorded in
  `docs/rebuild-plan-raw-2026-09-23.json` under `surveys[key=contract]`.
- **inferred** means read from the code and not run.

## How the solvers parse the file

1. **Loading.** `CXml` calls pugixml's `load_file(const char*)`, which opens the path with the
   narrow `fopen` (`cxml.cpp:184-194`, `pugixml.cpp:6939-6947`). pugixml's default options apply.
   The XML declaration, comments, processing instructions and DOCTYPE produce no nodes, and
   whitespace-only text is dropped. The encoding comes from a BOM, then from
   `encoding="iso-8859-1"` or `"latin1"` in the declaration; anything else is read as UTF-8
   (`pugixml.cpp:1978-1992`). Strings reach the solver as UTF-8.
2. **Unparseable file.** The root stays NULL, nothing is read, and every value stays 0 or empty.
   SPPS prints `Unable to read the scene mesh file :` and an empty path line, then exits 0. TCR
   prints the same and exits 1. No `Xml Property` line appears. VERIFIED P2 `bad_xml`: a
   config with its closing tag removed.
3. **Root.** The root is `doc.first_child()`. Its element name is never checked
   (`cxml.cpp:192`, `base_core_configuration.cpp:52-53`).
4. **Sections.** `GetChild(name)` returns the first child element with that name. Later
   duplicates are ignored (`cxml.cpp:171-179`).
5. **Attributes.** `GetProperty(name)` returns the first attribute with that name
   (`cxml.cpp:108-118, 158-164`). **A missing attribute prints `Xml Property <name> doesn't exist
   !` on stdout and reads as `""`, so it becomes 0.** `IsPropertyExist` tests for presence without
   a message (`cxml.cpp:98-107`). Only the eight attributes marked *optional* below are read that
   way.
6. **Integers** are read with `atoi` (`coreString.cpp:84-87`): only a leading integer counts.
   `"true"` reads 0, `"1.9"` reads 1, `"1e3"` reads 1, and `""` reads 0.
7. **Reals** are read with `atof` after the first `,` is replaced by `.`
   (`coreString.cpp:89-105`), and stored as 32-bit `float` (`decimal`, `coreString.h:41`). The
   solvers never call `setlocale` (grep of `spps/`, `ctr/` and `lib_interface/`), so the decimal
   point is the C locale's `.`. `"0,5"` reads 0.5: VERIFIED P2 `comma_decimal`, where the outputs
   were identical to the `.` run. `"1,000.5"` reads 1.0.
8. **Lists.** Each child node of `<sources>`, `<surface_absorption_enum>`, `<recepteursp>` and
   `<encombrement_enum>` is one item, whatever its element name
   (`base_core_configuration.cpp:127, 192, 237, 310`). Only `<recepteurss>` checks child names
   (`base_core_configuration.cpp:272, 278`). Non-whitespace text inside a list would become a nameless child node, and so an
   extra item whose every value is 0 (inferred). The writer emits no text content.
9. **Spectra.** Every child node of a spectrum-carrying element is a band entry, whatever its
   name. See "Bands" below.

## Bands: mapping by position

- **`freq_enum`.** The children are sorted ascending by `atoi(@freq)` with `std::sort`
  (`cxml.cpp:130-134, 149-156`). A band's index is its sorted position
  (`base_core_configuration.cpp:101-110`).
- **Spectra.** A source's, a material's, a point receiver's and a fitting's children are sorted
  by their own `@freq`, then assigned to band indices 0, 1, 2, ... in that order
  (`base_core_configuration.cpp:140-152, 201-224, 250-259, 314-326`). **A spectrum's `@freq` is
  never compared with `freq_enum`.** VERIFIED P2 `freq_sortkey`: every spectrum `@freq` in
  tutorial 1 was replaced by 700000 + its rank, and all 65 output files were identical to the
  unedited run.
- **`docalc`.** A band is computed only when `@docalc` is the exact string `"1"`
  (`base_core_configuration.cpp:106`). VERIFIED P2 `docalc_true`: `docalc="true"` on 1000 Hz
  removed that band's statistics column and every `1000 Hz` folder, with exit 0 and no message.
- **Bands that are not computed still take an index.** Every spectrum must also carry the
  `docalc="0"` bands.
- **Where the value is used.** A band's `@freq` names its output folders (`<f> Hz`, and plain
  `<f>` for particle files), and it is the key for directivity lookup (`sppsNantes.cpp:84, 206`;
  `spps/reportmanager.cpp:101`).
- **Duplicates** are computed twice and their folders collide. VERIFIED P2 `dup_freq`: the
  statistics had two `1000 Hz` columns, and 63 files were written instead of 65, exit 0.
- **TCR** computes Main results and point receivers for every band. `docalc` only controls its
  surface-receiver output (`TC_CalculationCore.cpp:219, 261, 344`). VERIFIED P2 `tcr_docalc0`:
  the 1000 Hz row of `Main results.gabe` and of the receiver files is filled, and no `1000 Hz`
  folder is written.

When a spectrum's entry count differs from the band count:

| Spectrum | Array sized by | Fewer entries than bands | More entries |
|---|---|---|---|
| source | its entry count (`base_core_configuration.cpp:141`) | out-of-bounds read at use (`sppsNantes.cpp:73`, `TC_CalculationCore.cpp:148, 169`). VERIFIED S `run_oneband`: the 1000 Hz level was applied to 500 Hz and the 1000 Hz output was 0, exit 0. VERIFIED P2 `tcr_oneband_src`: `L_Sabine` read 49.9 dB at 50 Hz, then NaN, -inf and -270 dB, exit 0 | ignored (`base_core_configuration.cpp:145`) |
| material | its entry count (`base_core_configuration.cpp:202`) | out-of-bounds read at use (`CalculationCore.cpp:231`, `TC_CalculationCore.cpp:98, 129`) (inferred) | ignored (`base_core_configuration.cpp:206`) |
| point receiver background noise | the band count (`coreTypes.cpp:36-44`) | missing bands read 0 dB (memset, `coreTypes.cpp:43`) | ignored (`base_core_configuration.cpp:254`) |
| fitting | the band count (`coreTypes.h:337`) | missing bands left uninitialised, since there is no memset (inferred) | ignored (`base_core_configuration.cpp:319`) |

## `workingdirectory` and paths

- **Concatenation.** Every file is `workingdirectory + name`, joined with no separator added
  (`sppsNantes.cpp:300-302, 316, 330-331`; `main_tc.cpp:66-68, 78`;
  `base_core_configuration.cpp:162-164`).
- **It must be absolute and end with a separator.** VERIFIED P2 `wd_nosep`: SPPS looked for
  `<run folder>mesh.cbin`, printed `Unable to read the scene mesh file :`, wrote nothing, and
  exited 0.
- **It must not be empty.** SPPS runs the whole solve and then aborts with `0xC0000409` in
  `st_mkdir("")` (`coreinitialisation.cpp:485` → `std_tools.cpp:42-49`, where boost's
  `create_directories` rejects an empty path). VERIFIED P2 `wd_empty`: only the 27 per-band
  surface-receiver files were written. TCR runs relative to the cwd (VERIFIED P2
  `tcr_wd_empty`, exit 0, 87 files).
- **Non-ASCII paths work.** Every data file is opened through `pugi::as_wide`, which converts
  UTF-8 to UTF-16 (`bin.cpp:123, 159`; `mbin.cpp:98, 167`; `gabe.cpp:363, 463`;
  `directivityParser.cpp:41`; `spps/reportmanager.cpp:107, 118, 127`; `std_tools.cpp:46`).
  VERIFIED S `run_é`, `run_Ł`. The *argument* is opened with the narrow API; see
  `docs/solver-contract.md`.
- **Which folder attributes need a trailing separator.**

  | Attribute | Trailing separator | Receipt |
  |---|---|---|
  | `simulation@recepteurss_directory` | yes | `sppsNantes.cpp:186, 409`; `TC_CalculationCore.cpp:302, 321-323` |
  | `simulation@particules_directory` | yes | `spps/reportmanager.cpp:101` |
  | `simulation@directivities_directory` | yes, or empty | `base_core_configuration.cpp:162-164` |
  | `simulation@direct_recepteurSOutputName`, `@sabine_…`, `@eyring_…` | yes | `TC_CalculationCore.cpp:295-323` |
  | `simulation@receiversp_directory` | **no**: the code appends one | `coreinitialisation.cpp:484`; `spps/reportmanager.cpp:632` |
  | `simulation@output_folder` | **no**: `/` is appended | `main_tc.cpp:149` |

## Element and attribute reference

**Key.** Every attribute a solver reads appears in the first column in backticks as
`<element>@<attribute>` (a band entry is `<parent>/bfreq@<attribute>`). A tool can collect them
with the pattern `` `[a-z_/]+@[A-Za-z0-9_]+` `` in the first column of this section's tables.
`freq_enum` is written without its `simulation/` prefix. The element names are the ones
upstream's GUI writes. Where "any child" is accepted, the name is not checked.

**Read by** says which solver *parses* the attribute, and so prints `Xml Property` when it is
missing. Every attribute read by `LoadCfgFile` is parsed by both solvers, even when only one uses
it.

**Writer** is the obligation on our writer:
- `always`: write it in both SPPS and TCR configs.
- `SPPS` or `TCR`: write it in that solver's config only.
- `if …`: write it only under that condition.
- `never`: do not write it.

The M3 gate uses this column to check attribute coverage and "0 `Xml Property` lines".

### `configuration` (the root)

| Attribute | Type, unit | Read by | Meaning; missing or bad value | Writer | Receipt |
|---|---|---|---|---|---|
| `configuration@workingdirectory` | string, absolute path | both | Prefix of every file name. Missing: `""` and a message; see "`workingdirectory` and paths" for what empty does | always: the run folder, with a trailing `\` | `base_core_configuration.cpp:55` |

### `condition_atmospherique`

**If the element is missing**, temperature, pressure, humidity, sound speed and density all stay
0 (memset, `base_core_configuration.cpp:39-42, 57-72`). VERIFIED P2 `no_atmo`: every particle in
every band was "lost by meshing problems", with the stderr loss warning, exit 0.

| Attribute | Type, unit | Read by | Meaning; missing or bad value | Writer | Receipt |
|---|---|---|---|---|---|
| `condition_atmospherique@temperature` | real, °C | both | Sound speed c = 343.2·√(T/293.15) with T in kelvin (`Celerite_du_son.cpp:46`). Density ρ = P·M/(R·T) (`Masse_volumique_air.cpp:51`). Missing: 0 °C, silently plausible | always | `base_core_configuration.cpp:62-66` |
| `condition_atmospherique@pression` | real, Pa | both | Static pressure. Missing: 0, so ρ = 0 and the ISO 9613-1 term (P_ref/P) divides by zero (`Coef_Att_Atmos.cpp:59`) (inferred) | always | `base_core_configuration.cpp:61, 66` |
| `condition_atmospherique@humidite` | real, % relative humidity | both | ISO 9613-1 input (`Coef_Att_Atmos.cpp:53-56`). Missing: 0 % | always | `base_core_configuration.cpp:60, 114` |
| `condition_atmospherique@z0` | real, m | both, used by SPPS | Ground roughness in c(z) = c0 + alog·ln(1 + z/z0) + blin·z (`spps/core_configuration.cpp:73-87`). Used only when alog or blin is non-zero (`coreinitialisation.cpp:133-150`). z0 = 0 with a gradient divides by zero (inferred) | always | `base_core_configuration.cpp:67` |
| `condition_atmospherique@alog` | real, m/s | both, used by SPPS | Logarithmic sound-speed gradient coefficient. 0 means a homogeneous medium | always | `base_core_configuration.cpp:68` |
| `condition_atmospherique@blin` | real, 1/s | both, used by SPPS | Linear sound-speed gradient coefficient. 0 means a homogeneous medium | always | `base_core_configuration.cpp:69` |
| `condition_atmospherique@disable_absatmo_computation` | int | both | `==1` uses `@absatmo` for every band instead of ISO 9613-1. Missing: a message, then ISO 9613-1. It is never rejected | always | `base_core_configuration.cpp:70, 111-114` |
| `condition_atmospherique@absatmo` | real, **1/m energy attenuation** | both | Used verbatim as m for every band when the flag is 1. The solver's own ISO path converts dB/m by ln10/10 (`base_core_configuration.cpp:114`), so a dB/m value entered here is 4.34 times too strong. VERIFIED P2 `tcr_absatmo`: absatmo = 0.01 gave TR = 0.163·V/(4·0.01·V + A) = 0.582143 s exactly, against 0.654066 s if it had been taken as dB/m. Missing: 0 | always, converted to 1/m from the project's explicit unit | `base_core_configuration.cpp:71, 111-112` |

### `simulation`: files and time

**If the element is missing**, there are no bands and every file name is empty. SPPS then fails
to read `<wd>` as a mesh and exits 0; TCR exits 1 (inferred from
`base_core_configuration.cpp:75-77` and "Loading" above).

| Attribute | Type, unit | Read by | Meaning; missing or bad value | Writer | Receipt |
|---|---|---|---|---|---|
| `simulation@modelName` | string, file name | both | The `.cbin`, relative to wd. Unreadable or no vertices: `Unable to read the scene mesh file :` and the path. SPPS then exits 0, TCR exits 1 | always | `base_core_configuration.cpp:90`; `coreinitialisation.cpp:387-400`; `sppsNantes.cpp:311-312`; `main_tc.cpp:73-74` |
| `simulation@tetrameshFileName` | string, file name | both | The `.mbin`. Unreadable: `Unable to read the tetrahedalization of the scene mesh file, calculation canceled.` SPPS exits 0, TCR exits 1 (VERIFIED S `tcr_nomesh`) | always | `base_core_configuration.cpp:91`; `coreinitialisation.cpp:450-458`; `sppsNantes.cpp:316-317`; `main_tc.cpp:78-79` |
| `simulation@pasdetemps` | real, s | both | Time step. Step count = (int)ceil(duree/pasdetemps). Missing in SPPS: a message, then `0xC0000409` after 13 s (VERIFIED P2 `no_dt`). TCR adds `pasdetemps="1.00"` itself, but an attribute we write comes first and wins: TCR then sizes its surface-receiver arrays by it (VERIFIED P2 `tcr_dt`: same files and sizes, 0.7 s instead of 0.1 s) | SPPS | `base_core_configuration.cpp:92-94`; `ctr/core_configuration.cpp:15-16` |
| `simulation@duree_simulation` | real, s | both | Simulated duration. Missing: 0 steps. TCR adds `"1.00"` as above | SPPS | `base_core_configuration.cpp:93-94`; `ctr/core_configuration.cpp:16` |
| `simulation@directivities_directory` | string, folder prefix | both | The directivity file path is wd + this + `source@directivity_file`. Missing: a message and `""`. **Upstream's GUI writes it only for SPPS** (`e_core_sppscore.h:213-217`), so its own TCR config prints `Xml Property directivities_directory doesn't exist !` (VERIFIED P1) | always (SPPS `loudspeakers\`, as upstream's GUI writes it; TCR `""` when no source uses a directivity file) | `base_core_configuration.cpp:96, 162-164` |
| `simulation@recepteurss_directory` | string, folder with a trailing separator | both | The folder for surface-receiver and cutting-plane files: `<f> Hz\` and `Global\` are appended. SPPS creates it and `Global\` even with no surface receivers (`sppsNantes.cpp:408-411`) | always | `base_core_configuration.cpp:78`; `sppsNantes.cpp:186-192, 408-413`; `TC_CalculationCore.cpp:302, 312, 321-323` |
| `simulation@recepteurss_filename` | string, file name | both | The surface-receiver RSBIN file (`.csbin`), per band and in `Global\` | always | `base_core_configuration.cpp:79` |
| `simulation@recepteurss_cut_filename` | string, file name | both, *optional* | The cutting-plane RSBIN file. Missing: `rs_cut.csbin`, with no message. Like the other *optional* attributes, it has a real default | always | `base_core_configuration.cpp:81-84` |
| `simulation@receiversp_directory` | string, folder name without a separator | both, used by SPPS | The folder for point-receiver results; the code appends the separator. TCR parses it but writes to a fixed `Punctual receivers/` (`main_tc.cpp:141`) | always | `base_core_configuration.cpp:86`; `coreinitialisation.cpp:484-487` |
| `simulation@receiversp_filename` | string, file name | both, used by SPPS | The per-receiver `.recp` file (GABE) | always | `base_core_configuration.cpp:87`; `baseReportManager.cpp:190-208` |
| `simulation@receiversp_filename_adv` | string, file name | both, used by SPPS | The per-receiver advanced-parameter file (`.gap`, GABE) | always | `base_core_configuration.cpp:88`; `sppsNantes.cpp:398`; `spps/reportmanager.cpp:871` |
| `simulation@cumul_filename` | string, file name | both, used by SPPS | Total energy over time, per band (GABE), at the wd root | always | `base_core_configuration.cpp:89`; `sppsNantes.cpp:395`; `spps/reportmanager.cpp:520-538` |

### `simulation/freq_enum`

| Attribute | Type, unit | Read by | Meaning; missing or bad value | Writer | Receipt |
|---|---|---|---|---|---|
| `freq_enum/bfreq@freq` | int, Hz | both | Band centre frequency, sort key and band index. See "Bands". No `freq_enum`: zero bands and nothing computed (inferred) | always: one entry per project band | `base_core_configuration.cpp:98-110`; `cxml.cpp:130-156` |
| `freq_enum/bfreq@docalc` | string | both | Only `"1"` computes the band. See "Bands" | always: `"1"` | `base_core_configuration.cpp:106-108` |

### `simulation`: SPPS settings

These are parsed only by SPPS (`spps/core_configuration.cpp:15-69`).

| Attribute | Type, unit | Read by | Meaning; missing or bad value | Writer | Receipt |
|---|---|---|---|---|---|
| `simulation@particules_directory` | string, folder with a trailing separator | SPPS | Particle files go to wd + this + `<f>` + separator + `@particules_filename`. The band folder has no ` Hz` | SPPS | `spps/core_configuration.cpp:19`; `spps/reportmanager.cpp:99-104` |
| `simulation@particules_filename` | string, file name | SPPS | The `.pbin` particle file per band | SPPS | `spps/core_configuration.cpp:18` |
| `simulation@stats_filename` | string, file name | SPPS | The particle-fate statistics GABE at the wd root. It is the only machine-readable loss figure | SPPS | `spps/core_configuration.cpp:20`; `sppsNantes.cpp:395`; `spps/reportmanager.cpp:482-517` |
| `simulation@nbparticules` | int | SPPS | Particles per source per band. Values below 1 become 1 with no message; a missing attribute prints the usual message and also becomes 1 | SPPS | `spps/core_configuration.cpp:21-23, 30` |
| `simulation@nbparticules_rendu` | int | SPPS | Particles per source saved to `.pbin`. Negative becomes 0. 0 means no particle folder and no collision CSVs. Above `nbparticules`, the save ratio fails its [0, 1] test and is set to 0, so no particle is saved (inferred) | SPPS | `spps/core_configuration.cpp:24-26, 31`; `sppsNantes.cpp:67-72, 332`; `spps/reportmanager.cpp:85-86` |
| `simulation@abs_atmo_calc` | int | SPPS; TCR in GUI mode | Non-zero enables air absorption. Missing: 0. TCR's external mode forces 1 | always | `spps/core_configuration.cpp:34`; `ctr/core_configuration.cpp:26, 30`; `CalculationCore.cpp:52` |
| `simulation@output_recp_bysource` | int | SPPS, *optional* | Non-zero also writes `<receiversp_directory>\<lbl>\<source name>\<receiversp_filename>`, so source names become folder names. Missing: 0 | SPPS | `spps/core_configuration.cpp:35-39`; `spps/reportmanager.cpp:620-656` |
| `simulation@random_seed` | int | SPPS, *optional* | Non-zero seeds the generator (`(uint32_t)seed`) **and** makes the run single-threaded. Otherwise there is one thread per computed band. Missing: 0 | SPPS | `spps/core_configuration.cpp:40-48, 60`; `sppsNantes.cpp:304-307, 369-387` |
| `simulation@save_surface_intersection` | int | SPPS, *optional* | Non-zero writes `particle_surface_collision_statistics.csv`, only when particles are saved. Missing: 1 | SPPS | `spps/core_configuration.cpp:49-53`; `spps/reportmanager.cpp:113-123` |
| `simulation@save_receivers_intersection` | int | SPPS, *optional* | Non-zero writes `particle_receivers_collision_statistics.csv`, only when particles are saved. Missing: 1 | SPPS | `spps/core_configuration.cpp:54-58`; `spps/reportmanager.cpp:124-132` |
| `simulation@direct_calc` | int | SPPS | Non-zero absorbs a particle at its first surface hit, leaving the direct field only. Missing: 0 | SPPS | `spps/core_configuration.cpp:61`; `CalculationCore.cpp:236-242` |
| `simulation@enc_calc` | int | SPPS | 0 silently ignores every fitting zone. Missing: 0 | SPPS | `spps/core_configuration.cpp:62`; `CalculationCore.cpp:34, 132, 384` |
| `simulation@computation_method` | int | SPPS | 0 is random: absorption is a Monte-Carlo draw and a particle's energy is constant. Non-zero is energetic: energy is multiplied by (1 - α). Missing: 0 | SPPS | `spps/core_configuration.cpp:63`; `CalculationCore.cpp:55, 138, 244` |
| `simulation@rayon_recepteurp` | real, m | SPPS | Point-receiver sphere radius. Energy is normalised by c·ρ/(4/3·π·r³). 0 gives NaN in every `.recp` value (VERIFIED P2 `radius0`, exit 0) | SPPS | `spps/core_configuration.cpp:64`; `sppsInitialisation.cpp:75-93` |
| `simulation@trans_epsilon` | real, exponent | SPPS | A particle dies once its energy is at or below E0·10^-eps (`sppsNantes.cpp:75`). **Missing or 0 kills the run silently**, with a message on stdout and exit 0. In random mode, upstream's default, every particle dies at its first surface hit (`CalculationCore.cpp:305`; VERIFIED P2 `eps_short`: total energy is 0 from the second 10 ms step on). In energetic mode with air absorption, every particle dies at the first step (VERIFIED S `run_noeps`) | SPPS | `spps/core_configuration.cpp:65`; `CalculationCore.cpp:55-60, 305-310` |
| `simulation@trans_calc` | int | SPPS | Non-zero enables transmission through materials that carry `@affaiblissement`. Missing: 0 | SPPS | `spps/core_configuration.cpp:66`; `CalculationCore.cpp:251, 264, 292` |
| `simulation@output_recs_byfreq` | int | SPPS; TCR in GUI mode | SPPS: 0 writes only the `Global\` surface-receiver file (VERIFIED P2 `recs_byfreq0`). **TCR tests the pointer, not the value**, so it always writes per band (VERIFIED P2 `tcr_recs_byfreq0`: identical output) | always | `spps/core_configuration.cpp:67`; `sppsNantes.cpp:183, 219, 225`; `ctr/core_configuration.cpp:25`; `TC_CalculationCore.cpp:354, 388, 398` |
| `simulation@surf_receiv_method` | int | SPPS | 0 adds a crossing particle's energy as it is. The GUI calls this "Soundmap: intensity" (`e_core_sppscore.h:60-64`). Non-zero divides it by cos(incidence) (`spps/reportmanager.cpp:187-190, 281-284`). Only exactly 1 then also multiplies by I0/p0²·ρc: "Soundmap: SPL" (`spps/reportmanager.cpp:540-578`). Other values divide without scaling (inferred). Missing: 0 | SPPS | `spps/core_configuration.cpp:68` |

### `simulation`: TCR settings

These are parsed only by TCR (`ctr/core_configuration.cpp:11-34`). **The presence of
`@direct_recepteurSOutputName` selects GUI mode**, which is the mode the writer uses. Without it,
TCR runs in "external core" mode: it forces air absorption on and writes fused files to
wd + `@output_folder` + `/` instead of `Main results.gabe` (`main_tc.cpp:106-156`).

| Attribute | Type, unit | Read by | Meaning; missing or bad value | Writer | Receipt |
|---|---|---|---|---|---|
| `simulation@direct_recepteurSOutputName` | string, folder with a trailing separator | TCR, *optional* | The folder prefix for direct-field surface receivers. Its presence selects GUI mode | TCR | `ctr/core_configuration.cpp:18-22`; `TC_CalculationCore.cpp:308-321` |
| `simulation@sabine_recepteurSOutputName` | string, folder with a trailing separator | TCR, GUI mode | The Sabine total-field prefix. Missing: `""`, so the fields share one folder (inferred) | TCR | `ctr/core_configuration.cpp:23`; `TC_CalculationCore.cpp:295-300, 322` |
| `simulation@eyring_recepteurSOutputName` | string, folder with a trailing separator | TCR, GUI mode | The Eyring total-field prefix. Missing: as above | TCR | `ctr/core_configuration.cpp:24`; `TC_CalculationCore.cpp:295-300, 323` |
| `simulation@output_folder` | string, folder name | TCR, external mode only | The output folder for fused files | never | `ctr/core_configuration.cpp:32`; `main_tc.cpp:148-155` |
| `simulation@do_angular_weighting` | int | TCR, external mode only | Surface angular weighting. GUI mode forces 1 (`ctr/core_configuration.cpp:27`) | never | `ctr/core_configuration.cpp:33` |

TCR also parses `simulation@output_recs_byfreq` and `simulation@abs_atmo_calc` (above).

### `sources`

Each child of `<sources>` is one source, whatever its element name. A source's index is its list
position; **`@id` is not read** (`base_core_configuration.cpp:121-183, 153`). With no
`<sources>`, nothing is emitted (inferred).

| Attribute | Type, unit | Read by | Meaning; missing or bad value | Writer | Receipt |
|---|---|---|---|---|---|
| `source@x`, `source@y`, `source@z` | real, m | both | Position, which must lie strictly inside the meshed volume. **Outside: SPPS crashes with `0xC0000005` and no message**, because `TranslateSourceAtTetrahedronVertex` dereferences a NULL tetrahedron (`sppsInitialisation.cpp:13-20`). The guard at `sppsNantes.cpp:58-65` is never reached. TCR exits 0 with results (VERIFIED P2 `src_out`, `tcr_src_out`; S `run_srcout`). The on-face check at `sppsNantes.cpp:322-325` prints to stderr and exits 0 without results when it fires, but a source on the floor plane did not trigger it (VERIFIED P2 `src_face`). Missing: 0 | always | `base_core_configuration.cpp:131`; `coreinitialisation.cpp:71-96` |
| `source@directivite` | int, `SOURCE_TYPE` | both | 0 omni, 1 unidirectional, 2 XY plane, 3 YZ plane, 4 XZ plane, 5 directivity balloon (`coreTypes.h:96-104`). Missing: 0. TCR's physics treats every source as omni (`TC_CalculationCore.cpp:160-180`). SPPS has no branch for other values (`sppsNantes.cpp:107-127`) (inferred) | always | `base_core_configuration.cpp:132` |
| `source@u`, `source@v`, `source@w` | real, direction | both, only for types 1 and 5 | Normalised on read. A zero vector gives NaN (inferred) | if type 1 or 5 | `base_core_configuration.cpp:135-139` |
| `source@delay` | real, s | both, used by SPPS | Emission start step = ceil(delay/pasdetemps), **cast to u16** (`sppsNantes.cpp:91`). The source emits only if that step is before the end (`sppsNantes.cpp:93`). VERIFIED P2 `delay_wrap`: a 655.37 s delay (65,537 steps) in a 200-step run emitted from step 1 instead of never. Missing: 0 | always | `base_core_configuration.cpp:133` |
| `source@name` | string | both, used by SPPS | **Copied with `strcpy` into a 50-byte GABE cell** (`gabe.cpp:174-178`, `gabe.h:230-233`) in `Sound level per source.recps` (`spps/reportmanager.cpp:593-595`). VERIFIED P2 `name_long`: a 120-byte name overflowed and left an unterminated 50-byte cell, and SPPS still exited 0. The name is also a folder name when `output_recp_bysource` is set (`spps/reportmanager.cpp:650`). Missing: `""` | always | `base_core_configuration.cpp:134` |
| `source@directivity_file` | string, file name | both, type 5 only | Loaded from wd + `directivities_directory` + this, once per distinct name (`base_core_configuration.cpp:155-178`). **Missing attribute: SPPS crashes `0xC0000005`** (VERIFIED P2 `dir_noattr`, S `run_dirempty`); TCR runs (P2 `tcr_dir_noattr`). File not found: the source emits nothing in any band, exit 0 (VERIFIED S `run_dirmiss`). See "Directivity files" | if type 5 | `base_core_configuration.cpp:155-178`; `sppsNantes.cpp:84-88` |
| `source/bfreq@db` | real, dB re 10⁻¹² W | both | Sound power level per band: w = 10⁻¹²·10^(db/10) W (`coreTypes.h:41`). Mapped by position | always | `base_core_configuration.cpp:147-149` |
| `source/bfreq@freq` | int | both | Sort key only. See "Bands" | always | `base_core_configuration.cpp:140` |

### `surface_absorption_enum`

Each child is one material, whatever its element name (`base_core_configuration.cpp:185-227`).

| Attribute | Type, unit | Read by | Meaning; missing or bad value | Writer | Receipt |
|---|---|---|---|---|---|
| `type_surface@id` | int (`atoi`, stored unsigned) | both | Matched with a `.cbin` face's `idMat`; the first matching material wins (`base_core_configuration.cpp:374-381`). **An undeclared non-zero id prints `Wrong project configuration…` on stderr and calls `exit(-1)`** (VERIFIED P2 `mat22_miss`, `tcr_mat22_miss`: `0xFFFFFFFF` from both solvers). **Id 0 is looked up once, before the loop**, so faces with idMat 0 that come before any other id get a NULL material unchecked, and SPPS crashes `0xC0000005` (VERIFIED S `run_mat0miss`). Faces with idMat 0 after another id do exit -1 | always | `base_core_configuration.cpp:196`; `coreinitialisation.cpp:409-434` |
| `type_surface@side_material` | int | both, *optional* | `==1` means double-sided; any other value means single-sided, where a particle crosses the back of the face when a tetrahedron lies behind it (`CalculationCore.cpp:218`). **Missing: double-sided** (the constructor's default, `coreTypes.h:137`) | always | `base_core_configuration.cpp:197-200` |
| `type_surface/bfreq@absorb` | real, α | both | Absorption coefficient. Not range-checked | always | `base_core_configuration.cpp:208` |
| `type_surface/bfreq@diffusion` | real, 0 to 1 | both, used by SPPS | Probability of a non-specular reflection: `diffusion==1 \|\| rand<diffusion` (`CalculationCore.cpp:318`) | always | `base_core_configuration.cpp:209` |
| `type_surface/bfreq@loi` | int, `REFLECTION_LAW` | both, used by SPPS | 0 specular, 1 uniform, 2 Lambert, 3 W2, 4 W3, 5 W4, 6 semi-diffuse (`coreTypes.h:83-92`). **6 falls through to the default branch and reflects specularly** (`dotreflection.h:23-45`) | always | `base_core_configuration.cpp:220-221` |
| `type_surface/bfreq@affaiblissement` | real, dB transmission loss | both, *optional*, used by SPPS | Its presence enables transmission with τ = 10^(-R/10) (`base_core_configuration.cpp:210-219`), used only when `trans_calc` is set. Absent: no transmission. The solver does no α/τ consistency check. Upstream's GUI leaves it out of a band whose absorption is 0 (`e_data_row_materiau.h:98-106, 131-134`), and so does the writer | if the material transmits | `base_core_configuration.cpp:210-219` |
| `type_surface/bfreq@freq` | int | both | Sort key only | always | `base_core_configuration.cpp:201` |

### `recepteursp` (point receivers)

Each child is one receiver, whatever its element name (`base_core_configuration.cpp:229-262`).

| Attribute | Type, unit | Read by | Meaning; missing or bad value | Writer | Receipt |
|---|---|---|---|---|---|
| `recepteur_ponctuel@x`, `recepteur_ponctuel@y`, `recepteur_ponctuel@z` | real, m | both | Position. **Outside the mesh, the receiver is never linked to a tetrahedron and records zero, silently** (VERIFIED P2 `rcv_out`: `.recp` sum 0.0, exit 0) | always | `base_core_configuration.cpp:244`; `coreinitialisation.cpp:178-213`; `sppsInitialisation.cpp:82` |
| `recepteur_ponctuel@id` | int | both | Stored. TCR's external mode uses it as a column label (`ctr/reportmanager.cpp:98`) | always | `base_core_configuration.cpp:245` |
| `recepteur_ponctuel@lbl` | string | both | **A folder name in SPPS and a file name in TCR (`<lbl>.gabe`), both unvalidated.** When the SPPS folder already exists, 0 to 19 is appended (VERIFIED P2 `label_dup`: `Receiver 1` and `Receiver 10`). In TCR the second receiver overwrites the first (VERIFIED P2 `tcr_label_dup`: one file). UTF-8 works (VERIFIED P2 `label_utf8`: `Récepteur Ł`) | always | `base_core_configuration.cpp:246`; `baseReportManager.cpp:190-208`; `ctr/reportmanager.cpp:148` |
| `recepteur_ponctuel@u`, `recepteur_ponctuel@v`, `recepteur_ponctuel@w` | real, direction | both, used by SPPS | Orientation, normalised on read and used in SPPS's lateral-energy terms (`spps/reportmanager.cpp:222`). A zero vector gives NaN (inferred). Missing: 0/0 | always | `base_core_configuration.cpp:248-249` |
| `recepteur_ponctuel/bfreq@db` | real, dB | both, used by SPPS | Background noise per band, written into the `.gap` (`spps/reportmanager.cpp:839`). The array is sized by the band count, so this is safe | always | `base_core_configuration.cpp:250-259` |
| `recepteur_ponctuel/bfreq@freq` | int | both | Sort key only | always | `base_core_configuration.cpp:250` |

### `recepteurss` (surface receivers and cutting planes)

Here **the child element name is checked**. Other children are ignored
(`base_core_configuration.cpp:263-302`).

| Attribute | Type, unit | Read by | Meaning; missing or bad value | Writer | Receipt |
|---|---|---|---|---|---|
| `recepteur_surfacique@id` | int | both | Matched with a `.cbin` face's `idRs`. A face whose `idRs` is not declared indexes the receiver vector at -1 (`coreinitialisation.cpp:291-304, 353-360`) (inferred). **If the first declared receiver has no faces, the `Global\` file is not written, even when other receivers have faces** (`baseReportManager.cpp:393-394, 432-433`) (inferred) | if surface receivers | `base_core_configuration.cpp:275` |
| `recepteur_surfacique@name` | string | both | Stored in the RSBIN, truncated to 254 bytes (`baseReportManager.cpp:42, 106, 253`; `rsbin.h:53, 93`) | if surface receivers | `base_core_configuration.cpp:276` |
| `recepteur_surfacique_coupe@id` | int | both | Cutting-plane id. Its presence turns cutting planes on (`base_core_configuration.cpp:280`) | if cutting planes | `base_core_configuration.cpp:283` |
| `recepteur_surfacique_coupe@name` | string | both | As for surface receivers | if cutting planes | `base_core_configuration.cpp:282` |
| `recepteur_surfacique_coupe@ax`, `recepteur_surfacique_coupe@ay`, `recepteur_surfacique_coupe@az`, `recepteur_surfacique_coupe@bx`, `recepteur_surfacique_coupe@by`, `recepteur_surfacique_coupe@bz`, `recepteur_surfacique_coupe@cx`, `recepteur_surfacique_coupe@cy`, `recepteur_surfacique_coupe@cz` | real, m | both | Plane corners A, B and C. The grid spans BC by BA | if cutting planes | `base_core_configuration.cpp:284-287` |
| `recepteur_surfacique_coupe@resolution` | real, m | both | Cell size: ceil(\|BC\|/res) × ceil(\|BA\|/res) cells, unchecked. 0 divides by zero (inferred) | if cutting planes | `base_core_configuration.cpp:288-298` |

### `encombrement_enum` (fitting zones)

Each child is one fitting, whatever its element name (`base_core_configuration.cpp:303-330`).

| Attribute | Type, unit | Read by | Meaning; missing or bad value | Writer | Receipt |
|---|---|---|---|---|---|
| `encombrement@id` | int | both | Matched with a tetrahedron's `idVolume` in the `.mbin` and a face's `idEn` in the `.cbin`. The first match wins (`base_core_configuration.cpp:384-391`). An undeclared id silently means no fitting (`coreinitialisation.cpp:151-176, 437-445`). **idVolume 0 means "no fitting", but TetGen `-A` gave every one of tutorial 1's 2,257 room tetrahedra idVolume 1** (read from `tests/fixtures/upstream/tutorial1/spps/tetramesh.mbin`), so a fitting with id 1 would fill the whole room (inferred) | if fittings | `base_core_configuration.cpp:313` |
| `encombrement/bfreq@alpha` | real | both, used by SPPS | Absorption per collision with a fitting element (`CalculationCore.cpp:137-151`) | if fittings | `base_core_configuration.cpp:321` |
| `encombrement/bfreq@lambda` | real, m | both, used by SPPS | Mean free path: the distance drawn is -λ·ln(1 - u) (`CalculationCore.cpp:37`) | if fittings | `base_core_configuration.cpp:322` |
| `encombrement/bfreq@loi_diff` | int, `DIFFUSION_LAW` | both, used by SPPS | 0 uniform, 1 uniform reflection, 2 Lambert reflection (`coreTypes.h:108-113`). Other values leave the direction unchanged, since the switch has no default (`CalculationCore.cpp:166-182`) | if fittings | `base_core_configuration.cpp:323` |
| `encombrement/bfreq@freq` | int | both | Sort key only | if fittings | `base_core_configuration.cpp:314` |

**Count.** These tables list **94 attributes** that at least one solver reads: 1 on the root, 8
in `condition_atmospherique`, 12 file and time attributes in `simulation`, 2 in `freq_enum`, 18
SPPS settings, 5 TCR settings (TCR also parses `abs_atmo_calc` and `output_recs_byfreq`, which
are counted with SPPS's), 12 per source, 7 per material, 10 per point receiver, 2 per surface
receiver, 12 per cutting plane and 5 per fitting. Of these, 8 are *optional*: read with
`IsPropertyExist`, so no message when missing. They are `recepteurss_cut_filename`,
`output_recp_bysource`, `random_seed`, `save_surface_intersection`,
`save_receivers_intersection`, `direct_recepteurSOutputName`, `side_material` and
`affaiblissement`.

## Directivity files

A type-5 source's file is read by `xhn_DirectivityParser::parse`
(`directivityParser.cpp:38-112`) in **both** solvers, although only SPPS uses it. Upstream's
sample is `spps/tests/speaker-test3.txt`: 24 frequencies from 40 to 8000 Hz, with 72 φ rows each.

- **Opening.** The path is wd + `directivities_directory` + `directivity_file`, opened through
  the wide API (`directivityParser.cpp:41`). If it cannot be opened, stdout gets `DirectivityBalloon : File not open`
  (`directivityParser.cpp:45-47`), and the source emits nothing in any band.
- **Lines.** Empty lines and lines starting with `;` are skipped. The rest is split on `,`
  (`directivityParser.cpp:60-62`).
- **Frequency blocks.** `"Frequency",<f>,<unit>` with exactly 3 fields sets the current
  frequency, `stod(<f>)` (`directivityParser.cpp:66-69`).
- **Data rows** need exactly **38 fields** and a current frequency above 0 (`directivityParser.cpp:71`). Field 1 has
  every non-digit character removed and is read as φ in degrees (`directivityParser.cpp:73-74`). That strips `-` and
  `.` too, so `"  5°"` reads 5. φ must be 0 to 360 and a multiple of 5 (`directivityParser.cpp:77`). Fields 2 to 38
  are θ = 0°, 5°, ..., 180°. Values are dB, and the particle's energy is multiplied by 10^(v/10)
  (`sppsNantes.cpp:119-126`). The θ = 0° and θ = 180° values of the first row are reused for
  every φ (`directivityParser.cpp:84-96`).
- **Rows that don't fit are skipped silently.** VERIFIED P2 `dir_badrow`: with a trailing comma
  on every data row, each row had 39 fields and was skipped. Every band then had 0 particles, and
  the run exited 0.
- **A non-numeric field aborts both solvers.** `stod` throws `std::invalid_argument`, which
  nothing catches: `0xC0000409` (VERIFIED P2 `dir_nan`, `tcr_dir_nan`).
- **Lookup is by exact frequency.** SPPS compares the band's integer `@freq` with the file's
  frequencies (`directivityBalloon.cpp:49-52`, `sppsNantes.cpp:84-88`). A band that is not in the
  file is skipped for that source; the only message needs `-v`. VERIFIED P2 `dir_partial`: with
  upstream's sample, bands from 10 to 20 kHz had total 0, exit 0.
- **Interpolation** is bilinear between 5° neighbours (`directivityBalloon.cpp:79-107`). It
  assumes every φ row from 0 to 355 exists for the frequency. A missing row makes `operator[]`
  create an empty map, whose `upper_bound` result is then dereferenced (`directivityBalloon.cpp:88-96`) (inferred).

## Ignored by the solvers

Upstream's GUI writes the following, and no solver reads any of it.

| GUI writes | Written by | Why it is ignored |
|---|---|---|
| `type_surface@resistivite` | `e_scene_bdd_materiaux_propmateriau.h:72` | Never looked up |
| `condition_atmospherique@lst_soltype` | `e_scene_projet_environnement.h:165` | Never looked up. The GUI converts it to `z0` itself (`e_scene_projet_environnement.h:205-207`) |
| `simulation@intensity_filename`, `simulation@intensity_folder`, `simulation@intensity_rp_filename` | `e_core_sppscore.h:69-71` | SPPS hard-codes `Intensity animation/<f> Hz/Intensity.rpi` and `Punctual receiver intensity.gabe` (`spps/reportmanager.cpp:719, 755`; `sppsNantes.cpp:402`) |
| `source@id` | `e_scene_sources_source.h:114` | A source's index is its list position (`base_core_configuration.cpp:153`) |
| `recepteur_ponctuel@name` | `e_scene_recepteursp_recepteur.h:113` | The solvers use `@lbl` (`base_core_configuration.cpp:244-249`) |
| `<subdomains>`, and `volume@id` and `volume@name` inside it | `e_scene_volumes_volume.h:146-153` | No `GetChild("subdomains")` |
| `type_surface@masse_volumique` | `e_scene_bdd_materiaux_propmateriau.h:71`, on user materials (tutorial 3) | Never looked up |
| `encombrement@x`, `encombrement@y`, `encombrement@z` | A fitting zone's exported properties (`e_scene_encombrements_encombrement_model.h:146-157`, tutorial 3's zone 1930) | A fitting is read as `@id` and its band entries only (`base_core_configuration.cpp:303-330`) |

**Receipts.**
- **The last two rows** (from tutorial 3's configs) are read from the code only: the names are
  not among those passed to `GetProperty` or `IsPropertyExist` (grep below).
- **VERIFIED P1.** All six rows were removed from both tutorial-1 configs: 279 characters from
  SPPS's and 150 from TCR's. The results were the same:
  - every output file was identical: SPPS 65 files, TCR 87. `.csbin` files were compared by size,
    because their padding is nondeterministic (M1)
  - stdout was identical apart from the run path
  - stderr was identical
- **Grep.** The names passed to `GetProperty`, `IsPropertyExist`, `GetChild` and
  `OrderChildsByProperty` in `spps/`, `ctr/` and `lib_interface/` include none of `resistivite`,
  `lst_soltype`, `intensity_*` or `subdomains`. `id` and `name` are read only on the other
  elements listed above.

Our writer writes none of them, and no `<surface_mesh>` or `<vertices>` either (M3).

## Parity with upstream's GUI

**The references.** Every run folder stored in upstream's tutorials at 929a5c8 holds the
`config.xml` and `mesh.cbin` the GUI handed the solver, and the project file it saved beside
them: tutorial 1's SPPS and TCR runs (2019-06-07) and tutorial 3's three SPPS runs (2019-06-18).
Tutorial 2 and `Industrial.proj` store no run folder. `crates/simpa-core/tests/parity_inputs.rs`
reads them from the upstream tree and compares what the solvers read from theirs and from ours,
value by value: `atoi` for an integer, `atof` into an `f32` for a real (by bits), a string
verbatim, each element where the solvers find it (lists by position, materials by id, band
entries by their rank after the solvers' sort). An attribute neither in the reference tables nor
in "Ignored by the solvers" fails the test, so nothing is skipped unseen. The scene mesh is
compared in `docs/formats/cbin.md`, "Parity with upstream's GUI".

**Four rules of the writer that come from this comparison** (2026-09-24):

1. **Lists are written last item first:** sources, point receivers, surface receivers and cutting
   planes, fitting zones. Upstream's GUI writes each child with `new wxXmlNode(parent, ...)`,
   which puts it first among its parent's children (`e_scene_sources_source.h:113`,
   `e_scene_recepteursp_recepteur.h:111`, `e_scene_recepteurss_recepteur.h:119`,
   `e_scene_recepteurss_recepteurcoupe.h:204`, `e_scene_encombrements_encombrement_model.h:150`,
   `e_scene_encombrements_encombrement_cuboide.h:354`), while it walks its elements in the order
   it holds them: the order it loaded them in, sorted by their id in the project file
   (`element.cpp:159`), with a new element appended (`element.cpp:850-852`). So each list comes
   out newest element first. Measured (`upstream_writes_every_list_newest_element_first`): in
   all five runs, each of the 11 lists of two or more is in strictly descending id, across
   tutorial 3's two source groups too. The order is solver input: sources and receivers are
   numbered by their position in the file, a seeded SPPS run draws its particles source by source
   in that order, and whether the `Global` surface-receiver file is written depends on the first
   receiver (`baseReportManager.cpp:393, 432`). A project holds the GUI's own order: the `.proj`
   import reads the project file in document order, which ascends by id in all four of upstream's
   tutorial projects, and `import_upstream` reverses the config's lists. Band entries are still
   written ascending; upstream's come descending for the same reason, and the solvers sort them
   (`cxml.cpp:130-133`).
   - **Not reproduced:** when the GUI fills its tree it also sorts each level by label
     (`element.cpp:516-519`), but that comparison reads freed memory: `WXSTRINGTOCHARPTR` is
     `(const char*)wxstr.mb_str()` (`isimpa/UtfConverter.h:37`), a pointer into a temporary buffer that
     is freed before `alphanum_comp` reads it (`element.cpp:568-577`). What order that sort
     leaves is not defined. In all five runs the lists come out in load order, which is also
     label order there, so the runs cannot tell the two apart.
   - **Open, in the `.proj` import:** a project file saved again after a new element was added
     lists that element first (the incremental save prepends a new node, `element.cpp:600-614`),
     and upstream's load moves it back by id. That sort (`element.cpp:64-104`) compares every
     node with the first node's id only, so it does not always finish sorted: `10, 30, 20` stays
     as it is. Our import reads document order, which equals upstream's load order whenever the
     document ascends by id. Each of upstream's four tutorial projects does.
2. **White and pink noise on upstream's 27 bands are computed as upstream's GUI computes them:**
   in `f32`, from its reference levels rounded to 2 decimals (`config_xml::band_levels_written`;
   `E_Property_Freq::LoadLwFromBdd` and `SetGlobalLevel`, `generic_element/e_property_freq.cpp`).
   Measured: the 39 source and receiver spectra of the five runs are all equal to upstream's
   floats in every band, where the `f64` formula (`Spectrum::band_levels_db`) misses 38 band
   values by one unit in the last place. Other shapes and other band sets keep the `f64` formula.
3. **A band that absorbs nothing has no `affaiblissement`,** as upstream's GUI writes it (see the
   attribute's row). Until 2026-09-24 the writer put a 300 dB loss there. In random mode a
   particle hitting such a band is absorbed only when its draw is exactly 0, and then transmits
   if the band has a loss (`CalculationCore.cpp:288-292`); without the attribute it never does,
   as with upstream's input.
4. **SPPS gets `directivities_directory="loudspeakers\"` in every run,** upstream's
   `CONST_REPORT_DIRECTIVITIES_FOLDER_PATH` (`appconfig.cpp:61`, `e_core_sppscore.h:217`), and a
   run's directivity files go in that folder. Until 2026-09-24 the writer wrote `""` when no
   source had a file. SPPS reads and stores the value (`base_core_configuration.cpp:96`), though
   it uses it only for a type-5 source. TCR keeps `""` when no source has a file: see below.

**Measured** on the five runs, with our config written into upstream's own run folder so that
`workingdirectory` is equal:

| Our project | Values the solver reads differently |
|---|---|
| `tutorial_1.proj` imported, SPPS run | 16 lines: 3 ids, 6 stored directions and 7 by-design lines, all listed below |
| `tutorial_1.proj` imported, TCR run | 15 lines: 3 ids, 6 stored directions and 6 by-design lines |
| tutorial 3's config and `.cbin` imported (with the two edits below) and written back, each run | 36 lines: 8 ids, 21 by-design lines and the 7 band values of the two edits; and a same-seed run of ours beside upstream's config with the same two edits gives 24 of 24 output files identical, the cutting plane's `.csbin` once decoded with its id mapped (`tutorial3_written_back_gives_upstreams_output`) |
| `tests/fixtures/projects/tutorial1.simpa` (tutorial 1's config and `.cbin` imported) | the ids and the by-design lines (`config_xml_import.rs`); and a same-seed run of ours beside upstream's own config gives every output file identical, the `.csbin` files once decoded with the receiver's id mapped (`config_xml_solver.rs`, `tutorial1_runs_clean_and_matches_upstreams_own_configuration`) |

Every expected line is listed in the test, so a new difference fails it and so does one that
goes away. Each comparison has its refusal in the same test: one `f32` step on one band of one
value, a changed temperature, a changed transmission loss and the receivers in the other order
are each reported. The comparison itself has its own test
(`the_comparison_gives_one_line_per_value_the_solver_reads_differently`): an int, a real one
`f32` step off, a string, an attribute left out and an element left out each give exactly one
line, while upstream's 15-digit text for a real and band entries in the other order give none.

**What still differs, why, and what the solver does with it.**

- **Ids.** `recepteur_ponctuel@id`, `recepteur_surfacique@id`, `recepteur_surfacique_coupe@id`
  and `encombrement@id` are ours, assigned from project order (`config_xml.rs`, "Solver ids"):
  tutorial 1's receivers 3669 and 3510 are our 1 and 0 and its scene receiver 3503 our 0;
  tutorial 3's fitting zones 2083 and 1930 are our 3 and 2. Upstream's are its GUI's session
  counters, renumbered at every load (`element.cpp:134, 143-144`; `docs/formats/cbin.md`, "What
  still differs", 1), and a project has nowhere to hold them. A point receiver's id is only
  stored in GUI mode. A scene receiver's and a fitting's are matched with the `.cbin` and
  `.mbin`, which carry ours, so every face gets the same receiver and fitting. A scene
  receiver's or cutting plane's id is also written into its `.csbin` output (`xmlIndex`), where
  3503 reads 0. **Measured:** same-seed runs of upstream's config against ours give every other
  output file byte for byte: tutorial 1 (`config_xml_solver.rs`), and each of tutorial 3's three
  runs, 24 of 24 files, its receivers, cutting plane and fitting zones all numbered ours
  (`parity_inputs.rs`, `tutorial3_written_back_gives_upstreams_output`; `docs/formats/cbin.md`).
  Equal ids would need upstream's stored in the project, a change to the `.simpa` format: a
  decision, not a writer fix.
- **Point-receiver directions (tutorial 1).** Upstream's GUI computes a receiver's direction when
  its position changes, and holds it at full precision for the rest of that session, which is
  when these runs were written (`-0.436852067708969`). Its project file keeps 6 significant
  digits (`-0.436852`), and the direction is recomputed only on a move
  (`e_scene_recepteursp_recepteur.h:195-219`), so upstream itself writes the 6-digit value after
  reopening the project (inferred; the GUI was not run). The import reads what the file holds.
  The two are up to 5e-7 apart in a unit vector, which SPPS uses only in its lateral-energy term
  (`spps/reportmanager.cpp:222`).
- **Only upstream's:** `<subdomains>` (ignored); `source@u`, `@v` and `@w` on an omni source (read
  only for types 1 and 5, `base_core_configuration.cpp:135-139`); `type_surface` 0, the GUI's
  default material, when no face uses it (the solvers look material 0 up once,
  `coreinitialisation.cpp:410`, and use it only for a face with `idMat` 0).
- **`directivities_directory` in TCR:** upstream's TCR config lacks it, so TCR prints
  `Xml Property directivities_directory doesn't exist !` and reads `""` (`cxml.cpp:108-118`).
  Ours writes `""`, the same value, without that line, which the contract fails
  (`xml_property_missing`). With a directivity file TCR gets `loudspeakers\`, where the file is;
  upstream's TCR would look for it in the run folder itself.
- **Only ours:** `save_surface_intersection` and `save_receivers_intersection`, at 1, the value
  SPPS takes when they are absent (`spps/core_configuration.cpp:50-59`). Upstream's
  GUI never writes them.
- **Tutorial 3 needs two edits to become a project.** Its `.proj` is refused for its fitting zones
  (`geometry::import`), and its config imports only with these:
  - material 100 has reflection law 2 (Lambert) in 6 of 27 bands and 0 in the others; a project
    holds one law per material, so the test sets law 0 in all, and those 6 values differ;
  - material 101 ("Open_door") transmits with a 0 dB loss in 5 of the 6 bands where it absorbs,
    but not at 125 Hz; a project's material transmits in every band that absorbs or in none, so
    the test gives it 0 dB at 125 Hz too, and that one value differs.

**Formatting only** (the solvers read the same values): upstream prints each real as its `f32`
at 15 significant digits (`0.310000002384186`), ours as the shortest decimal of the project's
value (`0.31`), and both read back to the same `f32`; band entries descending against ascending;
the order of the materials (looked up by id, `base_core_configuration.cpp:376-379`); the order of
the root's elements and of attributes (the solvers look both up by name); the attributes in
"Ignored by the solvers"; indentation and line ends.

## Corrections to the contract survey

The survey's 57 `config_xml` rows were checked against the source and against P1/P2. Changed:

- **An empty `workingdirectory` is not "cwd-relative" for SPPS.** It aborts `0xC0000409` after
  the solve (P2 `wd_empty`). TCR does run cwd-relative.
- **`trans_epsilon` missing kills the run in random mode too.** In random mode, the GUI default,
  every particle dies at its first surface hit (P2 `eps_short`). The survey showed only the
  energetic-mode case.
- **A missing `condition_atmospherique`** is verified: 100 % of particles are lost by meshing
  problems, with exit 0 (P2 `no_atmo`). It was inferred before.
- **A missing `pasdetemps`** is verified: a crash, `0xC0000409` (P2 `no_dt`). It was "UB,
  inferred".
- **`surf_receiv_method` 1 divides by cos(incidence) as well** as scaling by I0/p0²·ρc. Mode 0
  applies no angle weighting at all.
- **`recepteur_ponctuel@id` is not only stored.** TCR's external mode uses it as a column label.
- **`loi` 6 (semi-diffuse) reflects specularly** in SPPS (`dotreflection.h:43-44`).
- **Directivity: two new silent failures.** A band missing from the file is one (P2
  `dir_partial`). A row whose field count is not 38 is the other (P2 `dir_badrow`). A
  non-numeric field aborts both solvers (P2 `dir_nan`, `tcr_dir_nan`): verified, where it was
  inferred.
- **The ignored list** gains `recepteur_ponctuel@name` and `<subdomains>` (P1).
- **The "(all numeric attributes)" row** is split into integer and real parsing. `atoi` stops at
  the first non-digit.
- **Upstream's own TCR config is not clean.** It prints `Xml Property directivities_directory
  doesn't exist !` (P1). A classifier that counts `Xml Property` lines as FAIL would fail
  upstream's GUI output, so our writer must always write that attribute.

## Not examined

- TCR external mode in an actual run.
- Cutting planes and fittings in an actual run, beyond parity: tutorial 3's runs show that ours
  and upstream's inputs give the same output (`parity_inputs.rs`), not what that output means.
- Source types 1 to 4 in an actual run.
- The directivity balloon's orientation convention (`directivityBalloon.cpp:109-150`).
- Configs in UTF-16 or Latin-1: pugixml supports them, and the writer uses UTF-8.
- Behaviour on large models.
