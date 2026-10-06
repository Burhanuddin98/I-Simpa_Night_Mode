# M10 gate: Concept B, Geometry, Materials, Sources & receivers
# (docs/investigations/2026-09-29-m10/PLAN.md, sections 3 and 4).
#   (a) raw elmia.ply: a Console FAIL line with 'open' and '955'; highlighted faces > 0; Run disabled
#   (b) the corrected hall: the INFO line 'Closed volume, 0 self-intersections'; Materials '0 / 10'
#   (c) a pasted materials_6x6.tsv saves byte-identical to materials_6x6.expected.json
#   (d) a double-click on the box ceiling selects 2 faces, in group Ceiling
#   (e) a receiver outside shows RECEIVER_OUTSIDE, project unchanged; 'a/b' is LABEL_UNSAFE
#   (f) 50 edits then 50 undos (Ctrl+Z) save byte-identical to the original
#   (g) document.querySelectorAll('canvas').length == 1
#   (h) added by the plan, not in the gate text: no solver-computed acoustic number on screen
#       (narrowed in M12 to "outside the Results step": m12.ps1, PLAN.md P1 item 4)
# Each is a WebdriverIO test id (m10-a-console, ..., m10-h) run against the release app by
# `wdio run app/e2e/m10.conf.ts`; every id must pass, none skipped, nothing failing. Plus the
# static checks (M9's static checks, the command inventory across four places with M10's 28
# commands present, the UI lints, theme.css frozen), the Rust checks and the fixtures' recipe.
#
# M11 (docs/investigations/2026-09-29-m11/PLAN.md 4.5), none a loosening: the inventory no longer
# pins the count at 28 (M11 adds 9) but requires M10's 28 by name and the four sets equal; the core
# step runs the two tests M10 had to skip (both fixed in M11); the e2e step passes only on wdio's
# exit 0 (M10 MINOR A-4); and the app gets the solver build too, since Run is wired and the
# teaching room's control now reads Run enabled.
#
# Windows PowerShell 5.1 (pwsh is not installed on Grace; the gate text says pwsh). Grace-local:
# the e2e opens windows, and needs tauri-driver, the msedgedriver of the live WebView2 runtime,
# and the raw hall from the upstream checkout.
#
# Run: powershell -File tools/gates/m10.ps1 [-TargetDir C:\tmp\nm-target] [-E2eHome C:\tmp\nm-e2e]
#        [-Only all|static|e2e] [-Spec smoke,shell,viewport,materials,scene] [-FetchDriver] [-SkipCore]
#        [-SolversDir <dir>] [-ScreensDir <dir>]
# -Spec screens is not a gate spec: it saves the three steps' screenshots into -ScreensDir.
# Partial runs (-Only other than all, a -Spec subset, -SkipCore) never print "M10 PASSED".
param(
    [string]$TargetDir = 'C:\tmp\nm-target',
    [string]$E2eHome = 'C:\tmp\nm-e2e',
    [ValidateSet('all', 'static', 'e2e')][string]$Only = 'all',
    [string[]]$Spec = @('smoke', 'shell', 'viewport', 'materials', 'scene'),
    [string]$ElmiaRaw = 'B:\repos\I-Simpa-upstream\src\isimpa\resources\doc\tutorial\tutorial 2\elmia.ply',
    # The core crates' tests: the M1 solver build (a copy on C: in a worktree that has none),
    # upstream's tree, and a scratch root off B:.
    # Default: $SIMPA_SOLVERS_DIR, else the repo's own build, else the foundation's copy on C:.
    [string]$SolversDir = $(if ($env:SIMPA_SOLVERS_DIR) { $env:SIMPA_SOLVERS_DIR } else { '' }),
    [string]$SolversFallback = 'C:\tmp\nm-m10-solvers\bin',
    [string]$Upstream = 'B:\repos\I-Simpa-upstream',
    [switch]$FetchDriver,
    [switch]$SkipCore,
    # Where -Spec screens saves its PNGs (default: the run's work folder).
    [string]$ScreensDir = ''
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Set-Location $repo
$env:RUSTUP_HOME = "$env:USERPROFILE\.rustup"; $env:CARGO_HOME = "$env:USERPROFILE\.cargo"
$env:Path = "$env:CARGO_HOME\bin;$env:Path"; $env:CARGO_INCREMENTAL = '0'
if (-not [IO.Path]::IsPathRooted($TargetDir)) { throw "-TargetDir must be absolute (C:\tmp\nm-target): $TargetDir" }
$target = [IO.Path]::GetFullPath($TargetDir)
# Forward slashes: Git Bash has mangled backslashes in this variable into a folder in the repo.
$env:CARGO_TARGET_DIR = $target.Replace('\', '/')
$appDir = Join-Path $repo 'app'
$uiDir = Join-Path $appDir 'ui\src'
$tauriDir = Join-Path $appDir 'src-tauri'
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$work = Join-Path $target "gates\m10\$stamp"
New-Item -ItemType Directory -Force $work | Out-Null
$runStatic = $Only -eq 'all' -or $Only -eq 'static'
$runE2e = $Only -eq 'all' -or $Only -eq 'e2e'
$Spec = @($Spec | ForEach-Object { $_ -split ',' } | ForEach-Object { $_.Trim() } | Where-Object { $_ })

# The test ids each spec file must pass (PLAN.md 3; the smoke test is the harness's own).
$specIds = [ordered]@{
    smoke     = @('m10-smoke')
    shell     = @('m10-a-console', 'm10-b-console', 'm10-f', 'm10-h')
    viewport  = @('m10-a-highlight', 'm10-d', 'm10-g')
    materials = @('m10-c')
    scene     = @('m10-a-run', 'm10-b-materials', 'm10-e-outside', 'm10-e-label')
    # Not a gate spec: the screenshots (app/e2e/specs/m10.screens.e2e.ts), no required id.
    screens   = @()
}
$gateSpecs = @('smoke', 'shell', 'viewport', 'materials', 'scene')
foreach ($s in $Spec) { if (-not $specIds.Contains($s)) { throw "unknown -Spec '$s': one of $($specIds.Keys -join ', ')" } }
$fullRun = $Only -eq 'all' -and -not $SkipCore -and (@($gateSpecs | Where-Object { $Spec -notcontains $_ }).Count -eq 0)

# theme.css is pinned (M10 PLAN.md 2.4 rule 4; re-pinned for the glass layout, decision-log row 50): its git blob, which does
# not depend on the checkout's line endings.
$themeBlob = 'b734759d5d6333f22256246d0827efdefaa0b62e'

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

# ---- 1. static ---------------------------------------------------------------------------------
if ($runStatic) {

Check "M9 static checks (m9.ps1 -Only static -TargetDir $target)" {
    $log = Join-Path $work 'm9-static.log'
    $code = Native "powershell -NoProfile -ExecutionPolicy Bypass -File `"$repo\tools\gates\m9.ps1`" -Only static -TargetDir `"$target`"" $log
    Get-Content $log | Where-Object { $_ -match '^(PASS|FAIL) ' } | ForEach-Object { Note $_ }
    $code -eq 0
}

# M9 (g) compares the attributes, the handler and build.rs; a command missing from the
# capability file fails only at run time, so it joins the comparison here. M10's 28 commands must
# all be there by name; later milestones add to the set (M11: 9), so the count is not pinned.
$m10Commands = @('ping', 'panic_probe', 'panic_probe_unguarded', 'unguarded_panic_runs', 'bench_prepare',
    'bench_take', 'bench_clear', 'run_events_probe', 'exact_float_probe', 'project_new', 'project_load_text',
    'project_open', 'project_info', 'project_json', 'project_apply', 'project_undo', 'project_redo',
    'app_startup', 'selftest_report', 'scene_state', 'scene_new', 'scene_open', 'model_import', 'project_save',
    'edit_apply', 'edit_undo', 'edit_redo', 'scene_mesh')
Check "command inventory: M10's 28 commands present, the same set in the attributes, generate_handler!, build.rs and capabilities" {
    $strip = { param($p) [regex]::Replace((Get-Content $p -Raw), '//[^\n]*', '') }
    $attrs = @()
    foreach ($f in Get-ChildItem (Join-Path $tauriDir 'src') -Filter *.rs) {
        $t = & $strip $f.FullName
        $attrs += @([regex]::Matches($t, '#\[tauri::command\b[^\]]*\]\s*(?:#\[[^\]]*\]\s*)*pub\s+async\s+fn\s+(\w+)') | ForEach-Object { $_.Groups[1].Value })
    }
    $main = & $strip (Join-Path $tauriDir 'src\main.rs')
    $handler = @([regex]::Matches([regex]::Match($main, 'generate_handler!\[([^\]]*)\]').Groups[1].Value, 'commands::(\w+)') | ForEach-Object { $_.Groups[1].Value })
    $build = @([regex]::Matches((Get-Content (Join-Path $tauriDir 'build.rs') -Raw), '"(\w+)",') | ForEach-Object { $_.Groups[1].Value })
    $caps = (Get-Content (Join-Path $tauriDir 'capabilities\default.json') -Raw | ConvertFrom-Json).permissions
    $allow = @($caps | Where-Object { $_ -is [string] -and $_ -like 'allow-*' } | ForEach-Object { $_.Substring(6).Replace('-', '_') })
    $sets = [ordered]@{ attributes = $attrs; handler = $handler; 'build.rs' = $build; capabilities = $allow }
    $ref = (@($attrs | Sort-Object) -join ',')
    $same = $true
    foreach ($k in $sets.Keys) {
        $s = @($sets[$k] | Sort-Object)
        $dup = @($s | Group-Object | Where-Object { $_.Count -gt 1 } | ForEach-Object { $_.Name })
        Note ("{0,-13} {1} command(s){2}" -f $k, $s.Count, $(if ($dup) { "; twice: $($dup -join ', ')" } else { '' }))
        if (($s -join ',') -ne $ref -or $dup) { $same = $false }
    }
    if (-not $same) {
        foreach ($k in $sets.Keys) {
            $missing = @($attrs | Where-Object { $sets[$k] -notcontains $_ })
            $extra = @($sets[$k] | Where-Object { $attrs -notcontains $_ })
            if ($missing -or $extra) { Note "${k}: missing $($missing -join ', '); extra $($extra -join ', ')" }
        }
    }
    $absent = @($m10Commands | Where-Object { $attrs -notcontains $_ })
    Note "M10's $($m10Commands.Count) named commands: $(if ($absent) { "MISSING $($absent -join ', ')" } else { 'all present' }); $($attrs.Count) in all"
    $same -and $m10Commands.Count -eq 28 -and $absent.Count -eq 0
}

# The UI's ownership rules (PLAN.md 2.4), read on the code with comments removed.
function UiCode([string]$path) {
    $t = Get-Content $path -Raw
    $t = [regex]::Replace($t, '/\*[\s\S]*?\*/', '')
    [regex]::Replace($t, '(?m)(^|[^:"''`])//[^\n]*', '$1')
}
$uiFiles = @(Get-ChildItem $uiDir -Recurse -File -Include *.ts, *.tsx | Where-Object { $_.FullName -notmatch '\\bindings\\' -and $_.Name -notmatch '\.test\.ts$' })
function Rel([IO.FileInfo]$f) { $f.FullName.Substring($uiDir.Length + 1).Replace('\', '/') }

Check "lint: backend is called only from actions.ts, selftest.ts and App.tsx (boot)" {
    $bad = @()
    foreach ($f in $uiFiles) {
        $r = Rel $f
        if (@('actions.ts', 'selftest.ts', 'App.tsx', 'backend.ts') -contains $r) { continue }
        $hits = [regex]::Matches((UiCode $f.FullName), '\bbackend\s*\.\s*\w+')
        foreach ($h in $hits) { $bad += "${r}: $($h.Value)" }
    }
    $bad | ForEach-Object { Note "CALLS THE BACKEND: $_" }
    Note "$($uiFiles.Count) UI files read; $($bad.Count) call(s) outside the three"
    $bad.Count -eq 0
}

Check "lint: projectApply (M9's unchecked apply) only in selftest.ts" {
    $bad = @($uiFiles | Where-Object { @('selftest.ts', 'backend.ts') -notcontains (Rel $_) -and (UiCode $_.FullName) -match '\bprojectApply\b' } | ForEach-Object { Rel $_ })
    $bad | ForEach-Object { Note "USES projectApply: $_" }
    $bad.Count -eq 0
}

Check "lint: a canvas is created only under features/viewport/ and in gpu.ts" {
    $bad = @()
    foreach ($f in $uiFiles) {
        $r = Rel $f
        if ($r -like 'features/viewport/*' -or $r -eq 'gpu.ts') { continue }
        $t = UiCode $f.FullName
        if ($t -match '<canvas\b' -or $t -match 'createElement(NS)?\s*\([^)]*[''"`]canvas[''"`]' -or $t -match '\bOffscreenCanvas\b') { $bad += $r }
    }
    $bad | ForEach-Object { Note "MAKES A CANVAS: $_" }
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
Check "app crate: rustfmt --check" {
    $log = Join-Path $work 'fmt-app.log'
    $code = Native 'cargo fmt -p app --check' $log
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
    # Targets that write under <repo>\target, which on a B: worktree is the exFAT drive where
    # no build or scratch output may go (B:'s old target is corrupt; 47 GB of scratch once piled
    # up there). They are the core's as they stand, untouched by M10, and run where the repo's
    # target folder is on a fixed drive. Listed on every run, never dropped silently.
    $coreExcluded = [ordered]@{
        'simpa-core dump_helpers, gabe_golden, pbin_golden, poly_golden, tetgen_golden' = 'build the oracle into <repo>\target\oracle (tests/common/paths.rs oracle())'
        'simpa-core config_xml_write, parity_inputs'                                   = 'scratch folders under <repo>\target\test-runs (config_xml_support.rs fresh_run_dir)'
        'simpa parity_tutorials'                                                       = 'solver runs under <repo>\target\parity-bed'
    }
    $skipTargets = @('dump_helpers', 'gabe_golden', 'pbin_golden', 'poly_golden', 'tetgen_golden', 'config_xml_write',
        'parity_inputs', 'parity_tutorials', 'config_xml_support', 'parity_support')
    Check "core crates: cargo test -p simpa-core -p simpa (--no-fail-fast)" {
        $solvers = if ($SolversDir) { $SolversDir }
            elseif (Test-Path (Join-Path $repo 'target\solvers\bin\tetgen.exe')) { Join-Path $repo 'target\solvers\bin' }
            else { $SolversFallback }
        if (-not (Test-Path (Join-Path $solvers 'tetgen.exe'))) { throw "no solver build at $solvers (pass -SolversDir, or set SIMPA_SOLVERS_DIR)" }
        # The CLI's last fallback is the nearest <ancestor>\target\solvers\bin above simpa.exe
        # (run::manager candidates), which in the repo layout is the repo's own build. With cargo
        # building into $target, the same place is $target\target\solvers\bin. cli_run's scratch
        # test relies on it: its failing arm points $SIMPA_SOLVERS_DIR at an empty folder and
        # expects the run to get as far as the test's own preprocess.exe lookup. Stage the build
        # there, as the repo layout has it; nothing in any test changes.
        $fallback = Join-Path $target 'target\solvers\bin'
        New-Item -ItemType Directory -Force $fallback | Out-Null
        $staged = 0
        foreach ($exe in Get-ChildItem $solvers -Filter *.exe) {
            $dst = Join-Path $fallback $exe.Name
            if (-not (Test-Path $dst) -or (Sha256 $dst) -ne (Sha256 $exe.FullName)) { Copy-Item $exe.FullName $dst -Force; $staged++ }
        }
        Note "CLI fallback staged at ${fallback}: $((Get-ChildItem $fallback -Filter *.exe).Count) exe(s), $staged copied now"
        $env:SIMPA_SOLVERS_DIR = $solvers
        # The TetGen 1.6.0 reference two tests refuse by name: $SIMPA_TETGEN160 when set (a solver
        # build without solvers/build.ps1's layout beside it, such as C:\tmp\nm-m8a-solvers, has
        # none), else beside the solver build.
        # The build tree is build\, or build-p<hash>\ when solvers/build.ps1 applied patches\: the first that holds the file.
        if (-not $env:SIMPA_TETGEN160) {
            $t160 = @(Get-ChildItem (Split-Path -Parent $solvers) -Directory -Filter 'build*' -ErrorAction SilentlyContinue | Sort-Object Name | ForEach-Object { Join-Path $_.FullName 'src\tetgen\Release\tetgen.exe' } | Where-Object { Test-Path $_ } | Select-Object -First 1)
            $env:SIMPA_TETGEN160 = if ($t160.Count) { $t160[0] } else { Join-Path (Split-Path -Parent $solvers) 'build\src\tetgen\Release\tetgen.exe' }
        }
        $env:SIMPA_UPSTREAM = $Upstream
        $env:SIMPA_TEST_SCRATCH_ROOT = Join-Path $target 'test-scratch'
        # At most 4 solver processes at once (M11 PLAN.md T18): the tests run 4 at a time.
        $env:RUST_TEST_THREADS = '4'
        New-Item -ItemType Directory -Force $env:SIMPA_TEST_SCRATCH_ROOT | Out-Null
        $sel = { param($dir) @(Get-ChildItem $dir -Filter *.rs | ForEach-Object { $_.BaseName } | Where-Object { $skipTargets -notcontains $_ } | ForEach-Object { "--test $_" }) -join ' ' }
        $coreSel = & $sel (Join-Path $repo 'crates\simpa-core\tests')
        $cliSel = & $sel (Join-Path $repo 'crates\simpa\tests')
        foreach ($k in $coreExcluded.Keys) { Note "NOT RUN here: $k :: $($coreExcluded[$k])" }
        Note "solvers $solvers; TetGen 1.6.0 reference $env:SIMPA_TETGEN160 (present: $(Test-Path $env:SIMPA_TETGEN160)); upstream $Upstream; scratch $env:SIMPA_TEST_SCRATCH_ROOT; RUST_TEST_THREADS 4"
        $t0 = Get-Date
        $log1 = Join-Path $work 'cargo-test-core.log'
        $log2 = Join-Path $work 'cargo-test-cli.log'
        # No skip since M11: the log test writes under the scratch root, and the stale-file test
        # holds its file open with no sharing instead of trusting a read-only attribute (M11 C1-C3).
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

# ---- 3-7. the release build and the e2e, under one lock ------------------------------------------
if ($runE2e) {

$exe = Join-Path $target 'release\app.exe'
$node = Join-Path $E2eHome 'node'
$lockPath = Join-Path $E2eHome 'e2e.lock'
New-Item -ItemType Directory -Force $E2eHome | Out-Null
$lock = $null
$driverExe = $null
$tauriDriver = Join-Path $env:CARGO_HOME 'bin\tauri-driver.exe'

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
Check "build: npx tauri build --no-bundle (custom protocol, ui/dist embedded)" {
    if (-not $script:lock) { throw 'no lock' }
    $t0 = Get-Date
    $log = Join-Path $work 'tauri-build.log'
    Push-Location $appDir
    try { $code = Native 'npx --no-install tauri build --no-bundle' $log } finally { Pop-Location }
    Tail $log 4
    $fresh = (Test-Path $exe) -and (Get-Item $exe).LastWriteTime -ge $t0.AddSeconds(-1)
    Note "exit $code in $([math]::Round(((Get-Date) - $t0).TotalSeconds, 1)) s; app.exe rebuilt: $fresh"
    $script:built = $code -eq 0 -and $fresh
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

Check "harness: the e2e config and specs typecheck against it" {
    $log = Join-Path $work 'e2e-tsc.log'
    $code = Native "`"$appDir\node_modules\.bin\tsc.cmd`" -p `"$appDir\e2e\tsconfig.json`"" $log
    Tail $log 20
    $code -eq 0
}

Check "harness: the raw hall is present (gate a)" {
    Note $ElmiaRaw
    Test-Path -LiteralPath $ElmiaRaw
}

$junitDir = Join-Path $work 'wdio'
# The solver build the app finds (msedgedriver passes the environment on to app.exe): Run is
# wired since M11, and the teaching room's control reads it enabled only when the solvers are the
# verified build.
$e2eSolvers = if ($SolversDir) { $SolversDir }
    elseif (Test-Path (Join-Path $repo 'target\solvers\bin\tetgen.exe')) { Join-Path $repo 'target\solvers\bin' }
    else { $SolversFallback }
Check "e2e: wdio ran (verdict below) (-Spec $($Spec -join ','))" {
    if (-not $built) { throw 'no fresh app.exe from the build' }
    if (-not $script:driverExe) { throw 'no msedgedriver' }
    New-Item -ItemType Directory -Force $junitDir | Out-Null
    $env:M10_APP = $exe; $env:M10_WORK = $junitDir; $env:M10_REPO = $repo
    $env:M10_TAURI_DRIVER = $tauriDriver; $env:M10_NATIVE_DRIVER = $script:driverExe
    $env:M10_ELMIA_RAW = $ElmiaRaw; $env:M10_SPEC = $Spec -join ','
    $env:M10_SCREENS = if ($ScreensDir) { [IO.Path]::GetFullPath($ScreensDir) } else { Join-Path $work 'screens' }
    $env:SIMPA_SOLVERS_DIR = $e2eSolvers
    Note "the app finds its solvers in $e2eSolvers"
    $t0 = Get-Date
    $log = Join-Path $work 'wdio.log'
    $code = Native "`"$node\node_modules\.bin\wdio.cmd`" run app/e2e/m10.conf.ts" $log
    Note "exit $code in $([math]::Round(((Get-Date) - $t0).TotalSeconds, 1)) s; log $log; the ids are judged from the junit files below"
    # Passes only when wdio itself exited 0 (M10 MINOR A-4: a PASS line on exit 1 is not one).
    $code -eq 0
}

} finally {
    if ($lock) { $lock.Dispose(); Note 'e2e lock released' }
}

# ---- 7. the verdict, from the junit files ---------------------------------------------------------
Check "verdict: every required id passed, 0 failures, 0 skipped" {
    $required = @($Spec | ForEach-Object { $specIds[$_] })
    # The junit reporter writes a test's title with every run of punctuation made a space
    # ("m10-a-console: the raw..." becomes "m10 a console the raw..."), so an id is matched on
    # the same normal form, longest id first.
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
        $line = "{0,-7} {1,7:N1} s  {2,-16} {3}" -f $c.state, $c.time, $(if ($c.id) { $c.id } else { '(no id)' }), $c.name
        Note $line
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

# ---- 8. -------------------------------------------------------------------------------------------
$files = @(Get-ChildItem $work -Recurse -File)
Write-Host "`nwork folder: $work ($($files.Count) files, $([math]::Round(($files | Measure-Object Length -Sum).Sum / 1MB, 1)) MB)"
if (-not $fullRun) {
    if ($failures.Count) { Write-Host "M10 PARTIAL RUN (-Only $Only -Spec $($Spec -join ',')$(if ($SkipCore) { ' -SkipCore' })) FAILED: $($failures.Count) check(s)"; exit 1 }
    Write-Host "M10 PARTIAL RUN (-Only $Only -Spec $($Spec -join ',')$(if ($SkipCore) { ' -SkipCore' })): its checks passed; this is not a gate pass"; exit 0
}
if ($failures.Count) { Write-Host "M10 FAILED: $($failures.Count) check(s)"; exit 1 }
Write-Host "M10 PASSED"; exit 0
