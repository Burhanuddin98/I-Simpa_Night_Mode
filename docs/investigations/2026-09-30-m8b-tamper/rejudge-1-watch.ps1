# Round 1 process watch. One line per new solver process (spps, classicalTheory, tetgen, preprocess),
# with its whole parent chain. A solver whose chain reaches the gate (a command line naming m8a.ps1
# or r1-rejudge.ps1) is the gate starting a run, which it must not: the watch then stops that solver
# and the gate's process tree, and says so. Also logs each simpa.exe once. Ends when r1-watch.stop
# exists beside this script. Writes r1-watch.log beside it, and a heartbeat every 60 s.
$log = Join-Path $PSScriptRoot 'r1-watch.log'
$stop = Join-Path $PSScriptRoot 'r1-watch.stop'
function Out([string]$line) { Write-Output $line; Add-Content -LiteralPath $log -Value $line -Encoding utf8 }
function Chain($p) {
    $chain = @(); $cur = $p.ParentProcessId
    for ($i = 0; $i -lt 16 -and $cur; $i++) {
        $q = Get-CimInstance Win32_Process -Filter "ProcessId=$cur" -ErrorAction SilentlyContinue
        if (-not $q) { $chain += [pscustomobject]@{ Id = $cur; Name = '(gone)'; Cmd = '' }; break }
        $chain += [pscustomobject]@{ Id = $q.ProcessId; Name = $q.Name; Cmd = "$($q.CommandLine)" }
        $cur = $q.ParentProcessId
    }
    $chain
}
function KillTree([int]$id) {
    foreach ($c in @(Get-CimInstance Win32_Process -Filter "ParentProcessId=$id" -ErrorAction SilentlyContinue)) { KillTree $c.ProcessId }
    Stop-Process -Id $id -Force -ErrorAction SilentlyContinue
}
$seen = @{}; $beat = [Diagnostics.Stopwatch]::StartNew()
Out ("{0} WATCH start pid {1}" -f (Get-Date -Format 'HH:mm:ss'), $PID)
while (-not (Test-Path -LiteralPath $stop)) {
    $procs = @(Get-CimInstance Win32_Process -Filter "Name='spps.exe' OR Name='classicalTheory.exe' OR Name='tetgen.exe' OR Name='preprocess.exe' OR Name='simpa.exe'" -ErrorAction SilentlyContinue)
    foreach ($p in $procs) {
        if ($seen.ContainsKey($p.ProcessId)) { continue }
        $seen[$p.ProcessId] = $true
        $chain = @(Chain $p)
        $text = ($chain | ForEach-Object { "$($_.Id) $($_.Name) [$($_.Cmd.Substring(0, [Math]::Min(120, $_.Cmd.Length)))]" }) -join ' <- '
        $cmd = "$($p.CommandLine)"
        Out ("{0} {1} {2} pid {3} [{4}] <- {5}" -f (Get-Date -Format 'HH:mm:ss'), $(if ($p.Name -eq 'simpa.exe') { 'SIMPA ' } else { 'SOLVER' }), $p.Name, $p.ProcessId, $cmd.Substring(0, [Math]::Min(200, $cmd.Length)), $text)
        if ($p.Name -ne 'simpa.exe') {
            $gate = @($chain | Where-Object { $_.Cmd -match 'm8a\.ps1|r1-rejudge\.ps1' })
            if ($gate.Count) {
                $root = $gate[$gate.Count - 1]
                Out ("{0} GATE-SOLVER {1} pid {2} started by the gate: stopping it and the gate tree from pid {3}" -f (Get-Date -Format 'HH:mm:ss'), $p.Name, $p.ProcessId, $root.Id)
                Stop-Process -Id $p.ProcessId -Force -ErrorAction SilentlyContinue
                KillTree $root.Id
            }
        }
    }
    if ($beat.Elapsed.TotalSeconds -ge 60) { Out ("{0} WATCH alive, {1} processes seen" -f (Get-Date -Format 'HH:mm:ss'), $seen.Count); $beat.Restart() }
    Start-Sleep -Milliseconds 300
}
Out ("{0} WATCH stop" -f (Get-Date -Format 'HH:mm:ss'))
