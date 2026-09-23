"""Write the SPPS run-folder fixtures built on upstream's 5 m lib_interface cube.

    python tools/fixture-gen/mkcfg.py <lib_interface-dir> <runs-dir>

<lib_interface-dir> holds upstream's `cube.cbin` and `cube_mesh.mbin`
(tests/fixtures/upstream/lib_interface/). Each case becomes <runs-dir>/<case>/ with config.xml,
mesh.cbin and tetramesh.mbin, replacing any folder of that name. Nothing else is touched.

Ported from the contract survey's scratch mkcfg.py (docs/rebuild-plan-raw-2026-09-23.json:96,
:1543): the same template, 2 bands, 2,000 particles, seed 1, energetic mode. Changes: the
working directory is the placeholder, separators are always backslashes, files are written as
bytes, and the cases the survey made by hand (noeps; the truncated scene mesh) or never ran
(a missing, an empty and a degenerate .mbin; the gradient, vertex and on-face sources) are in
the table.
"""
import sys
from pathlib import Path

import fixture_common as fc

TEMPLATE = """<?xml version="1.0" encoding="utf-8"?>
<configuration workingdirectory="{wd}">
  <simulation recepteurss_directory="Surface receiver\\" recepteurss_filename="Sound level.csbin" recepteurss_cut_filename="rs_cut.csbin" receiversp_directory="Punctual receivers" receiversp_filename="Sound level.recp" receiversp_filename_adv="Advanced sound level.gap" cumul_filename="Total energy.recp" modelName="mesh.cbin" tetrameshFileName="tetramesh.mbin" abs_atmo_calc="1" enc_calc="1" trans_calc="1" direct_calc="0" duree_simulation="0.5" output_recp_bysource="0" output_recs_byfreq="1" surf_receiv_method="0" random_seed="1" computation_method="1" nbparticules="{npart}" nbparticules_rendu="10" pasdetemps="0.001" rayon_recepteurp="0.31" trans_epsilon="5" stats_filename="SPPS particle statistics.gabe" particules_directory="particles\\" particules_filename="particles.pbin" directivities_directory="" direct_recepteurSOutputName="Direct field\\" sabine_recepteurSOutputName="Total field (Sabine)\\" eyring_recepteurSOutputName="Total field (Eyring)\\">
    <freq_enum><bfreq freq="1000" docalc="1"/><bfreq freq="500" docalc="1"/></freq_enum>
  </simulation>
  <condition_atmospherique humidite="50" pression="101325" temperature="20" z0="0.02" alog="0" blin="{blin}" disable_absatmo_computation="0" absatmo="0"/>
  <surface_absorption_enum>
    <type_surface id="{mat}" side_material="1"><bfreq freq="500" absorb="0.2" diffusion="0.1" loi="1"/><bfreq freq="1000" absorb="0.3" diffusion="0.1" loi="1"/></type_surface>
  </surface_absorption_enum>
  <sources>
    <source id="1" name="S1" x="{sx}" y="{sy}" z="{sz}" delay="0" u="1" v="0" w="0" directivite="{dirtype}"{dirfile}>{srcbands}</source>
  </sources>
  <recepteursp>
    <recepteur_ponctuel id="2" lbl="R1" x="3.6" y="3.1" z="1.4" u="1" v="0" w="0"/>
  </recepteursp>
  <recepteurss/>
  <encombrement_enum/>
</configuration>
"""

PARTICLES = 2000
TWO = '<bfreq freq="500" db="90"/><bfreq freq="1000" db="90"/>'
ONE = '<bfreq freq="1000" db="90"/>'
SOURCE = ("1.2", "1.7", "2.1")

# name: dict of the fields that differ from spps_ok. cbin/mbin are byte transforms of upstream's
# cube files; "drop" removes a config attribute.
CASES = {
    "spps_ok": {},
    "spps_noeps": {"drop": "trans_epsilon"},
    "spps_oneband": {"srcbands": ONE},
    "spps_dirmiss": {"dirtype": 5, "dirfile": ' directivity_file="nope.txt"'},
    "spps_dirempty": {"dirtype": 5},
    "spps_mat0miss": {"mat": 5},
    "spps_mat7miss": {"mat": 5, "cbin": lambda b: fc.cbin_set_material(b, 7)},
    "spps_srcout": {"source": ("12.0", "1.7", "2.1")},
    # The survey's fx/trunc.cbin: the first 100 bytes of cube.cbin, inside the vertex block.
    "spps_unreadable_mesh": {"cbin": lambda b: b[:100]},
    "spps_nomesh": {"no_mbin": True},
    "spps_emptymesh": {"mbin": lambda b: fc.EMPTY_MBIN},
    "spps_degenerate": {"mbin": fc.mbin_repeat_corner},
    # A sound-speed gradient makes both solvers compute the ground height of every tetrahedron.
    "spps_gradient": {"blin": "0.001"},
    # A cube corner is a vertex of the mesh: SPPS moves the source towards the tetrahedron.
    "spps_srcvertex": {"source": ("0", "0", "0")},
    # spps_ok's source dropped to the floor, inside cube.cbin face 0 ((5,0,0), (0,0,0), (0,5,0)).
    "spps_srcface": {"source": ("1.2", "1.7", "0")},
}


def config(case: dict) -> str:
    sx, sy, sz = case.get("source", SOURCE)
    xml = TEMPLATE.format(
        wd=fc.PLACEHOLDER,
        npart=PARTICLES,
        mat=case.get("mat", 0),
        sx=sx,
        sy=sy,
        sz=sz,
        srcbands=case.get("srcbands", TWO),
        dirtype=case.get("dirtype", 0),
        dirfile=case.get("dirfile", ""),
        blin=case.get("blin", "0"),
    )
    if "drop" in case:
        xml = fc.drop_attr(xml, case["drop"])
    return xml


def main() -> None:
    if len(sys.argv) != 3:
        fc.die(__doc__.strip().splitlines()[2].strip())
    lib, runs = Path(sys.argv[1]), Path(sys.argv[2])
    cbin = (lib / "cube.cbin").read_bytes()
    mbin = (lib / "cube_mesh.mbin").read_bytes()
    for name, case in CASES.items():
        d = fc.fresh_dir(runs / name)
        fc.write_bytes(d / "mesh.cbin", case.get("cbin", lambda b: b)(cbin))
        if not case.get("no_mbin"):
            fc.write_bytes(d / "tetramesh.mbin", case.get("mbin", lambda b: b)(mbin))
        fc.write_text(d / "config.xml", config(case))
        fc.check_self_contained(d)
        print("wrote", name)


if __name__ == "__main__":
    main()
