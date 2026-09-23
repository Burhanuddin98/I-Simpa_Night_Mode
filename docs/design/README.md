# GUI design direction

**2026-09-23: Concept B approved.** The live canvas is
https://claude.ai/artifact/JMk69mXQnBUcj2USUgxdHk (private to the owner until shared). The files
here are copies of its artboards. They are self-contained Design Component pages and render only
inside that canvas.

| File | Status |
|---|---|
| `concept-b-approved.dc.html` | **Approved** as the layout and theme to build |
| `concept-a-rejected.dc.html` | Rejected: the top-left logo and wordmark and the serif headings read as a generic house style, not Dockyard's |

## What was approved

- **The theme is Dockyard Acoustics red and black.** Near-black panels, with the logo's red
  (`#E0202E`) for selection, the Run button, the active step and the source marker. The results map
  runs from black through red to yellow.
- **No logo or wordmark in the chrome.** The brand is carried by colour alone.
- **The layout:**
  - a menu bar with the project as a tab
  - a step bar (Geometry → Materials → Sources & receivers → Simulate → Results) with the variant switch on the right
  - a scene list on the left and a properties panel on the right
  - the 3D view, with a plan-view inset
  - a bottom panel with three tabs: Acoustics (RT against the DIN 18041 target, a Sabine/Eyring table, an absorption breakdown), Console (solver output sorted into FAIL, INFO and OK) and Runs (the run history)
- **Scrollbars match the theme.** This was the one fix asked for before approval.

## Still open

- **The detailed style.** "We can work on style later." Type is currently Barlow, Barlow
  Condensed and JetBrains Mono, and nothing about it is settled.
- **Red as brand colour versus red as error.** For now, bands outside the target are amber, and
  failures carry a FAIL label rather than relying on colour.

## Numbers in the mockup

- **Reverberation is computed, not made up.** It is Sabine and Eyring on upstream's 6 × 10 × 3 m
  teaching-room tutorial, with textbook absorption values.
- **The SPL map** uses the classical diffuse-field formula with an 85 dB source. It is not an SPPS
  solve.
- **The target** is a simplified ±20 % around DIN 18041 group A3 (0.55 s).
- **The console lines and run history are sample content.**
