# ISM (Image Source Method) implementations — Zeph Windows sitemap

## Summary
**Shoebox-only implementations:** C:\RoomGUI\ROM has three shoebox ISM engines in Python. No polyhedra ISM currently on Zeph; pyroomacoustics is not installed. Wayverb C++ release exists but unverified. BRAS geometry lives in subdirectories and C:\tmp\bras_surface but AQ_BRAS_ROOT env var not set on this machine.

---

## ISM Implementations

### 1. ROM — image_sources_shoebox
- **Path:** `C:\RoomGUI\ROM\romac\image_source.py` (lines 18–185)
- **Language:** Python (NumPy)
- **Entry point:** `image_sources_shoebox(Lx, Ly, Lz, src, rec, max_order=20, alpha_walls=None, scatter_walls=None, sr=44100, T=2.0, c=343.0, air_absorption=True, humidity=50.0, temperature=20.0)`
- **Polyhedra support:** Shoebox only — axis-aligned rectangular rooms with six rigid or absorptive walls
- **Visibility checks:** None — shoebox guarantees no occlusion; every image mirrors correctly
- **Max order:** Configurable `max_order` (default 20; total reflections across x/y/z axes)
- **Per-band absorption:** Supported via `alpha_walls` dict (keys: 'x0','x1','y0','y1','z0','z1')
- **Output form:** Impulse response ndarray [s], plus list of reflections with time/amplitude/order/distance/position indices (nx,ny,nz)
- **Dependencies:** NumPy, optional air_absorption_coefficient from .material_function
- **Analytical test:** Geometric spreading 1/(4πd); reflection coeff product via wall absorption; air absorption exp(−m·d)

### 2. ROM — image_source_ir_octave_bands
- **Path:** `C:\RoomGUI\ROM\romac\image_source.py` (lines 188–245)
- **Language:** Python (NumPy, SciPy)
- **Entry point:** `image_source_ir_octave_bands(Lx, Ly, Lz, src, rec, alpha_per_band, scatter_per_band=None, max_order=20, sr=44100, T=2.0, c=343.0, humidity=50.0, temperature=20.0)`
- **Polyhedra support:** Shoebox only
- **Visibility checks:** None
- **Max order:** Configurable per band
- **Per-band absorption:** Full per-band support; maps octave center freq → wall absorption dict; bandpass filters output via Butterworth (order 4, 1/√2 octave bw)
- **Output form:** Full-bandwidth IR (sum of filtered per-band IRs)
- **Dependencies:** NumPy, SciPy (butter, sosfilt), material_function.air_absorption_coefficient
- **Analytical test:** Each band computed independently, summed with individual air absorption exp(−m_f·t·c) per frequency

### 3. ROM — ism_spectral
- **Path:** `C:\RoomGUI\ROM\romac\ism_spectral.py` (lines 23–80+)
- **Language:** Python (NumPy, SciPy)
- **Entry point:** `ism_spectral(Lx, Ly, Lz, src, rec, materials, max_order=30, sr=44100, T=3.5, c=343.0, humidity=50.0, temperature=20.0, n_filt=64)`
- **Polyhedra support:** Shoebox only
- **Visibility checks:** None
- **Max order:** Configurable (default 30)
- **Per-band absorption:** Supported via MaterialFunction objects (cumulative spectral absorption along path)
- **Output form:** IR convolved with FIR filters representing cumulative material spectral response per reflection path
- **Dependencies:** NumPy, SciPy (firwin, fftconvolve), materials dict must provide MaterialFunction objects
- **Analytical test:** Each image source convolved with short FIR (n_filt=64 taps) representing frequency-dependent absorption cumulative

### 4. ROM — hybrid_ism_diffuse
- **Path:** `C:\RoomGUI\ROM\romac\image_source.py` (lines 248–250+, incomplete in extract)
- **Language:** Python
- **Entry point:** `hybrid_ism_diffuse(Lx, Ly, Lz, src, rec, alpha_walls, scatter_walls, max_order=50, sr=44100, T=3.5, c=343.0)`
- **Polyhedra support:** Shoebox only
- **Max order:** Configurable (default 50)
- **Per-band absorption:** Via alpha_walls, scatter_walls dicts
- **Role:** Hybrid specular + diffuse; blends ISM early with diffuse tails
- **Status:** Method signature known; implementation body not extracted

### 5. engines.py — run_ism
- **Path:** `C:\RoomGUI\ROM\engines.py` (lines 488–527)
- **Language:** Python (NumPy)
- **Entry point:** `run_ism(room, src, rec, sr=44100, duration=2.0, max_order=15, **kw)`
- **Polyhedra support:** Shoebox (via room.Lx, room.Ly, room.Lz; assumes rectangular)
- **Visibility checks:** None
- **Max order:** Configurable (default 15; enforced on total order, not per-axis)
- **Per-band absorption:** Single frequency (mid-band 500 Hz) via room.alpha_at(wall, f_mid); uses (-1)^order sign convention for pressure reflections
- **Output form:** Result object with normalized IR + metrics + runtime + image count
- **Dependencies:** room object with .c, .Lx/.Ly/.Lz, .alpha_at(wall, freq) method
- **Analytical test:** Pressure amplitude (-1)^order; reflection coeff R product per wall pair; normalization to max

### 6. Wayverb (C++ release)
- **Path:** `C:\RoomGUI\wayverb_release` and `C:\RoomGUI\wayverb_release_zip`
- **Language:** C++
- **Status:** Binary release, source not inspected on Zeph (project GitHub: Amulet-Audio/Wayverb, ISM engine described in docs as supporting arbitrary waveguides)
- **Polyhedra support:** Unknown on Zeph; upstream Wayverb includes arbitrary mesh ISM
- **Note:** Architectural plans in README but implementation details not verified here

---

## BRAS Geometry (CR2/CR3/CR4 with per-surface materials)

**Locations:**
- **I-Simpa FDTD examples:** `C:\repos\Room-Acoustics-Engine\I-Simpa FDTD\app\src-tauri\examples\` — `.simpa` fixture files (bras_cr2.simpa, bras_cr3.simpa, bras_cr4.simpa, elmia_corrected.ply)
- **Surface descriptions:** `C:\tmp\bras_surface\` — zip archive with per-material absorption per band
- **Environment variable:** `$env:AQ_BRAS_ROOT` not set on Zeph; intended to point to measurement and derived data (only configured on Grace: B:\data\BRAS or similar)
- **Mesh format:** `.ply` (polygonal) with per-triangle material labels; `.simpa` XML config with octave-band absorption lookup

**Current state:** BRAS CR2/CR3/CR4 are non-shoebox dodecahedron/polyhedral rooms; no ISM engine on Zeph currently loads them (ROM ISM is shoebox-only). CR4 tested in 2026-10-07 FDTD probe; mesh self-intersection in raw elmia.ply documented, corrected version at testdata/elmia_corrected.ply.

---

## Gaps

- **No arbitrary-polyhedra ISM on Zeph:** All shoebox. Wayverb exists but untested.
- **pyroomacoustics not installed:** Would provide ism.inverse() for arbitrary convex polyhedra if present.
- **No visibility/occlusion checks:** ROM ISM assumes unobstructed images (valid for shoebox, fails for BRAS).
- **BRAS geometry not integrated with ISM:** AQ_BRAS_ROOT env var unset; CR2/CR3/CR4 geometry available but no solver currently pairs ISM with BRAS per-surface materials.

---

Generated: 2026-10-08 22:47 (Zeph, Windows)
