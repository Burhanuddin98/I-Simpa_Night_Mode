# `.csbin`: surface receivers and cutting planes

Upstream name: `formatRSBIN` ("RSBIN"). SPPS writes one per band and one for all bands, for the
surface receivers (`Sound level.csbin`) and for the cutting planes (`rs_cut.csbin`). The
wxWidgets GUI also writes derived maps (`TR.csbin`, `EDT.csbin`, C50, STI and others) with the
same writer. The Rust side reads only. Reader: `crates/simpa-core/src/formats/csbin.rs`. Oracle:
`oracle/dump_csbin.cpp`.

Receipts are paths under `target/solvers/src-929a5c8/src/`. `rsbin.h` and `rsbin.cpp` mean
`lib_interface/input_output/exportRecepteurSurf/rsbin.{h,cpp}`.

## Layout

Little-endian throughout. The file is whole C structs written with `fstream::write` of `sizeof`
bytes (`rsbin.cpp:181-199`), so their padding goes to disk as is.

```
header                    44 bytes, t_FileHeader        (stored length: lengths.header)
node      × quantNodes    12 bytes, t_nodesPosition     (stored length: lengths.node)
for each of quantRS receivers:
  receiver                264 bytes, t_RecepteurS       (stored length: lengths.receiver)
  for each of its quantFaces faces:
    face                  16 bytes, t_FaceRS            (stored length: lengths.face)
    value × nbRecords     8 bytes, t_faceValue          (stored length: lengths.value)
```

The records nest. A receiver's faces follow it directly, and each face's values follow that face.
No offsets or totals are stored, so the file can only be read in order.

### Header, `t_FileHeader` (`rsbin.cpp:39-52`)

| Offset | Type | Upstream field | Meaning |
|---:|---|---|---|
| 0 | i32 | `formatVersion` | Must be 3 (`VERSION`, `rsbin.h:52`) |
| 4 | u32 | `t_FileHeader_Length` | 44 |
| 8 | u32 | `t_nodesPosition_Length` | 12 |
| 12 | u32 | `t_RecepteurS_Length` | 264 |
| 16 | u32 | `t_FaceRS_Length` | 16 |
| 20 | u32 | `t_faceValue_Length` | 8 |
| 24 | i32 | `quantNodes` | Node count |
| 28 | i32 | `quantRS` | Receiver count |
| 32 | i32 | `nbTimeStep` | Time step count |
| 36 | f32 | `timeStep` | Time step, in seconds |
| 40 | i32 (enum) | `recordType` | `RECEPTEURS_RECORD_TYPE`, 0 to 9 (`rsbin.h:62-74`) |

The header has no padding. Its five lengths are `sizeof` of each struct, filled in by the writer
(`rsbin.cpp:173-177`). Every fixture stores 44, 12, 264, 16 and 8. That is `StructLengths::NATIVE`
in Rust, and it matches `sizeof` in the oracle's MSVC x64 build.

`recordType`: 0 `SPL_STANDART`, 1 `SPL_GAIN`, 2 `TR`, 3 `EDT`, 4 `PRESSURE`, 5 `CLARITY`,
6 `DEFINITION`, 7 `TS`, 8 `ST`, 9 `STI`.

### Node, `t_nodesPosition` (`rsbin.h:81-84`)

| Offset | Type | Field |
|---:|---|---|
| 0 | f32 × 3 | `node[3]`: x, y, z in metres |

### Receiver, `t_RecepteurS` (`rsbin.h:89-94`)

| Offset | Type | Field |
|---:|---|---|
| 0 | i32 | `xmlIndex`: the receiver's id in the project XML |
| 4 | i32 | `quantFaces`: how many faces follow |
| 8 | char × 255 | `recepteurSName` (`STRING_SIZE`, `rsbin.h:53`) |
| 263 | 1 byte | **padding** |

The name is a C string. Writers zero all 255 bytes (`rsbin.h:141`) and then copy at most 254
(`lib_interface/input_output/baseReportManager.cpp:42,106,253`), so it is always NUL-terminated.
Its encoding is the solver's narrow encoding, not necessarily UTF-8. The name in upstream's Python
fixture is cp1252 `R\xe9cepteur coupe` (`python_bindings/tests/check_retrocompat_recsurf.py:23`).

### Face, `t_FaceRS` (`rsbin.h:99-103`)

| Offset | Type | Field |
|---:|---|---|
| 0 | i32 × 3 | `sommetsIndex[3]`: indices into the node array |
| 12 | i32 | `nbRecords`: how many values follow |

Only time steps at which something was recorded are stored (`baseReportManager.cpp:52-69` for
surface receivers, `:270-317` for cutting planes). A face can have 0 records:
`spps_1000hz.csbin` has one.

### Value, `t_faceValue` (`rsbin.h:111-117`)

| Offset | Type | Field |
|---:|---|---|
| 0 | u16 | `timeStep`: index of the time step, below `nbTimeStep` |
| 2 | 2 bytes | **padding** |
| 4 | f32 | `energy`: energy, level or time, depending on `recordType` |

The writers emit a face's values in increasing `timeStep` order. The reader does not require
this. The golden tests check that every fixture keeps it.

### Padding is uninitialised memory

Upstream's writers never clear struct padding. In the fixtures, bytes 2-3 of a `t_faceValue`
are non-zero in 102 of 200 records (lib_interface), 5228 of 60870 (python_bindings) and 904 of
6041 (spps_1000hz). They contain heap garbage such as `09`, `s_`, `le` and `tr`, and two runs of
the same solver differ there (verified 2026-09-23, `README.md`). The `t_RecepteurS` pad byte is
0 in all three fixtures, but nothing guarantees that. **The reader never reads padding**: it
reads each field at its C offset and steps over the rest of the record. The name's bytes after
its NUL are not exposed either.

## Versions and stored struct lengths

`VERSION` is 3. Upstream's `ImportBIN` (`rsbin.cpp:82-149`) sets
`versionConflict = formatVersion != 3` (`:103`). Only on a conflict does it step each record by
its stored length, by seeking `stored - sizeof` past the `sizeof` bytes it just read
(`:112-113, 120-121, 128-129, 135-136, 142-143`). For a version-3 file it ignores the stored
lengths and steps by `sizeof`.

This reader:
- **accepts version 3 only.** Anything else is `FormatError::Version`. Upstream would read a
  foreign version by its stored lengths, filling any field past a shorter struct from the next
  record's bytes. No file with another version is known, and every fixture is version 3.
- **steps every record by its stored length**, header included. It reads the fields at their
  C offsets, so a longer struct (fields appended, which is the scheme `rsbin.h:52` describes)
  is read correctly and its extra bytes are skipped.
- requires each stored length to cover the fields read from it: header ≥ 44, node ≥ 12,
  receiver ≥ 263, face ≥ 16, value ≥ 8. A shorter one is `Invalid`.

On every file upstream writes, the stored lengths equal `sizeof`, and the two readers agree
byte for byte. They differ only on a version-3 file with non-native lengths. No writer produces
one. The oracle refuses such a file (see below).

## Failure modes

| Input | Upstream `RSBIN::ImportBIN` | This reader |
|---|---|---|
| Missing file | Returns **true**. The header is left uninitialised, and the arrays are sized from stack garbage | `NotFound` |
| File ends early, anywhere | Returns true. The `fstream` read fails and the remaining fields are garbage | `Truncated` (every prefix of the fixture is tested) |
| `formatVersion` ≠ 3 | Reads by stored lengths | `Version` |
| Stored length below its fields' extent | Reads anyway | `Invalid` |
| Negative `quantNodes`, `quantRS`, `quantFaces` or `nbRecords` | `new T[negative]` throws or allocates huge | `Invalid` |
| Negative `nbTimeStep` | Stored as is | `Invalid` |
| A count larger than the bytes left | Allocates it and reads past the end | `Truncated`, before anything is reserved |
| `recordType` outside 0 to 9 | Stored as is | `Invalid` |
| A face vertex outside `0..quantNodes` | Stored as is. The GUI indexes the node array with it unchecked (`isimpa/3dengine/Core/Recepteurs_surfacique.cpp:322-326`) | `Invalid` |
| `timeStep` ≥ `nbTimeStep` | Stored as is. The GUI rejects it ("File receiver corrupted", `Recepteurs_surfacique.cpp:344-348`), and the parameter maps index with it unchecked (`isimpa/data_manager/projet_calculation.cpp:1036`) | `Invalid` |
| Bytes after the last record | Ignored | Ignored, as upstream does |

The reader never panics, and it allocates at most `2 * len + 64 KiB` for `len` input bytes. It
makes two passes. The first walks the whole file, checks every count, index and time step, and
allocates nothing. The second allocates exactly what the file holds. A face is 32 bytes in memory
(its vertices and a boxed slice of values) per 16 or more bytes on disk. A value is 8 per 8, a
node 12 per 12, and a receiver 56 plus its name per 263 or more. So no input can exceed twice its
size. `csbin_fuzz.rs` measures this, including a file of 20,000 zero-value faces, the densest case.

## Canonical dump

Rules from `README.md`, applied as follows. Every line is listed in file order:

```
csbin <formatVersion>
lengths <header> <node> <receiver> <face> <value>
timesteps <nbTimeStep> <timeStep:f32>
recordtype <recordType>
nodes <quantNodes>
node <x:f32> <y:f32> <z:f32>                      × quantNodes
receivers <quantRS>
  receiver <xmlIndex> <name:str>                  per receiver, then
  faces <quantFaces>
    face <v0> <v1> <v2>                           per face, then
    records <nbRecords>
      record <timeStep> <energy:f32>              × nbRecords
```

- The indentation above is for reading only. The dump has none.
- Integers are decimal. `f32` is its raw bits as 8 lowercase hex digits (`f32_hex`).
- `name` is `recepteurSName` up to its first NUL, or all 255 bytes when there is none, spelled
  with `str_token`. `Plane Receiver` is written `Plane\x20Receiver`.
- Padding and the name's bytes after the NUL are never dumped.
- Each count line (`nodes`, `receivers`, `faces`, `records`) gives the length of the list that
  follows it.
- On failure the dump is `error <kind>` (Rust) or `error` (oracle).

The start of `upstream/lib_interface/rs_cut.csbin`:

```
csbin 3
lengths 44 12 264 16 8
timesteps 5 3dcccccd
recordtype 0
nodes 30
node 00000000 40800000 3fcccccd
...
receivers 1
receiver 3389 Plane\x20Receiver
faces 40
face 1 0 5
records 5
record 0 338e9944
record 1 30e322b9
...
```

### What the oracle checks, and what it does not

`oracle/dump_csbin.cpp` dumps from the `t_ExchangeData` that upstream's own `ImportBIN` fills.
`ImportBIN` checks nothing, so calling it on a bad file is undefined behaviour. Before calling it,
the oracle walks the file itself using `ImportBIN`'s `sizeof` strides. It prints `error` for a
missing file, a version other than 3, stored lengths that differ from `sizeof`, a negative count,
an unknown record type, or a count the file cannot back. After `ImportBIN` returns, it applies
the vertex and time-step range checks to the parsed data.

`formatVersion` and the five stored lengths are the only dumped values that do not come from
`ImportBIN`. It reads them into a local `t_FileHeader` and never exposes them, so the oracle
reads them from the raw bytes.

Known divergence: for a version-3 file with non-native stored lengths, Rust dumps the file
(honouring the lengths) and the oracle prints `error`. No fixture or upstream writer produces
such a file. `csbin_golden.rs::stored_struct_lengths_are_honoured` covers the Rust behaviour.

## Fixtures

| Fixture | Nodes | Receivers | Faces | Values | `nbTimeStep` | `timeStep` |
|---|---:|---:|---:|---:|---:|---|
| `upstream/lib_interface/rs_cut.csbin` | 30 | 1 (`Plane Receiver`, xml 3389) | 40 | 200 | 5 | 0.1 s |
| `upstream/python_bindings/rs_cut.csbin` | 680 | 1 (`R\xe9cepteur coupe`, xml 920) | 1248 | 60870 | 50 | 0.01 s |
| `solver-outputs/tutorial1/spps_1000hz.csbin` | 732 | 1 (`Receiver`, xml 3503) | 934 | 6041 | 200 | 0.01 s |

All three are `SPL_STANDART` with native lengths, and each is consumed to its last byte.
Upstream's own tests assert part of this: `lib_interface/tests/io_test.cpp:546-556` checks
`nbTimeStep` 5 and `SPL_STANDART`, and `python_bindings/tests/check_retrocompat_recsurf.py:16-24`
checks 1 receiver, 1248 faces, the name and xml id 920.
