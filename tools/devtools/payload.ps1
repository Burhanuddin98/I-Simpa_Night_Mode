# What ships beside app.exe, in one place. Dot-source it and call Copy-NightModePayload: the portable
# package (package-zeph.ps1) and the installer (tools\installer\build-installer.ps1, M13) both stage
# their files with it, so the two cannot ship different sets.
#
#   app.exe
#   solvers\   the five solvers at solvers/manifest.json's code sha256 (FindSolverExe's layout, ExeSearch
#              candidate 3: <exe dir>\solvers\<name>), the manifest, and the C runtime they link
#              (decision 85: Microsoft.VC143.CRT and .OpenMP from Visual Studio's Redist, app-local)
#   manual\    the user manual and the tutorials' pages (parity A21, A43)
#   tutorials\ upstream's tutorials 1 to 3 as projects (A43)
#   examples\  the example rooms app.exe also carries (examples.rs), with their ATTRIBUTION.md
#   LICENSE.txt (GPL-3.0), TETGEN-LICENSE.txt (TetGen's AGPL-3.0), THIRD-PARTY-NOTICES.txt (A23)
#
# Every exe staged is then read with dumpbin: each DLL it imports must be beside it or a Windows
# system DLL, and a Visual C++ runtime DLL never counts as a system one (a machine without the
# redistributable has none), so a solver rebuilt against a new runtime DLL fails here, not on a
# user's machine.

. "$PSScriptRoot\..\..\solvers\pe-fingerprint.ps1"

$script:SolverExes ='spps.exe', 'classicalTheory.exe', 'preprocess.exe', 'tetgen.exe', 'spps-gpu.exe'
# Decision 85: the runtime DLLs the solvers import (dumpbin /dependents, 2026-10-09), from the
# Redist folder of the Visual Studio that builds them.
$script:CrtDlls = @{ 'Microsoft.VC143.CRT' = @('msvcp140.dll', 'vcruntime140.dll', 'vcruntime140_1.dll'); 'Microsoft.VC143.OpenMP' = @('vcomp140.dll') }

function Get-VsInstall {
  $vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
  if (-not (Test-Path $vswhere)) { throw "vswhere.exe not found: Visual Studio 2022 is needed for the C runtime (decision 85)" }
  $vs = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
  if (-not $vs) { throw 'no Visual Studio with the x64 C++ tools' }
  $vs
}

function Get-CrtRedistDir {
  $vs = Get-VsInstall
  $ver = (Get-Content "$vs\VC\Auxiliary\Build\Microsoft.VCRedistVersion.default.txt" -Raw).Trim()
  $dir = "$vs\VC\Redist\MSVC\$ver\x64"
  if (-not (Test-Path $dir)) { throw "the C runtime's Redist folder is missing: $dir" }
  $dir
}

function Get-Dumpbin {
  $vs = Get-VsInstall
  $tools = Get-ChildItem "$vs\VC\Tools\MSVC" -Directory | Sort-Object { [version]$_.Name } -Descending | Select-Object -First 1
  $db = "$($tools.FullName)\bin\Hostx64\x64\dumpbin.exe"
  if (-not (Test-Path $db)) { throw "dumpbin.exe not found: $db" }
  $db
}

# Throws unless every DLL each exe under $Root imports is beside it or a system DLL (see the header).
function Test-PayloadImports([string]$Root) {
  $db = Get-Dumpbin
  $sys = "$env:SystemRoot\System32"
  $bad = @()
  foreach ($exe in Get-ChildItem $Root -Recurse -Filter *.exe) {
    $dlls = & $db /nologo /dependents $exe.FullName | Where-Object { $_ -match '^\s+(\S+\.dll)\s*$' } | ForEach-Object { $Matches[1] }
    foreach ($d in $dlls) {
      if (Test-Path (Join-Path $exe.DirectoryName $d)) { continue }
      if ($d -match '^(msvcp|vcruntime|vcomp|concrt|vccorlib)\d') { $bad += "$($exe.Name) imports $d, which is not beside it"; continue }
      if ($d -match '^(api|ext)-ms-win-') { continue }
      if (-not (Test-Path (Join-Path $sys $d))) { $bad += "$($exe.Name) imports $d, neither beside it nor in $sys" }
    }
  }
  if ($bad) { throw ("the payload would not start on a clean machine:`n  " + ($bad -join "`n  ")) }
}

# Whether a solver is the verified build: its code sha256 (solvers/pe-fingerprint.ps1, the sha256 with
# the link-time fields zeroed) equals solvers/manifest.json's, as the gates and the app itself hold
# it. A rebuild from the same source with the same compiler passes; its raw sha256 differs only by the
# link time. Returns { name; sha256; code_sha256; want; verified }.
function Test-SolverVerified($Manifest, [string]$Exe) {
  $name = Split-Path -Leaf $Exe
  $want = $Manifest.code_sha256.$name
  if (-not $want) { throw "solvers/manifest.json has no code_sha256 row for $name" }
  $code = Get-CodeSha256 $Exe
  [ordered]@{ name = $name; sha256 = (Get-FileHash -Algorithm SHA256 $Exe).Hash.ToLower(); code_sha256 = $code; want = $want; verified = ($code -eq $want) }
}

function Copy-NightModePayload {
  param(
    [Parameter(Mandatory)][string]$Dest,
    [Parameter(Mandatory)][string]$Repo,
    [Parameter(Mandatory)][string]$Solvers,
    [Parameter(Mandatory)][string]$App,
    # Stage solvers whose code sha256 is not the manifest's (a build from source with another compiler);
    # without it such a solver stops the staging.
    [switch]$AllowUnverifiedSolvers
  )
  $manifest = Get-Content "$Repo\solvers\manifest.json" -Raw | ConvertFrom-Json
  New-Item -ItemType Directory -Force -Path "$Dest\solvers" | Out-Null
  Copy-Item $App "$Dest\app.exe"
  foreach ($exe in $script:SolverExes) {
    $code = Test-SolverVerified -Manifest $manifest -Exe "$Solvers\$exe"
    if (-not $code.verified) {
      $msg = "$exe code sha256 $($code.code_sha256) does not match solvers/manifest.json ($($code.want)): not the verified build"
      if (-not $AllowUnverifiedSolvers) { throw $msg }
      Write-Warning "$msg; staged anyway (-AllowUnverifiedSolvers), and the app will say Results unverified"
    }
    Copy-Item "$Solvers\$exe" "$Dest\solvers\$exe"
  }
  Copy-Item "$Repo\solvers\manifest.json" "$Dest\solvers\manifest.json"
  # Decision 85: the C runtime beside the solvers that import it.
  $redist = Get-CrtRedistDir
  foreach ($folder in $script:CrtDlls.Keys) {
    foreach ($dll in $script:CrtDlls[$folder]) { Copy-Item "$redist\$folder\$dll" "$Dest\solvers\$dll" }
  }
  # Parity A21: the user manual, readable without the app.
  New-Item -ItemType Directory -Force -Path "$Dest\manual" | Out-Null
  $manualSrc = "$Repo\app\src-tauri\manual"
  $pages = @(Get-ChildItem $manualSrc -Filter *.html)
  if (-not ($pages | Where-Object Name -eq 'manual.html')) { throw "$manualSrc has no manual.html" }
  $pages | ForEach-Object { Copy-Item $_.FullName "$Dest\manual\$($_.Name)" }
  # Parity A43: upstream's tutorials 1 to 3 travel inside app.exe (examples.rs) with their pages in manual\; the
  # projects are copied beside it too, as upstream's installer ships doc\tutorial, to open with File > Open.
  New-Item -ItemType Directory -Force -Path "$Dest\tutorials" | Out-Null
  foreach ($n in 1, 2, 3) {
    $proj = "$Repo\app\src-tauri\examples\tutorial_$n.simpa"
    if (-not (Test-Path $proj)) { throw "$proj is missing" }
    if (-not (Test-Path "$Dest\manual\tutorial-$n.html")) { throw "manual\tutorial-$n.html is missing" }
    Copy-Item $proj "$Dest\tutorials\tutorial_$n.simpa"
  }
  # The example rooms the landing page offers (examples.rs), as files to open with File > Open, with
  # their sources and licences.
  New-Item -ItemType Directory -Force -Path "$Dest\examples" | Out-Null
  $rooms = @(Get-ChildItem "$Repo\app\src-tauri\examples" -Filter *.simpa | Where-Object Name -notlike 'tutorial_*')
  if ($rooms.Count -eq 0) { throw 'app\src-tauri\examples has no example room' }
  $rooms | ForEach-Object { Copy-Item $_.FullName "$Dest\examples\$($_.Name)" }
  Copy-Item "$Repo\app\src-tauri\examples\ATTRIBUTION.md" "$Dest\examples\ATTRIBUTION.md"
  # Parity A23: the app's licence and the third-party notices beside it (About opens the same texts);
  # TetGen's own licence (the AGPL-3.0, CLAUDE.md) as its own file too.
  & python "$Repo\tools\devtools\third_party_notices.py" --check
  if ($LASTEXITCODE -ne 0) { throw 'THIRD-PARTY-NOTICES.txt is stale: run tools\devtools\third_party_notices.py' }
  Copy-Item "$Repo\LICENSE" "$Dest\LICENSE.txt"
  Copy-Item "$Repo\third_party\tetgen-1.5.0\LICENSE" "$Dest\TETGEN-LICENSE.txt"
  Copy-Item "$Repo\app\src-tauri\about\THIRD-PARTY-NOTICES.txt" "$Dest\THIRD-PARTY-NOTICES.txt"
  Test-PayloadImports $Dest
}
