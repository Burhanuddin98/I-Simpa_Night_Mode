# M7 gate: result readers and our own acoustic parameters.
# Gate: docs/rebuild-plan-raw-2026-09-23.json, milestone M7, verbatim:
# (a) cargo test -p core --test params_synthetic. Exact exponential decays with T in {0.3, 1.0, 3.0} s
#     and dt in {0.001, 0.01} s give: T20, T30 and EDT within 0.5% of T; C80 within 0.01 dB and D50
#     within 0.1 percentage points of the closed-form values. (`core` is the crate simpa-core.)
# (b) ISO 9613-1. Our attenuation matches the standard's tabulated values at 20 C / 50% RH,
#     transcribed into the test with page references, within 1% relative. The 20 C values are GOST
#     31295.1-2005 Table 1 (i), the Russian adoption, not ISO's own page (docs/params.md, "Sources").
# (c) Level calibration. A 20x20x20 m box with direct_calc=1, abs_atmo_calc=0 and an omni source of
#     Lw 100 dB per band, receivers at r = 2 m and 4 m. Our SPL must be within +/-0.5 dB of
#     Lw - 20*log10(r) - 11. This catches offsets of the kind Night Mode's 26 dB .gap bug made.
# (d) TCR cross-check. On the tutorial-1 box, TCR's per-band Sabine and Eyring RT equal our analytic
#     values within 0.5%, and no NaN appears in any output ("any value we display": the tables'
#     Global rows are NaN by design, docs/rebuild-plan.md, critic's flaw 1).
# (e) A fixture with receiver folders 'Seat' and 'Seat2' reads both.
# (f) DIN 18041 A3 target for V = 180 m3 = 0.552 +/- 0.001 s.
# Drives the release CLI and the tests:
#   simpa run <project> --solver spps|tcr --runs <root> --json     simpa results <run> --json
#   simpa dump gabe|csbin <file>                                    cargo test -p simpa-core|simpa
# Every check that can pass has a "says NO" partner that must refuse an input:
# - (a), (b), (f): the gate's predicate is applied here to the numbers the tests print, and the
#   tests' own refusals must run and pass: a decay 1 % off fails every bound, air 1 C or 5 % RH off
#   misses the table, a natural-log or neighbouring-group DIN formula misses 0.552 s;
# - (c): Night Mode's .gap level (energy over 1e-12, main:project/result_parser.cpp:486) and the
#   SPL moved 1 dB either way miss the bound in every band;
# - (d): the walls' alpha 5 % higher, run through TCR, gives analytic times outside 0.5 % of the
#   unchanged run in every band; a NaN planted in a copy of the run is found by the scan and the
#   copy is refused by simpa results;
# - (e): a copy with the two folders swapped reads the series swapped, and one with a third folder
#   'Seat3' is refused (exit 6);
# - the tail: cli_results' solver test must FAIL, with its panic, when the solvers are missing.
# Every solver call has a hard 5-minute timeout: a timeout is a FAIL with its time.
# Needs the M1 solver build ($env:SIMPA_SOLVERS_DIR, else target\solvers\bin) and, for the tail,
# upstream's tree ($env:SIMPA_UPSTREAM, else target\solvers\src-929a5c8).
# Run: powershell -File tools/gates/m7.ps1
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Set-Location $repo
$env:RUSTUP_HOME = "$env:USERPROFILE\.rustup"; $env:CARGO_HOME = "$env:USERPROFILE\.cargo"
$env:Path = "$env:CARGO_HOME\bin;$env:Path"; $env:CARGO_INCREMENTAL = '0'
$env:CARGO_TARGET_DIR = Join-Path $repo 'target'
$env:SIMPA_REQUIRE_UPSTREAM = '1'
if (-not $env:SIMPA_SOLVERS_DIR) { $env:SIMPA_SOLVERS_DIR = Join-Path $repo 'target\solvers\bin' }
$failures = @(); $script:checks = 0
# A check body returns exactly one bool. Anything else is a FAIL: a stray value leaking into the
# pipeline must never turn into a PASS.
function Check($name, [scriptblock]$body) {
    $script:checks++
    try {
        $r = @(& $body)
        if ($r.Count -eq 1 -and $r[0] -is [bool] -and $r[0]) {
            Write-Host "PASS  $name"
        } elseif ($r.Count -eq 1 -and $r[0] -is [bool]) {
            Write-Host "FAIL  $name"; $script:failures += $name
        } else {
            Write-Host "FAIL  $name :: the check returned $($r.Count) values, not one bool"; $script:failures += $name
        }
    } catch {
        Write-Host "FAIL  $name :: $($_.Exception.Message)"; $script:failures += $name
    }
}

$build = cmd /c "cargo build -q --release -p simpa 2>&1"
if ($LASTEXITCODE -ne 0) { $build | Select-Object -Last 20 | ForEach-Object { Write-Host $_ }; throw 'CLI build failed: refusing to test a stale simpa.exe' }
# Runs a cargo command; on failure prints its last lines so a FAIL is never silent. Its whole
# output stays in $script:cargoText.
function Cargo([string]$cmdline) {
    $out = cmd /c "$cmdline 2>&1"
    $code = $LASTEXITCODE
    $script:cargoText = (@($out) | ForEach-Object { "$_" }) -join "`n"
    if ($code -ne 0) { $out | Select-Object -Last 15 | ForEach-Object { Write-Host "      | $_" }; return $false }
    return $true
}
# A cargo command with extra environment variables, restored afterwards; its failure is not echoed.
function CargoQuiet([string]$cmdline, [hashtable]$vars = @{}) {
    $saved = @{}
    foreach ($k in $vars.Keys) { $saved[$k] = [Environment]::GetEnvironmentVariable($k); [Environment]::SetEnvironmentVariable($k, $vars[$k]) }
    try { $ok = Cargo $cmdline 6>$null } finally { foreach ($k in $saved.Keys) { [Environment]::SetEnvironmentVariable($k, $saved[$k]) } }
    [pscustomobject]@{ Ok = $ok; Text = $script:cargoText }
}
# One named test, which must run exactly once and pass; its output in $script:cargoText.
function OneTest([string]$crate, [string]$target, [string]$test) {
    $ok = Cargo "cargo test -p $crate --test $target $test -- --exact --nocapture"
    $ran = $script:cargoText -match 'test result: ok\. 1 passed'
    if (-not $ran) { Write-Host "      $test did not run exactly once and pass" }
    return ($ok -and $ran)
}
$simpa = Join-Path $repo 'target\release\simpa.exe'
$fx = Join-Path $repo 'tests\fixtures'
$work = Join-Path $repo ('target\gates\m7\' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
$calls = Join-Path $work 'calls'
New-Item -ItemType Directory -Force $work, $calls | Out-Null
Write-Host "work: $work"
Write-Host "solvers: $env:SIMPA_SOLVERS_DIR"

# One argument as the MSVC runtime (and Rust's std) splits a command line.
function QuoteArg([string]$a) {
    if ($a.Length -gt 0 -and $a -notmatch '[\s"]') { return $a }
    $s = '"'; $bs = 0
    foreach ($ch in $a.ToCharArray()) {
        if ($ch -eq [char]'\') { $bs++; continue }
        if ($ch -eq [char]'"') { $s += ('\' * (2 * $bs + 1)) + '"'; $bs = 0; continue }
        $s += ('\' * $bs) + $ch; $bs = 0
    }
    return $s + ('\' * (2 * $bs)) + '"'
}
function ReadText([string]$path) {
    if (-not (Test-Path -LiteralPath $path)) { return '' }
    $fs = New-Object IO.FileStream -ArgumentList $path, 'Open', 'Read', 'ReadWrite'
    try { return (New-Object IO.StreamReader -ArgumentList $fs, ([Text.Encoding]::UTF8)).ReadToEnd() } finally { $fs.Dispose() }
}
# simpa with a hard time limit; stdout and stderr go to numbered files under calls\. .Json holds
# stdout parsed when it is a JSON object.
$script:callNo = 0
function Simpa([string[]]$argv, [string]$label, [int]$limitSec = 300) {
    $script:callNo++
    $base = Join-Path $calls ('{0:D3}-{1}' -f $script:callNo, $label)
    $clock = [Diagnostics.Stopwatch]::StartNew()
    $p = Start-Process -FilePath $simpa -ArgumentList (($argv | ForEach-Object { QuoteArg $_ }) -join ' ') `
        -RedirectStandardOutput "$base.stdout.txt" -RedirectStandardError "$base.stderr.txt" -NoNewWindow -PassThru
    $null = $p.Handle
    if (-not $p.WaitForExit($limitSec * 1000)) {
        Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
        $null = $p.WaitForExit(10000)
        throw ('TIMEOUT: {0} still ran after {1:N1} s (limit {2} s) and was killed; logs {3}.*' -f $label, $clock.Elapsed.TotalSeconds, $limitSec, $base)
    }
    $p.WaitForExit()
    $out = ReadText "$base.stdout.txt"
    $json = $null
    if ($out.TrimStart().StartsWith('{')) { $json = $out | ConvertFrom-Json }
    [pscustomobject]@{ Exit = $p.ExitCode; Out = $out; Err = (ReadText "$base.stderr.txt"); Json = $json; Sec = $clock.Elapsed.TotalSeconds }
}
# `simpa run <project> --solver <s>`, which must exit 0 OK; the run folder.
function RunOk([string]$project, [string]$solver, [string]$root, [string]$label) {
    $o = Simpa @('run', $project, '--solver', $solver, '--runs', $root, '--json') $label
    if ($o.Exit -ne 0 -or $null -eq $o.Json -or $o.Json.verdict.status -ne 'OK') { throw "$label`: exit $($o.Exit), $($o.Err.Trim() -split "`n" | Select-Object -Last 1)" }
    Split-Path -Parent $o.Json.cwd
}
# `simpa results <run> --json`, which must exit 0; the report.
function Results([string]$run, [string]$label) {
    $o = Simpa @('results', $run, '--json') $label
    if ($o.Exit -ne 0 -or $null -eq $o.Json) { throw "$label`: results exit $($o.Exit): $($o.Err.Trim())" }
    $o.Json
}
function CopyTree([string]$from, [string]$to) {
    New-Item -ItemType Directory -Force (Split-Path -Parent $to) | Out-Null
    Copy-Item -LiteralPath $from -Destination $to -Recurse
    $to
}
# The float value of an f32 hex token of a canonical dump.
function F32([string]$hex) { [BitConverter]::ToSingle([BitConverter]::GetBytes([Convert]::ToUInt32($hex, 16)), 0) }
function NonFinite([string]$hex) { ([Convert]::ToUInt32($hex, 16) -band 0x7f800000) -eq 0x7f800000 }
# Every float value of a .gabe dump's rows not labelled Global, as (column, row, hex).
function GabeValues([string]$file) {
    $dump = (& $simpa dump gabe $file) -split "`n"
    if ($LASTEXITCODE -ne 0) { throw "dump gabe $file failed" }
    $labels = @(); $vals = @(); $col = -1; $type = ''; $row = 0
    foreach ($l in $dump) {
        $l = $l.TrimEnd()
        if ($l -match '^column (\S+) (.*)$') { $col++; $type = $Matches[1]; $name = $Matches[2]; $row = 0; continue }
        if ($l -match '^(gabe|readonly|columns|rows|digits) ') { continue }
        if ($col -lt 0) { continue }
        if ($col -eq 0) { $labels += $l; continue }
        if ($type -eq 'float') {
            if ($labels[$row] -ne 'Global') { $vals += [pscustomobject]@{ Column = $name; Row = $labels[$row]; Hex = $l } }
            $row++
        }
    }
    $vals
}
# Every record value of a .csbin dump, as hex.
function CsbinValues([string]$file) {
    $dump = (& $simpa dump csbin $file) -split "`n"
    if ($LASTEXITCODE -ne 0) { throw "dump csbin $file failed" }
    @($dump | Where-Object { $_ -match '^record \d+ ([0-9a-f]{8})' } | ForEach-Object { $Matches[1] })
}
function Params($band) { $band.parameters }

# --- (a) decays ----------------------------------------------------------------------------------
$aPattern = '(?m)^T (\S+) s, dt (\S+) s: EDT (\S+), T20 (\S+), T30 (\S+) \(relative\); C50 (\S+) dB, C80 (\S+) dB, D50 (\S+) points; Ts (\S+) \(relative\)'
Check "(a) cargo test -p simpa-core --test params_synthetic passes" { Cargo 'cargo test -q -p simpa-core --test params_synthetic' }
Check "(a) six exact decays, T 0.3/1/3 s x dt 1/10 ms: T20, T30, EDT within 0.5 %, C80 within 0.01 dB, D50 within 0.1 points" {
    if (-not (OneTest 'simpa-core' 'params_synthetic' 'exact_decays_meet_every_bound')) { return $false }
    $m = [regex]::Matches($script:cargoText, $aPattern)
    $ok = $m.Count -eq 6
    foreach ($x in $m) {
        $g = $x.Groups
        $times = @([double]$g[3].Value, [double]$g[4].Value, [double]$g[5].Value)
        $pass = ($times | Where-Object { $_ -gt 0.005 }).Count -eq 0 -and [double]$g[7].Value -le 0.01 -and [double]$g[8].Value -le 0.1
        Write-Host ("      T {0} s dt {1} s: EDT {2}, T20 {3}, T30 {4} rel; C80 {5} dB; D50 {6} pt: {7}" -f $g[1].Value, $g[2].Value, $g[3].Value, $g[4].Value, $g[5].Value, $g[7].Value, $g[8].Value, $(if ($pass) { 'within' } else { 'OUTSIDE' }))
        if (-not $pass) { $ok = $false }
    }
    $ok
}
Check "(a) says NO: a decay 1 % off fails all seven bounds, in all six cases" { OneTest 'simpa-core' 'params_synthetic' 'a_decay_one_percent_off_fails_every_bound' }
Check "(a) says NO: a series cut before -35 dB gives range_not_reached for T30, not a number" { OneTest 'simpa-core' 'params_synthetic' 'a_series_cut_before_minus_35_db_gives_not_evaluable_not_a_number' }

# --- (b) ISO 9613-1 ------------------------------------------------------------------------------
Check "(b) ISO 9613-1 at 20 C, 50 % RH: every band of the table within 1 % relative" {
    if (-not (OneTest 'simpa-core' 'params_air' 'the_gate_20c_50rh_matches_the_table_within_one_percent')) { return $false }
    $m = [regex]::Matches($script:cargoText, '(?m)^\s*(\d+) Hz: ours\s+(\S+) dB/km, table\s+(\S+) dB/km, ([+-]\S+) %')
    $worst = ($m | ForEach-Object { [math]::Abs([double]$_.Groups[4].Value) } | Measure-Object -Maximum).Maximum
    $outside = @($m | Where-Object { [math]::Abs([double]$_.Groups[4].Value) -gt 1.0 })
    Write-Host ("      {0} bands, worst {1:N3} %, {2} outside 1 %" -f $m.Count, $worst, $outside.Count)
    $m.Count -eq 24 -and $outside.Count -eq 0
}
Check "(b) says NO: 1 C or 5 % RH off puts at least 20 of the 24 bands outside 1 %" {
    if (-not (OneTest 'simpa-core' 'params_air' 'one_degree_or_five_percent_humidity_off_fails')) { return $false }
    $m = [regex]::Matches($script:cargoText, '(?m)^(.+?): (\d+) of 24 bands outside 1 %')
    $m | ForEach-Object { Write-Host "      $($_.Value)" }
    $m.Count -eq 4 -and @($m | Where-Object { [int]$_.Groups[2].Value -lt 20 }).Count -eq 0
}

# --- (c) level calibration ------------------------------------------------------------------------
function GateC([double]$spl, [double]$lw, [double]$r) { [math]::Abs($spl - ($lw - 20 * [math]::Log10($r) - 11)) -le 0.5 }
$script:levelRows = $null
function LevelRows {
    if ($null -ne $script:levelRows) { return , $script:levelRows }
    $run = RunOk (Join-Path $fx 'rooms\level_box_20m.simpa') 'spps' (Join-Path $work 'level') 'level-spps'
    $rep = Results $run 'level-results'
    [xml]$cfg = ReadText (Join-Path $run 'solve\config.xml')
    $lw = @{}; foreach ($b in $cfg.configuration.sources.source.bfreq) { $lw[[int]$b.freq] = [double][single][double]::Parse($b.db, [Globalization.CultureInfo]::InvariantCulture) }
    $src = $rep.spps.sources[0].position_m
    $rows = @()
    foreach ($r in $rep.spps.point_receivers) {
        $p = $r.position_m
        $dist = [math]::Sqrt([math]::Pow($p[0] - $src[0], 2) + [math]::Pow($p[1] - $src[1], 2) + [math]::Pow($p[2] - $src[2], 2))
        foreach ($b in $r.bands) {
            $spl = (Params $b).spl_db.value
            if ($null -eq $spl) { throw "$($r.label) $($b.freq_hz) Hz: SPL not evaluable: $((Params $b).spl_db.not_evaluable.message)" }
            $rows += [pscustomobject]@{ Label = $r.label; F = [int]$b.freq_hz; R = $dist; Spl = [double]$spl; Total = [double]$b.total_pa2; Lw = $lw[[int]$b.freq_hz] }
        }
    }
    $script:levelRows = $rows
    , $rows
}
Check "(c) level box through SPPS: SPL within +/-0.5 dB of Lw - 20 lg r - 11 at 2 m and 4 m, every band, Lw 100 dB as SPPS reads it" {
    $rows = LevelRows
    $ok = $rows.Count -eq 12
    foreach ($x in $rows) {
        $want = $x.Lw - 20 * [math]::Log10($x.R) - 11
        $pass = (GateC $x.Spl $x.Lw $x.R) -and ([math]::Abs($x.Lw - 100) -lt 1e-9)
        Write-Host ("      {0} {1,5} Hz r {2:N4} m: Lw {3} dB, SPL {4:N3} dB, bound {5:N3} dB ({6:+0.000;-0.000}): {7}" -f $x.Label, $x.F, $x.R, $x.Lw, $x.Spl, $want, ($x.Spl - $want), $(if ($pass) { 'within' } else { 'OUTSIDE' }))
        if (-not $pass) { $ok = $false }
    }
    $ok
}
Check "(c) says NO: Night Mode's .gap level, energy / 1e-12 (main:project/result_parser.cpp:486), misses the bound in every band" {
    $line = @(git show main:project/result_parser.cpp)[485]
    Write-Host "      result_parser.cpp:486: $($line.Trim())"
    $rows = LevelRows
    $caught = @($rows | Where-Object { -not (GateC (10 * [math]::Log10($_.Total / 1e-12)) $_.Lw $_.R) })
    $gap = ($rows | ForEach-Object { 10 * [math]::Log10($_.Total / 1e-12) - $_.Spl } | Measure-Object -Average).Average
    Write-Host ("      Night Mode's level reads {0:N3} dB above ours; caught in {1} of {2} bands" -f $gap, $caught.Count, $rows.Count)
    $line.Contains('/ 1e-12f') -and $caught.Count -eq $rows.Count
}
Check "(c) says NO: the SPL moved 1 dB up or down misses the bound in every band" {
    $rows = LevelRows
    $caught = @($rows | Where-Object { -not (GateC ($_.Spl + 1) $_.Lw $_.R) -and -not (GateC ($_.Spl - 1) $_.Lw $_.R) })
    Write-Host "      caught in $($caught.Count) of $($rows.Count) bands"
    $caught.Count -eq $rows.Count
}

# --- (d) TCR against the analytic values ----------------------------------------------------------
$tut = Join-Path $fx 'rooms\tutorial1_box_seeded.simpa'
$script:tcrRun = $null
function TcrRun {
    if ($null -eq $script:tcrRun) { $script:tcrRun = RunOk $tut 'tcr' (Join-Path $work 'tcr') 'tutorial-tcr' }
    $script:tcrRun
}
function Within([double]$a, [double]$b) { [math]::Abs($a / $b - 1) -le 0.005 }
Check "(d) tutorial-1 box through TCR: per band, TCR's Sabine and Eyring times equal core::params' on the run's inputs within 0.5 %" {
    $rep = Results (TcrRun) 'tutorial-tcr-results'
    $t = $rep.tcr
    if ($t.analytic.status -ne 'computed') { throw "analytic: $($t.analytic.why)" }
    $worst = 0.0; $bad = @()
    for ($i = 0; $i -lt $t.bands.Count; $i++) {
        $b = $t.bands[$i]; $a = $t.analytic.bands[$i]
        foreach ($pair in @(@($b.sabine.reverberation_time_s, $a.sabine_s.value), @($b.eyring.reverberation_time_s, $a.eyring_s.value))) {
            if ($null -eq $pair[1]) { $bad += "$($b.freq_hz) Hz not evaluable"; continue }
            $worst = [math]::Max($worst, [math]::Abs([double]$pair[0] / [double]$pair[1] - 1))
            if (-not (Within $pair[0] $pair[1])) { $bad += "$($b.freq_hz) Hz: TCR $($pair[0]) vs $($pair[1])" }
        }
    }
    Write-Host ("      {0} bands x 2 theories, worst |TCR/ours - 1| = {1:E2}; volume {2} m3, area {3} m2" -f $t.bands.Count, $worst, $t.analytic.volume_m3, $t.analytic.area_m2)
    $bad | Select-Object -First 5 | ForEach-Object { Write-Host "      $_" }
    $t.bands.Count -eq 27 -and $bad.Count -eq 0
}
Check "(d) the same from the project itself (cargo test gate_d_...): within 0.5 %, and the project's walls 5 % more absorbing outside it" {
    OneTest 'simpa' 'cli_results' 'gate_d_tcr_equals_the_analytic_sabine_and_eyring_and_says_no'
}
Check "(d) no NaN or infinity in any value TCR wrote for display: band rows of every table, every .csbin value" {
    $solve = Join-Path (TcrRun) 'solve'
    $n = 0; $bad = @()
    foreach ($f in Get-ChildItem -LiteralPath $solve -Recurse -File) {
        if ($f.Extension -eq '.gabe') {
            foreach ($v in GabeValues $f.FullName) { $n++; if (NonFinite $v.Hex) { $bad += "$($f.Name) $($v.Column) $($v.Row)" } }
        } elseif ($f.Extension -eq '.csbin') {
            foreach ($h in CsbinValues $f.FullName) { $n++; if (NonFinite $h) { $bad += $f.FullName } }
        }
    }
    Write-Host "      $n values scanned, $($bad.Count) not finite"
    $n -gt 1000 -and $bad.Count -eq 0
}
Check "(d) says NO: the walls' alpha 5 % higher (0.2 -> 0.21), through TCR, gives analytic times outside 0.5 % of the unchanged run in every band" {
    $text = [IO.File]::ReadAllText($tut)
    $old = '"absorption": [' + ((@('0.2') * 27) -join ', ') + ']'
    if (([regex]::Matches($text, [regex]::Escape($old))).Count -ne 1) { throw 'the walls material is not found exactly once' }
    $moved = Join-Path $work 'tutorial1_walls_plus_5_percent.simpa'
    [IO.File]::WriteAllText($moved, $text.Replace($old, '"absorption": [' + ((@('0.21') * 27) -join ', ') + ']'), (New-Object Text.UTF8Encoding $false))
    $a = (Results (RunOk $moved 'tcr' (Join-Path $work 'tcr-moved') 'moved-tcr') 'moved-results').tcr.analytic
    $t = (Results (TcrRun) 'tutorial-tcr-results-2').tcr
    $out = 0
    for ($i = 0; $i -lt $t.bands.Count; $i++) {
        $s = -not (Within $t.bands[$i].sabine.reverberation_time_s $a.bands[$i].sabine_s.value)
        $e = -not (Within $t.bands[$i].eyring.reverberation_time_s $a.bands[$i].eyring_s.value)
        if ($s -and $e) { $out++ }
    }
    $first = $t.bands[0]
    Write-Host ("      50 Hz: TCR Sabine {0:N4} s vs moved {1:N4} s; outside 0.5 % in {2} of {3} bands" -f $first.sabine.reverberation_time_s, $a.bands[0].sabine_s.value, $out, $t.bands.Count)
    $out -eq $t.bands.Count
}
Check "(d) says NO: a NaN planted in a band row of a copy's Main results is found by the scan, and simpa results refuses the copy (exit 6, results_outputs_invalid)" {
    $copy = CopyTree (TcrRun) (Join-Path $work 'tcr-nan\run')
    $main = Join-Path $copy 'solve\Main results.gabe'
    $vals = GabeValues $main
    $target = $vals | Where-Object { $_.Column -like 'TR_Sabine*' -and $_.Row -eq '100\x20Hz' } | Select-Object -First 1
    if ($null -eq $target) { throw 'no TR_Sabine value at 100 Hz' }
    $bytes = [IO.File]::ReadAllBytes($main)
    $want = [BitConverter]::GetBytes([Convert]::ToUInt32($target.Hex, 16))
    $at = -1
    for ($k = 0; $k -le $bytes.Length - 4; $k++) { if ($bytes[$k] -eq $want[0] -and $bytes[$k + 1] -eq $want[1] -and $bytes[$k + 2] -eq $want[2] -and $bytes[$k + 3] -eq $want[3]) { $at = $k; break } }
    if ($at -lt 0) { throw 'the value is not in the file' }
    $nan = [BitConverter]::GetBytes([uint32]0x7fc00000); for ($k = 0; $k -lt 4; $k++) { $bytes[$at + $k] = $nan[$k] }
    [IO.File]::WriteAllBytes($main, $bytes)
    $found = @(GabeValues $main | Where-Object { NonFinite $_.Hex })
    $o = Simpa @('results', $copy, '--json') 'nan-results'
    Write-Host "      scan finds $($found.Count) ($($found[0].Column) at $($found[0].Row)); simpa results exit $($o.Exit), $($o.Json.refused.code)"
    $found.Count -eq 1 -and $found[0].Column -like 'TR_Sabine*' -and $found[0].Row -eq '100\x20Hz' -and $o.Exit -eq 6 -and $o.Json.refused.code -eq 'results_outputs_invalid'
}

# --- (e) Seat and Seat2 --------------------------------------------------------------------------
# The 500 Hz column of a receiver folder's .recp, as dump hex.
function Recp500([string]$run, [string]$label) {
    $vals = GabeValues (Join-Path $run "solve\Punctual receivers\$label\Sound level.recp")
    @($vals | Where-Object { $_.Column -eq '500\x20Hz' } | ForEach-Object { $_.Hex })
}
function Hex32([double]$v) { '{0:x8}' -f [BitConverter]::ToUInt32([BitConverter]::GetBytes([single]$v), 0) }
function Series500($rep) {
    $out = [ordered]@{}
    foreach ($r in $rep.spps.point_receivers) { $out[$r.label] = @($r.bands[0].energy_pa2 | ForEach-Object { Hex32 $_ }) }
    $out
}
$seatsSpps = Join-Path $fx 'results\seats_spps'
$seatsTcr = Join-Path $fx 'results\seats_tcr'
Check "(e) the committed SPPS run reads both Seat and Seat2, each from its own folder, and they differ" {
    $s = Series500 (Results $seatsSpps 'seats-spps')
    $labels = @($s.Keys)
    $own = @($labels | Where-Object { (@($s[$_]) -join ',') -eq ((Recp500 $seatsSpps $_) -join ',') })
    Write-Host "      receivers [$($labels -join ', ')], each equal to its own folder's .recp: [$($own -join ', ')]"
    ($labels -join ',') -eq 'Seat,Seat2' -and $own.Count -eq 2 -and ((@($s['Seat']) -join ',') -ne (@($s['Seat2']) -join ','))
}
Check "(e) the committed TCR run reads both Seat.gabe and Seat2.gabe, each its own" {
    $rep = Results $seatsTcr 'seats-tcr'
    $ok = ((@($rep.tcr.point_receivers | ForEach-Object { $_.label })) -join ',') -eq 'Seat,Seat2'
    foreach ($r in $rep.tcr.point_receivers) {
        $direct = GabeValues (Join-Path $seatsTcr "solve\Punctual receivers\$($r.label).gabe") | Where-Object { $_.Column -like 'Direct*' -and $_.Row -eq '500\x20Hz' }
        $same = (Hex32 $r.bands[0].direct_db) -eq $direct.Hex
        Write-Host ("      {0}: direct {1:N2} dB at 500 Hz, its own file's {2}" -f $r.label, $r.bands[0].direct_db, $(if ($same) { 'equal' } else { 'DIFFERENT' }))
        if (-not $same) { $ok = $false }
    }
    $ok
}
Check "(e) says NO: a copy with the Seat and Seat2 folders swapped reads the two series swapped" {
    $copy = CopyTree $seatsSpps (Join-Path $work 'seats-swapped\run')
    $pr = Join-Path $copy 'solve\Punctual receivers'
    Rename-Item (Join-Path $pr 'Seat') 'tmp'; Rename-Item (Join-Path $pr 'Seat2') 'Seat'; Rename-Item (Join-Path $pr 'tmp') 'Seat2'
    $a = Series500 (Results $seatsSpps 'seats-original'); $b = Series500 (Results $copy 'seats-swapped')
    ((@($b['Seat']) -join ',') -eq (@($a['Seat2']) -join ',')) -and ((@($b['Seat2']) -join ',') -eq (@($a['Seat']) -join ','))
}
Check "(e) says NO: a copy with a third folder 'Seat3' is refused, exit 6, results_file_invalid" {
    $copy = CopyTree $seatsSpps (Join-Path $work 'seats-third\run')
    Copy-Item -LiteralPath (Join-Path $copy 'solve\Punctual receivers\Seat2') -Destination (Join-Path $copy 'solve\Punctual receivers\Seat3') -Recurse
    $o = Simpa @('results', $copy, '--json') 'seats-third'
    Write-Host "      exit $($o.Exit): $($o.Err.Trim())"
    $o.Exit -eq 6 -and $o.Json.refused.code -eq 'results_file_invalid' -and $o.Err.Contains('Seat3')
}

# --- (f) DIN 18041 --------------------------------------------------------------------------------
function GateF([double]$t) { [math]::Abs($t - 0.552) -le 0.001 }
Check "(f) DIN 18041 A3 at 180 m3 is 0.552 +/- 0.001 s" {
    if (-not (OneTest 'simpa-core' 'params_room' 'gate_f_din_18041_a3_at_180_m3_is_0_552_s')) { return $false }
    if ($script:cargoText -notmatch 'A3, 180 m.+?: ([0-9.]+) s') { throw 'the test printed no A3 value' }
    $t = [double]$Matches[1]
    Write-Host "      A3, 180 m3: $t s"
    GateF $t
}
Check "(f) says NO: the natural log for lg (1.4917 s) and group A2's formula (0.6944 s) miss 0.552 +/- 0.001 s" {
    $ln = 0.32 * [math]::Log(180) - 0.17; $a2 = 0.37 * [math]::Log10(180) - 0.14
    Write-Host ("      ln: {0:N4} s, A2: {1:N4} s" -f $ln, $a2)
    -not (GateF $ln) -and -not (GateF $a2)
}

# --- the tail -------------------------------------------------------------------------------------
Check "tests say NO without their inputs: SIMPA_SOLVERS_DIR naming a missing folder makes gate (c)'s test FAIL with the panic naming it" {
    $none = Join-Path $work 'no-such-folder'
    $s = CargoQuiet 'cargo test -q -p simpa --test cli_results gate_c_level_calibration_and_the_offsets_it_catches' @{ SIMPA_SOLVERS_DIR = $none }
    $hit = $s.Text.Contains("$none\spps.exe is missing")
    Write-Host "      solvers missing: $(if ($s.Ok) { 'PASSED' } else { 'failed' }), its panic seen: $hit"
    -not $s.Ok -and $hit
}
Check "cargo test -p simpa-core -p simpa" { Cargo 'cargo test -q -p simpa-core -p simpa' }
Check "clippy -D warnings (core and CLI)" { Cargo 'cargo clippy -q -p simpa-core -p simpa --all-targets -- -D warnings' }
Check "cargo fmt --check (core and CLI)" { Cargo 'cargo fmt -p simpa-core -p simpa --check' }

Write-Host "`nwork: $work"
$passed = $script:checks - $failures.Count
if ($failures.Count) { Write-Host "M7 FAILED: $($failures.Count) of $script:checks checks failed, $passed passed: $($failures -join '; ')"; exit 1 }
Write-Host "M7 PASSED: $script:checks of $script:checks checks"; exit 0
