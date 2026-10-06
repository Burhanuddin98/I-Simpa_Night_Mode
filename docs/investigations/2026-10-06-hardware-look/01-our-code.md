# Hardware look: recon of our UI code (2026-10-06)

Read-only recon of `app/ui/src` in the `ui` worktree. Paths are relative to `app/ui/src` unless they start with `app/` or `tools/`. Receipts are `file:line`. Nothing here was edited or run; counts of instances are by reading the JSX, not by running the app.

Two surprises first, because they change the plan:

1. **The theme.css pin is already stale.** `tools/gates/m10.ps1:84` and `tools/gates/m11.ps1:134` pin blob `b08cb4a52944a3d089e2a0040085191cc3186229`. `git hash-object app/ui/src/theme.css` now gives `b734759d5d6333f22256246d0827efdefaa0b62e`. Commit `ff820a9` (decision 64) rewrote the tokens and did not re-pin. So the lint `theme.css unchanged since the M10 foundation (git blob)` (m10.ps1:195, m11.ps1:288) fails today, before any hardware-look edit. Any theme.css edit needs a re-pin of both gate files; the re-pin is owed already.
2. **Three leftovers of the old zinc palette sit outside theme.css** and no longer match `--bg: #0c0a0a`: `features/viewport/fade.ts:11` `FADE_BG = '#09090b'` (the far-wall fade mixes toward this, not toward `--bg`), `features/viewport/engine.ts:191` `PANEL = 0x0f0f11` (the plan inset's clear colour), and `rgba(9, 9, 11, ...)` in `viewport.css:24`, `viewport.css:203`, `chrome.css:1036`, `acoustics.css` `.rw.glass` (about line 234). The chrome's own neutrals are warm; these are cool. Cosmetic, but a new hardware surface will be compared against them.

---

## 1. The window's layout as built

### Shell and slots (`App.tsx:124-143`)

```
.shell (flex column, 100% height, overflow hidden)       theme.css:91-96
  MenuBar                .menubar    36 px tall            chrome.css:6-22   (z-index 5)
  Landing                .landing    fixed, inset 0        chrome.css:1024-1034 (z-index 2; only with no project)
  .behind-landing (inert while landing shows)              chrome.css ~1040
    StepBar              .stepbar    44 px tall            chrome.css:~118   (z-index 1)
    .work (flex row, gap 8, padding 8)                     theme.css:185-191
      ScenePanel         .scene      248 px wide           chrome.css:~245
      main.center (flex column, gap 8, flex-grow 1)        theme.css:192-198
        Viewport         .viewport   flex-grow 1           viewport.css:7-12
        Dock             .dock       250 px tall           dock.css:4-15
      PropertiesPanel    .props      344 px wide           chrome.css:~435
  StatusBar              .statusbar  24 px tall            chrome.css:~712
  ImportDialog, SavePrompt  (.dialog-backdrop fixed, z-index 20)  chrome.css:~750
```

Exact numbers:

| Region | Size | Margin / gap | Receipt |
|---|---|---|---|
| `--gap` | 8 px | used by `.work` padding and gap, `.center` gap, bar margins | theme.css:40 |
| `.menubar` | height 36 px, padding 0 8 px, margin `8px 8px 0` | | chrome.css:6-22 |
| `.stepbar` | height 44 px, padding 0 12 px, margin `8px 8px 0` | | chrome.css (the `.stepbar` rule after the `.commands` block) |
| `.scene` | width 248 px, content height (`align-self: flex-start`, `max-height: 100%`), scrolls | folded: 36 px wide, height 36 px | chrome.css `.scene`; `.scene[data-folded]` chrome.css:~868; motion.css:26-29 |
| `.props` | width 344 px, content height, scrolls | folded: 36 px wide | chrome.css:~435; chrome.css:~868 |
| `.dock` | height 250 px | folded: `height: auto` (tab strip only, 34 px tabs) | dock.css:5, dock.css:~322-330 |
| `.dock-tabs` | height 34 px | | dock.css:17 |
| `.statusbar` | height 24 px, padding 0 12 px, margin `0 8px 8px` | gap 18 px between items | chrome.css:~712 |
| `.center` width | the remainder: window width minus 248, 344 and the 8 px gaps/padding | | flex-grow 1 |
| viewport tools | 32 px square buttons, top 12 / right 12 px, gap 2 | | viewport.css:47-68 |
| map panel `.vp-map-panel` | top 52, left 12, max-width 340 px | | viewport.css:~226 |
| `.vp-dock` | left 364, right 84, bottom 12 | | viewport.css:~330 |
| plan inset | left 12, bottom 12; drawing box 180 x 116 px | | viewport.css:92-122 |

So the visible gap between any two floating panels, and from a panel to the window edge, is 8 px; overlays inside the 3D region sit 12 px from its edges.

### Which element floats over which

- **Layer 0: the canvas and its backdrop.** `.viewport-host` is `position: fixed; inset: 0; z-index: 0` with `background: var(--bg) radial-gradient(ellipse at 50% 55%, rgba(224,32,46,0.07) 0%, rgba(9,9,11,0) 60%)` (viewport.css:20-25). The one WebGL canvas lives inside it (`.viewport-canvas`, absolute, 100% x 100%, viewport.css:26-34). The marker labels layer `.vp-labels` is also `position: fixed; inset: 0; z-index: 0; pointer-events: none` (viewport.css:169-175). `Viewport.tsx:126` mounts the host as a child of `.viewport`, but fixed positioning makes it fill the window.
- **Layer 1: the panels.** `.stepbar, .statusbar, .scene, .props, .dock { position: relative; z-index: 1 }` (chrome.css:~970-975). `.menubar` is z-index 5 (chrome.css:23) so its dropdowns (z-index 10, chrome.css:55) go over the rest.
- **Pointer pass-through.** `.work` and `.center` are `pointer-events: none` and their children take it back with `pointer-events: auto` (theme.css:199-211); `.center > .viewport` is `pointer-events: none` and `.viewport > *` re-enables it (viewport.css:13-18). So the canvas, under everything, gets every click that no panel or overlay takes.
- **Overlays inside `.viewport`** (positioned absolutely, all `.float-panel` glass): view tabs + Style menu top left (`.overlay-tl`, 12/12), tools column top right (`.tools`), model-check chips top centre (`.vp-chips`), results map panel and note (top 52, left 12), plan inset bottom left, `.vp-dock` (probe, legend, transport) bottom, axis gizmo bottom right (right 12, bottom 10).
- **Above the panels:** menu dropdown z 10, `.vp-menu` z 20 (viewport.css:238), `.landing` z 2 (and `.statusbar` 3 while it shows), `.dialog-backdrop` z 20, `.rw` (response window) z 30 (acoustics.css:~216). 17 `z-index` declarations in all.

### The glass tokens (theme.css:32-40)

`--glass: rgba(19,16,16,0.87)`, `--glass-blur: 12px`, `--glass-edge: rgba(255,236,230,0.1)`, `--gap: 8px`. The recipe is repeated, not shared, in six places: `.glass` (theme.css:213-219), `.float-panel` (viewport.css:40-46), `.vp-chip` (viewport.css:136-150), `.menubar`, `.stepbar`, `.scene`, `.props`, `.statusbar` (chrome.css), `.dock` (dock.css:9-13). Each has `background: var(--glass); backdrop-filter: blur(var(--glass-blur)) saturate(1.15); border: 1px solid var(--glass-edge); border-radius: 7px`. The text on a panel must never be faded with `opacity` (theme.css comment, line 32-33).

---

## 2. Styling architecture

### Import order (`main.tsx:4-10`)

`@fontsource-variable/inter` and `jetbrains-mono` CSS, then `theme.css`, then `App` (which pulls each package's CSS through its component import), then `motion.css`, then `identity.css` last ("so its transitions sit over the packages' own rules"). Anything in identity.css therefore wins on equal specificity, and `!important` wins over the packages' inline-ish rules.

### theme.css tokens, all of them (theme.css:7-41)

| Token | Value | Use |
|---|---|---|
| `color-scheme` | dark | |
| `--bg` | #0c0a0a | window background, canvas backdrop |
| `--panel` | #131010 | scrollbar thumb border only (theme.css:85) |
| `--line` | #221d1c | section dividers, progress track |
| `--line-soft` | #1a1615 | plane-row dividers |
| `--edge` | #2c2524 | field and button borders, switch track off |
| `--raised` | #171312 | fields, segmented tray, dropdown fill |
| `--raised-2` | #1e1918 | hover fill, project tab |
| `--badge` | #241e1d | step number badge |
| `--text` | #efe9e5 | main text |
| `--text-2` | #cbc2bd | secondary text |
| `--text-3` | #b0a6a0 | tertiary text, labels |
| `--red` | #e0202e | actions, selection, switch on, Run |
| `--red-hi` | #ff6b74 | red text (worst contrast token: 4.81 over white through a panel) |
| `--red-deep` | #3a0b10 | pressed/selected fill |
| `--red-wash` | #1a0b0e | defined; not used in the CSS read |
| `--ok` | #74d39f | OK state text |
| `--warn` | #f2a93b | warnings |
| `--fail-text` | #ffb3b8 | failure sentence text |
| `--sans` | 'Inter Variable', 'Segoe UI', system-ui, sans-serif | |
| `--cond` | same as `--sans` | `.label` headings |
| `--mono` | 'JetBrains Mono Variable', 'Cascadia Mono', Consolas, ui-monospace, monospace | numbers, codes |
| `--glass` | rgba(19,16,16,0.87) | |
| `--glass-blur` | 12px | |
| `--glass-edge` | rgba(255,236,230,0.1) | |
| `--gap` | 8px | |

Hard-coded colours that are not tokens: scrollbar `#34343c` (theme.css:72, 83; cool grey, a zinc leftover), `#fff` on pressed/selected, `#0a0808` and `#070606` in identity.css, `rgba(224,32,46,x)` red washes.

Base rules: `body` 13 px, `user-select: none`, `overflow: hidden` (theme.css:49-56); `button:focus-visible` and `input/select:focus-visible` get a 2 px `--red` outline (theme.css:66-69, 126-130); inputs `border-radius: 5px`, `--raised` fill, `--edge` border (theme.css:116-125); `.segmented` tray 26 px keys, selected key `--red-deep` fill with white text (theme.css:160-182); `.issue` lines (theme.css:137-159); `.label` 12 px, weight 600, `--text-3`.

### Where each package keeps its CSS

| File | Lines | Owns |
|---|---|---|
| `theme.css` | 219 | tokens, base, `.shell`, `.work`, `.center`, `.glass`, fields, `.issue`, `.segmented` |
| `chrome/chrome.css` | 1156 | menu bar, step bar, variant switch, scene list and rows, markers, props panels, check rows, fact grid, status bar, dialogs, `.switch`, `.fold`, `.landing` |
| `features/viewport/viewport.css` | 546 | canvas host, tools, view tabs, chips, plan inset, labels, map panel, `.vp-chip-btn`, `.vp-switch`, legend, transport, probe, Style menu |
| `features/materials/materials.css` | 435 | `.mat-*` panel, `.mat-option` radios, `.mg-*` grid, library menu |
| `features/dock/dock.css` | 329 | dock tabs, console, runs table, `.flag`, `.verified`, folded dock |
| `features/simulate/simulate.css` | 481 | `.run`, `.sim-*` settings, `.sim-bar`, `.res-*` results panel |
| `features/acoustics/acoustics.css` | 437 | `.ac-*` cards and tables, `.rw` response window |
| `motion.css` | 96 | see below |
| `identity.css` | 138 | see below |

### motion.css (read fully)

- `:root { interpolate-size: allow-keywords }` so height/width can animate to `auto` (line 15-17).
- Folding panels: `.scene, .props` transition width and height 240 ms `cubic-bezier(0.2,0.7,0.2,1)`; folded `height: 36px`; their children (except `.fold`) and `.dock > .dock-body` fade opacity 300 ms, transform 360 ms, `display 300ms allow-discrete`; folded children `opacity: 0`; `.dock` transitions height only (lines 19-45).
- `@starting-style`: children start at opacity 0; `.props[data-step-dir='next']` children slide in from `translateX(28px)`, `prev` from `-28px` (lines 47-60).
- Drop-downs: `.dropdown, .view-style-menu, .vp-menu, .mg-lib-list` fade 150 ms and move 170 ms; start at `opacity 0; translateY(-6px) scale(0.98)`; origin top left; closing is immediate (lines 62-80).
- No motion: `@media (prefers-reduced-motion: reduce)` and `html[data-no-motion]` both set `transition: none !important; animation: none !important` on `*, ::before, ::after` (lines 83-96). `main.tsx:13` sets `data-no-motion` when `navigator.webdriver`.

### identity.css (read fully)

Header: decision 64, "less like a stock AI-built app", styling only. Ten blocks:

1. (comment only; tokens live in theme.css.)
2. **Corner rule, with `!important`:** `*:not(.dirty-dot, .marker, .dot, .radio, .knob, .track, .switch-track, .switch-knob, .mat-swatch, .vp-switch *) { border-radius: 2px !important }` (lines 13-15). Everything else is 2 px square, regardless of what the package CSS says. The round exceptions are exactly: `.dirty-dot`, `.marker`, `.dot`, `.radio`, `.knob`, `.track`, `.switch-track`, `.switch-knob`, `.mat-swatch`, and anything inside `.vp-switch`. **A new round part (a knob, an LED lens, a jack) needs a class added to this `:not()` list or it is flattened to 2 px.** `.vp-switch *` is a descendant exemption, so its `.track` and `.knob` are covered twice.
   `box-shadow: none !important` on `.dropdown, .view-style-menu, .vp-menu, .mg-lib-list, .rw, .landing-card, .dialog, [role='dialog']` (lines 16-25).
3. `body { font-feature-settings: 'cv11' 1, 'ss01' 1, 'zero' 1, 'tnum' 1 }` (28-30): Inter's single-storey a, open digits, slashed zero, tabular figures.
4. **Physical buttons.** On `.run, .primary, .small-button, .tool, .commands, .view-style-button, .vp-chip-btn, .segmented > button`: `box-shadow: inset 0 1px 0 rgba(255,240,235,.12), inset 0 -1px 0 rgba(0,0,0,.5), 0 1px 0 rgba(0,0,0,.55) !important`; 70 ms transitions of transform, box-shadow, filter (35-51). `:hover:not(:disabled)` is `filter: brightness(1.12)`; `:active:not(:disabled)` is `translateY(1px)`, `brightness(.92)`, and a sunk inset shadow `!important` (52-75). Rows, menu items, `.fold`, `.step`, `.switch`, scene rows are deliberately not included.
5. **Fields set in:** `input:not([type=range],[type=checkbox],[type=radio]), select { background-color: #0a0808 !important; box-shadow: inset 0 1px 2px rgba(0,0,0,.75), inset 0 -1px 0 rgba(255,240,235,.06) !important }` (77-84).
6. `.segmented { box-shadow: inset 0 1px 3px rgba(0,0,0,.65) !important }`: a recessed tray (86-89).
7. **Slide switches:** `.switch-track, .vp-switch .track` inset slot shadow `!important`; `.switch-knob, .vp-switch .knob` get a white-to-black 35%/15% vertical gradient and a lit top edge `!important` (91-102).
8. **Radios as LEDs:** `.mat-option .radio { background: #070606; box-shadow: inset 0 1px 2px rgba(0,0,0,.85) }`; `.mat-option[aria-checked=true] .dot { box-shadow: 0 0 6px 1px rgba(224,32,46,.85) }` (104-111). This is the only glow in the chrome so far.
9. **Step bar:** `.step[aria-current=step]` pressed in (dark fill 0.35, `border-bottom-color: transparent !important`, inset shadow plus a 2 px `--red` inset foot, all `!important`); `.step .badge` keycap shadow (113-125).
10. **Machined edges `!important`:** `.scene, .props, .dock, .stepbar, .menubar, .statusbar, .float-panel:not(button)` get `inset 0 1px 0 rgba(255,240,235,.07), inset 0 -1px 0 rgba(0,0,0,.55)` (127-138). Because it is `!important` on the panel's own `box-shadow`, a panel cannot get an outer shadow or glow without editing this rule.

The `!important` count is the thing to know: identity.css decides the final `box-shadow` on every control it names, so a hardware look that wants a different shadow on those controls either edits identity.css or must beat `!important` (it cannot, without `!important` plus a later position or higher specificity).

---

## 3. Every kind of control

Counts are what can be on screen at once, from reading the JSX; "state" is what it shows today.

### Buttons

| Control | Class / role | File | Roughly | State shown |
|---|---|---|---|---|
| Run | `.run` (+ `.run-wrap`) | `features/simulate/simulate.css:9-42`; the button is in `features/simulate/SimulatePanel.tsx` | 1 (plus `.sim-run-big` simulate.css:396, 1) | red fill; `:disabled` opacity .45; `data-running='true'` keeps .85 and shows the percentage |
| Primary dialog action | `.primary` | chrome.css (`.dialog-actions button.primary`, `.landing-actions button.primary`) | 1-2 per dialog | red |
| Small button | `.small-button` | chrome.css (about 69-93 of the props block) | roughly 10-15 on a step (Add, Remove, Change bands, Cancel, etc.) | `aria-pressed='true'` variant; disabled |
| Tool | `.tool` | viewport.css:56-77 | about 10 in the tools column (3 mode tools plus disabled placeholders at `Viewport.tsx:229`, a separator and Frame model) plus 4-5 transport buttons (`ResultsOverlay.tsx:503-531`: start, back, play, forward) | `aria-pressed='true'` is `--red-deep` fill with `--red-hi` icon; disabled .35 |
| Commands | `.commands` | chrome.css (after `.dirty-dot`); `MenuBar.tsx:207` | 1, always disabled ("not built yet") | |
| Fold | `.fold` | chrome.css:~866-880 | 3 (scene, props, dock) | glyph only; not in the physical-button list |
| Style button | `.view-style-button` | viewport.css:~480; `ViewStyleMenu.tsx` | 1 | `aria-expanded='true'` red border |
| Chip button | `.vp-chip-btn` (`role="radio"` when in a group) | viewport.css:~262-285 | up to about 25 on Results (speeds 5, trails about 6, colour modes, bands) | `aria-checked='true'` is `--red-deep` fill, `--red` border |
| Wide button | `.wide-button` | chrome.css ~518; `GeometryPanel.tsx:33` | 1 (Import model) | |
| Menu item | `.menu-item` | chrome.css:~29-48 | 7 (File, Edit, View, Model, Simulate, Results, Help) | `aria-expanded`; disabled for unbuilt menus |
| Dropdown item | `.dropdown button` | chrome.css:~56-82 | 3-10 per open menu | disabled greys |
| Step | `.step` (button, `aria-current='step'`) | chrome.css (`.step`), identity.css:113-125 | 5 | current: red 2 px foot; badge red |
| Segmented | `.segmented > button` | theme.css:160-182 | View tabs 3 (`Viewport.tsx:191-210`, Section disabled), variants about 1-4 (`VariantSwitch.tsx:98`), Materials quantity 2 (`MaterialsGrid.tsx:536`), dialog rows | `aria-selected='true'`: `--red-deep` (or `--edge` for `.segmented.view`, viewport.css:84) |
| Scene row | `.scene-row` | chrome.css:~330-370 | one per surface group, source, receiver (tens to hundreds in a big hall) | `aria-pressed='true'`: `--red-deep` plus 1 px red inset ring |
| Point row | `.point-row` | chrome.css | one per source/receiver in the Sources panel | `aria-pressed` |
| Sim solver choice | `.sim-solver` (`role=radio`) | simulate.css:48-74 | 2 (SPPS, TCR) | `aria-checked` |
| Sim method | `.sim-method` (`role=radio`) | simulate.css:146-156; `SettingsEditor.tsx:~430` | 2 (Random, Energetic) | `aria-checked` |
| Row remove | `.row-remove` | chrome.css:~930-960 | one per source/receiver row, shown on hover | |
| Library menu items | `.mg-lib-list button`, `.mg-tools button` | materials.css:325-435 | about 5 plus a list of library materials | |

### Switches

| Control | Class | File | Roughly | State |
|---|---|---|---|---|
| `.switch` (role=switch) | `.switch`, `.switch-track` (32 x 18, 14 px knob), `.switch.compact` (26 x 14, 10 px knob) | chrome.css:~832-905; `SourceSwitch` at `ScenePanel.tsx:48-70`, used compact at `ScenePanel.tsx:235` and full at `SourcesPanel.tsx:354` | one per source, twice (list and panel) | on: red track, knob right, white knob; off: `--edge` track, grey knob; text "on"/"off" beside it (`.switch-text`) |
| `.vp-switch` (role=switch) | `.vp-switch .track/.knob` (32 x 18, 14 px) | viewport.css:~290-325; `ResultsOverlay.tsx:358` (diff), 370 (cumulative), 425 (smooth), 454 | 4 on Results | `aria-checked`; `disabled` |
| Checkbox | native `input[type=checkbox]` | `SettingsEditor.tsx:138` (`Toggle`: preserve walls, sound maps per band, echogram per source) and `:177` (one per band in `BandsEditor`) | 3 plus about 8-10 band checkboxes | unstyled native (excluded from the identity.css field rule) |

### Radios

`.mat-option` (role radio, `aria-checked`) with `.radio` and `.dot` children, `features/materials/materials.css:64-101`; one per candidate material in the selected group's panel (about 3-10). Already an LED lens in identity.css:104-111. `.sim-solver` and `.sim-method` and `.vp-chip-btn` also use `role=radio` but are drawn as buttons. The Style menu uses `role=menuitemradio` with a 7 px `::before` red dot (viewport.css:~515-525).

### Inputs

| Input | Where | Class | Roughly |
|---|---|---|---|
| Text commit field (`CommitInput`, Enter or blur commits, Esc restores) | `chrome/SourcesPanel.tsx:122` | `className` is passed in: `mono` for the X/Y/Z position fields (`SourcesPanel.tsx:283`), `sim-input mono` for settings (`SettingsEditor.tsx:~106`) | 3 position fields, `.name-input` 1, settings fields about 8 (particles, particles saved, duration, time step, receiver radius, extinction, temperature, humidity, pressure) |
| Variant rename | `chrome/VariantSwitch.tsx:45` | `.variant-rename input` 150 x 26 | 0-1 |
| Scene filter | `chrome/ScenePanel.tsx:164`, `.filter input` | | 1 |
| Materials grid cell editor | `features/materials/MaterialsGrid.tsx:772`, `.mg-editor` | grid cells `.mg-cell` are `<td>`, only the focused cell is an input | table of surface groups x about 8 bands, 1 input at a time |
| Reflection law | `MaterialsGrid.tsx:726`, `<select class="mg-law-select">` | | one per row |
| Colour range | `ResultsOverlay.tsx:272`, `.vp-range-input mono` (58 x 24) | | 2 (From, To) |
| Range slider (step) | `ResultsOverlay.tsx:533-543`, `type="range" class="vp-step" aria-label="Time step"`, min 0, max steps-1, step 1 | `accent-color: var(--red)` (viewport.css:~345) | 1, flex width |
| Range slider (see-through) | `ViewStyleMenu.tsx:71-78`, `type="range"`, min 0, max 60, step default 1, aria-label "Near walls' opacity, percent"; in `.view-style-glass`, width 90 px | `accent-color: var(--red)` | 1, only when Surfaces = See-through |
| Native selects | `AcousticsPane.tsx:460, 483, 510, 566` (source, receiver, DIN group, band), `SettingsEditor.tsx:184` (band preset) | `.ac-control select`, `.sim-preset select` | 5 |

There are exactly two `input[type=range]` in the app (confirmed by grep: ResultsOverlay.tsx:534 and ViewStyleMenu.tsx:72). Neither has custom track or thumb styling; both rely on `accent-color`.

### Tabs

- Step bar: 5 `.step` buttons (above).
- View tabs: 3 `role=tab` buttons in `.segmented.view` (`Viewport.tsx:191-210`, Section disabled, "not in this version").
- Dock tabs: 3 `.dock-tab` (`role=tab`), Acoustics, Console, Runs (`Dock.tsx:21-25, 80-106`), selected = `border-bottom` 2 px `--red` (dock.css:24-45), with a `.tab-badge` mono count (`.live` is `--red-hi`).
- Materials quantity: 2 tabs (absorption, scattering).

### Dropdowns and menus

`.dropdown` (menu bar, 7 menus, `MenuBar.tsx:182`, chrome.css:50-62: `--raised` fill, 220 px min, 30 px items), `.dropdown.vp-menu` (face context menu, `Viewport.tsx:129`), `.view-style-menu` (190 px min, `ViewStyleMenu.tsx`; groups Surfaces 4, Edges 2-3, Shading), `.mg-lib-list` (materials library, materials.css:393-432, z 6). All 4 are in the motion.css drop-in list and the identity.css no-shadow list. Note the `.dropdown` rule still has `box-shadow: 0 8px 24px` at chrome.css:61 which identity.css cancels with `!important`.

### Dialogs

`.dialog-backdrop` (fixed, z 20, `rgba(0,0,0,.55)`) around `.dialog` (min 380, max 520 px; `.dialog.prompt` 420 px) for `ImportDialog.tsx` and `SavePrompt.tsx`; `.landing-card` (820 px) on the landing page; `[role=alertdialog]` `.sim-confirm` inline in the band editor (`SettingsEditor.tsx:~213`); the response window `.rw` (fixed, centred, z 30, `min(980px, 100vw-48px)`; `.rw.glass` is 0.95 opaque of `rgba(9,9,11)`; acoustics.css:211-235). 0-1 visible at a time except the landing.

### Status bar (`chrome/StatusBar.tsx`, 24 px)

Left: `.status` (7 px `.dot` plus text; colour by class: default `--ok`, `.busy` `--red-hi`, `.bad` `--warn`; `StatusBar.tsx:23-39`), then the model fact, the variant name, a spacer, then two static strings "Solvers: I-Simpa 1.4.0 · SPPS, TCR" and "Units: m". Five or six items.

### Step badges

`.step .badge` (18 x 18, mono 10.5 px, `--badge` fill, `border-radius: 4px` which identity.css flattens to 2 px; chrome.css `.step .badge`). Current step: `--red` fill, white text. The number is the step's index; there is **no done/not-done state on the badge today** (see section 5).

### Other state elements

`.verdict.ok/.fail` (chrome.css ~543-550), `.state.ok/.fail` check marks (32 px column in `.check-row` and `.sim-state`), `.tag`/`.mat-flag`/`.ac-tag` (small badges), `.flag` (the word FAIL in the dock), `.sim-bar` (4 px progress bar, simulate.css:270-279), `.vp-legend-bar` (200 x 8 colour bar), `.marker` (source/receiver glyphs in the scene list: 10 px red dot, outline when off, 9 px white ring for receivers), `.swatch` 10 px, `.dirty-dot` 6 px red.

---

## 4. Where continuous values are set today (knob candidates)

Every one of these is a text field except the two sliders. All text fields are `CommitInput` (`chrome/SourcesPanel.tsx:~86-150`): a plain `<input>` with `spellCheck={false}`, committing on Enter or blur, Esc reverting, refusal shown inline. In SettingsEditor they go through `NumberField` (`SettingsEditor.tsx:~81-121`), strict decimal parse (`numbers.ts`), `className="sim-input mono"` (104 px wide, 26 px tall, right aligned, simulate.css:125-131) and an optional `.sim-unit` (18 px).

| Value | Component and field | Input element | Unit | Receipt |
|---|---|---|---|---|
| See-through (near walls opacity) | `ViewStyleMenu.tsx` | **`input[type=range]`**, 0 to 60, integer; readout "N %" in a `<span>` | % | ViewStyleMenu.tsx:71-79, state `style.glass` |
| Particles per source and band | `SettingsEditor.tsx` `NumberField group="spps" field="particles"` | text `CommitInput` | none (count) | SettingsEditor.tsx:~342; parse `parseCount` (whole number 0 to 2,147,483,647) |
| Particles saved for playback | `field="particles_saved"` | text | count | ~353 |
| Duration | `field="duration"` | text | s | ~369 |
| Time step | `field="time_step"` | text | ms | ~381; the step count and file size hints follow the typed draft (`onDraft`) |
| Receiver radius | `field="receiver_radius"` | text | m | SettingsEditor.tsx:395-405 |
| Particle extinction | `field="extinction"` | text | tens of dB | ~409 |
| Air temperature, relative humidity, pressure | `AirEditor` (`SettingsEditor.tsx:~245-275`), `NumberField group="air"` fields `temperature`, `humidity`, `pressure`; hint `realInputText(env.*)` | text | °C, %, Pa | |
| Source level (sound power, dB) | **not editable.** `SourcesPanel.tsx:301-316`: `<span className="mono">{powerText(source.power.global_db)}</span>` in a `.kv` row under "Emission", with the hint "Read-only in this build" | none (read-only text) | dB | The emission section is `data-input`, so it is exempt from the acoustic-number rule in `sources-panel` region (dom.ts:44-50) |
| Source position X, Y, Z | `SourcesPanel.tsx:277-296`, `.axis-field input.mono` | text | m | 3 per selected point |
| Playback speed | `ResultsOverlay.tsx:546-562` | **not a continuous control**: 5 `.vp-chip-btn` radios from `SPEEDS = [0.005, 0.01, 0.025, 0.1, 1]` (`animator.ts:34`), label via `speedLabel(x)`, rate line `rateText(anim)` | x | `Animator.setSpeed(x)` refuses any value not in `SPEEDS` (animator.ts:95-97), so a knob would have to snap to these 5 detents |
| Time step (playback position) | `ResultsOverlay.tsx:533`, **`input[type=range]`** 0 to steps-1 | range | step index | the scrub bar |
| Colour range low and high | `ResultsOverlay.tsx:272`, `.vp-range-input` | text, commits on Enter or blur | dB | |
| Trails | `ResultsOverlay.tsx:~566-585`, `TRAIL_LENGTHS` chips | 0 and lengths as radios | steps | detent-like already |

So of the candidates the user named: the see-through slider is the only real slider in a menu; playback speed is 5 fixed values (a 5-position rotary switch is truer than a continuous knob); source level is read-only text and would need an op to be editable (out of scope for a look change); everything else is a validated text field whose commit semantics (Enter/blur, refusal text, Esc, the live drafts that feed hints) a knob would have to preserve or sit beside, not replace.

---

## 5. Where state is shown that could be an LED

| State | Where it is now | Receipt | Tokens used |
|---|---|---|---|
| Model check lines OK/FAIL | `.check-row` with `.state.ok/.fail/.none` (a 32 px text column holding the words OK, FAIL or none), `.k` label, `.v` value; the heading `.verdict.ok/.fail` | `GeometryPanel.tsx:82-96`, chrome.css `.check-row` ~554-575 and `.state.ok/.fail` ~567-572 | `--ok`, `--red-hi` |
| Model check chip over the view | `.vp-chip.fail` with a 10 px `.swatch` in `--warn` and the word "Problem" | `Viewport.tsx:176-182`, viewport.css:136-165 | `--warn`, `--red-hi` |
| Simulate pre-run checks | `.sim-check` with `.sim-state.ok/.fail/.unchecked` | simulate.css:204-260 | |
| Run status | status bar `.status` with `.dot` (7 px, `currentColor`); busy text "Simulating · N %" | `StatusBar.tsx:23-39`, chrome.css:~735-760 | |
| Run progress | `.sim-bar` / `.sim-bar-fill` (4 px) in the Simulate panel; the step bar's Simulate sub and the Run button's percentage | simulate.css:270-279 | `--line`, `--red` |
| Run row status | `.c-status.ok/.fail/.muted/.warn/.live` | dock.css:~237-255 (`RunsPane`) | |
| Source enabled | `.switch` (on/off) and the scene row's `.marker.source` (filled red dot, outlined when `.off`) | `ScenePanel.tsx:48-70`, chrome.css:`.marker.source` ~370-380 | `--red` |
| "Ready" | `.status` text with its 7 px dot, default class (no modifier) is `--ok` green. States come from `statusStore` (`kind`: `ready`, `busy`, `bad`) | `StatusBar.tsx:35-38`, chrome.css:~735 | `--ok` |
| Step completion | **not shown.** `StepBar.tsx:59-66` renders a number badge and a text sub (`stepSubs`, e.g. "not closed" with `.sub.fail`). `aria-current` only marks the current step. There is no done flag, so an LED per step would need a derived state from `sceneStore`/`runStore` (`stepSubs` in `chrome/sceneModel.ts` already derives the subs) | `StepBar.tsx`, `steps.ts:2-8` | |
| FAIL / WARN flags in the dock | `.dock .flag` (the word FAIL) in `ConsolePane.tsx:106, 112` and `RunsPane.tsx:163, 231`; console lines coloured by tag (`.console-line.FAIL .tag --red-hi`, `.WARN --warn`, `.OK --ok`, dock.css:95-113); dock tab `.tab-badge.live` | dock.css:15-20 (the `.flag` rule at about line 154 of the file), 95-113 | `--red-hi`, `--warn`, `--ok` |
| Results verified | `.res-verdict.ok` ("Verified", then "Results verified"), `.warn` ("Not verified"), `.fail`; `data-results-state` on `.res-state`. The dock's runs table repeats it as `.verified.verified/.unverified` text | `ResultsPanel.tsx:97-123`, simulate.css:410-430, `RunsPane.tsx:154-170` | `--ok`, `--warn` |
| Unsaved changes | `.dirty-dot` 6 px red dot with `role=img` "Unsaved changes" | `MenuBar.tsx:204`, chrome.css `.dirty-dot` | `--red` |
| Radio choice | already an LED: `.mat-option .radio` lens with a 6 px red glow when checked | identity.css:104-111 | |

Rule that applies to all of them: **a failure always carries its word, never colour alone** (theme.css:135-136, `.issue`; `.switch` comment, chrome.css ~826-829: state also as text). An LED must sit beside the word, not replace it. Note `--ok` green is used for state text but red is "action, selection and error", so a green and an amber LED are both already in the token set; no new colours are needed.

---

## 6. Constraints a hardware look must respect

1. **One canvas.** `app/e2e/specs/m10.viewport.e2e.ts:8` and `:276`: `m10-g: document.querySelectorAll("canvas").length == 1 throughout` (receipt printed at :303, "one canvas at each of N points"), checked on the hall, every step, both view tabs, all dock tabs and the box. `tools/gates/m10.ps1:72` lists `m10-g` in the `viewport` spec group. So no `<canvas>` for knobs, meters, LCDs or LED glows anywhere in the DOM. All of it must be CSS, inline SVG, or DOM text. (The charts in `AcousticsPane`/`ResponseWindow` already respect this, as SVG/img.) Also: the plan inset and the Results map are drawn into the same canvas under the DOM (`engine.ts` header lines 11-12), so the hole `.plan-box` (viewport.css:118-122, drawn with `box-shadow: 0 0 0 400px var(--glass)` around a transparent box) must stay a transparent rectangle.
2. **No acoustic numbers outside Results.** Two detectors. `selftest.ts:268` `ACOUSTIC = /\d(?:[\d.,]*\d)?\s*(?:dB|Hz|kHz|ms|s|%|m²|m³|m|°C)(?![\p{L}\p{N}])/gu` is run over `document.body.innerText` (line 277-278) and `no_acoustic_numbers` requires 0 matches (`selftest.ts:345`). The e2e side is `app/e2e/lib/dom.ts:137` `ACOUSTIC_NUMBER = /\d\s*(dB|s|ms|%)(?![\p{L}\p{N}])/u` and `dom.ts:147-148` `PARAMETER_NUMBER` (T15, T20, T30, T60, RT60, EDT, RT, C50, C80, D50, Ts, STI, SPL, LF, LFC, G, Sabine, Eyring, "reverberation time" followed by a number). Both read **innerText**, so any visible text node `"23 %"`, `"500 ms"`, `"12 dB"` outside a Results region fails, and so would an LCD readout, a knob value label ("45 %") or a meter scale ("-20 dB") anywhere but Results. The exemptions are by region, not by look: `EXEMPT_REGIONS` (dom.ts:35-50) lets `[data-input]` hold unit numbers only inside `scene-list`, `sources-panel`, `materials-panel`, `simulate-settings` and `[data-geometry]` only inside `geometry-panel`, `statusbar-model-fact`, `plan-inset`, `material-group`; `RESULTS_REGIONS` (dom.ts:63-67) is `[data-props-step="results"]`, `[data-dock-panel="acoustics"]` and `[data-results-region]`, hidden from the check only while the Results step is current. Consequences: (a) the See-through slider's "N %" readout in `ViewStyleMenu.tsx:79` is a style number and is currently only legal because the menu is closed when the gates read the page (a knob readout that is always on screen would be read); (b) an always-visible LCD for source level, temperature, duration etc. is allowed only on the Simulate settings or Sources panels if marked `data-input` and sitting inside those regions; (c) a decorative LCD that prints a unit-bearing number anywhere else, including the status bar and menu bar, fails `no_acoustic_numbers`. Unitless numbers pass, and a number split from its unit by markup does not stop `innerText` (it concatenates), so do not rely on that.
3. **theme.css pinned by git blob.** `tools/gates/m10.ps1:84` `$themeBlob`, checked at `:195`; `tools/gates/m11.ps1:134`, checked at `:288`. Both pin `b08cb4a5...` and the file is already `b734759d...` (see top). Edits to theme.css need both gate files re-pinned (`git hash-object --path=app/ui/src/theme.css`). Prior re-pins: decision 50 (glass layout). Putting hardware tokens in a new file imported in `main.tsx` (as identity.css does) avoids touching the blob.
4. **Motion.** `prefers-reduced-motion` and `html[data-no-motion]` (set from `navigator.webdriver`, `main.tsx:13`) kill every `transition` and `animation` with `!important` (motion.css:83-96). The glow pulse (`glow.ts` header: holds still at mid pulse under both) and the room build (`build.ts`: never under either) follow the same rule. Any LED pulse, knob spin, needle ballistics or meter decay must be a CSS transition/animation (so motion.css kills it) or must read the same two conditions; gates compare frames, so anything animating under WebDriver breaks them. A `transition` set in identity.css is also covered by this rule (identity.css:47-51).
5. **Fonts (decision 60).** Inter Variable (text) and JetBrains Mono Variable (numbers, code), bundled via `@fontsource-variable/*` (`main.tsx:4-5`), both SIL OFL 1.1, licences in `public/licenses/`. `selftest.ts` `fonts_bundled_loaded` requires both families to load and **nothing else bundled** (`fonts.bundled.every(f => BUNDLED_FONTS.includes(f))`, selftest.ts:~346); M9's bundle check counts Inter 7 files, JetBrains Mono 6, other 0, OFL licences 2. So no seven-segment, LCD-dot-matrix or engraved display font may ship; an LCD look has to be done in JetBrains Mono (tabular, slashed zero via identity.css `'zero' 1`) with colour, glow and a scanline/segment-ghost background. Decision 60 also says "no spaced capitals in a display face anywhere".
6. **Contrast, decisions 61 and 64.** Every text colour must read at 4.5:1 or more *through a panel* over pure white and over #FCD270 (and #FFF7F2 for decision 64), "the brightest things the view can put behind a panel" (theme.css:34-36). Worst case today `--red-hi` over white at 4.81. At glass 0.87. So LED-lit text, an LCD's lit digits and any dim "unlit" segment used as text must be checked against the glass fill, not against an opaque dark; unlit ghost segments are decoration and must not carry meaning. `--text-3` #b0a6a0 is the dimmest permitted text. A darker inset (a `#0a0808` display well, as inputs now have) improves local contrast and is allowed, but the 0.87 glass must still not be turned into a near-black opaque plate without Burhan's say (decision 61 deliberately moved from 0.4 to 0.85, 64 to 0.87). The `.rw` window is already 0.95.
7. **Sentence case.** Decision 60: "Headings are sentence case in the text face; no spaced capitals in a display face anywhere." There is no `text-transform: uppercase` anywhere in the CSS (the only `text-transform` is `none`, chrome.css:311 and viewport.css:116). Engraved all-caps panel legends would break this rule. There is a source test, `wording.test.ts`, that scans every UI string and CSS text for the word "validated" (decision row 39), so panel legends must also avoid it.
8. **Red is reserved** for actions, selection and errors (viewport.css build comment; `glow.ts` header "not red: red is for actions and selection"; matcap comment, engine.ts:~236 "no red"). Today red also lights the checked radio LED and the on-state of switches (actions). A red power LED that means "on" is borderline; green (`--ok`) and amber (`--warn`) are the available status colours, red stays for "selected/error".
9. **Round things** need a class in the identity.css `:not()` list (section 2 above), or they become 2 px squares.
10. **`!important` order in identity.css** (sections 4, 5, 7, 9, 10 set `box-shadow !important`) means a new knob, LED or bevel on any of the named controls cannot add its own `box-shadow`; it must be added to those rules or placed on a child element/pseudo-element.
11. **Self-test geometry.** The step bar stays laid out under the landing page "the self-test reads the step bar's boxes" (`App.tsx:6-8`); `steps_visible` requires non-zero rects and `visibility != hidden` for all five (`selftest.ts:273-276`). The selectors `[data-step]`, `[data-part="name"]`, `[data-menu]`, `[data-dock-tab]`, `[data-part="viewport"]`, and `data-part`/`data-field`/`data-control` attributes on controls are read by gates and e2e specs; keep markup and attributes when restyling.
12. **Pointer rules.** Pass-through (theme.css:199-211) means any new decorative layer over the canvas must be `pointer-events: none`, or it eats the 3D view's clicks.

---

## 7. The three.js viewport's own look (so the chrome can match it)

**Surfaces and light.** `engine.ts` (header, lines 1-19) renders the room from inside with faces drawn `BackSide`, so near walls drop away. Shading is a camera-fixed matcap (decision 59, built from pixels at `engine.ts:238-262`): mid greys on black, `#767680` at the key light (upper left, 0.36/0.68 of the sphere) falling to `#4a4a52` at 45%, `#26262b` at 85% and `#1a1a1e` at the rim, with **no red** in it. Walls, floor and ceiling separate by angle to the eye; ambient 1.6 and a 2.6 directional light at (0.35, 0.55, 1) back the Lambert wire view (`engine.ts:434-437`). The material colour per surface group is muted and mixed 40% toward white (`engine.ts:~1118`), so the room reads grey with a hint of material hue. Edges are `LINE 0xededef` (near white), selection `SELECT 0xe0202e` (the same red as `--red`) with a red wash, warnings `WARN 0xf2a93b` (`--warn`), plan-inset clear colour `PANEL 0x0f0f11` at alpha 0.35 (`engine.ts:191-194, 787`). The canvas clears to transparent (`setClearColor(0x000000, 0)`, :739) over the page's `--bg` with a faint red radial (`rgba(224,32,46,0.07)` at 50%/55%).

**Depth cues and glow.** Everything that adds depth is quiet and monochrome: baked ambient occlusion per vertex (`ao.ts`, darkens corners by up to `AO_STRENGTH`, reach a twelfth of the room, computed once), a distance fade of the far side toward the page background (`fade.ts`, `FADE_MAX = 0.6`, `FADE_BG '#09090b'`, off in plan view), a ground plane with a fine grey grid that fades out plus a soft dark footprint shadow (`ground.ts`, "grey, never the red of the sound-level planes"), a glass-case factor so See-through walls are more solid edge-on and clearer face-on (`glass.ts`, capped), a dimensions overlay with measuring lines and mono labels on a `rgba(9,9,11,.78)` plate (`dims.ts`, viewport.css:199-212), and a build-up animation of the room rising floor to ceiling in 2.4 s with a faint warm cut line, "not red" (`build.ts`). The one warm thing is the sources' glow: a soft pool of `GLOW_RGB = [1, 0.22, 0.08]` (a warmer red than selection) peaking at `GLOW_PEAK 0.42`, pulsing over `PULSE_MS 2600` ms, falling off over a tenth of the room's longest side (1.5 to 8 m), still at mid pulse under reduced motion or WebDriver (`glow.ts:1-19`). The overall feel: a dim, matte, low-contrast grey room on warm black, lit from the upper left, red only as a source glow, a selection and a handful of actions. The chrome's matching moves would be: the same upper-left key (highlights on the top and left edge of a bevel, shade on the bottom and right: identity.css already puts the light line at the top and the dark one at the bottom), the same matte grey mids rather than chrome or brushed-metal highlights brighter than `#767680`, and the one warm pool of light reserved for sources: an LED's glow is the same visual idea as the source glow (a soft red halo, `0 0 6px 1px rgba(224,32,46,.85)` in identity.css:110 is already close to it), so LEDs would read as part of the same object if their halo copied the glow's warmth and slow pulse (with the same still-under-reduced-motion rule).

---

## Short list of what the recon could not settle

- Exact line numbers inside `chrome.css` for blocks I summarised with "about" (`.stepbar`, `.scene`, `.props` button rules, `.switch`) are approximate; the rules and values quoted are as read, only the line anchors are loose where marked "~".
- Instance counts are from reading the JSX, not from the running app; none were measured in the DOM.
- I did not run the gates, so "m10/m11 lint already fails on the theme.css blob" is deduced from the two hashes, not observed.
- The detailed contents of `AcousticsPane.tsx`, `ResponseWindow.tsx`, `RunsPane.tsx`, `resultsLayer.ts` and `particles.ts` were not read beyond selectors and class names.
