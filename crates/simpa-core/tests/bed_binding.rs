//! M8b tamper, round 2: the bed's binding of a run's files to its record, on real runs made by
//! this build (`docs/investigations/2026-09-30-m8b-tamper/FIXES.md`). Each test runs M8a's TCR
//! run `5x4x3-a0.2-tcr-air-off` through `bed::run`, with the M1 solvers (`$SIMPA_SOLVERS_DIR`,
//! see `common/paths.rs`; a missing build panics, nothing skips), into a scratch folder of its own
//! that is removed when it passes.

#[allow(dead_code)]
#[path = "common/paths.rs"]
mod paths;
#[allow(dead_code)]
#[path = "common/scratch.rs"]
mod scratch;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use simpa_core::bed::bind::{self, Seal, SealedFile, SealedRun};
use simpa_core::bed::file::BedFile;
use simpa_core::bed::read::{find_run_folder, manifest_of};
use simpa_core::bed::run::{self, Exes, Planned, Read, RunKey};
use simpa_core::run::manifest::{FILE_NAME as RUN_JSON, hash_tree};

const TCR: &str = "5x4x3-a0.2-tcr-air-off";
const BED: &str = "20260930T000000Z";

fn exes() -> Exes {
    let f = |n: &str| paths::solver_exe(n);
    Exes {
        spps: f("spps.exe"),
        tcr: f("classicalTheory.exe"),
        tetgen: f("tetgen.exe"),
        preprocess: f("preprocess.exe"),
    }
}

fn planned(key: &RunKey) -> Planned {
    let (runs, _) = run::plan(&BedFile::m8a(), None).unwrap();
    runs.into_iter().find(|p| &p.key == key).unwrap()
}

fn tcr_key(id: &str) -> RunKey {
    RunKey::Tcr { id: id.into() }
}

fn info(r: &Read) -> &simpa_core::bed::read::RunInfo {
    match r {
        Read::Spps(s) => &s.info,
        Read::Tcr(t) => &t.info,
    }
}

/// A scratch folder of the test's own, and in it the bed folder `20260930T000000Z`, empty.
fn bed_root(label: &str) -> (PathBuf, PathBuf) {
    let own = scratch::fresh("bed_binding", label);
    let root = own.join(BED);
    (own, root)
}

/// M8a's TCR run `5x4x3-a0.2` made by this build under the bed folder `root`, read as the bed
/// reads a run as it ends ([`run::run_one`]); returns the plan, the run's folder under the bed
/// (`runs/<id>/s1`) and the read.
fn fresh_tcr(root: &Path) -> (Planned, PathBuf, Read) {
    let p = planned(&tcr_key(TCR));
    let r = run::run_one(&p, root, &exes()).unwrap_or_else(|e| panic!("the TCR run: {e}"));
    let Read::Tcr(t) = &r else {
        panic!("not a TCR read")
    };
    assert_eq!(t.info.status, "OK", "{:?}", t.info.reasons);
    assert_eq!(run::check_planned(&p, &r), Ok(()));
    let dir = root.join(p.key.dir());
    (p, dir, r)
}

/// The run's folder `dir` as a seal holds it, hashed from disk.
fn sealed_run(dir: &Path) -> SealedRun {
    let folder = find_run_folder(dir).unwrap();
    SealedRun {
        run_folder: folder.file_name().unwrap().to_str().unwrap().to_string(),
        files: hash_tree(dir, &[])
            .unwrap()
            .into_iter()
            .map(|f| SealedFile(f.path, f.size, f.sha256))
            .collect(),
    }
}

/// A seal of the bed folder `root` holding `runs`, checked, written to `at` and loaded back.
fn seal_at(root: &Path, runs: BTreeMap<String, SealedRun>, at: &Path) -> Seal {
    let mut s = Seal::of_runs(root.file_name().unwrap().to_str().unwrap(), runs);
    s.sealed_from = root.display().to_string();
    s.sealed_utc = "2026-09-30T00:00:00Z".into();
    s.tool = "tests/bed_binding.rs".into();
    s.why = "a test".into();
    s.report_json_sha256 = "0".repeat(64);
    s.summary_json_sha256 = "0".repeat(64);
    s.check().unwrap();
    std::fs::write(at, s.to_json()).unwrap();
    let (loaded, _) = Seal::load(at).unwrap();
    assert_eq!(loaded, s);
    loaded
}

/// Copies the folder `from` to `to`, file by file.
fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let dst = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_tree(&e.path(), &dst);
        } else {
            std::fs::copy(e.path(), dst).unwrap();
        }
    }
}

/// U1's tamper on the run in `dir`: byte 17 of `Main results.gabe` set to another value, and
/// its `run.json` rewritten, by this build's own writer, with `outputs` naming the files as they
/// now are. Returns the byte's old and new values.
fn forge(dir: &Path) -> (u8, u8) {
    let folder = find_run_folder(dir).unwrap();
    let gabe = folder.join("solve").join("Main results.gabe");
    let mut bytes = std::fs::read(&gabe).unwrap();
    let old = bytes[17];
    bytes[17] = old.wrapping_add(1);
    std::fs::write(&gabe, &bytes).unwrap();
    let mut m = manifest_of(&folder).unwrap();
    m.outputs = Some(hash_tree(&folder, &[RUN_JSON]).unwrap());
    std::fs::write(folder.join(RUN_JSON), m.to_json()).unwrap();
    (old, bytes[17])
}

/// M8b round 2, `VERIFY-adversarial-1.md` finding 2: with a seal given, a run the seal does not
/// name was bound by its own `run.json` alone, so gate C's extension seeds (11 to 20), read from
/// the `--from` folder with the same seal, would escape it. A seal holds its whole bed: a run it
/// does not name is refused `bed_run_unbound`, whatever its `run.json` records.
#[test]
fn a_run_its_seal_does_not_name_is_refused() {
    let (own, root) = bed_root("unnamed");
    let (p, dir, _) = fresh_tcr(&root);
    let key = bind::seal_key(&p.key);
    // The seal naming the run: bound by both records, read.
    let seal = seal_at(
        &root,
        [(key.clone(), sealed_run(&dir))].into(),
        &own.join("seal-names-it.json"),
    );
    let r = run::read_in(&dir, &p, &seal).unwrap();
    assert_eq!(info(&r).bound_by.as_deref(), Some("run.json and seal"));
    // A seal of the same bed that names only another run (α 0.4's): this run, its run.json
    // recording every file it left, is refused before anything is read.
    let other = bind::seal_key(&tcr_key("5x4x3-a0.4-tcr-air-off"));
    let seal = seal_at(
        &root,
        [(other, sealed_run(&dir))].into(),
        &own.join("seal-names-another.json"),
    );
    let e = run::read_in(&dir, &p, &seal).unwrap_err();
    assert!(e.starts_with(bind::RUN_UNBOUND), "{e}");
    assert!(e.contains(&format!("names no run {key}")), "{e}");
}

/// M8b round 2, `VERIFY-adversarial-1.md` finding 1 (U1): a run was bound to the `run.json` in
/// its own folder, so an output changed and `run.json` rewritten to match was accepted. A run
/// this process made is bound to what it made, from memory (`run::made_record`): the same
/// tamper, made between the run's end and its read, is refused `bed_run_files_changed`, naming
/// `run.json` and the output.
#[test]
fn a_run_this_process_made_is_bound_to_what_it_made_not_to_its_run_json() {
    let (_, root) = bed_root("made");
    let p = planned(&tcr_key(TCR));
    let (dir, report) = run::launch(&p, &root, &exes()).unwrap();
    // As the run left it: bound by both, and what was made is the folder as hashed from disk.
    let r = run::read_fresh(&dir, &report, &p).unwrap();
    assert_eq!(
        info(&r).bound_by.as_deref(),
        Some("run.json and this process")
    );
    let made = run::made_record(&p, &report).unwrap();
    assert_eq!(made, sealed_run(&dir));
    assert_eq!(info(&r).made.as_ref(), Some(&made));
    assert_eq!(run::check_planned(&p, &r), Ok(()));
    // U1's tamper: one byte of an output changed, run.json forged to name it.
    let (old, new) = forge(&dir);
    assert_ne!(old, new);
    let e = run::read_fresh(&dir, &report, &p).unwrap_err();
    assert!(e.starts_with(bind::FILES_CHANGED), "{e}");
    assert!(
        e.contains(&format!("{}/{RUN_JSON} has", made.run_folder)),
        "{e}"
    );
    assert!(e.contains("solve/Main results.gabe has 2523 bytes"), "{e}");
    assert!(e.contains("what this process made"), "{e}");
    assert!(!e.contains("run.json's outputs"), "{e}");
}

/// M8b round 2: the seal of a bed this process ran (`simpa bed` writes it from `RunInfo::made`)
/// holds its runs for a re-read: taken from outside the bed folder, it binds the run as it was
/// made, and refuses it once tampered, forged `run.json` and all. The same seal refuses a
/// renamed copy of the bed, and the tampered run in it; and a copy of it inside the bed folder
/// is refused.
#[test]
fn the_seal_of_a_bed_this_process_ran_holds_its_runs_for_a_re_read() {
    let (own, root) = bed_root("fresh-seal");
    let (p, dir, r) = fresh_tcr(&root);
    let key = bind::seal_key(&p.key);
    let made = info(&r).made.clone().expect("what this process made");
    let at = own.join("outputs-seal.json");
    let seal = seal_at(&root, [(key.clone(), made)].into(), &at);
    seal.for_bed(&root).unwrap();
    bind::seal_outside(&root, &at).unwrap();
    let again = run::read_in(&dir, &p, &seal).unwrap();
    assert_eq!(info(&again).bound_by.as_deref(), Some("run.json and seal"));
    assert_eq!(run::check_planned(&p, &again), Ok(()));
    // The same seal, copied into the bed folder: refused there.
    let inside = root.join("outputs-seal.json");
    std::fs::copy(&at, &inside).unwrap();
    let e = bind::seal_outside(&root, &inside).unwrap_err();
    assert!(e.contains("is inside the bed folder"), "{e}");
    // A renamed copy of the bed, U1's tamper in it (VERIFY-adversarial-1.md, 2b): the seal is
    // not of that folder, and the run in it is not what the seal holds.
    let renamed = own.join("20260930T000001Z");
    copy_tree(&root, &renamed);
    let moved = renamed.join(p.key.dir());
    forge(&moved);
    let e = seal.for_bed(&renamed).unwrap_err();
    assert!(e.contains(&format!("the seal is of the bed {BED}")), "{e}");
    let e = run::read_in(&moved, &p, &seal).unwrap_err();
    assert!(e.starts_with(bind::FILES_CHANGED), "{e}");
    assert!(e.contains("run.json and seal record"), "{e}");
    assert!(e.contains("solve/Main results.gabe has 2523 bytes"), "{e}");
    // And in place.
    forge(&dir);
    let e = run::read_in(&dir, &p, &seal).unwrap_err();
    assert!(e.starts_with(bind::FILES_CHANGED), "{e}");
}
