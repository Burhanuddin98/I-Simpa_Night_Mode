# Hardware look in audio software: what works, what dates, 2024-2026

Researched 2026-10-06 via web search plus page fetches. Where a claim rests on a search snippet only (page not opened), it is marked (snippet). Target app: dark, red-accented room-acoustics desktop app, 4.5:1 text contrast, numbers are measurements.

## Examples

### Universal Audio UAD (Neve 1073, API Vision, SSL, Manley)
- Photo-faithful hardware faces. Reviewers call it "skeuomorphic to a fault", some Manley UIs hard to see; UA is said to be settling on a middle ground with flatter, clearer newer UIs (snippet): https://cdm.link/ua-spark-review/ , https://www.sweetwater.com/universal-audio-uad-plug-ins/series
- Why it works: the face is the product. Users know the 1073 and want that layout and its quirks (snippet, same CDM page).
- Lesson: copy hardware faces only when the face IS the product. Ours is a measurement tool, so borrow materials and indicators, not the layout of a specific unit.

### Arturia V Collection / Analog Lab
- Skeuomorphic GUIs of the originals ("gets me into the feel"), but all UIs are resizable, and V Collection 9 added macros, a browser and tutorials: https://www.soundonsound.com/reviews/arturia-v-collection-9 , https://gearspace.com/board/reviews/1383236-arturia-v-collection-9-a.html (snippet)
- Analog Lab 5 went flat; KVR users complained buttons "are not clearly BUTTONS, so you look at all the text and have to guess", others liked less clutter, and both sides said hover feedback and visual hints matter in either style: https://www.kvraudio.com/forum/viewtopic.php?t=557334&start=15
- Lesson: resizable UI is a hard requirement for skeuomorphic panels. Flat needs explicit signifiers, so give every control a visible edge/state.

### Native Instruments Reaktor / Kontakt
- Reaktor 6 dropped the older shared Kontakt-style look for the flat Maschine/Komplete Kontrol styling; thinner frames, crisper fonts, anti-aliased curved Structure cables that are easier to trace: https://www.soundonsound.com/reviews/native-instruments-reaktor-6 (snippet)
- Lesson: for cable/connection drawings, rendering clarity (curves that route around objects, anti-aliasing) beat realism.

### iZotope (Ozone/Neutron)
- Neutron 3 introduced the Gauge, a widget combining knob and slider "optimized for true human-computer interaction rather than traditional skeuomorphic designs"; they chose knobs first to leave room for visualisations, then questioned both (snippet): https://www.izotope.com/en/learn/izotope-product-design-creating-the-gauge.html , https://medium.com/izotope-design/izotope-product-design-making-the-gauge-b1e555e16005 (Medium returned 403, not read).
- Community friction on the redesigns: nodes obscure the analyzer and each other (snippet): https://vi-control.net/community/threads/your-thoughts-on-skeuomorphic-vs-flat-design-in-virtual-instruments-and-effects.89636/page-2
- I found no source for a "partly back to skeuomorphic" move; treat that premise as unconfirmed.
- Lesson: the data display gets the space; controls shrink to a value-bearing widget. Do not let overlays occlude the plot.

### FabFilter (Pro-Q 4)
- Flat but tactile: knobs light on hover, a value popup shows name and value. Interactions (read from the manual): vertical drag, mouse wheel ("perhaps the easiest"), double-click for typed entry ("1k", "A4", "2x" = +6 dB, "50%"), Ctrl/Cmd+click resets, Shift = fine drag/wheel, Alt links knobs: https://www.fabfilter.com/help/pro-q/using/knobs
- Sound On Sound: Pro-Q "reimagined the plug-in equaliser using the principles of good software interface design instead of preconceptions inherited from hardware": https://www.soundonsound.com/reviews/fabfilter-pro-q-4
- Lesson: the reference implementation of knob input. Copy the whole interaction table.

### Soundtoys (Decapitator, EchoBoy)
- Characterful but simple: a handful of knobs and buttons, complexity hidden (snippet): https://www.soundonsound.com/reviews/sound-toys-decapitator-panman , https://www.soundtoys.com/product/decapitator/
- Lesson: few controls, large, clearly labelled. Personality can live in the housing; the control count stays low. (No design-rationale source found for Soundtoys, only reviews.)

### Waves (2010s backlash)
- Criticised for ornate, inconsistent looks "scattered from photo-realistic to basic gradient", for skeuomorphic skins "not really based on anything real-world", and for OpenGL UIs that got slow: https://www.kvraudio.com/forum/viewtopic.php?t=465524&start=30 , https://www.kvraudio.com/forum/viewtopic.php?t=504811&start=15 (snippets)
- Production Expert singles out Waves Butch Vig Vocals as "unique, but way too busy", and notes hardware-faithful backwards controls on 1176 plugins: https://www.production-expert.com/production-expert-1/plugin-designs-why-we-love-some-and-hate-others
- A thesis redesign proposed a modular, accessibility-compliant set: https://www.behance.net/gallery/16220067/Waves-Audio-UI-Redesign-Thesis-Project
- Lesson: one consistent material system beats per-product costume; photo-real fakery of nothing real reads as noise.

### Ableton Live
- Explicitly rejected hardware mimicry: "this is software, it can look like anything"; "A slider is just a line, a dial is just a curved slider"; "zero decoration, zero distraction"; labels and values instead of giant knobs: https://ericcarl.link/blog/ableton-live-and-designing-for-authenticity/
- Lesson: the strongest counter-position. Numbers shown as text are denser and more legible than any knob.

### Bitwig and Reaper themes
- Both are flat and themeable; Reaper's ecosystem includes dark themes inspired by Bitwig/Live (Beatwing, RLive). Bitwig's theme file is undocumented: https://reaper.blog/2014/10/try-out-the-beatwing-theme-inspired-by-bitwig-studio/ , https://www.kvraudio.com/forum/viewtopic.php?t=577403&start=75 (snippets)
- Lesson: users value being able to adjust contrast/colour. Ship a contrast-safe default and keep tokens central.

### VCV Rack
- Nearly all modules are skeuomorphic; panels are SVG, guidelines say design as if designing hardware; NanoVG renders only a subset of SVG: https://community.vcvrack.com/t/new-to-vcv-module-development/24599 , https://community.vcvrack.com/t/programmatic-panel-design/7939
- Community: "usability is very important to me, eye candy not so much"; "highly detailed 3-D knobs look extremely dated"; high-contrast panels can be uncomfortable: https://community.vcvrack.com/t/how-much-do-you-care-about-the-visual-design-of-rack-modules/10460
- Jacks and cables are the one place the metaphor does real work: the connection is the interface. The MOD forum agrees skeuomorphism suits patchbays and is poor for DAWs/sequencers, and flags wasted space: https://forum.mod.audio/t/skeuomorphism-in-plugin-uis-should-we-avoid-it/1267
- Lesson: jacks and cables only if the user actually patches (e.g. routing source to receiver to output); otherwise they are decoration. Vector, flat-shaded, not bitmap-photo.

### Teenage Engineering (OP-1, OP-Z, OP-1 field) and Braun/Rams
- OP-1 (2011) read as Braun-like, modular front panel, "Dieter Rams" lineage with wit added; OP-1 field uses a glass high-res display showing interface state live: https://en.wikipedia.org/wiki/Teenage_Engineering_OP-1 , https://acquiremag.com/tech/teenage-engineering-op1-field/ , https://onlyonceshop.com/blog/from-braun-to-teenage-engineering (snippets)
- TE's web/app UI details were not retrieved; no claim made.
- Lesson: the Rams look is mostly restraint: few colours, one accent, strict grid, small coloured indicators. It translates to screens without any texture at all.

### Neo-skeuomorphism / liquid glass (2025-2026)
- Trend pieces claim a return of "depth, glow, texture, volume" and a "rebuttonization" backlash against touchscreens: https://www.shortcut.io/news-events/the-ux-ui-trends-defining-2025-nostalgic-chaotic-and-personal , https://www.userology.co/blogs/neo-skeuomorphism-ui-trends-2026-spatial (snippets; trend-blog quality, not research).
- NN/G on iOS 26 Liquid Glass: translucent layers over busy content hurt legibility, "anything placed on top of something else becomes harder to see", smaller crowded controls: https://www.nngroup.com/articles/liquid-glass/ . A report cites Infinum measuring some beta screens at 1.5:1 against the 4.5:1 WCAG minimum (secondary; snippet): https://studio2am.co/blogs/news/liquid-glass-design-failed-its-own-usability-tests-designers-are-using-it-anyway
- Lesson: glass panels are a legibility risk by construction. Text must sit on opaque or near-opaque fills; blur only behind non-text chrome.
- Not reached: Dribbble/Figma "hardware UI" shots beyond search listings (https://dribbble.com/JuCreates?page=2 appeared, not studied).

### Critiques: affordance vs decoration
- NN/G: flat design's core flaw is missing signifiers on clickables; eyetracking showed weak signifiers cost more effort; recommendation is "Flat 2.0", subtle shadows/highlights/layering for cues, not ornament: https://www.nngroup.com/articles/flat-design/ , https://www.nngroup.com/articles/skeuomorphism/
- Smashing Magazine's "Authentic Design" argues for honesty to the medium: https://www.smashingmagazine.com/2013/07/authentic-design/ (search result only, not read). A List Apart: not found/reached.
- Framing from a secondary summary: ask whether a visual choice is "communicating or decorating" (snippet, https://www.codexical.com/posts/2026-05-24-skeuomorphism-revival-flat-design-reaction).

## Implementation facts

### LED/LCD readouts
- DSEG (SIL OFL 1.1, commercial use allowed, 50+ variants incl. 7- and 14-segment, "Mini" variants with bolder gaps for small sizes). Unlit segments: draw the all-segments glyph ("8", or "~") in a dim colour behind, and the real text over it in the lit colour; "!" is the all-off glyph. Colon/space share width, period is zero-width, so alignment is kept: https://github.com/keshikan/DSEG
- Glow is not in the font; it is a second pass (blurred copy, additive). No source found for the technique; treat as own work, keep it subtle.
- Caveat for us: segment fonts have limited glyphs and poor readability for units and text; use for numerals only, and check the dim ghost segments do not read as part of the number.

### Knobs with a mouse
- Pro audio tools use vertical drag; circular drag is fiddly and unstable near the centre; show drag direction with an ns-resize cursor; JUCE offers vertical, horizontal and circular modes plus setMouseDragSensitivity: https://news.ycombinator.com/item?id=20385268 , https://forum.juce.com/t/rotary-knob-movement-sensitivity/48280 , https://docs.juce.com/master/classSlider.html (snippets)
- Full interaction set: see FabFilter above.

### Accessibility (WAI-ARIA slider)
- APG: role slider on the thumb; aria-valuenow/min/max; aria-valuetext when the number is not self-explanatory; aria-label or aria-labelledby; aria-orientation for vertical. Keys: Right/Up increase, Left/Down decrease by one step, Home/End to min/max, Page Up/Down optional larger step. Touch AT support is incomplete: https://www.w3.org/WAI/ARIA/apg/patterns/slider/ , https://developer.mozilla.org/en-US/docs/Web/Accessibility/ARIA/Roles/slider_role
- A desktop ImGui app has no DOM, so this is a keyboard-model spec (focusable, arrow/Home/End/PageUp, value always readable as text), not literal ARIA.

## What dates badly
- Photo-real or heavily bevelled 3D knobs: "extremely dated" per VCV users (above).
- Per-product inconsistent skins and fake-real hardware that references nothing real (Waves).
- Busy panels where decoration competes with the data (Butch Vig Vocals).
- Fixed-size, non-resizable skeuomorphic faces; slow gradient/OpenGL UIs (Waves).
- Copying hardware flaws verbatim (backwards 1176 controls).
- Ultra-flat with no signifiers (NN/G; Analog Lab 5 complaints).
- Translucent glass layers under text (iOS 26, NN/G).
- Circular-drag knobs as the only input.
- Wasted space for giant knobs that show no more than a number does.

## Rules that survive
1. Numbers are text first. Show every measurement as a legible value (4.5:1 minimum); a knob, LED or gauge is a second channel, never the only one. (Ableton, FabFilter)
2. Borrow materials and indicators, not whole devices: brushed bar, engraved legend, LED dot, LCD numerals. Keep one consistent material system across all panels. (Waves counter-example)
3. Every control needs a visible signifier at rest (edge, indicator line, cursor change) plus hover/active states; subtle highlight and shadow are enough. (NN/G Flat 2.0)
4. Texture lives on chrome and housings, never behind text. Text sits on opaque or near-opaque fills. (NN/G liquid glass)
5. Knob input: vertical drag, wheel, Shift for fine, double-click for typed entry, Ctrl-click reset, hover popup with name and value, drag-direction cursor. (FabFilter)
6. Keyboard parity: focusable, arrows step, Home/End, PageUp/PageDown coarse, readable label and value text. (APG)
7. Segment displays for numerals only: a dim all-segments layer behind, lit layer on top, glow subtle. Check contrast of lit digits, not just the ghosts. (DSEG)
8. Jacks and cables only where the user really connects things; draw cables as smooth, anti-aliased curves routed around objects. (Reaktor 6, VCV)
9. The plot or data view gets the space; controls must not occlude it. (iZotope complaints)
10. Make it scalable (vector, resizable, HiDPI) and keep colours as tokens so contrast can be corrected centrally. (Arturia, Reaper themes)
11. Restraint carries the instrument feel: grid, few colours, one red accent reserved for state/alarm, small coloured indicators. (Braun/TE lineage; the accent reservation is my inference)
12. Each decorative element must answer "does it communicate or decorate?" If it only decorates, drop it. (Smashing/NN/G framing)

## Gaps
- Unread: iZotope Medium article (403); the iZotope learn page returned only nav (summary from search snippet).
- Not reached: UAD's own design statements, Soundtoys design rationale, Teenage Engineering web/app UI specifics, A List Apart, Figma/Dribbble hardware-UI examples in depth, any source on LED glow technique.
- Many Waves/Arturia/VCV items are forum opinion, not measurement.
