# Parity gate: our build against original I-Simpa (docs/m5-m6-design.md, decision 3, "the parity
# bed"). Runs the bed and the tests it rests on, and says what each proves:
# (1) The solvers under test are the committed build: solvers/manifest.json's code sha256
#     (solvers/pe-fingerprint.ps1, the sha256 with the link timestamps zeroed, the same for every
#     build of the same code) of spps.exe, classicalTheory.exe, tetgen.exe and preprocess.exe, in
#     $SolversDir. Their sha256 is printed beside it: the manifest's link or another one.
# (2) The bed, crates/simpa/tests/parity_tutorials.rs: upstream's tutorials 1, 2 and 3, imported
#     and meshed by `simpa`, against the files original I-Simpa wrote into each .proj, and SPPS and
#     TCR run with one seed on our inputs and on the original inputs, every output compared.
#     tutorial_1, tutorial_2, tutorial_3, tutorial_3_same_seed_runs,
#     tutorial_3_each_region_check_says_no_alone and the_comparisons_say_no must pass, and the four
#     tests the plain suite ignores ((4) and (5)) must say so. tutorial_3 is end to end
#     (decision 12): import-proj, then `simpa mesh --parity` through preprocess.exe, our geometry
#     check, TetGen 1.5.0 and our builder, whose .poly, .1.* and .mbin must be upstream's byte for
#     byte with no id map (the import pins upstream's element ids, decision 13): the bed prints our
#     files' own size and sha256, and the gate holds them to upstream's. The default mode must mesh
#     it clean, each region its cell's volume. tutorial_2 meshes the .proj as it is: preprocess.exe
#     gives up on the hall, the .poly as written is meshed as upstream's GUI meshes it, the abort
#     recorded (preprocess.outcome "aborted") and said on stderr. tutorial_3_same_seed_runs runs
#     the 3 runs on that parity mesh's own .mbin: every output equals the original's, and the room
#     written 0 on run 0 must differ. tutorial_3_each_region_check_says_no_alone feeds each of the
#     four region checks a change of the default mesh only it can see.
# (3) The tests the bed rests on: parity_inputs.rs (config.xml and mesh.cbin, value by value) and
#     mesh_mbin_parity.rs (the .mbin builder, byte for byte).
# (4) Upstream's shipped 1.3.4 and 1.4.0 solvers against ours (the bed's ignored tests, given
#     $ReleaseBinaries): SPPS and TCR on tutorial 1's original inputs, where ours must be
#     bit-identical to 1.4.0 and 1.3.4's differences are measured and printed; and SPPS on
#     tutorial 3's parity mesh with transmission on, where 1.4.0 must write every file as ours and
#     1.3.4 must lose more than 10 % to loops too (docs/upstream-findings.md).
# (5) Tutorial 3's parity mesh against its default mesh (decision 12; the bed's heavy ignored
#     tests): the stored run with transmission on, seed 1, loses more than 10 % of its particle
#     records to loops on the parity mesh and under 0.01 % on the default mesh; the run manager
#     refuses the parity mesh before launch (run-folder exit 5, run --mesh exit 4); the run
#     verdict judges the parity run FAIL with particle_loss_excess; and the receiver levels of
#     the two meshes, 3 seeds each, differ by at most 0.2 dB (docs/upstream-findings.md).
# Every check has a "says NO":
# - (1) refuses a copy of the folder with one byte of spps.exe's .text inverted, and upstream's
#   own TetGen 1.6.0 build (the 929a5c8 reference, built by solvers/build.ps1 beside ours) in
#   place of ours;
# - (2) the verdict reader must read every test a bed transcript lists as failed as FAILED, and no
#   test as passed when the failed count and that list disagree;
#   the bed run with that TetGen 1.6.0 in the solver folder must FAIL tutorial_1, naming
#   TetGen's files; the_comparisons_say_no feeds each comparison the input that fails it, and
#   tutorial_3_same_seed_runs the room written 0; tutorial_3 must print its says-no:
#   preprocessing off refused on the box's self-intersections (our check's pairs TetGen's own),
#   TetGen 1.6.0's wrong room (1,220.9 m3 of 978.3) refused by the region volume check,
#   upstream's .poly with one region line changed giving another .mbin, zone 1 pinned to 1931
#   giving another .poly and .mbin, and both zones pinned to 1930 refused before meshing; the
#   tutorial_3 end-to-end reading refuses a transcript with any one of its values changed or
#   its lines missing;
# - (4) asserts that 1.3.4 differs, so the comparison can tell two builds apart;
# - (5) its reading refuses a transcript with any one of its findings changed, and its tests
#   carry their own: the default mesh is run OK by the same run manager, and the level bound
#   refuses the default mesh's levels 1 dB up.
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
    # CRLF reads as LF. '$' in .NET's multiline mode matches only before '\n', so a 'test <name> ...'
    # line ending in '\r' went unread, its test with it: the transcript below is CRLF whenever git
    # checks this file out with CRLF line ends (core.autocrlf true), and two of its three failed
    # tests vanished from the verdicts.
    $text = $text -replace "`r`n", "`n"
    $results = @{}
    $summary = [regex]::Matches($text, '(?m)^test result: \S+ (\d+) passed; (\d+) failed;')
    $failedCount = 0; foreach ($s in $summary) { $failedCount += [int]$s.Groups[2].Value }
    $failed = @(foreach ($l in [regex]::Matches($text, '(?m)^failures:[ \t]*\r?\n((?:    \S+[ \t]*(?:\r?\n|$))+)')) {
        $l.Groups[1].Value -split "`n" | ForEach-Object { $_.Trim() } | Where-Object { $_ }
    })
    $readable = $summary.Count -gt 0 -and $failedCount -eq $failed.Count
    # A test whose own output starts on the next line leaves 'test <name> ...' and a space, which
    # an editor may strip: the space is optional. `$` needs the CRLF-to-LF above: it would not
    # match before a '\r', and every bare line of a CRLF transcript would go unread.
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
. (Join-Path $repo 'solvers\pe-fingerprint.ps1')

$build = cmd /c "cargo build -q -p simpa --tests 2>&1"
if ($LASTEXITCODE -ne 0) { $build | Select-Object -Last 20 | ForEach-Object { Write-Host $_ }; throw 'build failed: refusing to test stale code' }

# --- (1) the solvers under test -------------------------------------------------------------------
$manifest = Get-Content (Join-Path $repo 'solvers\manifest.json') -Raw | ConvertFrom-Json
$exes = @('spps.exe', 'classicalTheory.exe', 'tetgen.exe', 'preprocess.exe')
function Short([string]$h) { if ($h -match '^[0-9a-f]{64}$') { $h.Substring(0, 16) } else { "'$h'" } }
# Whether a folder holds solvers/manifest.json's build: each executable's code sha256 is the
# manifest's. Prints both hashes of each, and whether its sha256 is the manifest's link.
function Test-SolverFolder([string]$dir) {
    $ok = $true
    foreach ($e in $exes) {
        $p = Join-Path $dir $e
        if (-not (Test-Path $p)) { throw "$p is missing" }
        $code = Get-CodeSha256 $p; $raw = Get-RawSha256 $p; $want = "$($manifest.code_sha256.$e)"
        if ($want -notmatch '^[0-9a-f]{64}$') { throw "solvers/manifest.json has no code sha256 for $e" }
        $link = if ($raw -eq $manifest.sha256.$e) { "the manifest's link" } else { "another link than the manifest's $(Short $manifest.sha256.$e)" }
        Write-Host ("      {0,-20} code sha256 {1} {2}; sha256 {3}, {4}" -f $e, (Short $code), $(if ($code -eq $want) { 'manifest' } else { "NOT the manifest's $(Short $want)" }), (Short $raw), $link)
        if ($code -ne $want) { $ok = $false }
    }
    $ok
}
Check "(1) the solvers under test are solvers/manifest.json's build (code sha256 of all 4)" {
    $ok = Test-SolverFolder $SolversDir
    Write-Host "      TetGen: $($manifest.tetgen.version), $($manifest.tetgen.source)"
    $ok -and $manifest.tetgen.version -eq '1.5.0'
}
Check "(1) says NO: the same folder with one byte of spps.exe's .text inverted is not the manifest's build" {
    $flipped = Join-Path $work 'solvers-spps-flipped'
    New-Item -ItemType Directory -Force $flipped | Out-Null
    foreach ($e in $exes) { if ($e -ne 'spps.exe') { Copy-Item (Join-Path $SolversDir $e) (Join-Path $flipped $e) } }
    $null = Copy-PeFlippedText (Join-Path $SolversDir 'spps.exe') (Join-Path $flipped 'spps.exe')
    -not (Test-SolverFolder $flipped)
}
Check "(1) says NO: upstream's TetGen 1.6.0 build is not the manifest's tetgen.exe" {
    if (-not (Test-Path $Tetgen160)) { throw "$Tetgen160 is missing: build it with solvers/build.ps1, or pass -Tetgen160" }
    $code = Get-CodeSha256 $Tetgen160
    Write-Host "      $Tetgen160 code sha256 $(Short $code), sha256 $(Short (Get-RawSha256 $Tetgen160)); manifest's tetgen.exe $(Short $manifest.code_sha256.'tetgen.exe'); the manifest's 1.6.0 reference $(Short $manifest.tetgen.upstream_160_reference_code_sha256)"
    ("$($manifest.code_sha256.'tetgen.exe')" -match '^[0-9a-f]{64}$') -and $code -ne $manifest.code_sha256.'tetgen.exe'
}

# --- (2) the bed -------------------------------------------------------------------------------------
$script:bed = CargoTest 'cargo test -p simpa --test parity_tutorials --no-fail-fast -- --nocapture --test-threads=1' 'bed' @{ SIMPA_TETGEN160 = $Tetgen160 }
foreach ($t in @('tutorial_1', 'tutorial_2', 'tutorial_3', 'tutorial_3_same_seed_runs', 'tutorial_3_each_region_check_says_no_alone', 'the_comparisons_say_no')) {
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
$ignoredTests = @('shipped_1_3_4_and_1_4_0_solvers_against_ours', 'shipped_solvers_on_tutorial_3_parity_mesh', 'tutorial_3_loops_parity_against_default', 'tutorial_3_receiver_levels_default_against_parity')
Check "(2) bed: the four tests the plain suite ignores say so, with their reasons, never skipped silently, and no other is ignored" {
    $ignored = @($script:bed.Results.GetEnumerator() | Where-Object { $_.Value -eq 'ignored' } | ForEach-Object { $_.Key } | Sort-Object)
    $reasons = @([regex]::Matches($script:bed.Text, '(?m)^test (\S+) \.\.\. ignored, (.+)$') | ForEach-Object { "$($_.Groups[1].Value): $($_.Groups[2].Value.Trim())" })
    Write-Host "      ignored: [$($ignored -join ', ')]"
    $reasons | ForEach-Object { Write-Host "        $_" }
    ($ignored -join ',') -eq (($ignoredTests | Sort-Object) -join ',') -and $reasons.Count -eq $ignoredTests.Count
}
# The tutorial_3 end-to-end lines of a bed transcript: the parity mesh's .poly and .mbin, our files'
# own size and the first 16 hex digits of their sha256 as the bed prints them, must be upstream's
# (4,889 bytes, 74b8f8311d5d0f64; 338,528 bytes, bc2f0904b13695d6); its 5 TetGen files equal; its
# verification failing by name; the default mesh OK.
function Test-T3EndToEnd([string]$t, [switch]$Quiet) {
    $poly = [regex]::Match($t, '(?m)^  simpa mesh --parity: .*?: (\d+) bytes, sha256 ([0-9a-f]{16}), upstream''s temp/scene_mesh\.poly byte for byte, with no id map')
    $tg = [regex]::Matches($t, '(?m)^  scene_mesh\.1\.(node|ele|face|neigh|edge): byte-identical, trailer excluded \(the parity mesh''s TetGen, with no id map').Count
    $mbin = [regex]::Match($t, '(?m)^  the parity mesh''s tetramesh\.mbin: each run''s, byte for byte \((\d+) bytes, sha256 ([0-9a-f]{16})\), with no id map')
    $verify = [regex]::IsMatch($t, '(?m)^  the parity mesh''s verification: FAIL, by name: 280 marker mismatches')
    $default = [regex]::Match($t, '(?m)^  simpa mesh \(default\): OK; .*')
    if (-not $Quiet) {
        Write-Host "      .poly: $(if ($poly.Success) { "$($poly.Groups[1].Value) bytes, $($poly.Groups[2].Value)" } else { 'no line' }); TetGen files equal: $tg of 5; .mbin: $(if ($mbin.Success) { "$($mbin.Groups[1].Value) bytes, $($mbin.Groups[2].Value)" } else { 'no line' }); parity verification names the marker mismatches: $verify"
        Write-Host "      $(if ($default.Success) { $default.Value.Trim() } else { 'default mode: no line' })"
    }
    $poly.Success -and $poly.Groups[1].Value -eq '4889' -and $poly.Groups[2].Value -eq '74b8f8311d5d0f64' -and $tg -eq 5 -and
        $mbin.Success -and $mbin.Groups[1].Value -eq '338528' -and $mbin.Groups[2].Value -eq 'bc2f0904b13695d6' -and $verify -and $default.Success
}
Check "(2) bed: tutorial_3 end to end: the parity mesh's .poly, .1.* and .mbin are upstream's byte for byte with no id map (our files' size and sha256), and the default mesh verifies clean" {
    Test-T3EndToEnd $script:bed.Text
}
# The end-to-end lines as a passing bed printed them (2026-09-24), shortened where the reading
# does not look.
$t3Lines = @'
  simpa mesh --parity: our .poly (4883 bytes, 88 facets, the box's 12 triangles in the user facet list) through preprocess.exe (preprocess.exe: 28 vertices merged, 2 faces destroyed, 31 splits): 4889 bytes, sha256 74b8f8311d5d0f64, upstream's temp/scene_mesh.poly byte for byte, with no id map
  scene_mesh.1.node: byte-identical, trailer excluded (the parity mesh's TetGen, with no id map)
  scene_mesh.1.ele: byte-identical, trailer excluded (the parity mesh's TetGen, with no id map)
  scene_mesh.1.face: byte-identical, trailer excluded (the parity mesh's TetGen, with no id map)
  scene_mesh.1.neigh: byte-identical, trailer excluded (the parity mesh's TetGen, with no id map)
  scene_mesh.1.edge: byte-identical, trailer excluded (the parity mesh's TetGen, with no id map)
  the parity mesh's tetramesh.mbin: each run's, byte for byte (338528 bytes, sha256 bc2f0904b13695d6), with no id map; 3285 tetrahedra, idVolume {1930, 2083, 2084, 2085, 2086}
  the parity mesh's verification: FAIL, by name: 280 marker mismatches (the box's facets all marker 88, upstream's reader), faces [90,91,92,93,94,95,96,97,98,99] uncovered, 88 and 89 deleted by preprocess; every region its cell's volume
  simpa mesh (default): OK; 19 of preprocess.exe's markers restored; zone 1's seed moved; 3394 tetrahedra
'@
Check "(2) says NO: the tutorial_3 end-to-end reading passes the lines of a passing bed and refuses each with one value changed or one line gone" {
    $cases = [ordered]@{
        '.poly one byte short' = $t3Lines -replace ': 4889 bytes, sha256', ': 4888 bytes, sha256'
        '.poly another sha256' = $t3Lines -replace '74b8f8311d5d0f64', '74b8f8311d5d0f65'
        'a TetGen file not equal' = $t3Lines -replace '(?m)^  scene_mesh\.1\.neigh: byte-identical', '  scene_mesh.1.neigh: differs'
        '.mbin one byte short' = $t3Lines -replace '\(338528 bytes', '(338527 bytes'
        '.mbin another sha256' = $t3Lines -replace 'bc2f0904b13695d6', 'bc2f0904b13695d7'
        'the verification line gone' = $t3Lines -replace '(?m)^  the parity mesh''s verification: .*\r?\n', ''
        'the default line gone' = $t3Lines -replace '(?m)^  simpa mesh \(default\): .*\r?\n?', ''
    }
    $good = Test-T3EndToEnd $t3Lines -Quiet
    $passed = @($cases.GetEnumerator() | Where-Object { $_.Value -eq $t3Lines -or (Test-T3EndToEnd $_.Value -Quiet) } | ForEach-Object { $_.Key })
    Write-Host "      the lines as printed: $good; changed and still read as passing: [$($passed -join '; ')] of $($cases.Count)"
    $good -and $passed.Count -eq 0
}
Check "(2) bed: tutorial_2 meshes as the .proj asks: preprocess.exe gives up, the .poly as written is meshed and the abort said" {
    $t2 = [regex]::Match($script:bed.Text, '(?m)^  simpa mesh on the \.proj as it is: OK; preprocess\.exe gives up after (\d+) splits and saves nothing, .*?; stderr: (simpa: note: preprocess\.exe .*)$')
    Write-Host "      $(if ($t2.Success) { "$($t2.Groups[1].Value) splits; $($t2.Groups[2].Value.Substring(0, [math]::Min(120, $t2.Groups[2].Value.Length)))" } else { 'no line' })"
    $t2.Success -and [int]$t2.Groups[1].Value -gt 0
}
Check "(2) says NO: tutorial_3 printed its refusals (preprocessing off, TetGen 1.6.0's wrong room, a changed region line, a changed pin, two zones pinned alike)" {
    $t = $script:bed.Text
    $off = [regex]::Match($t, '(?m)^  says no, preprocess off: exit 4, .*?tetgen_self_intersection.*?the same (\d+) pairs TetGen 1\.5\.0''s -d names')
    $wrong = [regex]::Match($t, '(?m)^  says no, TetGen 1\.6\.0 on the raw scene: .*?of ([0-9.]+) m\S* against the room''s ([0-9.]+) m\S*.*?fails the region volume check: .*?region_volume_mismatch.*?unmeshed_cells')
    $misplaced = [regex]::IsMatch($t, '(?m)^  says no, TetGen 1\.6\.0 in parity mode: .*fitting_region_misplaced')
    $region = [regex]::Match($t, '(?m)^  says no, upstream''s \.poly with zone 1''s seed 1 cm up: another \.mbin, (\d+) tetrahedra with another idVolume')
    $pin = [regex]::IsMatch($t, '(?m)^  says no, zone 1 pinned to 1931: the parity \.poly .*first difference.*; the \.mbin .*first difference')
    $clash = [regex]::IsMatch($t, '(?m)^  says no, both zones pinned to 1930: simpa mesh exits 2, .*solver_id_mapping_invalid')
    Write-Host "      preprocess off: $(if ($off.Success) { "$($off.Groups[1].Value) pairs, ours = TetGen's" } else { 'no line' }); TetGen 1.6.0: $(if ($wrong.Success) { "$($wrong.Groups[1].Value) m3 of $($wrong.Groups[2].Value) m3, refused by the volume check" } else { 'no line' }); 1.6.0 in parity mode names zone 1 misplaced: $misplaced; region line: $(if ($region.Success) { "$($region.Groups[1].Value) tetrahedra relabelled" } else { 'no line' })"
    Write-Host "      zone 1 pinned to 1931 gives other bytes: $pin; two zones pinned to 1930 refused before meshing: $clash"
    $off.Success -and $wrong.Success -and $misplaced -and $region.Success -and [int]$region.Groups[1].Value -gt 0 -and $pin -and $clash
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
if (-not (Test-Path (Join-Path $ReleaseBinaries 'inst140\spps.exe'))) { Write-Host "      $ReleaseBinaries holds no inst140\spps.exe: pass -ReleaseBinaries" }
$script:shipped = CargoTest 'cargo test -p simpa --test parity_tutorials -- shipped_1_3_4_and_1_4_0_solvers_against_ours shipped_solvers_on_tutorial_3_parity_mesh --ignored --exact --nocapture --test-threads=1' 'shipped' @{ SIMPA_RELEASE_BINARIES = $ReleaseBinaries }
Check "(4) upstream's shipped 1.4.0 SPPS and TCR are bit-identical to ours on tutorial 1; 1.3.4's differ (measured)" {
    $script:shipped.Text -split "`n" | Where-Object { $_ -match '^(spps|tcr): ' } | Select-Object -Last 6 | ForEach-Object { Write-Host "      $_" }
    $script:shipped.Results['shipped_1_3_4_and_1_4_0_solvers_against_ours'] -eq 'ok'
}
# The shipped solvers on tutorial 3's parity mesh, from the test's lines: 1.4.0's files all ours,
# and 1.3.4's loss to loops above 10 %, read from its counts.
function Test-T3Shipped([string]$t, [switch]$Quiet) {
    $same = [regex]::Match($t, '(?m)^  1\.4\.0 against ours: all (\d+) output files identical')
    $l134 = [regex]::Match($t, '(?m)^  1\.3\.4 spps\.exe on the parity mesh: (\d+) of (\d+) particle records lost to loops')
    $ratio = if ($l134.Success) { [double]$l134.Groups[1].Value / [double]$l134.Groups[2].Value } else { 0 }
    if (-not $Quiet) { Write-Host "      1.4.0 all files ours: $($same.Success); 1.3.4 loops: $(if ($l134.Success) { '{0:P4}' -f $ratio } else { 'no line' })" }
    $same.Success -and [int]$same.Groups[1].Value -gt 0 -and $ratio -gt 0.1
}
Check "(4) upstream's shipped 1.4.0 SPPS writes every file of tutorial 3's parity mesh run as ours, and 1.3.4 loses more than 10 % to loops too" {
    ($script:shipped.Results['shipped_solvers_on_tutorial_3_parity_mesh'] -eq 'ok') -and (Test-T3Shipped $script:shipped.Text)
}
$t3ShippedLines = @'
  1.3.4 spps.exe on the parity mesh: 533474 of 2649697 particle records lost to loops (20.1334 %), 50 to meshing; 24 output files
  1.4.0 against ours: all 24 output files identical
'@
Check "(4) says NO: the shipped tutorial 3 reading passes the lines of a passing run and refuses 1.3.4 at 10 % and 1.4.0 differing" {
    $cases = [ordered]@{
        '1.3.4 at 10 %' = $t3ShippedLines -replace '533474 of 2649697', '264969 of 2649697'
        '1.4.0 differs' = $t3ShippedLines -replace 'all 24 output files identical', '3 of 24 output files differ'
    }
    $good = Test-T3Shipped $t3ShippedLines -Quiet
    $passed = @($cases.GetEnumerator() | Where-Object { $_.Value -eq $t3ShippedLines -or (Test-T3Shipped $_.Value -Quiet) } | ForEach-Object { $_.Key })
    Write-Host "      the lines as printed: $good; changed and still read as passing: [$($passed -join '; ')] of $($cases.Count)"
    $good -and $passed.Count -eq 0
}

# --- (5) tutorial 3's parity mesh against its default mesh ----------------------------------------
$script:t3runs = CargoTest 'cargo test -p simpa --test parity_tutorials -- tutorial_3_loops_parity_against_default tutorial_3_receiver_levels_default_against_parity --ignored --exact --nocapture --test-threads=1' 't3-runs'
# The findings of the two tests, read from their lines: loops above 10 % on the parity mesh and
# under 0.01 % on the default mesh (from the counts), the run manager's two refusals and its OK,
# the run verdict's particle_loss_excess, and the level bound kept with its say-no printed.
function Test-T3Runs([string]$t, [switch]$Quiet) {
    $direct = [regex]::Match($t, '(?m)^  parity mesh, spps\.exe run directly: (\d+) of (\d+) particle records lost to loops .*?; judged by the run verdict \(limit 1 %\): FAIL, ([a-z_, ]+) \(')
    $default = [regex]::Match($t, '(?m)^  default mesh, simpa run-folder: exit 0, OK; (\d+) of (\d+) particle records lost to loops')
    $folder = [regex]::IsMatch($t, '(?m)^  parity mesh, simpa run-folder: exit 5, stage pre_launch, FAIL: mesh_invalid, marker_geometry_mismatches\r?$')
    $mesh = [regex]::IsMatch($t, '(?m)^  parity mesh, simpa run --mesh: exit 4, stage mesh, FAIL: mesh_missing\r?$')
    $levels = [regex]::Match($t, '(?m)^  largest mean difference ([0-9.]+) dB, bound 0\.2 dB; resolved at (\d+) of (\d+) receivers')
    $louder = [regex]::IsMatch($t, '(?m)^  says no, the default mesh''s levels 1 dB up: every receiver beyond the bound')
    $pr = if ($direct.Success) { [double]$direct.Groups[1].Value / [double]$direct.Groups[2].Value } else { 0 }
    $dr = if ($default.Success) { [double]$default.Groups[1].Value / [double]$default.Groups[2].Value } else { 1 }
    $codes = if ($direct.Success) { @($direct.Groups[3].Value -split ', ') } else { @() }
    if (-not $Quiet) {
        Write-Host ("      loops: parity mesh {0}, default mesh {1}; run-folder refuses the parity mesh before launch: {2}; run --mesh refuses it: {3}" -f $(if ($direct.Success) { '{0:P4}' -f $pr } else { 'no line' }), $(if ($default.Success) { '{0:P6}' -f $dr } else { 'no line' }), $folder, $mesh)
        Write-Host "      the run verdict on the parity run: [$($codes -join ', ')]; levels: $(if ($levels.Success) { "largest $($levels.Groups[1].Value) dB, resolved at $($levels.Groups[2].Value) of $($levels.Groups[3].Value)" } else { 'no line' }); the 1 dB say-no printed: $louder"
    }
    $direct.Success -and $pr -gt 0.1 -and $default.Success -and $dr -lt 1e-4 -and $folder -and $mesh -and ($codes -contains 'particle_loss_excess') -and
        $levels.Success -and [double]$levels.Groups[1].Value -le 0.2 -and [int]$levels.Groups[3].Value -eq 5 -and $louder
}
Check "(5) tutorial 3: the parity mesh loses more than 10 % to loops and the default mesh under 0.01 %; the run manager refuses the parity mesh before launch; the run verdict fails its run with particle_loss_excess; receiver levels within 0.2 dB" {
    $ok = ($script:t3runs.Results['tutorial_3_loops_parity_against_default'] -eq 'ok') -and ($script:t3runs.Results['tutorial_3_receiver_levels_default_against_parity'] -eq 'ok')
    Write-Host "      tutorial_3_loops_parity_against_default ... $($script:t3runs.Results['tutorial_3_loops_parity_against_default']); tutorial_3_receiver_levels_default_against_parity ... $($script:t3runs.Results['tutorial_3_receiver_levels_default_against_parity'])"
    (Test-T3Runs $script:t3runs.Text) -and $ok
}
$t3RunLines = @'
  parity mesh, simpa run-folder: exit 5, stage pre_launch, FAIL: mesh_invalid, marker_geometry_mismatches
  parity mesh, simpa run --mesh: exit 4, stage mesh, FAIL: mesh_missing
  default mesh, simpa run-folder: exit 0, OK; 2 of 2922796 particle records lost to loops (0.0001 %), 59 to meshing
  parity mesh, spps.exe run directly: 533967 of 2653740 particle records lost to loops (20.1213 %), 49 to meshing; judged by the run verdict (limit 1 %): FAIL, particle_loss_reported, particle_loss_excess (Warning 534016 particles has been in error on 2653740 particles.; 1 band(s) lost more than 1 % of their particles to loops and meshing)
  largest mean difference 0.119 dB, bound 0.2 dB; resolved at 1 of 5 receivers
  says no, the default mesh's levels 1 dB up: every receiver beyond the bound (smallest 0.930 dB)
'@
Check "(5) says NO: the tutorial 3 reading passes the lines of a passing run and refuses each finding changed" {
    $cases = [ordered]@{
        'parity loops at 10 %' = $t3RunLines -replace '533967 of 2653740', '265374 of 2653740'
        'default loops at 0.01 %' = $t3RunLines -replace '2 of 2922796', '293 of 2922796'
        'run-folder launches the parity mesh' = $t3RunLines -replace 'run-folder: exit 5, stage pre_launch, FAIL: mesh_invalid, marker_geometry_mismatches', 'run-folder: exit 0, stage solve, OK: '
        'run --mesh takes it' = $t3RunLines -replace 'run --mesh: exit 4, stage mesh, FAIL: mesh_missing', 'run --mesh: exit 0, stage solve, OK: '
        'the verdict without particle_loss_excess' = $t3RunLines -replace 'FAIL, particle_loss_reported, particle_loss_excess \(', 'FAIL, particle_loss_reported ('
        'levels 0.21 dB apart' = $t3RunLines -replace 'largest mean difference 0\.119 dB', 'largest mean difference 0.21 dB'
        'the level say-no gone' = $t3RunLines -replace '(?m)^  says no, the default mesh.*\r?\n?', ''
    }
    $good = Test-T3Runs $t3RunLines -Quiet
    $passed = @($cases.GetEnumerator() | Where-Object { $_.Value -eq $t3RunLines -or (Test-T3Runs $_.Value -Quiet) } | ForEach-Object { $_.Key })
    Write-Host "      the lines as printed: $good; changed and still read as passing: [$($passed -join '; ')] of $($cases.Count)"
    $good -and $passed.Count -eq 0
}

Write-Host "`nwork: $work"
$passed = $script:checks - $failures.Count - $blocked.Count
if ($failures.Count) { Write-Host "PARITY FAILED: $($failures.Count) of $script:checks checks failed, $($blocked.Count) BLOCKED, $passed passed"; exit 1 }
if ($blocked.Count) { Write-Host "PARITY PASSED EXCEPT $($blocked.Count) BLOCKED: $passed of $script:checks checks passed; blocked: $($blocked -join '; ')"; exit 3 }
Write-Host "PARITY PASSED: $script:checks of $script:checks checks"; exit 0
