# T20, part 1 (M8a re-read, set S4): result

2026-10-02 09:22-09:56 (realised 33 min; forecast 2,078 s). Pre-registration `PREREG-S4.md` (`d37a99c`).

- **Build:** `simpa.exe` built at `8bc7f4b`, the T20 twin, at 09:22, into `C:\tmp\nm-target-t20`. The report's
  `meta.git_commit` reads `0d34044` (dirty) because the bed reads git at the end of the run. The commits between,
  `edbfde0` (tests) and `0d34044` (a pre-registration), change no production code. The dirty tree is
  `app/src-tauri/Cargo.toml` line endings and untracked logs.
- **Output:** `B:\data\m8b-t20\20261002T072243Z\` (`report.json`, `summary.json`), log
  `B:\data\m8b-t20\reread.err.log`.
- **Reviewed:** by sentinel (scripts in `C:\tmp\t20-sentinel\`).

## Verdict: PASS

- **P0, T30 unchanged:** holds bit for bit. Every numeric leaf of gates A-E, the TCR check, the atmospheric table
  and the transports equals M8a's `report.json`, with 0 differing numbers and 0 verdict changes. The transport is
  seeded per cell, so it re-traces identically.
- **T20 is T20's own:** it equals T30 in 0 of 7,680 seed values and 0 of 68 transports.
- **Gate C-T20:** all 32 gated cells PASS (16 random, 16 energetic). There are no INCONCLUSIVE cells, no
  extensions, and no T20 left unjudged.
  - Worst `|d| + t·SE`: random 0.19 % (6x10x3 α 0.05, air on: d +0.088 %); energetic 0.15 % (6x10x3 α 0.4: d
    +0.044 %). The limit is 0.5 %.
  - Sources: 7,103 values and 577 `monte_carlo_noise` refusals, each used as its own value per the
    pre-registration. All 577 are in the reported 20x8x4 random cells.

## Reported

- **Seed spread, statistic B:** the bed does not carry B for T20, so it was computed here, normalised by the
  transport's T20 per band (B for T30 normalises by Kuttruff).
  - Gated cells: at most 0.48 % (6x10x3 α 0.2 random).
  - Reported 20x8x4 random cells: 2.81 % (α 0.4) and 2.54 % (α 0.2). M8a's T30 B fails there too, at 5.40 %,
    which is why those cells are reported and not gated.
- **T20 against Kuttruff:** the largest |mean| is 2.59 % (20x8x4 α 0.4 random). T20/T30 - 1 on seed means runs from
  -2.98 % to +0.15 %; the transport's T20/T30 - 1 runs from -1.14 % to +0.03 %.
- **The 20x8x4 reported cells, gate C:** INCONCLUSIVE in random α 0.2 (d +0.20 %, interval -0.32 to +0.72 %) and
  α 0.4 (d +0.29 %, -0.47 to +1.04 %). Their seed noise is large.
- **Range coverage:** this cannot be scored as pre-registered. The twin stores the transport's T20 as a receiver
  mean per band, not per receiver. T30's transport keeps per-receiver values; the twin did not copy them.
  - Scored against the receiver mean, 7,418 of 7,680 values are in range. 61 count as wrong-silent, all in the
    20x8x4 room, 59 of them in energetic α 0.4.
  - That count is an artefact. In that cell the transport's per-receiver T30 is 0.398 / 0.415 / 0.417 s against a
    mean of 0.410 s, so R000 sits 2.9 % below the mean.
  - Scaling T20 the same way puts the transport's R000 T20 near 0.393 s. The product reads 0.382 s there, about
    -2.8 %: inside the 5 % JND, outside energetic mode's narrow shown range (mc_sd about 0.002 s).
  - This is an estimate, not a measurement. It points the same way as EDT's energetic failure at a far receiver
    (VERDICT-2, H3, G4).
  - **Owed:** per-receiver transport T20 in the twin, and coverage scored per receiver. Part 3 carries it, and T20
    is not done without it.
