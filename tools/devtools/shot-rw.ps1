# Screenshots of the response window on CR4-27: default, zoomed by wheel, span 100 dB + 5-step bin.
# The app is started --e2e with the DevTools port; the window is opened and driven through the page
# (no mouse). Leaves the app running for Burhan to look at.
param([int]$Port = 9223, [string]$Tag = (Get-Date -Format 'HHmm'))
$ErrorActionPreference = 'Continue'
$sp = $PSScriptRoot
$out = 'B:\repos\I-Simpa_Night_Mode\.out\ui\cr4-27\plots'
function JS($expr, $t = 60) { (& python "$sp\cdp.py" $Port $expr $t 2>&1 | Out-String).Trim() }
function Shot($name) { & python "$sp\cdp-shot.py" $Port "$out\$name" 2>&1 | Out-String }
$env:SIMPA_SOLVERS_DIR = 'C:\tmp\nm-solvers-timebin\bin'
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$Port"
$exe = 'C:\tmp\nm-target\release\app.exe'
$proj = 'B:\repos\I-Simpa_Night_Mode\.out\ui\cr4-27\CR4-27.simpa'
"app.exe built $((Get-Item $exe).LastWriteTime.ToString('HH:mm:ss'))"
$p = Start-Process -FilePath $exe -ArgumentList @('--e2e', '--project', $proj) -PassThru
"pid $($p.Id)"
Start-Sleep -Seconds 8
"setStep: " + (JS "window.__m10.setStep('results')" 20)
# Wait for the report (the Acoustics tab stops saying Reading...), up to 150 s.
$t0 = Get-Date
do {
  Start-Sleep -Seconds 5
  $txt = JS "document.body.innerText.replace(/\s+/g,' ')" 20
} while ($txt -match 'Reading this run' -and ((Get-Date) - $t0).TotalSeconds -lt 150)
"report after $([int]((Get-Date) - $t0).TotalSeconds) s"
"acoustics tab: " + (JS "(() => { const el = [...document.querySelectorAll('button.dock-tab')].find(e => e.textContent.trim() === 'Acoustics'); el && el.click(); return !!el; })()" 20)
Start-Sleep -Seconds 2
"open response: " + (JS "(() => { const b = document.querySelector('[data-action=open-response]'); b && b.click(); return !!b; })()" 20)
Start-Sleep -Seconds 4
"hook: " + (JS "JSON.stringify(window.__m10.responseView ? window.__m10.responseView() : null)" 20)
Shot "rw-default-$Tag.png"
# Zoom in by wheel at 30 % of the map width, three notches.
"wheel: " + (JS "(() => { const el = document.querySelector('[data-part=response-map-box]'); if (!el) return 'no box'; const r = el.getBoundingClientRect(); for (let i = 0; i < 3; i++) el.dispatchEvent(new WheelEvent('wheel', { deltaY: -100, clientX: r.left + r.width * 0.3, clientY: r.top + r.height / 2, bubbles: true })); return 'ok'; })()" 20)
Start-Sleep -Seconds 2
"hook zoomed: " + (JS "JSON.stringify(window.__m10.responseView())" 20)
Shot "rw-zoomed-$Tag.png"
# Span 100 dB, 5-step bin.
"span: " + (JS "(() => { const b = document.querySelector('[data-span=\"100\"]'); b && b.click(); return !!b; })()" 20)
"bin: " + (JS "(() => { const b = document.querySelector('[data-bin=\"5\"]'); b && b.click(); return !!b; })()" 20)
Start-Sleep -Seconds 3
"hook span: " + (JS "JSON.stringify(window.__m10.responseView())" 20)
Shot "rw-span100-bin5-$Tag.png"
"shown text: " + (JS "document.querySelector('[data-part=response-shown]')?.innerText" 20)
"=== DONE ==="
