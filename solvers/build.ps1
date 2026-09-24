# Builds the solvers we ship:
# - spps.exe, classicalTheory.exe and preprocess.exe: I-Simpa's own, unchanged, from the pinned
#   upstream commit. The upstream checkout is only read (git archive), never written to.
#   Configuration matches the 2026-09-08 reference build in <upstream>\build_solvers: upstream's
#   own top-level CMakeLists, SKIPISIMPA=ON, Visual Studio 17 2022, x64, Release.
# - tetgen.exe: WIAS TetGen 1.5.0 from third_party/tetgen-1.5.0 (= upstream 4db335c, the TetGen
#   upstream shipped in 1.3.3 and 1.3.4; see its PROVENANCE.md), built by solvers/tetgen/
#   CMakeLists.txt. The pinned commit's own TetGen is 1.6.0, which never tests a facet's area
#   bound, so surface receivers are not refined (docs/m5-m6-design.md, decision 3).
#   Upstream's 1.6.0 target is still built, into the build tree only: its recorded cl and link
#   command lines are the reference ours must equal, and M1 runs it as the build its tetgen
#   checks must refuse. It is never copied into bin\.
# The manifest records each executable's sha256 and its code sha256 (solvers/pe-fingerprint.ps1):
# the sha256 with the link-time fields zeroed, the same for every build of the same code. The
# gates hold the solvers to the code sha256, so a rebuild needs no new manifest or fixtures.
param(
    [string]$Upstream = 'B:\repos\I-Simpa-upstream',
    [string]$Commit = '929a5c8e5f590b189f29155faeff8670f3699479',
    [string]$CpmCache = 'B:/repos/.cpm-cache',
    # Upstream always configures src/python_bindings, which needs SWIG even though we build no bindings.
    [string]$SwigRoot = "$env:LOCALAPPDATA\Microsoft\WinGet\Packages\SWIG.SWIG_Microsoft.Winget.Source_8wekyb3d8bbwe\swigwin-4.4.1",
    # A new name gives a from-scratch compile without deleting an earlier build tree.
    [string]$BuildName = 'build',
    # Where the source archive, the build trees, bin\ and the logs go. Default <repo>\target\solvers,
    # the build the gates and tests run; any other folder is a build beside it (a fresh folder is a
    # from-scratch build). Every build writes its own manifest to <that folder>\manifest.json.
    [string]$Root = '',
    # Also write this build's manifest over the committed solvers\manifest.json. Only needed when
    # the code sha256 changes (new source or new build settings); a plain rebuild changes only the
    # raw sha256 and the link time, which the gates do not hold the solvers to.
    [switch]$UpdateCommittedManifest
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
# PowerShell variable names ignore case: $root below IS $Root, so read the parameter first.
$customRoot = [bool]$Root
$root = if ($customRoot) { [IO.Path]::GetFullPath($Root) } else { Join-Path $repo 'target\solvers' }
$manifestPath = Join-Path $root 'manifest.json'
$committedManifest = Join-Path $repo 'solvers\manifest.json'
$src = Join-Path $root ('src-' + $Commit.Substring(0, 7))
$bld = Join-Path $root $BuildName
$tgSrc = Join-Path $repo 'third_party\tetgen-1.5.0'
$tgBld = Join-Path $root "$BuildName-tetgen150"
$bin = Join-Path $root 'bin'
# WIAS tetgen1.5.0.tar.gz, committed beside the files extracted from it unmodified.
$tgTarball = Join-Path $tgSrc 'tetgen1.5.0.tar.gz'
$tgUpstreamCommit = '4db335c123eb54986015dbd8192aa8a7e0ec0969'
# One log per invocation: a no-op rebuild must never overwrite the log of the real compile.
$log = Join-Path $root ("build-$BuildName-" + (Get-Date -Format 'yyyyMMdd-HHmmss') + '.log')
# Upstream's tetgen target is the 1.6.0 reference, built for comparison only.
$targets = @('spps', 'classicalTheory', 'preprocess', 'tetgen')
New-Item -ItemType Directory -Force $root, $bin | Out-Null
Set-Content -Path $log -Value "build started $(Get-Date -Format s)"
. (Join-Path $PSScriptRoot 'tetgen\build-commands.ps1')
. (Join-Path $PSScriptRoot 'tetgen\source.ps1')
. (Join-Path $PSScriptRoot 'pe-fingerprint.ps1')

function Run([string]$what, [string[]]$argv) {
    Add-Content $log "`n> $what $($argv -join ' ')"
    $ErrorActionPreference = 'Continue'   # native stderr must not throw under Windows PowerShell 5.1
    & $what @argv 2>&1 | ForEach-Object { "$_" } | Add-Content $log
    $code = $LASTEXITCODE
    $ErrorActionPreference = 'Stop'
    if ($code -ne 0) { throw "$what exited $code (see $log)" }
}

# The vendored TetGen must be the tarball's bytes. The tarball is WIAS's (its sha256), each of
# its members is read here and must equal the folder's file, and SHA256SUMS, the human-readable
# list, must say the same.
$tgTarballSha = (Get-FileHash -LiteralPath $tgTarball -Algorithm SHA256).Hash.ToLower()
if ($tgTarballSha -ne $TetgenTarballSha256) { throw "$tgTarball is $tgTarballSha, not WIAS's tetgen1.5.0.tar.gz ($TetgenTarballSha256): refusing to build" }
$tgFiles = Read-TetgenTarball $tgTarball
$bad = @(Get-TarballMismatches $tgFiles $tgSrc)
if ($bad.Count) { throw "third_party/tetgen-1.5.0 differs from its tarball in $($bad -join ', '): refusing to build a changed TetGen" }
$sums = @{}
foreach ($line in Get-Content (Join-Path $tgSrc 'SHA256SUMS')) {
    if ($line -notmatch '^([0-9a-f]{64})  (\S+)$') { throw "SHA256SUMS: unreadable line '$line'" }
    $sums[$Matches[2]] = $Matches[1]
}
if ($sums.Count -ne $tgFiles.Count -or @($tgFiles.Keys | Where-Object { $sums[$_] -ne $tgFiles[$_] }).Count) {
    throw "third_party/tetgen-1.5.0/SHA256SUMS does not list the tarball's members and their sha256"
}
# For the manifest, in SHA256SUMS's (ordinal) order.
$names = [string[]]@($tgFiles.Keys); [Array]::Sort($names, [StringComparer]::Ordinal)
$tgSorted = [ordered]@{}; foreach ($k in $names) { $tgSorted[$k] = $tgFiles[$k] }
$tgFiles = $tgSorted

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

Run 'cmake' @('-S', (Join-Path $PSScriptRoot 'tetgen'), '-B', $tgBld, '-G', 'Visual Studio 17 2022', '-A', 'x64',
    '-DCMAKE_BUILD_TYPE=Release', "-DTETGEN_SOURCE_DIR=$tgSrc")
Run 'cmake' @('--build', $tgBld, '--config', 'Release', '--parallel', '--target', 'tetgen')

# Ours must have been compiled and linked exactly as upstream's tetgen target, paths aside.
$refCmds = Get-BuildCommands (Join-Path $bld 'src\tetgen') (Join-Path $src 'src\tetgen')
$ourCmds = Get-BuildCommands $tgBld $tgSrc
$cmdDiff = Compare-BuildCommands $refCmds $ourCmds
Add-Content $log "`ntetgen command lines, upstream target (<) against ours (>): $($cmdDiff.Count) differ"
$cmdDiff | Add-Content $log
if ($cmdDiff.Count) { throw "TetGen 1.5.0 was not built with upstream's tetgen settings: $($cmdDiff.Count) command-line records differ (see $log)" }

$exes = [ordered]@{
    'spps.exe'            = Join-Path $bld 'src\spps\Release\spps.exe'
    'classicalTheory.exe' = Join-Path $bld 'src\ctr\Release\classicalTheory.exe'
    'tetgen.exe'          = Join-Path $tgBld 'Release\tetgen.exe'
    'preprocess.exe'      = Join-Path $bld 'src\preprocess\Release\preprocess.exe'
}
$hashes = [ordered]@{}; $codeHashes = [ordered]@{}
foreach ($name in $exes.Keys) {
    $built = $exes[$name]
    if (-not (Test-Path $built)) { throw "missing build output $built" }
    Copy-Item $built (Join-Path $bin $name) -Force
    $hashes[$name] = Get-RawSha256 (Join-Path $bin $name)
    # Throws on an executable whose link-time fields are not the measured ones.
    $codeHashes[$name] = Get-CodeSha256 (Join-Path $bin $name)
}
$ref16 = Join-Path $bld 'src\tetgen\Release\tetgen.exe'
if (-not (Test-Path $ref16)) { throw "missing build output $ref16" }
$ref16Code = Get-CodeSha256 $ref16

$ccf = Get-ChildItem (Join-Path $bld 'CMakeFiles') -Recurse -Filter 'CMakeCXXCompiler.cmake' | Select-Object -First 1
$compiler = (Select-String -Path $ccf.FullName -Pattern 'set\(CMAKE_CXX_COMPILER_VERSION "([^"]+)"').Matches[0].Groups[1].Value
$ccf = Get-ChildItem (Join-Path $tgBld 'CMakeFiles') -Recurse -Filter 'CMakeCXXCompiler.cmake' | Select-Object -First 1
$tgCompiler = (Select-String -Path $ccf.FullName -Pattern 'set\(CMAKE_CXX_COMPILER_VERSION "([^"]+)"').Matches[0].Groups[1].Value
if ($tgCompiler -ne $compiler) { throw "TetGen compiled by $tgCompiler, the solvers by $compiler" }
$manifest = [ordered]@{
    upstream_commit = $Commit
    cmake_version   = ((cmake --version | Select-Object -First 1) -replace 'cmake version ', '')
    generator       = 'Visual Studio 17 2022 (x64)'
    build_dir       = $BuildName
    build_log       = (Split-Path -Leaf $log)
    compiler        = $compiler
    options         = @('SKIPISIMPA=ON', 'CMAKE_BUILD_TYPE=Release', "CPM_SOURCE_CACHE=$CpmCache")
    crt             = 'dynamic (upstream default)'
    # This link's bytes: another build of the same code has other ones.
    sha256          = $hashes
    # The same for every build of the same code: what the gates and the run fixtures hold to.
    code_sha256     = $codeHashes
    code_sha256_of  = 'the exe with IMAGE_FILE_HEADER.TimeDateStamp and every IMAGE_DEBUG_DIRECTORY TimeDateStamp zeroed (solvers/pe-fingerprint.ps1)'
    tetgen          = [ordered]@{
        version                      = '1.5.0'
        source                       = 'third_party/tetgen-1.5.0'
        tarball                      = 'third_party/tetgen-1.5.0/tetgen1.5.0.tar.gz (WIAS)'
        tarball_sha256               = $tgTarballSha
        files_sha256                 = $tgFiles
        same_blobs_as_upstream       = $tgUpstreamCommit
        cmake                        = 'solvers/tetgen/CMakeLists.txt'
        build_dir                    = (Split-Path -Leaf $tgBld)
        command_lines                = "equal to upstream's tetgen target at $($Commit.Substring(0, 7)), $($refCmds.Count) records"
        exe_sha256                   = $hashes['tetgen.exe']
        exe_code_sha256              = $codeHashes['tetgen.exe']
        upstream_160_reference_sha256 = Get-RawSha256 $ref16
        upstream_160_reference_code_sha256 = $ref16Code
        upstream_160_reference       = "$BuildName\src\tetgen\Release\tetgen.exe (not shipped)"
    }
    built_at        = (Get-Date -Format s)
}
$manifest | ConvertTo-Json -Depth 4 | Set-Content $manifestPath -Encoding UTF8
if ($UpdateCommittedManifest) {
    $manifest | ConvertTo-Json -Depth 4 | Set-Content $committedManifest -Encoding UTF8
    Write-Host "committed manifest rewritten: $committedManifest"
}
Add-Content $log "`nbuild finished $(Get-Date -Format s)"
$warn = Select-String -Path $log -Pattern 'warning (C\d{4})' | ForEach-Object { $_.Matches[0].Groups[1].Value }
$compiled = @(Select-String -Path $log -Pattern '\.(cpp|cxx|c)$').Count
Write-Host "compiler warnings: $(@($warn).Count) lines over $compiled compiled-file lines (log: $log)"
$warn | Group-Object | Sort-Object Count -Descending | Select-Object -First 8 | ForEach-Object { Write-Host ("  {0} x{1}" -f $_.Name, $_.Count) }
Write-Host "tetgen 1.5.0 command lines equal upstream's tetgen target ($($refCmds.Count) records)"
Write-Host "solvers built into $bin; manifest $manifestPath"
$hashes.GetEnumerator() | ForEach-Object { Write-Host ("  {0,-20} sha256 {1}  code sha256 {2}" -f $_.Key, $_.Value.Substring(0, 16), $codeHashes[$_.Key].Substring(0, 16)) }
