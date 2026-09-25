# The cancelled-run test's race, measured by delaying the cancel on purpose (the review of piece C:
# the load receipt could not reproduce the original failure, so it could not tell the designs
# apart). One SPPS run at a time. Usage:
#   powershell -File cancel_race.ps1 -Simpa <simpa.exe> -Repo <worktree> -Work <folder>
param(
    [Parameter(Mandatory = $true)][string]$Simpa,
    [Parameter(Mandatory = $true)][string]$Repo,
    [Parameter(Mandatory = $true)][string]$Work
)
$ErrorActionPreference = 'Continue'
New-Item -ItemType Directory -Force $Work | Out-Null
$fixture = Join-Path $Repo 'tests\fixtures\rooms\seats_box.simpa'

function Project([int]$particles) {
    $text = [IO.File]::ReadAllText($fixture)
    if (-not $text.Contains('"particles_per_source": 2000,')) { throw 'the fixture is not the 2,000-particle seats box' }
    $path = Join-Path $Work "seats_$particles.simpa"
    [IO.File]::WriteAllText($path, $text.Replace('"particles_per_source": 2000,', "`"particles_per_source`": $particles,"))
    $path
}

function Run([string]$project, [string[]]$cancel, [string]$label) {
    $runs = Join-Path $Work 'runs'
    $t0 = Get-Date
    $out = & $Simpa run $project --solver spps --runs $runs @cancel --json 2>$null
    $code = $LASTEXITCODE
    $wall = ((Get-Date) - $t0).TotalMilliseconds
    $m = ($out -join "`n") | ConvertFrom-Json
    $solver = if ($m.outcome) { [math]::Round($m.outcome.elapsed_ms) } else { 'none' }
    $line = '{0,-28} exit {1,3}  {2,-9} stage {3,-10} solver {4,7} ms  files {5}/{6}  simpa {7,6:N0} ms' -f `
        $label, $code, $m.verdict.status, $m.stage, $solver, $m.files.present, $m.files.expected, $wall
    Write-Host $line
    $line
}

$lines = @()
$lines += "simpa: $Simpa"
$lines += "started: $(Get-Date -Format s)"
$short = Project 2000
$long = Project 1000000

# The 2,000-particle run the test first used, its cancel a timer: late by D ms, as a timer thread
# held up by load would be.
$lines += '--- 2,000 particles, --cancel-after-ms D (the first design) ---'
$lines += Run $short @() 'uncancelled'
foreach ($d in 1, 1, 1, 50, 100, 150, 200, 300, 400, 600, 1000) {
    $lines += Run $short @('--cancel-after-ms', "$d") "timer $d ms"
}
# 1,000,000 particles cancelled at SPPS's own 1 % line (the design now), and by timers late by
# up to 10 s: how much later the cancel may land and still find the solver running.
$lines += '--- 1,000,000 particles ---'
foreach ($i in 1..3) { $lines += Run $long @('--cancel-after-progress', '1') 'progress 1 %' }
foreach ($d in 150, 5000, 10000) {
    $lines += Run $long @('--cancel-after-ms', "$d") "timer $d ms"
}
$lines += "finished: $(Get-Date -Format s)"
[IO.File]::WriteAllLines((Join-Path $Work 'cancel_race.log'), $lines)
