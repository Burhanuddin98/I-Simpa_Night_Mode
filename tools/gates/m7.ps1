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
# Every check that can pass has a "says NO" partner that must refuse an input, with the fault in
# the code or the input, never added to a number the correct run produced (M7 follow-ups; the
# code faults are simpa_core::faults, compiled only into test builds):
# - (a), (b), (f): the gate's predicate is applied here to the numbers the tests print, and the
#   tests' own refusals must run and pass: a decay 1 % off fails every bound, a decay kinked above
#   -5 dB fails EDT only and one kinked below -25 dB T30 only (a wrong regression range breaks
#   that), air 1 C or 5 % RH off misses the table; the natural log put into the DIN code, and the
#   code's A2 and A4 formulas, miss 0.552 s. (a) is also run with the arrival detected;
# - (c): the level code path's reference constant replaced by Night Mode's 1e-12
#   (main:project/result_parser.cpp:486), and 1 dB off either way, misses the bound in every band.
#   The reverberant field is kept out by the 20 ms duration (no particle reaches a wall), which the
#   statistics must show; a run whose walls are reached and reflect fails that check. Beyond the
#   plan's bound (M7 review), the same run is held to the exact free field within its Monte-Carlo
#   noise, and the reference 0.15 dB off either way, or as rho c = 400 would read, misses that;
# - (d): the walls' alpha 5 % higher, run through TCR, gives analytic times outside 0.5 % of the
#   unchanged run in every band; one plane's alpha raised past the smallest increase the gate
#   resolves (measured) is caught, and 2 % below it is not; the air term dropped, or taken at ISO's
#   exact midband, misses TCR in some band; a NaN planted in a copy of the run is found by the scan
#   and the copy is refused by simpa results. Beyond the plan's box (M7 review), an asymmetric box,
#   where a mean over faces or materials, a swap of two groups' materials or a band's neighbour's
#   absorption each miss TCR by more than 0.5 % in every band;
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
# Main results' Global row for the areas and times: NaN by design (ctr/input_output/reportmanager.cpp:208).
# Every other value, receiver tables' Global rows included, must be finite.
function ByDesign([string]$file, $v) {
    (Split-Path -Leaf $file) -eq 'Main results.gabe' -and $v.Row -eq 'Global' -and ($v.Column -like 'A_*' -or $v.Column -like 'TR_*')
}
# Every float value of a .gabe dump, as (column, row, hex).
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
            $vals += [pscustomobject]@{ Column = $name; Row = $labels[$row]; Hex = $l }
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
# The plan's direct-arrival detection on the gate's own decays (the M7 critic: only the given
# arrival had been run): decay times pass; C50, C80 and D50 are refused unresolved, the truth
# bracketed, never returned as numbers.
Check "(a) with the arrival detected (Arrival::Detected): the six decays give EDT, T20 and T30 within 0.5 %, and C50, C80 and D50 refused unresolved with the closed form between the two ends of the onset bin" {
    if (-not (OneTest 'simpa-core' 'params_synthetic' 'gate_a_decays_with_the_arrival_detected')) { return $false }
    $m = [regex]::Matches($script:cargoText, '(?m)^detected, T (\S+) s, dt (\S+) s: EDT (\w+), T20 (\w+), T30 (\w+), C50 (\w+), C80 (\w+), D50 (\w+), Ts (\w+)')
    $m | ForEach-Object { Write-Host "      $($_.Value)" }
    $m.Count -eq 6 -and @($m | Where-Object { $_.Groups[3].Value -ne 'pass' -or $_.Groups[4].Value -ne 'pass' -or $_.Groups[5].Value -ne 'pass' -or $_.Groups[7].Value -ne 'unresolved' -or $_.Groups[8].Value -ne 'unresolved' }).Count -eq 0
}
Check "(a) says NO: a decay 1 % off fails all seven bounds, in all six cases" { OneTest 'simpa-core' 'params_synthetic' 'a_decay_one_percent_off_fails_every_bound' }
Check "(a) says NO: a series cut before -35 dB gives range_not_reached for T30, not a number" { OneTest 'simpa-core' 'params_synthetic' 'a_series_cut_before_minus_35_db_gives_not_evaluable_not_a_number' }
# On an exact exponential every sub-range gives the same slope, so the six decays above cannot see
# a wrong regression range (M7 review). A decay 5 % faster above -5 dB must fail EDT only, and one
# 5 % slower below -25 dB T30 only: a T20 fitted over -5..-35 dB, a T30 over -5..-25 dB, or an EDT
# or T20 starting at 0 or -5 dB where it should not, each breaks the pattern.
Check "(a) says NO to a wrong regression range: a decay kinked above -5 dB fails EDT only, one kinked below -25 dB fails T30 only, for T 0.3/1/3 s" {
    if (-not (OneTest 'simpa-core' 'params_synthetic' 'a_decay_off_only_where_one_parameter_looks_fails_that_parameter_only')) { return $false }
    $m = [regex]::Matches($script:cargoText, '(?m)^(early|late) kink, T (\S+) s: EDT (within|OUTSIDE), T20 (within|OUTSIDE), T30 (within|OUTSIDE)')
    $ok = $m.Count -eq 6
    foreach ($x in $m) {
        $g = $x.Groups
        $want = if ($g[1].Value -eq 'early') { 'OUTSIDE,within,within' } else { 'within,within,OUTSIDE' }
        $got = "$($g[3].Value),$($g[4].Value),$($g[5].Value)"
        Write-Host ("      {0} kink, T {1} s: EDT {2}, T20 {3}, T30 {4}: {5}" -f $g[1].Value, $g[2].Value, $g[3].Value, $g[4].Value, $g[5].Value, $(if ($got -eq $want) { 'as it must' } else { 'WRONG' }))
        if ($got -ne $want) { $ok = $false }
    }
    $ok
}

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
# rho c as SPPS computes it: rho = P M / (R T), M 28.9644 kg/kmol, R 8314.32 J/(K kmol)
# (Masse_volumique_air.cpp:45-52); c = 343.2 sqrt(T / 293.15) (Celerite_du_son.cpp:46).
function SolverRhoC([double]$tc, [double]$p) { $k = $tc + 273.15; $p * 28.9644 / (8314.32 * $k) * 343.2 * [math]::Sqrt($k / 293.15) }
# The mean of 1/d^2 over a ball of radius a centred r > a from the source:
# 3/(2 r a^3) [(a^2 - r^2)/2 ln((r + a)/(r - a)) + r a].
function MeanInvSq([double]$r, [double]$a) { 3.0 / (2.0 * $r * [math]::Pow($a, 3)) * (($a * $a - $r * $r) / 2.0 * [math]::Log(($r + $a) / ($r - $a)) + $r * $a) }
# The free field at a receiver sphere, as SPPS's receiver averages it: W rho c <1/d^2> / (4 pi p0^2).
function ExactLevel([double]$lw, [double]$r, [double]$a, [double]$rhoc) { 10 * [math]::Log10(1e-12 * [math]::Pow(10, $lw / 10) * $rhoc * (MeanInvSq $r $a) / (4 * [math]::PI * 4e-10)) }
# Every band within 4 mc_sd of the exact free field, and the band mean weighted by 1/mc_sd^2 within
# 4 of its standard deviation, with $offset dB added to every SPL.
function ExactCheck($rows, [double]$a, [double]$rhoc, [double]$offset) {
    $sum = 0.0; $w = 0.0; $bandsOk = $true
    foreach ($x in $rows) {
        $d = $x.Spl + $offset - (ExactLevel $x.Lw $x.R $a $rhoc)
        if ([math]::Abs($d) -gt 4 * $x.Sd) { $bandsOk = $false }
        $sum += $d / ($x.Sd * $x.Sd); $w += 1 / ($x.Sd * $x.Sd)
    }
    $mean = $sum / $w; $sd = 1 / [math]::Sqrt($w)
    [pscustomobject]@{ Mean = $mean; Sd = $sd; Ok = ($bandsOk -and [math]::Abs($mean) -le 4 * $sd) }
}
$script:levelRows = $null
function LevelRows {
    if ($null -ne $script:levelRows) { return , $script:levelRows }
    $run = RunOk (Join-Path $fx 'rooms\level_box_20m.simpa') 'spps' (Join-Path $work 'level') 'level-spps'
    $rep = Results $run 'level-results'
    $script:levelRep = $rep
    [xml]$cfg = ReadText (Join-Path $run 'solve\config.xml')
    $inv = [Globalization.CultureInfo]::InvariantCulture
    $atmo = $cfg.configuration.condition_atmospherique
    $script:levelRhoC = SolverRhoC ([double]::Parse($atmo.temperature, $inv)) ([double]::Parse($atmo.pression, $inv))
    $script:levelRadius = [double]$rep.spps.receiver_radius_m
    $lw = @{}; foreach ($b in $cfg.configuration.sources.source.bfreq) { $lw[[int]$b.freq] = [double][single][double]::Parse($b.db, $inv) }
    $src = $rep.spps.sources[0].position_m
    $rows = @()
    foreach ($r in $rep.spps.point_receivers) {
        $p = $r.position_m
        $dist = [math]::Sqrt([math]::Pow($p[0] - $src[0], 2) + [math]::Pow($p[1] - $src[1], 2) + [math]::Pow($p[2] - $src[2], 2))
        foreach ($b in $r.bands) {
            $spl = (Params $b).spl_db.value
            if ($null -eq $spl) { throw "$($r.label) $($b.freq_hz) Hz: SPL not evaluable: $((Params $b).spl_db.not_evaluable.message)" }
            $rows += [pscustomobject]@{ Label = $r.label; F = [int]$b.freq_hz; R = $dist; Spl = [double]$spl; Sd = [double](Params $b).spl_db.mc_sd; Total = [double]$b.total_pa2; Lw = $lw[[int]$b.freq_hz] }
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
Check "(c) what keeps the reverberant field out is the 20 ms duration: SPPS's statistics count 0 particles absorbed by the materials and every particle remaining, every band" {
    $null = LevelRows
    $bands = @($script:levelRep.spps.particles.bands)
    $bad = @($bands | Where-Object { $_.absorbed_by_materials -ne 0 -or $_.remaining -ne $_.total })
    Write-Host ("      {0} bands; absorbed by the materials {1}; remaining {2} of {3}" -f $bands.Count, (($bands | ForEach-Object { $_.absorbed_by_materials }) -join '/'), $bands[0].remaining, $bands[0].total)
    $bands.Count -eq 6 -and $bad.Count -eq 0
}
# The say-NOs put their fault into the code (M7 follow-ups; the M7 critic: they had added offsets
# to the SPL the correct run produced): cli_results' gate (c) test runs the level box, then
# computes its report again in-process with the level code path's reference constant replaced
# (simpa_core::faults::Fault::LevelReference, a test-only feature no normal build has).
Check "(c) says NO through the code: SPL's reference p0^2 replaced by Night Mode's 1e-12 (main:project/result_parser.cpp:486), and by p0^2 1 dB off either way, misses the bound in every band; p0^2 0.15 dB off either way, or as rho c = 400 would read, misses the exact free field" {
    $line = @(git show main:project/result_parser.cpp)[485]
    Write-Host "      result_parser.cpp:486: $($line.Trim())"
    if (-not (OneTest 'simpa' 'cli_results' 'gate_c_level_calibration_and_the_offsets_it_catches')) { return $false }
    $m = [regex]::Matches($script:cargoText, "(?m)^gate \(c\) says no through the code: (.+?): SPL (\S+) dB, outside the gate's bound in (\d+) of (\d+) bands")
    $m | ForEach-Object { Write-Host "      $($_.Groups[1].Value): SPL $($_.Groups[2].Value) dB, outside in $($_.Groups[3].Value) of $($_.Groups[4].Value) bands" }
    $x = [regex]::Matches($script:cargoText, '(?m)^exact says no through the code: (.+?): weighted mean (\S+) dB: (caught|PASSES)')
    $x | ForEach-Object { Write-Host "      exact: $($_.Groups[1].Value): mean $($_.Groups[2].Value) dB, $($_.Groups[3].Value)" }
    $line.Contains('/ 1e-12f') -and $m.Count -eq 3 -and @($m | Where-Object { $_.Groups[3].Value -ne $_.Groups[4].Value -or $_.Groups[4].Value -ne '12' }).Count -eq 0 -and $x.Count -eq 3 -and @($x | Where-Object { $_.Groups[3].Value -ne 'caught' }).Count -eq 0
}
Check "(c) says NO to a run whose walls are reached: the level box with its walls reflecting (alpha 0.5) and 120 ms, through SPPS, fails the no-reverberant-field check in every band" {
    if (-not (OneTest 'simpa' 'cli_results' 'gate_c_says_no_to_a_run_whose_walls_are_reached')) { return $false }
    $m = [regex]::Matches($script:cargoText, '(?m)^reflecting walls, (\d+) Hz: absorbed by the materials (\d+), remaining (\d+) of (\d+)')
    $m | ForEach-Object { Write-Host "      $($_.Groups[1].Value) Hz: absorbed by the materials $($_.Groups[2].Value), remaining $($_.Groups[3].Value) of $($_.Groups[4].Value)" }
    $m.Count -eq 6 -and @($m | Where-Object { [int]$_.Groups[2].Value -eq 0 -or $_.Groups[3].Value -eq $_.Groups[4].Value }).Count -eq 0
}
# The gate's reference assumes rho c = 400 and a point receiver, so it sits 0.11-0.30 dB below what
# SPPS should give, and a calibration error between about -0.6 and +0.2 dB passes it (M7 review).
# This check holds the same run to the exact free field, within the noise each value carries.
Check "(c) the exact free field, W rho c <1/d^2> / (4 pi p0^2) with SPPS's rho c and the mean over the receiver sphere: every band within 4 mc_sd, the band mean weighted by 1/mc_sd^2 within 4 of its standard deviation" {
    $rows = LevelRows
    foreach ($x in $rows) {
        $e = ExactLevel $x.Lw $x.R $script:levelRadius $script:levelRhoC
        Write-Host ("      {0} {1,5} Hz: SPL {2:N3} dB, exact {3:N3} dB ({4:+0.000;-0.000}, {5:+0.0;-0.0} mc_sd)" -f $x.Label, $x.F, $x.Spl, $e, ($x.Spl - $e), (($x.Spl - $e) / $x.Sd))
    }
    $c = ExactCheck $rows $script:levelRadius $script:levelRhoC 0
    Write-Host ("      rho c {0:N2}, R {1} m; weighted mean SPL - exact {2:+0.0000;-0.0000} dB, standard deviation {3:N4} dB: {4}" -f $script:levelRhoC, $script:levelRadius, $c.Mean, $c.Sd, $(if ($c.Ok) { 'within' } else { 'OUTSIDE' }))
    $rows.Count -eq 12 -and $c.Ok
}

# --- (d) TCR against the analytic values ----------------------------------------------------------
$tut = Join-Path $fx 'rooms\tutorial1_box_seeded.simpa'
$script:tcrRun = $null
function TcrRun {
    if ($null -eq $script:tcrRun) { $script:tcrRun = RunOk $tut 'tcr' (Join-Path $work 'tcr') 'tutorial-tcr' }
    $script:tcrRun
}
function Within([double]$a, [double]$b) { [math]::Abs($a / $b - 1) -le 0.005 }
# Per band, TCR's Sabine and Eyring times against core::params' on the run's own inputs.
function TcrAgainstAnalytic([string]$run, [string]$label) {
    $rep = Results $run $label
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
Check "(d) tutorial-1 box through TCR: per band, TCR's Sabine and Eyring times equal core::params' on the run's inputs within 0.5 %" {
    TcrAgainstAnalytic (TcrRun) 'tutorial-tcr-results'
}
Check "(d) the same from the project itself (cargo test gate_d_...): within 0.5 %, and the project's walls 5 % more absorbing outside it" {
    OneTest 'simpa' 'cli_results' 'gate_d_tcr_equals_the_analytic_sabine_and_eyring_and_says_no'
}
# Tutorial 1's floor (0.1) and ceiling (0.3) sit on equal areas and average to the walls' 0.2, so
# a mean over faces or over materials gives TCR's times too, and the checks above cannot see how
# the absorption is combined (M7 review). The asymmetric box can: the floor rising from 0.15 to
# 0.80 over the bands, the walls 0.1, the ceiling 0.3 (results_rooms.rs, asymmetric_box).
$asym = Join-Path $fx 'rooms\tutorial1_box_asymmetric.simpa'
Check "(d) asymmetric box through TCR: per band, TCR's Sabine and Eyring times equal core::params' on the run's inputs within 0.5 %" {
    TcrAgainstAnalytic (RunOk $asym 'tcr' (Join-Path $work 'tcr-asymmetric') 'asymmetric-tcr') 'asymmetric-tcr-results'
}
Check "(d) says NO on the asymmetric box: a mean over faces, a mean over materials, the floor's and walls' materials swapped, or each band's neighbour's absorption, is outside 0.5 % of TCR in all 27 bands, both theories" {
    if (-not (OneTest 'simpa' 'cli_results' 'gate_d_the_asymmetric_room_tells_apart_the_ways_of_combining_absorption')) { return $false }
    $m = [regex]::Matches($script:cargoText, '(?m)^asymmetric says no: (\w+) outside 0\.5 % in (\d+) of (\d+) bands, closest (\S+) %')
    $m | ForEach-Object { Write-Host "      $($_.Groups[1].Value): outside in $($_.Groups[2].Value) of $($_.Groups[3].Value) bands, closest $($_.Groups[4].Value) %" }
    $names = (@($m | ForEach-Object { $_.Groups[1].Value }) -join ',')
    $m.Count -eq 4 -and $names -eq 'FaceMean,MaterialMean,FloorWallsSwapped,NeighbourBand' -and @($m | Where-Object { $_.Groups[2].Value -ne '27' -or $_.Groups[3].Value -ne '27' }).Count -eq 0
}
Check "(d) no NaN or infinity in any value TCR wrote for display: every row of every table (receiver Global rows included; Main results' Global areas and times are NaN by design and must be), every .csbin value" {
    $solve = Join-Path (TcrRun) 'solve'
    $n = 0; $bad = @(); $design = 0
    foreach ($f in Get-ChildItem -LiteralPath $solve -Recurse -File) {
        if ($f.Extension -eq '.gabe') {
            foreach ($v in GabeValues $f.FullName) {
                if (ByDesign $f.FullName $v) { $design++; if (-not (NonFinite $v.Hex)) { $bad += "$($f.Name) $($v.Column) Global is not NaN" }; continue }
                $n++; if (NonFinite $v.Hex) { $bad += "$($f.Name) $($v.Column) $($v.Row)" }
            }
        } elseif ($f.Extension -eq '.csbin') {
            foreach ($h in CsbinValues $f.FullName) { $n++; if (NonFinite $h) { $bad += $f.FullName } }
        }
    }
    Write-Host "      $n values scanned, $($bad.Count) not finite; $design Global areas and times NaN by design"
    $bad | Select-Object -First 5 | ForEach-Object { Write-Host "      $_" }
    $n -gt 1000 -and $bad.Count -eq 0 -and $design -eq 4
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
# One surface, as the gate's say-NO was specified (the M7 critic: the check above changes the whole
# Walls group), and the air term, which had none, both through the code.
Check "(d) says NO through the code: the air term 4mV dropped, or m from ISO 9613-1 at the exact midband, misses TCR in some band; one plane's alpha raised past what the gate resolves is caught, and just below it is not" {
    if (-not (OneTest 'simpa' 'cli_results' 'gate_d_says_no_through_the_code_to_one_surface_and_to_the_air_term')) { return $false }
    $m = [regex]::Matches($script:cargoText, '(?m)^gate \(d\) says no through the code: (.+?): outside 0\.5 % in (\d+) of 27 bands')
    $m | ForEach-Object { Write-Host "      $($_.Groups[1].Value): outside 0.5 % in $($_.Groups[2].Value) of 27 bands" }
    $p = [regex]::Matches($script:cargoText, '(?m)^gate \(d\), one surface: (.+?) \((\d+) m2\): alpha \+5 % fails the gate in (\d+) of 27 bands; the smallest increase it catches is \+(\S+) % \(through TCR: \+\S+ % caught in (\d+) bands, \+\S+ % in (\d+)\)')
    $p | ForEach-Object { Write-Host "      $($_.Groups[1].Value) ($($_.Groups[2].Value) m2): +5 % fails in $($_.Groups[3].Value) bands; smallest increase caught +$($_.Groups[4].Value) % (just below: $($_.Groups[5].Value) bands, just above: $($_.Groups[6].Value))" }
    $m.Count -eq 2 -and @($m | Where-Object { [int]$_.Groups[2].Value -eq 0 }).Count -eq 0 -and $p.Count -eq 6 -and @($p | Where-Object { $_.Groups[5].Value -ne '0' -or $_.Groups[6].Value -eq '0' }).Count -eq 0
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
    $found = @(GabeValues $main | Where-Object { (NonFinite $_.Hex) -and -not (ByDesign $main $_) })
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
# Through the code (M7 follow-ups): the test puts the natural log into params::din18041's lg
# (simpa_core::faults::Fault::DinNaturalLog) and asks the code for groups A2 and A4.
Check "(f) says NO through the code: the natural log in place of lg, and groups A2's and A4's formulas, miss 0.552 +/- 0.001 s" {
    if (-not (OneTest 'simpa-core' 'params_room' 'gate_f_din_18041_a3_at_180_m3_is_0_552_s')) { return $false }
    if ($script:cargoText -notmatch 'says no through the code: ln for lg ([0-9.]+) s, A2 ([0-9.]+) s, A4 ([0-9.]+) s') { throw 'the test printed no say-NO values' }
    $v = @([double]$Matches[1], [double]$Matches[2], [double]$Matches[3])
    Write-Host ("      ln for lg: {0} s, A2: {1} s, A4: {2} s" -f $v[0], $v[1], $v[2])
    @($v | Where-Object { GateF $_ }).Count -eq 0
}

# --- the deliverable's schema ---------------------------------------------------------------------
Check "results --json validated against the committed schema by a JSON Schema validator: every committed run's report (SPPS and TCR) and a refusal of each exit; a wrong type or a missing field fails it" {
    if (-not (OneTest 'simpa' 'cli_results' 'every_report_and_refusal_validates_against_the_committed_schema')) { return $false }
    $m = [regex]::Matches($script:cargoText, '(?m)^schema says no: ([^:]+):')
    Write-Host "      says no to: $((@($m | ForEach-Object { $_.Groups[1].Value })) -join '; ')"
    $m.Count -eq 7
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
