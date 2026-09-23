"""Judge recorded solver runs against docs/solver-contract.md and write each expected.json.

    python tools/fixture-gen/mkexpected.py <runs-dir> <observed-root> --simpa <simpa.exe>
        [--contract docs/solver-contract.md] [--upstream <root>] [--check]

<observed-root> is what runsolvers.ps1 wrote (-Out): <case>/solve/, solver.stdout.txt,
solver.stderr.txt and run.json per real case. stub_* fixtures are judged from their stub.json.

For every fixture this script computes, independently of the Rust crates:
1. the solver's own verdict, from the Part B tables it parses out of the contract page (22 line
   patterns, exit classes) and from the post-run checks (statistics, expected files, TCR's
   non-finite values), read through `simpa dump`;
2. the pre-launch mesh check `run-folder` makes (docs/m5-m6-design.md, "Run folder"): the
   decision-6 invariants and the VerifyReport counts, on the fixture's .mbin and .cbin;
3. the final verdict: mesh_invalid plus the failing counts when (2) fails, else (1).

It then compares both verdicts with EXPECT, the expectation written down before the runs with
its receipt, and stops with exit 1 on any disagreement, writing nothing. Only when every case
agrees does it write <case>/expected.json and <runs-dir>/README.md (with --check it compares
them with the files on disk instead). Stub texts are checked against a real run's output or the
string literal in upstream's source.

Verdict rules (docs/solver-contract.md Part B, "Exit codes" and "Judging a run"):
- `codes` holds the decisive reasons: the FAIL-class lines and the exit class
  (exit_nonzero, crash_*). Only when neither says anything do the post-run checks decide
  (stats_*, particle_*, expected_file_missing, nonfinite_result). Post-run findings after a
  crash or a fatal line are consequences, kept under observed.post_run, not in `codes`.
- status CRASH for an exit >= 0xC0000000, else FAIL when `codes` is non-empty, else OK. The one
  exception is 0xFFFFFFFF (-1): the exit tables call it FAIL (SC:264, SC:275), although SC:278's
  rule alone would make it crash_other. It is exit_nonzero here.
- WARN-class lines go to `warnings`; they never change the status.
- The loss limit is design decision 9's proposed default, 0.01.
"""
import argparse
import collections
import hashlib
import json
import math
import re
import struct
import subprocess
import sys
import xml.etree.ElementTree as ET
from dataclasses import dataclass
from pathlib import Path

import fixture_common as fc

REPO = Path(__file__).resolve().parents[2]
LOSS_LIMIT = 0.01
ROW_COUNT = 22
CLASSES = {"PROGRESS", "INFO", "OK", "FAIL", "WARN"}
CRASH = {0xC0000005: "crash_access_violation", 0xC0000409: "crash_abort"}

# ---------------------------------------------------------------------------------------------
# What each case is expected to give, written from the contract before running it. `codes` and
# `status` are the verdict of `simpa run-folder`; `observed_*` is the solver's own verdict when
# the pre-launch mesh check refuses the folder (and so the solver would never start).

EXPECT = {
    "spps_ok": dict(
        status="OK", codes=[],
        receipt="Positive baseline: exit 0 with spps_end_of_calculation (SC:343), band columns 500 "
        "and 1000 Hz, total 2000 = nbparticules x 1 source per band and no loss (SC:352-361), every "
        "expected file present (SC:224-241)."),
    "spps_noeps": dict(
        status="FAIL", codes=["xml_property_missing"],
        receipt="trans_epsilon absent: row xml_property_missing (SC:314, cxml.cpp:115) is FAIL. Every "
        "particle dies at the first step, absorbed by the atmosphere, exit 0 (SC:98, VERIFIED S "
        "run_noeps; RAW:1387)."),
    "spps_oneband": dict(
        status="OK", codes=[],
        receipt="Source spectrum with 1 of 2 bands. No Part B signal fires: exit 0, no FAIL line, "
        "totals 2000 per band, no loss; the 1000 Hz band loses every particle to the atmosphere at "
        "the first step instead. Only the project rule band_set_mismatch (SC:55, VERIFIED S "
        "run_oneband; RAW:1471) catches it, and run-folder runs no validator."),
    "spps_dirmiss": dict(
        status="FAIL", codes=["directivity_not_open"],
        receipt="directivity_file names a missing file: row directivity_not_open (SC:319, "
        "directivityParser.cpp:46), exit 0, the source emits nothing (SC:82, VERIFIED S "
        "run_dirmiss; RAW:1194, RAW:1465)."),
    "spps_dirempty": dict(
        status="CRASH", codes=["xml_property_missing", "crash_access_violation"],
        receipt="Balloon source without a directivity_file attribute: row xml_property_missing, "
        "then a NULL directivity and 0xC0000005 (SC:82, SC:266, VERIFIED S run_dirempty; "
        "RAW:1465)."),
    "spps_mat0miss": dict(
        status="CRASH", codes=["crash_access_violation"],
        receipt="Faces use material 0, the config declares only 5: no check, 0xC0000005 with no "
        "message (SC:61, SC:266, VERIFIED S run_mat0miss; RAW:1477)."),
    "spps_mat7miss": dict(
        status="FAIL", codes=["material_missing", "exit_nonzero"],
        receipt="Faces use material 7, undeclared: row material_missing on stderr (SC:321, "
        "coreinitialisation.cpp:430) and exit 0xFFFFFFFF (SC:264, VERIFIED S run_mat7miss; "
        "RAW:1477)."),
    "spps_srcout": dict(
        status="CRASH", codes=["crash_access_violation"],
        receipt="Source at x = 12 m in a 5 m cube: NULL tetrahedron, 0xC0000005 with no message "
        "(SC:71, SC:266, VERIFIED S run_srcout; RAW:1435)."),
    "spps_unreadable_mesh": dict(
        status="FAIL", codes=["mesh_invalid"],
        observed_status="FAIL", observed_codes=["scene_mesh_unreadable"],
        receipt="mesh.cbin truncated to 100 bytes. run-folder cannot read the .cbin for mesh::verify, "
        "so it refuses before launch (m5-m6-design.md, Run folder). Run anyway, SPPS prints row "
        "scene_mesh_unreadable and the path (SC:315, coreinitialisation.cpp:396) and exits 0 "
        "(SC:263; RAW:1201)."),
    "spps_nomesh": dict(
        status="FAIL", codes=["mesh_invalid"],
        observed_status="FAIL", observed_codes=["tetra_mesh_unreadable"],
        receipt="No tetramesh.mbin: nothing for mesh::verify, refused before launch. Run anyway, SPPS "
        "prints row tetra_mesh_unreadable (SC:316, coreinitialisation.cpp:456) and exits 0 "
        "(SC:263; RAW:1267)."),
    "spps_emptymesh": dict(
        status="FAIL", codes=["mesh_invalid", "uncovered_scene_faces"],
        observed_status="FAIL", observed_codes=["tetra_mesh_empty", "tetra_mesh_unreadable"],
        receipt="tetramesh.mbin with T = N = 0: no tetrahedron face carries any of the 12 scene "
        "faces. Run anyway, SPPS prints the path, row tetra_mesh_empty (SC:317, coreTypes.cpp:241) "
        "and row tetra_mesh_unreadable (coreinitialisation.cpp:456), exit 0."),
    "spps_degenerate": dict(
        status="FAIL", codes=["mesh_invalid", "degenerate_tets"],
        observed_status="FAIL", observed_codes=["degenerate_tetrahedron", "exit_nonzero"],
        receipt="Tetrahedron 0 with corner D = corner A. Run anyway, SPPS prints row "
        "degenerate_tetrahedron without a newline (SC:322, coreTypes.cpp:213) and exits 1 "
        "(SC:265, first run of this case: the contract had it 'not run')."),
    "spps_gradient": dict(
        status="OK", codes=[],
        receipt="blin = 0.001: a sound-speed gradient, so both ground_height lines (SC:311, "
        "coreinitialisation.cpp:139, 148, INFO); otherwise spps_ok."),
    "spps_srcvertex": dict(
        status="OK", codes=[], warnings=["source_moved_off_vertex"],
        receipt="Source at the cube corner (0,0,0), a mesh vertex: row source_moved_off_vertex (SC:320, "
        "sppsInitialisation.cpp:27-29, WARN) moves it 0.5 % towards the tetrahedron's centre, "
        "and the run completes."),
    "spps_srcface": dict(
        status="FAIL", codes=["source_on_surface"],
        receipt="Source at (1.2, 1.7, 0), inside floor triangle 0: row source_on_surface on stderr "
        "(SC:323, sppsNantes.cpp:322-325), exit 0 with no results (SC:72)."),
    "spps_lossy": dict(
        status="FAIL", codes=["mesh_invalid", "unmarked_boundary_faces"],
        observed_status="FAIL", observed_codes=["particle_loss_reported"],
        receipt="Every neighbour link cut to -2: the 12 interior faces become faces with no "
        "neighbour and no marker, which decision 6 forbids, so run-folder refuses it before launch. "
        "Run anyway, SPPS loses 4000 of 4000 particles to meshing, prints row "
        "particle_loss_reported without a newline (SC:325, sppsNantes.cpp:438) and exits 0 "
        "(SC:263, VERIFIED S run_lossy; RAW:1541)."),
    "tcr_ok": dict(
        status="OK", codes=[],
        receipt="Positive TCR baseline on spps_ok's inputs: exit 0 (SC:273), Main results.gabe and "
        "Punctual receivers\\R1.gabe (SC:251-252), no non-finite value outside the Global row."),
    "tcr_mat7miss": dict(
        status="FAIL", codes=["material_missing", "exit_nonzero"],
        receipt="As spps_mat7miss: row material_missing, exit 0xFFFFFFFF (SC:275, VERIFIED P2 "
        "tcr_mat22_miss; RAW:1477)."),
    "tcr_srcout": dict(
        status="FAIL", codes=["nonfinite_result"],
        receipt="Source outside the room: TCR exits 0 with results (SC:71, SC:273; RAW:1435), but "
        "the direct field at R1 is -inf in every band, which nonfinite_result catches (SC:365)."),
    "tcr_nomesh": dict(
        status="FAIL", codes=["mesh_invalid"],
        observed_status="FAIL", observed_codes=["tetra_mesh_unreadable", "exit_nonzero"],
        receipt="No tetramesh.mbin: refused before launch. Run anyway, TCR prints row "
        "tetra_mesh_unreadable and exits 1 (SC:274, VERIFIED S tcr_nomesh; RAW:1267)."),
    "tcr_broken_hall": dict(
        status="FAIL", codes=["mesh_invalid", "unmarked_boundary_faces", "uncovered_scene_faces"],
        observed_status="FAIL", observed_codes=["xml_property_missing"],
        receipt="Night Mode's broken-hall TCR folder of 2026-09-08 (RAW:1057-1058), meshed from the "
        "self-intersecting raw hall with 535 facets skipped and no .neigh: 267 faces with no "
        "neighbour carry no marker and 338 of the 1,086 scene faces are carried by no tetrahedron "
        "face, which decision 6 forbids, so run-folder refuses it before launch (m5-m6-design.md, "
        "Run folder). Run anyway, TCR exits 0 with results, but Night Mode's config.xml lacks "
        "disable_absatmo_computation and absatmo, so row xml_property_missing fires twice."),
    "stub_config_path_missing": dict(
        status="FAIL", codes=["config_path_missing"],
        receipt="Row config_path_missing (SC:318, sppsNantes.cpp:287): SPPS started with no "
        "argument, exit 0 (SC:190-193). run-folder always passes config.xml, so only a stub "
        "reaches it."),
    "stub_degenerate_tetrahedron": dict(
        status="FAIL", codes=["degenerate_tetrahedron", "exit_nonzero"],
        receipt="Row degenerate_tetrahedron (SC:322) with exit 1, text from spps_degenerate. The real "
        "case is refused before launch by mesh::verify."),
    "stub_source_not_located": dict(
        status="FAIL", codes=["source_not_located"],
        receipt="Row source_not_located (SC:324, sppsNantes.cpp:63), no newline, once per band so "
        "both land on one stderr line. No real run reaches it (SC:324)."),
    "stub_particle_loss_unterminated": dict(
        status="FAIL", codes=["particle_loss_reported"],
        receipt="Row particle_loss_reported (SC:325) as SPPS's last output, with no newline (M6(e): "
        "the final stderr line must be captured). Text from spps_lossy, refused before launch."),
    "stub_scene_mesh_unreadable": dict(
        status="FAIL", codes=["scene_mesh_unreadable"],
        receipt="Row scene_mesh_unreadable and its continuation path (SC:315, SC:299-301). Text from "
        "spps_unreadable_mesh, refused before launch."),
    "stub_tetra_mesh_unreadable": dict(
        status="FAIL", codes=["tetra_mesh_unreadable"],
        receipt="Row tetra_mesh_unreadable (SC:316) with SPPS's exit 0. Text from spps_nomesh, refused "
        "before launch."),
    "stub_tetra_mesh_empty": dict(
        status="FAIL", codes=["tetra_mesh_empty", "tetra_mesh_unreadable"],
        receipt="Row tetra_mesh_empty with the path line before it (SC:317, SC:299-301). Text from "
        "spps_emptymesh, refused before launch."),
    "stub_unclassified_line": dict(
        status="FAIL", codes=["stats_unreadable", "expected_file_missing"],
        warnings=["unclassified_line"],
        receipt="A line no row matches is WARN unclassified_line (SC:326); SPPS's _DEBUG-only line "
        "(CalculationCore.cpp:368). The transcript claims success, so the post-run checks decide, "
        "and a stub writes no statistics or results: stats_unreadable, expected_file_missing "
        "(SC:362, SC:369)."),
}

# Stub texts and where each is confirmed: ("observed", case) or ("source", file, line).
STUB_EVIDENCE = {
    "SPPS version 2.2.1": ("observed", "spps_ok"),
    "The path of the XML configuration file must be specified!": ("source", "spps/sppsNantes.cpp", 287),
    "Error in input mesh, a tetrahedra have at least the same two vertices idTetra:0 vertices:4 5 7 4":
        ("observed", "spps_degenerate"),
    "Unable to find the source position!Unable to find the source position!":
        ("source", "spps/sppsNantes.cpp", 63),
    "End of calculation.": ("observed", "spps_ok"),
    "Output results files.": ("observed", "spps_ok"),
    "#50": ("pattern", "progress"),
    "#100": ("pattern", "progress"),
    "Warning 4000 particles has been in error on 4000 particles. The computation result may be "
    "wrong, please check the particles statitics file for more details.": ("observed", "spps_lossy"),
    "Unable to read the scene mesh file :": ("observed", "spps_unreadable_mesh"),
    "C:\\runs\\stub\\solve\\mesh.cbin": ("path", None),
    "C:\\runs\\stub\\solve\\tetramesh.mbin": ("path", None),
    "Unable to read the tetrahedalization of the scene mesh file, calculation canceled.":
        ("observed", "spps_nomesh"),
    "Tetrahedron file is empty, the calculation can't be done !": ("observed", "spps_emptymesh"),
    "La particule va sortir du perim�tre du volume car une face du domaine est mal "
    "orient�e ou le maillage est incorrect. La particule a �t� supprim�e":
        ("source", "spps/CalculationCore.cpp", 368),
}

CONSTRUCTION = {
    "spps_ok": "the survey's run_ok: cube.cbin, cube_mesh.mbin, 2 bands, 2,000 particles, seed 1",
    "spps_noeps": "spps_ok without the trans_epsilon attribute",
    "spps_oneband": "source spectrum with the 1000 Hz band only",
    "spps_dirmiss": "balloon source (directivite 5), directivity_file=\"nope.txt\", no such file",
    "spps_dirempty": "balloon source with no directivity_file attribute",
    "spps_mat0miss": "faces use material 0; the config declares only 5",
    "spps_mat7miss": "every face's idMat set to 7; the config declares only 5",
    "spps_srcout": "source at x = 12 m, outside the 5 m cube",
    "spps_unreadable_mesh": "mesh.cbin cut to its first 100 bytes (the survey's fx/trunc.cbin)",
    "spps_nomesh": "no tetramesh.mbin",
    "spps_emptymesh": "tetramesh.mbin with T = 0, N = 0 (8 bytes)",
    "spps_degenerate": "tetramesh.mbin with tetrahedron 0's corner D set to corner A",
    "spps_gradient": "blin = 0.001 (a sound-speed gradient)",
    "spps_srcvertex": "source at the cube corner (0, 0, 0), a mesh vertex",
    "spps_srcface": "source at (1.2, 1.7, 0), on floor triangle 0",
    "spps_lossy": "spps_ok with every neighbour link in tetramesh.mbin set to -2 (mklossy.py)",
    "tcr_ok": "spps_ok's folder, run by TCR",
    "tcr_mat7miss": "spps_mat7miss's folder, run by TCR",
    "tcr_srcout": "spps_srcout's folder, run by TCR",
    "tcr_nomesh": "spps_ok's folder without tetramesh.mbin, run by TCR",
    "tcr_broken_hall": "Night Mode's build-clean/sim_output/tcr/ (2026-09-08): config.xml, "
    "model.cbin, tetramesh.mbin",
}


# ---------------------------------------------------------------------------------------------
# The contract's classifier table

@dataclass(frozen=True)
class Row:
    id: str
    stream: str
    regex: "re.Pattern | None"
    cls: str


def load_rows(contract: Path) -> list[Row]:
    """The Part B line-pattern table, parsed from the page. Exits unless it has 22 rows."""
    text = contract.read_text(encoding="utf-8")
    head = "| Pattern id / reason | Stream | Pattern | Class | Receipt |"
    if head not in text:
        fc.die(f"{contract}: no classifier table header")
    rows = []
    for line in text[text.index(head):].splitlines()[2:]:
        if not line.startswith("|"):
            break
        cells = [c.strip() for c in re.split(r"(?<!\\)\|", line)[1:-1]]
        rid = cells[0].strip("`")
        pat = re.search(r"`([^`]*)`", cells[2])
        regex = re.compile(pat.group(1).replace("\\|", "|")) if pat else None
        cls = cells[3].split(":")[0].split()[0]
        if cls not in CLASSES:
            fc.die(f"row {rid}: unknown class {cls!r}")
        rows.append(Row(rid, cells[1], regex, cls))
    if len(rows) != ROW_COUNT or len({r.id for r in rows}) != ROW_COUNT:
        fc.die(f"{contract}: {len(rows)} classifier rows, expected {ROW_COUNT} distinct")
    if [r.id for r in rows if r.regex is None] != ["unclassified_line"]:
        fc.die("only unclassified_line may lack a pattern")
    return rows


# ---------------------------------------------------------------------------------------------
# Transcripts

@dataclass
class Line:
    stream: str
    text: str
    terminated: bool
    row: str = ""
    cls: str = ""


def decode(b: bytes) -> str:
    try:
        return b.decode("utf-8")
    except UnicodeDecodeError:
        return b.decode("cp1252", errors="replace")


def split_stream(data: bytes, stream: str) -> list[Line]:
    """Lines as a reader flushing a trailing partial line at EOF sees them (SC:286-287): CR LF or
    LF ends a line; a final piece without one is a line with terminated = False."""
    parts = data.split(b"\n")
    out = []
    for i, p in enumerate(parts):
        last = i == len(parts) - 1
        if last and not p:
            break
        if p.endswith(b"\r"):
            p = p[:-1]
        out.append(Line(stream, decode(p), not last))
    return out


def classify(lines: list[Line], rows: list[Row]) -> list[Line]:
    """Each line gets the first row whose pattern matches it (anchored, SC:288-290), by content
    only. The path after scene_mesh_unreadable and the one before tetra_mesh_empty are
    continuations of those events (SC:299-301)."""
    for ln in lines:
        for r in rows:
            if r.regex is not None and r.regex.match(ln.text):
                ln.row, ln.cls = r.id, r.cls
                break
        else:
            ln.row, ln.cls = "unclassified_line", "WARN"
    for stream in ("stdout", "stderr"):
        s = [ln for ln in lines if ln.stream == stream]
        for i, ln in enumerate(s):
            if ln.row == "scene_mesh_unreadable" and i + 1 < len(s) and s[i + 1].row == "unclassified_line":
                s[i + 1].row, s[i + 1].cls = "continuation:scene_mesh_unreadable", "INFO"
            if ln.row == "tetra_mesh_empty" and i > 0 and s[i - 1].row == "unclassified_line":
                s[i - 1].row, s[i - 1].cls = "continuation:tetra_mesh_empty", "INFO"
    return lines


def unique(xs):
    return list(dict.fromkeys(xs))


def is_crash(code: int) -> bool:
    return code >= 0xC0000000 and code != 0xFFFFFFFF


def exit_class(code: int) -> list[str]:
    if is_crash(code):
        return [CRASH.get(code, "crash_other")]
    return ["exit_nonzero"] if code != 0 else []


# ---------------------------------------------------------------------------------------------
# simpa dump

class Simpa:
    def __init__(self, exe: Path):
        self.exe = exe

    def dump(self, fmt: str, path: Path) -> list[str] | None:
        """The canonical dump's lines, or None when the file is missing or unreadable."""
        if not path.is_file():
            return None
        r = subprocess.run([str(self.exe), "dump", fmt, str(path)], capture_output=True)
        lines = r.stdout.decode("utf-8").splitlines()
        if r.returncode != 0 or not lines or lines[0].startswith("error"):
            return None
        return lines


def f32(h: str) -> float:
    return struct.unpack(">f", bytes.fromhex(h))[0]


def unescape_token(tok: str) -> str:
    return re.sub(r"\\x([0-9a-f]{2})", lambda m: chr(int(m.group(1), 16)), tok)


def gabe_columns(lines: list[str]) -> list[tuple[str, str, list]]:
    """(type, label, values) per column of a `simpa dump gabe`."""
    cols, i = [], 3
    while i < len(lines):
        _, typ, label = lines[i].split(" ", 2)
        i += 1
        if typ == "float":
            i += 1  # digits
        n = int(lines[i].split()[1])
        vals = lines[i + 1:i + 1 + n]
        i += 1 + n
        if typ == "float":
            vals = [f32(v) for v in vals]
        elif typ == "int":
            vals = [int(v) for v in vals]
        else:
            vals = [unescape_token(v) for v in vals]
        cols.append((typ, unescape_token(label), vals))
    return cols


# ---------------------------------------------------------------------------------------------
# config.xml as the solver reads it

class Config:
    def __init__(self, path: Path):
        self.root = ET.fromstring(path.read_bytes())
        self.sim = self.root.find("simulation")

    def get(self, name: str, default: str = "") -> str:
        return self.sim.get(name, default)

    def bands(self) -> list[str]:
        return [b.get("freq") for b in self.sim.find("freq_enum") if b.get("docalc") == "1"]

    def sources(self) -> list[ET.Element]:
        return list(self.root.find("sources"))

    def receivers(self) -> list[str]:
        node = self.root.find("recepteursp")
        return [] if node is None else [r.get("lbl") for r in node]

    def surface_kinds(self) -> tuple[bool, bool]:
        node = self.root.find("recepteurss")
        tags = [] if node is None else [c.tag for c in node]
        return "recepteur_surfacique" in tags, "recepteur_surfacique_coupe" in tags

    def fittings(self) -> list[int]:
        node = self.root.find("encombrement_enum")
        return [] if node is None else [int(e.get("id")) for e in node]


def expected_files(solver: str, cfg: Config) -> list[str]:
    """SC:224-254, for the config actually in the run folder."""
    bands = cfg.bands()
    labels = cfg.receivers()
    surf, cut = cfg.surface_kinds()
    rss, rsf, rsc = (cfg.get(k) for k in
                     ("recepteurss_directory", "recepteurss_filename", "recepteurss_cut_filename"))
    files = []
    if solver == "spps":
        files += [cfg.get("stats_filename"), cfg.get("cumul_filename")]
        rd = cfg.get("receiversp_directory")
        for lbl in labels:
            files += [f"{rd}\\{lbl}\\{n}" for n in (
                cfg.get("receiversp_filename"), cfg.get("receiversp_filename_adv"),
                "Punctual receiver intensity.gabe", "Sound level per source.recps")]
            if cfg.get("output_recp_bysource", "0") != "0":
                files += [f"{rd}\\{lbl}\\{s.get('name')}\\{cfg.get('receiversp_filename')}"
                          for s in cfg.sources()]
        files += [f"Intensity animation\\{f} Hz\\Intensity.rpi" for f in bands]
        byfreq = cfg.get("output_recs_byfreq", "1") != "0"
        for f in bands:
            if surf and byfreq:
                files.append(f"{rss}{f} Hz\\{rsf}")
            if cut and byfreq:
                files.append(f"{rss}{f} Hz\\{rsc}")
        if surf:
            files.append(f"{rss}Global\\{rsf}")
        if cut:
            files.append(f"{rss}Global\\{rsc}")
        if int(cfg.get("nbparticules_rendu", "0")) > 0:
            pd = cfg.get("particules_directory")
            for f in bands:
                files.append(f"{pd}{f}\\{cfg.get('particules_filename')}")
                if cfg.get("save_surface_intersection", "1") != "0":
                    files.append(f"{pd}{f}\\particle_surface_collision_statistics.csv")
                if cfg.get("save_receiver_intersection", "1") != "0":
                    files.append(f"{pd}{f}\\particle_receivers_collision_statistics.csv")
    else:
        files.append("Main results.gabe")
        files += [f"Punctual receivers\\{lbl}.gabe" for lbl in labels]
        if surf or cut:
            for key in ("direct_recepteurSOutputName", "sabine_recepteurSOutputName",
                        "eyring_recepteurSOutputName"):
                pre = cfg.get(key)
                for folder in [f"{f} Hz" for f in bands] + ["Global"]:
                    if surf:
                        files.append(f"{pre}{rss}{folder}\\{rsf}")
                    if cut:
                        files.append(f"{pre}{rss}{folder}\\{rsc}")
    return files


STATS_ROWS = {
    "Particles absorbed by the atmosphere": "absorbed_atmosphere",
    "Particles absorbed by the materials": "absorbed_materials",
    "Particles absorbed by the fittings": "absorbed_fittings",
    "Particles lost by infinite loops": "lost_loops",
    "Particles lost by meshing problems": "lost_meshing",
    "Particles remaining at the end of the calculation": "remaining",
    "Total": "total",
}


def post_run(solver: str, solve: Path, cfg: Config, simpa: Simpa) -> dict:
    """The post-run checks of SC:346-369. Returns codes and what they were computed from."""
    codes, detail = [], {}
    if solver == "spps":
        dump = simpa.dump("gabe", solve / cfg.get("stats_filename"))
        if dump is None:
            codes.append("stats_unreadable")
        else:
            cols = gabe_columns(dump)
            labels = cols[0][2]
            per_band = {}
            for typ, label, vals in cols[1:]:
                per_band[label] = {STATS_ROWS.get(labels[i], labels[i]): v for i, v in enumerate(vals)}
            detail["stats"] = per_band
            if set(per_band) != {f"{f} Hz" for f in cfg.bands()}:
                codes.append("stats_band_mismatch")
            need = int(cfg.get("nbparticules")) * len(cfg.sources())
            if any(b["total"] < need for b in per_band.values()):
                codes.append("particle_total_short")
            if any(b["total"] > 0 and (b["lost_meshing"] + b["lost_loops"]) / b["total"] > LOSS_LIMIT
                   for b in per_band.values()):
                codes.append("particle_loss_excess")
    else:
        bad = []
        for rel in ["Main results.gabe"] + [f"Punctual receivers\\{l}.gabe" for l in cfg.receivers()]:
            dump = simpa.dump("gabe", solve / rel)
            if dump is None:
                continue
            cols = gabe_columns(dump)
            rows = cols[0][2]
            for typ, label, vals in cols[1:]:
                for i, v in enumerate(vals):
                    global_row = rel == "Main results.gabe" and rows[i] == "Global"
                    if typ == "float" and not math.isfinite(v) and not global_row:
                        bad.append(f"{rel} [{label.replace(chr(13), ' ')}] {rows[i]} = {v}")
        if bad:
            codes.append("nonfinite_result")
            detail["nonfinite"] = bad
    missing = [p for p in expected_files(solver, cfg)
               if not (solve / p).is_file() or (solve / p).stat().st_size == 0]
    if missing:
        codes.append("expected_file_missing")
        detail["missing_files"] = missing
    return {"codes": codes, **detail}


# ---------------------------------------------------------------------------------------------
# Pre-launch mesh check: a reference for mesh::verify (m5-m6-design.md, decision 6 and the
# VerifyReport counts), used only to derive these fixtures' expectations.

COUNT_ORDER = ["index_errors", "degenerate_tets", "inverted_tets", "unmarked_boundary_faces",
               "marker_out_of_range", "nonmutual_neighbors", "asymmetric_internal_faces",
               "marker_geometry_mismatches", "uncovered_scene_faces", "unknown_volume_ids"]


def sub(a, b):
    return (a[0] - b[0], a[1] - b[1], a[2] - b[2])


def dot(a, b):
    return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]


def cross(a, b):
    return (a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0])


def point_triangle_distance(p, a, b, c) -> float:
    """Distance from p to triangle abc (Ericson, Real-Time Collision Detection, 5.1.5)."""
    ab, ac, ap = sub(b, a), sub(c, a), sub(p, a)
    d1, d2 = dot(ab, ap), dot(ac, ap)
    if d1 <= 0 and d2 <= 0:
        q = a
    else:
        bp = sub(p, b)
        d3, d4 = dot(ab, bp), dot(ac, bp)
        cp = sub(p, c)
        d5, d6 = dot(ab, cp), dot(ac, cp)
        vc, vb, va = d1 * d4 - d3 * d2, d5 * d2 - d1 * d6, d3 * d6 - d5 * d4
        if d3 >= 0 and d4 <= d3:
            q = b
        elif d6 >= 0 and d5 <= d6:
            q = c
        elif vc <= 0 and d1 >= 0 and d3 <= 0:
            t = d1 / (d1 - d3)
            q = (a[0] + t * ab[0], a[1] + t * ab[1], a[2] + t * ab[2])
        elif vb <= 0 and d2 >= 0 and d6 <= 0:
            t = d2 / (d2 - d6)
            q = (a[0] + t * ac[0], a[1] + t * ac[1], a[2] + t * ac[2])
        elif va <= 0 and (d4 - d3) >= 0 and (d5 - d6) >= 0:
            t = (d4 - d3) / ((d4 - d3) + (d5 - d6))
            q = (b[0] + t * (c[0] - b[0]), b[1] + t * (c[1] - b[1]), b[2] + t * (c[2] - b[2]))
        else:
            den = 1.0 / (va + vb + vc)
            v, w = vb * den, vc * den
            q = (a[0] + ab[0] * v + ac[0] * w, a[1] + ab[1] * v + ac[1] * w, a[2] + ab[2] * v + ac[2] * w)
    d = sub(p, q)
    return math.sqrt(dot(d, d))


def mesh_check(folder: Path, cfg: Config, simpa: Simpa) -> dict:
    cbin_name, mbin_name = cfg.get("modelName"), cfg.get("tetrameshFileName")
    if not (folder / mbin_name).is_file():
        return {"mesh_check": "fail", "error": f"{mbin_name} is missing", "codes": []}
    cb = simpa.dump("cbin", folder / cbin_name)
    if cb is None:
        return {"mesh_check": "fail", "error": f"{cbin_name} cannot be read", "codes": []}
    mb = simpa.dump("mbin", folder / mbin_name)
    if mb is None:
        return {"mesh_check": "fail", "error": f"{mbin_name} cannot be read", "codes": []}
    nv = int(cb[1].split()[1])
    verts = [tuple(f32(h) for h in cb[2 + i].split()) for i in range(nv)]
    nf = int(cb[2 + nv].split()[1])
    faces = [tuple(int(x) for x in cb[3 + nv + i].split()[:3]) for i in range(nf)]
    nn = int(mb[1].split()[1])
    nodes = [tuple(f32(h) for h in mb[2 + i].split()[1:]) for i in range(nn)]
    nt = int(mb[2 + nn].split()[1])
    tets = []
    for i in range(nt):
        t = mb[3 + nn + i].split()
        corners = [int(x) for x in t[1:5]]
        fcs = []
        for f in range(4):
            fa = t[6 + 6 * f:12 + 6 * f]  # after the word "face"
            fcs.append(([int(x) for x in fa[1:4]], int(fa[4]), int(fa[5])))
        tets.append((corners, int(t[5]), fcs))

    c = collections.Counter()
    flat = 0
    scale = max([1.0] + [abs(x) for v in verts for x in v])
    tol = 32 * 2.0 ** -24 * scale
    covered = set()
    volume_by_id = collections.defaultdict(float)
    ids = collections.Counter(t[1] for t in tets)
    room = ids.most_common(1)[0][0] if ids else None
    allowed = {room} | set(cfg.fittings())
    for k, (corners, idv, fcs) in enumerate(tets):
        if any(not 0 <= x < nn for x in corners) or any(
                not 0 <= x < nn for fv, _, _ in fcs for x in fv) or any(n >= nt for _, _, n in fcs):
            c["index_errors"] += 1
            continue
        A, B, C, D = (nodes[x] for x in corners)
        det = dot(sub(A, D), cross(sub(B, D), sub(C, D)))
        edge = max(math.dist(p, q) for p in (A, B, C, D) for q in (A, B, C, D))
        if len(set(corners)) < 4:
            c["degenerate_tets"] += 1
        elif abs(det) <= 2.0 ** -23 * edge ** 3:
            flat += 1
        elif det >= 0:
            c["inverted_tets"] += 1
        volume_by_id[idv] += abs(det) / 6
        if idv not in allowed:
            c["unknown_volume_ids"] += 1
        for fv, marker, n in fcs:
            if n < 0 and marker < 0:
                c["unmarked_boundary_faces"] += 1
            if marker >= nf:
                c["marker_out_of_range"] += 1
            elif marker >= 0:
                covered.add(marker)
                tri = [verts[x] for x in faces[marker]]
                if any(point_triangle_distance(nodes[x], *tri) > tol for x in fv):
                    c["marker_geometry_mismatches"] += 1
            if n >= 0:
                back = [g for g in tets[n][2] if g[2] == k]
                if not back:
                    c["nonmutual_neighbors"] += 1
                elif marker >= 0 and back[0][1] != marker:
                    c["asymmetric_internal_faces"] += 1
    c["uncovered_scene_faces"] = nf - len(covered)
    counts = {k: c[k] for k in COUNT_ORDER if c[k]}
    out = {
        "mesh_check": "fail" if counts else "pass",
        "tetrahedra": nt, "nodes": nn, "scene_faces": nf, "room_id": room,
        "counts": counts, "codes": list(counts),
    }
    if flat:
        # Whether a sliver is "at or below the f32 noise floor" depends on mesh::verify's own
        # threshold, so slivers are reported but not required of it.
        out["slivers"] = {"count": flat,
                          "rule": "corners distinct, |6V| <= 2^-23 * (longest edge)^3"}
    return out


# ---------------------------------------------------------------------------------------------
# Judging one case

def judge(case: Path, obs_root: Path, rows: list[Row], simpa: Simpa) -> dict:
    name = case.name
    stub = name.startswith("stub_")
    solver = "spps" if stub else name.split("_")[0]
    if stub:
        spec = json.loads((case / fc.STUB_JSON).read_text(encoding="utf-8"))
        buf = {"stdout": b"", "stderr": b""}
        for ln in spec["lines"]:
            buf[ln["stream"]] += ln["text"].encode("utf-8") + (b"\n" if ln["newline"] else b"")
        out, err, code, solve = buf["stdout"], buf["stderr"], spec["exit_code"] & 0xFFFFFFFF, case
        source = "stub.json, played back by the stub solver (not run here)"
        rundir = None
    else:
        o = obs_root / name
        run = json.loads((o / "run.json").read_text(encoding="utf-8-sig"))
        if run.get("timed_out"):
            fc.die(f"{name}: the recorded run timed out")
        out, err = (o / "solver.stdout.txt").read_bytes(), (o / "solver.stderr.txt").read_bytes()
        code, solve = int(run["exit_code"], 16), o / "solve"
        exe = Path(run["exe"])
        source = f"{exe.name} sha256 {hashlib.sha256(exe.read_bytes()).hexdigest()[:16]}, run by runsolvers.ps1"
        rundir = str(solve) + "\\"
    lines = classify(split_stream(out, "stdout") + split_stream(err, "stderr"), rows)
    cfg = Config(solve / "config.xml")
    post = post_run(solver, solve, cfg, simpa)
    fail = unique([ln.row for ln in lines if ln.cls == "FAIL"]) + exit_class(code)
    if solver == "spps" and not fail and not any(ln.row == "spps_end_of_calculation" for ln in lines):
        fc.die(f"{name}: SPPS exited 0 with no FAIL line and no End of calculation; no code for that")
    codes = fail if fail else post["codes"]
    status = "CRASH" if is_crash(code) else ("FAIL" if codes else "OK")
    warnings = unique([ln.row for ln in lines if ln.cls == "WARN"])

    def norm(text: str) -> str:
        return text.replace(rundir, fc.PLACEHOLDER) if rundir else text

    shown = [ln for ln in lines if ln.cls not in ("PROGRESS",)]
    observed = {
        "source": source,
        "exit_code": f"0x{code:08X}",
        "status": status,
        "codes": codes,
        "warnings": warnings,
        "lines": [norm(ln.text) for ln in shown if ln.cls in ("FAIL", "WARN", "OK")
                  or ln.row.startswith("continuation:") or ln.row == "ground_height"],
        "transcript": [{"stream": ln.stream, "row": ln.row, "terminated": ln.terminated,
                        "text": norm(ln.text)} for ln in shown],
        "progress_lines": sum(ln.cls == "PROGRESS" for ln in lines),
        "post_run": post,
    }
    pre = mesh_check(case, Config(case / "config.xml"), simpa)
    if pre["mesh_check"] == "fail":
        final_status, final_codes = "FAIL", ["mesh_invalid"] + pre["codes"]
    else:
        final_status, final_codes = status, codes
    return {"solver": solver, "status": final_status, "codes": final_codes,
            "warnings": warnings if pre["mesh_check"] == "pass" else [],
            "pre_launch": pre, "observed": observed, "_lines": lines}


def disagreements(name: str, got: dict) -> list[str]:
    exp = EXPECT.get(name)
    if exp is None:
        return [f"{name}: no expectation written down"]
    bad = []
    if got["status"] != exp["status"] or got["codes"] != exp["codes"]:
        bad.append(f"{name}: expected {exp['status']} {exp['codes']}, got {got['status']} {got['codes']}")
    if got["warnings"] != exp.get("warnings", []):
        bad.append(f"{name}: expected warnings {exp.get('warnings', [])}, got {got['warnings']}")
    o = got["observed"]
    if "observed_status" in exp:
        if (o["status"], o["codes"]) != (exp["observed_status"], exp["observed_codes"]):
            bad.append(f"{name}: expected the solver's own verdict {exp['observed_status']} "
                       f"{exp['observed_codes']}, got {o['status']} {o['codes']}")
    elif got["pre_launch"]["mesh_check"] != "pass":
        bad.append(f"{name}: pre-launch mesh check failed unexpectedly: {got['pre_launch']}")
    return bad


def check_stub_texts(runs: dict, upstream: Path, rows: list[Row]) -> list[str]:
    """Every stub line's text is the solver's own: seen in a real run, or a literal in source."""
    bad = []
    seen = {n: {ln.text for ln in r["_lines"]} for n, r in runs.items() if not n.startswith("stub_")}
    for name, r in runs.items():
        if not name.startswith("stub_"):
            continue
        for ln in r["_lines"]:
            ev = STUB_EVIDENCE.get(ln.text)
            if ev is None:
                bad.append(f"{name}: no evidence recorded for {ln.text!r}")
            elif ev[0] == "observed" and ln.text not in seen.get(ev[1], set()):
                bad.append(f"{name}: {ln.text!r} is not in {ev[1]}'s output")
            elif ev[0] == "source":
                src = decode((upstream / "src" / ev[1]).read_bytes()).splitlines()
                want = ln.text.split("!")[0] + "!" if ln.text.endswith("!") else ln.text
                if want not in src[ev[2] - 1]:
                    bad.append(f"{name}: {want!r} is not at {ev[1]}:{ev[2]}")
            elif ev[0] == "pattern" and ln.row != ev[1]:
                bad.append(f"{name}: {ln.text!r} is not a {ev[1]} line")
    return bad


# ---------------------------------------------------------------------------------------------
# Output

def expected_json(name: str, r: dict) -> str:
    exp = EXPECT[name]
    doc = {
        "solver": r["solver"],
        "status": r["status"],
        "codes": r["codes"],
        "warnings": r["warnings"],
        "receipt": exp["receipt"],
        "pre_launch": r["pre_launch"],
        "observed": r["observed"],
    }
    return json.dumps(doc, indent=2, ensure_ascii=False) + "\n"


def readme(runs: dict, rows: list[Row]) -> str:
    real = [n for n in runs if not n.startswith("stub_")]
    stubs = [n for n in runs if n.startswith("stub_")]
    L = [
        "# Run-folder fixtures",
        "",
        "Inputs for `simpa run-folder` (docs/m5-m6-design.md, \"Run folder\"), each with the verdict",
        "it must give. Generated; do not edit by hand. Regenerate on Windows, from the repo root:",
        "",
        "```",
        "python tools/fixture-gen/mkcfg.py tests/fixtures/upstream/lib_interface tests/fixtures/runs",
        "python tools/fixture-gen/mklossy.py tests/fixtures/runs",
        "python tools/fixture-gen/mktcr.py tests/fixtures/runs [--broken-hall build-clean/sim_output/tcr]",
        "python tools/fixture-gen/mkstubs.py tests/fixtures/runs",
        "powershell -File tools/fixture-gen/runsolvers.ps1 -Runs tests/fixtures/runs -Out <fresh scratch folder>",
        "python tools/fixture-gen/mkexpected.py tests/fixtures/runs <that folder> --simpa <simpa.exe>",
        "```",
        "",
        "`mktcr.py` without `--broken-hall` keeps the committed `tcr_broken_hall`; its source folder",
        "exists on one machine only. `mkexpected.py --check` compares instead of writing.",
        "`python tools/fixture-gen/test_fixture_gen.py` runs the negative tests of these checks.",
        "",
        "## A fixture",
        "",
        "- `config.xml` has `workingdirectory=\"__RUNDIR__\"`. `run-folder` replaces it with the",
        "  absolute run folder plus `\\`; every file it names is in the folder.",
        "- `stub_*` folders add `stub.json` for the stub solver and reuse `spps_ok`'s inputs.",
        "- `expected.json`:",
        "  - `status` and `codes`: the verdict `run-folder` must give. `codes` are the decisive",
        "    reasons and must all appear; a verdict may add consequences (for example",
        "    `expected_file_missing` after a crash).",
        "  - `warnings`: WARN-class rows seen.",
        "  - `pre_launch`: the mesh check `run-folder` makes before launch, computed by",
        "    `mkexpected.py`'s reference of decision 6 and the VerifyReport counts. When it",
        "    fails, the verdict is FAIL with `mesh_invalid` and the failing counts' codes.",
        "  - `observed`: what the real solver did when run anyway, launched as Part B says",
        "    (fresh copy, cwd = the folder, argument `config.xml`): exit code, the solver's own",
        "    status and codes, the decisive lines, the full transcript classified by row (paths",
        "    under the run folder written as `__RUNDIR__`), and the post-run checks. For stubs",
        "    it is the playback of `stub.json`.",
        "",
        "## What the runs showed",
        "",
        "Observed with the M1 solvers of `solvers/manifest.json` (`spps.exe` `cacbbee25d70e6ff`,",
        "`classicalTheory.exe` `6382f32139097604`). Every survey behaviour cited in the receipts",
        "reproduced. Beyond it:",
        "",
        "- **`spps_oneband` is not caught by any Part B signal.** Exit 0, no FAIL line, 2,000",
        "  particles per band, no loss: the 1000 Hz band's particles are all absorbed by the",
        "  atmosphere at the first step. Only the project rule `band_set_mismatch` refuses it.",
        "- **`tcr_srcout` is caught after all**, by `nonfinite_result`: the direct field at R1 is",
        "  -inf in every band, although TCR exits 0.",
        "- **Exit `0xFFFFFFFF` is FAIL**, as the exit tables say (SC:264, SC:275). The rule at",
        "  SC:278, \"any exit at or above `0xC0000000` is CRASH\", would make it `crash_other`.",
        "- **Night Mode's broken-hall config fails on its own**: TCR prints `xml_property_missing`",
        "  for `disable_absatmo_computation` and `absatmo`, and R2's direct field is -inf.",
        "- **Rows the contract had not seen run:** `degenerate_tetrahedron` (exit 1, no",
        "  newline), `tetra_mesh_empty`, `ground_height`, `source_moved_off_vertex` and",
        "  `source_on_surface` all print exactly as the table says. A source inside a floor",
        "  triangle of the cube does trip the on-face check, unlike P2's `src_face`.",
        "",
        "## Cases",
        "",
        "| case | built as | status | codes | solver alone |",
        "|---|---|---|---|---|",
    ]
    for n in real:
        r = runs[n]
        o = r["observed"]
        alone = "same" if r["pre_launch"]["mesh_check"] == "pass" else (
            f"{o['status']} {', '.join(o['codes'])} (exit {o['exit_code']})")
        L.append(f"| `{n}` | {CONSTRUCTION[n]} | {r['status']} | {', '.join(r['codes']) or '-'} | {alone} |")
    L += ["", "| stub | status | codes | exit |", "|---|---|---|---|"]
    for n in stubs:
        r = runs[n]
        L.append(f"| `{n}` | {r['status']} | {', '.join(r['codes'])} | {r['observed']['exit_code']} |")
    L += [
        "",
        "## Classifier coverage",
        "",
        "Which fixture's output hits each row of docs/solver-contract.md Part B. \"Through",
        "run-folder\" lists real cases whose mesh passes the pre-launch check, so the solver",
        "actually starts; \"refused before launch\" lists real cases that print the row only",
        "when run directly.",
        "",
        "| # | row | class | through run-folder | refused before launch | stubs |",
        "|---|---|---|---|---|---|",
    ]
    for i, row in enumerate(rows, 1):
        hit = {n for n, r in runs.items() if any(ln.row == row.id for ln in r["_lines"])}
        ok = sorted(n for n in hit if n in real and runs[n]["pre_launch"]["mesh_check"] == "pass")
        refused = sorted(n for n in hit if n in real and n not in ok)
        st = sorted(n for n in hit if n in stubs)
        cell = lambda xs: ", ".join(f"`{x}`" for x in xs) or "-"
        L.append(f"| {i} | `{row.id}` | {row.cls} | {cell(ok)} | {cell(refused)} | {cell(st)} |")
    return "\n".join(L) + "\n"


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.strip().splitlines()[0])
    ap.add_argument("runs", type=Path)
    ap.add_argument("observed", type=Path)
    ap.add_argument("--simpa", type=Path, required=True)
    ap.add_argument("--contract", type=Path, default=REPO / "docs/solver-contract.md")
    ap.add_argument("--upstream", type=Path, default=Path(r"B:\repos\I-Simpa-upstream"))
    ap.add_argument("--check", action="store_true")
    a = ap.parse_args()
    rows = load_rows(a.contract)
    simpa = Simpa(a.simpa)
    names = sorted(p.name for p in a.runs.iterdir() if p.is_dir() and re.match(r"(spps|tcr|stub)_", p.name))
    order = [n for n in EXPECT if n in names] + [n for n in names if n not in EXPECT]
    runs = {n: judge(a.runs / n, a.observed, rows, simpa) for n in order}
    bad = [m for n, r in runs.items() for m in disagreements(n, r)]
    bad += [f"{n}: fixture missing" for n in EXPECT if n not in runs]
    bad += check_stub_texts(runs, a.upstream, rows)
    for n, r in runs.items():
        print(f"{n:34} {r['status']:5} {', '.join(r['codes']) or '-':60} "
              f"(solver alone: {r['observed']['status']} {', '.join(r['observed']['codes']) or '-'})")
    if bad:
        print("\nDISAGREEMENTS:\n  " + "\n  ".join(bad))
        sys.exit(1)
    outputs = {a.runs / n / fc.EXPECTED_JSON: expected_json(n, r) for n, r in runs.items()}
    outputs[a.runs / "README.md"] = readme(runs, rows)
    if a.check:
        stale = [str(p) for p, text in outputs.items()
                 if not p.is_file() or p.read_bytes() != text.encode("utf-8")]
        if stale:
            print("\nSTALE:\n  " + "\n  ".join(stale))
            sys.exit(1)
        print(f"\n{len(outputs)} files up to date")
        return
    for p, text in outputs.items():
        fc.write_text(p, text)
    print(f"\nwrote {len(outputs)} files; every case agrees with its expectation")


if __name__ == "__main__":
    main()
