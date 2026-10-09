# Assemble a portable build for Zeph into OneDrive (syncs to Zeph): app.exe + solvers\ beside it
# (ExeSearch candidate 3: <exe dir>\solvers\<name>), the solver manifest, the user manual (manual\),
# a README. Verifies every
# solver's hash, spps-gpu.exe included, against the repo's solvers/manifest.json before packing.
# -Solvers is a folder holding all five exes at the manifest's hashes (the A5 bed's C:\tmp\nm-solvers-a5
# is one); -Repo the worktree whose manifest and branch name the package carries.
param(
  [string]$Sha = 'unknown',
  [string]$Branch = 'gpu',
  [string]$Repo = 'B:\repos\I-Simpa_Night_Mode\.claude\worktrees\gpu',
  [string]$Solvers = 'C:\tmp\nm-solvers-a5',
  [string]$App = 'C:\tmp\nm-target\release\app.exe'
)
$ErrorActionPreference = 'Stop'
$stamp = Get-Date -Format 'yyyy-MM-dd-HHmm'
$dest = "C:\Users\Burhan\OneDrive\I-Simpa-Night-Mode-builds\nm-$Branch-$stamp-$Sha"
$manifest = Get-Content "$Repo\solvers\manifest.json" -Raw | ConvertFrom-Json

New-Item -ItemType Directory -Force -Path "$dest\solvers" | Out-Null
Copy-Item $App "$dest\app.exe"
foreach ($exe in 'spps.exe','classicalTheory.exe','preprocess.exe','tetgen.exe','spps-gpu.exe') {
  $h = (Get-FileHash -Algorithm SHA256 "$Solvers\$exe").Hash.ToLower()
  $want = $manifest.sha256.$exe
  if (-not $want) { throw "solvers/manifest.json has no row for $exe" }
  if ($h -ne $want) { throw "$exe sha256 $h does not match solvers/manifest.json ($want)" }
  Copy-Item "$Solvers\$exe" "$dest\solvers\$exe"
}
Copy-Item "$Repo\solvers\manifest.json" "$dest\solvers\manifest.json"
# Parity A21: the user manual, readable without the app.
New-Item -ItemType Directory -Force -Path "$dest\manual" | Out-Null
$manualSrc = "$Repo\app\src-tauri\manual"
$pages = @(Get-ChildItem $manualSrc -Filter *.html)
if (-not ($pages | Where-Object Name -eq 'manual.html')) { throw "$manualSrc has no manual.html" }
$pages | ForEach-Object { Copy-Item $_.FullName "$dest\manual\$($_.Name)" }

@"
I-Simpa Night Mode, portable build $stamp ($Branch branch $Sha)

Run:   app.exe            (double-click, or app.exe --project <file.simpa>)
Read:  manual\manual.html (the user manual; Help > User manual opens the copy app.exe carries)
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
"@ | Set-Content -Encoding utf8 "$dest\README.txt"

$total = (Get-ChildItem $dest -Recurse -File | Measure-Object Length -Sum).Sum
'{0}  ({1:N1} MB)' -f $dest, ($total/1MB)
Get-ChildItem $dest -Recurse -File | % { '  {0,10:N0}  {1}' -f $_.Length, $_.FullName.Substring($dest.Length + 1) }
