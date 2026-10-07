# I-Simpa Night Mode: Windows build for testing (2026-10-08)

**Download:** `I-Simpa-Night-Mode-2026-10-08-95ec062-win64.zip` on this branch (10.0 MB), sha256
`c37ad088ad9f0647e078434bdb808196831c1f3ed40a39b429de490301d4da72`. The same files, unzipped, are in the folder
`I-Simpa-Night-Mode-2026-10-08-95ec062/` (`app.exe`, and `solvers/` with `spps-gpu.exe` and the other four), for a
single file: open it and use "Download raw file". Keep `app.exe` and `solvers\` side by side.

Built from commit `95ec062` (the `rebuild` branch). This branch holds only the built package, kept apart from the
code's history; it is a stop-gap until the build is published as a GitHub Release. It replaces the 2026-10-07 build
(2e6345a): this one adds auralization (Results step: Listen, a synthesised impulse response per source or summed,
Play and Save at 48 kHz, three bundled dry recordings) and the run-quality advisor's closing fixes.

## Run it

1. Unzip anywhere (a folder you can write to).
2. Double-click `app.exe`. The solvers are in `solvers\` beside it and are found there; nothing to set.

## What it needs

- Windows 10 or 11, x64.
- Microsoft Edge WebView2 Runtime (Windows 11 has it).
- Microsoft Visual C++ 2015-2022 x64 Redistributable (the solvers link it).
- For **SPPS on the GPU**: an NVIDIA GPU of the Turing generation or newer (RTX 20xx and up; the exe holds code for
  sm_75 and sm_120) and an NVIDIA driver of the **R580 series or newer** (it is built against CUDA 13.2). With an
  older driver, or no NVIDIA GPU, the "SPPS on the GPU" entry is greyed with the reason; SPPS on the CPU still runs.

## What to check on a new machine

- The Simulate step's solver choice: does "SPPS on the GPU" name your card?
- Run the CR4 example on the GPU (Grace, RTX 5070: about 1.5 s at 1 kHz, 2 x 300 k particles, 10 s).
- Does the 3D view draw (WebGPU, falling back to WebGL2)?
- Results step, Listen: the impulse response and a clip through the room play and save.

## The solvers in `solvers\`

Checked against `solvers/manifest.json` (in the zip and in the repository) before every run; a solver that does not
match is refused. spps, classicalTheory, preprocess and tetgen are I-Simpa's (upstream 929a5c8 plus this repository's
`patches/`); spps-gpu is this repository's `solvers/spps-gpu` (version 0.2.1). Licences: GPL-3.0 for the app and
I-Simpa's solvers; TetGen under the AGPL-3.0; the bundled clips are public domain (LibriVox) and CC0 (VCSL), with
their provenance in the repository (`app/src-tauri/examples/clips/`). Source: this repository, branch `rebuild`.
