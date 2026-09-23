# `.pbin`: particle trajectories

SPPS writes a `.pbin` when `nbparticules_rendu` is above zero. It holds the positions and energies
of a sample of its particles, one record per time step. Upstream's GUI animates it. We read it;
we never write it.

| | |
|---|---|
| Written by | SPPS `ReportManager` (`spps/input_output/reportmanager.cpp`), and `particleio::ParticuleIO` (`lib_interface/input_output/particles/part_io.cpp`, used by the Python bindings' samples) |
| Read by | `ParticuleIO::OpenForRead` / `NextParticle` / `NextTimeStep`, called by upstream's GUI (`isimpa/3dengine/Core/Particules.cpp:139`, `LoadPBin`) |
| Rust | `crates/simpa-core/src/formats/pbin.rs`, read only |
| Oracle | `oracle/dump_pbin.cpp`, on `ParticuleIO` |
| Fixture | `tests/fixtures/solver-outputs/tutorial1/spps_50hz.pbin`, 1,780 bytes, 17 particles, 101 steps |

Receipts below are into `target/solvers/src-929a5c8/src/`: `lib_interface/` unless another root
is named.

## Platform dependence

Upstream types every integer header field as `bpLong`, `#define`d as `unsigned long`
(`input_output/particles/part_binary.h:37`). The structs are written raw with
`write((char*)&s, sizeof(s))`, so the file layout is the writing compiler's struct layout:

| Data model | `unsigned long` | `binaryFHeader` | `binaryPHeader` | `binaryPTimeStep` |
|---|---|---|---|---|
| **Windows x64 (LLP64), MSVC or MinGW-w64: what we read** | 4 bytes | **28** | **8** (4 + 2 + 2 padding) | **16** |
| Windows or Linux 32-bit (ILP32) | 4 bytes | 28 | 8 | 16 |
| Linux or macOS 64-bit (LP64) | 8 bytes | 56 (6 × 8 + 4 + 4 padding) | 16 (8 + 2 + 6 padding) | 16 |

`vec3` is `base_vec3<float>`, since `decimal` is `float` (`Core/mathlib.h:54`, `:239`).
All values are little-endian.

The header stores the writer's own `sizeof` of all three structs, and the reader requires them to be
28/16/8. An LP64 file fails earlier. Read with the Windows layout, its bytes 4..8 are the zero high
half of its 8-byte `nbParticles`, so its `formatVersion` reads as 0, and the reader returns
`Version`.

## Byte layout (Windows)

```
offset  size  field                       struct
     0     4  nbParticles        u32      binaryFHeader  (part_binary.h:44-52)
     4     4  formatVersion      u32
     8     4  fileInfoLength     u32      = 28
    12     4  particleInfoLength u32      = 16
    16     4  particleHeaderInfoLength u32 = 8
    20     4  nbTimeStepMax      u32
    24     4  timeStep           f32
    28        then nbParticles particle records, back to back:

  +0       4  nbTimeStep         u32      binaryPHeader  (part_binary.h:57-62)
  +4       2  firstTimeStep      u16
  +6       2  padding                     never read, never dumped
  +8          then nbTimeStep steps:

  +0       4  position.x         f32      binaryPTimeStep (part_binary.h:68-73)
  +4       4  position.y         f32
  +8       4  position.z         f32
 +12       4  energy             f32
```

**Size.** A file is exactly `28 + Σ over particles (8 + 16 × nbTimeStep)` bytes. The fixture is
28 + 17 × 8 + 101 × 16 = 1,780. There is no magic number, no terminator and no checksum.

## Fields

- **`nbParticles`.** SPPS first writes the requested count, `nbparticules_rendu` × number of
  sources (`spps/sppsNantes.cpp:332`, `spps/input_output/reportmanager.cpp:134`). At close it
  rewrites it with the number of particles actually written, those that recorded at least one step
  (`reportmanager.cpp:296`, `:337-343`). The fixture requested 20 and holds 17. If SPPS dies
  before closing, the header keeps the requested count while the body has fewer particles, and the
  reader reports `Truncated`. `ParticuleIO` writes 0 at open and the real count at `Close`
  (`part_io.cpp:93-106`, `:50-70`).
- **`formatVersion`.** `PARTICLE_BINARY_VERSION_INFORMATION`, which is 1 (`part_binary.h:36`). Both
  writers store it (`part_io.cpp:63`, `reportmanager.cpp:140`). No other version is written
  anywhere in this upstream tree.
- **`fileInfoLength`, `particleInfoLength`, `particleHeaderInfoLength`.** The writer's
  `sizeof(binaryFHeader)`, `sizeof(binaryPTimeStep)` and `sizeof(binaryPHeader)`
  (`part_io.cpp:60-62`, `reportmanager.cpp:137-139`).
- **`nbTimeStepMax`.** The two writers mean different things by it. SPPS stores the simulation's
  time-step count, `IPROP_QUANT_TIMESTEP` (`reportmanager.cpp:135`, `sppsNantes.cpp:333`), so the
  fixture holds 200 while no particle exceeds 17 steps. `ParticuleIO` stores the largest particle
  step count (`part_io.cpp:128-129`). Upstream's GUI uses it only to estimate memory
  (`Particules.cpp:167`). Nothing checks it against the data, and neither do we.
- **`timeStep`.** The simulation time step in seconds, as `f32`. The fixture holds `3c23d70a`
  (0.01).
- **`nbTimeStep`.** The number of step records that follow. SPPS never writes 0 (it skips empty
  particles, `reportmanager.cpp:294`). `ParticuleIO` writes 0 when `NewParticle` is not followed
  by `NewPositionParticle`. We accept 0.
- **`firstTimeStep`.** The simulation step of the particle's first record. SPPS keeps it in an
  `entier` (`int`, `reportmanager.h:160`) and passes it through the `unsigned short` constructor
  parameter (`part_binary.h:59`), so a first step above 65535 is already wrapped in the file.
  Upstream's reader widens it to `unsigned long` (`part_io.cpp:242`). It is 0 for every particle
  in the fixture.
- **Padding** after `firstTimeStep`. SPPS builds the header on the stack
  (`reportmanager.cpp:297`), so these two bytes are whatever was there. They are zero in the
  fixture but not guaranteed to be, and they are never dumped.
- **Steps.** Any `f32` bit pattern is carried through, NaN and signalling NaN included.

## Upstream's reader

`OpenForRead` (`part_io.cpp:178-209`) opens the file, `memset`s a `binaryFHeader`, reads
`sizeof(binaryFHeader)` bytes into it, and keeps every field except `formatVersion`, which it
reduces to `isoldversionfile = (formatVersion != 1)` (`:196`). `GetHeaderData` (`:171-176`) exposes
`timeStep`, `nbParticles` and `nbTimeStepMax`. `NextParticle` (`:222-255`) reads one
`binaryPHeader` into a `memset` struct and exposes `nbTimeStep` and `firstTimeStep`. `NextTimeStep`
(`:256-284`) reads one `binaryPTimeStep` and exposes `x, y, z, energy`. It throws when asked for
more particles than `nbParticles` (`:250`) or more steps than `nbTimeStep` (`:277`).

What it does not do matters as much:

1. **It never checks a read.** `OpenForRead` returns true whenever the file opens, however short
   it is. A read past the end leaves a particle header zeroed, or partly filled, by its `memset`
   (`part_io.cpp:239`). A step comes back at position (0, 0, 0), from `base_vec3`'s default
   constructor (`Core/mathlib.h:95`), with whatever `energy` the uninitialised
   `binaryPTimeStep` held (`part_binary.h:69`). Either way it still reports success, so a crashed
   SPPS run loads with empty particles and origin-bound steps where its data ran out. We return
   `Truncated`.
2. **It never looks past the last declared particle.** We return `Invalid` on any remaining bytes.
   Those bytes are what a `ParticuleIO` file looks like when it was never `Close`d: the header
   still says 0 particles.
3. **For `formatVersion != 1`** it re-strides by the header's length fields: it seeks to
   `particle header + particleHeaderInfoLength` after each particle header and to
   `step + particleInfoLength` after each step (`:245-248`, `:265-271`). No writer produces such a
   file, and we do not port the branch. We return `Version`.
4. **After a particle with no steps**, its skip test `currentstep < nbtimestep-1` (`:229`) underflows
   (`nbtimestep - 1` on an unsigned 0), so it seeks to `that particle's header +
   particleHeaderInfoLength + particleInfoLength × 0` (`:232`). With the true size 8 this is a
   no-op. With any other value it reads the next particle from somewhere else. This is why the
   length fields must be 28/16/8 even in a version-1 file. A cross-check found it (see
   Verification): all 35 mismatches between the two readers were exactly the files with
   `particleHeaderInfoLength ≠ 8` and an empty particle before the last.
5. **Positions are `unsigned long`.** `lastParticuleHeaderInfo` holds `tellp()` in 32 bits
   (`part_io.hpp:130`, `part_io.cpp:234`). In a file over 4 GiB, the seek in item 4 would land
   in the wrong place. We use `usize` offsets throughout. This was not tested: no such file exists.
6. **Partial readers.** If a caller reads all but one step of a particle, the skip test in item 4
   is false and the next particle header is read from the last step. That is an upstream bug that
   only affects callers that stop early. The GUI reads either every step or none
   (`Particules.cpp:203`), and so do we.

## Failure modes

Checks run in this order. The first failure is returned.

| Condition | Rust | Upstream `ParticuleIO` | Oracle |
|---|---|---|---|
| File missing | `NotFound` | `OpenForRead` false | `error` |
| Fewer than 28 bytes | `Truncated` | partial header, zero-filled, success | `error` |
| `formatVersion ≠ 1` | `Version` | re-strided read (item 3) | `error` (declared, not upstream's) |
| Sizes not 28/16/8 | `Invalid` | reads, possibly off the layout (item 4) | `error` (declared, not upstream's) |
| `nbParticles × 8` > bytes left | `Truncated`, nothing reserved | reads zeros past the end | `error` |
| A particle header runs past the end | `Truncated` | zeros, or a partly filled header | `error` |
| `nbTimeStep × 16` > bytes left | `Truncated`, nothing reserved | steps at (0, 0, 0) with uninitialised energy | `error` |
| Bytes after the last declared particle | `Invalid` | ignored | `error` |

**Allocation.** The model is two flat vectors: 8 bytes per particle header and 16 per step, both
reserved at their exact size. It needs no more memory than the file, well inside the gate budget
of 2 × len + 64 KiB. Every count is checked against the bytes remaining before anything is
reserved. A first pass sums the step counts, and a second pass fills the step vector.

## Rust API

```rust
pub const FORMAT_VERSION: u32 = 1;
pub const FILE_HEADER_SIZE: usize = 28;
pub const PARTICLE_HEADER_SIZE: usize = 8;
pub const TIME_STEP_SIZE: usize = 16;
pub struct FileHeader { nb_particles, format_version, file_info_length, particle_info_length,
                        particle_header_info_length, nb_time_step_max: u32, time_step: f32 }
pub struct ParticleHeader { nb_time_step: u32, first_time_step: u16 }
pub struct TimeStep { position: [f32; 3], energy: f32 }
pub struct ParticleFile { header: FileHeader, particles: Vec<ParticleHeader>, steps: Vec<TimeStep> }
impl ParticleFile { fn iter(&self) -> impl Iterator<Item = (&ParticleHeader, &[TimeStep])>;
                    fn byte_len(&self) -> usize; }
pub fn read(bytes: &[u8]) -> Result<ParticleFile>;
pub fn read_file(path: &Path) -> Result<ParticleFile>;
pub fn dump(value: &ParticleFile) -> String;
pub fn dump_file(path: &Path) -> String;
```

The steps are flat, in file order. Particle `i` owns the `particles[i].nb_time_step` steps that
follow those of the particles before it, and `iter()` pairs them.

## Canonical dump

This follows the shared rules in `README.md`: floats as raw bits, one record per line, `\n` line
ends.

```
pbin <formatVersion>
header <nbParticles> <formatVersion> <fileInfoLength> <particleInfoLength> <particleHeaderInfoLength> <nbTimeStepMax> <timeStep:f32>
particles <n>
particle <nbTimeStep> <firstTimeStep>          (n times, in file order; each followed by:)
step <x:f32> <y:f32> <z:f32> <energy:f32>      (nbTimeStep times, in file order)
```

- The `header` line is `binaryFHeader` in its field order. `formatVersion` also appears on the
  first line, which is the format-and-version line every dump starts with.
- The `particle` line is `binaryPHeader` in its field order. Its first token, `nbTimeStep`, is the
  count line for the `step` records that follow it. `firstTimeStep` is decimal.
- Padding is never dumped.
- Integers are decimal. `timeStep`, `x`, `y`, `z` and `energy` are 8 lowercase hex digits
  (`f32_hex`).
- On failure, Rust prints `error <notfound|truncated|version|invalid|io>` and the oracle prints
  `error`.

The fixture's dump is 121 lines (3 + 17 + 101) and begins:

```
pbin 1
header 17 1 28 16 8 200 3c23d70a
particles 17
particle 2 0
step 403884a2 41017216 402d8e7e 2cb6225b
step 40310942 410d1bd3 3f9ca05d 2cb6225b
particle 7 0
```

## The oracle

`oracle/dump_pbin.cpp` drives upstream's `ParticuleIO` for every value it prints: the header
through `GetHeaderData`, the particles through `NextParticle`, and the steps through
`NextTimeStep`. `ParticuleIO` keeps `formatVersion` (as a bool) and the three length fields
private. So the oracle also reads upstream's own `binaryFHeader` exactly as `OpenForRead` does
(`part_io.cpp:189-192`), prints those four fields from it, and checks that the three exposed
fields match.

Upstream's reader cannot report a short file, so the oracle tracks the offset that reader has
reached. It prints `error` where upstream would read past the end or leave bytes unread, and it
does so before printing anything from such a read. It also prints `error` for the two cases where
we refuse a file that upstream reads by a different rule: a version other than 1, and length
fields other than upstream's `sizeof`s. Both are marked "declared" in the failure table. Agreement
is not claimed there. `static_assert`s pin the 28/8/16 sizes, so the oracle cannot be built with
another layout.

## Verification (2026-09-23)

- `pbin_golden`: 14 tests. The fixture's header values, per-particle counts and first and last
  steps are checked, along with its byte arithmetic (28 + 17 × 8 + 101 × 16 = 1,780) and dump
  shape. Oracle agreement is part of this suite. The negative tests cover:
  - a missing file, which returns `NotFound`
  - every proper prefix of the fixture (all 1,780 of them), each `Truncated`
  - a particle count or step count up to `u32::MAX`, which is `Truncated` with peak allocation
    within budget
  - trailing bytes and an under-declared count, both `Invalid`
  - versions 0, 2 and `u32::MAX`, plus an LP64 header, all `Version`
  - length fields other than 28/16/8, all `Invalid`
  - padding, which is ignored
  - empty files and particles with no steps, which are accepted
- `pbin_fuzz`: four proptests of 10,000 cases each, with peak allocation ≤ budget(len) on every
  case and no panics. The four inputs are arbitrary bytes, arbitrary bytes behind a valid 20-byte
  prefix, mutations of the fixture, and structured version-1 files. A planted over-reservation
  was caught by three of them.
- Oracle cross-check, a throwaway run not committed: the fixture, a missing path and 703 synthetic
  files, 0 mismatches. The synthetic files were 200 valid, 300 mutations of the fixture and 200
  invalid, plus 3 edge cases. 251 dumps were identical, including 187 signalling NaNs carried
  bit-exact and 100 files with an empty particle before the last. The other 454 failed on both
  sides.
