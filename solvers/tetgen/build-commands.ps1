# Dot-sourced by solvers/build.ps1 and tools/gates/m1.ps1.
#
# Get-BuildCommands reads the cl and link command lines MSBuild recorded for a `tetgen` target
# (<target dir>\tetgen.dir\Release\tetgen.tlog\{CL,link}.command.1.tlog) and replaces the two
# paths that legitimately differ between build trees, the source folder and the target's build
# folder, by <SRC> and <TGT>. Two targets built with the same settings give equal lists; any
# difference in a flag, a define, the runtime library, the optimisation level or the source set
# does not.
#
# $tlogDir overrides where the records are read from (default: the target's own tlog folder);
# the paths replaced are still $targetDir's and $sourceDir's.
function Get-BuildCommands([string]$targetDir, [string]$sourceDir, [string]$tlogDir = '') {
    $tlog = if ($tlogDir) { $tlogDir } else { Join-Path $targetDir 'tetgen.dir\Release\tetgen.tlog' }
    $records = @()
    foreach ($kind in 'CL', 'link') {
        $f = Join-Path $tlog "$kind.command.1.tlog"
        if (-not (Test-Path -LiteralPath $f)) { throw "no $kind command record at $f" }
        $text = [IO.File]::ReadAllText($f).ToUpperInvariant()   # UTF-16LE with a BOM
        foreach ($pair in @(@($sourceDir, '<SRC>'), @($targetDir, '<TGT>'))) {
            $full = [IO.Path]::GetFullPath($pair[0]).TrimEnd('\').ToUpperInvariant()
            $text = $text.Replace($full, $pair[1]).Replace($full.Replace('\', '/'), $pair[1])
        }
        $n = 0
        foreach ($rec in ($text -split '(?m)^\^')) {
            $lines = @($rec -split "`r?`n" | ForEach-Object { $_.Trim() } | Where-Object { $_ })
            if ($lines.Count -eq 0) { continue }
            $n++
            $records += "$kind ^" + ($lines -join ' ')
        }
        if ($n -eq 0) { throw "$f records no command" }
    }
    return @($records | Sort-Object)
}

# The records of one list missing from the other, each prefixed '<' (only in $a) or '>' (only
# in $b). Empty when both targets were built with the same command lines.
function Compare-BuildCommands([string[]]$a, [string[]]$b) {
    $d = @(Compare-Object $a $b)
    return @($d | ForEach-Object { $(if ($_.SideIndicator -eq '<=') { '< ' } else { '> ' }) + $_.InputObject })
}
