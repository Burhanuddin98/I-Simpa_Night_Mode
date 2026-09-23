# M5 gate: process layer, meshing and mesh verification.
# Gate: docs/rebuild-plan-raw-2026-09-23.json lines 149-157 (M5), as amended by
# docs/m5-m6-design.md ("Gate amendments", decisions 1-8). Drives the release CLI:
#   simpa mesh <project.simpa | file.poly> --out <dir> [--json] [--cancel-after-ms <n>]  exit 0, 4, 130
#   simpa mesh-verify <dir> [--json] [--room-id <n>] [--fittings <a,b>]                  exit 0, 4
#   simpa run <project> --solver tcr --mesh <dir> --runs <root> --json                    exit 4 when refused
#   simpa dump mbin|cbin|poly <file>
# (a) Box: `simpa mesh rooms/tutorial1_box.simpa` exits 0 with the box's own flags (-pq2 -A -n).
#     mesh-verify reports unmarked_boundary_faces, degenerate_tets, uncovered_scene_faces and
#     index_errors 0, and every tetrahedron is oriented and wound as the .mbin says. The .var is
#     byte-identical to upstream's tutorial-1 .var. More than 2 tet faces carry markers 0/1, each
#     <= 0.1 m2 x (1 + 1e-4): BLOCKED while open decision 3 stands (the pinned TetGen ignores it).
# (b) Corrected hall: no *_skipped.face; every .1.face row has a marker >= 0 and the markers cover
#     all 7,860 scene faces; marker_geometry_mismatches and unmarked_boundary_faces 0; the .poly
#     coordinates parsed back equal the .cbin float32 vertices exactly (max |delta| = 0).
# (c) `simpa mesh <file.poly>` takes a raw .poly: the survey's tg_bad exits 4 with
#     tetgen_skipped_facets, markers [8, 9, 12] mapped to scene faces [8, 9, 12], and no .mbin.
#     The committed broken hall gives mesh-verify exit 4 with tetgen_skipped_facets (535) and
#     neigh_missing.
# (d) (d1) Mesh the box, then move a vertex: `run --mesh <dir>` exits 4 with mesh_out_of_date and
#     no solver starts. (d2) A re-mesh cancelled 1 ms into TetGen leaves no tetramesh.mbin, and
#     `run --mesh <dir>` exits 4 with mesh_missing.
# (e) A fitting zone in the box (rooms/tutorial1_box_fitting.simpa): room tetrahedra carry
#     idVolume 0, only tetrahedra inside the zone carry its solver id 2 (decision 1), and the id-2
#     volume equals the zone's 1 m3 to 1e-9 relative.
# (f) The oracle dump of every .mbin this gate wrote equals the Rust dump
#     (tools/oracle/diff.ps1 -Corpus <work> -Generated 0: mismatches 0).
# (g) `simpa mesh rooms/elmia_corrected.simpa --cancel-after-ms 50` exits 130 with TetGen killed
#     while it ran (tetgen.cancelled true, tetgen.exit_code null, no .1.ele written), and 2 s
#     later `tasklist /FI "IMAGENAME eq tetgen.exe"` lists none.
# Every check that can pass has a "says NO" check that must refuse an input:
# - a clause's own refusal sits beside it, named "says NO";
# - the checks that only claim a mesh was built ((a) box, (b) hall, (c) raw-.poly control,
#   (e) fitting) share one "meshed" predicate, which must refuse tg_bad's result;
# - mesh-verify's clean verdicts in (a), (b) and (e) are refused by (a)'s read with room id 1
#   and by (c)'s broken hall;
# - the tail (tests, clippy, fmt) must pass a clean scratch crate and refuse a copy with one
#   planted fault each, and the tests that need upstream's tree or the solvers must FAIL, with
#   their panic, when those are missing.
# A check that an open decision blocks prints BLOCKED; the gate then exits 3, never 0.
# Every TetGen or solver call has a hard 5-minute timeout: a timeout is a FAIL with its time.
# Run: powershell -File tools/gates/m5.ps1
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
$work = Join-Path $repo ('target\gates\m5\' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
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
# Runs a program with a hard time limit; stdout and stderr go to numbered files under calls\. A
# program still running at the limit is killed (for simpa, its Job Object then ends TetGen or the
# solver) and the call throws, so the check FAILs with the elapsed time.
$script:callNo = 0
function Invoke-Timed([string]$exe, [string[]]$argv, [int]$limitSec, [string]$label) {
    $script:callNo++
    $base = Join-Path $calls ('{0:D3}-{1}' -f $script:callNo, $label)
    $clock = [Diagnostics.Stopwatch]::StartNew()
    $p = Start-Process -FilePath $exe -ArgumentList (($argv | ForEach-Object { QuoteArg $_ }) -join ' ') `
        -RedirectStandardOutput "$base.stdout.txt" -RedirectStandardError "$base.stderr.txt" -NoNewWindow -PassThru
    $null = $p.Handle   # keeps ExitCode readable once the process has ended
    if (-not $p.WaitForExit($limitSec * 1000)) {
        Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
        $null = $p.WaitForExit(10000)
        throw ('TIMEOUT: {0} still ran after {1:N1} s (limit {2} s) and was killed; logs {3}.*' -f $label, $clock.Elapsed.TotalSeconds, $limitSec, $base)
    }
    $p.WaitForExit()
    [pscustomobject]@{ Exit = $p.ExitCode; Out = (ReadText "$base.stdout.txt"); Err = (ReadText "$base.stderr.txt"); Sec = $clock.Elapsed.TotalSeconds; Base = $base }
}
# simpa with a 5-minute limit; .Json holds stdout parsed when it is a JSON object.
function Simpa([string[]]$argv, [string]$label) {
    $r = Invoke-Timed $simpa $argv 300 $label
    $json = $null
    if ($r.Out.TrimStart().StartsWith('{')) { $json = $r.Out | ConvertFrom-Json }
    $r | Add-Member -NotePropertyName Json -NotePropertyValue $json -PassThru
}
# Whether tasklist lists a process of this image name.
function ImageRunning([string]$image) {
    $out = (& tasklist.exe /FI "IMAGENAME eq $image" /NH) -join "`n"
    return $out.ToLower().Contains($image.ToLower())
}
function F32([string]$h) { [double][BitConverter]::ToSingle([BitConverter]::GetBytes([Convert]::ToUInt32($h, 16)), 0) }
function F64([string]$h) { [BitConverter]::Int64BitsToDouble([Convert]::ToInt64($h, 16)) }
function Sub($a, $b) { [double[]]@(($a[0] - $b[0]), ($a[1] - $b[1]), ($a[2] - $b[2])) }
function Cross($a, $b) { [double[]]@(($a[1] * $b[2] - $a[2] * $b[1]), ($a[2] * $b[0] - $a[0] * $b[2]), ($a[0] * $b[1] - $a[1] * $b[0])) }
function Dot($a, $b) { $a[0] * $b[0] + $a[1] * $b[1] + $a[2] * $b[2] }
# A .mbin through `simpa dump mbin`: nodes as f64 from their f32 bits; per tetrahedron its
# corners v, idVolume id and four faces, each (a, b, c, marker, neighbour).
function Read-Mbin([string]$file) {
    $r = Invoke-Timed $simpa @('dump', 'mbin', $file) 300 'dump-mbin'
    if ($r.Exit -ne 0) { throw "simpa dump mbin $file exited $($r.Exit): $($r.Out)" }
    $nodes = New-Object 'System.Collections.Generic.List[double[]]'
    $tets = New-Object 'System.Collections.Generic.List[object]'
    foreach ($line in $r.Out -split "`n") {
        $t = $line.Trim() -split '\s+'
        if ($t[0] -eq 'node') { $nodes.Add([double[]]@((F32 $t[1]), (F32 $t[2]), (F32 $t[3]))) }
        elseif ($t[0] -eq 'tetra') {
            $faces = @()
            for ($k = 0; $k -lt 4; $k++) { $o = 6 + 6 * $k; $faces += , ([int[]]@($t[$o + 1], $t[$o + 2], $t[$o + 3], $t[$o + 4], $t[$o + 5])) }
            $tets.Add([pscustomobject]@{ v = [int[]]@($t[1], $t[2], $t[3], $t[4]); id = [int]$t[5]; faces = $faces })
        }
    }
    if ($nodes.Count -eq 0 -or $tets.Count -eq 0) { throw "the dump of $file has no nodes or no tetrahedra" }
    [pscustomobject]@{ Nodes = $nodes; Tets = $tets }
}
# (A-D).((B-D)x(C-D)): below 0 for every tetrahedron of a valid .mbin (docs/formats/mbin.md).
function Orient($m, $t) {
    $d = $m.Nodes[$t.v[3]]
    Dot (Sub ($m.Nodes[$t.v[0]]) $d) (Cross (Sub ($m.Nodes[$t.v[1]]) $d) (Sub ($m.Nodes[$t.v[2]]) $d))
}
function NegativeTets($m) { $n = 0; foreach ($t in $m.Tets) { if ((Orient $m $t) -lt 0) { $n++ } }; $n }
function FaceArea($m, $f) {
    $a = $m.Nodes[$f[0]]
    $c = Cross (Sub ($m.Nodes[$f[1]]) $a) (Sub ($m.Nodes[$f[2]]) $a)
    0.5 * [math]::Sqrt((Dot $c $c))
}
# The markers of a TetGen .1.face, and its declared row count.
function Read-FaceMarkers([string]$path) {
    $declared = -1; $markers = New-Object 'System.Collections.Generic.List[int]'
    foreach ($line in [IO.File]::ReadLines($path)) {
        $s = $line.Trim()
        if ($s -eq '' -or $s.StartsWith('#')) { continue }
        $t = $s -split '\s+'
        if ($declared -lt 0) {
            if ($t.Count -lt 2 -or [int]$t[1] -eq 0) { throw "$path has no marker column" }
            $declared = [int]$t[0]; continue
        }
        $markers.Add([int]$t[4])
    }
    [pscustomobject]@{ Declared = $declared; Markers = $markers }
}
# Every row has a marker >= 0 and below the scene's face count, and together they cover all faces.
function FaceMarkerVerdict($fm, [int]$sceneFaces) {
    $neg = 0; $out = 0; $seen = New-Object 'System.Collections.Generic.HashSet[int]'
    foreach ($k in $fm.Markers) { if ($k -lt 0) { $neg++ } elseif ($k -ge $sceneFaces) { $out++ } else { $null = $seen.Add($k) } }
    [pscustomobject]@{ Rows = $fm.Markers.Count; Declared = $fm.Declared; Negative = $neg; OutOfRange = $out; Covered = $seen.Count
        Ok = ($fm.Markers.Count -eq $fm.Declared -and $neg -eq 0 -and $out -eq 0 -and $seen.Count -eq $sceneFaces) }
}
# The vertices of a .cbin (f32) or .poly (f64) through `simpa dump`, as one flat f64 array.
function Read-DumpVertices([string]$kind, [string]$file) {
    $r = Invoke-Timed $simpa @('dump', $kind, $file) 300 "dump-$kind"
    if ($r.Exit -ne 0) { throw "simpa dump $kind $file exited $($r.Exit)" }
    $lines = $r.Out -split "`n"
    $i = 0; while ($i -lt $lines.Count -and -not $lines[$i].StartsWith('vertices ')) { $i++ }
    if ($i -ge $lines.Count) { throw "no vertices in the $kind dump of $file" }
    $n = [int]($lines[$i].Trim().Split(' ')[1])
    $v = New-Object double[] (3 * $n)
    for ($k = 0; $k -lt $n; $k++) {
        $t = $lines[$i + 1 + $k].Trim() -split '\s+'
        for ($c = 0; $c -lt 3; $c++) {
            if ($kind -eq 'cbin') { $v[3 * $k + $c] = F32 $t[$c] } else { $v[3 * $k + $c] = F64 $t[$c] }
        }
    }
    , $v
}
# Largest |a - b| over two coordinate arrays; +inf when their lengths differ or a value is NaN.
function MaxDelta([double[]]$a, [double[]]$b) {
    if ($a.Length -ne $b.Length -or $a.Length -eq 0) { return [double]::PositiveInfinity }
    $m = 0.0
    for ($i = 0; $i -lt $a.Length; $i++) {
        $d = [math]::Abs($a[$i] - $b[$i])
        if ([double]::IsNaN($d)) { return [double]::PositiveInfinity }
        if ($d -gt $m) { $m = $d }
    }
    $m
}
# A copy of a project file with one exact text edit, which must match exactly once.
function Edited([string]$from, [string]$to, [string]$find, [string]$replace) {
    $text = [IO.File]::ReadAllText($from)
    $n = ([regex]::Matches($text, [regex]::Escape($find))).Count
    if ($n -ne 1) { throw "edit of ${from}: '$find' matches $n times, not once" }
    [IO.File]::WriteAllText($to, $text.Replace($find, $replace), (New-Object Text.UTF8Encoding $false))
    $to
}
function RunDir($m) { Split-Path -Parent $m.cwd }
function Codes($m) { @($m.verdict.reasons | ForEach-Object { $_.code }) }
# What every check that claims "meshes" requires of a `simpa mesh --json` call into $out: exit 0,
# status OK, and a tetramesh.mbin written. tg_bad's result must fail it.
function Meshed($r, [string]$out) { $r.Exit -eq 0 -and $r.Json.status -eq 'OK' -and (Test-Path (Join-Path $out 'tetramesh.mbin')) }

$boxRoom = Join-Path $fx 'rooms\tutorial1_box.simpa'
$hallRoom = Join-Path $fx 'rooms\elmia_corrected.simpa'
$fitRoom = Join-Path $fx 'rooms\tutorial1_box_fitting.simpa'

# --- (a) the tutorial box -------------------------------------------------------------------------
$boxMesh = Join-Path $work 'box-mesh'
Check "(a) box meshes with its own settings: exit 0, OK, TetGen argv -pq2 -A -n scene_mesh.poly, .mbin written" {
    $r = Simpa @('mesh', $boxRoom, '--out', $boxMesh, '--json') 'mesh-box'
    $m = $r.Json
    Write-Host "      exit $($r.Exit), $($m.status), argv '$(@($m.tetgen.argv) -join ' ')', $($m.counts.build.tetrahedra) tetrahedra, TetGen $([math]::Round($m.tetgen.elapsed_ms, 1)) ms, simpa mesh $([math]::Round($r.Sec * 1000)) ms"
    (Meshed $r $boxMesh) -and (@($m.tetgen.argv) -join ' ') -eq '-pq2 -A -n scene_mesh.poly'
}
Check "(a) mesh-verify: unmarked_boundary_faces, degenerate_tets, uncovered_scene_faces, index_errors 0; inverted_tets and misordered_faces 0" {
    $r = Simpa @('mesh-verify', $boxMesh, '--json') 'verify-box'
    $x = $r.Json.mesh
    Write-Host "      exit $($r.Exit), codes [$(@($r.Json.codes) -join ', ')]: $($x.tetrahedra) tets, unmarked $($x.unmarked_boundary_faces), degenerate $($x.degenerate_tets), uncovered $($x.uncovered_scene_faces), index $($x.index_errors), inverted $($x.inverted_tets), misordered $($x.misordered_faces)"
    $r.Exit -eq 0 -and $x.tetrahedra -gt 0 -and $x.unmarked_boundary_faces -eq 0 -and $x.degenerate_tets -eq 0 -and $x.uncovered_scene_faces -eq 0 -and
        $x.index_errors -eq 0 -and $x.inverted_tets -eq 0 -and $x.misordered_faces -eq 0 -and @($r.Json.codes).Count -eq 0
}
Check "(a) says NO: the same folder read with upstream's room id 1 fails (exit 4, unknown_volume_ids)" {
    $r = Simpa @('mesh-verify', $boxMesh, '--json', '--room-id', '1') 'verify-box-room1'
    Write-Host "      exit $($r.Exit), codes [$(@($r.Json.codes) -join ', ')]"
    $r.Exit -eq 4 -and (@($r.Json.codes) -contains 'unknown_volume_ids')
}
$script:boxM = $null
Check "(a) orientation, computed here from the dump: (A-D).((B-D)x(C-D)) < 0 for 100 % of tetrahedra" {
    $script:boxM = Read-Mbin (Join-Path $boxMesh 'tetramesh.mbin')
    $neg = NegativeTets $script:boxM
    Write-Host "      $neg of $($script:boxM.Tets.Count) tetrahedra oriented as the .mbin convention says"
    $neg -eq $script:boxM.Tets.Count
}
Check "(a) says NO: the same tetrahedra with two corners swapped are counted as inverted (0 %)" {
    $flipped = [pscustomobject]@{ Nodes = $script:boxM.Nodes; Tets = @($script:boxM.Tets | ForEach-Object { [pscustomobject]@{ v = [int[]]@($_.v[0], $_.v[2], $_.v[1], $_.v[3]); id = $_.id; faces = $_.faces } }) }
    $neg = NegativeTets $flipped
    Write-Host "      $neg of $($flipped.Tets.Count) oriented after the swap"
    $neg -eq 0
}
$upVar = Join-Path $fx 'upstream\tutorial1\tetgen\scene_mesh.var'
Check "(a) the .var is byte-identical to upstream's tutorial-1 .var" {
    $ours = [IO.File]::ReadAllBytes((Join-Path $boxMesh 'scene_mesh.var')); $theirs = [IO.File]::ReadAllBytes($upVar)
    $same = $ours.Length -eq $theirs.Length -and -not (Compare-Object $ours $theirs -SyncWindow 0)
    Write-Host "      ours $($ours.Length) bytes, upstream's $($theirs.Length) bytes: $(if ($same) { 'identical' } else { 'DIFFER' })"
    $same
}
Check "(a) says NO: the byte comparison refuses upstream's .var with a final CRLF added" {
    $theirs = [IO.File]::ReadAllBytes($upVar); $bent = $theirs + [byte[]]@(13, 10)
    -not ($bent.Length -eq $theirs.Length -and -not (Compare-Object $bent $theirs -SyncWindow 0))
}
Check "(a) receiver faces refined: more than 2 tet faces carry markers 0/1, each <= 0.1 m2 x (1 + 1e-4)" {
    $floor = @(); foreach ($t in $script:boxM.Tets) { foreach ($f in $t.faces) { if ($f[3] -eq 0 -or $f[3] -eq 1) { $floor += , $f } } }
    $areas = @($floor | ForEach-Object { FaceArea $script:boxM $_ })
    $max = ($areas | Measure-Object -Maximum).Maximum
    Write-Host "      $($floor.Count) tet faces carry marker 0 or 1, largest $max m2 (bound $(0.1 * (1 + 1e-4)) m2)"
    if ($floor.Count -gt 2 -and $max -le 0.1 * (1 + 1e-4)) { return $true }
    # Blocked only on decision 3's measured signature: the floor unrefined, 2 faces of 30 m2.
    if ($floor.Count -eq 2 -and @($areas | Where-Object { [math]::Abs($_ - 30) -le 1e-3 }).Count -eq 2) {
        return (Blocked 'open decision 3 (docs/m5-m6-design.md): the pinned TetGen 1.6.0 ignores the .var area bound (check_subface, tetgen.cxx:27347-27388), so the floor receiver stays 2 faces of 30 m2. Ways out, for Burhan and Michael: patch TetGen, or pre-split the receiver faces in the .poly')
    }
    $false
}

# --- (b) the corrected hall -----------------------------------------------------------------------
$hallMesh = Join-Path $work 'hall-mesh'
Check "(b) hall meshes: exit 0, OK, no *_skipped.* file, skipped_rows 0" {
    $r = Simpa @('mesh', $hallRoom, '--out', $hallMesh, '--json') 'mesh-hall'
    $m = $r.Json
    $skipped = @(Get-ChildItem $hallMesh -Filter '*_skipped.*' -File)
    Write-Host "      exit $($r.Exit), $($m.status), argv '$(@($m.tetgen.argv) -join ' ')', $($m.counts.build.tetrahedra) tetrahedra, $($m.counts.build.face_rows) .face rows, TetGen $([math]::Round($m.tetgen.elapsed_ms)) ms, simpa mesh $([math]::Round($r.Sec, 2)) s; $($skipped.Count) skipped files"
    (Meshed $r $hallMesh) -and $skipped.Count -eq 0 -and $m.skipped_rows -eq 0 -and $m.counts.scene_faces -eq 7860
}
Check "(b) every .1.face row has a marker >= 0 and the markers cover all 7,860 scene faces" {
    $v = FaceMarkerVerdict (Read-FaceMarkers (Join-Path $hallMesh 'scene_mesh.1.face')) 7860
    Write-Host "      $($v.Rows) rows ($($v.Declared) declared): $($v.Negative) with a marker < 0, $($v.OutOfRange) at or above 7,860, $($v.Covered) of 7,860 scene faces covered"
    $v.Ok
}
Check "(b) says NO: the broken hall's model.1.face (no markers, 796 rows under a 641 header) fails the marker check" {
    $v = FaceMarkerVerdict (Read-FaceMarkers (Join-Path $fx 'meshes\broken_hall\model.1.face')) 7860
    Write-Host "      $($v.Rows) rows ($($v.Declared) declared), $($v.Negative) with a marker < 0, $($v.Covered) covered"
    -not $v.Ok
}
Check "(b) mesh-verify: 7,860 scene faces, uncovered 0, marker_geometry_mismatches 0, unmarked_boundary_faces 0, no codes" {
    $r = Simpa @('mesh-verify', $hallMesh, '--json') 'verify-hall'
    $x = $r.Json.mesh
    Write-Host "      exit $($r.Exit), codes [$(@($r.Json.codes) -join ', ')]: scene faces $($x.scene_faces), uncovered $($x.uncovered_scene_faces), marker mismatches $($x.marker_geometry_mismatches) (max distance $($x.max_marker_distance_m) m, tolerance $($x.marker_tolerance_m) m), unmarked $($x.unmarked_boundary_faces)"
    $r.Exit -eq 0 -and $x.scene_faces -eq 7860 -and $x.uncovered_scene_faces -eq 0 -and $x.marker_geometry_mismatches -eq 0 -and $x.unmarked_boundary_faces -eq 0 -and @($r.Json.codes).Count -eq 0
}
$script:hallPoly = $null; $script:hallCbin = $null
Check "(b) .poly coordinates parsed back equal the .cbin float32 vertices exactly (max |delta| = 0)" {
    $script:hallPoly = Read-DumpVertices 'poly' (Join-Path $hallMesh 'scene_mesh.poly')
    $script:hallCbin = Read-DumpVertices 'cbin' (Join-Path $hallMesh 'mesh.cbin')
    $d = MaxDelta $script:hallPoly $script:hallCbin
    Write-Host "      $($script:hallPoly.Length / 3) .poly vertices, $($script:hallCbin.Length / 3) .cbin vertices, max |delta| $d"
    $script:hallCbin.Length -eq 3 * 3926 -and $d -eq 0
}
Check "(b) says NO: one .poly coordinate one ulp off gives max |delta| > 0" {
    $bent = [double[]]$script:hallPoly.Clone()
    $bent[5] = [BitConverter]::Int64BitsToDouble([BitConverter]::DoubleToInt64Bits($bent[5]) + 1)
    $d = MaxDelta $bent $script:hallCbin
    Write-Host "      max |delta| $d"
    $d -gt 0
}

# --- (c) broken input -----------------------------------------------------------------------------
$script:tgBad = $null
Check "(c) control: a valid raw .poly (upstream's tutorial-1 scene_mesh.poly) meshes: exit 0, OK, .mbin written" {
    $out = Join-Path $work 'raw-poly-mesh'
    $r = Simpa @('mesh', (Join-Path $fx 'upstream\tutorial1\tetgen\scene_mesh.poly'), '--out', $out, '--json') 'mesh-raw-poly'
    Write-Host "      exit $($r.Exit), $($r.Json.status), argv '$(@($r.Json.tetgen.argv) -join ' ')', $($r.Json.counts.build.tetrahedra) tetrahedra"
    Meshed $r $out
}
Check "(c) tg_bad .poly: exit 4, tetgen_skipped_facets, markers [8, 9, 12] mapped to scene faces [8, 9, 12], no .mbin" {
    $out = Join-Path $work 'tg-bad-mesh'
    $r = Simpa @('mesh', (Join-Path $fx 'meshes\tg_bad\scene_mesh.poly'), '--out', $out, '--json') 'mesh-tg-bad'
    $script:tgBad = [pscustomobject]@{ R = $r; Out = $out }
    $m = $r.Json
    $markers = (@($m.skipped_facets) | ForEach-Object { $_.marker }) -join ','
    $faces = (@($m.skipped_facets) | ForEach-Object { $_.scene_face }) -join ','
    $mbin = Test-Path (Join-Path $out 'tetramesh.mbin')
    Write-Host "      exit $($r.Exit), $($m.status) [$(@($m.codes) -join ', ')], skipped markers [$markers] -> scene faces [$faces], .mbin written: $mbin"
    $r.Exit -eq 4 -and $m.status -eq 'FAIL' -and (@($m.codes) -contains 'tetgen_skipped_facets') -and $markers -eq '8,9,12' -and $faces -eq '8,9,12' -and -not $mbin
}
Check "(c) broken hall: mesh-verify exit 4 with tetgen_skipped_facets (535) and neigh_missing" {
    $r = Simpa @('mesh-verify', (Join-Path $fx 'meshes\broken_hall'), '--json') 'verify-broken-hall'
    $codes = @($r.Json.codes)
    Write-Host "      exit $($r.Exit), codes [$($codes -join ', ')], skipped facets $($r.Json.skipped_facets)"
    $r.Exit -eq 4 -and ($codes -contains 'tetgen_skipped_facets') -and ($codes -contains 'neigh_missing') -and $r.Json.skipped_facets -eq 535
}
Check "(a)(b)(c)(e) says NO: the 'meshed' predicate of the box, hall, raw-.poly and fitting checks refuses tg_bad's result (exit 4, FAIL, no .mbin)" {
    if ($null -eq $script:tgBad) { throw 'the tg_bad check did not run' }
    $meshed = Meshed $script:tgBad.R $script:tgBad.Out
    Write-Host "      tg_bad: exit $($script:tgBad.R.Exit), $($script:tgBad.R.Json.status); meshed: $meshed"
    -not $meshed
}

# --- (d) a stale mesh -----------------------------------------------------------------------------
$dMesh = Join-Path $work 'stale-mesh'
$dRuns = Join-Path $work 'stale-runs'
Check "(d) control: the box with its fresh mesh runs (TCR --mesh <dir>): exit 0, OK, no mesh/ in the run folder" {
    $r = Simpa @('mesh', $boxRoom, '--out', $dMesh, '--json') 'mesh-box-for-d'
    if ($r.Exit -ne 0) { throw "meshing the box exited $($r.Exit)" }
    $o = Simpa @('run', $boxRoom, '--solver', 'tcr', '--mesh', $dMesh, '--runs', $dRuns, '--json') 'run-box-fresh-mesh'
    $m = $o.Json
    Write-Host "      exit $($o.Exit), $($m.verdict.status) [$((Codes $m) -join ', ')], mesh $($m.mesh.manifest), solver $([math]::Round($m.outcome.elapsed_ms)) ms"
    $o.Exit -eq 0 -and $m.verdict.status -eq 'OK' -and -not (Test-Path (Join-Path (RunDir $m) 'mesh')) -and $m.mesh.manifest -eq (Join-Path $dMesh 'mesh.json')
}
Check "(d1) a vertex moved after meshing: run --mesh exits 4 with mesh_out_of_date, stage mesh, and no solver starts" {
    $moved = Edited $boxRoom (Join-Path $work 'box_vertex_moved.simpa') "`"vertices`": [`n      [6.0, 0.0, 0.0]," "`"vertices`": [`n      [6.0, 0.0, 0.25],"
    $o = Simpa @('run', $moved, '--solver', 'tcr', '--mesh', $dMesh, '--runs', $dRuns, '--json') 'run-box-stale'
    $m = $o.Json; $dir = RunDir $m
    $started = (Test-Path (Join-Path $dir 'solver.stdout.txt')) -or (Test-Path (Join-Path $dir 'solve'))
    Write-Host "      exit $($o.Exit), $($m.verdict.status) [$((Codes $m) -join ', ')], stage $($m.stage), outcome $(if ($null -eq $m.outcome) { 'null' } else { 'present' }), solver started: $started"
    $o.Exit -eq 4 -and (@(Codes $m) -join ',') -eq 'mesh_out_of_date' -and $m.stage -eq 'mesh' -and $null -eq $m.outcome -and -not $started
}
Check "(d2) a re-mesh cancelled 1 ms into TetGen: exit 130, CANCELLED, no tetramesh.mbin; run --mesh exits 4 with mesh_missing" {
    $c = Simpa @('mesh', $boxRoom, '--out', $dMesh, '--json', '--cancel-after-ms', '1') 'mesh-box-cancel-1ms'
    $mbin = Test-Path (Join-Path $dMesh 'tetramesh.mbin')
    $o = Simpa @('run', $boxRoom, '--solver', 'tcr', '--mesh', $dMesh, '--runs', $dRuns, '--json') 'run-box-missing'
    $m = $o.Json; $dir = RunDir $m
    $started = (Test-Path (Join-Path $dir 'solver.stdout.txt')) -or (Test-Path (Join-Path $dir 'solve'))
    Write-Host "      re-mesh exit $($c.Exit), $($c.Json.status), tetgen cancelled $($c.Json.tetgen.cancelled); .mbin left: $mbin; run exit $($o.Exit) [$((Codes $m) -join ', ')], solver started: $started"
    $c.Exit -eq 130 -and $c.Json.status -eq 'CANCELLED' -and -not $mbin -and $o.Exit -eq 4 -and (@(Codes $m) -join ',') -eq 'mesh_missing' -and -not $started
}

# --- (e) a fitting zone -----------------------------------------------------------------------------
$fitMesh = Join-Path $work 'fitting-mesh'
$zoneLo = @(1.0, 1.0, 0.5); $zoneHi = @(2.0, 2.0, 1.5)
# Per idVolume: tetrahedra, volume, and how many sit on the wrong side of the zone box: an id-2
# tetrahedron with a corner outside it, or an id-0 one whose centroid is strictly inside it.
function ZoneCensus($m, $lo, $hi) {
    $eps = 1e-6
    $s = @{ n0 = 0; n2 = 0; other = 0; v0 = 0.0; v2 = 0.0; out2 = 0; in0 = 0 }
    foreach ($t in $m.Tets) {
        $vol = [math]::Abs((Orient $m $t)) / 6
        $p = @(); foreach ($i in $t.v) { $p += , $m.Nodes[$i] }
        if ($t.id -eq 2) {
            $s.n2++; $s.v2 += $vol
            foreach ($q in $p) { for ($c = 0; $c -lt 3; $c++) { if ($q[$c] -lt $lo[$c] - $eps -or $q[$c] -gt $hi[$c] + $eps) { $s.out2++; break } } }
        } elseif ($t.id -eq 0) {
            $s.n0++; $s.v0 += $vol
            $inside = $true
            for ($c = 0; $c -lt 3; $c++) { $g = ($p[0][$c] + $p[1][$c] + $p[2][$c] + $p[3][$c]) / 4; if (-not ($g -gt $lo[$c] + $eps -and $g -lt $hi[$c] - $eps)) { $inside = $false } }
            if ($inside) { $s.in0++ }
        } else { $s.other++ }
    }
    $s
}
$script:fitM = $null
Check "(e) the box with one fitting zone meshes: exit 0, OK, volume_ids room 0 and fittings [2]; mesh-verify --fittings 2 passes" {
    $r = Simpa @('mesh', $fitRoom, '--out', $fitMesh, '--json') 'mesh-fitting'
    $v = Simpa @('mesh-verify', $fitMesh, '--json', '--fittings', '2') 'verify-fitting'
    Write-Host "      exit $($r.Exit), $($r.Json.status), room $($r.Json.volume_ids.room), fittings [$(@($r.Json.volume_ids.fittings) -join ', ')], $($r.Json.counts.build.tetrahedra) tetrahedra; verify exit $($v.Exit) [$(@($v.Json.codes) -join ', ')], volume by id $($v.Json.mesh.volume_by_id | ConvertTo-Json -Compress)"
    (Meshed $r $fitMesh) -and $r.Json.volume_ids.room -eq 0 -and (@($r.Json.volume_ids.fittings) -join ',') -eq '2' -and $v.Exit -eq 0 -and @($v.Json.codes).Count -eq 0
}
Check "(e) room tetrahedra are 0, only tetrahedra inside the zone (1,1,0.5)-(2,2,1.5) are 2, id-2 volume = 1 m3 to 1e-9 relative" {
    $script:fitM = Read-Mbin (Join-Path $fitMesh 'tetramesh.mbin')
    $s = ZoneCensus $script:fitM $zoneLo $zoneHi
    $rel = [math]::Abs($s.v2 - 1.0) / 1.0
    Write-Host "      id 2: $($s.n2) tets, $($s.v2) m3 (relative error $rel), $($s.out2) outside the zone; id 0: $($s.n0) tets, $($s.v0) m3, $($s.in0) inside the zone; other ids: $($s.other)"
    $s.n2 -gt 0 -and $s.out2 -eq 0 -and $s.in0 -eq 0 -and $s.other -eq 0 -and $rel -le 1e-9 -and [math]::Abs($s.v0 + $s.v2 - 180) / 180 -le 1e-9
}
Check "(e) says NO: the same census against the zone moved 0.5 m in x finds id-2 tetrahedra outside it" {
    $s = ZoneCensus $script:fitM @(1.5, 1.0, 0.5) @(2.5, 2.0, 1.5)
    Write-Host "      $($s.out2) id-2 tets outside the moved zone, $($s.in0) id-0 tets inside it"
    $s.out2 -gt 0
}

# --- (g) cancel --------------------------------------------------------------------------------------
Check "(g) says NO: the tasklist check sees a running process (a private copy of the stub solver), and not once it is stopped" {
    $probeDir = Join-Path $work 'image-probe'; New-Item -ItemType Directory -Force $probeDir | Out-Null
    $image = "m5-image-probe-$PID.exe"
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
$hallCancel = Join-Path $work 'hall-cancel'
$script:cancelEnd = $null
Check "(g) cancel 50 ms into TetGen on the hall: exit 130, CANCELLED, tetgen.cancelled true, exit_code null, no .1.ele, no .mbin" {
    if (ImageRunning 'tetgen.exe') { throw 'cannot judge: a tetgen.exe was already running before the cancel' }
    $r = Simpa @('mesh', $hallRoom, '--out', $hallCancel, '--json', '--cancel-after-ms', '50') 'mesh-hall-cancel'
    $script:cancelEnd = Get-Date
    $m = $r.Json
    $ele = Test-Path (Join-Path $hallCancel 'scene_mesh.1.ele'); $mbin = Test-Path (Join-Path $hallCancel 'tetramesh.mbin')
    Write-Host "      exit $($r.Exit), $($m.status) [$(@($m.codes) -join ', ')], tetgen cancelled $($m.tetgen.cancelled), exit_code $(if ($null -eq $m.tetgen.exit_code) { 'null' } else { $m.tetgen.exit_code }), TetGen ran $([math]::Round($m.tetgen.elapsed_ms, 1)) ms, simpa mesh $([math]::Round($r.Sec * 1000)) ms; .1.ele $ele, .mbin $mbin"
    $r.Exit -eq 130 -and $m.status -eq 'CANCELLED' -and $m.tetgen.cancelled -eq $true -and $null -eq $m.tetgen.exit_code -and -not $ele -and -not $mbin
}
Check "(g) 2 s after the cancel, tasklist /FI `"IMAGENAME eq tetgen.exe`" lists none" {
    if ($null -eq $script:cancelEnd) { throw 'the cancel did not run' }
    $wait = 2000 - ((Get-Date) - $script:cancelEnd).TotalMilliseconds
    if ($wait -gt 0) { Start-Sleep -Milliseconds ([int]$wait) }
    $running = ImageRunning 'tetgen.exe'
    Write-Host "      $([math]::Round(((Get-Date) - $script:cancelEnd).TotalSeconds, 1)) s after simpa exited: tetgen.exe $(if ($running) { 'STILL LISTED' } else { 'not listed' })"
    -not $running
}

# --- (f) oracle parity of every .mbin written above ------------------------------------------------
$diffPs1 = Join-Path $repo 'tools\oracle\diff.ps1'
function OracleDiff([string]$corpus, [string]$label) {
    $r = Invoke-Timed 'powershell.exe' @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $diffPs1, '-Corpus', $corpus, '-Generated', '0') 1800 $label
    $text = $r.Out + "`n" + $r.Err
    $mm = [regex]::Match($text, '(?m)^mismatches: (\d+)'); $fm = [regex]::Match($text, '(?m)^fixtures: \d+ checked \(.*\bmbin (\d+)\b')
    [pscustomobject]@{ Exit = $r.Exit; Text = $text; Sec = $r.Sec
        Mismatches = $(if ($mm.Success) { [int]$mm.Groups[1].Value } else { -1 })
        Mbins = $(if ($fm.Success) { [int]$fm.Groups[1].Value } else { -1 }) }
}
Check "(f) oracle parity: the dump of every .mbin this gate wrote is upstream's reader's dump (diff.ps1 -Corpus <work> -Generated 0: mismatches 0)" {
    $ours = @(Get-ChildItem $work -Recurse -Filter '*.mbin' -File)
    $fixtureMbins = @(Get-ChildItem $fx -Recurse -Filter '*.mbin' -File)
    $d = OracleDiff $work 'oracle-diff'
    $d.Text -split "`n" | Where-Object { $_ -match '^(MISMATCH|fixtures:|checked:|mismatches:|refused)' } | ForEach-Object { Write-Host "      | $_" }
    Write-Host "      $($ours.Count) .mbin written by this gate, $($fixtureMbins.Count) committed; diff.ps1 compared $($d.Mbins) .mbin, exit $($d.Exit), $([math]::Round($d.Sec)) s"
    $ours.Count -ge 1 -and $d.Mbins -eq ($ours.Count + $fixtureMbins.Count) -and $d.Mismatches -eq 0 -and $d.Exit -eq 0
}
Check "(f) says NO: a corpus holding one truncated .mbin gives mismatches: 1, naming it" {
    $neg = Join-Path $work 'oracle-negative'; New-Item -ItemType Directory -Force $neg | Out-Null
    $bytes = [IO.File]::ReadAllBytes((Join-Path $boxMesh 'tetramesh.mbin'))
    [IO.File]::WriteAllBytes((Join-Path $neg 'truncated.mbin'), [byte[]]$bytes[0..99])
    $d = OracleDiff $neg 'oracle-diff-negative'
    $named = @($d.Text -split "`n" | Where-Object { $_ -match '^MISMATCH mbin .*truncated\.mbin' }).Count
    Write-Host "      exit $($d.Exit), mismatches $($d.Mismatches), MISMATCH lines naming it: $named"
    $d.Exit -ne 0 -and $d.Mismatches -eq 1 -and $named -eq 1
}

# After (f), so that the killed mesh folder is not in the oracle corpus.
Check "timeouts say NO: a hall mesh given a 0 s limit is killed, the call throws TIMEOUT with its time, and no tetgen.exe is left 2 s later" {
    $msg = $null
    try { $null = Invoke-Timed $simpa @('mesh', $hallRoom, '--out', (Join-Path $work 'hall-timeout')) 0 'mesh-hall-timeout' } catch { $msg = $_.Exception.Message }
    Start-Sleep -Seconds 2
    $running = ImageRunning 'tetgen.exe'
    Write-Host "      $msg; tetgen.exe 2 s later: $(if ($running) { 'STILL LISTED' } else { 'not listed' })"
    $null -ne $msg -and $msg.StartsWith('TIMEOUT: mesh-hall-timeout still ran after') -and -not $running
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
if ($failures.Count) { Write-Host "M5 FAILED: $($failures.Count) of $script:checks checks failed, $($blocked.Count) BLOCKED, $passed passed"; exit 1 }
if ($blocked.Count) { Write-Host "M5 PASSED EXCEPT $($blocked.Count) BLOCKED: $passed of $script:checks checks passed; blocked: $($blocked -join '; ')"; exit 3 }
Write-Host "M5 PASSED: $script:checks of $script:checks checks"; exit 0
