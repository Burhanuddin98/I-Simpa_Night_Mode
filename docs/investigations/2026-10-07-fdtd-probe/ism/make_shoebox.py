"""A shoebox .simpa (7.3 x 5.1 x 3.2 m, one material, alpha 0.10 flat, scattering 0, double-sided as BRAS) built
from bras_cr2.simpa's schema: the bed that separates SPPS from the polyhedral image-source code, since the box's
exact answer is the closed-form image lattice. Writes C:\\tmp\\nm-spps-projects\\shoebox_s0.simpa."""
import copy, itertools, json, uuid
from pathlib import Path

L = (7.3, 5.1, 3.2)
src = Path(r'C:\repos\Room-Acoustics-Engine\I-Simpa FDTD\app\src-tauri\examples\bras_cr2.simpa')
p = json.loads(src.read_text(encoding='utf-8'))
V = [list(v) for v in itertools.product([0, L[0]], [0, L[1]], [0, L[2]])]


def quad(i, j, k, l):
    return [[i, j, k], [i, k, l]]


F = quad(0, 1, 3, 2) + quad(4, 6, 7, 5) + quad(0, 4, 5, 1) + quad(2, 3, 7, 6) + quad(0, 2, 6, 4) + quad(1, 5, 7, 3)
mat = copy.deepcopy(p['materials'][0])
mat.update(id=str(uuid.uuid4()), name='box_wall', absorption=[0.10] * 6, scattering=[0.0] * 6, double_sided=True,
           transmission_loss_db=None)
grp = {'id': str(uuid.uuid4()), 'name': 'box', 'material': mat['id']}
p['geometry'] = {'vertices': V, 'faces': [f + [grp['id']] for f in F]}
p['materials'] = [mat]
p['surface_groups'] = [grp]
s0 = copy.deepcopy(p['sources'][0]); s0.update(id=str(uuid.uuid4()), name='LS1', position=[1.9, 2.2, 1.4])
p['sources'] = [s0]
rx = []
for name, pos in [('MP1', [5.6, 3.9, 1.1]), ('MP2', [3.1, 1.0, 2.3]), ('MP3', [6.4, 0.8, 0.7])]:
    r = copy.deepcopy(p['point_receivers'][0]); r.update(id=str(uuid.uuid4()), name=name, position=pos); rx.append(r)
p['point_receivers'] = rx
p['surface_receivers'] = []
p['fitting_zones'] = []
p['variants'] = []; p['active_variant'] = None
p['name'] = 'shoebox_s0'
p['solvers']['spps'].update(particles_per_source=10_000_000, duration_s=0.3)
out = Path(r'C:\tmp\nm-spps-projects\shoebox_s0.simpa')
out.write_text(json.dumps(p, indent=1), encoding='utf-8')
print('wrote', out, 'receiver radius', p['solvers']['spps']['receiver_radius_m'])
