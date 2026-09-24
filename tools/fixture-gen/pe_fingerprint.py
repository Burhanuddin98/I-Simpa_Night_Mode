"""A solver executable's fingerprint that survives a rebuild: its code sha256.

The code sha256 is the sha256 of the PE file with every field the linker fills with the link time
set to zero. solvers/pe-fingerprint.ps1 defines it for solvers/build.ps1 and the gates; this is
the same definition for the fixture generators, and test_fixture_gen.py holds the two to each
other. The fields, measured on 2026-09-24 by two from-scratch builds into two folders:

- IMAGE_FILE_HEADER.TimeDateStamp (the COFF header);
- IMAGE_DEBUG_DIRECTORY.TimeDateStamp of every debug-directory entry (one POGO entry each).

Both are found by parsing the headers. A PE file in which the link time could reach any other
field (a CheckSum, a CodeView record, a debug entry of a type the builds do not have) is refused
with NotMeasured: it is a change in how the solvers are linked, to be measured again.

    python tools/fixture-gen/pe_fingerprint.py <exe>...   prints code sha256, sha256, name
"""
import hashlib
import struct
import sys
from dataclasses import dataclass
from pathlib import Path

# Debug-directory entry types whose data the measurement covered (winnt.h IMAGE_DEBUG_TYPE_*).
MEASURED_DEBUG_TYPES = {13: "POGO"}
DEBUG_TYPE_NAMES = {
    0: "UNKNOWN", 1: "COFF", 2: "CODEVIEW", 3: "FPO", 4: "MISC", 5: "EXCEPTION", 6: "FIXUP",
    7: "OMAP_TO_SRC", 8: "OMAP_FROM_SRC", 9: "BORLAND", 10: "RESERVED10", 11: "CLSID",
    12: "VC_FEATURE", 13: "POGO", 14: "ILTCG", 15: "MPX", 16: "REPRO", 20: "EX_DLLCHARACTERISTICS",
}


class NotMeasured(ValueError):
    """The file is not a PE32+ image of the measured shape: no code sha256 is defined for it."""


@dataclass
class Section:
    name: str
    va: int
    vsize: int
    rsize: int
    rptr: int


@dataclass
class Headers:
    coff: int  # offset of IMAGE_FILE_HEADER
    opt: int  # offset of the optional header
    opt_size: int
    sections: list[Section]


def _need(d: bytes, end: int, part: str, what: str) -> None:
    if end > len(d):
        raise NotMeasured(f"{what}: not a PE file of the measured shape: {part} runs past the end "
                          f"of the file ({len(d)} bytes)")


def headers(d: bytes, what: str = "the file") -> Headers:
    """The headers of a PE32+ image; NotMeasured for anything else or headers past the end."""
    _need(d, 0x40, "the DOS header", what)
    if d[:2] != b"MZ":
        raise NotMeasured(f"{what}: not a PE file: no MZ signature")
    pe = struct.unpack_from("<I", d, 0x3C)[0]
    _need(d, pe + 24, "the PE signature and COFF header", what)
    if d[pe:pe + 4] != b"PE\0\0":
        raise NotMeasured(f"{what}: not a PE file: no PE signature at 0x{pe:X}")
    coff = pe + 4
    nsec, = struct.unpack_from("<H", d, coff + 2)
    opt_size, = struct.unpack_from("<H", d, coff + 16)
    opt = coff + 20
    _need(d, opt + opt_size, "the optional header", what)
    if opt_size < 112:
        raise NotMeasured(f"{what}: not a PE file of the measured shape: optional header of "
                          f"{opt_size} bytes")
    magic, = struct.unpack_from("<H", d, opt)
    if magic != 0x20B:
        raise NotMeasured(f"{what}: not PE32+ (optional header magic 0x{magic:X}): the code sha256 "
                          "is defined for the x64 solvers only")
    table = opt + opt_size
    _need(d, table + 40 * nsec, "the section table", what)
    sections = []
    for i in range(nsec):
        s = table + 40 * i
        vsize, va, rsize, rptr = struct.unpack_from("<IIII", d, s + 8)
        sections.append(Section(d[s:s + 8].rstrip(b"\0").decode("ascii", "replace"), va, vsize,
                                rsize, rptr))
    return Headers(coff, opt, opt_size, sections)


def link_time_fields(d: bytes, what: str = "the file") -> list[tuple[int, int, str]]:
    """(offset, length, field) of each link-time field of a PE32+ image; NotMeasured otherwise."""
    h = headers(d, what)
    opt = h.opt
    checksum, = struct.unpack_from("<I", d, opt + 64)
    if checksum:
        raise NotMeasured(f"{what}: its CheckSum is set (0x{checksum:08X}), which the measured "
                          "builds never have: it covers the link time, so the code sha256 is not "
                          "defined for this link")
    fields = [(h.coff + 4, 4, "IMAGE_FILE_HEADER.TimeDateStamp")]
    ndirs, = struct.unpack_from("<I", d, opt + 108)
    if ndirs < 7 or h.opt_size < 112 + 8 * 7:
        return fields
    dbg_rva, dbg_size = struct.unpack_from("<II", d, opt + 112 + 48)
    if dbg_size == 0:
        return fields
    if dbg_size % 28:
        raise NotMeasured(f"{what}: its debug directory is {dbg_size} bytes, not a whole number of "
                          "28-byte entries")
    off = next((s.rptr + dbg_rva - s.va for s in h.sections
                if s.va <= dbg_rva and dbg_rva + dbg_size <= s.va + s.rsize), None)
    if off is None:
        raise NotMeasured(f"{what}: its debug directory (RVA 0x{dbg_rva:X}, {dbg_size} bytes) lies "
                          "in no section's file data")
    _need(d, off + dbg_size, "the debug directory", what)
    for k in range(dbg_size // 28):
        e = off + 28 * k
        typ, = struct.unpack_from("<I", d, e + 12)
        if typ not in MEASURED_DEBUG_TYPES:
            name = DEBUG_TYPE_NAMES.get(typ, f"type {typ}")
            raise NotMeasured(f"{what}: debug entry {k} is {name}, which the measured builds do not "
                              f"have (they have {', '.join(MEASURED_DEBUG_TYPES.values())} only): "
                              "the code sha256 is not defined for this link")
        fields.append((e + 4, 4, f"IMAGE_DEBUG_DIRECTORY[{k}].TimeDateStamp "
                                 f"({MEASURED_DEBUG_TYPES[typ]})"))
    return fields


def code_bytes(path: Path) -> bytes:
    """The file's bytes with its link-time fields zeroed."""
    d = bytearray(Path(path).read_bytes())
    for off, n, _ in link_time_fields(bytes(d), str(path)):
        d[off:off + n] = bytes(n)
    return bytes(d)


def code_sha256(path: Path) -> str:
    """The code sha256 of a PE32+ file, lower-case hex; NotMeasured for a file of another shape."""
    return hashlib.sha256(code_bytes(path)).hexdigest()


def raw_sha256(path: Path) -> str:
    """The file's plain sha256, lower-case hex."""
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


if __name__ == "__main__":
    status = 0
    for a in sys.argv[1:]:
        try:
            print(f"{code_sha256(Path(a))}  {raw_sha256(Path(a))}  {a}")
        except NotMeasured as e:
            print(f"refused: {e}", file=sys.stderr)
            status = 1
    sys.exit(status)
