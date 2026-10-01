"""SPPS runs (HARNESS-PLAN.md P8-P17, sections 3 and 7): the matrix and its plan-only mode, the
guards, the solver check, the report reader and the probe mode.

STUB (HARNESS-PLAN.md section 6, step 3): every function returns None. Tests T12-T14 hold the
contract below. Nothing here may launch a run of the test before ADDENDUM-A1.md is committed.

Contract:
- Refused(RuntimeError) carries `code`, one of: 'reserved_seed', 'heldout_before_a1',
  'outside_data_root', 'solver_unverified', 'solver_build_unverified'.
- plan() -> [run], opening no file: 144 tested runs (P13: 7 rooms x 3 steps x 3 seeds at 150k
  plus F6 x 3 x 3 at 50k, in each of P11's two modes) and 28 truth runs (7 rooms x K = 4, Random,
  1,000,000 particles, 0.1 ms, P17's length). A run is a dict with 'run_id', 'room', 'kind'
  ('tested' or 'truth'), 'mode' ('random' or 'energetic'), 'time_step_s', 'particles_per_source',
  'random_seed' (P12) and 'duration_s' (P10 for tested runs: the room project's own; P17 for truth).
- a1_committed(repo=None) -> bool: ADDENDUM-A1.md is committed on the branch's HEAD.
- heldout_seed(seed) -> bool: P12's tested and truth seeds, Synth-fresh's 2026100101 and
  ISM-fresh's 2026100102.
- require_not_heldout(seed, a1=None): Refused('heldout_before_a1') for a held-out seed while A1 is
  not committed (a1 None: ask a1_committed()). Synth-fresh and ISM-fresh call it too.
- check_solvers(solvers_dir, manifest=None) -> {'verified': bool, 'checks': [{'name', 'path',
  'code_sha256', 'expected', 'matches'}]}: spps.exe, classicalTheory.exe, tetgen.exe and
  preprocess.exe by code sha256 (tools/fixture-gen/pe_fingerprint.py) against
  solvers/manifest.json, as M8a's E1 does (tools/gates/m8a.ps1:8-9).
- launch(run, *, project, run_dir, solvers_dir, data_root=DATA_ROOT, launch_log=LAUNCH_LOG,
  simpa_exe=None, a1=None): the guards first, in this order, each raising Refused before anything
  is written or launched: the seed ('reserved_seed' for RESERVED_SEEDS, then require_not_heldout),
  the folder ('outside_data_root' unless run_dir lies inside data_root), the solvers
  ('solver_unverified' unless check_solvers verifies them). Then <run_dir>/project.simpa is
  written, one line appended to launch_log, `simpa run <project> --solver spps --runs <run_dir>
  --json` run with its stderr kept and classed, and <run_dir>/report.json written from
  `simpa results <run folder> --json`.
- read_run(run_dir, *, data_root=DATA_ROOT) -> series_from_report(<run_dir>/report.json), after
  refusing a folder outside data_root ('outside_data_root'), a run whose <run_dir>/project.simpa
  has a reserved random_seed ('reserved_seed'), and a report whose solver_build status is not
  'verified' ('solver_build_unverified'; P14, backlog 38, results/report.rs:899-901).
- series_from_report(report) -> [{'label', 'band_hz', 'energy_pa2' (float64 array), 'arrival_s',
  'half_width' (receiver_radius_m / speed_of_sound_m_s), 'dt' (time_step_s)}], one per point
  receiver and band: the old real loader's field access (the scratchpad's edtsimp/loaders.py:159-181,
  sha256 ea4d4c09...).
- probe(...): section 7's probe mode. It records PROBE.md's numbers and opens no output file.

P14 and the CLI (HARNESS-PLAN.md 8.1): `simpa run` records no solver check in run.json
(crates/simpa/src/mesh_run.rs:520 sets `verify: None`), and results::solver_build reads such a run
'solver_build_unrecorded' (crates/simpa-core/src/results.rs:467-514). The guard stays as written:
the truth and tested runs wait for backlog 54's CLI half. The probes run no `simpa results`, so only
check_solvers before the run applies to them.
"""
from pathlib import Path

SCRATCH = Path(r'C:\tmp\m8b-edt')
DATA_ROOT = SCRATCH / 'heldout'
PROBE_ROOT = SCRATCH / 'probe'
LAUNCH_LOG = SCRATCH / 'launch.log'
RESERVED_SEEDS = (9998, 9999)


class Refused(RuntimeError):
    """A guard refused: nothing was written or launched."""

    def __init__(self, code, detail=''):
        super().__init__('%s: %s' % (code, detail))
        self.code = code


def plan():
    return None


def a1_committed(repo=None):
    return None


def heldout_seed(seed):
    return None


def require_not_heldout(seed, a1=None):
    return None


def check_solvers(solvers_dir, manifest=None):
    return None


def launch(run, *, project, run_dir, solvers_dir, data_root=DATA_ROOT, launch_log=LAUNCH_LOG,
           simpa_exe=None, a1=None):
    return None


def read_run(run_dir, *, data_root=DATA_ROOT):
    return None


def series_from_report(report):
    return None


def probe(*args, **kwargs):
    return None
