# Parity gate: our build against original I-Simpa (docs/m5-m6-design.md, decision 3, "the parity
# bed"). Runs the bed and the tests it rests on, and says what each proves:
# (1) The solvers under test are the committed build: solvers/manifest.json's sha256 of spps.exe,
#     classicalTheory.exe, tetgen.exe and preprocess.exe, in $SolversDir.
# (2) The bed, crates/simpa/tests/parity_tutorials.rs: upstream's tutorials 1, 2 and 3, imported
#     and meshed by `simpa`, against the files original I-Simpa wrote into each .proj, and SPPS and
#     TCR run with one seed on our inputs and on the original inputs, every output compared.
#     tutorial_1, tutorial_2, tutorial_3 and the_comparisons_say_no must pass, and the two tests
#     the plain suite ignores must say so. tutorial_3_same_seed_runs, one of those, is run here and
#     is BLOCKED, never passed, only on its measured signature: every run differs with the room
#     written as idVolume 0 (decision 1) and matches with the room's parts kept as TetGen numbered
#     them. Reversing decision 1 is Burhan's and Michael's call.
# (3) The tests the bed rests on: parity_inputs.rs (config.xml and mesh.cbin, value by value) and
#     mesh_mbin_parity.rs (the .mbin builder, byte for byte).
# (4) Upstream's shipped 1.3.4 and 1.4.0 SPPS and TCR against ours on tutorial 1's original inputs
#     (the bed's ignored test, given $ReleaseBinaries): ours must be bit-identical to 1.4.0; 1.3.4's
#     differences are measured and printed.
# Every check has a "says NO":
# - (1) refuses upstream's own TetGen 1.6.0 build (the 929a5c8 reference, built by
#   solvers/build.ps1 beside ours) in place of ours;
# - (2) the bed run with that TetGen 1.6.0 in the solver folder must FAIL tutorial_1, naming
#   TetGen's files; the_comparisons_say_no feeds each comparison the input that fails it;
# - (4) asserts that 1.3.4 differs, so the comparison can tell two builds apart.
# A check an open decision blocks prints BLOCKED; the gate then exits 3, never 0.
# Run: powershell -File tools/gates/parity.ps1 [-SolversDir <bin>] [-ReleaseBinaries <folder>]
param(
    # The solver build under test: spps.exe, classicalTheory.exe, tetgen.exe, preprocess.exe.
    [string]$SolversDir = $(if ($env:SIMPA_SOLVERS_DIR) { $env:SIMPA_SOLVERS_DIR } else { '' }),
    # Upstream's installed releases: inst134\core\... and inst140\ (docs/investigations/...).
    [string]$ReleaseBinaries = 'B:\repos\I-Simpa_Night_Mode\target\investigate\release-binary',
    # Upstream's own TetGen 1.6.0 build, for the refusals. Default: beside $SolversDir's build.
    [string]$Tetgen160 = ''
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Set-Location $repo
$env:RUSTUP_HOME = "$env:USERPROFILE\.rustup"; $env:CARGO_HOME = "$env:USERPROFILE\.cargo"
$env:Path = "$env:CARGO_HOME\bin;$env:Path"; $env:CARGO_INCREMENTAL = '0'
# This tree's own build folder: a folder another checkout builds into can hand cargo that
# checkout's compiled code (cargo judges freshness by file times).
$env:CARGO_TARGET_DIR = Join-Path $repo 'target'
$env:SIMPA_REQUIRE_UPSTREAM = '1'
if (-not $env:SIMPA_UPSTREAM) { $env:SIMPA_UPSTREAM = 'B:\repos\I-Simpa-upstream' }
if (-not $SolversDir) { $SolversDir = Join-Path $repo 'target\solvers\bin' }
if (-not $Tetgen160) { $Tetgen160 = Join-Path (Split-Path -Parent $SolversDir) 'build\src\tetgen\Release\tetgen.exe' }
$env:SIMPA_SOLVERS_DIR = $SolversDir

$failures = @(); $blocked = @(); $script:checks = 0
function Blocked([string]$reason) { @{ blocked = $reason } }
# A check body returns exactly one bool, or `Blocked <reason>`. Anything else is a FAIL.
function Check($name, [scriptblock]$body) {
    $script:checks++
    try {
        $r = @(& $body)
        if ($r.Count -eq 1 -and $r[0] -is [hashtable] -and $r[0].ContainsKey('blocked')) {
            Write-Host "BLOCKED  $name :: $($r[0].blocked)"; $script:blocked += $name
        } elseif ($r.Count -eq 1 -and $r[0] -is [bool] -and $r[0]) {
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
$work = Join-Path $repo ('target\gates\parity\' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
New-Item -ItemType Directory -Force $work | Out-Null
Write-Host "work: $work"
Write-Host "solvers: $SolversDir"
# A cargo test run with extra environment variables, its whole output kept in $work\<label>.txt:
# per test, 'ok', 'FAILED' or 'ignored', and the output.
function CargoTest([string]$cmdline, [string]$label, [hashtable]$vars = @{}) {
    $saved = @{}
    foreach ($k in $vars.Keys) { $saved[$k] = [Environment]::GetEnvironmentVariable($k); [Environment]::SetEnvironmentVariable($k, $vars[$k]) }
    try { $out = cmd /c "$cmdline 2>&1"; $code = $LASTEXITCODE } finally { foreach ($k in $saved.Keys) { [Environment]::SetEnvironmentVariable($k, $saved[$k]) } }
    $text = (@($out) | ForEach-Object { "$_" }) -join "`n"
    [IO.File]::WriteAllText((Join-Path $work "$label.txt"), $text)
    # With --nocapture a test's own lines come between its 'test <name> ... ' and its verdict, so
    # the verdict is read from what libtest prints last: an ignored test says so on its own line,
    # a failed one is listed under the final 'failures:', and one that ran otherwise passed, once
    # a 'test result:' line shows the binary finished.
    $results = @{}
    $finished = [regex]::IsMatch($text, '(?m)^test result: ')
    $failed = @()
    $lists = [regex]::Matches($text, '(?ms)^failures:\s*\n((?:    \S+\s*\n?)+)')
    if ($lists.Count) { $failed = @($lists[$lists.Count - 1].Groups[1].Value -split "`n" | ForEach-Object { $_.Trim() } | Where-Object { $_ }) }
    foreach ($m in [regex]::Matches($text, '(?m)^test (\S+) \.\.\. (.*)$')) {
        $name = $m.Groups[1].Value
        # libtest prints an ignored test's reason after it: 'ignored, <reason>'.
        if ($m.Groups[2].Value.Trim() -match '^ignored(,|$)') { $results[$name] = 'ignored' }
        elseif ($failed -contains $name) { $results[$name] = 'FAILED' }
        elseif ($finished) { $results[$name] = 'ok' }
        else { $results[$name] = 'unfinished' }
    }
    [pscustomobject]@{ Code = $code; Text = $text; Results = $results }
}
function Sha([string]$path) { (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLower() }

$build = cmd /c "cargo build -q -p simpa --tests 2>&1"
if ($LASTEXITCODE -ne 0) { $build | Select-Object -Last 20 | ForEach-Object { Write-Host $_ }; throw 'build failed: refusing to test stale code' }

# --- (1) the solvers under test -------------------------------------------------------------------
$manifest = Get-Content (Join-Path $repo 'solvers\manifest.json') -Raw | ConvertFrom-Json
$exes = @('spps.exe', 'classicalTheory.exe', 'tetgen.exe', 'preprocess.exe')
Check "(1) the solvers under test are solvers/manifest.json's build (sha256 of all 4)" {
    $ok = $true
    foreach ($e in $exes) {
        $p = Join-Path $SolversDir $e
        if (-not (Test-Path $p)) { throw "$p is missing" }
        $h = Sha $p; $want = $manifest.sha256.$e
        Write-Host ("      {0,-20} {1} {2}" -f $e, $h.Substring(0, 16), $(if ($h -eq $want) { 'manifest' } else { "NOT the manifest's $($want.Substring(0, 16))" }))
        if ($h -ne $want) { $ok = $false }
    }
    Write-Host "      TetGen: $($manifest.tetgen.version), $($manifest.tetgen.source)"
    $ok -and $manifest.tetgen.version -eq '1.5.0'
}
Check "(1) says NO: upstream's TetGen 1.6.0 build is not the manifest's tetgen.exe" {
    if (-not (Test-Path $Tetgen160)) { throw "$Tetgen160 is missing: build it with solvers/build.ps1, or pass -Tetgen160" }
    $h = Sha $Tetgen160
    Write-Host "      $Tetgen160 $($h.Substring(0, 16)); manifest $($manifest.sha256.'tetgen.exe'.Substring(0, 16)); the manifest's 1.6.0 reference $($manifest.tetgen.upstream_160_reference_sha256.Substring(0, 16))"
    $h -ne $manifest.sha256.'tetgen.exe'
}

# --- (2) the bed -------------------------------------------------------------------------------------
$script:bed = CargoTest 'cargo test -p simpa --test parity_tutorials --no-fail-fast -- --nocapture --test-threads=1' 'bed'
foreach ($t in @('tutorial_1', 'tutorial_2', 'tutorial_3', 'the_comparisons_say_no')) {
    Check "(2) bed: $t" {
        $r = $script:bed.Results[$t]
        Write-Host "      $t ... $r"
        $r -eq 'ok'
    }
}
Check "(2) bed: the two tests the plain suite ignores say so, with their reasons, never skipped silently" {
    $a = $script:bed.Results['tutorial_3_same_seed_runs']; $b = $script:bed.Results['shipped_1_3_4_and_1_4_0_solvers_against_ours']
    Write-Host "      tutorial_3_same_seed_runs ... $a; shipped_1_3_4_and_1_4_0_solvers_against_ours ... $b"
    $a -eq 'ignored' -and $b -eq 'ignored'
}
Check "(2) bed: tutorial_3_same_seed_runs, run here (decision 1: the room written as idVolume 0)" {
    $t3 = CargoTest 'cargo test -p simpa --test parity_tutorials tutorial_3_same_seed_runs -- --ignored --exact --nocapture' 'bed-t3-runs'
    $r = $t3.Results['tutorial_3_same_seed_runs']
    Write-Host "      tutorial_3_same_seed_runs ... $r"
    if ($r -eq 'ok') { return $true }
    # BLOCKED only on the measured signature: each of the 3 runs differs with the room as 0 and
    # matches with the room's parts kept as TetGen numbered them.
    $sig = [regex]::Matches($t3.Text, '(?m)^run \d: (\d+) of (\d+) output files differ with idVolume \{[^}]*2084: 0[^}]*\}.*?; with the room''s parts kept as TetGen numbered them \(\{[^}]*2084: 2084[^}]*\}\), 0 differ$')
    Write-Host "      runs with the decision-1 signature: $($sig.Count) of 3"
    if ($r -eq 'FAILED' -and $sig.Count -eq 3) {
        return (Blocked 'decision 1 (docs/m5-m6-design.md): the .mbin builder writes the room as idVolume 0, and SPPS then leaves a fitting on scene faces where upstream''s room overwrites it (coreinitialisation.cpp:151-176); with the room kept as TetGen numbers it, all 3 tutorial-3 runs match the original. Reversing decision 1 is Burhan''s and Michael''s call')
    }
    $false
}
Check "(2) says NO: the bed with upstream's TetGen 1.6.0 in the solver folder fails tutorial_1, naming TetGen's files" {
    if (-not (Test-Path $Tetgen160)) { throw "$Tetgen160 is missing" }
    $bin16 = Join-Path $work 'solvers-tetgen160'
    New-Item -ItemType Directory -Force $bin16 | Out-Null
    foreach ($e in @('spps.exe', 'classicalTheory.exe', 'preprocess.exe')) { Copy-Item (Join-Path $SolversDir $e) (Join-Path $bin16 $e) }
    Copy-Item $Tetgen160 (Join-Path $bin16 'tetgen.exe')
    $r = CargoTest 'cargo test -p simpa --test parity_tutorials tutorial_1 -- --exact --nocapture' 'bed-tetgen160' @{ SIMPA_SOLVERS_DIR = $bin16 }
    $named = $r.Text -match 'tutorial 1: scene_mesh\.1\.'
    Write-Host "      tutorial_1 ... $($r.Results['tutorial_1']); names a TetGen file: $named"
    $r.Results['tutorial_1'] -eq 'FAILED' -and $named
}

# --- (3) what the bed rests on ----------------------------------------------------------------------
$script:core = CargoTest 'cargo test -p simpa-core --test parity_inputs --test mesh_mbin_parity --no-fail-fast' 'core'
Check "(3) parity_inputs.rs and mesh_mbin_parity.rs pass, every test run" {
    $n = $script:core.Results.Count
    $bad = @($script:core.Results.GetEnumerator() | Where-Object { $_.Value -ne 'ok' } | ForEach-Object { "$($_.Key) $($_.Value)" })
    Write-Host "      $n tests; not ok: $($bad -join ', ')"
    $n -ge 20 -and $bad.Count -eq 0 -and $script:core.Code -eq 0
}

# --- (4) the shipped solvers --------------------------------------------------------------------------
Check "(4) upstream's shipped 1.4.0 SPPS and TCR are bit-identical to ours on tutorial 1; 1.3.4's differ (measured)" {
    if (-not (Test-Path (Join-Path $ReleaseBinaries 'inst140\spps.exe'))) { throw "$ReleaseBinaries holds no inst140\spps.exe: pass -ReleaseBinaries" }
    $r = CargoTest 'cargo test -p simpa --test parity_tutorials shipped_1_3_4_and_1_4_0_solvers_against_ours -- --ignored --exact --nocapture' 'shipped' @{ SIMPA_RELEASE_BINARIES = $ReleaseBinaries }
    $r.Text -split "`n" | Where-Object { $_ -match '^(spps|tcr): ' } | Select-Object -Last 6 | ForEach-Object { Write-Host "      $_" }
    $r.Results['shipped_1_3_4_and_1_4_0_solvers_against_ours'] -eq 'ok'
}

Write-Host "`nwork: $work"
$passed = $script:checks - $failures.Count - $blocked.Count
if ($failures.Count) { Write-Host "PARITY FAILED: $($failures.Count) of $script:checks checks failed, $($blocked.Count) BLOCKED, $passed passed"; exit 1 }
if ($blocked.Count) { Write-Host "PARITY PASSED EXCEPT $($blocked.Count) BLOCKED: $passed of $script:checks checks passed; blocked: $($blocked -join '; ')"; exit 3 }
Write-Host "PARITY PASSED: $script:checks of $script:checks checks"; exit 0
