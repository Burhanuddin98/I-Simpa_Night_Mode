# 03 - Acoustics tools and measurement hardware (research)

Date 2026-10-06. Method: WebSearch plus WebFetch. Evidence grades: **[S]** stated by a fetched or search-returned source (URL given); **[K]** author's background knowledge, NOT verified this session, no URL, treat as hypothesis. Vendor pages gave text only; no screenshot was inspected visually. Where a page is the place to see the look, it is marked "look here".

## A. Room-acoustics and measurement software

| Tool | Visual language | Source |
|---|---|---|
| ODEON | Multi-window Windows desktop app. 3D OpenGL view for geometry, materials, source/receiver checking; auto-switches between model view (outside the room) and camera view (inside). Grid response gives colour-mapped parameter maps with a scale at the right that can be zoomed and scrolled like other graphs. Dense parameter lists and graphs. [S] Look here: manual. | https://odeon.dk/download/Version18/OdeonManual.pdf ; https://odeon.dk/product/whats-new/previous-versions/odeon-17-features/ |
| CATT-Acoustic | 32-bit Windows MDI application: many child windows, minimised to icons at the bottom; 2D plan/side/end views with mouse zoom/pan and optional grid; maps exported as spreadsheets; a customised text editor is part of the workflow. Classic dialog-and-text-file style. [S] | https://www.catt.se/CATT-Acoustic.htm ; https://www.catt.se/TUCT/TUCToverview.html ; http://www.catt.se/udisplay.htm |
| EASE 5 / EASE Evac (AFMG) | Room-mapping module for viewing the room, mappings and reflectograms; solid-rendering view; floor plans and section views loaded as reference graphics. Evac renders SPL / STI / ALCons mappings over a room with ceiling-loudspeaker layouts. CAD-like, tool-heavy. [S] | https://www.afmg.eu/en/ease ; https://www.afmg.eu/en/ease-evac ; https://www.afmg.eu/sites/default/files/2024-12/EASE_5-TE_UsersGuide.pdf |
| Treble | Cloud-native, browser-based (app.treble.tech); imports IFC/SketchUp models; reviewers call it intuitive and good for presenting to non-acousticians; emphasis on audio-visual auralisation. Modern web idiom. [S] Look here: docs. | https://www.treble.tech/insights/better-acoustics-better-workflows ; https://docs.treble.tech/user-guide ; https://www.softwareadvice.com/simulation/treble-acoustic-simulation-suite-profile/ |
| Pachyderm | Rhinoceros plugin: lives inside Rhino's viewport and command line, results drawn in the host CAD model. Look at the model screenshot. [S] | https://www.food4rhino.com/en/app/pachyderm-acoustical-simulation ; https://www.orase.org/pachyderm ; https://www.researchgate.net/figure/Acoustical-simulation-model-in-Pachyderm-Acoustics-plugin-for-Rhinoceros_fig3_344270319 |
| COMSOL Acoustics | Model Builder tree at left, Settings pane, Graphics window with model/results; results are false-colour SPL fields. Docs show the Model Builder next to a Graphics window. [S] | https://www.comsol.com/acoustics-module ; https://www.comsol.com/release/6.0/acoustics-module ; https://www.comsol.com/blogs/modeling-room-acoustics-using-a-hybrid-approach/ |
| I-Simpa (Gustave Eiffel) | Open-source GUI hosting 3D sound-propagation codes; original uses wxWidgets (3.1.4 dependency), i.e. native Windows-style toolbar, tree and property panes. Our app is the dark reskin. [S] | https://github.com/Universite-Gustave-Eiffel/I-Simpa ; https://i-simpa.univ-gustave-eiffel.fr/ ; https://github.com/Universite-Gustave-Eiffel/I-Simpa/blob/main/Docs/Building.md |
| REW | Java app. Measurements list as overlapping panels on the left (selected = white background, others grey), graph area on the right; separate Generator, SPL Meter and RTA windows; green Play button, red meter on-button. [S] | https://roomeqwizard.com/betahelp/help/html/welcome.html ; https://www.roomeqwizard.com/help/help_en-GB/html/spectrum.html ; https://docs.minidsp.com/product-manuals/ears-pro/rew-step-by-step.html ; https://www.roomeqwizard.com/features.html (feature list only) |
| ARTA | Three small separate programs (ARTA, STEPS, LIMP), each plot-centred: impulse/step response, ETC, waterfall, sonogram, 1/n-octave bars, room-acoustic parameters. Utilitarian, graph-first. [S] Look here: manual. | https://artalabs.hr/download/ARTA-user-manual.pdf ; https://www.artalabs.hr/ |
| Smaart (Rational Acoustics) | Control Bar down the right side with a docked broadband level meter at top (doubles as clock); input selector; SPL metric dropdown; RTA drawn as Bars, Lines or Both; SPL meter as dB level meter or gauge. Live-sound tradition: big readable meters, few menus. [S] | https://support.rationalacoustics.com/support/solutions/articles/150000214120-what-is-smaart- ; https://downloads.rationalacoustics.com/documentation/smaart-v9/Smaart_LE_v9.1_User_Guide.pdf |
| NTi XL2 (UI) | Push buttons shown as icons on the display, rotary wheel plus Enter key to navigate. [S] | https://www.nti-audio.com/wp-content/uploads/XL2-Manual.pdf |
| Audio Precision APx500 | Multi-mode UI: Sequence Mode and Bench Mode; Bench Mode has live meters/monitors for waveform, FFT, RMS level, frequency, THD+N, described as inspired by the older SYS-2700 software. [S] | https://www.ap.com/software ; https://www.audioprecision.com/analyzers-accessories/apx555 |

**What users expect (interpretation from the above):** a stable 3D view with orbit/walk modes and receiver grids (ODEON, EASE); mapped parameters with a zoomable colour scale; separate meter and generator windows that can stay open (REW, Smaart); fixed units and decimals; map export to spreadsheets (CATT). Treble shows that "presentable" is a differentiator against ODEON/CATT density.

Not reached: no screenshot of CATT, EASE or APx500 viewed; XL2 and APx product pages gave no hardware visual description; B&K product page redirected (hbkworld.com) and was not followed.

## B. Physical front panels

| Item | Materials / colours / legends / state display | Source |
|---|---|---|
| B&K 2250/2270 | Large high-resolution touch colour screen, high-contrast; "traffic light" status indicator; IP44 enclosure; Li-ion pack; awarded for "ergonomics and attractive design". Hardware keys not itemised by the sources. [S] Look here: datasheet. | https://www.bksv.com/media/doc/bp2151.pdf ; https://maximinstruments.com/pdf_files/BRUEL_KJAER_2250_Datasheet.pdf ; https://www.atecorp.com/products/bruel-kjaer/2250 |
| B&K LAN-XI | Not reached. [K] Modular frames with front connector panels and per-channel status LEDs; unverified. | none |
| NTi XL2 | Rotary wheel plus enter key, on-screen soft buttons, optional 4-key marker keypad. [S] | https://www.nti-audio.com/en/support/know-how/monitoring-audio-with-the-xl2 ; https://us.shop.nti-audio.com/products/input-keypad-xl2 |
| Norsonic Nor150 | 4.3" colour touch-screen plus real keyboard; SETUP key opens main menu; front LED turns red on overload. [S] | https://www.norsonic.com/products/sound-level-meters/nor150-sound-and-vibration-analyser/ ; http://www.schallmessung.com/wp-content/uploads/2017/08/nor150_manual_SW2_en.pdf |
| Audio Precision APx | Sources give no front-panel description (only rear/top label locations). Panel look not established. [S] | https://www.manualslib.com/manual/1388664/Audio-Precision-Apx555.html |
| Neve 1073 | Fluted grey knobs for the three EQ band gains; red 22-way wing-knob input sensitivity; blue wing-knob high-pass turnover; two white push-buttons (EQ in/out, polarity). Original switches ELMA / Diamond H. [S] | https://www.soundonsound.com/reviews/neve-1073n ; https://www.proaudiodesign.com/blogs/news/a-brief-history-of-the-legendary-neve-1073 ; https://groupdiy.com/threads/neve-1073-1084-original-wiring.54302/ |
| SSL 4000 | EQ knobs colour-coded by band: HF red, HMF green, LMF blue, LF brown or black. [S] | https://www.bhphotovideo.com/lit_files/984282.pdf ; https://support.solidstatelogic.com/hc/en-gb/articles/28729404274717-SSL-4K-G-Channel-Strip-Plug-in-User-Guide |
| API 500 series (550A) | Large silver gain knob (+-12 dB, five steps each way), smaller blue (or clear) frequency knob with pointer, seven centre frequencies; console channels carry an 8-segment LED full-scale meter. [S] | https://www.audiotechnology.com/reviews/api-500-series-lunchbox ; https://www.soundonsound.com/reviews/api-5500 |
| Teenage Engineering OP-1 | 320x160 AMOLED screen; blue, green, white, orange encoders; orange-and-black palette; monospaced type per a third-party design essay. [S, secondary] | https://teenage.engineering/products/op-1 ; https://en.wikipedia.org/wiki/Teenage_Engineering_OP-1 ; https://blakecrosley.com/guides/design/teenage-engineering |
| Braun / Rams | T 1000: wood case with anodised aluminium frames, removable metal front lid carrying operating instructions, scale behind a clear window at top, colour accents only on the FM key and red selector markings. Ten principles incl. "as little design as possible", "unobtrusive", "understandable". [S] | https://www.designundtext.com/en/2.1.2_braun-design-t1000-rams-pbdd.php ; https://collection.powerhouse.com.au/object/474059 ; https://designmuseum.org/designers/dieter-rams |

Unverified, background knowledge only [K], no URL: Neve/SSL/API legends are small screen-printed sans capitals (white or black on grey, blue-grey or black panels); brushed-aluminium faceplates on API 500. Verify from the product pages above before using.

## C. What transfers to a screen and what does not

Transfers:
- **Colour as a role code, used sparingly.** SSL band colours, Neve red/blue wing knobs, API blue frequency vs silver gain, Braun's red only on the selector, TE's orange. For this app: one red accent (action/identity), at most 2-3 further role colours each tied to a function (band, source/receiver), never decoration.
- **Primary/secondary control pairing** (API 550A): big control = main quantity, small = secondary. Maps to a primary slider plus a smaller stepper or readout.
- **State as hardware LED language**: traffic-light status (B&K 2250), red overload LED (Nor150), segmented LED meters (API console, Smaart level meter). Solver status (idle/running/warn/fail) and levels can use discrete segments.
- **Short caps legends, consistently placed**, and a clear "window" for the one primary readout (Rams scale window).
- **Quiet housing, one accent, instructions to hand** (Braun lid): chrome recedes, data stays foreground.
- **Instrument windows that stay open** (REW generator/SPL meter, Smaart control bar).

Does not transfer, or must change:
- **Screen-printed low contrast.** Printed legends can be dim grey on dark and still read under room light; on screen text needs 4.5:1 (WCAG 2.x body text) and 3:1 for large text and UI component boundaries. Standard: https://www.w3.org/TR/WCAG21/#contrast-minimum (not fetched this session). Dim legends are a failure, not a style.
- **Physical affordances** (grip, detents, switch throw) are absent; a drawn knob needs drag, scroll, keyboard and numeric-entry paths plus a visible value, or a field is faster. The OP-1's four-encoder constraint is a hardware limit, not a goal.
- **Tiny type.** 6-8 px print legends are illegible at screen density; keep 11-12 px or more.
- **Skeuomorphic shading everywhere** (brushed metal, bevels) costs contrast and clarity in a dense data UI; confine it to a few chrome elements (meter bezel, header bar), never data tables or the 3D view.
- **LED colour alone** (red/green) fails colour-blind users and 3:1 contrast; pair with shape, position or text.
- **Needle inertia and glare effects** only where they carry information (peak-hold, smoothing); meters need a numeric readout beside them, as Smaart pairs bar with dB value.
- **Density.** Hardware has few controls because it must; ODEON/CATT users expect many parameters visible. A hardware look must not reduce parameter access.
