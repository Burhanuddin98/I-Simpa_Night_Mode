# Dot-sourced by solvers/build.ps1 and tools/gates/m1.ps1.
#
# Get-BuildCommands reads the cl and link command lines MSBuild recorded for a `tetgen` target
# (<target dir>\tetgen.dir\Release\tetgen.tlog\{CL,link}.command.1.tlog) and replaces the two
# paths that legitimately differ between build trees, the source folder and the target's build
# folder, by <SRC> and <TGT>. Two targets built with the same settings give equal lists; any
# difference in a flag, a define, the runtime library, the optimisation level or the source set
# does not.
#
# Case matters. MSBuild writes every path in these records upper-cased and every flag as given,
# and cl flags are case-sensitive (/GR enables RTTI, /Gr makes __fastcall the default calling
# convention). So only the two paths are matched without regard to case; the rest of each record
# is kept, and compared, exactly as written.
#
# $tlogDir overrides where the records are read from (default: the target's own tlog folder);
# the paths replaced are still $targetDir's and $sourceDir's.
function Get-BuildCommands([string]$targetDir, [string]$sourceDir, [string]$tlogDir = '') {
    $tlog = if ($tlogDir) { $tlogDir } else { Join-Path $targetDir 'tetgen.dir\Release\tetgen.tlog' }
    $records = @()
    foreach ($kind in 'CL', 'link') {
        $f = Join-Path $tlog "$kind.command.1.tlog"
        if (-not (Test-Path -LiteralPath $f)) { throw "no $kind command record at $f" }
        $text = [IO.File]::ReadAllText($f)   # UTF-16LE with a BOM
        foreach ($pair in @(@($sourceDir, '<SRC>'), @($targetDir, '<TGT>'))) {
            $full = [IO.Path]::GetFullPath($pair[0]).TrimEnd('\')
            foreach ($form in $full, $full.Replace('\', '/')) {
                $text = [regex]::Replace($text, [regex]::Escape($form), $pair[1], 'IgnoreCase')
            }
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
    return @($records | Sort-Object -CaseSensitive)
}

# The records of one list missing from the other, each prefixed '<' (only in $a) or '>' (only
# in $b). Empty when both targets were built with the same command lines. Case-sensitive.
function Compare-BuildCommands([string[]]$a, [string[]]$b) {
    $d = @(Compare-Object $a $b -CaseSensitive)
    return @($d | ForEach-Object { $(if ($_.SideIndicator -eq '<=') { '< ' } else { '> ' }) + $_.InputObject })
}
