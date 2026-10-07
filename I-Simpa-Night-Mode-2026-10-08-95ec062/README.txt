I-Simpa Night Mode, portable build 2026-10-08-0157 (gpu branch 95ec062)

Run:   app.exe            (double-click, or app.exe --project <file.simpa>)
The solvers (spps, spps-gpu, classicalTheory, preprocess, tetgen) are in solvers\ beside app.exe and
are found there; no environment variable is needed. They are the verified build of
solvers\manifest.json (upstream 929a5c8 + patches 0001 time bin, 0002 sparse maps; spps-gpu from
this repo's solvers\spps-gpu).

SPPS on the GPU: the Simulate step's solver choice offers "SPPS on the GPU" when the app finds a CUDA
device (it asks solvers\spps-gpu.exe --probe at start), naming the card; when it finds none, the entry
is greyed with the reason. spps-gpu.exe holds code for Turing (RTX 2060, sm_75) and Blackwell (RTX 5070,
sm_120) and is built against CUDA 13.2: it needs an NVIDIA driver of the R580 series or newer. An
older driver gives "no CUDA device" in the greyed entry; update the driver from nvidia.com and start
the app again. A project with fittings, a stratified atmosphere or measured directivity balloons is
refused by the GPU solver with its reason shown under the run; run those on SPPS.

Needs on the machine (Windows 10/11 x64):
  - Microsoft Edge WebView2 Runtime (Windows 11 has it; else https://developer.microsoft.com/microsoft-edge/webview2/)
  - Microsoft Visual C++ 2015-2022 x64 Redistributable (the solvers link the CRT dynamically)
  - a GPU with WebGPU or WebGL2 through WebView2: any laptop GPU; the 3D view runs on it.

The app writes runs under <project folder>\runs\. A run of the dense CR4 case (27 bands, 0.1 m plane)
needs about 7 GB of RAM in the solver and 7 GB of disk for its maps.
