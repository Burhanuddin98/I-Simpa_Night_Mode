# M2 gate: the solver file formats, read and written by our code, agree with upstream's readers.
#   (a)(b) golden values and negative cases, per format
#   (c)    fuzz: 10,000 cases per reader, no panic, allocation within budget
#   (d)    oracle differential over every fixture and 1,000 generated models per writer format
# Run: powershell -File tools/gates/m2.ps1
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Set-Location $repo
$env:RUSTUP_HOME = "$env:USERPROFILE\.rustup"; $env:CARGO_HOME = "$env:USERPROFILE\.cargo"
$env:Path = "$env:CARGO_HOME\bin;$env:Path"
Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue
$env:CARGO_INCREMENTAL = '0'   # exFAT B: cannot hard-link the incremental cache; avoids noise
$failures = @()
function Check($name, [scriptblock]$body) {
    try {
        $ok = & $body
        if ($ok) { Write-Host "PASS  $name" } else { Write-Host "FAIL  $name"; $script:failures += $name }
    } catch {
        Write-Host "FAIL  $name :: $($_.Exception.Message)"; $script:failures += $name
    }
}
$formats = 'cbin', 'mbin', 'poly', 'tetgen', 'gabe', 'csbin', 'pbin'

# The oracle first: several golden tests cross-check against it and skip when it is absent.
Check "oracle builds from upstream's readers" {
    powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $repo 'oracle\build.ps1') | Out-Null
    $LASTEXITCODE -eq 0 -and (Test-Path (Join-Path $repo 'target\oracle\all\oracle.exe'))
}

foreach ($fmt in $formats) {
    Check "$fmt golden and negative tests" { cmd /c "cargo test -q -p simpa-core --test ${fmt}_golden 2>&1" | Out-Null; $LASTEXITCODE -eq 0 }
}
foreach ($fmt in $formats) {
    Check "$fmt fuzz (10,000 cases, no panic, allocation within budget)" {
        cmd /c "cargo test -q -p simpa-core --test ${fmt}_fuzz -- --test-threads=1 2>&1" | Out-Null; $LASTEXITCODE -eq 0
    }
}
Check "dump helpers agree with the oracle" { cmd /c "cargo test -q -p simpa-core --test dump_helpers 2>&1" | Out-Null; $LASTEXITCODE -eq 0 }
Check "clippy -D warnings" { cmd /c "cargo clippy -q --workspace --all-targets -- -D warnings 2>&1" | Out-Null; $LASTEXITCODE -eq 0 }
Check "cargo fmt --check" { cmd /c "cargo fmt --all --check 2>&1" | Out-Null; $LASTEXITCODE -eq 0 }
$diffOut = @()
$corpus = ''
Check "real solver corpus generated (SPPS x3 seeds with particles, TCR, TetGen x2)" {
    $script:corpus = (powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $repo 'tools\oracle\make-corpus.ps1') | Select-Object -Last 1)
    $LASTEXITCODE -eq 0 -and $script:corpus -and (Test-Path $script:corpus)
}
Check "oracle differential: mismatches 0" {
    $script:diffOut = powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $repo 'tools\oracle\diff.ps1') -Generated 1000 -Corpus $script:corpus 2>&1 | ForEach-Object { "$_" }
    $LASTEXITCODE -eq 0 -and ($script:diffOut -match '^mismatches: 0$')
}
$diffOut | Where-Object { $_ -match '^(fixtures|generated|checked|mismatches|artifacts|MISMATCH)' } | ForEach-Object { Write-Host "      $_" }

if ($failures.Count) { Write-Host "`nM2 FAILED: $($failures.Count) check(s)"; exit 1 }
Write-Host "`nM2 PASSED"; exit 0
