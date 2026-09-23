# Runs the real solvers on the run-folder fixtures, as docs/solver-contract.md Part B launches
# them, and records what they did. mkexpected.py then judges the records.
#
#   powershell -File tools/fixture-gen/runsolvers.ps1 -Runs tests/fixtures/runs -Out <scratch>
#       [-Solvers <dir>] [-Case <name>,<name>] [-TimeoutSec 300]
#
# Per case (a folder under -Runs whose name starts with spps_ or tcr_; stub_ folders are for the
# stub solver, not these):
# - <Out>/<case>/solve/ is created fresh and must not exist: an old run is never reused;
# - every fixture file except expected.json is copied into it, and __RUNDIR__ in its config.xml
#   is replaced by the folder's absolute path plus a backslash (UTF-8, no BOM, bytes otherwise
#   unchanged);
# - the solver runs with cwd = solve/ and the single argument config.xml;
# - stdout and stderr go, separately and byte for byte, to <Out>/<case>/solver.stdout.txt and
#   solver.stderr.txt, beside solve/, and the exit code (as u32 hex) and time to run.json.
# The solvers come from -Solvers, else $env:SIMPA_SOLVERS_DIR, else <repo>/target/solvers/bin.
# Precedent: tools/gates/m1.ps1.
param(
    [Parameter(Mandatory = $true)][string]$Runs,
    [Parameter(Mandatory = $true)][string]$Out,
    [string]$Solvers = '',
    [string[]]$Case = @(),
    [int]$TimeoutSec = 300
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
if (-not $Solvers) { $Solvers = $env:SIMPA_SOLVERS_DIR }
if (-not $Solvers) { $Solvers = Join-Path $repo 'target\solvers\bin' }
$exe = @{
    spps = Join-Path $Solvers 'spps.exe'
    tcr  = Join-Path $Solvers 'classicalTheory.exe'
}
foreach ($e in $exe.Values) {
    if (-not (Test-Path $e)) { throw "solver not found: $e (set -Solvers or SIMPA_SOLVERS_DIR)" }
}
$Runs = (Resolve-Path $Runs).Path
New-Item -ItemType Directory -Force $Out | Out-Null
$Out = (Resolve-Path $Out).Path
$utf8 = New-Object System.Text.UTF8Encoding($false)

$cases = Get-ChildItem $Runs -Directory | Where-Object { $_.Name -match '^(spps|tcr)_' }
if ($Case.Count) { $cases = $cases | Where-Object { $Case -contains $_.Name } }
foreach ($c in $cases) {
    $solver = $c.Name.Split('_')[0]
    $caseOut = Join-Path $Out $c.Name
    $solve = Join-Path $caseOut 'solve'
    if (Test-Path $caseOut) { throw "$caseOut exists: every run gets a fresh folder" }
    New-Item -ItemType Directory $solve | Out-Null
    Get-ChildItem $c.FullName -File | Where-Object { $_.Name -ne 'expected.json' } |
        ForEach-Object { Copy-Item $_.FullName $solve }
    $cfg = Join-Path $solve 'config.xml'
    $text = [IO.File]::ReadAllText($cfg, $utf8)
    if (-not $text.Contains('__RUNDIR__')) { throw "$($c.Name): config.xml has no __RUNDIR__" }
    [IO.File]::WriteAllText($cfg, $text.Replace('__RUNDIR__', "$solve\"), $utf8)

    $stdout = Join-Path $caseOut 'solver.stdout.txt'
    $stderr = Join-Path $caseOut 'solver.stderr.txt'
    $t0 = Get-Date
    $p = Start-Process -FilePath $exe[$solver] -ArgumentList 'config.xml' -WorkingDirectory $solve `
        -NoNewWindow -PassThru -RedirectStandardOutput $stdout -RedirectStandardError $stderr
    $null = $p.Handle  # keeps the handle, so ExitCode is available after the wait
    $timedOut = -not $p.WaitForExit($TimeoutSec * 1000)
    if ($timedOut) { $p.Kill(); $p.WaitForExit() }
    $elapsed = ((Get-Date) - $t0).TotalMilliseconds
    $hex = '0x{0:X8}' -f $p.ExitCode
    $record = [ordered]@{
        case       = $c.Name
        solver     = $solver
        exe        = $exe[$solver]
        argv       = @('config.xml')
        exit_code  = $hex
        timed_out  = $timedOut
        elapsed_ms = [math]::Round($elapsed)
    }
    [IO.File]::WriteAllText((Join-Path $caseOut 'run.json'), ($record | ConvertTo-Json), $utf8)
    Write-Host ("{0,-24} {1,-5} exit {2}  {3,7:N0} ms{4}" -f $c.Name, $solver, $hex, $elapsed, $(if ($timedOut) { '  TIMEOUT' } else { '' }))
}
