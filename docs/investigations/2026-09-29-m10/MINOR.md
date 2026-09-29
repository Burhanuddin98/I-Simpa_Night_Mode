# M10 review: the minor findings, held for later

2026-09-29. From `REVIEW-gate-design.md` (branch `m10` at `8030e88`). None is fixed here. The one
major finding, `m10-a-highlight` passing with the overlay invisible, is fixed in `c41e656` (GATE.md).

Anything still open when M10 merges moves to `docs/v1.1-backlog.md`, with its receipt and a
"done when" test, as the project's CLAUDE.md asks.

## Where each went (M11 plan, 2026-09-29, `docs/investigations/2026-09-29-m11/PLAN.md` section 8)

An item went into M11 if it lets a wrong or misleading number reach a user, or it breaks a gate's
honesty. Every other item went to `docs/v1.1-backlog.md`, which is now the only place it lives.

| Item | Went to |
|---|---|
| A-2 | **M11, foundation:** the parameter-name rule in `m10-h` and `m11-h` (PLAN 4.2, rule 2) |
| A-3 | No action: a note, kept |
| A-4 | **M11, foundation:** `m10.ps1`'s line becomes "e2e: wdio ran (verdict below)", passing only on exit 0 (PLAN 4.5) |
| A-5 | Backlog 7 |
| B-2 | Backlog 8 |
| B-5 | Backlog 9 |
| B-6 | Backlog 10 |
| B-9 | Backlog 11 |
| B-11 | Backlog 12 |
| B-17 | Backlog 13 |
| B-18 | **M11, foundation and project package:** no volume for a refused model, one precision for the dimensions; check `m11-b18` |
| B-20 | Backlog 14, Burhan's to decide |
| B-23 | Backlog 15 |
| B-24 | Backlog 16 |
| B-25 | Backlog 17, Burhan's to decide |

## Gate honesty (review part A)

| # | Finding | Receipt | Done when |
|---|---|---|---|
| A-2 | `m10-h` sees only numbers with a unit. `ACOUSTIC_NUMBER` (`app/e2e/lib/dom.ts`) needs `dB`, `s`, `ms` or `%` after the digit, so `STI 0.62 · D50 0.45` and `1.8 sec` pass outside the Acoustics panel. Minor in M10, which shows no such number; **major from M12**, because STI is in v1 (row 19) | mutation H2: 4 of 4 passed | before M12: a parameter name (`T20\|T30\|EDT\|C50\|C80\|D50\|STI\|G\|SPL\|RT`) followed by a number fails `m10-h`, and H2 fails it |
| A-4 | The gate prints `PASS  e2e: wdio run …` when wdio exits 1. The verdict line and the exit code are right; only the line misleads a reader scanning PASS lines | A1: `exit 1 in 11.1 s` beside `PASS  e2e: wdio run` | the check is named "e2e: wdio ran (verdict below)", or passes only on exit 0 |
| A-5 | `app/e2e/specs/m10.screens.e2e.ts:19-21`: `CTRL`, `ENTER`, `BACKSPACE` are literal private-use characters (`EE 80 89`, `EE 80 87`, `EE 80 83`), shown as `''` in an editor. Not a gate spec | the bytes | they are `''`-style escapes, as in the other specs |
| A-3 | Note, not a defect: the wall control in `m10-d` is what catches a whole-group fill (D2), because Ceiling is exactly faces 10 and 11. Keep it | D2 | none |

## Design fidelity (review part B, tagged fix or minor)

| # | Finding | Done when |
|---|---|---|
| B-2 | Commands is disabled but looks live; only its tooltip says so | it has the disabled style Run has |
| B-5 | Materials in the scene list show full names, truncated ("Linoleum on concr…"); the design and PLAN.md ask for the short name, and no short-name logic exists | a short name is shown, or the fixture's names are short |
| B-6 | The added PROJECT section cuts its message: `FAIL SOURCE_NONE No source is enabl…` | the whole message is readable (wrap or tooltip) |
| B-9 | No floor grid; the design draws a 24 px grid on the floor | the grid is drawn |
| B-11 | The plan inset has no S1 and R1-R3 labels and no faint 22 px ring round the source | both are drawn |
| B-17 | On the refused hall a raw code is a label: `degenerate_faces   1 · 1 faces` after the Units row | the row reads in words, like the others |
| B-18 | Geometry numbers mislead: dimensions mix precisions (`41.45 m`, `29.71 m`, `16.1 m`; the design shows `10.00 m`), and the refused raw hall shows `Volume 0 m³`, which is not a measured zero: the hall encloses no volume. **The second is a wrong number reaching a user, so it is a candidate for v1 under the CLAUDE.md rule** | one precision for all three; a refused model shows no volume, or says why |
| B-23 | A RECEIVER_OUTSIDE refusal outlines X, Y and Z in red, though only X changed; the refusal points at `/position` | only the edited cell is outlined, or the three are outlined as one group |
| B-24 | FAIL and OK labels are JetBrains Mono at weight 600, which is not bundled (only 400 and 500), so the bold is synthesised | weight 600 is bundled, or the labels use 500 |
| B-25 | The design's `::-webkit-scrollbar` rules do nothing in WebView2 154, because `* { scrollbar-width; scrollbar-color }` makes Chromium 121+ draw the standard bar: a 10 px gutter with arrows, and a `#5A5A61` hover thumb, not the red one. The approved colours are there. `theme.css` is frozen, so a fix goes through the frozen-hash update in `m10.ps1`. **Burhan's call** | Burhan decides; if fixed, the Console thumb under the pointer is `#E0202E` in a screenshot |

## For Burhan to decide, not a defect

- **B-20, the Materials panel layout.** The grid (every material × band, Absorption and
  Scattering, + Material, Delete, Fill row) replaces the design's α strip and scattering line. At
  900 px the panel scrolls and the grid's buttons sit at its bottom edge (`review-screens/3-materials.png`).
