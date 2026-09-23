"""Shared helpers for the run-folder fixture generators in this folder.

Every generator writes bytes, never text mode, so its output is identical on every OS: LF line
endings, UTF-8 without a BOM, and `workingdirectory="__RUNDIR__"` (the placeholder the repo's
other solver fixtures use, `tests/fixtures/upstream/tutorial1/PROVENANCE.md`). Paths inside a
run folder are relative to it: `modelName`, `tetrameshFileName` and every directivity file
resolve inside the folder once `__RUNDIR__` is replaced by its absolute path plus a backslash.
"""
import re
import shutil
import struct
import sys
from pathlib import Path

PLACEHOLDER = "__RUNDIR__"

# The solver inputs a run folder may hold (docs/solver-contract.md, "The run folder"), and the
# fixture's own files, which a run never copies into the solver's folder except stub.json.
EXPECTED_JSON = "expected.json"
STUB_JSON = "stub.json"


def die(msg: str) -> None:
    sys.exit(f"{Path(sys.argv[0]).name}: {msg}")


def fresh_dir(path: Path) -> Path:
    """An empty folder at `path`: an existing one is removed first. Only used on fixture output."""
    if path.exists():
        shutil.rmtree(path)
    path.mkdir(parents=True)
    return path


def write_bytes(path: Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)


def write_text(path: Path, text: str) -> None:
    """UTF-8, no BOM, LF: the same bytes on every OS."""
    write_bytes(path, text.encode("utf-8"))


def attr(xml: str, name: str) -> str:
    """The value of the one attribute `name` in `xml`; exits unless there is exactly one."""
    found = re.findall(rf'\b{name}="([^"]*)"', xml)
    if len(found) != 1:
        die(f"attribute {name}: expected 1 occurrence, found {len(found)}")
    return found[0]


def sub_attr(xml: str, name: str, value: str) -> str:
    """Replaces the one attribute `name`; exits unless there is exactly one."""
    new, n = re.subn(rf'\b{name}="[^"]*"', f'{name}="{value}"', xml)
    if n != 1:
        die(f"attribute {name}: expected 1 occurrence, found {n}")
    return new


def drop_attr(xml: str, name: str) -> str:
    """Removes the one attribute `name` and the space before it."""
    new, n = re.subn(rf' {name}="[^"]*"', "", xml)
    if n != 1:
        die(f"attribute {name}: expected 1 occurrence, found {n}")
    return new


def check_self_contained(folder: Path) -> None:
    """Exits unless the folder's config.xml carries the placeholder and names files inside it."""
    xml = (folder / "config.xml").read_text(encoding="utf-8")
    if attr(xml, "workingdirectory") != PLACEHOLDER:
        die(f"{folder.name}: workingdirectory is not {PLACEHOLDER}")
    for name in ("modelName", "tetrameshFileName"):
        rel = attr(xml, name)
        if "/" in rel or "\\" in rel or ":" in rel:
            die(f"{folder.name}: {name}={rel!r} is not a plain file name")


# ---------------------------------------------------------------------------------------------
# .cbin and .mbin byte patches. Layouts: docs/formats/cbin.md ("Byte layout") and
# docs/formats/mbin.md. Decoding for checks goes through `simpa dump`, never through these.

def cbin_counts(data: bytes) -> tuple[int, int, int]:
    """(vertices, faces, offset of the first face record) of a single-node .cbin as ExportBIN
    writes it: 296 + 12V + 24F bytes (docs/formats/cbin.md)."""
    if len(data) < 24:
        die(f".cbin of {len(data)} bytes has no vertex count")
    (v,) = struct.unpack_from("<I", data, 20)
    face_count_at = 292 + 12 * v
    if len(data) < face_count_at + 4:
        die(f".cbin of {len(data)} bytes ends before its face count (V={v})")
    (f,) = struct.unpack_from("<I", data, face_count_at)
    if len(data) != 296 + 12 * v + 24 * f:
        die(f".cbin is not a single-node ExportBIN file ({len(data)} bytes, V={v}, F={f})")
    return v, f, face_count_at + 4


def cbin_set_material(data: bytes, id_mat: int) -> bytes:
    """Every face's idMat (u32 at +12 of its 24-byte record) set to `id_mat`."""
    _, f, first = cbin_counts(data)
    out = bytearray(data)
    for k in range(f):
        struct.pack_into("<I", out, first + 24 * k + 12, id_mat)
    return bytes(out)


MBIN_TET = 100


def mbin_counts(data: bytes) -> tuple[int, int, int]:
    """(tetrahedra, nodes, offset of the first tetrahedron record)."""
    t, n = struct.unpack_from("<II", data, 0)
    base = 8 + 12 * n
    if len(data) != base + MBIN_TET * t:
        die(f".mbin size {len(data)} is not 8 + 12N + 100T for T={t}, N={n}")
    return t, n, base


def mbin_cut_links(data: bytes) -> tuple[bytes, int]:
    """Every neighbour link >= 0 set to -2 (no tetrahedron across): the survey's mklossy.py."""
    t, _, base = mbin_counts(data)
    out = bytearray(data)
    cut = 0
    for k in range(t):
        for face in range(4):
            off = base + MBIN_TET * k + 20 + 20 * face + 16
            if struct.unpack_from("<i", out, off)[0] >= 0:
                struct.pack_into("<i", out, off, -2)
                cut += 1
    return bytes(out), cut


def mbin_repeat_corner(data: bytes, tet: int = 0) -> bytes:
    """Tetrahedron `tet` gets its corner D equal to corner A: the solver's exit(1) case
    (coreTypes.cpp:210-216)."""
    t, _, base = mbin_counts(data)
    if not 0 <= tet < t:
        die(f"tetrahedron {tet} out of range 0..{t}")
    out = bytearray(data)
    (a,) = struct.unpack_from("<i", out, base + MBIN_TET * tet)
    struct.pack_into("<i", out, base + MBIN_TET * tet + 12, a)
    return bytes(out)


EMPTY_MBIN = struct.pack("<II", 0, 0)
"""T = 0, N = 0: upstream reads an empty mesh and prints "Tetrahedron file is empty"
(coreTypes.cpp:188, :241)."""
