# GUI audit, 2026-10-09

**Burhan, 02:32, verbatim:** "TAKE AN AUDIT OF THE GUI, YOU WILL FIND THAT THERE ARE THINGS THE GUI NEEDS, HERE IS A
PRACTICAL EXAMPLE FOR YOU, THE MATERIALS WINDOW, THOUGH STYLISH HAS THESE TABLES THAT NEED TO BE FILLED BUT THE LITERAL
WINDOW IN WHICH THEY EXIST IS SO SMALL THAT YOU HAVE TO SCROLL RIGHTWARDS IN LITTLE INCREMENTS AND FILL UP LITTLE BOXES
THAT CAN BARELY BE SEEN. THE WHOLE GUI HAS LITTLE DETAILS LIKE THESE THAT NEED TO BE NOTICED AND ADDRESSED."

## How it was taken

The release build of `panel-glass` (760d42a: `gpu` 95ec062 plus the 35 % panels and the hover tilt), started with
`--e2e` in its own WebView2 profile and driven over the DevTools protocol; nothing clicked in Burhan's window. Two
projects, copied from `.out\aural\` so nothing of his was opened: CR4 in octave bands (8 materials, 1 run) and CR4 in
third-octave bands (18 band columns). Every step, every dock tab, the seven menus, the command palette, the response
window and the Listen window, at 1280×720, 1536×864 (Zeph at 125 %) and 1920×1080. Each state has a screenshot and a
measurement: sideways scrolling, inputs under 56×22 px or 12 px text, click targets under 24×24 px (WCAG 2.5.8), text
under 11 px, text cut off, controls outside the window or clipped by their panel.

Screenshots, the measurements (`audit.json`, `*-audit2.json`) and the two drivers: `.out\gui-audit-2026-10-09\`.

Not covered: keyboard-only use, the import dialog with a real file, contrast through the glass (needs a pixel read,
not a token sum), and a run with particles saved (the playback panel was seen only in its empty state).

## A. Things that stop the work

**A1. The materials table (his example).** It lives in the properties panel, which is 346 px wide at every window
size, so the table's window is 318 px wide whether the screen is 1280 or 1920 wide.

| | Octave project | Third-octave project |
|---|---|---|
| Band columns | 6 | 18 (100 Hz to 5 kHz) |
| Columns fully visible (name + bands + law) | 6 of 8 | 6 of 20 |
| Table width / window width | 424 / 318 px | 856 / 318 px (63 % hidden) |
| Cell | 36 × 34 px, 10.5 px mono | same |

- The **reflection-law column is 0 px visible** for all 8 materials until you scroll right (`clipped`, 1920×1080).
- **+ Material and + From library are fully hidden** below the panel's visible area at 1536×864 and 1920×1080; you
  scroll the panel to find them.
- Values are shown rounded (`≈0.07`) in 10.5 px text; the band headings are 10.5 px too.
- Screens: `third-1536x864-materials-props-2x.png`, `oct-1920x1080-materials-end.png`.

**A2. Results: the numbers are not on screen.** The Acoustics tab opens in the 250 px dock and its first block is
"Why values are missing" prose; the T30/EDT tables are below it, and the reverberation chart is cut at its right edge
("target, shaded" runs off, a horizontal scrollbar under it). Meanwhile the right panel in Results holds four lines.
Screen: `oct2-1536x864-results-run.png`.

**A3. Results: two panels sit on the room.** The map options and the playback card stack in the middle of the view,
overlapping each other's edge and covering most of the model. The playback card (speed, trails, scrubber) is shown in
full when the run saved no particles and can play nothing. Same screen.

## B. Layout that does not adapt

**B1. Panels are fixed-size.** Scene list 248 px and properties 346 px at 1280, 1536 and 1920 alike; at 1920×1080 the
clear view between them is 1308 px wide while the materials table scrolls in 318. Nothing can be dragged wider.

**B2. 1280×720 breaks the step bar.** "Variant" is drawn over the Results step, and the variant switch gets its own
9 px horizontal scrollbar. Simulate's settings run off the bottom of the panel. Screen: `1280x720-simulate.png`.

**B3. The properties panel scrolls sideways by 10 px** at every size (its content is 344 px in a 334 px box): a
permanent horizontal scrollbar for nothing.

## C. Details

| # | What | Measured | Screen |
|---|---|---|---|
| C1 | Source positions cut off in the Sources list: "(0.00, 4.50, 1.…" hides z | 116 px shown, 124-138 needed | `oct2-1536x864-sources-picked.png` |
| C2 | Delete (22×22) sits right beside the on/off switch (30×22) on every source row | both under 24×24 | any step, left panel |
| C3 | Fold buttons on all three panels are 22×22; "Preserve walls" checkbox is 13×13 | under 24×24 | Simulate |
| C4 | 10.5 px text across the materials table, step badges, run counts, OK/Ready states; a 9 px axis label in the view | under 11 px | all |
| C5 | "Model closed" / "room closed" means watertight, but reads as "the file is closed" | wording | status bar, step bar |
| C6 | "Sound-map time step" box is empty: it means "same as the time step", but looks unfilled | blank input | Simulate |
| C7 | The Listen window opens over the step bar, hiding the Results step and the variant switch | overlap | `oct2-1536x864-listen.png` |
| C8 | The source's red glow smears through the 35 % dock when a source sits behind it (tonight's change) | look | any 1536 screen |
| C9 | Simulate's number boxes use 11.5 px text | under 12 px | Simulate |

## Recommended order

1. **A1, the materials table**: needs one choice (below), then a build.
2. **A2 + A3, Results**: the numbers first, the explanation folded under them; the chart fitted to its box; the
   playback card hidden or reduced to one line when no particles were saved; the map options docked to an edge.
3. **B1 + B3**: side panels draggable wider, remembered; the sideways 10 px removed.
4. **C1-C9 and B2** in one pass: targets to 24 px, text floor 11 px (12 px for values you type), delete moved away from
   the switch, "closed" reworded, the empty box shows its default, the Listen window placed clear of the step bar,
   the 1280 step bar.

The choice for A1: where the table is edited.
- **A wide sheet** (recommended): the panel keeps the short list for assigning a material; "Edit table" opens the full
  table in a floating window sized to its columns (all 18 third-octave bands at about 44 px, 12 px text), like the
  response window.
- **A dock tab**: the table as a "Materials" tab in the bottom dock, which is already full-width and can fill the window.
- **A wider panel only**: the properties panel made draggable; the table still scrolls with third-octave bands.
