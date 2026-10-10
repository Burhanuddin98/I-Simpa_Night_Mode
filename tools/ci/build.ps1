# Builds I-Simpa Night Mode from source, end to end, into the per-user Windows installer. The one build
# path: README.md's "Build from source" runs this script, and so does .github/workflows/build.yml.
#
#   1. The build tools it can fetch itself, each checked: NSIS 3.11 and SWIG 4.4.1 (sha256 pinned below),
#      Microsoft's WebView2 bootstrapper (its Authenticode signature, in build-installer.ps1).
#   2. Upstream I-Simpa at the pinned tag, cloned --depth 1, its commit held to solvers/manifest.json's.
#   3. spps, classicalTheory, preprocess (upstream + patches\) and TetGen 1.5.0 (third_party\):
#      solvers\build.ps1. spps-gpu (this repository's CUDA port): solvers\spps-gpu\build.cmd.
#   4. Each solver's code sha256 against solvers/manifest.json (the sha256 with the link time zeroed;
#      solvers\pe-fingerprint.ps1). Equal means the verified build, the one the gates ran; a different
#      compiler gives other code, which stops the build unless -AllowUnverifiedSolvers is given (the app
#      then refuses to run that solver: its run checklist shows "the verified build of the solvers"
#      Blocked). spps-gpu is built without -lineinfo, so its code holds no folder path and any
#      checkout and CUDA 13.2 folder give the verified code.
#   5. The app: npm ci, then the Tauri release build (UI bundled into app.exe), CARGO_TARGET_DIR honoured.
#   6. The installer: tools\installer\build-installer.ps1, into -Out\<version>-<commit>-<stamp>\.
#
# Needs on PATH: git, cmake (the verified build used 4.3.2), python 3 (with its headers, as python.org's installer gives),
# node 24 with npm, rustup/cargo (rust-toolchain.toml picks the toolchain), CUDA 13.2's nvcc (or
# -FetchCuda); and Visual Studio 2022 with the "Desktop development with C++" workload. Network: GitHub
# (upstream, Boost through upstream's CPM), npm, crates.io, SourceForge (NSIS, SWIG), go.microsoft.com
# (WebView2), developer.download.nvidia.com (-FetchCuda).
#
# Run (Windows PowerShell 5.1, from anywhere):
#   powershell -ExecutionPolicy Bypass -File tools\ci\build.ps1 [-Work <dir>] [-Out <dir>]
#     [-CpmCache <dir>] [-Upstream <checkout at the tag>] [-FetchCuda] [-AllowUnverifiedSolvers]
# Defaults: -Work <repo>\target\ci, -Out <Work>\installer, CARGO_TARGET_DIR or <Work>\cargo-target.
param(
  [string]$Work = '',
  [string]$Out = '',
  [string]$CpmCache = '',
  [string]$Upstream = '',
  # Fetch CUDA 13.2.1's compiler from NVIDIA's redistributable archives instead of using an installed one.
  [switch]$FetchCuda,
  [switch]$AllowUnverifiedSolvers
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'   # Invoke-WebRequest's progress bar slows it tenfold under 5.1
$started = Get-Date
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
if (-not $Work) { $Work = Join-Path $repo 'target\ci' }
$Work = [IO.Path]::GetFullPath($Work)
if (-not $Out) { $Out = Join-Path $Work 'installer' }
if (-not $CpmCache) { $CpmCache = Join-Path $Work 'cpm-cache' }
$cargoTarget = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $Work 'cargo-target' }
$tools = Join-Path $Work 'tools'
$logs = Join-Path $Work 'logs'
New-Item -ItemType Directory -Force $Work, $Out, $tools, $logs | Out-Null

$UpstreamUrl = 'https://github.com/Universite-Gustave-Eiffel/I-Simpa.git'
$UpstreamTag = 'v1.4.0_snapshot_14_01_2026'
# sha256 of the zips as SourceForge served them on 2026-10-10; NSIS's equals the copy M13 was built with,
# SWIG's swig.exe equals winget's SWIG.SWIG 4.4.1.
$Downloads = @(
  @{ name = 'nsis-3.11'; url = 'https://downloads.sourceforge.net/project/nsis/NSIS%203/3.11/nsis-3.11.zip'; sha256 = 'c7d27f780ddb6cffb4730138cd1591e841f4b7edb155856901cdf5f214394fa1' },
  @{ name = 'swigwin-4.4.1'; url = 'https://downloads.sourceforge.net/project/swig/swigwin/swigwin-4.4.1/swigwin-4.4.1.zip'; sha256 = 'ce01474c81120eab381491d8d45cbcce4768fd1e5c23ffc7654b522702769598' }
)
$WebView2Url = 'https://go.microsoft.com/fwlink/p/?LinkId=2124703'
# -FetchCuda: the parts of CUDA 13.2.1 nvcc needs, as NVIDIA's redistrib_13.2.1.json lists them.
$CudaRedist = 'https://developer.download.nvidia.com/compute/cuda/redist'
$CudaParts = @(
  @{ name = 'cuda_nvcc'; version = '13.2.78'; sha256 = 'da33f46a1a907a12abd0c192bbe907057b1e2269fdccfc778101fb02161e1c59' },
  @{ name = 'cuda_crt'; version = '13.2.78'; sha256 = '0e19f9d23451d77e32794d53bb110a4eecb26d9542391dbb3f997a688c9ddecc' },
  @{ name = 'libnvvm'; version = '13.2.78'; sha256 = '2cbd83a3d8bd594cb53cb1b7d0e129b0931f21915aa61c71c7491374b23e9b62' },
  @{ name = 'cuda_cudart'; version = '13.2.75'; sha256 = '8c7f187543545cefdbf55f66ecb8c990e159e38a78694fda50e251fcb2f4fe5f' },
  @{ name = 'cuda_cccl'; version = '13.2.75'; sha256 = '4c799e2c502ccc9a712d90baa102d9a2f6c50d6303f9e07a227ef4756c4d0a6a' }
)

function Step([string]$what) { Write-Host ("[{0}] {1}" -f (Get-Date -Format 'HH:mm:ss'), $what) }

# A native command through cmd.exe, its output to a log (Windows PowerShell 5.1 turns a native
# program's stderr into errors); throws with the log's tail on a non-zero exit.
function Native([string]$cmdline, [string]$log) {
  cmd /c "$cmdline > `"$log`" 2>&1"
  $code = $LASTEXITCODE
  if ($code -ne 0) { Get-Content $log -Tail 40 | Write-Host; throw "exit $code from: $cmdline (log: $log)" }
}

function Need([string]$exe, [string]$hint) {
  $c = Get-Command $exe -ErrorAction SilentlyContinue
  if (-not $c) { throw "$exe is not on PATH: $hint" }
  $c.Source
}

# --- 0. prerequisites, named before anything is built
Step 'prerequisites'
Need git 'install Git for Windows' | Out-Null
Need cmake 'install CMake (the verified build used 4.3.2)' | Out-Null
Need python 'install Python 3 from python.org (its headers are needed by upstream''s configure)' | Out-Null
Need node 'install Node.js 24' | Out-Null
Need npm 'install Node.js 24' | Out-Null
Need cargo 'install Rust with rustup (https://rustup.rs)' | Out-Null
if (-not $FetchCuda) { Need nvcc 'install the CUDA Toolkit 13.2, or pass -FetchCuda' | Out-Null }
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
if (-not (Test-Path $vswhere)) { throw 'Visual Studio 2022 is not installed (vswhere.exe not found)' }
$vs = & $vswhere -version '[17.0,18.0)' -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $vs) { throw 'no Visual Studio 2022 with the x64 C++ tools (the "Desktop development with C++" workload)' }
$manifest = Get-Content "$repo\solvers\manifest.json" -Raw | ConvertFrom-Json
$dirty = & git -C $repo status --porcelain --untracked-files=no
if ($dirty) { Write-Warning 'the tree has uncommitted changes: About and the installer will say so' }

# --- 1. the build tools it fetches
foreach ($d in $Downloads) {
  $dir = Join-Path $tools $d.name
  if (Test-Path $dir) { continue }
  Step "download $($d.name)"
  $zip = Join-Path $tools "$($d.name).zip"
  Invoke-WebRequest -UseBasicParsing -UserAgent 'curl/8' $d.url -OutFile $zip
  $h = (Get-FileHash -Algorithm SHA256 $zip).Hash.ToLower()
  if ($h -ne $d.sha256) { throw "$zip is sha256 $h, not the pinned $($d.sha256)" }
  Expand-Archive $zip (Join-Path $tools "$($d.name).tmp") -Force
  Move-Item (Join-Path $tools "$($d.name).tmp\$($d.name)") $dir
  Remove-Item -Recurse -Force (Join-Path $tools "$($d.name).tmp"), $zip
}
$makensis = Join-Path $tools 'nsis-3.11\makensis.exe'
$swigRoot = Join-Path $tools 'swigwin-4.4.1'
$webview2 = Join-Path $tools 'MicrosoftEdgeWebview2Setup.exe'
if (-not (Test-Path $webview2)) {
  Step 'download the WebView2 bootstrapper'
  Invoke-WebRequest -UseBasicParsing $WebView2Url -OutFile $webview2
}
if ($FetchCuda) {
  # NVIDIA's redistributable archives of CUDA 13.2.1, only what nvcc needs to build spps-gpu, merged
  # into one folder ahead of any installed CUDA on PATH.
  $cuda = Join-Path $tools 'cuda-13.2.1'
  if (-not (Test-Path (Join-Path $cuda 'bin\nvcc.exe'))) {
    New-Item -ItemType Directory -Force $cuda | Out-Null
    foreach ($p in $CudaParts) {
      Step "download CUDA $($p.name) $($p.version)"
      $zip = Join-Path $tools "$($p.name).zip"
      Invoke-WebRequest -UseBasicParsing "$CudaRedist/$($p.name)/windows-x86_64/$($p.name)-windows-x86_64-$($p.version)-archive.zip" -OutFile $zip
      $h = (Get-FileHash -Algorithm SHA256 $zip).Hash.ToLower()
      if ($h -ne $p.sha256) { throw "$zip is sha256 $h, not the pinned $($p.sha256)" }
      $tmp = Join-Path $tools "$($p.name).tmp"
      Expand-Archive $zip $tmp -Force
      $top = Get-ChildItem $tmp -Directory | Select-Object -First 1
      Get-ChildItem $top.FullName | ForEach-Object { Copy-Item $_.FullName $cuda -Recurse -Force }
      Remove-Item -Recurse -Force $tmp, $zip
    }
  }
  $env:CUDA_PATH = $cuda
  $env:PATH = "$cuda\bin;$env:PATH"
}
$nvcc = Need nvcc 'install the CUDA Toolkit 13.2, or pass -FetchCuda'
$nvccVersion = ((& nvcc --version) | Select-String 'release (\d+\.\d+)').Matches[0].Groups[1].Value
Write-Host "  nvcc: $nvcc (CUDA $nvccVersion)"
if ($nvccVersion -ne '13.2') { Write-Warning "nvcc is CUDA $nvccVersion; the verified spps-gpu.exe was built with 13.2, so its code sha256 will differ" }

# --- 2. upstream at the pinned tag
if (-not $Upstream) {
  $Upstream = Join-Path $Work 'upstream'
  if (-not (Test-Path (Join-Path $Upstream '.git'))) {
    Step "clone upstream I-Simpa $UpstreamTag"
    Native "git clone --quiet --depth 1 --branch $UpstreamTag $UpstreamUrl `"$Upstream`"" (Join-Path $logs 'upstream-clone.log')
  }
}
$head = (& git -C $Upstream rev-parse HEAD).Trim()
if ($head -ne $manifest.upstream_commit) { throw "upstream at $Upstream is $head, not $($manifest.upstream_commit) ($UpstreamTag)" }

# --- 3. the solvers
$solverRoot = Join-Path $Work 'solvers'
$bin = Join-Path $solverRoot 'bin'
Step 'solvers: upstream + patches, TetGen 1.5.0 (solvers\build.ps1)'
& "$repo\solvers\build.ps1" -Upstream $Upstream -Commit $manifest.upstream_commit -CpmCache ($CpmCache -replace '\\', '/') -SwigRoot $swigRoot -Root $solverRoot
Step 'spps-gpu (solvers\spps-gpu\build.cmd, sm_75 + sm_120)'
Native "`"$repo\solvers\spps-gpu\build.cmd`" `"$bin`" fat" (Join-Path $logs 'spps-gpu.log')

# --- 4. the solvers against the manifest
. "$repo\tools\devtools\payload.ps1"
$check = @($script:SolverExes | ForEach-Object { Test-SolverVerified -Manifest $manifest -Exe (Join-Path $bin $_) })
$check | ForEach-Object { Write-Host ('  {0,-20} code sha256 {1}  {2}' -f $_.name, $_.code_sha256.Substring(0, 16), $(if ($_.verified) { 'verified (= solvers/manifest.json)' } else { "NOT the verified build (manifest $($_.want.Substring(0, 16)))" })) }
$unverified = @($check | Where-Object { -not $_.verified })
if ($unverified.Count -and -not $AllowUnverifiedSolvers) {
  throw "$($unverified.Count) solver(s) are not the verified build: $(($unverified | ForEach-Object name) -join ', '). The usual causes: another MSVC or CUDA version than solvers/manifest.json records ($($manifest.compiler), nvcc $($manifest.spps_gpu.nvcc)), or, for spps-gpu, SPPS_GPU_LINEINFO=1 set (line info puts folder paths in its code). Rerun with -AllowUnverifiedSolvers to build anyway; the app will then refuse to run those solvers."
}

# --- 5. the app
Step "app: npm ci + tauri build (CARGO_TARGET_DIR $cargoTarget)"
$env:CARGO_TARGET_DIR = $cargoTarget
Push-Location (Join-Path $repo 'app')
try {
  Native 'npm ci --no-audit --no-fund' (Join-Path $logs 'npm-ci.log')
  Native 'npx --no-install tauri build --no-bundle' (Join-Path $logs 'tauri-build.log')
} finally { Pop-Location }
$app = Join-Path $cargoTarget 'release\app.exe'
if (-not (Test-Path $app)) { throw "the app build wrote no $app" }

# --- 6. the installer
Step 'installer (tools\installer\build-installer.ps1)'
$before = @(Get-ChildItem $Out -Directory -ErrorAction SilentlyContinue | ForEach-Object FullName)
& "$repo\tools\installer\build-installer.ps1" -Solvers $bin -App $app -Out $Out -Makensis $makensis -WebView2 $webview2 -AllowUnverifiedSolvers:$AllowUnverifiedSolvers
$dir = Get-ChildItem $Out -Directory | Where-Object { $before -notcontains $_.FullName -and (Test-Path (Join-Path $_.FullName 'installer.json')) } | Select-Object -First 1
if (-not $dir) { throw "build-installer.ps1 left no new build folder under $Out" }
$receipt = Get-Content (Join-Path $dir.FullName 'installer.json') -Raw | ConvertFrom-Json
$minutes = [Math]::Round(((Get-Date) - $started).TotalMinutes, 1)
Step ("done in $minutes min: $($receipt.installer)")
Write-Host "  sha256 $($receipt.sha256)  ($($receipt.bytes) bytes, solvers verified: $($receipt.solvers_verified))"

if ($env:GITHUB_OUTPUT) {
  Add-Content $env:GITHUB_OUTPUT "dir=$($dir.FullName)"
  Add-Content $env:GITHUB_OUTPUT "installer=$($receipt.installer)"
}
if ($env:GITHUB_STEP_SUMMARY) {
  $rows = $check | ForEach-Object { "| $($_.name) | ``$($_.code_sha256.Substring(0, 16))`` | $(if ($_.verified) { 'verified' } else { 'not the verified build' }) |" }
  @("### Installer", '', "``$(Split-Path -Leaf $receipt.installer)``, $($receipt.bytes) bytes, sha256 ``$($receipt.sha256)``, built in $minutes min", '',
    '| solver | code sha256 | against solvers/manifest.json |', '|---|---|---|') + $rows | Add-Content $env:GITHUB_STEP_SUMMARY
}
