# Original I-Simpa meshing of tutorial 1: synthesis of 6 lenses plus 4 checks of my own

My scratch folder is `B:/repos/I-Simpa_Night_Mode/target/investigate/synthesis/`. One command of mine failed and left `build.bat` and an empty `area.patch` in `target/investigate/`. I moved them (not deleted) to `synthesis/stray/`. Nothing tracked was touched, and cargo was not used.

**Headline.**
- The 6-tet mesh is upstream's own regression. It is not caused by our importer or our flags.
- Upstream's 2019 mesh came from TetGen 1.5.0, which enforces the `.var` area limit.
- Upstream moved to TetGen 1.6.0 in 2021. 1.6.0 stores the limit and never tests it. An upstream contributor reported this 3 days after the merge, and it was never reverted.
- New in this pass:
  1. Putting the area test back into 1.6 is **not enough on its own**. The mesh stays at 6 tets.
  2. A 1.4.0 Windows user **most likely does not crash**. The crash we hit comes from the corner order our converter writes.

## 1. What upstream does to mesh tutorial 1 today (v1.4.0, SPPS core)
1. **Remeshes on every run.** `projet.cpp:751` calls `RunCoreMaillage` and `projet_maillage.cpp:157` clears the folder, so the 2019 mesh inside the `.proj` is never reused. CONFIRMED (solver-effect).
2. **Uses the saved settings, not the new-project defaults.** A saved project loads through the XML constructor (`e_core_core_tetconf.h:45-53`), which never calls `InitProperties`.
   - SPPS block, `projet_config.xml:2479-2489`: minratio=2, appendparams="", isareaconstraint=1, constraintrecepteurss=0.1, ismaxvol=0, preprocess=1, debugmode=0.
   - The TCR block (`:2555-2565`) is identical.
   - CONFIRMED: I re-read both blocks.
3. **Writes `scene_mesh.poly`:** 8 nodes and 12 triangle facets, marker = face index. The floor is markers 0 and 1: two right triangles of 30 m² each.
4. **Runs `preprocess.exe <poly>`.** It reports "Vertices merged : 0 / Coplanar Faces destroyed : 0 / Face splitted : 0" and leaves the file byte-identical, with the shipped 1.3.4 and 1.4.0 binaries alike. CONFIRMED (release-binary F7, pipeline 5).
5. **Writes `scene_mesh.var`:** rows `1 0 0.1` and `2 1 0.1`, i.e. markers 0 and 1 limited to 0.1 m². CONFIRMED.
6. **Runs `tetgen.exe -pq2 -A -n "<poly>"`.** The command is built at `projet_maillage.cpp:163-171`: `-a<maxvol>` only if ismaxvol, then `-pq<minratio> -A -n <appendparams>`. There is no `-Y` and no `-a`.
7. **Result with the shipped 1.4.0 `tetgen.exe`** (sha `a70854ac…`, banner "Version 1.6"): 8 nodes, 6 tets, 2 floor faces of 30 m². The log says "Opening scene_mesh.var." CONFIRMED by 3 lenses.
8. **Converts to `tetramesh.mbin`**, with each tet's corners reversed to (d,c,b,a) and coordinates passed through float32. CONFIRMED on the 2019 files: pipeline reproduced the fixture byte for byte (sha `8a6b3943…`, 234,492 B). For the 1.4.0 binary this is [inferred]; see §3.
- **New project** (defaults since `bda1bb0`, 2019-06-24): `-pq5 -A -n -Y` with no `.var`. This gives 6 tets on 1.4.0 and on 1.3.4 alike. CONFIRMED.
- **Built from source at 929a5c8:** the vendored TetGen is official WIAS 1.6.0 (0 diff lines after stripping CR). Our build (`c1c754f1…`) matches the shipped exe in all 7 flag combinations both were run on. CONFIRMED (archaeology F1, release-binary F3).

## 2. Does upstream's TetGen honour the `.var`, and since when not?
- **No.** It stopped at `ef2f764` "Update tetgen to 1.6" (wbinek, PR #275, 2021-04-20). That commit went to `dev` and reached `main` via `d32e88d` on 2024-09-26.
- **First release that ships it:** v1.4.0 preview, published 2026-01-09, binaries linked 2026-01-14.
- **Before that:** `4db335c` (2016-08-11) vendored 1.5.0, with 0 diff lines against WIAS 1.5.0. v1.3.3 and v1.3.4 ship "Version 1.5 / November 4, 2013". CONFIRMED (history 1, archaeology F2, release-binary F2).
- **Upstream was told.** wbinek, 2021-04-23, PR #275 comment 825675074: "Tetgen 1.6 does not respect the face area constrain properly. This leads to problems making maps on model surfaces… I would recommend reverting". There was no reply and no revert. The v1.4.0 notes say only "Tetgen update by @wbinek in #275". CONFIRMED.
- **Not an upstream edit.** Official WIAS 1.6.0 has the same code (tarball sha256 `87b5e61e…ce39`). No TetGen release exists after 1.6.0. CONFIRMED.
- **The code difference:**
  - 1.5.0 `checkfac4split` (`tetgen.cxx:24580-24585`) splits a face when area > areabound.
  - 1.6.0 has no `checkfac4split`. `areabound(` is only copied (lines 12362, 12607, 12727, …), and the three reads are commented out. `check_subface` (27347-27388) tests only radius/emin. CONFIRMED.
- **NEW: putting the test back is necessary but not sufficient** (`synthesis/arms*.json`).
  - I added a 10-line area test to `check_subface` (`tetgen16_area/area.patch`). I built the patched and unpatched official 1.6.0 with identical flags.
  - **On upstream's `.poly`:** both builds give 6 tets and 2 floor faces of 30 m², at `-pq2` and at `-pq5`, with the `.var`. The `-VVVV` log says "Tried to split 2 subfaces, 2 were rejected."
    - [inferred] Each box face is two right triangles. A right triangle's circumcentre lies on its long edge, which is a segment between two facets. `split_subface` (27712 ff.) rejects a point that lands on a segment (ENCSEGMENT → FENSEDIN), where 1.5 splits the segment instead.
    - This also explains release-binary F4: 1.6 adds 0 points even at `-q1.414`.
  - **Same box, each face written as one quad facet** (`merged_in/`):
    - patched: 704 nodes, 2130 tets, 942 floor faces, max 0.1117 m² (11 of 942 faces are over 0.1);
    - unpatched, and our pinned exe: 9 tets, 3 floor faces of up to 30 m². CONFIRMED.
- **1.5.1 is not a fix:** 2110 tets, 944 floor faces, max 0.375 m² against the 0.1 limit (archaeology F6). Only 1.5.0 reproduces 2019.

## 3. What a 1.4.0 user running tutorial 1 gets (Windows)
- **Floor map:** 2 faces of 30 m², so a flat map. The 2019 mesh's floor level falls from −55.79 dB (under 1 m from the source) to −61.02 dB (5 m and beyond), Global band. The 6-tet mesh gives −59.25 and −58.97 dB. That is one unseeded run per mesh (solver-effect 7). CONFIRMED.
- **Point receivers and T20 are unaffected.** With the same seed and the source at (3.05, 5.1, 1.8), levels differ by at most 0.028 dB in any band. T20 at 1 kHz is 0.766 s on both, within 0.0005 s. CONFIRMED.
- **TCR** runs: TR_Sabine 0.58057, within 4.1e-7 of the 2019-mesh run. Its maps also have 2 faces. CONFIRMED.
- **Crash: most likely no.** The source (3, 5, 1.8) lies on the facet shared by tets 0 and 4. I ran the shipped 1.4.0 `spps.exe` (`9bc7db4c…`) on the fixture config:
  - GUI corner order (`e456a8b4…`): exits `0x00000000` and writes results (`synthesis/runs/v140_gui_order`). CONFIRMED.
  - TetGen corner order (`8b4439c3…`): exits `0xC0000005` in 0.0 s, stdout only "SPPS version 2.2.1". CONFIRMED.
  - Which order the 1.4.0 GUI writes:
    - The loader builds `ivec4 sommets(ToInt(GetNextToken())-1, …×4)` (`Objet3D_maillage.cpp:160-163`). C++ leaves the evaluation order of these arguments unspecified.
    - MSVC 19.44 evaluates them right to left, at `/O2` and at `/Od`: input 11 22 33 44 gives a=43 b=32 c=21 d=10 (`synthesis/evalorder/`). CONFIRMED.
    - The shipped `isimpa.exe` was linked by MSVC linker 14.44. CONFIRMED.
    - That the shipped GUI actually writes that order is [inferred]; I did not run the GUI.
- **Verdict:** point results are correct, surface maps are flat, and there is no crash [inferred, one step]. The survival margin is only 2.4e-7 in a float32 test (pipeline 7).
- **Our crash:** our builder writes TetGen's order (`build.rs:117`, "corners as written"). Our gate mesh `target/gates/probe-box/tetramesh.mbin` (`28a27918…`) fails the float32 test: +2.384e-7 in tets 0 and 4, so the source is in no tet (`locate.py`). CONFIRMED.

## 4. How the 2019 mesh (2,257 tets, 934 floor faces) was made
- TetGen 1.5.0 ran `meshing\tetgen\tetgen.exe -pq2 -A -n D:\picaut\…\scene_mesh.poly`, with the `.var` limiting markers 0 and 1 to 0.1 m². The command is in the trailer of all 5 `.1.*` files, dated 2019-06-07 11:58:42. CONFIRMED.
- These were upstream's defaults at the time (minratio 2, appendparams ""), before `bda1bb0`.
- Four lenses each reproduced it: rebuilt WIAS 1.5.0 / `4db335c`, and the shipped 1.3.3 and 1.3.4 exes. Each gives 732 nodes, 2257 tets, 934 floor faces, max 0.09976 m², and all 5 output files are byte-identical apart from the trailer. CONFIRMED.
- Tutorial 3 is also byte-identical. Tutorial 2 differs only in the 17th digit on 1,036 node lines (C runtime printf rounding).
- Only the floor is fine: walls reach 0.85–1.98 m² and the ceiling 2.33–3.40 m². Tets grade from a median 0.016 m³ near the floor to 0.50 m³ at 2.5–3 m. The largest tet is 1.75 m³, so no `-a` was used (forensics 3).

## 5. Options for the rebuild
- **A. Patch TetGen 1.6.**
  - Measured: the 10-line test alone does nothing on I-Simpa-style triangle facets.
  - It works only with extra work, either:
    - (a) our `.poly` writer merges each surface's coplanar triangles into one polygon facet (942 floor faces, max 0.1117 m², 2130 tets), and the `.mbin` builder maps each output face back to its source triangle for SPPS markers; or
    - (b) a larger patch that splits the segment when the circumcentre lands on it. Not written, not measured.
  - Parity with 2019: none. Risk: we own a change inside TetGen's refinement.
- **B. Pre-split receiver faces in our `.poly`.** Nobody measured this.
  - [inferred] With stock 1.6 and `-Y`, the receiver faces would be exactly our triangles, so under the limit by construction.
  - Cost: an area-limited 2D triangulator (including holes) in our own code, plus the new edge points written into neighbouring facets. Parity: none.
- **C. TetGen 1.5.0.**
  - Measured: byte-identical to 2019 on tutorials 1 and 3, and identical on 2 apart from the printf digit.
  - Licence: the same AGPL-3-or-later / WIAS paid dual licence (`I-Simpa-upstream/src/tetgen/LICENSE` = official 1.5.0). Nothing changes for a free GPL release.
  - Provenance: WIAS `tetgen1.5.0.tar.gz` (sha256 `4d114861…9cf3`) = upstream `4db335c` = what upstream shipped in 1.3.3 and 1.3.4.
  - Cost: TetGen would no longer come from the pinned tag.
  - Risks:
    - 1.5.0 writes no `_skipped.face` (absent from the 1.3.4 exe), which `crates/simpa-core/src/formats/tetgen.rs:2` reads to report self-intersections (arc item 12).
    - Robustness on real halls is unmeasured.
    - 1.5.1 must not be substituted for it.
- **D. TetGen switches.**
  - `-a0.1` on 1.6: 3314 tets, 259 floor faces, max 0.579 m². It misses the limit and refines the whole volume (measured).
  - `-a`, `-m` and `.mtr` act on tets only (source). `tetunsuitable` is a library callback with no switch.
  - `-i` (extra points from a `.a.node` file on receiver faces) was never measured.
  - Upstream's own advice in #356 (drop `-Y`, lower the volume limit) amounts to `-a`, which does not give the limit.
- **Needed whichever option is chosen:** write corners (d,c,b,a) with the float32 round trip, as upstream does (this reproduced the fixture `.mbin` byte for byte). That keeps SPPS alive on tutorial 1, but only by 2.4e-7. The location and NULL-dereference bug (`coreinitialisation.cpp:71-95`, `sppsInitialisation.cpp:20`) needs its own patch, tested against a reference like any solver fix.

**Recommendation: C.** It is the only option measured to reproduce upstream's documented behaviour (the three 2019 tutorials), and it adds no code inside TetGen and no licence change. It is an architecture choice, so it is your call.

**Single confirming measurement:** mesh `testdata/elmia_corrected.ply` with 1.5.0, through our `.poly`/`.var` writer with a surface receiver declared. It passes if:
- it exits 0 with no self-intersection stop;
- every receiver face is within its limit;
- SPPS's count of particles lost to meshing problems is no higher than on the 1.6 mesh.

If it fails, A(a) is the measured fallback, at 0.1117 m² max.

## Contradictions between reports, resolved from receipts
1. **Saved settings.** Archaeology (U1, U3) and release-binary (F8) read the first `mesh_conf` block (`:2423`: ismaxvol=1/0.1, area 0/5). That block belongs to MD_Octave: its Properties hold "Diffusion equation resolution - Tolerance". SPPS and TCR are 1/0.1 with ismaxvol=0 (re-read). So archaeology's "`-a0.1`, 3314 tets" is wrong for SPPS.
2. **Crash.** Solver-effect's "1.4.0 user crashes" used the TetGen-order `.mbin` (`8b4439c3…`, the same as pipeline's file-order mesh), which the GUI does not write. My shipped-exe runs above settle it.
3. **Cause of the 6 tets.** The premise "`check_subface` lacks the area test" (4 lenses) and release-binary F4 ("1.6's radius-edge check is inert too") are both true. My test shows the missing area test is not the only obstacle.
4. **First release with 1.6.** The `v1.3.5_snapshot_2026_01_09` tag contains 1.6, but there is no release object between v1.3.4 and v1.4.0. Both reports hold.
5. **Default flags.** `-pq5 -A -n -Y` applies only to new projects since 2019-06-24. The tutorial keeps `-pq2`.

## UNKNOWN, and what would settle each
- **U1. What the 1.4.0 GUI actually writes and shows.** Settle by running the shipped `isimpa.exe` on `tutorial_1.proj`: expect `tetramesh.mbin` sha `e456a8b4…` and no crash, and capture the console.
- **U2. Linux and macOS builds (GCC, Clang):** corner order and crash. Settle with the `.mbin` written by the flatpak build.
- **U3. 1.5.0 on complex or self-intersecting input:** robustness and what it reports. Settle with the elmia run above, plus raw `elmia.ply` with `-d`.
- **U4. Which 1.6 rejection branch fires** (ENCSEGMENT or SHARPCORNER). Settle by printing `iloc` in the patched `split_subface`.
- **U5. Why 11 of 942 faces exceed 0.1 m² under A(a).** [inferred] Flips after the face phase. Settle by rechecking face areas after tet refinement.
- **U6. Whether WIAS ever answered wbinek.** Ask wbinek or nicolas-f.
- **U7. Which build made the 2019 mesh.** Its project says `appversion="1.3.4"` 18 months before the 1.3.4 release, so [inferred] a dev build.

Side note: the read-only `B:/repos/I-Simpa-upstream` has untracked `build_solvers/` and `session-logs/` in it (history 7).