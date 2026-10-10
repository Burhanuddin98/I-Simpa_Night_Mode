# I-Simpa Night Mode v1 (draft)

The version number is set at release (decision 86). This draft lists what the release carries and what it does not.

A new desktop interface for room-acoustics simulation with the solvers of
[I-Simpa](https://github.com/Universite-Gustave-Eiffel/I-Simpa) (Université Gustave Eiffel): SPPS particle
tracing and TCR classical theory, plus this repository's SPPS port to NVIDIA GPUs. Windows 10 and 11, x64. GPL-3.0.

## Install

Download `I-Simpa-Night-Mode-<version>-<commit>-win64-setup.exe` and its `.sha256`, check the hash
(`Get-FileHash` in PowerShell), run it. It installs for the current user, needs no administrator rights, and
opens `.simpa` files. It is not code-signed: SmartScreen asks once (More info, Run anyway).

## What it does

- **Start:** example rooms (Elmia, an industrial hall, BRAS CR1-CR4), upstream's tutorials 1-3 with what
  upstream expects and what this build gives, a box room of any size, PLY/OBJ/STL and upstream `.proj` import,
  recent projects, crash recovery, drag and drop, Close project and Home.
- **Geometry:** a model check before any run (watertight, self-intersections, flipped normals, open edges,
  units), repair to a new file, replace the model keeping its groups, surface groups, sound-level planes at any
  angle, fitting zones.
- **Materials:** absorption, scattering, transmission and the reflection law per band; upstream's reference
  library; spreadsheet copy and paste.
- **Sources and receivers:** placed on any face; spectra with dB(A) entry and a project library; directivity;
  receiver orientation and groups.
- **Simulate:** SPPS, TCR or SPPS on the GPU; every solver setting; memory and run-time forecasts; a checklist
  that blocks a run that cannot succeed; background runs with a live console; a job list.
- **Results:** SPL, G, EDT, T15, T20, T30, C50, C80, D50, Ts, STI and dB(A) to ISO 3382-1 and IEC 60268-16,
  with chosen decay ranges and C/D limits; sound maps and T30/EDT/C80/D50 maps per face; difference maps;
  particle playback; Listen: dry clips through the room, with a Dry/Room switch; CSV, JSON, PNG and WAV export.
- **Every value shown passes a check;** a value that does not is shown as REFUSED with its reason.

## What is verified

- The solvers are upstream's at tag `v1.4.0_snapshot_14_01_2026` with two patches that change how map data is
  stored. With them, SPPS output is byte-identical to upstream's own build (39 of 39 files), and every TCR value
  is identical (15 files): `docs/investigations/2026-10-06-third-octave-bug/BED-0001.md`, `BED-0002.md`,
  `BED-TCR.md`.
- Upstream's tutorials 2 and 3 reproduce upstream's figures: `docs/investigations/2026-10-09-tutorials/`.
- Each new result parameter has a test bed against an analytical reference; the parameter maps have a
  consistency bed against the point receivers.

## Not in this version

Stereo or binaural listening (Listen is mono, from the energy echogram); pausing a run; the command palette,
section view and measure tool; upstream's Python console; languages other than English; an update check.
Fitting zones do not mesh in some non-convex halls (the run is refused with the reason). The full list is
in the manual (Help, User manual) and `docs/v1.1-backlog.md`.
