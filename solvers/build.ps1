# Builds I-Simpa's solvers, unchanged, from the pinned upstream commit.
# The upstream checkout is only read (git archive), never written to.
# Configuration matches the 2026-09-08 reference build in <upstream>\build_solvers:
# upstream's own top-level CMakeLists, SKIPISIMPA=ON, Visual Studio 17 2022, x64, Release.
param(
    [string]$Upstream = 'B:\repos\I-Simpa-upstream',
    [string]$Commit = '929a5c8e5f590b189f29155faeff8670f3699479',
    [string]$CpmCache = 'B:/repos/.cpm-cache',
    # Upstream always configures src/python_bindings, which needs SWIG even though we build no bindings.
    [string]$SwigRoot = "$env:LOCALAPPDATA\Microsoft\WinGet\Packages\SWIG.SWIG_Microsoft.Winget.Source_8wekyb3d8bbwe\swigwin-4.4.1",
    # A new name gives a from-scratch compile without deleting an earlier build tree.
    [string]$BuildName = 'build'
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$root = Join-Path $repo 'target\solvers'
$src = Join-Path $root ('src-' + $Commit.Substring(0, 7))
$bld = Join-Path $root $BuildName
$bin = Join-Path $root 'bin'
# One log per invocation: a no-op rebuild must never overwrite the log of the real compile.
$log = Join-Path $root ("build-$BuildName-" + (Get-Date -Format 'yyyyMMdd-HHmmss') + '.log')
$targets = @('spps', 'classicalTheory', 'tetgen', 'preprocess')
New-Item -ItemType Directory -Force $root, $bin | Out-Null
Set-Content -Path $log -Value "build started $(Get-Date -Format s)"

function Run([string]$what, [string[]]$argv) {
    Add-Content $log "`n> $what $($argv -join ' ')"
    $ErrorActionPreference = 'Continue'   # native stderr must not throw under Windows PowerShell 5.1
    & $what @argv 2>&1 | ForEach-Object { "$_" } | Add-Content $log
    $code = $LASTEXITCODE
    $ErrorActionPreference = 'Stop'
    if ($code -ne 0) { throw "$what exited $code (see $log)" }
}

if (-not (Test-Path (Join-Path $src 'CMakeLists.txt'))) {
    New-Item -ItemType Directory -Force $src | Out-Null
    $tar = Join-Path $root 'upstream.tar'
    Run 'git' @('-C', $Upstream, 'archive', '--format=tar', '-o', $tar, $Commit)
    Run 'tar' @('-xf', $tar, '-C', $src)
}

Run 'cmake' @('-S', $src, '-B', $bld, '-G', 'Visual Studio 17 2022', '-A', 'x64',
    '-DSKIPISIMPA=ON', '-DCMAKE_BUILD_TYPE=Release', "-DCPM_SOURCE_CACHE=$CpmCache",
    ('-DSWIG_EXECUTABLE=' + (Join-Path $SwigRoot 'swig.exe')), ('-DSWIG_DIR=' + (Join-Path $SwigRoot 'Lib')))
Run 'cmake' (@('--build', $bld, '--config', 'Release', '--parallel', '--target') + $targets)

$exes = [ordered]@{
    'spps.exe'            = 'src\spps\Release\spps.exe'
    'classicalTheory.exe' = 'src\ctr\Release\classicalTheory.exe'
    'tetgen.exe'          = 'src\tetgen\Release\tetgen.exe'
    'preprocess.exe'      = 'src\preprocess\Release\preprocess.exe'
}
$hashes = [ordered]@{}
foreach ($name in $exes.Keys) {
    $built = Join-Path $bld $exes[$name]
    if (-not (Test-Path $built)) { throw "missing build output $built" }
    Copy-Item $built (Join-Path $bin $name) -Force
    $hashes[$name] = (Get-FileHash (Join-Path $bin $name) -Algorithm SHA256).Hash.ToLower()
}

$ccf = Get-ChildItem (Join-Path $bld 'CMakeFiles') -Recurse -Filter 'CMakeCXXCompiler.cmake' | Select-Object -First 1
$compiler = (Select-String -Path $ccf.FullName -Pattern 'set\(CMAKE_CXX_COMPILER_VERSION "([^"]+)"').Matches[0].Groups[1].Value
$manifest = [ordered]@{
    upstream_commit = $Commit
    cmake_version   = ((cmake --version | Select-Object -First 1) -replace 'cmake version ', '')
    generator       = 'Visual Studio 17 2022 (x64)'
    build_dir       = $BuildName
    build_log       = (Split-Path -Leaf $log)
    compiler        = $compiler
    options         = @('SKIPISIMPA=ON', 'CMAKE_BUILD_TYPE=Release', "CPM_SOURCE_CACHE=$CpmCache")
    crt             = 'dynamic (upstream default)'
    sha256          = $hashes
    built_at        = (Get-Date -Format s)
}
$manifest | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $repo 'solvers\manifest.json') -Encoding UTF8
Add-Content $log "`nbuild finished $(Get-Date -Format s)"
$warn = Select-String -Path $log -Pattern 'warning (C\d{4})' | ForEach-Object { $_.Matches[0].Groups[1].Value }
$compiled = @(Select-String -Path $log -Pattern '\.(cpp|cxx|c)$').Count
Write-Host "compiler warnings: $(@($warn).Count) lines over $compiled compiled-file lines (log: $log)"
$warn | Group-Object | Sort-Object Count -Descending | Select-Object -First 8 | ForEach-Object { Write-Host ("  {0} x{1}" -f $_.Name, $_.Count) }
Write-Host "solvers built into $bin"
$hashes.GetEnumerator() | ForEach-Object { Write-Host ("  {0,-20} {1}" -f $_.Key, $_.Value.Substring(0, 16)) }
