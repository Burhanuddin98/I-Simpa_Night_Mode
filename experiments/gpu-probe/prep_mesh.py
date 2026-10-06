"""A1 (PLAN.md): turns a run's solve folder into the mesh-walk probe's input, `<out>.walk`.
Reads tetramesh.mbin (docs/formats/mbin.md), mesh.cbin (docs/formats/cbin.md) and config.xml; writes,
little-endian:
  u32 T, u32 F, u32 R, i32 src_tet
  f32 src[3], f32 rx_radius, f32 c_sound, f32 pad
  T x 4 x f32[4]  each face's plane (nx, ny, nz, d), unit normal pointing OUT of the tetrahedron,
                  n.x = d on the face (face i is opposite vertex i)
  T x 4 x i32     neighbour across face i (-1: none)
  T x 4 x i32     scene face index of face i (-1: interior)
  F x f32         each scene face's absorption at the band
  R x f32[3]      the point receivers
Imports nothing from the codebase.
usage: python prep_mesh.py <solve-dir> <freq-hz> <source-name> <out.walk>"""
import struct, sys, math
import xml.etree.ElementTree as ET
from pathlib import Path

solve, freq, src_name, out = Path(sys.argv[1]), int(sys.argv[2]), sys.argv[3], Path(sys.argv[4])

# --- mbin
b = (solve / "tetramesh.mbin").read_bytes()
T, N = struct.unpack_from("<II", b, 0)
nodes = list(struct.iter_unpack("<3f", b[8:8 + 12 * N]))
tets = []
o = 8 + 12 * N
for t in range(T):
    v = struct.unpack_from("<25i", b, o + 100 * t)
    corners = v[0:4]
    faces = [v[5 + 5 * f: 10 + 5 * f] for f in range(4)]  # a, b, c, marker, neighbor
    tets.append((corners, faces))
assert len(b) == 8 + 12 * N + 100 * T, "mbin size"

# --- cbin faces' material id
c = (solve / "mesh.cbin").read_bytes()
V = struct.unpack_from("<I", c, 20)[0]
F = struct.unpack_from("<I", c, 292 + 12 * V)[0]
face_mat = [struct.unpack_from("<IIIIii", c, 296 + 12 * V + 24 * i)[3] for i in range(F)]

# --- config.xml: absorption per material at the band, source, receivers, radius, sound speed
root = ET.parse(solve / "config.xml").getroot()
alpha_by_mat = {}
for ts in root.iter("type_surface"):
    for bf in ts.iter("bfreq"):
        if int(float(bf.get("freq"))) == freq:
            alpha_by_mat[int(ts.get("id"))] = float(bf.get("absorb"))
alpha = [alpha_by_mat.get(m, 0.0) for m in face_mat]
missing = sorted({m for m in face_mat if m not in alpha_by_mat})
src = next(s for s in root.iter("source") if s.get("name") == src_name)
src_p = [float(src.get(k)) for k in "xyz"]
rx = [[float(r.get(k)) for k in "xyz"] for r in root.iter("recepteur_ponctuel")]
sim = root.find("simulation")
rx_r = float(sim.get("rayon_recepteurp"))
cond = root.find("condition_atmospherique")
temp_c = float(cond.get("temperature")) if cond is not None and cond.get("temperature") else 20.0
c_sound = 331.4 * math.sqrt(1 + temp_c / 273.15)  # probe only: SPPS's own formula is A2's job

# --- planes
def sub(a, b): return [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
def cross(a, b): return [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
def dot(a, b): return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
planes, neigh, mark = [], [], []
for corners, faces in tets:
    P = [nodes[i] for i in corners]
    for f in range(4):
        tri = [P[k] for k in range(4) if k != f]
        n = cross(sub(tri[1], tri[0]), sub(tri[2], tri[0]))
        L = math.sqrt(dot(n, n)) or 1.0
        n = [x / L for x in n]
        d = dot(n, tri[0])
        if dot(n, P[f]) > d:  # the opposite vertex must be inside: flip to point out
            n = [-x for x in n]; d = -d
        planes.append((n[0], n[1], n[2], d))
        neigh.append(faces[f][4] if faces[f][4] >= 0 else -1)
        mark.append(faces[f][3] if faces[f][3] >= 0 else -1)

def inside(t, p, eps=1e-6):
    return all(dot(planes[4 * t + f][:3], p) <= planes[4 * t + f][3] + eps for f in range(4))
src_tet = next((t for t in range(T) if inside(t, src_p)), -1)

with open(out, "wb") as w:
    w.write(struct.pack("<IIIi", T, F, len(rx), src_tet))
    w.write(struct.pack("<6f", *src_p, rx_r, c_sound, 0.0))
    for p in planes: w.write(struct.pack("<4f", *p))
    w.write(struct.pack(f"<{4 * T}i", *neigh))
    w.write(struct.pack(f"<{4 * T}i", *mark))
    w.write(struct.pack(f"<{F}f", *alpha))
    for r in rx: w.write(struct.pack("<3f", *r))
scene_faces = sum(1 for m in mark if m >= 0)
print(f"{out}: {T} tetrahedra, {N} nodes, {F} scene faces ({scene_faces} tetra faces on them), "
      f"{len(rx)} receivers r={rx_r} m, source {src_name} {src_p} in tetra {src_tet}, c={c_sound:.2f} m/s, "
      f"alpha at {freq} Hz {min(alpha):.4f}-{max(alpha):.4f}, materials missing {missing}")
