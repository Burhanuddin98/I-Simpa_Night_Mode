# M13: the Windows installer. Stages the payload with tools\devtools\payload.ps1 (the files the portable
# package ships, nothing listed twice), writes the explicit install and uninstall file lists for
# night-mode.nsi, compiles it with NSIS, and writes the installer's size and sha256 beside it.
#
# Version (decision 86): Cargo's [workspace.package] version, the one About shows (CARGO_PKG_VERSION);
# tauri.conf.json and app.exe's own version resource must say the same, and app.exe must carry the
# commit being packaged (build_identity.rs), or nothing is built: a stale app.exe never ships.
#
# Run (Windows PowerShell 5.1):
#   powershell -File tools\installer\build-installer.ps1 [-Solvers C:\tmp\nm-solvers-gpu]
#     [-App C:\tmp\nm-target\release\app.exe] [-Out B:\repos\I-Simpa_Night_Mode\.out\m13]
#     [-Makensis <makensis.exe>] [-WebView2 <MicrosoftEdgeWebview2Setup.exe>]
# NSIS 3.11 and the bootstrapper are build inputs kept under -Out\tools (gitignored), not in the repo.
# Each build gets its own folder, -Out\<version>-<commit>-<stamp>\: the payload, the file lists, the
# installer, its .sha256, installer.json and the makensis log.
param(
  [string]$Solvers = 'C:\tmp\nm-solvers-gpu',
  [string]$App = 'C:\tmp\nm-target\release\app.exe',
  [string]$Out = 'B:\repos\I-Simpa_Night_Mode\.out\m13',
  [string]$Makensis = '',
  [string]$WebView2 = ''
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
. "$repo\tools\devtools\payload.ps1"
if (-not $Makensis) { $Makensis = "$Out\tools\nsis\nsis-3.11\makensis.exe" }
if (-not $WebView2) { $WebView2 = "$Out\tools\dl-webview2\MicrosoftEdgeWebview2Setup.exe" }
foreach ($f in $Makensis, $WebView2, $App) { if (-not (Test-Path $f)) { throw "missing: $f" } }

# --- the version and the commit (decision 86)
$cargo = Get-Content "$repo\Cargo.toml" -Raw
if ($cargo -notmatch '(?ms)^\[workspace\.package\].*?^version\s*=\s*"(\d+\.\d+\.\d+)"') { throw 'Cargo.toml has no [workspace.package] version' }
$version = $Matches[1]
$tauri = (Get-Content "$repo\app\src-tauri\tauri.conf.json" -Raw | ConvertFrom-Json).version
if ($tauri -ne $version) { throw "tauri.conf.json says $tauri, Cargo.toml $version" }
$exeVersion = (Get-Item $App).VersionInfo.ProductVersion
if ($exeVersion -ne $version) { throw "app.exe's version resource says '$exeVersion', Cargo.toml ${version}: rebuild app.exe" }
$commit = (& git -C $repo rev-parse --short=12 HEAD).Trim()
if (& git -C $repo status --porcelain --untracked-files=no) { $commit = "$commit with uncommitted changes" }
# About's commit is compiled in (SIMPA_BUILD_COMMIT): the exe must hold this one.
$bytes = [IO.File]::ReadAllBytes($App)
$ascii = [Text.Encoding]::ASCII.GetString($bytes)
if ($ascii.IndexOf($commit, [StringComparison]::Ordinal) -lt 0) { throw "app.exe does not carry commit '$commit' (About would name another build): rebuild it from this tree" }
$short = $commit.Substring(0, 7)

# --- the bootstrapper the installer carries: Microsoft's, signed
$sig = Get-AuthenticodeSignature $WebView2
if ($sig.Status -ne 'Valid' -or $sig.SignerCertificate.Subject -notmatch '^CN=Microsoft Corporation,') { throw "$WebView2 is not signed by Microsoft ($($sig.Status))" }

# --- the payload, staged fresh in a folder of this build's own (nothing earlier is deleted or reused)
$build = "$Out\$version-$short-$(Get-Date -Format 'yyyyMMdd-HHmmss')"
if (Test-Path $build) { throw "$build exists" }
$payload = "$build\payload"
Copy-NightModePayload -Dest $payload -Repo $repo -Solvers $Solvers -App $App
$files = @(Get-ChildItem $payload -Recurse -File | Sort-Object FullName)
$sizeKb = [int][Math]::Ceiling((($files | Measure-Object Length -Sum).Sum) / 1KB)

# --- the explicit lists: one File and one Delete per file, folders removed deepest first and only if empty
# A '$' in a file or folder name is escaped as NSIS's '$$'; the $INSTDIR prefix is the variable, never escaped.
$q = { param($s) $s.Replace('$', '$$') }
$inst = New-Object System.Collections.Generic.List[string]
$uninst = New-Object System.Collections.Generic.List[string]
$inst.Add('!macro PAYLOAD_INSTALL')
$uninst.Add('!macro PAYLOAD_UNINSTALL')
$dirs = @{}
foreach ($g in $files | Group-Object { $_.DirectoryName }) {
  $rel = $g.Name.Substring($payload.Length).TrimStart('\')
  $target = '$INSTDIR' + $(if ($rel) { '\' + (& $q $rel) } else { '' })
  if ($rel) { $dirs[$rel] = $true }
  $inst.Add("  SetOutPath `"$target`"")
  foreach ($f in $g.Group) {
    $inst.Add("  File `"$(& $q $f.FullName)`"")
    $uninst.Add("  Delete `"$target\$(& $q $f.Name)`"")
  }
}
foreach ($d in $dirs.Keys | Sort-Object { $_.Split('\').Count } -Descending) { $uninst.Add("  RMDir `"`$INSTDIR\$(& $q $d)`"") }
$inst.Add('!macroend')
$uninst.Add('!macroend')
$nsh = "$build\payload-files.nsh"
Set-Content -Encoding utf8 $nsh (@('; Written by build-installer.ps1 from the staged payload; do not edit.') + $inst + '' + $uninst)

# --- compile
$name = "I-Simpa-Night-Mode-$version-$short-win64-setup.exe"
$exe = "$build\$name"
$log = "$build\build-installer.log"
$defs = @(
  "/DVERSION=$version", "/DCOMMIT=$commit", "/DOUTFILE=$exe", "/DPAYLOAD_NSH=$nsh",
  "/DLICENSE_FILE=$payload\LICENSE.txt", "/DICON=$repo\app\src-tauri\icons\icon.ico",
  "/DWEBVIEW2=$WebView2", "/DSIZE_KB=$sizeKb"
)
& $Makensis /V3 /INPUTCHARSET UTF8 @defs "$PSScriptRoot\night-mode.nsi" *> $log
if ($LASTEXITCODE -ne 0) { Get-Content $log -Tail 30; throw "makensis failed ($LASTEXITCODE): $log" }

# --- the receipt
$h = (Get-FileHash -Algorithm SHA256 $exe).Hash.ToLower()
$len = (Get-Item $exe).Length
$receipt = [ordered]@{
  installer = $exe; bytes = $len; sha256 = $h; version = $version; commit = $commit
  built = (Get-Date -Format 'yyyy-MM-dd HH:mm:ss'); makensis = (& $Makensis /VERSION)
  webview2_bootstrapper = [ordered]@{ path = $WebView2; sha256 = (Get-FileHash $WebView2).Hash.ToLower(); version = (Get-Item $WebView2).VersionInfo.FileVersion }
  crt_redist = (Get-CrtRedistDir)
  payload = @($files | ForEach-Object { [ordered]@{ path = $_.FullName.Substring($payload.Length + 1); bytes = $_.Length; sha256 = (Get-FileHash $_.FullName).Hash.ToLower(); version = $_.VersionInfo.FileVersion } })
}
$receipt | ConvertTo-Json -Depth 5 | Set-Content -Encoding utf8 "$build\installer.json"
"$h *$name" | Set-Content -Encoding ascii "$exe.sha256"
'{0}  {1:N0} bytes ({2:N1} MB)  sha256 {3}  ({4} files staged)' -f $exe, $len, ($len / 1MB), $h, $files.Count
