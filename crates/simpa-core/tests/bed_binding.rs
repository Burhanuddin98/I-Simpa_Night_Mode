//! M8b tamper, round 2: the bed's binding of a run's files to its record, on a real run made by
//! this build (`docs/investigations/2026-09-30-m8b-tamper/FIXES.md`). Each test runs M8a's TCR
//! run `5x4x3-a0.2-tcr-air-off` through `bed::run::run_one`, with the M1 solvers
//! (`$SIMPA_SOLVERS_DIR`, see `common/paths.rs`; a missing build panics, nothing skips), into a
//! scratch folder of its own that is removed when it passes.

#[allow(dead_code)]
#[path = "common/paths.rs"]
mod paths;
#[allow(dead_code)]
#[path = "common/scratch.rs"]
mod scratch;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use simpa_core::bed::bind::{self, SEAL_VERSION, Seal, SealedFile, SealedRun};
use simpa_core::bed::file::BedFile;
use simpa_core::bed::run::{self, Exes, Planned, Read, RunKey};
use simpa_core::run::manifest::hash_tree;

const TCR: &str = "5x4x3-a0.2-tcr-air-off";

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

/// A bed folder `<scratch>/<label>/20260930T000000Z` holding M8a's TCR run `5x4x3-a0.2` made
/// by this build, read as the bed reads it as it ends; returns the folder, the plan and the
/// run's folder under it (`runs/<id>/s1`).
fn fresh_tcr(label: &str) -> (PathBuf, Planned, PathBuf) {
    let root = scratch::fresh("bed_binding", label).join("20260930T000000Z");
    let p = planned(&tcr_key(TCR));
    let r = run::run_one(&p, &root, &exes()).unwrap_or_else(|e| panic!("the TCR run: {e}"));
    let Read::Tcr(t) = &r else {
        panic!("not a TCR read")
    };
    assert_eq!(t.info.status, "OK", "{:?}", t.info.reasons);
    assert_eq!(run::check_planned(&p, &r), Ok(()));
    let dir = root.join(p.key.dir());
    (root, p, dir)
}

/// The run's folder `dir` as a seal holds it.
fn sealed_run(dir: &Path) -> SealedRun {
    let folder = simpa_core::bed::read::find_run_folder(dir).unwrap();
    SealedRun {
        run_folder: folder.file_name().unwrap().to_str().unwrap().to_string(),
        files: hash_tree(dir, &[])
            .unwrap()
            .into_iter()
            .map(|f| SealedFile(f.path, f.size, f.sha256))
            .collect(),
    }
}

/// A seal of the bed folder `root` holding `runs` (by `runs/<cell>/s<seed>`), its totals added
/// up, checked.
fn seal_of(root: &Path, runs: BTreeMap<String, SealedRun>) -> Seal {
    let (files, bytes) = runs
        .values()
        .flat_map(|r| &r.files)
        .fold((0, 0), |(n, b), f| (n + 1, b + f.1));
    let s = Seal {
        seal_version: SEAL_VERSION,
        bed: root.file_name().unwrap().to_str().unwrap().to_string(),
        sealed_from: root.display().to_string(),
        sealed_utc: "2026-09-30T00:00:00Z".into(),
        tool: "tests/bed_binding.rs".into(),
        why: "a test".into(),
        provenance: BTreeMap::new(),
        report_json_sha256: "0".repeat(64),
        summary_json_sha256: "0".repeat(64),
        files,
        bytes,
        runs,
    };
    s.check().unwrap();
    s
}

/// M8b round 2, `VERIFY-adversarial-1.md` finding 2: with a seal given, a run the seal does not
/// name was bound by its own `run.json` alone, so gate C's extension seeds (11 to 20), read from
/// the `--from` folder with the same seal, would escape it. A seal holds its whole bed: a run it
/// does not name is refused `bed_run_unbound`, whatever its `run.json` records.
#[test]
fn a_run_its_seal_does_not_name_is_refused() {
    let (root, p, dir) = fresh_tcr("unnamed");
    let key = bind::seal_key(&p.key);
    // The seal naming the run: bound by both records, read.
    let seal = seal_of(&root, [(key.clone(), sealed_run(&dir))].into());
    let r = run::read_in(&dir, &p, Some(&seal)).unwrap();
    let Read::Tcr(t) = &r else { unreachable!() };
    assert_eq!(t.info.bound_by.as_deref(), Some("run.json and seal"));
    // A seal of the same bed that names only another run (α 0.4's): this run, its run.json
    // recording every file it left, is refused before anything is read.
    let other = bind::seal_key(&tcr_key("5x4x3-a0.4-tcr-air-off"));
    let seal = seal_of(&root, [(other, sealed_run(&dir))].into());
    let e = run::read_in(&dir, &p, Some(&seal)).unwrap_err();
    assert!(e.starts_with(bind::RUN_UNBOUND), "{e}");
    assert!(e.contains(&format!("names no run {key}")), "{e}");
}
