"""Extract upstream tutorial 2's own TetGen mesh of the hall, and check its face order.

    python tools/fixture-gen/extract_tutorial2_mesh.py <upstream-root> <out-dir>
        [--room tests/fixtures/rooms/elmia_corrected.simpa]

Reads src/isimpa/resources/doc/tutorial/tutorial 2/tutorial_2.proj (a zip) from the upstream
checkout and writes its instance1/temp/scene_mesh.{poly,1.node,1.ele,1.face,1.neigh} to
<out-dir>, unchanged. This is gate M6(c)'s floor mesh (docs/m5-m6-design.md, "Gate
amendments"). It is about 18 MB: extract it at gate time, never commit it.

Then it checks that .poly facet i is elmia_corrected.simpa face i, with the same three
vertices compared as f32 in any order, and that facet i carries marker i. The .1.face markers are
.poly facet markers, so only then does the floor mesh index elmia_corrected's .cbin faces
correctly. Exit 1 when any facet differs. Measured 2026-09-23 at upstream 929a5c8: 7,860 of
7,860, so no remapping is needed. It also counts facets wound the same way as their face.
"""
import argparse
import json
import struct
import sys
import zipfile
from pathlib import Path

PROJ = "src/isimpa/resources/doc/tutorial/tutorial 2/tutorial_2.proj"
MEMBERS = ("scene_mesh.poly", "scene_mesh.1.node", "scene_mesh.1.ele", "scene_mesh.1.face",
           "scene_mesh.1.neigh")
PREFIX = "instance1/temp/"
REPO = Path(__file__).resolve().parents[2]


def f32(x: float) -> float:
    return struct.unpack("<f", struct.pack("<f", x))[0]


def data_lines(text: str):
    for line in text.splitlines():
        s = line.split("#", 1)[0].strip()
        if s:
            yield s.split()


def read_poly(text: str) -> tuple[dict, list]:
    """Nodes by their 1-based index, and facets in file order as (marker, (a, b, c))."""
    it = data_lines(text)
    n = int(next(it)[0])
    nodes = {}
    for _ in range(n):
        t = next(it)
        nodes[int(t[0])] = tuple(f32(float(v)) for v in t[1:4])
    facets = []
    for _ in range(int(next(it)[0])):
        head, poly = next(it), next(it)
        if head[0] != "1" or poly[0] != "3":
            sys.exit(f"facet {len(facets)} is not one triangle")
        facets.append((int(head[2]), tuple(int(v) for v in poly[1:4])))
    return nodes, facets


def rotations(t: tuple) -> set:
    return {t, (t[1], t[2], t[0]), (t[2], t[0], t[1])}


def compare(nodes: dict, facets: list, room: dict) -> dict:
    """How many facets are their room face (vertex set, f32), carry their own index as marker,
    and are wound the same way."""
    verts = [tuple(f32(c) for c in v) for v in room["geometry"]["vertices"]]
    faces = [tuple(f[:3]) for f in room["geometry"]["faces"]]
    same = marker = winding = 0
    first_bad = None
    for i, ((m, tri), face) in enumerate(zip(facets, faces)):
        p = tuple(nodes[k] for k in tri)
        r = tuple(verts[k] for k in face)
        if sorted(p) == sorted(r):
            same += 1
            winding += p in rotations(r)
        elif first_bad is None:
            first_bad = i
        marker += m == i
    return {"facets": len(facets), "faces": len(faces), "same": same, "marker": marker,
            "winding": winding, "first_bad": first_bad}


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.strip().splitlines()[0])
    ap.add_argument("upstream", type=Path)
    ap.add_argument("out", type=Path)
    ap.add_argument("--room", type=Path, default=REPO / "tests/fixtures/rooms/elmia_corrected.simpa")
    a = ap.parse_args()
    z = zipfile.ZipFile(a.upstream / PROJ)
    a.out.mkdir(parents=True, exist_ok=True)
    data = {m: z.read(PREFIX + m) for m in MEMBERS}
    for m, b in data.items():
        (a.out / m).write_bytes(b)
    print(f"extracted {len(MEMBERS)} files, {sum(map(len, data.values())):,} bytes, to {a.out}")

    nodes, facets = read_poly(data["scene_mesh.poly"].decode("ascii"))
    c = compare(nodes, facets, json.loads(a.room.read_text(encoding="utf-8")))
    print(f"facet i is face i (vertex sets as f32): {c['same']} of {c['faces']} "
          f"({c['facets']} facets); marker i on facet i: {c['marker']}; "
          f"same winding: {c['winding']}")
    ok = c["facets"] == c["faces"] and c["same"] == c["faces"] and c["marker"] == c["faces"]
    if not ok:
        print(f"MISMATCH: the markers do not index {a.room.name}'s faces"
              + (f" (first differing facet {c['first_bad']})" if c["first_bad"] is not None else ""))
        sys.exit(1)
    print(f"the floor mesh's markers index {a.room.name}'s faces directly")


if __name__ == "__main__":
    main()
