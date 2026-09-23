# M4 gate: geometry import, check and repair; the two reference rooms regenerated.
# Drives the CLI, whose JSON shapes are fixed in crates/simpa/src/main.rs:
#   simpa check <model|project> [--units m] [--up z] --json   -> exit 0 ok, 3 refused
#   simpa import-proj <file.proj> <out.simpa> --json          -> summary of the imported project
#   simpa repair <in> <out.simpa> --json                      -> {"changes":[...]}
# Run: powershell -File tools/gates/m4.ps1
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Set-Location $repo
$env:RUSTUP_HOME = "$env:USERPROFILE\.rustup"; $env:CARGO_HOME = "$env:USERPROFILE\.cargo"
$env:Path = "$env:CARGO_HOME\bin;$env:Path"; $env:CARGO_INCREMENTAL = '0'
Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue
# No test reads this today. Tests that need upstream's tree panic whenever it is missing
# (crates/simpa-core/tests/common/paths.rs); gates M5 and M6 prove that on every run.
$env:SIMPA_REQUIRE_UPSTREAM = '1'
$failures = @(); $script:checks = 0
function Check($name, [scriptblock]$body) {
    $script:checks++
    try {
        $ok = & $body
        if ($ok) { Write-Host "PASS  $name" } else { Write-Host "FAIL  $name"; $script:failures += $name }
    } catch {
        Write-Host "FAIL  $name :: $($_.Exception.Message)"; $script:failures += $name
    }
}
function Near($a, $b, $tol) { [math]::Abs([double]$a - [double]$b) -le $tol }
# Positions come from float32 project data: compare to 1e-5 m, never as text.
function NearPoint($p, $q) { (Near $p[0] $q[0] 1e-5) -and (Near $p[1] $q[1] 1e-5) -and (Near $p[2] $q[2] 1e-5) }

$build = cmd /c "cargo build -q --release -p simpa 2>&1"
if ($LASTEXITCODE -ne 0) { $build | Select-Object -Last 20 | ForEach-Object { Write-Host $_ }; throw 'CLI build failed: refusing to test a stale simpa.exe' }
# Runs a cargo command; on failure prints its last lines so a FAIL is never silent.
function Cargo([string]$cmdline) {
    $out = cmd /c "$cmdline 2>&1"
    if ($LASTEXITCODE -ne 0) { $out | Select-Object -Last 15 | ForEach-Object { Write-Host "      | $_" }; return $false }
    return $true
}
$simpa = Join-Path $repo 'target\release\simpa.exe'
$tut = 'B:\repos\I-Simpa-upstream\src\isimpa\resources\doc\tutorial'
$work = Join-Path $repo ('target\gates\m4\' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
New-Item -ItemType Directory -Force $work | Out-Null
function SimpaJson([string[]]$argv) {
    $out = & $simpa @argv
    $script:lastExit = $LASTEXITCODE
    return ($out -join "`n") | ConvertFrom-Json
}

# (a) the raw hall is refused, for the right reasons
Check "(a) raw elmia.ply refused: exit 3, open_edges 955, nonmanifold_edges 9" {
    $r = SimpaJson @('check', "$tut\tutorial 2\elmia.ply", '--units', 'm', '--up', 'z', '--json')
    Write-Host "      exit $script:lastExit, open $($r.open_edges), nonmanifold $($r.nonmanifold_edges), verdict $($r.verdict)"
    $script:lastExit -eq 3 -and $r.open_edges -eq 955 -and $r.nonmanifold_edges -eq 9
}

# (b) the corrected hall from upstream's own project
$hall = Join-Path $work 'elmia_corrected.simpa'
Check "(b) import-proj tutorial_2: 3,926 vertices, 7,860 faces, 10 groups, area 4,001.8 +/- 0.1 m2" {
    $s = SimpaJson @('import-proj', "$tut\tutorial 2\tutorial_2.proj", $hall, '--json')
    Write-Host "      $($s.vertices) vertices, $($s.faces) faces, $(@($s.groups).Count) groups, area $($s.area_m2) m2"
    $script:lastExit -eq 0 -and $s.vertices -eq 3926 -and $s.faces -eq 7860 -and @($s.groups).Count -eq 10 -and (Near $s.area_m2 4001.8 0.1)
}
# Everything but the free-text name and description must equal the committed fixture.
function Normalized($path) { $o = Get-Content $path -Raw | ConvertFrom-Json; $o.PSObject.Properties.Remove('name'); $o.PSObject.Properties.Remove('description'); $o | ConvertTo-Json -Depth 64 -Compress }
Check "(b)(c) import-proj reproduces the committed room fixtures (all but name and description)" {
    $boxOut = Join-Path $work 'tutorial1_box_again.simpa'
    & $simpa import-proj "$tut\tutorial 1\tutorial_1.proj" $boxOut | Out-Null
    $same = @()
    foreach ($pair in @(@($hall, 'elmia_corrected.simpa'), @($boxOut, 'tutorial1_box.simpa'))) {
        $a = Normalized $pair[0]; $b = Normalized (Join-Path $repo ('tests\fixtures\rooms\' + $pair[1]))
        Write-Host "      $($pair[1]): $(if ($a -eq $b) { 'identical' } else { 'DIFFERS' })"
        $same += ($a -eq $b)
    }
    -not ($same -contains $false)
}
Check "(b) corrected hall passes check: 11,790 edges all used twice, 0 self-intersections, volume > 0" {
    $r = SimpaJson @('check', $hall, '--json')
    Write-Host "      exit $script:lastExit, edges $($r.edges), manifold $($r.manifold_edges), self-intersections $($r.self_intersections), volume $($r.signed_volume_m3)"
    $script:lastExit -eq 0 -and $r.edges -eq 11790 -and $r.manifold_edges -eq 11790 -and $r.self_intersections -eq 0 -and $r.signed_volume_m3 -gt 0
}

# (c) the tutorial box
$box = Join-Path $work 'tutorial1_box.simpa'
Check "(c) import-proj tutorial_1: 8 vertices, 12 faces, groups, receiver faces, 180 m3, positions" {
    $s = SimpaJson @('import-proj', "$tut\tutorial 1\tutorial_1.proj", $box, '--json')
    $g = @{}; foreach ($x in $s.groups) { $g[$x.name] = (@($x.faces) | Sort-Object) -join ',' }
    $rs = @($s.surface_receivers | Where-Object { $_.name -eq 'Receiver' })
    Write-Host "      $($s.vertices) v, $($s.faces) f, volume $($s.volume_m3), groups $(($g.GetEnumerator() | Sort-Object Name | ForEach-Object { "$($_.Name)={$($_.Value)}" }) -join ' ')"
    $src = @($s.sources); $rcv = @($s.receivers)
    $script:lastExit -eq 0 -and $s.vertices -eq 8 -and $s.faces -eq 12 -and (Near $s.volume_m3 180 0.0005) -and
        $g['Ceiling'] -eq '10,11' -and $g['Floor'] -eq '0,1' -and $g['Walls'] -eq '2,3,4,5,6,7,8,9' -and
        $rs.Count -eq 1 -and ((@($rs[0].faces) | Sort-Object) -join ',') -eq '0,1' -and
        $src.Count -eq 1 -and (NearPoint $src[0] @(3, 5, 1.8)) -and $rcv.Count -eq 2 -and
        @($rcv | Where-Object { NearPoint $_ @(1, 1, 1.8) }).Count -eq 1 -and @($rcv | Where-Object { NearPoint $_ @(3, 7, 1.8) }).Count -eq 1
}

# (d)(e)(f)(g) and the rest of M4's properties live in the geometry test suites
Check "(d)-(g) geometry suites: repair log, interpenetrating boxes, importers, re-import" {
    $ok = $true
    foreach ($t in (Get-ChildItem (Join-Path $repo 'crates\simpa-core\tests') -Filter 'geometry_*.rs' | Where-Object { $_.BaseName -notlike '*support*' })) {
        if (-not (Cargo "cargo test -q -p simpa-core --test $($t.BaseName)")) { Write-Host "      $($t.BaseName) failed"; $ok = $false }
    }
    $ok
}
Check "(d) CLI repair on the injected box logs exactly 3 changes and passes check" {
    $bad = Join-Path $repo 'tests\fixtures\geometry\box_three_faults.simpa'
    if (-not (Test-Path $bad)) { throw "missing $bad" }
    $fixed = Join-Path $work 'box_repaired.simpa'
    $r = SimpaJson @('repair', $bad, $fixed, '--json')
    Write-Host "      $(@($r.changes).Count) changes: $((@($r.changes) | ForEach-Object { $_.kind }) -join ', ')"
    $n = @($r.changes).Count
    & $simpa check $fixed | Out-Null
    $n -eq 3 -and $LASTEXITCODE -eq 0
}
Check "(e) CLI check refuses two interpenetrating boxes and lists face pairs" {
    $f = Join-Path $repo 'tests\fixtures\geometry\two_boxes_interpenetrating.simpa'
    if (-not (Test-Path $f)) { throw "missing $f" }
    $r = SimpaJson @('check', $f, '--json')
    Write-Host "      exit $script:lastExit, $(@($r.intersecting_pairs).Count) intersecting face pairs"
    $script:lastExit -eq 3 -and @($r.intersecting_pairs).Count -gt 0
}

Check "workspace tests, parallel" { Cargo 'cargo test -q --workspace --exclude app' }
Check "clippy -D warnings (core and CLI)" { Cargo 'cargo clippy -q -p simpa-core -p simpa --all-targets -- -D warnings' }
Check "cargo fmt --check (core and CLI)" { Cargo 'cargo fmt -p simpa-core -p simpa --check' }

Write-Host "`nwork: $work"
if ($failures.Count) { Write-Host "M4 FAILED: $($failures.Count) of $script:checks checks"; exit 1 }
Write-Host "M4 PASSED: $script:checks of $script:checks checks"; exit 0
