# The hardware look: what the research says, and what to build

2026-10-06, 05:36 to 05:50. Burhan's order: "think of acoustic equipment, the knobs, switches, the brushed materials,
the leds, the plug ins, the jacks, how can we incorporate that style here" (05:33); "research this specific layout,
the code we have written this in, and explore stylistic possibilities, read online, other peoples work and seriously
check this out" (05:36). Four reports feed this one: `01-our-code.md` (our layout, controls and constraints, with
file:line receipts), `02-audio-software.md` (UAD, Arturia, NI, iZotope, FabFilter, Soundtoys, Waves, Ableton, VCV Rack,
Teenage Engineering, the 2025-26 "neo-skeuomorphism" pieces and the NN/G critiques), `03-acoustics-tools-and-hardware.md`
(ODEON, CATT, EASE, Treble, REW, Smaart, APx, B&K, NTi, Norsonic, Neve, SSL, API, Braun), `04-web-techniques.md` (knobs,
LEDs, LCDs, brushed metal, toggles, meters, cables, accessibility, all without a canvas). Every claim below has its
receipt in one of those; where a report could not verify something, this one says so.

## The short version

1. **Borrow the materials and the indicators, never a whole device.** Every maker who copied a hardware face got
   either a product whose face *is* the product (UAD's Neve, where the user wants the 1073's layout) or a dated mess
   (Waves' "photo-real skins of nothing real", VCV users calling detailed 3D knobs "extremely dated"). Our face is not
   the product; the room and its numbers are. So: an LED beside a state, an LCD well around a readout, a brushed strip
   on one bar, a toggle for on/off. Not a rack unit. (02, examples and "what dates badly".)
2. **The instrument lineage that does not date is restraint**: Braun/Rams and Teenage Engineering. Few colours, one
   accent, a strict grid, small coloured indicators, legends to hand, the data in a clear window. That is already what
   we have (warm black, one red, sentence case, dense panels). The hardware moves should add *indicators and
   materials* to that, not noise. (02 Teenage Engineering and Rams; 03 Braun T 1000.)
3. **Numbers stay text first.** Ableton's position ("a dial is just a curved slider; labels and values instead of
   giant knobs") and FabFilter's practice (flat but tactile, value popup, the full keyboard and typed-entry set) both
   win on legibility. A knob, LED or meter is a second channel beside the value, never the only one. Our rule
   that a failure always carries its word applies to every LED. (02 Ableton, FabFilter; 01 section 5.)
4. **Glass under text is a legibility failure by construction** (NN/G on iOS 26 Liquid Glass; a cited measurement
   of 1.5:1 against the 4.5:1 minimum). We moved the panels to 0.87 for exactly this reason (decisions 61, 64).
   Brushed metal and bevels belong on chrome, never behind text or on data tables. (02; 03 section C.)
5. **Jacks and cables only where the user really patches.** They work in VCV Rack because the connection is the
   interface. In our app the one candidate, choosing a source for the Results response, is a list; a cable there
   would be decoration, keyboard-hostile and gone under Windows high contrast. **Drop H7.** (02 VCV, MOD forum; 04 §5.)
6. **Our own code sets hard limits that decide the shapes**, more than taste does (01 §6): one canvas (so knobs,
   meters and LCDs are CSS/SVG/DOM); no unit-bearing number outside Results and the marked input regions (so an
   always-on LCD readout is legal in only a few places); only Inter and JetBrains Mono may be bundled (so a seven-
   segment font needs a decision, and there is a way that needs none); `identity.css` owns the final `box-shadow`
   on every control it names and flattens every corner it does not exempt; nothing may move under reduced motion
   or WebDriver; red is for actions, selection and errors; the theme.css blob is pinned by the gates.

## What our code allows, and where (the receipts are in 01)

| Constraint | What it means for the hardware look |
|---|---|
| One `<canvas>` (m10-g, checked on every step and tab) | Knobs, LEDs, LCDs, meters, cables: inline SVG, CSS, DOM text. Nothing else. |
| No acoustic number outside Results (`selftest.ts:268`, `dom.ts:137-148`; read from `innerText`) | An LCD that prints `12 dB`, `500 ms`, `23 %` is legal only inside `[data-input]` on the Simulate settings, Sources, Materials and Scene regions, `[data-geometry]` on the geometry panel, status-bar model fact and plan inset, and on Results. The See-through slider's "N %" is legal only because the menu is closed when the gates read. Unitless numbers pass anywhere. |
| Fonts: Inter and JetBrains Mono only (decision 60; `fonts_bundled_loaded` fails on any other family; M9 counts the files) | A seven-segment font (DSEG, OFL, fine for GPL) needs a new decision and two gate edits. **Without one**, the LCD look is JetBrains Mono (tabular figures and slashed zero are already on) in amber on a dark well with a faint glow. |
| `identity.css` `!important` on `box-shadow` for `.run .primary .small-button .tool .commands .view-style-button .vp-chip-btn .segmented>button`, fields, switches, radios, the current step, and all panels | A new bevel, glow or well on those elements goes into `identity.css` itself or onto a child/pseudo-element. A new file cannot out-rank it. |
| `*:not(...) { border-radius: 2px !important }` | Every new round part (LED lens, knob cap, jack) needs its class added to the exemption list or it is squared. |
| `prefers-reduced-motion` and `html[data-no-motion]` kill all CSS motion; the glow and build-up read the same flags | LED pulses, meter decay, knob sweeps: CSS transitions/animations only, or JS that reads the two flags. Blink at most 3/s and never as the only cue. |
| Contrast 4.5:1 through the glass over white and #FCD270 (decisions 61, 64); worst today `--red-hi` 4.81 | Lit LCD digits, lit LED text and any legend on brushed metal are measured against the lightest point behind them. Unlit ghost segments are decoration, never meaning. |
| Red reserved for actions, selection, errors; `--ok` green and `--warn` amber exist | LED colours: green = good, amber = busy/warning, red = fault or selected. A red "power on" LED is borderline; use green for on. |
| Sentence case, no spaced capitals, no `text-transform` anywhere | Engraved ALL-CAPS panel legends are out. Legends are Inter, sentence case. |
| theme.css blob pinned in `m10.ps1:84` and `m11.ps1:134`, **already stale since decision 64 (`b734759d` vs pinned `b08cb4a5`)** | Re-pin is owed before the next gate regardless. New hardware tokens go in a new file (`hardware.css`, imported after `identity.css`) to leave theme.css alone. |
| Pass-through pointer layers over the canvas | Any decorative overlay (a cable SVG, a bezel) is `pointer-events: none` or it steals clicks from the 3D view. |

Two bits of rot the recon found, to fix with the first hardware commit: `fade.ts:11 FADE_BG '#09090b'`,
`engine.ts:191 PANEL 0x0f0f11` and four `rgba(9, 9, 11, ...)` rules still carry the old cool palette against the
warm `--bg: #0c0a0a`.

## What the makers learned (the rules that survive, from 02 and 03)

- Every control shows a signifier at rest (edge, pointer line, cursor) plus hover and active states; subtle
  shadow and highlight are enough (NN/G "Flat 2.0"). Our physical buttons already do this.
- One consistent material system across every panel; per-product costumes are the Waves failure.
- Knob input, if any: vertical drag, wheel, Shift for fine, double-click for typed entry, Ctrl-click reset, a
  hover popup with name and value, an `ns-resize` cursor; focusable, arrows step, Home/End, PageUp/Down (FabFilter;
  W3C APG slider). Circular drag is unstable near the centre and never the only mode.
- Segment displays for numerals only; units and words in the text face; a dim all-segments layer behind, the lit
  layer on top, a subtle glow (DSEG).
- Cables as smooth anti-aliased curves routed round objects, and only where patching is real (Reaktor 6, VCV).
- The plot or the room gets the space; controls must not occlude it (iZotope's Neutron complaints).
- Vector, resizable, tokens central so contrast can be corrected in one place (Arturia, Reaper themes).
- Colour as a role code, sparingly: SSL's band colours, Neve's red and blue wing knobs, Braun's red only on the
  selector. For us: red stays the one accent; green and amber are the two status colours; no more.
- Primary/secondary control pairing (API 550A): the big control is the main quantity, the small one the secondary.
- State in the hardware LED language: traffic-light status (B&K 2250), a red overload LED (Nor150), segmented LED
  meters (API consoles, Smaart's level meter). Pair every LED with text, shape or position, never colour alone.
- What acousticians expect (ODEON, CATT, EASE, REW, Smaart): density, many parameters visible, a stable 3D view,
  colour-mapped results with a zoomable scale, meter and generator windows that stay open. A hardware look must not
  reduce parameter access or hide numbers behind knobs. (03 §A, "what users expect".)

## What dates badly (do not build)

Photo-real or heavily bevelled 3D knobs. Fake-real hardware that references nothing real. Busy panels where
decoration competes with the data. Fixed-size faces. Hardware flaws copied verbatim (backwards 1176 controls).
Ultra-flat with no signifiers. Translucent glass under text. Circular-drag knobs as the only input. Giant knobs that
show no more than a number does. Dim "screen-printed" legends on screen. LED colour alone. Live SVG noise filters
over large areas (unmeasured in WebView2; cost grows with area and octaves; a pre-rendered tile costs nothing).

## The direction: a measurement instrument, not a studio rack

B&K, NTi and Norsonic make instruments: a quiet dark housing, one clear display window, a traffic-light status
LED, a red overload LED, small legends, a rotary wheel. That is the closest real-world object to what this app is
(a tool that refuses to lie about a number), and it is the look that will not read as a vibe-coded web app: there
is no web template for it. Neve and SSL are the other pole, the studio rack; their knobs and brushed faceplates
read as "music gear" and would pull a room-acoustics tool toward the plugin idiom that 02 shows dating fastest.

So: **instrument first, rack second.** Indicators and display wells everywhere they carry state; metal on one
bar; knobs only where a continuous value is really turned; toggles where a thing is really switched.

## The plan, one change at a time, each shown in the app

Each item names its receipt in 01/04, its legal places under our rules, and its test.

**P0. Prerequisites (one commit).** Re-pin the theme.css blob in `m10.ps1` and `m11.ps1` (owed since decision 64).
Fix the six cool-palette leftovers. Add `hardware.css` imported after `identity.css`, with the new round classes
(`.led`, `.knob`, `.knob *`, `.sw`, `.jack`) added to `identity.css`'s `:not()` list. Define tokens there:
`--led-ok`, `--led-warn`, `--led-fail` (from `--ok`, `--warn`, `--red`), `--lcd-on` (amber `#f2a93b` family),
`--well: #0a0808`. Test: `npm test`, the self-test's `fonts_bundled_loaded` still true, no new canvas.

**H1. LEDs for state** (the cheapest, the most visible). A 10 px lens (04 §2 CSS: radial highlight, `box-shadow`
halo when lit, steady, `aria-hidden`, the word stays beside it), at:
- the model check lines (`GeometryPanel.tsx:82-96`): green lit for OK, red for a failure, beside the existing word;
- the status bar's Ready dot (`StatusBar.tsx:23-39`): the 7 px dot becomes the lens; green ready, amber busy, red bad;
- each source's row in the scene list (`.marker.source`, `ScenePanel.tsx`): lit when enabled, unlit when off;
- each step's badge (`StepBar.tsx:59-66`): a lens lit green once the step is complete. There is no done-state
  today; derive it in `chrome/sceneModel.ts` beside `stepSubs` (geometry: check OK; materials: all assigned;
  sources: at least one enabled source and one receiver; simulate: a run exists; results: verified);
- the Results verdict (`ResultsPanel.tsx:97-123`): green Verified, amber Not verified, red Withheld;
- the run row status in the dock (`RunsPane.tsx`).
Halo: the same warmth and slow pulse as the sources' glow (`glow.ts`), still under reduced motion, so the LEDs read
as part of the same object as the 3D view's light. Test: `wording.test.ts` unchanged; a DOM test that every
`.led` has a text sibling; forced-colors rule from 04 §6.

**H5. Toggle switches** (next, because the switch already exists and only its drawing changes). `.switch` and
`.vp-switch` keep `role=switch`, `aria-checked` and their text "on"/"off"; the drawing becomes a bat-handle lever in a
recessed well (04 §4 CSS), 24 x 24 px minimum hit area, 3:1 lever contrast, transition only under
no-preference. Where: source on/off (scene list and Sources panel), the four Results switches, the Style menu's
three checkboxes become toggles too. Test: the e2e specs read `aria-checked`, unchanged.

**H6. The run's progress as a segmented LED meter.** `.sim-bar` (4 px) becomes a 20-cell meter (04 §4: `data-on` per
cell, amber, green near the top); the percentage stays as text beside it (it already does, in the Run button and the
Simulate sub); peak-hold from JS, none under reduced motion. The status bar's "Simulating · N %" already carries a
`%` number outside Results; check how the self-test passes today (it runs with no run active) before adding more.

**H3. Display wells for readouts.** Not a new font: JetBrains Mono (tabular, slashed zero) in amber on a `--well`
plate with a 1 px inset top shadow and a faint `text-shadow` glow, the unit in Inter beside it. Legal places only:
the Dimensions cells on the geometry panel (`[data-geometry]`), the Simulate settings' hints and the run time/finish
clock (`[data-input]` in `simulate-settings`), the playback clock on Results, the plan inset's "L x W m". If Burhan
wants true seven-segment digits: DSEG7 Classic, OFL 1.1, as a decision that amends 60 (the bundle check counts a
third family and its licence), numerals only, ghost layer hidden under forced-colors. Recommend trying the no-font
version first; it costs no gate change.

**H2. Brushed aluminium on one bar.** The step bar only (the menu bar holds dropdowns over the 3D view and should
stay quiet). CSS `repeating-linear-gradient` hairlines (04 §3), alpha at or under 6 % under text, no SVG noise
filter, labels on a flat scrim if contrast drops under 4.5:1 at the lightest hairline. The current step stays
pressed in. Try it and show it; if it reads as "music gear" rather than "instrument", drop it and keep the
anodised dark. This is the one move most likely to be dropped.

**H4. Knobs, last and few.** 01 §4: only two real sliders exist (See-through, playback scrub); every other value
is a validated text field with commit semantics (Enter/blur, refusal text, Esc, live drafts feeding hints) that a
knob must preserve, not replace; source level is read-only; playback speed is five fixed values. So:
- a `<Knob>` of our own, about 80 lines (04 §1: `role=slider`, pointer capture, vertical drag, wheel via a
  non-passive listener, Shift fine, double-click reset, arrows/Home/End, `aria-valuetext` with the unit), SVG arc
  on a `conic-gradient` cap, 24 px minimum;
- where it earns its place: See-through (replaces the range input in the Style menu; readout unitless or
  menu-only), playback speed as a five-detent rotary switch (true to `SPEEDS`), air temperature and humidity in the
  Simulate settings beside their fields (the field stays the primary, the knob turns it);
- not for particles (a count over four decades), duration, time step, receiver radius or positions: a field is
  faster and exact, as Ableton argues and ODEON's users expect.

**H7. Jacks and cables: dropped**, per point 5. If a connection ever becomes a real user act (routing a source to a
receiver to an auralisation output), revisit with 04 §5's bezier SVG and a list as the real control.

Order: P0, H1, H5, H6, H3, H2 (trial), H4 (See-through and speed first). Each shown in the app before the next, with
a before/after capture; nothing committed until he says keep, as tonight.

## What this research could not settle

No screenshot of CATT, EASE, APx or the B&K 2250 was viewed; the panel-legend typography and brushed-faceplate claims
for Neve/SSL/API are unverified background knowledge (03 marks them [K]). The iZotope "partly back to skeuomorphic"
premise found no source and is withdrawn. feTurbulence's cost in WebView2 was not measured. Dribbble/Figma
"hardware UI" work and Teenage Engineering's own app UI were not studied. Several Waves/Arturia/VCV points are forum
opinion. None of these gaps changes the plan; they bound how confident the "what dates badly" list is.
