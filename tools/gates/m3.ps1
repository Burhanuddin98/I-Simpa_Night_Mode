# M3 gate: the project schema, the config.xml writer and the pre-launch validator.
#   (a) schema: 10,000 generated projects round-trip byte-identically; undo restores exactly
#   (b) validator: every negative fixture yields exactly its reason code; positive projects pass
#   (c) the solver as oracle: an exported cube config runs SPPS and TCR with no missing property
#   (d) attribute coverage against upstream's GUI-written tutorial 1 config: missing 0
# Plus the whole workspace test suite at normal parallelism, clippy and fmt.
# Run: powershell -File tools/gates/m3.ps1
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Set-Location $repo
$env:RUSTUP_HOME = "$env:USERPROFILE\.rustup"; $env:CARGO_HOME = "$env:USERPROFILE\.cargo"
$env:Path = "$env:CARGO_HOME\bin;$env:Path"; $env:CARGO_INCREMENTAL = '0'
Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue
$failures = @()
function Check($name, [scriptblock]$body) {
    try {
        $ok = & $body
        if ($ok) { Write-Host "PASS  $name" } else { Write-Host "FAIL  $name"; $script:failures += $name }
    } catch {
        Write-Host "FAIL  $name :: $($_.Exception.Message)"; $script:failures += $name
    }
}
function CargoTest([string]$testArgs) { cmd /c "cargo test -q -p simpa-core $testArgs 2>&1"; return $LASTEXITCODE -eq 0 }

cmd /c "cargo build -q --release -p simpa 2>&1" | Out-Null
$simpa = Join-Path $repo 'target\release\simpa.exe'
$bin = Join-Path $repo 'target\solvers\bin'
$work = Join-Path $repo ('target\gates\m3\' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
New-Item -ItemType Directory -Force $work | Out-Null

# (a)
Check "(a) schema round trip: 10,000 projects, byte-identical, bit-exact, full undo" { CargoTest '--test schema_roundtrip' | Out-Null; $LASTEXITCODE -eq 0 }

# (b)
$negDir = Join-Path $repo 'tests\fixtures\negative\schema'
$expected = Get-Content (Join-Path $negDir 'expected.json') -Raw | ConvertFrom-Json
$names = @($expected.PSObject.Properties.Name)
$matched = 0; $bad = @()
foreach ($file in $names) {
    $want = $expected.$file
    $argv = @('validate', (Join-Path $negDir $file), '--json')
    $ctxFile = Join-Path $negDir (([IO.Path]::GetFileNameWithoutExtension($file)) + '.context.json')
    if (Test-Path $ctxFile) {
        $hash = (Get-Content $ctxFile -Raw | ConvertFrom-Json).mesh_input_hash
        if ($hash) { $argv += @('--mesh-hash', $hash) }
    }
    $out = & $simpa @argv; $code = $LASTEXITCODE
    $issues = @($out | ConvertFrom-Json)
    $errors = @($issues | Where-Object { $_.severity -eq 'error' })
    $warns = @($issues | Where-Object { $_.severity -eq 'warning' -and $_.code -eq $want })
    $ok = ($code -eq 2 -and $errors.Count -eq 1 -and $errors[0].code -eq $want) -or
          ($code -eq 0 -and $errors.Count -eq 0 -and $warns.Count -ge 1)
    if ($ok) { $matched++ } else { $bad += "$file (want $want, got exit ${code}: $(($issues | ForEach-Object { "$($_.severity) $($_.code)" }) -join ', '))" }
}
Write-Host "      $matched/$($names.Count) matched"
$bad | ForEach-Object { Write-Host "      MISMATCH $_" }
Check "(b) every negative fixture yields exactly its code ($matched/$($names.Count), at least 20)" { $matched -eq $names.Count -and $names.Count -ge 20 }
Check "(b) positive projects validate with no error" {
    $all = $true
    foreach ($p in Get-ChildItem (Join-Path $repo 'tests\fixtures\projects') -Filter *.simpa) {
        & $simpa validate $p.FullName | Out-Null
        if ($LASTEXITCODE -ne 0) { Write-Host "      $($p.Name) exit $LASTEXITCODE"; $all = $false }
    }
    $all
}
Check "(b) validator fixture and project suites" { (CargoTest '--test validate_fixtures') -and (CargoTest '--test validate_projects') -and (CargoTest '--test validate_contract_docs') }

# (c)
foreach ($solver in 'spps', 'tcr') {
    $dir = Join-Path $work "cube-$solver"
    Check "(c) $solver runs the exported cube config with no missing property" {
        & $simpa export-config (Join-Path $repo 'tests\fixtures\projects\cube.simpa') $dir --solver $solver | Out-Null
        if ($LASTEXITCODE -ne 0) { throw 'export-config failed' }
        Copy-Item (Join-Path $repo 'tests\fixtures\upstream\lib_interface\cube_mesh.mbin') (Join-Path $dir 'tetramesh.mbin')
        $exe = if ($solver -eq 'spps') { 'spps.exe' } else { 'classicalTheory.exe' }
        $p = Start-Process (Join-Path $bin $exe) -ArgumentList 'config.xml' -WorkingDirectory $dir -NoNewWindow -Wait -PassThru `
            -RedirectStandardOutput (Join-Path $dir '_stdout.txt') -RedirectStandardError (Join-Path $dir '_stderr.txt')
        $stdout = Get-Content (Join-Path $dir '_stdout.txt')
        $missing = @($stdout | Where-Object { $_ -match "Xml Property .* doesn't exist" })
        $missing | ForEach-Object { Write-Host "      $_" }
        $done = if ($solver -eq 'spps') { @($stdout | Where-Object { $_ -match '^End of calculation' }).Count -ge 1 } else { $true }
        $p.ExitCode -eq 0 -and $missing.Count -eq 0 -and $done
    }
}
Check "(c) config writer suites (write, solver)" { (CargoTest '--test config_xml_write') -and (CargoTest '--test config_xml_solver') }

# (d)
$cov = @()
Check "(d) attribute coverage against upstream tutorial 1: missing 0" {
    $script:cov = cmd /c "cargo test -q -p simpa-core --test config_xml_import -- --nocapture 2>&1"
    $LASTEXITCODE -eq 0 -and ($script:cov -match 'missing: 0')
}
$cov | Where-Object { $_ -match 'missing:' } | Select-Object -First 3 | ForEach-Object { Write-Host "      $_" }

# Whole suite, at normal parallelism (the fuzz allocation counters are per thread now).
Check "workspace tests, parallel" { cmd /c "cargo test -q --workspace 2>&1" | Out-Null; $LASTEXITCODE -eq 0 }
Check "clippy -D warnings" { cmd /c "cargo clippy -q --workspace --all-targets -- -D warnings 2>&1" | Out-Null; $LASTEXITCODE -eq 0 }
Check "cargo fmt --check" { cmd /c "cargo fmt --all --check 2>&1" | Out-Null; $LASTEXITCODE -eq 0 }

Write-Host "`nrun folders: $work"
if ($failures.Count) { Write-Host "M3 FAILED: $($failures.Count) check(s)"; exit 1 }
Write-Host "M3 PASSED"; exit 0
