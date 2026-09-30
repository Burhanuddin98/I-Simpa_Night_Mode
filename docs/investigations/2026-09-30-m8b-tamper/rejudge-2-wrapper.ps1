# Round 2 re-judge of the M8a bed on m8b-tamper (round 1's wrapper, its paths changed). Stamps each
# gate line with the clock, echoes it, and writes it through one writer held open with
# FileShare.ReadWrite, so a reader never blocks an append. The gate finds the committed seal by the
# bed folder's name, and refuses to start without one.
$log = 'C:\tmp\nm-judge-tamper\r2\rejudge-2-gate.txt'
$env:CARGO_TARGET_DIR = 'C:/tmp/nm-target'
$env:CARGO_BUILD_JOBS = '16'
$env:CARGO_INCREMENTAL = '0'
$env:SIMPA_SOLVERS_DIR = 'C:/tmp/nm-m8a-solvers'
Set-Location 'B:\repos\I-Simpa_Night_Mode\.claude\worktrees\m8b-tamper'
$fs = New-Object IO.FileStream -ArgumentList $log, 'Create', 'Write', 'ReadWrite'
$w = New-Object IO.StreamWriter -ArgumentList $fs, (New-Object Text.UTF8Encoding -ArgumentList $false)
$w.AutoFlush = $true
try {
    $w.WriteLine("START $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss zzz') wrapper pid $PID head $(git rev-parse HEAD) status [$(git status --porcelain)]")
    powershell -NoProfile -File tools/gates/m8a.ps1 -From C:/tmp/nm-m8a-bed/20260929T093134Z -Jobs 12 2>&1 |
        ForEach-Object { "$(Get-Date -Format 'HH:mm:ss') $_" } |
        ForEach-Object { $_; $w.WriteLine($_) }
    $code = $LASTEXITCODE
    $w.WriteLine("END $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss zzz') exit $code head $(git rev-parse HEAD)")
    $w.WriteLine('=== DONE ===')
} finally { $w.Dispose() }
