# Runs spps.exe on a bed arm and samples its working set and commit every second: the realised memory number.
#   powershell -File run_sampled.ps1 -Exe <spps.exe> -Arm <folder>
param([string]$Exe, [string]$Arm)
Set-Location $Arm
$t0 = Get-Date
$p = Start-Process -FilePath $Exe -ArgumentList 'config.xml' -WorkingDirectory $Arm -RedirectStandardOutput "$Arm\stdout.txt" -RedirectStandardError "$Arm\stderr.txt" -PassThru -NoNewWindow
$peakWs = 0; $peakPm = 0; $n = 0
while (-not $p.HasExited) {
    try { $p.Refresh(); if ($p.WorkingSet64 -gt $peakWs) { $peakWs = $p.WorkingSet64 }; if ($p.PagedMemorySize64 -gt $peakPm) { $peakPm = $p.PagedMemorySize64 } } catch {}
    $n++; Start-Sleep -Milliseconds 1000
}
$el = [int]((Get-Date) - $t0).TotalSeconds
$line = '{0}: exit {1} in {2} s, {3} samples, peak working set {4:N0} MB, peak commit {5:N0} MB' -f (Split-Path -Leaf $Arm), $p.ExitCode, $el, $n, ($peakWs / 1MB), ($peakPm / 1MB)
$line
Set-Content -Path "$Arm\memory.txt" -Value $line -Encoding UTF8
