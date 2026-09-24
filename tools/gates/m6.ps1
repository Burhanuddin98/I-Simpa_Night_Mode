# M6 gate: run manager and end-to-end solves (the arc item 12 milestone).
# Gate: docs/rebuild-plan-raw-2026-09-23.json lines 159-167 (M6), as amended by
# docs/m5-m6-design.md ("Gate amendments", decisions 9-11). Drives the release CLI:
#   simpa run <project> --solver spps|tcr [--mesh <dir>] --runs <root> --json   exit 0, 2, 3, 4, 5, 130
#   simpa run-folder <dir> --solver spps|tcr [--solver-exe <exe>] --runs <root> --json   exit 0, 5, 130
#   simpa mesh <project> --from-tetgen <dir> --out <dir> --json
# (a) rooms/tutorial1_box_seeded.simpa (seed 1, 10,000 particles), SPPS: exit 0, OK; 0 FAIL, 0 WARN
#     and 0 'Xml Property' lines; 0 stderr 'particles has been in error' lines; per band,
#     lost_by_meshing and lost_by_loops no higher than on upstream's own mesh of the same room with
#     the same seed and config (Burhan, 2026-09-24 05:13, "No worse than upstream", as (c) judges
#     the hall): its 2019 TetGen output (tests/fixtures/upstream/tutorial1/tetgen) built with
#     --from-tetgen, whose .mbin must be the 2019 tetramesh.mbin byte for byte; total = nbparticules
#     x sources per band (decision 10); every expected file present, the surface receiver's Global
#     .csbin included. A control, the box on TetGen 1.6.0's 6-tetrahedron mesh in upstream's corner
#     order (tests/fixtures/solver-outputs/tutorial1), must meet every clause; judged against that
#     control, which loses none, the box's own run must be refused.
# (b) The same box, TCR: exit 0, OK, 'Main results.gabe' and 'Punctual receivers/<lbl>.gabe' present.
# (c) rooms/elmia_loss_gate.simpa (seed 1, 100,000 particles per source, 125-4,000 Hz), SPPS: exit 0,
#     OK, 0 stderr particle-loss lines. Per band, its lost_by_meshing must be <= the floor measured in
#     the same gate run on upstream's own tutorial_2 mesh (extracted at gate time, built with
#     --from-tetgen) with the identical config, plus the tolerance 4 sqrt(max(floor, 1))
#     (decision 9: PROPOSED, open decision 7). The per-band ratios are printed.
# (d) Every negative case in tests/fixtures/runs through run-folder (the stub solver for stub_*):
#     exit 5, status FAIL or CRASH, the expected reason codes; reported as 'N/N'. The 11 cases the
#     gate names must be among them. Cases expected OK are positives, reported apart.
# (e) Every row of docs/solver-contract.md's classification table is hit by some fixture's solver
#     output through run-folder, and the stub whose final stderr line has no newline has that line
#     captured.
# (f) --cancel-after-progress 1 exits 130, CANCELLED, with SPPS killed mid-run, and 2 s later
#     `tasklist /FI "IMAGENAME eq spps.exe"` lists none. Stop-Process on simpa.exe mid-run also
#     leaves no spps.exe 2 s later.
# (g) A run under a path containing U+0141 is OK with the same file count as its ASCII twin.
# (h) Two runs of one project get two distinct folders, and no 'R10'-style suffixed receiver appears.
# Every check that can pass has a "says NO" check that must refuse an input:
# - a clause's own refusal sits beside it, named "says NO", or is a control that must fail the
#   same predicate ((a)'s control, (f)'s long box left alone);
# - (d)'s three checks share the expectation rules and the named-case lookup, refused in
#   "(d) says NO"; (c)'s config identity and per-band bound each have a "says NO";
# - (c)'s free-space, floor-mesh and run checks are preconditions of the bound, not claims of
#   their own: each fails on its own failure, and the bound check throws without them;
# - the tail (tests, clippy, fmt) must pass a clean scratch crate and refuse a copy with one
#   planted fault each, and the tests that need upstream's tree or the solvers must FAIL, with
#   their panic, when those are missing.
# A check that an open decision blocks prints BLOCKED; the gate then exits 3, never 0.
# Every solver call has a hard timeout, 45 minutes for a hall SPPS run and 5 minutes for anything
# else: a timeout is a FAIL with its time.
# Run: powershell -File tools/gates/m6.ps1
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Set-Location $repo
$env:RUSTUP_HOME = "$env:USERPROFILE\.rustup"; $env:CARGO_HOME = "$env:USERPROFILE\.cargo"
$env:Path = "$env:CARGO_HOME\bin;$env:Path"; $env:CARGO_INCREMENTAL = '0'
Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue
# House style from M4; no test reads it today. Tests that need upstream's tree or the solvers panic
# whenever those are missing (crates/simpa-core/tests/common/paths.rs), whatever this says, and the
# check "tests say NO without their inputs" proves that on every run of this gate.
$env:SIMPA_REQUIRE_UPSTREAM = '1'
$failures = @(); $blocked = @(); $script:checks = 0
# A check body returns exactly one bool, or `Blocked <reason>` when an open decision stops it.
# Anything else (no value, several values, a non-bool) is a FAIL: a stray value leaking into the
# pipeline must never turn into a PASS.
function Blocked([string]$reason) { @{ blocked = $reason } }
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

$build = cmd /c "cargo build -q --release -p simpa 2>&1"
if ($LASTEXITCODE -ne 0) { $build | Select-Object -Last 20 | ForEach-Object { Write-Host $_ }; throw 'CLI build failed: refusing to test a stale simpa.exe' }
# Runs a cargo command; on failure prints its last lines so a FAIL is never silent. Its whole
# output stays in $script:cargoText for the checks that must see why it failed.
function Cargo([string]$cmdline) {
    $out = cmd /c "$cmdline 2>&1"
    $code = $LASTEXITCODE
    $script:cargoText = (@($out) | ForEach-Object { "$_" }) -join "`n"
    if ($code -ne 0) { $out | Select-Object -Last 15 | ForEach-Object { Write-Host "      | $_" }; return $false }
    return $true
}
$simpa = Join-Path $repo 'target\release\simpa.exe'
$stub = Join-Path $repo 'target\release\simpa-stub-solver.exe'
$fx = Join-Path $repo 'tests\fixtures'
$work = Join-Path $repo ('target\gates\m6\' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
$calls = Join-Path $work 'calls'
New-Item -ItemType Directory -Force $work, $calls | Out-Null
Write-Host "work: $work"

# --- Helpers ----------------------------------------------------------------------------------

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
# A text file another process may still hold open, as UTF-8.
function ReadText([string]$path) {
    if (-not (Test-Path -LiteralPath $path)) { return '' }
    $fs = New-Object IO.FileStream -ArgumentList $path, 'Open', 'Read', 'ReadWrite'
    try { return (New-Object IO.StreamReader -ArgumentList $fs, ([Text.Encoding]::UTF8)).ReadToEnd() } finally { $fs.Dispose() }
}
# Starts simpa with the given arguments; stdout and stderr go to numbered files under calls\.
$script:callNo = 0
function Start-Simpa([string[]]$argv, [string]$label) {
    $script:callNo++
    $base = Join-Path $calls ('{0:D3}-{1}' -f $script:callNo, $label)
    $p = Start-Process -FilePath $simpa -ArgumentList (($argv | ForEach-Object { QuoteArg $_ }) -join ' ') `
        -RedirectStandardOutput "$base.stdout.txt" -RedirectStandardError "$base.stderr.txt" -NoNewWindow -PassThru
    $null = $p.Handle   # keeps ExitCode readable once the process has ended
    [pscustomobject]@{ Process = $p; Base = $base; Clock = [Diagnostics.Stopwatch]::StartNew() }
}
# simpa with a hard time limit. A call still running at the limit is killed (its Job Object then
# ends the solver) and throws, so the check FAILs with the elapsed time. .Json holds stdout parsed
# when it is a JSON object.
function Simpa([string[]]$argv, [string]$label, [int]$limitSec = 300) {
    $s = Start-Simpa $argv $label
    if (-not $s.Process.WaitForExit($limitSec * 1000)) {
        Stop-Process -Id $s.Process.Id -Force -ErrorAction SilentlyContinue
        $null = $s.Process.WaitForExit(10000)
        throw ('TIMEOUT: {0} still ran after {1:N1} s (limit {2} s) and was killed; logs {3}.*' -f $label, $s.Clock.Elapsed.TotalSeconds, $limitSec, $s.Base)
    }
    $s.Process.WaitForExit()
    $out = ReadText "$($s.Base).stdout.txt"
    $json = $null
    if ($out.TrimStart().StartsWith('{')) { $json = $out | ConvertFrom-Json }
    [pscustomobject]@{ Exit = $s.Process.ExitCode; Out = $out; Err = (ReadText "$($s.Base).stderr.txt"); Json = $json; Sec = $s.Clock.Elapsed.TotalSeconds }
}
# Whether tasklist lists a process of this image name.
function ImageRunning([string]$image) {
    $out = (& tasklist.exe /FI "IMAGENAME eq $image" /NH) -join "`n"
    return $out.ToLower().Contains($image.ToLower())
}
# A copy of a project file with exact text edits, each of which must match exactly once.
function Edited([string]$from, [string]$to, $edits) {
    $text = [IO.File]::ReadAllText($from)
    foreach ($e in $edits) {
        $n = ([regex]::Matches($text, [regex]::Escape($e[0]))).Count
        if ($n -ne 1) { throw "edit of ${from}: '$($e[0])' matches $n times, not once" }
        $text = $text.Replace($e[0], $e[1])
    }
    [IO.File]::WriteAllText($to, $text, (New-Object Text.UTF8Encoding $false))
    $to
}
function RunDir($m) { Split-Path -Parent $m.cwd }
function Codes($m) { @($m.verdict.reasons | ForEach-Object { $_.code }) }
function Config($m) { [xml](ReadText (Join-Path $m.cwd 'config.xml')) }
# A run's config.xml as text with its workingdirectory blanked: two runs of one config compare equal.
function ConfigSansDir($m) { (ReadText (Join-Path $m.cwd 'config.xml')) -replace 'workingdirectory="[^"]*"', 'workingdirectory=""' }
function FileCount([string]$dir) { @(Get-ChildItem -LiteralPath $dir -Recurse -File).Count }
function Summary($o) {
    $m = $o.Json
    if ($null -eq $m) { return "exit $($o.Exit), no manifest; stderr: $($o.Err.Trim() -split "`n" | Select-Object -Last 1)" }
    $solver = if ($null -eq $m.outcome) { 'not started' } else { "solver $([math]::Round($m.outcome.elapsed_ms)) ms" }
    "exit $($o.Exit), $($m.verdict.status) [$((Codes $m) -join ', ')], stage $($m.stage), $solver, simpa $([math]::Round($o.Sec, 1)) s, files $($m.files.present)/$($m.files.expected)"
}
# The size of a run's folder in MB (0 when the run gave no manifest).
function RunSize($o) {
    if ($null -eq $o.Json) { return 0 }
    [math]::Round((@(Get-ChildItem -LiteralPath (RunDir $o.Json) -Recurse -File) | Measure-Object Length -Sum).Sum / 1MB, 1)
}
function Labels([string]$project) { @(([IO.File]::ReadAllText($project) | ConvertFrom-Json).point_receivers | ForEach-Object { $_.name }) }

$seeded = Join-Path $fx 'rooms\tutorial1_box_seeded.simpa'
$lossGate = Join-Path $fx 'rooms\elmia_loss_gate.simpa'
$labels = Labels $seeded
# The seeded box with its source moved 5 cm in x and 10 cm in y, off every internal facet of its
# 6-tetrahedron mesh; and the same with 1,000,000 particles, a run that cannot end by itself
# within the cancel checks' bounds.
$moveSource = @('"position": [3.0, 5.0, 1.8]', '"position": [3.05, 5.1, 1.8]')
$controlBox = Edited $seeded (Join-Path $work 'box_source_moved.simpa') @(, $moveSource)
$longBox = Edited $seeded (Join-Path $work 'box_long_run.simpa') @($moveSource, @('"particles_per_source": 10000,', '"particles_per_source": 1000000,'))

# --- (a) the seeded box, SPPS ----------------------------------------------------------------------
# Every way a run misses gate (a), in words; none when it meets every clause. Losses are judged
# against $ref, a run of the same project on upstream's own mesh of the room (Burhan, 2026-09-24
# 05:13, "No worse than upstream"): per band, lost by meshing and lost by loops no higher than its.
function UnmetA($o, $m, [int]$bands, $ref) {
    $why = @()
    if ($o.Exit -ne 0) { $why += "exit $($o.Exit)" }
    if ($m.verdict.status -ne 'OK') { $why += "status $($m.verdict.status) [$((Codes $m) -join ', ')]" }
    if ($m.lines.fail -ne 0) { $why += "$($m.lines.fail) FAIL lines" }
    if ($m.lines.warn -ne 0) { $why += "$($m.lines.warn) WARN lines" }
    $dir = RunDir $m
    $out = ReadText (Join-Path $dir 'solver.stdout.txt'); $err = ReadText (Join-Path $dir 'solver.stderr.txt')
    $xmlLines = ([regex]::Matches($out + "`n" + $err, '(?m)^Xml Property')).Count
    if ($xmlLines) { $why += "$xmlLines 'Xml Property' lines" }
    $lossLines = ([regex]::Matches($err, 'particles has been in error')).Count
    if ($lossLines) { $why += "$lossLines stderr 'particles has been in error' lines" }
    $cfg = Config $m
    $nb = [int64]$cfg.configuration.simulation.nbparticules
    $sources = @($cfg.SelectNodes('//source')).Count
    $bs = @($m.particles.bands | Where-Object { $null -ne $_ })
    if ($bs.Count -ne $bands) { $why += "$($bs.Count) bands in the statistics, expected $bands" }
    $refBands = @{}
    foreach ($b in @($ref.particles.bands | Where-Object { $null -ne $_ })) { $refBands[[string]$b.freq_hz] = $b }
    if ($refBands.Count -ne $bands) { $why += "$($refBands.Count) bands in the reference run, expected $bands" }
    foreach ($b in $bs) {
        $r = $refBands[[string]$b.freq_hz]
        if ($null -eq $r) { $why += "$($b.freq_hz) Hz is not in the reference run" }
        elseif ($b.lost_by_meshing_problems -gt $r.lost_by_meshing_problems -or $b.lost_by_infinite_loops -gt $r.lost_by_infinite_loops) {
            $why += "$($b.freq_hz) Hz lost $($b.lost_by_meshing_problems) by meshing and $($b.lost_by_infinite_loops) by loops, the reference $($r.lost_by_meshing_problems) and $($r.lost_by_infinite_loops)"
        }
        if ($b.total -ne $nb * $sources) { $why += "$($b.freq_hz) Hz total $($b.total), expected $nb x $sources" }
    }
    if ($m.files.expected -eq 0 -or $m.files.present -ne $m.files.expected) { $why += "files $($m.files.present)/$($m.files.expected)" }
    $sim = $cfg.configuration.simulation
    $csbin = Join-Path $m.cwd ($sim.recepteurss_directory + 'Global\' + $sim.recepteurss_filename)
    if (-not (Test-Path -LiteralPath $csbin) -or (Get-Item -LiteralPath $csbin).Length -eq 0) { $why += "no surface receiver file $csbin" }
    , $why
}
# The bands of a run with a loss, as 'Hz: meshing/loops'.
function Lossy($m) { (@($m.particles.bands | Where-Object { $null -ne $_ -and ($_.lost_by_meshing_problems + $_.lost_by_infinite_loops) -gt 0 }) | ForEach-Object { "$($_.freq_hz) Hz: $($_.lost_by_meshing_problems)/$($_.lost_by_infinite_loops)" }) -join ', ' }
$aRuns = Join-Path $work 'box-spps-runs'
$script:refA = $null; $script:boxA = $null
Check "(a) reference: upstream's own mesh of the box (its 2019 TetGen output, --from-tetgen) is the 2019 tetramesh.mbin byte for byte, and SPPS on it with the seeded box's config exits 0, OK" {
    $ref = Join-Path $work 'box-upstream-2019'
    $r = Simpa @('mesh', $seeded, '--from-tetgen', (Join-Path $fx 'upstream\tutorial1\tetgen'), '--out', $ref, '--json') 'mesh-box-upstream'
    if ($r.Exit -ne 0) { throw "mesh --from-tetgen exited $($r.Exit)" }
    $mine = Get-FileHash -Algorithm SHA256 (Join-Path $ref 'tetramesh.mbin'); $theirs = Get-FileHash -Algorithm SHA256 (Join-Path $fx 'upstream\tutorial1\spps\tetramesh.mbin')
    $o = Simpa @('run', $seeded, '--solver', 'spps', '--mesh', $ref, '--runs', $aRuns, '--json') 'run-box-spps-upstream-mesh'
    $script:refA = $o
    Write-Host "      $($r.Json.counts.build.tetrahedra) tetrahedra, sha256 $($mine.Hash.Substring(0, 16)) (2019: $($theirs.Hash.Substring(0, 16))); $(Summary $o); bands with a loss (meshing/loops): [$(Lossy $o.Json)]"
    $mine.Hash -eq $theirs.Hash -and $o.Exit -eq 0 -and $null -ne $o.Json -and $o.Json.verdict.status -eq 'OK'
}
Check "(a) seeded box, SPPS: exit 0, OK; 0 FAIL/WARN/'Xml Property' lines; no particle-loss line; per band lost no more than on upstream's own mesh (27 bands); totals nbparticules x sources; 65/65 files with the Global .csbin" {
    if ($null -eq $script:refA -or $null -eq $script:refA.Json) { throw 'the reference run gave no manifest' }
    $o = Simpa @('run', $seeded, '--solver', 'spps', '--runs', $aRuns, '--json') 'run-box-spps-seeded'
    $script:boxA = $o
    $m = $o.Json
    Write-Host "      $(Summary $o)"
    if ($null -eq $m) { return $false }
    $tets = ((ReadText (Join-Path (RunDir $m) 'mesh\mesh.json')) | ConvertFrom-Json).counts.build.tetrahedra
    Write-Host "      the run's mesh: $tets tetrahedra; bands with a loss (meshing/loops): ours [$(Lossy $m)], upstream's own mesh [$(Lossy $script:refA.Json)]"
    $why = UnmetA $o $m 27 $script:refA.Json
    $why | ForEach-Object { Write-Host "      unmet: $_" }
    $why.Count -eq 0
}
$script:controlA = $null; $script:controlSix = $null
Check "(a) control: the seeded box on TetGen 1.6.0's 6-tetrahedron mesh in upstream's corner order meets every (a) clause against upstream's own mesh" {
    if ($null -eq $script:refA -or $null -eq $script:refA.Json) { throw 'the reference run gave no manifest' }
    # The run (g) and (h) reuse: the box with its source 5 cm off, on its own mesh.
    $script:controlA = Simpa @('run', $controlBox, '--solver', 'spps', '--runs', $aRuns, '--json') 'run-box-spps-control'
    Write-Host "      moved source, its own mesh (for (g) and (h)): $(Summary $script:controlA)"
    # The predicate's control: tests/fixtures/solver-outputs/tutorial1's 6 tetrahedra, built by
    # --from-tetgen in upstream's corner order, on which SPPS locates the source and loses nothing.
    $six = Join-Path $work 'box-six-tetrahedra'
    $r = Simpa @('mesh', $seeded, '--from-tetgen', (Join-Path $fx 'solver-outputs\tutorial1'), '--basename', 'tetgen_scene_mesh', '--out', $six, '--json') 'mesh-box-six'
    if ($r.Exit -ne 0) { throw "mesh --from-tetgen exited $($r.Exit)" }
    $o = Simpa @('run', $seeded, '--solver', 'spps', '--mesh', $six, '--runs', $aRuns, '--json') 'run-box-spps-six'
    Write-Host "      $(Summary $o)"
    $script:controlSix = $o
    if ($null -eq $o.Json) { return $false }
    $why = UnmetA $o $o.Json 27 $script:refA.Json
    $why | ForEach-Object { Write-Host "      unmet: $_" }
    $b = @($o.Json.particles.bands)
    Write-Host "      $($r.Json.counts.build.tetrahedra) tetrahedra; $($b.Count) bands, totals $(($b | ForEach-Object { $_.total } | Sort-Object -Unique) -join ', '), lost by meshing $(($b | Measure-Object lost_by_meshing_problems -Sum).Sum), by loops $(($b | Measure-Object lost_by_infinite_loops -Sum).Sum)"
    $why.Count -eq 0 -and $script:controlA.Exit -eq 0
}
Check "(a) says NO: the seeded box's own run judged against the 6-tetrahedron mesh, which loses none, misses (a) at 2 kHz only" {
    if ($null -eq $script:boxA.Json -or $null -eq $script:controlSix.Json) { throw 'a run gave no manifest' }
    $why = UnmetA $script:boxA $script:boxA.Json 27 $script:controlSix.Json
    Write-Host "      unmet: $($why -join '; ')"
    $why.Count -eq 1 -and $why[0] -like '2000 Hz lost 1 by meshing and 0 by loops, the reference 0 and 0'
}
Check "(a) says NO: the control's manifest with one particle more lost by meshing at 50 Hz misses (a) against the control itself" {
    $c = $script:controlSix.Json | ConvertTo-Json -Depth 32 | ConvertFrom-Json
    $c.particles.bands[0].lost_by_meshing_problems = $c.particles.bands[0].lost_by_meshing_problems + 1
    $why = UnmetA $script:controlSix $c 27 $script:controlSix.Json
    Write-Host "      unmet: $($why -join '; ')"
    $why.Count -eq 1 -and $why[0] -like '50 Hz lost 1 by meshing*'
}

# --- (b) the seeded box, TCR -----------------------------------------------------------------------
$bRuns = Join-Path $work 'box-tcr-runs'
# The TCR files gate (b) names that are missing or empty.
function TcrMissing($m, [string[]]$names) {
    $miss = @()
    foreach ($f in @('Main results.gabe') + @($names | ForEach-Object { "Punctual receivers\$_.gabe" })) {
        $p = Join-Path $m.cwd $f
        if (-not (Test-Path -LiteralPath $p) -or (Get-Item -LiteralPath $p).Length -eq 0) { $miss += $f }
    }
    , $miss
}
$script:tcrA = $null
Check "(b) seeded box, TCR: exit 0, OK, 'Main results.gabe' and 'Punctual receivers\<lbl>.gabe' for $($labels -join ', ')" {
    $o = Simpa @('run', $seeded, '--solver', 'tcr', '--runs', $bRuns, '--json') 'run-box-tcr'
    $script:tcrA = $o
    Write-Host "      $(Summary $o)"
    $miss = TcrMissing $o.Json $labels
    Write-Host "      missing: [$($miss -join ', ')]"
    $o.Exit -eq 0 -and $o.Json.verdict.status -eq 'OK' -and $miss.Count -eq 0 -and $o.Json.files.present -eq $o.Json.files.expected
}
Check "(b) says NO: a receiver the run does not have ('Receiver 3') is reported missing" {
    $miss = TcrMissing $script:tcrA.Json (@($labels) + 'Receiver 3')
    Write-Host "      missing: [$($miss -join ', ')]"
    $miss.Count -eq 1 -and $miss[0] -eq 'Punctual receivers\Receiver 3.gabe'
}

# --- (d) negative run folders ---------------------------------------------------------------------
# Every way a verdict misses a case's expected.json (tests/fixtures/runs/README.md); none when it
# meets it. The same rules as crates/simpa/tests/run_folder_fixtures.rs.
function Unmet($expected, $m, [int]$code) {
    $out = @()
    $status = [string]$m.verdict.status; $want = [string]$expected.status
    if ($status -ne $want) { $out += "status $status, expected $want" }
    $wantExit = if ($want -eq 'OK') { 0 } else { 5 }
    if ($code -ne $wantExit) { $out += "exit $code, expected $wantExit" }
    $got = @(Codes $m)
    foreach ($c in @($expected.codes)) { if ($got -notcontains $c) { $out += "code $c missing from [$($got -join ', ')]" } }
    foreach ($g in @($expected.codes_any_of)) {
        if (@(@($g) | Where-Object { $got -contains $_ }).Count -eq 0) { $out += "none of [$(@($g) -join ', ')] in [$($got -join ', ')]" }
    }
    $warned = @(); foreach ($w in @($m.verdict.warnings)) { if ($warned -notcontains $w.code) { $warned += $w.code } }
    $wantW = @($expected.warnings)
    if (($warned -join ',') -ne ($wantW -join ',')) { $out += "warnings [$($warned -join ', ')], expected [$($wantW -join ', ')]" }
    , $out
}
$dRuns = Join-Path $work 'run-folder-runs'
$cases = @(Get-ChildItem (Join-Path $fx 'runs') -Directory | Sort-Object Name)
$script:caseRuns = @{}; $script:caseResults = @()
foreach ($case in $cases) {
    $exp = [IO.File]::ReadAllText((Join-Path $case.FullName 'expected.json')) | ConvertFrom-Json
    $argv = @('run-folder', $case.FullName, '--solver', $exp.solver, '--runs', $dRuns, '--json')
    if ($case.Name -like 'stub_*') { $argv += @('--solver-exe', $stub) }
    $problems = @(); $o = $null
    try {
        $o = Simpa $argv "run-folder-$($case.Name)"
        if ($null -eq $o.Json) { $problems = @("no manifest: $(Summary $o)") } else { $problems = Unmet $exp $o.Json $o.Exit; $script:caseRuns[$case.Name] = RunDir $o.Json }
    } catch { $problems = @($_.Exception.Message) }
    $status = if ($o -and $o.Json) { $o.Json.verdict.status } else { '?' }
    $codes = if ($o -and $o.Json) { (Codes $o.Json) -join ', ' } else { '' }
    Write-Host ('      {0,-34} {1,-9} exit {2,3} {3,6:N0} ms  {4}  {5}' -f $case.Name, $status, $(if ($o) { $o.Exit } else { '?' }), $(if ($o) { $o.Sec * 1000 } else { 0 }), $codes, $(if ($problems.Count) { 'UNMET: ' + ($problems -join '; ') } else { 'ok' }))
    $script:caseResults += [pscustomobject]@{ Name = $case.Name; Positive = ($exp.status -eq 'OK'); Status = $status; Problems = $problems; Run = $o }
}
$named = @('spps_noeps', 'spps_oneband', 'spps_dirmiss', 'spps_dirempty', 'spps_mat0miss', 'spps_mat7miss', 'spps_srcout', 'spps_lossy', 'tcr_broken_hall', 'spps_unreadable_mesh', 'tcr_nomesh')
# The names that are not among the run cases expected to fail (a positive case counts as missing).
function MissingNamed([string[]]$names) {
    $neg = @($script:caseResults | Where-Object { -not $_.Positive } | ForEach-Object { $_.Name })
    @($names | Where-Object { $neg -notcontains $_ })   # callers collect it with @()
}
Check "(d) the 11 cases the gate names are fixtures, each expected FAIL or CRASH" {
    $missing = @(MissingNamed $named)
    Write-Host "      named: $($named -join ', '); missing or not negative: [$($missing -join ', ')]"
    $missing.Count -eq 0
}
Check "(d) negative run folders: exit 5, FAIL or CRASH, the expected reason codes" {
    $neg = @($script:caseResults | Where-Object { -not $_.Positive })
    $met = @($neg | Where-Object { $_.Problems.Count -eq 0 -and @('FAIL', 'CRASH') -contains $_.Status })
    Write-Host "      $($met.Count)/$($neg.Count) negative run folders matched expected.json"
    $neg.Count -gt 0 -and $met.Count -eq $neg.Count
}
Check "(d) positive run folders (expected OK): exit 0, OK" {
    $pos = @($script:caseResults | Where-Object { $_.Positive })
    $met = @($pos | Where-Object { $_.Problems.Count -eq 0 })
    Write-Host "      $($met.Count)/$($pos.Count) positive run folders matched expected.json ($(($pos | ForEach-Object { $_.Name }) -join ', '))"
    $pos.Count -gt 0 -and $met.Count -eq $pos.Count
}
Check "(d) says NO: the expectation check reports each way a verdict can miss expected.json, and the named-case lookup reports a name that is no negative fixture" {
    $m = '{"verdict": {"status": "FAIL", "reasons": [{"code": "mesh_invalid"}, {"code": "degenerate_tets"}], "warnings": [{"code": "unclassified_line"}, {"code": "unclassified_line"}]}}' | ConvertFrom-Json
    $base = '{"status": "FAIL", "codes": ["mesh_invalid"], "codes_any_of": [["degenerate_tets", "inverted_tets"]], "warnings": ["unclassified_line"]}'
    $ok = (Unmet ($base | ConvertFrom-Json) $m 5).Count -eq 0
    $cases = @(
        @('"status": "FAIL"', '"status": "CRASH"', 5, 'status FAIL, expected CRASH'),
        @('"status": "FAIL"', '"status": "FAIL"', 0, 'exit 0, expected 5'),
        @('"codes": ["mesh_invalid"]', '"codes": ["exit_nonzero"]', 5, 'code exit_nonzero missing'),
        @('[["degenerate_tets", "inverted_tets"]]', '[["inverted_tets"]]', 5, 'none of'),
        @('"warnings": ["unclassified_line"]', '"warnings": []', 5, 'warnings'))
    $caught = 0
    foreach ($c in $cases) {
        $got = Unmet ($base.Replace($c[0], $c[1]) | ConvertFrom-Json) $m $c[2]
        if (@($got | Where-Object { $_ -like "*$($c[3])*" }).Count -eq 1) { $caught++ } else { Write-Host "      not caught: $($c[3]) (got: $($got -join '; '))" }
    }
    $absent = @(MissingNamed (@($named) + 'spps_no_such_case' + 'spps_ok'))
    $namedOk = ($absent -join ',') -eq 'spps_no_such_case,spps_ok'
    Write-Host "      the met case: $(if ($ok) { 'no complaint' } else { 'COMPLAINED' }); $caught of $($cases.Count) misses caught; named-case lookup with a name that is no fixture and the positive spps_ok added reports [$($absent -join ', ')]"
    $ok -and $caught -eq $cases.Count -and $namedOk
}

# --- (e) classifier coverage ------------------------------------------------------------------------
# The classification table of docs/solver-contract.md Part B: id, stream, anchored pattern (with
# the Markdown escape \| undone), class.
function Read-LineRules {
    $doc = [IO.File]::ReadAllLines((Join-Path $repo 'docs\solver-contract.md'), [Text.Encoding]::UTF8)
    $i = 0; while ($i -lt $doc.Count -and -not $doc[$i].StartsWith('| Pattern id / reason |')) { $i++ }
    if ($i -ge $doc.Count) { throw 'docs/solver-contract.md has no classification table' }
    $rules = @()
    for ($j = $i + 2; $j -lt $doc.Count -and $doc[$j].StartsWith('|'); $j++) {
        $cols = [regex]::Split($doc[$j], '(?<!\\)\|')
        $pm = [regex]::Match($cols[3], '`(\^[^`]*)`')
        $rules += [pscustomobject]@{
            Id = [regex]::Match($cols[1], '`([a-z_]+)`').Groups[1].Value; Stream = $cols[2].Trim()
            Pattern = $(if ($pm.Success) { New-Object regex -ArgumentList ($pm.Groups[1].Value.Replace('\|', '|')) } else { $null }); Class = ($cols[4].Trim() -split '[\s:]')[0] }
    }
    $rules
}
# The rows one log's lines hit, in order, with the contract's continuation lines: the line after
# scene_mesh_unreadable (a path) and the line before tetra_mesh_empty (a file name) belong to
# their event. A line no pattern matches is unclassified_line.
function Classify-Log([string]$text, $rules) {
    $lines = $text -split "`n"
    $last = $lines.Count; if ($last -gt 0 -and $lines[$last - 1] -eq '') { $last-- }
    $hits = New-Object 'System.Collections.Generic.List[string]'; $pending = $false; $skipNext = $false
    for ($k = 0; $k -lt $last; $k++) {
        $l = $lines[$k].TrimEnd([char]13)
        if ($skipNext) { $skipNext = $false; continue }
        $id = $null
        foreach ($r in $rules) { if ($null -ne $r.Pattern -and $r.Pattern.IsMatch($l)) { $id = $r.Id; break } }
        if ($null -eq $id) { if ($pending) { $hits.Add('unclassified_line') }; $pending = $true; continue }
        if ($pending -and $id -ne 'tetra_mesh_empty') { $hits.Add('unclassified_line') }
        $pending = $false
        $hits.Add($id)
        if ($id -eq 'scene_mesh_unreadable') { $skipNext = $true }
    }
    if ($pending) { $hits.Add('unclassified_line') }
    , $hits
}
# Row id -> the cases whose solver output hit it, over the (d) runs (all but $without).
function Coverage($rules, [string]$without) {
    $hit = @{}
    foreach ($name in $script:caseRuns.Keys) {
        if ($name -eq $without) { continue }
        foreach ($log in 'solver.stdout.txt', 'solver.stderr.txt') {
            foreach ($id in (Classify-Log (ReadText (Join-Path $script:caseRuns[$name] $log)) $rules)) {
                if (-not $hit.ContainsKey($id)) { $hit[$id] = @() }
                if ($hit[$id] -notcontains $name) { $hit[$id] += $name }
            }
        }
    }
    $hit
}
$script:rules = @()
Check "(e) every row of docs/solver-contract.md's classification table is hit by some fixture's solver output through run-folder" {
    $script:rules = @(Read-LineRules)
    $hit = Coverage $script:rules ''
    $missed = @($script:rules | Where-Object { -not $hit.ContainsKey($_.Id) } | ForEach-Object { $_.Id })
    Write-Host "      $($script:rules.Count - $missed.Count)/$($script:rules.Count) rows hit ($(@($script:rules | Where-Object { $null -eq $_.Pattern }).Count) without a pattern); unclassified_line by [$(@($hit['unclassified_line']) -join ', ')]; missed [$($missed -join ', ')]"
    $script:rules.Count -ge 22 -and $missed.Count -eq 0
}
Check "(e) says NO: without stub_config_path_missing's output, config_path_missing is unhit" {
    $hit = Coverage $script:rules 'stub_config_path_missing'
    Write-Host "      config_path_missing hit by [$(@($hit['config_path_missing']) -join ', ')]"
    -not $hit.ContainsKey('config_path_missing')
}
# Whether a log ends in a line with no newline that starts with $prefix.
function EndsUnterminated([byte[]]$bytes, [string]$prefix) {
    if ($bytes.Length -eq 0 -or $bytes[-1] -eq 10) { return $false }
    $text = [Text.Encoding]::UTF8.GetString($bytes)
    ($text -split "`n")[-1].StartsWith($prefix)
}
$unterminated = 'Warning 4000 particles has been in error'
Check "(e) stub_particle_loss_unterminated: its final stderr line, with no newline, is captured as the log's last line and gives particle_loss_reported" {
    $r = @($script:caseResults | Where-Object { $_.Name -eq 'stub_particle_loss_unterminated' })
    if ($r.Count -ne 1 -or $null -eq $r[0].Run.Json) { throw 'the stub case did not run' }
    $bytes = [IO.File]::ReadAllBytes((Join-Path $script:caseRuns['stub_particle_loss_unterminated'] 'solver.stderr.txt'))
    $ok = EndsUnterminated $bytes $unterminated
    $codes = @(Codes $r[0].Run.Json)
    Write-Host "      solver.stderr.txt $($bytes.Length) bytes, last byte $($bytes[-1]), ends unterminated in '$unterminated...': $ok; codes [$($codes -join ', ')]"
    $ok -and $codes -contains 'particle_loss_reported'
}
Check "(e) says NO: the same log with a newline added is not unterminated" {
    $bytes = [IO.File]::ReadAllBytes((Join-Path $script:caseRuns['stub_particle_loss_unterminated'] 'solver.stderr.txt'))
    -not (EndsUnterminated ([byte[]]($bytes + [byte[]]@(10))) $unterminated)
}

# --- (c) the corrected hall against upstream's own mesh -----------------------------------------------
# decision 9, PROPOSED (open decision 7): a 4-sigma Poisson bound on the difference of two counts.
function LossBound([double]$floor) { $floor + 4 * [math]::Sqrt([math]::Max($floor, 1)) }
Check "(c) says NO: the PROPOSED bound refuses ours = floor + 4 sqrt(floor) + 1 (floor 144: bound 192, ours 193), and passes ours = bound" {
    (193 -gt (LossBound 144)) -and (192 -le (LossBound 144)) -and (4 -le (LossBound 0)) -and -not (5 -le (LossBound 0))
}
$upstreamRoot = if ($env:SIMPA_UPSTREAM) { $env:SIMPA_UPSTREAM } else { Join-Path $repo 'target\solvers\src-929a5c8' }
$floorTetgen = Join-Path $work 'floor-tetgen'; $floorMesh = Join-Path $work 'floor-mesh'; $cRuns = Join-Path $work 'hall-runs'
Check "(c) free space on B: at least 1 GB before the hall runs (measured: one run's folder 80 MB, peak drop 90 MB)" {
    $free = (Get-PSDrive B).Free
    Write-Host "      B: free $([math]::Round($free / 1GB, 2)) GB"
    $free -ge 1GB
}
Check "(c) floor mesh: upstream's tutorial_2 TetGen set extracted (its markers index the hall's faces) and built with --from-tetgen: exit 0, OK" {
    $proj = Join-Path $upstreamRoot 'src\isimpa\resources\doc\tutorial\tutorial 2\tutorial_2.proj'
    if (-not (Test-Path -LiteralPath $proj)) { throw "no upstream tree at ${upstreamRoot}: $proj is missing. Extract it with powershell -File solvers/build.ps1, or set SIMPA_UPSTREAM to a checkout at 929a5c8" }
    $py = Get-Command python -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    if (-not $py) { throw 'python is not on PATH: gate M6(c) needs it to extract the floor mesh' }
    $x = & $py.Source (Join-Path $repo 'tools\fixture-gen\extract_tutorial2_mesh.py') $upstreamRoot $floorTetgen | ForEach-Object { "$_" }
    $pyExit = $LASTEXITCODE
    $x | ForEach-Object { Write-Host "      | $_" }
    if ($pyExit -ne 0) { return $false }
    $r = Simpa @('mesh', $lossGate, '--from-tetgen', $floorTetgen, '--out', $floorMesh, '--json') 'mesh-floor'
    $b = $r.Json.counts.build
    Write-Host "      exit $($r.Exit), $($r.Json.status) [$(@($r.Json.codes) -join ', ')], source $($r.Json.source): $($b.tetrahedra) tetrahedra, $($b.face_rows) .face rows"
    $r.Exit -eq 0 -and $r.Json.status -eq 'OK' -and $r.Json.source -eq 'external'
}
$script:ours = $null; $script:floor = $null
Check "(c) our run of elmia_loss_gate (its own mesh): exit 0, OK, 6 bands 125-4,000 Hz, totals nbparticules x sources, 0 stderr particle-loss lines" {
    $free0 = (Get-PSDrive B).Free
    $o = Simpa @('run', $lossGate, '--solver', 'spps', '--runs', $cRuns, '--json') 'run-hall-ours' 2700
    $script:ours = $o
    Write-Host "      $(Summary $o); $(RunSize $o) MB in its run folder; B: free fell $([math]::Round(($free0 - (Get-PSDrive B).Free) / 1MB)) MB over the run (the whole drive, other writers included)"
    if ($null -eq $o.Json) { return $false }
    $m = $o.Json; $cfg = Config $m
    $nb = [int64]$cfg.configuration.simulation.nbparticules; $sources = @($cfg.SelectNodes('//source')).Count
    $freqs = (@($m.particles.bands) | ForEach-Object { $_.freq_hz }) -join ','
    $totals = @($m.particles.bands | Where-Object { $_.total -ne $nb * $sources }).Count
    $loss = ([regex]::Matches((ReadText (Join-Path (RunDir $m) 'solver.stderr.txt')), 'particles has been in error')).Count
    Write-Host "      seed $($cfg.configuration.simulation.random_seed), $nb particles x $sources sources, bands [$freqs], $totals bands with another total, $loss particle-loss lines; mesh $((ReadText (Join-Path (RunDir $m) 'mesh\mesh.json') | ConvertFrom-Json).counts.build.tetrahedra) tetrahedra"
    $o.Exit -eq 0 -and $m.verdict.status -eq 'OK' -and $freqs -eq '125,250,500,1000,2000,4000' -and $totals -eq 0 -and $loss -eq 0
}
Check "(c) floor run on upstream's mesh with the identical config (config.xml equal but for workingdirectory): statistics in the same 6 bands" {
    if ($null -eq $script:ours -or $null -eq $script:ours.Json) { throw 'our run gave no manifest' }
    $free0 = (Get-PSDrive B).Free
    $o = Simpa @('run', $lossGate, '--solver', 'spps', '--mesh', $floorMesh, '--runs', $cRuns, '--json') 'run-hall-floor' 2700
    $script:floor = $o
    Write-Host "      $(Summary $o); $(RunSize $o) MB in its run folder; B: free fell $([math]::Round(($free0 - (Get-PSDrive B).Free) / 1MB)) MB over the run (the whole drive, other writers included)"
    if ($null -eq $o.Json) { return $false }
    $same = (ConfigSansDir $script:ours.Json) -ceq (ConfigSansDir $o.Json)
    $freqs = (@($o.Json.particles.bands) | ForEach-Object { $_.freq_hz }) -join ','
    Write-Host "      config.xml identical but for workingdirectory: $same; floor bands [$freqs]"
    $same -and $freqs -eq '125,250,500,1000,2000,4000'
}
Check "(c) says NO: the config comparison refuses the floor's config.xml with random_seed 1 changed to 2" {
    if ($null -eq $script:ours.Json -or $null -eq $script:floor.Json) { throw 'a hall run gave no manifest' }
    $floorCfg = ConfigSansDir $script:floor.Json
    $n = ([regex]::Matches($floorCfg, 'random_seed="1"')).Count
    if ($n -ne 1) { throw "random_seed=`"1`" appears $n times in the floor's config.xml, not once" }
    -not ((ConfigSansDir $script:ours.Json) -ceq $floorCfg.Replace('random_seed="1"', 'random_seed="2"'))
}
Check "(c) per band, our lost_by_meshing <= floor + 4 sqrt(max(floor, 1)) (tolerance PROPOSED: open decision 7, docs/m5-m6-design.md decision 9)" {
    if ($null -eq $script:ours.Json -or $null -eq $script:floor.Json) { throw 'a hall run gave no manifest' }
    $ob = @($script:ours.Json.particles.bands | Where-Object { $null -ne $_ }); $fb = @($script:floor.Json.particles.bands | Where-Object { $null -ne $_ })
    if ($ob.Count -ne 6 -or $fb.Count -ne 6) { throw "bands: ours $($ob.Count), floor $($fb.Count), expected 6 each" }
    Write-Host ('      {0,7} {1,9} {2,9} {3,7} {4,9} {5,11} {6,11}' -f 'band Hz', 'ours', 'floor', 'ratio', 'bound', 'ours loops', 'floor loops')
    $bad = 0; $atOrBelow = 0
    for ($i = 0; $i -lt 6; $i++) {
        $o = [double]$ob[$i].lost_by_meshing_problems; $f = [double]$fb[$i].lost_by_meshing_problems; $bound = LossBound $f
        if ($ob[$i].freq_hz -ne $fb[$i].freq_hz) { throw "band $i is $($ob[$i].freq_hz) Hz in ours, $($fb[$i].freq_hz) Hz in the floor" }
        $ratio = if ($f -gt 0) { '{0:N2}' -f ($o / $f) } else { 'n/a' }
        if ($o -gt $bound) { $bad++ }
        if ($o -le $f) { $atOrBelow++ }
        Write-Host ('      {0,7} {1,9} {2,9} {3,7} {4,9:N1} {5,11} {6,11}  {7}' -f $ob[$i].freq_hz, $o, $f, $ratio, $bound, $ob[$i].lost_by_infinite_loops, $fb[$i].lost_by_infinite_loops, $(if ($o -gt $bound) { 'OVER' } else { 'ok' }))
    }
    Write-Host "      per $($ob[0].total) particles per band; wall: ours $([math]::Round($script:ours.Sec, 1)) s (solver $([math]::Round($script:ours.Json.outcome.elapsed_ms / 1000, 1)) s), floor $([math]::Round($script:floor.Sec, 1)) s (solver $([math]::Round($script:floor.Json.outcome.elapsed_ms / 1000, 1)) s); floor verdict $($script:floor.Json.verdict.status) [$((Codes $script:floor.Json) -join ', ')]"
    Write-Host "      for open decision 7 (information, not a check): the stricter rule ours <= floor holds in $atOrBelow of 6 bands"
    $bad -eq 0
}

# --- (f) cancel ----------------------------------------------------------------------------------------
Check "(f) says NO: the tasklist check sees a running process (a private copy of the stub solver), and not once it is stopped" {
    $probeDir = Join-Path $work 'image-probe'; New-Item -ItemType Directory -Force $probeDir | Out-Null
    $image = "m6-image-probe-$PID.exe"
    Copy-Item $stub (Join-Path $probeDir $image)
    [IO.File]::WriteAllText((Join-Path $probeDir 'stub.json'), '{"exit_code": 0, "lines": [{"stream": "stdout", "text": "x", "newline": true, "delay_ms": 30000}]}')
    $p = Start-Process -FilePath (Join-Path $probeDir $image) -ArgumentList 'config.xml' -WorkingDirectory $probeDir -WindowStyle Hidden -PassThru
    Start-Sleep -Milliseconds 500
    $seen = ImageRunning $image
    Stop-Process -Id $p.Id -Force; $null = $p.WaitForExit(10000)
    $gone = -not (ImageRunning $image)
    Remove-Item (Join-Path $probeDir $image)
    Write-Host "      running: listed $seen; stopped: gone $gone"
    $seen -and $gone
}
$fRuns = Join-Path $work 'cancel-runs'
Check "(f) control: the long box (1,000,000 particles) left alone runs past the 5 s bound and writes every file, so an unkilled solver fails the cancel check" {
    $o = Simpa @('run', $longBox, '--solver', 'spps', '--runs', $fRuns, '--json') 'run-long-alone'
    Write-Host "      $(Summary $o)"
    $o.Exit -eq 0 -and $o.Json.verdict.status -eq 'OK' -and $o.Json.outcome.elapsed_ms -ge 5000 -and $o.Json.files.present -eq $o.Json.files.expected
}
$script:cancelEnd = $null
Check "(f) --cancel-after-progress 1: exit 130, CANCELLED [cancelled], exit_code null, SPPS stopped within 5 s with its files incomplete" {
    if (ImageRunning 'spps.exe') { throw 'cannot judge: an spps.exe was already running before the cancel' }
    $o = Simpa @('run', $longBox, '--solver', 'spps', '--runs', $fRuns, '--json', '--cancel-after-progress', '1') 'run-long-cancel'
    $script:cancelEnd = Get-Date
    $m = $o.Json
    Write-Host "      $(Summary $o); outcome cancelled $($m.outcome.cancelled), exit_code $(if ($null -eq $m.outcome.exit_code) { 'null' } else { $m.outcome.exit_code })"
    $o.Exit -eq 130 -and $m.verdict.status -eq 'CANCELLED' -and ((Codes $m) -join ',') -eq 'cancelled' -and $m.outcome.cancelled -eq $true -and
        $null -eq $m.outcome.exit_code -and $m.outcome.elapsed_ms -lt 5000 -and $m.files.present -lt $m.files.expected
}
Check "(f) 2 s after the cancel, tasklist /FI `"IMAGENAME eq spps.exe`" lists none" {
    if ($null -eq $script:cancelEnd) { throw 'the cancel did not run' }
    $wait = 2000 - ((Get-Date) - $script:cancelEnd).TotalMilliseconds
    if ($wait -gt 0) { Start-Sleep -Milliseconds ([int]$wait) }
    $running = ImageRunning 'spps.exe'
    Write-Host "      $([math]::Round(((Get-Date) - $script:cancelEnd).TotalSeconds, 1)) s after simpa exited: spps.exe $(if ($running) { 'STILL LISTED' } else { 'not listed' })"
    -not $running
}
Check "(f) Stop-Process on simpa.exe at its first PROGRESS line: SPPS was running, and 2 s later no spps.exe is listed and its process is gone" {
    if (ImageRunning 'spps.exe') { throw 'cannot judge: an spps.exe was already running before the run' }
    $s = Start-Simpa @('run', $longBox, '--solver', 'spps', '--runs', $fRuns) 'run-long-killed'
    $first = $null
    while ($s.Clock.Elapsed.TotalSeconds -lt 300 -and -not $s.Process.HasExited) {
        $m = [regex]::Match((ReadText "$($s.Base).stderr.txt"), '(?m)^PROGRESS  .*$')
        if ($m.Success) { $first = $m.Value.Trim(); break }
        Start-Sleep -Milliseconds 20
    }
    $kids = @(Get-CimInstance Win32_Process -Filter "ParentProcessId=$($s.Process.Id)" | Where-Object { $_.Name -ieq 'spps.exe' })
    $listed = ImageRunning 'spps.exe'
    Stop-Process -Id $s.Process.Id -Force -ErrorAction SilentlyContinue
    $killed = Get-Date
    $null = $s.Process.WaitForExit(10000)
    if ($null -eq $first) { throw "no PROGRESS line within $([math]::Round($s.Clock.Elapsed.TotalSeconds)) s (simpa exited: $($s.Process.HasExited))" }
    $wait = 2000 - ((Get-Date) - $killed).TotalMilliseconds
    if ($wait -gt 0) { Start-Sleep -Milliseconds ([int]$wait) }
    $after = ImageRunning 'spps.exe'
    $alive = @($kids | Where-Object { Get-Process -Id $_.ProcessId -ErrorAction SilentlyContinue })
    Write-Host "      killed simpa at '$first' ($([math]::Round($s.Clock.Elapsed.TotalSeconds, 1)) s in): spps children $(@($kids | ForEach-Object { $_.ProcessId }) -join ', '), listed then $listed; 2 s later listed $after, children alive $($alive.Count)"
    $kids.Count -ge 1 -and $listed -and -not $after -and $alive.Count -eq 0
}

Check "timeouts say NO: the long box given a 2 s limit is killed at the limit, the call throws TIMEOUT with its time, and no spps.exe is left 2 s later" {
    $msg = $null
    try { $null = Simpa @('run', $longBox, '--solver', 'spps', '--runs', $fRuns, '--json') 'run-long-timeout' 2 } catch { $msg = $_.Exception.Message }
    Start-Sleep -Seconds 2
    $running = ImageRunning 'spps.exe'
    Write-Host "      $msg; spps.exe 2 s later: $(if ($running) { 'STILL LISTED' } else { 'not listed' })"
    $null -ne $msg -and $msg.StartsWith('TIMEOUT: run-long-timeout still ran after') -and -not $running
}

# --- (g) a run folder under a non-ASCII path ----------------------------------------------------------
$L = [string][char]0x0141
Check "(g) runs under a path with U+0141 are OK with the same file count as their ASCII twins (TCR box, SPPS box, run-folder spps_ok)" {
    $lRoot = Join-Path $work "${L}odz runs"
    $pairs = @()
    $t = Simpa @('run', $seeded, '--solver', 'tcr', '--runs', $lRoot, '--json') 'run-box-tcr-L'
    $pairs += , @('TCR box', $t, $script:tcrA)
    $s = Simpa @('run', $controlBox, '--solver', 'spps', '--runs', $lRoot, '--json') 'run-box-spps-L'
    $pairs += , @('SPPS box', $s, $script:controlA)
    $lFixture = Join-Path $work "${L}odz fixtures\spps_ok"
    New-Item -ItemType Directory -Force (Split-Path -Parent $lFixture) | Out-Null
    Copy-Item -Recurse (Join-Path $fx 'runs\spps_ok') $lFixture
    $f = Simpa @('run-folder', $lFixture, '--solver', 'spps', '--runs', (Join-Path $work "${L}odz fixtures\runs"), '--json') 'run-folder-spps_ok-L'
    $pairs += , @('run-folder spps_ok', $f, (@($script:caseResults | Where-Object { $_.Name -eq 'spps_ok' })[0].Run))
    $ok = $true
    foreach ($p in $pairs) {
        $u = $p[1].Json; $a = $p[2].Json
        if ($null -eq $u -or $null -eq $a) { Write-Host "      $($p[0]): no manifest"; $ok = $false; continue }
        $nu = FileCount $u.cwd; $na = FileCount $a.cwd
        $under = $u.cwd.Contains($L)
        Write-Host "      $($p[0]): $($u.verdict.status), $nu files in solve\ ($($u.files.present)/$($u.files.expected) expected) under U+0141: $under; ASCII twin $($a.verdict.status), $na files"
        if (-not ($p[1].Exit -eq 0 -and $u.verdict.status -eq 'OK' -and $under -and $nu -eq $na -and $nu -gt 0 -and $u.files.total -eq $a.files.total)) { $ok = $false }
    }
    $ok
}
Check "(g) says NO: a copy of the U+0141 TCR run's solve\ with one file removed no longer has the ASCII run's count" {
    $src = @(Get-ChildItem -LiteralPath (Join-Path $work "${L}odz runs") -Directory | Where-Object { $_.Name -like '*-tcr' })[0].FullName
    $copy = Join-Path $work "${L}odz copy"
    Copy-Item -Recurse -LiteralPath (Join-Path $src 'solve') $copy
    Remove-Item -LiteralPath (Join-Path $copy 'Main results.gabe')
    (FileCount $copy) -ne (FileCount $script:tcrA.Json.cwd)
}

# --- (h) two runs, two folders, no suffixed receivers -------------------------------------------------
# Entries of a folder beyond the expected names, and expected names missing from it.
function EntryDiff([string]$folder, [string[]]$want) {
    $have = @(Get-ChildItem -LiteralPath $folder | ForEach-Object { $_.Name })
    [pscustomobject]@{ Extra = @($have | Where-Object { $want -notcontains $_ }); Missing = @($want | Where-Object { $have -notcontains $_ }) }
}
Check "(h) the same project run twice (TCR and SPPS, one runs root each): two distinct folders, and exactly one receiver entry per label" {
    $ok = $true
    $t2 = Simpa @('run', $seeded, '--solver', 'tcr', '--runs', $bRuns, '--json') 'run-box-tcr-again'
    $s2 = Simpa @('run', $controlBox, '--solver', 'spps', '--runs', $aRuns, '--json') 'run-box-spps-again'
    foreach ($pair in @(@('TCR', $script:tcrA, $t2, 'Punctual receivers', @($labels | ForEach-Object { "$_.gabe" })), @('SPPS', $script:controlA, $s2, $null, $labels))) {
        $a = $pair[1].Json; $b = $pair[2].Json
        if ($null -eq $a -or $null -eq $b) { Write-Host "      $($pair[0]): a run gave no manifest"; $ok = $false; continue }
        $sub = $pair[3]; if ($null -eq $sub) { $sub = (Config $b).configuration.simulation.receiversp_directory }
        $distinct = (RunDir $a) -ne (RunDir $b) -and (Test-Path (Join-Path (RunDir $a) 'run.json')) -and (Test-Path (Join-Path (RunDir $b) 'run.json'))
        foreach ($m in @($a, $b)) {
            $d = EntryDiff (Join-Path $m.cwd $sub) $pair[4]
            Write-Host "      $($pair[0]) $(Split-Path -Leaf (RunDir $m)): $sub extra [$($d.Extra -join ', ')], missing [$($d.Missing -join ', ')]"
            if ($d.Extra.Count -or $d.Missing.Count) { $ok = $false }
        }
        Write-Host "      $($pair[0]): folders $(Split-Path -Leaf (RunDir $a)) and $(Split-Path -Leaf (RunDir $b)), distinct $distinct"
        if (-not $distinct) { $ok = $false }
    }
    $ok
}
Check "(h) says NO: a receiver folder holding a suffixed duplicate ('Receiver 10') is refused" {
    $f = Join-Path $work 'receivers-negative'
    foreach ($n in @($labels) + 'Receiver 10') { New-Item -ItemType Directory -Force (Join-Path $f $n) | Out-Null }
    $d = EntryDiff $f $labels
    Write-Host "      extra [$($d.Extra -join ', ')]"
    $d.Extra.Count -eq 1 -and $d.Extra[0] -eq 'Receiver 10'
}

# --- the tail, and what it must refuse ------------------------------------------------------------
# A cargo command through Cargo with extra environment variables, restored afterwards; the helper's
# echo of a failure is not printed. Its verdict and its whole output.
function CargoQuiet([string]$cmdline, [hashtable]$vars = @{}) {
    $saved = @{}
    foreach ($k in $vars.Keys) { $saved[$k] = [Environment]::GetEnvironmentVariable($k); [Environment]::SetEnvironmentVariable($k, $vars[$k]) }
    try { $ok = Cargo $cmdline 6>$null } finally { foreach ($k in $saved.Keys) { [Environment]::SetEnvironmentVariable($k, $saved[$k]) } }
    [pscustomobject]@{ Ok = $ok; Text = $script:cargoText }
}
# A scratch crate, its own workspace, in its own folder: a fresh folder per variant, because on B:
# (exFAT, whole-second mtimes) a rewrite within the second of a build is not seen by cargo.
function New-TailCrate([string]$name, [string]$lib) {
    $dir = Join-Path $work $name
    New-Item -ItemType Directory -Force (Join-Path $dir 'src') | Out-Null
    [IO.File]::WriteAllText((Join-Path $dir 'Cargo.toml'), "[package]`nname = `"tail_scratch`"`nversion = `"0.0.0`"`nedition = `"2021`"`n`n[workspace]`n")
    [IO.File]::WriteAllText((Join-Path $dir 'src\lib.rs'), $lib)
    Join-Path $dir 'Cargo.toml'
}
Check "the tail says NO: a scratch crate that passes cargo test, clippy -D warnings and fmt --check fails each once a failing test, an unused variable and a one-line fn body are planted" {
    $clean = New-TailCrate 'tail-clean' "pub fn two() -> i32 {`n    2`n}`n`n#[test]`nfn two_is_two() {`n    assert_eq!(two(), 2);`n}`n"
    $planted = New-TailCrate 'tail-planted' "pub fn two() -> i32 { let unused = 1; 2 }`n`n#[test]`nfn two_is_three() {`n    assert_eq!(two(), 3);`n}`n"
    $cmds = [ordered]@{ test = 'cargo test -q --manifest-path "{0}"'; clippy = 'cargo clippy -q --manifest-path "{0}" --all-targets -- -D warnings'; fmt = 'cargo fmt --manifest-path "{0}" --check' }
    $why = @{ test = 'test result: FAILED'; clippy = 'unused variable'; fmt = 'Diff in' }
    $ok = $true; $seen = @()
    foreach ($k in $cmds.Keys) {
        $c = CargoQuiet ($cmds[$k] -f $clean); $p = CargoQuiet ($cmds[$k] -f $planted)
        $hit = $p.Text.Contains($why[$k])
        $seen += "$k clean $(if ($c.Ok) { 'passes' } else { 'FAILS' }), planted $(if ($p.Ok) { 'PASSES' } else { 'fails' }) ('$($why[$k])': $hit)"
        if (-not $c.Ok -or $p.Ok -or -not $hit) { $ok = $false }
    }
    Write-Host "      $($seen -join '; ')"
    $ok
}
Check "tests say NO without their inputs: SIMPA_UPSTREAM or SIMPA_SOLVERS_DIR naming a missing folder makes geometry_import_proj or mesh_poly FAIL with the panic naming it, never pass or skip" {
    $none = Join-Path $work 'no-such-folder'
    $u = CargoQuiet 'cargo test -q -p simpa-core --test geometry_import_proj' @{ SIMPA_UPSTREAM = $none }
    $s = CargoQuiet 'cargo test -q -p simpa-core --test mesh_poly' @{ SIMPA_SOLVERS_DIR = $none }
    $uHit = $u.Text.Contains("$none is not an upstream source tree"); $sHit = $s.Text.Contains("$none\tetgen.exe is missing: this test runs the M1 solver build")
    $result = { param($t) ([regex]::Matches($t, '(?m)^test result: .*$') | ForEach-Object { $_.Value.Trim() }) -join ' | ' }
    Write-Host "      upstream missing: $(if ($u.Ok) { 'PASSED' } else { 'failed' }), its panic seen: $uHit; $(& $result $u.Text)"
    Write-Host "      solvers missing: $(if ($s.Ok) { 'PASSED' } else { 'failed' }), its panic seen: $sHit; $(& $result $s.Text)"
    -not $u.Ok -and $uHit -and -not $s.Ok -and $sHit
}
Check "workspace tests, parallel" { Cargo 'cargo test -q --workspace --exclude app' }
Check "clippy -D warnings (core and CLI)" { Cargo 'cargo clippy -q -p simpa-core -p simpa --all-targets -- -D warnings' }
Check "cargo fmt --check (core and CLI)" { Cargo 'cargo fmt -p simpa-core -p simpa --check' }

Write-Host "`nwork: $work"
$passed = $script:checks - $failures.Count - $blocked.Count
if ($failures.Count) { Write-Host "M6 FAILED: $($failures.Count) of $script:checks checks failed, $($blocked.Count) BLOCKED, $passed passed"; exit 1 }
if ($blocked.Count) { Write-Host "M6 PASSED EXCEPT $($blocked.Count) BLOCKED: $passed of $script:checks checks passed; blocked: $($blocked -join '; ')"; exit 3 }
Write-Host "M6 PASSED: $script:checks of $script:checks checks"; exit 0
