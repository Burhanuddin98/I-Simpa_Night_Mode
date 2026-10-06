# Builds box_tracer.exe with nvcc and the VS 2022 host compiler, into -Out (default C:\tmp\nm-gpu-probe).
# -Arch native compiles for the GPU in this machine (Grace: RTX 5070; Zeph: sm_75 RTX 2060).
param([string]$Out = 'C:\tmp\nm-gpu-probe', [string]$Arch = 'native')
$ErrorActionPreference = 'Stop'
$src = Join-Path $PSScriptRoot 'box_tracer.cu'
New-Item -ItemType Directory -Force -Path $Out | Out-Null
$vs = Get-ChildItem 'C:\Program Files\Microsoft Visual Studio\2022' -Directory | Select-Object -First 1
$vsdevcmd = Join-Path $vs.FullName 'Common7\Tools\VsDevCmd.bat'
if (-not (Test-Path $vsdevcmd)) { throw "no VS 2022 VsDevCmd.bat at $vsdevcmd" }
$exe = Join-Path $Out 'box_tracer.exe'
$log = Join-Path $Out 'build.log'
# The dev environment through cmd (Launch-VsDevShell.ps1 wants vswhere.exe on PATH; VsDevCmd does not).
$cmd = "call `"$vsdevcmd`" -arch=amd64 -no_logo && nvcc --version && nvcc -O3 -std=c++17 -arch=$Arch -lineinfo -o `"$exe`" `"$src`""
# VsDevCmd prints a harmless vswhere.exe complaint on stderr; PowerShell 5.1 would make it fatal under Stop.
$ErrorActionPreference = 'Continue'
& cmd.exe /c $cmd > $log 2>&1
$code = $LASTEXITCODE
$ErrorActionPreference = 'Stop'
Get-Content $log | ForEach-Object { $_.ToString() }
if ($code -ne 0) { throw "nvcc exit $code (log $log)" }
Get-Item $exe | ForEach-Object { "built $($_.FullName) $($_.LastWriteTime.ToString('HH:mm:ss')) $([math]::Round($_.Length/1KB)) KB" }
