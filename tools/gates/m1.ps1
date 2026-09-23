# M1 gate: the solvers we build are the unchanged upstream solvers.
# The binaries themselves differ from the 2026-09-08 reference build (MSVC embeds a timestamp),
# so equivalence is judged on what they output from identical, seeded inputs.
# Run from anywhere: powershell -File tools/gates/m1.ps1
$ErrorActionPreference = 'Stop'
$failures = @()
function Check($name, [scriptblock]$body) {
    try {
        $ok = & $body
        if ($ok) { Write-Host "PASS  $name" } else { Write-Host "FAIL  $name"; $script:failures += $name }
    } catch {
        Write-Host "FAIL  $name :: $($_.Exception.Message)"; $script:failures += $name
    }
}

$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$up = 'B:\repos\I-Simpa-upstream'
$commit = '929a5c8e5f590b189f29155faeff8670f3699479'
$bin = Join-Path $repo 'target\solvers\bin'
$fix = Join-Path $repo 'tests\fixtures\upstream\tutorial1'
$manifest = Get-Content (Join-Path $repo 'solvers\manifest.json') -Raw | ConvertFrom-Json
$ref = @{
    spps   = Join-Path $up 'build_solvers\src\spps\Release\spps.exe'
    tcr    = Join-Path $up 'build_solvers\src\ctr\Release\classicalTheory.exe'
    tetgen = Join-Path $up 'build_solvers\src\tetgen\Release\tetgen.exe'
}
$new = @{
    spps   = Join-Path $bin 'spps.exe'
    tcr    = Join-Path $bin 'classicalTheory.exe'
    tetgen = Join-Path $bin 'tetgen.exe'
}
# Fresh folder per gate run, so no earlier output is ever reused or needs deleting.
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$work = Join-Path $repo "target\gates\m1\$stamp"
$trackedBefore = @(git -C $up status --porcelain --untracked-files=no).Count

function New-Run([string]$kind, [string]$label) {
    $dir = Join-Path $work "$kind-$label"
    New-Item -ItemType Directory -Force $dir | Out-Null
    Copy-Item (Join-Path $fix "$kind\*") $dir
    $cfg = Join-Path $dir 'config.xml'
    if (Test-Path $cfg) {
        (Get-Content $cfg -Raw).Replace('__RUNDIR__', "$dir\") | Set-Content $cfg -NoNewline -Encoding UTF8
    }
    return $dir
}
function Invoke-Solver([string]$exe, [string]$dir, [string[]]$argv) {
    $p = Start-Process -FilePath $exe -ArgumentList $argv -WorkingDirectory $dir -NoNewWindow -Wait -PassThru `
        -RedirectStandardOutput (Join-Path $dir '_stdout.txt') -RedirectStandardError (Join-Path $dir '_stderr.txt')
    return $p.ExitCode
}
function Get-Outputs([string]$dir, [string[]]$inputs) {
    $h = @{}
    Get-ChildItem $dir -Recurse -File |
        Where-Object { $_.Name -notin $inputs -and $_.Name -notlike '_std*.txt' } |
        ForEach-Object { $h[$_.FullName.Substring($dir.Length + 1)] = (Get-FileHash $_.FullName -Algorithm SHA256).Hash }
    return $h
}
function Compare-Outputs($a, $b) {
    $keys = @($a.Keys) + @($b.Keys) | Sort-Object -Unique
    return @($keys | Where-Object { $a[$_] -ne $b[$_] })
}
# .csbin (RSBIN surface receivers) is written by dumping whole C structs, padding included, so two
# runs of the SAME executable differ in 2 bytes per 8-byte record (verified 2026-09-23). Bytes are
# meaningless there; M1 checks the same files at the same sizes, and M2 compares them decoded.
function Get-Sizes([string]$dir) {
    $h = @{}
    Get-ChildItem $dir -Recurse -File -Filter *.csbin | ForEach-Object { $h[$_.FullName.Substring($dir.Length + 1)] = $_.Length }
    return $h
}

# (1) and (2): what we built is what the manifest says, from the pinned commit.
foreach ($exeName in 'spps.exe', 'classicalTheory.exe', 'tetgen.exe', 'preprocess.exe') {
    Check "$exeName exists and matches manifest.json" {
        $p = Join-Path $bin $exeName
        (Test-Path $p) -and ((Get-FileHash $p -Algorithm SHA256).Hash.ToLower() -eq $manifest.sha256.$exeName)
    }
}
Check "manifest records upstream $($commit.Substring(0,7))" { $manifest.upstream_commit -eq $commit }
Check "reference binaries present" { ($ref.Values | Where-Object { -not (Test-Path $_) }).Count -eq 0 }

# (4) and (5): SPPS, seeded, new against reference.
$sppsNew = New-Run 'spps' 'new'; $sppsRef = New-Run 'spps' 'ref'
$exitNew = Invoke-Solver $new.spps $sppsNew @('config.xml')
$exitRef = Invoke-Solver $ref.spps $sppsRef @('config.xml')
Check "SPPS banner is version 2.2.1" { (Get-Content (Join-Path $sppsNew '_stdout.txt') -TotalCount 1) -match '^SPPS version 2\.2\.1' }
Check "SPPS exits 0 on both (new $exitNew, reference $exitRef)" { $exitNew -eq 0 -and $exitRef -eq 0 }
Check "SPPS reaches 'End of calculation' on both" {
    (Select-String -Path (Join-Path $sppsNew '_stdout.txt') -Pattern '^End of calculation' -Quiet) -and
    (Select-String -Path (Join-Path $sppsRef '_stdout.txt') -Pattern '^End of calculation' -Quiet)
}
$sppsInputs = @('config.xml', 'mesh.cbin', 'tetramesh.mbin')
$oNew = Get-Outputs $sppsNew $sppsInputs; $oRef = Get-Outputs $sppsRef $sppsInputs
$diff = Compare-Outputs $oNew $oRef
Check "SPPS produced output on both (new $($oNew.Count), reference $($oRef.Count) files)" { $oNew.Count -gt 0 -and $oRef.Count -eq $oNew.Count }
Check "SPPS stats GABE and every .recp byte-identical" {
    @($diff | Where-Object { $_ -like '*.recp' -or $_ -like '*particle statistics.gabe' }).Count -eq 0
}
$nonCsbin = @($diff | Where-Object { $_ -notlike '*.csbin' })
Check "SPPS every non-.csbin output byte-identical ($($nonCsbin.Count) differ)" { $nonCsbin.Count -eq 0 }
Check "SPPS .csbin: same files at the same sizes" { @(Compare-Outputs (Get-Sizes $sppsNew) (Get-Sizes $sppsRef)).Count -eq 0 }

# (6): TCR, deterministic, new against reference.
$tcrNew = New-Run 'tcr' 'new'; $tcrRef = New-Run 'tcr' 'ref'
$exitNew = Invoke-Solver $new.tcr $tcrNew @('config.xml')
$exitRef = Invoke-Solver $ref.tcr $tcrRef @('config.xml')
Check "TCR exits 0 on both (new $exitNew, reference $exitRef)" { $exitNew -eq 0 -and $exitRef -eq 0 }
$oNew = Get-Outputs $tcrNew $sppsInputs; $oRef = Get-Outputs $tcrRef $sppsInputs
$diff = Compare-Outputs $oNew $oRef
Check "TCR produced output on both (new $($oNew.Count), reference $($oRef.Count) files)" { $oNew.Count -gt 0 -and $oRef.Count -eq $oNew.Count }
$nonCsbin = @($diff | Where-Object { $_ -notlike '*.csbin' })
Check "TCR every non-.csbin output byte-identical ($($nonCsbin.Count) differ)" { $nonCsbin.Count -eq 0 }
Check "TCR .csbin: same files at the same sizes" { @(Compare-Outputs (Get-Sizes $tcrNew) (Get-Sizes $tcrRef)).Count -eq 0 }

# (7): TetGen on tutorial 1's own input, comment lines excluded.
$tgNew = New-Run 'tetgen' 'new'; $tgRef = New-Run 'tetgen' 'ref'
$flags = @('-pq5', '-A', '-n', '-Y', 'scene_mesh.poly')
$exitNew = Invoke-Solver $new.tetgen $tgNew $flags
$exitRef = Invoke-Solver $ref.tetgen $tgRef $flags
Check "TetGen exits 0 on both (new $exitNew, reference $exitRef)" { $exitNew -eq 0 -and $exitRef -eq 0 }
foreach ($ext in 'node', 'ele', 'face', 'neigh') {
    Check "TetGen .1.$ext identical outside comments" {
        $a = Get-Content (Join-Path $tgNew "scene_mesh.1.$ext") | Where-Object { $_ -notmatch '^\s*#' }
        $b = Get-Content (Join-Path $tgRef "scene_mesh.1.$ext") | Where-Object { $_ -notmatch '^\s*#' }
        ($a.Count -gt 0) -and (@(Compare-Object $a $b -SyncWindow 0).Count -eq 0)
    }
}

# (3): the upstream checkout was not touched.
Check "upstream tracked files untouched (before $trackedBefore, after)" {
    $trackedBefore -eq 0 -and @(git -C $up status --porcelain --untracked-files=no).Count -eq 0
}
Check "upstream HEAD still $($commit.Substring(0,7))" { (git -C $up rev-parse HEAD) -eq $commit }

Write-Host "`nrun folders: $work"
if ($failures.Count) { Write-Host "M1 FAILED: $($failures.Count) check(s)"; exit 1 }
Write-Host "M1 PASSED"; exit 0
