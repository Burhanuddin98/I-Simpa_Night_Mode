# M12 gate: Concept B, Results (docs/investigations/2026-10-03-m12/PLAN.md; spec
# docs/rebuild-plan-raw-2026-09-23.json, M12).
#   (a) every number the Results step shows equals `simpa results <run> --json` at the displayed
#       precision: 0 mismatches
#   (b) 0 DOM elements for any parameter whose status in beds/summary.json is not PASS
#   (c) 3 sampled (face, step) texels of a surface map == the .csbin's float32 exactly
#   (d) at 5 steps, the rendered particle count == the particles alive in the .pbin
#   (e) the DIN 18041 A3 target for the 180 m3 teaching room reads 0.55 s
#   (f) the variant switch replaces the RT series with the other run's JSON: 0 mismatches
# plus m10-h and m11-h, narrowed to "no solver-computed number outside the Results step" (they run
# inside m11.ps1, this gate's prior gate).
#
# P1 built the contracts (PLAN.md P1): beds/summary.json regenerated from its evidence byte for
# byte (item 1), the report's per-parameter bed status (item 2), the Results step's IPC reads and
# their decoders (item 3), the narrowed checker (item 4). P2 added the e2e section and its spec,
# app/e2e/specs/m12.acoustics.e2e.ts (a, b, e, f); P3's spec m12.viewport.e2e.ts carries (c, d).
# A spec not written yet is pending and its ids fail as NOT BUILT, so this gate cannot print
# "M12 PASSED" until every id runs and passes. The e2e takes m11.ps1's lock (port 4444).
#
# Windows PowerShell 5.1 (pwsh is not installed on Grace).
# Run: powershell -File tools/gates/m12.ps1 [-TargetDir C:\tmp\nm-target] [-Only all|static|e2e]
#        [-Spec acoustics,viewport] [-E2eHome C:\tmp\nm-e2e] [-FetchDriver]
#        [-SolversDir C:\tmp\nm-m8a-solvers] [-BedData B:\data] [-SkipPrior]
# Partial runs (-Only static or e2e, a -Spec subset, -SkipPrior) never print "M12 PASSED".
param(
    [string]$TargetDir = 'C:\tmp\nm-target',
    [ValidateSet('all', 'static', 'e2e')][string]$Only = 'all',
    [string[]]$Spec = @('acoustics', 'viewport'),
    [string]$E2eHome = 'C:\tmp\nm-e2e',
    [switch]$FetchDriver,
    [string]$SolversDir = 'C:\tmp\nm-m8a-solvers',
    [string]$Upstream = 'B:\repos\I-Simpa-upstream',
    # The bed evidence beds/summary.json is derived from (m8b-bed, m8b-edt).
    [string]$BedData = 'B:\data',
    [switch]$SkipPrior
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Set-Location $repo
$env:RUSTUP_HOME = "$env:USERPROFILE\.rustup"; $env:CARGO_HOME = "$env:USERPROFILE\.cargo"
$env:Path = "$env:CARGO_HOME\bin;$env:Path"; $env:CARGO_INCREMENTAL = '0'
if (-not $env:CARGO_BUILD_JOBS) { $env:CARGO_BUILD_JOBS = '16' }
if (-not [IO.Path]::IsPathRooted($TargetDir)) { throw "-TargetDir must be absolute (C:\tmp\nm-target): $TargetDir" }
$target = [IO.Path]::GetFullPath($TargetDir)
if ($target.Substring(0, 2) -ieq $repo.Substring(0, 2)) { throw "-TargetDir must not be on the repository's drive ($($repo.Substring(0, 2))): $target" }
# Forward slashes: Git Bash has mangled backslashes in this variable into a folder in the repo.
$env:CARGO_TARGET_DIR = $target.Replace('\', '/')
$appDir = Join-Path $repo 'app'
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$work = Join-Path $target "gates\m12\$stamp"
New-Item -ItemType Directory -Force $work | Out-Null
# What is untracked in the repository before the run: the run must add nothing to it (B: is exFAT).
function Untracked { @(cmd /c "git -C `"$repo`" status --porcelain --untracked-files=all 2>nul" | Where-Object { $_ -match '^\?\? ' }) }
$untrackedBefore = Untracked
$fullRun = $Only -eq 'all' -and -not $SkipPrior

$failures = @()
function Check($name, [scriptblock]$body) {
    try {
        $ok = & $body
        if ($ok) { Write-Host "PASS  $name" } else { Write-Host "FAIL  $name"; $script:failures += $name }
    } catch {
        Write-Host "FAIL  $name :: $($_.Exception.Message)"; $script:failures += $name
    }
}
function Note([string]$text) { Write-Host "      $text" }
# A native program through cmd, its output to a log file; returns the exit code.
function Native([string]$cmdline, [string]$log) {
    cmd /c "$cmdline > `"$log`" 2>&1"
    $LASTEXITCODE
}
function Tail([string]$log, [int]$n = 6) { if (Test-Path $log) { Get-Content $log -Tail $n | ForEach-Object { Note $_ } } }
# `cargo test` with its counts noted; true when it exits 0 and ran at least one test.
function CargoTest([string]$args_, [string]$name) {
    $log = Join-Path $work "$name.log"
    $code = Native "cargo test -q $args_" $log
    $results = @(Get-Content $log | Where-Object { $_ -match '^test result' })
    $passed = ($results | ForEach-Object { if ($_ -match '(\d+) passed') { [int]$Matches[1] } } | Measure-Object -Sum).Sum
    $failed = ($results | ForEach-Object { if ($_ -match '(\d+) failed') { [int]$Matches[1] } } | Measure-Object -Sum).Sum
    Note "$passed passed, $failed failed; log $log"
    if ($code -ne 0) { Tail $log 12 }
    $code -eq 0 -and $passed -gt 0
}

# ---- 1. the contracts (P1) and the report's room (P2) ---------------------------------------------
if ($Only -ne 'e2e') {

Check "item 1: beds/summary.json is tools/bed/summary.py's output from the evidence under $BedData, byte for byte" {
    $env:SIMPA_BED_DATA = $BedData
    try { CargoTest '-p simpa-core --test bed_summary' 'bed-summary' } finally { Remove-Item Env:\SIMPA_BED_DATA }
}

Check "item 1: every parameter's derived status (gate (b) renders only PASS)" {
    $s = Get-Content (Join-Path $repo 'beds\summary.json') -Raw | ConvertFrom-Json
    $names = @($s.parameters.PSObject.Properties.Name)
    foreach ($n in $names) {
        $p = $s.parameters.$n
        Note ("{0,-7} {1}{2}" -f $n, $p.status, $(if ($p.reasons.Count) { "  ($($p.reasons[0]))" } else { '' }))
    }
    $names.Count -eq 11 -and @($names | Where-Object { @('PASS', 'FAIL') -notcontains $s.parameters.$_.status }).Count -eq 0
}

Check "item 2: the report carries each parameter's bed status from beds/summary.json (results version 12; the required fields pinned)" {
    $a = CargoTest '-p simpa-core --lib -- results::bed results::report::tests::the_required_fields_are_pinned_to_the_results_version' 'report-bed-unit'
    $b = CargoTest '-p simpa --test cli_results -- gate_e_seat_and_seat2 an_spps_report_carries the_committed_schema' 'report-bed-cli'
    $a -and $b
}

Check "item 3: the Results step's IPC reads, the CLI's report and the result files bit for bit (cargo test -p app results_data)" {
    CargoTest '-p app results_data' 'ipc-reads'
}

Check "item 3: the SMAP and PART decoders (node --test ui/src/resultsData.test.ts)" {
    $log = Join-Path $work 'ui-results-data.log'
    Push-Location $appDir
    try { $code = Native 'node --test ui/src/resultsData.test.ts' $log } finally { Pop-Location }
    Tail $log 4
    $code -eq 0
}

Check "item 4: m10-h and m11-h narrowed to no number outside the Results step, and a leak onto another step still fails (node --test e2e/lib/acoustic.test.ts)" {
    $log = Join-Path $work 'harness-acoustic.log'
    Push-Location $appDir
    try { $code = Native 'node --test e2e/lib/acoustic.test.ts' $log } finally { Pop-Location }
    Tail $log 4
    $code -eq 0 -and (Select-String -Path $log -Pattern 'M12: a number in the Results regions' -Quiet)
}

Check "P2: the report carries the room: DIN 18041 targets (A3 at 180 m3 reads 0.55 s), absorption by surface group, SPPS Sabine (cli_results m12_)" {
    CargoTest '-p simpa --test cli_results -- m12_' 'report-room'
}

Check "P2: the UI's unit tests (npm test: the Acoustics tab's model among them) and typecheck" {
    $log = Join-Path $work 'ui-test.log'
    Push-Location $appDir
    try { $code = Native 'npm test -s' $log; $tc = Native 'npm run -s typecheck' (Join-Path $work 'ui-typecheck.log') } finally { Pop-Location }
    Get-Content $log | Where-Object { $_ -match '^. (tests|pass|fail) |not ok' } | ForEach-Object { Note $_ }
    Tail (Join-Path $work 'ui-typecheck.log') 10
    $code -eq 0 -and $tc -eq 0 -and (Select-String -Path $log -Pattern 'Acoustics' -Quiet)
}

}

# ---- 2. the gate's e2e ids (P2's acoustics spec; P3's viewport spec) ------------------------------
# Each spec file's ids. A spec file not written yet is reported as pending and its ids fail as NOT
# BUILT: a run that lacks any required id never prints "M12 PASSED".
$specIds = [ordered]@{
    # P2 (PLAN.md P2): app/e2e/specs/m12.acoustics.e2e.ts
    acoustics = @('m12-a', 'm12-b', 'm12-e', 'm12-f')
    # P3 (PLAN.md P3): app/e2e/specs/m12.viewport.e2e.ts
    viewport  = @('m12-c', 'm12-d')
}
$Spec = @($Spec | ForEach-Object { $_ -split ',' } | ForEach-Object { $_.Trim() } | Where-Object { $_ })
foreach ($s in $Spec) { if (-not $specIds.Contains($s)) { throw "unknown -Spec '$s': one of $(@($specIds.Keys) -join ', ')" } }
if (@($specIds.Keys | Where-Object { $Spec -notcontains $_ }).Count) { $fullRun = $false }
if ($Only -eq 'static') { Note 'the e2e ids (a)-(f): not run (-Only static)' }
if ($Only -ne 'static') {

$exe = Join-Path $target 'release\app.exe'
$simpa = Join-Path $target 'release\simpa.exe'
$node = Join-Path $E2eHome 'node'
$lockPath = Join-Path $E2eHome 'e2e.lock'
New-Item -ItemType Directory -Force $E2eHome | Out-Null
$lock = $null
$driverExe = $null
$tauriDriver = Join-Path $env:CARGO_HOME 'bin\tauri-driver.exe'
$privateSolvers = Join-Path $work 'solvers'
$junitDir = Join-Path $work 'wdio'
function Sha256([string]$path) { (Get-FileHash -Algorithm SHA256 $path).Hash.ToLowerInvariant() }
. (Join-Path $repo 'solvers\pe-fingerprint.ps1')

# Port 4444 is fixed, and every build and e2e run holds this lock (m11.ps1's), waiting up to 30 min.
Check "e2e lock ($lockPath)" {
    $t0 = Get-Date
    while (-not $script:lock) {
        try { $script:lock = [IO.File]::Open($lockPath, 'OpenOrCreate', 'ReadWrite', 'None') }
        catch {
            if (((Get-Date) - $t0).TotalMinutes -gt 30) { throw 'another build or e2e run held the lock for 30 min' }
            Start-Sleep -Seconds 5
        }
    }
    Note "held after $([math]::Round(((Get-Date) - $t0).TotalSeconds)) s"
    $true
}

try {

$built = $false
Check "build: npx tauri build --no-bundle (custom protocol, ui/dist embedded) and simpa.exe" {
    if (-not $script:lock) { throw 'no lock' }
    $t0 = Get-Date
    $log = Join-Path $work 'tauri-build.log'
    Push-Location $appDir
    try { $code = Native 'npx --no-install tauri build --no-bundle' $log } finally { Pop-Location }
    Tail $log 4
    $fresh = (Test-Path $exe) -and (Get-Item $exe).LastWriteTime -ge $t0.AddSeconds(-1)
    $cli = Native 'cargo build -q --release -p simpa --bin simpa' (Join-Path $work 'simpa-build.log')
    Note "exit $code in $([math]::Round(((Get-Date) - $t0).TotalSeconds, 1)) s; app.exe rebuilt: $fresh; simpa.exe build exit $cli"
    $script:built = $code -eq 0 -and $fresh -and $cli -eq 0
    $script:built
}

Check "harness: tauri-driver present" {
    if (-not (Test-Path $tauriDriver)) { throw "no ${tauriDriver}: cargo install tauri-driver --locked" }
    Note $tauriDriver
    $true
}

Check "harness: msedgedriver matches the WebView2 runtime exactly" {
    $pv = (Get-ItemProperty 'HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}').pv
    $dir = Join-Path $E2eHome "msedgedriver\$pv"
    $script:driverExe = Join-Path $dir 'msedgedriver.exe'
    $url = "https://msedgedriver.microsoft.com/$pv/edgedriver_win64.zip"
    if (-not (Test-Path $script:driverExe)) {
        if (-not $FetchDriver) { throw "no msedgedriver $pv at $($script:driverExe); rerun with -FetchDriver, or fetch $url" }
        New-Item -ItemType Directory -Force $dir | Out-Null
        $zip = Join-Path $dir 'edgedriver_win64.zip'
        $ProgressPreference = 'SilentlyContinue'
        Invoke-WebRequest -UseBasicParsing -Uri $url -OutFile $zip
        Expand-Archive -Force $zip $dir
    }
    $v = @(cmd /c "`"$($script:driverExe)`" --version 2>nul")[0]
    Note "WebView2 runtime pv $pv; $v"
    "$v" -match [regex]::Escape($pv)
}

Check "harness: WebdriverIO installed from app/e2e/package-lock.json (by its sha256)" {
    $lockFile = Join-Path $appDir 'e2e\package-lock.json'
    $want = Sha256 $lockFile
    $stampFile = Join-Path $node '.lock.sha256'
    $have = if (Test-Path $stampFile) { (Get-Content $stampFile -Raw).Trim() } else { '' }
    New-Item -ItemType Directory -Force $node | Out-Null
    if ($have -ne $want -or -not (Test-Path (Join-Path $node 'node_modules\.bin\wdio.cmd'))) {
        Copy-Item (Join-Path $appDir 'e2e\package.json'), $lockFile $node -Force
        Push-Location $node
        try { $code = Native 'npm ci --no-audit --no-fund' (Join-Path $work 'e2e-npm-ci.log') } finally { Pop-Location }
        if ($code -ne 0) { Tail (Join-Path $work 'e2e-npm-ci.log') 10; throw "npm ci exited $code" }
        Set-Content -Path $stampFile -Value $want -Encoding ascii
    } else { Note "current: lock sha256 $($want.Substring(0, 16))" }
    Test-Path (Join-Path $node 'node_modules\.bin\wdio.cmd')
}

Check "harness: the e2e configs, libraries and specs typecheck, and the libraries' tests pass (node --test e2e/lib)" {
    $log = Join-Path $work 'e2e-tsc.log'
    $code = Native "`"$appDir\node_modules\.bin\tsc.cmd`" -p `"$appDir\e2e\tsconfig.json`"" $log
    Tail $log 20
    $tlog = Join-Path $work 'e2e-lib-test.log'
    Push-Location $appDir
    try { $tcode = Native 'node --test "e2e/lib/*.test.ts"' $tlog } finally { Pop-Location }
    Get-Content $tlog | Where-Object { $_ -match '^. (tests|pass|fail) |not ok' } | ForEach-Object { Note $_ }
    $code -eq 0 -and $tcode -eq 0
}

# The app runs a private copy of the verified build (m11.ps1, PLAN.md 4.1 F8).
Check "harness: the private solver copy, each executable the verified build by code sha256" {
    New-Item -ItemType Directory -Force $privateSolvers | Out-Null
    $manifest = Get-Content (Join-Path $repo 'solvers\manifest.json') -Raw | ConvertFrom-Json
    $ok = $true
    foreach ($name in 'spps.exe', 'classicalTheory.exe', 'tetgen.exe', 'preprocess.exe') {
        $src = Join-Path $SolversDir $name
        if (-not (Test-Path $src)) { Note "MISSING $src"; $ok = $false; continue }
        $dst = Join-Path $privateSolvers $name
        Copy-Item $src $dst -Force
        if ((Get-CodeSha256 $dst) -ne $manifest.code_sha256.$name) { Note "$name differs from the manifest"; $ok = $false }
    }
    $ok
}

$present = @($Spec | Where-Object { Test-Path (Join-Path $appDir "e2e\specs\m12.$_.e2e.ts") })
$pending = @($Spec | Where-Object { $present -notcontains $_ })
foreach ($p in $pending) { Note "PENDING: app/e2e/specs/m12.$p.e2e.ts is not written yet (its ids fail below)" }
Check "e2e: wdio ran (verdict below) (-Spec $($present -join ','))" {
    if (-not $built) { throw 'no fresh app.exe from the build' }
    if (-not $script:driverExe) { throw 'no msedgedriver' }
    if ($present.Count -eq 0) { throw 'no spec file to run' }
    New-Item -ItemType Directory -Force $junitDir | Out-Null
    $env:M11_APP = $exe; $env:M11_WORK = $junitDir; $env:M11_REPO = $repo
    $env:M11_TAURI_DRIVER = $tauriDriver; $env:M11_NATIVE_DRIVER = $script:driverExe
    $env:M11_SOLVERS = $privateSolvers; $env:M11_GATEWORK = $work
    $env:M12_SPEC = $present -join ','; $env:M12_SIMPA = $simpa
    # msedgedriver passes the environment on to app.exe: this is where the app finds its solvers.
    $env:SIMPA_SOLVERS_DIR = $privateSolvers
    $t0 = Get-Date
    $log = Join-Path $work 'wdio.log'
    $code = Native "`"$node\node_modules\.bin\wdio.cmd`" run app/e2e/m12.conf.ts" $log
    Note "exit $code in $([math]::Round(((Get-Date) - $t0).TotalSeconds, 1)) s; log $log; the ids are judged from the junit files below"
    Get-Content $log | Where-Object { $_ -match 'receipt' } | ForEach-Object { Note $_.Trim() }
    $code -eq 0
}

} finally {
    if ($lock) { $lock.Dispose(); Note 'e2e lock released' }
}

# The verdict, from the junit files (m11.ps1's reading).
Check "verdict: every required id passed, 0 failures, 0 skipped" {
    $required = @($Spec | ForEach-Object { $specIds[$_] })
    $norm = { param($s) (("$s" -replace '[^A-Za-z0-9]+', ' ').Trim().ToLowerInvariant()) + ' ' }
    $allIds = @($specIds.Values | ForEach-Object { $_ } | Sort-Object Length -Descending)
    $cases = @()
    foreach ($f in @(Get-ChildItem $junitDir -Filter 'junit-*.xml' -ErrorAction SilentlyContinue)) {
        [xml]$x = Get-Content $f.FullName -Raw
        foreach ($tc in $x.SelectNodes('//testcase')) {
            $state = if ($tc.SelectSingleNode('failure|error')) { 'FAILED' } elseif ($tc.SelectSingleNode('skipped')) { 'SKIPPED' } else { 'passed' }
            $n = & $norm $tc.name
            $id = @($allIds | Where-Object { $n.StartsWith((& $norm $_)) } | Select-Object -First 1)[0]
            $msg = ''
            $fail = $tc.SelectSingleNode('failure|error')
            if ($fail) { $msg = "$($fail.GetAttribute('message'))" }
            $cases += [pscustomobject]@{ id = $id; name = $tc.name; state = $state; time = [double]$tc.time; message = $msg }
        }
    }
    foreach ($c in $cases) {
        Note ("{0,-7} {1,7:N1} s  {2,-8} {3}" -f $c.state, $c.time, $(if ($c.id) { $c.id } else { '(no id)' }), $c.name)
        if ($c.message) { Note ("          " + $c.message.Substring(0, [Math]::Min(400, $c.message.Length))) }
    }
    $passedIds = @($cases | Where-Object { $_.state -eq 'passed' } | ForEach-Object { $_.id })
    $missing = @($required | Where-Object { $passedIds -notcontains $_ })
    foreach ($id in $missing) { Note "NOT PASSED: $id" }
    $failedN = @($cases | Where-Object { $_.state -eq 'FAILED' }).Count
    $skippedN = @($cases | Where-Object { $_.state -eq 'SKIPPED' }).Count
    Note "$($cases.Count) test(s); required $($required.Count); failures $failedN; skipped $skippedN"
    $cases.Count -gt 0 -and $missing.Count -eq 0 -and $failedN -eq 0 -and $skippedN -eq 0
}

}

# ---- 3. prior gate ---------------------------------------------------------------------------------
if (-not $SkipPrior -and $Only -eq 'all') {
    Check "prior gate: m11.ps1 in full prints M11 PASSED (it runs m10.ps1 and m9.ps1)" {
        $log = Join-Path $work 'm11.log'
        $code = Native "powershell -NoProfile -ExecutionPolicy Bypass -File `"$repo\tools\gates\m11.ps1`" -TargetDir `"$target`" -SolversDir `"$SolversDir`" -Upstream `"$Upstream`"" $log
        Get-Content $log | Where-Object { $_ -match '^(PASS|FAIL) |^M11 ' } | ForEach-Object { Note $_ }
        $code -eq 0 -and (Select-String -Path $log -Pattern '^M11 PASSED' -Quiet)
    }
} else { Note 'prior gate: not run (-SkipPrior or -Only static)' }

# ---- 4. file counts --------------------------------------------------------------------------------
$files = @(Get-ChildItem $work -Recurse -File)
Write-Host "`nwork folder (C:): $work ($($files.Count) files)"
$left = @(Untracked | Where-Object { $untrackedBefore -notcontains $_ })
Write-Host "files this run left in the repository (B:): $($left.Count)"
$left | Select-Object -First 10 | ForEach-Object { Write-Host "      $_" }
if ($left.Count) { $failures += 'files left on B:' }
if (-not $fullRun) {
    if ($failures.Count) { Write-Host "M12 PARTIAL RUN (-Only $Only$(if ($SkipPrior) { ' -SkipPrior' })) FAILED: $($failures.Count) check(s)"; exit 1 }
    Write-Host "M12 PARTIAL RUN (-Only $Only$(if ($SkipPrior) { ' -SkipPrior' })): its checks passed; this is not a gate pass"; exit 0
}
if ($failures.Count) { Write-Host "M12 FAILED: $($failures.Count) check(s)"; exit 1 }
Write-Host "M12 PASSED"; exit 0
