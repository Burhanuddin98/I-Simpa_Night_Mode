# Builds a release of I-Simpa Night Mode on a machine whose toolchain matches solvers/manifest.json (Grace),
# so the installer carries the verified solvers, then tags the commit locally and prints the commands that
# publish it. It pushes and uploads nothing itself: publishing is Burhan's word.
#
# Why here and not in CI: a CI runner's MSVC can differ from the manifest's, its solvers then come out
# unverified, and the app refuses to run an unverified solver; CI proves the source builds (build.yml), this
# script makes the installer people download.
#
#   powershell -ExecutionPolicy Bypass -File tools\release.ps1 -Version 1.0.0 [-DryRun]
param(
  [Parameter(Mandatory = $true)][string]$Version,
  # Check everything and build, but create no tag.
  [switch]$DryRun
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot

if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw "Version '$Version' is not MAJOR.MINOR.PATCH." }
$tag = "v$Version"

Push-Location $repo
try {
  # 1. A clean tree, on a commit GitHub already has, with no tag of this name yet.
  $dirty = git status --porcelain --untracked-files=no
  if ($dirty) { throw "The work tree has uncommitted changes; commit or discard them first:`n$dirty" }
  $head = (git rev-parse HEAD).Trim()
  git fetch --quiet origin
  $onOrigin = git branch -r --contains $head
  if (-not $onOrigin) { throw "HEAD $($head.Substring(0, 7)) is not on any origin branch; push it first." }
  if (git tag --list $tag) { throw "Tag $tag exists already." }

  # 2. The version is raised by hand in both files (decision 86); refuse a mismatch rather than guess.
  $cargo = (Select-String -Path 'Cargo.toml' -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
  $tauri = (Get-Content 'app\src-tauri\tauri.conf.json' -Raw | ConvertFrom-Json).version
  if ($cargo -ne $Version -or $tauri -ne $Version) {
    throw "Version $Version, but Cargo.toml says $cargo and app\src-tauri\tauri.conf.json says $tauri. Set both to $Version, commit, push, and run this again."
  }
  if (-not (Test-Path 'docs\release-notes-v1.md')) { throw 'docs\release-notes-v1.md is missing.' }

  # 3. The build, with the verified solvers or not at all (no -AllowUnverifiedSolvers).
  $out = Join-Path $repo ".out\release\$Version"
  New-Item -ItemType Directory -Force $out | Out-Null
  & (Join-Path $repo 'tools\ci\build.ps1') -Out $out
  $dir = Get-ChildItem $out -Directory | Where-Object { Test-Path (Join-Path $_.FullName 'installer.json') } |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1
  if (-not $dir) { throw "No installer.json under $out." }
  $receipt = Get-Content (Join-Path $dir.FullName 'installer.json') -Raw | ConvertFrom-Json
  if (-not $receipt.solvers_verified) { throw 'The installer does not carry the verified solvers; it is not a release.' }
  $exe = Join-Path $dir.FullName (Split-Path -Leaf $receipt.installer)
  $sha = "$exe.sha256"
  if (-not (Test-Path $sha)) { "$($receipt.sha256)  $(Split-Path -Leaf $exe)" | Set-Content -Encoding ascii $sha }

  # 4. The tag, local only.
  if ($DryRun) {
    Write-Host "Dry run: no tag. The installer is $exe"
  } else {
    git tag -a $tag -m "I-Simpa Night Mode $Version" $head
    Write-Host "Tagged $tag at $($head.Substring(0, 7)) (local)."
  }

  Write-Host ''
  Write-Host "Installer $exe"
  Write-Host "  sha256 $($receipt.sha256), $($receipt.bytes) bytes, solvers verified"
  Write-Host ''
  Write-Host 'To publish (Burhan):'
  Write-Host "  git push origin $tag"
  Write-Host "  gh release create $tag `"$exe`" `"$sha`" --title `"I-Simpa Night Mode $Version`" --notes-file docs\release-notes-v1.md"
} finally {
  Pop-Location
}
