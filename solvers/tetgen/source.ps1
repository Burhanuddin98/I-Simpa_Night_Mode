# Dot-sourced by solvers/build.ps1 and tools/gates/m1.ps1.
#
# third_party/tetgen-1.5.0 carries WIAS's tetgen1.5.0.tar.gz beside the files extracted from it.
# These functions read the tarball itself, in memory (gzip, then the tar headers), so the claim
# "the folder is the tarball, unmodified" is re-derived on every build and gate run rather than
# trusted from SHA256SUMS.

# The sha256 of WIAS's tetgen1.5.0.tar.gz as downloaded from
# https://wias-berlin.de/software/tetgen/1.5/src/tetgen1.5.0.tar.gz on 2026-09-23 (272,513 bytes).
$TetgenTarballSha256 = '4d114861d5ef2063afd06ef38885ec46822e90e7b4ea38c864f76493451f9cf3'

# The regular-file members of a .tar.gz, as an ordered map name -> sha256, with the first path
# component removed (tar --strip-components=1). Throws on any member type it does not handle
# (links, long-name and pax headers), so an archive it cannot read fully is never half-checked.
function Read-TetgenTarball([string]$path) {
    $fs = [IO.File]::OpenRead($path)
    try {
        $gz = New-Object IO.Compression.GZipStream($fs, [IO.Compression.CompressionMode]::Decompress)
        $ms = New-Object IO.MemoryStream
        $gz.CopyTo($ms)
    } finally { $fs.Dispose() }
    $d = $ms.ToArray()
    $sha = [Security.Cryptography.SHA256]::Create()
    $members = [ordered]@{}
    $pos = 0
    while ($pos + 512 -le $d.Length) {
        $name = [Text.Encoding]::ASCII.GetString($d, $pos, 100).TrimEnd([char]0)
        if ($name.Length -eq 0) { break }   # the all-zero block that ends the archive
        $size = [Convert]::ToInt64([Text.Encoding]::ASCII.GetString($d, $pos + 124, 12).Trim([char]0, ' '), 8)
        $type = [char]$d[$pos + 156]
        # POSIX ustar ("ustar\0") keeps a name prefix at 345; GNU tar ("ustar  ") keeps times there.
        if ([Text.Encoding]::ASCII.GetString($d, $pos + 257, 6) -eq "ustar`0") {
            $prefix = [Text.Encoding]::ASCII.GetString($d, $pos + 345, 155).TrimEnd([char]0)
            if ($prefix) { $name = "$prefix/$name" }
        }
        $pos += 512
        if ($type -eq '0' -or $type -eq [char]0) {
            if ($pos + $size -gt $d.Length) { throw "${path}: member $name runs past the end of the archive" }
            $rel = ($name -split '/', 2)[1]
            if (-not $rel) { throw "${path}: member $name has no path below the top folder" }
            $members[$rel] = ([BitConverter]::ToString($sha.ComputeHash($d, $pos, [int]$size)) -replace '-', '').ToLower()
        } elseif ($type -ne '5') {
            throw "${path}: member $name has tar type '$type', which this reader does not handle"
        }
        $pos += [int][math]::Ceiling($size / 512) * 512
    }
    if ($members.Count -eq 0) { throw "${path}: no file members" }
    return $members
}

# The tarball members a folder lacks or holds with other bytes; empty when it holds all of them
# unchanged.
function Get-TarballMismatches($members, [string]$dir) {
    @($members.Keys | Where-Object {
        $f = Join-Path $dir $_
        -not (Test-Path -LiteralPath $f) -or (Get-FileHash -LiteralPath $f -Algorithm SHA256).Hash.ToLower() -ne $members[$_]
    })
}
