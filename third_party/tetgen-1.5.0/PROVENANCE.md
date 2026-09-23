# TetGen 1.5.0: provenance

This folder is WIAS TetGen 1.5.0 ("Version 1.5, November 4, 2013"), unmodified. It is the
TetGen that upstream I-Simpa vendored in 2016 and shipped in its releases 1.3.3 and 1.3.4. Our
`tetgen.exe` is built from it (`solvers/build.ps1`, `solvers/tetgen/CMakeLists.txt`).

## Source

- **Tarball:** `tetgen1.5.0.tar.gz`, from `https://wias-berlin.de/software/tetgen/1.5/src/tetgen1.5.0.tar.gz`
  (fetched 2026-09-23, HTTP 200).
  - sha256 `4d114861d5ef2063afd06ef38885ec46822e90e7b4ea38c864f76493451f9cf3`
  - 272,513 bytes
  - 8 members, all dated 2013-11-06.
- **Extracted with** `tar -xzf tetgen1.5.0.tar.gz --strip-components=1`. Nothing was changed.
  `SHA256SUMS` lists every member's sha256. `solvers/build.ps1` refuses to build if any file
  differs from it, and so does `tools/gates/m1.ps1`.
- **Added by us, not from WIAS:**
  - this file
  - `SHA256SUMS`
  - `.gitattributes` (`* -text`), so that git never rewrites line endings. The sources are
    LF, except `example.poly`, which is CRLF in the tarball.

## Equal to upstream I-Simpa's vendored copy

Upstream vendored TetGen at `4db335c123eb54986015dbd8192aa8a7e0ec0969` (2016-08-11, nicolas-f,
"Build tetgen from sources"). The copy stayed unchanged until `ef2f7648ce4c` (2021-04-20, wbinek,
"Update tetgen to 1.6"). That commit's parent, `a0d4d4330d8c`, is the commit tagged `v1.3.4`.
Tag `v1.3.3` has the same blobs.

Proof, 2026-09-23, run against a clone of `github.com/Universite-Gustave-Eiffel/I-Simpa`: each
file was exported with `git show 4db335c:src/tetgen/<file>` and compared with this folder.

| file | CR-insensitive `diff` lines | raw bytes | git blob id, ours = `4db335c`'s |
|---|---|---|---|
| `tetgen.cxx` | 0 | identical | `da4ef3d73513af1acad895153b65e5d80f66842b` |
| `tetgen.h` | 0 | identical | `3196e031f5318369c6842296cf58fa45a2af611b` |
| `predicates.cxx` | 0 | identical | `33817d7999cf04ee831bcae75e053a39d9b783cb` |
| `LICENSE` | 0 | identical | `e253c3d965ddda3797c1ce80fedde3c84c54ecd4` |
| `README` | 0 | identical | `bc5cfa04c0e901cb5c5890b9783a781fa31eda1a` |

- Our blob ids come from `git hash-object --no-filters`.
- `tools/gates/m1.ps1` recomputes these five blob ids on every run. It also refuses upstream's
  `src/tetgen` at `929a5c8`, which is TetGen 1.6.0.
- Upstream's copy had its own `CMakeLists.txt` and lacked `makefile` and `example.poly`. Here the
  tarball's `CMakeLists.txt`, `makefile` and `example.poly` are kept as shipped. None of them is
  used by our build.

## Why 1.5.0, and not the pinned 1.6.0 or 1.5.1

Burhan decided this on 2026-09-23 (`target/investigate/DECISIONS.md`, decision 3, then "Our own
build").

- **TetGen 1.6.0 never tests a facet's area bound.** 1.6.0 is the version vendored at the pinned
  `929a5c8`. It reads the `.var` and stores the bound, but `check_subface` tests only the
  radius-edge ratio. So surface receivers are never refined. Upstream's tutorial 1 meshes to
  6 tets with 1.6.0, and its floor receiver is 2 faces of 30 m².
- **1.5.0 enforces the bound** (`checkfac4split`, `tetgen.cxx:24580-24585`). It reproduces
  upstream's 2019 tutorial meshes (see below).
- **1.5.1 is not a substitute.** On tutorial 1 its largest floor face is 0.375 m², against a
  0.1 m² limit.

## Licence

`LICENSE` is the TetGen licence: **AGPL-3.0-or-later, or a paid commercial licence from WIAS.**

- The text is the same as upstream's `src/tetgen/LICENSE`:
  - at `4db335c`: the same blob;
  - at `929a5c8`: 0 lines of CR-insensitive diff. The 1.6.0 tarball ships no LICENSE, so
    upstream kept this one.
- So this change leaves our licence position exactly where upstream's is. A free GPL release is
  fine. A closed release is not, unless we buy the WIAS licence (see `CLAUDE.md`).

## Build

- **What `solvers/tetgen/CMakeLists.txt` builds:** `add_executable(tetgen tetgen.cxx
  predicates.cxx)` under `cmake_minimum_required(VERSION 3.10)`, with no `TETLIBRARY`. That is
  upstream's own `src/tetgen` target at `929a5c8`, with the same CMake policy floor.
- **The command-line check in `solvers/build.ps1`:**
  - The script builds upstream's 1.6.0 target in the same run, as the reference.
  - It reads the cl and link command lines MSBuild recorded for both targets.
  - It refuses the build unless the two sets are equal once paths are removed.
  - Recorded flags: `/O2 /Ob2 /MD /GR /EHsc /W3`, with `_MBCS WIN32 _WINDOWS NDEBUG`.
- **The rebuild check in M1:** `tools/gates/m1.ps1` builds the folder again from scratch. The
  result must equal `target/solvers/bin/tetgen.exe` byte for byte, apart from the link
  timestamps.

## Measured

Built with MSVC 19.44.35226.0, `tetgen.exe` sha256 `9800a02af3ca4297…`, 2026-09-23. Each run
used `-pq2 -A -n`, upstream's tutorial settings. That is the command recorded in the trailer of
every 2019 `.1.*` file.

**Inputs:** each project's `temp/scene_mesh.poly`, plus `.var` where the project has one, taken
from the tutorial `.proj` zips at `929a5c8`.

**Compared against:** the 2019 `.1.*` files in the same zip. The one `# Generated by` line is
excluded.

| tutorial | nodes | tets | `.node` | `.ele` `.face` `.neigh` `.edge` |
|---|---|---|---|---|
| 1 (`.var`: 0.1 m² on the floor) | 732 | 2,257 | identical | identical |
| 2 (Elmia hall, no `.var`) | 41,607 | 161,543 | 1,036 of 41,607 lines differ in text only | identical |
| 3 (no `.var`) | 835 | 3,285 | identical | identical |

- **Tutorial 2's `.node` lines:** every coordinate parses to the same double. The 1,047 differing
  coordinates are exact decimal ties at the 17th significant digit, such as `…945312` against
  `…945313` for the float32 value 0.0977344512939453125. Today's UCRT `printf` rounds a tie to
  even. The 2019 runtime rounded it up.
- **TetGen 1.6.0 on the same inputs:**
  - tutorial 1: 8 nodes, 6 tets
  - tutorial 2: 29,472 nodes, 123,718 tets
  - tutorial 3: 522 nodes, 2,137 tets
  - Every output file differs.
- **Known difference from 1.6.0 on bad input:** 1.5.0 writes no `_skipped.face`. On
  self-intersecting input it exits 3 and prints the pair of facets. 1.6.0 crashes on the raw
  Elmia hall. See `target/investigate/report-confirm150.md`.
