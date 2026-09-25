# Decision 5 against upstream's box layout without the scene correction (2026-09-24)

The receipt behind `docs/m5-m6-design.md`, "Decision 5 against upstream's layout (measured
2026-09-24)": item 8 of the tutorial-3 follow-ups. Copied here on 2026-09-25 from the gitignored
scratch folders the follow-ups' report listed as deletable (the tutorial-3 follow-ups' critic):

| File | What | From |
|---|---|---|
| `item8-2.txt` | the transcript the design doc's table is read from: each corpus project with an enabled box zone, meshed with decision 5's layout and with upstream's, the region check held to the welded geometry and to the geometry as written | `target/agents/fu-fix-behaviour-scratch/item8-2.txt` |
| `zz_item8_measure.rs.txt` | the scratch test that printed it (a `crates/simpa-core/tests/` file when it ran, never committed; renamed so nothing compiles it) | `target/agents/fu-fix-behaviour-scratch/zz_item8_measure.rs` |
| `item8.txt`, `zz_item8_measure-first.rs.txt` | the first run and its test, before the region check against the geometry as written was added | `target/agents/fu-behaviour-scratch/` |

The test ran on the `t3-followups` branch on 2026-09-24, in the fix round of its behaviour piece.
It is a receipt, not a gate: nothing in the suite re-runs it, and the layout it measured was not adopted.
