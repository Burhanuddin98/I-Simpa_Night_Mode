# Generates a corpus of REAL solver output for the oracle differential, so the read-only formats
# (gabe, csbin, pbin, tetgen) are checked on hundreds of files rather than a handful of fixtures.
# Runs our M1-built solvers on the tutorial-1 fixture: SPPS at several seeds with particle export
# on, TCR once, and TetGen at two settings. Prints the corpus directory on its last line.
#   powershell -File tools/oracle/make-corpus.ps1 [-Seeds 3]
param([int]$Seeds = 3)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$bin = Join-Path $repo 'target\solvers\bin'
$fix = Join-Path $repo 'tests\fixtures\upstream\tutorial1'
$out = Join-Path $repo ('target\oracle-corpus\' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
foreach ($exe in 'spps.exe', 'classicalTheory.exe', 'tetgen.exe') {
    if (-not (Test-Path (Join-Path $bin $exe))) { throw "missing ${exe}; run solvers/build.ps1 first" }
}

function Invoke-Case([string]$kind, [string]$name, [string]$exe, [string[]]$argv, [hashtable]$attrs) {
    $dir = Join-Path $out $name
    New-Item -ItemType Directory -Force $dir | Out-Null
    Copy-Item (Join-Path $fix "$kind\*") $dir
    $cfg = Join-Path $dir 'config.xml'
    if (Test-Path $cfg) {
        $x = (Get-Content $cfg -Raw).Replace('__RUNDIR__', "$dir\")
        foreach ($k in $attrs.Keys) { $x = $x -replace "\b$k=`"[^`"]*`"", "$k=`"$($attrs[$k])`"" }
        Set-Content $cfg $x -NoNewline -Encoding UTF8
    }
    $p = Start-Process -FilePath (Join-Path $bin $exe) -ArgumentList $argv -WorkingDirectory $dir -NoNewWindow -Wait -PassThru `
        -RedirectStandardOutput (Join-Path $dir '_stdout.txt') -RedirectStandardError (Join-Path $dir '_stderr.txt')
    if ($p.ExitCode -ne 0) { throw "$exe exited $($p.ExitCode) for $name" }
}

for ($s = 1; $s -le $Seeds; $s++) {
    Invoke-Case 'spps' "spps-seed$s" 'spps.exe' @('config.xml') @{ random_seed = "$s"; nbparticules_rendu = '20' }
}
Invoke-Case 'tcr' 'tcr' 'classicalTheory.exe' @('config.xml') @{}
Invoke-Case 'tetgen' 'tetgen-q5' 'tetgen.exe' @('-pq5', '-A', '-n', '-Y', 'scene_mesh.poly') @{}
Invoke-Case 'tetgen' 'tetgen-plain' 'tetgen.exe' @('-p', '-A', '-n', 'scene_mesh.poly') @{}
$count = @(Get-ChildItem $out -Recurse -File).Count
Write-Host "corpus: $count files"
Write-Output $out
