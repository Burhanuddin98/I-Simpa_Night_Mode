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
# Every file diffed here is a valid input (the invalid-by-design fixtures below are judged apart),
# so a failure on either side is a mismatch. The oracle
# cannot say WHY upstream failed, so "both failed" would prove nothing; failure kinds are covered
# by the negative golden tests instead.
function Same([string]$rust, [string]$orc) {
    if ($rust.StartsWith('error') -or $orc.StartsWith('error')) { return $false }
    return $rust -ceq $orc
}

# The committed fixtures that are invalid on purpose, the M5 and M6 negative cases. For these the
# claim is not parity: our reader must REFUSE each one (its dump an error), whatever upstream's
# reader makes of it. A listed file our reader accepts is a mismatch, and a listed file that is
# gone stops the run, so the list cannot hide a valid file or go stale. Paths are repo-relative.
$invalidByDesign = [ordered]@{
    'tests\fixtures\meshes\broken_hall\model.1.face'     = "TetGen's partial write after exit 3: the header declares 641 faces and 796 rows follow. Upstream's reader stops at 641; ours refuses the extra rows"
    'tests\fixtures\meshes\broken_hall\model.poly'       = "facets without markers (facet header '1'): upstream's poly reader reads 0 faces from it; ours refuses the short facet header"
    'tests\fixtures\runs\spps_unreadable_mesh\mesh.cbin' = 'cut to its first 100 bytes (fixture spps_unreadable_mesh)'
}
foreach ($rel in $invalidByDesign.Keys) {
    if (-not (Test-Path -LiteralPath (Join-Path $repo $rel))) { throw "invalid-by-design fixture $rel is listed but missing: update the list in tools/oracle/diff.ps1" }
}

$mismatches = 0; $checked = 0; $refused = 0
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
    $rel = $f.FullName.Substring($repo.Length + 1)
    if ($invalidByDesign.Contains($rel)) {
        $upstream = if ($o.StartsWith('error')) { 'refused it too' } else { 'read it' }
        if ($r.StartsWith('error')) {
            $refused++
            Write-Host "refused as designed: $fmt $rel (upstream's reader $upstream): $($invalidByDesign[$rel])"
            continue
        }
        $mismatches++
        Write-Host "MISMATCH $fmt $rel is invalid by design, but our reader accepts it"
        continue
    }
    if (-not (Same $r $o)) {
        $mismatches++
        $tag = "fixture-$checked-$fmt"
        Set-Content (Join-Path $work "$tag.rust.txt") $r -NoNewline; Set-Content (Join-Path $work "$tag.oracle.txt") $o -NoNewline
        Write-Host "MISMATCH $fmt $rel"
    }
}
Write-Host "fixtures: $checked checked ($(($byFormat.GetEnumerator() | Sort-Object Name | ForEach-Object { "$($_.Name) $($_.Value)" }) -join ', ')), $refused of them refused as invalid by design"
if ($refused -ne $invalidByDesign.Count) { $mismatches++; Write-Host "MISMATCH $refused of $($invalidByDesign.Count) invalid-by-design fixtures were refused" }

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
