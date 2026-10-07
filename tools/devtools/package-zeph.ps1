# Assemble a portable build for Zeph into OneDrive (syncs to Zeph): app.exe + solvers\ beside it
# (ExeSearch candidate 3: <exe dir>\solvers\<name>), the solver manifest, a README. Verifies the solver
# hashes against the repo's solvers/manifest.json before packing.
param([string]$Sha = 'unknown')
$ErrorActionPreference = 'Stop'
$stamp = Get-Date -Format 'yyyy-MM-dd-HHmm'
$dest = "C:\Users\Burhan\OneDrive\I-Simpa-Night-Mode-builds\nm-ui-$stamp-$Sha"
$repo = 'B:\repos\I-Simpa_Night_Mode\.claude\worktrees\ui'
$solvers = 'C:\tmp\nm-solvers-timebin\bin'
$manifest = Get-Content "$repo\solvers\manifest.json" -Raw | ConvertFrom-Json

New-Item -ItemType Directory -Force -Path "$dest\solvers" | Out-Null
Copy-Item 'C:\tmp\nm-target\release\app.exe' "$dest\app.exe"
foreach ($exe in 'spps.exe','classicalTheory.exe','preprocess.exe','tetgen.exe') {
  $h = (Get-FileHash -Algorithm SHA256 "$solvers\$exe").Hash.ToLower()
  $want = $manifest.sha256.$exe
  if ($h -ne $want) { throw "$exe sha256 $h does not match solvers/manifest.json ($want)" }
  Copy-Item "$solvers\$exe" "$dest\solvers\$exe"
}
Copy-Item "$repo\solvers\manifest.json" "$dest\solvers\manifest.json"

@"
I-Simpa Night Mode, portable build $stamp (ui branch $Sha)

Run:   app.exe            (double-click, or app.exe --project <file.simpa>)
The solvers (spps, classicalTheory, preprocess, tetgen) are in solvers\ beside app.exe and are found
there; no environment variable is needed. They are the verified build of solvers\manifest.json
(upstream 929a5c8 + patches 0001 time bin, 0002 sparse maps).

Needs on the machine (Windows 10/11 x64):
  - Microsoft Edge WebView2 Runtime (Windows 11 has it; else https://developer.microsoft.com/microsoft-edge/webview2/)
  - Microsoft Visual C++ 2015-2022 x64 Redistributable (the solvers link the CRT dynamically)
  - a GPU with WebGL2 through WebView2: any laptop GPU; the 3D view runs on it.

The app writes runs under <project folder>\runs\. A run of the dense CR4 case (27 bands, 0.1 m plane)
needs about 7 GB of RAM in the solver and 7 GB of disk for its maps.
"@ | Set-Content -Encoding utf8 "$dest\README.txt"

$total = (Get-ChildItem $dest -Recurse -File | Measure-Object Length -Sum).Sum
'{0}  ({1:N1} MB)' -f $dest, ($total/1MB)
Get-ChildItem $dest -Recurse -File | % { '  {0,10:N0}  {1}' -f $_.Length, $_.FullName.Substring($dest.Length + 1) }
