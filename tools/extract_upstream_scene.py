#!/usr/bin/env python3
"""Extract a repaired scene mesh from an upstream I-Simpa .proj archive into a
layered PLY that Night Mode's own loader accepts.

WHY THIS EXISTS
---------------
The raw tutorial geometry upstream ships (elmia.ply) is self-intersecting. TetGen
refuses it with exit 3 and leaves partial output, Night Mode continues anyway, and
SPPS then destroys ~99.997% of its particles on a mesh of the wrong domain. See
docs/release-arc-plan.md item 12.

Upstream avoids this by running its scene-correction step BEFORE exporting to
TetGen, and its tutorial .proj carries the already-corrected result. This tool
lifts that corrected mesh out so Night Mode can be driven end to end on geometry
that actually meshes, while the real repair path is built.

This is a BRIDGE, not the fix. The fix is repair-before-export inside Night Mode.

USAGE
-----
    python tools/extract_upstream_scene.py <upstream.proj> <out.ply> [layers.ply]

e.g.
    python tools/extract_upstream_scene.py \
        "B:/repos/I-Simpa-upstream/src/isimpa/resources/doc/tutorial/tutorial 2/tutorial_2.proj" \
        testdata/elmia_corrected.ply \
        "B:/repos/I-Simpa-upstream/src/isimpa/resources/doc/tutorial/tutorial 2/elmia.ply"

THE LAYERS ARGUMENT MATTERS ACOUSTICALLY
----------------------------------------
The corrected mesh inside the .proj carries its faces in ONE group called
"model" -- the surface-group split that names ceiling, floor, audience and so on
did not survive upstream's correction step in a form this file preserves. Loaded
as-is, the whole hall gets a single material, which is not the room.

Passing the ORIGINAL layered .ply restores that split: each corrected face is
assigned the layer of the nearest original face by centroid. That is a heuristic,
not a proof. It is defensible here because correction subdivides faces in place
rather than moving surfaces, so a corrected face's centroid lies on or very near
the original it came from. Per-layer counts are printed so the split can be
sanity-checked against the original -- if a layer comes back empty or wildly
out of proportion, do not trust the result.

Without this argument the output is still geometrically correct and still
meshes cleanly; it is simply acoustically uniform, which is useful only for
testing the pipeline, never for a number anyone quotes.

FORMAT NOTES (derived from upstream src/lib_interface/input_output/bin.cpp and
verified byte-wise against tutorial_2.proj's sceneMesh.bin)
------------------------------------------------------------------------------
  header : uint32 majorVersion, uint32 minorVersion          (observed 1.2)
  node   : uint16 nodeType, uint16 pad, uint32 firstSon, uint32 nextBrother
  type 0 : VERTICES  -> uint32 count, then count * (float x, y, z)
  type 1 : GROUP     -> char name[255], 1 pad byte, uint32 nbFace,
                        then nbFace face records
  type 4 : seen in v1.2 files, undocumented in the v1.x reader; skipped via
           nextBrother. Carries no geometry we need.

  The face record in these v1.2 files is 32 bytes, NOT the 24 the struct in
  bin.cpp implies:
        uint32 a, uint32 b, uint32 c, then 20 bytes of ids/padding.
  The trailing fields are near-constant across the corpus (a uint16 0xFFFF at
  offset 12 on every one of 7860 faces, two 0/1 bytes, one 0/1 uint32, then
  zeros), consistent with idRs / idEn / idMaterial packing plus alignment. Only
  the three indices are load-bearing here, and the record size is taken from the
  node span rather than assumed, so a different packing is detected rather than
  silently misread.
"""
import struct
import sys
import zipfile
from pathlib import Path

NODE_VERTICES = 0
NODE_GROUP = 1


def read_nodes(blob):
    """Walk the node chain. Returns (vertices, [(group_name, faces)])."""
    major, minor = struct.unpack_from("<II", blob, 0)
    verts, groups = [], []
    off, guard = 8, 0
    while off + 12 <= len(blob):
        guard += 1
        if guard > 10000:
            raise RuntimeError("node chain did not terminate; file is malformed")
        node_type, _pad, _first_son, next_brother = struct.unpack_from("<HHII", blob, off)
        payload = off + 12
        if node_type == NODE_VERTICES:
            count = struct.unpack_from("<I", blob, payload)[0]
            payload += 4
            verts = [struct.unpack_from("<fff", blob, payload + 12 * i) for i in range(count)]
        elif node_type == NODE_GROUP:
            name = blob[payload:payload + 255].split(b"\0")[0].decode("latin-1")
            payload += 256
            nb_face = struct.unpack_from("<I", blob, payload)[0]
            payload += 4
            end = next_brother if next_brother else len(blob)
            span = end - payload
            if nb_face == 0:
                rec = 0
            else:
                if span % nb_face:
                    raise RuntimeError(
                        f"group {name!r}: {span} payload bytes is not a whole "
                        f"multiple of {nb_face} faces; unknown record packing")
                rec = span // nb_face
                if rec < 12:
                    raise RuntimeError(f"group {name!r}: {rec}-byte face record is too small")
            faces = [struct.unpack_from("<III", blob, payload + rec * i) for i in range(nb_face)]
            groups.append((name, faces))
        # type 4 and anything else: skip via nextBrother
        if not next_brother or next_brother <= off:
            break
        off = next_brother
    return (major, minor), verts, groups


def write_layered_ply(path, verts, groups):
    """Night Mode's PLY loader wants per-face layer_id plus a layer element
    carrying each layer's name as a uchar list. mesh/ply_loader.cpp decodes it."""
    total = sum(len(f) for _, f in groups)
    with open(path, "w", newline="\n") as out:
        out.write("ply\nformat ascii 1.0\n")
        out.write(f"element vertex {len(verts)}\n")
        out.write("property float x\nproperty float y\nproperty float z\n")
        out.write(f"element face {total}\n")
        out.write("property list uchar int vertex_indices\nproperty int layer_id\n")
        out.write(f"element layer {len(groups)}\n")
        out.write("property list uchar uchar layer_name\nend_header\n")
        for x, y, z in verts:
            out.write(f"{x:.9g} {y:.9g} {z:.9g}\n")
        for layer_id, (_name, faces) in enumerate(groups):
            for a, b, c in faces:
                out.write(f"3 {a} {b} {c} {layer_id}\n")
        for name, _faces in groups:
            codes = [ord(ch) for ch in name]
            out.write(f"{len(codes)} " + " ".join(str(c) for c in codes) + "\n")
    return total


def read_layered_ply(path):
    """Read an ascii PLY with per-face layer_id and a named layer element.
    Returns (vertices, [(tri, layer_id)], [layer_name])."""
    text = path.read_bytes().decode("ascii", "replace").replace("\r\n", "\n").replace("\r", "\n")
    lines = text.split("\n")
    head = lines.index("end_header")
    counts = {}
    for line in lines[:head]:
        parts = line.split()
        if len(parts) == 3 and parts[0] == "element":
            counts[parts[1]] = int(parts[2])
    i = head + 1
    verts = [tuple(float(v) for v in lines[i + k].split()[:3]) for k in range(counts["vertex"])]
    i += counts["vertex"]
    tris = []
    for k in range(counts.get("face", 0)):
        tok = lines[i + k].split()
        n = int(tok[0])
        idx = [int(v) for v in tok[1:1 + n]]
        layer = int(tok[1 + n]) if len(tok) > 1 + n else 0
        for j in range(1, n - 1):          # fan-triangulate polygons
            tris.append(((idx[0], idx[j], idx[j + 1]), layer))
    i += counts.get("face", 0)
    names = []
    for k in range(counts.get("layer", 0)):
        tok = lines[i + k].split()
        names.append("".join(chr(int(c)) for c in tok[1:1 + int(tok[0])]))
    return verts, tris, names


def assign_layers(verts, groups, src_ply):
    """Split the single corrected group into the original's layers by nearest
    original-face centroid. Returns groups in the original layer order."""
    ov, otris, onames = read_layered_ply(src_ply)
    if not otris or not onames:
        raise RuntimeError(f"{src_ply} carries no layered faces")
    ocent = []
    for (a, b, c), layer in otris:
        ax, ay, az = ov[a]; bx, by, bz = ov[b]; cx, cy, cz = ov[c]
        ocent.append((((ax + bx + cx) / 3.0, (ay + by + cy) / 3.0, (az + bz + cz) / 3.0), layer))
    faces = [f for _n, fs in groups for f in fs]
    buckets = [[] for _ in onames]
    for a, b, c in faces:
        ax, ay, az = verts[a]; bx, by, bz = verts[b]; cx, cy, cz = verts[c]
        px, py, pz = (ax + bx + cx) / 3.0, (ay + by + cy) / 3.0, (az + bz + cz) / 3.0
        best, best_d = 0, None
        for (qx, qy, qz), layer in ocent:
            d = (px - qx) ** 2 + (py - qy) ** 2 + (pz - qz) ** 2
            if best_d is None or d < best_d:
                best_d, best = d, layer
        buckets[best].append((a, b, c))
    return [(onames[k], buckets[k]) for k in range(len(onames))]


def main(argv):
    if len(argv) not in (3, 4):
        print(__doc__)
        return 2
    proj, dst = Path(argv[1]), Path(argv[2])
    layers_src = Path(argv[3]) if len(argv) == 4 else None
    if not proj.is_file():
        print(f"error: no such .proj: {proj}")
        return 1

    with zipfile.ZipFile(proj) as archive:
        names = [n for n in archive.namelist() if n.endswith("sceneMesh.bin")]
        if not names:
            print(f"error: {proj.name} contains no sceneMesh.bin")
            return 1
        blob = archive.read(names[0])
    print(f"read {names[0]}: {len(blob)} bytes")

    version, verts, groups = read_nodes(blob)
    print(f"cbin version {version[0]}.{version[1]}: {len(verts)} vertices, {len(groups)} group(s)")
    if not verts or not groups:
        print("error: no geometry recovered")
        return 1

    limit = len(verts)
    bad = sum(1 for _n, fs in groups for f in fs if max(f) >= limit)
    if bad:
        print(f"error: {bad} face(s) index past the {limit} vertices present")
        return 1

    if layers_src is not None:
        if not layers_src.is_file():
            print(f"error: no such layered ply: {layers_src}")
            return 1
        groups = assign_layers(verts, groups, layers_src)
        empty = [n for n, fs in groups if not fs]
        if empty:
            print(f"warning: {len(empty)} layer(s) received no faces: {', '.join(empty)}")
    else:
        print("note: no layered .ply given -- output is ONE group, acoustically uniform")

    for name, faces in groups:
        print(f"  {name!r:24} {len(faces)} faces")
    dst.parent.mkdir(parents=True, exist_ok=True)
    total = write_layered_ply(dst, verts, groups)
    print(f"wrote {dst} ({total} faces, {dst.stat().st_size} bytes)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
