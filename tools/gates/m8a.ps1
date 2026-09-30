# M8a gate: the T30 physics bed, as pre-registered in docs/investigations/2026-09-29-m8a/SPEC.md
# (sections 5 and 7), decided in docs/decision-log.md rows 1, 10, 11 and 17.
#   A  SPPS's T30 against Kuttruff's corrected Eyring (gamma^2 from the room's geometry): the 95 %
#      interval inside +-5 % in every band of every gated cell, random and energetic, air off and on
#   B  seed spread of the cell mean <= 2 % over the ten seeds
#   C  T30 against the independent transport: |d| + t*SE <= 0.5 % (one extension to 20 seeds)
#   D  TCR's Eyring time against its analytic value within 0.5 %
#   E1 the four executables are solvers/manifest.json's build (code sha256), before any run, and
#      every run's run.json names the file checked; E2 every run OK; E3 the reference; E4 the
#      transport reproduces HIGH to 1e-9 s; E5 cargo test --release -p simpa-core --test
#      params_kuttruff; E6 every seed's T30 a value or a noise refusal; E7 the bed file is the matrix
# Plain Eyring, Sabine, the 20x8x4 m room and upstream's atmospheric validation are reported, never
# gated. The say-NO partners must each fail or refuse:
#   N1 a copy of spps.exe with one .text byte flipped: the bed refuses before any run
#   N2 Kuttruff with its 1/2 dropped, N3 plain Eyring as A's reference: A fails in every alpha 0.4
#      gated cell; N4 the transport reflecting evenly over the hemisphere: C fails in every gated
#      cell; N7 the transport with the air off: C fails in every air-on gated cell; N8 D with the
#      physical K: D fails in every band (crates/simpa/tests/bed_m8a.rs, faults through the code,
#      re-reading the bed's own runs)
#   N5 seed 10 of 5x4x3 alpha 0.4 random at 31,000 particles in its place: the cell does not pass
#   N6 walls of scattering 0: the bed refuses the cell (params_reference_not_applicable, E3)
# A FAIL is a finding: no limit, reference or cell changes because of a result.
#
# Run: powershell -File tools/gates/m8a.ps1 -BedRoot <dir off B:>      (runs the bed, about 14 h)
#      powershell -File tools/gates/m8a.ps1 -From <root>\<stamp>       (re-reads an earlier bed)
# Needs $env:SIMPA_SOLVERS_DIR (the verified solver build). Honours $env:CARGO_TARGET_DIR (B:'s old
# target folder fails with os error 1392: build elsewhere), else <repo>\target. -Bed <file> runs
# another bed file, for checking the gate itself: it is exploratory and never prints 'M8a PASSED'.
# Upstream's tree for the atmospheric validation: -Upstream, else $env:SIMPA_UPSTREAM, else
# B:\repos\I-Simpa-upstream when it is there (read-only).
# With -From, the bed's runs are held to their run.json's output hashes, or, for a bed made before
# M8b (whose run.json has none), to its committed seal: -Seal <file>, else
# beds\m8a-<the -From folder's name>\outputs-seal.json when it is there. Without either, every run
# of such a bed is refused (bed_run_unbound) and the gate fails.
param(
    [string]$BedRoot = '',
    [string]$From = '',
    [string]$Bed = '',
    [int]$Jobs = 4,
    [string]$Upstream = '',
    [string]$Seal = '',
    [double]$BedTimeoutHours = 40
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Set-Location $repo
$env:RUSTUP_HOME = "$env:USERPROFILE\.rustup"; $env:CARGO_HOME = "$env:USERPROFILE\.cargo"
$env:Path = "$env:CARGO_HOME\bin;$env:Path"; $env:CARGO_INCREMENTAL = '0'
if (-not $env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR = Join-Path $repo 'target' }
$target = $env:CARGO_TARGET_DIR
if (-not $env:SIMPA_SOLVERS_DIR) { throw 'SIMPA_SOLVERS_DIR is not set: point it at the verified solver build' }
if ([bool]$BedRoot -eq [bool]$From) { throw 'give exactly one of -BedRoot <dir> (run the bed) and -From <root>\<stamp> (read an earlier one)' }
if (-not $Bed) { $Bed = Join-Path $repo 'beds\m8a.json' }
if (-not $Upstream) {
    if ($env:SIMPA_UPSTREAM) { $Upstream = $env:SIMPA_UPSTREAM }
    elseif (Test-Path 'B:\repos\I-Simpa-upstream') { $Upstream = 'B:\repos\I-Simpa-upstream' }
}
# The suite's upstream-reading tests take the same tree.
if ($Upstream) { $env:SIMPA_UPSTREAM = $Upstream }
if ($From -and -not $Seal) {
    $candidate = Join-Path $repo ('beds\m8a-' + (Split-Path -Leaf $From) + '\outputs-seal.json')
    if (Test-Path -LiteralPath $candidate) { $Seal = $candidate }
}
if ($Seal -and -not $From) { throw '-Seal holds an earlier bed''s runs: give it with -From' }
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
function Note([string]$text) { Write-Host "      $text" }
# Runs a cargo command; on failure prints its last lines so a FAIL is never silent. Its whole
# output stays in $script:cargoText.
function Cargo([string]$cmdline) {
    $out = cmd /c "$cmdline 2>&1"
    $code = $LASTEXITCODE
    $script:cargoText = (@($out) | ForEach-Object { "$_" }) -join "`n"
    if ($code -ne 0) { $out | Select-Object -Last 15 | ForEach-Object { Write-Host "      | $_" }; return $false }
    return $true
}
# One named test, which must run exactly once and pass, with extra environment variables.
function OneTest([string]$crate, [string]$testTarget, [string]$test, [hashtable]$vars = @{}, [switch]$Release, [switch]$Ignored) {
    $saved = @{}
    foreach ($k in $vars.Keys) { $saved[$k] = [Environment]::GetEnvironmentVariable($k); [Environment]::SetEnvironmentVariable($k, $vars[$k]) }
    try {
        $rel = if ($Release) { '--release ' } else { '' }
        $ign = if ($Ignored) { '--ignored ' } else { '' }
        $ok = Cargo "cargo test $rel-p $crate --test $testTarget $test -- $ign--exact --nocapture"
    } finally { foreach ($k in $saved.Keys) { [Environment]::SetEnvironmentVariable($k, $saved[$k]) } }
    $ran = $script:cargoText -match 'test result: ok\. 1 passed'
    if (-not $ran) { Note "$test did not run exactly once and pass" }
    return ($ok -and $ran)
}
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
# A program with a hard time limit; stdout and stderr go to files under the work folder.
$script:callNo = 0
function Call([string]$exe, [string[]]$argv, [string]$label, [double]$limitSec) {
    $script:callNo++
    $base = Join-Path $work ('{0:D2}-{1}' -f $script:callNo, $label)
    $clock = [Diagnostics.Stopwatch]::StartNew()
    $p = Start-Process -FilePath $exe -ArgumentList (($argv | ForEach-Object { QuoteArg $_ }) -join ' ') `
        -RedirectStandardOutput "$base.stdout.txt" -RedirectStandardError "$base.stderr.txt" -NoNewWindow -PassThru
    $null = $p.Handle
    if (-not $p.WaitForExit([int][Math]::Min([double][int]::MaxValue, $limitSec * 1000))) {
        Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
        $null = $p.WaitForExit(10000)
        throw ('TIMEOUT: {0} still ran after {1:N0} s (limit {2:N0} s) and was killed; logs {3}.*' -f $label, $clock.Elapsed.TotalSeconds, $limitSec, $base)
    }
    $p.WaitForExit()
    [pscustomobject]@{ Exit = $p.ExitCode; Out = (ReadText "$base.stdout.txt"); Err = (ReadText "$base.stderr.txt"); Sec = $clock.Elapsed.TotalSeconds; Base = $base }
}

$work = Join-Path $target ('gates\m8a\' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
New-Item -ItemType Directory -Force $work | Out-Null
Write-Host "work: $work"
Write-Host "solvers: $env:SIMPA_SOLVERS_DIR"
Write-Host "bed file: $Bed"
Write-Host "upstream: $(if ($Upstream) { $Upstream } else { '(none: the atmospheric validation is not run)' })"
if ($From) { Write-Host "seal: $(if ($Seal) { $Seal } else { '(none: runs without output hashes in run.json are refused)' })" }

# --- E1 before anything runs ----------------------------------------------------------------------
. (Join-Path $repo 'solvers\pe-fingerprint.ps1')
$manifest = Get-Content -Raw (Join-Path $repo 'solvers\manifest.json') | ConvertFrom-Json
$exeNames = @('spps.exe', 'classicalTheory.exe', 'tetgen.exe', 'preprocess.exe')
$raw = @{}
$e1ok = $true
# The loop variable is not $name: Check's own parameter of that name would hide it in the body.
foreach ($exeName in $exeNames) {
    $exePath = Join-Path $env:SIMPA_SOLVERS_DIR $exeName
    $title = "E1 $exeName in SIMPA_SOLVERS_DIR has the code sha256 solvers/manifest.json lists"
    Check $title {
        $code = Get-CodeSha256 $exePath
        $script:raw[$exeName] = Get-RawSha256 $exePath
        Note "code $code, raw $($script:raw[$exeName])"
        $code -eq $manifest.code_sha256.$exeName
    }
    if ($failures -contains $title) { $e1ok = $false }
}

# --- E5 -------------------------------------------------------------------------------------------
Check "E5 cargo test --release -p simpa-core --test params_kuttruff: Kuttruff within the bare 0.6 % of the committed transport T30s in M8's 8 cells" {
    Cargo 'cargo test -q --release -p simpa-core --test params_kuttruff'
}

$build = cmd /c "cargo build -q --release -p simpa 2>&1"
if ($LASTEXITCODE -ne 0) { $build | Select-Object -Last 20 | ForEach-Object { Write-Host $_ }; throw 'CLI build failed: refusing to run the bed with a stale simpa.exe' }
$simpa = Join-Path $target 'release\simpa.exe'

# --- the bed --------------------------------------------------------------------------------------
$stamp = $null; $runsDir = $null; $report = $null
if ($e1ok) {
    Check "simpa bed ran and exited 0 (0 only when report.pass is true; 8 not passed, 5 a run not OK)" {
        $argv = @('bed', $Bed, '--jobs', "$Jobs", '--json')
        if ($From) { $argv += @('--out', (Join-Path $work 'reread'), '--from', $From) } else { $argv += @('--out', $BedRoot) }
        if ($Seal) { $argv += @('--seal', $Seal) }
        if ($Upstream) { $argv += @('--upstream', $Upstream) }
        $o = Call $simpa $argv 'bed' ($BedTimeoutHours * 3600)
        $m = [regex]::Matches($o.Err, '(?m)(\S+report\.json)\s*$')
        if ($m.Count) { $script:stamp = Split-Path -Parent $m[$m.Count - 1].Groups[1].Value }
        Note ('exit {0} after {1:N0} s; {2}' -f $o.Exit, $o.Sec, $script:stamp)
        $o.Err -split "`n" | Where-Object { $_ -match '^FAIL|^simpa:' } | Select-Object -First 40 | ForEach-Object { Note $_.TrimEnd() }
        $o.Exit -eq 0
    }
} else {
    Check "simpa bed not started: E1 failed, and the bed would refuse" { $false }
}
if ($stamp -and (Test-Path (Join-Path $stamp 'report.json'))) {
    $runsDir = if ($From) { $From } else { $stamp }
    $report = Get-Content -Raw (Join-Path $stamp 'report.json') | ConvertFrom-Json
}

# --- report.json ----------------------------------------------------------------------------------
Check "report.json validates against its schema (simpa bed --schema, by a JSON Schema validator)" {
    if (-not $report) { Note 'no report.json'; return $false }
    OneTest 'simpa' 'bed_m8a' 'report_validates_against_its_schema' @{ SIMPA_BED_FROM = $runsDir; SIMPA_BED_REPORT = (Join-Path $stamp 'report.json'); SIMPA_BED_FILE = $Bed } -Release -Ignored
}
Check "report.exploratory is false: the bed file is M8a's matrix (E7)" {
    if (-not $report) { return $false }
    $report.exploratory -is [bool] -and -not $report.exploratory
}
Check "report.pass is the JSON true" {
    if (-not $report) { return $false }
    Note ("{0} failures" -f @($report.failures).Count)
    @($report.failures) | Select-Object -First 60 | ForEach-Object { Note $_ }
    $report.pass -is [bool] -and $report.pass
}
Check "E1 again, from the files: every run's run.json executable, and every mesh.json's TetGen, is the raw sha256 of the file checked" {
    if (-not $runsDir) { return $false }
    $runJsons = @(Get-ChildItem -LiteralPath (Join-Path $runsDir 'runs') -Recurse -Filter 'run.json' -File)
    $bad = @()
    foreach ($f in $runJsons) {
        $m = Get-Content -Raw $f.FullName | ConvertFrom-Json
        $want = if ($m.solver -eq 'tcr') { $raw['classicalTheory.exe'] } else { $raw['spps.exe'] }
        if ($m.exe.sha256 -ne $want) { $bad += "$($f.FullName): $($m.exe.sha256)" }
        $mesh = Join-Path $f.DirectoryName 'mesh\mesh.json'
        if (Test-Path $mesh) {
            $t = (Get-Content -Raw $mesh | ConvertFrom-Json).tetgen.program_sha256
            if ($t -and $t -ne $raw['tetgen.exe']) { $bad += "$mesh`: tetgen $t" }
        }
    }
    Note "$($runJsons.Count) run.json read; $($bad.Count) not the checked files"
    $bad | Select-Object -First 10 | ForEach-Object { Note $_ }
    $runJsons.Count -gt 0 -and $bad.Count -eq 0
}
Check "the bed's files: fewer than the 20,000 a bed may leave (B: is exFAT with 128 KB clusters)" {
    if (-not $stamp) { return $false }
    $all = @(Get-ChildItem -LiteralPath $runsDir -Recurse -File)
    $bytes = ($all | Measure-Object -Property Length -Sum).Sum
    Note ("{0} files, {1:N1} MB under {2}; the report counted {3} files, {4:N1} MB" -f $all.Count, ($bytes / 1MB), $runsDir, $report.files.files, ($report.files.bytes / 1MB))
    $all.Count -lt 20000
}
Check "the decays and plots: tools/bed/m8a_plots.py writes decays/<cell>.npz, plots/<cell>.png and plots/summary.png" {
    if (-not $report) { return $false }
    $o = Call 'py' @('-3', (Join-Path $repo 'tools\bed\m8a_plots.py'), $stamp) 'plots' 1800
    if ($o.Exit -ne 0) { Note $o.Err.Trim(); return $false }
    $cells = @($report.cells | ForEach-Object { $_.id })
    $missing = @($cells | Where-Object { -not (Test-Path (Join-Path $stamp "plots\$_.png")) -or -not (Test-Path (Join-Path $stamp "decays\$_.npz")) })
    Note "$($cells.Count) cells; missing: $($missing -join ', ')"
    $missing.Count -eq 0 -and (Test-Path (Join-Path $stamp 'plots\summary.png'))
}

# --- say NO ---------------------------------------------------------------------------------------
Check "N1 says NO: a copy of spps.exe with one .text byte flipped: the bed refuses before any run (E1), exit 2, nothing written" {
    $dir = Join-Path $work 'n1-solvers'
    New-Item -ItemType Directory -Force $dir | Out-Null
    foreach ($exeName in $exeNames) {
        $src = Join-Path $env:SIMPA_SOLVERS_DIR $exeName
        if ($exeName -eq 'spps.exe') { $null = Copy-PeFlippedText $src (Join-Path $dir $exeName) } else { Copy-Item -LiteralPath $src -Destination (Join-Path $dir $exeName) }
    }
    $out = Join-Path $work 'n1-out'
    $saved = $env:SIMPA_SOLVERS_DIR
    try {
        $env:SIMPA_SOLVERS_DIR = $dir
        $argv = @('bed', $Bed, '--out', $out, '--jobs', "$Jobs")
        if ($runsDir) { $argv += @('--from', $runsDir) }
        if ($runsDir -and $Seal) { $argv += @('--seal', $Seal) }
        $o = Call $simpa $argv 'n1' 300
    } finally { $env:SIMPA_SOLVERS_DIR = $saved }
    $written = @(Get-ChildItem -LiteralPath $out -Recurse -ErrorAction SilentlyContinue).Count
    Note ("exit {0}; {1}; {2} things written" -f $o.Exit, (($o.Err -split "`n" | Where-Object { $_ -match 'refused' }) -join ' '), $written)
    $o.Exit -eq 2 -and $o.Err -match 'refused before any run \(E1\)' -and $written -eq 0
}
$sayNoVars = @{ SIMPA_BED_FROM = $runsDir; SIMPA_BED_REPORT = $(if ($stamp) { Join-Path $stamp 'report.json' } else { '' }); SIMPA_BED_FILE = $Bed; SIMPA_BED_JOBS = "$Jobs"; SIMPA_BED_SEAL = $Seal }
foreach ($t in @(
        @('N2 says NO: Kuttruff with its 1/2 dropped (through results::reference): A fails in every alpha 0.4 gated cell', 'n2_kuttruff_without_its_half_fails_a'),
        @('N3 says NO: plain Eyring as A''s reference: A fails in every alpha 0.4 gated cell', 'n3_plain_eyring_as_the_reference_fails_a'),
        @('N4 says NO: the transport reflecting evenly over the hemisphere, traced again: C fails in every gated cell', 'n4_a_transport_that_does_not_reflect_by_lamberts_law_fails_c'),
        @('N7 says NO: the transport traced with the air off: C fails in every air-on gated cell', 'n7_the_transport_without_air_fails_c_in_every_air_on_cell'),
        @('N8 says NO: TCR''s analytic time with the physical K (through results::tcr): D fails in every band of every gated TCR run', 'n8_tcrs_analytic_time_with_the_physical_constant_fails_d'))) {
    Check $t[0] {
        if (-not $runsDir) { return $false }
        $ok = OneTest 'simpa' 'bed_m8a' $t[1] $sayNoVars -Release -Ignored
        [regex]::Matches($script:cargoText, '(?m)^.*(Fail|Pass|NotJudged|Inconclusive|None).*$') | Select-Object -First 12 | ForEach-Object { Note $_.Value.TrimEnd() }
        $ok
    }
}
Check "N5 says NO: seed 10 of 5x4x3 alpha 0.4 random at 31,000 particles in its place: the cell does not pass" {
    if (-not $report) { return $false }
    $n5 = $report.say_no.n5
    Note ("run {0}; cell {1}, failed by {2}" -f $(if ($n5.run) { $n5.run.status } else { $n5.error }), $n5.cell_verdict, (@($n5.failed_by) -join ', '))
    $n5.as_required -is [bool] -and $n5.as_required
}
Check "N6 says NO: walls of scattering 0: the bed refuses the cell (params_reference_not_applicable in every band, E3)" {
    if (-not $report) { return $false }
    $n6 = $report.say_no.n6
    Note ("run {0}; kuttruff_s refused: {1}" -f $(if ($n6.run) { $n6.run.status } else { $n6.error }), ((@($n6.codes) | Select-Object -Unique) -join ', '))
    $n6.as_required -is [bool] -and $n6.as_required
}

# --- the code -------------------------------------------------------------------------------------
# The bed's own tests: the comparisons, the pass logic (a cell outside tolerance fails the
# report), the bed file, the plan, E1's code sha256. The whole suite is not this gate's: on Grace
# mesh_project's every_failure_code_fires_on_its_input hangs at the commit M8a started from too.
Check "the bed's unit tests: cargo test -p simpa-core --lib bed::" { Cargo 'cargo test -q -p simpa-core --lib bed::' }
Check "clippy -D warnings (core and CLI)" { Cargo 'cargo clippy -q -p simpa-core -p simpa --all-targets -- -D warnings' }
Check "cargo fmt --check (core and CLI)" { Cargo 'cargo fmt -p simpa-core -p simpa --check' }

Write-Host "`nwork: $work"
if ($stamp) { Write-Host "bed: $stamp" }
$passed = $script:checks - $failures.Count
$reportPass = $report -and ($report.pass -is [bool]) -and $report.pass
if ($failures.Count -or -not $reportPass) { Write-Host "M8a FAILED: $($failures.Count) of $script:checks checks failed, $passed passed: $($failures -join '; ')"; exit 1 }
Write-Host "M8a PASSED: $script:checks of $script:checks checks"; exit 0
