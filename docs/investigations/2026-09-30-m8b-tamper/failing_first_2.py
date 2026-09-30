"""Failing-first receipts for M8b tamper, round 2 (m8b-tamper): for each fix, revert only it (exact
text replacements, each found exactly once), run its tests and see them fail, restore every file
byte for byte (checked), run them again and see them pass. Appends to the log given as argv[1];
argv[2:] picks cases by the start of their name. Deletes nothing: each file is written back from
the bytes read before the revert."""

import difflib
import os
import subprocess
import sys
import time

WT = r"B:\repos\I-Simpa_Night_Mode\.claude\worktrees\m8b-tamper"
ENV = dict(
    os.environ,
    CARGO_TARGET_DIR="C:/tmp/nm-target",
    CARGO_BUILD_JOBS="16",
    CARGO_INCREMENTAL="0",
    SIMPA_SOLVERS_DIR="C:/tmp/nm-m8a-solvers",
    SIMPA_UPSTREAM="B:/repos/I-Simpa-upstream",
)

CORE_LIB = ["cargo", "test", "-p", "simpa-core", "--lib", "--"]
BINDING = ["cargo", "test", "-p", "simpa-core", "--test", "bed_binding", "--"]
CLI_BIN = ["cargo", "test", "-p", "simpa", "--bin", "simpa", "--"]
RS = "crates/simpa-core/src/bed/run.rs"
BIND = "crates/simpa-core/src/bed/bind.rs"
CMD = "crates/simpa/src/bed_cmd.rs"

# name, [(file under WT, the fix's text, the text with the fix reverted)], command
CASES = [
    (
        "f2-commitA: a run its seal does not name is refused (read_folder, at commit A)",
        [
            (
                RS,
                "        Some(s) => Some(s.run(&p.key).ok_or_else(|| unsealed(folder, s, &p.key))?),",
                "        Some(s) => s.run(&p.key),",
            )
        ],
        BINDING + ["--exact", "a_run_its_seal_does_not_name_is_refused"],
    ),
    (
        "f1-bind: run.json alone binds nothing (bind)",
        [
            (
                BIND,
                "    if made.is_none() && sealed.is_none() {",
                "    if made.is_none() && sealed.is_none() && outputs.is_none() {",
            )
        ],
        CORE_LIB + ["--exact", "bed::bind::tests::a_run_bound_only_by_its_own_run_json_is_refused"],
    ),
    (
        "f1-fresh: a run this process made is bound to what it made, from memory (read_fresh)",
        [
            (
                RS,
                "    let made = made_record(p, report)?;",
                """    let made = made_record(
        p,
        &manager::RunReport {
            dir: report.dir.clone(),
            manifest: read::manifest_of(&report.dir)?,
        },
    )?;""",
            )
        ],
        BINDING + ["--exact", "a_run_this_process_made_is_bound_to_what_it_made_not_to_its_run_json"],
    ),
    (
        "f1-cli: --from without --seal is refused (parse)",
        [
            (
                CMD,
                """        (Some(_), None) => {
            return Err(
                "--from needs --seal <file>: an earlier bed's runs are read only against \\
                        its committed seal (beds/<bed>-<stamp>/outputs-seal.json), never on the \\
                        word of the run.json in the folder being judged"
                    .into(),
            );
        }""",
                """        (Some(f), None) => Some((f, PathBuf::new())),""",
            )
        ],
        CLI_BIN + ["--exact", "bed_cmd::tests::from_without_a_seal_is_refused"],
    ),
    (
        "f1-outside: a seal inside the bed folder it would hold is refused (seal_outside)",
        [(BIND, "    if file.starts_with(&bed) {", "    if false && file.starts_with(&bed) {")],
        ["cargo", "test", "--no-fail-fast", "-p", "simpa-core", "--lib", "--test", "bed_binding", "--",
         "a_seal_inside_its_bed_folder_is_refused",
         "the_seal_of_a_bed_this_process_ran_holds_its_runs_for_a_re_read"],
    ),
    (
        "f2-final-a: a run its seal does not name is refused (read_in's check alone reverted: bind's rule still refuses it, by another message)",
        [
            (
                RS,
                """    let sealed = seal
        .run(&p.key)
        .ok_or_else(|| unsealed(&folder, seal, &p.key))?;
    read_folder(dir, &folder, p, None, Some(sealed))""",
                """    let sealed = seal.run(&p.key);
    read_folder(dir, &folder, p, None, sealed)""",
            )
        ],
        BINDING + ["--exact", "a_run_its_seal_does_not_name_is_refused"],
    ),
    (
        "f2-final-b: a run its seal does not name is refused (read_in's check and bind's rule both reverted: accepted on run.json)",
        [
            (
                RS,
                """    let sealed = seal
        .run(&p.key)
        .ok_or_else(|| unsealed(&folder, seal, &p.key))?;
    read_folder(dir, &folder, p, None, Some(sealed))""",
                """    let sealed = seal.run(&p.key);
    read_folder(dir, &folder, p, None, sealed)""",
            ),
            (
                BIND,
                "    if made.is_none() && sealed.is_none() {",
                "    if made.is_none() && sealed.is_none() && outputs.is_none() {",
            ),
        ],
        BINDING + ["--exact", "a_run_its_seal_does_not_name_is_refused"],
    ),
    (
        "d1-reads: two seeds that read the same T30 at a receiver are refused (refuse_shared_outputs)",
        [(RS, "            read.entry((i, key)).or_default().push(*s);", "            let _ = (i, key);")],
        CORE_LIB
        + [
            "--exact",
            "bed::tests::a_seed_reading_another_seeds_t30_does_not_pass_the_bed",
            "bed::tests::ten_seeds_reading_one_seeds_t30_do_not_pass_the_bed",
        ],
    ),
    (
        "f4-case: a seal is for its bed folder by the name the file system stores (for_bed)",
        [
            (
                BIND,
                """        let stored = std::fs::canonicalize(from)
            .map_err(|e| format!("the bed folder {}: {e}", from.display()))?;
        let name = stored.file_name().and_then(|n| n.to_str()).unwrap_or("");""",
                """        let name = from.file_name().and_then(|n| n.to_str()).unwrap_or("");""",
            )
        ],
        CORE_LIB + ["--exact", "bed::bind::tests::a_seal_is_for_its_bed_folder_by_the_name_the_file_system_stores"],
    ),
    (
        "f8-git: meta.git_commit is the work tree's when the bed started (git_meta)",
        [(CMD, "    (start.0, start.1, end.0, end.1)", """    let _ = start;
    (end.0.clone(), end.1, end.0, end.1)""")],
        CLI_BIN + ["--exact", "bed_cmd::tests::the_work_tree_is_recorded_as_it_was_when_the_bed_started"],
    ),
    (
        "r2-receivers: only a receiver's file shared by two seeds refuses them (refuse_shared_outputs)",
        [(RS, "            if is_receiver_file(path) {", "            if true || is_receiver_file(path) {")],
        ["cargo", "test", "--no-fail-fast", "-p", "simpa-core", "--lib", "--test", "bed_binding", "--",
         "only_a_receivers_file_shared_by_two_seeds_refuses_them",
         "seeds_whose_particle_statistics_coincide_are_not_refused"],
    ),
]


def run(cmd):
    t0 = time.time()
    p = subprocess.run(cmd, cwd=WT, env=ENV, capture_output=True, text=True, encoding="utf-8", errors="replace")
    return p.returncode, p.stdout + p.stderr, time.time() - t0


def keep(text):
    """The lines that say what happened: results, panics, assertion messages, compile errors."""
    out = []
    lines = text.splitlines()
    for i, line in enumerate(lines):
        if i > 0 and "panicked at" in lines[i - 1]:
            out.append(line)
        elif (
            line.startswith("test ")
            or "panicked at" in line
            or line.startswith("test result")
            or "assertion" in line
            or line.startswith("  left")
            or line.startswith(" right")
            or line.startswith("failures:")
            or line.startswith("    ")
            and "::" in line
            or line.startswith("error")
            or line.startswith("bed_")
            or "bed_run" in line
            or "bed_seed" in line
            or "did not fail" in line
        ):
            out.append(line)
    return "\n".join(out[:60])


def main():
    log = open(sys.argv[1], "a", encoding="utf-8", newline="\n")
    only = sys.argv[2:]
    for name, edits, cmd in CASES:
        if only and not any(name.startswith(o) for o in only):
            continue
        originals = {}
        log.write(f"==== {name}\n")
        head = subprocess.run(["git", "rev-parse", "--short", "HEAD"], cwd=WT, capture_output=True, text=True).stdout.strip()
        log.write(f"tree: HEAD {head} and the working tree as it stands\n")
        reverted_files = {}
        for rel, old, new in edits:
            path = os.path.join(WT, rel)
            if path not in originals:
                originals[path] = open(path, "rb").read()
                reverted_files[path] = originals[path].decode("utf-8").replace("\r\n", "\n")
            text = reverted_files[path]
            assert text.count(old) == 1, (name, rel, "the fix's text is not there once")
            reverted_files[path] = text.replace(old, new)
        for path, reverted in reverted_files.items():
            text = originals[path].decode("utf-8").replace("\r\n", "\n")
            shown = os.path.relpath(path, WT).replace(os.sep, "/")
            log.write(f"revert in {shown}:\n")
            log.writelines(difflib.unified_diff(text.splitlines(True), reverted.splitlines(True), "fixed", "reverted", n=1))
        log.write(f"command: {' '.join(cmd)}\n")
        try:
            for path, reverted in reverted_files.items():
                crlf = b"\r\n" in originals[path]
                open(path, "wb").write((reverted.replace("\n", "\r\n") if crlf else reverted).encode("utf-8"))
            code, text_out, secs = run(cmd)
        finally:
            for path, original in originals.items():
                open(path, "wb").write(original)
        restored = all(open(p, "rb").read() == o for p, o in originals.items())
        log.write(f"--- with the revert: exit {code} ({secs:.0f} s)\n{keep(text_out)}\n")
        log.write(f"--- restored byte for byte: {restored}\n")
        code2, text_out2, secs2 = run(cmd)
        log.write(f"--- after the restore: exit {code2} ({secs2:.0f} s)\n{keep(text_out2)}\n\n")
        log.flush()
        verdict = "FAILED then PASSED" if code != 0 and code2 == 0 and restored else "UNEXPECTED"
        print(f"{name}: with revert exit {code}, restored {restored}, after exit {code2}: {verdict}", flush=True)
    log.close()


if __name__ == "__main__":
    main()
