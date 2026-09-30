"""Failing-first receipts for M8b tamper (m8b-tamper): for each fix, revert only it (one exact
text replacement in one file), run its tests and see them fail, restore the file byte for byte
(checked), run them again and see them pass. Writes the log given as argv[1]. Deletes nothing:
each file is written back from the bytes read before the revert."""

import difflib
import os
import subprocess
import sys
import time

WT = r"B:\repos\I-Simpa_Night_Mode\.claude\worktrees\m8b-tamper"
CORE = os.path.join(WT, "crates", "simpa-core")
ENV = dict(
    os.environ,
    CARGO_TARGET_DIR="C:/tmp/nm-target",
    CARGO_BUILD_JOBS="16",
    CARGO_INCREMENTAL="0",
    SIMPA_SOLVERS_DIR="C:/tmp/nm-m8a-solvers",
    SIMPA_UPSTREAM="B:/repos/I-Simpa-upstream",
    SIMPA_BED_FROM="C:/tmp/nm-m8a-bed/20260929T093134Z",
    SIMPA_BED_REPORT="C:/tmp/nm-m8a-bed/20260929T093134Z/report.json",
    SIMPA_BED_SEAL=WT + r"\beds\m8a-20260929T093134Z\outputs-seal.json",
    SIMPA_BED_JOBS="4",
)

LIB = ["cargo", "test", "-p", "simpa-core", "--lib", "--"]

CASES = [
    (
        "41-seeds: two seeds of a cell may share no solver output file (into_reads)",
        r"src\bed\run.rs",
        """    for seeds in reads.spps.values_mut() {
        refuse_shared_outputs(seeds);
    }
    refuse_shared_outputs(&mut reads.atmospheric.spps);""",
        """    if false {
        refuse_shared_outputs(&mut reads.atmospheric.spps);
    }""",
        LIB
        + [
            "--exact",
            "bed::tests::a_seed_carrying_another_seeds_outputs_does_not_pass_the_bed",
            "bed::tests::ten_seeds_carrying_one_seeds_outputs_do_not_pass_the_bed",
        ],
    ),
    (
        "41-run.json: a run's files held to run.json's output hashes (bind)",
        r"src\bed\bind.rs",
        """        problems.extend(
            differences("run.json's outputs", &record, &here)
                .into_iter()
                .map(|p| format!("{name}/{p}")),
        );""",
        """        let _ = (&record, &here);""",
        LIB
        + [
            "--exact",
            "bed::bind::tests::a_run_bound_to_run_json_is_refused_when_any_file_is_not_its_records",
        ],
    ),
    (
        "41-seal: a sealed run's files held to the seal (bind)",
        r"src\bed\bind.rs",
        """        problems.extend(differences("the seal", &record, &disk));""",
        """        let _ = &record;""",
        LIB
        + [
            "--exact",
            "bed::bind::tests::a_run_bound_to_a_seal_is_refused_when_any_file_is_not_its_records",
        ],
    ),
    (
        "41-unbound: a run with neither record refused (bind)",
        r"src\bed\bind.rs",
        """    if outputs.is_none() && sealed.is_none() {""",
        """    if false && outputs.is_none() && sealed.is_none() {""",
        LIB
        + [
            "--exact",
            "bed::bind::tests::a_run_with_no_output_hashes_and_no_seal_entry_is_refused",
        ],
    ),
    (
        "41-record: run.json records what the run left (run manager)",
        r"src\run\manager.rs",
        """        let outputs = hash_tree(&self.dir, &[FILE_NAME]).ok();""",
        """        let outputs: Option<Vec<super::manifest::OutputRef>> = None;
        let _ = hash_tree;""",
        [
            "cargo",
            "test",
            "-p",
            "simpa-core",
            "--test",
            "run_manager",
            "--",
            "--exact",
            "run_json_records_every_file_the_run_left",
        ],
    ),
    (
        "42-disk: project.simpa, config.xml, scene and mesh from disk held to the plan (check_planned)",
        r"src\bed\run.rs",
        """    problems.extend(disk_problems(p, info));""",
        """    let _ = disk_problems;""",
        LIB
        + [
            "--exact",
            "bed::run::tests::a_tcr_run_whose_run_json_names_another_plan_is_refused_on_its_files",
            "bed::run::tests::a_run_whose_files_on_disk_are_not_the_plans_is_refused",
            "bed::run::tests::a_read_not_bound_or_not_hashed_from_disk_is_refused",
            "bed::tests::a_tcr_run_of_another_plan_does_not_pass_the_bed",
        ],
    ),
    (
        "42-D: D's reference the planned project's analytic time, not the run's own (evaluate_tcr)",
        r"src\bed\check.rs",
        """            .map(|b| (b.freq_hz, b.eyring_s, plan(b.freq_hz).and_then(|x| x.1)))""",
        """            .map(|b| (b.freq_hz, b.eyring_s, b.analytic_eyring_s))""",
        LIB
        + [
            "--exact",
            "bed::tests::a_tcr_run_of_another_plan_does_not_pass_the_bed",
            "bed::check::tests::d_fails_a_run_whose_bands_are_not_the_beds",
        ],
    ),
    (
        "42-D on the M8a bed: say-NO N8's fault held over the reading only, as before M8b (bed_m8a.rs)",
        r"..\simpa\tests\bed_m8a.rs",
        """    let judged = faults::with(Fault::TcrAnalyticPhysicalConstant, || {
        let reads = read(&l.bed, &from, |k| matches!(k, RunKey::Tcr { .. }));
        judge(&l, &reads, &traced(&rep), &rep)
    });""",
        """    let reads = faults::with(Fault::TcrAnalyticPhysicalConstant, || {
        read(&l.bed, &from, |k| matches!(k, RunKey::Tcr { .. }))
    });
    let judged = judge(&l, &reads, &traced(&rep), &rep);""",
        [
            "cargo",
            "test",
            "-p",
            "simpa",
            "--test",
            "bed_m8a",
            "--",
            "--ignored",
            "--exact",
            "--nocapture",
            "n8_tcrs_analytic_time_with_the_physical_constant_fails_d",
        ],
    ),
]


def run(cmd):
    t0 = time.time()
    p = subprocess.run(cmd, cwd=WT, env=ENV, capture_output=True, text=True, encoding="utf-8", errors="replace")
    return p.returncode, p.stdout + p.stderr, time.time() - t0


def keep(text):
    """The lines that say what happened: results, panics, assertion messages."""
    out = []
    for line in text.splitlines():
        if (
            line.startswith("test ")
            or "panicked at" in line
            or line.startswith("test result")
            or "assertion" in line
            or line.startswith("  left")
            or line.startswith(" right")
            or line.startswith("failures:")
            or line.startswith("    bed::")
            or line.startswith("    run_json")
            or line.startswith("error")
            or "did not fail" in line
            or ": D " in line
        ):
            out.append(line)
    return "\n".join(out[:60])


def main():
    log = open(sys.argv[1], "w", encoding="utf-8", newline="\n")
    only = sys.argv[2:]
    for name, rel, old, new, cmd in CASES:
        if only and not any(name.startswith(o) for o in only):
            continue
        path = os.path.join(CORE, rel)
        original = open(path, "rb").read()
        crlf = b"\r\n" in original
        text = original.decode("utf-8").replace("\r\n", "\n")
        assert text.count(old) == 1, (name, "the fix's text is not there once")
        reverted = text.replace(old, new)
        out = reverted.replace("\n", "\r\n") if crlf else reverted
        log.write(f"==== {name}\n")
        shown = os.path.relpath(os.path.normpath(path), WT).replace(os.sep, "/")
        log.write(f"file: {shown}\n")
        log.write("revert:\n")
        log.writelines(
            difflib.unified_diff(
                text.splitlines(True), reverted.splitlines(True), "fixed", "reverted", n=1
            )
        )
        log.write(f"command: {' '.join(cmd)}\n")
        try:
            open(path, "wb").write(out.encode("utf-8"))
            code, text_out, secs = run(cmd)
        finally:
            open(path, "wb").write(original)
        restored = open(path, "rb").read() == original
        log.write(f"--- with the revert: exit {code} ({secs:.0} s)\n{keep(text_out)}\n")
        log.write(f"--- restored byte for byte: {restored}\n")
        code2, text_out2, secs2 = run(cmd)
        log.write(f"--- after the restore: exit {code2} ({secs2:.0} s)\n{keep(text_out2)}\n\n")
        log.flush()
        verdict = "FAILED then PASSED" if code != 0 and code2 == 0 else "UNEXPECTED"
        print(f"{name}: with revert exit {code}, restored {restored}, after exit {code2}: {verdict}", flush=True)
    log.close()


if __name__ == "__main__":
    main()
