# Provenance: M10 UI fixtures

The three data files here are written by one recipe, `crates/simpa-core/tests/ui_fixtures.rs`,
built from core types only (nothing in it imports app or UI code). Regenerate with
`SIMPA_WRITE_FIXTURES=1 cargo test -p simpa-core --test ui_fixtures`; without the variable the
same test fails when a committed file differs from its recipe by one byte. It runs with the core
crates' tests and in `tools/gates/m10.ps1`. Plan: `docs/investigations/2026-09-29-m10/PLAN.md`,
section 5.

| file | sha256 |
|---|---|
| `teaching_room.simpa` | `5ba9b3f53a44a1a3f935b07c2ff5040a41d4d98b6acb6b8382633bc54b3eeb7d` |
| `materials_6x6.tsv` | `5cc56d5bbe83ea96d7868dda030a40282cd8cfe3a8484e16d5c121eb76741818` |
| `materials_6x6.expected.json` | `1c037138e4c07263fd920577697db1f9a3db2f181465af552b9de969593f8820` |

`tests/fixtures/** -text` keeps the bytes: the TSV's CRLF endings are part of the test.

## `teaching_room.simpa`

The room of the approved design (`docs/design/concept-b-approved.dc.html`):

- **Geometry.** A 10 x 6 x 3 m box, x 0-10, y 0-6, z 0-3: 8 vertices and 12 triangles with
  outward normals, two per group in group order. The Front wall is at x = 0, the Rear wall at
  x = 10, the Right wall at y = 0 and the Left wall at y = 6. Octave bands, 125 Hz to 4 kHz.
- **Groups.** Floor, Ceiling, Left wall, Right wall, Front wall, Rear wall.
- **Materials.** Six, every absorption and scattering value 0.10, specular, double-sided, no
  transmission. The swatches are the design's; the glass swatch is new.

  | material | used by |
  |---|---|
  | Linoleum on concrete | Floor |
  | Mineral-fibre tile | Ceiling |
  | Painted plaster | Left wall, Right wall, Front wall |
  | Slotted wood panel | Rear wall |
  | Heavy velour curtain | (unused) |
  | Glass window | (unused) |

- **Source.** S1 at (2, 3, 1.5): omni, pink, a global level of 85 (dB re 1 pW).
- **Receivers.** R1 at (5, 2, 1.2), R2 at (8, 4, 1.2), R3 at (3.5, 4.5, 1.2), each oriented
  along +x.
- **Ids** are fixed with `Uuid::from_u128`: the project 0x1, groups 0x100 + i, materials
  0x200 + i, the source 0x300, receivers 0x400 + i.

The recipe asserts: the M4 check is ok, with an enclosed volume of 180 m3 and an area of 216 m2;
`validate()` returns no issue at all, 0 errors and 0 warnings, so Run's blockers are exactly
`M11_PENDING`; no material is upstream's `Default` placeholder. The app's own test
(`app/src-tauri/src/scene.rs`) holds the placeholder rule to 6 of 6 groups assigned.

## `materials_6x6.tsv`

Six rows, the six materials in project order, by six octave bands of absorption, as a
spreadsheet copies a block: CRLF line endings with a trailing CRLF, no header row, no name
column.

- Rows 1 to 5 are the design's textbook values (`concept-b-approved.dc.html`, the `MATERIALS`
  table: lino, tile, plaster, panel, curtain).
- Row 6 is the "ordinary window glass" row common absorption-coefficient tables give: 0.35,
  0.25, 0.18, 0.12, 0.07, 0.04. It was not checked against a primary source here; the fixture
  needs plausible values, not authoritative ones.
- **One cell is not a textbook value.** The wood panel at 4 kHz (design value 0.40) holds
  `0.39509836780726515`: the first decimal a seeded xorshift64 finds in [0.395, 0.405) that is
  its double's shortest spelling, has 17 significant digits, and that `serde_json::from_str`
  reads to a different double in this build. It is also the text JavaScript's
  `String(Number(cell))` gives, so it is what a paste sends over IPC. A lossy reader anywhere
  between the paste and the save therefore changes the saved bytes. (At 0.35 no such decimal
  exists, and above 0.5 no double needs 17 digits: the recipe's comment says why.)

The recipe asserts that every cell matches the UI's `parseStrictDecimal` grammar
`^[+-]?(\d+(\.\d*)?|\.\d+)([eE][+-]?\d+)?$`, lies in [0, 1], and that the odd cell is misread.

## `materials_6x6.expected.json`

The honest part. The recipe reads the **committed** `.tsv` bytes with its own reader (split on
CRLF, then on tabs, each cell by `str::parse::<f64>`, correctly rounded as JavaScript's
`Number()` is), loads the **committed** `teaching_room.simpa` with `schema::load`, applies one
`SetMaterialBand { material: materials[row], quantity: absorption, band, value }` per cell
through `History::apply`, and writes `schema::to_json`. So the file is the core's output for that
input. The UI must reach the same bytes by its own path (its TSV parser, `opText`,
`Op::from_json`, `History`, `schema::save`); the file is never typed to match what the UI
produces.

The recipe also asserts that the start file differs from the expected file, so gate (c)'s byte
comparison can fail, and that the odd cell reaches the file with its exact bits.

Pinned so the two paths agree: the grid's default row order is project order; a paste lands at
the focused cell, all or nothing; one paste is one `batch` op and one undo step.
