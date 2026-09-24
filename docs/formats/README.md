# Solver file formats

One page per file the solvers or TetGen read or write, and two for files of our own.

| Page | File | Canonical dump |
|---|---|---|
| `cbin.md` | the scene mesh, `mesh.cbin` | yes |
| `mbin.md` | the tetrahedral mesh, `tetramesh.mbin` | yes |
| `poly.md` | TetGen's input, `scene_mesh.poly` | yes |
| `tetgen.md` | TetGen's output, `.1.{node,ele,face,neigh}` and `_skipped.{node,face}` | yes |
| `var.md` | TetGen's facet-area constraints, `scene_mesh.var` | no: we write it and TetGen reads it |
| `gabe.md` | result tables, `.gabe` and its relatives | yes |
| `csbin.md` | surface-receiver results, `.csbin` | yes |
| `pbin.md` | particle files, `.pbin` | yes |
| `config_xml.md` | the solver configuration, `config.xml` | no: `docs/solver-contract.md` holds its rules |
| `mesh-manifest.md` | `mesh.json`, the mesh folder, and the mesher's and mesh verifier's reason codes | no: our own format |
| `results-json.md` | `simpa results --json`, a run's results and parameters for the Results screen, with its schema `results-json.schema.json` | no: our own format |

Each binary format's page gives:
- the byte layout
- sizes, sentinels and version fields
- failure modes
- receipts into upstream's `lib_interface`, at `target/solvers/src-929a5c8/src/lib_interface`
- the format's **canonical dump**

The project is GPL-3 and non-commercial (decided 2026-09-23), so upstream's source may be read
and ported directly.

## Canonical dump

The canonical dump is the text both the Rust reader (`crates/simpa-core/src/formats/<fmt>.rs`)
and the C++ oracle (`oracle/dump_<fmt>.cpp`, built on upstream's own readers) produce for the
same file. The gate diffs the two. Rules:

- **Layout.** One record per line, tokens separated by single spaces, `\n` line ends, no trailing
  space.
- **First line:** the format name, then its version tokens, e.g. `cbin 1 0`.
- **Lists:** a count line `<list> <n>` comes before each list's records, and records appear in
  file order.
- **Integers** are written in decimal.
- **Floats** are written as their raw bits: `f32` as 8 lowercase hex digits and `f64` as 16
  (`f32_hex` / `f64_hex`). This keeps NaN, -0.0 and the last bit exact, with no printf
  rounding.
- **Strings** go through `str_token`: printable ASCII other than `\` is kept, and every other
  byte (space included) is written `\xHH`. An empty string is `\x`.
- **What gets dumped** is what upstream's reader exposes, in its own field order. Padding and
  garbage are never dumped. `.csbin` struct padding is uninitialised memory and differs between
  two runs of the same solver (verified 2026-09-23).
- **Failure.** When upstream's reader fails, the oracle prints `error` and exits 1. Rust prints
  `error <kind>`, where kind is `notfound`, `truncated`, `version`, `invalid` or `io`. The diff
  counts "both failed" as agreement. The negative tests check the kind.

Build one format's oracle with `powershell -File oracle/build.ps1 -Only <fmt>`, and run it with
`target\oracle\<fmt>\oracle.exe dump <fmt> <file>`. `-UpstreamSrc <tree>\src` reads upstream from
another tree at the same commit. The tests build the oracle they need on demand, from
`$SIMPA_UPSTREAM` or the M1 archive (`crates/simpa-core/tests/common/paths.rs`), and fail when it
cannot be built: no cross-check is skipped.
