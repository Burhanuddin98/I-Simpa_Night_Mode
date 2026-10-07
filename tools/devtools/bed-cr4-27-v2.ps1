# Bed v2 for reader fix part 2: app --e2e on CR4-27 with WebView2's debugging port; the Results step is
# opened through window.__m10.setStep('results') over the DevTools protocol (no mouse); memory sampled
# every 5 s; the step's visible text read back when the UI is idle. Log C:\tmp\nm-bed-part2.log.
param([int]$Seconds = 300, [int]$Port = 9223)
$log = 'C:\tmp\nm-bed-part2.log'
$cdp = "$PSScriptRoot\cdp.py"
function L($m) { $line = '{0}  {1}' -f (Get-Date -Format 'HH:mm:ss'), $m; $line | Add-Content -Encoding utf8 $log; $line }
function JS($expr, $t = 60) { & python $cdp $Port $expr $t 2>&1 | Out-String }
Remove-Item $log -ErrorAction SilentlyContinue
$env:SIMPA_SOLVERS_DIR = 'C:\tmp\nm-solvers-timebin\bin'
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$Port"
$exe = 'C:\tmp\nm-target\release\app.exe'
$proj = 'B:\repos\I-Simpa_Night_Mode\.out\ui\cr4-27\CR4-27.simpa'
L ("app.exe built {0}  {1:N1} MB; project {2}" -f (Get-Item $exe).LastWriteTime.ToString('HH:mm'), ((Get-Item $exe).Length/1MB), $proj)
$p = Start-Process -FilePath $exe -ArgumentList @('--e2e', '--project', $proj) -PassThru
L "started pid $($p.Id) (--e2e, debug port $Port)"
$peakWs = 0; $peakPm = 0; $peakWv = 0
$t0 = Get-Date
$stepSet = $false; $idleRead = $false
function Sample {
  $procs = Get-Process -Id $p.Id -ErrorAction SilentlyContinue
  if (-not $procs) { return $null }
  $kids = Get-CimInstance Win32_Process -Filter "ParentProcessId = $($p.Id)" -ErrorAction SilentlyContinue
  $wv = 0
  foreach ($k in $kids) {
    $kp = Get-Process -Id $k.ProcessId -ErrorAction SilentlyContinue; if ($kp) { $wv += $kp.WorkingSet64 }
    $gk = Get-CimInstance Win32_Process -Filter "ParentProcessId = $($k.ProcessId)" -ErrorAction SilentlyContinue
    foreach ($g in $gk) { $gp = Get-Process -Id $g.ProcessId -ErrorAction SilentlyContinue; if ($gp) { $wv += $gp.WorkingSet64 } }
  }
  $ws = $procs.WorkingSet64; $pm = $procs.PrivateMemorySize64
  if ($ws -gt $script:peakWs) { $script:peakWs = $ws }; if ($pm -gt $script:peakPm) { $script:peakPm = $pm }; if ($wv -gt $script:peakWv) { $script:peakWv = $wv }
  L ("t+{0,4}s  app WS {1,7:N0} MB  private {2,7:N0} MB  webview tree WS {3,7:N0} MB" -f [int]((Get-Date) - $t0).TotalSeconds, ($ws/1MB), ($pm/1MB), ($wv/1MB))
  return $true
}
while (((Get-Date) - $t0).TotalSeconds -lt $Seconds) {
  Start-Sleep -Seconds 5
  if ($null -eq (Sample)) { L "app EXITED after $([int]((Get-Date) - $t0).TotalSeconds) s (exit code $($p.ExitCode))"; break }
  $el = ((Get-Date) - $t0).TotalSeconds
  if (-not $stepSet -and $el -ge 10) {
    $r = JS "window.__m10 ? window.__m10.setStep('results') : 'no hooks'" 20
    L "setStep('results') -> $($r.Trim())"
    $stepSet = $true
    $tStep = Get-Date
  }
  if ($stepSet -and -not $idleRead -and ((Get-Date) - $tStep).TotalSeconds -ge 20) {
    # Poll the step's text; stop reading once the 'Checking' line is gone or 240 s passed.
    $txt = JS "document.body.innerText.replace(/\s+/g,' ').slice(0, 1500)" 20
    if ($txt -notmatch 'Checking this run' -or ((Get-Date) - $tStep).TotalSeconds -ge 240) {
      L "RESULTS TEXT after $([int]((Get-Date) - $tStep).TotalSeconds) s: $($txt.Trim())"
      $idleRead = $true
    }
  }
}
if (Get-Process -Id $p.Id -ErrorAction SilentlyContinue) {
  L ("ALIVE after {0} s; peak app WS {1:N0} MB, peak private {2:N0} MB, peak webview tree WS {3:N0} MB" -f $Seconds, ($peakWs/1MB), ($peakPm/1MB), ($peakWv/1MB))
}
L "=== DONE ==="
