# TetGen source archaeology: upstream I-Simpa meshing

Scratch folder: `B:/repos/I-Simpa_Night_Mode/target/investigate/tetgen-archaeology/`. Nothing was deleted and no tracked files were touched.

## Headline

Upstream did not modify TetGen. It upgraded from 1.5.0 to 1.6.0 on 2021-04-20, the commit right after the v1.3.4 tag. Official 1.6.0 dropped the facet area check.

- Upstream's 2019 tutorial mesh came from TetGen 1.5.0 with `-pq2 -A -n` plus the `.var`. Rerunning that gives byte-identical output.
- Upstream's own current release (v1.4.0 snapshot, TetGen 1.6.0) gives the same 6-tet mesh we get. So this is a regression inside upstream itself, not something our build introduced.

## Findings with receipts

**F1. The vendored copy is official 1.6.0, byte for byte apart from line endings.**
- WIAS tarballs were fetched directly from `https://wias-berlin.de/software/tetgen/1.5/src/tetgen{1.6.0,1.5.1,1.5.0}.tar.gz`, all HTTP 200. sha256:
  - 1.6.0: `87b5e61e…ce39`
  - 1.5.1: `e46a4434…b850`
  - 1.5.0: `4d114861…9cf3`
- After stripping CR, the vendored `tetgen.cxx`, `tetgen.h` and `predicates.cxx` hash the same as the official 1.6.0 files, and `diff` prints 0 lines. `tetgen.cxx` sha256 is `a10f5e74…36bc`.
- The size gaps equal the line counts exactly (CRLF only):
  - `tetgen.cxx`: 1,323,080 − 1,286,513 = 36,567 lines
  - `tetgen.h`: 149,544 − 145,931 = 3,613 lines
  - `predicates.cxx`: 197,745 − 193,035 = 4,710 lines
- I found no I-Simpa-specific edits.
- Version string: `tetgen.h:7-8` reads "Version 1.6.0 / August 31, 2020".
- `src/tetgen/README` still says "TetGen version 1.5 (released on November 4, 2013)". The README and LICENSE match official 1.5.0 after CR stripping; the 1.6.0 tarball has no LICENSE. These are stale leftovers from the 2016 vendoring.

**F2. Upstream history of `src/tetgen/tetgen.cxx`.** GitHub API, commits filtered by that path, returns only 2 commits:
- `4db335c123eb`, 2016-08-11, nicolas-f, "Build tetgen from sources". Raw files at that commit are identical to official 1.5.0 (0 diff lines; 1.5.1 differs by 6,258 lines).
- `ef2f7648ce4c`, 2021-04-20, wbinek, "Update tetgen to 1.6". It touches only `predicates.cxx`, `tetgen.cxx` and `tetgen.h`. Its parent is `a0d4d4330d8c`, which is exactly the commit tagged **v1.3.4** (release published 2020-12-23).
- TetGen version at each tag (raw `tetgen.h` line 7): v1.3.3 and v1.3.4 are "Version 1.5"; v1.3.5_snapshot_2026_01_09 is "Version 1.6.0".

**F3. Pinned 1.6.0 never reads the facet area bound anywhere in refinement.**
- Every call that reads `areabound(` is a copy into `setareabound` when an element is split or flipped: `tetgen.cxx` 12362, 12607, 12727, 12899-12900, 13030, 16539.
- `areaboundindex` appears only at `tetgen.cxx:5048-5054` (allocation) and `tetgen.h:3049-3056` (accessors).
- The bound is written at 13898-13904 and 25066-25077, only when `b->quality` is set. The segment length bound from the `.var` is written at 25081-25097 and is also never read.
- `checkconstraints` is referenced at 4761, 4860, 4865, 5051, 12360, 12605, 12725, 12898, 13029, 16538 and 25017. None of those is a refinement test.
- `check_subface` (27347-27388) tests only `radius/emin > b->minratio`.
- The subface refinement phase is `delaunayrefinement` 29377-29551. It is gated `if (!b->nobisect)`, needs `cdtrefine&2` (default 7, `tetgen.h:748`), and uses only `check_enc_subface` plus `check_subface`.
- `repairencfacs` (27953-28050) uses the same two tests.

**F4. No switch combination makes pinned 1.6.0 honour the `.var` facet bound.**
- A subface can be split only by encroachment or by the radius-edge ratio (`-q`), and only without `-Y`.
- `-Y` switches off all subface splitting: 29377, and the `split_tetrahedron` branches at 28821/28884 and 28888/29015.
- `-a#`, `-a` (`.vol` or regions) and `-m`/`.mtr` act only on tets, in `check_tetrahedron` 28191-28264 and `checktet4split` 28373-28402. Faces feel them only indirectly, when a rejected tet Steiner point encroaches a subface.
- `in->tetunsuitable` (28404) is a library callback with no command-line switch.
- The `.var` file is loaded (`tetgenio::load_plc` 2447-2450; stdout prints "Opening scene_mesh.var.") and then has no effect.

**F5. Official 1.5.0 and 1.5.1 do split subfaces by area bound; 1.6.0 removed that code.**
- 1.5.0:
  - `checkfac4split` (line 24535) at 24580-24585: `if (checkconstraints && (areabound(*chkfac) > 0.0)) { if (area > areabound(*chkfac)) { qflag = 1; return 1; } }`
  - `checkseg4split` (24149) has the same check for segment length at 24168-24169.
  - Refinement is gated `if (!b->nobisect || checkconstraints)` at 25415.
  - Under `-Y`, `splitsubface` (24676, from 24685) splits only facets with a nonzero bound.
- 1.5.1 has the same structure: area test at 25517-25522; the subface check also covers `-a#`, `-a` and `-m` sizing (25524-25562); gate at 26412; `-Y` handling at 25659-25669.
- 1.6.0 has 0 occurrences of `checkfac4split` or `checkseg4split`; 1.5.0 and 1.5.1 each have 6.

**F6. Measured on the exact 2019 inputs** (`scene_mesh.poly` and `.var` extracted from `tutorial_1.proj` `instance2/temp/`).

The `.var` constrains facet markers 0 and 1, which are the two floor triangles, to 0.100000001 m². The TetGen footer inside the 2019 `.1.node/.ele/.face` reads `# Generated by meshing\tetgen\tetgen.exe -pq2 -A -n D:\picaut\...\scene_mesh.poly`: no `-a`, no `-Y`.

| TetGen binary | flags | .var | nodes | tets | floor faces | max floor area (m²) |
|---|---|---|---|---|---|---|
| 2019 upstream files | -pq2 -A -n | yes | 732 | 2257 | 934 | 0.0998 |
| official 1.5.0 (MSVC build) | -pq2 -A -n | yes | 732 | 2257 | 934 | 0.0998 |
| upstream release 1.3.4 `tetgen.exe` | -pq2 -A -n | yes | 732 | 2257 | 934 | 0.0998 |
| official 1.5.1 (MSVC build) | -pq2 -A -n | yes | 679 | 2110 | 944 | 0.3750 |
| 1.5.0 | -pq2 -A -n -Y | yes | 11 | 18 | 8 | 7.5 |
| 1.5.0 / release 1.3.4 | -pq2 -A -n | no | 31 | 60 | 10 | 13.43 |
| our 1.6.0 | -pq2 -A -n, ±.var, ±-Y | either | 8 | 6 | 2 | 30.0 |
| upstream release 1.4.0 `tetgen.exe` | -pq2 -A -n, -pq5 -A -n | yes | 8 | 6 | 2 | 30.0 |
| our 1.6.0 | -a0.1 -pq2 -A -n | no | 781 | 3314 | 259 | 0.579 |
| 1.5.0 | -a0.1 -pq2 -A -n | no | 1214 | 4406 | 496 | 0.2134 |

Output hashes, computed with CR stripped and the `# Generated` line removed:

| file | 2019 = rebuilt 1.5.0 = release 1.3.4 | release 1.4.0 = our 1.6.0 |
|---|---|---|
| `.node` | `566f2484dfe2207e` | `1bd16b3cd111cf78` |
| `.ele` | `2ff1dd51f3c4f01f` | `4096e024e88f7d07` |
| `.face` | `d4bc5d582f3d4ca8` | `070253660ec2bf7e` |

Rebuilt 1.5.0 also matches the 2019 `.edge` and `.neigh` files.

Where the binaries came from:
- `rel/isimpa-1.3.4-win64.zip`, 104,059,742 bytes. `meshing/tetgen/tetgen.exe` is 452,096 bytes, sha256 `db3d5add…e7cd`, contains the text "Version 1.5", and has no `_skipped.face`.
- `rel/isimpa-1.4.0_windows-no-install-x64.zip`, 64,758,950 bytes. `noinstall/Release/tetgen.exe` is 519,680 bytes, sha256 `a70854ac…d73d`, contains "Version 1.6" and `_skipped.face`.
- Our pinned build: sha256 `c1c754f1…1f61`.
- Build scripts are `build-1.5.x/build.bat`: VS2022 `cl` with `/O2`, and `/Od` for `predicates.cxx`. Each build took about 3 s.

**F7. Checks on the GUI side.**
- `-Y` is the stored default, not hardcoded: `e_core_core_tetconf.h:101-104` sets `minratio` to 5 and `appendparams` to `"-Y"`.
- `-Y` is removed only when the `isareaconstraint` box is toggled (`Modified()`, 84-90).
- The command is built at `projet_maillage.cpp:163-171`: `-a<maxvol>` if `ismaxvol`, then `-pq<minratio> -A -n <appendparams>`. The `.var` is written at 217-224.
- This logic is unchanged between v1.3.4 and 929a5c8; the diff shows only i18n, path and licence-header changes. So the flag design that relies on `.var` survived the move to a TetGen that ignores it.
- 1.5.0's own behaviour explains why `-Y` is dropped: with `-Y`, only 8 floor faces at 7.5 m² (see F6), because the floor boundary segments are shared with walls that have no bound [inferred from `checkseg4split`/`splitsegment` 24284-24305].

## UNKNOWN

- **U1. Why the 2019 command line does not match the saved settings.** Both report snapshots from 2019-06-07 record `ismaxvol=1`, `maxvol=0.1`, `isareaconstraint=0`, `appendparams=""`, `minratio=2` (`appversion="1.3.4"`). Yet the TetGen footer shows no `-a0.1`, and a `.var` with 0.1 exists. [inferred] The mesh was made under earlier settings (`isareaconstraint=1`, constraint 0.1) and not remeshed afterwards. The footer path also says `instance1` while the zip says `instance2`.
- **U2. Why 1.5.1 only partly honours the bound** (maximum 0.375 m² against a 0.1 bound). The cause is not traced.
- **U3. What current upstream actually runs when it remeshes `tutorial_1.proj`.** [inferred] Saved values override the `InitProperties` defaults, giving `-a0.1 -pq2 -A -n` with no `.var`, which is 3,314 tets with 1.6.0.
- **U4. Whether the 2019 `tetgen.exe` was built from the vendored source.** The byte-identical output makes 1.5.0 near-certain, but I did not check its build provenance.

## What would settle each

- **U1:** read `instance2/projet_config.xml` history or the `.finfo` files, or reproduce in the 1.3.4 GUI by toggling `isareaconstraint` and reading the "Meshing with the following parameters" log line.
- **U2:** run 1.5.1 with `-V` on the same inputs and diff `splitsubface` rejections against 1.5.0; start with the `smarktest3ed` and `useinsertradius` paths at 25650-25700.
- **U3:** run the 1.4.0 release GUI on `tutorial_1.proj`, remesh, and capture the logged command.
- **U4:** unpack the 1.3.2/1.3.3 installers from 2016-2019 and compare their `tetgen.exe` output hashes with the table above.