# Oracle differential for M2(d): upstream's own readers (oracle.exe) and ours (simpa dump) must
# print identical canonical dumps for every fixture, and for -Generated models per writer format
# that Rust generates and writes.
#   powershell -File tools/oracle/diff.ps1 -Generated 1000
param(
    [int]$Generated = 1000,
    # A directory of real solver output (tools/oracle/make-corpus.ps1); every file of a known
    # format in it is diffed like a fixture.
    [string]$Corpus = ''
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Set-Location $repo
$env:RUSTUP_HOME = "$env:USERPROFILE\.rustup"; $env:CARGO_HOME = "$env:USERPROFILE\.cargo"
$env:Path = "$env:CARGO_HOME\bin;$env:Path"

powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $repo 'oracle\build.ps1') | Out-Host
if ($LASTEXITCODE -ne 0) { throw 'oracle build failed' }
cmd /c "cargo build --release -p simpa 2>&1" | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'simpa build failed' }
$oracle = Join-Path $repo 'target\oracle\all\oracle.exe'
$simpa = Join-Path $repo 'target\release\simpa.exe'
$work = Join-Path $repo ('target\oracle-diff\' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
New-Item -ItemType Directory -Force $work | Out-Null

$kinds = @{ '.cbin' = 'cbin'; '.mbin' = 'mbin'; '.poly' = 'poly'; '.node' = 'tetgen'; '.ele' = 'tetgen';
    '.face' = 'tetgen'; '.neigh' = 'tetgen'; '.gabe' = 'gabe'; '.recp' = 'gabe'; '.recps' = 'gabe';
    '.gap' = 'gabe'; '.rpi' = 'gabe'; '.csbin' = 'csbin'; '.pbin' = 'pbin' }

function Get-Dump([string]$exe, [string[]]$argv) {
    $psi = New-Object System.Diagnostics.ProcessStartInfo $exe
    $psi.Arguments = ($argv | ForEach-Object { '"' + $_ + '"' }) -join ' '
    $psi.RedirectStandardOutput = $true; $psi.RedirectStandardError = $true; $psi.UseShellExecute = $false
    $p = [System.Diagnostics.Process]::Start($psi)
    $out = $p.StandardOutput.ReadToEnd(); $null = $p.StandardError.ReadToEnd(); $p.WaitForExit()
    return $out.Replace("`r`n", "`n")
}
# Every file diffed here is a valid input, so a failure on either side is a mismatch. The oracle
# cannot say WHY upstream failed, so "both failed" would prove nothing; failure kinds are covered
# by the negative golden tests instead.
function Same([string]$rust, [string]$orc) {
    if ($rust.StartsWith('error') -or $orc.StartsWith('error')) { return $false }
    return $rust -ceq $orc
}

$mismatches = 0; $checked = 0
$roots = @(Join-Path $repo 'tests\fixtures')
if ($Corpus) { $roots += $Corpus }
$fixtures = $roots | ForEach-Object { Get-ChildItem $_ -Recurse -File } | Where-Object { $kinds.ContainsKey($_.Extension.ToLower()) }
$byFormat = @{}
foreach ($f in $fixtures) {
    $byFormat[$kinds[$f.Extension.ToLower()]] = 1 + [int]$byFormat[$kinds[$f.Extension.ToLower()]]
    $fmt = $kinds[$f.Extension.ToLower()]
    $r = Get-Dump $simpa @('dump', $fmt, $f.FullName)
    $o = Get-Dump $oracle @('dump', $fmt, $f.FullName)
    $checked++
    if (-not (Same $r $o)) {
        $mismatches++
        $tag = "fixture-$checked-$fmt"
        Set-Content (Join-Path $work "$tag.rust.txt") $r -NoNewline; Set-Content (Join-Path $work "$tag.oracle.txt") $o -NoNewline
        Write-Host "MISMATCH $fmt $($f.FullName.Substring($repo.Length + 1))"
    }
}
Write-Host "fixtures: $checked checked ($(($byFormat.GetEnumerator() | Sort-Object Name | ForEach-Object { "$($_.Name) $($_.Value)" }) -join ', '))"

foreach ($fmt in 'cbin', 'mbin', 'poly') {
    $bad = 0
    for ($seed = 1; $seed -le $Generated; $seed++) {
        $file = Join-Path $work "gen.$fmt"
        $r = Get-Dump $simpa @('gen', $fmt, "$seed", $file)
        $o = Get-Dump $oracle @('dump', $fmt, $file)
        $checked++
        if (-not (Same $r $o)) {
            $mismatches++; $bad++
            if ($bad -le 5) {
                Copy-Item $file (Join-Path $work "gen-$seed.$fmt")
                Set-Content (Join-Path $work "gen-$seed-$fmt.rust.txt") $r -NoNewline; Set-Content (Join-Path $work "gen-$seed-$fmt.oracle.txt") $o -NoNewline
            }
        }
    }
    Write-Host "generated $fmt`: $Generated checked, $bad mismatched"
}
Write-Host "checked: $checked"
Write-Host "mismatches: $mismatches"
Write-Host "artifacts: $work"
if ($mismatches -ne 0) { exit 1 }
exit 0
