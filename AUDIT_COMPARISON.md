> **SUPERSEDED 2026-09-08 — historical record of the 2026-04-04 audit.**
> Its twenty-item priority list was largely worked off *within that same session*: 14 of 20 items,
> and all seven marked CRITICAL, have code in the tree today. The feature tables below are stale
> in the same way. **Do not quote this file for current state.**
> What is actually still missing: [docs/ui-parity-backlog.md](docs/ui-parity-backlog.md).

# I-Simpa Custom GUI vs Original — Feature Comparison Audit

## Legend
- Y = Implemented and working
- P = Partially implemented / stubbed
- N = Not implemented
- B = Broken / needs fix

---

## 1. PROJECT MANAGEMENT

| Feature | Original | Custom | Gap |
|---------|----------|--------|-----|
| New project | Y | Y | - |
| Open project | Y | Y | - |
| Save / Save As | Y | Y | - |
| Save copy | Y | N | Missing |
| Recent files | Y | Y | - |
| Undo/Redo | Y | Y | - |
| Preferences dialog | Y | N | Missing |
| Language selection | Y | N | Missing |
| Drag-drop file open | Y | N | Missing |

## 2. SCENE TREE

| Feature | Original | Custom | Gap |
|---------|----------|--------|-----|
| Scene tab (geometry tree) | Y | Y | - |
| Calculation tab (solver config tree) | Y | N | Missing — config is in Properties panel |
| Results tab (result browser tree) | Y | N | Missing — results are in Results panel |
| User Preferences tab | Y | N | Missing |
| Multi-column tree | Y | N | Custom uses flat outliner |
| Element rename in tree | Y | Y | - |
| Context menus per element | Y | Y | - |

## 3. GEOMETRY

| Feature | Original | Custom | Gap |
|---------|----------|--------|-----|
| Import PLY | Y | Y | - |
| Import OBJ | N | Y | Custom has MORE |
| Import STL | N | Y | Custom has MORE |
| Import 3DS | N | Y | Custom has MORE |
| Create box room | Y | Y | - |
| Face selection in 3D | Y | Y | - |
| Multi-face selection | Y | Y (Shift+Click) | - |
| Group merge | Y | Y | - |
| Group delete | Y | Y | - |
| Normal flip | Y | Y | - |
| Surface group from selection | Y | N | Missing |
| Vertex editing | Y | N | Missing |

## 4. MATERIALS

| Feature | Original | Custom | Gap |
|---------|----------|--------|-----|
| Built-in material library | Y | Y (11 materials) | - |
| Per-band absorption editing | Y | Y (27 bands) | - |
| Per-band diffusion editing | Y | Y | - |
| Diffusion law per band | Y | Y (6 laws) | - |
| Material assignment to groups | Y | Y | - |
| Drag-drop assignment | Y | N | Custom uses click-assign mode |
| Material categories/groups | Y | N | Missing |
| Application vs User materials | Y | N | Single flat list |
| Import/Export XML | Y | Y | - |
| Import appconst.xml | N | Y | Custom has MORE |
| Visual material cards | N | Y | Custom has MORE |

## 5. SOURCES

| Feature | Original | Custom | Gap |
|---------|----------|--------|-----|
| Point sources | Y | Y | - |
| Position editing | Y | Y | - |
| Power spectrum editing | Y | Y (27 bands) | - |
| Directivity patterns | Y (library) | P (5 basic types, no .dir files) | Partial |
| Source delay | Y | Y | - |
| Enable/disable toggle | Y | Y | - |
| Custom directivity files | Y | N | Missing |
| Directivity visualization | Y | N | Missing |

## 6. RECEIVERS

| Feature | Original | Custom | Gap |
|---------|----------|--------|-----|
| Punctual receivers | Y | Y | - |
| Surface receivers (scene type) | Y | P (struct exists, not fully wired) | Partial |
| Surface receivers (plane type) | Y | Y | - |
| Cutting plane receivers | Y | P (clipping plane exists, not receiver) | Partial |
| Background noise per receiver | Y | P (global only, not per-receiver) | Partial |
| Receiver orientation | Y | Y | - |

## 7. ENCUMBRANCES / FITTINGS

| Feature | Original | Custom | Gap |
|---------|----------|--------|-----|
| Cuboid fitting zones | Y | Y | - |
| Face-group fitting zones | Y | P (struct exists, not fully wired) | Partial |
| Per-band absorption | Y | Y | - |
| Per-band mean free path | Y | Y | - |
| Diffusion law | Y | Y | - |
| Enable/disable | Y | Y | - |
| 3D wireframe display | Y | Y | - |
| Config.xml export | Y | Y | - |

## 8. ENVIRONMENT

| Feature | Original | Custom | Gap |
|---------|----------|--------|-----|
| Temperature | Y | Y | - |
| Humidity | Y | Y | - |
| Pressure | Y | Y | - |
| Ground roughness (z0) | Y | Y | - |
| Meteo profiles (5 types) | Y | Y | - |
| Ground type presets | Y | N | Missing |
| Custom celerity gradient | Y | Y (aLog, bLin) | - |

## 9. SOLVER CONFIG

| Feature | Original | Custom | Gap |
|---------|----------|--------|-----|
| SPPS full parameter set | Y | Y | - |
| TCR config | Y | Y (basic) | - |
| TLM solver | Y | N | Missing (separate solver) |
| Frequency band selector | Y | Y (27 bands) | - |
| Octave/third-octave presets | Y | N | Missing — manual selection only |
| Mesh quality parameters | Y | N | Hardcoded TetGen flags |
| Mesh preprocess toggle | Y | N | Missing |
| Volume constraint | Y | N | Missing |

## 10. MESH GENERATION

| Feature | Original | Custom | Gap |
|---------|----------|--------|-----|
| TetGen integration | Y | Y | - |
| Preprocess (mesh repair) | Y | P (calls preprocess.exe if found) | - |
| Volume constraint | Y | N | Hardcoded -pq1.5 |
| Quality ratio | Y | N | Hardcoded |
| Mesh preview in viewport | Y | N | Missing |
| Neighbor computation fallback | N | Y | Custom has MORE |
| Per-group .cbin export | Y | Y (fixed in this session) | - |

## 11. 3D VIEWPORT

| Feature | Original | Custom | Gap |
|---------|----------|--------|-----|
| Face rendering | Y | Y | - |
| Wireframe overlay | Y | Y | - |
| Camera orbit | Y | Y | - |
| Camera pan | Y | Y | - |
| Camera zoom | Y | Y | - |
| First-person WASD fly | Y | Y | - |
| View presets (Top/Front/Right) | Y | Y | - |
| **Face display: Outside/Inside/None** | Y | N | **Missing** |
| **Material color mode (Original/Acoustic)** | Y | N | **Missing** |
| **Wireframe mode: All/Contour/None** | Y | P (All or None only) | **Partial** |
| Grid display (XY/XZ/YZ) | Y | Y (ground plane only) | Partial |
| Axis arrows (RGB colored) | Y | Y (Y-axis only) | Partial |
| Cutting plane | Y | Y | - |
| Selection highlighting | Y | Y | - |
| Click-to-deselect | Y | Y (fixed in this session) | - |
| Source/receiver 3D icons | Y | Y (glowing spheres) | - |
| Encumbrance box display | Y | Y | - |
| Measurement tool | Y | Y | - |
| Screenshot export | Y | Y | - |

## 12. RESULT VISUALIZATION — THE BIG GAPS

| Feature | Original | Custom | Gap |
|---------|----------|--------|-----|
| **Surface colormap (SPL on surfaces)** | Y (smooth, palette-based) | B (basic .csbin load, no palette) | **Needs rewrite** |
| **Particle points mode** | Y (2px colored points) | Y (rainbow glowing points) | Different style |
| **Particle trail/ray mode** | Y (line segments per particle) | Y (rainbow fading trails) | Different style |
| **Intensity vector arrows** | Y (blue 3px arrows at receivers) | N | **Missing** |
| **Iso-contour lines** | Y (white lines at constant SPL) | N | **Missing** |
| **Color legend bar** | Y (gradient + labels + units) | N | **Missing** |
| **Time-step modes (instant/cumul/sum)** | Y | N (particles only) | **Missing for surface maps** |
| **Acoustic parameter maps (RT/EDT/C50/D50/TS/STI)** | Y (computed from SPL) | N | **Missing** |
| **Result comparison/subtraction** | Y (wizard) | N | **Missing** |
| **Per-source echogram** | Y | N | **Missing** |
| Play/Pause/Stop animation | Y | Y | - |
| Frame stepping (prev/next) | Y | N | Only slider |
| Speed control | Y (refresh rate ms) | P (hardcoded 0.25x) | Partial |
| Frequency band result plot | Y | Y | - |
| Schroeder curve | Y | Y | - |
| Acoustic parameters (RT60/EDT/C80/D50/Ts) | Y | Y | - |

## 13. ANIMATION CONTROLS

| Feature | Original | Custom | Gap |
|---------|----------|--------|-----|
| Play/Pause | Y | Y | - |
| Stop (reset) | Y | Y | - |
| Previous/Next step buttons | Y | N | **Missing — only slider** |
| Speed adjustment (refresh ms) | Y | N | **Hardcoded speed** |
| **Animation toolbar** | Y | N | **Missing — controls in Results panel** |

## 14. PYTHON SCRIPTING

| Feature | Original | Custom | Gap |
|---------|----------|--------|-----|
| Python console | Y | Y | - |
| Full embedded Python | Y (in-process) | N (subprocess) | Different architecture |
| Project tree API | Y (IDEVENT system) | P (JSON export) | Partial |
| Element manipulation | Y | N | Missing |
| Event registration | Y | N | Missing |
| Context menu integration | Y | N | Missing |
| Script file execution | Y | Y | - |

## 15. EXPORT / REPORTING

| Feature | Original | Custom | Gap |
|---------|----------|--------|-----|
| Save project | Y | Y | - |
| Export scene (.cbin) | Y | Y | - |
| Screenshot | Y | Y | - |
| Material import/export | Y | Y | - |
| CSV result export | N | Y | Custom has MORE |
| **Result comparison wizard** | Y | N | **Missing** |
| **Console export to file** | Y | Y | - |
| **Graph export** | Y | N | **Missing** |

---

## PRIORITY FIXES (highest impact first)

### CRITICAL — Must have for parity with original

1. **Color legend bar** — gradient + min/max/avg labels + units. Without this, colormaps are unreadable.
2. **Intensity vector arrows** — blue arrows at receiver positions showing sound direction. Original renders these as 3px GL_LINES.
3. **Iso-contour lines** — white lines on surfaces at constant SPL levels. Standard acoustic visualization.
4. **Surface colormap improvements** — smooth vertex coloring with configurable palette (not just jet), transparency control, front/back face toggle.
5. **Time-step animation for surface maps** — instant/cumulative/sum modes.
6. **Acoustic parameter surface maps** — compute RT/EDT/C50/D50/TS/STI from SPL data and display as colormaps.
7. **Animation prev/next step buttons + speed slider** — basic playback UX.

### HIGH — Expected by users

8. Remove solid cubes heatmap (cluttered, not in original)
9. Mesh quality parameters UI (volume constraint, radius/edge ratio)
10. Frequency band preset buttons (All / Octave / Third-octave)
11. Ground type presets dropdown
12. Per-source echogram
13. Result comparison/subtraction tool

### MEDIUM — Nice to have

14. Face display modes (outside/inside/none)
15. Material color mode (original/acoustic)
16. Contour-only wireframe mode
17. Full XY/XZ/YZ grid toggles
18. Proper axis arrows (RGB XYZ)
19. Animation speed slider
20. Directivity pattern file loading
