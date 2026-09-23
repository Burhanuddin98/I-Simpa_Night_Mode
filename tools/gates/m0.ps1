# M0 gate: toolchain, workspace build, tests, lint, format, pinned upstream.
# Exits 0 only if every check passes. Run from the repo root: pwsh tools/gates/m0.ps1
$ErrorActionPreference = 'Stop'
$failures = @()
function Check($name, [scriptblock]$body) {
    try {
        $ok = & $body
        if ($ok) { Write-Host "PASS  $name" } else { Write-Host "FAIL  $name"; $script:failures += $name }
    } catch {
        Write-Host "FAIL  $name :: $($_.Exception.Message)"; $script:failures += $name
    }
}

$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Set-Location $repo
$pinned = ((Get-Content rust-toolchain.toml) -match '^channel' -replace '.*"(.*)".*', '$1')

Check "rustc matches rust-toolchain.toml ($pinned)" { (rustc --version) -like "rustc $pinned *" }
Check "cargo build --workspace" { cargo build --workspace --quiet; $LASTEXITCODE -eq 0 }
Check "cargo test --workspace" { cargo test --workspace --quiet; $LASTEXITCODE -eq 0 }
Check "cargo clippy -D warnings" { cargo clippy --workspace --all-targets --quiet -- -D warnings; $LASTEXITCODE -eq 0 }
Check "cargo fmt --check" { cargo fmt --all --check; $LASTEXITCODE -eq 0 }
Check "cli reports the pinned solver commit" { (cargo run --quiet -p simpa -- --version) -match 'upstream 929a5c8' }
# Through cmd: Windows PowerShell 5.1 turns a native program's stderr into a terminating error.
Check "cli refuses an unknown command with exit 2" { cmd /c "cargo run --quiet -p simpa -- bogus 2>nul"; $LASTEXITCODE -eq 2 }

$up = 'B:\repos\I-Simpa-upstream'
Check "upstream HEAD is 929a5c8" { (git -C $up rev-parse HEAD) -eq '929a5c8e5f590b189f29155faeff8670f3699479' }
Check "upstream has no tracked modifications" { -not (git -C $up status --porcelain --untracked-files=no) }
$untracked = git -C $up status --porcelain --untracked-files=normal | Where-Object { $_ -like '??*' }
if ($untracked) { Write-Host "WARN  upstream has untracked entries (not ours to delete): $($untracked -join ', ')" }

if ($failures.Count) { Write-Host "`nM0 FAILED: $($failures.Count) check(s)"; exit 1 }
Write-Host "`nM0 PASSED"; exit 0
