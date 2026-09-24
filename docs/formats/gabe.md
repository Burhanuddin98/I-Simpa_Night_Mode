# `.gabe`: GABE v2 tables

Upstream name: `formatGABE::GABE` ("Generic Array Binary Exchange"). A GABE file is a list of
typed columns. Each column holds floats, integers or 50-byte strings. The first column usually
labels the rows. The Rust side reads only. Reader: `crates/simpa-core/src/formats/gabe.rs`.
Oracle: `oracle/dump_gabe.cpp`.

Receipts are paths under `target/solvers/src-929a5c8/src/`. `gabe.h` and `gabe.cpp` mean
`lib_interface/input_output/gabe/gabe.{h,cpp}`.

## Which files are GABE

The extension says what the table holds. The layout is the same in every case.

| File | Writer | What it holds |
|---|---|---|
| SPPS statistics `.gabe` | `spps/input_output/reportmanager.cpp:503-517` | Particle fates by band. Integer columns. Row labels in column 0 |
| SPPS sound level over time `.gabe` | `spps/input_output/reportmanager.cpp:538` | Float columns, rows labelled `"<n> ms"` |
| `.recp` (punctual receiver) | `lib_interface/input_output/baseReportManager.cpp:190`, `spps/input_output/reportmanager.cpp:614,704,871` | Float column per band, row labels `"<t> ms"` |
| `.recps` | `spps/sppsNantes.cpp:406` (`Sound level per source.recps`) | Each source's energy at the receiver per band, the time series summed, in the `.recp`'s unit (Pa², not a level in dB; `docs/results.md`, "What is read"). One row per source |
| `.gap` | GUI name `Advanced sound level.gap` (`isimpa/data_manager/tree_core/e_core_core.h:108`) | Advanced receiver parameters |
| `.rpi` | `spps/input_output/reportmanager.cpp:755` (`Intensity.rpi`) | Intensity |
| TCR `Main results.gabe` | `ctr/main_tc.cpp:137`, `ctr/input_output/reportmanager.cpp:154-216` | Float column per quantity (`A_Sabine`, `TR_Sabine`, `L_Sabine` and the Eyring ones). One row per band, then a `Global` row |
| TCR `rs*.gabe`, `rscut*.gabe`, `rp.gabe` | `ctr/input_output/reportmanager.cpp:51,84,104` | Surface and punctual receiver values |

## Layout

Little-endian throughout. The format is version 2 (`GABE_VERSION`, `gabe.cpp:43`). The current
writer (`GABE::Save`, `gabe.cpp:455-515`) writes each field on its own and uses `seekp` over the
padding. Older writers dumped whole structs, so their padding holds garbage. Upstream's reader
(`GABE::Load`, `gabe.cpp:353-438`) reads each field on its own and uses `seekg` over the same
padding, so both kinds of file read the same way.

```
file header                       20 bytes
for each of `cols` columns:
  column header                   280 bytes
  column payload                  depends on the type:
    float  (50)                   i32 numOfDigits, then f32 × rows
    int    (51)                   1 padding byte, then i32 × rows
    string (52)                   1 padding byte, then 50-byte cell × rows
    any other type                sizeofCol bytes, skipped
```

### File header, `t_FileHeader` (`gabe.cpp:49-57`, read at `gabe.cpp:371-378`)

| Offset | Type | Field | Meaning |
|---:|---|---|---|
| 0 | i32 | `formatVersion` | 2 |
| 4 | i32 | `t_FileHeader_Length` | Not used on read. The current writer writes 0 (`gabe.cpp:474`). The old struct writer wrote 20 |
| 8 | i32 | `t_ColHeader_Length` | Not used on read. The current writer writes 0 (`gabe.cpp:475`). The old struct writer wrote 280 |
| 12 | i32 | `cols` | Number of columns written. Only non-null columns count (`gabe.cpp:470-473`) |
| 16 | u8 | `readOnly` | 0 or 1. Tells I-Simpa's GUI whether the user may edit the table |
| 17 | 3 bytes | padding | Skipped. Garbage in old files (`retrocompat_test.gabe` has `9b 48 00`) |

### Column header, `t_ColHeader` (`gabe.cpp:59-70`, read at `gabe.cpp:393-401`)

| Offset | Type | Field | Meaning |
|---:|---|---|---|
| 0 | u16 | `colType` | `GABE_OBJECTTYPE` (`gabe.h:50-56`): 50 float, 51 int, 52 short string. 0-49 are "user defined" |
| 2 | 2 bytes | padding | Skipped |
| 4 | i32 | `rows` | Row count. For the three known types, this alone decides the payload length |
| 8 | i64 | `sizeofCol` | `sizeof(Td) × rows` (`gabe.h:183`). Used on read only to skip unknown types |
| 16 | i64 | `sizeofHeader` | `sizeof(Ts)` (`gabe.h:184`): 4 for float (`t_HeaderFloat`), 1 for the others (the empty `t_NoStruct`, `gabe.h:130-133`). Never used on read |
| 24 | 255 bytes | `label` | Column label, NUL-padded (`STRING_LABEL_LENGTH`, `gabe.h:58`) |
| 279 | 1 byte | padding | Skipped |

### Payloads

- **Float, 50** (`gabe.cpp:136-142`). An `i32 numOfDigits` (`t_HeaderFloat`, `gabe.h:204-208`)
  comes first. It is the display precision: the default is 12 (`stdgabe.h:87`), and TCR writes
  its own `COMMA_PRECISION_*` values (`ctr/input_output/reportmanager.cpp:145,190-199`). Then
  `rows` `f32` values follow.
- **Int, 51** (`gabe.cpp:203-207`). One padding byte (`sizeof(t_NoStruct)`), then `rows` `i32`
  values. The byte is garbage in old files: it is `06` in `retrocompat_test.gabe`.
- **Short string, 52** (`gabe.cpp:185-189`). One padding byte, then `rows` cells of 50 bytes each
  (`t_StringShort`, `gabe.h:230-233`). A cell holds a NUL-terminated string. Bytes after the NUL
  are padding.
- **Any other type** (`gabe.cpp:426-434`). The reader seeks `sizeofCol` bytes past the column
  header and keeps nothing. It does not skip `sizeofHeader`.

### Text

Labels and cells are raw bytes with no declared encoding. Old files are Latin-1 or cp1252
(`absorb\xe9es`). Current TCR labels are UTF-8 (`A_Sabine\rm\xc2\xb2`). Solvers join a
quantity and its unit with `\r`, as in `TR_Sabine\rs`. The reader keeps labels and cells as bytes.
`Column::name()` and `Column::unit()` split them at the `\r`.

### Sizes and sentinels

- The file header is 20 bytes and each column header is 280. A non-empty table is therefore at
  least 20 + 280 × cols bytes long, apart from the trailing-seek case below.
- **Trailing seeks are never written.** `Save` skips padding with `seekp` (`gabe.cpp:485,501,507`,
  and in `WriteFile` at `gabe.cpp:181,209`). A seek that is the last operation on the file adds
  no bytes. A table with no columns is 17 bytes long. A table whose last column is an empty int
  or string column ends 2 bytes early. Upstream's own reader accepts both.
- **TCR's `Global` row is NaN by design** for non-energetic quantities: `A_*` and `TR_*` get
  `NAN` (`ctr/input_output/reportmanager.cpp:208`), and `L_*` gets the energetic sum
  (`:206`). In the fixtures this NaN is the quiet NaN `7fc00000`.
- **Strings of 50 bytes or more overflow upstream's writer.** `SetString` uses `strcpy` into a
  50-byte cell (`gabe.cpp:174-178`). A 50-byte cell with no NUL is read as all 50 bytes. Labels
  are copied with `strncpy` into 255 bytes (`gabe.cpp:88-91`), so a 255-byte label also has no
  NUL.

## Versions

Upstream accepts any `formatVersion <= 2` (`gabe.cpp:381-383`) and reads it with the v2 layout.
The comment at `gabe.cpp:43` says the version must change whenever the `t_*` structures change,
so version 1 and older had a different layout by definition. **This reader accepts only 2.** Every
fixture is version 2, including the file from the old struct writer. Any other version is
`FormatError::Version`, and it is checked before the rest of the header is read. A future
version's header may be shorter, so that case is reported as a version error, not a truncation.

## Failure modes

`GABE::Load` checks only two things: that the file opened (`gabe.cpp:367-368`) and the version.
It never checks a read, and an unknown column type is the only place it checks the stream state
(`gabe.cpp:429`). `t_ColHeader colHeader` is declared inside the loop, and its constructor
initialises only the label (`gabe.cpp:66-69`). After a short read, the next column therefore
reuses the previous column's type and row count from the same stack slot.

| Input | Upstream `Load` | This reader |
|---|---|---|
| Missing file | `false` | `NotFound` |
| `formatVersion` > 2 | `false` | `Version` |
| `formatVersion` <= 1 | Reads it with the v2 layout | `Version` (deliberate) |
| `readOnly` byte other than 0 or 1 | Copied into a `bool` (UB) | `Invalid` (deliberate) |
| `cols` < 0 | `reserve(huge)` throws `std::length_error`, which is uncaught | `Invalid` |
| `cols` more than the bytes can hold | `reserve` of up to 2³¹ pointers | `Truncated`, before anything is reserved |
| `rows` < 0 on a known type | `new Td[negative]` throws, uncaught (`gabe.h:145`) | `Invalid` |
| `rows` more than the bytes can hold | Allocates `rows × sizeof(Td)`, reads short, returns `true` | `Truncated`, before anything is reserved |
| `sizeofCol` < 0 on an unknown type | Seeks backwards | `Invalid` |
| **Truncated anywhere** | **Usually `true`.** A short column keeps zeros, and later columns are invented from the stale header (see below) | `Truncated` |
| Padding missing only at end of file | `true` | Accepted, the same as upstream (writer quirk above) |
| Bytes after the last column | Ignored | Ignored. `read_prefix` reports how many bytes were used |
| `sizeofCol`/`sizeofHeader` wrong on a known type | Ignored | Ignored |

**Evidence for the truncation row** (oracle build of 2026-09-23 on
`nightmode-2026-09-08/spps_stats.gabe`):
- Cut to 2,504 of 2,505 bytes, the file dumps identically to the whole file. The missing byte was
  the high byte of the last `100000`, and the zero-filled buffer happened to supply it.
- Cut to 1,000 bytes, inside the third column's header, it loads "successfully" with all 7
  columns.
  - The `250 Hz` column comes back as seven zeros. Its real values include 54 and 99944.
  - The last four columns are `column int \x` with 7 rows of zeros each, invented from the stale
    header.
- Cut to 17 bytes, it returns `false`. At that point the column header is uninitialised, so which
  path it takes is undefined behaviour, not a check.

## Canonical dump

What upstream's `GABE` object exposes after `Load`, in its order:
- `IsReadOnly()` and `GetCols()`
- for each column: its type, `GetLabel()` and `GetSize()`
- `headerData.numOfDigits` for float columns
- every `GetValue(i)`

`GABE` keeps no copy of the version, so the oracle reads the first 4 bytes of the file itself.
Unknown-type columns are not kept by upstream and are not dumped. Padding, the two header lengths,
`sizeofCol` and `sizeofHeader` are not exposed and are not dumped.

```
gabe <formatVersion>
readonly <0|1>
columns <n>
then per column, in file order, one of:
  column float <label>
  digits <numOfDigits>
  rows <r>
  <f32_hex>                 × r
or
  column int <label>
  rows <r>
  <decimal i32>             × r
or
  column shortstring <label>
  rows <r>
  <str_token of the cell>   × r
```

- `<label>` is `str_token` of the label bytes up to the first NUL, at most 255 bytes. An empty
  label is `\x`.
- A cell is its bytes up to the first NUL, at most 50.
- The oracle uses `strnlen` with the same limits, because upstream's fields need not end in a NUL.
- On failure the Rust side prints `error <kind>` and the oracle prints `error`.

The start of `nightmode-2026-09-08/tcr_main_results.gabe`:

```
gabe 2
readonly 1
columns 7
column shortstring TC
rows 7
125\x20Hz
...
Global
column float A_Sabine\x0dm\xc2\xb2
digits 2
rows 7
43c9ca8b
...
7fc00000
```

## API

- `read(bytes)` and `read_file(path)` return a `Gabe`: `version`, `read_only`, and `columns`, a
  list of `Column { label, data }`. `data` is one of `Float { num_of_digits, values }`,
  `Int(values)` or `ShortString(cells)`.
- `read_prefix(bytes)` also returns how many bytes the table used.
- `dump`, `dump_file`.
- Helpers:
  - `Gabe::column(name)` matches the label before its `\r`.
  - `Gabe::row_labels()` returns column 0 when it is a string column.
  - `Gabe::row_index(label)`.
  - `Gabe::band_series(name)` returns the band rows, with the `Global` row kept apart.

## Verification (2026-09-23)

- **Golden tests** (`tests/gabe_golden.rs`, 16 tests):
  - Every fixture uses all of its bytes: `nightmode-2026-09-08/spps_stats.gabe` uses 2,505 of
    2,505.
  - That table's lost-by-meshing row reads 38, 54, 30, 42, 50, 48.
  - TCR `TR_Sabine` reads 5.591, 2.698, 1.645, 1.298, 1.252, 1.126, and its `Global` is NaN.
  - The upstream retrocompat file gives the values upstream's own Python test expects.
  - Every proper prefix of every fixture is `Truncated`, and version 3 is `Version`.
- **Oracle agreement:**
  - 7 of 7 fixtures agree byte for byte.
  - So do 400 synthetic valid tables from a throwaway generator under `target/agents/gabe/` (not
    committed). They cover unknown column types, NaN payloads, infinities, -0 and subnormals,
    255-byte labels and 50-byte cells with no NUL, garbage padding, and missing trailing padding.
- **Fuzz** (`tests/gabe_fuzz.rs`): 4 proptests of 10,000 cases each, over arbitrary bytes, bytes
  behind a v2 header, mutated fixtures and structured random tables. There were no panics, and
  every case's peak allocation stayed within `budget(len)`.
