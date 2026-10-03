# M12 gate: Concept B, Results (docs/investigations/2026-10-03-m12/PLAN.md; spec
# docs/rebuild-plan-raw-2026-09-23.json, M12).
#   (a) every number the Results step shows equals `simpa results <run> --json` at the displayed
#       precision: 0 mismatches
#   (b) 0 DOM elements for any parameter whose status in beds/summary.json is not PASS
#   (c) 3 sampled (face, step) texels of a surface map == the .csbin's float32 exactly
#   (d) at 5 steps, the rendered particle count == the particles alive in the .pbin
#   (e) the DIN 18041 A3 target for the 180 m3 teaching room reads 0.55 s
#   (f) the variant switch replaces the RT series with the other run's JSON: 0 mismatches
# plus m10-h and m11-h, narrowed to "no solver-computed number outside the Results step" (they run
# inside m11.ps1, this gate's prior gate).
#
# P1 built the contracts (beds/summary.json regenerated from its evidence byte for byte, the
# report's per-parameter bed status, the Results step's IPC reads, the narrowed checker); P2 the
# Acoustics tab (a, b, e, f), P3 the viewport (c, d); P4 merged them and made (b) not vacuous: with
# all eleven parameters PASS, m12-b hides nothing, so m12-b-plant (specs/m12.bedplant.e2e.ts) runs
# the app and the CLI with C80 planted FAIL through core ($SIMPA_BED_DEMOTE, which can only demote)
# and finds no element for it. An id left in $gateIds fails as NOT BUILT, so this gate cannot print
# "M12 PASSED" until every one runs and passes.
#
# Windows PowerShell 5.1 (pwsh is not installed on Grace).
# Run: powershell -File tools/gates/m12.ps1 [-TargetDir C:\tmp\nm-target] [-Only all|static]
#        [-SolversDir C:\tmp\nm-m8a-solvers] [-BedData B:\data] [-SkipPrior]
# Partial runs (-Only static, -SkipPrior) never print "M12 PASSED".
param(
    [string]$TargetDir = 'C:\tmp\nm-target',
    [ValidateSet('all', 'static')][string]$Only = 'all',
    [string]$SolversDir = 'C:\tmp\nm-m8a-solvers',
    [string]$Upstream = 'B:\repos\I-Simpa-upstream',
    # The bed evidence beds/summary.json is derived from (m8b-bed, m8b-edt).
    [string]$BedData = 'B:\data',
    [switch]$SkipPrior
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Set-Location $repo
$env:RUSTUP_HOME = "$env:USERPROFILE\.rustup"; $env:CARGO_HOME = "$env:USERPROFILE\.cargo"
$env:Path = "$env:CARGO_HOME\bin;$env:Path"; $env:CARGO_INCREMENTAL = '0'
if (-not $env:CARGO_BUILD_JOBS) { $env:CARGO_BUILD_JOBS = '16' }
if (-not [IO.Path]::IsPathRooted($TargetDir)) { throw "-TargetDir must be absolute (C:\tmp\nm-target): $TargetDir" }
$target = [IO.Path]::GetFullPath($TargetDir)
if ($target.Substring(0, 2) -ieq $repo.Substring(0, 2)) { throw "-TargetDir must not be on the repository's drive ($($repo.Substring(0, 2))): $target" }
# Forward slashes: Git Bash has mangled backslashes in this variable into a folder in the repo.
$env:CARGO_TARGET_DIR = $target.Replace('\', '/')
$appDir = Join-Path $repo 'app'
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$work = Join-Path $target "gates\m12\$stamp"
New-Item -ItemType Directory -Force $work | Out-Null
# What is untracked in the repository before the run: the run must add nothing to it (B: is exFAT).
function Untracked { @(cmd /c "git -C `"$repo`" status --porcelain --untracked-files=all 2>nul" | Where-Object { $_ -match '^\?\? ' }) }
$untrackedBefore = Untracked
$fullRun = $Only -eq 'all' -and -not $SkipPrior

# The e2e ids of the gate (PLAN.md, P2 and P3): none is built yet.
$gateIds = [ordered]@{
    'm12-a' = 'P2: DOM numbers == simpa results --json at the displayed precision'
    'm12-b' = 'P2: no DOM element for a parameter not PASS in beds/summary.json'
    'm12-c' = 'P3: 3 sampled texels == the .csbin float32s'
    'm12-d' = 'P3: rendered particle count == alive in the .pbin at 5 steps'
    'm12-e' = 'P2: DIN 18041 A3 target 0.55 s for the 180 m3 teaching room'
    'm12-f' = 'P2: the variant switch replaces the RT series, 0 mismatches'
}

$failures = @()
function Check($name, [scriptblock]$body) {
    try {
        $ok = & $body
        if ($ok) { Write-Host "PASS  $name" } else { Write-Host "FAIL  $name"; $script:failures += $name }
    } catch {
        Write-Host "FAIL  $name :: $($_.Exception.Message)"; $script:failures += $name
    }
}
function Note([string]$text) { Write-Host "      $text" }
# A native program through cmd, its output to a log file; returns the exit code.
function Native([string]$cmdline, [string]$log) {
    cmd /c "$cmdline > `"$log`" 2>&1"
    $LASTEXITCODE
}
function Tail([string]$log, [int]$n = 6) { if (Test-Path $log) { Get-Content $log -Tail $n | ForEach-Object { Note $_ } } }
# `cargo test` with its counts noted; true when it exits 0 and ran at least one test.
function CargoTest([string]$args_, [string]$name) {
    $log = Join-Path $work "$name.log"
    $code = Native "cargo test -q $args_" $log
    $results = @(Get-Content $log | Where-Object { $_ -match '^test result' })
    $passed = ($results | ForEach-Object { if ($_ -match '(\d+) passed') { [int]$Matches[1] } } | Measure-Object -Sum).Sum
    $failed = ($results | ForEach-Object { if ($_ -match '(\d+) failed') { [int]$Matches[1] } } | Measure-Object -Sum).Sum
    Note "$passed passed, $failed failed; log $log"
    if ($code -ne 0) { Tail $log 12 }
    $code -eq 0 -and $passed -gt 0
}

# ---- 1. the contracts (P1) -------------------------------------------------------------------------
Check "item 1: beds/summary.json is tools/bed/summary.py's output from the evidence under $BedData, byte for byte" {
    $env:SIMPA_BED_DATA = $BedData
    try { CargoTest '-p simpa-core --test bed_summary' 'bed-summary' } finally { Remove-Item Env:\SIMPA_BED_DATA }
}

Check "item 1: every parameter's derived status (gate (b) renders only PASS)" {
    $s = Get-Content (Join-Path $repo 'beds\summary.json') -Raw | ConvertFrom-Json
    $names = @($s.parameters.PSObject.Properties.Name)
    foreach ($n in $names) {
        $p = $s.parameters.$n
        Note ("{0,-7} {1}{2}" -f $n, $p.status, $(if ($p.reasons.Count) { "  ($($p.reasons[0]))" } else { '' }))
    }
    $names.Count -eq 11 -and @($names | Where-Object { @('PASS', 'FAIL') -notcontains $s.parameters.$_.status }).Count -eq 0
}

Check "item 2: the report carries each parameter's bed status from beds/summary.json (results version 12; the required fields pinned)" {
    $a = CargoTest '-p simpa-core --lib -- results::bed results::report::tests::the_required_fields_are_pinned_to_the_results_version' 'report-bed-unit'
    $b = CargoTest '-p simpa --test cli_results -- gate_e_seat_and_seat2 an_spps_report_carries the_committed_schema' 'report-bed-cli'
    $a -and $b
}

Check "item 3: the Results step's IPC reads, the CLI's report and the result files bit for bit (cargo test -p app results_data)" {
    CargoTest '-p app results_data' 'ipc-reads'
}

Check "item 3: the SMAP and PART decoders (node --test ui/src/resultsData.test.ts)" {
    $log = Join-Path $work 'ui-results-data.log'
    Push-Location $appDir
    try { $code = Native 'node --test ui/src/resultsData.test.ts' $log } finally { Pop-Location }
    Tail $log 4
    $code -eq 0
}

Check "item 4: m10-h and m11-h narrowed to no number outside the Results step, and a leak onto another step still fails (node --test e2e/lib/acoustic.test.ts)" {
    $log = Join-Path $work 'harness-acoustic.log'
    Push-Location $appDir
    try { $code = Native 'node --test e2e/lib/acoustic.test.ts' $log } finally { Pop-Location }
    Tail $log 4
    $code -eq 0 -and (Select-String -Path $log -Pattern 'M12: a number in the Results regions' -Quiet)
}

# ---- P2: the report's room, the Acoustics tab's model, and its e2e (gate (a), (b), (e), (f)) -------
Check "P2: the report carries the room: DIN 18041 targets (A3 at 180 m3 reads 0.55 s), absorption by surface group, SPPS Sabine (cli_results m12_; results version 12)" {
    CargoTest '-p simpa --test cli_results -- m12_' 'report-room'
}

Check "P2: run_report names each material id by the open project's surface groups (cargo test -p app the_surface_groups)" {
    CargoTest '-p app the_surface_groups' 'group-names'
}

Check "P2: the Acoustics tab's model and the e2e comparison rules (node --test, each with its say-NO)" {
    $log = Join-Path $work 'p2-node.log'
    Push-Location (Join-Path $repo 'app')
    try { $code = Native 'node --test ui/src/features/acoustics/model.test.ts e2e/lib/acoustics.test.ts' $log } finally { Pop-Location }
    Tail $log 8
    $code -eq 0
}

# The assay's fixes (ASSAY.md beside PLAN.md): no string the UI can show says "validated", on any
# step (the Acoustics tab's page check reads one step only); the particle count of gate (d) passes
# through the draw's own kept() gate, with no cull after the count branch.
Check "assay: no UI string says `"validated`" on any step, and the particle count goes through the draw's kept() gate (node --test ui/src/wording.test.ts ui/src/features/viewport/particles.test.ts)" {
    $log = Join-Path $work 'assay-node.log'
    Push-Location (Join-Path $repo 'app')
    try { $code = Native 'node --test ui/src/wording.test.ts ui/src/features/viewport/particles.test.ts' $log } finally { Pop-Location }
    Tail $log 8
    $code -eq 0
}

# app/e2e/specs/m12.acoustics.e2e.ts, run by `m11.ps1 -Only e2e -Spec m12.acoustics` on m11.ps1's
# harness (the release build, the drivers, the private verified solvers, the e2e lock, the focus
# watcher), as P3's spec is. The ids are read from its junit verdict lines.
# P4: gate (b)'s plant runs in the same call, its own session (specs/m12.bedplant.e2e.ts): with all
# eleven PASS in beds/summary.json, m12-b alone hides nothing, so (b) needs both ids.
$p2Spec = 'm12.acoustics,m12.bedplant'
$p2Ids = [ordered]@{
    'm12-a' = 'every number on the Acoustics tab == simpa results --json at the displayed precision, every selection; each cell''s visible row and column labels name its paths; no other digit; ranges, statuses, refusals; marks word for word; MQ2 words; no "validated"'
    'm12-b' = 'no element for a parameter not PASS in beds/summary.json, under every selection; every PASS parameter rendered with its range and status, T30 drawn with decision 46''s mark, EDT with both row 37 marks'
    'm12-b-plant' = '(b) not vacuous: C80 planted FAIL through core ($SIMPA_BED_DEMOTE) has 0 elements under every selection; the other ten rendered with range and status'
    'm12-e' = 'the DIN 18041 A3 target of the 180 m3 box reads 0.55 s, from the report; A1 reads its own'
    'm12-f' = "the variant switch replaces the RT series and numbers with the other run's JSON, 0 mismatches, both ways"
}
foreach ($id in $p2Ids.Keys) { $gateIds.Remove($id) }
if ($Only -eq 'all') {
    $script:p2Log = Join-Path $work 'p2-e2e.log'
    Check "P2 e2e: m11.ps1 -Only e2e -Spec $p2Spec exits 0" {
        $code = Native "powershell -NoProfile -ExecutionPolicy Bypass -File `"$repo\tools\gates\m11.ps1`" -Only e2e -Spec $p2Spec -TargetDir `"$target`" -SolversDir `"$SolversDir`" -Upstream `"$Upstream`"" $script:p2Log
        Get-Content $script:p2Log | Where-Object { $_ -match '^(PASS|FAIL) |^M11 |receipt' } | ForEach-Object { Note $_.Trim() }
        $code -eq 0
    }
    foreach ($id in $p2Ids.Keys) {
        Check "${id}: $($p2Ids[$id])" {
            (Test-Path $script:p2Log) -and @(Get-Content $script:p2Log | Where-Object { $_ -match "^\s+passed\s+[\d.,]+ s\s+$([regex]::Escape($id))\s" }).Count -eq 1
        }
    }
} else { Note "P2 e2e ($p2Spec): not run (-Only static)" }

# ---- P3: the viewport's e2e (gate (c), (d), MQ4), on m11.ps1's harness ---------------------------
# app/e2e/specs/m12.viewport.e2e.ts, run by `m11.ps1 -Only e2e -Spec m12.viewport`: the release
# build, tauri-driver and msedgedriver, the private verified solver copy, the projects on C:, the
# e2e lock and the focus watcher are m11.ps1's. The ids are read from its junit verdict lines.
$p3Spec = 'm12.viewport'
$p3Ids = [ordered]@{
    'm12-c'       = '3 sampled texels of the map read back from the GPU == the .csbin float32s, bit for bit'
    'm12-d'       = 'at 5 steps the particles drawn (counted on the GPU) == alive in the .pbin, map on the same step'
    'm12-mq4'     = 'MQ4: particles saved 0 says "No particles saved for this run" and how to turn it on, with the file size'
    'm12-p3-maps' = 'legend range, band choice, difference from the baseline run, Play on one timeline'
}
foreach ($id in @('m12-c', 'm12-d')) { $gateIds.Remove($id) }
if ($Only -eq 'all') {
    $script:p3Log = Join-Path $work 'p3-e2e.log'
    Check "P3 e2e: m11.ps1 -Only e2e -Spec $p3Spec exits 0" {
        $code = Native "powershell -NoProfile -ExecutionPolicy Bypass -File `"$repo\tools\gates\m11.ps1`" -Only e2e -Spec $p3Spec -TargetDir `"$target`" -SolversDir `"$SolversDir`" -Upstream `"$Upstream`"" $script:p3Log
        Get-Content $script:p3Log | Where-Object { $_ -match '^(PASS|FAIL) |^M11 |receipt' } | ForEach-Object { Note $_.Trim() }
        $code -eq 0
    }
    foreach ($id in $p3Ids.Keys) {
        Check "${id}: $($p3Ids[$id])" {
            (Test-Path $script:p3Log) -and @(Get-Content $script:p3Log | Where-Object { $_ -match "^\s+passed\s+[\d.,]+ s\s+$([regex]::Escape($id))\s" }).Count -eq 1
        }
    }
} else { Note "P3 e2e ($p3Spec): not run (-Only static)" }

# ---- 2. the gate's e2e ids (P2, P3; wired by P4) ---------------------------------------------------
if ($Only -eq 'all') {
    foreach ($id in $gateIds.Keys) {
        Check "$id NOT BUILT: $($gateIds[$id])" { $false }
    }
} else { Note 'the e2e ids (a)-(f): not run (-Only static)' }

# ---- 3. prior gate ---------------------------------------------------------------------------------
if (-not $SkipPrior -and $Only -eq 'all') {
    Check "prior gate: m11.ps1 in full prints M11 PASSED (it runs m10.ps1 and m9.ps1)" {
        $log = Join-Path $work 'm11.log'
        $code = Native "powershell -NoProfile -ExecutionPolicy Bypass -File `"$repo\tools\gates\m11.ps1`" -TargetDir `"$target`" -SolversDir `"$SolversDir`" -Upstream `"$Upstream`"" $log
        Get-Content $log | Where-Object { $_ -match '^(PASS|FAIL) |^M11 ' } | ForEach-Object { Note $_ }
        $code -eq 0 -and (Select-String -Path $log -Pattern '^M11 PASSED' -Quiet)
    }
} else { Note 'prior gate: not run (-SkipPrior or -Only static)' }

# ---- 4. file counts --------------------------------------------------------------------------------
$files = @(Get-ChildItem $work -Recurse -File)
Write-Host "`nwork folder (C:): $work ($($files.Count) files)"
$left = @(Untracked | Where-Object { $untrackedBefore -notcontains $_ })
Write-Host "files this run left in the repository (B:): $($left.Count)"
$left | Select-Object -First 10 | ForEach-Object { Write-Host "      $_" }
if ($left.Count) { $failures += 'files left on B:' }
if (-not $fullRun) {
    if ($failures.Count) { Write-Host "M12 PARTIAL RUN (-Only $Only$(if ($SkipPrior) { ' -SkipPrior' })) FAILED: $($failures.Count) check(s)"; exit 1 }
    Write-Host "M12 PARTIAL RUN (-Only $Only$(if ($SkipPrior) { ' -SkipPrior' })): its checks passed; this is not a gate pass"; exit 0
}
if ($failures.Count) { Write-Host "M12 FAILED: $($failures.Count) check(s)"; exit 1 }
Write-Host "M12 PASSED"; exit 0
