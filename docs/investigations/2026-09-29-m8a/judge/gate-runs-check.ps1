# Judge (gate lens): every run folder of the M8a bed checked against SPEC.md sections 2.1-2.4,
# typed here from the spec, not from the code. Read-only on the bed.
param([string]$Bed = 'C:\tmp\nm-m8a-bed\20260929T093134Z', [string]$Solvers = 'C:\tmp\nm-m8a-solvers')
$ErrorActionPreference = 'Stop'
. 'B:\repos\I-Simpa_Night_Mode\.claude\worktrees\m8a\solvers\pe-fingerprint.ps1'
$manifest = Get-Content -Raw 'B:\repos\I-Simpa_Night_Mode\.claude\worktrees\m8a\solvers\manifest.json' | ConvertFrom-Json
$raw = @{}; $code = @{}
foreach ($n in 'spps.exe', 'classicalTheory.exe', 'tetgen.exe', 'preprocess.exe') {
    $p = Join-Path $Solvers $n
    $raw[$n] = Get-RawSha256 $p; $code[$n] = Get-CodeSha256 $p
    "{0,-20} code {1} manifest {2} match {3}; raw {4}" -f $n, $code[$n], $manifest.code_sha256.$n, ($code[$n] -eq $manifest.code_sha256.$n), $raw[$n]
}
$rooms = @{
    '6x10x3' = @{ gated = $true; src = @(3.0, 5.0, 1.8); rec = @(@(1.0, 1.0, 1.8), @(3.0, 7.0, 1.8), @(5.0, 8.5, 1.2)) }
    '5x4x3'  = @{ gated = $true; src = @(2.52, 1.97, 1.53); rec = @(@(1.0, 1.0, 1.0), @(4.0, 3.0, 2.0), @(1.0, 3.0, 1.9)) }
    '20x8x4' = @{ gated = $false; src = @(4.02, 3.97, 1.53); rec = @(@(2.0, 2.0, 1.2), @(10.0, 6.0, 1.8), @(18.0, 3.0, 2.5)) }
}
# room, alpha, random N, random dur, energetic dur, energetic eps  (SPEC 2.3 table)
$rows = @(
    @('6x10x3', 0.05, 8400000, 4, 4, 7), @('6x10x3', 0.1, 18000000, 3, 2, 7), @('6x10x3', 0.2, 33000000, 2, 1, 9), @('6x10x3', 0.4, 110000000, 1.5, 0.5, 9),
    @('5x4x3', 0.05, 3500000, 3, 3, 7), @('5x4x3', 0.1, 7900000, 2, 2, 7), @('5x4x3', 0.2, 14000000, 1.5, 0.8, 9), @('5x4x3', 0.4, 31000000, 1, 0.4, 9),
    @('20x8x4', 0.05, 1500000, 5.7, 5.7, 7), @('20x8x4', 0.1, 1500000, 4.3, 2.9, 7), @('20x8x4', 0.2, 1500000, 2.9, 1.5, 9), @('20x8x4', 0.4, 1500000, 2.2, 0.8, 9)
)
$off = '125,250,500,1000,2000,4000'; $on = '125,250,500,1000,2000,4000,8000'
$expect = @()
foreach ($r in $rows) {
    $room = $rooms[$r[0]]
    $airs = if ($room.gated) { @($false, $true) } else { @($false) }
    foreach ($air in $airs) {
        foreach ($m in 'random', 'energetic') {
            $id = '{0}-a{1}-{2}-air-{3}' -f $r[0], $r[1], $m, $(if ($air) { 'on' } else { 'off' })
            foreach ($s in 1..10) {
                $expect += [pscustomobject]@{ id = $id; seed = $s; room = $r[0]; alpha = $r[1]; method = $m; air = $air; gated = $room.gated
                    n = $(if ($m -eq 'random') { $r[2] } else { 1500000 }); dur = $(if ($m -eq 'random') { $r[3] } else { $r[4] })
                    eps = $(if ($m -eq 'random') { 7 } else { $r[5] }); solver = 'spps' }
            }
        }
        $tid = '{0}-a{1}-tcr-air-{2}' -f $r[0], $r[1], $(if ($air) { 'on' } else { 'off' })
        $expect += [pscustomobject]@{ id = $tid; seed = 1; room = $r[0]; alpha = $r[1]; method = 'tcr'; air = $air; gated = $room.gated; n = $null; dur = $null; eps = $null; solver = 'tcr' }
    }
}
$expect += [pscustomobject]@{ id = 'say-no-n5'; seed = 10; room = '5x4x3'; alpha = 0.4; method = 'random'; air = $false; gated = $false; n = 31000; dur = 1; eps = 7; solver = 'spps' }
$expect += [pscustomobject]@{ id = 'say-no-n6'; seed = 1; room = '5x4x3'; alpha = 0.2; method = 'energetic'; air = $false; gated = $false; n = 150000; dur = 0.8; eps = 9; solver = 'spps'; scat = 0 }
"expected matrix runs: {0} ({1} SPPS cell runs, {2} gated; {3} TCR, {4} gated)" -f $expect.Count, @($expect | ? { $_.solver -eq 'spps' -and $_.id -notlike 'say-no*' }).Count, @($expect | ? { $_.solver -eq 'spps' -and $_.gated }).Count, @($expect | ? { $_.solver -eq 'tcr' }).Count, @($expect | ? { $_.solver -eq 'tcr' -and $_.gated }).Count

$problems = New-Object System.Collections.ArrayList
$dtSeen = @{}; $checked = 0; $meshChecked = 0; $cfgChecked = 0
function P($s) { [void]$script:problems.Add($s) }
foreach ($e in $expect) {
    $dir = Join-Path $Bed ("runs\{0}\s{1}" -f $e.id, $e.seed)
    $folders = @(Get-ChildItem -LiteralPath $dir -Directory -ErrorAction SilentlyContinue | ? { Test-Path (Join-Path $_.FullName 'run.json') })
    if ($folders.Count -ne 1) { P "$($e.id) s$($e.seed): $($folders.Count) run folders"; continue }
    $f = $folders[0].FullName
    $rj = Get-Content -Raw (Join-Path $f 'run.json') | ConvertFrom-Json
    $checked++
    if ($rj.solver -ne $e.solver) { P "$($e.id) s$($e.seed): solver $($rj.solver)" }
    $want = if ($e.solver -eq 'tcr') { $raw['classicalTheory.exe'] } else { $raw['spps.exe'] }
    if ($rj.exe.sha256 -ne $want) { P "$($e.id) s$($e.seed): exe sha256 $($rj.exe.sha256)" }
    # The exe the run names, hashed now.
    if ((Get-RawSha256 $rj.exe.path) -ne $want) { P "$($e.id) s$($e.seed): $($rj.exe.path) is not the checked file now" }
    if ($rj.outcome.exit_code -ne 0) { P "$($e.id) s$($e.seed): exit $($rj.outcome.exit_code)" }
    $mj = Join-Path $f 'mesh\mesh.json'
    if (-not (Test-Path $mj)) { P "$($e.id) s$($e.seed): no mesh.json" } else {
        $t = (Get-Content -Raw $mj | ConvertFrom-Json).tetgen.program_sha256
        if (-not $t) { P "$($e.id) s$($e.seed): mesh.json has no tetgen sha256" } elseif ($t -ne $raw['tetgen.exe']) { P "$($e.id) s$($e.seed): tetgen $t" } else { $meshChecked++ }
    }
    [xml]$cx = Get-Content -Raw (Join-Path $f 'solve\config.xml')
    $sim = $cx.configuration.simulation
    $freqs = (@($sim.freq_enum.bfreq | ? { $_.docalc -eq '1' } | % { $_.freq }) -join ',')
    $wantF = if ($e.air) { $on } else { $off }
    if ($freqs -ne $wantF) { P "$($e.id) s$($e.seed): bands $freqs" }
    if ([int]$sim.abs_atmo_calc -ne [int]$e.air) { P "$($e.id) s$($e.seed): abs_atmo_calc $($sim.abs_atmo_calc)" }
    $surf = @($cx.configuration.surface_absorption_enum.type_surface)
    if ($surf.Count -ne 1) { P "$($e.id) s$($e.seed): $($surf.Count) surface types" }
    $scat = if ($null -ne $e.scat) { $e.scat } else { 1 }
    foreach ($b in @($surf[0].bfreq)) {
        if ([double]$b.absorb -ne $e.alpha -or [double]$b.diffusion -ne $scat -or $b.loi -ne '2') { P "$($e.id) s$($e.seed): $($b.freq) Hz absorb $($b.absorb) diffusion $($b.diffusion) loi $($b.loi)" }
    }
    $rec = @($cx.configuration.recepteursp.recepteur_ponctuel)
    $room = $rooms[$e.room]
    if ($rec.Count -ne 3) { P "$($e.id) s$($e.seed): $($rec.Count) receivers" }
    foreach ($i in 0..2) {
        $r = @($rec | ? { $_.lbl -eq ('R{0:D3}' -f $i) })
        if ($r.Count -ne 1 -or [double]$r[0].x -ne $room.rec[$i][0] -or [double]$r[0].y -ne $room.rec[$i][1] -or [double]$r[0].z -ne $room.rec[$i][2]) { P "$($e.id) s$($e.seed): receiver R00$i" }
        elseif ($e.solver -eq 'spps' -and [double]$sim.rayon_recepteurp -ne 0.31) { P "$($e.id) s$($e.seed): radius $($sim.rayon_recepteurp)" }
    }
    $src = @($cx.configuration.sources.source)
    if ($src.Count -ne 1 -or [double]$src[0].x -ne $room.src[0] -or [double]$src[0].y -ne $room.src[1] -or [double]$src[0].z -ne $room.src[2]) { P "$($e.id) s$($e.seed): source" }
    if ($e.solver -eq 'spps') {
        $cfgChecked++
        $key = '{0}/{1}/{2}' -f $e.room, $e.method, $(if ($e.air) { 'on' } else { 'off' })
        if (-not $dtSeen.ContainsKey($key)) { $dtSeen[$key] = @() }
        $dtSeen[$key] += $sim.pasdetemps
        if ([double]$sim.pasdetemps -ne 0.001) { P "$($e.id) s$($e.seed): dt $($sim.pasdetemps)" }
        if ([double]$sim.duree_simulation -ne [double]$e.dur) { P "$($e.id) s$($e.seed): duration $($sim.duree_simulation), want $($e.dur)" }
        if ([int64]$sim.nbparticules -ne [int64]$e.n) { P "$($e.id) s$($e.seed): particles $($sim.nbparticules), want $($e.n)" }
        if ([int]$sim.random_seed -ne $e.seed) { P "$($e.id) s$($e.seed): random_seed $($sim.random_seed)" }
        $cm = if ($e.method -eq 'random') { 0 } else { 1 }
        if ([int]$sim.computation_method -ne $cm) { P "$($e.id) s$($e.seed): computation_method $($sim.computation_method)" }
        if ([double]$sim.trans_epsilon -ne [double]$e.eps) { P "$($e.id) s$($e.seed): trans_epsilon $($sim.trans_epsilon)" }
    }
}
# Nothing in runs/ that the matrix does not name (other than the atmospheric validation).
$names = @($expect | % { $_.id } | Select-Object -Unique) + @('atmospheric-validation', 'atmospheric-validation-tcr')
$extra = @(Get-ChildItem -LiteralPath (Join-Path $Bed 'runs') -Directory | ? { $names -notcontains $_.Name })
foreach ($x in $extra) { P "unexpected folder runs\$($x.Name)" }
foreach ($id in ($expect | ? { $_.solver -eq 'spps' -and $_.id -notlike 'say-no*' } | % { $_.id } | Select-Object -Unique)) {
    $seeds = @(Get-ChildItem -LiteralPath (Join-Path $Bed "runs\$id") -Directory | % { $_.Name })
    if (($seeds | Sort-Object) -join ',' -ne ((1..10 | % { "s$_" } | Sort-Object) -join ',')) { P "$id seeds: $($seeds -join ',')" }
}
$allRunJson = @(Get-ChildItem -LiteralPath (Join-Path $Bed 'runs') -Recurse -Filter 'run.json' -File).Count
"run folders checked against the spec: $checked; run.json files under runs/: $allRunJson; mesh.json TetGen sha256 checked: $meshChecked; SPPS config.xml checked: $cfgChecked"
"dt per (room, method, air):"
$dtSeen.GetEnumerator() | Sort-Object Name | % { "  {0,-28} {1} runs, dt values: {2}" -f $_.Name, $_.Value.Count, (($_.Value | Select-Object -Unique) -join ',') }
"problems: $($problems.Count)"
$problems | Select-Object -First 50
