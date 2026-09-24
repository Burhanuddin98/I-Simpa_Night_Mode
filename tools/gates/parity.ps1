# Parity gate: our build against original I-Simpa (docs/m5-m6-design.md, decision 3, "the parity
# bed"). Runs the bed and the tests it rests on, and says what each proves:
# (1) The solvers under test are the committed build: solvers/manifest.json's sha256 of spps.exe,
#     classicalTheory.exe, tetgen.exe and preprocess.exe, in $SolversDir.
# (2) The bed, crates/simpa/tests/parity_tutorials.rs: upstream's tutorials 1, 2 and 3, imported
#     and meshed by `simpa`, against the files original I-Simpa wrote into each .proj, and SPPS and
#     TCR run with one seed on our inputs and on the original inputs, every output compared.
#     tutorial_1, tutorial_2, tutorial_3, tutorial_3_same_seed_runs and the_comparisons_say_no must
#     pass, and the one test the plain suite ignores (4) must say so. tutorial_3 is end to end
#     (decision 12): import-proj, then `simpa mesh --parity` through preprocess.exe, our geometry
#     check, TetGen 1.5.0 and our builder, whose .poly, .1.* and .mbin must be upstream's through
#     the recorded id map, and the default mode must mesh it clean, each region its cell's volume.
#     tutorial_3_same_seed_runs runs the 3 runs on that parity mesh's own .mbin: every output
#     equals the original's, and the room written 0 on run 0 must differ.
# (3) The tests the bed rests on: parity_inputs.rs (config.xml and mesh.cbin, value by value) and
#     mesh_mbin_parity.rs (the .mbin builder, byte for byte).
# (4) Upstream's shipped 1.3.4 and 1.4.0 SPPS and TCR against ours on tutorial 1's original inputs
#     (the bed's ignored test, given $ReleaseBinaries): ours must be bit-identical to 1.4.0; 1.3.4's
#     differences are measured and printed.
# Every check has a "says NO":
# - (1) refuses upstream's own TetGen 1.6.0 build (the 929a5c8 reference, built by
#   solvers/build.ps1 beside ours) in place of ours;
# - (2) the verdict reader must read every test a bed transcript lists as failed as FAILED, and no
#   test as passed when the failed count and that list disagree;
#   the bed run with that TetGen 1.6.0 in the solver folder must FAIL tutorial_1, naming
#   TetGen's files; the_comparisons_say_no feeds each comparison the input that fails it, and
#   tutorial_3_same_seed_runs the room written 0; tutorial_3 must print its three says-no:
#   preprocessing off refused on the box's self-intersections (our check's pairs TetGen's own),
#   TetGen 1.6.0's wrong room (1,220.9 m3 of 978.3) refused by the region volume check, and
#   upstream's .poly with one region line changed giving another .mbin;
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
    [pscustomobject]@{ Code = $code; Text = $text; Results = (Get-TestVerdicts $text) }
}
# Per test of the output of one or more libtest binaries, 'ok', 'FAILED', 'ignored',
# 'unfinished' or 'unreadable'. With --nocapture a test's own lines come between its
# 'test <name> ... ' and its verdict, so the verdict is read from what libtest prints last: an
# ignored test says so on its own line, a failed one is listed, one name per line, under its
# binary's last 'failures:', and one that ran otherwise passed, once 'test result:' lines show
# the binaries finished. When their failed counts do not add up to the names listed, no test that
# ran is read as passed.
function Get-TestVerdicts([string]$text) {
    $results = @{}
    $summary = [regex]::Matches($text, '(?m)^test result: \S+ (\d+) passed; (\d+) failed;')
    $failedCount = 0; foreach ($s in $summary) { $failedCount += [int]$s.Groups[2].Value }
    $failed = @(foreach ($l in [regex]::Matches($text, '(?m)^failures:[ \t]*\r?\n((?:    \S+[ \t]*(?:\r?\n|$))+)')) {
        $l.Groups[1].Value -split "`n" | ForEach-Object { $_.Trim() } | Where-Object { $_ }
    })
    $readable = $summary.Count -gt 0 -and $failedCount -eq $failed.Count
    # A test whose own output starts on the next line leaves 'test <name> ...' and a space, which
    # an editor may strip: the space is optional.
    foreach ($m in [regex]::Matches($text, '(?m)^test (\S+) \.\.\.(?:[ \t](.*))?$')) {
        $name = $m.Groups[1].Value
        # libtest prints an ignored test's reason after it: 'ignored, <reason>'.
        if ($m.Groups[2].Value.Trim() -match '^ignored(,|$)') { $results[$name] = 'ignored' }
        elseif ($failed -contains $name) { $results[$name] = 'FAILED' }
        elseif ($summary.Count -eq 0) { $results[$name] = 'unfinished' }
        elseif ($readable) { $results[$name] = 'ok' }
        else { $results[$name] = 'unreadable' }
    }
    $results
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
$script:bed = CargoTest 'cargo test -p simpa --test parity_tutorials --no-fail-fast -- --nocapture --test-threads=1' 'bed' @{ SIMPA_TETGEN160 = $Tetgen160 }
foreach ($t in @('tutorial_1', 'tutorial_2', 'tutorial_3', 'tutorial_3_same_seed_runs', 'the_comparisons_say_no')) {
    Check "(2) bed: $t" {
        $r = $script:bed.Results[$t]
        Write-Host "      $t ... $r"
        $r -eq 'ok'
    }
}
# The shape of a real bed run's tail (2026-09-24, spps.exe with one code byte changed): three
# tests failed, and before the reader took every listed name, two of them read as passed.
$threeFailed = @'
test the_comparisons_say_no ...
thread 'the_comparisons_say_no' (6712) panicked at crates\simpa\tests\parity_tutorials.rs:394:5:
test tutorial_1 ... t1-tcr: 15 output files from the original's inputs, 15 from ours; 0 differ
thread 'tutorial_1' (7536) panicked at crates\simpa\tests\parity_tutorials.rs:394:5:
test tutorial_2 ... tutorial 2: TetGen argv ["-pq2","-A","-n","scene_mesh.poly"]
ok
test tutorial_3_same_seed_runs ...
FAILED

failures:

failures:
    the_comparisons_say_no
    tutorial_1
    tutorial_3_same_seed_runs

test result: FAILED. 1 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 16.42s
'@
Check "(2) says NO: the verdict reader reads each of 3 failed tests of a bed transcript as FAILED, and reads no test as passed when the failed count and the list disagree" {
    $v = Get-TestVerdicts $threeFailed
    $cut = Get-TestVerdicts ($threeFailed -replace '(?m)^    tutorial_1\r?\n', '')
    Write-Host "      $(($v.GetEnumerator() | Sort-Object Name | ForEach-Object { "$($_.Name) $($_.Value)" }) -join ', '); with tutorial_1 cut from the list: tutorial_1 $($cut['tutorial_1']), tutorial_2 $($cut['tutorial_2'])"
    ($v['the_comparisons_say_no'] -eq 'FAILED') -and ($v['tutorial_1'] -eq 'FAILED') -and ($v['tutorial_3_same_seed_runs'] -eq 'FAILED') -and
        ($v['tutorial_2'] -eq 'ok') -and ($cut['tutorial_1'] -eq 'unreadable') -and ($cut['tutorial_2'] -eq 'unreadable')
}
Check "(2) bed: the one test the plain suite ignores says so, with its reason, never skipped silently" {
    $b = $script:bed.Results['shipped_1_3_4_and_1_4_0_solvers_against_ours']
    $ignored = @($script:bed.Results.GetEnumerator() | Where-Object { $_.Value -eq 'ignored' } | ForEach-Object { $_.Key })
    Write-Host "      shipped_1_3_4_and_1_4_0_solvers_against_ours ... $b; ignored: [$($ignored -join ', ')]"
    $b -eq 'ignored' -and $ignored.Count -eq 1
}
Check "(2) bed: tutorial_3 end to end: the parity mesh's .poly, .1.* and .mbin are upstream's through the id map, and the default mesh verifies clean" {
    $t = $script:bed.Text
    $poly = [regex]::Match($t, '(?m)^  simpa mesh --parity: .*?: (\d+) bytes, sha256 ([0-9a-f]{16}), upstream''s temp/scene_mesh\.poly byte for byte through the id map')
    $tg = [regex]::Matches($t, '(?m)^  scene_mesh\.1\.(node|ele|face|neigh|edge): byte-identical, trailer excluded \(the parity mesh''s TetGen').Count
    $mbin = [regex]::Match($t, '(?m)^  the parity mesh''s tetramesh\.mbin: each run''s, byte for byte \((\d+) bytes\), through the id map')
    $verify = [regex]::IsMatch($t, '(?m)^  the parity mesh''s verification: FAIL, by name: 280 marker mismatches')
    $default = [regex]::Match($t, '(?m)^  simpa mesh \(default\): OK; .*')
    Write-Host "      .poly: $(if ($poly.Success) { "$($poly.Groups[1].Value) bytes, $($poly.Groups[2].Value)" } else { 'no line' }); TetGen files equal: $tg of 5; .mbin: $(if ($mbin.Success) { "$($mbin.Groups[1].Value) bytes" } else { 'no line' }); parity verification names the defect: $verify"
    Write-Host "      $(if ($default.Success) { $default.Value.Trim() } else { 'default mode: no line' })"
    $poly.Success -and $poly.Groups[1].Value -eq '4889' -and $poly.Groups[2].Value -eq '74b8f8311d5d0f64' -and $tg -eq 5 -and
        $mbin.Success -and $mbin.Groups[1].Value -eq '338528' -and $verify -and $default.Success
}
Check "(2) says NO: tutorial_3 printed its three refusals (preprocessing off, TetGen 1.6.0's wrong room, a changed region line)" {
    $t = $script:bed.Text
    $off = [regex]::Match($t, '(?m)^  says no, preprocess off: exit 4, .*?tetgen_self_intersection.*?the same (\d+) pairs TetGen 1\.5\.0''s -d names')
    $wrong = [regex]::Match($t, '(?m)^  says no, TetGen 1\.6\.0 on the raw scene: .*?of ([0-9.]+) m\S* against the room''s ([0-9.]+) m\S*.*?fails the region volume check: .*?region_volume_mismatch.*?unmeshed_cells')
    $misplaced = [regex]::IsMatch($t, '(?m)^  says no, TetGen 1\.6\.0 in parity mode: .*fitting_region_misplaced')
    $region = [regex]::Match($t, '(?m)^  says no, upstream''s \.poly with zone 1''s seed 1 cm up: another \.mbin, (\d+) tetrahedra with another idVolume')
    Write-Host "      preprocess off: $(if ($off.Success) { "$($off.Groups[1].Value) pairs, ours = TetGen's" } else { 'no line' }); TetGen 1.6.0: $(if ($wrong.Success) { "$($wrong.Groups[1].Value) m3 of $($wrong.Groups[2].Value) m3, refused by the volume check" } else { 'no line' }); 1.6.0 in parity mode names zone 1 misplaced: $misplaced; region line: $(if ($region.Success) { "$($region.Groups[1].Value) tetrahedra relabelled" } else { 'no line' })"
    $off.Success -and $wrong.Success -and $misplaced -and $region.Success -and [int]$region.Groups[1].Value -gt 0
}
Check "(2) bed: tutorial_3_same_seed_runs printed its 3 runs equal and the room written 0 refused" {
    $eq = [regex]::Matches($script:bed.Text, '(?m)^  run \d: all \d+ output files equal to the original inputs'' run').Count
    $no = [regex]::Match($script:bed.Text, '(?m)^  run 0 with the room written 0 \(the builder before 2026-09-24\): (\d+) of (\d+) files differ')
    Write-Host "      runs equal: $eq of 3; room written 0: $(if ($no.Success) { "$($no.Groups[1].Value) of $($no.Groups[2].Value) files differ" } else { 'no line' })"
    $eq -eq 3 -and $no.Success -and [int]$no.Groups[1].Value -gt 0
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
