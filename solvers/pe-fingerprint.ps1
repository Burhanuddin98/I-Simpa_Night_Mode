# A solver executable's fingerprint that survives a rebuild: its "code sha256", the sha256 of the
# PE file with every field the linker fills with the link time set to zero. Dot-sourced by
# solvers/build.ps1 (which records it in solvers/manifest.json), tools/gates/m1.ps1 and
# tools/gates/parity.ps1 (which hold the solver folder to it). tools/fixture-gen/pe_fingerprint.py
# is the same definition in Python; test_fixture_gen.py holds the two to each other.
#
# The fields, measured on 2026-09-24 by two from-scratch builds of solvers/build.ps1 into two
# folders (the byte differences of all five executables, spps, classicalTheory, preprocess, TetGen
# 1.5.0 and upstream's TetGen 1.6.0 reference, fall in these and nowhere else):
#   - IMAGE_FILE_HEADER.TimeDateStamp (the COFF header);
#   - IMAGE_DEBUG_DIRECTORY.TimeDateStamp of every debug-directory entry (one POGO entry each).
# Both are found by parsing the headers, never at offsets taken from one file.
#
# A PE file in which the link time could reach any other field is refused, not hashed: a
# CheckSum (link /RELEASE covers the timestamp with it), a CodeView record (link /DEBUG: its GUID
# and age change on every link) or any debug entry of a type these builds do not have. Such a
# file is a change in how the solvers are linked; the refusal names it, so it is measured again
# instead of being passed or failed on a definition that was never checked for it.

# Debug-directory entry types whose data the measurement covered (winnt.h IMAGE_DEBUG_TYPE_*).
$PeMeasuredDebugTypes = @{ 13 = 'POGO' }
$PeDebugTypeNames = @{
    0 = 'UNKNOWN'; 1 = 'COFF'; 2 = 'CODEVIEW'; 3 = 'FPO'; 4 = 'MISC'; 5 = 'EXCEPTION'; 6 = 'FIXUP'
    7 = 'OMAP_TO_SRC'; 8 = 'OMAP_FROM_SRC'; 9 = 'BORLAND'; 10 = 'RESERVED10'; 11 = 'CLSID'
    12 = 'VC_FEATURE'; 13 = 'POGO'; 14 = 'ILTCG'; 15 = 'MPX'; 16 = 'REPRO'; 20 = 'EX_DLLCHARACTERISTICS'
}

# The headers of a PE32+ image: the COFF header's and the optional header's offsets, the optional
# header's size, and the section table ({ Name; Va; VSize; RSize; RPtr } each). Throws, naming
# what it found, on anything that is not PE32+ or whose headers run past the end of the file.
function Read-PeHeaders([byte[]]$d, [string]$what = 'the file') {
    function Need([int64]$end, [string]$part) {
        if ($end -gt $d.Length) { throw "${what}: not a PE file of the measured shape: $part runs past the end of the file ($($d.Length) bytes)" }
    }
    Need 0x40 'the DOS header'
    if ($d[0] -ne 0x4D -or $d[1] -ne 0x5A) { throw "${what}: not a PE file: no MZ signature" }
    $pe = [int64][BitConverter]::ToUInt32($d, 0x3C)
    Need ($pe + 24) 'the PE signature and COFF header'
    if ([BitConverter]::ToUInt32($d, [int]$pe) -ne 0x4550) { throw "${what}: not a PE file: no PE signature at 0x$('{0:X}' -f $pe)" }
    $coff = [int]$pe + 4
    $nsec = [BitConverter]::ToUInt16($d, $coff + 2)
    $optSize = [BitConverter]::ToUInt16($d, $coff + 16)
    $opt = $coff + 20
    Need ([int64]$opt + $optSize) 'the optional header'
    if ($optSize -lt 112) { throw "${what}: not a PE file of the measured shape: optional header of $optSize bytes" }
    $magic = [BitConverter]::ToUInt16($d, $opt)
    if ($magic -ne 0x20B) { throw "${what}: not PE32+ (optional header magic 0x$('{0:X}' -f $magic)): the code sha256 is defined for the x64 solvers only" }
    $table = [int64]$opt + $optSize
    Need ($table + 40 * $nsec) 'the section table'
    $sections = @(for ($i = 0; $i -lt $nsec; $i++) {
        $s = [int]$table + 40 * $i
        [pscustomobject]@{
            Name = [Text.Encoding]::ASCII.GetString($d, $s, 8).TrimEnd([char]0)
            VSize = [BitConverter]::ToUInt32($d, $s + 8); Va = [BitConverter]::ToUInt32($d, $s + 12)
            RSize = [BitConverter]::ToUInt32($d, $s + 16); RPtr = [BitConverter]::ToUInt32($d, $s + 20)
        }
    })
    [pscustomobject]@{ Coff = $coff; Opt = $opt; OptSize = $optSize; Sections = $sections }
}

# The link-time fields of a PE32+ image: one record per field, { Offset; Length; Field }. Throws,
# naming what it found, on anything that is not a PE32+ image of the measured shape.
function Get-PeLinkTimeFields([byte[]]$d, [string]$what = 'the file') {
    $h = Read-PeHeaders $d $what
    $opt = $h.Opt
    $sum = [BitConverter]::ToUInt32($d, $opt + 64)
    if ($sum -ne 0) { throw "${what}: its CheckSum is set (0x$('{0:X8}' -f $sum)), which the measured builds never have: it covers the link time, so the code sha256 is not defined for this link" }
    $fields = New-Object System.Collections.ArrayList
    [void]$fields.Add([pscustomobject]@{ Offset = $h.Coff + 4; Length = 4; Field = 'IMAGE_FILE_HEADER.TimeDateStamp' })
    $ndirs = [BitConverter]::ToUInt32($d, $opt + 108)
    if ($ndirs -lt 7 -or $h.OptSize -lt 112 + 8 * 7) { return $fields.ToArray() }   # no debug directory slot
    $dbgRva = [BitConverter]::ToUInt32($d, $opt + 112 + 48)
    $dbgSize = [BitConverter]::ToUInt32($d, $opt + 112 + 52)
    if ($dbgSize -eq 0) { return $fields.ToArray() }
    if ($dbgSize % 28 -ne 0) { throw "${what}: its debug directory is $dbgSize bytes, not a whole number of 28-byte entries" }
    $sec = @($h.Sections | Where-Object { $dbgRva -ge $_.Va -and [int64]$dbgRva + $dbgSize -le [int64]$_.Va + $_.RSize } | Select-Object -First 1)
    if (-not $sec.Count) { throw "${what}: its debug directory (RVA 0x$('{0:X}' -f $dbgRva), $dbgSize bytes) lies in no section's file data" }
    $off = [int64]$sec[0].RPtr + ($dbgRva - $sec[0].Va)
    if ($off + $dbgSize -gt $d.Length) { throw "${what}: not a PE file of the measured shape: the debug directory runs past the end of the file ($($d.Length) bytes)" }
    for ($k = 0; $k -lt $dbgSize / 28; $k++) {
        $e = [int]$off + 28 * $k
        $type = [int][BitConverter]::ToUInt32($d, $e + 12)
        if (-not $PeMeasuredDebugTypes.ContainsKey($type)) {
            $name = if ($PeDebugTypeNames.ContainsKey($type)) { $PeDebugTypeNames[$type] } else { "type $type" }
            throw "${what}: debug entry $k is $name, which the measured builds do not have (they have $(@($PeMeasuredDebugTypes.Values) -join ', ') only): the code sha256 is not defined for this link"
        }
        [void]$fields.Add([pscustomobject]@{ Offset = $e + 4; Length = 4; Field = "IMAGE_DEBUG_DIRECTORY[$k].TimeDateStamp ($($PeMeasuredDebugTypes[$type]))" })
    }
    return $fields.ToArray()
}

# A PE file's bytes with its link-time fields zeroed.
function Get-PeCodeBytes([string]$path) {
    $d = [IO.File]::ReadAllBytes($path)
    foreach ($f in (Get-PeLinkTimeFields $d $path)) { [Array]::Clear($d, $f.Offset, $f.Length) }
    return , $d
}

function ConvertTo-Sha256Hex([byte[]]$bytes) {
    $sha = [Security.Cryptography.SHA256]::Create()
    try { return ([BitConverter]::ToString($sha.ComputeHash($bytes)) -replace '-', '').ToLower() } finally { $sha.Dispose() }
}

# The code sha256 of a PE32+ file, lower-case hex. Throws on a file of another shape.
function Get-CodeSha256([string]$path) { ConvertTo-Sha256Hex (Get-PeCodeBytes $path) }

# The file's plain sha256, lower-case hex.
function Get-RawSha256([string]$path) { (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLower() }

# The gates' says-NO input: a copy of a PE file with the middle byte of its .text section inverted.
function Copy-PeFlippedText([string]$from, [string]$to) {
    $d = [IO.File]::ReadAllBytes($from)
    $text = @((Read-PeHeaders $d $from).Sections | Where-Object { $_.Name -eq '.text' })
    if ($text.Count -ne 1 -or $text[0].RSize -eq 0) { throw "${from}: no .text section with file data" }
    $i = [int]$text[0].RPtr + [int]($text[0].RSize / 2)
    $d[$i] = $d[$i] -bxor 0xFF
    [IO.File]::WriteAllBytes($to, $d)
    return $to
}

# A copy of a PE file with every link-time field set to $stamp: the same code, linked at another
# time, as a rebuild gives it.
function Copy-PeRestamped([string]$from, [string]$to, [uint32]$stamp) {
    $d = [IO.File]::ReadAllBytes($from)
    $b = [BitConverter]::GetBytes($stamp)
    foreach ($f in (Get-PeLinkTimeFields $d $from)) { [Array]::Copy($b, 0, $d, $f.Offset, 4) }
    [IO.File]::WriteAllBytes($to, $d)
    return $to
}
