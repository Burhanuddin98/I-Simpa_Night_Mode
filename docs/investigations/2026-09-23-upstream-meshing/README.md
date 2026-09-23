# What original I-Simpa does to mesh a room (2026-09-23)

This investigation was run at Burhan's request, before decision 3 was taken. Six independent
lenses looked at the question, one synthesis combined them, and a challenger re-checked five
claims. All five held.

**Read `synthesise.md` first.** Then:
- `challenge.md`, the re-check;
- `confirm150.md`, TetGen 1.5.0 against 1.6.0 on the Elmia hall;
- `DECISIONS.md`, Burhan's decisions verbatim.

The lens reports are `pipeline.md`, `forensics.md`, `tetgen-archaeology.md`, `history.md`,
`release-binary.md` and `solver-effect.md`.

Paths in these reports point to scratch evidence under `target/investigate/` on Grace. That
folder is gitignored and may be gone. Every conclusion carries its receipt: upstream file and
line, commit, URL or command output.

**The outcome:** we build our own version, with upstream's SPPS and TCR unchanged, TetGen
1.5.0, and solver inputs identical to original I-Simpa's, proven by a parity bed.
