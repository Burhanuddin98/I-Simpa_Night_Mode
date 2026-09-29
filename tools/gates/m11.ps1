# M11 gate: Concept B, Simulate, Console, Runs (docs/investigations/2026-09-29-m11/PLAN.md, 4).
#   (a) run the box from the UI: Runs row OK, 'Particles lost 0.00 %', Console counts = run.json's
#   (b) during the hall solve: 200 IPC pings p99 < 100 ms; an orbit drag's frame time p95 < 33 ms
#   (c) Cancel: the row reads 'Cancelled', and 2 s later no spps.exe runs from the gate's copy
#   (d) closing the window mid-solve, and Stop-Process on app.exe: no spps.exe 2 s later
#   (e) a forced mesh failure: a Runs row FAIL with MESH_TETGEN_SKIPPED; the Results step no number
# plus m11-h (no solver-computed acoustic number, the diagnostic allowance proven), row 22's items,
# m11-b18, and m11-focus (the test windows visible, and never taking keyboard focus).
#
# THIS IS THE FOUNDATION'S SKELETON (PLAN.md 9.0). It runs the static checks, the Rust checks, the
# fixtures' recipe, the core crates' tests, the release build, the harness prerequisites (the
# private solver copy, the projects on C:, the mesh-failure run), the e2e specs that exist, the
# prior gates, and the file counts. Spec files the packages have not written yet are reported as
# pending, and their ids fail: a run that lacks any required id never prints "M11 PASSED". Still
# to come with the packages and at integration: the gate, close, kill and after specs, the
# focus watcher (focus-watch.ps1 and its judge), and m11-h's checker.
#
# Windows PowerShell 5.1 (pwsh is not installed on Grace). Grace-local: the e2e opens windows,
# visible and unfocused (app.exe builds its window with focused(false) under --e2e), and needs
# tauri-driver, the msedgedriver of the live WebView2 runtime, and the solver build.
#
# Run: powershell -File tools/gates/m11.ps1 [-TargetDir C:\tmp\nm-target] [-E2eHome C:\tmp\nm-e2e]
#        [-Only all|static|e2e] [-Spec smoke,gate,close,kill,after,simulate,dock,project]
#        [-SolversDir C:\tmp\nm-m8a-solvers] [-SkipCore] [-SkipPrior] [-FetchDriver]
# Partial runs (-Only other than all, a -Spec subset, -SkipCore, -SkipPrior) never print
# "M11 PASSED".
param(
    [string]$TargetDir = 'C:\tmp\nm-target',
    [string]$E2eHome = 'C:\tmp\nm-e2e',
    [ValidateSet('all', 'static', 'e2e')][string]$Only = 'all',
    [string[]]$Spec = @('smoke', 'gate', 'close', 'kill', 'after', 'simulate', 'dock', 'project'),
    [string]$SolversDir = 'C:\tmp\nm-m8a-solvers',
    [string]$Upstream = 'B:\repos\I-Simpa-upstream',
    [switch]$SkipCore,
    [switch]$SkipPrior,
    [switch]$FetchDriver
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
$uiDir = Join-Path $appDir 'ui\src'
$tauriDir = Join-Path $appDir 'src-tauri'
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$work = Join-Path $target "gates\m11\$stamp"
New-Item -ItemType Directory -Force $work | Out-Null
# What is untracked in the repository before the run: the run must add nothing to it (B: is exFAT).
function Untracked { @(cmd /c "git -C `"$repo`" status --porcelain --untracked-files=all 2>nul" | Where-Object { $_ -match '^\?\? ' }) }
$untrackedBefore = Untracked
$runStatic = $Only -eq 'all' -or $Only -eq 'static'
$runE2e = $Only -eq 'all' -or $Only -eq 'e2e'
$Spec = @($Spec | ForEach-Object { $_ -split ',' } | ForEach-Object { $_.Trim() } | Where-Object { $_ })

# The test ids each spec file must pass (PLAN.md 4.1, 4.2).
$specIds = [ordered]@{
    smoke    = @('m11-smoke')
    gate     = @('m11-a', 'm11-b-ipc', 'm11-b-frames', 'm11-c', 'm11-e-row', 'm11-e-results', 'm11-h', 'm11-r22-default')
    close    = @('m11-d-close')
    kill     = @('m11-d-kill')
    after    = @('m11-d-after')
    simulate = @()
    dock     = @()
    project  = @('m11-r22-a9', 'm11-r22-a3', 'm11-r22-g42', 'm11-r22-m26', 'm11-r22-m5', 'm11-r22-m1', 'm11-b18')
}
$allSpecs = @($specIds.Keys)
foreach ($s in $Spec) { if (-not $specIds.Contains($s)) { throw "unknown -Spec '$s': one of $($allSpecs -join ', ')" } }
$fullRun = $Only -eq 'all' -and -not $SkipCore -and -not $SkipPrior -and (@($allSpecs | Where-Object { $Spec -notcontains $_ }).Count -eq 0)

# theme.css is frozen since the M10 foundation (M10 PLAN.md 2.4 rule 4; M11 PLAN.md 3.4 rule 5).
$themeBlob = 'dd3e89e270604ee648944c854c68ba3a2b51d331'

# M11's nine commands (PLAN.md 2.2), on top of M10's 28.
$m11Commands = @('run_start', 'run_cancel', 'runs_list', 'run_results', 'proj_import', 'material_library',
    'solvers_status', 'app_events', 'app_quit')

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
function Sha256([string]$path) { (Get-FileHash -Algorithm SHA256 $path).Hash.ToLowerInvariant() }
. (Join-Path $repo 'solvers\pe-fingerprint.ps1')

# Code with comments removed, for the lints.
function RustCode([string]$path) { [regex]::Replace((Get-Content $path -Raw), '//[^\n]*', '') }
function UiCode([string]$path) {
    $t = Get-Content $path -Raw
    $t = [regex]::Replace($t, '/\*[\s\S]*?\*/', '')
    [regex]::Replace($t, '(?m)(^|[^:"''`])//[^\n]*', '$1')
}
$uiFiles = @(Get-ChildItem $uiDir -Recurse -File -Include *.ts, *.tsx | Where-Object { $_.FullName -notmatch '\\bindings\\' -and $_.Name -notmatch '\.test\.ts$' })
function Rel([IO.FileInfo]$f) { $f.FullName.Substring($uiDir.Length + 1).Replace('\', '/') }

# ---- 1. static ---------------------------------------------------------------------------------
if ($runStatic) {

Check "M10 static checks (m10.ps1 -Only static -SkipCore; they run M9's)" {
    $log = Join-Path $work 'm10-static.log'
    $code = Native "powershell -NoProfile -ExecutionPolicy Bypass -File `"$repo\tools\gates\m10.ps1`" -Only static -SkipCore -TargetDir `"$target`" -SolversDir `"$SolversDir`"" $log
    Get-Content $log | Where-Object { $_ -match '^(PASS|FAIL) ' } | ForEach-Object { Note $_ }
    $code -eq 0
}

Check "command inventory: 37 commands (M10's 28 and M11's 9), the same set in the attributes, generate_handler!, build.rs and capabilities" {
    $attrs = @()
    foreach ($f in Get-ChildItem (Join-Path $tauriDir 'src') -Filter *.rs) {
        $attrs += @([regex]::Matches((RustCode $f.FullName), '#\[tauri::command\b[^\]]*\]\s*(?:#\[[^\]]*\]\s*)*pub\s+async\s+fn\s+(\w+)') | ForEach-Object { $_.Groups[1].Value })
    }
    $main = RustCode (Join-Path $tauriDir 'src\main.rs')
    $handler = @([regex]::Matches([regex]::Match($main, 'generate_handler!\[([^\]]*)\]').Groups[1].Value, 'commands::(\w+)') | ForEach-Object { $_.Groups[1].Value })
    $build = @([regex]::Matches((Get-Content (Join-Path $tauriDir 'build.rs') -Raw), '"(\w+)",') | ForEach-Object { $_.Groups[1].Value })
    $caps = (Get-Content (Join-Path $tauriDir 'capabilities\default.json') -Raw | ConvertFrom-Json).permissions
    $allow = @($caps | Where-Object { $_ -is [string] -and $_ -like 'allow-*' } | ForEach-Object { $_.Substring(6).Replace('-', '_') })
    $ref = (@($attrs | Sort-Object) -join ',')
    $same = $true
    foreach ($pair in @(@('attributes', $attrs), @('handler', $handler), @('build.rs', $build), @('capabilities', $allow))) {
        $s = @($pair[1] | Sort-Object)
        Note ("{0,-13} {1} command(s)" -f $pair[0], $s.Count)
        if (($s -join ',') -ne $ref -or @($s | Group-Object | Where-Object { $_.Count -gt 1 }).Count) { $same = $false }
    }
    $absent = @($m11Commands | Where-Object { $attrs -notcontains $_ })
    if ($absent) { Note "M11 commands missing: $($absent -join ', ')" }
    $same -and $attrs.Count -eq 37 -and $absent.Count -eq 0
}

Check "lint: the M11 commands are called only from actions.ts (and declared in backend.ts)" {
    $calls = 'runStart|runCancel|runsList|runResults|projImport|materialLibrary|solversStatus|appEvents|appQuit'
    $bad = @()
    foreach ($f in $uiFiles) {
        $r = Rel $f
        if (@('actions.ts', 'backend.ts') -contains $r) { continue }
        foreach ($h in [regex]::Matches((UiCode $f.FullName), "\bbackend\s*\.\s*($calls)\b")) { $bad += "${r}: $($h.Value)" }
    }
    $bad | ForEach-Object { Note "CALLS A RUN COMMAND: $_" }
    $bad.Count -eq 0
}

# m11-focus, the static half (PLAN.md 4.2): nothing asks for focus, hides, minimises, moves or
# keeps a window on top; the test windows are built unfocused.
Check "lint (m11-focus): no code asks for window focus, hides, minimises, moves or pins a window" {
    $bad = @()
    $rustBad = 'set_focus|SetForegroundWindow|SetFocus\(|BringWindowToTop|SwitchToThisWindow|AllowSetForegroundWindow|\.focused\(true\)|always_on_top|\.minimize\(|\.hide\(|visible\(false\)'
    foreach ($f in Get-ChildItem (Join-Path $tauriDir 'src') -Recurse -Filter *.rs) {
        foreach ($h in [regex]::Matches((RustCode $f.FullName), $rustBad)) { $bad += "src-tauri/$($f.Name): $($h.Value)" }
    }
    $uiBad = 'setFocus\(|\.minimize\(|\.hide\(|setAlwaysOnTop|setPosition\(|setVisible\(false\)'
    foreach ($f in $uiFiles) { foreach ($h in [regex]::Matches((UiCode $f.FullName), $uiBad)) { $bad += "ui/$(Rel $f): $($h.Value)" } }
    $e2eBad = 'setWindowSize|setWindowRect|maximizeWindow|minimizeWindow|fullscreenWindow|switchWindow|switchToWindow|closeWindow'
    foreach ($f in Get-ChildItem (Join-Path $appDir 'e2e') -Recurse -File -Include *.ts | Where-Object { $_.FullName -notmatch '\\node_modules\\' }) {
        foreach ($h in [regex]::Matches((UiCode $f.FullName), $e2eBad)) { $bad += "e2e/$($f.Name): $($h.Value)" }
    }
    $conf = Get-Content (Join-Path $tauriDir 'tauri.conf.json') -Raw | ConvertFrom-Json
    foreach ($w in @($conf.app.windows)) {
        if ($w.visible -eq $false) { $bad += "tauri.conf.json: window '$($w.label)' visible: false" }
        if ($w.focus -eq $true) { $bad += "tauri.conf.json: window '$($w.label)' focus: true" }
        if ($null -ne $w.alwaysOnTop) { $bad += "tauri.conf.json: window '$($w.label)' alwaysOnTop" }
        if ($w.create -ne $false) { $bad += "tauri.conf.json: window '$($w.label)' is created by the config, not by setup" }
    }
    $main = RustCode (Join-Path $tauriDir 'src\main.rs')
    $unfocused = $main -match '!\(args\.e2e \|\| args\.selftest\.is_some\(\)\)' -and $main -match '\.focused\(focus\)'
    if (-not $unfocused) { $bad += 'main.rs: the window is not built with .focused(!(e2e || selftest))' }
    $bad | ForEach-Object { Note "FOCUS: $_" }
    Note "test windows built unfocused: $unfocused"
    $bad.Count -eq 0
}

Check "lint: no PID-based kill in app/src-tauri (the Job Object ends the solver)" {
    $bad = @()
    foreach ($f in Get-ChildItem (Join-Path $tauriDir 'src') -Recurse -Filter *.rs) {
        foreach ($h in [regex]::Matches((RustCode $f.FullName), 'taskkill|TerminateProcess|OpenProcess|sysinfo')) { $bad += "$($f.Name): $($h.Value)" }
    }
    $cargo = Get-Content (Join-Path $tauriDir 'Cargo.toml') -Raw
    if ($cargo -match '(?m)^\s*sysinfo\s*=') { $bad += 'Cargo.toml: sysinfo' }
    $bad | ForEach-Object { Note "KILLS BY PID: $_" }
    $bad.Count -eq 0
}

Check "lint: theme.css unchanged since the M10 foundation (git blob)" {
    $blob = @(cmd /c "git hash-object --path=app/ui/src/theme.css -- app/ui/src/theme.css 2>nul")[0]
    Note "theme.css blob $blob; frozen blob $themeBlob"
    $blob -eq $themeBlob
}

Check "UI: tsc typecheck (app and its node --test suites)" {
    Push-Location $appDir
    try { $code = Native 'npm run -s typecheck' (Join-Path $work 'typecheck.log') } finally { Pop-Location }
    Tail (Join-Path $work 'typecheck.log') 20
    $code -eq 0
}

Check "UI: npm test (checksum known answers; node --test ui/src/**/*.test.ts)" {
    Push-Location $appDir
    try { $code = Native 'npm test -s' (Join-Path $work 'npm-test.log') } finally { Pop-Location }
    Get-Content (Join-Path $work 'npm-test.log') | Where-Object { $_ -match 'known answers|^. (tests|pass|fail) |not ok' } | ForEach-Object { Note $_ }
    $code -eq 0
}

# ---- 2. Rust ------------------------------------------------------------------------------------
Check "app crate: unit tests" {
    $log = Join-Path $work 'cargo-test-app.log'
    $code = Native 'cargo test -q -p app' $log
    Get-Content $log | Where-Object { $_ -match '^test result|FAILED|panicked' } | ForEach-Object { Note $_ }
    $code -eq 0
}
Check "app crate: clippy -D warnings" {
    $log = Join-Path $work 'clippy-app.log'
    $code = Native 'cargo clippy -q -p app --no-deps --all-targets -- -D warnings' $log
    Tail $log 20
    $code -eq 0
}
Check "core crates: clippy -D warnings" {
    $log = Join-Path $work 'clippy-core.log'
    $code = Native 'cargo clippy -q -p simpa-core -p simpa --all-targets -- -D warnings' $log
    Tail $log 20
    $code -eq 0
}
Check "workspace: rustfmt --check" {
    $log = Join-Path $work 'fmt.log'
    $code = Native 'cargo fmt --all --check' $log
    Tail $log 10
    $code -eq 0
}
Check "fixtures: tests/fixtures/ui equal their recipe (cargo test -p simpa-core --test ui_fixtures)" {
    $log = Join-Path $work 'ui-fixtures.log'
    $code = Native 'cargo test -q -p simpa-core --test ui_fixtures' $log
    Get-Content $log | Where-Object { $_ -match '^test result|FAILED|panicked' } | ForEach-Object { Note $_ }
    $code -eq 0
}
if (-not $SkipCore) {
    # Only the targets that write under <repo>\target stay out: on a B: worktree that is the exFAT
    # drive where no build or scratch output may go. Listed on every run, never dropped silently.
    # Since M11 nothing else is skipped: the hang and the run::manager unit test are fixed (C1-C3),
    # and cli_run needs no dev-tree solver staging (C4), so none is made.
    $coreExcluded = [ordered]@{
        'simpa-core dump_helpers, gabe_golden, pbin_golden, poly_golden, tetgen_golden' = 'build the oracle into <repo>\target\oracle (tests/common/paths.rs oracle())'
        'simpa-core config_xml_write, parity_inputs'                                   = 'scratch folders under <repo>\target\test-runs (config_xml_support.rs fresh_run_dir)'
        'simpa parity_tutorials'                                                       = 'solver runs under <repo>\target\parity-bed'
    }
    $skipTargets = @('dump_helpers', 'gabe_golden', 'pbin_golden', 'poly_golden', 'tetgen_golden', 'config_xml_write',
        'parity_inputs', 'parity_tutorials', 'config_xml_support', 'parity_support')
    Check "core crates: cargo test -p simpa-core -p simpa (--no-fail-fast, 4 test threads, no skip)" {
        if (-not (Test-Path (Join-Path $SolversDir 'tetgen.exe'))) { throw "no solver build at $SolversDir (pass -SolversDir)" }
        $staged = Join-Path $target 'target\solvers\bin'
        if (Test-Path $staged) { Note "NOTE: $staged exists (M10's staging); nothing here relies on it" }
        $env:SIMPA_SOLVERS_DIR = $SolversDir
        $env:SIMPA_UPSTREAM = $Upstream
        $env:SIMPA_TEST_SCRATCH_ROOT = Join-Path $target 'test-scratch'
        # At most 4 solver processes at once: the tests run 4 at a time.
        $env:RUST_TEST_THREADS = '4'
        New-Item -ItemType Directory -Force $env:SIMPA_TEST_SCRATCH_ROOT | Out-Null
        $sel = { param($dir) @(Get-ChildItem $dir -Filter *.rs | ForEach-Object { $_.BaseName } | Where-Object { $skipTargets -notcontains $_ } | ForEach-Object { "--test $_" }) -join ' ' }
        $coreSel = & $sel (Join-Path $repo 'crates\simpa-core\tests')
        $cliSel = & $sel (Join-Path $repo 'crates\simpa\tests')
        foreach ($k in $coreExcluded.Keys) { Note "NOT RUN here: $k :: $($coreExcluded[$k])" }
        Note "solvers $SolversDir; upstream $Upstream; scratch $env:SIMPA_TEST_SCRATCH_ROOT; RUST_TEST_THREADS 4"
        $t0 = Get-Date
        $log1 = Join-Path $work 'cargo-test-core.log'
        $log2 = Join-Path $work 'cargo-test-cli.log'
        $c1 = Native "cargo test -q --no-fail-fast -p simpa-core --lib $coreSel" $log1
        $c2 = Native "cargo test -q --no-fail-fast -p simpa --bins $cliSel" $log2
        Remove-Item Env:\RUST_TEST_THREADS
        $results = @(Get-Content $log1, $log2 | Where-Object { $_ -match '^test result' })
        $failed = @(Get-Content $log1, $log2 | Where-Object { $_ -match ' --- FAILED$|^test .* ... FAILED$|^thread .* panicked at' })
        $sum = { param($re) ($results | ForEach-Object { if ($_ -match $re) { [int]$Matches[1] } } | Measure-Object -Sum).Sum }
        Note ("{0} test binaries: {1} passed, {2} failed, {3} ignored, in {4} s; logs {5}, {6}" -f $results.Count,
            (& $sum '(\d+) passed'), (& $sum '(\d+) failed'), (& $sum '(\d+) ignored'),
            [math]::Round(((Get-Date) - $t0).TotalSeconds), $log1, $log2)
        $failed | Select-Object -First 20 | ForEach-Object { Note $_ }
        $c1 -eq 0 -and $c2 -eq 0 -and $results.Count -gt 0
    }
} else { Note 'core crates: skipped (-SkipCore, for iteration only; not a gate pass)' }

}

# ---- 3-6. the release build, the prerequisites and the e2e, under one lock ----------------------
if ($runE2e) {

$exe = Join-Path $target 'release\app.exe'
$simpa = Join-Path $target 'release\simpa.exe'
$node = Join-Path $E2eHome 'node'
$lockPath = Join-Path $E2eHome 'e2e.lock'
New-Item -ItemType Directory -Force $E2eHome | Out-Null
$lock = $null
$driverExe = $null
$tauriDriver = Join-Path $env:CARGO_HOME 'bin\tauri-driver.exe'
$privateSolvers = Join-Path $work 'solvers'
$projects = Join-Path $work 'p'

# Port 4444 is fixed, and parallel packages share one app.exe and one ui/dist: every build and
# e2e run holds this lock, waiting up to 30 min for it.
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
    $v = @(cmd /c 'cargo install --list 2>nul') | Where-Object { $_ -match '^tauri-driver v' } | Select-Object -First 1
    Note "$tauriDriver ($v)"
    [bool]$v
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
        Note "installed: $((Get-ChildItem (Join-Path $node 'node_modules') -Recurse -File).Count) files under $node"
    } else { Note "current: lock sha256 $($want.Substring(0, 16))" }
    Test-Path (Join-Path $node 'node_modules\.bin\wdio.cmd')
}

Check "harness: the e2e configs and specs typecheck against it" {
    $log = Join-Path $work 'e2e-tsc.log'
    $code = Native "`"$appDir\node_modules\.bin\tsc.cmd`" -p `"$appDir\e2e\tsconfig.json`"" $log
    Tail $log 20
    $code -eq 0
}

# PLAN.md 4.1, F8: the app runs a private copy of the verified build, so "no spps.exe" means no
# process running from that copy's path; other sessions' solvers are neither counted nor touched.
Check "harness: the private solver copy, each executable the verified build by code sha256" {
    New-Item -ItemType Directory -Force $privateSolvers | Out-Null
    $manifest = Get-Content (Join-Path $repo 'solvers\manifest.json') -Raw | ConvertFrom-Json
    $ok = $true
    foreach ($name in 'spps.exe', 'classicalTheory.exe', 'tetgen.exe', 'preprocess.exe') {
        $src = Join-Path $SolversDir $name
        if (-not (Test-Path $src)) { Note "MISSING $src"; $ok = $false; continue }
        $dst = Join-Path $privateSolvers $name
        Copy-Item $src $dst -Force
        $code = Get-CodeSha256 $dst
        $want = $manifest.code_sha256.$name
        $match = $code -eq $want
        if (-not $match) { $ok = $false }
        Note ("{0,-20} code sha256 {1} {2}" -f $name, $code.Substring(0, 16), $(if ($match) { 'verified' } else { "DIFFERS from the manifest's $want" }))
    }
    $ok
}

Check "harness: the projects copied to C: (runs never land on B:), and the mesh-failure run made by the core" {
    $fx = Join-Path $repo 'tests\fixtures\ui'
    foreach ($pair in @(@('box', 'box_run.simpa'), @('long', 'box_long.simpa'), @('hall', 'hall_run.simpa'), @('meshfail', 'box_run.simpa'), @('room', 'teaching_room.simpa'))) {
        $d = Join-Path $projects $pair[0]
        New-Item -ItemType Directory -Force $d | Out-Null
        Copy-Item (Join-Path $fx $pair[1]) $d -Force
    }
    # The Interrupted control of the dock spec: a run folder with no run.json.
    New-Item -ItemType Directory -Force (Join-Path $projects 'long\runs\20260101-000000-000-spps\solve') | Out-Null
    # Gate (e)'s run: the core's real run manager with the stand-in TetGen (PLAN.md 5, F3).
    $env:SIMPA_SOLVERS_DIR = $privateSolvers
    $log = Join-Path $work 'meshfail-run.json'
    $mf = Join-Path $projects 'meshfail'
    $code = Native "`"$simpa`" run `"$mf\box_run.simpa`" --solver spps --tetgen `"$fx\tetgen_skips.bat`" --runs `"$mf\runs`" --json" $log
    $m = Get-Content $log -Raw | ConvertFrom-Json
    $codes = @($m.verdict.reasons | ForEach-Object { $_.code })
    Note "mesh-failure run: exit $code, stage $($m.stage), status $($m.verdict.status), reasons $($codes -join ', ')"
    $code -eq 4 -and $m.stage -eq 'mesh' -and $m.verdict.status -eq 'FAIL' -and $codes -contains 'tetgen_skipped_facets'
}

$junitDir = Join-Path $work 'wdio'
$present = @($Spec | Where-Object { Test-Path (Join-Path $appDir "e2e\specs\m11.$_.e2e.ts") })
$pending = @($Spec | Where-Object { $present -notcontains $_ })
foreach ($p in $pending) { Note "PENDING: app/e2e/specs/m11.$p.e2e.ts is not written yet (its ids fail below)" }
Check "e2e: wdio ran (verdict below) (-Spec $($present -join ','))" {
    if (-not $built) { throw 'no fresh app.exe from the build' }
    if (-not $script:driverExe) { throw 'no msedgedriver' }
    if ($present.Count -eq 0) { throw 'no spec file to run' }
    New-Item -ItemType Directory -Force $junitDir | Out-Null
    $env:M11_APP = $exe; $env:M11_WORK = $junitDir; $env:M11_REPO = $repo
    $env:M11_TAURI_DRIVER = $tauriDriver; $env:M11_NATIVE_DRIVER = $script:driverExe
    $env:M11_SPEC = $present -join ','; $env:M11_SOLVERS = $privateSolvers; $env:M11_P = $projects
    # msedgedriver passes the environment on to app.exe: this is where the app finds its solvers.
    $env:SIMPA_SOLVERS_DIR = $privateSolvers
    $t0 = Get-Date
    $log = Join-Path $work 'wdio.log'
    $code = Native "`"$node\node_modules\.bin\wdio.cmd`" run app/e2e/m11.conf.ts" $log
    Note "exit $code in $([math]::Round(((Get-Date) - $t0).TotalSeconds, 1)) s; log $log; the ids are judged from the junit files below"
    Get-Content $log | Where-Object { $_ -match 'receipt' } | ForEach-Object { Note $_.Trim() }
    $code -eq 0
}

} finally {
    if ($lock) { $lock.Dispose(); Note 'e2e lock released' }
}

# The verdict, from the junit files.
Check "verdict: every required id passed, 0 failures, 0 skipped" {
    $required = @($Spec | ForEach-Object { $specIds[$_] })
    # The junit reporter writes a test's title with every run of punctuation made a space, so an
    # id is matched on the same normal form, longest id first.
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
        Note ("{0,-7} {1,7:N1} s  {2,-16} {3}" -f $c.state, $c.time, $(if ($c.id) { $c.id } else { '(no id)' }), $c.name)
        if ($c.message) { Note ("          " + $c.message.Substring(0, [Math]::Min(300, $c.message.Length))) }
    }
    $passedIds = @($cases | Where-Object { $_.state -eq 'passed' } | ForEach-Object { $_.id })
    $missing = @($required | Where-Object { $passedIds -notcontains $_ })
    $failedN = @($cases | Where-Object { $_.state -eq 'FAILED' }).Count
    $skippedN = @($cases | Where-Object { $_.state -eq 'SKIPPED' }).Count
    Note "$($cases.Count) test(s); required $($required.Count); not passed: $($missing -join ', '); failures $failedN; skipped $skippedN"
    $cases.Count -gt 0 -and $missing.Count -eq 0 -and $failedN -eq 0 -and $skippedN -eq 0
}

}

# ---- 7. prior gates (PLAN.md 4.1, T19) -------------------------------------------------------------
if (-not $SkipPrior -and $Only -eq 'all') {
    Check "prior gate: m10.ps1 -SkipCore exits 0" {
        $log = Join-Path $work 'm10.log'
        $code = Native "powershell -NoProfile -ExecutionPolicy Bypass -File `"$repo\tools\gates\m10.ps1`" -SkipCore -TargetDir `"$target`" -SolversDir `"$SolversDir`"" $log
        Get-Content $log | Where-Object { $_ -match '^(PASS|FAIL) |^M10 ' } | ForEach-Object { Note $_ }
        $code -eq 0
    }
    Check "prior gate: m9.ps1 in full prints M9 PASSED" {
        $log = Join-Path $work 'm9.log'
        $code = Native "powershell -NoProfile -ExecutionPolicy Bypass -File `"$repo\tools\gates\m9.ps1`" -TargetDir `"$target`"" $log
        Get-Content $log | Where-Object { $_ -match '^(PASS|FAIL) |^M9 ' } | ForEach-Object { Note $_ }
        $code -eq 0 -and (Select-String -Path $log -Pattern '^M9 PASSED' -Quiet)
    }
} else { Note 'prior gates: not run (-SkipPrior or a partial -Only)' }

# ---- 9. file counts ----------------------------------------------------------------------------------
$files = @(Get-ChildItem $work -Recurse -File)
Write-Host "`nwork folder (C:): $work ($($files.Count) files, $([math]::Round(($files | Measure-Object Length -Sum).Sum / 1MB, 1)) MB)"
$left = @(Untracked | Where-Object { $untrackedBefore -notcontains $_ })
Write-Host "files this run left in the repository (B:): $($left.Count)"
$left | Select-Object -First 10 | ForEach-Object { Write-Host "      $_" }
if ($left.Count) { $failures += 'files left on B:' }
if (-not $fullRun) {
    if ($failures.Count) { Write-Host "M11 PARTIAL RUN (-Only $Only -Spec $($Spec -join ',')$(if ($SkipCore) { ' -SkipCore' })$(if ($SkipPrior) { ' -SkipPrior' })) FAILED: $($failures.Count) check(s)"; exit 1 }
    Write-Host "M11 PARTIAL RUN (-Only $Only -Spec $($Spec -join ',')$(if ($SkipCore) { ' -SkipCore' })$(if ($SkipPrior) { ' -SkipPrior' })): its checks passed; this is not a gate pass"; exit 0
}
if ($failures.Count) { Write-Host "M11 FAILED: $($failures.Count) check(s)"; exit 1 }
Write-Host "M11 PASSED"; exit 0
