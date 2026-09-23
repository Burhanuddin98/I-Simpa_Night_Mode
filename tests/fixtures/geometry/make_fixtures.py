"""Writes the hand-made mesh import fixtures in this folder (M4 gate item (f)).

Run from anywhere: `python tests/fixtures/geometry/make_fixtures.py`. The output is
deterministic; `tests/geometry_import_meshes.rs` asserts what each file must import to.

Every model is a closed box, wound outward (counter-clockwise seen from outside):

    v0 (x0,y0,z0) v1 (x1,y0,z0) v2 (x1,y1,z0) v3 (x0,y1,z0)
    v4 (x0,y0,z1) v5 (x1,y0,z1) v6 (x1,y1,z1) v7 (x0,y1,z1)

    quads: z0 (0,3,2,1)  z1 (4,5,6,7)  y0 (0,1,5,4)  x1 (1,2,6,5)  y1 (2,3,7,6)  x0 (3,0,4,7)
"""

import os
import struct

HERE = os.path.dirname(os.path.abspath(__file__))

QUADS = {
    "z0": (0, 3, 2, 1),
    "z1": (4, 5, 6, 7),
    "y0": (0, 1, 5, 4),
    "x1": (1, 2, 6, 5),
    "y1": (2, 3, 7, 6),
    "x0": (3, 0, 4, 7),
}
ORDER = ["z0", "z1", "y0", "x1", "y1", "x0"]


def corners(x0, x1, y0, y1, z0, z1):
    return [
        (x0, y0, z0), (x1, y0, z0), (x1, y1, z0), (x0, y1, z0),
        (x0, y0, z1), (x1, y0, z1), (x1, y1, z1), (x0, y1, z1),
    ]


def triangles():
    """The 12 triangles of the box, fan-split from each quad's first vertex, in ORDER."""
    out = []
    for side in ORDER:
        a, b, c, d = QUADS[side]
        out.append((side, (a, b, c)))
        out.append((side, (a, c, d)))
    return out


def write(name, data):
    path = os.path.join(HERE, name)
    mode = "wb" if isinstance(data, bytes) else "w"
    kwargs = {} if isinstance(data, bytes) else {"newline": "\n", "encoding": "ascii"}
    with open(path, mode, **kwargs) as f:
        f.write(data)


def normal(p, q, r):
    u = [q[k] - p[k] for k in range(3)]
    v = [r[k] - p[k] for k in range(3)]
    n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]]
    length = sum(c * c for c in n) ** 0.5
    return [c / length for c in n]


# ---------------------------------------------------------------------------------------------
# STL: a 6 x 10 x 3 m box, ASCII and its binary twin. 36 vertex records, 8 distinct points.

STL_BOX = corners(0.0, 6.0, 0.0, 10.0, 0.0, 3.0)


def stl_ascii():
    lines = ["solid box"]
    for _, (a, b, c) in triangles():
        p = [STL_BOX[a], STL_BOX[b], STL_BOX[c]]
        n = normal(*p)
        lines.append("  facet normal %g %g %g" % tuple(n))
        lines.append("    outer loop")
        for v in p:
            lines.append("      vertex %g %g %g" % v)
        lines.append("    endloop")
        lines.append("  endfacet")
    lines.append("endsolid box")
    return "\n".join(lines) + "\n"


def stl_binary():
    # The header starts with "solid" on purpose: many exporters do that, and the reader must not
    # take the word alone as proof of an ASCII file.
    header = b"solid box, binary twin of box.stl".ljust(80, b" ")
    tris = triangles()
    out = bytearray(header + struct.pack("<I", len(tris)))
    for _, (a, b, c) in tris:
        p = [STL_BOX[a], STL_BOX[b], STL_BOX[c]]
        out += struct.pack("<3f", *normal(*p))
        for v in p:
            out += struct.pack("<3f", *v)
        out += struct.pack("<H", 0)
    return bytes(out)


# ---------------------------------------------------------------------------------------------
# OBJ: a box modelled Y-up, 6 wide (x), 3 high (y), 10 deep (z). Groups by usemtl (floor,
# ceiling, walls) and, differently, by g (one per side), so the two groupings can be told apart.

OBJ_BOX = corners(0.0, 6.0, 0.0, 3.0, 0.0, 10.0)
# In a Y-up file the y0 side is the floor and y1 the ceiling.
OBJ_MATERIAL = {"y0": "floor", "y1": "ceiling", "z0": "walls", "z1": "walls", "x0": "walls", "x1": "walls"}


def obj():
    lines = [
        "# Box modelled Y-up: x 0..6, y 0..3 (height), z 0..10.",
        "# Imported --up z it keeps these axes; --up y maps (x, y, z) to (x, -z, y).",
        "mtllib box.mtl",
        "o room box",
    ]
    for v in OBJ_BOX:
        lines.append("v %g %g %g" % v)
    lines.append("vt 0 0")
    lines.append("vn 0 1 0")
    lines.append("s off")
    for i, side in enumerate(ORDER):
        q = QUADS[side]
        lines.append("g side %s" % side)
        lines.append("usemtl %s" % OBJ_MATERIAL[side])
        if i % 3 == 0:
            refs = ["%d" % (v + 1) for v in q]
        elif i % 3 == 1:
            refs = ["%d/1/1" % (v + 1) for v in q]
        else:
            # Relative references: -8 is the first of the 8 vertices.
            refs = ["%d//1" % (v - 8) for v in q]
        lines.append("f " + " ".join(refs))
    return "\n".join(lines) + "\n"


# ---------------------------------------------------------------------------------------------
# PLY, upstream's layered convention: a box with decimal coordinates, as quads with a layer_id,
# and a layer element naming the layers. ASCII, binary big-endian and little-endian twins.

PLY_BOX = corners(-0.35, 5.65, 0.1, 10.1, 0.0, 3.3)
PLY_LAYERS = ["floor", "ceiling", "walls", "unused layer"]
PLY_LAYER_OF = {"z0": 0, "z1": 1, "y0": 2, "x1": 2, "y1": 2, "x0": 2}


def ply_float(encoding):
    header = [
        "ply",
        "format %s 1.0" % encoding,
        "comment box with upstream's layer_id and layer_name convention",
        "element vertex 8",
        "property float x",
        "property float y",
        "property float z",
        "element face 6",
        "property list uchar int vertex_indices",
        "property int layer_id",
        "element layer %d" % len(PLY_LAYERS),
        "property list uchar uchar layer_name",
        "end_header",
    ]
    head = ("\n".join(header) + "\n").encode("ascii")
    if encoding == "ascii":
        body = []
        for v in PLY_BOX:
            body.append("%s %s %s" % tuple(repr(c) for c in v))
        for side in ORDER:
            body.append("4 %d %d %d %d %d" % (QUADS[side] + (PLY_LAYER_OF[side],)))
        for name in PLY_LAYERS:
            body.append(" ".join([str(len(name))] + [str(ord(ch)) for ch in name]))
        return head + ("\n".join(body) + "\n").encode("ascii")
    e = ">" if encoding == "binary_big_endian" else "<"
    out = bytearray(head)
    for v in PLY_BOX:
        out += struct.pack(e + "3f", *v)
    for side in ORDER:
        out += struct.pack(e + "B4ii", 4, *QUADS[side], PLY_LAYER_OF[side])
    for name in PLY_LAYERS:
        out += struct.pack(e + "B", len(name)) + name.encode("ascii")
    return bytes(out)


# The same idea with every fix Night Mode's reader needed: integer vertex coordinates (int16 x,
# int32 y), a double z, an extra vertex property between them, ushort list counts, uint indices,
# a uchar layer_id, a uint-counted char layer name, and an element the reader must skip. Faces
# are triangles here, listed in the order `triangles()` gives.

MIXED_BOX = corners(0, 6, 0, 10, 0.0, 3.0)
MIXED_LAYERS = ["floor", "ceiling", "walls"]


def ply_mixed(encoding):
    header = [
        "ply",
        "format %s 1.0" % encoding,
        "element vertex 8",
        "property short x",
        "property uchar confidence",
        "property int y",
        "property double z",
        "element junk 2",
        "property list uchar float stuff",
        "property ushort tag",
        "element face 12",
        "property ushort flags",
        "property list ushort uint vertex_indices",
        "property uchar layer_id",
        "element layer 3",
        "property list uint char layer_name",
        "end_header",
    ]
    head = ("\n".join(header) + "\n").encode("ascii")
    junk = [([0.5, -1.25], 7), ([], 65535)]
    tris = triangles()
    if encoding == "ascii":
        body = []
        for x, y, z in MIXED_BOX:
            body.append("%d 200 %d %s" % (x, y, repr(z)))
        for items, tag in junk:
            body.append(" ".join([str(len(items))] + [repr(v) for v in items] + [str(tag)]))
        for side, t in tris:
            body.append("9 3 %d %d %d %d" % (t + (PLY_LAYER_OF[side],)))
        for name in MIXED_LAYERS:
            body.append(" ".join([str(len(name))] + [str(ord(ch)) for ch in name]))
        return head + ("\n".join(body) + "\n").encode("ascii")
    e = ">" if encoding == "binary_big_endian" else "<"
    out = bytearray(head)
    for x, y, z in MIXED_BOX:
        out += struct.pack(e + "hBid", x, 200, y, z)
    for items, tag in junk:
        out += struct.pack(e + "B", len(items)) + b"".join(struct.pack(e + "f", v) for v in items)
        out += struct.pack(e + "H", tag)
    for side, t in tris:
        out += struct.pack(e + "HH3IB", 9, 3, *t, PLY_LAYER_OF[side])
    for name in MIXED_LAYERS:
        out += struct.pack(e + "I", len(name)) + name.encode("ascii")
    return bytes(out)


def main():
    write("box.stl", stl_ascii())
    write("box_binary.stl", stl_binary())
    write("box_yup.obj", obj())
    write("box.ply", ply_float("ascii"))
    write("box_be.ply", ply_float("binary_big_endian"))
    write("box_le.ply", ply_float("binary_little_endian"))
    write("box_mixed.ply", ply_mixed("ascii"))
    write("box_mixed_be.ply", ply_mixed("binary_big_endian"))


if __name__ == "__main__":
    main()
