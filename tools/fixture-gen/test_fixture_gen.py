r"""Tests of the fixture generators' checks: each check is shown saying no to a bad input.

    python tools/fixture-gen/test_fixture_gen.py

Needs the `simpa` CLI: $SIMPA_CLI, else $CARGO_TARGET_DIR/debug/simpa.exe, else
<repo>/target/debug/simpa.exe. The end-to-end tests also need the solvers: $SIMPA_SOLVERS_DIR,
else <repo>/target/solvers/bin. The stub-text tests need upstream's source: $SIMPA_UPSTREAM,
else B:\repos\I-Simpa-upstream. A missing one fails the test that needs it, loudly; nothing is
skipped.
"""
import copy
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import extract_tutorial2_mesh as t2  # noqa: E402
import fixture_common as fc  # noqa: E402
import mkexpected as mx  # noqa: E402
import mkrooms  # noqa: E402

REPO = HERE.parents[1]
RUNS = REPO / "tests/fixtures/runs"
ROOMS = REPO / "tests/fixtures/rooms"
CONTRACT = REPO / "docs/solver-contract.md"


def upstream_dir() -> Path:
    d = Path(os.environ.get("SIMPA_UPSTREAM") or r"B:\repos\I-Simpa-upstream")
    if not (d / "src").is_dir():
        raise AssertionError(f"upstream checkout not found at {d}; set SIMPA_UPSTREAM")
    return d


def simpa_exe() -> Path:
    candidates = [os.environ.get("SIMPA_CLI")]
    if os.environ.get("CARGO_TARGET_DIR"):
        candidates.append(str(Path(os.environ["CARGO_TARGET_DIR"]) / "debug/simpa.exe"))
    candidates.append(str(REPO / "target/debug/simpa.exe"))
    for c in candidates:
        if c and Path(c).is_file():
            return Path(c)
    raise AssertionError(f"simpa CLI not found (tried {candidates}); build it with "
                         "`cargo build -p simpa` or set SIMPA_CLI")


def solvers_dir() -> Path:
    d = Path(os.environ.get("SIMPA_SOLVERS_DIR") or REPO / "target/solvers/bin")
    if not (d / "spps.exe").is_file():
        raise AssertionError(f"solvers not found in {d}; set SIMPA_SOLVERS_DIR")
    return d


def lines(stdout: bytes = b"", stderr: bytes = b"") -> list:
    rows = mx.load_rows(CONTRACT)
    return mx.classify(mx.split_stream(stdout, "stdout") + mx.split_stream(stderr, "stderr"), rows)


class ContractTable(unittest.TestCase):
    def test_the_page_has_22_rows(self):
        rows = mx.load_rows(CONTRACT)
        self.assertEqual(len(rows), 22)
        self.assertEqual(rows[-1].id, "unclassified_line")

    def test_a_page_missing_a_row_is_refused(self):
        text = CONTRACT.read_text(encoding="utf-8")
        cut = "\n".join(l for l in text.splitlines() if not l.startswith("| `ground_height`"))
        with tempfile.TemporaryDirectory() as tmp:
            p = Path(tmp) / "contract.md"
            p.write_text(cut, encoding="utf-8")
            with self.assertRaises(SystemExit):
                mx.load_rows(p)


class Classifier(unittest.TestCase):
    def test_a_final_line_without_newline_is_captured(self):
        ls = lines(stderr=b"Warning 4 particles has been in error on 4 particles. The computation")
        self.assertEqual([(l.row, l.terminated) for l in ls], [("particle_loss_reported", False)])

    def test_an_unknown_line_is_unclassified(self):
        ls = lines(stdout=b"SPPS version 2.2.1\r\nSomething new\r\n")
        self.assertEqual([l.row for l in ls], ["spps_banner", "unclassified_line"])

    def test_the_path_after_the_scene_mesh_message_is_its_continuation(self):
        ls = lines(stdout=b"Unable to read the scene mesh file :\r\nC:\\r\\mesh.cbin\r\n")
        self.assertEqual([l.row for l in ls],
                         ["scene_mesh_unreadable", "continuation:scene_mesh_unreadable"])
        alone = lines(stdout=b"C:\\r\\mesh.cbin\r\n")
        self.assertEqual([l.row for l in alone], ["unclassified_line"])

    def test_the_path_before_the_empty_mesh_message_is_its_continuation(self):
        ls = lines(stdout=b"C:\\r\\t.mbin\r\nTetrahedron file is empty, the calculation can't be done !\r\n")
        self.assertEqual([l.row for l in ls], ["continuation:tetra_mesh_empty", "tetra_mesh_empty"])

    def test_progress_must_be_the_whole_line(self):
        self.assertEqual([l.row for l in lines(stdout=b"#12.5\n#12.5 x\n")],
                         ["progress", "unclassified_line"])

    def test_exit_classes(self):
        self.assertEqual(mx.exit_class(0), [])
        self.assertEqual(mx.exit_class(1), ["exit_nonzero"])
        self.assertEqual(mx.exit_class(0xFFFFFFFF), ["exit_nonzero"])
        self.assertEqual(mx.exit_class(0xC0000005), ["crash_access_violation"])
        self.assertEqual(mx.exit_class(0xC0000409), ["crash_abort"])
        self.assertEqual(mx.exit_class(0xC0000017), ["crash_other"])


class MeshCheck(unittest.TestCase):
    def check(self, case: str) -> dict:
        return mx.mesh_check(RUNS / case, mx.Config(RUNS / case / "config.xml"), mx.Simpa(simpa_exe()))

    def test_the_cube_passes(self):
        r = self.check("spps_ok")
        self.assertEqual((r["mesh_check"], r["room_id"], r["counts"]), ("pass", 1, {}))

    def test_cut_links_are_unmarked_boundary_faces(self):
        self.assertEqual(self.check("spps_lossy")["counts"], {"unmarked_boundary_faces": 12})

    def test_a_repeated_corner_is_degenerate(self):
        self.assertEqual(self.check("spps_degenerate")["counts"], {"degenerate_tets": 1})

    def test_an_empty_mesh_covers_no_scene_face(self):
        self.assertEqual(self.check("spps_emptymesh")["counts"], {"uncovered_scene_faces": 12})

    def test_missing_and_unreadable_inputs_fail(self):
        self.assertIn("missing", self.check("spps_nomesh")["error"])
        self.assertIn("cannot be read", self.check("spps_unreadable_mesh")["error"])

    def test_a_marker_on_the_wrong_face_is_a_geometry_mismatch(self):
        with tempfile.TemporaryDirectory() as tmp:
            d = Path(tmp)
            for f in ("config.xml", "mesh.cbin", "tetramesh.mbin"):
                shutil.copy(RUNS / "spps_ok" / f, d / f)
            b = bytearray((d / "tetramesh.mbin").read_bytes())
            _, _, base = fc.mbin_counts(bytes(b))
            # Tetrahedron 0, face 2 carries marker 2 (docs/formats/mbin.md example); make it 11.
            off = base + 20 + 20 * 2 + 12
            self.assertEqual(int.from_bytes(b[off:off + 4], "little", signed=True), 2)
            b[off:off + 4] = (11).to_bytes(4, "little", signed=True)
            (d / "tetramesh.mbin").write_bytes(bytes(b))
            r = mx.mesh_check(d, mx.Config(d / "config.xml"), mx.Simpa(simpa_exe()))
            self.assertEqual(r["counts"]["marker_geometry_mismatches"], 1)
            self.assertEqual(r["counts"]["uncovered_scene_faces"], 1)

    def test_the_broken_halls_slivers_depend_on_the_floor(self):
        r = self.check("tcr_broken_hall")
        self.assertEqual(r["counts"], {"degenerate_tets": 65, "unmarked_boundary_faces": 267,
                                       "uncovered_scene_faces": 338})
        self.assertEqual(r["codes_any_of"], [["degenerate_tets", "inverted_tets"]])
        sweep = {s["floor"]: (s["degenerate_tets"], s["inverted_tets"]) for s in r["floor_sweep"]}
        self.assertEqual(sweep["0"], (0, 22))
        self.assertEqual(sweep["2^-23"], (65, 0))

    def test_a_floor_that_alone_decides_the_mesh_is_refused(self):
        sweep = {None: {"degenerate_tets": 0, "inverted_tets": 0},
                 -23: {"degenerate_tets": 3, "inverted_tets": 0}}
        with self.assertRaises(SystemExit):
            mx.floor_dependence("x", sweep, {"degenerate_tets": 3})

    def test_a_count_some_floors_lack_is_not_required(self):
        sweep = {None: {"degenerate_tets": 0, "inverted_tets": 0},
                 -23: {"degenerate_tets": 3, "inverted_tets": 0}}
        moving, any_of = mx.floor_dependence("x", sweep, {"degenerate_tets": 3, "index_errors": 1})
        self.assertEqual((moving, any_of), (["degenerate_tets"], []))
        sweep[None]["inverted_tets"] = 2
        moving, any_of = mx.floor_dependence("x", sweep, {"degenerate_tets": 3})
        self.assertEqual(any_of, [["degenerate_tets", "inverted_tets"]])


class BandCheck(unittest.TestCase):
    def test_a_short_spectrum_is_refused_and_a_full_one_passes(self):
        self.assertEqual(mx.band_check(mx.Config(RUNS / "spps_ok" / "config.xml")), {"bands": "pass"})
        r = mx.band_check(mx.Config(RUNS / "spps_oneband" / "config.xml"))
        self.assertEqual(r["bands"], "fail")
        self.assertEqual(len(r["short_sources"]), 1)

    def test_an_entry_missing_for_a_band_not_computed_passes(self):
        with tempfile.TemporaryDirectory() as t:
            text = (RUNS / "spps_oneband" / "config.xml").read_text(encoding="utf-8")
            # 1000 Hz, the band past the one entry, is no longer computed.
            text = text.replace('<bfreq freq="1000" docalc="1"/>', '<bfreq freq="1000" docalc="0"/>')
            path = Path(t) / "config.xml"
            path.write_text(text, encoding="utf-8")
            self.assertEqual(mx.band_check(mx.Config(path)), {"bands": "pass"})


class Expectations(unittest.TestCase):
    def test_a_verdict_that_differs_from_the_expectation_is_reported(self):
        rows = mx.load_rows(CONTRACT)
        got = {"status": "FAIL", "codes": ["exit_nonzero"], "codes_any_of": [], "warnings": [],
               "pre_launch": {"mesh_check": "pass"}, "observed": {}, "_lines": []}
        self.assertTrue(mx.disagreements("spps_ok", got, rows))
        got_ok = dict(got, status="OK", codes=[])
        self.assertEqual(mx.disagreements("spps_ok", got_ok, rows), [])

    def test_a_missing_any_of_group_is_reported(self):
        rows = mx.load_rows(CONTRACT)
        pre = mx.mesh_check(RUNS / "tcr_broken_hall", mx.Config(RUNS / "tcr_broken_hall/config.xml"),
                            mx.Simpa(simpa_exe()))
        got = {"status": "FAIL",
               "codes": ["mesh_invalid", "unmarked_boundary_faces", "uncovered_scene_faces"],
               "codes_any_of": pre["codes_any_of"], "warnings": [], "pre_launch": pre,
               "observed": {"status": "FAIL", "codes": ["xml_property_missing"]}, "_lines": []}
        self.assertEqual(mx.disagreements("tcr_broken_hall", got, rows), [])
        bad = mx.disagreements("tcr_broken_hall", dict(got, codes_any_of=[]), rows)
        self.assertEqual(len(bad), 1)
        self.assertIn("codes_any_of", bad[0])

    def judge_stub(self, name: str, edit) -> list:
        """disagreements() for a copy of the committed stub `name` with edit(lines) applied."""
        rows = mx.load_rows(CONTRACT)
        with tempfile.TemporaryDirectory() as tmp:
            d = Path(tmp) / name
            shutil.copytree(RUNS / name, d)
            spec = json.loads((d / "stub.json").read_text(encoding="utf-8"))
            edit(spec["lines"])
            (d / "stub.json").write_text(json.dumps(spec, ensure_ascii=False), encoding="utf-8")
            got = mx.judge(d, Path(tmp), rows, mx.Simpa(simpa_exe()))
        return mx.disagreements(name, got, rows)

    def test_a_stub_whose_final_stderr_line_gains_a_newline_is_refused(self):
        name = "stub_particle_loss_unterminated"
        self.assertEqual(self.judge_stub(name, lambda lines: None), [])

        def terminate(lines):
            self.assertEqual((lines[-1]["stream"], lines[-1]["newline"]), ("stderr", False))
            lines[-1]["newline"] = True

        bad = self.judge_stub(name, terminate)
        self.assertTrue(any("particle_loss_reported ends in a newline" in m for m in bad), bad)
        self.assertTrue(any("final stderr line must be" in m for m in bad), bad)

    def test_a_stub_line_moved_to_the_wrong_stream_is_refused(self):
        name = "stub_degenerate_tetrahedron"
        self.assertEqual(self.judge_stub(name, lambda lines: None), [])

        def to_stdout(lines):
            self.assertEqual(lines[1]["stream"], "stderr")
            lines[1].update(stream="stdout", newline=True)

        bad = self.judge_stub(name, to_stdout)
        self.assertTrue(any("degenerate_tetrahedron on stdout, the contract says stderr" in m
                            for m in bad), bad)
        self.assertTrue(any("no stderr at all" in m for m in bad), bad)

    def test_a_stub_text_with_no_evidence_is_reported(self):
        stub = {"_lines": lines(stdout=b"Invented text\n")}
        bad = mx.check_stub_texts({"stub_x": stub}, upstream_dir(), mx.load_rows(CONTRACT))
        self.assertEqual(len(bad), 1)
        self.assertIn("Invented text", bad[0])

    def test_a_stub_text_changed_from_the_source_literal_is_reported(self):
        stub = {"_lines": lines(stdout=b"The path of the XML configuration file must be given!\n")}
        mx.STUB_EVIDENCE["The path of the XML configuration file must be given!"] = (
            "source", "spps/sppsNantes.cpp", 287)
        try:
            bad = mx.check_stub_texts({"stub_x": stub}, upstream_dir(), mx.load_rows(CONTRACT))
        finally:
            del mx.STUB_EVIDENCE["The path of the XML configuration file must be given!"]
        self.assertEqual(len(bad), 1)

    def test_every_committed_expected_json_names_known_codes(self):
        known = {r.id for r in mx.load_rows(CONTRACT)} | {
            "exit_nonzero", "crash_access_violation", "crash_abort", "crash_other",
            "stats_band_mismatch", "particle_total_short", "particle_loss_excess",
            "expected_file_missing", "nonfinite_result", "stats_unreadable", "cancelled",
            "mesh_invalid", "band_set_mismatch"} | set(mx.COUNT_ORDER)
        cases = sorted(p for p in RUNS.iterdir() if p.is_dir())
        self.assertEqual(len(cases), len(mx.EXPECT))
        for case in cases:
            doc = json.loads((case / "expected.json").read_text(encoding="utf-8"))
            any_of = {x for group in doc["codes_any_of"] for x in group}
            self.assertLessEqual(set(doc["codes"]) | set(doc["warnings"]) | any_of, known, case.name)
            self.assertIn(doc["status"], {"OK", "FAIL", "CRASH"})
            self.assertEqual(doc["status"] == "OK", doc["codes"] == [], case.name)
            fc.check_self_contained(case)


class FixtureCommon(unittest.TestCase):
    def test_an_absolute_working_directory_is_refused(self):
        with tempfile.TemporaryDirectory() as tmp:
            d = Path(tmp)
            xml = (RUNS / "spps_ok/config.xml").read_text(encoding="utf-8")
            (d / "config.xml").write_text(xml.replace("__RUNDIR__", "C:\\run\\"), encoding="utf-8")
            with self.assertRaises(SystemExit):
                fc.check_self_contained(d)
            (d / "config.xml").write_text(xml.replace('"mesh.cbin"', '"..\\mesh.cbin"'), encoding="utf-8")
            with self.assertRaises(SystemExit):
                fc.check_self_contained(d)

    def test_a_truncated_cbin_is_refused_by_the_patcher(self):
        with self.assertRaises(SystemExit):
            fc.cbin_set_material((RUNS / "spps_unreadable_mesh/mesh.cbin").read_bytes(), 7)

    def test_the_cube_has_12_links_to_cut(self):
        mbin, cut = fc.mbin_cut_links((RUNS / "spps_ok/tetramesh.mbin").read_bytes())
        self.assertEqual(cut, 12)
        self.assertEqual(mbin, (RUNS / "spps_lossy/tetramesh.mbin").read_bytes())


class Rooms(unittest.TestCase):
    def test_an_unstated_change_is_refused(self):
        text = (ROOMS / "tutorial1_box_seeded.simpa").read_text(encoding="utf-8")
        want = json.loads(text)
        want["name"] = "renamed"
        with self.assertRaises(SystemExit):
            mkrooms.expect_equal("x", text, want)

    def test_an_invalid_fitting_zone_fails_validation(self):
        text = (ROOMS / "tutorial1_box_fitting.simpa").read_text(encoding="utf-8")
        bad = text.replace('"mean_free_path_m": [1.0,', '"mean_free_path_m": [0.0,', 1)
        self.assertNotEqual(bad, text)
        with tempfile.TemporaryDirectory() as tmp:
            p = Path(tmp) / "bad.simpa"
            p.write_text(bad, encoding="utf-8", newline="\n")
            with self.assertRaises(SystemExit):
                mkrooms.check_with_cli(simpa_exe(), p)

    def test_a_non_canonical_layout_is_refused(self):
        doc = json.loads((ROOMS / "tutorial1_box_seeded.simpa").read_text(encoding="utf-8"))
        with tempfile.TemporaryDirectory() as tmp:
            p = Path(tmp) / "pretty.simpa"
            p.write_text(json.dumps(doc, indent=2) + "\n", encoding="utf-8", newline="\n")
            with self.assertRaises(SystemExit):
                mkrooms.check_with_cli(simpa_exe(), p)

    def test_the_committed_rooms_are_the_derivation(self):
        for name, text in mkrooms.derive(ROOMS).items():
            self.assertEqual((ROOMS / name).read_bytes(), text.encode("utf-8"), name)


class Tutorial2(unittest.TestCase):
    NODES = {1: (0.0, 0.0, 0.0), 2: (1.0, 0.0, 0.0), 3: (0.0, 1.0, 0.0), 4: (0.0, 0.0, 1.0)}
    FACETS = [(0, (1, 2, 3)), (1, (1, 2, 4))]
    ROOM = {"geometry": {"vertices": [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
                         "faces": [[0, 1, 2, "g"], [0, 1, 3, "g"]]}}

    def test_the_same_faces_match(self):
        c = t2.compare(self.NODES, self.FACETS, self.ROOM)
        self.assertEqual((c["same"], c["marker"], c["winding"]), (2, 2, 2))

    def test_a_different_face_is_counted(self):
        room = copy.deepcopy(self.ROOM)
        room["geometry"]["faces"][1] = [0, 2, 3, "g"]
        c = t2.compare(self.NODES, self.FACETS, room)
        self.assertEqual((c["same"], c["first_bad"]), (1, 1))

    def test_a_reversed_face_matches_but_is_wound_the_other_way(self):
        room = copy.deepcopy(self.ROOM)
        room["geometry"]["faces"][0] = [0, 2, 1, "g"]
        c = t2.compare(self.NODES, self.FACETS, room)
        self.assertEqual((c["same"], c["winding"]), (2, 1))

    def test_a_vertex_moved_below_f32_resolution_still_matches(self):
        room = copy.deepcopy(self.ROOM)
        room["geometry"]["vertices"][1] = [1.0 + 1e-12, 0.0, 0.0]
        self.assertEqual(t2.compare(self.NODES, self.FACETS, room)["same"], 2)
        room["geometry"]["vertices"][1] = [1.0 + 1e-6, 0.0, 0.0]
        self.assertEqual(t2.compare(self.NODES, self.FACETS, room)["same"], 0)


class EndToEnd(unittest.TestCase):
    """Runs the real solvers on every committed fixture, then checks the committed files."""

    def test_the_runner_refuses_missing_solvers(self):
        with tempfile.TemporaryDirectory() as tmp:
            r = subprocess.run(["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", str(HERE / "runsolvers.ps1"),
                                "-Runs", str(RUNS), "-Out", tmp, "-Solvers", str(Path(tmp) / "none")],
                               capture_output=True, text=True)
            self.assertNotEqual(r.returncode, 0)
            self.assertIn("solver not found", r.stdout + r.stderr)

    def run_cases(self, out: Path, case: str) -> subprocess.CompletedProcess:
        env = dict(os.environ, SIMPA_SOLVERS_DIR=str(solvers_dir()))
        return subprocess.run(["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", str(HERE / "runsolvers.ps1"),
                               "-Runs", str(RUNS), "-Out", str(out), "-Case", case],
                              capture_output=True, text=True, env=env)

    def test_the_runner_refuses_an_unknown_case(self):
        with tempfile.TemporaryDirectory() as tmp:
            for case in ("spps_okk", "spps_ok,nope", "stub_config_path_missing"):
                out = Path(tmp) / case.replace(",", "_")
                r = self.run_cases(out, case)
                self.assertNotEqual(r.returncode, 0, case)
                self.assertIn("unknown case", r.stdout + r.stderr, case)
                self.assertEqual(list(out.iterdir()), [], case)

    def test_the_runner_takes_a_comma_list_under_file(self):
        with tempfile.TemporaryDirectory() as tmp:
            r = self.run_cases(Path(tmp), "spps_ok,tcr_ok")
            self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
            self.assertEqual(sorted(p.name for p in Path(tmp).iterdir()), ["spps_ok", "tcr_ok"])
            self.assertIn("ran 2 case(s)", r.stdout)

    def test_mkexpected_reads_simpa_upstream(self):
        with tempfile.TemporaryDirectory() as tmp:
            env = dict(os.environ, SIMPA_UPSTREAM=str(Path(tmp) / "none"))
            r = subprocess.run([sys.executable, str(HERE / "mkexpected.py"), str(RUNS), tmp,
                                "--simpa", str(simpa_exe()), "--check"],
                               capture_output=True, text=True, env=env)
            self.assertNotEqual(r.returncode, 0)
            self.assertIn("upstream checkout not found", r.stderr)

    def test_committed_expectations_reproduce_and_a_tampered_one_is_caught(self):
        with tempfile.TemporaryDirectory() as tmp:
            obs = Path(tmp) / "observed"
            env = dict(os.environ, SIMPA_SOLVERS_DIR=str(solvers_dir()))
            r = subprocess.run(["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", str(HERE / "runsolvers.ps1"),
                                "-Runs", str(RUNS), "-Out", str(obs)], capture_output=True, text=True, env=env)
            self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
            check = [sys.executable, str(HERE / "mkexpected.py"), None, str(obs), "--simpa", str(simpa_exe()),
                     "--upstream", str(upstream_dir()), "--check"]
            r = subprocess.run(check[:2] + [str(RUNS)] + check[3:], capture_output=True, text=True)
            self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
            runs = Path(tmp) / "runs"
            shutil.copytree(RUNS, runs)
            p = runs / "spps_ok/expected.json"
            p.write_bytes(p.read_bytes().replace(b'"status": "OK"', b'"status": "FAIL"', 1))
            r = subprocess.run(check[:2] + [str(runs)] + check[3:], capture_output=True, text=True)
            self.assertEqual(r.returncode, 1)
            self.assertIn("STALE", r.stdout)


if __name__ == "__main__":
    unittest.main(verbosity=2)
